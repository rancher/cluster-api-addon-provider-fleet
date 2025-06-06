use std::{fmt::Display, str::FromStr};

use crate::api::comparable::ResourceDiff;
use educe::Educe;
use fleet_api_rs::fleet_cluster::{ClusterAgentEnvVars, ClusterAgentTolerations};
use k8s_openapi::{
    api::core::v1::{ConfigMap, ObjectReference},
    apimachinery::pkg::apis::meta::v1::{Condition, LabelSelector},
};
use kube::{
    CustomResource, KubeSchema, Resource,
    api::{ObjectMeta, TypeMeta},
    core::{ParseExpressionError, Selector},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, ser};
use serde_with::{DisplayFromStr, serde_as};
use serde_yaml::Value;

pub const AGENT_NAMESPACE: &str = "fleet-addon-agent";
pub const EXPERIMENTAL_OCI_STORAGE: &str = "EXPERIMENTAL_OCI_STORAGE";
pub const EXPERIMENTAL_HELM_OPS: &str = "EXPERIMENTAL_HELM_OPS";

/// This provides a config for fleet addon functionality
#[derive(CustomResource, Deserialize, Serialize, Clone, Default, Debug, KubeSchema, PartialEq)]
#[kube(
    kind = "FleetAddonConfig",
    group = "addons.cluster.x-k8s.io",
    version = "v1alpha1",
    status = "FleetAddonConfigStatus",
    validation = "self.metadata.name == 'fleet-addon-config'"
)]
#[serde(rename_all = "camelCase")]
pub struct FleetAddonConfigSpec {
    /// Enable clusterClass controller functionality.
    ///
    /// This will create Fleet `ClusterGroups` for each `ClusterClaster` with the same name.
    pub cluster_class: Option<ClusterClassConfig>,

    /// Enable Cluster config funtionality.
    ///
    /// This will create Fleet Cluster for each Cluster with the same name.
    /// In case the cluster specifies topology.class, the name of the `ClusterClass`
    /// will be added to the Fleet Cluster labels.
    pub cluster: Option<ClusterConfig>,

    // Fleet chart configuratoin options
    pub config: Option<FleetConfig>,

    // Fleet chart installation options
    pub install: Option<FleetInstall>,
}

impl Default for FleetAddonConfig {
    fn default() -> Self {
        Self {
            metadata: ObjectMeta::default(),
            spec: FleetAddonConfigSpec {
                cluster_class: Some(ClusterClassConfig::default()),
                cluster: Some(ClusterConfig::default()),
                config: Some(FleetConfig::default()),
                ..Default::default()
            },
            status: Option::default(),
        }
    }
}
impl ResourceDiff for FleetAddonConfig {
    fn diff(&self, _: &Self) -> bool {
        true
    }
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FleetAddonConfigStatus {
    pub installed_version: Option<String>,
    /// conditions represents the observations of a Fleet addon current state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClusterClassConfig {
    /// Setting to disable setting owner references on the created resources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_owner_references: Option<bool>,

    /// Allow to patch resources, maintaining the desired state.
    /// If is not set, resources will only be re-created in case of removal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch_resource: Option<bool>,
}

impl Default for ClusterClassConfig {
    fn default() -> Self {
        Self {
            patch_resource: Some(true),
            set_owner_references: Some(true),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClusterConfig {
    /// Apply a `ClusterGroup` for a `ClusterClass` referenced from a different namespace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_class_group: Option<bool>,

    /// Allow to patch resources, maintaining the desired state.
    /// If is not set, resources will only be re-created in case of removal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch_resource: Option<bool>,

    /// Setting to disable setting owner references on the created resources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_owner_references: Option<bool>,

    /// Naming settings for the fleet cluster
    #[serde(skip_serializing_if = "Option::is_none")]
    pub naming: Option<NamingStrategy>,

    /// Namespace selection for the fleet agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_namespace: Option<String>,

    /// Agent taint toleration settings for every cluster
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_tolerations: Option<Vec<ClusterAgentTolerations>>,

    /// Host network allows to deploy agent configuration using hostNetwork: true setting
    /// which eludes dependency on the CNI configuration for the cluster.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_network: Option<bool>,

    /// `AgentEnvVars` are extra environment variables to be added to the agent deployment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_env_vars: Option<Vec<ClusterAgentEnvVars>>,

    /// Import settings for the CAPI cluster. Allows to import clusters based on a set of labels,
    /// set on the cluster or the namespace.
    #[serde(flatten)]
    pub selectors: Selectors,

    #[cfg(feature = "agent-initiated")]
    /// Prepare initial cluster for agent initiated connection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_initiated: Option<bool>,
}

#[derive(Resource, Serialize, Deserialize, Default, Clone, Debug)]
#[resource(inherit = ConfigMap)]
pub struct FleetSettings {
    #[serde(flatten, default)]
    pub types: Option<TypeMeta>,
    pub metadata: ObjectMeta,
    pub data: Option<FleetSettingsSpec>,
}

impl ResourceDiff for FleetSettings {
    fn diff(&self, other: &Self) -> bool {
        self.data != other.data
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, Default, Clone, Debug, Educe)]
#[educe(PartialEq)]
pub struct FleetSettingsSpec {
    #[serde(default)]
    #[serde_as(as = "DisplayFromStr")]
    pub fleet: FleetChartValues,

    #[educe(PartialEq(ignore))]
    #[serde(flatten)]
    pub other: Value,
}

impl FromStr for FleetChartValues {
    type Err = serde_yaml::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_yaml::from_str(s)
    }
}

impl Display for FleetChartValues {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&serde_yaml::to_string(self).map_err(ser::Error::custom)?)
    }
}

impl FleetAddonConfigSpec {
    /// Returns reference to `FeatureGates` if defined.
    pub(crate) fn feature_gates(&self) -> Option<&FeatureGates> {
        self.config.as_ref()?.feature_gates.as_ref()
    }

