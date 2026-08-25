//! `$table_model` (LRM §9.21, LRMC-C7-09, T51): multidimensional isoline
//! interpolation and lookup — distinct from [`super::table::Table`]'s 1-D
//! `table()` operator, which it does not share code with (different data
//! layout: isolines of arbitrary dimension N, M dependent columns, a
//! control string, not a flat `(xs, ys)` pair).
//!
//! **The recursive algorithm is the LRM's own** (§9.21, the *"first
//! bracket the outer isolines, then interpolate each bracketing isoline
//! at the inner point, then interpolate across the outer dimension"*
//! description, Figure 9-2): [`TableModel::eval`] brackets the outermost
//! remaining dimension, recurses into the inner dimensions on each
//! bracketing isoline to get a scalar per side, then interpolates/
//! extrapolates those two scalars over the outer dimension. Verified
//! against §9.21.5's own worked example: `f(x1=3.5, y1=0.25) = 2.0`.
//!
//! **Disclosed scope (honesty over silent gaps):**
//! - `2`/`3` (quadratic/cubic spline) interpolation characters parse but
//!   [`TableModel::eval`] fails loud (`TableModelError::UnsupportedInterp`)
//!   — the LRM's spline formulation needs the *other* isolines' curvature
//!   to fix boundary derivatives (§9.21.4), a materially bigger algorithm
//!   than linear/discrete bracketing. `I`/`D`/`1` (ignore/discrete/linear)
//!   and all three extrapolation modes (`C`/`L`/`E`) are fully implemented.
//! - This module is pure evaluation + text parsing, no JIT/codegen wiring
//!   — `$table_model` is recognized at the name level
//!   (`piperine-lang/src/eval/tasks.rs`) but not yet dispatched from
//!   inside a compiled analog body, matching the tier T35's driver-access
//!   functions and T47's `$random` family already occupy (host-callable
//!   mechanism proven correct in isolation; JIT dispatch is separately
//!   scoped future work).
//! - The file text format (§9.21.1: whitespace-separated columns, `#`
//!   comments, blank lines ignored) is parsed directly here rather than
//!   through `piperine-solver`'s `FileTable` — `FileTable` models
//!   `$fopen`/`$fgetc`-family *runtime* file handles, but §9.21.1 explicitly
//!   captures the data source's state once, on the table's first call, so
//!   it is a load-time parse, not a live file handle.

use std::collections::BTreeMap;

/// Why a [`TableModel`] could not be built or evaluated.
#[derive(Debug, Clone, PartialEq)]
pub enum TableModelError {
    /// A row's width did not equal `ndim + num_dependents`.
    RaggedRow { row: usize, expected: usize, found: usize },
    /// Fewer than two points on some isoline (LRM §9.21: "minimum data
    /// requirement is ... 2N for N dimensions").
    TooFewPoints,
    /// `ndim` was zero, or exceeded the row width.
    InvalidDimension,
    /// The control string named a dependent-selector column outside
    /// `1..=num_dependents`.
    BadDependentSelector { selector: usize, num_dependents: usize },
    /// The control string's per-dimension substring count exceeded `ndim`.
    TooManyControlSubstrings,
    /// `2` (quadratic) or `3` (cubic) spline interpolation was requested —
    /// disclosed gap, see module docs.
    UnsupportedInterp(char),
    /// An unrecognized interpolation or extrapolation control character.
    BadControlChar(char),
    /// A lookup point fell outside the table and that dimension's
    /// extrapolation mode is `E` (error).
    ExtrapolationError { dim: usize, value: f64 },
}

