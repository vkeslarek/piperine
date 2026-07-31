//! `table(x, xs, ys)` — 1-D lookup as a **pure piecewise function**.
//!
//! Extracted from `piperine-codegen`'s `resolve/pom/analog_ops.rs::Table`,
//! semantics unchanged: linear interpolation with **end clamp**, `xs`
//! strictly increasing, `"linear"` the only interpolation mode. Codegen
//! keeps its own copy (which lowers to a nested-`If` POM expression so its
//! Jacobian falls out of symbolic differentiation) until
//! `codegen-simplification` delegates to this one.
//!
//! **The design rule to preserve:** a `Behavior` that can be expressed as a
//! pure piecewise function *must* be, because its derivative is then the
//! segment slope — read off the same piecewise structure `value` uses,
//! never hand-derived — and therefore cannot be wrong. `Table::segment`
//! is that shared structure: both [`Table::value`] and
//! [`Table::derivative`] locate the same segment and read from the same
//! per-segment slope. Reserve hand-written Jacobians for kinds that
//! genuinely need state (`ideal.md` §3.9 C).

/// Why a `Table` could not be constructed.
#[derive(Debug, Clone, PartialEq)]
pub enum TableError {
    /// Fewer than two `(x, y)` points.
    TooFewPoints,
    /// `xs` and `ys` have different lengths.
    LengthMismatch,
    /// `xs` is not strictly increasing.
    NotStrictlyIncreasing,
    /// An interpolation mode other than `"linear"` was requested.
    UnsupportedMode(String),
}

impl std::fmt::Display for TableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TableError::TooFewPoints => write!(f, "table needs at least 2 points"),
            TableError::LengthMismatch => write!(f, "table xs and ys must have the same length"),
            TableError::NotStrictlyIncreasing => {
                write!(f, "table breakpoints (xs) must be strictly increasing")
            }
            TableError::UnsupportedMode(mode) => {
                write!(f, "table interpolation mode: only \"linear\" is supported, got {mode:?}")
            }
        }
    }
}

impl std::error::Error for TableError {}

/// A 1-D measured-data lookup: linear interpolation between `(xs[i], ys[i])`
/// points, clamped flat beyond both ends.
#[derive(Debug)]
pub struct Table {
    xs: Vec<f64>,
    ys: Vec<f64>,
}

impl Table {
    /// Build a `Table` with `"linear"` interpolation (the only mode this
    /// crate — and codegen's `table()` — supports).
    pub fn new(xs: Vec<f64>, ys: Vec<f64>) -> Result<Self, TableError> {
        Self::with_mode(xs, ys, "linear")
    }

    /// Build a `Table`, validating the interpolation `mode` explicitly —
    /// the loud-refusal path for anything but `"linear"`.
    pub fn with_mode(xs: Vec<f64>, ys: Vec<f64>, mode: &str) -> Result<Self, TableError> {
        if mode != "linear" {
            return Err(TableError::UnsupportedMode(mode.to_string()));
        }
        if xs.len() != ys.len() {
            return Err(TableError::LengthMismatch);
        }
        if xs.len() < 2 {
            return Err(TableError::TooFewPoints);
        }
        if !xs.windows(2).all(|w| w[0] < w[1]) {
            return Err(TableError::NotStrictlyIncreasing);
        }
        Ok(Self { xs, ys })
    }

    /// The segment slope `(ys[i+1] - ys[i]) / (xs[i+1] - xs[i])`.
    fn slope(&self, i: usize) -> f64 {
        (self.ys[i + 1] - self.ys[i]) / (self.xs[i + 1] - self.xs[i])
    }

    /// The interior segment index `i` such that `xs[i] <= x < xs[i+1]`, or
    /// `None` when `x` falls in a clamp region (`x <= xs[0]` or
    /// `x >= xs[n-1]`). The single lookup [`value`](Self::value) and
    /// [`derivative`](Self::derivative) share.
    fn segment(&self, x: f64) -> Option<usize> {
        let n = self.xs.len();
        if x <= self.xs[0] || x >= self.xs[n - 1] {
            return None;
        }
        (0..n - 1).find(|&i| x < self.xs[i + 1])
    }

    /// Linear interpolation with end clamp: `ys[0]` at/below `xs[0]`,
    /// `ys[n-1]` at/above `xs[n-1]`, else `ys[i] + (x - xs[i]) * slope_i`.
    pub fn value(&self, x: f64) -> f64 {
        match self.segment(x) {
            Some(i) => self.ys[i] + (x - self.xs[i]) * self.slope(i),
            None if x <= self.xs[0] => self.ys[0],
            None => self.ys[self.ys.len() - 1],
        }
    }

