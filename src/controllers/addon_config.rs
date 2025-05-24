use base64::prelude::*;
use chrono::Local;
use futures::StreamExt as _;
use std::{fmt::Display, io, str::FromStr, sync::Arc, time::Duration};

use k8s_openapi::{
    api::core::v1::{self, ConfigMap, Endpoints, ObjectReference},
    apimachinery::pkg::apis::meta::v1::{Condition, Time},
};
use kube::{
    api::{ApiResource, DynamicObject, ObjectMeta, Patch, PatchParams, TypeMeta},
    client::scope::Namespace,
    core::object::HasSpec,
    runtime::{
        controller::Action,
        watcher::{self, Config, Event},
    },
    Api, Resource, ResourceExt,
};
use serde::{de::DeserializeOwned, ser, Deserialize, Serialize};
use serde_json::Value;
use serde_with::{serde_as, DisplayFromStr};
use thiserror::Error;
use tracing::{field::display, info, instrument, Span};

use crate::{
    api::{
        capi_cluster::Cluster,
        fleet_addon_config::{
            FeatureGates, FleetAddonConfig, FleetSettings, Install, InstallOptions, Server,
        },
    },
    telemetry,
};

use super::{
    controller::{patch, Context},
    helm::{
        self,
        install::{ChartSearch, FleetChart, HelmOperation},
    },
    PatchError,
};

#[derive(Resource, Serialize, Deserialize, Default, Clone, Debug)]
#[resource(inherit = ConfigMap)]
pub struct FleetConfig {
    #[serde(flatten, default)]
    pub types: Option<TypeMeta>,
    pub metadata: ObjectMeta,
    pub data: FleetConfigSpec,
}

#[serde_as]
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct FleetConfigSpec {
    #[serde_as(as = "DisplayFromStr")]
    pub config: FleetConfigData,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct FleetConfigData {
    #[serde(rename = "apiServerURL")]
    pub api_server_url: String,

    #[serde(rename = "apiServerCA")]
    pub api_server_ca: String,

    #[serde(flatten)]
    pub other: Value,
}

impl FromStr for FleetConfigData {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}

impl Display for FleetConfigData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&serde_json::to_string(self).map_err(ser::Error::custom)?)
    }
}

#[derive(Resource, Deserialize, Serialize, Clone, Debug)]
#[resource(inherit = ConfigMap)]
struct CertConfigMap {
    metadata: ObjectMeta,
    data: CertData,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct CertData {
    #[serde(rename = "ca.crt")]
    ca_crt: String,
}

impl FleetAddonConfig {
    #[instrument(skip_all, fields(reconcile_id, name = self.name_any(), namespace = self.namespace()))]
    pub async fn reconcile_helm(&mut self, ctx: Arc<Context>) -> crate::Result<Action> {
        let _current = Span::current().record("reconcile_id", display(telemetry::get_trace_id()));
        if let Some(requeue) = self.update_flags(ctx.clone()).await? {
            return Ok(requeue);
        }

        let chart = FleetChart {
            repo: "https://rancher.github.io/fleet-helm-charts/".into(),
            namespace: "cattle-fleet-system".into(),
            wait: true,
            update_dependency: true,
            create_namespace: true,
            bootstrap_local_cluster: self.spec.bootstrap().unwrap_or_default(),
            feature_gates: self.spec.feature_gates().cloned().unwrap_or_default(),
            version: Option::default(),
        };

        let status = self.status.get_or_insert_default();
        chart.add_repo()?.wait().await?;

        status.conditions.push(Condition {
            last_transition_time: Time(Local::now().to_utc()),
            message: format!("Repo added: {}", chart.repo),
            observed_generation: self.metadata.generation,
            reason: "RepoAdd".into(),
            status: "True".into(),
            type_: "RepoAdd".into(),
        });

        chart.update_repo()?.wait().await?;

        status.conditions.push(Condition {
            last_transition_time: Time(Local::now().to_utc()),
            message: format!("Repo updated: {}", chart.repo),
            observed_generation: self.metadata.generation,
            reason: "RepoUpdate".into(),
            status: "True".into(),
            type_: "RepoUpdate".into(),
        });

        if let Some(install) = &self.spec.install {
            if let Some(requeue) = self
                .install_fleet(FleetChart {
                    version: Some(install.install_version.clone()),
                    ..chart.clone()
                })
                .await?
            {
                return Ok(requeue);
            }
        }

        return Ok(Action::await_change());
    }

