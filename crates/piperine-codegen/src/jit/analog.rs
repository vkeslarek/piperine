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

use crate::ir::{CrossDir, Domain, IrExpr, LoweredBody, StateKind, NodeId, StateId, VarId};

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
    /// `digital_terminals[i]` is `true` when `terminals[i]` is a
    /// digital-domain node (a `Bit`/`Logic` port read by bare name inside
    /// this analog body, not through `V`/`I`). Such a terminal is never an
    /// MNA unknown — nothing in a `V`/`I` contribution stamps a row for
    /// it — so the device must not connect it to the netlist (it would be
    /// a structurally empty, singular row); its `volts[i]` is bridged in
    /// externally instead (`AnalogInstance::sync_vars`-adjacent).
    digital_terminals: Vec<bool>,
    /// Exclusive upper bounds of the `params`/`state`/`vars` bank slots
    /// the compiled code actually loads ([`FlatAnalog::read_bounds`]) —
    /// the eval-time bounds contract, distinct from the symbol-table
    /// counts used for bank *allocation*.
    read_bounds: (usize, usize, usize),
    num_ports: usize,
    num_params: usize,
    num_state_slots: usize,
    /// Number of `$limit` vold slots (appended to the state bank after the
    /// module's runtime-state slots).
    num_limits: usize,
    /// Per-`$limit` updated value `vlim` (one row per slot); `None` without
    /// any `$limit`. The device stores these back into the state bank.
    limit_update: Option<AnalogFn>,
    /// Per-`$limit` seed value `vcrit` (one row per slot); `None` without any
    /// `$limit`. Used to initialize the vold slots at device creation.
    limit_seed: Option<AnalogFn>,
    /// Per-`$limit` raw (unlimited) `vnew` value (one row per slot); `None`
    /// without any `$limit`. Used with `limit_branches` to detect the branch
    /// polarity when building the limited Norton linearization point.
    limit_vnew: Option<AnalogFn>,
    /// Per-`$limit` junction branch as terminal slot indices `(plus, minus)`
    /// (`None` slot = ground); the outer `None` means the branch was not
    /// uniquely identifiable and the raw voltage is used.
    limit_branches: Vec<Option<(Option<usize>, Option<usize>)>>,
    /// Number of module-level persistent variable slots (the vars bank).
    num_vars: usize,
    num_forces: usize,
    num_noise: usize,
    num_ac_stims: usize,
    /// Per-force branch terminals `(plus, minus)`.
    force_terminals: Vec<(NodeId, NodeId)>,
    /// Per-`ac_stim` branch terminals `(plus, minus)`.
    ac_stim_terminals: Vec<(NodeId, NodeId)>,
    /// Per-noise-source terminals `(plus, minus)`.
    noise_terminals: Vec<(NodeId, NodeId)>,
    /// Per-noise-source flicker exponents (one row per source, `0` for
    /// white noise): `S(f) = psd * (1 / f)^exponent` evaluated against
    /// `SimCtx.frequency`. `None` when every source is white.
    noise_exponents: Option<AnalogFn>,
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
    /// Per-force AC stimulus magnitude/phase rows (one entry per force; 0 for
    /// forces without an `ac_stim`). `None` when no force carries a stimulus.
    force_ac_mag: Option<AnalogFn>,
    force_ac_phase: Option<AnalogFn>,
    /// Noise PSD per source; `None` without noise.
    noise: Option<AnalogFn>,
    /// `ac_stim` magnitude and phase rows (one per source); `None` without
    /// AC stimuli.
    ac_stim_mag: Option<AnalogFn>,
    ac_stim_phase: Option<AnalogFn>,
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
    pub fn compile(module: &LoweredBody) -> Result<Self, CodegenError> {
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

    /// `true` when terminal `i` is digital-domain (never an MNA unknown —
    /// see [`AnalogKernel::digital_terminals`]).
    pub fn is_digital_terminal(&self, i: usize) -> bool {
        self.digital_terminals.get(i).copied().unwrap_or(false)
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

    /// Number of `$limit` vold slots. They occupy state-bank slots
    /// `[num_state_slots − num_limits, num_state_slots)`.
    pub fn num_limits(&self) -> usize {
        self.num_limits
    }

    /// State-bank slot index of the first `$limit` vold slot.
    pub fn limit_base(&self) -> usize {
        self.num_state_slots - self.num_limits
    }

    /// Compute each `$limit`'s updated value `vlim` at `volts` (using the
    /// current vold stored in `state`). The device writes these back into the
    /// state bank to seed the next Newton iteration.
    pub fn eval_limit_update(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.limit_update {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Initial `vold` per `$limit` slot (`vcrit`), for seeding the state bank
    /// at device creation (ngspice MODEINITJCT).
    pub fn eval_limit_seed(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.limit_seed {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Raw (unlimited) `vnew` per `$limit` slot.
    pub fn eval_limit_vnew(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.limit_vnew {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Junction branch (terminal slots) per `$limit`.
    pub fn limit_branches(&self) -> &[Option<(Option<usize>, Option<usize>)>] {
        &self.limit_branches
    }

    /// Branch terminals `(plus, minus)` per force row.
    pub fn force_terminals(&self) -> &[(NodeId, NodeId)] {
        &self.force_terminals
    }

    /// Terminals `(plus, minus)` per noise source.
    pub fn noise_terminals(&self) -> &[(NodeId, NodeId)] {
        &self.noise_terminals
    }

    pub fn num_ac_stims(&self) -> usize {
        self.num_ac_stims
    }

    /// Terminals `(plus, minus)` per `ac_stim` source.
    pub fn ac_stim_terminals(&self) -> &[(NodeId, NodeId)] {
        &self.ac_stim_terminals
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

    /// The max `State`/`Var` id the compiled code actually loads, as
    /// `(params_read, state_read, vars_read)` (from [`FlatAnalog::read_bounds`]).
    /// A kernel with `state_read == 0 && vars_read == 0` reads no runtime
    /// state/vars, so its residual/charge can be recomputed outside the
    /// solver from terminal voltages alone (the common R/C/nonlinear case).
    pub fn read_bounds(&self) -> (usize, usize, usize) {
        self.read_bounds
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

    /// Bounds contract for every `eval_*` entry point: the JIT'd code
    /// loads `volts[0..num_terminals]`, `params[0..num_params]`,
    /// `state[0..num_state_slots]`, and `vars[0..num_vars]` unchecked —
    /// an undersized slice is out-of-bounds native reads (a segfault at
    /// best). Check here, once, so a bad caller panics with a message
    /// instead (fail loud, GAPS).
    fn check_input_lens(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64]) {
        let (params_read, state_read, vars_read) = self.read_bounds;
        assert!(
            volts.len() >= self.terminals.len(),
            "kernel `{}`: {} volt(s) for {} terminal(s)",
            self.name, volts.len(), self.terminals.len()
        );
        assert!(
            params.len() >= params_read,
            "kernel `{}`: {} param(s), reads up to {}",
            self.name, params.len(), params_read
        );
        assert!(
            state.len() >= state_read,
            "kernel `{}`: {} state slot(s), reads up to {}",
            self.name, state.len(), state_read
        );
        assert!(
            vars.len() >= vars_read,
            "kernel `{}`: {} var(s), reads up to {} (module-level vars incl. any digital-read shadows)",
            self.name, vars.len(), vars_read
        );
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
        self.check_input_lens(volts, params, state, vars);
        Self::call(self.residual, volts, params, state, vars, sim, out);
    }

    /// Accumulate conductances into `out[0..n²]` (row-major). Pre-zeroed.
    pub fn eval_jacobian(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        self.check_input_lens(volts, params, state, vars);
        Self::call(self.jacobian, volts, params, state, vars, sim, out);
    }

    /// Accumulate terminal charges into `out[0..n]`. No-op without reactive parts.
    pub fn eval_charge(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.charge {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Accumulate `dQ/dV` into `out[0..n²]`. No-op without reactive parts.
    pub fn eval_charge_jacobian(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.charge_jacobian {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Write each force's source value `E_i(V)` to `out[0..num_forces]`.
    pub fn eval_force(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.force {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Write `dE_i/dV_j` to `out[0..num_forces·n]` (row-major).
    pub fn eval_force_jacobian(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.force_jacobian {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Write each `ac_stim` source's magnitude and phase (radians) to
    /// `mags`/`phases` (`num_ac_stims` each).
    pub fn eval_ac_stim(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, mags: &mut [f64], phases: &mut [f64]) {
        if let (Some(m), Some(p)) = (self.ac_stim_mag, self.ac_stim_phase) {
            self.check_input_lens(volts, params, state, vars);
            Self::call(m, volts, params, state, vars, sim, mags);
            Self::call(p, volts, params, state, vars, sim, phases);
        }
    }

    /// True when at least one force branch carries an AC stimulus.
    pub fn has_force_ac_stim(&self) -> bool {
        self.force_ac_mag.is_some()
    }

    /// Write each force branch's AC stimulus magnitude and phase (radians) to
    /// `mags`/`phases` (`num_forces` each; 0 for branches without a stimulus).
    pub fn eval_force_ac_stim(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, mags: &mut [f64], phases: &mut [f64]) {
        if let (Some(m), Some(p)) = (self.force_ac_mag, self.force_ac_phase) {
            self.check_input_lens(volts, params, state, vars);
            Self::call(m, volts, params, state, vars, sim, mags);
            Self::call(p, volts, params, state, vars, sim, phases);
        }
    }

    /// Write each noise source's PSD at `sim.frequency` to
    /// `out[0..num_noise]`: the source's PSD expression, scaled by
    /// `(1 / f)^exponent` for flicker sources.
    pub fn eval_noise(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.noise {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f, volts, params, state, vars, sim, out);
        }
        if let Some(f) = self.noise_exponents {
            let mut exponents = vec![0.0; self.num_noise];
            Self::call(f, volts, params, state, vars, sim, &mut exponents);
            for (psd, exponent) in out.iter_mut().zip(exponents) {
                if exponent != 0.0 && sim.frequency > 0.0 {
                    *psd *= sim.frequency.powf(-exponent);
                }
            }
        }
    }

    /// Write each runtime state's input value to `out[0..num_state_slots]`.
    pub fn eval_state_inputs(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.state_inputs {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Write each event's trigger value to `out[0..events.len()]`.
    pub fn eval_event_triggers(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.event_triggers {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Write each event action's value to `out[0..num_event_actions]`.
    pub fn eval_event_actions(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.event_actions {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// The device's `$bound_step` hint, or infinity.
    pub fn eval_bound_step(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx) -> f64 {
        match self.bound_step {
            Some(f) => {
                self.check_input_lens(volts, params, state, vars);
                let mut out = [f64::INFINITY];
                Self::call(f, volts, params, state, vars, sim, &mut out);
                out[0]
            }
            None => f64::INFINITY,
        }
    }
}

/// Collect the unique `$limit` expressions across every flattened expression,
/// in a stable order. Each becomes a `vold` slot appended to the state bank
/// (see `AnalogEmitter::emit_limit`). Uniqueness is by structural equality so
/// the same limit reused across contributions shares one slot.
fn collect_limits(flat: &FlatAnalog) -> Vec<IrExpr> {
    let mut limits: Vec<IrExpr> = Vec::new();
    let mut scan = |e: &IrExpr| {
        e.visit(&mut |node| {
            if matches!(node, IrExpr::Sim(crate::ir::SimQuery::Limit { .. }))
                && !limits.iter().any(|l| l == node)
            {
                limits.push(node.clone());
            }
        });
    };
    for c in &flat.resistive {
        scan(&c.expr);
    }
    for c in &flat.charge {
        scan(&c.expr);
    }
    for f in &flat.forces {
        scan(&f.expr);
    }
    limits
}

/// The junction branch `(plus, minus)` a `$limit` acts on: the single branch
/// read inside its first argument `vnew` (e.g. `V(pp,n)` or `type·V(bp,ep)`).
/// Used to feed the *limited* branch voltage into the Norton linearization
/// point (see `AnalogInstance::load_dc`). Returns `None` if `vnew` has no
/// unique branch, in which case the limiter falls back to the raw voltage.
fn limit_branch(limit: &IrExpr) -> Option<(NodeId, NodeId)> {
    let IrExpr::Sim(crate::ir::SimQuery::Limit { args, .. }) = limit else {
        return None;
    };
    let vnew = args.first()?;
    let mut found: Option<(NodeId, NodeId)> = None;
    let mut count = 0usize;
    vnew.visit(&mut |node| {
        if let IrExpr::Branch { plus, minus, .. } = node {
            count += 1;
            found = Some((*plus, *minus));
        }
    });
    if count == 1 { found } else { None }
}

// ─── Compiler ─────────────────────────────────────────────────────────────────

/// Builds every kernel function inside one Cranelift JIT module.
struct AnalogCompiler<'m> {
    module: &'m LoweredBody,
    flat: FlatAnalog,
    terminals: Vec<NodeId>,
    num_ports: usize,
    slot: HashMap<NodeId, usize>,
    jit: JITModule,
    math_ids: HashMap<&'static str, FuncId>,
    fb_ctx: FunctionBuilderContext,
    /// Unique `$limit` expressions, in slot order (see `AnalogEmitter::limits`).
    limits: Vec<IrExpr>,
    /// State-bank slot where `$limit` vold slots begin (= module state count).
    limit_base: usize,
}

impl<'m> AnalogCompiler<'m> {
    fn new(module: &'m LoweredBody, flat: FlatAnalog) -> Result<Self, CodegenError> {
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

        let limits = collect_limits(&flat);
        let limit_base = module.symbols.num_states();

        Ok(Self {
            module,
            flat,
            terminals,
            num_ports,
            slot,
            jit,
            math_ids,
            fb_ctx: FunctionBuilderContext::new(),
            limits,
            limit_base,
        })
    }

    /// Ports in declaration order, then every non-ground internal node the
    /// flattened body touches.
    fn terminal_order(module: &LoweredBody, flat: &FlatAnalog) -> (Vec<NodeId>, usize) {
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
        for &(plus, minus, _, _) in &flat.noise {
            add(plus);
            add(minus);
        }
        (terminals, num_ports)
    }

    fn compile(mut self) -> Result<AnalogKernel, CodegenError> {
        let read_bounds = self.flat.read_bounds();
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

        // AC drive attached to force branches (ideal AC voltage stimulus). One
        // row per force; branches without a stimulus contribute 0. Compiled
        // only when at least one force carries an `ac_stim`.
        let (force_ac_mag_id, force_ac_phase_id) = if forces.iter().any(|f| f.ac_stim.is_some()) {
            let mags: Vec<IrExpr> = forces
                .iter()
                .map(|f| f.ac_stim.as_ref().map_or(IrExpr::Real(0.0), |(m, _)| m.clone()))
                .collect();
            let phases: Vec<IrExpr> = forces
                .iter()
                .map(|f| f.ac_stim.as_ref().map_or(IrExpr::Real(0.0), |(_, p)| p.clone()))
                .collect();
            (
                Some(self.compile_rows("force_ac_mag", &mags)?),
                Some(self.compile_rows("force_ac_phase", &phases)?),
            )
        } else {
            (None, None)
        };

        let noise_id = if noise.is_empty() {
            None
        } else {
            let psds: Vec<IrExpr> = noise.iter().map(|(_, _, psd, _)| psd.clone()).collect();
            Some(self.compile_rows("noise", &psds)?)
        };
        // `$limit` update: one row per slot yielding the limited value `vlim`,
        // which the device writes back into the state bank each Newton
        // iteration to become the next iteration's `vold`.
        let limit_update_id = if self.limits.is_empty() {
            None
        } else {
            let rows = self.limits.clone();
            Some(self.compile_rows("limit_update", &rows)?)
        };
        // `$limit` seed: the critical voltage `vcrit` (arg 3) per slot. Junctions
        // start limiting from vcrit — ngspice's MODEINITJCT — so a diode begins
        // near turn-on instead of at 0 V (which floats the node to the supply
        // and makes vold crawl up chasing a runaway node).
        let limit_seed_id = if self.limits.is_empty() {
            None
        } else {
            let seeds: Vec<IrExpr> = self
                .limits
                .iter()
                .map(|l| match l {
                    IrExpr::Sim(crate::ir::SimQuery::Limit { args, .. }) if args.len() >= 4 => {
                        args[3].clone()
                    }
                    _ => IrExpr::Real(0.0),
                })
                .collect();
            Some(self.compile_rows("limit_seed", &seeds)?)
        };
        // Raw `vnew` (arg 0) per slot, for branch-polarity detection.
        let limit_vnew_id = if self.limits.is_empty() {
            None
        } else {
            let vnews: Vec<IrExpr> = self
                .limits
                .iter()
                .map(|l| match l {
                    IrExpr::Sim(crate::ir::SimQuery::Limit { args, .. }) if !args.is_empty() => {
                        args[0].clone()
                    }
                    _ => IrExpr::Real(0.0),
                })
                .collect();
            Some(self.compile_rows("limit_vnew", &vnews)?)
        };
        // Junction branch (as terminal slots) per limit.
        let limit_branches: Vec<Option<(Option<usize>, Option<usize>)>> = self
            .limits
            .iter()
            .map(|l| {
                limit_branch(l).map(|(p, m)| {
                    (self.slot.get(&p).copied(), self.slot.get(&m).copied())
                })
            })
            .collect();
        let ac_stims = std::mem::take(&mut self.flat.ac_stims);
        let (ac_stim_mag_id, ac_stim_phase_id) = if ac_stims.is_empty() {
            (None, None)
        } else {
            let mags: Vec<IrExpr> = ac_stims.iter().map(|s| s.mag.clone()).collect();
            let phases: Vec<IrExpr> = ac_stims.iter().map(|s| s.phase.clone()).collect();
            (
                Some(self.compile_rows("ac_stim_mag", &mags)?),
                Some(self.compile_rows("ac_stim_phase", &phases)?),
            )
        };
        // Flicker exponent rows (0 for white sources) — only compiled when
        // at least one source is flicker.
        let noise_exp_id = if noise.iter().any(|(_, _, _, exp)| exp.is_some()) {
            let rows: Vec<IrExpr> = noise
                .iter()
                .map(|(_, _, _, exp)| exp.clone().unwrap_or(IrExpr::Real(0.0)))
                .collect();
            Some(self.compile_rows("noise_exponents", &rows)?)
        } else {
            None
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
                    StateKind::Delay { delay } => RuntimeState::Delay { delay: delay.clone() },
                    StateKind::Slew { rise, fall } => {
                        RuntimeState::Slew { rise: rise.clone(), fall: fall.clone() }
                    }
                    StateKind::Idt { ic } => {
                        RuntimeState::Integrator { ic: ic.clone(), modulus: None }
                    }
                    StateKind::IdtMod { ic, modulus } => RuntimeState::Integrator {
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
            num_state_slots: self.module.symbols.num_states() + self.limits.len(),
            num_limits: self.limits.len(),
            limit_update: limit_update_id.map(|id| get(&self.jit, id)),
            limit_seed: limit_seed_id.map(|id| get(&self.jit, id)),
            limit_vnew: limit_vnew_id.map(|id| get(&self.jit, id)),
            limit_branches,
            num_vars: self.module.symbols.vars().count(),
            num_forces: forces.len(),
            num_noise: noise.len(),
            num_ac_stims: ac_stims.len(),
            force_terminals: forces.iter().map(|f| (f.plus, f.minus)).collect(),
            ac_stim_terminals: ac_stims.iter().map(|s| (s.plus, s.minus)).collect(),
            noise_terminals: noise.iter().map(|&(p, m, _, _)| (p, m)).collect(),
            noise_exponents: noise_exp_id.map(|id| get(&self.jit, id)),
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
            force_ac_mag: force_ac_mag_id.map(|id| get(&self.jit, id)),
            force_ac_phase: force_ac_phase_id.map(|id| get(&self.jit, id)),
            noise: noise_id.map(|id| get(&self.jit, id)),
            ac_stim_mag: ac_stim_mag_id.map(|id| get(&self.jit, id)),
            ac_stim_phase: ac_stim_phase_id.map(|id| get(&self.jit, id)),
            state_inputs: state_inputs_id.map(|id| get(&self.jit, id)),
            event_triggers: event_triggers_id.map(|id| get(&self.jit, id)),
            event_actions: event_actions_id.map(|id| get(&self.jit, id)),
            bound_step: bound_step_id.map(|id| get(&self.jit, id)),
            digital_terminals: self
                .terminals
                .iter()
                .map(|&id| self.module.symbols.node(id).domain == Domain::Digital)
                .collect(),
            read_bounds,
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
            limits: &self.limits,
            limit_base: self.limit_base,
            cse: HashMap::new(),
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
            .chain(self.ac_stims.iter().flat_map(|s| [&s.mag, &s.phase]))
            .chain(self.bound_steps.iter())
            .chain(self.noise.iter().map(|(_, _, psd, _)| psd))
            .chain(self.runtime_states.iter().map(|(_, input)| input))
            .chain(self.events.iter().flat_map(FlatEvent::exprs))
    }

    /// How far into the `params`/`state`/`vars` banks the compiled code
    /// actually reads: `(params, state, vars)` as exclusive upper bounds
    /// (max referenced id + 1). The symbol-table counts overcount — a
    /// behavior-local `var` is inlined away and a `ddt` state is
    /// substituted out of the residual — so the eval-time bounds contract
    /// ([`AnalogKernel::check_input_lens`]) uses these instead.
    fn read_bounds(&self) -> (usize, usize, usize) {
        let (mut params, mut state, mut vars) = (0usize, 0usize, 0usize);
        for expr in self.exprs() {
            expr.visit(&mut |e| match e {
                IrExpr::Param(id) => params = params.max(id.0 as usize + 1),
                IrExpr::State(id) => state = state.max(id.0 as usize + 1),
                IrExpr::Var(id) => vars = vars.max(id.0 as usize + 1),
                _ => {}
            });
        }
        (params, state, vars)
    }
}
