use kube::ResourceExt;
use kube::runtime::predicates;

pub fn generation_with_deletion(obj: &impl ResourceExt) -> Option<u64> {
    match obj.meta().deletion_timestamp {
        Some(_) => predicates::resource_version(obj),
        None => predicates::generation(obj),
    }
}
