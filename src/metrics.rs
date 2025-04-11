//! This module defines the individual metric types.
//!
//! If the `metrics` feature is enabled, they contain metric types based on atomics
//! which can be modified without needing mutable access.
//!
//! If the `metrics` feature is disabled, all operations defined on these types are noops,
//! and the structs don't collect actual data.

use std::any::Any;

/// The types of metrics supported by this crate.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum MetricType {
    /// A [`Counter].
    Counter,
    /// A [`Gauge].
    Gauge,
}

/// The value of an individual metric item.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MetricValue {
    /// A [`Counter`] value.
    Counter(u64),
    /// A [`Gauge`] value.
    Gauge(i64),
}

impl MetricValue {
    /// Returns the value as [`f32`].
    pub fn to_f32(&self) -> f32 {
        match self {
            MetricValue::Counter(value) => *value as f32,
            MetricValue::Gauge(value) => *value as f32,
        }
    }
}

/// Trait for metric items.
pub trait Metric: std::fmt::Debug {
    /// The type of this metric.
    fn r#type(&self) -> MetricType;

    /// The current value of this metric.
    fn value(&self) -> MetricValue;

    /// The description of this metric.
    fn description(&self) -> &'static str;

    /// Cast this metric to [`Any`] for downcasting to concrete types.
    fn as_any(&self) -> &dyn Any;
}

/// Open Metrics [`Counter`] to measure discrete events.
///
/// Single monotonically increasing value metric.
#[derive(Debug, Clone)]
pub struct Counter {
    /// The actual prometheus counter.
    #[cfg(feature = "metrics")]
    pub(crate) counter: prometheus_client::metrics::counter::Counter,
    /// What this counter measures.
    description: &'static str,
}

impl Metric for Counter {
    fn value(&self) -> MetricValue {
        MetricValue::Counter(self.get())
    }

    fn r#type(&self) -> MetricType {
        MetricType::Counter
    }

    fn description(&self) -> &'static str {
        self.description
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Counter {
    /// Constructs a new counter, based on the given `description`.
    pub fn new(description: &'static str) -> Self {
        Counter {
            #[cfg(feature = "metrics")]
            counter: Default::default(),
            description,
        }
    }

    /// Increase the [`Counter`] by 1, returning the previous value.
    pub fn inc(&self) -> u64 {
        #[cfg(feature = "metrics")]
        {
            self.counter.inc()
        }
        #[cfg(not(feature = "metrics"))]
        0
    }

    /// Increase the [`Counter`] by `u64`, returning the previous value.
    #[cfg(feature = "metrics")]
    pub fn inc_by(&self, v: u64) -> u64 {
        self.counter.inc_by(v)
    }

    /// Set the [`Counter`] value.
    /// Warning: this is not default behavior for a counter that should always be monotonically increasing.
    #[cfg(feature = "metrics")]
    pub fn set(&self, v: u64) -> u64 {
        self.counter
            .inner()
            .store(v, std::sync::atomic::Ordering::Relaxed);
        v
    }

    /// Set the [`Counter`] value.
    /// Warning: this is not default behavior for a counter that should always be monotonically increasing.
    #[cfg(not(feature = "metrics"))]
    pub fn set(&self, _v: u64) -> u64 {
        0
    }

    /// Increase the [`Counter`] by `u64`, returning the previous value.
    #[cfg(not(feature = "metrics"))]
    pub fn inc_by(&self, _v: u64) -> u64 {
        0
    }

    /// Get the current value of the [`Counter`].
    pub fn get(&self) -> u64 {
        #[cfg(feature = "metrics")]
        {
            self.counter.get()
        }
        #[cfg(not(feature = "metrics"))]
        0
    }
}

/// Open Metrics [`Gauge`].
#[derive(Debug, Clone)]
pub struct Gauge {
    /// The actual prometheus gauge.
    #[cfg(feature = "metrics")]
    pub(crate) gauge: prometheus_client::metrics::gauge::Gauge,
    /// What this gauge tracks.
    description: &'static str,
}

impl Metric for Gauge {
    fn r#type(&self) -> MetricType {
        MetricType::Gauge
    }

    fn description(&self) -> &'static str {
        self.description
    }

    fn value(&self) -> MetricValue {
        MetricValue::Gauge(self.get())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Gauge {
    /// Constructs a new gauge, based on the given `description`.
    pub fn new(description: &'static str) -> Self {
        Self {
            #[cfg(feature = "metrics")]
            gauge: Default::default(),
            description,
        }
    }

    /// Increase the [`Gauge`] by 1, returning the previous value.
    pub fn inc(&self) -> i64 {
        #[cfg(feature = "metrics")]
        {
            self.gauge.inc()
        }
        #[cfg(not(feature = "metrics"))]
        0
    }
    /// Increase the [`Gauge`] by `i64`, returning the previous value.
    #[cfg(feature = "metrics")]
    pub fn inc_by(&self, v: i64) -> i64 {
        self.gauge.inc_by(v)
    }
    /// Increase the [`Gauge`] by `i64`, returning the previous value.
    #[cfg(not(feature = "metrics"))]
    pub fn inc_by(&self, _v: u64) -> u64 {
        0
    }

    /// Decrease the [`Gauge`] by 1, returning the previous value.
    pub fn dec(&self) -> i64 {
        #[cfg(feature = "metrics")]
        {
            self.gauge.dec()
        }
        #[cfg(not(feature = "metrics"))]
        0
    }
    /// Decrease the [`Gauge`] by `i64`, returning the previous value.
    #[cfg(feature = "metrics")]
    pub fn dec_by(&self, v: i64) -> i64 {
        self.gauge.dec_by(v)
    }
    /// Decrease the [`Gauge`] by `i64`, returning the previous value.
    #[cfg(not(feature = "metrics"))]
    pub fn dec_by(&self, _v: u64) -> u64 {
        0
    }

    /// Set the [`Gauge`] value.
    #[cfg(feature = "metrics")]
    pub fn set(&self, v: i64) -> i64 {
        self.gauge
            .inner()
            .store(v, std::sync::atomic::Ordering::Relaxed);
        v
    }
    /// Set the [`Gauge`] value.
    #[cfg(not(feature = "metrics"))]
    pub fn set(&self, _v: i64) -> i64 {
        0
    }

    /// Get the [`Gauge`] value.
    #[cfg(feature = "metrics")]
    pub fn get(&self) -> i64 {
        self.gauge
            .inner()
            .load(std::sync::atomic::Ordering::Relaxed)
    }
    /// Get the [`Gauge`] value.
    #[cfg(not(feature = "metrics"))]
    pub fn get(&self) -> i64 {
        0
    }
}
