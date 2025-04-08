//! Metrics library for iroh
#![deny(missing_docs, rustdoc::broken_intra_doc_links)]
#![cfg_attr(iroh_docsrs, feature(doc_auto_cfg))]

#[cfg(feature = "service")]
pub mod metrics;

/// Exposes core types and traits
mod base;
pub use base::*;

#[cfg(feature = "static_core")]
pub mod static_core;

/// Exposes iroh metrics
#[cfg(feature = "service")]
pub mod service;

use std::{collections::HashMap, time::Duration};

/// Reexports `struct_iterable` to make matching versions easier.
pub use struct_iterable;

/// Potential errors from this library.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Indicates that the metrics have not been enabled.
    #[error("Metrics not enabled")]
    NoMetrics,
    /// Any IO related error.
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
}

/// Parses Prometheus metrics from a string.
pub fn parse_prometheus_metrics(data: &str) -> HashMap<String, f64> {
    let mut metrics = HashMap::new();
    for line in data.lines() {
        if line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let metric = parts[0];
        let value = parts[1].parse::<f64>();
        if value.is_err() {
            continue;
        }
        metrics.insert(metric.to_string(), value.unwrap());
    }
    metrics
}

/// Configuration for pushing metrics to a remote endpoint.
#[derive(PartialEq, Eq, Debug, Default, serde::Deserialize, Clone)]
pub struct PushMetricsConfig {
    /// The push interval.
    pub interval: Duration,
    /// The endpoint url for the push metrics collector.
    pub endpoint: String,
    /// The name of the service you're exporting metrics for.
    ///
    /// Generally, `metrics_exporter` is good enough for use
    /// outside of production deployments.
    pub service_name: String,
    /// The name of the instance you're exporting metrics for.
    ///
    /// This should be device-unique. If not, this will sum up
    /// metrics from different devices.
    ///
    /// E.g. `username-laptop`, `username-phone`, etc.
    ///
    /// Another potential scheme with good privacy would be a
    /// keyed blake3 hash of the secret key. (This gives you
    /// an identifier that is as unique as a `NodeID`, but
    /// can't be correlated to `NodeID`s.)
    pub instance_name: String,
    /// The username for basic auth for the push metrics collector.
    pub username: Option<String>,
    /// The password for basic auth for the push metrics collector.
    pub password: String,
}
