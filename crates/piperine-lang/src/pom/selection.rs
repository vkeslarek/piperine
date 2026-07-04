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

pub type NodeSelection<'a> = Selection<crate::pom::node::Node<'a>>;

impl<'a> NodeSelection<'a> {
    /// Sub-select via the selector grammar.
    pub fn where_path(&self, design: &'a crate::pom::design::Design, path: &str) -> Result<NodeSelection<'a>, crate::pom::error::SelectorError> {
        let sel = path.parse::<crate::pom::selector::Selector>()?;
        crate::pom::selector::Evaluator::new(design).evaluate(&sel, self.clone())
    }
}

impl<T> Selection<T> {
    /// Create an empty selection.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Create a selection from a pre-existing vector.
    pub fn from_vec(items: Vec<T>) -> Self {
        Self { items }
    }

    /// Number of elements in the selection.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the selection contains no elements.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns the first element, or `None` if empty.
    pub fn first(&self) -> Option<&T> {
        self.items.first()
    }

    /// Returns the element at `i`, or `None` if out of bounds.
    pub fn get(&self, i: usize) -> Option<&T> {
        self.items.get(i)
    }

    /// Iterate over references to the elements.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_empty() {
        let sel: Selection<i32> = Selection::new();
        assert!(sel.is_empty());
        assert_eq!(sel.len(), 0);
        assert_eq!(sel.first(), None);
    }

    #[test]
    fn selection_from_vec() {
        let sel = Selection::from_vec(vec![1, 2, 3]);
        assert_eq!(sel.len(), 3);
        assert!(!sel.is_empty());
        assert_eq!(sel.first(), Some(&1));
    }

    #[test]
    fn selection_get_bounds_checked() {
        let sel = Selection::from_vec(vec![10, 20]);
        assert_eq!(sel.get(0), Some(&10));
        assert_eq!(sel.get(1), Some(&20));
        assert_eq!(sel.get(2), None);
    }

    #[test]
    fn selection_one_exactly_one() {
        let sel = Selection::from_vec(vec![42]);
        assert_eq!(sel.one(), Ok(42));
    }

    #[test]
    fn selection_one_zero_errors() {
        let sel: Selection<i32> = Selection::new();
        let err = sel.one().unwrap_err();
        assert!(matches!(err, ReflectError::NotFound(_)));
    }

    #[test]
    fn selection_one_many_errors() {
        let sel = Selection::from_vec(vec![1, 2]);
        let err = sel.one().unwrap_err();
        assert!(matches!(err, ReflectError::Other(_)));
    }

    #[test]
    fn selection_iter() {
        let sel = Selection::from_vec(vec![1, 2, 3]);
        let collected: Vec<&i32> = sel.iter().collect();
        assert_eq!(collected, vec![&1, &2, &3]);
    }

    #[test]
    fn selection_filter() {
        let sel = Selection::from_vec(vec![1, 2, 3, 4, 5]);
        let evens = sel.filter(|x| *x % 2 == 0);
        assert_eq!(evens.len(), 2);
        assert_eq!(evens.get(0), Some(&2));
    }

    #[test]
    fn selection_map() {
        let sel = Selection::from_vec(vec![1, 2, 3]);
        let doubled: Vec<i32> = sel.map(|x| x * 2);
        assert_eq!(doubled, vec![2, 4, 6]);
    }
}

impl<T> From<Vec<T>> for Selection<T> {
    fn from(items: Vec<T>) -> Self {
        Self::from_vec(items)
    }
}