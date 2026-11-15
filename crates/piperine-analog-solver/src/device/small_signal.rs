//! Small-signal assembly (abi.md §10 "Ac", design `assemble_ac`): at the
//! operating point, with every limit holding `x̂ = x`, each term adds its
//! partials against continuous pins times its operator's response, 1 for
//! `FLOW`, `jω` for `DDT`, `exp(−jω·td)` for `DELAY`; each partial `g` of a
//! term against a `STIM` input adds `−w·g·(RE + j·IM)` to the right side of
//! `plus` and its negative to `minus`, `w` that term's response (A2: through a
//! `DDT` term it picks up `jω` with no further rule).

use num_complex::Complex64;
use piperine_abi::raw::{input, op};

use crate::math::linear::Stamp;
use crate::netlist::AnalogReference;

use super::assembly::InstanceState;
use super::flow::Linear;
use super::plan::{InputPlan, InstancePlan, TermPlan};

/// A complex stamp list.
pub type ComplexStamps = Vec<Stamp<AnalogReference, Complex64>>;

/// The response of term `t`'s operator at angular frequency `omega`.
pub(crate) fn response(t: &TermPlan, st: &InstanceState, omega: f64) -> Complex64 {
    match t.op {
        op::DDT => Complex64::new(0.0, omega),
        op::DELAY => {
            let td = t.td.map_or(0.0, |(f, w)| st.fns[f].f64(w));
            Complex64::from_polar(1.0, -omega * td)
        }
        _ => Complex64::new(1.0, 0.0),
    }
}

/// Adds `w·g` at (`plus`, `j`) and its negative at (`minus`, `j`) for each
/// partial of `lin`.
pub(crate) fn stamp_matrix(t: &TermPlan, lin: &Linear, w: Complex64, out: &mut ComplexStamps) {
    for (c, g) in &lin.cols {
        let v = w * g;
        if let Some(p) = &t.plus {
            out.push(Stamp::Matrix(p.clone(), c.clone(), v));
        }
        if let Some(m) = &t.minus {
            out.push(Stamp::Matrix(m.clone(), c.clone(), -v));
        }
    }
}

/// Adds `s` to the right side of `plus` and `−s` to that of `minus`.
pub(crate) fn stamp_source(t: &TermPlan, s: Complex64, out: &mut ComplexStamps) {
    if let Some(p) = &t.plus {
        out.push(Stamp::Rhs(p.clone(), s));
    }
    if let Some(m) = &t.minus {
        out.push(Stamp::Rhs(m.clone(), -s));
    }
}

/// The `F64` a strength cell holds, 0 when no function writes it.
fn strength(st: &InstanceState, at: Option<(usize, usize)>) -> f64 {
    at.map_or(0.0, |(f, w)| st.fns[f].f64(w))
}

/// The phasor `RE + j·IM` of a `STIM` input.
fn phasor(x: &InputPlan, st: &InstanceState) -> Complex64 {
    Complex64::new(strength(st, x.re), strength(st, x.im))
}

/// The right side term `t` adds through its partials against `STIM` inputs.
pub(crate) fn stimulus(t: &TermPlan, inst: &InstancePlan, st: &InstanceState, w: Complex64, out: &mut ComplexStamps) {
    let Some((fi, _)) = t.value else { return };
    for (p, i) in &t.via {
        let x = &inst.inputs[*i];
        if x.kind == input::STIM {
            let g = st.fns[fi].jac[*p];
            stamp_source(t, -(w * g * phasor(x, st)), out);
        }
    }
}
