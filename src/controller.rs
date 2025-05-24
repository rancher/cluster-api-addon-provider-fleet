use crate::api::bundle_namespace_mapping::BundleNamespaceMapping;
use crate::api::capi_cluster::Cluster;
use crate::api::capi_clusterclass::ClusterClass;
use crate::api::fleet_addon_config::FleetAddonConfig;
use crate::api::fleet_cluster;
use crate::api::fleet_clustergroup::ClusterGroup;
use crate::controllers::addon_config::FleetConfig;
use crate::controllers::controller::{fetch_config, Context, DynamicStream, FleetController};
use crate::metrics::Diagnostics;
use crate::multi_dispatcher::{broadcaster, BroadcastStream, MultiDispatcher};
use crate::{Error, Metrics};

use chrono::Local;
use clap::Parser;
use futures::{Stream, StreamExt};

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::api::{Patch, PatchParams};
use kube::core::DeserializeGuard;
use kube::runtime::reflector::store::Writer;
use kube::runtime::reflector::ObjectRef;
use kube::runtime::{metadata_watcher, predicates, reflector, watcher, WatchStreamExt};
use kube::{
    api::Api,
    client::Client,
    runtime::{
        controller::{Action, Controller},
        watcher::Config,
    },
};
use kube::{Resource, ResourceExt};
use tokio::sync::Barrier;

use std::collections::BTreeMap;

use std::ops::Deref;
use std::sync::Arc;
use tokio::{sync::RwLock, time::Duration};
use tracing::{self, warn};

/// State shared between the controller and the web server
#[derive(Clone)]
pub struct State {
    /// Diagnostics populated by the reconciler
    diagnostics: Arc<RwLock<Diagnostics>>,
    /// Metrics registry
    registry: prometheus::Registry,
    metrics: Metrics,

    /// Additional flags for controller
    pub flags: Flags,

    // dispatcher
    dispatcher: MultiDispatcher,
    // shared stream of dynamic events
    stream: BroadcastStream<DynamicStream>,

    // k8s api server minor version
    pub version: u32,

    // Controller readiness barrier
    pub barrier: Arc<Barrier>,
}

#[derive(Parser, Debug, Clone, Default)]
pub struct Flags {
    /// helm install allows to select container for performing fleet chart installation
    #[arg(long)]
    pub helm_install: bool,
}

impl State {
    /// # Panics
    ///
    /// Panics if the default metrics cannot be registered with the registry.
    #[must_use]
    pub fn new(version: u32) -> Self {
        let registry = prometheus::Registry::default();
        Self {
            metrics: Metrics::default().register(&registry).unwrap(),
            registry,
            flags: Flags::parse(),
            dispatcher: MultiDispatcher::new(128),
            diagnostics: Arc::default(),
            stream: BroadcastStream::new(Arc::default()),
            version,
            barrier: Arc::new(Barrier::new(3)),
        }
    }

    /// Metrics getter
    #[must_use]
    pub fn metrics(&self) -> Vec<prometheus::proto::MetricFamily> {
        self.registry.gather()
    }

    /// State getter
    pub async fn diagnostics(&self) -> Diagnostics {
        self.diagnostics.read().await.clone()
    }

    // Create a Controller Context that can update State
    #[must_use]
    pub fn to_context(&self, client: Client) -> Arc<Context> {
        Arc::new(Context {
            client,
            metrics: self.metrics.clone(),
            diagnostics: self.diagnostics.clone(),
            dispatcher: self.dispatcher.clone(),
            stream: self.stream.clone(),
            version: self.version,
            barrier: self.barrier.clone(),
        })
    }
}

trait ControllerDefault: WatchStreamExt {
    fn default_handling<K>(self) -> impl WatchStreamExt<Item = Result<K, watcher::Error>>
    where
        K: Resource<DynamicType = ()> + 'static,
        Self: Stream<Item = Result<watcher::Event<K>, watcher::Error>> + Sized,
    {
        self.modify(|g| g.managed_fields_mut().clear())
            .touched_objects()
            .predicate_filter(predicates::resource_version)
            .default_backoff()
    }

