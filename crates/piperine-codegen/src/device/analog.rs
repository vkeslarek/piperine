//! The analog side of a device instance: MNA stamping around an
//! [`AnalogKernel`], including the reactive companion model, ideal-source
//! branch rows, runtime operators (`delay`/`slew`), and noise.

use std::collections::VecDeque;
use std::sync::Arc;

use num_complex::Complex64;

use piperine_solver::abi::{AnalogReference, BranchIdentifier, Netlist, NodeIdentifier};
use piperine_solver::abi::AcAnalysisContext;
use piperine_solver::abi::{DcAnalysisResult, DcAnalysisState};
use piperine_solver::abi::Noise;
use piperine_solver::abi::{TransientAnalysisContext, TransientAnalysisState};
use piperine_solver::abi::CircularArrayBuffer2;
use piperine_solver::abi::{TrBdf2, TrBdf2Phase};
use piperine_solver::abi::{AsIndex, Stamp};
use piperine_solver::abi::Context;

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
                if let Some(m) = *modulus
                    && m > 0.0 {
                        *value -= m * (*value / m).floor();
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
    event_periods: Vec<(f64, f64)>,
    /// Module-level persistent variable values read by the kernel through
    /// the D2A bridge (`vars[VarId]`). Synced from the digital side after
    /// each `eval_discrete` call.
    vars: Vec<f64>,
    /// Last accepted node voltages (for `bound_step_hint`).
    last_volts: Vec<f64>,
    /// Whether `$limit` voltage limiting was still moving at the last load
    /// (vetoes Newton convergence — see `update_limits`).
    limiting_active: bool,
    /// Per-`$limit` seed voltage `vcrit`, kept so `update_limits` can tell an
    /// unbiased (still-seeded) junction from a tracked one.
    limit_seeds: Vec<f64>,
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
                let param_names = kernel.param_names();
                let value = |expr: &piperine_lang::parse::ast::Expr| {
                    let resolve = |name: &str| -> Option<f64> {
                        param_names.iter().position(|n| n == name)
                            .and_then(|i| params.get(i).copied())
                    };
                    crate::ir::pom_eval_const(expr, &resolve)
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
                CompiledTrigger::Timer { period, phase } => {
                    let param_names = kernel.param_names();
                    let resolve = |name: &str| -> Option<f64> {
                        param_names.iter().position(|n| n == name)
                            .and_then(|i| params.get(i).copied())
                    };
                    let p = crate::ir::pom_eval_const(period, &resolve)
                        .map_err(CodegenError::ConstEval)?;
                    // First fire at `phase` (a phased timer `@timer(period, phase)`),
                    // or at `period` for the unphased `@timer(period)` (phase ≤ 0).
                    let ph = crate::ir::pom_eval_const(phase, &resolve)
                        .map_err(CodegenError::ConstEval)?;
                    Ok(if ph > 0.0 { (p, ph) } else { (p, p) })
                }
                _ => Ok((0.0, 0.0)),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let event_detectors = kernel
            .events()
            .iter()
            .zip(&event_periods)
            .map(|(_, &(_period, first_fire))| EventDetector { seeded: false, prev: 0.0, next_fire: first_fire })
            .collect();

        let sim = SimCtx { param_given_mask, ..Default::default() };
        let n = kernel.num_terminals();
        let num_vars = kernel.num_vars();
        let num_limits = kernel.num_limits();
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
            limiting_active: false,
            limit_seeds: vec![0.0; num_limits],
        };
        instance.fire_initial_events();
        instance.seed_limits();
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
            if *value != 0.0
                && let Some(row) = &self.node_refs[i] {
                    stamps.push(Stamp::Rhs(row.clone(), *value));
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
    fn force_stamps(&self, volts: &[f64], src_scale: f64) -> Vec<Stamp<AnalogReference, f64>> {
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
        // Source stepping: scale the forced value (and its bias dependence) by
        // the independent-source factor. Internal-node-collapse forces
        // (`V(c,cp) <- 0`) have `e = 0`, so they are untouched; only real
        // driven voltages ramp. `1.0` in normal operation.
        if src_scale != 1.0 {
            for v in &mut e {
                *v *= src_scale;
            }
            for v in &mut de {
                *v *= src_scale;
            }
        }

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
        state: &DcAnalysisState<'_>,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.sync_sim(context, Analysis::Dc);
        let volts = self.collect_volts(&|k| {
            state.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0)
        });
        let (res, jac) = self.eval_rhs_jac(&volts);
        // With `$limit`, the residual was evaluated at the *limited* junction
        // voltages, so the Norton companion must linearize there too
        // (ngspice: `cdeq = cd − gd·vlim`, not `cd − gd·vnode`). Otherwise the
        // node is pinned at a non-solution.
        let veff = self.limited_volts(&volts);
        let rhs = self.norton_rhs(&veff, &res, &jac);
        let mut stamps = self.nodal_stamps(&rhs, &jac);
        stamps.extend(self.force_stamps(&volts, state.src_scale));
        self.update_limits(&volts);
        stamps
    }

    /// Node voltages with each `$limit` junction branch replaced by its limited
    /// value `vlim` — the linearization point for the Norton transform when
    /// voltage limiting is active. Non-junction nodes are unchanged. Returns
    /// `volts` unchanged when the device has no `$limit`.
    fn limited_volts(&self, volts: &[f64]) -> Vec<f64> {
        let nl = self.kernel.num_limits();
        if nl == 0 {
            return volts.to_vec();
        }
        let mut vlim = vec![0.0; nl];
        self.kernel
            .eval_limit_update(volts, &self.params, &self.state, &self.vars, &self.sim, &mut vlim);
        let mut vnew = vec![0.0; nl];
        self.kernel
            .eval_limit_vnew(volts, &self.params, &self.state, &self.vars, &self.sim, &mut vnew);
        let mut veff = volts.to_vec();
        for (i, branch) in self.kernel.limit_branches().iter().enumerate() {
            let Some((plus, minus)) = branch else { continue };
            let vp = plus.map_or(0.0, |p| volts[p]);
            let vm = minus.map_or(0.0, |m| volts[m]);
            let vbr_raw = vp - vm;
            // `vnew = type · vbr_raw`, type = ±1: recover the branch polarity so
            // the limited node-space voltage is `vlim / type`.
            let ty = if vbr_raw.abs() > 1e-12 { (vnew[i] / vbr_raw).signum() } else { 1.0 };
            let vbr_eff = vlim[i] * ty;
            // Move the minus node if it is a real node (keeps a shared plus node
            // — e.g. a BJT base' — fixed); otherwise move the plus node.
            if let Some(m) = minus {
                veff[*m] = vp - vbr_eff;
            } else if let Some(p) = plus {
                veff[*p] = vm + vbr_eff;
            }
        }
        veff
    }

    /// Advance the `$limit` vold slots after loading: store this iteration's
    /// limited voltages so the next Newton iteration limits against them
    /// (ngspice stores the limited junction voltage in device state). This is
    /// what makes junction devices converge — without it a stiff exponential
    /// overshoots and stalls. Called each iteration of DC and transient loads;
    /// AC/noise reuse the converged DC vold (limiter inactive there).
    fn update_limits(&mut self, volts: &[f64]) {
        let nl = self.kernel.num_limits();
        if nl == 0 {
            return;
        }
        let base = self.kernel.limit_base();
        let mut vlim = vec![0.0; nl];
        self.kernel
            .eval_limit_update(volts, &self.params, &self.state, &self.vars, &self.sim, &mut vlim);
        let mut vnew = vec![0.0; nl];
        self.kernel
            .eval_limit_vnew(volts, &self.params, &self.state, &self.vars, &self.sim, &mut vnew);
        // A junction is "still limiting" iff pnjlim actually clamped this
        // iteration — the limited value differs from the raw branch voltage
        // (ngspice's `Check == 1`). While that holds, the Newton loop must not
        // declare convergence (see PiperineDevice::limiting_active): a clamped
        // junction can momentarily satisfy KCL at a non-solution voltage. Tiny
        // Newton jitter once limiting is off (vnew ≈ vlim) must NOT veto, hence
        // the tolerance below.
        let mut active = false;
        for (i, v) in vlim.into_iter().enumerate() {
            let old = self.state[base + i];
            // Preserve the vcrit seed until the junction is first biased:
            // `pnjlim(0, vcrit) = 0` on the opening iterations would discard the
            // seed and let the node float to the supply (ngspice MODEINITJCT).
            let seeded = (old - self.limit_seeds[i]).abs() <= 1e-12;
            if seeded && v < old {
                continue;
            }
            if (vnew[i] - v).abs() > 1e-6 + 1e-4 * vnew[i].abs() {
                active = true;
            }
            self.state[base + i] = v;
        }
        self.limiting_active = active;
    }

    /// Whether junction voltage limiting is still moving (see `update_limits`).
    pub fn limiting_active(&self) -> bool {
        self.limiting_active
    }

    /// Instance parameter names, in kernel order (aligned with the values).
    pub fn param_names(&self) -> &[String] {
        self.kernel.param_names()
    }

    /// Current value of parameter `name`, or `None` if the kernel has no such
    /// parameter.
    pub fn param(&self, name: &str) -> Option<f64> {
        self.param_index(name).map(|i| self.params[i])
    }

    /// Overwrite parameter `name` with `value`, taking effect on the next load.
    /// Returns `false` when there is no such parameter. The matrix structure is
    /// unchanged, so this needs only a restamp — never a rebuild.
    pub fn set_param(&mut self, name: &str, value: f64) -> bool {
        match self.param_index(name) {
            Some(i) => {
                self.params[i] = value;
                true
            }
            None => false,
        }
    }

    fn param_index(&self, name: &str) -> Option<usize> {
        self.kernel.param_names().iter().position(|n| n == name)
    }

    /// Seed each `$limit` vold slot with its critical voltage `vcrit`, so a
    /// junction starts limiting near turn-on (ngspice MODEINITJCT) rather than
    /// from 0 V. `vcrit` depends only on params/temperature, not node voltages.
    fn seed_limits(&mut self) {
        let nl = self.kernel.num_limits();
        if nl == 0 {
            return;
        }
        let base = self.kernel.limit_base();
        let zeros = vec![0.0; self.num_terminals()];
        let mut seeds = vec![0.0; nl];
        self.kernel
            .eval_limit_seed(&zeros, &self.params, &self.state, &self.vars, &self.sim, &mut seeds);
        for (i, s) in seeds.iter().enumerate() {
            self.state[base + i] = *s;
        }
        self.limit_seeds = seeds;
    }

    /// `@initial` UIC seeds: `(plus, minus, value)` in netlist references —
    /// the branch voltage `V(plus,minus)` the device wants at t=0 (SPICE
    /// `.ic`). Values are instance-constant. Ground terminals are `None`.
    pub fn initial_conditions(&self) -> Vec<(Option<AnalogReference>, Option<AnalogReference>, f64)> {
        let nic = self.kernel.num_initial_conditions();
        if nic == 0 {
            return Vec::new();
        }
        let zeros = vec![0.0; self.num_terminals()];
        let mut vals = vec![0.0; nic];
        self.kernel
            .eval_initial_conditions(&zeros, &self.params, &self.state, &self.vars, &self.sim, &mut vals);
        self.kernel
            .initial_condition_terminals()
            .iter()
            .zip(vals)
            .map(|(&(p, m), v)| (self.terminal_ref(p), self.terminal_ref(m), v))
            .collect()
    }

    pub fn load_transient(
        &mut self,
        states: &TransientAnalysisState<'_>,
        tran_ctx: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        let dt: f64 = tran_ctx.h;
        self.sim.abstime = tran_ctx.time;
        self.sim.step = dt;
        self.sim.tfinal = tran_ctx.tfinal;
        self.sync_sim(context, Analysis::Tran);

        let volts = self.collect_volts(&|k| {
            states.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0)
        });
        let (res, mut jac) = self.eval_rhs_jac(&volts);
        // TR-BDF2 reactive companion. The kernel stamps the per-phase
        // companion derived from `TrBdf2::phase_coeffs` (the single source of
        // truth, MD-07). The Jacobian carries `c0·dQ/dV`; the history charges
        // enter the RHS explicitly (the linearisation point is the current
        // Newton guess, so folding them into the Norton transform would cancel
        // the reactive current at convergence and collapse every step to DC).
        //
        // TR stage subtlety: the trapezoidal companion is
        //   i_{C,n+γ} = (2/(γh))(Q_{n+γ} − Q_n) − i_{C,n}
        // which needs the *previous capacitor current* `i_{C,n}` — not a pure
        // charge derivative. The kernel re-derives `i_{C,n}` from the prior
        // step's BDF2 formula (coeffs at `prev_h`, charges at view 1/2/3),
        // which is fixed across the TR Newton iteration, hence idempotent. The
        // BDF2 stage is a pure derivative and needs no current term.
        let veff = self.limited_volts(&volts);
        let mut rhs = self.norton_rhs(&veff, &res, &jac);
        if self.kernel.has_reactive() && dt > 0.0 {
            let (c0, c1, c2) = TrBdf2::phase_coeffs(tran_ctx.phase, tran_ctx.h);
            let n = self.num_terminals();
            let mut qjac = vec![0.0; n * n];
            self.kernel
                .eval_charge_jacobian(&volts, &self.params, &self.state, &self.vars, &self.sim, &mut qjac);
            for (j, q) in jac.iter_mut().zip(&qjac) {
                *j += c0 * q;
            }
            let prev_volts = self.collect_volts(&|k| {
                states.view(1).and_then(|s| s.get(k).copied()).unwrap_or(0.0)
            });
            let prev2_volts = self.collect_volts(&|k| {
                states.view(2).and_then(|s| s.get(k).copied()).unwrap_or(0.0)
            });
            let mut q_now = vec![0.0; n];
            let mut q_prev = vec![0.0; n];
            let mut q_prev2 = vec![0.0; n];
            self.kernel.eval_charge(&volts, &self.params, &self.state, &self.vars, &self.sim, &mut q_now);
            self.kernel.eval_charge(&prev_volts, &self.params, &self.state, &self.vars, &self.sim, &mut q_prev);
            self.kernel.eval_charge(&prev2_volts, &self.params, &self.state, &self.vars, &self.sim, &mut q_prev2);

            // Trapezoidal previous-current term (TR stage only): re-derive the
            // capacitor current at x_n (= view 1) from the prior step's BDF2.
            let mut prev_i_c = vec![0.0; n];
            if matches!(tran_ctx.phase, TrBdf2Phase::Trapezoidal) && tran_ctx.prev_h > 0.0 {
                let (d0, d1, d2) = TrBdf2::phase_coeffs(TrBdf2Phase::Bdf2, tran_ctx.prev_h);
                let prev3_volts = self.collect_volts(&|k| {
                    states.view(3).and_then(|s| s.get(k).copied()).unwrap_or(0.0)
                });
                let mut q_prev3 = vec![0.0; n];
                self.kernel.eval_charge(&prev3_volts, &self.params, &self.state, &self.vars, &self.sim, &mut q_prev3);
                for i in 0..n {
                    prev_i_c[i] = d0 * q_prev[i] + d1 * q_prev2[i] + d2 * q_prev3[i];
                }
            }

            for i in 0..n {
                // RHS equivalent source: `c0·(dQ/dV·v_guess) − i_C(v_guess)`.
                let coupling: f64 = (0..n).map(|jx| qjac[i * n + jx] * volts[jx]).sum();
                let mut i_c = c0 * q_now[i] + c1 * q_prev[i] + c2 * q_prev2[i];
                // TR stage: the trapezoidal companion subtracts i_{C,n}.
                if matches!(tran_ctx.phase, TrBdf2Phase::Trapezoidal) {
                    i_c -= prev_i_c[i];
                }
                rhs[i] += c0 * coupling - i_c;
            }
        }
        let mut stamps = self.nodal_stamps(&rhs, &jac);
        // Transient never source-steps (that homotopy is DC-only) → full scale.
        stamps.extend(self.force_stamps(&volts, 1.0));
        // Inductor flux companion `V(p,n) = dΦ/dt`, Φ = L·ib, on the force
        // branch's own current unknown. DC uses no flux (dt = 0 → the
        // inductor is a short, already forced to 0 V by `force_stamps`).
        if self.kernel.has_force_flux() && dt > 0.0 {
            // The inductor flux companion `V = dΦ/dt` is the dual of the
            // capacitor case: the TR stage would need the previous *voltage*
            // (force value) for a true trapezoidal companion. That dual
            // tracking is a follow-up; for now both phases use the pure
            // derivative form (`TrBdf2::phase_coeffs`), matching the prior
            // behaviour — no regression, just no TR-stage accuracy gain for
            // inductors yet.
            let (c0, c1, c2) = TrBdf2::phase_coeffs(tran_ctx.phase, tran_ctx.h);
            stamps.extend(self.force_flux_stamps(&volts, states, c0, c1, c2));
        }
        self.update_limits(&volts);
        stamps
    }

    /// Absolute landing points this instance's `@timer` events fire at within
    /// `(from, from + horizon]`. Each timer fires every `period` (its current
    /// `next_fire` advanced into the window); those fire times are exactly the
    /// integrator breakpoints a periodic/switched source needs so it never
    /// steps over a switching edge. Non-timer events (crossings) are detected
    /// reactively and contribute no static breakpoints here.
    pub fn next_breakpoints(&self, from: f64, horizon: f64) -> Vec<f64> {
        let mut out = Vec::new();
        let end = from + horizon;
        for (det, &(period, _first_fire)) in self.event_detectors.iter().zip(&self.event_periods) {
            if period <= 0.0 || !period.is_finite() {
                continue;
            }
            // First fire strictly after `from` (next_fire may lag if the timer
            // hasn't been advanced past the current step yet).
            let mut t = det.next_fire;
            while t <= from {
                t += period;
            }
            while t <= end {
                out.push(t);
                t += period;
            }
        }
        out
    }

    /// LTE-driven timestep suggestion for the transient stepper. Evaluates
    /// the charge at `t_n` and `t_{n-1}` (plus `t_{n-2}` for order ≥ 2),
    /// computes the (order+1)-th divided difference, and returns the
    /// largest dt the model can tolerate given `trtol·chgtol`.
    ///
    /// Returns `None` when the kernel has no reactive ports, when history is
    /// too short, or when the charge has not meaningfully changed.
    pub fn suggest_transient_step(
        &self,
        state_history: &TransientAnalysisState<'_>,
        time_history: &[f64],
        method: piperine_solver::abi::IntegrationMethod,
        context: &Context,
    ) -> Option<f64> {
        if !self.kernel.has_reactive() || time_history.is_empty() {
            return None;
        }
        let dt = time_history[0];
        if dt <= 0.0 {
            return None;
        }
        let order = method.order();
        let trunc = method.truncation_coefficient();

        let q_now = self.charge_at(state_history, 0);
        let q_prev = self.charge_at(state_history, 1);
        // charge_at(2) is only needed for order ≥ 2; compute lazily.
        let order_gte_2 = order >= 2;
        let q_prev2 = if order_gte_2 { self.charge_at(state_history, 2) } else { Vec::new() };

        let ddiv_mag = match order {
            1 => q_now.iter()
                .zip(&q_prev)
                .map(|(&n, &p)| (n - p).abs())
                .fold(0.0_f64, f64::max),
            _ => {
                let p2 = if q_prev2.is_empty() { &q_prev } else { &q_prev2 };
                q_now.iter()
                    .zip(&q_prev)
                    .zip(p2)
                    .map(|((&n, &p1), &p2)| (n - 2.0 * p1 + p2).abs())
                    .fold(0.0_f64, f64::max)
            }
        };

        if ddiv_mag == 0.0 {
            return None;
        }

        let lte = trunc * ddiv_mag;

        let q_mag = q_now.iter()
            .zip(&q_prev)
            .map(|(&n, &p)| n.abs().max(p.abs()))
            .fold(0.0_f64, f64::max);
        let tol = context.tolerances.trtol * context.tolerances.chgtol + context.tolerances.reltol * q_mag + context.tolerances.abstol;

        if lte <= 0.0 || tol <= 0.0 {
            return None;
        }

        let power = 1.0 / ((order + 1) as f64);
        let safety = 0.9_f64;
        let suggested = dt * (safety * tol / lte).powf(power);
        if suggested.is_finite() && suggested > 0.0 {
            Some(suggested)
        } else {
            None
        }
    }

    /// Evaluate the charge vector at the `lookback`-th history point.
    fn charge_at(&self, state_history: &TransientAnalysisState<'_>, lookback: usize) -> Vec<f64> {
        let n = self.num_terminals();
        let volts = self.collect_volts(&|k| {
            state_history.view(lookback).and_then(|s| s.get(k).copied()).unwrap_or(0.0)
        });
        let mut q = vec![0.0; n];
        self.kernel.eval_charge(&volts, &self.params, &self.state, &self.vars, &self.sim, &mut q);
        q
    }

    /// Inductor flux companion stamps for reactive force branches: the branch
    /// equation `V(p,n) = dΦ/dt` with `Φ = L·ib` becomes `… − c0·L·ib =
    /// history`, where `history = L·(c1·ib_{n-1} + c2·ib_{n-2})` reads the
    /// branch-current unknown's own past values.
    fn force_flux_stamps(
        &self,
        volts: &[f64],
        states: &TransientAnalysisState<'_>,
        c0: f64,
        c1: f64,
        c2: f64,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        let terms = self.kernel.flux_terms();
        let mut coeffs = vec![0.0; terms.len()];
        self.kernel
            .eval_force_flux(volts, &self.params, &self.state, &self.vars, &self.sim, &mut coeffs);
        let force_terminals = self.kernel.force_terminals();
        let mut stamps = Vec::new();
        for (&(force_idx, tp, tm), &l) in terms.iter().zip(&coeffs) {
            if l == 0.0 {
                continue;
            }
            let this_branch = &self.force_refs[force_idx];
            // The branch current the flux integrates: the force branch whose
            // terminals are `(tp, tm)` (self = this force; else a mutual
            // partner). Orientation flips the sign if terminals are reversed.
            let (target_idx, sign) = force_terminals
                .iter()
                .position(|&(p, m)| p == tp && m == tm)
                .map(|k| (Some(k), 1.0))
                .or_else(|| force_terminals.iter().position(|&(p, m)| p == tm && m == tp).map(|k| (Some(k), -1.0)))
                .unwrap_or((None, 1.0));
            let Some(target_idx) = target_idx else { continue };
            let target_branch = &self.force_refs[target_idx];
            let bi = target_branch.as_index();
            let ib_prev = bi.and_then(|k| states.view(1).and_then(|s| s.get(k).copied())).unwrap_or(0.0);
            let ib_prev2 = bi.and_then(|k| states.view(2).and_then(|s| s.get(k).copied())).unwrap_or(0.0);
            let coeff = l * sign;
            // Branch `force_idx` equation gains `−c0·coeff·ib_target`; the flux
            // history goes to its RHS.
            stamps.push(Stamp::Matrix(this_branch.clone(), target_branch.clone(), -c0 * coeff));
            stamps.push(Stamp::Rhs(this_branch.clone(), coeff * (c1 * ib_prev + c2 * ib_prev2)));
        }
        stamps
    }

    pub fn load_ac(
        &mut self,
        dc_op: &DcAnalysisResult,
        ac_ctx: &AcAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex64>> {
        self.sync_sim(context, Analysis::Ac);
        let freq: f64 = ac_ctx.frequency;
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
        // Inductor flux admittance in small-signal: `V(p,n) = jω·Φ`, Φ = Σ
        // L·I(branch), so the branch equation gains `−jω·L·ib` on its own (and
        // mutual partner) branch-current unknown. Mirrors the transient
        // companion with `jω` in place of the BDF coefficient.
        if self.kernel.has_force_flux() {
            let terms = self.kernel.flux_terms();
            let mut coeffs = vec![0.0; terms.len()];
            self.kernel
                .eval_force_flux(&volts, &self.params, &self.state, &self.vars, &self.sim, &mut coeffs);
            let force_terminals = self.kernel.force_terminals();
            for (&(force_idx, tp, tm), &l) in terms.iter().zip(&coeffs) {
                if l == 0.0 {
                    continue;
                }
                let (target_idx, sign) = force_terminals
                    .iter()
                    .position(|&(p, m)| p == tp && m == tm)
                    .map(|k| (Some(k), 1.0))
                    .or_else(|| force_terminals.iter().position(|&(p, m)| p == tm && m == tp).map(|k| (Some(k), -1.0)))
                    .unwrap_or((None, 1.0));
                let Some(target_idx) = target_idx else { continue };
                stamps.push(Stamp::Matrix(
                    self.force_refs[force_idx].clone(),
                    self.force_refs[target_idx].clone(),
                    Complex64::new(0.0, -omega * l * sign),
                ));
            }
        }
        // Ideal AC voltage stimulus attached to a force branch: the branch
        // equation RHS becomes `mag·e^{jφ}` (V(plus) − V(minus) = stim).
        if self.kernel.has_force_ac_stim() {
            let nf = self.kernel.num_forces();
            let mut mags = vec![0.0; nf];
            let mut phases = vec![0.0; nf];
            self.kernel
                .eval_force_ac_stim(&volts, &self.params, &self.state, &self.vars, &self.sim, &mut mags, &mut phases);
            for (i, branch) in self.force_refs.iter().enumerate() {
                let stim = Complex64::from_polar(mags[i], phases[i]);
                if stim != Complex64::ZERO {
                    stamps.push(Stamp::Rhs(branch.clone(), stim));
                }
            }
        }
        // `ac_stim` sources: `mag·e^{j·phase}` enters the residual at the
        // branch terminals, so it lands on the RHS negated at `plus` (the
        // system is `A·x = b` with the residual moved to `b`).
        let ns = self.kernel.num_ac_stims();
        if ns > 0 {
            let mut mags = vec![0.0; ns];
            let mut phases = vec![0.0; ns];
            self.kernel
                .eval_ac_stim(&volts, &self.params, &self.state, &self.vars, &self.sim, &mut mags, &mut phases);
            for (i, &(plus, minus)) in self.kernel.ac_stim_terminals().iter().enumerate() {
                let stim = Complex64::from_polar(mags[i], phases[i]);
                if stim == Complex64::ZERO {
                    continue;
                }
                if let Some(p) = self.terminal_ref(plus) {
                    stamps.push(Stamp::Rhs(p, -stim));
                }
                if let Some(m) = self.terminal_ref(minus) {
                    stamps.push(Stamp::Rhs(m, stim));
                }
            }
        }
        stamps
    }

    pub fn noise_current_psd(
        &mut self,
        dc_point: &DcAnalysisResult,
        ac_context: &piperine_solver::abi::AcAnalysisContext,
    ) -> Vec<Noise> {
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
        // Thread the AC frequency into SimCtx so flicker noise can read it
        // (formerly `_ac_context` was ignored here).
        self.sim.frequency = ac_context.frequency;
        let mut psd = vec![0.0; count];
        self.kernel
            .eval_noise(&volts, &self.params, &self.state, &self.vars, &self.sim, &mut psd);

        self.noise_refs
            .iter()
            .zip(psd)
            .filter_map(|((plus, minus), value)| {
                // A ground-mapped terminal is the reference node, not a
                // reason to drop the source (mirrors the OSDI device).
                let plus = plus.clone().unwrap_or_else(AnalogReference::ground);
                let minus = minus.clone().unwrap_or_else(AnalogReference::ground);
                (value > 0.0).then_some(Noise::new((plus, minus), value))
            })
            .collect()
    }

    /// Service runtime operators at the accepted solution point at time `t`.
    pub fn accept_timestep(&mut self, state: &CircularArrayBuffer2<f64>, t: f64) {
        let volts = self.collect_volts(&|k| {
            state.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0)
        });
        if !self.operators.is_empty() {
            let mut inputs = vec![0.0; self.state.len()];
            self.kernel
                .eval_state_inputs(&volts, &self.params, &self.state, &self.vars, &self.sim, &mut inputs);
            for op in &mut self.operators {
                let slot = op.slot();
                self.state[slot] = op.accept(t, inputs[slot]);
            }
        }
        self.detect_events(&volts, t);
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
            fired[i] = detector.fired(trigger, triggers[i], time, self.event_periods[i].0);
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
        self.sim.temperature = context.tolerances.temperature;
        self.sim.gmin = context.tolerances.gmin;
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