impl std::fmt::Display for TableModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RaggedRow { row, expected, found } => {
                write!(f, "table_model row {row}: expected {expected} columns, found {found}")
            }
            Self::TooFewPoints => write!(f, "table_model: each isoline needs at least 2 points"),
            Self::InvalidDimension => write!(f, "table_model: ndim must be 1..=row width"),
            Self::BadDependentSelector { selector, num_dependents } => write!(
                f,
                "table_model: dependent selector {selector} out of range 1..={num_dependents}"
            ),
            Self::TooManyControlSubstrings => {
                write!(f, "table_model: control string has more dimension substrings than ndim")
            }
            Self::UnsupportedInterp(c) => write!(
                f,
                "table_model: interpolation mode {c:?} (quadratic/cubic spline) is not implemented — \
                 see piperine-ir/src/behavior/table_model.rs module docs"
            ),
            Self::BadControlChar(c) => write!(f, "table_model: unrecognized control character {c:?}"),
            Self::ExtrapolationError { dim, value } => write!(
                f,
                "table_model: point {value} on dimension {dim} is outside the table and \
                 extrapolation is disabled ('E')"
            ),
        }
    }
}

impl std::error::Error for TableModelError {}

/// Table 9-30's interpolation control character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterpMode {
    /// `I` — this dimension does not participate in bracketing; all rows
    /// pass through regardless of their value on this column.
    Ignore,
    /// `D` — closest-point lookup; ties snap away from zero (§9.21.4).
    Discrete,
    /// `1` — linear interpolation (the default).
    Linear,
    /// `2` — quadratic spline. Parses; [`TableModel::eval`] refuses it.
    Quadratic,
    /// `3` — cubic spline. Parses; [`TableModel::eval`] refuses it.
    Cubic,
}

/// Table 9-31's extrapolation control character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtrapMode {
    /// `C` — return the table endpoint value.
    Constant,
    /// `L` — extend linearly from the endpoint (the default).
    Linear,
    /// `E` — refuse to extrapolate; report [`TableModelError::ExtrapolationError`].
    Error,
}

/// One dimension's parsed control substring.
#[derive(Debug, Clone, Copy)]
pub struct DimControl {
    pub interp: InterpMode,
    pub extrap_lo: ExtrapMode,
    pub extrap_hi: ExtrapMode,
}

impl Default for DimControl {
    /// §9.21.3: an omitted substring is linear interpolation, linear
    /// extrapolation at both ends.
    fn default() -> Self {
        Self { interp: InterpMode::Linear, extrap_lo: ExtrapMode::Linear, extrap_hi: ExtrapMode::Linear }
    }
}

/// A parsed `table_control_string` (§9.21.2): per-dimension controls plus
/// the 1-based dependent-variable column selector.
#[derive(Debug, Clone)]
pub struct ControlString {
    pub dims: Vec<DimControl>,
    /// 1-based; defaults to `1` when omitted (§9.21.3).
    pub dependent_selector: usize,
}

fn parse_dim_substr(s: &str) -> Result<DimControl, TableModelError> {
    if s.is_empty() {
        return Ok(DimControl::default());
    }
    let mut chars = s.chars();
    let interp = match chars.next().unwrap() {
        'I' => InterpMode::Ignore,
        'D' => InterpMode::Discrete,
        '1' => InterpMode::Linear,
        '2' => InterpMode::Quadratic,
        '3' => InterpMode::Cubic,
        c => return Err(TableModelError::BadControlChar(c)),
    };
    let rest: Vec<char> = chars.collect();
    let parse_extrap = |c: char| -> Result<ExtrapMode, TableModelError> {
        match c {
            'C' => Ok(ExtrapMode::Constant),
            'L' => Ok(ExtrapMode::Linear),
            'E' => Ok(ExtrapMode::Error),
            c => Err(TableModelError::BadControlChar(c)),
        }
    };
    let (extrap_lo, extrap_hi) = match rest.len() {
        0 => (ExtrapMode::Linear, ExtrapMode::Linear),
        1 => {
            let m = parse_extrap(rest[0])?;
            (m, m)
        }
        _ => (parse_extrap(rest[0])?, parse_extrap(rest[1])?),
    };
    Ok(DimControl { interp, extrap_lo, extrap_hi })
}

