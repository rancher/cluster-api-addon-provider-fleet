use crate::api::comparable::ResourceDiff;
use crate::api::fleet_addon_config::FleetAddonConfig;
use crate::controllers::PatchError;
use crate::metrics::Diagnostics;
use crate::multi_dispatcher::{BroadcastStream, MultiDispatcher, typed_gvk};
use crate::{Error, Metrics, telemetry};
use chrono::Utc;

use futures::Stream;
use futures::stream::SelectAll;
use k8s_openapi::{ClusterResourceScope, NamespaceResourceScope};

use kube::api::{DynamicObject, Patch, PatchParams, PostParams};

use kube::runtime::events::{Event, EventType};
use kube::runtime::{finalizer, watcher};

use kube::{api::Api, client::Client, runtime::controller::Action};

use serde::Serialize;
use serde::de::DeserializeOwned;
use tracing::field::display;

use std::fmt::Debug;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{Barrier, RwLock};
use tracing::{self, Span, debug, info, instrument};

use super::{
    BundleResult, ConfigFetchResult, GetOrCreateError, GetOrCreateResult, PatchResult, SyncError,
};

pub static FLEET_FINALIZER: &str = "fleet.addons.cluster.x-k8s.io";

pub(crate) type DynamicStream = SelectAll<
    Pin<Box<dyn Stream<Item = Result<watcher::Event<DynamicObject>, watcher::Error>> + Send>>,
>;

// Context for the reconciler
#[derive(Clone)]
pub struct Context {
    /// Kubernetes client
    pub client: Client,
    /// Diagnostoics read by the web server
    pub diagnostics: Arc<RwLock<Diagnostics>>,
    /// Prom metrics
    pub metrics: Metrics,
    // Dispatcher for dynamic resource controllers
    pub dispatcher: MultiDispatcher,
    // shared stream of dynamic events
    pub stream: BroadcastStream<DynamicStream>,
    // k8s minor version
    pub version: u32,
    // Controller readiness barrier
    pub barrier: Arc<Barrier>,
}

#[instrument(skip_all, fields(name = res.name_any(), namespace = res.namespace(), api_version = typed_gvk::<R>(&()).api_version(), kind = R::kind(&()).to_string()), err)]
pub(crate) async fn get_or_create<R>(ctx: Arc<Context>, res: &R) -> GetOrCreateResult<Action>
where
    R: std::fmt::Debug,
    R: Clone + Serialize + DeserializeOwned,
    R: kube::Resource<DynamicType = (), Scope = NamespaceResourceScope>,
    R: kube::ResourceExt + GetApi,
{
    let api = R::get_api(ctx.client.clone(), res.get_namespace());

    let obj = api
        .get_metadata_opt(res.name_any().as_str())
        .await
        .map_err(GetOrCreateError::Lookup)?;

    if obj.is_some() {
        return Ok(Action::await_change());
    }

    api.create(&PostParams::default(), res)
        .await
        .map_err(GetOrCreateError::Create)?;

    info!("Created object");
    match ctx
        .diagnostics
        .read()
        .await
        .recorder(ctx.client.clone())
        // Record object creation
        .publish(
            &Event {
                type_: EventType::Normal,
                reason: "Created".into(),
                note: Some(format!(
                    "Created fleet object `{}` in `{}`",
                    res.name_any(),
                    res.namespace().unwrap_or_default()
                )),
                action: "Creating".into(),
                secondary: None,
            },
            &res.object_ref(&()),
        )
        .await
    {
        // Ignore forbidden errors on namespace deletion
        Err(kube::Error::Api(e)) if &e.reason == "Forbidden" => (),
        e => e?,
    }

    Ok(Action::await_change())
}

