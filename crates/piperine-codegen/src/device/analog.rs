//! The analog side of a device instance: MNA stamping around an
//! [`AnalogKernel`], including the reactive companion model, ideal-source
//! branch rows, runtime operators (`delay`/`slew`), and noise.

use std::collections::VecDeque;
use std::sync::Arc;

use num_complex::Complex64;

use piperine_solver::analog::{AnalogReference, BranchIdentifier, Netlist, NodeIdentifier};
use piperine_solver::analysis::ac::AcAnalysisContext;
use piperine_solver::analysis::dc::{DcAnalysisResult, DcAnalysisState};
use piperine_solver::analysis::noise::Noise;
use piperine_solver::analysis::transient::{TransientAnalysisContext, TransientAnalysisState};
use piperine_solver::math::circular_array::CircularArrayBuffer2;
use piperine_solver::math::linear::Stamp;
use piperine_solver::solver::Context;

use crate::ir::{Analysis, CrossDir};
use crate::jit::analog::{AnalogKernel, CompiledTrigger, RuntimeState};
use crate::jit::{CodegenError, SimCtx};

/// A runtime-serviced analog operator: updated once per accepted timestep,
/// its output read by the kernel through the state array.
enum Operator {
    /// `delay(x, t)` — a `(time, value)` history ring.
    Delay { slot: usize, delay: f64, history: VecDeque<(f64, f64)> },
    /// `slew(x, rise, fall)` — rate-limited follower.
    Slew { slot: usize, rise: f64, fall: f64, output: f64, time: f64 },
    /// `idt`/`idtmod` — implicit-Euler accumulator (`value += dt·x`, wrapped
    /// into `[0, modulus)` when given). The kernel adds the in-step `dt·x`
    /// term itself; `value` is the integral up to the last accepted step.
    Integrate { slot: usize, modulus: Option<f64>, value: f64, time: f64 },
}

impl Operator {
    /// Advance to `time` with the operator input `input`; returns the new
    /// output value.
    fn accept(&mut self, time: f64, input: f64) -> f64 {
        match self {
            Operator::Delay { delay, history, .. } => {
                history.push_back((time, input));
                let target = time - *delay;
                // Drop history strictly older than the target, keeping one
                // sample at or before it for interpolation.
                while history.len() > 1 && history[1].0 <= target {
                    history.pop_front();
                }
                match history.front() {
                    Some(&(t0, v0)) => match history.get(1) {
                        Some(&(t1, v1)) if t1 > t0 && target >= t0 => {
                            let frac = ((target - t0) / (t1 - t0)).clamp(0.0, 1.0);
                            v0 + frac * (v1 - v0)
                        }
                        _ => v0,
                    },
                    None => input,
                }
            }
            Operator::Slew { rise, fall, output, time: last_time, .. } => {
                let dt = (time - *last_time).max(0.0);
                let delta = input - *output;
                let limited = if delta >= 0.0 {
                    delta.min(*rise * dt)
                } else {
                    delta.max(-*fall * dt)
                };
                *output += limited;
                *last_time = time;
                *output
            }
            Operator::Integrate { modulus, value, time: last_time, .. } => {
                let dt = (time - *last_time).max(0.0);
                *value += dt * input;
                if let Some(m) = *modulus {
                    if m > 0.0 {
                        *value -= m * (*value / m).floor();
                    }
                }
                *last_time = time;
                *value
            }
        }
    }

    fn slot(&self) -> usize {
        match self {
            Operator::Delay { slot, .. }
            | Operator::Slew { slot, .. }
            | Operator::Integrate { slot, .. } => *slot,
        }
    }
}

/// Per-event transition detector: remembers the previous accepted trigger
/// value (crossings) or the next fire time (timers).
struct EventDetector {
    /// A trigger value has been observed (crossing detection is armed).
    seeded: bool,
    prev: f64,
    next_fire: f64,
}

