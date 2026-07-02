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

use crate::ir::Analysis;
use crate::jit::analog::{AnalogKernel, RuntimeState};
use crate::jit::{CodegenError, SimCtx};

/// A runtime-serviced analog operator: updated once per accepted timestep,
/// its output read by the kernel through the state array.
enum Operator {
    /// `delay(x, t)` — a `(time, value)` history ring.
    Delay { slot: usize, delay: f64, history: VecDeque<(f64, f64)> },
    /// `slew(x, rise, fall)` — rate-limited follower.
    Slew { slot: usize, rise: f64, fall: f64, output: f64, time: f64 },
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
        }
    }

    fn slot(&self) -> usize {
        match self {
            Operator::Delay { slot, .. } | Operator::Slew { slot, .. } => *slot,
        }
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
            .map(|t| {
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
                })
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;

        let mut sim = SimCtx::default();
        sim.param_given_mask = param_given_mask;
        let n = kernel.num_terminals();
        Ok(Self {
            state: vec![0.0; kernel.num_state_slots()],
            kernel,
            node_refs,
            force_refs,
            noise_refs,
            params,
            sim,
            operators,
            last_volts: vec![0.0; n],
        })
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
            .eval_residual(volts, &self.params, &self.state, &self.sim, &mut res);
        self.kernel
            .eval_jacobian(volts, &self.params, &self.state, &self.sim, &mut jac);
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
            .eval_force(volts, &self.params, &self.state, &self.sim, &mut e);
        self.kernel
            .eval_force_jacobian(volts, &self.params, &self.state, &self.sim, &mut de);

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
                .eval_charge_jacobian(&volts, &self.params, &self.state, &self.sim, &mut qjac);
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
                .eval_charge_jacobian(&volts, &self.params, &self.state, &self.sim, &mut qjac);
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
            .eval_noise(&volts, &self.params, &self.state, &self.sim, &mut psd);

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
                .eval_state_inputs(&volts, &self.params, &self.state, &self.sim, &mut inputs);
            for op in &mut self.operators {
                let slot = op.slot();
                self.state[slot] = op.accept(ctx.time, inputs[slot]);
            }
        }
        self.last_volts = volts;
    }

    pub fn bound_step_hint(&self) -> f64 {
        if !self.kernel.has_bound_step() {
            return f64::INFINITY;
        }
        self.kernel
            .eval_bound_step(&self.last_volts, &self.params, &self.state, &self.sim)
    }

    fn sync_sim(&mut self, context: &Context, analysis: Analysis) {
        self.sim.temperature = context.temperature;
        self.sim.gmin = context.gmin.into();
        self.sim.current_analysis = super::analysis_code(analysis);
    }
}

fn dc_op_voltage(reference: &AnalogReference, dc_point: &DcAnalysisResult) -> Option<f64> {
    dc_point.get(reference.variable().clone())
}
