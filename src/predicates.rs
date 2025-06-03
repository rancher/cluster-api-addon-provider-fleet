use kube::runtime::predicates;
use kube::ResourceExt;

pub fn generation_with_deletion(obj: &impl ResourceExt) -> Option<u64> {
    match obj.meta().deletion_timestamp {
        Some(_) => predicates::resource_version(obj),
        None => predicates::generation(obj),
    }
}

/// Filters known Cluster annotations that do not need propagation. 
pub fn annotation_filter(key: &str) -> bool {
    !key.contains("kubernetes.io/") && !key.contains("k8s.io/")
}
