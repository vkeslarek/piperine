//! One solve's evaluation (design `device::assembly`, abi.md F3, A4): the
//! assembly keeps, per analog function, the bytes it last read and the
//! outputs and partials it returned. A function whose read bytes are
//! unchanged is not called and its outputs are reused (F3). The discrete pins
//! analog functions read are the assembly's own byte slots, written by
//! `set_discrete` (abi.md §10). A non-finite value or seeded partial that the
//! solve reads fails it, naming instance, cell and origin (A4).

use piperine_abi::raw::{ambient, op};

use super::ddt::{History, Stage, stamp_ddt};
use super::delay::{Delayed, stamp_delay};
use super::device_error;
use super::flow::{stamp_flow, stamp_rows};
use super::limit::{LimitState, advance, linear, take_iterates, vetoes};
use super::plan::{AnalogPlan, FnPlan, InstancePlan, Source, TermPlan};
use crate::error::Error;
use crate::math::linear::Stamp;
use crate::netlist::AnalogReference;

/// The coordinates of one solve (abi.md §4.2, ambient inputs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ambient {
    /// The time of the solve; 0 where time does not exist.
    pub time: f64,
    /// The sweep point's frequency, else 0.
    pub freq: f64,
    /// An equilibrium solve.
    pub is_static: bool,
    /// The continuation variables; at rest (the session `gmin`, 1.0) in the
    /// last solve of a sequence.
    pub gmin: f64,
    pub source_scale: f64,
}

/// What the solver holds for one held input (abi.md §4.2, §4.3).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Held {
    /// `VALUE`: for a `LIMIT`, the iterate `x̂` held for this iteration.
    pub value: f64,
    /// For a `LIMIT`, the `PROPOSED` value `x` of this iteration.
    pub proposed: f64,
    /// `LAST`: the value held.
    pub last: f64,
    /// `AGE`: 0 none held, 1 from the last accepted solution, 2 from the
    /// previous iteration.
    pub age: u8,
}

/// The last call of one function.
#[derive(Debug)]
pub(crate) struct FnState {
    /// The bytes of every read cell at the last call, concatenated.
    read: Vec<u8>,
    /// Whether `out` and `jac` hold a call's results.
    valid: bool,
    /// One buffer per written cell.
    pub out: Vec<Vec<u8>>,
    /// One entry per declared partial.
    pub jac: Vec<f64>,
}

impl FnState {
    /// The `F64` at byte 0 of write `w`.
    pub fn f64(&self, w: usize) -> f64 {
        f64::from_le_bytes(self.out[w][..8].try_into().expect("an F64 cell"))
    }
}

/// The mutable side of one instance.
#[derive(Debug)]
pub(crate) struct InstanceState {
    pub fns: Vec<FnState>,
    pub held: Vec<Held>,
    /// Per term; used by `DDT` terms only.
    pub ddt: Vec<History>,
    /// Per term; used by `DELAY` terms only.
    pub delay: Vec<Delayed>,
}

/// What a rejected step must undo (abi.md §5 I8): every term history and
/// every limit's held state.
#[derive(Clone, Debug, PartialEq)]
pub struct AssemblySnapshot {
    ddt: Vec<Vec<History>>,
    held: Vec<Vec<Held>>,
    delay: Vec<Vec<Delayed>>,
}

/// The device assembly of one circuit: its plan, the discrete slots its
/// functions read, and every function's last call.
#[derive(Debug)]
pub struct Assembly {
    pub(crate) plan: AnalogPlan,
    slots: Vec<Vec<u8>>,
    previous: Vec<Vec<u8>>,
    pub(crate) state: Vec<InstanceState>,
    scratch: Vec<u8>,
}

impl Assembly {
    /// An assembly over `plan`: every slot all zero (P3), no function called.
    pub fn new(plan: AnalogPlan) -> Assembly {
        let slots: Vec<Vec<u8>> = plan.slots.iter().map(|n| vec![0; *n]).collect();
        let state = plan
            .instances
            .iter()
            .map(|inst| InstanceState {
                fns: inst
                    .functions
                    .iter()
                    .map(|f| FnState {
                        read: Vec::new(),
                        valid: false,
                        out: f.write_lens.iter().map(|n| vec![0; *n]).collect(),
                        jac: vec![0.0; f.seed.len()],
                    })
                    .collect(),
                held: vec![Held::default(); inst.inputs.len()],
                ddt: vec![History::default(); inst.terms.len()],
                delay: vec![Delayed::default(); inst.terms.len()],
            })
            .collect();
        Assembly { previous: slots.clone(), slots, plan, state, scratch: Vec::new() }
    }

