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
use super::limit::Limits;
use super::plan::{InputPlan, InstancePlan, TermPlan};

/// A complex stamp list.
pub type ComplexStamps = Vec<Stamp<AnalogReference, Complex64>>;

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

/// One instance at the operating point, read at an angular frequency: what the small-signal
/// quantities of its inputs and terms are there.
pub(crate) struct SmallSignal<'a> {
    /// The instance's plan.
    instance: &'a InstancePlan,
    /// The instance's evaluated state.
    state: &'a InstanceState,
    /// The angular frequency, radians per second.
    omega: f64,
}

impl<'a> SmallSignal<'a> {
    /// The instance `instance` in `state`, read at `omega`.
    pub(crate) fn new(instance: &'a InstancePlan, state: &'a InstanceState, omega: f64) -> SmallSignal<'a> {
        SmallSignal { instance, state, omega }
    }

    /// The response of term `t`'s operator at the angular frequency.
    pub(crate) fn response(&self, t: &TermPlan) -> Complex64 {
        match t.op {
            op::DDT => Complex64::new(0.0, self.omega),
            op::DELAY => {
                let td = t.td.map_or(0.0, |(f, w)| self.state.fns[f].f64(w));
                Complex64::from_polar(1.0, -self.omega * td)
            }
            _ => Complex64::new(1.0, 0.0),
        }
    }

    /// Adds, for term `t`, its matrix part (`w·g` at (`plus`, `j`) and its negative at (`minus`, `j`)
    /// for each partial) to `matrix` and its stimulus right side to `rhs`, `w` the term's response.
    pub(crate) fn stamp_term(&self, t: &TermPlan, matrix: &mut ComplexStamps, rhs: &mut ComplexStamps) {
        let w = self.response(t);
        self.stamp_matrix(t, w, matrix);
        self.stimulus(t, w, rhs);
    }

    /// Adds `w·g` at (`plus`, `j`) and its negative at (`minus`, `j`) for each
    /// partial of term `t`.
    fn stamp_matrix(&self, t: &TermPlan, w: Complex64, out: &mut ComplexStamps) {
        for (c, g) in Limits::new(self.instance).linear(t, self.state).cols() {
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
    fn stamp_source(t: &TermPlan, s: Complex64, out: &mut ComplexStamps) {
        if let Some(p) = &t.plus {
            out.push(Stamp::Rhs(p.clone(), s));
        }
        if let Some(m) = &t.minus {
            out.push(Stamp::Rhs(m.clone(), -s));
        }
    }

    /// The right side term `t` adds through its partials against `STIM` inputs, `w` its response.
    fn stimulus(&self, t: &TermPlan, w: Complex64, out: &mut ComplexStamps) {
        let Some((fi, _)) = t.value else { return };
        for (p, i) in &t.via {
            let x = &self.instance.inputs[*i];
            if x.kind == input::STIM {
                let g = self.state.fns[fi].jac[*p];
                Self::stamp_source(t, -(w * g * self.phasor(x)), out);
            }
        }
    }

    /// The `F64` a strength cell holds, 0 when no function writes it.
    fn strength(&self, at: Option<(usize, usize)>) -> f64 {
        at.map_or(0.0, |(f, w)| self.state.fns[f].f64(w))
    }

    /// The phasor `RE + j·IM` of a `STIM` input.
    fn phasor(&self, x: &InputPlan) -> Complex64 {
        Complex64::new(self.strength(x.re), self.strength(x.im))
    }

    /// The injection of input `input`: per row, the sum over the terms that read it of `±w·g`,
    /// `w` the term's response at the frequency.
    fn injection_rows(&self, input: usize) -> Vec<(AnalogReference, Complex64)> {
        let mut rows: Vec<(AnalogReference, Complex64)> = Vec::new();
        let mut add = |row: &Option<AnalogReference>, value: Complex64| {
            let Some(row) = row else { return };
            match rows.iter_mut().find(|(known, _)| known.idx() == row.idx()) {
                Some((_, sum)) => *sum += value,
                None => rows.push((row.clone(), value)),
            }
        };
        for term in &self.instance.terms {
            let Some((function, _)) = term.value else { continue };
            let response = self.response(term);
            for (partial, _) in term.via.iter().filter(|(_, read)| *read == input) {
                let value = response * self.state.fns[function].jac[*partial];
                add(&term.plus, value);
                add(&term.minus, -value);
            }
        }
        rows
    }

    /// Appends one column per `NOISE` input of the instance.
    pub(crate) fn noise_columns(&self, out: &mut Vec<NoiseColumn>) {
        for (position, input) in self.instance.inputs.iter().enumerate().filter(|(_, input)| input.kind == input::NOISE) {
            out.push(NoiseColumn {
                name: format!("{}.{}", self.instance.path, input.name),
                instance: self.instance.path.clone(),
                input: position,
                psd: self.strength(input.psd),
                rows: self.injection_rows(position),
            });
        }
    }

    /// Appends one column per `STIM` input of the instance.
    pub(crate) fn stimulus_columns(&self, out: &mut Vec<StimulusColumn>) {
        for (position, input) in self.instance.inputs.iter().enumerate().filter(|(_, input)| input.kind == input::STIM) {
            let phasor = self.phasor(input);
            out.push(StimulusColumn {
                name: format!("{}.{}", self.instance.path, input.name),
                phase: phasor.im.atan2(phasor.re),
                rows: self.injection_rows(position),
            });
        }
    }

    /// The linearization of `term`, with the response of its operator at the frequency.
    pub(crate) fn term_linearization(&self, term: &TermPlan) -> TermLinearization {
        let response = self.response(term);
        let unknowns = Limits::new(self.instance).linear(term, self.state).cols().map(|(column, partial)| (column.clone(), response * partial)).collect();
        let noise = match term.value {
            Some((function, _)) => term
                .via
                .iter()
                .filter(|(_, read)| self.instance.inputs[*read].kind == input::NOISE)
                .map(|(partial, read)| (*read, response * self.state.fns[function].jac[*partial]))
                .collect(),
            None => Vec::new(),
        };
        TermLinearization { unknowns, noise }
    }
}
