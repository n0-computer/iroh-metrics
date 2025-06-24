//! Functions to encode metrics into the [OpenMetrics text format].
//!
//! [OpenMetrics text format]: https://github.com/prometheus/OpenMetrics/blob/main/specification/OpenMetrics.md

#![allow(missing_docs)]

use std::{
    borrow::Cow,
    fmt::{self, Write},
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};

use crate::{
    MetricItem, MetricType, MetricValue, MetricsGroup, MetricsSource, Registry, RwLockRegistry,
};

pub(crate) fn write_eof(writer: &mut impl Write) -> fmt::Result {
    writer.write_str("# EOF\n")
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemSchema {
    pub r#type: MetricType,
    pub name: String,
    pub help: String,
    pub prefixes: Vec<String>,
    pub labels: Vec<(String, String)>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Schema {
    pub items: Vec<ItemSchema>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Values {
    pub items: Vec<MetricValue>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Update {
    pub schema: Option<Schema>,
    pub values: Values,
}

#[derive(Debug)]
pub struct Item<'a> {
    pub schema: &'a ItemSchema,
    pub value: &'a MetricValue,
}

impl<'a> Item<'a> {
    fn as_metric_item(&self) -> MetricItem<'a> {
        MetricItem {
            name: &self.schema.name,
            help: &self.schema.help,
            metric: self.value,
        }
    }

    pub fn encode_openmetrics(
        &self,
        writer: &mut impl std::fmt::Write,
    ) -> Result<(), crate::Error> {
        let item = self.as_metric_item();
        item.encode_openmetrics(
            writer,
            self.schema.prefixes.as_slice(),
            self.schema
                .labels
                .iter()
                .map(|(a, b)| (a.as_str(), b.as_str())),
        )?;
        Ok(())
    }
}

/// Decoder for metrics received from an [`Encoder`]
///
/// Implements [`MetricSource`] to export the decoded metrics to OpenMetrics.
#[derive(Debug, Default)]
pub struct Decoder {
    schema: Option<Schema>,
    values: Values,
}

impl Decoder {
    pub fn import(&mut self, update: Update) {
        if let Some(schema) = update.schema {
            self.schema = Some(schema);
        }
        self.values = update.values;
    }

    pub fn import_bytes(&mut self, data: &[u8]) -> Result<(), postcard::Error> {
        let update = postcard::from_bytes(data)?;
        self.import(update);
        Ok(())
    }

    pub fn iter(&self) -> DecoderIter {
        DecoderIter {
            pos: 0,
            inner: self,
        }
    }
}

#[derive(Debug)]
pub struct DecoderIter<'a> {
    pos: usize,
    inner: &'a Decoder,
}

impl<'a> Iterator for DecoderIter<'a> {
    type Item = Item<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let schema = self.inner.schema.as_ref()?.items.get(self.pos)?;
        let value = self.inner.values.items.get(self.pos)?;
        self.pos += 1;
        Some(Item { schema, value })
    }
}

impl MetricsSource for Decoder {
    fn encode_openmetrics(&self, writer: &mut impl std::fmt::Write) -> Result<(), crate::Error> {
        for item in self.iter() {
            item.encode_openmetrics(writer)?;
        }
        write_eof(writer)?;
        Ok(())
    }
}

impl MetricsSource for Arc<RwLock<Decoder>> {
    fn encode_openmetrics(&self, writer: &mut impl std::fmt::Write) -> Result<(), crate::Error> {
        self.read().expect("poisoned").encode_openmetrics(writer)
    }
}

#[derive(Debug)]
pub struct Encoder {
    registry: Arc<RwLock<Registry>>,
    last_schema_version: u64,
}

impl Encoder {
    pub fn new(registry: RwLockRegistry) -> Self {
        Self {
            registry,
            last_schema_version: 0,
        }
    }

    pub fn export(&mut self) -> Update {
        let registry = self.registry.read().expect("poisoned");
        let current = registry.schema_version();
        let schema = if current != self.last_schema_version {
            self.last_schema_version = current;
            let mut schema = Schema::default();
            registry.encode_schema(&mut schema);
            Some(schema)
        } else {
            None
        };
        let mut values = Values::default();
        registry.encode_values(&mut values);
        Update { schema, values }
    }

    pub fn export_bytes(&mut self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_stdvec(&self.export())
    }
}

