//! `HostElement` (T14, DVT-19/23): a trait mirroring
//! [`Element`](piperine_solver::abi::Element)'s defaulted shape — two
//! required methods (`name`, `capabilities`), everything else defaulted —
//! so a device written in Rust implements only what it does and inherits
//! the inert surface for the rest, exactly the way `Element` itself works
//! (`ideal.md` §2's table).
//!
//! What is mirrored here is the subset `ideal.md` §2 groups as "what an
//! author actually reaches for": identity, analog stamping, time control,
//! state/lifecycle, and introspection. Digital participation
//! (`seq_phase`/`comb_phase`/`boundary`), noise, and `.disto` are not
//! mirrored — a device that needs them implements
//! [`Element`](piperine_solver::abi::Element) directly (level 0 is always
//! available, `ideal.md` §1) rather than going through `HostElement`.
//!
//! // SPEC_DEVIATION: `HostElement` mirrors a working subset of `Element`'s
//! // ~40 methods (identity, analog stamping, time control, checkpoint/
//! // restore, limiting/validation, introspection) rather than all of them.
//! // Reason: the omitted methods (digital multi-phase, noise PSD, `.disto`
//! // 2nd/3rd derivatives, param-derivative sensitivity) have no bearing on
//! // T14–T18's acceptance cases and each is independently reachable by
//! // implementing `Element` directly — the documented level-0 escape hatch
//! // this feature relies on everywhere else. Extending the mirror to cover
//! // them is additive and does not change this shape.

use piperine_solver::abi::{
    DigitalNet, ElementCapabilities, ElementCheckpoint, Invalidation, LimitingReport, LogicValue, ParamDescriptor,
    ParamError, UnknownAllocator, Value,
};

use super::stamp::{Ctx, StampSink, StateView};

/// A device written in the host language: Rust here (T14); Python bridges
/// through the same contract in a later phase (T27). Mirrors `Element`'s
/// defaulted shape (`ideal.md` §6) — implement what the device does,
/// inherit the inert surface for the rest.
pub trait HostElement: Send + Sync {
    // ── Identity (required) ─────────────────────────────────────────────

    /// Source-level identity, for diagnostics and result mapping.
    fn name(&self) -> &str;

    /// Which operations this device actually implements — required, exactly
    /// as [`Element::capabilities`](piperine_solver::abi::Element::capabilities)
    /// is, so the solver never discovers behavior by trial.
    fn capabilities(&self) -> ElementCapabilities;

    // ── Analog stamping ──────────────────────────────────────────────────

