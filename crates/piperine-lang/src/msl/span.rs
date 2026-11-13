//! Byte-range spans over a source string.

use std::ops::Range;

/// A half-open byte range `start..end` into the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    /// The smallest span covering both `self` and `other`.
    pub fn to(self, other: Span) -> Span {
        Span { start: self.start.min(other.start), end: self.end.max(other.end) }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }
}

impl From<Range<usize>> for Span {
    fn from(r: Range<usize>) -> Self {
        Span { start: r.start, end: r.end }
    }
}

impl From<Span> for miette::SourceSpan {
    fn from(s: Span) -> Self {
        (s.start, s.len()).into()
    }
}