    /// The plan this assembly runs.
    pub fn plan(&self) -> &AnalogPlan {
        &self.plan
    }

    /// Writes discrete slot `slot` (a boundary event). When the bytes differ,
    /// the old ones become its `PREVIOUS` and every function that reads it
    /// runs again in the next solve. Panics when `bytes` is not the slot's size.
    pub fn set_discrete(&mut self, slot: u32, bytes: &[u8]) {
        let s = slot as usize;
        assert_eq!(bytes.len(), self.slots[s].len(), "discrete slot {slot} holds {} bytes", self.slots[s].len());
        if self.slots[s] != bytes {
            std::mem::swap(&mut self.slots[s], &mut self.previous[s]);
            self.slots[s].copy_from_slice(bytes);
        }
    }

    /// Evaluates, at iterate `x`, every analog function a Newton solve needs
    /// (those writing terms or limits) whose read bytes changed since its
    /// last call: the functions writing limits first, then, with each
    /// limit's `x̂` taken from them, the rest. Then checks every value and
    /// seeded partial the solve reads.
    pub fn evaluate(&mut self, x: &[f64], amb: Ambient) -> Result<(), Error> {
        for (inst, st) in self.plan.instances.iter().zip(self.state.iter_mut()) {
            for pass in [true, false] {
                for (fi, f) in inst.functions.iter().enumerate() {
                    if f.rows && f.limits == pass {
                        let ctx = Reads { x, amb, slots: &self.slots, previous: &self.previous, held: &st.held };
                        run(inst, f, &mut st.fns[fi], &ctx, &mut self.scratch)?;
                    }
                }
                if pass {
                    take_iterates(inst, st);
                }
            }
            check_finite(inst, st)?;
        }
        Ok(())
    }

    /// The limits that veto convergence of the iteration just assembled:
    /// those with `|x̂ − x| > 1e-6 + 1e-4·|x|`.
    pub fn limiting_active(&self) -> Vec<LimitState> {
        let mut out = Vec::new();
        for (inst, st) in self.plan.instances.iter().zip(&self.state) {
            vetoes(inst, st, &mut out);
        }
        out
    }

    /// Ends a Newton iteration: every written limit's `LAST` becomes the `x̂`
    /// of the iteration just assembled, with `AGE = 2`.
    // SPEC_DEVIATION: design.md gives `end_iteration(&mut self, x)`; it takes
    // no solution.
    // Reason: `x̂` is the `LIMITED` written in the iteration's own evaluation,
    // which already ran at the iterate; the next solution is not read.
    pub fn end_iteration(&mut self) {
        for (inst, st) in self.plan.instances.iter().zip(self.state.iter_mut()) {
            advance(inst, st, 2);
        }
    }

    /// Accepts the solution `x` at `amb.time`: evaluates there, every written
    /// limit's `LAST` becomes its `x̂` with `AGE = 1`, and every `DELAY`
    /// term's history gains its value.
    pub fn accept(&mut self, x: &[f64], amb: Ambient) -> Result<(), Error> {
        self.evaluate(x, amb)?;
        for (inst, st) in self.plan.instances.iter().zip(self.state.iter_mut()) {
            advance(inst, st, 1);
            for (ti, t) in inst.terms.iter().enumerate() {
                if t.op == op::DELAY {
                    let v = linear(t, inst, st).v;
                    st.delay[ti].push(amb.time, v, max_of(t, &self.slots));
                }
            }
        }
        Ok(())
    }
}

impl Assembly {
    /// The stamps of an equilibrium solve at iterate `x`: every `FLOW` and
    /// `DELAY` term in its Norton form; a `DDT` term adds nothing (its
    /// positions carry 0).
    pub fn assemble_dc(&mut self, x: &[f64], amb: Ambient) -> Result<Vec<Stamp<AnalogReference, f64>>, Error> {
        self.assemble(x, amb, None)
    }

