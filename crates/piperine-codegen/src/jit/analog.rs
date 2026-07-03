//! Analog kernel compilation: a flattened analog body to native
//! residual/Jacobian/charge/force/noise functions.
//!
//! One [`AnalogKernel`] is compiled per module and shared (`Arc`) across
//! instances; instances own their parameter values and operator state.
//!
//! Every compiled function uses the same ABI:
//!
//! ```c
//! void fn(const f64 *volts, const f64 *params, const f64 *state,
//!         const SimCtx *sim, f64 *out);
//! ```
//!
//! `volts[i]` is the voltage at terminal `i` ([`AnalogKernel::terminals`]
//! order: ports first, then module-internal nodes); `state[i]` is the current
//! value of runtime-state slot `i` (serviced by the device between steps).

use std::collections::HashMap;

use cranelift_codegen::ir::{types, AbiParam, InstBuilder, MemFlags, Signature, Value};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module};

use crate::ir::{CrossDir, IrExpr, IrModule, IrStateKind, NodeId, StateId, VarId};

use super::emit::AnalogEmitter;
use super::flatten::{
    AnalogFlattener, FlatAnalog, FlatContrib, FlatDiagnostic, FlatEvent, FlatEventTrigger,
    FlatForce,
};
use super::{math, CodegenError, SimCtx};

/// The uniform analog JIT function type.
///
/// ```c
/// void fn(const f64 *volts, const f64 *params, const f64 *state,
///         const f64 *vars, const SimCtx *sim, f64 *out);
/// ```
///
/// `vars[i]` is the current value of module-level persistent variable `i`
/// (the D2A bridge: analog bodies read digital register values through
/// this bank). Unused when the module has no module-level vars.
type AnalogFn = unsafe extern "C" fn(*const f64, *const f64, *const f64, *const f64, *const SimCtx, *mut f64);

/// A runtime-serviced operator state (`delay` / `slew` / `idt` / `idtmod`).
/// The device evaluates the config expressions once per instance (they must
/// be parameter-constant) and updates `state[id]` from the compiled
/// state-input function each accepted timestep.
#[derive(Debug, Clone)]
pub enum RuntimeState {
    Delay { delay: IrExpr },
    Slew { rise: IrExpr, fall: IrExpr },
    /// `idt`/`idtmod` accumulator: `state[id]` holds the integral up to the
    /// last accepted step (starting at `ic`); the kernel reads it as
    /// `state + dt·x` (implicit Euler). `modulus` wraps the accumulator
    /// (`idtmod`).
    Integrator { ic: IrExpr, modulus: Option<IrExpr> },
}

/// One runtime state slot: which `StateId` it services and how.
#[derive(Debug, Clone)]
pub struct RuntimeStateSpec {
    pub id: StateId,
    pub kind: RuntimeState,
}

/// How a compiled runtime event fires. Trigger *values* come from
/// [`AnalogKernel::eval_event_triggers`]; the device detects the transition
/// against the previous accepted value.
#[derive(Debug, Clone)]
pub enum CompiledTrigger {
    Initial,
    Cross(CrossDir),
    Above,
    /// Fires every `period` seconds (parameter-constant).
    Timer { period: IrExpr },
}

/// A compiled runtime analog event: its trigger plus the vars-bank slots its
/// actions write, in body order. Action values are rows of
/// [`AnalogKernel::eval_event_actions`], concatenated across events.
#[derive(Debug, Clone)]
pub struct CompiledEvent {
    pub trigger: CompiledTrigger,
    pub action_vars: Vec<VarId>,
}

