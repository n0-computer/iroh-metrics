//! Functions to encode metrics into the [OpenMetrics text format].
//!
//! [OpenMetrics text format]: https://github.com/prometheus/OpenMetrics/blob/main/specification/OpenMetrics.md

use std::{
    borrow::Cow,
    fmt::{self, Write},
};

use crate::{MetricItem, MetricType, MetricValue, MetricsGroup};

pub(crate) fn write_eof(writer: &mut impl Write) -> fmt::Result {
    writer.write_str("# EOF\n")
}

impl dyn MetricsGroup {
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
    pub(crate) fn encode_openmetrics<'a>(
        &self,
        writer: &mut impl Write,
        prefixes: &[&str],
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

fn write_prefix_name(writer: &mut impl Write, prefixes: &[&str], name: &str) -> fmt::Result {
    for prefix in prefixes {
        writer.write_str(prefix)?;
        writer.write_str("_")?;
    }
    writer.write_str(name)?;
    Ok(())
}
