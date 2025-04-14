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
#[non_exhaustive]
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
    /// Returns the type of this metric.
    fn r#type(&self) -> MetricType;

    /// Returns the current value of this metric.
    fn value(&self) -> MetricValue;

    /// Returns the help string for this metric.
    fn help(&self) -> &'static str;

    /// Casts this metric to [`Any`] for downcasting to concrete types.
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
    help: &'static str,
}

impl Metric for Counter {
    fn value(&self) -> MetricValue {
        MetricValue::Counter(self.get())
    }

    fn r#type(&self) -> MetricType {
        MetricType::Counter
    }

    fn help(&self) -> &'static str {
        self.help
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Counter {
    /// Constructs a new counter, based on the given `help`.
    pub fn new(help: &'static str) -> Self {
        Counter {
            #[cfg(feature = "metrics")]
            counter: Default::default(),
            help,
        }
    }

    /// Increases the [`Counter`] by 1, returning the previous value.
    pub fn inc(&self) -> u64 {
        #[cfg(feature = "metrics")]
        {
            self.counter.inc()
        }
        #[cfg(not(feature = "metrics"))]
        0
    }

    /// Increases the [`Counter`] by `u64`, returning the previous value.
    pub fn inc_by(&self, v: u64) -> u64 {
        #[cfg(feature = "metrics")]
        {
            self.counter.inc_by(v)
        }
        #[cfg(not(feature = "metrics"))]
        {
            let _ = v;
            0
        }
    }

    /// Sets the [`Counter`] value, returning the previous value.
    ///
    /// Warning: this is not default behavior for a counter that should always be monotonically increasing.
    pub fn set(&self, v: u64) -> u64 {
        #[cfg(feature = "metrics")]
        {
            self.counter
                .inner()
                .swap(v, std::sync::atomic::Ordering::Relaxed)
        }
        #[cfg(not(feature = "metrics"))]
        {
            let _ = v;
            0
        }
    }

    /// Returns the current value of the [`Counter`].
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
    help: &'static str,
}

impl Metric for Gauge {
    fn r#type(&self) -> MetricType {
        MetricType::Gauge
    }

    fn help(&self) -> &'static str {
        self.help
    }

    fn value(&self) -> MetricValue {
        MetricValue::Gauge(self.get())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Gauge {
    /// Constructs a new gauge, based on the given `help`.
    pub fn new(help: &'static str) -> Self {
        Self {
            #[cfg(feature = "metrics")]
            gauge: Default::default(),
            help,
        }
    }

    /// Increases the [`Gauge`] by 1, returning the previous value.
    pub fn inc(&self) -> i64 {
        #[cfg(feature = "metrics")]
        {
            self.gauge.inc()
        }
        #[cfg(not(feature = "metrics"))]
        0
    }

    /// Increases the [`Gauge`] by `i64`, returning the previous value.
    pub fn inc_by(&self, v: i64) -> i64 {
        #[cfg(feature = "metrics")]
        {
            self.gauge.inc_by(v)
        }
        #[cfg(not(feature = "metrics"))]
        {
            let _ = v;
            0
        }
    }

    /// Decreases the [`Gauge`] by 1, returning the previous value.
    pub fn dec(&self) -> i64 {
        #[cfg(feature = "metrics")]
        {
            self.gauge.dec()
        }
        #[cfg(not(feature = "metrics"))]
        0
    }

    /// Decreases the [`Gauge`] by `i64`, returning the previous value.
    pub fn dec_by(&self, v: i64) -> i64 {
        #[cfg(feature = "metrics")]
        {
            self.gauge.dec_by(v)
        }
        #[cfg(not(feature = "metrics"))]
        {
            let _ = v;
            0
        }
    }

    /// Sets the [`Gauge`] to `v`, returning the previous value.
    pub fn set(&self, v: i64) -> i64 {
        #[cfg(feature = "metrics")]
        {
            self.gauge.set(v)
        }
        #[cfg(not(feature = "metrics"))]
        {
            let _ = v;
            0
        }
    }

    /// Returns the [`Gauge`] value.
    pub fn get(&self) -> i64 {
        #[cfg(feature = "metrics")]
        {
            self.gauge.get()
        }
        #[cfg(not(feature = "metrics"))]
        0
    }
}
