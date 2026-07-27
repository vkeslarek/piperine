//! The `Element` object model: the composed device ABI (`Element:
//! AnalogDevice + DigitalDevice + Introspect`), `ElementCapabilities`,
//! and the stamping/lifecycle vocabulary every participant and every
//! driver speaks (MD-01 as amended 2026-07-19).
use num_complex::Complex64;
use std::collections::HashSet;

use crate::analyses::ac::AcAnalysisContext;
use crate::prelude::DcAnalysisResult;
use crate::analyses::dc::DcAnalysisState;
use crate::analyses::noise::Noise;
use crate::analyses::transient::{TransientAnalysisContext, TransientAnalysisState};
use crate::analog::AnalogReference;
use crate::core::introspect::{
    Invalidation, ObservableDescriptor, ParamDescriptor, ParamError, QueryDescriptor, TerminalDescriptor, Value,
};
use crate::digital::{DigitalNet, LogicValue};
use crate::digital::interface::{DigitalPorts, EvalCtx, EventSink};
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::linear::Stamp;
use crate::analyses::Context;

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
        /// The model checkpoints its mutable non-accept-gated state for
        /// rollback on a rejected step. The solver calls
        /// [`Element::checkpoint_state`] before every candidate attempt
        /// (transient `attempt_step`, DC homotopy before each strategy) and
        /// [`Element::restore_state`] on rejection; on acceptance the
        /// checkpoint is dropped. A device that returns `None` (default) is
        /// stateless and pays nothing.
        const SUPPORTS_ROLLBACK = 1 << 9;
        // `1 << 10` is unused: `SUPPORTS_QUERIES` was removed in P6 (CLN-11).
        // It promised a host hint that a model overrides `list_queries`/`query`,
        // but nothing declared it and nothing read it — and both methods have
        // working `read_opvars`-derived defaults, so a hint gates nothing. The
        // bit position is left vacant rather than renumbering the flags above.
        /// The model is eligible for stamp bypass: when its terminal voltages
        /// are unchanged within tolerance since the last evaluation, the
        /// solver may skip re-evaluating and re-stamping it for that Newton
        /// iteration (reusing its previous contribution). Suppressed globally
        /// while any element reports an active `LimitingReport`. Opt-in — a
        /// model only sets this when its stamps are a pure function of
        /// terminal voltages (linear devices, settled logic).
        const BYPASS_OK = 1 << 11;

        // ── Jacobian / derivative capability (ABI-23) ───────────────────────
        /// The model provides analytic second derivatives — the `.disto`
        /// Hessian compiled from symbolic differentiation (DISTO-03). The
        /// `.disto` driver checks this before solving for HD2; a device
        /// without it contributes no second-order nonlinear current. Every
        /// in-tree JIT device that compiles a `.disto` 2nd-derivative kernel
        /// sets this (ABI-26).
        const HAS_DISTO2 = 1 << 12;
        /// The model provides analytic third derivatives — the `.disto`
        /// third-order Hessian compiled from symbolic differentiation
        /// (DISTO-03). The `.disto` driver checks this before solving for
        /// HD3; a device without it contributes no third-order nonlinear
        /// current.
        const HAS_DISTO3 = 1 << 13;
        /// The model's Jacobian is finite-difference (numeric), not analytic
        /// — e.g., a plugin that perturbs its residual to fill the Jacobian.
        /// Analyses that require analytic derivatives (`.disto`) fail loud
        /// on these devices (ABI-25): the method of nonlinear currents
        /// needs the exact Hessian, never a numeric perturbation.
        const NUMERIC_JACOBIAN = 1 << 14;
    }
}

/// Opaque device-state checkpoint for rollback on rejected timesteps
/// (ABI-01). Devices pack whatever mutable non-accept-gated state they own
/// into the `(int, real)` carrier — the same shape as
/// [`DigitalDevice::digital_hidden_snapshot`]'s carrier, deliberately, so
/// per-step rollback and PSS full-state re-entry stay compatible. A default
/// `None` from [`Element::checkpoint_state`] means stateless (zero cost).
#[derive(Debug, Clone, PartialEq)]
pub struct ElementCheckpoint {
    /// Integer state: digital registers, edge-detection memory.
    pub int_state: Vec<i64>,
    /// Real state: limiter (`active`, `seeds`) and vold slots, analog vars.
    pub real_state: Vec<f64>,
}