    fn default_with_reflect<K>(
        self,
        writer: Writer<K>,
    ) -> impl WatchStreamExt<Item = Result<K, watcher::Error>>
    where
        K: Resource<DynamicType = ()> + Clone + 'static,
        Self: Stream<Item = Result<watcher::Event<K>, watcher::Error>> + Sized,
    {
        self.modify(|g| g.managed_fields_mut().clear())
            .reflect(writer)
            .touched_objects()
            .predicate_filter(predicates::resource_version)
            .default_backoff()
    }
}

impl<St: ?Sized> ControllerDefault for St where St: Stream {}

/// # Panics
///
/// Panics if the kube Client cannot be created or if the watcher stream panics unexpectedly.
pub async fn run_fleet_addon_config_controller(state: State) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");

    let config_controller = Controller::new(
        Api::<FleetAddonConfig>::all(client.clone()),
        Config::default().any_semantic(),
    )
    .watches(
        Api::<DeserializeGuard<FleetConfig>>::all(client.clone()),
        Config::default().fields("metadata.name=fleet-controller"),
        |config| config.0.ok().map(|_| ObjectRef::new("fleet-addon-config")),
    )
    .shutdown_on_signal()
    .run(
        FleetAddonConfig::reconcile_config_sync,
        error_policy,
        state.to_context(client.clone()),
    )
    .default_backoff()
    .for_each(|_| futures::future::ready(()));

    let dynamic_watches_controller = Controller::new(
        Api::<FleetAddonConfig>::all(client.clone()),
        Config::default().any_semantic(),
    )
    .shutdown_on_signal()
    .run(
        FleetAddonConfig::reconcile_dynamic_watches,
        error_policy,
        state.to_context(client.clone()),
    )
    .default_backoff()
    .for_each(|_| futures::future::ready(()));

    let watcher = broadcaster(state.dispatcher.clone(), state.stream.clone())
        .for_each(|_| futures::future::ready(()));

    // Reconcile initial state of watches
    Arc::new(
        fetch_config(client.clone())
            .await
            .expect("failed to get FleetAddonConfig resource"),
    )
    .update_watches(state.to_context(client.clone()))
    .await
    .expect("Initial dynamic watches setup to succeed");

    // Signal that this controller is ready
    state.barrier.wait().await;

    tokio::select! {
        () = watcher => {panic!("This should not happen before controllers exit")},
        _ = futures::future::join(dynamic_watches_controller, config_controller) => {}
    };
}

/// # Panics
///
/// Panics if the kube Client cannot be created or if the watcher stream panics unexpectedly.
pub async fn run_fleet_helm_controller(state: State) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");
    let (reader, writer) = reflector::store();
    let fleet_addon_config = watcher(
        Api::<FleetAddonConfig>::all(client.clone()),
        Config::default().any_semantic(),
    )
    .default_with_reflect(writer)
    .predicate_filter(predicates::generation);

    let fleet_addon_config_controller = Controller::for_stream(fleet_addon_config, reader)
        .shutdown_on_signal()
        .run(
            |obj, ctx| async move {
                let mut obj = obj.deref().clone();
                obj.metadata.managed_fields = None;
                let res = FleetAddonConfig::reconcile_helm(&mut obj, ctx.clone()).await;
                let status = obj.status.get_or_insert_default();
                let conditions = &mut status.conditions;
                let mut message = "Addon provider is ready".to_string();
                let mut status_message = "True";
                if let Err(ref e) = res {
                    message = format!("FleetAddonConfig reconcile error: {e}");
                    status_message = "False";
                }
                conditions.push(Condition {
                    last_transition_time: Time(Local::now().to_utc()),
                    message,
                    observed_generation: obj.metadata.generation,
                    reason: "Ready".into(),
                    status: status_message.into(),
                    type_: "Ready".into(),
                });

                let status = obj.status.get_or_insert_default();
                let mut uniques: BTreeMap<String, Condition> = BTreeMap::new();
                status
                    .conditions
                    .iter()
                    .for_each(|e| match uniques.get(&e.type_) {
                        Some(existing)
                            if existing.message == e.message
                                && existing.reason == e.reason
                                && existing.status == e.status
                                && existing.observed_generation == e.observed_generation => {}
                        _ => {
                            uniques.insert(e.type_.clone(), e.clone());
                        }
                    });
                status.conditions = uniques.into_values().collect();

                let api: Api<FleetAddonConfig> = Api::all(ctx.client.clone());
                let patch = api
                    .patch_status(
                        &obj.name_any(),
                        &PatchParams::apply("fleet-addon-controller").force(),
                        &Patch::Apply(obj),
                    )
                    .await;
                match res {
                    Ok(_) => match patch {
                        Ok(_) => res,
                        Err(e) => Ok(Err(e)?),
                    },
                    e => e,
                }
            },
            error_policy,
            state.to_context(client.clone()),
        )
        .default_backoff()
        .for_each(|_| futures::future::ready(()));
    tokio::join!(fleet_addon_config_controller);
}