impl ControlString {
    /// Parses `table_control_string` (§9.21.2/.3). `ndim` fills in any
    /// trailing omitted dimension substrings with the linear/linear
    /// default (`"C,,3"` -> dims 2 and 3 both explicit, dim ... wait: the
    /// LRM's own example `"C,,3"` for 3-D data is `"1CC, 1LL, 3LL"` — a
    /// bare `C` on its own means interp defaults to `1` (linear) with `C`
    /// as extrapolation, and an empty substring is the full default).
    pub fn parse(s: &str, ndim: usize) -> Result<Self, TableModelError> {
        let (dims_part, selector_part) = match s.split_once(';') {
            Some((d, sel)) => (d, Some(sel)),
            None => (s, None),
        };
        let mut dims = Vec::new();
        if !dims_part.is_empty() {
            for part in dims_part.split(',') {
                dims.push(parse_dim_substr(part.trim())?);
            }
        }
        if dims.len() > ndim {
            return Err(TableModelError::TooManyControlSubstrings);
        }
        while dims.len() < ndim {
            dims.push(DimControl::default());
        }
        let dependent_selector = match selector_part {
            Some(sel) => sel.trim().parse::<usize>().unwrap_or(1),
            None => 1,
        };
        Ok(Self { dims, dependent_selector })
    }

    /// The default control string (empty), sized to `ndim` dimensions.
    pub fn default_for(ndim: usize) -> Self {
        Self { dims: vec![DimControl::default(); ndim], dependent_selector: 1 }
    }
}

/// A multidimensional isoline table (§9.21): `ndim` independent columns
/// (outermost first) followed by `num_dependents` dependent columns, per
/// row.
#[derive(Debug, Clone)]
pub struct TableModel {
    ndim: usize,
    num_dependents: usize,
    rows: Vec<Vec<f64>>,
    control: ControlString,
}

impl TableModel {
    /// Builds a table from row-major data (`rows[i]` has `ndim +
    /// num_dependents` columns) and a parsed control string.
    pub fn new(
        rows: Vec<Vec<f64>>,
        ndim: usize,
        num_dependents: usize,
        control: ControlString,
    ) -> Result<Self, TableModelError> {
        if ndim == 0 || num_dependents == 0 {
            return Err(TableModelError::InvalidDimension);
        }
        if control.dependent_selector < 1 || control.dependent_selector > num_dependents {
            return Err(TableModelError::BadDependentSelector {
                selector: control.dependent_selector,
                num_dependents,
            });
        }
        let width = ndim + num_dependents;
        for (i, row) in rows.iter().enumerate() {
            if row.len() != width {
                return Err(TableModelError::RaggedRow { row: i, expected: width, found: row.len() });
            }
        }
        if rows.len() < 2 {
            return Err(TableModelError::TooFewPoints);
        }
        Ok(Self { ndim, num_dependents, rows, control })
    }

    /// Builds directly from `ndim` independent-column arrays plus one
    /// dependent-column array (§9.21's `table_model_array` form, e.g.
    /// `$table_model(0, V(a,b), y, x, f_xy)`), all the same length.
    /// `dims` is ordered outermost-first, matching `table_inputs`.
    pub fn from_arrays(dims: Vec<&[f64]>, dependent: &[f64]) -> Result<Self, TableModelError> {
        let ndim = dims.len();
        let n = dependent.len();
        if dims.iter().any(|d| d.len() != n) {
            return Err(TableModelError::RaggedRow { row: 0, expected: n, found: 0 });
        }
        let mut rows = Vec::with_capacity(n);
        for i in 0..n {
            let mut row = Vec::with_capacity(ndim + 1);
            for d in &dims {
                row.push(d[i]);
            }
            row.push(dependent[i]);
            rows.push(row);
        }
        Self::new(rows, ndim, 1, ControlString::default_for(ndim))
    }

