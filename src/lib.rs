//! Metrics library for iroh
#![deny(missing_docs, rustdoc::broken_intra_doc_links)]
#![cfg_attr(iroh_docsrs, feature(doc_auto_cfg))]

/// Exposes core types and traits
mod base;
pub use base::*;
#[cfg(feature = "derive")]
pub use iroh_metrics_derive::MetricsGroup;

mod metrics;
pub use metrics::*;

#[cfg(feature = "static_core")]
pub mod static_core;

#[cfg(feature = "service")]
pub mod service;

use std::collections::HashMap;

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
