use thiserror::Error;

#[derive(Error, Debug)]
pub enum SyncError {
    #[error("{0}")]
    ClusterSync(#[from] ClusterSyncError),

    #[error("{0}")]
    GroupSync(#[from] GroupSyncError),

    #[error("{0}")]
    LabelCheck(#[from] LabelCheckError),

    #[error("Cluster registration token create error {0}")]
    ClusterRegistrationTokenSync(#[from] GetOrCreateError),

    #[error("BundleNamespaceMapping delete error: {0}")]
    BundleNsMappingDelete(#[from] kube::Error),
}

pub type ClusterSyncResult<T, E = ClusterSyncError> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum ClusterSyncError {
    #[error("Cluster create error: {0}")]
    GetOrCreateError(#[from] GetOrCreateError),

    #[error("Cluster update error: {0}")]
    PatchError(#[from] PatchError),

    #[error("Cluster group update error: {0}")]
    GroupPatchError(#[source] PatchError),

    #[error("Namespace annotations update error: {0}")]
    NamespacePatchError(#[source] PatchError),

    #[error("Cluster BundleNamespaceMapping update error: {0}")]
    BundleNamespaceMappingError(#[source] PatchError),

    #[error("Cluster BundleNamespaceMapping lookup error")]
    MappingLookupError(#[from] kube::Error),

    #[error("Cluster json encoding error: {0}")]
    ClusterEncodeError(#[from] serde_json::Error),
}

pub type GroupSyncResult<T, E = GroupSyncError> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum GroupSyncError {
    #[error("Cluster group create error: {0}")]
    GetOrCreateError(#[from] GetOrCreateError),

    #[error("Cluster group update error: {0}")]
    PatchError(#[from] PatchError),

    #[error("Unable to find origin ClusterClass for the ClusterGroup: {0}")]
    ClassLookup(#[from] kube::Error),
}

pub type GetOrCreateResult<T, E = GetOrCreateError> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum GetOrCreateError {
    #[error("Lookup error: {0}")]
    Lookup(#[source] kube::Error),

    #[error("Create error: {0}")]
    Create(#[source] kube::Error),

    #[error("Diagnostics error: {0}")]
    Event(#[from] kube::Error),
}

pub type LabelCheckResult<T, E = LabelCheckError> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum LabelCheckError {
    #[error("Namespace lookup error: {0}")]
    NamespaceLookup(#[from] kube::Error),

    #[error("Parse expression error: {0}")]
    Expression(#[from] kube::core::ParseExpressionError),
}

pub type PatchResult<T, E = PatchError> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum PatchError {
    #[error("Get error: {0}")]
    Get(#[source] kube::Error),

    #[error("Patch error: {0}")]
    Patch(#[source] kube::Error),

    #[error("Diagnostics error: {0}")]
    Event(#[from] kube::Error),
}

pub type BundleResult<T, E = BundleError> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum BundleError {
    #[error("Label Check error: {0}")]
    LabelCheck(#[from] LabelCheckError),

    #[error("{0}")]
    Config(#[from] ConfigFetchError),

    #[error("BundleNamespaceMapping creating error: {0}")]
    Mapping(#[from] BundleMappingError),
}

#[derive(Error, Debug)]
pub enum BundleMappingError {
    #[error("ClusterClass lookup error: {0}")]
    ClusterClassLookup(#[from] kube::Error),
}

pub type ConfigFetchResult<T> = std::result::Result<T, ConfigFetchError>;

#[derive(Error, Debug)]
pub enum ConfigFetchError {
    #[error("Config lookup error: {0}")]
    Lookup(#[from] kube::Error),
}

pub mod addon_config;
pub mod cluster;
pub mod cluster_class;
pub mod cluster_group;
pub mod controller;
pub mod helm;