/// Structured limiting feedback from a device limiter (ABI-09): the limiter
/// reports the unknown it clamped, the proposed vs limited value, and which
/// limiter fired. `is_some()` gates Newton convergence (a clamped junction
/// can momentarily satisfy KCL at a non-solution voltage); `limited_value`
/// applied to `net` steers the Newton guess to the clamped point
/// (pnjlim/fetlim lineage); `device` + `limiter_name` + `reason` are
/// diagnostics for hosts (HOST-10 — surfaced on `op.stats.limiting`).
#[derive(Debug, Clone)]
pub struct LimitingReport {
    /// The instance label of the device whose limiter fired (HOST-10).
    pub device: String,
    /// The unknown the limiter clamped (node voltage or branch current).
    pub net: AnalogReference,
    /// The raw Newton-proposed value before limiting.
    pub proposed: f64,
    /// The clamped value the solver should use.
    pub limited_value: f64,
    /// Which limiter fired (`"pnjlim"`, `"fetlim"`, `"limvds"`, …).
    pub limiter_name: &'static str,
    /// Why the limiter clamped (diagnostic, not behavioral).
    pub reason: LimitReason,
}

/// Why a limiter clamped a value (diagnostic for hosts).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LimitReason {
    /// Junction voltage step too large (pnjlim/fetlim).
    VoltageStep,
    /// Drain-source voltage step too large (limvds).
    VdsStep,
    /// Custom limiter reason (plugin-defined).
    Other(&'static str),
}

/// Analog participation: MNA loading + the analog lifecycle/convergence hooks.
///
/// Every method defaults to a no-op that contributes nothing, so an element
/// with no analog side inherits the empty surface untouched. The analog
/// drivers only ever call these through capability flags ([`ElementCapabilities`])
/// — declaring `ANALOG` without overriding the loaders is a visible bug, not
/// a silent no-op.
pub trait AnalogDevice: Send + Sync {
    // ── Analog lifecycle ──────────────────────────────────────────────────────

    /// Structured limiting feedback (ABI-09): when a device limiter clamps a
    /// value, return a [`LimitingReport`] naming the clamped unknown, the
    /// proposed vs limited value, and which limiter fired. The solver gates
    /// Newton convergence on `is_some()` and applies `limited_value` to
    /// `net`. Default `None` — a device that does not limit inherits this.
    fn limiting_report(&self) -> Option<LimitingReport> { None }

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
    fn next_breakpoints(&self, _from: f64, _horizon: f64) -> Vec<f64> { Vec::new() }

    /// `@initial` UIC seeds: the branch `(plus, minus)` and the voltage the
    /// device wants across it at t=0 (SPICE `.ic`). Ground terminals are
    /// `None`. Empty for devices without an initial-condition force. The
    /// transient analysis seeds these into the t=0 state.
    fn initial_conditions(
        &self,
    ) -> Vec<(Option<AnalogReference>, Option<AnalogReference>, f64)> {
        Vec::new()
    }