/// A compiled analog device kernel.
pub struct AnalogKernel {
    name: String,
    /// Terminal order: module ports first, then internal analog nodes
    /// referenced by the body. `terminals[i]` is the node driving `volts[i]`.
    terminals: Vec<NodeId>,
    num_ports: usize,
    num_params: usize,
    num_state_slots: usize,
    /// Number of module-level persistent variable slots (the vars bank).
    num_vars: usize,
    num_forces: usize,
    num_noise: usize,
    /// Per-force branch terminals `(plus, minus)`.
    force_terminals: Vec<(NodeId, NodeId)>,
    /// Per-noise-source terminals `(plus, minus)`.
    noise_terminals: Vec<(NodeId, NodeId)>,
    runtime_states: Vec<RuntimeStateSpec>,
    events: Vec<CompiledEvent>,
    num_event_actions: usize,
    diagnostics: Vec<FlatDiagnostic>,
    residual: AnalogFn,
    jacobian: AnalogFn,
    /// Charge `Q(V)` and its Jacobian; `None` without reactive contributions.
    charge: Option<AnalogFn>,
    charge_jacobian: Option<AnalogFn>,
    /// Force source values `E_i(V)` and their Jacobian (`num_forces × n`
    /// row-major); `None` without forces.
    force: Option<AnalogFn>,
    force_jacobian: Option<AnalogFn>,
    /// Noise PSD per source; `None` without noise.
    noise: Option<AnalogFn>,
    /// Runtime-state input values (one per state slot); `None` without
    /// runtime states.
    state_inputs: Option<AnalogFn>,
    /// Event trigger values (one per event) and action values (one per
    /// action); `None` without runtime events.
    event_triggers: Option<AnalogFn>,
    event_actions: Option<AnalogFn>,
    /// Minimum `$bound_step` expression; `None` without bound steps.
    bound_step: Option<AnalogFn>,
    _jit: JITModule,
}

// The JITModule is frozen after `finalize_definitions`; the function pointers
// are immutable native code.
unsafe impl Send for AnalogKernel {}
unsafe impl Sync for AnalogKernel {}

