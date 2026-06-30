//! POM `Selection<T>` — the universal result type for relation navigation.

use super::error::ReflectError;

/// An ordered, immutable collection of nodes returned by relation accessors.
///
/// Every relation (`Module::ports()`, `Module::instances()`, etc.) returns a
/// `Selection<T>`, even when the result is one node or none. This gives a
/// single shape for zero/one/many results and makes bulk operations uniform.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection<T> {
    items: Vec<T>,
}

impl<T> Selection<T> {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn from_vec(items: Vec<T>) -> Self {
        Self { items }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn first(&self) -> Option<&T> {
        self.items.first()
    }

    pub fn get(&self, i: usize) -> Option<&T> {
        self.items.get(i)
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    /// Exactly-one: returns the single element, or an error if zero or > 1.
    pub fn one(self) -> Result<T, ReflectError> {
        match self.items.len() {
            0 => Err(ReflectError::NotFound("selection was empty".into())),
            1 => Ok(self.items.into_iter().next().unwrap()),
            _ => Err(ReflectError::Other(format!(
                "selection had {} elements, expected exactly 1",
                self.items.len()
            ))),
        }
    }
}

impl<T> Selection<T> {
    /// Filter by a predicate — returns a new `Selection`.
    pub fn filter(self, pred: impl Fn(&T) -> bool) -> Selection<T> {
        Selection::from_vec(self.items.into_iter().filter(pred).collect())
    }

    /// Map each element to a new type — returns a `Vec<U>`.
    pub fn map<U>(self, f: impl Fn(T) -> U) -> Vec<U> {
        self.items.into_iter().map(f).collect()
    }
}

impl<T> Default for Selection<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> IntoIterator for Selection<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<T> From<Vec<T>> for Selection<T> {
    fn from(items: Vec<T>) -> Self {
        Self::from_vec(items)
    }
}