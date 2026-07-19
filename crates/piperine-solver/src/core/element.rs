use num_complex::Complex64;
use std::collections::HashSet;

use crate::analysis::ac::AcAnalysisContext;
use crate::prelude::DcAnalysisResult;
use crate::analysis::dc::DcAnalysisState;
use crate::analysis::noise::Noise;
use crate::analysis::transient::{TransientAnalysisContext, TransientAnalysisState};
use crate::analog::AnalogReference;
use crate::core::introspect::{
    Invalidation, ParamDescriptor, ParamError, QueryDescriptor, TerminalDescriptor, Value,
};
use crate::digital::{DigitalNet, LogicValue};
use crate::digital::interface::{DigitalPorts, EvalCtx, EventSink};
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::linear::Stamp;
use crate::math::unit::Second;
use crate::solver::Context;

bitflags::bitflags! {
    /// What an [`Element`] participates in, declared up front. The solver and
    /// scheduler build their plans from this descriptor instead of discovering
    /// behavior by trial downcast — a JIT-compiled PHDL block, a Rust plugin,
    /// and a future co-sim peripheral all advertise through the same table.
    ///
    /// Coarse grain (`ANALOG`/`DIGITAL`) describes which engines a model can
    /// participate in. The finer flags describe which **analyses** the analog
    /// path contributes to and which **dependencies** the model has, so the
    /// solver can skip work it cannot affect.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ElementCapabilities: u32 {
        /// Contributes to the analog system (MNA stamps in DC/AC/transient/noise).
        const ANALOG = 1 << 0;
        /// Participates in the digital scheduler (drives/reads logic nets).
        const DIGITAL = 1 << 1;
        /// Its digital logic samples analog node voltages (A2D), so it must be
        /// evaluated on every analog solve even without a pending digital event.
        const SAMPLES_ANALOG = 1 << 2;

        // ── Per-analysis participation (subset of `ANALOG`) ──────────────────
        /// `load_dc` contributes to the DC operating point.
        const LOADS_DC = 1 << 3;
        /// `load_ac` contributes to the small-signal AC sweep.
        const LOADS_AC = 1 << 4;
        /// `load_transient` contributes to time-domain integration.
        const LOADS_TRAN = 1 << 5;
        /// `noise_current_psd` returns non-empty sources.
        const EMITS_NOISE = 1 << 6;

        // ── Cross-domain dependencies ────────────────────────────────────────
        /// Analog load reads the digital net snapshot (D2A bridge). Implies
        /// `ANALOG`. The DC and transient drivers must order the digital settle
        /// before stamping this element.
        const DEPENDS_ON_DIGITAL = 1 << 7;

        // ── Loader/ABI capabilities ──────────────────────────────────────────
        /// The model allocated internal MNA unknowns (auxiliary branch currents,
        /// hidden states) during circuit construction. The matrix shape is fixed
        /// before analysis, but the loader needs this flag to know the element
        /// took the allocation seam.
        const HAS_INTERNAL_UNKNOWNS = 1 << 8;
        /// The model owns hidden state with non-trivial checkpoint/rollback
        /// (delayed outputs, latches, A2D detector state). The transient solver
        /// must drive `Element::checkpoint_state`/`rollback_state`/`commit_state`
        /// around candidate timesteps. (Hooks land with Phase 4.)
        const SUPPORTS_ROLLBACK = 1 << 9;
        /// The model overrides `list_queries`/`query` with typed metadata
        /// beyond the `read_opvars` default. Hosts can rely on this flag to skip
        /// the default scan.
        const SUPPORTS_QUERIES = 1 << 10;
        /// The model is eligible for stamp bypass: when its terminal voltages
        /// are unchanged within tolerance since the last evaluation, the
        /// solver may skip re-evaluating and re-stamping it for that Newton
        /// iteration (reusing its previous contribution). Suppressed globally
        /// while any element reports `limiting_active()`. Opt-in — a model
        /// only sets this when its stamps are a pure function of terminal
        /// voltages (linear devices, settled logic).
        const BYPASS_OK = 1 << 11;

        /// declaration consumed by solver-performance follow-up
        const STAMPS_CHARGE = 1 << 13;
        /// declaration consumed by solver-performance follow-up
        const ANALYTIC_JACOBIAN = 1 << 14;
    }
}

