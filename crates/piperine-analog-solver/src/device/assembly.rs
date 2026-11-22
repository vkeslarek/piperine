//! One solve's evaluation (design `device::assembly`, abi.md F3, A4): the
//! assembly keeps, per analog function, the bytes it last read and the
//! outputs and partials it returned. A function whose read bytes are
//! unchanged is not called and its outputs are reused (F3). The discrete pins
//! analog functions read are the assembly's own byte slots, written by
//! `set_discrete` (abi.md §10). A non-finite value or seeded partial that the
//! solve reads fails it, naming instance, cell and origin (A4).

use piperine_abi::raw::{ambient, op, watch};

use super::ddt::{History, Stage, companion_current, stamp_ddt};
use super::delay::{Delayed, stamp_delay};
use super::device_error;
use super::flow::{stamp_flow, stamp_rows};
use super::limit::{LimitState, advance, linear, take_iterates, vetoes};
use super::watch::{Accepted, TraceRecord, Watched, accept_watches, record_traces, tolerance};
use super::small_signal::{
    ComplexStamps, NoiseColumn, SmallSignal, StimulusColumn, TermLinearization, response, stamp_matrix, stimulus,
};
use super::plan::{AnalogPlan, FnPlan, InstancePlan, Source, TermPlan};
use crate::error::{Error, RunCode, SolverDomain};
use crate::math::integration::DdtState;
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
    /// Per watch.
    pub watched: Vec<Watched>,
}

/// A `CROSS` watch that went from `≤ 0` at the last accepted solution to `> 0` at a solution
/// not yet accepted (`Assembly::rising`).
#[derive(Clone, Debug, PartialEq)]
pub struct Rising {
    /// The instance, in bind order, and the watch, in its shape's order.
    pub instance: usize,
    pub watch: usize,
    /// Its value at the last accepted solution, and at the solution.
    pub before: f64,
    pub now: f64,
    /// Its `ttol` and `etol` pins' values; `None`: `DEFAULT`.
    pub ttol: Option<f64>,
    pub etol: Option<f64>,
}

/// What a rejected step must undo (abi.md §5 I8): every term history and
/// every limit's held state.
#[derive(Clone, Debug, PartialEq)]
pub struct AssemblySnapshot {
    ddt: Vec<Vec<History>>,
    held: Vec<Vec<Held>>,
    delay: Vec<Vec<Delayed>>,
    watched: Vec<Vec<Watched>>,
}

