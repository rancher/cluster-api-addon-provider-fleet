// Trait for resources that can be compared
pub(crate) trait ResourceDiff: kube::ResourceExt {
    fn diff(&self, other: &Self) -> bool {
        self.meta() != other.meta()
    }
}
