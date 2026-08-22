//! [`AnalogKernel`] — one compiled analog body per module, shared (`Arc`)
//! across instances — together with the types it is made of: the uniform
//! `AnalogFn` JIT signature, the `AnalogCapability` contract each optional
//! capability implements, the `AnalogCore` always-present half, and the
//! runtime-state / compiled-event descriptors instances service between steps.

use crate::emit::OwnedJit;

use crate::resolve::{CrossDir, LoweredBody, NodeId, StateId, VarId};

use crate::flatten::analog::{AnalogFlattener, FlatDiagnostic};
use crate::emit::abi::SimCtx;
use crate::error::CodegenError;
use super::compile::AnalogCompiler;

use super::ac_stim::AcStim;
use super::constraints::Constraints;
use super::coverage::Coverage;
use super::forces::Forces;
use super::limexp::Limexp;
use super::limits::Limits;
use super::noise::Noise;
use super::reactive::Reactive;

use piperine_lang::ast::Expr as PomExpr;


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
pub(super) type AnalogFn = unsafe extern "C" fn(*const f64, *const f64, *const f64, *const f64, *const SimCtx, *mut f64);

/// The parameter-indexed `∂F/∂p` kernel ABI (DVG-02): one extra `usize`
/// argument selects the parameter, and the function writes
/// `∂(contrib.expr)/∂param` per resistive contribution into `out` — **one
/// function regardless of parameter count**, the per-index design the
/// `.disto` per-combination wall (`TryFromIntError` on MOS2/MOS3 with ~60
/// model-card params) proved untenable.
///
/// ```c
/// void fn(const f64 *volts, const f64 *params, const f64 *state,
///         const f64 *vars, const SimCtx *sim, usize param_idx, f64 *out);
/// ```
///
/// `out` must be pre-zeroed and hold one `f64` per resistive contribution
/// (see [`AnalogKernel::param_jacobian_contribs`]).
pub(super) type AnalogParamFn = unsafe extern "C" fn(*const f64, *const f64, *const f64, *const f64, *const SimCtx, usize, *mut f64);

/// An ordered branch (node pair) in the analog netlist.
pub type Branch = (NodeId, NodeId);
/// An ordered pair of branches for second-derivative disto.
pub type Disto2Pair = (Branch, Branch);
/// An ordered triple of branches for third-derivative disto.
pub type Disto3Triple = (Branch, Branch, Branch);

