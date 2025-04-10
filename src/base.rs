#[cfg(feature = "metrics")]
pub use prometheus_client::registry::Registry;

use crate::{
    metrics::{Counter, Gauge},
    MetricType,
};

/// Description of a group of metrics.
pub trait MetricsGroup:
    struct_iterable::Iterable + std::fmt::Debug + 'static + Send + Sync
{
    /// Initializes this metric group.
    #[cfg(feature = "metrics")]
    fn register(&self, registry: &mut prometheus_client::registry::Registry) {
        let sub_registry = registry.sub_registry_with_prefix(self.name());

        for (metric, item) in self.iter() {
            if let Some(counter) = item.downcast_ref::<Counter>() {
                sub_registry.register(metric, counter.description, counter.counter.clone());
            }
            if let Some(gauge) = item.downcast_ref::<Gauge>() {
                sub_registry.register(metric, gauge.description, gauge.gauge.clone());
            }
        }
    }

    /// The name of this metric group.
    fn name(&self) -> &'static str;

    /// Returns the metrics descriptions.
    fn describe(&self) -> Vec<MetricDescription> {
        let mut res = vec![];
        for (name, item) in self.iter() {
            if let Some(item) = item.downcast_ref::<Counter>() {
                res.push(MetricDescription {
                    name,
                    description: item.description,
                    r#type: MetricType::Counter,
                });
            }
            if let Some(item) = item.downcast_ref::<Gauge>() {
                res.push(MetricDescription {
                    name,
                    description: item.description,
                    r#type: MetricType::Gauge,
                });
            }
        }
        res
    }

    /// Returns an iterator over all metric items with their values and types.
    fn values(&self) -> ValuesIter {
        ValuesIter { inner: self.iter() }
    }
}

/// Iterator over metric items with their values.
///
/// Returned from [`MetricsGroup::values`].
#[derive(Debug)]
pub struct ValuesIter<'a> {
    inner: std::vec::IntoIter<(&'static str, &'a dyn std::any::Any)>,
}