impl EventDetector {
    /// Whether the event fires given the trigger value at the accepted
    /// solution, updating the detector state.
    fn fired(&mut self, trigger: &CompiledTrigger, value: f64, time: f64, period: f64) -> bool {
        let fired = match trigger {
            // Fired once at instance creation, never here.
            CompiledTrigger::Initial => false,
            CompiledTrigger::Above => {
                let rose = if self.seeded { self.prev <= 0.0 } else { true };
                rose && value > 0.0
            }
            CompiledTrigger::Cross(dir) => {
                let rising = self.seeded && self.prev <= 0.0 && value > 0.0;
                let falling = self.seeded && self.prev >= 0.0 && value < 0.0;
                match dir {
                    CrossDir::Rising => rising,
                    CrossDir::Falling => falling,
                    CrossDir::Either => rising || falling,
                }
            }
            CompiledTrigger::Timer { .. } => {
                let fires = period > 0.0 && time >= self.next_fire;
                if fires {
                    while self.next_fire <= time {
                        self.next_fire += period;
                    }
                }
                fires
            }
        };
        self.prev = value;
        self.seeded = true;
        fired
    }
}

/// The analog half of a device instance.
pub struct AnalogInstance {
    kernel: Arc<AnalogKernel>,
    /// Per-terminal netlist references (`None` = ground).
    node_refs: Vec<Option<AnalogReference>>,
    /// One MNA branch-current unknown per force row.
    force_refs: Vec<AnalogReference>,
    /// Netlist references for each noise source's terminals (`None` when a
    /// terminal is ground-mapped).
    noise_refs: Vec<(Option<AnalogReference>, Option<AnalogReference>)>,
    params: Vec<f64>,
    sim: SimCtx,
    /// Runtime-state values read by the kernel (`state[StateId]`).
    state: Vec<f64>,
    operators: Vec<Operator>,
    /// One detector per runtime event, in kernel event order. `periods[i]`
    /// is the (parameter-constant) timer period, 0 for non-timers.
    event_detectors: Vec<EventDetector>,
    event_periods: Vec<f64>,
    /// Module-level persistent variable values read by the kernel through
    /// the D2A bridge (`vars[VarId]`). Synced from the digital side after
    /// each `eval_discrete` call.
    vars: Vec<f64>,
    /// Last accepted node voltages (for `bound_step_hint`).
    last_volts: Vec<f64>,
}