    /// Contribute this device's DC-operating-point stamps through `sink`.
    fn load_dc(&mut self, _ctx: &Ctx<'_>, _sink: &mut StampSink) {}

    /// Contribute this device's transient stamps through `sink`.
    fn load_transient(&mut self, _ctx: &Ctx<'_>, _sink: &mut StampSink) {}

    /// Pre-freeze internal-unknown allocation (a branch current, a hidden
    /// state) — mirrors
    /// [`AnalogDevice::allocate_unknowns`](piperine_solver::abi::AnalogDevice::allocate_unknowns)
    /// exactly (already the solver's own safe wrapper; no reason to
    /// re-wrap it).
    fn allocate_unknowns(&mut self, _alloc: &mut UnknownAllocator<'_>) {}

    /// Set the instance temperature; recompute temperature-dependent state.
    fn set_temperature(&mut self, _t: f64) {}

    /// Refresh cached state from the current solution before stamping.
    fn update(&mut self, _state: &StateView<'_>, _ctx: &Ctx<'_>) {}

    // ── Time control ─────────────────────────────────────────────────────

    /// Absolute landing points this device requires the integrator to hit
    /// within `(from, from + horizon]` — a `Wave`'s known discontinuities,
    /// a `transition`'s pending ramp edges, and so on (`ideal.md` §3.9 A's
    /// `pending_edges` contract, lifted to the ABI).
    fn next_breakpoints(&self, _from: f64, _horizon: f64) -> Vec<f64> {
        Vec::new()
    }

    /// Largest timestep this device can tolerate from here (`$bound_step`).
    fn bound_step_hint(&self) -> f64 {
        f64::INFINITY
    }

    // ── Lifecycle ─────────────────────────────────────────────────────────

    /// Called after each **accepted** solution point. Mutating state here
    /// (never in `load_dc`/`load_transient`) is what keeps a device correct
    /// under timestep rejection (`ideal.md` §7) — the same trap PHDL
    /// authors already face.
    fn accept_timestep(&mut self, _state: &StateView<'_>, _t: f64) {}

    /// The dynamic digital subscriber's post-settle dispatch (T23,
    /// DVT-27) — mirrors
    /// [`Element::on_digital_settled`](piperine_solver::abi::Element::on_digital_settled)
    /// exactly. Called once per accepted step, strictly after the
    /// scheduler has settled every net for this instant, with the final
    /// settled values (never a mid-delta-cycle one). A `Sequencer`'s
    /// `wait(edge(...))` (T24) is built on this.
    fn on_digital_settled(&mut self, _nets: &[piperine_solver::abi::LogicValue], _t: f64) {}

    /// Digital events queued during this accept (T21, DVT-33: a sequencer's
    /// `t.drive(net, value)` goes through here) — drained once, immediately
    /// after [`Self::accept_timestep`] returns, by the blanket adapter and
    /// forwarded into the solver's real `EventSink`. `HostElement` itself
    /// never touches `EventSink` directly, keeping the raw digital-
    /// scheduling ABI out of this trait's day-to-day surface; a device
    /// with nothing to drive (the overwhelming common case) pays nothing.
    ///
    /// // SPEC_DEVIATION: `Element::accept_timestep`'s real ABI already
    /// // carries an `EventSink` parameter; the original T14 mirror
    /// // dropped it (documented as "digital participation... not
    /// // mirrored"). T21 needs exactly one digital capability — driving a
    /// // net from a sequencer — so rather than widening
    /// // `accept_timestep`'s signature (a breaking change to every
    /// // existing `HostElement` impl, including this trait's own T18
    /// // proof devices), this adds one narrow, independently-defaulted
    /// // hook. A device that needs the full digital surface still
    /// // implements `Element` directly (the documented level-0 escape
    /// // hatch).
    fn drain_digital_drives(&mut self) -> Vec<(DigitalNet, LogicValue, f64)> {
        Vec::new()
    }

    /// Snapshot mutable non-accept-gated state for rollback on a rejected
    /// step. `None` (default) = stateless, zero cost — mirrors
    /// [`Element::checkpoint_state`](piperine_solver::abi::Element::checkpoint_state).
    fn checkpoint_state(&self) -> Option<ElementCheckpoint> {
        None
    }

    /// Restore a snapshot produced by [`Self::checkpoint_state`].
    fn restore_state(&mut self, _checkpoint: &ElementCheckpoint) {}

    /// Structured limiting feedback (T16, `ideal.md` §3.9 D4): a level-1
    /// limiter's still-clamping state, surfaced so it vetoes Newton
    /// convergence exactly as a PHDL device's does.
    fn limiting_report(&self) -> Option<LimitingReport> {
        None
    }

    /// Structured validation findings (T22, DVT-29): a `Sequencer`'s
    /// `expect`/`warn` route here, through `dv-core`'s channel — never a
    /// print. Mirrors
    /// [`Element::validation_reports`](piperine_solver::abi::Element::validation_reports)
    /// exactly (`Option`, drained by the solver only on elements declaring
    /// [`ElementCapabilities::EMITS_VALIDATION`]); `None` is the default,
    /// zero-cost for every `HostElement` that never reports anything.
    fn validation_reports(&self) -> Option<piperine_solver::abi::ValidationReport> {
        None
    }

    // ── Introspection ────────────────────────────────────────────────────

    /// Operating-point variables as flat name/value pairs.
    fn read_opvars(&self) -> Vec<(String, f64)> {
        Vec::new()
    }

    /// Declared parameters and their metadata.
    fn list_params(&self) -> Vec<ParamDescriptor> {
        Vec::new()
    }

    /// The current value of parameter `name`, or `None`.
    fn get_param(&self, _name: &str) -> Option<Value> {
        None
    }

    /// Write parameter `name`, returning what the change invalidates
    /// (MD-18: restamp-class fine, rebuild-class fails loud at the caller —
    /// `Session::set` already enforces this generically for every
    /// `Element`, host devices included, since the blanket adapter presents
    /// one through `Introspect::set_param` exactly like any other).
    fn set_param(&mut self, name: &str, _value: Value) -> Result<Invalidation, ParamError> {
        Err(ParamError::Unknown(name.to_string()))
    }
}
