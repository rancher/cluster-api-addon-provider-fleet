use crate::api::bundle_namespace_mapping::BundleNamespaceMapping;
use crate::api::capi_cluster::{Cluster, FLEET_WORKSPACE_ANNOTATION};

use crate::api::fleet_addon_config::FleetAddonConfig;
use crate::api::fleet_cluster::{self};

#[cfg(feature = "agent-initiated")]
use crate::api::fleet_cluster_registration_token::ClusterRegistrationToken;
use crate::api::fleet_clustergroup::ClusterGroup;
use crate::controllers::addon_config::to_dynamic_event;
use crate::controllers::controller::GetApi;
use futures::StreamExt as _;
use k8s_openapi::api::core::v1::Namespace;
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::{
    ApiResource, DeleteParams, DynamicObject, GroupVersionKind, ListParams, PatchParams,
};

use kube::client::scope;
use kube::runtime::watcher::{self, Config};
use kube::{Api, Client};
use kube::{
    Resource,
    api::{Patch, ResourceExt},
    runtime::controller::Action
};
use serde::Serialize;
use serde_json::{Value, json};
use tracing::{debug, info};

use std::sync::Arc;

use super::controller::{
    Context, FleetBundle, FleetController, fetch_config, get_or_create, patch,
};
use super::{BundleResult, ClusterSyncError, ClusterSyncResult};

pub static CONTROLPLANE_INITIALIZED_CONDITION: &str = "ControlPlaneInitialized";

pub struct FleetClusterBundle {
    namespace: Namespace,
    template_sources: TemplateSources,
    fleet: fleet_cluster::Cluster,
    fleet_group: Option<ClusterGroup>,
    mapping: Option<BundleNamespaceMapping>,
    #[cfg(feature = "agent-initiated")]
    cluster_registration_token: Option<ClusterRegistrationToken>,
    config: FleetAddonConfig,
}

pub struct TemplateSources(Cluster);

#[derive(Serialize)]
struct TemplateValues {
    #[serde(rename = "Cluster")]
    cluster: Cluster,
    #[serde(rename = "ControlPlane")]
    control_plane: DynamicObject,
    #[serde(rename = "InfrastructureCluster")]
    infrastructure_cluster: DynamicObject,
}

impl TemplateSources {
    fn new(cluster: &Cluster) -> Self {
        TemplateSources(cluster.clone())
    }

    async fn resolve(&self, client: Client) -> Option<Value> {
        // We need to remove all dynamic or unnessesary values from these resources
        let mut cluster = self.0.clone();

        debug!("Mapping template values for Cluster: {}", cluster.metadata.name.clone()?);

        cluster.status = None;
        cluster.meta_mut().managed_fields = None;
        cluster.meta_mut().resource_version = None;

        // Get the ControlPlaneReference
        let reference = self.0.spec.proxy.control_plane_ref.as_ref()?;
        
        // Get the API version from CRD.
        // Note: This assumes the storage version is the right one. 
        let crd_name = format!("{}.{}", to_plural(&reference.kind).to_lowercase(), reference.api_group);
        let crds: Api<CustomResourceDefinition> = Api::all(client.clone());
        let crd: CustomResourceDefinition = crds.get(&crd_name).await.ok()?; 
        let api_version = crd.spec.versions.iter().find(|&version| version.storage)?;

        debug!("Fetching ControlPlane resource: {} - {}/{}", reference.kind, &reference.api_group, &api_version.name);

        let resource = ApiResource::from_gvk(&GroupVersionKind::gvk(
            &reference.api_group,
            &api_version.name,
            &reference.kind,
        ));
        let api = Api::<DynamicObject>::namespaced_with(
            client.clone(),
            cluster.get_namespace(),
            &resource,
        );
        let mut control_plane = api.get(&reference.name).await.ok()?;

        debug!("Found {} object: {}", &control_plane.types.clone()?.kind, &control_plane.metadata.name.clone()?);

        if let Some(data_object) = control_plane.data.as_object_mut() {
            data_object.remove("status");
        }
        control_plane.meta_mut().managed_fields = None;
        control_plane.meta_mut().resource_version = None;

        // Get the InfraReference
        let infra_reference = self.0.spec.proxy.infrastructure_ref.as_ref()?;

        // Get the API version from CRD.
        // Note: This assumes the storage version is the right one. 
        let crd_name = format!("{}.{}", to_plural(&infra_reference.kind).to_lowercase(), infra_reference.api_group);
        let crds: Api<CustomResourceDefinition> = Api::all(client.clone());
        let crd: CustomResourceDefinition = crds.get(&crd_name).await.ok()?; 
        let api_version = crd.spec.versions.iter().find(|&version| version.storage)?;

        debug!("Fetching InfrastructureCluster resource: {} - {}/{}", &infra_reference.kind, &infra_reference.api_group, &api_version.name);

        let resource = ApiResource::from_gvk(&GroupVersionKind::gvk(
            &infra_reference.api_group,
            &api_version.name,
            &infra_reference.kind,
        ));
        let api = Api::<DynamicObject>::namespaced_with(
            client.clone(),
            cluster.get_namespace(),
            &resource,
        );
        let mut infrastructure_cluster = api.get(&infra_reference.name).await.ok()?;

        debug!("Found {} object: {}", &infrastructure_cluster.types.clone()?.kind, &infrastructure_cluster.metadata.name.clone()?);

        if let Some(data_object) = infrastructure_cluster.data.as_object_mut() {
            data_object.remove("status");
        }
        infrastructure_cluster.meta_mut().managed_fields = None;
        infrastructure_cluster.meta_mut().resource_version = None;

        let values = TemplateValues {
            cluster,
            control_plane,
            infrastructure_cluster,
        };

        serde_json::to_value(values).ok()
    }
}