    #[instrument(skip_all, fields(reconcile_id, name = self.name_any(), namespace = self.namespace()))]
    pub async fn reconcile_config_sync(
        self: Arc<Self>,
        ctx: Arc<Context>,
    ) -> crate::Result<Action> {
        let _current = Span::current().record("reconcile_id", display(telemetry::get_trace_id()));
        let ns = Namespace::from("cattle-fleet-system");
        let mut fleet_config: FleetConfig = ctx.client.get("fleet-controller", &ns).await?;

        if let Some(server) = self.spec().config.as_ref().and_then(|c| c.server.as_ref()) {
            self.update_certificate(ctx.clone(), &mut fleet_config, server)
                .await?;
            self.update_url(ctx.clone(), &mut fleet_config, server)
                .await?;
        }

        fleet_config.meta_mut().managed_fields = None;
        fleet_config.types = Some(TypeMeta::resource::<FleetConfig>());

        let api: Api<FleetConfig> = Api::namespaced(ctx.client.clone(), "cattle-fleet-system");
        api.patch(
            &fleet_config.name_any(),
            &PatchParams::apply("addon-provider-fleet").force(),
            &Patch::Apply(&fleet_config),
        )
        .await?;

        info!("Updated fleet config map");

        Ok(Action::await_change())
    }

    #[instrument(skip_all, fields(reconcile_id, name = self.name_any(), namespace = self.namespace()))]
    pub async fn update_watches(
        self: Arc<Self>,
        ctx: Arc<Context>,
    ) -> DynamicWatcherResult<Action> {
        info!("Reconciling dynamic watches");
        let cluster_selector = self.cluster_selector()?;
        let ns_selector = self.namespace_selector()?;
        let ns_config = Config::default().labels_from(&ns_selector);
        let cluster_config = Config::default().labels_from(&cluster_selector);

        let mut stream = ctx.stream.stream.lock().await;
        stream.clear();

        if ctx.version >= 32 {
            stream.push(
                watcher::watcher(
                    Api::all_with(ctx.client.clone(), &ApiResource::erase::<Cluster>(&())),
                    cluster_config.streaming_lists(),
                )
                .boxed(),
            );

            stream.push(
                watcher::watcher(
                    Api::all_with(
                        ctx.client.clone(),
                        &ApiResource::erase::<v1::Namespace>(&()),
                    ),
                    ns_config.streaming_lists(),
                )
                .boxed(),
            );
        } else {
            stream.push(
                watcher::watcher(Api::<Cluster>::all(ctx.client.clone()), cluster_config)
                    .map(to_dynamic_event)
                    .boxed(),
            );

            stream.push(
                watcher::watcher(Api::<v1::Namespace>::all(ctx.client.clone()), ns_config)
                    .map(to_dynamic_event)
                    .boxed(),
            );
        }

        info!("Reconciled dynamic watches to match selectors: namespace={ns_selector}, cluster={cluster_selector}");
        Ok(Action::await_change())
    }

    #[instrument(skip_all, fields(reconcile_id, name = self.name_any(), namespace = self.namespace()))]
    pub async fn reconcile_dynamic_watches(
        self: Arc<Self>,
        ctx: Arc<Context>,
    ) -> crate::Result<Action> {
        let _current = Span::current().record("reconcile_id", display(telemetry::get_trace_id()));
        self.update_watches(ctx).await?;

        Ok(Action::await_change())
    }

    fn default_endpoint_lookup(endpoints: Endpoints) -> Option<String> {
        let subsets = endpoints.subsets?;
        let subnet = subsets.first()?.clone();
        let addresses = subnet.addresses?;
        let ports = subnet.ports?;
        let address = addresses.first()?.clone();
        let port = ports.first()?.clone();

        let url = address.hostname.or(Some(address.ip))?;
        let name = port.name;
        let port = port.port;
        match name {
            Some(name) => Some(format!("{name}://{url}:{port}")),
            None => Some(url),
        }
    }

    async fn update_certificate(
        &self,
        ctx: Arc<Context>,
        fleet_config: &mut FleetConfig,
        fleet_install: &Server,
    ) -> AddonConfigSyncResult<()> {
        let ns = Namespace::from("default");
        let cert_config_map: CertConfigMap = match fleet_install {
            Server::InferLocal(true) => ctx.client.get("kube-root-ca.crt", &ns).await?,
            Server::Custom(InstallOptions {
                api_server_ca_config_ref: Some(config_ref),
                ..
            }) => ctx.client.fetch(config_ref).await?,
            _ => return Ok(()),
        };

        fleet_config.data.config.api_server_ca =
            BASE64_STANDARD.encode(cert_config_map.data.ca_crt);

        Ok(())
    }

    async fn update_url(
        &self,
        ctx: Arc<Context>,
        fleet_config: &mut FleetConfig,
        fleet_install: &Server,
    ) -> AddonConfigSyncResult<()> {
        let api_server_url = match fleet_install {
            Server::InferLocal(true) => {
                if let Some(api_server_url) = {
                    let ns = Namespace::from("default");
                    FleetAddonConfig::default_endpoint_lookup(
                        ctx.client.get("kubernetes", &ns).await?,
                    )
                } {
                    api_server_url
                } else {
                    return Ok(());
                }
            }
            Server::Custom(InstallOptions {
                api_server_url: Some(api_server_url),
                ..
            }) => api_server_url.clone(),
            _ => return Ok(()),
        };

        fleet_config.data.config.api_server_url = api_server_url;

        Ok(())
    }

