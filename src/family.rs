//! Metric families with labels.
//!
//! A [`Family`] is a collection of metrics indexed by label sets. Targeted for
//! low cardinality (tens to ~100 label combinations). For high cardinality,
//! an alternative implementation should be considered due to lock contention.
//! This was designed as such to avoid memory bloat in general use cases.

#[cfg(feature = "metrics")]
use std::collections::HashMap;
use std::{
    borrow::Cow,
    fmt::{self, Write},
    sync::Arc,
};

#[cfg(feature = "metrics")]
use parking_lot::RwLock;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[cfg(feature = "metrics")]
use crate::MetricValue;
#[cfg(feature = "metrics")]
use crate::encoding::{
    encode_metric_value, encode_schema_item, encode_value_item, write_prefix_name,
};
use crate::{
    Metric,
    encoding::{Schema, Values},
    labels::EncodeLabelSet,
};

/// Trait for type-erased Family encoding.
///
/// Implemented by `Family<L, M>` to enable dynamic dispatch in derive macros.
pub trait FamilyEncoder: Send + Sync + 'static {
    /// Encodes to OpenMetrics text format.
    fn encode_openmetrics_dyn(
        &self,
        writer: &mut dyn Write,
        name: &str,
        help: &str,
        prefixes: &[&str],
        registry_labels: &[(&str, &str)],
    ) -> fmt::Result;

    /// Encodes schema for binary encoding.
    fn encode_schema_dyn(
        &self,
        schema: &mut Schema,
        name: &str,
        help: &str,
        prefixes: &[&str],
        registry_labels: &[(Cow<'_, str>, Cow<'_, str>)],
    );

    /// Encodes values for binary encoding.
    fn encode_values_dyn(&self, values: &mut Values);

    /// Returns true if the family has no entries.
    fn is_empty_dyn(&self) -> bool;
}

/// A family metric item for iteration.
#[derive(Clone, Copy)]
pub struct FamilyItem<'a> {
    pub(crate) name: &'static str,
    pub(crate) help: &'static str,
    pub(crate) family: &'a dyn FamilyEncoder,
}

impl fmt::Debug for FamilyItem<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FamilyItem")
            .field("name", &self.name)
            .field("help", &self.help)
            .finish_non_exhaustive()
    }
}

impl<'a> FamilyItem<'a> {
    /// Creates a new family item.
    pub fn new(name: &'static str, help: &'static str, family: &'a dyn FamilyEncoder) -> Self {
        Self { name, help, family }
    }

    /// Returns the name of this family.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the help text of this family.
    pub fn help(&self) -> &'static str {
        self.help
    }

    /// Returns true if the family has no entries.
    pub fn is_empty(&self) -> bool {
        self.family.is_empty_dyn()
    }

    /// Encodes to OpenMetrics text format.
    pub fn encode_openmetrics(
        &self,
        writer: &mut dyn fmt::Write,
        prefixes: &[&str],
        labels: &[(Cow<'_, str>, Cow<'_, str>)],
    ) -> fmt::Result {
        let label_refs: Vec<_> = labels
            .iter()
            .map(|(k, v)| (k.as_ref(), v.as_ref()))
            .collect();
        self.family
            .encode_openmetrics_dyn(writer, self.name, self.help, prefixes, &label_refs)
    }

    /// Encodes schema for binary encoding.
    pub fn encode_schema(
        &self,
        schema: &mut Schema,
        prefixes: &[&str],
        labels: &[(Cow<'_, str>, Cow<'_, str>)],
    ) {
        self.family
            .encode_schema_dyn(schema, self.name, self.help, prefixes, labels);
    }

    /// Encodes values for binary encoding.
    pub fn encode_values(&self, values: &mut Values) {
        self.family.encode_values_dyn(values);
    }
}

#[cfg(feature = "metrics")]
type Constructor<M> = Arc<dyn Fn() -> M + Send + Sync>;

/// A family of metrics indexed by labels.
#[cfg(feature = "metrics")]
pub struct Family<L, M>
where
    L: EncodeLabelSet,
    M: Metric,
{
    inner: Arc<RwLock<HashMap<L, Arc<M>>>>,
    constructor: Constructor<M>,
}