/// Initialize the controller and shared state (given the crd is installed)
///
/// # Panics
///
/// Panics if the kube Client cannot be created.
pub async fn run_cluster_controller(state: State) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");

    let (sub, reader) = state.dispatcher.subscribe();
    let ns_controller = Controller::for_shared_stream(sub, reader)
        .shutdown_on_signal()
        .run(
            Cluster::add_namespace_dynamic_watch,
            error_policy,
            state.to_context(client.clone()),
        )
        .default_backoff()
        .for_each(|_| futures::future::ready(()));

    let fleet = metadata_watcher(
        Api::<fleet_cluster::Cluster>::all(client.clone()),
        Config::default().any_semantic(),
    )
    .default_handling();

    let groups = metadata_watcher(
        Api::<ClusterGroup>::all(client.clone()),
        Config::default()
            .labels_from(&ClusterGroup::group_selector())
            .any_semantic(),
    )
    .default_handling();

    let mappings = metadata_watcher(
        Api::<BundleNamespaceMapping>::all(client.clone()),
        Config::default().any_semantic(),
    )
    .default_handling();

    let (sub, reader) = state.dispatcher.subscribe();
    let clusters = Controller::for_shared_stream(sub, reader.clone())
        .owns_stream(fleet)
        .owns_stream(groups)
        .watches_stream(mappings, move |mapping| {
            reader
                .state()
                .into_iter()
                .filter_map(move |c: Arc<Cluster>| {
                    let in_namespace =
                        c.spec.proxy.topology.as_ref()?.class_namespace == mapping.namespace();
                    in_namespace.then_some(ObjectRef::from_obj(&*c))
                })
        })
        .shutdown_on_signal()
        .run(
            Cluster::reconcile,
            error_policy,
            state.to_context(client.clone()),
        )
        .default_backoff()
        .for_each(|_| futures::future::ready(()));

    // Signal that this controller is ready
    state.barrier.wait().await;

    tokio::join!(clusters, ns_controller);
}

/// Initialize the controller and shared state (given the crd is installed)
///
/// # Panics
///
/// Panics if the kube Client cannot be created.
pub async fn run_cluster_class_controller(state: State) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");

    let group_controller = Controller::new(
        Api::<ClusterGroup>::all(client.clone()),
        Config::default()
            .labels_from(&ClusterGroup::group_selector())
            .any_semantic(),
    )
    .shutdown_on_signal()
    .run(
        ClusterGroup::reconcile,
        error_policy,
        state.to_context(client.clone()),
    )
    .default_backoff()
    .for_each(|_| futures::future::ready(()));

    let (reader, writer) = reflector::store();
    let cluster_classes = watcher(
        Api::<ClusterClass>::all(client.clone()),
        Config::default().any_semantic(),
    )
    .default_with_reflect(writer);

    let groups = metadata_watcher(
        Api::<ClusterGroup>::all(client.clone()),
        Config::default()
            .labels_from(&ClusterGroup::group_selector())
            .any_semantic(),
    )
    .default_handling();

    let cluster_class_controller = Controller::for_stream(cluster_classes, reader)
        .owns_stream(groups)
        .shutdown_on_signal()
        .run(
            ClusterClass::reconcile,
            error_policy,
            state.to_context(client.clone()),
        )
        .default_backoff()
        .for_each(|_| futures::future::ready(()));

    // Signal that this controller is ready
    state.barrier.wait().await;

    tokio::join!(group_controller, cluster_class_controller);
}

#[allow(clippy::needless_pass_by_value)]
fn error_policy(doc: Arc<impl kube::Resource>, error: &Error, ctx: Arc<Context>) -> Action {
    warn!("reconcile failed: {:?}", error);
    ctx.metrics.reconcile_failure(doc, error);
    Action::requeue(Duration::from_secs(10))
}
