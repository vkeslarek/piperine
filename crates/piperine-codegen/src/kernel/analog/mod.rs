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
//!
//! The optional analog capabilities (reactive/charge, forces, limits, noise,
//! `ac_stim`) are grouped into their own sub-structs (`kernel/analog/{name}.rs`)
//! held as `Option<Capability>` on [`AnalogKernel`] — presence of the
//! capability *is* `Some(_)`; a `has_<cap>()` query is
//! `self.<cap>.is_some()` (or an emptiness check on the capability's inner
//! data), never a separately-tracked bool.

mod ac_stim;
mod compile;
mod forces;
mod limits;
mod noise;
mod reactive;

use cranelift_jit::JITModule;

use crate::resolve::{CrossDir, LoweredBody, NodeId, StateId, VarId};

use crate::flatten::analog::{AnalogFlattener, FlatDiagnostic};
use crate::emit::abi::SimCtx;
use crate::error::CodegenError;
use compile::AnalogCompiler;

use ac_stim::AcStim;
use forces::Forces;
use limits::Limits;
use noise::Noise;
use reactive::Reactive;

use piperine_lang::parse::ast::Expr as PomExpr;

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

/// A compiled analog capability: JIT'd functions plus the terminal/branch
/// metadata its stamper needs. Presence of the capability *is* `Some(_)` on
/// the [`AnalogKernel`] field that holds it — see the module docs.
trait AnalogCapability {
    /// How many parallel rows this capability compiles (branches / sources /
    /// slots) — the count driving its `num_*()`-style accessor.
    fn count(&self) -> usize;
}

/// A runtime-serviced operator state (`delay` / `slew` / `transition` /
/// `idt` / `idtmod`).
/// The device evaluates the config expressions once per instance (they must
/// be parameter-constant) and updates `state[id]` from the compiled
/// state-input function each accepted timestep.
#[derive(Debug, Clone)]
pub enum RuntimeState {
    Delay { delay: PomExpr },
    Slew { rise: PomExpr, fall: PomExpr },
    /// `transition(x, td, rise, fall)` — piecewise-linear walk to the latest
    /// input; `td` delays the ramp start. `ttol` (5th language argument) is
    /// a breakpoint-placement hint and is intentionally not carried here.
    Transition { delay: PomExpr, rise: PomExpr, fall: PomExpr },
    /// `idt`/`idtmod` accumulator: `state[id]` holds the integral up to the
    /// last accepted step (starting at `ic`); the kernel reads it as
    /// `state + dt·x` (implicit Euler). `modulus` wraps the accumulator
    /// (`idtmod`).
    Integrator { ic: PomExpr, modulus: Option<PomExpr> },
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
    /// Fires every `period` seconds, first fire at `phase` (both
    /// parameter-constant). `phase = 0` reproduces `@timer(period)`.
    Timer { period: PomExpr, phase: PomExpr },
}

/// A compiled runtime analog event: its trigger plus the vars-bank slots its
/// actions write, in body order. Action values are rows of
/// [`AnalogKernel::eval_event_actions`], concatenated across events.
#[derive(Debug, Clone)]
pub struct CompiledEvent {
    pub trigger: CompiledTrigger,
    pub action_vars: Vec<VarId>,
}

/// The residual system every device has: terminals, parameter/state
/// bookkeeping, and the always-present residual/Jacobian/state-input
/// functions. Everything optional (reactive, forces, limits, noise,
/// `ac_stim`) lives beside this in [`AnalogKernel`] as `Option<Capability>`.
struct AnalogCore {
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
    /// Parameter names in `ParamId` order, for const-evaluating runtime
    /// state expressions (delay, slew, ic, …) at device creation.
    param_names: Vec<String>,
    /// Bitmask of params whose *presence* the body queries (`$param_given`
    /// — the optional `T?` machinery), bit `id.min(63)` like the instance
    /// given-mask. A live value write cannot flip presence, so a write to a
    /// presence-queried, not-given param is structural
    /// ([`Invalidation::Rebuild`](piperine_solver::abi::Invalidation)).
    presence_mask: u64,
    num_ports: usize,
    num_params: usize,
    num_state_slots: usize,
    /// Number of module-level persistent variable slots (the vars bank).
    num_vars: usize,
    residual: AnalogFn,
    jacobian: AnalogFn,
    /// Runtime-state input values (one per state slot); `None` without
    /// runtime states.
    state_inputs: Option<AnalogFn>,
    _jit: JITModule,
}

