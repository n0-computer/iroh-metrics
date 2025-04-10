use std::fmt;

/// Trait for iterating over the fields of a struct.
pub trait Iterable {
    /// Returns the number of fields in the struct.
    fn field_count(&self) -> usize;
    /// Returns the field name and dyn reference to the field.
    fn field(&self, n: usize) -> Option<(&'static str, &dyn std::any::Any)>;
}

impl dyn Iterable {
    /// Returns an iterator over the fields of the struct.
    pub fn iter(&self) -> FieldIter<'_> {
        FieldIter::new(self)
    }
}

/// Helper trait to convert from `self` to `dyn Iterable`.
pub trait IntoIterable {
    /// Returns `self` as `dyn Iterable`
    fn as_iterable(&self) -> &dyn Iterable;

    /// Returns an iterator over the fields of the struct.
    fn iter(&self) -> FieldIter<'_> {
        FieldIter::new(self.as_iterable())
    }
}

impl<T> IntoIterable for T
where
    T: Iterable,
{
    fn as_iterable(&self) -> &dyn Iterable {
        self
    }
}

/// Iterator over the fields of a struct.
///
/// Returned from [`IntoIterable::iter`].
pub struct FieldIter<'a> {
    pos: usize,
    inner: &'a dyn Iterable,
}

impl fmt::Debug for FieldIter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FieldIter")
    }
}

impl<'a> FieldIter<'a> {
    pub(crate) fn new(inner: &'a dyn Iterable) -> Self {
        Self { pos: 0, inner }
    }
}
impl<'a> Iterator for FieldIter<'a> {
    type Item = (&'static str, &'a dyn std::any::Any);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos == self.inner.field_count() {
            None
        } else {
            let out = self.inner.field(self.pos);
            self.pos += 1;
            out
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.inner.field_count() - self.pos;
        (n, Some(n))
    }
}