/// The device assembly of one circuit: its plan, the discrete slots its
/// functions read, and every function's last call.
#[derive(Debug)]
pub struct Assembly {
    pub(crate) plan: AnalogPlan,
    slots: Vec<Vec<u8>>,
    /// Whether the discrete side has written each slot since the assembly was made.
    written: Vec<bool>,
    previous: Vec<Vec<u8>>,
    pub(crate) state: Vec<InstanceState>,
    scratch: Vec<u8>,
    /// `TRACE` records not yet drained; never part of a snapshot (A5).
    traces: Vec<TraceRecord>,
    /// Newton iterations since the last recorded converged solve.
    iteration: u64,
    /// The transient stage of the last assembly (`None` after an equilibrium one): the stage the
    /// next converged solve's companion currents belong to.
    stage: Option<Stage>,
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
                watched: vec![Watched::default(); inst.watches.len()],
            })
            .collect();
        Assembly { written: vec![false; slots.len()], previous: slots.clone(), slots, plan, state, scratch: Vec::new(), traces: Vec::new(), iteration: 0, stage: None }
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
        self.written[s] = true;
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
        self.evaluate_for(x, amb, Need::NEWTON)
    }

    /// [`evaluate`](Self::evaluate) for the outputs `need` names.
    pub(crate) fn evaluate_for(&mut self, x: &[f64], amb: Ambient, need: Need) -> Result<(), Error> {
        for (inst, st) in self.plan.instances.iter().zip(self.state.iter_mut()) {
            for pass in [true, false] {
                for (fi, f) in inst.functions.iter().enumerate() {
                    let needed =
                        f.rows || (need.stim && f.stim) || (need.noise && f.noise) || (need.watch && f.watch);
                    if needed && f.limits == pass {
                        let ctx = Reads { x, amb, slots: &self.slots, previous: &self.previous, held: &st.held };
                        if run(inst, f, &mut st.fns[fi], &ctx, &mut self.scratch)? {
                            record_traces(inst, f, &st.fns[fi], self.iteration, &mut self.traces);
                        }
                    }
                }
                if pass {
                    take_iterates(inst, st, need.small_signal);
                }
            }
            check_finite(inst, st, need)?;
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
        self.iteration += 1;
        for (inst, st) in self.plan.instances.iter().zip(self.state.iter_mut()) {
            advance(inst, st, 2);
        }
    }

    /// Accepts the solution `x` at `amb.time`: evaluates there, with the
    /// functions writing `VAR`, `CROSS` and `CAP` watches; every written
    /// limit's `LAST` becomes its `x̂` with `AGE = 1`; every `DELAY` term's
    /// history gains its value; the watches report (see `watch`).
    pub fn accept(&mut self, x: &[f64], amb: Ambient) -> Result<Accepted, Error> {
        self.evaluate_for(x, amb, Need::ACCEPT)?;
        self.iteration = 0;
        let mut acc = Accepted { fired: Vec::new(), cap: f64::INFINITY };
        for (index, (inst, st)) in self.plan.instances.iter().zip(self.state.iter_mut()).enumerate() {
            accept_watches(index, inst, st, &self.slots, &self.written, &mut acc)?;
            advance(inst, st, 1);
            for (ti, t) in inst.terms.iter().enumerate() {
                if t.op == op::DELAY {
                    let v = linear(t, inst, st).v;
                    st.delay[ti].push(amb.time, v, max_of(t, &self.slots));
                }
            }
        }
        Ok(acc)
    }

    /// Whether a `CROSS` watch is written by an analog function (a transient locates it).
    pub fn has_crossings(&self) -> bool {
        self.plan.instances.iter().any(|inst| inst.watches.iter().any(|w| w.kind == watch::CROSS && w.value.is_some()))
    }

    /// The `CROSS` watches that went from `≤ 0` at the last accepted solution to `> 0` at `x`,
    /// evaluated as `accept` evaluates them, with nothing accepted: no watch, limit or history
    /// moves (docs/spec §17.3: the locator reads a step before it is accepted). A tolerance pin
    /// that is not finite and positive is `E0510`.
    pub fn rising(&mut self, x: &[f64], amb: Ambient) -> Result<Vec<Rising>, Error> {
        self.evaluate_for(x, amb, Need::ACCEPT)?;
        let mut out = Vec::new();
        for (index, (inst, st)) in self.plan.instances.iter().zip(&self.state).enumerate() {
            for (wi, w) in inst.watches.iter().enumerate() {
                let Some((fi, slot)) = w.value.filter(|_| w.kind == watch::CROSS) else { continue };
                let now = st.fns[fi].f64(slot);
                let Some(before) = st.watched[wi].cross else { continue };
                if before <= 0.0 && now > 0.0 {
                    out.push(Rising {
                        instance: index,
                        watch: wi,
                        before,
                        now,
                        ttol: tolerance(inst, w, "ttol", w.ttol, &self.slots, &self.written)?,
                        etol: tolerance(inst, w, "etol", w.etol, &self.slots, &self.written)?,
                    });
                }
            }
        }
        Ok(out)
    }

    /// The bytes of `VAR` watch `name` of instance `path` at the last
    /// accepted solution.
    pub fn var(&self, path: &str, name: &str) -> Option<&[u8]> {
        let i = self.plan.instances.iter().position(|inst| inst.path == path)?;
        let w = self.plan.instances[i].watches.iter().position(|w| w.name == name)?;
        self.state[i].watched[w].var.as_deref()
    }

    /// The names of the terms and of the `VAR` watches of instance `path`, in
    /// declaration order; `None` for an unknown instance.
    pub fn observable_names(&self, path: &str) -> Option<(Vec<&str>, Vec<&str>)> {
        let inst = self.plan.instances.iter().find(|inst| inst.path == path)?;
        let terms = inst.terms.iter().map(|t| t.name.as_str()).collect();
        let vars = inst.watches.iter().filter(|w| w.kind == watch::VAR).map(|w| w.name.as_str()).collect();
        Some((terms, vars))
    }

    /// The value of term `name` of instance `path` at its last evaluation;
    /// `None` for an unknown name, a term with no value or one not yet evaluated.
    pub fn term_value(&self, path: &str, name: &str) -> Option<f64> {
        let i = self.plan.instances.iter().position(|inst| inst.path == path)?;
        let (f, w) = self.plan.instances[i].terms.iter().find(|t| t.name == name)?.value?;
        let state = &self.state[i].fns[f];
        state.valid.then(|| state.f64(w))
    }

    /// Every `TRACE` record written since the last drain, in order.
    pub fn drain_traces(&mut self) -> Vec<TraceRecord> {
        std::mem::take(&mut self.traces)
    }

    /// The values at `x` of the continuous pins the digital side's reactions
    /// read, in instance, reaction and read order (0 for ground).
    pub fn samples(&self, x: &[f64]) -> Vec<f64> {
        self.plan.samples.iter().map(|s| s.map_or(0.0, |i| x[i])).collect()
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
        self.stage = stage;
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

    /// The small-signal system at operating point `op` and sweep frequency
    /// `amb.freq`: the matrix `Σ resp(ω)·∂v` and the stimulus right side
    /// `−Σ w·g·(RE + j·IM)`. Every limit holds `x̂ = x`.
    // SPEC_DEVIATION: design.md gives `assemble_ac(op, omega)`; it takes the
    // ambient, whose `freq` gives `ω = 2π·freq`.
    // Reason: functions read `FREQ` and the continuation values as ambient
    // inputs (abi.md §4.2); the caller supplies all of them.
    pub fn assemble_ac(&mut self, op: &[f64], amb: Ambient) -> Result<(ComplexStamps, ComplexStamps), Error> {
        self.evaluate_for(op, amb, Need::AC)?;
        let omega = 2.0 * std::f64::consts::PI * amb.freq;
        let (mut matrix, mut rhs) = (Vec::new(), Vec::new());
        for (inst, st) in self.plan.instances.iter().zip(&self.state) {
            for t in &inst.terms {
                let w = response(t, st, omega);
                stamp_matrix(t, &linear(t, inst, st), w, &mut matrix);
                stimulus(t, inst, st, w, &mut rhs);
            }
        }
        Ok((matrix, rhs))
    }

    /// The noise sources at operating point `op` and sweep frequency
    /// `amb.freq`: one column per `NOISE` input, through every term that
    /// reads it, with its density `PSD` (abi.md §10 "Noise"). Densities are
    /// evaluated here only (A4).
    // SPEC_DEVIATION: design.md gives `noise_sources(op, freq)`; it takes the
    // ambient, as `assemble_ac` does.
    // Reason: `FREQ` and the continuation values are ambient inputs.
    pub fn noise_sources(&mut self, op: &[f64], amb: Ambient) -> Result<Vec<NoiseColumn>, Error> {
        self.evaluate_for(op, amb, Need::NOISE)?;
        let omega = 2.0 * std::f64::consts::PI * amb.freq;
        let mut out = Vec::new();
        for (inst, st) in self.plan.instances.iter().zip(&self.state) {
            SmallSignal::new(inst, st, omega).noise_columns(&mut out);
        }
        Ok(out)
    }

    /// The small-signal stimuli of instance `instance` at `operating_point` and sweep
    /// frequency `ambient.freq`: one column per `STIM` input, through every term that reads it, with
    /// the phase of its phasor. The input-referred noise reads the transfer from them (design
    /// D11). An `instance` that is not in the plan is an error naming it.
    pub fn stimulus_columns(&mut self, operating_point: &[f64], ambient: Ambient, instance: &str) -> Result<Vec<StimulusColumn>, Error> {
        let index = self.instance_index(instance)?;
        self.evaluate_for(operating_point, ambient, Need::AC)?;
        let omega = 2.0 * std::f64::consts::PI * ambient.freq;
        let mut out = Vec::new();
        SmallSignal::new(&self.plan.instances[index], &self.state[index], omega).stimulus_columns(&mut out);
        Ok(out)
    }

    /// Term `term` of instance `instance` linearized at `operating_point` and sweep frequency
    /// `ambient.freq`: its partials against the unknowns and against each `NOISE` input, the
    /// operator's response applied. An unknown instance or term is an error naming it.
    pub fn term_linearization(
        &mut self,
        operating_point: &[f64],
        ambient: Ambient,
        instance: &str,
        term: &str,
    ) -> Result<TermLinearization, Error> {
        let index = self.instance_index(instance)?;
        let Some(term_index) = self.plan.instances[index].terms.iter().position(|t| t.name == term) else {
            return Err(device_error(format!("instance `{instance}` has no term `{term}`")));
        };
        self.evaluate_for(operating_point, ambient, Need { small_signal: true, ..Need::NEWTON })?;
        let omega = 2.0 * std::f64::consts::PI * ambient.freq;
        let instance_plan = &self.plan.instances[index];
        Ok(SmallSignal::new(instance_plan, &self.state[index], omega).term_linearization(&instance_plan.terms[term_index]))
    }

    /// The position of the instance at `path` in the plan.
    fn instance_index(&self, path: &str) -> Result<usize, Error> {
        self.plan
            .instances
            .iter()
            .position(|inst| inst.path == path)
            .ok_or_else(|| device_error(format!("no instance `{path}` in the plan")))
    }

    /// Records, for every `DDT` term, its value at the converged solution `x`
    /// as the newest history row: every limit holds `x̂ = x` for it, as in a
    /// small-signal solve, so the row is the term's argument at the solution
    /// and not its limit linearization `q(x̂) + g·(x − x̂)`, which for a
    /// nonlinear `q` and a held `x̂` far from `x` is not `q(x)` at all. Called once per converged solve, where the
    /// Newton state buffer gains its row (a stage, an operating point, an
    /// initial-condition row), so the two stay aligned.
    // SPEC_DEVIATION: design.md gives `push_stage(&mut self)`; it takes the
    // solution and its ambient.
    // Reason: the buffer row the companion reads is the converged `x`, which
    // no assembly evaluated (the last assemble ran at the iterate before it).
    pub fn push_stage(&mut self, x: &[f64], amb: Ambient) -> Result<(), Error> {
        self.evaluate_for(x, amb, Need { small_signal: true, ..Need::NEWTON })?;
        self.iteration = 0;
        let stage = self.stage.take();
        for (inst, st) in self.plan.instances.iter().zip(self.state.iter_mut()) {
            for (ti, t) in inst.terms.iter().enumerate() {
                if t.op == op::DDT {
                    let q = linear(t, inst, st).v;
                    let i = stage.map_or(0.0, |stage| companion_current(stage, q, &st.ddt[ti]));
                    st.ddt[ti].push(q, i);
                }
            }
        }
        Ok(())
    }

    /// The unknowns each `DDT` term of `ddt_states` contributes to, in the same order.
    pub fn ddt_unknowns(&self) -> Vec<Vec<usize>> {
        let mut out = Vec::new();
        for inst in &self.plan.instances {
            for t in inst.terms.iter().filter(|t| t.op == op::DDT) {
                out.push([&t.plus, &t.minus].into_iter().flatten().filter_map(|r| r.idx()).collect());
            }
        }
        out
    }

    /// The `DDT` states of the step the last two stages completed: per term, its value at `t_n`,
    /// `t_{n+γ}` and `t_{n+1}` (the three newest history rows), its companion current at `t_n`
    /// and `t_{n+1}`, and its own `abstol` when the instance gave a finite positive one. What the
    /// truncation test measures (§15.2 rule 1, §17.2 Resolution); a row no stage produced reads
    /// 0 for its current.
    pub fn ddt_states(&self) -> Vec<DdtState> {
        let mut out = Vec::new();
        for (inst, st) in self.plan.instances.iter().zip(&self.state) {
            for (ti, t) in inst.terms.iter().enumerate().filter(|(_, t)| t.op == op::DDT) {
                let h = &st.ddt[ti];
                let abstol = t.abstol.map(|s| f64::from_le_bytes(self.slots[s][..8].try_into().expect("an F64 at byte 0")));
                out.push(DdtState {
                    q: [h.back(3), h.back(2), h.back(1)],
                    i: [h.current_back(3), h.current_back(1)],
                    abstol: abstol.filter(|a| a.is_finite() && *a > 0.0),
                });
            }
        }
        out
    }

    /// `∂F/∂param` of one instance at the equilibrium iterate `x` (abi.md §10
    /// "Host procedures"): the residual-shaped right-side stamps
    /// `Rhs(plus, Σ g·d)` and `Rhs(minus, −Σ g·d)` of every `FLOW` term whose
    /// value has a partial `g` against a discrete pin of `wrt`. `wrt` lists
    /// each such pin with `d`, the derivative of its bytes with respect to the
    /// param (1 for the param's own pin, the chained product through discrete
    /// functions for a derived pin). Partials against a param are seeded only
    /// here (never in a Newton solve); a `DDT` or `DELAY` term adds nothing at
    /// an equilibrium. A term with no declared path yields no stamp.
    pub fn param_stamps(
        &mut self,
        instance: usize,
        wrt: &[(u32, f64)],
        x: &[f64],
        amb: Ambient,
    ) -> Result<Vec<Stamp<AnalogReference, f64>>, Error> {
        let inst = &self.plan.instances[instance];
        let st = &self.state[instance];
        let ctx = Reads { x, amb, slots: &self.slots, previous: &self.previous, held: &st.held };
        let shape = inst.handle.shape();
        let mut d = vec![0.0; inst.terms.len()];
        let mut scratch: Vec<u8> = Vec::new();
        for f in inst.functions.iter().filter(|f| f.rows && !f.limits) {
            let sf = shape.functions.get(f.k as usize).expect("validated");
            let pin_d = |c: &piperine_abi::raw::Cell| {
                let discrete = shape
                    .pins
                    .get(c.index as usize)
                    .is_some_and(|p| p.kind == piperine_abi::raw::kind::DISCRETE);
                (c.kind == piperine_abi::raw::cell::PIN && discrete)
                    .then(|| wrt.iter().find(|(q, _)| *q == c.index))
                    .flatten()
            };
            let seed: Vec<u8> = sf
                .partials
                .iter()
                .map(|p| u8::from(sf.reads.get(p.read as usize).is_some_and(|c| pin_d(c).is_some())))
                .collect();
            if seed.iter().all(|s| *s == 0) {
                continue;
            }
            scratch.clear();
            for (source, len) in &f.reads {
                ctx.push(*source, *len, &mut scratch);
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
            let mut out: Vec<Vec<u8>> = f.write_lens.iter().map(|n| vec![0; *n]).collect();
            let mut output: Vec<&mut [u8]> = out.iter_mut().map(Vec::as_mut_slice).collect();
            let mut jac = vec![0.0; seed.len()];
            inst.handle.eval(f.k, &input, &mut output, &seed, &mut jac).map_err(|e| {
                device_error(format!("instance `{}`: function `{}` ({}): {e}", inst.path, f.name, f.origin))
            })?;
            for (pi, p) in sf.partials.iter().enumerate() {
                let (Some(r), Some(w)) = (sf.reads.get(p.read as usize), sf.writes.get(p.write as usize)) else { continue };
                let Some((_, dp)) = pin_d(r) else { continue };
                if w.kind == piperine_abi::raw::cell::TERM && w.part == piperine_abi::raw::part::VALUE {
                    d[w.index as usize] += jac[pi] * dp;
                }
            }
        }
        let mut out = Vec::new();
        for (t, g) in inst.terms.iter().zip(d) {
            if g == 0.0 || t.op != op::FLOW {
                continue;
            }
            if let Some(p) = &t.plus {
                out.push(Stamp::Rhs(p.clone(), g));
            }
            if let Some(m) = &t.minus {
                out.push(Stamp::Rhs(m.clone(), -g));
            }
        }
        Ok(out)
    }

    /// The term histories and limit states, to restore when a step is rejected.
    pub fn snapshot(&self) -> AssemblySnapshot {
        AssemblySnapshot {
            ddt: self.state.iter().map(|s| s.ddt.clone()).collect(),
            held: self.state.iter().map(|s| s.held.clone()).collect(),
            delay: self.state.iter().map(|s| s.delay.clone()).collect(),
            watched: self.state.iter().map(|s| s.watched.clone()).collect(),
        }
    }

    /// Returns every term history and limit state to `snap`.
    pub fn restore(&mut self, snap: &AssemblySnapshot) {
        for (i, st) in self.state.iter_mut().enumerate() {
            st.ddt.clone_from(&snap.ddt[i]);
            st.held.clone_from(&snap.held[i]);
            st.delay.clone_from(&snap.delay[i]);
            st.watched.clone_from(&snap.watched[i]);
        }
    }
}

/// The bound of a `DELAY` term: the `F64` at byte 0 of its `max` slot, or
/// unbounded.
fn max_of(t: &TermPlan, slots: &[Vec<u8>]) -> f64 {
    t.max.map_or(f64::INFINITY, |s| f64::from_le_bytes(slots[s][..8].try_into().expect("an F64 at byte 0")))
}

/// Which outputs one evaluation needs beyond the rows of a Newton solve.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Need {
    /// `STIM` strengths (a small-signal sweep).
    pub stim: bool,
    /// `NOISE` densities (a noise sweep).
    pub noise: bool,
    /// `VAR`, `CROSS` and `CAP` watches (an accepted solution).
    pub watch: bool,
    /// A small-signal linearization: every limit holds `x̂ = x`.
    pub small_signal: bool,
}

impl Need {
    pub const NEWTON: Need = Need { stim: false, noise: false, watch: false, small_signal: false };
    pub const AC: Need = Need { stim: true, noise: false, watch: false, small_signal: true };
    pub const NOISE: Need = Need { stim: false, noise: true, watch: false, small_signal: true };
    pub const ACCEPT: Need = Need { stim: false, noise: false, watch: true, small_signal: false };
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

/// Calls `f` unless its read bytes are those of its last call (F3);
/// whether it was called. Its `TRACE` buffers start each call empty (A5).
pub(crate) fn run(
    inst: &InstancePlan,
    f: &FnPlan,
    st: &mut FnState,
    ctx: &Reads<'_>,
    scratch: &mut Vec<u8>,
) -> Result<bool, Error> {
    scratch.clear();
    for (source, len) in &f.reads {
        ctx.push(*source, *len, scratch);
    }
    if st.valid && st.read == *scratch {
        return Ok(false);
    }
    for (slot, _) in &f.traces {
        st.out[*slot].fill(0);
    }
    st.jac.fill(0.0);
    st.valid = false;
    inst.handle.eval_packed(f.k, scratch, &mut st.out, &f.seed, &mut st.jac).map_err(|e| {
        device_error(format!("instance `{}`: function `{}` ({}): {e}", inst.path, f.name, f.origin))
    })?;
    st.read.clear();
    st.read.extend_from_slice(scratch);
    st.valid = true;
    Ok(true)
}

/// A4: every term value, limit iterate and seeded partial a Newton solve
/// reads is finite, and the strengths the sweep `need` names.
fn check_finite(inst: &InstancePlan, st: &InstanceState, need: Need) -> Result<(), Error> {
    let fail = |cell: String, origin: &str, v: f64| {
        Err(Error::run(
            RunCode::E0507,
            SolverDomain::Device,
            format!(
                "instance `{}`: {cell} ({origin}) is not finite: {v} (abi.md A4; §17.1 rule 4: a \
                 contribution or stamp shall not be NaN or infinity)",
                inst.path
            ),
        ))
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
        // Strengths only where the sweep reads them (A4).
        for (what, at, read) in [("RE", x.re, need.stim), ("IM", x.im, need.stim), ("PSD", x.psd, need.noise)] {
            let Some((fi, w)) = at.filter(|_| read) else { continue };
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