impl Iterator for ValuesIter<'_> {
    type Item = MetricItem;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (name, item) = self.inner.next()?;
            if let Some(item) = item.downcast_ref::<Counter>() {
                break Some(MetricItem {
                    name,
                    r#type: MetricType::Counter,
                    description: item.description,
                    value: MetricValue::Counter(item.get()),
                });
            } else if let Some(item) = item.downcast_ref::<Gauge>() {
                break Some(MetricItem {
                    name,
                    r#type: MetricType::Gauge,
                    description: item.description,
                    value: MetricValue::Gauge(item.get()),
                });
            } else {
                continue;
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

/// A metric item with its current value.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct MetricItem {
    /// The type of this metric item.
    pub r#type: MetricType,
    /// The name of this metric item.
    pub name: &'static str,
    /// The description of this metric item.
    pub description: &'static str,
    /// The current value.
    pub value: MetricValue,
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

/// Extension methods for types implementing [`MetricsGroup`].
///
/// This contains non-dyn-compatible methods, which is why they can't live on the [`MetricsGroup`] trait.
pub trait MetricsGroupExt: MetricsGroup + Default {
    /// Create a new instance and register with a registry.
    #[cfg(feature = "metrics")]
    fn new(registry: &mut prometheus_client::registry::Registry) -> Self {
        let m = Self::default();
        m.register(registry);
        m
    }
}

impl<T> MetricsGroupExt for T where T: MetricsGroup + Default {}

/// Trait for a set of structs implementing [`MetricsGroup`].
pub trait MetricsGroupSet {
    /// Returns an iterator of references to structs implementing [`MetricsGroup`].
    fn iter(&self) -> impl Iterator<Item = &dyn MetricsGroup>;

    /// Returns the name of this metrics group set.
    fn name(&self) -> &'static str;

    /// Register all metrics groups in this set onto a prometheus client registry.
    #[cfg(feature = "metrics")]
    fn register(&self, registry: &mut prometheus_client::registry::Registry) {
        for metric in self.iter() {
            metric.register(registry)
        }
    }
}

/// Returns the metric item representation.
#[derive(Debug, Clone)]
pub struct MetricDescription {
    /// The name of the metric.
    pub name: &'static str,
    /// The description of the metric.
    pub description: &'static str,
    /// The type of the metric.
    pub r#type: MetricType,
}

/// Ensure metrics can be used without `metrics` feature.
/// All ops are noops then, get always returns 0.
#[cfg(all(test, not(feature = "metrics")))]
mod tests {
    use super::Counter;

    #[test]
    fn test() {
        let counter = Counter::new("foo");
        counter.inc();
        assert_eq!(counter.get(), 0);
    }
}

/// Tests with the `metrics` feature,
#[cfg(all(test, feature = "metrics"))]
mod tests {
    use struct_iterable::Iterable;

    use super::*;

    #[derive(Debug, Clone, Iterable)]
    pub struct FooMetrics {
        pub metric_a: Counter,
        pub metric_b: Counter,
    }

    impl Default for FooMetrics {
        fn default() -> Self {
            Self {
                metric_a: Counter::new("metric_a"),
                metric_b: Counter::new("metric_b"),
            }
        }
    }

    impl MetricsGroup for FooMetrics {
        fn name(&self) -> &'static str {
            "foo"
        }
    }

    #[derive(Debug, Clone, Iterable)]
    pub struct BarMetrics {
        pub count: Counter,
    }

    impl Default for BarMetrics {
        fn default() -> Self {
            Self {
                count: Counter::new("Bar Count"),
            }
        }
    }

    impl MetricsGroup for BarMetrics {
        fn name(&self) -> &'static str {
            "bar"
        }
    }

    #[derive(Debug, Clone, Default)]
    struct CombinedMetrics {
        foo: FooMetrics,
        bar: BarMetrics,
    }

    impl MetricsGroupSet for CombinedMetrics {
        fn name(&self) -> &'static str {
            "combined"
        }

        fn iter(&self) -> impl Iterator<Item = &dyn MetricsGroup> {
            [
                &self.foo as &dyn MetricsGroup,
                &self.bar as &dyn MetricsGroup,
            ]
            .into_iter()
        }
    }

    #[test]
    fn test_metric_description() -> Result<(), Box<dyn std::error::Error>> {
        let metrics = FooMetrics::default();
        let items = metrics.describe();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "metric_a");
        assert_eq!(items[0].description, "metric_a");
        assert_eq!(items[0].r#type, MetricType::Counter);
        assert_eq!(items[1].name, "metric_b");
        assert_eq!(items[1].description, "metric_b");
        assert_eq!(items[1].r#type, MetricType::Counter);

        Ok(())
    }

    #[test]
    fn test_solo_registry() -> Result<(), Box<dyn std::error::Error>> {
        use prometheus_client::{encoding::text::encode, registry::Registry};

        let mut registry = Registry::default();
        let metrics = FooMetrics::default();
        metrics.register(&mut registry);

        metrics.metric_a.inc();
        metrics.metric_b.inc_by(2);
        metrics.metric_b.inc_by(3);
        assert_eq!(metrics.metric_a.get(), 1);
        assert_eq!(metrics.metric_b.get(), 5);
        metrics.metric_a.set(0);
        metrics.metric_b.set(0);
        assert_eq!(metrics.metric_a.get(), 0);
        assert_eq!(metrics.metric_b.get(), 0);
        metrics.metric_a.inc_by(5);
        metrics.metric_b.inc_by(2);
        assert_eq!(metrics.metric_a.get(), 5);
        assert_eq!(metrics.metric_b.get(), 2);

        let exp = "# HELP foo_metric_a metric_a.
# TYPE foo_metric_a counter
foo_metric_a_total 5
# HELP foo_metric_b metric_b.
# TYPE foo_metric_b counter
foo_metric_b_total 2
# EOF
";
        let mut enc = String::new();
        encode(&mut enc, &registry).expect("writing to string always works");

        assert_eq!(enc, exp);
        Ok(())
    }

    #[test]
    fn test_metric_sets() {
        use prometheus_client::{encoding::text::encode, registry::Registry};

        let metrics = CombinedMetrics::default();
        metrics.foo.metric_a.inc();
        metrics.bar.count.inc_by(10);

        let mut collected = vec![];
        // manual collection and iteration with manual downcasting
        for group in metrics.iter() {
            for (name, metric) in group.iter() {
                if let Some(counter) = metric.downcast_ref::<Counter>() {
                    collected.push((group.name(), name, counter.description, counter.get()));
                }
            }
        }
        assert_eq!(
            collected,
            vec![
                ("foo", "metric_a", "metric_a", 1),
                ("foo", "metric_b", "metric_b", 0),
                ("bar", "count", "Bar Count", 10),
            ]
        );

        // automatic collection and encoding with prometheus_client
        let mut registry = Registry::default();
        let sub = registry.sub_registry_with_prefix("combined");
        metrics.register(sub);
        let exp = "# HELP combined_foo_metric_a metric_a.
# TYPE combined_foo_metric_a counter
combined_foo_metric_a_total 1
# HELP combined_foo_metric_b metric_b.
# TYPE combined_foo_metric_b counter
combined_foo_metric_b_total 0
# HELP combined_bar_count Bar Count.
# TYPE combined_bar_count counter
combined_bar_count_total 10
# EOF
";
        let mut enc = String::new();
        encode(&mut enc, &registry).expect("writing to string always works");

        assert_eq!(enc, exp);
    }

    #[cfg(feature = "derive")]
    #[test]
    fn test_derive() {
        use crate::{struct_iterable::Iterable, MetricValue, MetricsGroup};

        #[derive(Debug, Clone, MetricsGroup, Iterable)]
        #[metrics(name = "my-metrics")]
        struct Metrics {
            /// Counts foos
            ///
            /// Only the first line is used for the OpenMetrics description
            foo: Counter,
            // no description: use field name as description
            bar: Counter,
            /// Measures baz
            baz: Gauge,
        }

        let metrics = Metrics::default();

        metrics.foo.inc();
        metrics.bar.inc_by(2);
        metrics.baz.set(3);

        let values: Vec<_> = metrics.values().collect();
        let foo = values[0];
        let bar = values[1];
        let baz = values[2];
        assert_eq!(metrics.name(), "my-metrics");
        assert_eq!(foo.value, MetricValue::Counter(1));
        assert_eq!(foo.name, "foo");
        assert_eq!(foo.description, "Counts foos");
        assert_eq!(bar.value, MetricValue::Counter(2));
        assert_eq!(bar.name, "bar");
        assert_eq!(bar.description, "bar");
        assert_eq!(baz.value, MetricValue::Gauge(3));
        assert_eq!(baz.name, "baz");
        assert_eq!(baz.description, "Measures baz");

        #[derive(Debug, Clone, MetricsGroup, Iterable)]
        struct FooMetrics {}
        let metrics = FooMetrics::default();
        assert_eq!(metrics.name(), "foo_metrics");
    }
}
