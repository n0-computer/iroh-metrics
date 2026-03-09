//! Label types for metric families.

use std::{borrow::Cow, hash::Hash};

/// A label value that can be encoded as a string for OpenMetrics output.
#[derive(Debug, Clone, PartialEq)]
pub enum LabelValue<'a> {
    /// String value.
    Str(Cow<'a, str>),
    /// Signed integer.
    Int(i64),
    /// Unsigned integer.
    Uint(u64),
    /// Boolean.
    Bool(bool),
}

impl LabelValue<'_> {
    /// Converts to string representation for encoding.
    pub fn as_str(&self) -> Cow<'_, str> {
        match self {
            LabelValue::Str(s) => Cow::Borrowed(s.as_ref()),
            LabelValue::Int(v) => Cow::Owned(v.to_string()),
            LabelValue::Uint(v) => Cow::Owned(v.to_string()),
            LabelValue::Bool(v) => Cow::Borrowed(if *v { "true" } else { "false" }),
        }
    }
}

impl<'a> From<&'a str> for LabelValue<'a> {
    fn from(s: &'a str) -> Self {
        LabelValue::Str(Cow::Borrowed(s))
    }
}

impl From<String> for LabelValue<'static> {
    fn from(s: String) -> Self {
        LabelValue::Str(Cow::Owned(s))
    }
}

macro_rules! impl_from_int {
    ($($t:ty => $variant:ident),*) => {
        $(
            impl From<$t> for LabelValue<'static> {
                fn from(v: $t) -> Self {
                    LabelValue::$variant(v as _)
                }
            }
        )*
    };
}

impl_from_int!(i64 => Int, i32 => Int, u64 => Uint, u32 => Uint, u16 => Uint);

impl From<bool> for LabelValue<'static> {
    fn from(v: bool) -> Self {
        LabelValue::Bool(v)
    }
}

/// A key-value label pair.
pub type LabelPair<'a> = (&'static str, LabelValue<'a>);

/// Trait for types that can be encoded as a set of labels.
///
/// Implement this for label structs to use with [`Family`](crate::Family).
/// The struct must also implement `Clone + Hash + Eq + Send + Sync`.
pub trait EncodeLabelSet: Hash + Eq + Clone + Send + Sync + 'static {
    /// Returns the labels as key-value pairs.
    fn encode_label_pairs(&self) -> Vec<LabelPair<'_>>;
}

/// Empty label set for metrics that don't need labels.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct NoLabels;

impl EncodeLabelSet for NoLabels {
    fn encode_label_pairs(&self) -> Vec<LabelPair<'_>> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_value_as_str() {
        assert_eq!(LabelValue::from("hello").as_str(), "hello");
        assert_eq!(LabelValue::from(42i64).as_str(), "42");
        assert_eq!(LabelValue::from(42u64).as_str(), "42");
        assert_eq!(LabelValue::from(true).as_str(), "true");
        assert_eq!(LabelValue::from(false).as_str(), "false");
    }

    #[test]
    fn test_no_labels() {
        assert!(NoLabels.encode_label_pairs().is_empty());
    }
}