    /// The stamps of transient stage `stage` at iterate `x`: `FLOW` terms in
    /// their Norton form, `DDT` terms through the TR-BDF2 companion over
    /// their own history, `DELAY` terms at `amb.time − td` of theirs.
    pub fn assemble_tran(
        &mut self,
        x: &[f64],
        amb: Ambient,
        stage: Stage,
    ) -> Result<Vec<Stamp<AnalogReference, f64>>, Error> {
        self.assemble(x, amb, Some(stage))
    }

    fn assemble(
        &mut self,
        x: &[f64],
        amb: Ambient,
        stage: Option<Stage>,
    ) -> Result<Vec<Stamp<AnalogReference, f64>>, Error> {
        self.evaluate(x, amb)?;
        let mut out = Vec::new();
        for (inst, st) in self.plan.instances.iter().zip(&self.state) {
            for (ti, t) in inst.terms.iter().enumerate() {
                let lin = linear(t, inst, st);
                match (t.op, stage) {
                    (op::DDT, Some(stage)) => stamp_ddt(t, &lin, &st.ddt[ti], stage, x, &mut out),
                    (op::DDT, None) => stamp_rows(t, &lin, 0.0, 0.0, &mut out),
                    (op::DELAY, Some(_)) => {
                        let td = t.td.map_or(0.0, |(f, w)| st.fns[f].f64(w));
                        let at = (amb.time, td, max_of(t, &self.slots));
                        stamp_delay(&inst.path, t, &lin, &st.delay[ti], at, x, &mut out)?;
                    }
                    _ => stamp_flow(t, &lin, x, &mut out),
                }
            }
        }
        Ok(out)
    }

    /// Records, for every `DDT` term, its value at the converged solution `x`
    /// as the newest history row. Called once per converged solve, where the
    /// Newton state buffer gains its row (a stage, an operating point, an
    /// initial-condition row), so the two stay aligned.
    // SPEC_DEVIATION: design.md gives `push_stage(&mut self)`; it takes the
    // solution and its ambient.
    // Reason: the buffer row the companion reads is the converged `x`, which
    // no assembly evaluated (the last assemble ran at the iterate before it).
    pub fn push_stage(&mut self, x: &[f64], amb: Ambient) -> Result<(), Error> {
        self.evaluate(x, amb)?;
        for (inst, st) in self.plan.instances.iter().zip(self.state.iter_mut()) {
            for (ti, t) in inst.terms.iter().enumerate() {
                if t.op == op::DDT {
                    let q = linear(t, inst, st).v;
                    st.ddt[ti].push(q);
                }
            }
        }
        Ok(())
    }

    /// The term histories and limit states, to restore when a step is rejected.
    pub fn snapshot(&self) -> AssemblySnapshot {
        AssemblySnapshot {
            ddt: self.state.iter().map(|s| s.ddt.clone()).collect(),
            held: self.state.iter().map(|s| s.held.clone()).collect(),
            delay: self.state.iter().map(|s| s.delay.clone()).collect(),
        }
    }

    /// Returns every term history and limit state to `snap`.
    pub fn restore(&mut self, snap: &AssemblySnapshot) {
        for (i, st) in self.state.iter_mut().enumerate() {
            st.ddt.clone_from(&snap.ddt[i]);
            st.held.clone_from(&snap.held[i]);
            st.delay.clone_from(&snap.delay[i]);
        }
    }
}

/// The bound of a `DELAY` term: the `F64` at byte 0 of its `max` slot, or
/// unbounded.
fn max_of(t: &TermPlan, slots: &[Vec<u8>]) -> f64 {
    t.max.map_or(f64::INFINITY, |s| f64::from_le_bytes(slots[s][..8].try_into().expect("an F64 at byte 0")))
}

/// Everything a read cell's bytes come from.
pub(crate) struct Reads<'a> {
    pub x: &'a [f64],
    pub amb: Ambient,
    pub slots: &'a [Vec<u8>],
    pub previous: &'a [Vec<u8>],
    pub held: &'a [Held],
}

