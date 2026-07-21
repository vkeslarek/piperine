//! The analog side of a device instance: MNA stamping around an
//! [`AnalogKernel`], including the reactive companion model, ideal-source
//! branch rows, runtime operators (`delay`/`slew`/`transition`), and noise.

use std::collections::VecDeque;
use std::sync::Arc;

use num_complex::Complex64;

use piperine_solver::abi::{AnalogReference, Netlist, NodeIdentifier};
use piperine_solver::abi::AcAnalysisContext;
use piperine_solver::abi::{DcAnalysisResult, DcAnalysisState};
use piperine_solver::abi::Noise;
use piperine_solver::abi::{TransientAnalysisContext, TransientAnalysisState};
use piperine_solver::abi::CircularArrayBuffer2;
use piperine_solver::abi::{TrBdf2, TrBdf2Phase};
use piperine_solver::abi::{AsIndex, Stamp};
use piperine_solver::abi::Context;

use crate::resolve::{Analysis, NodeId};
use crate::kernel::analog::{AnalogKernel, RuntimeState};
use crate::error::CodegenError;
use crate::emit::abi::SimCtx;

mod events;
mod forces;
mod limits;
mod operators;

use events::EventSystem;
use forces::ForceStamper;
use limits::Limiter;
use operators::Operator;

/// Read-only context shared by every capability's stamp: the compiled
/// kernel, the instance's netlist references, and its parameter/state/var
/// banks — everything a capability needs to evaluate its own compiled
/// functions and turn slot indices into solver references.
struct LoadCtx<'a> {
    kernel: &'a AnalogKernel,
    node_refs: &'a [Option<AnalogReference>],
    params: &'a [f64],
    state: &'a [f64],
    vars: &'a [f64],
    sim: &'a SimCtx,
}

impl LoadCtx<'_> {
    /// The netlist reference for a kernel terminal node (`None` = ground).
    fn terminal_ref(&self, node: NodeId) -> Option<AnalogReference> {
        self.kernel
            .terminals()
            .iter()
            .position(|&t| t == node)
            .and_then(|i| self.node_refs[i].clone())
    }
}

/// One capability's contribution to an MNA load at a given analysis point.
/// `AnalogInstance::load_dc`/`load_transient` iterate the present
/// capabilities and fold their stamps — internal to codegen, never a
/// solver-facing `Element` facet (MD-01: `PiperineDevice` stays flat, no
/// downcast, no per-capability trait object crosses the solver ABI).
trait Stamps {
    /// Real-valued (DC/transient) stamps at `volts`, scaled by the
    /// independent-source ramp factor `src_scale` (1.0 outside DC source
    /// stepping).
    fn stamp(&self, cx: &LoadCtx<'_>, volts: &[f64], src_scale: f64) -> Vec<Stamp<AnalogReference, f64>>;
}

/// The analog half of a device instance.
pub struct AnalogInstance {
    kernel: Arc<AnalogKernel>,
    /// Per-terminal netlist references (`None` = ground).
    node_refs: Vec<Option<AnalogReference>>,
    /// Forces capability: one MNA branch-current unknown per force row.
    forces: ForceStamper,
    /// Netlist references for each noise source's terminals (`None` when a
    /// terminal is ground-mapped).
    noise_refs: Vec<(Option<AnalogReference>, Option<AnalogReference>)>,
    params: Vec<f64>,
    sim: SimCtx,
    /// Runtime-state values read by the kernel (`state[StateId]`).
    state: Vec<f64>,
    operators: Vec<Operator>,
    /// Events capability: per-event trigger detectors + timer periods.
    events: EventSystem,
    /// Module-level persistent variable values read by the kernel through
    /// the D2A bridge (`vars[VarId]`). Synced from the digital side after
    /// each `eval_discrete` call.
    vars: Vec<f64>,
    /// Last accepted node voltages (for `bound_step_hint`).
    last_volts: Vec<f64>,
    /// Limits capability: `$limit` voltage-limiting runtime state.
    limiter: Limiter,
}

impl AnalogInstance {
    pub fn kernel(&self) -> &AnalogKernel {
        &self.kernel
    }

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

        let forces = ForceStamper::new(label, &kernel, netlist);