/// A compiled analog capability: JIT'd functions plus the terminal/branch
/// metadata its stamper needs. Presence of the capability *is* `Some(_)` on
/// the [`AnalogKernel`] field that holds it — see the module docs.
pub(super) trait AnalogCapability {
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
    /// (`idtmod`); `offset` shifts the wrap window to `[offset,
    /// offset+modulus)` (§4.5.5, LRMW2-30) — `0` (a no-op shift) for a
    /// plain `idt` or an `idtmod` with no `offset` argument.
    Integrator { ic: PomExpr, modulus: Option<PomExpr>, offset: PomExpr },
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
    /// `initial_step`/`initial_step("dc", ...)` — the analysis-name list
    /// (empty = every analysis), LRM §5.10.2. Fired once by
    /// `AnalogInstance::fire_initial_events`.
    Initial(Vec<String>),
    /// `final_step`/`final_step("dc", ...)` — LRM §5.10.2, LRMW2-08. Fired
    /// once by `AnalogInstance::fire_final_events`, at
    /// `Element::finish_analysis` time (after the last accepted point).
    Final(Vec<String>),
    Cross(CrossDir),
    Above,
    /// Fires every `period` seconds, first fire at `phase` (both
    /// parameter-constant). `phase = 0` reproduces `@timer(period)`.
    Timer { period: PomExpr, phase: Box<PomExpr> },
    /// §5.10.3.4 — `delta`/`time_tol`/`expr_tol`/`enable` are all
    /// parameter-constant (evaluated once, same treatment as `Timer`'s
    /// `period`/`phase`); the per-step trigger value itself comes from
    /// `eval_event_triggers` like `Cross`/`Above`.
    AbsDelta { delta: PomExpr, time_tol: Box<PomExpr>, expr_tol: Box<PomExpr>, enable: Box<PomExpr> },
    /// A bare signal/named-event sensitivity (§5.6/§5.10.4) — the per-step
    /// trigger value comes from `eval_event_triggers` like `Cross`/`Above`;
    /// fires on any change (`piperine_ir::events::Trigger::AnyChange`).
    Named,
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
pub(super) struct AnalogCore {
    pub(super) name: String,
    /// Terminal order: module ports first, then internal analog nodes
    /// referenced by the body. `terminals[i]` is the node driving `volts[i]`.
    pub(super) terminals: Vec<NodeId>,
    /// Source-level names parallel to [`terminals`](Self::terminals), looked
    /// up from the symbol table at compile time. Surfaced through
    /// [`AnalogKernel::terminal_name`] so the introspection bridge (ABI-27)
    /// returns names, not positional indices.
    pub(super) terminal_names: Vec<String>,
    /// Per-state-slot introspection names (ABI-47): one entry per slot in
    /// `[0, num_state_slots)`. Runtime-serviced operators carry their kind
    /// name plus slot id (`"ddt[3]"`, `"delay[5]"`, …); trailing `$limit`
    /// vold slots are `"vold[k]"`. Surfaced through
    /// [`AnalogKernel::state_slot_names`].
    pub(super) state_slot_names: Vec<String>,
    /// `digital_terminals[i]` is `true` when `terminals[i]` is a
    /// digital-domain node (a `Bit`/`Logic` port read by bare name inside
    /// this analog body, not through `V`/`I`). Such a terminal is never an
    /// MNA unknown — nothing in a `V`/`I` contribution stamps a row for
    /// it — so the device must not connect it to the netlist (it would be
    /// a structurally empty, singular row); its `volts[i]` is bridged in
    /// externally instead (`AnalogInstance::sync_vars`-adjacent).
    pub(super) digital_terminals: Vec<bool>,
    /// Exclusive upper bounds of the `params`/`state`/`vars` bank slots
    /// the compiled code actually loads ([`FlatAnalog::read_bounds`]) —
    /// the eval-time bounds contract, distinct from the symbol-table
    /// counts used for bank *allocation*.
    pub(super) read_bounds: (usize, usize, usize),
    /// Parameter names in `ParamId` order, for const-evaluating runtime
    /// state expressions (delay, slew, ic, …) at device creation.
    pub(super) param_names: Vec<String>,
    /// Bitmask of params whose *presence* the body queries (`$param_given`
    /// — the optional `T?` machinery), bit `id.min(63)` like the instance
    /// given-mask. A live value write cannot flip presence, so a write to a
    /// presence-queried, not-given param is structural
    /// ([`Invalidation::Rebuild`](piperine_solver::abi::Invalidation)).
    pub(super) presence_mask: u64,
    pub(super) num_ports: usize,
    pub(super) num_params: usize,
    pub(super) num_state_slots: usize,
    /// Number of module-level persistent variable slots (the vars bank).
    pub(super) num_vars: usize,
    /// Variable initializers (Annex A.2.2.1): `(bank slot, init expr)`
    /// for every module var declared with `= constant_expression`. The
    /// instance seeds its vars bank from these at construction, after
    /// evaluating each expression against the instance's own params (the
    /// kernel is `Arc`-shared; init *values* are per-instance).
    pub(super) var_inits: Vec<(usize, piperine_lang::ast::Expr)>,
    /// Every `ddt(x, abstol|nature)` call's tolerance expression that
    /// carried one (§4.5.3, LRMW2-37) — evaluated once per instance
    /// (`AnalogInstance::new`, param-const-eval like `Idt`'s `ic`) and
    /// folded (minimum) into the LTE timestep tolerance
    /// (`AnalogInstance::suggest_transient_step`). Empty for a body with no
    /// `ddt` call that named a tolerance.
    pub(super) ddt_abstols: Vec<PomExpr>,
    pub(super) residual: AnalogFn,
    pub(super) jacobian: AnalogFn,
    /// Runtime-state input values (one per state slot); `None` without
    /// runtime states.
    pub(super) state_inputs: Option<AnalogFn>,
    /// `idt(x, ic, assert)`'s `assert` value (one per state slot,
    /// §4.5.4/LRMW2-29); `None` when no `idt` call in this body named an
    /// `assert` argument.
    pub(super) assert_inputs: Option<AnalogFn>,
    pub(super) _jit: OwnedJit,
}

/// A compiled analog device kernel.
pub struct AnalogKernel {
    pub(super) core: AnalogCore,
    /// `ddt`/charge capability; `has_reactive()` is `self.reactive.is_some()`.
    pub(super) reactive: Option<Reactive>,
    /// `V`/`I`-source branch forces; `has_force_current()`/`has_force_flux()`/
    /// `has_force_ac_stim()` are emptiness checks on the inner fields.
    pub(super) forces: Option<Forces>,
    /// `$limit` (pnjlim/fetlim) vold-slot bookkeeping.
    pub(super) limits: Option<Limits>,
    /// `limexp()` output-history state (LRM §4.5.13).
    pub(super) limexp: Option<Limexp>,
    /// Noise sources.
    pub(super) noise: Option<Noise>,
    /// `ac_stim` contributions.
    pub(super) ac_stim: Option<AcStim>,
    /// Constraint margin functions (DVC-10): one per declared **pointwise**
    /// margin; `has_margins()` is `self.constraints.is_some()`.
    pub(super) constraints: Option<Constraints>,
    /// `cover` bin-mapper functions (T33, DVT-40): one per declared
    /// `cover` point, plus the module's `cover cross` catalog (no
    /// compiled function of its own). `has_coverage()` is
    /// `self.coverage.is_some()`.
    pub(super) coverage: Option<Coverage>,
    /// Resistive Jacobian with every `idt`/`idtmod` state load replaced by
    /// its input expression — the linear-operator view of the integrator.
    /// `load_ac` scales it by 1/(jω). `None` without integrator states.
    /// Independent of `reactive`: a body can `idt` a signal with no `ddt`
    /// contribution at all (and vice versa), so this is its own capability,
    /// not nested inside [`Reactive`].
    pub(super) ac_idt_jacobian: Option<AnalogFn>,
    pub(super) runtime_states: Vec<RuntimeStateSpec>,
    pub(super) events: Vec<CompiledEvent>,
    pub(super) num_event_actions: usize,
    /// Event trigger values (one per event) and action values (one per
    /// action); `None` without runtime events.
    pub(super) event_triggers: Option<AnalogFn>,
    pub(super) event_actions: Option<AnalogFn>,
    pub(super) diagnostics: Vec<FlatDiagnostic>,
    /// Each `diagnostics[i]`'s format string, interned into the process-wide
    /// `FormatPool` exactly once at compile time (`AnalogCompiler`'s final
    /// assembly) — the trampoline call at runtime passes this id, never a
    /// string, per call (design.md P0.1: "no per-call allocation").
    pub(super) diagnostic_fmt_ids: Vec<u32>,
    /// Minimum `$bound_step` expression; `None` without bound steps.
    pub(super) bound_step: Option<AnalogFn>,
    /// `$discontinuity` fired-this-evaluation flag (LRMC-C7-11, T42);
    /// `None` without any `$discontinuity` call.
    pub(super) discontinuity: Option<AnalogFn>,
    /// T27/LRMW2-27 (§9.7.3): `$fatal(...)` fired-this-evaluation flag —
    /// folded via the same `max`-of-guards pattern as `discontinuity`
    /// across every `$fatal(...)` call site, so any one whose guard is
    /// true this Newton iteration sets it. Unlike `diagnostics`
    /// (deferred to `accept_timestep`), the device checks this every
    /// iteration (`AnalogInstance::check_fatal`) — `$fatal` terminates
    /// immediately, not waiting for the step to be accepted.
    /// `None` without any `$fatal` call.
    pub(super) fatal: Option<AnalogFn>,
    /// Every `$fatal` call site's format string, interned the same way
    /// `diagnostic_fmt_ids` is. When [`Self::fatal`] fires, every one of
    /// these is emitted — the aggregate flag does not distinguish *which*
    /// call site's guard was true (the same approximation `discontinuity`
    /// already makes for `$discontinuity`'s `degree`).
    pub(super) fatal_fmt_ids: Vec<u32>,
    /// `.disto` 2nd-derivative kernel (DISTO-03): one compiled function per
    /// ordered branch pair `(j, k)` in `disto2_pairs` (same index), each
    /// writing `∂²(contrib)/∂V(j)∂V(k)` per contribution (resistive first,
    /// then charge) into its own `nc`-sized output slice. Empty when every
    /// contribution is linear (all second derivatives fold to zero). One
    /// function per pair (rather than one function unrolling every pair)
    /// keeps each Cranelift function small — a many-branch device unrolled
    /// into a single function overwhelmed Cranelift's own compilation.
    pub(super) disto2: Vec<AnalogFn>,
    /// Ordered branch pairs `(j, k)` the disto2 kernel emits rows for, in
    /// `out` row order — only pairs with at least one nonzero row.
    pub(super) disto2_pairs: Vec<Disto2Pair>,
    /// Contribution terminals `(plus, minus)` in disto2 row order:
    /// resistive first, then charge (the split is `disto2_charge_start`).
    pub(super) disto2_contribs: Vec<(NodeId, NodeId)>,
    /// Index in `disto2_contribs` where charge contributions begin.
    pub(super) disto2_charge_start: usize,
    /// `.disto` 3rd-derivative kernel (DISTO-03): one compiled function per
    /// ordered branch triple in `disto3_triples` (same index), each writing
    /// `∂³(contrib)/∂V(j)∂V(k)∂V(l)` per contribution (same row order as
    /// `disto2_contribs`) into its own `nc`-sized output slice. Empty when
    /// no contribution has a third derivative.
    pub(super) disto3: Vec<AnalogFn>,
    /// Ordered branch triples `(j, k, l)` the disto3 kernel emits rows
    /// for, in `out` row order — only triples with a nonzero row.
    pub(super) disto3_triples: Vec<Disto3Triple>,
    /// `@initial` UIC seed terminal pairs and their (param-only) value rows.
    pub(super) initial_condition_terminals: Vec<(NodeId, NodeId)>,
    pub(super) initial_conditions: Option<AnalogFn>,
    /// Per-seed bitmask of param ids the seed expression references
    /// (NGP-10). Bit `i` set means the seed reads param `i`. A seed fires
    /// at instance time only when at least one of its backing params was
    /// actually overridden at the instance site — a default `ic = 0.0`
    /// would otherwise clamp every transient to zero. The mask is `0` for
    /// a constant seed (no param references), which then fires unconditionally.
    pub(super) initial_condition_param_deps: Vec<u64>,
    /// Operating-point variable evaluation (ABI-30): one row per non-shadow
    /// module `var`, reading the same `state`/`vars` banks the residual
    /// uses, evaluated against the final post-solve voltages. `None` when
    /// the module declares no analog vars (zero overhead — no function
    /// compiled, the eval call is a no-op).
    pub(super) opvars: Option<AnalogFn>,
    pub(super) opvar_names: Vec<String>,
    /// Source-level names of the persistent-var bank slots (PIA-06), aligned
    /// with `0..num_vars`. Empty for slots without a source name. The
    /// observable catalog reads this to match an `@name`/`@kind` sidecar
    /// entry to its var-bank slot; absent `@name` keeps the positional
    /// `var[k]` fallback (PIA-08, no regression).
    pub(super) var_names: Vec<String>,
    /// The `∂F/∂p` parameter-indexed kernel (DVG-02): `None` when the body
    /// declares no differentiable parameter (zero cost — no function
    /// compiled). One function regardless of parameter count; the parameter
    /// index selects which derivative to compute.
    pub(super) param_jacobian: Option<AnalogParamFn>,
    /// Resistive contribution terminals `(plus, minus)` in `out` row order —
    /// the stamp targets the ABI method reads to turn each scalar the kernel
    /// writes into a residual-shaped `∂F/∂param`. Empty when
    /// [`param_jacobian`](Self::param_jacobian) is `None`.
    pub(super) param_contribs: Vec<(NodeId, NodeId)>,
    /// The parameter indices (into [`param_names`](AnalogCore::param_names))
    /// the kernel differentiates analytically — `Real`-typed restamp-class
    /// parameters (admissibility, DVG-03). A request for any other index
    /// writes nothing and the driver fails loud.
    pub(super) param_indices: Vec<usize>,
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
    /// point is the only caller that passes `true` (the host's
    /// `SessionBuilder::disto(true)`).
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

