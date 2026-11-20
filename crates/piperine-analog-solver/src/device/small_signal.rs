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
use super::limit::linear;
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

/// One noise source: a `NOISE` input injected through every term that reads
/// it, so one input on two terms is one fully correlated source (A2).
#[derive(Clone, Debug, PartialEq)]
pub struct NoiseColumn {
    /// `<instance path>.<input name>`.
    pub name: String,
    /// The path of the instance that declares the input.
    pub instance: String,
    /// The input's position among the instance's inputs: what tells apart two inputs of one name.
    pub input: usize,
    /// Its density `PSD` at this frequency.
    pub psd: f64,
    /// The injection `b`: per row, the sum over terms of `±w·g`.
    pub rows: Vec<(AnalogReference, Complex64)>,
}

/// One small-signal stimulus: a `STIM` input injected through every term that reads it, at unit
/// magnitude, with the phase of its phasor (the input-referred noise reads the transfer from
/// it, design D11).
#[derive(Clone, Debug, PartialEq)]
pub struct StimulusColumn {
    /// `<instance path>.<input name>`.
    pub name: String,
    /// The phase `atan2(IM, RE)` of the stimulus phasor, radians.
    pub phase: f64,
    /// The injection: per row, the sum over terms of `±w·g`, in the sign convention of
    /// [`NoiseColumn::rows`].
    pub rows: Vec<(AnalogReference, Complex64)>,
}

/// A term at the operating point, linearized against the unknowns and the noise inputs, its
/// operator's response already applied: the small-signal value of a flow output read from the
/// term itself.
#[derive(Clone, Debug, PartialEq)]
pub struct TermLinearization {
    /// `w·∂v/∂x_j` for each unknown the term's value depends on.
    pub unknowns: Vec<(AnalogReference, Complex64)>,
    /// `w·∂v/∂n_k` for each `NOISE` input `k` (its position among the instance's inputs) the
    /// term's value reads directly.
    pub noise: Vec<(usize, Complex64)>,
}

/// The injection of input `input` of `inst`: per row, the sum over the terms that read it of
/// `±w·g`, `w` the term's response at `omega`.
fn injection_rows(inst: &InstancePlan, st: &InstanceState, omega: f64, input: usize) -> Vec<(AnalogReference, Complex64)> {
    let mut rows: Vec<(AnalogReference, Complex64)> = Vec::new();
    let mut add = |r: &Option<AnalogReference>, v: Complex64| {
        let Some(r) = r else { return };
        match rows.iter_mut().find(|(q, _)| q.idx() == r.idx()) {
            Some((_, s)) => *s += v,
            None => rows.push((r.clone(), v)),
        }
    };
    for t in &inst.terms {
        let Some((fi, _)) = t.value else { continue };
        let w = response(t, st, omega);
        for (p, _) in t.via.iter().filter(|(_, j)| *j == input) {
            let v = w * st.fns[fi].jac[*p];
            add(&t.plus, v);
            add(&t.minus, -v);
        }
    }
    rows
}

/// Appends one column per `NOISE` input of `inst`.
pub(crate) fn noise_columns(inst: &InstancePlan, st: &InstanceState, omega: f64, out: &mut Vec<NoiseColumn>) {
    for (i, x) in inst.inputs.iter().enumerate().filter(|(_, x)| x.kind == input::NOISE) {
        out.push(NoiseColumn {
            name: format!("{}.{}", inst.path, x.name),
            instance: inst.path.clone(),
            input: i,
            psd: strength(st, x.psd),
            rows: injection_rows(inst, st, omega, i),
        });
    }
}

/// Appends one column per `STIM` input of `inst`.
pub(crate) fn stimulus_columns(inst: &InstancePlan, st: &InstanceState, omega: f64, out: &mut Vec<StimulusColumn>) {
    for (i, x) in inst.inputs.iter().enumerate().filter(|(_, x)| x.kind == input::STIM) {
        let phasor = phasor(x, st);
        out.push(StimulusColumn {
            name: format!("{}.{}", inst.path, x.name),
            phase: phasor.im.atan2(phasor.re),
            rows: injection_rows(inst, st, omega, i),
        });
    }
}

/// The linearization of term `t` of `inst`, with the response of its operator at `omega`.
pub(crate) fn term_linearization(t: &TermPlan, inst: &InstancePlan, st: &InstanceState, omega: f64) -> TermLinearization {
    let w = response(t, st, omega);
    let unknowns = linear(t, inst, st).cols.into_iter().map(|(c, g)| (c, w * g)).collect();
    let noise = match t.value {
        Some((fi, _)) => t
            .via
            .iter()
            .filter(|(_, i)| inst.inputs[*i].kind == input::NOISE)
            .map(|(p, i)| (*i, w * st.fns[fi].jac[*p]))
            .collect(),
        None => Vec::new(),
    };
    TermLinearization { unknowns, noise }
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
