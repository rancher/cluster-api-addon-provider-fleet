use fleet_api_rs::fleet_cluster::{ClusterSpec, ClusterStatus};
use kube::{
    Resource, ResourceExt,
    api::{ObjectMeta, TypeMeta},
};
use serde::{Deserialize, Serialize};

use crate::api::comparable::ResourceDiff;
use std::collections::HashSet;

#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[resource(inherit = fleet_api_rs::fleet_cluster::Cluster)]
pub struct Cluster {
    #[serde(flatten, default)]
    pub types: Option<TypeMeta>,
    pub metadata: ObjectMeta,
    pub spec: ClusterSpec,
    pub status: Option<ClusterStatus>,
}

impl ResourceDiff for Cluster {
    fn diff(&self, other: &Self) -> bool {
        // Resource was just created
        if other.status.is_none() {
            return true;
        }

        let template_values_equal = self
            .spec
            .template_values
            .as_ref()
            .unwrap_or(&std::collections::BTreeMap::new())
            .iter()
            .all(|(k, v)| {
                other
                    .spec
                    .template_values
                    .as_ref()
                    .unwrap_or(&std::collections::BTreeMap::new())
                    .get(k)
                    == Some(v)
            });

        let spec_equal = template_values_equal
            && self.spec.agent_namespace == other.spec.agent_namespace
            && self.spec.host_network == other.spec.host_network
            && self.spec.agent_env_vars == other.spec.agent_env_vars
            && self.spec.agent_tolerations == other.spec.agent_tolerations;

        if !spec_equal {
            return true;
        }

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

        !annotations_equal || !labels_equal || !owner_references_equal
    }
}