/// A compiled analog device kernel.
pub struct AnalogKernel {
    core: AnalogCore,
    /// `ddt`/charge capability; `has_reactive()` is `self.reactive.is_some()`.
    reactive: Option<Reactive>,
    /// `V`/`I`-source branch forces; `has_force_current()`/`has_force_flux()`/
    /// `has_force_ac_stim()` are emptiness checks on the inner fields.
    forces: Option<Forces>,
    /// `$limit` (pnjlim/fetlim) vold-slot bookkeeping.
    limits: Option<Limits>,
    /// Noise sources.
    noise: Option<Noise>,
    /// `ac_stim` contributions.
    ac_stim: Option<AcStim>,
    /// Resistive Jacobian with every `idt`/`idtmod` state load replaced by
    /// its input expression — the linear-operator view of the integrator.
    /// `load_ac` scales it by 1/(jω). `None` without integrator states.
    /// Independent of `reactive`: a body can `idt` a signal with no `ddt`
    /// contribution at all (and vice versa), so this is its own capability,
    /// not nested inside [`Reactive`].
    ac_idt_jacobian: Option<AnalogFn>,
    runtime_states: Vec<RuntimeStateSpec>,
    events: Vec<CompiledEvent>,
    num_event_actions: usize,
    /// Event trigger values (one per event) and action values (one per
    /// action); `None` without runtime events.
    event_triggers: Option<AnalogFn>,
    event_actions: Option<AnalogFn>,
    diagnostics: Vec<FlatDiagnostic>,
    /// Minimum `$bound_step` expression; `None` without bound steps.
    bound_step: Option<AnalogFn>,
    /// `.disto` 2nd-derivative kernel (DISTO-03): one compiled function per
    /// ordered branch pair `(j, k)` in `disto2_pairs` (same index), each
    /// writing `∂²(contrib)/∂V(j)∂V(k)` per contribution (resistive first,
    /// then charge) into its own `nc`-sized output slice. Empty when every
    /// contribution is linear (all second derivatives fold to zero). One
    /// function per pair (rather than one function unrolling every pair)
    /// keeps each Cranelift function small — a many-branch device unrolled
    /// into a single function overwhelmed Cranelift's own compilation.
    disto2: Vec<AnalogFn>,
    /// Ordered branch pairs `(j, k)` the disto2 kernel emits rows for, in
    /// `out` row order — only pairs with at least one nonzero row.
    disto2_pairs: Vec<((NodeId, NodeId), (NodeId, NodeId))>,
    /// Contribution terminals `(plus, minus)` in disto2 row order:
    /// resistive first, then charge (the split is `disto2_charge_start`).
    disto2_contribs: Vec<(NodeId, NodeId)>,
    /// Index in `disto2_contribs` where charge contributions begin.
    disto2_charge_start: usize,
    /// `.disto` 3rd-derivative kernel (DISTO-03): one compiled function per
    /// ordered branch triple in `disto3_triples` (same index), each writing
    /// `∂³(contrib)/∂V(j)∂V(k)∂V(l)` per contribution (same row order as
    /// `disto2_contribs`) into its own `nc`-sized output slice. Empty when
    /// no contribution has a third derivative.
    disto3: Vec<AnalogFn>,
    /// Ordered branch triples `(j, k, l)` the disto3 kernel emits rows
    /// for, in `out` row order — only triples with a nonzero row.
    disto3_triples: Vec<((NodeId, NodeId), (NodeId, NodeId), (NodeId, NodeId))>,
    /// `@initial` UIC seed terminal pairs and their (param-only) value rows.
    initial_condition_terminals: Vec<(NodeId, NodeId)>,
    initial_conditions: Option<AnalogFn>,
}

