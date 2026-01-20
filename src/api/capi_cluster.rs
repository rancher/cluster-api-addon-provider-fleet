use std::collections::BTreeMap;

use cluster_api_rs::capi_cluster::{ClusterSpec, ClusterStatus};
use fleet_api_rs::{
    fleet_bundle_namespace_mapping::{
        BundleNamespaceMappingBundleSelector, BundleNamespaceMappingNamespaceSelector,
    },
    fleet_clustergroup::{ClusterGroupSelector, ClusterGroupSpec},
};
use k8s_openapi::api::core::v1::Namespace;
use kube::{
    CustomResource, Resource, ResourceExt as _,
    api::{ObjectMeta, TypeMeta},
};
#[cfg(feature = "agent-initiated")]
use rand::distr::{Alphanumeric, SampleString as _};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    bundle_namespace_mapping::BundleNamespaceMapping,
    fleet_addon_config::ClusterConfig,
    fleet_cluster,
    fleet_clustergroup::{CLUSTER_CLASS_LABEL, CLUSTER_CLASS_NAMESPACE_LABEL, ClusterGroup},
};

#[cfg(feature = "agent-initiated")]
use super::fleet_cluster_registration_token::ClusterRegistrationToken;

pub static FLEET_WORKSPACE_ANNOTATION: &str =
    "field.cattle.io/allow-fleetworkspace-creation-for-existing-namespace";

/// `ClusterProxy` defines the desired state of the CAPI Cluster.
#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "cluster.x-k8s.io",
    version = "v1beta2",
    kind = "Cluster",
    plural = "clusters"
)]
#[kube(namespaced)]
#[kube(status = "ClusterStatus")]
pub struct ClusterProxy {
    #[serde(flatten)]
    pub proxy: ClusterSpec,
}

impl From<&Cluster> for ObjectMeta {
    fn from(cluster: &Cluster) -> Self {
        Self {
            name: Some(cluster.name_any()),
            namespace: cluster.namespace(),
            ..Default::default()
        }
    }
}

impl Cluster {
    pub(crate) fn to_group(self: &Cluster, config: Option<&ClusterConfig>) -> Option<ClusterGroup> {
        config?.apply_class_group().then_some(true)?;

        let class = self.cluster_class_name()?;
        // Cluster groups creation for cluster class namespace are handled by ClusterClass controller
        let class_namespace = self.cluster_class_namespace()?;

        let labels = {
            let mut labels = BTreeMap::default();
            labels.insert(CLUSTER_CLASS_LABEL.to_string(), class.to_string());
            labels.insert(
                CLUSTER_CLASS_NAMESPACE_LABEL.to_string(),
                class_namespace.to_string(),
            );
            Some(labels)
        };

        Some(ClusterGroup {
            types: Some(TypeMeta::resource::<ClusterGroup>()),
            metadata: ObjectMeta {
                name: Some(format!("{class}.{class_namespace}")),
                namespace: self.namespace(),
                labels: labels.clone(),
                owner_references: self.owner_ref(&()).into_iter().map(Into::into).collect(),
                ..Default::default()
            },
            spec: ClusterGroupSpec {
                selector: Some(ClusterGroupSelector {
                    match_labels: labels,
                    ..Default::default()
                }),
            },
            ..Default::default()
        })
    }