/// A device limiter's structured feedback: which unknown it clamped and to
/// what value this iteration. Where `limiting_active()` only vetoes the
/// convergence test, a hint lets the solver steer — it applies the limited
/// value to the Newton guess before testing convergence, so the iteration
/// continues from the clamped point instead of oscillating around it
/// (pnjlim/fetlim lineage).
#[derive(Debug, Clone)]
pub struct ConvergenceHint {
    /// The unknown the limiter clamped (node voltage or branch current).
    pub net: AnalogReference,
    /// The value the limiter clamped it to.
    pub limited_value: f64,
}

/// A single thing the solver simulates — the one contract over every
/// participant, analog or digital or both.
///
/// There is no separate "analog device" or "digital device" type and no
/// downcast: an element implements exactly the operations it needs and declares
/// them through [`capabilities`]. A pure resistor stamps the analog system and
/// leaves the digital methods at their defaults; a logic gate does the reverse;
/// a comparator or DAC does both over one shared object, so mixed-signal
/// coupling (analog load reading digital state, digital events reading analog
/// history) is native rather than bridged.
///
/// The analog methods ([`load_dc`], [`load_ac`], [`load_transient`],
/// [`noise_current_psd`], plus the lifecycle hooks) default to no-ops that
/// contribute nothing. The digital methods ([`boundary`], [`init`],
/// [`seq_phase`], [`comb_phase`]) default to an element that drives no nets.
/// Every element still declares [`capabilities`] so the solver never guesses.
///
/// [`capabilities`]: Element::capabilities
/// [`load_dc`]: Element::load_dc
/// [`load_ac`]: Element::load_ac
/// [`load_transient`]: Element::load_transient
/// [`noise_current_psd`]: Element::noise_current_psd
/// [`boundary`]: Element::boundary
/// [`init`]: Element::init
/// [`seq_phase`]: Element::seq_phase
/// [`comb_phase`]: Element::comb_phase
pub trait Element: Send + Sync {
    // ── Identity & capabilities ───────────────────────────────────────────────

    /// Source-level identity, for diagnostics and result mapping.
    fn name(&self) -> &str;

    /// Which of the operations below this element actually implements. Required
    /// — an element must declare itself so the solver and scheduler can plan
    /// without probing. Forgetting a flag is a visible bug, not a silent no-op.
    fn capabilities(&self) -> ElementCapabilities;

    // ── Analog lifecycle ──────────────────────────────────────────────────────

    /// Whether a device limiter is currently clamping (pnjlim/fetlim). While
    /// active the global Newton loop must not declare convergence.
    fn limiting_active(&self) -> bool { false }

    /// Structured limiting feedback: which unknown was clamped, and to what.
    /// The solver applies the hint to the Newton guess before the convergence
    /// test. Default `None` — a device that only knows *that* it limited
    /// keeps reporting through [`limiting_active`](Element::limiting_active);
    /// a device that knows *what* it limited upgrades to a hint.
    fn convergence_hint(&self) -> Option<ConvergenceHint> { None }

    /// Largest timestep the element can tolerate from here (`$bound_step`).
    fn bound_step_hint(&self) -> f64 { f64::INFINITY }

    /// Absolute landing points this element requires the integrator to hit
    /// within `(from, from + horizon]`. Time-varying source models (pulse
    /// edges, PWL corners, `@timer` fires) and digital switching times declare
    /// their discontinuities here so the stepper never steps over a kink. The
    /// default is empty — elements without discontinuities need not override.
    ///
    /// The solver reads this each step and merges it with the digital event
    /// queue. The times are absolute (not relative), so they survive step
    /// rollback.
    fn next_breakpoints(&self, _from: Second, _horizon: Second) -> Vec<Second> { Vec::new() }

    /// `@initial` UIC seeds: the branch `(plus, minus)` and the voltage the
    /// device wants across it at t=0 (SPICE `.ic`). Ground terminals are
    /// `None`. Empty for devices without an initial-condition force. The
    /// transient analysis seeds these into the t=0 state.
    fn initial_conditions(
        &self,
    ) -> Vec<(Option<AnalogReference>, Option<AnalogReference>, f64)> {
        Vec::new()
    }