        // Noise terminals resolve through the kernel terminal order.
        let terminal_slot = |node: crate::resolve::NodeId| {
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
                    crate::resolve::pom_eval_const(expr, &resolve)
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
                    RuntimeState::Transition { delay, rise, fall } => Operator::Transition {
                        slot: spec.id.0 as usize,
                        delay: value(delay)?,
                        rise: value(rise)?.abs(),
                        fall: value(fall)?.abs(),
                        start: 0.0,
                        target: 0.0,
                        t_change: 0.0,
                        seeded: false,
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

        let events = EventSystem::new(&kernel, &params)?;

        let sim = SimCtx { param_given_mask, ..Default::default() };
        let n = kernel.num_terminals();
        let num_vars = kernel.num_vars();
        let num_limits = kernel.num_limits();
        let mut instance = Self {
            state,
            kernel,
            node_refs,
            forces,
            noise_refs,
            params,
            sim,
            operators,
            events,
            vars: vec![0.0; num_vars],
            last_volts: vec![0.0; n],
            limiter: Limiter::new(num_limits),
        };
        instance.fire_initial_events();
        instance.limiter.seed(&instance.kernel, n, &instance.params, &mut instance.state, &instance.vars, &instance.sim);
        Ok(instance)
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

    /// This instance's read-only stamping context (kernel + netlist refs +
    /// parameter/state/var banks), for the capability `Stamps` impls.
    fn ctx(&self) -> LoadCtx<'_> {
        LoadCtx {
            kernel: &self.kernel,
            node_refs: &self.node_refs,
            params: &self.params,
            state: &self.state,
            vars: &self.vars,
            sim: &self.sim,
        }
    }

    /// The netlist reference for a kernel terminal node (None = ground).
    fn terminal_ref(&self, node: crate::resolve::NodeId) -> Option<AnalogReference> {
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
        // Independent-source ramp (source stepping): current sources scale
        // themselves through `$simparam("sourceScaleFactor")` (ngspice
        // CKTsrcFact); forced voltages are scaled at stamp time below.
        self.sim.srcfact = state.src_scale;
        let volts = self.collect_volts(&|k| {
            state.latest().and_then(|s| s.get(k).copied()).unwrap_or(0.0)
        });
        let (res, jac) = self.eval_rhs_jac(&volts);
        // With `$limit`, the residual was evaluated at the *limited* junction
        // voltages, so the Norton companion must linearize there too
        // (ngspice: `cdeq = cd − gd·vlim`, not `cd − gd·vnode`). Otherwise the
        // node is pinned at a non-solution.
        let veff = self.limiter.limited_volts(&self.ctx(), &volts);
        let rhs = self.norton_rhs(&veff, &res, &jac);
        let mut stamps = self.nodal_stamps(&rhs, &jac);
        stamps.extend(self.forces.stamp(&self.ctx(), &volts, state.src_scale));
        self.limiter.update(&self.kernel, &volts, &self.params, &mut self.state, &self.vars, &self.sim);
        stamps
    }

    /// Whether junction voltage limiting is still moving (see `Limiter::update`).
    pub fn limiting_active(&self) -> bool {
        self.limiter.active()
    }

    /// Runtime banks read by the kernel — `(state, vars)` — exposed for
    /// opt-in per-step recording (the host's `Trace.i` recompute path).
    pub fn runtime_banks(&self) -> (&[f64], &[f64]) {
        (&self.state, &self.vars)
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

    /// Whether a live write to `name` would flip optional-param presence:
    /// the kernel branches on `$param_given(name)` and this instance was
    /// built without the parameter given. The value alone cannot surface
    /// the presence-guarded behavior — such a write is structural
    /// (`Invalidation::Rebuild`; the host re-elaborates, LIVE-14).
    pub fn set_flips_presence(&self, name: &str) -> bool {
        self.param_index(name).is_some_and(|i| {
            self.kernel.presence_queried(i)
                && (self.sim.param_given_mask >> i.min(63)) & 1 == 0
        })
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
        // Transient never source-steps (that homotopy is DC-only).
        self.sim.srcfact = 1.0;

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
        let veff = self.limiter.limited_volts(&self.ctx(), &volts);
        let mut rhs = self.norton_rhs(&veff, &res, &jac);
        if self.kernel.has_reactive() && dt > 0.0 {
            // `stage_coeffs`: after a discontinuity (`prev_h = 0`) the TR
            // stage degrades to backward Euler — the `i_{C,n}` term below is
            // unavailable there, and the full trapezoid weight without it
            // doubles the derivative estimate.
            let (c0, c1, c2) = TrBdf2::stage_coeffs(tran_ctx.phase, tran_ctx.h, tran_ctx.prev_h);
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
        stamps.extend(self.forces.stamp(&self.ctx(), &volts, 1.0));
        // Inductor flux companion `V(p,n) = dΦ/dt`, Φ = L·ib, on the force
        // branch's own current unknown. DC uses no flux (dt = 0 → the
        // inductor is a short, already forced to 0 V by `force_stamps`).
        if self.kernel.has_force_flux() && dt > 0.0 {
            // The inductor flux companion `V = dΦ/dt` is the dual of the
            // capacitor case. The TR stage's trapezoidal form
            //   V_{n+γ} = (2/(γh))(Φ_{n+γ} − Φ_n) − V_n
            // needs the previous *branch voltage* `V_n` (the dual of the
            // capacitor's `i_{C,n}` term) — without it the TR stage
            // systematically doubles the derivative estimate and the RL
            // trajectory runs at the wrong time constant. `V_n` is read from
            // the accepted history (view 1), fixed across the Newton
            // iteration; after a discontinuity (`prev_h = 0`) it is taken as
            // 0, mirroring the capacitor's `i_{C,n} = 0` restart convention.
            stamps.extend(self.force_flux_stamps(&volts, states, tran_ctx));
        }
        self.limiter.update(&self.kernel, &volts, &self.params, &mut self.state, &self.vars, &self.sim);
        stamps
    }

    /// Absolute landing points this instance's `@timer` events fire at within
    /// `(from, from + horizon]`, plus the pending ramp edges of any
    /// `transition` operator (ramp start at `t_change + td`, ramp end at
    /// `+ rise/fall`). Each timer fires every `period` (its current
    /// `next_fire` advanced into the window); those fire times are exactly the
    /// integrator breakpoints a periodic/switched source needs so it never
    /// steps over a switching edge. Non-timer events (crossings) are detected
    /// reactively and contribute no static breakpoints here.
    pub fn next_breakpoints(&self, from: f64, horizon: f64) -> Vec<f64> {
        let mut out = Vec::new();
        let end = from + horizon;
        self.events.next_breakpoints(from, end, &mut out);
        for op in &self.operators {
            op.pending_edges(from, end, &mut out);
        }
        out
    }

    /// LTE-driven timestep suggestion for the transient stepper. Evaluates
    /// the charge at `t_n`, `t_{n-1}` and `t_{n-2}`, computes the second
    /// divided difference scaled by the trapezoidal LTE coefficient (1/12 —
    /// TR-BDF2's trapezoidal stage is the order-2 form this estimate models),
    /// and returns the largest dt the model can tolerate given `trtol·chgtol`.
    ///
    /// Returns `None` when the kernel has no reactive ports, when history is
    /// too short, or when the charge has not meaningfully changed.
    pub fn suggest_transient_step(
        &self,
        state_history: &TransientAnalysisState<'_>,
        time_history: &[f64],
        context: &Context,
    ) -> Option<f64> {
        if !self.kernel.has_reactive() || time_history.is_empty() {
            return None;
        }
        let dt = time_history[0];
        if dt <= 0.0 {
            return None;
        }
        // Order-2 trapezoidal LTE coefficient (TR-BDF2's TR stage).
        const ORDER: usize = 2;
        const TRUNC: f64 = 1.0 / 12.0;

        let q_now = self.charge_at(state_history, 0);
        let q_prev = self.charge_at(state_history, 1);
        let q_prev2 = self.charge_at(state_history, 2);

        let p2 = if q_prev2.is_empty() { &q_prev } else { &q_prev2 };
        let ddiv_mag = q_now
            .iter()
            .zip(&q_prev)
            .zip(p2)
            .map(|((&n, &p1), &p2)| (n - 2.0 * p1 + p2).abs())
            .fold(0.0_f64, f64::max);

        if ddiv_mag == 0.0 {
            return None;
        }

        let lte = TRUNC * ddiv_mag;

        let q_mag = q_now.iter()
            .zip(&q_prev)
            .map(|(&n, &p)| n.abs().max(p.abs()))
            .fold(0.0_f64, f64::max);
        let tol = context.tolerances.trtol * context.tolerances.chgtol + context.tolerances.reltol * q_mag + context.tolerances.abstol;

        if lte <= 0.0 || tol <= 0.0 {
            return None;
        }

        let power = 1.0 / ((ORDER + 1) as f64);
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
        tran_ctx: &TransientAnalysisContext,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        // `stage_coeffs`: backward-Euler TR stage after a discontinuity
        // (`prev_h = 0`), where the `V_n` correction below is unavailable.
        let (c0, c1, c2) = TrBdf2::stage_coeffs(tran_ctx.phase, tran_ctx.h, tran_ctx.prev_h);
        let terms = self.kernel.flux_terms();
        let mut coeffs = vec![0.0; terms.len()];
        self.kernel
            .eval_force_flux(volts, &self.params, &self.state, &self.vars, &self.sim, &mut coeffs);
        let force_terminals = self.kernel.force_terminals();
        let mut stamps = Vec::new();
        // TR-stage trapezoidal correction: each flux-carrying branch row
        // subtracts its previous branch voltage `V_n` once (see the caller's
        // companion note). Applied per branch, not per flux term — a branch
        // with self + mutual terms still has one `V_n`.
        if matches!(tran_ctx.phase, TrBdf2Phase::Trapezoidal) && tran_ctx.prev_h > 0.0 {
            let mut corrected: Vec<usize> = Vec::new();
            for (&(force_idx, _, _), &l) in terms.iter().zip(&coeffs) {
                if l == 0.0 || corrected.contains(&force_idx) {
                    continue;
                }
                corrected.push(force_idx);
                let (plus, minus) = force_terminals[force_idx];
                let read_prev = |node: crate::resolve::NodeId| -> f64 {
                    self.terminal_ref(node)
                        .and_then(|r| r.idx())
                        .and_then(|k| states.view(1).and_then(|s| s.get(k).copied()))
                        .unwrap_or(0.0)
                };
                let v_prev = read_prev(plus) - read_prev(minus);
                if v_prev != 0.0 {
                    stamps.push(Stamp::Rhs(self.forces.refs()[force_idx].clone(), -v_prev));
                }
            }
        }
        for (&(force_idx, tp, tm), &l) in terms.iter().zip(&coeffs) {
            if l == 0.0 {
                continue;
            }
            let this_branch = &self.forces.refs()[force_idx];
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
            let target_branch = &self.forces.refs()[target_idx];
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
        // Integrator admittance `dRes/dV / (jω)` for `idt`/`idtmod` terms —
        // the linear-operator stamp `X/(jω)` = `−j·X/ω`.
        if self.kernel.has_ac_idt() {
            let mut gjac = vec![0.0; n * n];
            self.kernel
                .eval_ac_idt_jacobian(&volts, &self.params, &self.state, &self.vars, &self.sim, &mut gjac);
            for i in 0..n {
                for j in 0..n {
                    let g = gjac[i * n + j];
                    if g != 0.0 {
                        complex_stamp(&mut stamps, &self.node_refs, i, j, Complex64::new(0.0, -g / omega));
                    }
                }
            }
        }
        // Force branches stay ideal in small-signal: same topology rows,
        // zero source perturbation.
        for (i, branch) in self.forces.refs().iter().enumerate() {
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
                    self.forces.refs()[force_idx].clone(),
                    self.forces.refs()[target_idx].clone(),
                    Complex64::new(0.0, -omega * l * sign),
                ));
            }
        }
        // Series-impedance terms in small-signal: the same real `−R` coupling
        // as DC/transient (`V = R·I` holds at every frequency).
        if self.kernel.has_force_current() {
            let terms = self.kernel.current_terms();
            let mut coeffs = vec![0.0; terms.len()];
            self.kernel
                .eval_force_current(&volts, &self.params, &self.state, &self.vars, &self.sim, &mut coeffs);
            for (&(force_idx, tp, tm), &r) in terms.iter().zip(&coeffs) {
                if r == 0.0 {
                    continue;
                }
                let Some((target_idx, sign)) = self.forces.branch_target(&self.kernel, tp, tm) else { continue };
                stamps.push(Stamp::Matrix(
                    self.forces.refs()[force_idx].clone(),
                    self.forces.refs()[target_idx].clone(),
                    Complex64::new(-r * sign, 0.0),
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
            for (i, branch) in self.forces.refs().iter().enumerate() {
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

    /// The `.disto` second-derivative kernel evaluated at the DC operating
    /// point, remapped to solver references (DISTO-03). `None` for a fully
    /// linear device (the kernel is not even compiled then).
    pub fn load_disto2(
        &mut self,
        dc_op: &DcAnalysisResult,
        context: &Context,
    ) -> Option<piperine_solver::abi::Disto2> {
        if !self.kernel.has_disto2() {
            return None;
        }
        self.sync_sim(context, Analysis::Ac);
        let refs = self.node_refs.clone();
        let volts = self.collect_volts(&|k| {
            refs.iter()
                .flatten()
                .find(|r| r.idx() == Some(k))
                .and_then(|r| dc_op.get(r.variable().clone()))
                .unwrap_or(0.0)
        });
        let num_pairs = self.kernel.disto2_pairs().len();
        let num_contribs = self.kernel.disto2_contribs().len();
        let mut values = vec![0.0; num_pairs * num_contribs];
        self.kernel
            .eval_disto2(&volts, &self.params, &self.state, &self.vars, &self.sim, &mut values);
        let pairs = self
            .kernel
            .disto2_pairs()
            .iter()
            .map(|&((a, b), (c, d))| {
                (
                    (self.terminal_ref(a), self.terminal_ref(b)),
                    (self.terminal_ref(c), self.terminal_ref(d)),
                )
            })
            .collect();
        let contribs = self
            .kernel
            .disto2_contribs()
            .iter()
            .map(|&(p, m)| (self.terminal_ref(p), self.terminal_ref(m)))
            .collect();
        Some(piperine_solver::abi::Disto2 {
            pairs,
            contribs,
            charge_start: self.kernel.disto2_charge_start(),
            values,
        })
    }

    /// The `.disto` third-derivative kernel evaluated at the DC operating
    /// point, remapped to solver references (DISTO-03). `None` when no
    /// contribution has a third derivative.
    pub fn load_disto3(
        &mut self,
        dc_op: &DcAnalysisResult,
        context: &Context,
    ) -> Option<piperine_solver::abi::Disto3> {
        if !self.kernel.has_disto3() {
            return None;
        }
        self.sync_sim(context, Analysis::Ac);
        let refs = self.node_refs.clone();
        let volts = self.collect_volts(&|k| {
            refs.iter()
                .flatten()
                .find(|r| r.idx() == Some(k))
                .and_then(|r| dc_op.get(r.variable().clone()))
                .unwrap_or(0.0)
        });
        let num_triples = self.kernel.disto3_triples().len();
        let num_contribs = self.kernel.disto2_contribs().len();
        let mut values = vec![0.0; num_triples * num_contribs];
        self.kernel
            .eval_disto3(&volts, &self.params, &self.state, &self.vars, &self.sim, &mut values);
        let triples = self
            .kernel
            .disto3_triples()
            .iter()
            .map(|&((a, b), (c, d), (e, f))| {
                (
                    (self.terminal_ref(a), self.terminal_ref(b)),
                    (self.terminal_ref(c), self.terminal_ref(d)),
                    (self.terminal_ref(e), self.terminal_ref(f)),
                )
            })
            .collect();
        let contribs = self
            .kernel
            .disto2_contribs()
            .iter()
            .map(|&(p, m)| (self.terminal_ref(p), self.terminal_ref(m)))
            .collect();
        Some(piperine_solver::abi::Disto3 {
            triples,
            contribs,
            charge_start: self.kernel.disto2_charge_start(),
            values,
        })
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
    pub fn terminal_node_ids(&self) -> &[crate::resolve::NodeId] {
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