    /// Returns the value of `bootstrap_local_cluster` if defined.
    pub(crate) fn bootstrap(&self) -> Option<bool> {
        self.config.as_ref()?.bootstrap_local_cluster
    }
}

impl ClusterConfig {
    pub(crate) fn agent_install_namespace(&self) -> String {
        self.agent_namespace
            .clone()
            .unwrap_or(AGENT_NAMESPACE.to_string())
    }

    pub(crate) fn agent_tolerations(&self) -> Vec<ClusterAgentTolerations> {
        let agent_tolerations = vec![
            ClusterAgentTolerations {
                effect: Some("NoSchedule".into()),
                operator: Some("Exists".into()),
                key: Some("node.kubernetes.io/not-ready".into()),
                ..Default::default()
            },
            ClusterAgentTolerations {
                effect: Some("NoSchedule".into()),
                operator: Some("Exists".into()),
                key: Some("node.cluster.x-k8s.io/uninitialized".into()),
                ..Default::default()
            },
            ClusterAgentTolerations {
                effect: Some("NoSchedule".into()),
                operator: Some("Equal".into()),
                key: Some("node.cloudprovider.kubernetes.io/uninitialized".into()),
                value: Some("true".into()),
                ..Default::default()
            },
        ];

        self.agent_tolerations.clone().unwrap_or(agent_tolerations)
    }

    #[cfg(feature = "agent-initiated")]
    pub(crate) fn agent_initiated_connection(&self) -> bool {
        self.agent_initiated.filter(|&set| set).is_some()
    }

    pub(crate) fn apply_naming(&self, name: String) -> String {
        let strategy = self.naming.clone().unwrap_or_default();
        strategy.apply(name.clone().into()).unwrap_or(name)
    }

    pub(crate) fn apply_class_group(&self) -> bool {
        self.apply_class_group.is_some_and(|enabled| enabled)
    }
}

/// `NamingStrategy` is controlling Fleet cluster naming
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, Default, PartialEq)]
pub struct NamingStrategy {
    /// Specify a prefix for the Cluster name, applied to created Fleet cluster
    pub prefix: Option<String>,
    /// Specify a suffix for the Cluster name, applied to created Fleet cluster
    pub suffix: Option<String>,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            apply_class_group: Some(true),
            set_owner_references: Some(true),
            naming: Option::default(),
            agent_namespace: AGENT_NAMESPACE.to_string().into(),
            host_network: Some(true),
            #[cfg(feature = "agent-initiated")]
            agent_initiated: Some(true),
            selectors: Selectors::default(),
            patch_resource: Some(true),
            agent_env_vars: None,
            agent_tolerations: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FleetConfig {
    /// fleet server url configuration options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<Server>,
    /// feature gates controlling experimental features
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_gates: Option<FeatureGates>,
    /// Enable auto-installation of a fleet agent in the local cluster.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap_local_cluster: Option<bool>,
}

impl Default for FleetConfig {
    fn default() -> Self {
        Self {
            server: Option::default(),
            feature_gates: Some(FeatureGates::default()),
            bootstrap_local_cluster: None,
        }
    }
}

/// Feature toggles for enabling or disabling experimental functionality.
/// This struct controls access to specific experimental features.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FeatureGates {
    /// Enables experimental OCI  storage support.
    pub experimental_oci_storage: bool,

