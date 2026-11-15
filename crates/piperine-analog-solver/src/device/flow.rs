//! `FLOW` terms (abi.md A1, design `FLOW`): a term `{plus, minus}` of value
//! `v` with partials `g_j` against continuous pins `j` adds the Norton form
//! of its linearization at the iterate: `g_j` at (`plus`, `j`), `−g_j` at
//! (`minus`, `j`), `−(v − Σ g_j·x_j)` to the right side of `plus` and its
//! negative to `minus`. A `GROUND` side has no row.

use crate::math::linear::Stamp;
use crate::netlist::AnalogReference;

use super::assembly::InstanceState;
use super::plan::TermPlan;

/// A term at the iterate: its value and its partials against continuous
/// pins, one entry per declared partial (zeros included, so the positions
/// are the same in every solve).
#[derive(Clone, Debug, Default)]
pub(crate) struct Linear {
    pub v: f64,
    pub cols: Vec<(AnalogReference, f64)>,
}

impl Linear {
    /// `Σ g_j·x_j` over the iterate.
    pub fn dot(&self, x: &[f64]) -> f64 {
        self.cols.iter().fold(0.0, |s, (c, g)| s + g * x[c.idx().expect("a column is an unknown")])
    }
}

/// Term `t`'s value and direct partials from the last call of its writer.
pub(crate) fn direct(t: &TermPlan, st: &InstanceState) -> Linear {
    let Some((fi, w)) = t.value else { return Linear::default() };
    let f = &st.fns[fi];
    Linear { v: f.f64(w), cols: t.direct.iter().map(|(p, c)| (c.clone(), f.jac[*p])).collect() }
}

/// Adds `weight·g_j` at (`plus`, `j`) and its negative at (`minus`, `j`),
/// and `rhs` to the right side of `plus` and its negative to `minus`.
pub(crate) fn stamp_rows(
    t: &TermPlan,
    lin: &Linear,
    weight: f64,
    rhs: f64,
    out: &mut Vec<Stamp<AnalogReference, f64>>,
) {
    for (c, g) in &lin.cols {
        let g = weight * g;
        if let Some(p) = &t.plus {
            out.push(Stamp::Matrix(p.clone(), c.clone(), g));
        }
        if let Some(m) = &t.minus {
            out.push(Stamp::Matrix(m.clone(), c.clone(), -g));
        }
    }
    if let Some(p) = &t.plus {
        out.push(Stamp::Rhs(p.clone(), rhs));
    }
    if let Some(m) = &t.minus {
        out.push(Stamp::Rhs(m.clone(), -rhs));
    }
}

/// The Norton stamps of a `FLOW` term at iterate `x`.
pub(crate) fn stamp_flow(t: &TermPlan, lin: &Linear, x: &[f64], out: &mut Vec<Stamp<AnalogReference, f64>>) {
    stamp_rows(t, lin, 1.0, -(lin.v - lin.dot(x)), out);
}
