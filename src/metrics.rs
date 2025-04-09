//! This module defines the individual metric types.
//!
//! If the `metrics` feature is enabled, they contain metric types based on atomics
//! which can be modified without needing mutable access.
//!
//! If the `metrics` feature is disabled, all operations defined on these types are noops,
//! and the structs don't collect actual data.

/// The types of metrics supported by this crate.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MetricType {
    /// A [`Counter].
    Counter,
    /// A [`Gauge].
    Gauge,
}

/// Open Metrics [`Counter`] to measure discrete events.
///
/// Single monotonically increasing value metric.
#[derive(Debug, Clone)]
pub struct Counter {
    /// The actual prometheus counter.
    #[cfg(feature = "metrics")]
    pub counter: prometheus_client::metrics::counter::Counter,
    /// What this counter measures.
    pub description: &'static str,
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
    pub gauge: prometheus_client::metrics::gauge::Gauge,
    /// What this gauge tracks.
    pub description: &'static str,
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