    /// Enables experimental Helm operations support.
    pub experimental_helm_ops: bool,

    // Enables syncing of feature gates to a ConfigMap.
    pub config_map: Option<FeaturesConfigMap>,
}

impl Display for FeatureGates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let oci = self.experimental_oci_storage;
        let helm_ops = self.experimental_helm_ops;
        let config_map = self.config_map_ref();
        f.write_str(&format!("ref={config_map:#?}, oci={oci}, helm={helm_ops}"))
    }
}

impl FeatureGates {
    /// Returns reference to a `ConfigMap` if defined.
    pub(crate) fn config_map_ref(&self) -> Option<&ObjectReference> {
        self.config_map.as_ref()?.reference.as_ref()
    }

    /// Merge the feature gates environment variables with a provided optional input.
    pub(crate) fn merge_features(&self, settings: &mut FleetSettingsSpec) {
        // Sync the feature flags to the map.
        let env_map = settings.fleet.extra_env.get_or_insert_default();

        match env_map
            .iter_mut()
            .find(|env| env.name == EXPERIMENTAL_HELM_OPS)
        {
            Some(helm_ops) => helm_ops.value = self.experimental_helm_ops.to_string(),
            None => env_map.push(EnvironmentVariable {
                name: EXPERIMENTAL_HELM_OPS.to_string(),
                value: self.experimental_helm_ops.to_string(),
            }),
        }

        match env_map
            .iter_mut()
            .find(|env| env.name == EXPERIMENTAL_OCI_STORAGE)
        {
            Some(helm_ops) => helm_ops.value = self.experimental_oci_storage.to_string(),
            None => env_map.push(EnvironmentVariable {
                name: EXPERIMENTAL_OCI_STORAGE.to_string(),
                value: self.experimental_oci_storage.to_string(),
            }),
        }
    }
}

impl Default for FeatureGates {
    fn default() -> Self {
        Self {
            // Unless is set otherwise, these features are enabled by CAAPF
            experimental_oci_storage: true,
            experimental_helm_ops: true,
            config_map: None,
        }
    }
}

/// `FeaturesConfigMap` references a `ConfigMap` where to apply feature flags.
/// If a `ConfigMap` is referenced, the controller will update it instead of upgrading the Fleet chart.
#[derive(Clone, Default, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FeaturesConfigMap {
    // The reference to a ConfigMap resource
    #[serde(rename = "ref")]
    pub reference: Option<ObjectReference>,
}

/// `FleetChartValues` represents Fleet chart values.
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FleetChartValues {
    pub extra_env: Option<Vec<EnvironmentVariable>>,
    #[serde(flatten)]
    pub other: Value,
}

