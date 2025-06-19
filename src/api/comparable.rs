use std::collections::HashSet;

use k8s_openapi::api::core::v1::Namespace;

// Trait for resources that can be compared
pub(crate) trait ResourceDiff: kube::ResourceExt {
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

        !annotations_equal || !labels_equal || !owner_references_equal
    }
}

impl ResourceDiff for Namespace {}