    /// The derivative: the segment slope inside the breakpoints, `0` in a
    /// clamp region (a flat extrapolation has no slope) — read from the
    /// same `segment` lookup `value` uses, never hand-derived.
    pub fn derivative(&self, x: f64) -> f64 {
        match self.segment(x) {
            Some(i) => self.slope(i),
            None => 0.0,
        }
    }
}

// ── `Table` as a `Behavior` kind (T8) ───────────────────────────────────
//
// `Behavior.table(x, xs, ys)` — PWL in voltage, a member of the `Behavior`
// family by construction: `value`/`jacobian` are exactly [`Table::value`]/
// [`Table::derivative`], which is the whole point of extracting `table` as
// a pure piecewise function first (`ideal.md` §3.9 C) — its derivative was
// already free before `Behavior` existed to consume it.

impl super::BehaviorKind for Table {
    fn value(&self, inputs: &[f64]) -> f64 {
        Table::value(self, inputs[0])
    }
    fn jacobian(&self, inputs: &[f64], out: &mut [f64]) {
        out[0] = self.derivative(inputs[0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavior::BehaviorKind;

    #[test]
    fn linear_interpolation_between_points() {
        let t = Table::new(vec![0.0, 1.0, 2.0], vec![0.0, 10.0, 10.0]).unwrap();
        assert_eq!(t.value(0.5), 5.0);
        assert_eq!(t.value(1.5), 10.0);
    }

    #[test]
    fn end_clamp_below_and_above() {
        let t = Table::new(vec![0.0, 1.0], vec![5.0, 15.0]).unwrap();
        assert_eq!(t.value(-10.0), 5.0, "clamp below xs[0]");
        assert_eq!(t.value(10.0), 15.0, "clamp above xs[n-1]");
    }

    #[test]
    fn derivative_is_the_segment_slope() {
        let t = Table::new(vec![0.0, 2.0, 4.0], vec![0.0, 10.0, 0.0]).unwrap();
        assert_eq!(t.derivative(1.0), 5.0, "slope of segment 0: 10/2");
        assert_eq!(t.derivative(3.0), -5.0, "slope of segment 1: -10/2");
    }

    #[test]
    fn derivative_is_zero_in_the_clamp_region() {
        let t = Table::new(vec![0.0, 1.0], vec![0.0, 10.0]).unwrap();
        assert_eq!(t.derivative(-5.0), 0.0);
        assert_eq!(t.derivative(5.0), 0.0);
    }

    #[test]
    fn refuses_fewer_than_two_points() {
        assert_eq!(Table::new(vec![0.0], vec![0.0]).unwrap_err(), TableError::TooFewPoints);
        assert_eq!(Table::new(vec![], vec![]).unwrap_err(), TableError::TooFewPoints);
    }

    #[test]
    fn refuses_unequal_lengths() {
        assert_eq!(
            Table::new(vec![0.0, 1.0, 2.0], vec![0.0, 1.0]).unwrap_err(),
            TableError::LengthMismatch
        );
    }

    #[test]
    fn refuses_non_strictly_increasing_xs() {
        assert_eq!(
            Table::new(vec![0.0, 1.0, 1.0], vec![0.0, 1.0, 2.0]).unwrap_err(),
            TableError::NotStrictlyIncreasing
        );
        assert_eq!(
            Table::new(vec![0.0, 2.0, 1.0], vec![0.0, 1.0, 2.0]).unwrap_err(),
            TableError::NotStrictlyIncreasing
        );
    }

    #[test]
    fn refuses_any_mode_but_linear() {
        assert_eq!(
            Table::with_mode(vec![0.0, 1.0], vec![0.0, 1.0], "cubic").unwrap_err(),
            TableError::UnsupportedMode("cubic".to_string())
        );
    }

    #[test]
    fn table_is_a_behavior_kind_whose_jacobian_is_the_segment_slope() {
        let t = Table::new(vec![0.0, 2.0, 4.0], vec![0.0, 10.0, 0.0]).unwrap();
        assert_eq!(BehaviorKind::value(&t, &[1.0]), 5.0);
        let mut out = [0.0];
        BehaviorKind::jacobian(&t, &[1.0], &mut out);
        assert_eq!(out[0], 5.0, "same segment slope as Table::derivative");
    }
}