    /// The source-level name of terminal `i`, in
    /// [`terminals`](Self::terminals) order (ABI-27). Panics on out-of-range
    /// `i` — every caller is internal and bounds the index via
    /// [`num_terminals`](Self::num_terminals) first.
    pub fn terminal_name(&self, i: usize) -> &str {
        &self.core.terminal_names[i]
    }

    /// Source-level name of `node`, looking it up through the terminal order
    /// used by [`terminals`](Self::terminals) (ABI-27). Returns `None` for
    /// a `NodeId` not in the kernel's terminal set — internal-only callers
    /// treat that as "no name to surface" (e.g., a force/noise pair whose
    /// terminal was pruned).
    pub fn name_of_node(&self, node: NodeId) -> Option<&str> {
        self.core
            .terminals
            .iter()
            .position(|&t| t == node)
            .map(|i| self.terminal_name(i))
    }

    /// Per-slot names for the runtime state bank (ABI-47), one entry per
    /// slot in `[0, num_state_slots)`. Used by the introspection bridge to
    /// surface `.state` rows with readable names rather than positional
    /// indices.
    pub fn state_slot_names(&self) -> &[String] {
        &self.core.state_slot_names
    }

    /// Named terminal pairs `(plus, minus)` per force branch (ABI-47),
    /// looked up through [`name_of_node`](Self::name_of_node). Pairs whose
    /// terminals aren't in the kernel's terminal set fall back to `"gnd"`
    /// (matching the resolver's `NodeId::GROUND` convention).
    pub fn force_terminal_name_pairs(&self) -> Vec<(String, String)> {
        self.force_terminals()
            .iter()
            .map(|&(p, m)| {
                let p_name = self.name_of_node(p).unwrap_or("gnd").to_string();
                let m_name = self.name_of_node(m).unwrap_or("gnd").to_string();
                (p_name, m_name)
            })
            .collect()
    }