#[cfg(feature = "metrics")]
impl<L, M> Family<L, M>
where
    L: EncodeLabelSet,
    M: Metric + Default + 'static,
{
    /// Creates a new family using `M::default()` for new metrics.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            constructor: Arc::new(M::default),
        }
    }
}

#[cfg(feature = "metrics")]
impl<L, M> Default for Family<L, M>
where
    L: EncodeLabelSet,
    M: Metric + Default + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "metrics")]
impl<L, M> Family<L, M>
where
    L: EncodeLabelSet,
    M: Metric,
{
    /// Creates a new family with a custom constructor (useful for Histogram buckets).
    pub fn with_constructor<F: Fn() -> M + Send + Sync + 'static>(constructor: F) -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
            constructor: Arc::new(constructor),
        }
    }

    /// Gets or creates a metric for the given labels.
    pub fn get_or_create(&self, labels: &L) -> Arc<M> {
        if let Some(metric) = self.inner.read().get(labels) {
            return Arc::clone(metric);
        }

        let mut guard = self.inner.write();
        if let Some(metric) = guard.get(labels) {
            return Arc::clone(metric);
        }

        let metric = Arc::new((self.constructor)());
        guard.insert(labels.clone(), Arc::clone(&metric));
        metric
    }

    /// Removes the metric for the given labels.
    pub fn remove(&self, labels: &L) -> Option<Arc<M>> {
        self.inner.write().remove(labels)
    }

    /// Removes all metrics.
    pub fn clear(&self) {
        self.inner.write().clear();
    }

    /// Returns the number of label combinations tracked.
    pub fn len(&self) -> usize {
        self.inner.read().len()
    }

    /// Returns true if empty.
    pub fn is_empty(&self) -> bool {
        self.inner.read().is_empty()
    }

    /// Encodes to OpenMetrics text format.
    pub fn encode_openmetrics<'a>(
        &self,
        writer: &mut (impl Write + ?Sized),
        name: &str,
        help: &str,
        prefixes: &[&str],
        registry_labels: impl Iterator<Item = (&'a str, &'a str)>,
    ) -> fmt::Result
    where
        L: Ord,
    {
        let reg_labels: Vec<_> = registry_labels
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self.encode_openmetrics_impl(writer, name, help, prefixes, &reg_labels)
    }

    fn encode_openmetrics_impl(
        &self,
        writer: &mut (impl Write + ?Sized),
        name: &str,
        help: &str,
        prefixes: &[&str],
        registry_labels: &[(String, String)],
    ) -> fmt::Result
    where
        L: Ord,
    {
        let guard = self.inner.read();
        if guard.is_empty() {
            return Ok(());
        }

        let mut entries: Vec<_> = guard.iter().collect();
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));

        let metric_type = entries[0].1.r#type();

        writer.write_str("# HELP ")?;
        write_prefix_name(writer, prefixes, name)?;
        writeln!(writer, " {help}.")?;

        writer.write_str("# TYPE ")?;
        write_prefix_name(writer, prefixes, name)?;
        writeln!(writer, " {}", metric_type.as_str())?;

        for (label_set, metric) in entries {
            let mut all_labels = registry_labels.to_vec();
            all_labels.extend(
                label_set
                    .encode_label_pairs()
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.as_str().into_owned())),
            );
            encode_metric_value(writer, name, prefixes, &all_labels, &metric.value())?;
        }
        Ok(())
    }

    /// Encodes schema for binary encoding.
    pub fn encode_schema(
        &self,
        schema: &mut Schema,
        name: &str,
        help: &str,
        prefixes: &[&str],
        registry_labels: &[(Cow<'_, str>, Cow<'_, str>)],
    ) where
        L: Ord,
    {
        let guard = self.inner.read();
        let mut entries: Vec<_> = guard.iter().collect();
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));

        for (labels, metric) in entries {
            let mut all_labels: Vec<_> = registry_labels
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            all_labels.extend(
                labels
                    .encode_label_pairs()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.as_str().to_string())),
            );
            encode_schema_item(schema, name, help, prefixes, &all_labels, metric.r#type());
        }
    }

    /// Encodes values for binary encoding. Order must match schema.
    pub fn encode_values(&self, values: &mut Values)
    where
        L: Ord,
    {
        let guard = self.inner.read();
        let mut entries: Vec<_> = guard.iter().collect();
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));

        for (_, metric) in entries {
            encode_value_item(values, metric.value());
        }
    }
}