    /// Operating-point variables (`gm`, `vbe`, …) as flat name/value pairs.
    /// The introspection layer ([`query`](Element::query)) reads through this by
    /// default; a model with typed or documented queries overrides those methods.
    fn read_opvars(&self) -> Vec<(String, f64)> { Vec::new() }

    // ── Introspection: parameters, queries, terminals (OSDI-style) ────────────
    //
    // All optional. A model exposes as much or as little metadata as it has;
    // hosts (sweeps, optimization, CLI/UI) discover and poke it through
    // this uniform surface without knowing the device family.

    /// Declared parameters and their metadata. Empty when the element exposes no
    /// runtime-inspectable parameters.
    fn list_params(&self) -> Vec<ParamDescriptor> { Vec::new() }

    /// The current value of parameter `name`, or `None` if there is no such
    /// parameter.
    fn get_param(&self, _name: &str) -> Option<Value> { None }

    /// Write parameter `name`, returning what the change invalidates so the
    /// caller recomputes exactly as much as needed. The default rejects every
    /// write as unknown; a model with writable parameters overrides this.
    fn set_param(&mut self, name: &str, _value: Value) -> Result<Invalidation, ParamError> {
        Err(ParamError::Unknown(name.to_string()))
    }

    /// Declared queries (operating variables, terminal quantities, internal
    /// state, counters). Defaults to one [`QueryKind::OperatingVariable`] per
    /// [`read_opvars`](Element::read_opvars) entry.
    fn list_queries(&self) -> Vec<QueryDescriptor> {
        self.read_opvars()
            .into_iter()
            .map(|(name, _)| QueryDescriptor::opvar(name))
            .collect()
    }

