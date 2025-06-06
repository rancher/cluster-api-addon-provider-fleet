use std::collections::{BTreeMap, HashSet};

use fleet_api_rs::fleet_clustergroup::{
    ClusterGroupSelector, ClusterGroupSpec, ClusterGroupStatus,
};
use k8s_openapi::api::core::v1::ObjectReference;
use kube::{
    Resource, ResourceExt as _,
    api::{ObjectMeta, TypeMeta},
    core::{Expression, Selector},
    runtime::reflector::ObjectRef,
};
use serde::{Deserialize, Serialize};

use super::capi_clusterclass::ClusterClass;
use crate::api::comparable::ResourceDiff;

pub static CLUSTER_CLASS_LABEL: &str = "clusterclass-name.fleet.addons.cluster.x-k8s.io";
pub static CLUSTER_CLASS_NAMESPACE_LABEL: &str =
    "clusterclass-namespace.fleet.addons.cluster.x-k8s.io";

#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[resource(inherit = fleet_api_rs::fleet_clustergroup::ClusterGroup)]
pub struct ClusterGroup {
    #[serde(flatten, default)]
    pub types: Option<TypeMeta>,
    pub metadata: ObjectMeta,
    pub spec: ClusterGroupSpec,
    pub status: Option<ClusterGroupStatus>,
}

impl ResourceDiff for ClusterGroup {
    fn diff(&self, other: &Self) -> bool {
        let annotations_equal = self
            .annotations()
            .iter()
            .all(|(k, v)| other.annotations().get(k) == Some(v));
        let labels_equal = self
            .labels()
            .iter()
            .all(|(k, v)| other.labels().get(k) == Some(v));

        let owner_uids: HashSet<String> = other
            .owner_references()
            .iter()
            .map(|r| &r.uid)
            .cloned()
            .collect();
        let owner_references_equal = self
            .owner_references()
            .iter()
            .all(|self_ref| owner_uids.contains(&self_ref.uid));

        self.spec != other.spec || !annotations_equal || !labels_equal || !owner_references_equal
    }
}

impl ClusterGroup {
    pub(crate) fn cluster_class_namespace(&self) -> Option<String> {
        self.labels()
            .iter()
            .find_map(|(key, class_ns)| (*key == CLUSTER_CLASS_NAMESPACE_LABEL).then_some(class_ns))
            .cloned()
    }

    pub(crate) fn cluster_class_name(&self) -> Option<String> {
        self.labels()
            .iter()
            .find_map(|(key, class)| (*key == CLUSTER_CLASS_LABEL).then_some(class))
            .cloned()
    }

    pub(crate) fn cluster_class_ref(&self) -> Option<ObjectReference> {
        let name = self.cluster_class_name()?;
        let namespace = self.cluster_class_namespace()?;
        Some(
            ObjectRef::<ClusterClass>::new(&name)
                .within(&namespace)
                .into(),
        )
    }

    pub(crate) fn group_selector() -> Selector {
        Selector::from_iter([
            Expression::Exists(CLUSTER_CLASS_LABEL.to_string()),
            Expression::Exists(CLUSTER_CLASS_NAMESPACE_LABEL.to_string()),
        ])
    }
}

impl From<&ClusterClass> for ClusterGroup {
    fn from(cluster_class: &ClusterClass) -> Self {
        let labels = {
            let mut labels = cluster_class.labels().clone();
            labels.insert(CLUSTER_CLASS_LABEL.to_string(), cluster_class.name_any());
            labels.insert(
                CLUSTER_CLASS_NAMESPACE_LABEL.to_string(),
                cluster_class.namespace().unwrap_or_default(),
            );
            Some(labels)
        };

        let match_labels = {
            let mut labels = BTreeMap::default();
            labels.insert(CLUSTER_CLASS_LABEL.to_string(), cluster_class.name_any());
            labels.insert(
                CLUSTER_CLASS_NAMESPACE_LABEL.to_string(),
                cluster_class.namespace().unwrap_or_default(),
            );
            Some(labels)
        };

        Self {
            types: Some(TypeMeta::resource::<ClusterGroup>()),
            metadata: ObjectMeta {
                name: Some(cluster_class.name_any()),
                namespace: cluster_class.namespace(),
                labels,
                owner_references: cluster_class
                    .owner_ref(&())
                    .into_iter()
                    .map(Into::into)
                    .collect(),
                ..Default::default()
            },
            spec: ClusterGroupSpec {
                selector: Some(ClusterGroupSelector {
                    match_labels,
                    ..Default::default()
                }),
            },
            ..Default::default()
        }
    }
}
