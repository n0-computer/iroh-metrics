use std::sync::OnceLock;

use erased_set::ErasedSyncSet;
#[cfg(feature = "metrics")]
use prometheus_client::{encoding::text::encode, registry::Registry};
#[cfg(not(feature = "metrics"))]
type Registry = ();

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

/// Description of a group of metrics.
pub trait Metric:
    Default + struct_iterable::Iterable + Sized + std::fmt::Debug + 'static + Send + Sync
{
    /// Initializes this metric group.
    #[cfg(feature = "metrics")]
    fn new(registry: &mut prometheus_client::registry::Registry) -> Self {
        let sub_registry = registry.sub_registry_with_prefix(Self::name());

        let this = Self::default();
        for (metric, counter) in this.iter() {
            if let Some(counter) = counter.downcast_ref::<Counter>() {
                sub_registry.register(metric, counter.description, counter.counter.clone());
            }
        }
        this
    }

    /// Initializes this metric group.
    #[cfg(not(feature = "metrics"))]
    fn new(_: &mut ()) -> Self {
        Self::default()
    }

    /// The name of this metric group.
    fn name() -> &'static str;

    /// Access to this metrics group to record a metric.
    /// Only records if this metric is registered in the global registry.
    #[cfg(feature = "metrics")]
    fn with_metric<T, F: FnOnce(&Self) -> T>(f: F) {
        Self::try_get().map(f);
    }

    /// Access to this metrics group to record a metric.
    #[cfg(not(feature = "metrics"))]
    fn with_metric<T, F: FnOnce(&Self) -> T>(_f: F) {
        // nothing to do
    }

    /// Attempts to get the current metric from the global registry.
    fn try_get() -> Option<&'static Self> {
        Core::get().and_then(|c| c.get_collector::<Self>())
    }
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

/// Interface for all single value based metrics.
pub trait MetricType {
    /// Returns the name of the metric
    fn name(&self) -> &'static str;
}

/// Interface for all distribution based metrics.
pub trait HistogramType {
    /// Returns the name of the metric
    fn name(&self) -> &'static str;
}