impl dyn MetricsGroup {
    pub(crate) fn encode_schema<'a>(
        &self,
        schema: &mut Schema,
        prefix: Option<&'a str>,
        labels: &[(Cow<'a, str>, Cow<'a, str>)],
    ) {
        let name = self.name();
        let prefixes = if let Some(prefix) = prefix {
            &[prefix, name] as &[&str]
        } else {
            &[name]
        };
        for metric in self.iter() {
            let labels = labels.iter().map(|(k, v)| (k.as_ref(), v.as_ref()));
            metric.encode_schema(schema, prefixes, labels);
        }
    }

    pub(crate) fn encode_values<'a>(&self, values: &mut Values) {
        for metric in self.iter() {
            metric.encode_value(values);
        }
    }

    pub(crate) fn encode_openmetrics<'a>(
        &self,
        writer: &'a mut impl Write,
        prefix: Option<&'a str>,
        labels: &[(Cow<'a, str>, Cow<'a, str>)],
    ) -> fmt::Result {
        let name = self.name();
        let prefixes = if let Some(prefix) = prefix {
            &[prefix, name] as &[&str]
        } else {
            &[name]
        };
        for metric in self.iter() {
            let labels = labels.iter().map(|(k, v)| (k.as_ref(), v.as_ref()));
            metric.encode_openmetrics(writer, prefixes, labels)?;
        }
        Ok(())
    }
}

impl MetricItem<'_> {
    pub(crate) fn encode_schema<'a>(
        &self,
        schema: &mut Schema,
        prefixes: &[&str],
        labels: impl Iterator<Item = (&'a str, &'a str)> + 'a,
    ) {
        let item = crate::encoding::ItemSchema {
            name: self.name().to_string(),
            prefixes: prefixes.iter().map(|s| s.to_string()).collect(),
            help: self.help().to_string(),
            labels: labels
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            r#type: self.r#type(),
        };
        schema.items.push(item);
    }

    fn encode_value(&self, values: &mut Values) {
        values.items.push(self.value())
    }

    pub(crate) fn encode_openmetrics<'a>(
        &self,
        writer: &mut impl Write,
        prefixes: &[impl AsRef<str>],
        labels: impl Iterator<Item = (&'a str, &'a str)> + 'a,
    ) -> fmt::Result {
        writer.write_str("# HELP ")?;
        write_prefix_name(writer, prefixes, self.name())?;
        writer.write_str(" ")?;
        writer.write_str(self.help())?;
        writer.write_str(".\n")?;

        writer.write_str("# TYPE ")?;
        write_prefix_name(writer, prefixes, self.name())?;
        writer.write_str(" ")?;
        writer.write_str(self.r#type().as_str())?;
        writer.write_str("\n")?;

        write_prefix_name(writer, prefixes, self.name())?;
        let suffix = match self.r#type() {
            MetricType::Counter => "_total",
            MetricType::Gauge => "",
        };
        writer.write_str(suffix)?;
        write_labels(writer, labels)?;
        writer.write_char(' ')?;
        match self.value() {
            MetricValue::Counter(value) => {
                encode_u64(writer, value)?;
            }
            MetricValue::Gauge(value) => {
                encode_i64(writer, value)?;
            }
        }
        writer.write_str("\n")?;
        Ok(())
    }
}

fn write_labels<'a>(
    writer: &mut impl Write,
    labels: impl Iterator<Item = (&'a str, &'a str)> + 'a,
) -> fmt::Result {
    let mut is_first = true;
    let mut labels = labels.peekable();
    while let Some((key, value)) = labels.next() {
        let is_last = labels.peek().is_none();
        if is_first {
            writer.write_char('{')?;
            is_first = false;
        }
        writer.write_str(key)?;
        writer.write_str("=\"")?;
        writer.write_str(value)?;
        writer.write_str("\"")?;
        if is_last {
            writer.write_char('}')?;
        } else {
            writer.write_char(',')?;
        }
    }
    Ok(())
}

fn encode_u64(writer: &mut impl Write, v: u64) -> fmt::Result {
    writer.write_str(itoa::Buffer::new().format(v))?;
    Ok(())
}

fn encode_i64(writer: &mut impl Write, v: i64) -> fmt::Result {
    writer.write_str(itoa::Buffer::new().format(v))?;
    Ok(())
}

fn write_prefix_name(
    writer: &mut impl Write,
    prefixes: &[impl AsRef<str>],
    name: &str,
) -> fmt::Result {
    for prefix in prefixes {
        writer.write_str(prefix.as_ref())?;
        writer.write_str("_")?;
    }
    writer.write_str(name)?;
    Ok(())
}