/// A family of metrics indexed by labels (no-op when metrics disabled).
#[cfg(not(feature = "metrics"))]
pub struct Family<L, M>
where
    L: EncodeLabelSet,
    M: Metric,
{
    default_metric: Arc<M>,
    _labels: std::marker::PhantomData<L>,
}

#[cfg(not(feature = "metrics"))]
impl<L, M> Family<L, M>
where
    L: EncodeLabelSet,
    M: Metric + Default + 'static,
{
    /// Creates a new family using `M::default()` for new metrics.
    pub fn new() -> Self {
        Self {
            default_metric: Arc::new(M::default()),
            _labels: std::marker::PhantomData,
        }
    }
}

#[cfg(not(feature = "metrics"))]
impl<L, M> Default for Family<L, M>
where
    L: EncodeLabelSet,
    M: Metric + Default + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "metrics"))]
impl<L, M> Family<L, M>
where
    L: EncodeLabelSet,
    M: Metric,
{
    /// Creates a new family with a custom constructor (useful for Histogram buckets).
    pub fn with_constructor<F: Fn() -> M + Send + Sync + 'static>(constructor: F) -> Self {
        Self {
            default_metric: Arc::new(constructor()),
            _labels: std::marker::PhantomData,
        }
    }

    /// Gets or creates a metric for the given labels (returns default metric).
    pub fn get_or_create(&self, _labels: &L) -> Arc<M> {
        Arc::clone(&self.default_metric)
    }

    /// Removes the metric for the given labels (no-op).
    pub fn remove(&self, _labels: &L) -> Option<Arc<M>> {
        None
    }

    /// Removes all metrics (no-op).
    pub fn clear(&self) {}

    /// Returns the number of label combinations tracked.
    pub fn len(&self) -> usize {
        0
    }

    /// Returns true if empty.
    pub fn is_empty(&self) -> bool {
        true
    }

    /// Encodes to OpenMetrics text format (no-op).
    pub fn encode_openmetrics<'a>(
        &self,
        _writer: &mut impl Write,
        _name: &str,
        _help: &str,
        _prefixes: &[&str],
        _registry_labels: impl Iterator<Item = (&'a str, &'a str)>,
    ) -> fmt::Result
    where
        L: Ord,
    {
        Ok(())
    }

    /// Encodes schema for binary encoding (no-op).
    pub fn encode_schema(
        &self,
        _schema: &mut Schema,
        _name: &str,
        _help: &str,
        _prefixes: &[&str],
        _registry_labels: &[(Cow<'_, str>, Cow<'_, str>)],
    ) where
        L: Ord,
    {
    }

    /// Encodes values for binary encoding (no-op).
    pub fn encode_values(&self, _values: &mut Values)
    where
        L: Ord,
    {
    }
}

// ============================================================================
// Trait impls: metrics ENABLED
// ============================================================================

#[cfg(feature = "metrics")]
impl<L, M> Clone for Family<L, M>
where
    L: EncodeLabelSet,
    M: Metric,
{
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            constructor: Arc::clone(&self.constructor),
        }
    }
}