#[instrument(skip_all, fields(name = res.name_any(), namespace = res.namespace(), api_version = typed_gvk::<R>(&()).api_version(), kind = R::kind(&()).to_string()), err)]
pub(crate) async fn patch<R>(
    ctx: Arc<Context>,
    res: &mut R,
    pp: &PatchParams,
) -> PatchResult<Action>
where
    R: Clone + Serialize + DeserializeOwned + Debug,
    R: kube::Resource<DynamicType = ()>,
    R: ResourceDiff + GetApi,
{
    let api = R::get_api(ctx.client.clone(), res.get_namespace());
    res.meta_mut().managed_fields = None;

    // Perform patch after comparison
    if let Some(existing) = api
        .get_opt(&res.name_any())
        .await
        .map_err(PatchError::Get)?
    {
        if !res.diff(&existing) {
            return Ok(Action::await_change());
        }
    }

    api.patch(&res.name_any(), pp, &Patch::Apply(&res))
        .await
        .map_err(PatchError::Patch)?;

    info!("Updated object");
    match ctx
        .diagnostics
        .read()
        .await
        .recorder(ctx.client.clone())
        // Record object creation
        .publish(
            &Event {
                type_: EventType::Normal,
                reason: "Updated".into(),
                note: Some(format!(
                    "Updated `{}/{}` object `{}` in `{}`",
                    typed_gvk::<R>(&()).api_version(),
                    R::kind(&()),
                    res.name_any(),
                    res.namespace().unwrap_or("cluster scope".to_string())
                )),
                action: "Creating".into(),
                secondary: None,
            },
            &res.object_ref(&()),
        )
        .await
    {
        // Ignore forbidden errors on namespace deletion
        Err(kube::Error::Api(e)) if &e.reason == "Forbidden" => (),
        e => e?,
    }

    Ok(Action::await_change())
}

/// Helper trait for getting [`kube::Api`] instances for a Kubernetes resource's scope
///
/// Not intended to be implemented manually, it is blanket-implemented for all types that implement [`Resource`]
/// for either the [namespace](`NamespaceResourceScope`) or [cluster](`ClusterResourceScope`) scopes.
///
/// Source: <https://github.com/stackabletech/operator-rs/blob/61c8a4f5a0c152dbcafadea6e0d0b82b59c02a32/crates/stackable-operator/src/client.rs#L559C1-L643C1>
/// Implemented locally to avoid a dependency on the external crate
pub trait GetApi: kube::Resource + Sized {
    /// The namespace type for `Self`'s scope.
    ///
    /// This will be [`str`] for namespaced resource, and [`()`] for cluster-scoped resources.
    type Namespace: ?Sized;
    /// Get a [`kube::Api`] for `Self`'s native scope..
    fn get_api(client: kube::Client, ns: &Self::Namespace) -> kube::Api<Self>
    where
        Self::DynamicType: Default;
    /// Get the namespace of `Self`.
    fn get_namespace(&self) -> &Self::Namespace;
}

impl<K> GetApi for K
where
    K: kube::Resource,
    (K, K::Scope): GetApiImpl<Resource = K>,
{
    type Namespace = <(K, K::Scope) as GetApiImpl>::Namespace;

    fn get_api(client: kube::Client, ns: &Self::Namespace) -> kube::Api<Self>
    where
        Self::DynamicType: Default,
    {
        <(K, K::Scope) as GetApiImpl>::get_api(client, ns)
    }

    fn get_namespace(&self) -> &Self::Namespace {
        <(K, K::Scope) as GetApiImpl>::get_namespace(self)
    }
}

#[doc(hidden)]
// Workaround for https://github.com/rust-lang/rust/issues/20400
pub trait GetApiImpl {
    type Resource: kube::Resource;
    type Namespace: ?Sized;
    fn get_api(client: kube::Client, ns: &Self::Namespace) -> kube::Api<Self::Resource>
    where
        <Self::Resource as kube::Resource>::DynamicType: Default;
    fn get_namespace(res: &Self::Resource) -> &Self::Namespace;
}