    /// Read query `name`. Defaults to scanning
    /// [`read_opvars`](Element::read_opvars); a model with typed queries
    /// overrides this.
    fn query(&self, name: &str) -> Option<Value> {
        self.read_opvars()
            .into_iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| Value::Real(v))
    }

    /// Declared terminals (name, domain, direction, required). Empty when the
    /// element does not describe its terminals.
    fn list_terminals(&self) -> Vec<TerminalDescriptor> { Vec::new() }

    fn setup(&mut self, _ctx: &Context) -> crate::result::Result<()> { Ok(()) }
    fn destroy(&mut self) {}

    /// Pre-freeze internal-unknown allocation. Called by [`CircuitBuilder::build`]
    /// once per element, in insertion order, before the matrix shape freezes.
    /// Elements that allocate internal MNA unknowns (auxiliary branch currents,
    /// hidden states) do so here via [`UnknownAllocator::branch`] and MUST
    /// declare [`ElementCapabilities::HAS_INTERNAL_UNKNOWNS`]. Default: no-op.
    fn allocate_unknowns(&mut self, _alloc: &mut crate::core::builder::UnknownAllocator<'_>) {}

    /// Set the instance temperature; recompute temperature-dependent constants.
    fn set_temperature(&mut self, _t: f64) {}

    /// Refresh cached state from the current solution before stamping.
    fn update(&mut self, _state: &CircularArrayBuffer2<f64>, _ctx: &Context) {}

    /// Called after each accepted solution point at time `t`. Elements that
    /// couple into the digital world (A2D bridges, analog event detectors)
    /// emit their net value-changes through `sink` — the same write-only
    /// façade digital evaluation uses — so the analog side never names the
    /// scheduler's queue.
    fn accept_timestep(
        &mut self,
        _state: &CircularArrayBuffer2<f64>,
        _t: f64,
        _nets: &[LogicValue],
        _sink: &mut dyn EventSink,
    ) {
    }

    // ── Analog loading ────────────────────────────────────────────────────────

    fn load_dc(
        &mut self,
        _state: &DcAnalysisState<'_>,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        Vec::new()
    }

    fn load_ac(
        &mut self,
        _dc_op: &DcAnalysisResult,
        _ac_ctx: &AcAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex64>> {
        Vec::new()
    }

    fn load_transient(
        &mut self,
        _states: &TransientAnalysisState<'_>,
        _tran_ctx: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        Vec::new()
    }

    fn noise_current_psd(
        &mut self,
        _dc_point: &DcAnalysisResult,
        _ac_context: &AcAnalysisContext,
    ) -> Vec<Noise> {
        Vec::new()
    }

    // ── Digital evaluation ────────────────────────────────────────────────────
    //
    // The delta cycle is two-phase to preserve non-blocking (NBA) semantics
    // across register chains (SPEC §9): the scheduler calls `seq_phase` on every
    // woken element first, then `comb_phase` on every woken element, so a
    // register samples the pre-edge net snapshot instead of racing ahead.

    /// Boundary wiring: the nets this element reads (its sensitivity list) and
    /// the nets it drives. Defaults to driving/reading nothing.
    fn boundary(&self) -> DigitalPorts<'_> {
        DigitalPorts { inputs: &[], outputs: &[] }
    }

    /// Power-on: apply register initial values and emit initial output events
    /// (typically at `t = 0`). No-op for elements with no digital state.
    fn init(&mut self, _sink: &mut dyn EventSink) {}

    /// Phase 1 (register commit): detect clock edges against the previous
    /// evaluation and commit register writes from the pre-settle net snapshot.
    /// Returns whether any clocked block fired. **Must not** emit output events
    /// — those happen in [`comb_phase`](Element::comb_phase).
    fn seq_phase(&mut self, _ctx: &EvalCtx<'_>) -> bool { false }

    /// Phase 2 (combinational): recompute outputs from live `ctx.nets` and the
    /// (possibly just-committed) register banks, emitting change events into
    /// `sink`.
    fn comb_phase(&mut self, _ctx: &EvalCtx<'_>, _sink: &mut dyn EventSink) {}

    /// Fused one-shot evaluation: [`seq_phase`](Element::seq_phase) then
    /// [`comb_phase`](Element::comb_phase) in a single call. Used by external
    /// co-simulators that don't participate in the scheduler's two-phase cycle.
    fn evaluate(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        self.seq_phase(ctx);
        self.comb_phase(ctx, sink);
    }

    /// Convenience: true if any of the element's input nets is in `changed`.
    fn has_input_on(&self, changed: &HashSet<DigitalNet>) -> bool {
        self.boundary().inputs.iter().any(|n| changed.contains(n))
    }

    // ── Numerical integration feedback ────────────────────────────────────────

    /// LTE-driven timestep suggestion, called by the transient stepper after
    /// an accepted step. Reactive devices override this to report the
    /// maximum timestep they can tolerate; elements without charge/flux
    /// history (pure resistors, pure digital) leave this at the default
    /// `None`.
    ///
    /// - `state`: the accepted analog solution history at `t_n`, `t_{n-1}`,
    ///   `t_{n-2}`, …
    /// - `time_history`: the accepted step sizes `[dt_n, dt_{n-1}, …]`.
    /// - `context`: solver tolerances (`trtol`, `chgtol`, `reltol`,
    ///   `abstol`).
    fn suggest_transient_step(
        &self,
        _state: &TransientAnalysisState<'_>,
        _time_history: &[f64],
        _context: &Context,
    ) -> Option<f64> {
        None
    }

    /// Runtime state/var banks for opt-in per-step recording
    /// (`TransientAnalysisOptions::record_device_state`). Devices whose
    /// analog residual reads runtime banks (`delay`/`transition`/`idt`
    /// state, module `vars`) override to expose them so a trace can later
    /// recompute branch currents at each recorded step; the default is
    /// empty banks (nothing to record, zero cost).
    fn runtime_banks(&self) -> (&[f64], &[f64]) {
        (&[], &[])
    }

    /// Hidden digital state (module vars, edge-detection memory) as an
    /// opaque `(int, real)` carrier, snapshotted into each recorded
    /// [`crate::result::TransientStep`] and restored verbatim on full-state
    /// re-entry (PSS shots, `TransientSolver::with_initial_state`) — the
    /// shot-state contract requires register state to round-trip with the
    /// digital nets. `None` = stateless (pure combinational) element.
    fn digital_hidden_snapshot(&self) -> Option<(Vec<i64>, Vec<f64>)> {
        None
    }

    /// Restore a state previously produced by [`Self::digital_hidden_snapshot`].
    /// Called on full-state re-entry after `init`, before the first settle.
    fn digital_hidden_restore(&mut self, _state: &(Vec<i64>, Vec<f64>)) {}
}
