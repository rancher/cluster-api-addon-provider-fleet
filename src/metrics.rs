use std::sync::Arc;

use crate::Error;
use jiff::Timestamp;
use kube::{
    Client, ResourceExt,
    runtime::events::{Recorder, Reporter},
};
use prometheus::{HistogramVec, IntCounter, IntCounterVec, Registry, histogram_opts, opts};
use serde::Serialize;
use tokio::time::Instant;

#[derive(Clone)]
pub struct Metrics {
    pub reconciliations: IntCounter,
    pub failures: IntCounterVec,
    pub reconcile_duration: HistogramVec,
}

impl Default for Metrics {
    fn default() -> Self {
        let reconcile_duration = HistogramVec::new(
            histogram_opts!(
                "caapf_controller_reconcile_duration_seconds",
                "The duration of reconcile to complete in seconds"
            )
            .buckets(vec![0.01, 0.1, 0.25, 0.5, 1., 5., 15., 60.]),
            &[],
        )
        .unwrap();
        let failures = IntCounterVec::new(
            opts!(
                "caapf_controller_reconciliation_errors_total",
                "reconciliation errors",
            ),
            &["instance", "error"],
        )
        .unwrap();
        let reconciliations =
            IntCounter::new("caapf_controller_reconciliations_total", "reconciliations").unwrap();
        Metrics {
            reconciliations,
            failures,
            reconcile_duration,
        }
    }
}

impl Metrics {
    /// Register API metrics to start tracking them.
    /// Register metrics with the provided registry.
    ///
    /// # Errors
    ///
    /// Returns `prometheus::Error` if:
    /// - A metric with the same name is already registered
    /// - Metric names don't follow naming conventions
    /// - Other registry-related errors occur
    pub fn register(self, registry: &Registry) -> Result<Self, prometheus::Error> {
        registry.register(Box::new(self.reconcile_duration.clone()))?;
        registry.register(Box::new(self.failures.clone()))?;
        registry.register(Box::new(self.reconciliations.clone()))?;
        Ok(self)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn reconcile_failure<C: kube::Resource>(&self, obj: Arc<C>, e: &Error) {
        self.failures
            .with_label_values(&[obj.name_any(), e.metric_label()])
            .inc();
    }

    #[must_use]
    pub fn count_and_measure(&self) -> ReconcileMeasurer {
        self.reconciliations.inc();
        ReconcileMeasurer {
            start: Instant::now(),
            metric: self.reconcile_duration.clone(),
        }
    }
}

/// Diagnostics to be exposed by the web server
#[derive(Clone, Serialize)]
pub struct Diagnostics {
    #[serde(deserialize_with = "from_ts")]
    pub last_event: Timestamp,
    #[serde(skip)]
    pub reporter: Reporter,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            last_event: Timestamp::now(),
            reporter: "caapf-controller".into(),
        }
    }
}

impl Diagnostics {
    pub fn recorder(&self, client: Client) -> Recorder {
        Recorder::new(client, self.reporter.clone())
    }
}

/// Smart function duration measurer
///
/// Relies on Drop to calculate duration and register the observation in the histogram
pub struct ReconcileMeasurer {
    start: Instant,
    metric: HistogramVec,
}

impl Drop for ReconcileMeasurer {
    fn drop(&mut self) {
        #[allow(clippy::cast_precision_loss)]
        let duration = self.start.elapsed().as_millis() as f64 / 1000.0;
        self.metric.with_label_values::<&str>(&[]).observe(duration);
    }
}