impl FleetBundle for FleetClusterBundle {
    #[allow(refining_impl_trait)]
    async fn sync(&mut self, ctx: Arc<Context>) -> ClusterSyncResult<Action> {
        let cluster = &mut self.fleet;

        if let Some(template) = self.template_sources.resolve(ctx.client.clone()).await {
            let template = serde_json::from_value(template)?;
            cluster.spec.template_values = Some(template);
        }

        if let Some(mapping) = self.mapping.as_mut() {
            if self.config.cluster_patch_enabled() {
                let cluster_name = cluster.name_any();
                patch(
                    ctx.clone(),
                    mapping,
                    &PatchParams::apply(&format!("cluster-{cluster_name}-addon-provider-fleet")),
                )
                .await
                .map_err(ClusterSyncError::BundleNamespaceMappingError)?;

                let class_namespace = mapping.namespace().unwrap_or_default();
                let cluster_namespace = mapping.name_any();
                info!(
                    "Updated BundleNamespaceMapping for cluster {cluster_name} between class namespace: {class_namespace} and cluster namespace: {cluster_namespace}"
                );
            }
        }

        if self.config.cluster_patch_enabled() {
            patch(
                ctx.clone(),
                cluster,
                &PatchParams::apply("addon-provider-fleet"),
            )
            .await?
        } else {
            get_or_create(ctx.clone(), cluster).await?
        };

        #[cfg(feature = "agent-initiated")]
        if let Some(cluster_registration_token) = self.cluster_registration_token.as_ref() {
            get_or_create(ctx.clone(), cluster_registration_token).await?;
        }

        if let Some(group) = self.fleet_group.as_mut() {
            let cluster_name = self.fleet.name_any();
            if self.config.cluster_patch_enabled() {
                patch(
                    ctx.clone(),
                    group,
                    &PatchParams::apply(&format!("cluster-{cluster_name}-addon-provider-fleet")),
                )
                .await
                .map_err(ClusterSyncError::GroupPatchError)?;
            }
        }

        // Ensure the fleet workspace annotation is present.
        patch(
            ctx.clone(),
            &mut self.namespace,
            &PatchParams::apply("namespace-addon-provider-fleet"),
        )
        .await
        .map_err(ClusterSyncError::NamespacePatchError)?;

        debug!(
            "Added fleet annotation to namespace {}.",
            self.fleet.get_namespace()
        );

        Ok(Action::await_change())
    }