// The JITModule is frozen after `finalize_definitions`; the function pointers
// are immutable native code.
unsafe impl Send for AnalogKernel {}
unsafe impl Sync for AnalogKernel {}

/// Process-wide count of analog JIT compilations, for MD-18 enforcement
/// tests (a sweep must compile once, never once per point).
static COMPILE_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl AnalogKernel {
    /// Flatten and compile `module`'s analog body, including the `.disto`
    /// 2nd/3rd-derivative kernels.
    pub fn compile(module: &LoweredBody) -> Result<Self, CodegenError> {
        Self::compile_with_options(module, true)
    }

    /// Flatten and compile `module`'s analog body. `compile_disto` gates
    /// the `.disto` 2nd/3rd-derivative kernels (DISTO-03): every ordered
    /// controlling-branch combination compiles as its own small Cranelift
    /// function (`compile_disto2`/`compile_disto3`), and a many-branch
    /// device (several controlling terminals — a MOSFET, say) pays a real,
    /// non-trivial compile cost for those kernels. Callers that will never
    /// run `.disto` on this circuit (every analysis but `.disto` itself)
    /// pass `false` to skip that cost entirely — the host's `.disto` entry
    /// point is the only caller that passes `true` (`SimSession::build_circuit`).
    pub fn compile_with_options(module: &LoweredBody, compile_disto: bool) -> Result<Self, CodegenError> {
        COMPILE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let flat = AnalogFlattener::new(module).flatten()?;
        AnalogCompiler::new(module, flat)?.compile(compile_disto)
    }

    /// How many analog kernels this process has JIT-compiled so far.
    /// Deltas prove (or disprove) compile-once behavior across a sweep —
    /// meaningful only when nothing else compiles concurrently.
    pub fn compile_count() -> usize {
        COMPILE_COUNT.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn name(&self) -> &str {
        &self.core.name
    }

    /// All terminals: ports first, then internal nodes.
    pub fn terminals(&self) -> &[NodeId] {
        &self.core.terminals
    }

    /// `true` when terminal `i` is digital-domain (never an MNA unknown —
    /// see [`AnalogCore::digital_terminals`]).
    pub fn is_digital_terminal(&self, i: usize) -> bool {
        self.core.digital_terminals.get(i).copied().unwrap_or(false)
    }

    /// How many leading terminals are module ports.
    pub fn num_ports(&self) -> usize {
        self.core.num_ports
    }

    pub fn num_terminals(&self) -> usize {
        self.core.terminals.len()
    }

    pub fn num_params(&self) -> usize {
        self.core.num_params
    }

    /// Parameter names in `ParamId` order.
    pub fn param_names(&self) -> &[String] {
        &self.core.param_names
    }

    /// Whether the body queries param `idx`'s presence (`$param_given`).
    pub fn presence_queried(&self, idx: usize) -> bool {
        (self.core.presence_mask >> idx.min(63)) & 1 == 1
    }

    pub fn num_forces(&self) -> usize {
        self.forces.as_ref().map_or(0, AnalogCapability::count)
    }

    pub fn num_noise(&self) -> usize {
        self.noise.as_ref().map_or(0, AnalogCapability::count)
    }

    /// Number of `$limit` vold slots. They occupy state-bank slots
    /// `[num_state_slots − num_limits, num_state_slots)`.
    pub fn num_limits(&self) -> usize {
        self.limits.as_ref().map_or(0, AnalogCapability::count)
    }

    /// State-bank slot index of the first `$limit` vold slot.
    pub fn limit_base(&self) -> usize {
        self.core.num_state_slots - self.num_limits()
    }

    /// Compute each `$limit`'s updated value `vlim` at `volts` (using the
    /// current vold stored in `state`). The device writes these back into the
    /// state bank to seed the next Newton iteration.
    pub fn eval_limit_update(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(l) = &self.limits {
            self.check_input_lens(volts, params, state, vars);
            Self::call(l.update, volts, params, state, vars, sim, out);
        }
    }

    /// Initial `vold` per `$limit` slot (`vcrit`), for seeding the state bank
    /// at device creation (ngspice MODEINITJCT).
    pub fn eval_limit_seed(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(l) = &self.limits {
            self.check_input_lens(volts, params, state, vars);
            Self::call(l.seed, volts, params, state, vars, sim, out);
        }
    }

    /// Raw (unlimited) `vnew` per `$limit` slot.
    pub fn eval_limit_vnew(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(l) = &self.limits {
            self.check_input_lens(volts, params, state, vars);
            Self::call(l.vnew, volts, params, state, vars, sim, out);
        }
    }

    /// Junction branch (terminal slots) per `$limit`.
    pub fn limit_branches(&self) -> &[Option<(Option<usize>, Option<usize>)>] {
        self.limits.as_ref().map_or(&[], |l| &l.branches)
    }

    /// Branch terminals `(plus, minus)` per force row.
    pub fn force_terminals(&self) -> &[(NodeId, NodeId)] {
        self.forces.as_ref().map_or(&[], |f| &f.terminals)
    }

    /// Whether any force carries an inductor flux companion.
    pub fn has_force_flux(&self) -> bool {
        self.forces.as_ref().is_some_and(|f| f.flux.is_some())
    }

    /// Per flux term: `(force_idx, target_plus, target_minus)`.
    pub fn flux_terms(&self) -> &[(usize, NodeId, NodeId)] {
        self.forces.as_ref().map_or(&[], |f| &f.flux_meta)
    }

    /// Flux coefficients, one per term (in `flux_terms` order).
    pub fn eval_force_flux(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.forces.as_ref().and_then(|f| f.flux) {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Whether any force value carries a series-impedance (`R·I`) term.
    pub fn has_force_current(&self) -> bool {
        self.forces.as_ref().is_some_and(|f| f.current.is_some())
    }

    /// Per series-impedance term: `(force_idx, target_plus, target_minus)`.
    pub fn current_terms(&self) -> &[(usize, NodeId, NodeId)] {
        self.forces.as_ref().map_or(&[], |f| &f.current_meta)
    }

    /// Series-impedance coefficients, one per term (in `current_terms` order).
    pub fn eval_force_current(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.forces.as_ref().and_then(|f| f.current) {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Number of `@initial` UIC seeds.
    pub fn num_initial_conditions(&self) -> usize {
        self.initial_condition_terminals.len()
    }

    /// Branch terminals `(plus, minus)` per `@initial` seed.
    pub fn initial_condition_terminals(&self) -> &[(NodeId, NodeId)] {
        &self.initial_condition_terminals
    }

    /// Evaluate the `@initial` seed values (param-only) into `out`.
    pub fn eval_initial_conditions(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.initial_conditions {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Terminals `(plus, minus)` per noise source.
    pub fn noise_terminals(&self) -> &[(NodeId, NodeId)] {
        self.noise.as_ref().map_or(&[], |n| &n.terminals)
    }

    pub fn num_ac_stims(&self) -> usize {
        self.ac_stim.as_ref().map_or(0, AnalogCapability::count)
    }

    /// Terminals `(plus, minus)` per `ac_stim` source.
    pub fn ac_stim_terminals(&self) -> &[(NodeId, NodeId)] {
        self.ac_stim.as_ref().map_or(&[], |a| &a.terminals)
    }

    /// Size of the runtime-state value array instances must provide.
    pub fn num_state_slots(&self) -> usize {
        self.core.num_state_slots
    }

    /// Number of module-level persistent variable slots (the vars bank).
    /// Instances must provide a slice of at least this many `f64` values.
    pub fn num_vars(&self) -> usize {
        self.core.num_vars
    }

    /// The max `State`/`Var` id the compiled code actually loads, as
    /// `(params_read, state_read, vars_read)` (from [`FlatAnalog::read_bounds`]).
    /// A kernel with `state_read == 0 && vars_read == 0` reads no runtime
    /// state/vars, so its residual/charge can be recomputed outside the
    /// solver from terminal voltages alone (the common R/C/nonlinear case).
    pub fn read_bounds(&self) -> (usize, usize, usize) {
        self.core.read_bounds
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
        self.reactive.is_some()
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
        let (params_read, state_read, vars_read) = self.core.read_bounds;
        assert!(
            volts.len() >= self.core.terminals.len(),
            "kernel `{}`: {} volt(s) for {} terminal(s)",
            self.core.name, volts.len(), self.core.terminals.len()
        );
        assert!(
            params.len() >= params_read,
            "kernel `{}`: {} param(s), reads up to {}",
            self.core.name, params.len(), params_read
        );
        assert!(
            state.len() >= state_read,
            "kernel `{}`: {} state slot(s), reads up to {}",
            self.core.name, state.len(), state_read
        );
        assert!(
            vars.len() >= vars_read,
            "kernel `{}`: {} var(s), reads up to {} (module-level vars incl. any digital-read shadows)",
            self.core.name, vars.len(), vars_read
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
        Self::call(self.core.residual, volts, params, state, vars, sim, out);
    }

    /// Accumulate conductances into `out[0..n²]` (row-major). Pre-zeroed.
    pub fn eval_jacobian(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        self.check_input_lens(volts, params, state, vars);
        Self::call(self.core.jacobian, volts, params, state, vars, sim, out);
    }

    /// Accumulate terminal charges into `out[0..n]`. No-op without reactive parts.
    pub fn eval_charge(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(r) = &self.reactive {
            self.check_input_lens(volts, params, state, vars);
            Self::call(r.charge, volts, params, state, vars, sim, out);
        }
    }

    /// Accumulate `dQ/dV` into `out[0..n²]`. No-op without reactive parts.
    pub fn eval_charge_jacobian(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(r) = &self.reactive {
            self.check_input_lens(volts, params, state, vars);
            Self::call(r.charge_jacobian, volts, params, state, vars, sim, out);
        }
    }

    /// True when the body integrates at least one signal (`idt`/`idtmod`).
    pub fn has_ac_idt(&self) -> bool {
        self.ac_idt_jacobian.is_some()
    }

    /// Accumulate the AC integrator Jacobian (`∂res/∂V` with `idt(x)` read
    /// as `x`) into `out[0..n²]`. No-op without integrator states.
    pub fn eval_ac_idt_jacobian(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.ac_idt_jacobian {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    /// Write each force's source value `E_i(V)` to `out[0..num_forces]`.
    pub fn eval_force(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = &self.forces {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f.value, volts, params, state, vars, sim, out);
        }
    }

    /// Write `dE_i/dV_j` to `out[0..num_forces·n]` (row-major).
    pub fn eval_force_jacobian(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = &self.forces {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f.jacobian, volts, params, state, vars, sim, out);
        }
    }

    /// Write each `ac_stim` source's magnitude and phase (radians) to
    /// `mags`/`phases` (`num_ac_stims` each).
    #[allow(clippy::too_many_arguments)]
    pub fn eval_ac_stim(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, mags: &mut [f64], phases: &mut [f64]) {
        if let Some(a) = &self.ac_stim {
            self.check_input_lens(volts, params, state, vars);
            Self::call(a.mag, volts, params, state, vars, sim, mags);
            Self::call(a.phase, volts, params, state, vars, sim, phases);
        }
    }

    /// True when at least one force branch carries an AC stimulus.
    pub fn has_force_ac_stim(&self) -> bool {
        self.forces.as_ref().is_some_and(|f| f.ac_mag.is_some())
    }

    /// Write each force branch's AC stimulus magnitude and phase (radians) to
    #[allow(clippy::too_many_arguments)]
    /// `mags`/`phases` (`num_forces` each; 0 for branches without a stimulus).
    pub fn eval_force_ac_stim(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, mags: &mut [f64], phases: &mut [f64]) {
        if let Some((m, p)) = self.forces.as_ref().and_then(|f| f.ac_mag.zip(f.ac_phase)) {
            self.check_input_lens(volts, params, state, vars);
            Self::call(m, volts, params, state, vars, sim, mags);
            Self::call(p, volts, params, state, vars, sim, phases);
        }
    }

    /// Write each noise source's PSD at `sim.frequency` to
    /// `out[0..num_noise]`: the source's PSD expression, scaled by
    /// `(1 / f)^exponent` for flicker sources.
    pub fn eval_noise(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        let Some(n) = &self.noise else { return };
        self.check_input_lens(volts, params, state, vars);
        Self::call(n.source, volts, params, state, vars, sim, out);
        if let Some(f) = n.exponents {
            let mut exponents = vec![0.0; n.count()];
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
        if let Some(f) = self.core.state_inputs {
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

    /// Whether the device has any nonlinear contribution (a compiled
    /// `.disto` 2nd-derivative kernel).
    pub fn has_disto2(&self) -> bool {
        !self.disto2.is_empty()
    }

    /// Ordered branch pairs `(j, k)` the disto2 kernel emits, in `out` row
    /// order.
    pub fn disto2_pairs(&self) -> &[((NodeId, NodeId), (NodeId, NodeId))] {
        &self.disto2_pairs
    }

    /// Contribution terminals `(plus, minus)` in disto2 row order (resistive
    /// first, then charge).
    pub fn disto2_contribs(&self) -> &[(NodeId, NodeId)] {
        &self.disto2_contribs
    }

    /// Index in `disto2_contribs` where charge contributions begin.
    pub fn disto2_charge_start(&self) -> usize {
        self.disto2_charge_start
    }

    /// Write the `.disto` second derivatives: for each ordered branch pair
    /// `(j, k)` in [`AnalogKernel::disto2_pairs`], `∂²(contrib)/∂V(j)∂V(k)`
    /// per contribution at `out[pair·num_contribs + contrib]`. Only nonzero
    /// rows are stored — `out` must be pre-zeroed. No-op for a fully linear
    /// device.
    pub fn eval_disto2(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if self.disto2.is_empty() {
            return;
        }
        self.check_input_lens(volts, params, state, vars);
        let nc = self.disto2_contribs.len();
        for (i, &f) in self.disto2.iter().enumerate() {
            Self::call(f, volts, params, state, vars, sim, &mut out[i * nc..(i + 1) * nc]);
        }
    }

    /// Whether the device has a compiled `.disto` 3rd-derivative kernel.
    pub fn has_disto3(&self) -> bool {
        !self.disto3.is_empty()
    }

    /// Ordered branch triples `(j, k, l)` the disto3 kernel emits, in `out`
    /// row order.
    pub fn disto3_triples(&self) -> &[((NodeId, NodeId), (NodeId, NodeId), (NodeId, NodeId))] {
        &self.disto3_triples
    }

    /// Write the `.disto` third derivatives: for each ordered branch triple
    /// `(j, k, l)` in [`AnalogKernel::disto3_triples`],
    /// `∂³(contrib)/∂V(j)∂V(k)∂V(l)` per contribution at
    /// `out[triple·num_contribs + contrib]` (same contribution row order as
    /// [`AnalogKernel::disto2_contribs`]). Only nonzero rows are stored —
    /// `out` must be pre-zeroed. No-op without third derivatives.
    pub fn eval_disto3(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if self.disto3.is_empty() {
            return;
        }
        self.check_input_lens(volts, params, state, vars);
        let nc = self.disto2_contribs.len();
        for (i, &f) in self.disto3.iter().enumerate() {
            Self::call(f, volts, params, state, vars, sim, &mut out[i * nc..(i + 1) * nc]);
        }
    }
}