impl AnalogInstance {
    /// Wire an instance into the netlist. `terminals` must cover every
    /// kernel terminal (ports first, then internal nodes).
    pub fn new(
        label: &str,
        kernel: Arc<AnalogKernel>,
        terminals: &[NodeIdentifier],
        params: Vec<f64>,
        param_given_mask: u64,
        netlist: &mut Netlist,
    ) -> Result<Self, CodegenError> {
        if terminals.len() != kernel.num_terminals() {
            return Err(CodegenError::Invalid(format!(
                "`{label}` connects {} terminals, kernel `{}` has {}",
                terminals.len(),
                kernel.name(),
                kernel.num_terminals()
            )));
        }
        if params.len() != kernel.num_params() {
            return Err(CodegenError::Invalid(format!(
                "`{label}` provides {} params, kernel `{}` has {}",
                params.len(),
                kernel.name(),
                kernel.num_params()
            )));
        }

        let node_refs: Vec<Option<AnalogReference>> = terminals
            .iter()
            .enumerate()
            .map(|(i, t)| {
                if kernel.is_digital_terminal(i) {
                    // Never an MNA unknown (see `AnalogKernel::digital_terminals`) —
                    // connecting it would leave a structurally empty,
                    // singular row. `volts[i]` reads 0.0 until the D2A
                    // bridge is extended to fill it from live digital
                    // state (tracked separately).
                    return None;
                }
                let reference = netlist.connect_node(t.clone());
                reference.idx().is_some().then_some(reference)
            })
            .collect();

        let force_refs = (0..kernel.num_forces())
            .map(|i| netlist.connect_branch(BranchIdentifier::new(label, format!("force{i}"))))
            .collect();

        // Noise terminals resolve through the kernel terminal order.
        let terminal_slot = |node: crate::ir::NodeId| {
            kernel
                .terminals()
                .iter()
                .position(|&t| t == node)
                .and_then(|i| node_refs[i].clone())
        };
        let module_noise = kernel.noise_terminals();
        let noise_refs = module_noise
            .iter()
            .map(|&(plus, minus)| (terminal_slot(plus), terminal_slot(minus)))
            .collect();

        let operators = kernel
            .runtime_states()
            .iter()
            .map(|spec| {
                let value = |expr: &crate::ir::IrExpr| {
                    expr.eval_const(&|id| params.get(id.0 as usize).copied())
                        .map_err(CodegenError::ConstEval)
                };
                Ok(match &spec.kind {
                    RuntimeState::Delay { delay } => Operator::Delay {
                        slot: spec.id.0 as usize,
                        delay: value(delay)?,
                        history: VecDeque::new(),
                    },
                    RuntimeState::Slew { rise, fall } => Operator::Slew {
                        slot: spec.id.0 as usize,
                        rise: value(rise)?.abs(),
                        fall: value(fall)?.abs(),
                        output: 0.0,
                        time: 0.0,
                    },
                    RuntimeState::Integrator { ic, modulus } => Operator::Integrate {
                        slot: spec.id.0 as usize,
                        modulus: modulus.as_ref().map(&value).transpose()?,
                        value: value(ic)?,
                        time: 0.0,
                    },
                })
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;

        // Integrators start at their initial condition; every other state
        // slot starts at 0.
        let mut state = vec![0.0; kernel.num_state_slots()];
        for op in &operators {
            if let Operator::Integrate { slot, value, .. } = op {
                state[*slot] = *value;
            }
        }

        // Timer periods are parameter-constant, evaluated once.
        let event_periods = kernel
            .events()
            .iter()
            .map(|e| match &e.trigger {
                CompiledTrigger::Timer { period } => period
                    .eval_const(&|id| params.get(id.0 as usize).copied())
                    .map_err(CodegenError::ConstEval),
                _ => Ok(0.0),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let event_detectors = kernel
            .events()
            .iter()
            .zip(&event_periods)
            .map(|(_, &period)| EventDetector { seeded: false, prev: 0.0, next_fire: period })
            .collect();

        let mut sim = SimCtx::default();
        sim.param_given_mask = param_given_mask;
        let n = kernel.num_terminals();
        let num_vars = kernel.num_vars();
        let mut instance = Self {
            state,
            kernel,
            node_refs,
            force_refs,
            noise_refs,
            params,
            sim,
            operators,
            event_detectors,
            event_periods,
            vars: vec![0.0; num_vars],
            last_volts: vec![0.0; n],
        };
        instance.fire_initial_events();
        Ok(instance)
    }

    /// Execute `@ initial` event actions once, at zero volts (before any
    /// solve, only parameters and power-on variable values are visible).
    fn fire_initial_events(&mut self) {
        let fired: Vec<bool> = self
            .kernel
            .events()
            .iter()
            .map(|e| matches!(e.trigger, CompiledTrigger::Initial))
            .collect();
        if fired.iter().any(|&f| f) {
            let volts = vec![0.0; self.num_terminals()];
            self.apply_event_actions(&fired, &volts);
        }
    }

    /// Evaluate all action rows at `volts` and write the fired events'
    /// actions into the vars bank, in body order.
    fn apply_event_actions(&mut self, fired: &[bool], volts: &[f64]) {
        let mut values = vec![0.0; self.kernel.num_event_actions()];
        self.kernel
            .eval_event_actions(volts, &self.params, &self.state, &self.vars, &self.sim, &mut values);
        let mut row = 0;
        for (event, &event_fired) in self.kernel.events().iter().zip(fired) {
            for var in &event.action_vars {
                if event_fired {
                    self.vars[var.0 as usize] = values[row];
                }
                row += 1;
            }
        }
    }

    /// Update the module-level variable bank from the digital side (the D2A
    /// bridge). Called after each `eval_discrete` so the analog body sees
    /// the latest register values. `values[VarId.0]` is the variable's
    /// current value as an `f64`.
    pub fn sync_vars(&mut self, values: &[f64]) {
        for (i, v) in values.iter().enumerate() {
            if i < self.vars.len() {
                self.vars[i] = *v;
            }
        }
    }

    fn num_terminals(&self) -> usize {
        self.node_refs.len()
    }

    /// Gather terminal voltages through a solver lookup.
    fn collect_volts(&self, get_v: &dyn Fn(usize) -> f64) -> Vec<f64> {
        self.node_refs
            .iter()
            .map(|r| {
                r.as_ref()
                    .and_then(AnalogReference::idx)
                    .map(get_v)
                    .unwrap_or(0.0)
            })
            .collect()
    }

    /// Residual + Jacobian at the given voltages.
    fn eval_rhs_jac(&self, volts: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let n = self.num_terminals();
        let mut res = vec![0.0; n];
        let mut jac = vec![0.0; n * n];
        self.kernel
            .eval_residual(volts, &self.params, &self.state, &self.vars, &self.sim, &mut res);
        self.kernel
            .eval_jacobian(volts, &self.params, &self.state, &self.vars, &self.sim, &mut jac);
        (res, jac)
    }

    /// Norton current source: `−res + J·V`.
    fn norton_rhs(&self, volts: &[f64], res: &[f64], jac: &[f64]) -> Vec<f64> {
        let n = self.num_terminals();
        (0..n)
            .map(|i| {
                let coupling: f64 = (0..n).map(|j| jac[i * n + j] * volts[j]).sum();
                -res[i] + coupling
            })
            .collect()
    }

    fn nodal_stamps(&self, rhs: &[f64], jac: &[f64]) -> Vec<Stamp<AnalogReference, f64>> {
        let n = self.num_terminals();
        let mut stamps = Vec::new();
        for (i, value) in rhs.iter().enumerate() {
            if *value != 0.0 {
                if let Some(row) = &self.node_refs[i] {
                    stamps.push(Stamp::Rhs(row.clone(), *value));
                }
            }
        }
        for i in 0..n {
            for j in 0..n {
                let g = jac[i * n + j];
                if g == 0.0 {
                    continue;
                }
                if let (Some(row), Some(col)) = (&self.node_refs[i], &self.node_refs[j]) {
                    stamps.push(Stamp::Matrix(row.clone(), col.clone(), g));
                }
            }
        }
        stamps
    }

    /// Ideal-source rows: per force `i`, a branch-current unknown `ib_i`,
    /// KCL coupling at its terminals, and the branch equation
    /// `V(p) − V(m) − E_i(V) = 0`, Newton-linearised.
    fn force_stamps(&self, volts: &[f64]) -> Vec<Stamp<AnalogReference, f64>> {
        let nf = self.kernel.num_forces();
        if nf == 0 {
            return Vec::new();
        }
        let n = self.num_terminals();
        let mut e = vec![0.0; nf];
        let mut de = vec![0.0; nf * n];
        self.kernel
            .eval_force(volts, &self.params, &self.state, &self.vars, &self.sim, &mut e);
        self.kernel
            .eval_force_jacobian(volts, &self.params, &self.state, &self.vars, &self.sim, &mut de);

        let mut stamps = Vec::new();
        for (i, (branch, &(plus, minus))) in self
            .force_refs
            .iter()
            .zip(self.kernel.force_terminals().iter())
            .enumerate()
        {
            let plus_ref = self.terminal_ref(plus);
            let minus_ref = self.terminal_ref(minus);
            // KCL: ib leaves `plus`, enters `minus`.
            if let Some(p) = &plus_ref {
                stamps.push(Stamp::Matrix(p.clone(), branch.clone(), 1.0));
                stamps.push(Stamp::Matrix(branch.clone(), p.clone(), 1.0));
            }
            if let Some(m) = &minus_ref {
                stamps.push(Stamp::Matrix(m.clone(), branch.clone(), -1.0));
                stamps.push(Stamp::Matrix(branch.clone(), m.clone(), -1.0));
            }
            // Controlled-source coupling: −∂E/∂V_j on the branch row.
            let mut rhs = e[i];
            for j in 0..n {
                let g = de[i * n + j];
                if g == 0.0 {
                    continue;
                }
                if let Some(col) = &self.node_refs[j] {
                    stamps.push(Stamp::Matrix(branch.clone(), col.clone(), -g));
                }
                rhs -= g * volts[j];
            }
            stamps.push(Stamp::Rhs(branch.clone(), rhs));
        }
        stamps
    }

    /// The netlist reference for a kernel terminal node (None = ground).
    fn terminal_ref(&self, node: crate::ir::NodeId) -> Option<AnalogReference> {
        self.kernel
            .terminals()
            .iter()
            .position(|&t| t == node)
            .and_then(|i| self.node_refs[i].clone())
    }

    // ── Analysis loads ──

    pub fn load_dc(
        &mut self,
        state: &DcAnalysisState,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.sync_sim(context, Analysis::Dc);
        let volts = self.collect_volts(&|k| {
            state.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0)
        });
        let (res, jac) = self.eval_rhs_jac(&volts);
        let rhs = self.norton_rhs(&volts, &res, &jac);
        let mut stamps = self.nodal_stamps(&rhs, &jac);
        stamps.extend(self.force_stamps(&volts));
        stamps
    }

    pub fn load_transient(
        &mut self,
        states: &TransientAnalysisState,
        tran_ctx: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        let dt: f64 = tran_ctx.dt.into();
        let alpha = if dt > 0.0 { 1.0 / dt } else { 0.0 };
        self.sim.abstime = tran_ctx.time.into();
        self.sim.step = dt;
        self.sim.tfinal = tran_ctx.tfinal.into();
        self.sync_sim(context, Analysis::Tran);

        let volts = self.collect_volts(&|k| {
            states.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0)
        });
        let (res, mut jac) = self.eval_rhs_jac(&volts);
        // Backward-Euler companion: `alpha·dQ/dV` on the Jacobian; the
        // history source falls out of the Norton transform because the
        // linearisation point is the previously accepted solution.
        if self.kernel.has_reactive() {
            let n = self.num_terminals();
            let mut qjac = vec![0.0; n * n];
            self.kernel
                .eval_charge_jacobian(&volts, &self.params, &self.state, &self.vars, &self.sim, &mut qjac);
            for (j, q) in jac.iter_mut().zip(&qjac) {
                *j += alpha * q;
            }
        }
        let rhs = self.norton_rhs(&volts, &res, &jac);
        let mut stamps = self.nodal_stamps(&rhs, &jac);
        stamps.extend(self.force_stamps(&volts));
        stamps
    }

    pub fn load_ac(
        &mut self,
        dc_op: &DcAnalysisResult,
        ac_ctx: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex64>> {
        self.sync_sim(context, Analysis::Ac);
        let freq: f64 = ac_ctx.frequency.into();
        let omega = 2.0 * std::f64::consts::PI * freq;

        let refs = self.node_refs.clone();
        let volts = self.collect_volts(&|k| {
            refs.iter()
                .flatten()
                .find(|r| r.idx() == Some(k))
                .and_then(|r| dc_op.get(r.variable().clone()))
                .unwrap_or(0.0)
        });

        let n = self.num_terminals();
        let (_, jac) = self.eval_rhs_jac(&volts);
        let mut stamps = Vec::new();
        let complex_stamp =
            |stamps: &mut Vec<Stamp<AnalogReference, Complex64>>,
             refs: &[Option<AnalogReference>],
             i: usize,
             j: usize,
             value: Complex64| {
                if let (Some(row), Some(col)) = (&refs[i], &refs[j]) {
                    stamps.push(Stamp::Matrix(row.clone(), col.clone(), value));
                }
            };
        for i in 0..n {
            for j in 0..n {
                let g = jac[i * n + j];
                if g != 0.0 {
                    complex_stamp(&mut stamps, &self.node_refs, i, j, Complex64::new(g, 0.0));
                }
            }
        }
        // Reactive admittance `jω·dQ/dV`.
        if self.kernel.has_reactive() {
            let mut qjac = vec![0.0; n * n];
            self.kernel
                .eval_charge_jacobian(&volts, &self.params, &self.state, &self.vars, &self.sim, &mut qjac);
            for i in 0..n {
                for j in 0..n {
                    let c = qjac[i * n + j];
                    if c != 0.0 {
                        complex_stamp(&mut stamps, &self.node_refs, i, j, Complex64::new(0.0, omega * c));
                    }
                }
            }
        }
        // Force branches stay ideal in small-signal: same topology rows,
        // zero source perturbation.
        for (i, branch) in self.force_refs.iter().enumerate() {
            let (plus, minus) = self.kernel.force_terminals()[i];
            if let Some(p) = self.terminal_ref(plus) {
                stamps.push(Stamp::Matrix(p.clone(), branch.clone(), Complex64::new(1.0, 0.0)));
                stamps.push(Stamp::Matrix(branch.clone(), p, Complex64::new(1.0, 0.0)));
            }
            if let Some(m) = self.terminal_ref(minus) {
                stamps.push(Stamp::Matrix(m.clone(), branch.clone(), Complex64::new(-1.0, 0.0)));
                stamps.push(Stamp::Matrix(branch.clone(), m, Complex64::new(-1.0, 0.0)));
            }
        }
        stamps
    }

    pub fn noise_current_psd(&mut self, dc_point: &DcAnalysisResult) -> Vec<Noise> {
        let count = self.kernel.num_noise();
        if count == 0 {
            return Vec::new();
        }
        let refs = self.node_refs.clone();
        let volts = self.collect_volts(&|k| {
            refs.iter()
                .flatten()
                .find(|r| r.idx() == Some(k))
                .and_then(|r| dc_op_voltage(r, dc_point))
                .unwrap_or(0.0)
        });
        let mut psd = vec![0.0; count];
        self.kernel
            .eval_noise(&volts, &self.params, &self.state, &self.vars, &self.sim, &mut psd);

        self.noise_refs
            .iter()
            .zip(psd)
            .filter_map(|((plus, minus), value)| {
                let (plus, minus) = (plus.clone()?, minus.clone()?);
                (value > 0.0).then(|| Noise {
                    terminals: (plus, minus),
                    value: value.into(),
                })
            })
            .collect()
    }

    /// Service runtime operators at the accepted solution point.
    pub fn accept_timestep(&mut self, state: &CircularArrayBuffer2<f64>, ctx: &Context) {
        let volts = self.collect_volts(&|k| {
            state.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0)
        });
        if !self.operators.is_empty() {
            let mut inputs = vec![0.0; self.state.len()];
            self.kernel
                .eval_state_inputs(&volts, &self.params, &self.state, &self.vars, &self.sim, &mut inputs);
            for op in &mut self.operators {
                let slot = op.slot();
                self.state[slot] = op.accept(ctx.time, inputs[slot]);
            }
        }
        self.detect_events(&volts, ctx.time);
        self.last_volts = volts;
    }

    /// Runtime events: evaluate the trigger values at the accepted solution,
    /// detect transitions, and execute fired events' variable updates.
    fn detect_events(&mut self, volts: &[f64], time: f64) {
        let num_events = self.kernel.events().len();
        if num_events == 0 {
            return;
        }
        let mut triggers = vec![0.0; num_events];
        self.kernel
            .eval_event_triggers(volts, &self.params, &self.state, &self.vars, &self.sim, &mut triggers);
        let mut fired = vec![false; num_events];
        for (i, detector) in self.event_detectors.iter_mut().enumerate() {
            let trigger = &self.kernel.events()[i].trigger;
            fired[i] = detector.fired(trigger, triggers[i], time, self.event_periods[i]);
        }
        if fired.iter().any(|&f| f) {
            self.apply_event_actions(&fired, volts);
        }
    }

    pub fn bound_step_hint(&self) -> f64 {
        if !self.kernel.has_bound_step() {
            return f64::INFINITY;
        }
        self.kernel
            .eval_bound_step(&self.last_volts, &self.params, &self.state, &self.vars, &self.sim)
    }

    /// The last accepted terminal voltages (kernel terminal order).
    /// Used by the A2D bridge to pass analog voltages to the digital side.
    pub fn last_volts(&self) -> &[f64] {
        &self.last_volts
    }

    /// The analog kernel's terminal NodeIds (terminal order).
    pub fn terminal_node_ids(&self) -> &[crate::ir::NodeId] {
        self.kernel.terminals()
    }

    fn sync_sim(&mut self, context: &Context, analysis: Analysis) {
        self.sim.temperature = context.temperature;
        self.sim.gmin = context.gmin.into();
        self.sim.current_analysis = super::analysis_code(analysis);
        // Outside transient there is no integration step; companion terms
        // that scale with `sim.step` (the `idt` in-step coupling) vanish.
        if analysis != Analysis::Tran {
            self.sim.step = 0.0;
        }
    }
}

fn dc_op_voltage(reference: &AnalogReference, dc_point: &DcAnalysisResult) -> Option<f64> {
    dc_point.get(reference.variable().clone())
}
