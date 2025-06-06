use std::io;

use controllers::{
    BundleError, SyncError,
    addon_config::{
        AddonConfigSyncError, DynamicWatcherError, FleetPatchError, ReconcileConfigSyncError,
    },
    helm,
};
use futures::channel::mpsc::TrySendError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Kube Error: {0}")]
    KubeError(#[from] kube::Error),

    #[error("Config fetch error: {0}")]
    ConfigFetch(#[source] kube::Error),

    #[error("Bundle error: {0}")]
    BundleError(#[from] BundleError),

    #[error("Fleet error: {0}")]
    FleetError(#[from] SyncError),

    #[error("Fleet config error: {0}")]
    FleetConfigError(#[from] AddonConfigSyncError),

    #[error("Fleet chart patch error: {0}")]
    FleetChartPatchError(#[from] FleetPatchError),

    #[error("Fleet repo add error: {0}")]
    RepoAdd(#[from] helm::RepoAddError),

    #[error("Fleet repo update error: {0}")]
    RepoUpdate(#[from] helm::RepoUpdateError),

    #[error("Error waiting for commadnd: {0}")]
    CommandError(#[from] io::Error),

    #[error("Dynamic watcher error: {0}")]
    DynamicWatcherError(#[from] DynamicWatcherError),

    #[error("Reconcile config sync error: {0}")]
    ReconcileConfigSync(#[from] ReconcileConfigSyncError),

    #[error("Namespace trigger error: {0}")]
    TriggerError(#[from] TrySendError<()>),

    #[error("Finalizer Error: {0}")]
    // NB: awkward type because finalizer::Error embeds the reconciler error (which is this)
    // so boxing this error to break cycles
    FinalizerError(#[source] Box<kube::runtime::finalizer::Error<Error>>),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn metric_label(&self) -> String {
        format!("{self:?}").to_lowercase()
    }
}

/// Expose all controller components used by main
pub mod controller;
pub use crate::controller::*;
pub mod api;
pub mod controllers;
mod multi_dispatcher;
pub mod predicates;

/// Log and trace integrations
pub mod telemetry;

/// Metrics
mod metrics;
pub use metrics::Metrics;

/*
#[cfg(test)] pub mod fixtures;
*/
