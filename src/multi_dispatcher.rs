use std::{
    hash::Hash,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use async_broadcast::{InactiveReceiver, Receiver, Sender};
use async_stream::stream;
use futures::{lock::Mutex, ready, Stream, StreamExt as _};
use kube::{
    api::{DynamicObject, GroupVersionKind},
    runtime::{
        reflector::{store::Writer, Lookup, Store},
        watcher::{Event, Result},
    },
    Resource,
};
use pin_project::pin_project;
use serde::de::DeserializeOwned;

#[derive(Clone)]
pub struct MultiDispatcher {
    dispatch_tx: Sender<Event<DynamicObject>>,
    // An inactive reader that prevents the channel from closing until the
    // writer is dropped.
    _dispatch_rx: InactiveReceiver<Event<DynamicObject>>,
}

impl MultiDispatcher {
    #[must_use]
    pub fn new(buf_size: usize) -> Self {
        // Create a broadcast (tx, rx) pair
        let (mut dispatch_tx, dispatch_rx) = async_broadcast::broadcast(buf_size);
        // The tx half will not wait for any receivers to be active before
        // broadcasting events. If no receivers are active, events will be
        // buffered.
        dispatch_tx.set_await_active(false);
        Self {
            dispatch_tx,
            _dispatch_rx: dispatch_rx.deactivate(),
        }
    }

    /// Return a handle to a typed subscriber
    ///
    /// Multiple subscribe handles may be obtained, by either calling
    /// `subscribe` multiple times, or by calling `clone()`
    ///
    /// This function returns a `Some` when the [`Writer`] is constructed through
    /// [`Writer::new_shared`] or [`store_shared`], and a `None` otherwise.
    #[must_use]
    pub fn subscribe<K>(&self) -> (TypedReflectHandle<K>, Store<K>)
    where
        K: Resource + Clone + DeserializeOwned,
        K::DynamicType: Eq + Clone + Hash + Default,
    {
        let sub = TypedReflectHandle::new(self.dispatch_tx.new_receiver());
        let reader = sub.reader();
        (sub, reader)
    }

    /// Broadcast an event to any downstream listeners subscribed on the store
    pub(crate) async fn broadcast_event(&mut self, event: &Event<DynamicObject>) {
        match event {
            // Broadcast stores are pre-initialized
            Event::InitDone => {}
            ev => {
                let _ = self.dispatch_tx.broadcast_direct(ev.clone()).await;
            }
        }
    }
}

/// `BroadcastStream` allows to stream shared list of dynamic objects,
/// sources of which can be changed at any moment.
pub struct BroadcastStream<W> {
    pub stream: Arc<Mutex<W>>,
}

impl<W> Clone for BroadcastStream<W> {
    fn clone(&self) -> Self {
        Self {
            stream: self.stream.clone(),
        }
    }
}

impl<W> BroadcastStream<W>
where
    W: Stream<Item = Result<Event<DynamicObject>>> + Unpin,
{
    pub fn new(stream: Arc<Mutex<W>>) -> Self {
        Self { stream }
    }
}

impl<W> Stream for BroadcastStream<W>
where
    W: Stream<Item = Result<Event<DynamicObject>>> + Unpin,
{
    type Item = W::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(mut stream) = self.stream.try_lock() {
            return stream.poll_next_unpin(cx);
        }

        Poll::Pending
    }
}

/// A handle to a shared dynamic object stream
///
/// [`TypedReflectHandle`]s are created by calling [`subscribe()`] on a [`TypedDispatcher`],
/// Each shared stream reader should be polled independently and driven to readiness
/// to avoid deadlocks. When the [`TypedDispatcher`]'s buffer is filled, backpressure
/// will be applied on the root stream side.
///
/// When the root stream is dropped, or it ends, all [`TypedReflectHandle`]s
/// subscribed to the shared stream will also terminate after all events yielded by
/// the root stream have been observed. This means [`TypedReflectHandle`] streams
/// can still be polled after the root stream has been dropped.
#[pin_project]
pub struct TypedReflectHandle<K>
where
    K: Lookup + Clone + 'static,
    K::DynamicType: Eq + std::hash::Hash + Clone,
    K: DeserializeOwned,
{
    #[pin]
    rx: Receiver<Event<DynamicObject>>,
    store: Writer<K>,
}

impl<K> TypedReflectHandle<K>
where
    K: Lookup + Clone + 'static,
    K::DynamicType: Eq + std::hash::Hash + Clone + Default,
    K: DeserializeOwned,
{
    pub(super) fn new(rx: Receiver<Event<DynamicObject>>) -> TypedReflectHandle<K> {
        Self {
            rx,
            // Initialize a ready store by default
            store: {
                let mut store: Writer<K> = Writer::default();
                store.apply_watcher_event(&Event::InitDone);
                store
            },
        }
    }

    pub fn reader(&self) -> Store<K> {
        self.store.as_reader()
    }
}

pub fn gvk(obj: &DynamicObject) -> Option<GroupVersionKind> {
    let gvk = obj.types.clone()?;
    gvk.try_into().ok()
}

pub fn typed_gvk<K: Resource>(dt: &K::DynamicType) -> GroupVersionKind {
    GroupVersionKind::gvk(&K::group(dt), &K::version(dt), &K::kind(dt))
}

impl<K> Stream for TypedReflectHandle<K>
where
    K: Resource + Clone + 'static,
    K::DynamicType: Eq + std::hash::Hash + Clone + Default,
    K: DeserializeOwned,
{
    type Item = Arc<K>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        loop {
            return match ready!(this.rx.as_mut().poll_next(cx)) {
                Some(event) => {
                    let obj = match event {
                        Event::InitApply(obj) | Event::Apply(obj)
                            if gvk(&obj) == Some(typed_gvk::<K>(&Default::default())) =>
                        {
                            obj.try_parse::<K>()
                                .ok()
                                .inspect(|o| {
                                    this.store.apply_watcher_event(&Event::Apply(o.clone()));
                                })
                                .map(Arc::new)
                        }
                        Event::Delete(obj)
                            if gvk(&obj) == Some(typed_gvk::<K>(&Default::default())) =>
                        {
                            obj.try_parse::<K>()
                                .ok()
                                .inspect(|o| {
                                    this.store.apply_watcher_event(&Event::Delete(o.clone()));
                                })
                                .map(Arc::new)
                        }
                        _ => None,
                    };

                    // Skip propagating all objects which do not belong to the cache
                    if obj.is_none() {
                        continue;
                    }

                    Poll::Ready(obj)
                }
                None => Poll::Ready(None),
            };
        }
    }
}

pub fn broadcaster<W>(
    mut writer: MultiDispatcher,
    mut broadcast: BroadcastStream<W>,
) -> impl Stream<Item = W::Item>
where
    W: Stream<Item = Result<Event<DynamicObject>>> + Unpin,
{
    stream! {
        while let Some(event) = broadcast.next().await {
            match event {
                Ok(ev) => {
                    writer.broadcast_event(&ev).await;
                    yield Ok(ev);
                },
                Err(ev) => yield Err(ev)
            }
        }
    }
}