#[cfg(feature = "metrics")]
impl<L, M> FamilyEncoder for Family<L, M>
where
    L: EncodeLabelSet + Ord,
    M: Metric + 'static,
{
    fn encode_openmetrics_dyn(
        &self,
        writer: &mut dyn Write,
        name: &str,
        help: &str,
        prefixes: &[&str],
        registry_labels: &[(&str, &str)],
    ) -> fmt::Result {
        let reg_labels: Vec<_> = registry_labels
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self.encode_openmetrics_impl(writer, name, help, prefixes, &reg_labels)
    }

    fn encode_schema_dyn(
        &self,
        schema: &mut Schema,
        name: &str,
        help: &str,
        prefixes: &[&str],
        registry_labels: &[(Cow<'_, str>, Cow<'_, str>)],
    ) {
        self.encode_schema(schema, name, help, prefixes, registry_labels);
    }

    fn encode_values_dyn(&self, values: &mut Values) {
        self.encode_values(values);
    }

    fn is_empty_dyn(&self) -> bool {
        self.is_empty()
    }
}

#[cfg(feature = "metrics")]
impl<L, M> fmt::Debug for Family<L, M>
where
    L: EncodeLabelSet + fmt::Debug,
    M: Metric,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let guard = self.inner.read();
        f.debug_struct("Family")
            .field("len", &guard.len())
            .field("labels", &guard.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(feature = "metrics")]
impl<L, M> Serialize for Family<L, M>
where
    L: EncodeLabelSet + Serialize + Ord,
    M: Metric,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;

        let guard = self.inner.read();
        let mut entries: Vec<_> = guard.iter().collect();
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));

        let mut seq = serializer.serialize_seq(Some(entries.len()))?;
        for (labels, metric) in entries {
            seq.serialize_element(&(labels, metric.value()))?;
        }
        seq.end()
    }
}

#[cfg(feature = "metrics")]
impl<'de, L, M> Deserialize<'de> for Family<L, M>
where
    L: EncodeLabelSet + Deserialize<'de>,
    M: Metric + Default + 'static,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let entries: Vec<(L, MetricValue)> = Vec::deserialize(deserializer)?;
        let family = Family::new();
        {
            let mut guard = family.inner.write();
            for (labels, value) in entries {
                let metric = Arc::new(M::default());
                metric.set_value(value);
                guard.insert(labels, metric);
            }
        }
        Ok(family)
    }
}

#[cfg(not(feature = "metrics"))]
impl<L, M> Clone for Family<L, M>
where
    L: EncodeLabelSet,
    M: Metric,
{
    fn clone(&self) -> Self {
        Self {
            default_metric: Arc::clone(&self.default_metric),
            _labels: std::marker::PhantomData,
        }
    }
}

#[cfg(not(feature = "metrics"))]
impl<L, M> FamilyEncoder for Family<L, M>
where
    L: EncodeLabelSet + Ord,
    M: Metric + 'static,
{
    fn encode_openmetrics_dyn(
        &self,
        _writer: &mut dyn Write,
        _name: &str,
        _help: &str,
        _prefixes: &[&str],
        _registry_labels: &[(&str, &str)],
    ) -> fmt::Result {
        Ok(())
    }

    fn encode_schema_dyn(
        &self,
        _schema: &mut Schema,
        _name: &str,
        _help: &str,
        _prefixes: &[&str],
        _registry_labels: &[(Cow<'_, str>, Cow<'_, str>)],
    ) {
    }

    fn encode_values_dyn(&self, _values: &mut Values) {}

    fn is_empty_dyn(&self) -> bool {
        true
    }
}

#[cfg(not(feature = "metrics"))]
impl<L, M> fmt::Debug for Family<L, M>
where
    L: EncodeLabelSet,
    M: Metric,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Family").field("disabled", &true).finish()
    }
}

#[cfg(not(feature = "metrics"))]
impl<L, M> Serialize for Family<L, M>
where
    L: EncodeLabelSet,
    M: Metric,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        serializer.serialize_seq(Some(0))?.end()
    }
}

#[cfg(not(feature = "metrics"))]
impl<'de, L, M> Deserialize<'de> for Family<L, M>
where
    L: EncodeLabelSet,
    M: Metric + Default + 'static,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let _: serde::de::IgnoredAny = serde::de::Deserialize::deserialize(deserializer)?;
        Ok(Family::new())
    }
}

#[cfg(all(test, feature = "metrics"))]
mod tests {
    use std::borrow::Cow;

    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::{Counter, Histogram, LabelPair, NoLabels};