impl AnalogKernel {
    /// Flatten and compile `module`'s analog body.
    pub fn compile(module: &IrModule) -> Result<Self, CodegenError> {
        let flat = AnalogFlattener::new(module).flatten()?;
        AnalogCompiler::new(module, flat)?.compile()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// All terminals: ports first, then internal nodes.
    pub fn terminals(&self) -> &[NodeId] {
        &self.terminals
    }

    /// How many leading terminals are module ports.
    pub fn num_ports(&self) -> usize {
        self.num_ports
    }

    pub fn num_terminals(&self) -> usize {
        self.terminals.len()
    }

    pub fn num_params(&self) -> usize {
        self.num_params
    }

    pub fn num_forces(&self) -> usize {
        self.num_forces
    }

    pub fn num_noise(&self) -> usize {
        self.num_noise
    }

    /// Branch terminals `(plus, minus)` per force row.
    pub fn force_terminals(&self) -> &[(NodeId, NodeId)] {
        &self.force_terminals
    }

    /// Terminals `(plus, minus)` per noise source.
    pub fn noise_terminals(&self) -> &[(NodeId, NodeId)] {
        &self.noise_terminals
    }

    /// Size of the runtime-state value array instances must provide.
    pub fn num_state_slots(&self) -> usize {
        self.num_state_slots
    }

    /// Number of module-level persistent variable slots (the vars bank).
    /// Instances must provide a slice of at least this many `f64` values.
    pub fn num_vars(&self) -> usize {
        self.num_vars
    }

    pub fn runtime_states(&self) -> &[RuntimeStateSpec] {
        &self.runtime_states
    }

    /// Runtime analog events, in body order.
    pub fn events(&self) -> &[CompiledEvent] {
        &self.events
    }

    /// Total number of event action rows (across all events).
    pub fn num_event_actions(&self) -> usize {
        self.num_event_actions
    }

    /// Diagnostics collected (not executed) from the analog body.
    pub fn diagnostics(&self) -> &[FlatDiagnostic] {
        &self.diagnostics
    }

    pub fn has_reactive(&self) -> bool {
        self.charge.is_some()
    }

    pub fn has_bound_step(&self) -> bool {
        self.bound_step.is_some()
    }

    fn call(f: AnalogFn, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        unsafe {
            f(
                volts.as_ptr(),
                params.as_ptr(),
                state.as_ptr(),
                vars.as_ptr(),
                sim as *const SimCtx,
                out.as_mut_ptr(),
            )
        }
    }

    /// Accumulate branch currents into `out[0..n]`. `out` must be pre-zeroed.
    pub fn eval_residual(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        Self::call(self.residual, volts, params, state, vars, sim, out);
    }

    /// Accumulate conductances into `out[0..n²]` (row-major). Pre-zeroed.
    pub fn eval_jacobian(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        Self::call(self.jacobian, volts, params, state, vars, sim, out);
    }

    /// Accumulate terminal charges into `out[0..n]`. No-op without reactive parts.
    pub fn eval_charge(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.charge {
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Accumulate `dQ/dV` into `out[0..n²]`. No-op without reactive parts.
    pub fn eval_charge_jacobian(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.charge_jacobian {
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Write each force's source value `E_i(V)` to `out[0..num_forces]`.
    pub fn eval_force(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.force {
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Write `dE_i/dV_j` to `out[0..num_forces·n]` (row-major).
    pub fn eval_force_jacobian(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.force_jacobian {
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Write each noise source's PSD to `out[0..num_noise]`.
    pub fn eval_noise(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.noise {
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Write each runtime state's input value to `out[0..num_state_slots]`.
    pub fn eval_state_inputs(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.state_inputs {
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Write each event's trigger value to `out[0..events.len()]`.
    pub fn eval_event_triggers(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.event_triggers {
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Write each event action's value to `out[0..num_event_actions]`.
    pub fn eval_event_actions(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.event_actions {
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// The device's `$bound_step` hint, or infinity.
    pub fn eval_bound_step(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx) -> f64 {
        match self.bound_step {
            Some(f) => {
                let mut out = [f64::INFINITY];
                Self::call(f, volts, params, state, vars, sim, &mut out);
                out[0]
            }
            None => f64::INFINITY,
        }
    }
}

// ─── Compiler ─────────────────────────────────────────────────────────────────

/// Builds every kernel function inside one Cranelift JIT module.
struct AnalogCompiler<'m> {
    module: &'m IrModule,
    flat: FlatAnalog,
    terminals: Vec<NodeId>,
    num_ports: usize,
    slot: HashMap<NodeId, usize>,
    jit: JITModule,
    math_ids: HashMap<&'static str, FuncId>,
    fb_ctx: FunctionBuilderContext,
}

impl<'m> AnalogCompiler<'m> {
    fn new(module: &'m IrModule, flat: FlatAnalog) -> Result<Self, CodegenError> {
        let mut jit_builder = JITBuilder::new(cranelift_module::default_libcall_names())
            .map_err(|e| CodegenError::Module(e.to_string()))?;
        for f in math::MATH_FNS {
            jit_builder.symbol(f.name, f.symbol);
        }
        let mut jit = JITModule::new(jit_builder);

        let mut math_ids = HashMap::new();
        for f in math::MATH_FNS {
            let mut sig = jit.make_signature();
            for _ in 0..f.arity {
                sig.params.push(AbiParam::new(types::F64));
            }
            sig.returns.push(AbiParam::new(types::F64));
            let id = jit
                .declare_function(f.name, Linkage::Import, &sig)
                .map_err(|e| CodegenError::Module(e.to_string()))?;
            math_ids.insert(f.name, id);
        }

        let (terminals, num_ports) = Self::terminal_order(module, &flat);
        let slot = terminals
            .iter()
            .enumerate()
            .map(|(i, &n)| (n, i))
            .collect();

        Ok(Self {
            module,
            flat,
            terminals,
            num_ports,
            slot,
            jit,
            math_ids,
            fb_ctx: FunctionBuilderContext::new(),
        })
    }

    /// Ports in declaration order, then every non-ground internal node the
    /// flattened body touches.
    fn terminal_order(module: &IrModule, flat: &FlatAnalog) -> (Vec<NodeId>, usize) {
        let mut terminals: Vec<NodeId> = module.ports.iter().map(|p| p.node).collect();
        let num_ports = terminals.len();
        let mut add = |node: NodeId| {
            if !node.is_ground() && !terminals.contains(&node) {
                terminals.push(node);
            }
        };
        let mut pairs = Vec::new();
        for expr in flat.exprs() {
            expr.collect_branches(&mut pairs);
        }
        for contrib in flat.resistive.iter().chain(&flat.charge) {
            pairs.push((contrib.plus, contrib.minus));
        }
        for force in &flat.forces {
            pairs.push((force.plus, force.minus));
        }
        for &(p, m) in &pairs {
            add(p);
            add(m);
        }
        for &(plus, minus, _) in &flat.noise {
            add(plus);
            add(minus);
        }
        (terminals, num_ports)
    }

    fn compile(mut self) -> Result<AnalogKernel, CodegenError> {
        let resistive = std::mem::take(&mut self.flat.resistive);
        let charge = std::mem::take(&mut self.flat.charge);
        let forces = std::mem::take(&mut self.flat.forces);
        let noise = std::mem::take(&mut self.flat.noise);
        let bound_steps = std::mem::take(&mut self.flat.bound_steps);
        let runtime_inputs = self.flat.runtime_states.clone();

        let residual_id = self.compile_residual("residual", &resistive)?;
        let jacobian_id = self.compile_jacobian("jacobian", &resistive)?;

        let (charge_id, charge_jac_id) = if charge.is_empty() {
            (None, None)
        } else {
            (
                Some(self.compile_residual("charge", &charge)?),
                Some(self.compile_jacobian("charge_jacobian", &charge)?),
            )
        };

        let (force_id, force_jac_id) = if forces.is_empty() {
            (None, None)
        } else {
            (
                Some(self.compile_rows("force", &forces.iter().map(|f| f.expr.clone()).collect::<Vec<_>>())?),
                Some(self.compile_force_jacobian("force_jacobian", &forces)?),
            )
        };

        let noise_id = if noise.is_empty() {
            None
        } else {
            let psds: Vec<IrExpr> = noise.iter().map(|(_, _, psd)| psd.clone()).collect();
            Some(self.compile_rows("noise", &psds)?)
        };

        let state_inputs_id = if runtime_inputs.is_empty() {
            None
        } else {
            // One row per state *slot*; slots without a runtime input write 0.
            let mut rows = vec![IrExpr::Real(0.0); self.module.symbols.num_states()];
            for (id, input) in &runtime_inputs {
                rows[id.0 as usize] = input.clone();
            }
            Some(self.compile_rows("state_inputs", &rows)?)
        };

        let events = std::mem::take(&mut self.flat.events);
        let (event_triggers_id, event_actions_id) = if events.is_empty() {
            (None, None)
        } else {
            // Trigger rows: the watched expression (0 for initial/timer —
            // those fire on lifecycle/clock, not on a value transition).
            let trigger_rows: Vec<IrExpr> = events
                .iter()
                .map(|e| match &e.trigger {
                    FlatEventTrigger::Cross { expr, .. } | FlatEventTrigger::Above { expr } => {
                        expr.clone()
                    }
                    FlatEventTrigger::Initial | FlatEventTrigger::Timer { .. } => IrExpr::Real(0.0),
                })
                .collect();
            let action_rows: Vec<IrExpr> = events
                .iter()
                .flat_map(|e| e.actions.iter().map(|a| a.value.clone()))
                .collect();
            let actions_id = if action_rows.is_empty() {
                None
            } else {
                Some(self.compile_rows("event_actions", &action_rows)?)
            };
            (Some(self.compile_rows("event_triggers", &trigger_rows)?), actions_id)
        };
        let num_event_actions = events.iter().map(|e| e.actions.len()).sum();
        let compiled_events: Vec<CompiledEvent> = events
            .iter()
            .map(|e| CompiledEvent {
                trigger: match &e.trigger {
                    FlatEventTrigger::Initial => CompiledTrigger::Initial,
                    FlatEventTrigger::Cross { dir, .. } => CompiledTrigger::Cross(*dir),
                    FlatEventTrigger::Above { .. } => CompiledTrigger::Above,
                    FlatEventTrigger::Timer { period } => {
                        CompiledTrigger::Timer { period: period.clone() }
                    }
                },
                action_vars: e.actions.iter().map(|a| a.var).collect(),
            })
            .collect();

        let bound_step_id = if bound_steps.is_empty() {
            None
        } else {
            let min = bound_steps
                .into_iter()
                .reduce(|a, b| IrExpr::MathCall("min".into(), vec![a, b]))
                .expect("non-empty");
            Some(self.compile_rows("bound_step", &[min])?)
        };

        self.jit
            .finalize_definitions()
            .map_err(|e| CodegenError::Module(e.to_string()))?;

        let get = |jit: &JITModule, id: FuncId| -> AnalogFn {
            // SAFETY: every function is compiled with the shared 5-pointer
            // signature built by `begin_fn`.
            unsafe { std::mem::transmute(jit.get_finalized_function(id)) }
        };

        let runtime_states = runtime_inputs
            .iter()
            .map(|(id, _)| {
                let kind = match &self.module.symbols.state(*id).kind {
                    IrStateKind::Delay { delay } => RuntimeState::Delay { delay: delay.clone() },
                    IrStateKind::Slew { rise, fall } => {
                        RuntimeState::Slew { rise: rise.clone(), fall: fall.clone() }
                    }
                    IrStateKind::Idt { ic } => {
                        RuntimeState::Integrator { ic: ic.clone(), modulus: None }
                    }
                    IrStateKind::IdtMod { ic, modulus } => RuntimeState::Integrator {
                        ic: ic.clone(),
                        modulus: Some(modulus.clone()),
                    },
                    other => {
                        return Err(CodegenError::Invalid(format!(
                            "`{}` is not a runtime-serviced state",
                            other.name()
                        )))
                    }
                };
                Ok(RuntimeStateSpec { id: *id, kind })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(AnalogKernel {
            name: self.module.name.clone(),
            num_ports: self.num_ports,
            num_params: self.module.symbols.num_params(),
            num_state_slots: self.module.symbols.num_states(),
            num_vars: self.module.symbols.vars().count(),
            num_forces: forces.len(),
            num_noise: noise.len(),
            force_terminals: forces.iter().map(|f| (f.plus, f.minus)).collect(),
            noise_terminals: noise.iter().map(|&(p, m, _)| (p, m)).collect(),
            runtime_states,
            events: compiled_events,
            num_event_actions,
            diagnostics: std::mem::take(&mut self.flat.diagnostics),
            residual: get(&self.jit, residual_id),
            jacobian: get(&self.jit, jacobian_id),
            charge: charge_id.map(|id| get(&self.jit, id)),
            charge_jacobian: charge_jac_id.map(|id| get(&self.jit, id)),
            force: force_id.map(|id| get(&self.jit, id)),
            force_jacobian: force_jac_id.map(|id| get(&self.jit, id)),
            noise: noise_id.map(|id| get(&self.jit, id)),
            state_inputs: state_inputs_id.map(|id| get(&self.jit, id)),
            event_triggers: event_triggers_id.map(|id| get(&self.jit, id)),
            event_actions: event_actions_id.map(|id| get(&self.jit, id)),
            bound_step: bound_step_id.map(|id| get(&self.jit, id)),
            terminals: std::mem::take(&mut self.terminals),
            _jit: self.jit,
        })
    }

    // ── Function skeletons ──

    /// Residual shape: `out[plus] += expr; out[minus] -= expr` per contribution.
    fn compile_residual(&mut self, name: &str, contribs: &[FlatContrib]) -> Result<FuncId, CodegenError> {
        let exprs: Vec<&IrExpr> = contribs.iter().map(|c| &c.expr).collect();
        self.build_fn(name, &exprs, |emitter, slot, out_ptr| {
            for contrib in contribs {
                let current = emitter.emit(&contrib.expr)?;
                if let Some(&p) = slot.get(&contrib.plus) {
                    emitter.accumulate_f64(current, out_ptr, p);
                }
                if let Some(&m) = slot.get(&contrib.minus) {
                    let negated = emitter.builder.ins().fneg(current);
                    emitter.accumulate_f64(negated, out_ptr, m);
                }
            }
            Ok(())
        })
    }

    /// Jacobian shape: `out[row·n + col] += ∂I/∂V` stamps per contribution.
    fn compile_jacobian(&mut self, name: &str, contribs: &[FlatContrib]) -> Result<FuncId, CodegenError> {
        let n = self.terminals.len();
        let exprs: Vec<&IrExpr> = contribs.iter().map(|c| &c.expr).collect();
        // Derivatives may reference branches beyond the primal expression's
        // (they don't today, but keep the precomputed set complete).
        self.build_fn(name, &exprs, |emitter, slot, out_ptr| {
            for contrib in contribs {
                let mut pairs = Vec::new();
                contrib.expr.collect_branches(&mut pairs);
                let plus = slot.get(&contrib.plus).copied();
                let minus = slot.get(&contrib.minus).copied();
                for (a, b) in pairs {
                    let derivative = contrib.expr.d_dv(a, b);
                    let g = emitter.emit(&derivative)?;
                    let col_a = slot.get(&a).copied();
                    let col_b = slot.get(&b).copied();
                    let stamp = |emitter: &mut AnalogEmitter, row: Option<usize>, col: Option<usize>, negate: bool| {
                        if let (Some(r), Some(c)) = (row, col) {
                            let v = if negate { emitter.builder.ins().fneg(g) } else { g };
                            emitter.accumulate_f64(v, out_ptr, r * n + c);
                        }
                    };
                    stamp(emitter, plus, col_a, false);
                    stamp(emitter, plus, col_b, true);
                    stamp(emitter, minus, col_a, true);
                    stamp(emitter, minus, col_b, false);
                }
            }
            Ok(())
        })
    }

    /// Row shape: `out[i] = expr_i`.
    fn compile_rows(&mut self, name: &str, rows: &[IrExpr]) -> Result<FuncId, CodegenError> {
        let exprs: Vec<&IrExpr> = rows.iter().collect();
        self.build_fn(name, &exprs, |emitter, _slot, out_ptr| {
            for (i, row) in rows.iter().enumerate() {
                let value = emitter.emit(row)?;
                emitter.store_f64(value, out_ptr, i);
            }
            Ok(())
        })
    }

    /// Force Jacobian shape: `out[i·n + j] = ∂E_i/∂V_j` per force row and
    /// terminal column.
    fn compile_force_jacobian(&mut self, name: &str, forces: &[FlatForce]) -> Result<FuncId, CodegenError> {
        let n = self.terminals.len();
        let exprs: Vec<&IrExpr> = forces.iter().map(|f| &f.expr).collect();
        self.build_fn(name, &exprs, |emitter, slot, out_ptr| {
            for (i, force) in forces.iter().enumerate() {
                let mut pairs = Vec::new();
                force.expr.collect_branches(&mut pairs);
                for (a, b) in pairs {
                    let derivative = force.expr.d_dv(a, b);
                    let g = emitter.emit(&derivative)?;
                    if let Some(&col) = slot.get(&a) {
                        emitter.accumulate_f64(g, out_ptr, i * n + col);
                    }
                    if let Some(&col) = slot.get(&b) {
                        let neg = emitter.builder.ins().fneg(g);
                        emitter.accumulate_f64(neg, out_ptr, i * n + col);
                    }
                }
            }
            Ok(())
        })
    }

    /// Build one function with the shared ABI: prepare parameter values and
    /// branch voltages, then let `body` emit the stores.
    fn build_fn(
        &mut self,
        name: &str,
        exprs: &[&IrExpr],
        body: impl FnOnce(&mut AnalogEmitter, &HashMap<NodeId, usize>, Value) -> Result<(), CodegenError>,
    ) -> Result<FuncId, CodegenError> {
        let ptr_ty = self.jit.target_config().pointer_type();
        let mut sig = Signature::new(self.jit.isa().default_call_conv());
        for _ in 0..6 {
            sig.params.push(AbiParam::new(ptr_ty));
        }

        let func_id = self
            .jit
            .declare_function(name, Linkage::Export, &sig)
            .map_err(|e| CodegenError::Module(e.to_string()))?;

        let mut ctx = self.jit.make_context();
        ctx.func.signature = sig;
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut self.fb_ctx);

        let math: HashMap<&'static str, cranelift_codegen::ir::FuncRef> = self
            .math_ids
            .iter()
            .map(|(&name, &id)| (name, self.jit.declare_func_in_func(id, builder.func)))
            .collect();

        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let volts_ptr = builder.block_params(entry)[0];
        let params_ptr = builder.block_params(entry)[1];
        let state_ptr = builder.block_params(entry)[2];
        let vars_ptr = builder.block_params(entry)[3];
        let sim_ptr = builder.block_params(entry)[4];
        let out_ptr = builder.block_params(entry)[5];

        // Parameter values, indexed by ParamId.
        let params: Vec<Value> = (0..self.module.symbols.num_params())
            .map(|i| {
                builder
                    .ins()
                    .load(types::F64, MemFlags::trusted(), params_ptr, (i * 8) as i32)
            })
            .collect();

        // Branch voltages for every pair read by any expression (including
        // through derivatives, whose branches are a subset of the primal's).
        let mut pairs = Vec::new();
        for expr in exprs {
            expr.collect_branches(&mut pairs);
        }
        let mut branch_voltages = HashMap::new();
        for (plus, minus) in pairs {
            let load = |builder: &mut FunctionBuilder, node: NodeId| match self.slot.get(&node) {
                Some(&i) => {
                    builder
                        .ins()
                        .load(types::F64, MemFlags::trusted(), volts_ptr, (i * 8) as i32)
                }
                None => builder.ins().f64const(0.0), // ground
            };
            let vp = load(&mut builder, plus);
            let vm = load(&mut builder, minus);
            let v = builder.ins().fsub(vp, vm);
            branch_voltages.insert((plus, minus), v);
        }

        let mut emitter = AnalogEmitter {
            builder: &mut builder,
            branch_voltages: &branch_voltages,
            params: &params,
            state_ptr,
            vars_ptr,
            sim_ptr,
            math: &math,
        };
        body(&mut emitter, &self.slot, out_ptr)?;

        builder.ins().return_(&[]);
        builder.finalize();

        self.jit
            .define_function(func_id, &mut ctx)
            .map_err(|e| CodegenError::Module(e.to_string()))?;
        Ok(func_id)
    }
}

impl FlatAnalog {
    /// Every expression in the flattened body (for terminal discovery).
    fn exprs(&self) -> impl Iterator<Item = &IrExpr> {
        self.resistive
            .iter()
            .chain(&self.charge)
            .map(|c| &c.expr)
            .chain(self.forces.iter().map(|f| &f.expr))
            .chain(self.bound_steps.iter())
            .chain(self.noise.iter().map(|(_, _, psd)| psd))
            .chain(self.runtime_states.iter().map(|(_, input)| input))
            .chain(self.events.iter().flat_map(FlatEvent::exprs))
    }
}