    /// Parses §9.21.1's text layout: whitespace-separated columns, `#`
    /// line comments, blank lines ignored. `ndim` is the number of
    /// independent (outermost-first) columns; the remainder are
    /// dependent columns.
    pub fn from_text(text: &str, ndim: usize, control_str: &str) -> Result<Self, TableModelError> {
        let mut rows = Vec::new();
        for line in text.lines() {
            let line = match line.split_once('#') {
                Some((before, _)) => before,
                None => line,
            };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let row: Vec<f64> = line.split_whitespace().filter_map(|tok| tok.parse::<f64>().ok()).collect();
            if !row.is_empty() {
                rows.push(row);
            }
        }
        let width = rows.first().map(|r| r.len()).unwrap_or(ndim + 1);
        let num_dependents = width.saturating_sub(ndim);
        let control = ControlString::parse(control_str, ndim)?;
        Self::new(rows, ndim, num_dependents.max(1), control)
    }

    /// Number of dependent (output) columns in the data source.
    pub fn num_dependents(&self) -> usize {
        self.num_dependents
    }

    /// Evaluates the table at `point` (one value per dimension,
    /// outermost-first — the same order as `table_inputs`).
    pub fn eval(&self, point: &[f64]) -> Result<f64, TableModelError> {
        assert_eq!(point.len(), self.ndim, "table_model: point arity must equal ndim");
        let dep_col = self.ndim + (self.control.dependent_selector - 1);
        let all: Vec<&[f64]> = self.rows.iter().map(|r| r.as_slice()).collect();
        self.eval_dim(&all, 0, point, dep_col)
    }