    async fn cleanup(&mut self, ctx: Arc<Context>) -> Result<Action, super::SyncError> {
        if let Some(mapping) = self.mapping.as_ref() {
            let ns = mapping.namespace();
            let other_clusters = ctx
                .client
                .list::<Cluster>(
                    &ListParams::default(),
                    &scope::Namespace::from(ns.clone().unwrap_or_default()),
                )
                .await?;

            let referencing_cluster = other_clusters.iter().find(|c| {
                c.cluster_class_namespace() == ns
                    && c.name_any() != self.fleet.name_any()
                    && c.metadata.deletion_timestamp.is_none()
            });

            if referencing_cluster.is_some() {
                return Ok(Action::await_change());
            }

            let bundle_namespace_mapping = BundleNamespaceMapping::get_api(ctx.client.clone(), mapping.get_namespace());

            if bundle_namespace_mapping.get_opt(&mapping.name_any()).await?.is_some() {
                bundle_namespace_mapping.delete(&mapping.name_any(), &DeleteParams::default()).await?;
            }
        }

        // List all other clusters in this namespace
        let other_clusters = Cluster::get_api(ctx.client.clone(), self.fleet.get_namespace())
            .list(
                &ListParams::default()
                    .fields(&format!("metadata.name!={}", self.fleet.name_any()))
                    .limit(1),
            )
            .await?;
        // If no other clusters are found in this namespace, remove the fleet workspace annotation.
        if other_clusters.items.is_empty() {
            let patch = json!({
                "metadata": {
                    "annotations": {
                        FLEET_WORKSPACE_ANNOTATION: null
                    }
                }
            });
            Namespace::get_api(ctx.client.clone(), &())
                .patch_metadata(
                    self.fleet.get_namespace(),
                    &PatchParams::default(),
                    &Patch::Merge(&patch),
                )
                .await?;
            debug!(
                "Removed fleet annotation from namespace {}.",
                self.fleet.get_namespace()
            );
        }

        Ok(Action::await_change())
    }
}

impl FleetController for Cluster {
    type Bundle = FleetClusterBundle;

    async fn to_bundle(&self, ctx: Arc<Context>) -> BundleResult<Option<FleetClusterBundle>> {
        let config = fetch_config(ctx.client.clone()).await?;

        if !config.cluster_operations_enabled() {
            return Ok(None);
        }

        if self.cluster_ready().is_none_or(|initialized| !initialized){
            debug!("ControlPlane not yet initialized. Nothing to do.");
            return Ok(None);
        }

        Ok(Some(FleetClusterBundle {
            template_sources: TemplateSources::new(self),
            fleet: self.to_cluster(config.spec.cluster.as_ref()),
            fleet_group: self.to_group(config.spec.cluster.as_ref()),
            mapping: self.to_bundle_ns_mapping(config.spec.cluster.as_ref()),
            #[cfg(feature = "agent-initiated")]
            cluster_registration_token: self
                .to_cluster_registration_token(config.spec.cluster.as_ref()),
            config,
            namespace: self.to_namespace(),
        }))
    }
}

impl Cluster {
    #[must_use]
    pub fn cluster_ready(&self) -> Option<bool> {
        let status = self.status.clone()?;

        match status.initialization {
            Some(initialization) => initialization.control_plane_initialized,
            None => None
        }
    }

    /// Adds a dynamic watcher for a specific namespace.
    ///
    /// # Errors
    ///
    /// This function will return an error if the watcher cannot be created or added to the stream.
    pub async fn add_namespace_dynamic_watch(
        ns: Arc<Namespace>,
        ctx: Arc<Context>,
    ) -> crate::Result<Action> {
        if ctx.version >= 32 {
            ctx.stream.stream.lock().await.push(
                watcher::watcher(
                    Api::namespaced_with(
                        ctx.client.clone(),
                        &ns.name_any(),
                        &ApiResource::erase::<Cluster>(&()),
                    ),
                    Config::default().streaming_lists(),
                )
                .boxed(),
            );
        } else {
            ctx.stream.stream.lock().await.push(
                watcher::watcher(
                    Api::<Cluster>::namespaced(ctx.client.clone(), &ns.name_any()),
                    Config::default(),
                )
                .map(to_dynamic_event)
                .boxed(),
            );
        }

        let name = ns.name_any();
        info!("Reconciled dynamic watches: added namespace watch on {name}");

        Ok(Action::await_change())
    }
}

// Simple pluralizer.
// Duplicating the code from kube (without special casing) because it's simple enough.
// See: https://github.com/kube-rs/kube/blob/2.0.1/kube-derive/src/custom_resource.rs#L856
fn to_plural(word: &str) -> String {
    // Words ending in s, x, z, ch, sh will be pluralized with -es (eg. foxes).
    if word.ends_with('s')
        || word.ends_with('x')
        || word.ends_with('z')
        || word.ends_with("ch")
        || word.ends_with("sh")
    {
        return format!("{word}es");
    }

    // Words ending in y that are preceded by a consonant will be pluralized by
    // replacing y with -ies (eg. puppies).
    if word.ends_with('y')
        && let Some(c) = word.chars().nth(word.len() - 2)
        && !matches!(c, 'a' | 'e' | 'i' | 'o' | 'u')
    {
        // Remove 'y' and add `ies`
        let mut chars = word.chars();
        chars.next_back();
        return format!("{}ies", chars.as_str());
    }

    // All other words will have "s" added to the end (eg. days).
    format!("{word}s")
}