    /// Named terminal pairs `(plus, minus)` per noise source (ABI-47),
    /// looked up through [`name_of_node`](Self::name_of_node).
    pub fn noise_terminal_name_pairs(&self) -> Vec<(String, String)> {
        self.noise_terminals()
            .iter()
            .map(|&(p, m)| {
                let p_name = self.name_of_node(p).unwrap_or("gnd").to_string();
                let m_name = self.name_of_node(m).unwrap_or("gnd").to_string();
                (p_name, m_name)
            })
            .collect()
    }

    /// `true` when terminal `i` is digital-domain (never an MNA unknown —
    /// see `AnalogCore::digital_terminals`).
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

    /// `ddt(x, abstol|nature)` tolerance expressions declared by this body
    /// (§4.5.3, LRMW2-37) — one per `ddt` call that named a second
    /// argument. Empty for a body with no tolerance-carrying `ddt` call.
    pub fn ddt_abstols(&self) -> &[PomExpr] {
        &self.core.ddt_abstols
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

    /// State-bank slot index of the first `$limit` vold slot. `$limit`
    /// slots are the state bank's last `num_limits` slots — `limexp` slots
    /// sit just before them (`limexp_base()`).
    pub fn limit_base(&self) -> usize {
        self.core.num_state_slots - self.num_limits()
    }

    /// Number of `limexp()` `yold` slots (LRM §4.5.13). They occupy
    /// state-bank slots `[limexp_base(), limexp_base() + num_limexp())`,
    /// just before the `$limit` vold slots.
    pub fn num_limexp(&self) -> usize {
        self.limexp.as_ref().map_or(0, AnalogCapability::count)
    }

    /// State-bank slot index of the first `limexp()` `yold` slot.
    pub fn limexp_base(&self) -> usize {
        self.core.num_state_slots - self.num_limits() - self.num_limexp()
    }

    /// Compute each `limexp()` call site's updated (limited) output `ylim`
    /// at the current `yold` (in `state`). The device writes these back
    /// into the state bank to seed the next Newton iteration.
    pub fn eval_limexp_update(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(l) = &self.limexp {
            self.check_input_lens(volts, params, state, vars);
            Self::call(l.update, volts, params, state, vars, sim, out);
        }
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

    /// Per-slot `(limiter_name, reason)` catalog (phdl-introspection-attributes
    /// PIA-15/16), in slot order. The name is the `$limit` call-site `kind`;
    /// the reason is inferred from the kind. Read by the device's
    /// `limiting_report` to name the slot that clamped.
    pub fn limit_catalog(&self) -> &[(&'static str, piperine_solver::abi::LimitReason)] {
        self.limits.as_ref().map_or(&[], |l| &l.catalog)
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

    /// Per-seed param-dependency bitmasks (NGP-10), aligned with
    /// [`initial_condition_terminals`](Self::initial_condition_terminals).
    /// Bit `i` set means the seed reads param `i`. A seed fires at instance
    /// time only when at least one of its backing params was overridden at
    /// the instance site — `AnalogInstance::initial_conditions` consults
    /// this against its `param_given_mask`.
    pub fn initial_condition_param_deps(&self) -> &[u64] {
        &self.initial_condition_param_deps
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

    /// Number of declared pointwise constraint margins (DVC-10).
    pub fn num_margins(&self) -> usize {
        self.constraints.as_ref().map_or(0, AnalogCapability::count)
    }

    /// Whether the module declares any pointwise constraint margins.
    pub fn has_margins(&self) -> bool {
        self.constraints.is_some()
    }

    /// The declared pointwise margins' provenance (DVC-10), in authored
    /// order — parallel to the indices [`eval_margin`](Self::eval_margin)
    /// takes.
    pub fn margin_catalog(&self) -> &[super::constraints::MarginInfo] {
        self.constraints.as_ref().map_or(&[], |c| &c.catalog)
    }

    /// Evaluate margin `index` at the given point, returning its signed
    /// value (`m ≥ 0` ⟺ satisfied). Panics on out-of-range `index` — every
    /// caller is internal and bounds the index via
    /// [`num_margins`](Self::num_margins) first (same contract as
    /// [`terminal_name`](Self::terminal_name)).
    pub fn eval_margin(&self, index: usize, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx) -> f64 {
        let f = self
            .constraints
            .as_ref()
            .and_then(|c| c.margins.get(index))
            .unwrap_or_else(|| {
                panic!(
                    "kernel `{}`: margin #{index} out of range ({} margin(s))",
                    self.core.name,
                    self.num_margins()
                )
            })
            .eval;
        self.check_input_lens(volts, params, state, vars);
        let mut out = [0.0];
        Self::call(f, volts, params, state, vars, sim, &mut out);
        out[0]
    }

    /// Number of declared `cover` points (T33, DVT-40).
    pub fn num_cover_points(&self) -> usize {
        self.coverage.as_ref().map_or(0, AnalogCapability::count)
    }

    /// Whether the module declares any `cover` points.
    pub fn has_coverage(&self) -> bool {
        self.coverage.is_some()
    }

    /// The declared `cover` points' provenance, in authored order —
    /// parallel to the indices [`eval_cover`](Self::eval_cover) takes.
    pub fn cover_catalog(&self) -> &[super::coverage::CoverInfo] {
        self.coverage.as_ref().map_or(&[], |c| &c.catalog)
    }

    /// The module's declared `cover cross` catalog — no compiled function;
    /// the device combines the named points' already-mapped bin indices.
    pub fn cover_cross_catalog(&self) -> &[super::coverage::CoverCrossInfo] {
        self.coverage.as_ref().map_or(&[], |c| &c.crosses)
    }

    /// Evaluate `cover` point `index` at the given point, returning its
    /// raw pointwise value (the caller maps it to a bin index — this
    /// function has no notion of bins). Same panic contract as
    /// [`eval_margin`](Self::eval_margin).
    pub fn eval_cover(&self, index: usize, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx) -> f64 {
        let f = self
            .coverage
            .as_ref()
            .and_then(|c| c.points.get(index))
            .unwrap_or_else(|| {
                panic!(
                    "kernel `{}`: cover point #{index} out of range ({} cover point(s))",
                    self.core.name,
                    self.num_cover_points()
                )
            })
            .eval;
        self.check_input_lens(volts, params, state, vars);
        let mut out = [0.0];
        Self::call(f, volts, params, state, vars, sim, &mut out);
        out[0]
    }

    /// Terminals `(plus, minus)` per `ac_stim` source.
    pub fn ac_stim_terminals(&self) -> &[(NodeId, NodeId)] {
        self.ac_stim.as_ref().map_or(&[], |a| &a.terminals)
    }

    /// Size of the runtime-state value array instances must provide.
    /// Whether this kernel's **DC** stamps are a pure function of the terminal
    /// voltages — the `ElementCapabilities::BYPASS_OK` contract.
    ///
    /// Everything that makes a stamp depend on something else disqualifies it:
    /// a history-dependent operator slot (`delay`/`slew`/`transition`/`idt`),
    /// a runtime event (whose action mutates state), a `$limit` limiter (whose
    /// `vold` slot advances per evaluation), or a diagnostic (whose side effect
    /// must not be skipped). A `ddt` contribution does **not** disqualify: charge
    /// is not part of the DC stamp.
    pub fn dc_stamps_depend_only_on_terminal_voltages(&self) -> bool {
        self.runtime_states.is_empty()
            && self.events.is_empty()
            && self.limits.is_none()
            && self.diagnostics.is_empty()
    }

    pub fn num_state_slots(&self) -> usize {
        self.core.num_state_slots
    }

    /// Number of module-level persistent variable slots (the vars bank).
    /// Instances must provide a slice of at least this many `f64` values.
    pub fn num_vars(&self) -> usize {
        self.core.num_vars
    }

    /// The persistent-var initializers (Annex A.2.2.1): `(bank slot, init
    /// expr)` pairs the instance seeds its vars bank from at construction
    /// (values are per-instance — evaluated against the instance's params).
    pub fn var_inits(&self) -> &[(usize, piperine_lang::ast::Expr)] {
        &self.core.var_inits
    }

    /// The max `State`/`Var` id the compiled code actually loads, as
    /// `(params_read, state_read, vars_read)` (from `FlatAnalog::read_bounds`).
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

    /// Emits every collected `$display`/`$write`/... diagnostic through
    /// `ctx.io`'s sink (`emit::runtime_io`'s `pip_task_*` trampolines).
    /// **Callers must invoke this at most once per accepted (converged,
    /// committed) solution point — never per Newton iteration** (design.md
    /// Risks table: naive per-iteration wiring "prints thousands of
    /// duplicate lines"). `AnalogInstance::accept_timestep` is this
    /// method's only production caller, itself only reachable from
    /// `Element::accept_timestep`'s own accepted-step contract.
    ///
    /// A diagnostic with an empty format string is skipped — today the
    /// only producer of an empty format is `` @(final_step) ``'s
    /// unsupported-but-not-yet-fail-loud placeholder
    /// (`flatten/analog.rs`'s `EventSource::FinalStep` arm), not a real
    /// `$display` call; firing it here would print a spurious blank line
    /// every accepted step for any design using that construct
    /// (SPEC_DEVIATION: fixing `final_step` itself is out of this task's
    /// scope — LRMC-C5-04's own event-source work, not C7-07's).
    pub fn emit_diagnostics(&self, ctx: &mut SimCtx) {
        for (d, &fmt_id) in self.diagnostics.iter().zip(&self.diagnostic_fmt_ids) {
            if d.format.is_empty() {
                continue;
            }
            let ctx_ptr = ctx as *mut SimCtx;
            // T27/LRMW2-27: `$fatal` no longer lands in `self.diagnostics`
            // at all — `flatten/analog.rs`'s `TaskCall` arm routes it into
            // `fatal_flags`/`fatal_formats` instead, so this severity is
            // never `Fatal` in practice; `emit_fatal_diagnostics` (checked
            // every Newton iteration, not just accept_timestep) is `$fatal`'s
            // only emission path now. `Severity` keeps the `Fatal` variant
            // (still a valid POM value elsewhere), so this match stays
            // exhaustive rather than assuming the invariant.
            match d.severity {
                crate::resolve::Severity::Fatal => {
                    crate::emit::runtime_io::pip_task_fatal(ctx_ptr, fmt_id, 0, std::ptr::null());
                }
                _ => {
                    crate::emit::runtime_io::pip_task_display(ctx_ptr, fmt_id, 0, std::ptr::null());
                }
            }
        }
    }

    pub fn has_reactive(&self) -> bool {
        self.reactive.is_some()
    }

    pub fn has_bound_step(&self) -> bool {
        self.bound_step.is_some()
    }

    pub fn has_discontinuity(&self) -> bool {
        self.discontinuity.is_some()
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

    /// Write each `idt` state's `assert` value to `out[0..num_state_slots]`
    /// (§4.5.4/LRMW2-29) — a slot with no `assert` argument writes `0.0`
    /// (never asserted). No-op (leaves `out` untouched) when no `idt` call
    /// in this body named an `assert` argument.
    pub fn eval_assert_inputs(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.core.assert_inputs {
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

    /// Whether `$discontinuity` fired anywhere in this device's behavior
    /// for the given inputs — a compiled boolean-as-`f64` (nonzero = true,
    /// matching the `select`-built flag `flatten/analog.rs` folds every
    /// `$discontinuity(...)` call site into).
    pub fn eval_discontinuity(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx) -> bool {
        match self.discontinuity {
            Some(f) => {
                self.check_input_lens(volts, params, state, vars);
                let mut out = [0.0];
                Self::call(f, volts, params, state, vars, sim, &mut out);
                out[0] != 0.0
            }
            None => false,
        }
    }

    /// Whether this kernel has any `$fatal` call site (T27/LRMW2-27).
    pub fn has_fatal(&self) -> bool {
        self.fatal.is_some()
    }

    /// Whether `$fatal` fired anywhere in this device's behavior for the
    /// given inputs (T27/LRMW2-27) — same compiled-flag shape as
    /// [`Self::eval_discontinuity`], but callable every Newton iteration:
    /// `$fatal` must terminate immediately, not wait for the step to be
    /// accepted (LRM §9.7.3).
    pub fn eval_fatal(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx) -> bool {
        match self.fatal {
            Some(f) => {
                self.check_input_lens(volts, params, state, vars);
                let mut out = [0.0];
                Self::call(f, volts, params, state, vars, sim, &mut out);
                out[0] != 0.0
            }
            None => false,
        }
    }

    /// Emits every `$fatal` call site's message through `ctx.io` (T27) —
    /// called once, only when [`Self::eval_fatal`] just returned `true`.
    pub fn emit_fatal_diagnostics(&self, ctx: &mut SimCtx) {
        let ctx_ptr = ctx as *mut SimCtx;
        for &fmt_id in &self.fatal_fmt_ids {
            crate::emit::runtime_io::pip_task_fatal(ctx_ptr, fmt_id, 0, std::ptr::null());
        }
    }

    /// Whether the device has any nonlinear contribution (a compiled
    /// `.disto` 2nd-derivative kernel).
    pub fn has_disto2(&self) -> bool {
        !self.disto2.is_empty()
    }

    /// Ordered branch pairs `(j, k)` the disto2 kernel emits, in `out` row
    /// order.
    pub fn disto2_pairs(&self) -> &[Disto2Pair] {
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
    pub fn disto3_triples(&self) -> &[Disto3Triple] {
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

    /// Whether this kernel compiles an opvar-evaluation function (ABI-30).
    /// `false` for a module with no non-shadow analog `var`s — the eval
    /// path is absent and `read_opvars` returns empty.
    pub fn has_opvars(&self) -> bool {
        self.opvars.is_some()
    }

    /// Source-level names of the operating-point variables (ABI-30), in
    /// `eval_opvars` row order. Empty when the kernel compiled no opvar
    /// path.
    pub fn opvar_names(&self) -> &[String] {
        &self.opvar_names
    }

    /// Source-level names of the persistent-var bank slots (PIA-06), aligned
    /// with `0..num_vars`. Empty string for a slot with no source name.
    pub fn var_names(&self) -> &[String] {
        &self.var_names
    }

    /// Write each opvar's value to `out[0..opvar_names.len()]` (ABI-30),
    /// evaluated against the final post-solve voltages and the same
    /// `state`/`vars` banks the residual reads. No-op when the kernel
    /// compiled no opvar path (zero overhead for devices without vars).
    pub fn eval_opvars(&self, volts: &[f64], params: &[f64], state: &[f64], vars: &[f64], sim: &SimCtx, out: &mut [f64]) {
        if let Some(f) = self.opvars {
            self.check_input_lens(volts, params, state, vars);
            Self::call(f, volts, params, state, vars, sim, out);
        }
    }

    // ── Parameter Jacobian ∂F/∂p (DVG-02) ─────────────────────────────────────

    /// Whether this kernel compiled a `∂F/∂p` function (one or more
    /// differentiable parameters). `false` for a body with no `Real`-typed
    /// restamp-class parameter — no function is compiled (zero cost).
    pub fn has_param_jacobian(&self) -> bool {
        self.param_jacobian.is_some()
    }

    /// How many `∂F/∂p` JIT functions this kernel compiled: `0` when
    /// [`has_param_jacobian`](Self::has_param_jacobian) is false, `1`
    /// otherwise — **never one per parameter**, the design invariant that
    /// keeps a ~60-parameter model card off the Cranelift function-count
    /// wall `.disto` hits on MOS2/MOS3.
    pub fn num_param_jacobian_functions(&self) -> usize {
        self.param_jacobian.is_some() as usize
    }

    /// The parameter indices the kernel differentiates analytically (into
    /// [`param_names`](Self::param_names)). A driver refuses any index not in
    /// this slice (DVG-03 admissibility).
    pub fn param_jacobian_indices(&self) -> &[usize] {
        &self.param_indices
    }

    /// Resistive contribution terminals `(plus, minus)` in `out` row order —
    /// the stamp targets for the ABI method that turns each kernel scalar
    /// into a residual-shaped `∂F/∂param`. Empty without a kernel.
    pub fn param_jacobian_contribs(&self) -> &[(NodeId, NodeId)] {
        &self.param_contribs
    }

    /// Write `∂(contrib.expr)/∂param` per resistive contribution to
    /// `out[0..num_contribs]` for the given parameter index (DVG-02).
    /// `out` must be pre-zeroed and sized to
    /// [`param_jacobian_contribs`](Self::param_jacobian_contribs). A
    /// non-differentiable index writes nothing (the driver fails loud
    /// before calling — DVG-03). No-op when the kernel compiled no function.
    #[allow(clippy::too_many_arguments)]
    pub fn eval_param_jacobian(
        &self,
        param_idx: usize,
        volts: &[f64],
        params: &[f64],
        state: &[f64],
        vars: &[f64],
        sim: &SimCtx,
        out: &mut [f64],
    ) {
        if let Some(f) = self.param_jacobian {
            self.check_input_lens(volts, params, state, vars);
            unsafe {
                f(
                    volts.as_ptr(),
                    params.as_ptr(),
                    state.as_ptr(),
                    vars.as_ptr(),
                    sim as *const SimCtx,
                    param_idx,
                    out.as_mut_ptr(),
                )
            }
        }
    }
}