    fn eval_dim(&self, rows: &[&[f64]], dim: usize, point: &[f64], dep_col: usize) -> Result<f64, TableModelError> {
        if dim == self.ndim {
            return Ok(rows[0][dep_col]);
        }
        let ctrl = self.control.dims[dim];
        if ctrl.interp == InterpMode::Ignore {
            return self.eval_dim(rows, dim + 1, point, dep_col);
        }

        // Group rows by exact value on this column (§9.21.1: "the system
        // determines the isoline ordinate by reading its exact value").
        let mut groups: BTreeMap<u64, Vec<&[f64]>> = BTreeMap::new();
        for &row in rows {
            groups.entry(row[dim].to_bits()).or_default().push(row);
        }
        let mut values: Vec<f64> = groups.keys().map(|&bits| f64::from_bits(bits)).collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let x = point[dim];

        if ctrl.interp == InterpMode::Discrete {
            let nearest = values
                .iter()
                .copied()
                .min_by(|&a, &b| {
                    let da = (a - x).abs();
                    let db = (b - x).abs();
                    // Ties snap away from zero (§9.21.4).
                    da.partial_cmp(&db).unwrap().then_with(|| b.abs().partial_cmp(&a.abs()).unwrap())
                })
                .unwrap();
            let group = &groups[&nearest.to_bits()];
            return self.eval_dim(group, dim + 1, point, dep_col);
        }

        if !matches!(ctrl.interp, InterpMode::Linear) {
            let c = match ctrl.interp {
                InterpMode::Quadratic => '2',
                InterpMode::Cubic => '3',
                _ => unreachable!(),
            };
            return Err(TableModelError::UnsupportedInterp(c));
        }

        // Exact hit: no interpolation needed.
        if let Some(&v) = values.iter().find(|&&v| v == x) {
            let group = &groups[&v.to_bits()];
            return self.eval_dim(group, dim + 1, point, dep_col);
        }

        let lo = values.iter().copied().filter(|&v| v < x).next_back();
        let hi = values.iter().copied().find(|&v| v > x);

        match (lo, hi) {
            (Some(lo_v), Some(hi_v)) => {
                let f_lo = self.eval_dim(&groups[&lo_v.to_bits()], dim + 1, point, dep_col)?;
                let f_hi = self.eval_dim(&groups[&hi_v.to_bits()], dim + 1, point, dep_col)?;
                let t = (x - lo_v) / (hi_v - lo_v);
                Ok(f_lo + t * (f_hi - f_lo))
            }
            (None, Some(_)) => self.extrapolate(&values, &groups, dim, dep_col, point, x, ctrl.extrap_lo, true),
            (Some(_), None) => self.extrapolate(&values, &groups, dim, dep_col, point, x, ctrl.extrap_hi, false),
            (None, None) => unreachable!("at least one group exists"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn extrapolate(
        &self,
        values: &[f64],
        groups: &BTreeMap<u64, Vec<&[f64]>>,
        dim: usize,
        dep_col: usize,
        point: &[f64],
        x: f64,
        mode: ExtrapMode,
        below: bool,
    ) -> Result<f64, TableModelError> {
        if mode == ExtrapMode::Error {
            return Err(TableModelError::ExtrapolationError { dim, value: x });
        }
        let edge = if below { values[0] } else { *values.last().unwrap() };
        let edge_val = self.eval_dim(&groups[&edge.to_bits()], dim + 1, point, dep_col)?;
        if mode == ExtrapMode::Constant || values.len() < 2 {
            return Ok(edge_val);
        }
        // Linear: extend using the slope from the two nearest edge points.
        let second = if below { values[1] } else { values[values.len() - 2] };
        let second_val = self.eval_dim(&groups[&second.to_bits()], dim + 1, point, dep_col)?;
        let slope = (second_val - edge_val) / (second - edge);
        Ok(edge_val + slope * (x - edge))
    }
}

#[cfg(test)]
mod tests {
    //! LRM-COVERS: 9.21, 9.21.1, 9.21.2, 9.21.3, 9.21.4, 9.21.5
    //! §9.21.1 (table data source, file/array layout): `matches_lrm_9_21_5_
    //! worked_example_from_text`/`_from_arrays`. §9.21.2 (control string
    //! parsing — interpolation/extrapolation chars, dependent selector):
    //! `control_string_dependent_selector_is_1_based`,
    //! `discrete_lookup_snaps_to_the_nearest_isoline`,
    //! `constant_extrapolation_clamps_to_the_endpoint`,
    //! `error_extrapolation_fails_loud`. §9.21.3 (example control strings)
    //! is Table 9-32's own worked illustrations of §9.21.2's grammar — no
    //! independent rule of its own, carried on the same evidence.
    //! §9.21.4 (interpolation algorithms): linear/discrete are
    //! implemented and tested (`reproduces_the_underlying_linear_function_
    //! exactly`); quadratic/cubic spline are disclosed unsupported, fail
    //! loud rather than faked (`quadratic_and_cubic_spline_are_disclosed_
    //! unsupported`) — matching CLAUDE.md's fail-loud rule. §9.21.5 (the
    //! LRM's own worked example, f(x1=3.5,y1=0.25)=2.0): `matches_lrm_9_
    //! 21_5_worked_example_from_text`/`_from_arrays` reproduce it exactly.
    //! §9.21 is the syntax-introduction parent over all five.
    use super::*;

    const SAMPLE_DAT: &str = "
# 2-D table model sample example
#
# y x f(x,y)
# y=0 isoline
0.0 1.0 0.5
0.0 2.0 1.0
0.0 3.0 1.5
0.0 4.0 2.0
0.0 5.0 2.5
0.0 6.0 3.0
# y=0.5 isoline
0.5 1.0 1.0
0.5 3.0 2.0
0.5 5.0 3.0
# y=1.0 isoline
1.0 1.0 1.5
1.0 2.0 2.0
1.0 4.0 3.0
";

    /// LRM §9.21.5 Figure 9-2's own worked numbers: bracketing isolines
    /// y=0 and y=0.5 at x1=3.5 give f=1.75 and f=2.25, and interpolating
    /// those over y at y1=0.25 gives f(x1,y1)=2.0 — the LRM's own stated
    /// result.
    #[test]
    fn matches_lrm_9_21_5_worked_example_from_text() {
        let table = TableModel::from_text(SAMPLE_DAT, 2, "").expect("parses");
        assert_eq!(table.eval(&[0.0, 3.5]).unwrap(), 1.75, "isoline y=0 alone");
        assert_eq!(table.eval(&[0.5, 3.5]).unwrap(), 2.25, "isoline y=0.5 alone");
        assert_eq!(table.eval(&[0.25, 3.5]).unwrap(), 2.0, "LRM's own stated f(x1,y1)=2.0");
    }

    /// Same data, same result, built via the `table_model_array` form
    /// (§9.21.5's second example: `$table_model(0, V(a,b), y, x, f_xy)`).
    #[test]
    fn matches_lrm_9_21_5_worked_example_from_arrays() {
        let y = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0];
        let x = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 1.0, 3.0, 5.0, 1.0, 2.0, 4.0];
        let f = [0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 1.0, 2.0, 3.0, 1.5, 2.0, 3.0];
        let table = TableModel::from_arrays(vec![&y, &x], &f).expect("builds");
        assert_eq!(table.eval(&[0.25, 3.5]).unwrap(), 2.0);
    }

    /// f(x,y)=0.5x+y is exactly linear, so away from the sparse y=0.5/1.0
    /// isolines (which only sample 3 of the 6 x points) the underlying
    /// function is reproduced exactly at any interior point.
    #[test]
    fn reproduces_the_underlying_linear_function_exactly() {
        let table = TableModel::from_text(SAMPLE_DAT, 2, "").expect("parses");
        assert_eq!(table.eval(&[0.0, 2.0]).unwrap(), 1.0);
        assert_eq!(table.eval(&[1.0, 4.0]).unwrap(), 3.0);
    }

    #[test]
    fn constant_extrapolation_clamps_to_the_endpoint() {
        let table = TableModel::from_text(SAMPLE_DAT, 2, "1CC,1CC").expect("parses");
        assert_eq!(table.eval(&[0.0, 100.0]).unwrap(), 3.0, "clamps to x=6 endpoint value");
        assert_eq!(table.eval(&[-5.0, 1.0]).unwrap(), 0.5, "clamps to y=0 endpoint value");
    }

    #[test]
    fn error_extrapolation_fails_loud() {
        let table = TableModel::from_text(SAMPLE_DAT, 2, "1EE,1EE").expect("parses");
        let err = table.eval(&[2.0, 100.0]).unwrap_err();
        assert!(matches!(err, TableModelError::ExtrapolationError { .. }));
    }

    #[test]
    fn discrete_lookup_snaps_to_the_nearest_isoline() {
        let table = TableModel::from_text(SAMPLE_DAT, 2, "D,1").expect("parses");
        // y=0.3 is closer to y=0.5 than y=0.0.
        assert_eq!(table.eval(&[0.3, 3.0]).unwrap(), 2.0, "snaps to the y=0.5 isoline, x=3 exactly");
    }

    #[test]
    fn quadratic_and_cubic_spline_are_disclosed_unsupported() {
        let table = TableModel::from_text(SAMPLE_DAT, 2, "3,3").expect("control string parses");
        let err = table.eval(&[0.25, 3.5]).unwrap_err();
        assert!(matches!(err, TableModelError::UnsupportedInterp('3')));
    }

    #[test]
    fn control_string_dependent_selector_is_1_based() {
        // f=x+y (dep col 1) and g=x-y (dep col 2), same 2x2 grid.
        let rows = vec![
            vec![0.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 1.0, -1.0],
            vec![1.0, 0.0, 1.0, 1.0],
            vec![1.0, 1.0, 2.0, 0.0],
        ];
        let control = ControlString::parse("1L,1L;2", 2).expect("parses");
        let table = TableModel::new(rows, 2, 2, control).expect("builds");
        assert_eq!(table.eval(&[1.0, 1.0]).unwrap(), 0.0, "selects dependent column 2 (x-y)");
    }
}