    async fn install_fleet(&mut self, chart: FleetChart) -> AddonConfigSyncResult<Option<Action>> {
        let expected_version = match chart.version.as_ref() {
            Some(version) => &version.clone().normalized(),
            None => return Ok(None),
        };

        let status = self.status.get_or_insert_default();
        let installed_chart_meta = FleetChart::get_metadata("fleet-crd").await?;
        let search_result = chart
            .search_repo()
            .await?
            .into_iter()
            .find(|r| r.name == "fleet/fleet-crd");
        match (installed_chart_meta, search_result, expected_version) {
            (Some(installed), Some(search), Install::FollowLatest(true))
                if search.app_version != installed.app_version =>
            {
                chart.fleet_crds(&HelmOperation::Upgrade)?.wait().await?;
            }
            (Some(installed), Some(_), Install::Version(expected))
                if expected.strip_prefix("v").unwrap_or(expected) != installed.app_version =>
            {
                chart.fleet_crds(&HelmOperation::Upgrade)?.wait().await?;
            }
            (None, Some(_), _) => {
                chart.fleet_crds(&HelmOperation::Install)?.wait().await?;
            }
            (Some(_), Some(_), Install::FollowLatest(false) | Install::Version(_)) => {}
            (_, _, _) => return Ok(Some(Action::requeue(Duration::from_secs(10)))),
        };

        let installed_chart_meta = FleetChart::get_metadata("fleet").await?;
        let search_result = chart
            .search_repo()
            .await?
            .into_iter()
            .find(|r| r.name == "fleet/fleet");

        match (
            installed_chart_meta,
            search_result.as_ref(),
            expected_version,
        ) {
            (Some(installed), Some(search), Install::FollowLatest(true))
                if search.app_version != installed.app_version =>
            {
                chart.fleet(&HelmOperation::Upgrade)?.wait().await?;
                status.conditions.push(Condition {
                    last_transition_time: Time(Local::now().to_utc()),
                    message: format!("Updated fleet to version {}", search.app_version),
                    observed_generation: self.metadata.generation,
                    reason: "Installed".into(),
                    status: "True".into(),
                    type_: "Installed".into(),
                });
                status.installed_version = search.app_version.clone().into();
            }
            (Some(installed), Some(_), Install::Version(expected))
                if expected.strip_prefix("v").unwrap_or(expected) != installed.app_version =>
            {
                chart.fleet(&HelmOperation::Upgrade)?.wait().await?;
                status.conditions.push(Condition {
                    last_transition_time: Time(Local::now().to_utc()),
                    message: format!("Updated fleet to version {expected}"),
                    observed_generation: self.metadata.generation,
                    reason: "Installed".into(),
                    status: "True".into(),
                    type_: "Installed".into(),
                });
                status.installed_version = expected.clone().into();
            }
            (None, Some(ChartSearch { app_version, .. }), Install::FollowLatest(_))
            | (None, Some(_), Install::Version(app_version)) => {
                chart.fleet(&HelmOperation::Install)?.wait().await?;
                status.conditions.push(Condition {
                    last_transition_time: Time(Local::now().to_utc()),
                    message: format!("Installed fleet version {app_version}"),
                    observed_generation: self.metadata.generation,
                    reason: "Installed".into(),
                    status: "True".into(),
                    type_: "Installed".into(),
                });
                status.installed_version = app_version.clone().into();
            }
            (Some(installed), Some(_), Install::FollowLatest(false)) => {
                status.installed_version = installed.app_version.into();
            }
            (Some(_), Some(_), Install::Version(_)) => {}
            (_, _, _) => return Ok(Some(Action::requeue(Duration::from_secs(10)))),
        };

        Ok(None)
    }