impl<K> GetApiImpl for (K, NamespaceResourceScope)
where
    K: kube::Resource<Scope = NamespaceResourceScope>,
{
    type Namespace = str;
    type Resource = K;

    fn get_api(client: kube::Client, ns: &Self::Namespace) -> kube::Api<K>
    where
        <Self::Resource as kube::Resource>::DynamicType: Default,
    {
        Api::namespaced(client, ns)
    }

    fn get_namespace(res: &Self::Resource) -> &Self::Namespace {
        res.meta().namespace.as_deref().unwrap_or("default")
    }
}

impl<K> GetApiImpl for (K, ClusterResourceScope)
where
    K: kube::Resource<Scope = ClusterResourceScope>,
{
    type Namespace = ();
    type Resource = K;

    fn get_api(client: kube::Client, (): &Self::Namespace) -> kube::Api<K>
    where
        <Self::Resource as kube::Resource>::DynamicType: Default,
    {
        Api::all(client)
    }

    fn get_namespace(_res: &Self::Resource) -> &Self::Namespace {
        &()
    }
}

pub(crate) async fn fetch_config(client: Client) -> ConfigFetchResult<FleetAddonConfig> {
    Ok(Api::all(client)
        .get_opt("fleet-addon-config")
        .await?
        .unwrap_or_default())
}

pub(crate) trait FleetBundle {
    async fn sync(&mut self, ctx: Arc<Context>) -> Result<Action, impl Into<SyncError>>;
    #[allow(clippy::unused_async)]
    async fn cleanup(&mut self, _ctx: Arc<Context>) -> Result<Action, SyncError> {
        Ok(Action::await_change())
    }
}

pub(crate) trait FleetController
where
    Self: std::fmt::Debug,
    Self: Clone + Serialize + DeserializeOwned,
    Self: kube::Resource<DynamicType = (), Scope = NamespaceResourceScope>,
    Self: kube::ResourceExt,
{
    type Bundle: FleetBundle;

    #[instrument(skip_all, fields(reconcile_id, name = self.name_any(), namespace = self.namespace()), err)]
    async fn reconcile(self: Arc<Self>, ctx: Arc<Context>) -> crate::Result<Action> {
        let _current = Span::current().record("reconcile_id", display(telemetry::get_trace_id()));

        ctx.diagnostics.write().await.last_event = Utc::now();

        let api = Self::get_api(ctx.client.clone(), self.get_namespace());
        debug!("Reconciling");

        finalizer(&api, FLEET_FINALIZER, self, |event| async {
            match event {
                finalizer::Event::Apply(c) => match c.to_bundle(ctx.clone()).await? {
                    Some(mut bundle) => bundle
                        .sync(ctx)
                        .await
                        .map_err(Into::into)
                        .map_err(Into::into),
                    _ => Ok(Action::await_change()),
                },
                finalizer::Event::Cleanup(c) => c.cleanup(ctx).await,
            }
        })
        .await
        .map_err(|e| Error::FinalizerError(Box::new(e)))
    }

    async fn cleanup(&self, ctx: Arc<Context>) -> crate::Result<Action> {
        if let Some(mut bundle) = self.to_bundle(ctx.clone()).await? {
            return Ok(bundle.cleanup(ctx).await?);
        }

        match ctx
            .diagnostics
            .read()
            .await
            .recorder(ctx.client.clone())
            // Cleanup is perfomed by owner reference
            .publish(
                &Event {
                    type_: EventType::Normal,
                    reason: "DeleteRequested".into(),
                    note: Some(format!("Delete `{}`", self.name_any())),
                    action: "Deleting".into(),
                    secondary: None,
                },
                &self.object_ref(&()),
            )
            .await
        {
            // Ignore forbidden errors on namespace deletion
            Err(kube::Error::Api(e)) if &e.reason == "Forbidden" => (),
            e => e?,
        }

        Ok(Action::await_change())
    }

    async fn to_bundle(&self, ctx: Arc<Context>) -> BundleResult<Option<Self::Bundle>>;
}