impl Reads<'_> {
    /// Appends the bytes of one read cell of `len` bytes to `buf`.
    fn push(&self, source: Source, len: usize, buf: &mut Vec<u8>) {
        let f = |v: f64| v.to_le_bytes();
        match source {
            Source::Unknown(i) => buf.extend_from_slice(&f(self.x[i])),
            Source::Zero => buf.resize(buf.len() + len, 0),
            Source::Slot(s) => buf.extend_from_slice(&self.slots[s]),
            Source::Previous(s) => buf.extend_from_slice(&self.previous[s]),
            Source::Held(i) => buf.extend_from_slice(&f(self.held[i].value)),
            Source::Last(i) => buf.extend_from_slice(&f(self.held[i].last)),
            Source::Age(i) => buf.push(self.held[i].age),
            Source::Ambient(ambient::STATIC) => buf.push(u8::from(self.amb.is_static)),
            Source::Ambient(k) => {
                let v = match k {
                    ambient::TIME => self.amb.time,
                    ambient::FREQ => self.amb.freq,
                    ambient::GMIN => self.amb.gmin,
                    _ => self.amb.source_scale,
                };
                buf.extend_from_slice(&f(v));
            }
        }
    }
}

/// Calls `f` unless its read bytes are those of its last call (F3).
pub(crate) fn run(
    inst: &InstancePlan,
    f: &FnPlan,
    st: &mut FnState,
    ctx: &Reads<'_>,
    scratch: &mut Vec<u8>,
) -> Result<(), Error> {
    scratch.clear();
    for (source, len) in &f.reads {
        ctx.push(*source, *len, scratch);
    }
    if st.valid && st.read == *scratch {
        return Ok(());
    }
    let mut at = 0;
    let input: Vec<&[u8]> = f
        .reads
        .iter()
        .map(|(_, len)| {
            let cell = &scratch[at..at + len];
            at += len;
            cell
        })
        .collect();
    st.jac.fill(0.0);
    let mut output: Vec<&mut [u8]> = st.out.iter_mut().map(Vec::as_mut_slice).collect();
    st.valid = false;
    inst.handle.eval(f.k, &input, &mut output, &f.seed, &mut st.jac).map_err(|e| {
        device_error(format!("instance `{}`: function `{}` ({}): {e}", inst.path, f.name, f.origin))
    })?;
    st.read.clear();
    st.read.extend_from_slice(scratch);
    st.valid = true;
    Ok(())
}

/// A4: every term value, limit iterate and seeded partial a Newton solve
/// reads is finite.
fn check_finite(inst: &InstancePlan, st: &InstanceState) -> Result<(), Error> {
    let fail = |cell: String, origin: &str, v: f64| {
        Err(device_error(format!(
            "instance `{}`: {cell} ({origin}) is not finite: {v} (abi.md A4; LRM §7.3.2.1: an analog \
             expression shall not evaluate to NaN or infinity)",
            inst.path
        )))
    };
    for t in &inst.terms {
        let Some((fi, w)) = t.value else { continue };
        let fs = &st.fns[fi];
        let v = fs.f64(w);
        if !v.is_finite() {
            return fail(format!("term `{}`", t.name), &t.origin, v);
        }
        if let Some((tf, tw)) = t.td {
            let td = st.fns[tf].f64(tw);
            if !td.is_finite() {
                return fail(format!("term `{}`.TIME", t.name), &t.origin, td);
            }
        }
        for p in t.direct.iter().map(|(p, _)| p).chain(t.via.iter().map(|(p, _)| p)) {
            if !fs.jac[*p].is_finite() {
                let f = &inst.functions[fi];
                return fail(format!("partial {p} of term `{}` in function `{}`", t.name, f.name), &t.origin, fs.jac[*p]);
            }
        }
    }
    for x in &inst.inputs {
        for (what, at) in [("PROPOSED", x.proposed), ("LIMITED", x.limited)] {
            let Some((fi, w)) = at else { continue };
            let v = st.fns[fi].f64(w);
            if !v.is_finite() {
                return fail(format!("input `{}`.{what}", x.name), &x.origin, v);
            }
        }
        if let Some((fi, _)) = x.proposed {
            for (p, _) in &x.p {
                let v = st.fns[fi].jac[*p];
                if !v.is_finite() {
                    return fail(format!("partial {p} of input `{}`.PROPOSED", x.name), &x.origin, v);
                }
            }
        }
    }
    Ok(())
}
