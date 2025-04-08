pub use crate::metrics::{Counter, Gauge};

/// Description of a group of metrics.
pub trait Metric: struct_iterable::Iterable + std::fmt::Debug + 'static + Send + Sync {
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
    fn describe(&self) -> Vec<MetricItem> {
        let mut res = vec![];
        for (metric, counter) in self.iter() {
            if let Some(item) = counter.downcast_ref::<Counter>() {
                res.push(MetricItem {
                    name: metric.to_string(),
                    description: item.description.to_string(),
                    r#type: "counter".to_string(),
                });
            }
            if let Some(item) = counter.downcast_ref::<Gauge>() {
                res.push(MetricItem {
                    name: metric.to_string(),
                    description: item.description.to_string(),
                    r#type: "gauge".to_string(),
                });
            }
        }
        res
    }
}

/// Extension methods for types implementing [`Metric`].
///
/// This contains non-dyn-compatible methods, which is why they can't live on the [`Metric`] trait.
pub trait MetricExt: Metric + Default {
    /// Create a new instance and register with a registry.
    #[cfg(feature = "metrics")]
    fn new(registry: &mut prometheus_client::registry::Registry) -> Self {
        let m = Self::default();
        m.register(registry);
        m
    }
}

impl<T> MetricExt for T where T: Metric + Default {}

/// Trait for a set of structs implementing [`Metric`].
pub trait MetricSet {
    /// Returns an iterator of references to structs implmenting [`Metric`].
    fn iter<'a>(&'a self) -> impl IntoIterator<Item = &'a dyn Metric>;

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

/// Interface for all single value based metrics.
pub trait MetricType {
    /// Returns the name of the metric
    fn name(&self) -> &'static str;
}

/// Returns the metric item representation.
#[derive(Debug, Clone)]
pub struct MetricItem {
    /// The name of the metric.
    pub name: String,
    /// The description of the metric.
    pub description: String,
    /// The type of the metric.
    pub r#type: String,
}

/// Interface for all distribution based metrics.
pub trait HistogramType {
    /// Returns the name of the metric
    fn name(&self) -> &'static str;
}

#[cfg(test)]
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

    impl Metric for FooMetrics {
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

    impl Metric for BarMetrics {
        fn name(&self) -> &'static str {
            "bar"
        }
    }

    #[derive(Debug, Clone, Default)]
    struct CombinedMetrics {
        foo: FooMetrics,
        bar: BarMetrics,
    }

    impl MetricSet for CombinedMetrics {
        fn name(&self) -> &'static str {
            "combined"
        }

        fn iter<'a>(&'a self) -> impl IntoIterator<Item = &'a dyn Metric> {
            [&self.foo as &dyn Metric, &self.bar as &dyn Metric]
        }
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_metric_description() -> Result<(), Box<dyn std::error::Error>> {
        let metrics = FooMetrics::default();
        let items = metrics.describe();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].name, "metric_a");
        assert_eq!(items[0].description, "metric_a");
        assert_eq!(items[0].r#type, "counter");
        assert_eq!(items[1].name, "metric_b");
        assert_eq!(items[1].description, "metric_b");
        assert_eq!(items[1].r#type, "counter");

        Ok(())
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn test_solo_registry() -> Result<(), Box<dyn std::error::Error>> {
        use prometheus_client::encoding::text::encode;
        use prometheus_client::registry::Registry;

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

    #[cfg(feature = "metrics")]
    #[test]
    fn test_metric_sets() {
        use prometheus_client::encoding::text::encode;
        use prometheus_client::registry::Registry;

        let metrics = CombinedMetrics::default();
        metrics.foo.metric_a.inc();
        metrics.bar.count.inc_by(10);

        let mut collected = vec![];
        // manual collection and iteration if not using prometheus_client
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
        let mut sub = registry.sub_registry_with_prefix("combined");
        metrics.register(&mut sub);
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
}