    pub(crate) fn to_cluster(
        self: &Cluster,
        config: Option<&ClusterConfig>,
    ) -> fleet_cluster::Cluster {
        let empty = ClusterConfig::default();
        let config = config.unwrap_or(&empty);
        let class = self.cluster_class_name();
        let ns = self.namespace().unwrap_or_default();
        let class_namespace = self.cluster_class_namespace().unwrap_or(ns);
        let annotations = self.annotations().clone();
        let labels = {
            let mut labels = self.labels().clone();
            if let Some(class) = class {
                labels.insert(CLUSTER_CLASS_LABEL.to_string(), class.to_string());
                labels.insert(
                    CLUSTER_CLASS_NAMESPACE_LABEL.to_string(),
                    class_namespace.to_string(),
                );
            }
            labels
        };

        fleet_cluster::Cluster {
            types: Some(TypeMeta::resource::<fleet_cluster::Cluster>()),
            metadata: ObjectMeta {
                annotations: Some(annotations),
                labels: Some(labels),
                owner_references: config
                    .set_owner_references
                    .is_some_and(|set| set)
                    .then_some(self.owner_ref(&()).into_iter().collect()),
                name: config.apply_naming(self.name_any()).into(),
                ..self.into()
            },
            #[cfg(feature = "agent-initiated")]
            spec: if config.agent_initiated_connection() {
                fleet_api_rs::fleet_cluster::ClusterSpec {
                    client_id: Some(Alphanumeric.sample_string(&mut rand::rng(), 64)),
                    agent_namespace: config.agent_install_namespace().into(),
                    agent_tolerations: config.agent_tolerations().into(),
                    host_network: config.host_network,
                    agent_env_vars: config.agent_env_vars.clone(),
                    ..Default::default()
                }
            } else {
                fleet_api_rs::fleet_cluster::ClusterSpec {
                    kube_config_secret: Some(format!("{}-kubeconfig", self.name_any())),
                    agent_namespace: config.agent_install_namespace().into(),
                    agent_tolerations: config.agent_tolerations().into(),
                    host_network: config.host_network,
                    agent_env_vars: config.agent_env_vars.clone(),
                    ..Default::default()
                }
            },
            #[cfg(not(feature = "agent-initiated"))]
            spec: fleet_api_rs::fleet_cluster::ClusterSpec {
                kube_config_secret: Some(format!("{}-kubeconfig", self.name_any())),
                agent_namespace: config.agent_install_namespace().into(),
                agent_tolerations: config.agent_tolerations().into(),
                host_network: config.host_network,
                agent_env_vars: config.agent_env_vars.clone(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub(crate) fn to_bundle_ns_mapping(
        &self,
        config: Option<&ClusterConfig>,
    ) -> Option<BundleNamespaceMapping> {
        config?.apply_class_group().then_some(true)?;

        let topology = self.spec.proxy.topology.as_ref()?;
        let class_namespace = topology.class_ref.namespace.clone()?;

        let match_labels = {
            let mut labels = BTreeMap::default();
            labels.insert("kubernetes.io/metadata.name".into(), self.namespace()?);
            Some(labels)
        };

        Some(BundleNamespaceMapping {
            types: Some(TypeMeta::resource::<BundleNamespaceMapping>()),
            metadata: ObjectMeta {
                name: self.namespace(),
                namespace: Some(class_namespace),
                ..Default::default()
            },
            bundle_selector: BundleNamespaceMappingBundleSelector::default(),
            namespace_selector: BundleNamespaceMappingNamespaceSelector {
                match_labels,
                ..Default::default()
            },
        })
    }

    #[cfg(feature = "agent-initiated")]
    pub(crate) fn to_cluster_registration_token(
        self: &Cluster,
        config: Option<&ClusterConfig>,
    ) -> Option<ClusterRegistrationToken> {
        use fleet_api_rs::fleet_cluster_registration_token::ClusterRegistrationTokenSpec;

        config?.agent_initiated?.then_some(true)?;

        ClusterRegistrationToken {
            metadata: self.into(),
            spec: ClusterRegistrationTokenSpec {
                ttl: Some("1h".into()),
            },
            ..Default::default()
        }
        .into()
    }

    pub(crate) fn to_namespace(self: &Cluster) -> Namespace {
        Namespace {
            metadata: ObjectMeta {
                name: self.namespace(),
                annotations: Some({
                    let mut map = BTreeMap::new();
                    map.insert(FLEET_WORKSPACE_ANNOTATION.to_string(), "true".to_string());
                    map
                }),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub(crate) fn cluster_class_namespace(&self) -> Option<String> {
        self.spec
            .proxy
            .topology
            .as_ref()?
            .class_ref
            .namespace.clone()
    }

    pub(crate) fn cluster_class_name(&self) -> Option<String> {
        let topology = self.spec.proxy.topology.as_ref()?;
        Some(topology.class_ref.name.clone())
    }
}