/// `EnvironmentVariable` is a simple name/value pair.
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentVariable {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FleetInstall {
    /// Chart version to install
    #[serde(flatten)]
    pub install_version: Install,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Install {
    /// Follow the latest version of the chart on install
    FollowLatest(bool),

    /// Use specific version to install
    Version(String),
}

impl Install {
    /// Perform version normalization for comparison with `helm search` `app_version` output
    pub(crate) fn normalized(self) -> Self {
        match self {
            Install::FollowLatest(_) => self,
            Install::Version(version) => {
                Install::Version(version.strip_prefix("v").unwrap_or(&version).into())
            }
        }
    }
}

impl Default for Install {
    fn default() -> Self {
        Self::FollowLatest(true)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Server {
    InferLocal(bool),
    Custom(InstallOptions),
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstallOptions {
    pub api_server_ca_config_ref: Option<ObjectReference>,
    pub api_server_url: Option<String>,
}

impl NamingStrategy {
    #[must_use]
    pub fn apply(&self, name: Option<String>) -> Option<String> {
        name.map(|name| match &self.prefix {
            Some(prefix) => prefix.clone() + &name,
            None => name,
        })
        .map(|name| match &self.suffix {
            Some(suffix) => name + suffix,
            None => name,
        })
    }
}

/// Selectors is controlling Fleet import strategy settings.
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Selectors {
    /// Namespace label selector. If set, only clusters in the namespace matching label selector will be imported.
    pub namespace_selector: LabelSelector,

    /// Cluster label selector. If set, only clusters matching label selector will be imported.
    pub selector: LabelSelector,
}

impl FleetAddonConfig {
    // Raw cluster selector
    pub(crate) fn cluster_selector(&self) -> Result<Selector, ParseExpressionError> {
        self.spec
            .cluster
            .as_ref()
            .map(|c| c.selectors.selector.clone())
            .unwrap_or_default()
            .try_into()
    }

    // Raw namespace selector
    pub(crate) fn namespace_selector(&self) -> Result<Selector, ParseExpressionError> {
        self.spec
            .cluster
            .as_ref()
            .map(|c| c.selectors.namespace_selector.clone())
            .unwrap_or_default()
            .try_into()
    }

    // Check for general cluster operations, like create, patch, etc. Evaluates to false if disabled.
    pub(crate) fn cluster_operations_enabled(&self) -> bool {
        self.spec.cluster.is_some()
    }

    // Check for general ClusterClass operations, like create, patch, etc. Evaluates to false if disabled.
    pub(crate) fn cluster_class_operations_enabled(&self) -> bool {
        self.spec.cluster_class.is_some()
    }

    // Check for general cluster patching setting.
    pub(crate) fn cluster_patch_enabled(&self) -> bool {
        self.spec
            .cluster
            .as_ref()
            .map(|c| c.patch_resource)
            .unwrap_or_default()
            .filter(|&enabled| enabled)
            .is_some()
    }

    // Check for general clusterClass patching setting.
    pub(crate) fn cluster_class_patch_enabled(&self) -> bool {
        self.spec
            .cluster_class
            .as_ref()
            .map(|c| c.patch_resource)
            .unwrap_or_default()
            .filter(|&enabled| enabled)
            .is_some()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::api::fleet_addon_config::{
        FeatureGates, FleetChartValues, FleetSettingsSpec, NamingStrategy,
    };

    #[tokio::test]
    async fn test_naming_strategy() {
        assert_eq!(
            Some("prefixtestsuffix".to_string()),
            NamingStrategy {
                prefix: "prefix".to_string().into(),
                suffix: "suffix".to_string().into(),
            }
            .apply("test".to_string().into())
        );

        assert_eq!(
            Some("testsuffix".to_string()),
            NamingStrategy {
                suffix: "suffix".to_string().into(),
                ..Default::default()
            }
            .apply("test".to_string().into())
        );

        assert_eq!(
            Some("prefixtest".to_string()),
            NamingStrategy {
                prefix: "prefix".to_string().into(),
                ..Default::default()
            }
            .apply("test".to_string().into())
        );

        assert_eq!(
            Some("test".to_string()),
            NamingStrategy {
                ..Default::default()
            }
            .apply("test".to_string().into())
        );

        assert_eq!(
            None,
            NamingStrategy {
                prefix: "prefix".to_string().into(),
                suffix: "suffix".to_string().into(),
            }
            .apply(None)
        );
    }

    #[tokio::test]
    async fn test_sync_config_map() {
        let want_fleet_data = r"extraEnv:
- name: EXPERIMENTAL_HELM_OPS
  value: 'true'
- name: foo
  value: bar
- name: EXPERIMENTAL_OCI_STORAGE
  value: 'true'
foo:
  bar: foobar
";
        let fleet_data = r#"extraEnv:
- name: EXPERIMENTAL_HELM_OPS
  value: "false"
- name: foo
  value: bar
foo:
  bar: foobar
"#;
        let mut data = FleetSettingsSpec {
            fleet: FleetChartValues::from_str(fleet_data).unwrap(),
            ..Default::default()
        };
        let feature_gates = FeatureGates::default();

        feature_gates.merge_features(&mut data);

        assert_eq!(want_fleet_data.to_string(), data.fleet.to_string());
    }

    #[tokio::test]
    async fn test_sync_empty_config_map() {
        let want_fleet_data = r"extraEnv:
- name: EXPERIMENTAL_HELM_OPS
  value: 'false'
- name: EXPERIMENTAL_OCI_STORAGE
  value: 'false'
";
        let mut data = FleetSettingsSpec::default();
        let feature_gates = FeatureGates {
            experimental_oci_storage: false,
            experimental_helm_ops: false,
            config_map: None,
        };

        feature_gates.merge_features(&mut data);

        assert_eq!(want_fleet_data.to_string(), data.fleet.to_string());
    }
}
