//! `table_model` at elaboration (§22.2.15, PI-10): the control string is parsed (`E0703`), the table is
//! read as the constant it is and validated (`E0704`), and the call lowers to `Expr::Table` over the
//! samples, which the run only evaluates.

use piperine_ir::{Constant, Expr, ExprId as LoweredExpr, Extrapolation, Origin, Real, TableControl, TableDegree, ValueType};

use super::consts::{Bindings, CompositeKind, ConstTree};
use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::msl::Diagnostic;
use crate::pom::ExprId;

/// The control string of a `table_model` (§22.2.15 rules 2 to 4): one interpolation character, then zero,
/// one or two extrapolation characters. One extrapolation character applies to both ends, two to the low
/// end and then the high end, none is `L` at both.
pub(super) struct ControlText<'a>(pub(super) &'a str);

impl ControlText<'_> {
    /// The control the text says, or why it is none (`E0703`).
    pub(super) fn parse(&self) -> Result<TableControl, String> {
        let mut chars = self.0.chars();
        let degree = match chars.next() {
            Some('D') => TableDegree::Closest,
            Some('1') => TableDegree::Linear,
            Some('2') => TableDegree::Quadratic,
            Some('3') => TableDegree::Cubic,
            _ => return Err(format!("`{}` does not start with an interpolation character: `D`, `1`, `2` or `3`", self.0)),
        };
        let ends: Vec<Extrapolation> = chars
            .map(|c| match c {
                'C' => Ok(Extrapolation::Constant),
                'L' => Ok(Extrapolation::Linear),
                'E' => Ok(Extrapolation::Error),
                other => Err(format!("`{}` has `{other}` where an extrapolation character, `C`, `L` or `E`, is expected", self.0)),
            })
            .collect::<Result<_, _>>()?;
        let (low, high) = match ends.as_slice() {
            [] => (Extrapolation::Linear, Extrapolation::Linear),
            [both] => (*both, *both),
            [low, high] => (*low, *high),
            _ => return Err(format!("`{}` has more than two extrapolation characters", self.0)),
        };
        Ok(TableControl { degree, low, high })
    }
}

/// The samples of a `table_model`, `(abscissa, value)` pairs, as the elaborator validates them.
pub(super) struct Samples<'a>(pub(super) &'a [(f64, f64)]);

impl Samples<'_> {
    /// How many points the interpolation of `degree` needs: two for the closest sample and the line, one more
    /// for each degree of a spline.
    fn needed(degree: TableDegree) -> usize {
        match degree {
            TableDegree::Closest | TableDegree::Linear => 2,
            TableDegree::Quadratic => 3,
            TableDegree::Cubic => 4,
        }
    }

    /// Why the samples are no table for `degree` (`E0704`): fewer points than the interpolation needs, or the
    /// first row, counted from 1, whose abscissa does not exceed the one before.
    pub(super) fn refusal(&self, degree: TableDegree) -> Option<String> {
        let needed = Self::needed(degree);
        if self.0.len() < needed {
            return Some(format!("the table has {} point(s) and the interpolation needs at least {needed}", self.0.len()));
        }
        let row = self.0.windows(2).position(|pair| pair[0].0 >= pair[1].0)?;
        Some(format!("the abscissae are not strictly increasing: row {} ({}) does not exceed row {} ({})", row + 2, self.0[row + 1].0, row + 1, self.0[row].0))
    }
}

impl Elaboration<'_, '_> {
    /// `table_model(x, table, control)` at `expr`: the control and the table are elaboration constants; the call is
    /// `Expr::Table` over `x` and the samples. `None` after a diagnostic (`E0703`, `E0704`).
    pub(super) fn lower_table_model(&mut self, scope: &mut Scope, expr: ExprId, args: &[ExprId], origin: Origin) -> Option<LoweredExpr> {
        let [x, table, control] = args else {
            self.unsupported(scope.file, self.design.expr_node(expr).span, "a `table_model` called with other than three arguments");
            return None;
        };
        let (control_span, table_span) = (self.design.expr_node(*control).span, self.design.expr_node(*table).span);
        let text = match self.consts.text_of_expression(*control, &Bindings::new()) {
            Ok((_, text)) => text,
            Err(fault) => {
                self.const_fault(&fault, "the control of a `table_model`");
                return None;
            }
        };
        let parsed = match (ControlText(&text).parse(), self.table_samples(scope, *table)?) {
            (Ok(parsed), samples) => (parsed, samples),
            (Err(why), _) => {
                self.report(scope.file, Diagnostic::new("E0703", format!("the control string of `table_model` is not of the form of §22.2.15: {why}"), control_span, "the control string"));
                return None;
            }
        };
        let (control, samples) = parsed;
        if let Some(why) = Samples(&samples).refusal(control.degree) {
            self.report(scope.file, Diagnostic::new("E0704", format!("the table of `table_model` is refused: {why}"), table_span, "the table"));
            return None;
        }
        let x = self.lower_expr(scope, *x)?;
        let table = samples.iter().map(|(abscissa, value)| (Real::new(*abscissa), Real::new(*value))).collect();
        Some(self.emit(Expr::Table { x, table, control }, ValueType::F64, origin))
    }

    /// The `(abscissa, value)` pairs the constant `table` folds to; `None` after a diagnostic.
    fn table_samples(&mut self, scope: &Scope, table: ExprId) -> Option<Vec<(f64, f64)>> {
        let span = self.design.expr_node(table).span;
        let tree = match self.consts.tree(table, &Bindings::new()) {
            Ok(tree) => tree,
            Err(fault) => {
                self.const_fault(&fault, "the table of a `table_model`");
                return None;
            }
        };
        let pair = |row: &ConstTree| match row {
            ConstTree::Composite { kind: CompositeKind::Tuple, parts } => match parts.as_slice() {
                [ConstTree::Scalar(Constant::Real(a)), ConstTree::Scalar(Constant::Real(v))] => Some((a.get(), v.get())),
                _ => None,
            },
            _ => None,
        };
        let samples = match &tree {
            ConstTree::Composite { kind: CompositeKind::Array, parts } => parts.iter().map(pair).collect::<Option<Vec<_>>>(),
            _ => None,
        };
        if samples.is_none() {
            self.unsupported(scope.file, span, "a `table_model` table that is not an array of `(f64, f64)` pairs known at elaboration");
        }
        samples
    }
}
