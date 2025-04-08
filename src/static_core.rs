//! Metrics collection in a static, process-level global metrics collector.
//!
//! Enables and manages a global registry of metrics.
//! Divided up into modules, each module has its own metrics.
//!
//! - To increment a **counter**, use the [`crate::inc`] macro with a value.
//! - To increment a **counter** by 1, use the [`crate::inc_by`] macro.
//!
//! To expose the metrics, start the metrics service with `start_metrics_server()`.
//!
//! # Example:
//! ```rust
//! use iroh_metrics::{inc, inc_by, static_core::Core, Counter, Metric};
//! use struct_iterable::Iterable;
//!
//! #[derive(Debug, Clone, Iterable)]
//! pub struct Metrics {
//!     pub things_added: Counter,
//! }
//!
//! impl Default for Metrics {
//!     fn default() -> Self {
//!         Self {
//!             things_added: Counter::new(
//!                 "things_added tracks the number of things we have added",
//!             ),
//!         }
//!     }
//! }
//!
//! impl Metric for Metrics {
//!     fn name(&self) -> &'static str {
//!         "my_metrics"
//!     }
//! }
//!
//! Core::init(|reg, metrics| {
//!     let m = Metrics::default();
//!     m.register(reg);
//!     metrics.insert(m);
//! });
//!
//! inc_by!(Metrics, things_added, 2);
//! inc!(Metrics, things_added);
//! ```

use std::sync::OnceLock;

use erased_set::ErasedSyncSet;
#[cfg(feature = "metrics")]
use prometheus_client::{encoding::text::encode, registry::Registry};

use crate::base::Metric;

#[cfg(not(feature = "metrics"))]
type Registry = ();

/// This struct can be used with the functions in [`crate::service`] to use them with
/// the global static [`Core`] defined in this module.
#[cfg(feature = "service")]
#[derive(Clone, Copy, Debug)]
pub struct GlobalRegistry;

#[cfg(feature = "service")]
impl crate::service::MetricsSource for GlobalRegistry {
    fn encode_openmetrics(&self) -> Result<String, crate::Error> {
        let core = crate::static_core::Core::get().ok_or(crate::Error::NoMetrics)?;
        Ok(core.encode())
    }
}

static CORE: OnceLock<Core> = OnceLock::new();

/// Core is the base metrics struct.
///
/// It manages the mapping between the metrics name and the actual metrics.
/// It also carries a single prometheus registry to be used by all metrics.
#[derive(Debug, Default)]
pub struct Core {
    #[cfg(feature = "metrics")]
    registry: Registry,
    metrics_map: ErasedSyncSet,
}

impl Core {
    /// Must only be called once to init metrics.
    ///
    /// Panics if called a second time.
    pub fn init<F: FnOnce(&mut Registry, &mut ErasedSyncSet)>(f: F) {
        Self::try_init(f).expect("must only be called once");
    }

    /// Trieds to init the metrics.
    #[cfg_attr(not(feature = "metrics"), allow(clippy::let_unit_value))]
    pub fn try_init<F: FnOnce(&mut Registry, &mut ErasedSyncSet)>(f: F) -> std::io::Result<()> {
        let mut registry = Registry::default();
        let mut metrics_map = ErasedSyncSet::new();
        f(&mut registry, &mut metrics_map);

        CORE.set(Core {
            metrics_map,
            #[cfg(feature = "metrics")]
            registry,
        })
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "already set"))
    }

    /// Returns a reference to the core metrics.
    pub fn get() -> Option<&'static Self> {
        CORE.get()
    }

    /// Returns a reference to the prometheus registry.
    #[cfg(feature = "metrics")]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Returns a reference to the mapped metrics instance.
    pub fn get_collector<T: Metric>(&self) -> Option<&T> {
        self.metrics_map.get::<T>()
    }

    /// Encodes the current metrics registry to a string in
    /// the prometheus text exposition format.
    #[cfg(feature = "metrics")]
    pub fn encode(&self) -> String {
        let mut buf = String::new();
        encode(&mut buf, &self.registry).expect("writing to string always works");
        buf
    }
}

/// Increments the given counter or gauge by 1.
#[macro_export]
macro_rules! inc {
    ($m:ty, $f:ident) => {
        if let Some(m) = $crate::static_core::Core::get().and_then(|c| c.get_collector::<$m>()) {
            m.$f.inc();
        }
    };
}

/// Increments the given counter or gauge by `n`.
#[macro_export]
macro_rules! inc_by {
    ($m:ty, $f:ident, $n:expr) => {
        if let Some(m) = $crate::static_core::Core::get().and_then(|c| c.get_collector::<$m>()) {
            m.$f.inc_by($n);
        }
    };
}

/// Sets the given counter or gauge to `n`.
#[macro_export]
macro_rules! set {
    ($m:ty, $f:ident, $n:expr) => {
        <$m as $crate::static_core::Metric>::with_metric(|m| m.$f.set($n));
    };
}

/// Decrements the given gauge by 1.
#[macro_export]
macro_rules! dec {
    ($m:ty, $f:ident) => {
        if let Some(m) = $crate::static_core::Core::get().and_then(|c| c.get_collector::<$m>()) {
            m.$f.dec();
        }
    };
}

/// Decrements the given gauge `n`.
#[macro_export]
macro_rules! dec_by {
    ($m:ty, $f:ident, $n:expr) => {
        if let Some(m) = $crate::static_core::Core::get().and_then(|c| c.get_collector::<$m>()) {
            m.$f.dec_by($n);
        }
    };
}