    #[derive(Clone, Hash, PartialEq, Eq, Debug, PartialOrd, Ord, Serialize, Deserialize)]
    struct TestLabels {
        method: String,
        status: u16,
    }

    impl EncodeLabelSet for TestLabels {
        fn encode_label_pairs(&self) -> Vec<LabelPair<'_>> {
            vec![
                (
                    "method",
                    crate::LabelValue::Str(Cow::Borrowed(&self.method)),
                ),
                ("status", crate::LabelValue::Uint(self.status as u64)),
            ]
        }
    }

    fn labels(method: &str, status: u16) -> TestLabels {
        TestLabels {
            method: method.into(),
            status,
        }
    }

    #[test]
    fn test_family_operations() {
        // get_or_create returns same metric for same labels
        let family: Family<TestLabels, Counter> = Family::new();
        let c1 = family.get_or_create(&labels("GET", 200));
        c1.inc();
        let c2 = family.get_or_create(&labels("GET", 200));
        c2.inc();
        assert_eq!(c1.get(), 2);

        // different labels get different metrics
        family.get_or_create(&labels("GET", 404)).inc_by(5);
        assert_eq!(family.get_or_create(&labels("GET", 404)).get(), 5);
        assert_eq!(family.len(), 2);

        // remove
        let removed = family.remove(&labels("GET", 200));
        assert_eq!(removed.unwrap().get(), 2);
        assert_eq!(family.len(), 1);

        // clear
        family.get_or_create(&labels("POST", 200)).inc();
        family.clear();
        assert!(family.is_empty());

        // with_constructor for custom metrics
        let hist_family: Family<NoLabels, Histogram> =
            Family::with_constructor(|| Histogram::new(vec![1.0, 5.0, 10.0]));
        hist_family.get_or_create(&NoLabels).observe(2.5);
        hist_family.get_or_create(&NoLabels).observe(7.5);
        assert_eq!(hist_family.get_or_create(&NoLabels).count(), 2);
    }

    #[test]
    fn test_serde_roundtrip() {
        let family: Family<TestLabels, Counter> = Family::new();
        family.get_or_create(&labels("GET", 200)).inc_by(10);
        family.get_or_create(&labels("POST", 201)).inc_by(5);

        let bytes = postcard::to_stdvec(&family).unwrap();
        let decoded: Family<TestLabels, Counter> = postcard::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.get_or_create(&labels("GET", 200)).get(), 10);
        assert_eq!(decoded.get_or_create(&labels("POST", 201)).get(), 5);
    }

    #[test]
    fn test_encoding() {
        let family: Family<TestLabels, Counter> = Family::new();
        family.get_or_create(&labels("GET", 200)).inc_by(10);
        family.get_or_create(&labels("POST", 201)).inc_by(5);

        // OpenMetrics without registry labels
        let mut out = String::new();
        family
            .encode_openmetrics(
                &mut out,
                "requests",
                "HTTP requests",
                &["http"],
                std::iter::empty(),
            )
            .unwrap();
        assert!(out.contains("# HELP http_requests HTTP requests."));
        assert!(out.contains("# TYPE http_requests counter"));
        assert!(out.contains(r#"http_requests_total{method="GET",status="200"} 10"#));
        assert!(out.contains(r#"http_requests_total{method="POST",status="201"} 5"#));

        // OpenMetrics with registry labels
        let mut out = String::new();
        family
            .encode_openmetrics(
                &mut out,
                "requests",
                "HTTP requests",
                &["http"],
                [("service", "api")].into_iter(),
            )
            .unwrap();
        assert!(out.contains(r#"http_requests_total{service="api",method="GET",status="200"} 10"#));

        // Binary schema + values
        let mut schema = Schema::default();
        family.encode_schema(&mut schema, "requests", "HTTP requests", &["http"], &[]);
        assert_eq!(schema.items.len(), 2);

        let mut values = Values::default();
        family.encode_values(&mut values);
        assert_eq!(values.items.len(), 2);
    }
}