    async fn update_flags(&mut self, ctx: Arc<Context>) -> FleetPatchResult<Option<Action>> {
        if let Some(feature_gates) = self.spec.feature_gates() {
            if let Some(reference) = feature_gates.config_map_ref() {
                feature_gates
                    .update_config_map(ctx.clone(), reference)
                    .await?;

                self.status
                    .get_or_insert_default()
                    .conditions
                    .push(Condition {
                        last_transition_time: Time(Local::now().to_utc()),
                        message: format!(
                            "Updated chart flags to the expected state: {feature_gates}"
                        ),
                        observed_generation: self.metadata.generation,
                        reason: "FlagsUpdate".into(),
                        status: "True".into(),
                        type_: "FlagsUpdate".into(),
                    });

                return Ok(Some(Action::await_change()));
            }
        }

        Ok(None)
    }
}

impl FeatureGates {
    async fn update_config_map(
        &self,
        ctx: Arc<Context>,
        reference: &ObjectReference,
    ) -> ConfigMapSyncResult<()> {
        let mut settings: FleetSettings = ctx.client.fetch(reference).await?;
        self.merge_features(settings.data.get_or_insert_default());
        patch(
            ctx,
            &mut settings,
            &PatchParams::apply("addon-provider-fleet").force(),
        )
        .await?;

        Ok(())
    }
}

/// Converts a typed watcher event to a dynamic object event.
///
/// # Errors
///
/// This function will return an error if the conversion to `DynamicObject` fails.
pub fn to_dynamic_event<R>(
    ev: Result<Event<R>, watcher::Error>,
) -> Result<Event<DynamicObject>, watcher::Error>
where
    R: Resource<DynamicType = ()> + Serialize + DeserializeOwned,
{
    let convert = |o| {
        let value = serde_json::to_value(o)?;
        let mut obj: DynamicObject = serde_json::from_value(value)?;
        if obj.types.is_none() {
            obj.types = Some(TypeMeta::resource::<R>());
        }
        Ok(obj)
    };
    let dynamic = |o| {
        convert(o)
            .map_err(kube::Error::SerdeError)
            .map_err(watcher::Error::WatchFailed)
    };
    let ev = match ev? {
        watcher::Event::Apply(o) => watcher::Event::Apply(dynamic(o)?),
        watcher::Event::Delete(o) => watcher::Event::Delete(dynamic(o)?),
        watcher::Event::InitApply(o) => watcher::Event::InitApply(dynamic(o)?),
        watcher::Event::Init => watcher::Event::Init,
        watcher::Event::InitDone => watcher::Event::InitDone,
    };
    Ok(ev)
}

pub type FleetPatchResult<T> = std::result::Result<T, FleetPatchError>;

#[derive(Error, Debug)]
pub enum FleetPatchError {
    #[error("Fleet patch error: {0}")]
    FleetPatch(#[from] helm::FleetPatchError),

    #[error("Fleet repo search error: {0}")]
    RepoSearch(#[from] helm::RepoSearchError),

    #[error("Fleet metadata check error: {0}")]
    MetadataGet(#[from] helm::MetadataGetError),

    #[error("Error waiting for command: {0}")]
    CommandError(#[from] io::Error),

    #[error(transparent)]
    ConfigMapSyncError(#[from] ConfigMapSyncError),
}

pub type AddonConfigSyncResult<T> = std::result::Result<T, AddonConfigSyncError>;

#[derive(Error, Debug)]
pub enum AddonConfigSyncError {
    #[error("Certificate config map fetch error: {0}")]
    CertificateConfigMapFetch(#[from] kube::Error),

    #[error("Fleet install error: {0}")]
    FleetInstall(#[from] helm::FleetInstallError),

    #[error("Fleet CRD install error: {0}")]
    CRDInstall(#[from] helm::FleetCRDInstallError),

    #[error("Fleet repo search error: {0}")]
    RepoSearch(#[from] helm::RepoSearchError),

    #[error("Fleet metadata check error: {0}")]
    MetadataGet(#[from] helm::MetadataGetError),

    #[error("Error waiting for command: {0}")]
    CommandError(#[from] io::Error),
}

pub type DynamicWatcherResult<T> = std::result::Result<T, DynamicWatcherError>;

#[derive(Error, Debug)]
pub enum DynamicWatcherError {
    #[error("Invalid selector encountered: {0}")]
    SelectorParseError(#[from] kube::core::ParseExpressionError),
}

pub type ConfigMapSyncResult<T> = std::result::Result<T, ConfigMapSyncError>;

#[derive(Error, Debug)]
pub enum ConfigMapSyncError {
    #[error("FleetSettings read error: {0}")]
    FetchConfigMap(#[from] kube::Error),

    #[error("FleetSettings patch error: {0}")]
    Patch(#[from] PatchError),
}

mod tests {
    #[test]
    fn test() {
        use crate::controllers::addon_config::FleetConfigData;
        let data = r#"{
            "systemDefaultRegistry": "",
            "agentImage": "rancher/fleet-agent:v0.9.4",
            "agentImagePullPolicy": "IfNotPresent",
            "apiServerURL": "https://192.168.1.123:43473",
            "apiServerCA": "test",
            "agentCheckinInterval": "15m",
            "ignoreClusterRegistrationLabels": false,
            "bootstrap": {
              "paths": "",
              "repo": "",
              "secret": "",
              "branch":  "master",
              "namespace": "fleet-local",
              "agentNamespace": ""
            },
            "webhookReceiverURL": "",
            "githubURLPrefix": ""
          }"#;

        let _config: FleetConfigData = serde_json::from_str(data).unwrap();
    }
}
