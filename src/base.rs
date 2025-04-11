use std::any::Any;

#[cfg(feature = "metrics")]
pub use prometheus_client::registry::Registry;

use crate::{
    iterable::{FieldIter, IntoIterable, Iterable},
    Metric,
};

/// Trait for structs containing metric items.
pub trait MetricsGroup:
    Any + Iterable + IntoIterable + std::fmt::Debug + 'static + Send + Sync
{
    /// Registers all metric items in this metrics group to a [`prometheus_client::registry::Registry`].
    #[cfg(feature = "metrics")]
    fn register(&self, registry: &mut prometheus_client::registry::Registry) {
        use crate::{Counter, Gauge};
        let sub_registry = registry.sub_registry_with_prefix(self.name());
        for item in self.iter() {
            if let Some(counter) = item.as_any().downcast_ref::<Counter>() {
                sub_registry.register(item.name(), item.description(), counter.counter.clone());
            }
            if let Some(gauge) = item.as_any().downcast_ref::<Gauge>() {
                sub_registry.register(item.name(), item.description(), gauge.gauge.clone());
            }
        }
    }

    /// Returns the name of this metrics group.
    fn name(&self) -> &'static str;

    /// Returns an iterator over all metric items with their values and descriptions.
    fn iter(&self) -> MetricsIter {
        MetricsIter {
            inner: self.field_iter(),
        }
    }
}

/// Iterator over metric items.
///
/// Returned from [`MetricsGroup::iter`].
#[derive(Debug)]
pub struct MetricsIter<'a> {
    inner: FieldIter<'a>,
}

impl<'a> Iterator for MetricsIter<'a> {
    type Item = MetricItem<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let (name, metric) = self.inner.next()?;
        Some(MetricItem { name, metric })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

/// A metric item with its current value.
#[derive(Debug, Clone, Copy)]
pub struct MetricItem<'a> {
    name: &'static str,
    metric: &'a dyn Metric,
}

impl MetricItem<'_> {
    /// Returns the name of this metric item.
    pub fn name(&self) -> &'static str {
        self.name
    }
}

impl<'a> std::ops::Deref for MetricItem<'a> {
    type Target = &'a dyn Metric;
    fn deref(&self) -> &Self::Target {
        &self.metric
    }
}

/// Trait for a set of structs implementing [`MetricsGroup`].
pub trait MetricsGroupSet {
    /// Returns the name of this metrics group set.
    fn name(&self) -> &'static str;

    /// Returns an iterator over all metrics in this metrics group set.
    ///
    /// The iterator yields tuples of `(&str, MetricItem)`. The `&str` is the group name.
    fn iter(&self) -> impl Iterator<Item = (&'static str, MetricItem<'_>)> {
        self.groups()
            .flat_map(|group| group.iter().map(|item| (group.name(), item)))
    }

    /// Returns an iterator over the [`MetricsGroup`] in this struct.
    fn groups(&self) -> impl Iterator<Item = &dyn MetricsGroup>;

    /// Register all metrics groups in this set onto a prometheus client registry.
    #[cfg(feature = "metrics")]
    fn register(&self, registry: &mut prometheus_client::registry::Registry) {
        for group in self.groups() {
            group.register(registry)
        }
    }
}

/// Ensure metrics can be used without `metrics` feature.
/// All ops are noops then, get always returns 0.
#[cfg(all(test, not(feature = "metrics")))]
mod tests {
    use crate::Counter;

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
    use super::*;
    use crate::{iterable::Iterable, Counter, Gauge, MetricType};

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

        fn groups(&self) -> impl Iterator<Item = &dyn MetricsGroup> {
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
        let items: Vec<_> = metrics.iter().collect();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name(), "metric_a");
        assert_eq!(items[0].description(), "metric_a");
        assert_eq!(items[0].r#type(), MetricType::Counter);
        assert_eq!(items[1].name(), "metric_b");
        assert_eq!(items[1].description(), "metric_b");
        assert_eq!(items[1].r#type(), MetricType::Counter);

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

        // Using `iter` to iterate over all metrics in the group set.
        let collected = metrics.iter().map(|(group, metric)| {
            (
                group,
                metric.name(),
                metric.description(),
                metric.value().to_f32(),
            )
        });
        assert_eq!(
            collected.collect::<Vec<_>>(),
            vec![
                ("foo", "metric_a", "metric_a", 1.0),
                ("foo", "metric_b", "metric_b", 0.0),
                ("bar", "count", "Bar Count", 10.0),
            ]
        );

        // Using manual downcasting.
        let mut collected = vec![];
        for group in metrics.groups() {
            for metric in group.iter() {
                if let Some(counter) = metric.as_any().downcast_ref::<Counter>() {
                    collected.push((group.name(), metric.name(), counter.get()));
                }
            }
        }
        assert_eq!(
            collected,
            vec![
                ("foo", "metric_a", 1),
                ("foo", "metric_b", 0),
                ("bar", "count", 10),
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

    #[test]
    fn test_derive() {
        use crate::{MetricValue, MetricsGroup};

        #[derive(Debug, Clone, MetricsGroup)]
        #[metrics(name = "my-metrics")]
        struct Metrics {
            /// Counts foos
            ///
            /// Only the first line is used for the OpenMetrics description
            foo: Counter,
            // no description: use field name as description
            bar: Counter,
            /// This docstring is not used as prometheus description
            #[metrics(description = "Measures baz")]
            baz: Gauge,
        }

        let metrics = Metrics::default();
        assert_eq!(metrics.name(), "my-metrics");

        metrics.foo.inc();
        metrics.bar.inc_by(2);
        metrics.baz.set(3);

        let mut values = metrics.iter();
        let foo = values.next().unwrap();
        let bar = values.next().unwrap();
        let baz = values.next().unwrap();
        assert_eq!(foo.value(), MetricValue::Counter(1));
        assert_eq!(foo.name(), "foo");
        assert_eq!(foo.description(), "Counts foos");
        assert_eq!(bar.value(), MetricValue::Counter(2));
        assert_eq!(bar.name(), "bar");
        assert_eq!(bar.description(), "bar");
        assert_eq!(baz.value(), MetricValue::Gauge(3));
        assert_eq!(baz.name(), "baz");
        assert_eq!(baz.description(), "Measures baz");

        #[derive(Debug, Clone, MetricsGroup)]
        struct FooMetrics {}
        let metrics = FooMetrics::default();
        assert_eq!(metrics.name(), "foo_metrics");
        let mut values = metrics.iter();
        assert!(values.next().is_none());
    }
}