    /// Pre-freeze internal-unknown allocation. Called by `CircuitBuilder::build`
    /// once per element, in insertion order, before the matrix shape freezes.
    /// Elements that allocate internal MNA unknowns (auxiliary branch currents,
    /// hidden states) do so here via `UnknownAllocator::branch` and MUST
    /// declare [`ElementCapabilities::HAS_INTERNAL_UNKNOWNS`]. Default: no-op.
    fn allocate_unknowns(&mut self, _alloc: &mut crate::core::builder::UnknownAllocator<'_>) {}

    /// Set the instance temperature; recompute temperature-dependent constants.
    fn set_temperature(&mut self, _t: f64) {}

    /// Refresh cached state from the current solution before stamping.
    fn update(&mut self, _state: &CircularArrayBuffer2<f64>, _ctx: &Context) {}

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

    /// Second derivatives of the device's nonlinear contributions at the DC
    /// operating point — the `.disto` nonlinear-current source data
    /// (DISTO-03). Default `None`: a fully linear device contributes no
    /// nonlinear currents.
    fn load_disto2(
        &mut self,
        _dc_op: &DcAnalysisResult,
        _context: &Context,
    ) -> Option<crate::analyses::disto::Disto2> {
        None
    }

    /// Third derivatives of the device's nonlinear contributions at the DC
    /// operating point (DISTO-03). Default `None`: contributions of degree
    /// ≤ 2 have no third-order nonlinear current.
    fn load_disto3(
        &mut self,
        _dc_op: &DcAnalysisResult,
        _context: &Context,
    ) -> Option<crate::analyses::disto::Disto3> {
        None
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
}

/// Digital participation: two-phase delta cycle + hidden-state round-trip.
///
/// The delta cycle is two-phase to preserve non-blocking (NBA) semantics
/// across register chains (SPEC §9): the scheduler calls `seq_phase` on every
/// woken element first, then `comb_phase` on every woken element, so a
/// register samples the pre-edge net snapshot instead of racing ahead.
///
/// Every method defaults to an element that drives no nets, so a purely
/// analog device inherits the inert digital surface untouched.
pub trait DigitalDevice: Send + Sync {
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
    /// — those happen in [`comb_phase`](DigitalDevice::comb_phase).
    fn seq_phase(&mut self, _ctx: &EvalCtx<'_>) -> bool { false }

    /// Phase 2 (combinational): recompute outputs from live `ctx.nets` and the
    /// (possibly just-committed) register banks, emitting change events into
    /// `sink`.
    fn comb_phase(&mut self, _ctx: &EvalCtx<'_>, _sink: &mut dyn EventSink) {}

    /// Fused one-shot evaluation: [`seq_phase`](DigitalDevice::seq_phase) then
    /// [`comb_phase`](DigitalDevice::comb_phase) in a single call. Used by external
    /// co-simulators that don't participate in the scheduler's two-phase cycle.
    fn evaluate(&mut self, ctx: &EvalCtx<'_>, sink: &mut dyn EventSink) {
        self.seq_phase(ctx);
        self.comb_phase(ctx, sink);
    }

    /// Convenience: true if any of the element's input nets is in `changed`.
    fn has_input_on(&self, changed: &HashSet<DigitalNet>) -> bool {
        self.boundary().inputs.iter().any(|n| changed.contains(n))
    }

    /// Hidden digital state (module vars, edge-detection memory) as an
    /// opaque `(int, real)` carrier, snapshotted into each recorded
    /// [`crate::core::result::TransientStep`] and restored verbatim on full-state
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

/// OSDI-style introspection: parameters, queries, terminals, opvars.
///
/// All optional. A model exposes as much or as little metadata as it has;
/// hosts (sweeps, optimization, CLI/UI) discover and poke it through
/// this uniform surface without knowing the device family.
pub trait Introspect: Send + Sync {
    /// Operating-point variables (`gm`, `vbe`, …) as flat name/value pairs.
    /// The introspection layer ([`query`](Introspect::query)) reads through this by
    /// default; a model with typed or documented queries overrides those methods.
    fn read_opvars(&self) -> Vec<(String, f64)> { Vec::new() }

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
    /// state, counters). Defaults to one [`QueryKind::OperatingVariable`](crate::prelude::QueryKind::OperatingVariable) per
    /// [`read_opvars`](Introspect::read_opvars) entry.
    fn list_queries(&self) -> Vec<QueryDescriptor> {
        self.read_opvars()
            .into_iter()
            .map(|(name, _)| QueryDescriptor::opvar(name))
            .collect()
    }

    /// Read query `name`. Defaults to scanning
    /// [`read_opvars`](Introspect::read_opvars); a model with typed queries
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

    /// Model identity (ABI-46): type id + version, the family a host
    /// renders against. Defaults to no identity — a host falls back to the
    /// instance name.
    fn model_descriptor(&self) -> crate::core::introspect::ModelDescriptor {
        crate::core::introspect::ModelDescriptor::default()
    }

    /// Per-slot names for the runtime state bank (ABI-47): one entry per
    /// state slot, in bank order. Empty when the device declares no
    /// introspectable runtime state. A host uses this to render
    /// `.state`/`.op` rows with kind names (`"ddt[0]"`, `"vold[1]"`, …)
    /// rather than positional indices.
    fn list_state_slot_names(&self) -> Vec<String> { Vec::new() }

    /// Named terminal pairs `(plus, minus)` per force branch (ABI-47),
    /// sourced from the kernel's `force_terminals` catalog. Empty for a
    /// device with no `V(...) <- ...` potential forces.
    fn list_force_terminal_pairs(&self) -> Vec<(String, String)> { Vec::new() }

    /// Named terminal pairs `(plus, minus)` per noise source (ABI-47),
    /// sourced from the kernel's `noise_terminals` catalog. Empty for a
    /// device with no noise contributions.
    fn list_noise_terminal_pairs(&self) -> Vec<(String, String)> { Vec::new() }

    /// Device-declared observables a host may request for per-step
    /// recording (ABI-32): branch currents, charge/flux/state slots, and
    /// module vars — each with a relative cost hint. Default empty — a
    /// stateless device declares nothing, so a `ProbeSelection`
    /// referencing it fails loud at setup (ABI-35). A host pairs this
    /// catalog with [`ProbeSelection`](crate::core::introspect::ProbeSelection)
    /// to record only the observables it wants.
    fn list_observables(&self) -> Vec<ObservableDescriptor> { Vec::new() }
}

/// A single thing the solver simulates — the one contract over every
/// participant, analog or digital or both.
///
/// There is no separate "analog device" or "digital device" type and no
/// downcast: an element implements exactly the operations it needs and declares
/// them through [`capabilities`]. `Element` is the conjunction of three
/// concern-scoped supertraits — [`AnalogDevice`] (MNA loading + analog
/// lifecycle), [`DigitalDevice`] (two-phase delta cycle), and [`Introspect`]
/// (OSDI-style parameters/queries/terminals) — whose methods all default, so
/// a pure resistor overrides only [`load_dc`](AnalogDevice::load_dc) and
/// inherits the inert digital/introspection surfaces; a logic gate does the
/// reverse; a comparator or DAC does both over one shared object, so
/// mixed-signal coupling (analog load reading digital state, digital events
/// reading analog history) is native rather than bridged. The object is not
/// split — only its surface is grouped so each concern is separately legible,
/// and the solver never names a supertrait to select behavior:
/// [`capabilities`] gates, as before.
///
/// `Element` itself keeps only identity and the cross-cutting lifecycle that
/// isn't purely one concern: [`name`](Element::name),
/// [`capabilities`](Element::capabilities), [`setup`](Element::setup),
/// [`destroy`](Element::destroy), the analog→digital bridge hook
/// [`accept_timestep`](Element::accept_timestep), and
/// [`runtime_banks`](Element::runtime_banks).
///
/// [`capabilities`]: Element::capabilities
pub trait Element: AnalogDevice + DigitalDevice + Introspect {
    // ── Identity & capabilities ───────────────────────────────────────────────

    /// Source-level identity, for diagnostics and result mapping.
    fn name(&self) -> &str;

    /// Which of the operations below this element actually implements. Required
    /// — an element must declare itself so the solver and scheduler can plan
    /// without probing. Forgetting a flag is a visible bug, not a silent no-op.
    fn capabilities(&self) -> ElementCapabilities;

    fn setup(&mut self, _ctx: &Context) -> crate::core::result::Result<()> { Ok(()) }
    fn destroy(&mut self) {}

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

    /// Runtime state/var banks for opt-in per-step recording
    /// (`TransientAnalysisOptions::record_device_state`). Devices whose
    /// analog residual reads runtime banks (`delay`/`transition`/`idt`
    /// state, module `vars`) override to expose them so a trace can later
    /// recompute branch currents at each recorded step; the default is
    /// empty banks (nothing to record, zero cost).
    fn runtime_banks(&self) -> (&[f64], &[f64]) {
        (&[], &[])
    }

    /// Snapshot the device's mutable non-accept-gated state for rollback on a
    /// rejected step (ABI-01). The solver calls this before every candidate
    /// attempt (transient `attempt_step`, DC homotopy before each strategy);
    /// on rejection it calls [`restore_state`](Element::restore_state) with
    /// the snapshot, on acceptance the snapshot is dropped. Default `None` =
    /// stateless (the solver skips the restore — zero cost). Accept-gated
    /// state (operators, event detectors, `last_volts`) is advanced only in
    /// [`accept_timestep`](Element::accept_timestep) and is naturally safe —
    /// never checkpoint it.
    fn checkpoint_state(&self) -> Option<ElementCheckpoint> { None }

    /// Restore device state from a snapshot produced by
    /// [`checkpoint_state`](Element::checkpoint_state) (ABI-02). Called before
    /// a retry after the previous attempt was rejected, so the retry starts
    /// from the last accepted device state — not the dirty rejected-attempt
    /// state. Default: no-op (stateless device).
    fn restore_state(&mut self, _checkpoint: &ElementCheckpoint) {}
}
