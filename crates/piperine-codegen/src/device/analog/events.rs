//! Events capability: runtime analog event detection (`cross`/`above`/
//! `@timer`) and the `@initial`/fired-event variable-action dispatch. The
//! fire decision itself comes from `piperine_ir::events::EventDetector`;
//! this file keeps everything that is not a detector — the period table,
//! `CompiledTrigger` -> `signal::events::Trigger` mapping (`Initial` has no
//! counterpart, it is `@initial` dispatch, not a detector), and the
//! `@initial`/fired-event variable-action dispatch.

use piperine_ir::events::{AbsDeltaDetector, CrossDir as SignalCrossDir, EventDetector, Trigger};

use crate::emit::abi::SimCtx;
use crate::error::CodegenError;
use crate::kernel::analog::{AnalogKernel, CompiledTrigger};
use crate::resolve::CrossDir;

use super::AnalogInstance;

/// Codegen's `CrossDir` (`resolve::stmt`) to the signal crate's own —
/// distinct types by design (the purity guard forbids `piperine-ir`
/// depending on codegen's resolved form), same three variants.
fn to_signal_cross_dir(dir: CrossDir) -> SignalCrossDir {
    match dir {
        CrossDir::Rising => SignalCrossDir::Rising,
        CrossDir::Falling => SignalCrossDir::Falling,
        CrossDir::Either => SignalCrossDir::Either,
    }
}

/// One detector per runtime event, in kernel event order, plus each event's
/// (parameter-constant) timer period (`0` for non-timers). `absdelta`
/// events additionally get their own `AbsDeltaDetector` (its `fired` shape
/// doesn't fit `Trigger`, see `piperine-ir`'s own doc comment) and their
/// four parameter-constant arguments, `None`/zeros for every other trigger
/// kind — a parallel `Vec` aligned by event index rather than an enum
/// wrapping both detector kinds, since only one event in a handful is
/// typically `absdelta`.
pub(super) struct EventSystem {
    detectors: Vec<EventDetector>,
    periods: Vec<(f64, f64)>,
    abs_delta: Vec<Option<AbsDeltaDetector>>,
    /// `(delta, time_tol, expr_tol, enable)` per event — `enable` is a
    /// per-step trigger value in the LRM's own grammar, but treated as an
    /// elaboration-time constant here (documented scope limit, matching
    /// `Timer`'s own `period`/`phase` precedent); the common idiom
    /// (§5.10.3.4's own example) passes parameters for all four anyway.
    abs_delta_args: Vec<(f64, f64, f64, f64)>,
}

impl EventSystem {
    pub(super) fn new(kernel: &AnalogKernel, params: &[f64]) -> Result<Self, CodegenError> {
        let param_names = kernel.param_names();
        let resolve = |name: &str| -> Option<f64> {
            param_names.iter().position(|n| n == name).and_then(|i| params.get(i).copied())
        };
        // Timer periods are parameter-constant, evaluated once.
        let periods = kernel
            .events()
            .iter()
            .map(|e| match &e.trigger {
                CompiledTrigger::Timer { period, phase } => {
                    let p = crate::resolve::pom_eval_const(period, &resolve)
                        .map_err(CodegenError::ConstEval)?;
                    // First fire at `phase` (a phased timer `@timer(period, phase)`),
                    // or at `period` for the unphased `@timer(period)` (phase ≤ 0).
                    let ph = crate::resolve::pom_eval_const(phase, &resolve)
                        .map_err(CodegenError::ConstEval)?;
                    Ok(if ph > 0.0 { (p, ph) } else { (p, p) })
                }
                _ => Ok((0.0, 0.0)),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let abs_delta_args = kernel
            .events()
            .iter()
            .map(|e| match &e.trigger {
                CompiledTrigger::AbsDelta { delta, time_tol, expr_tol, enable } => {
                    let d = crate::resolve::pom_eval_const(delta, &resolve).map_err(CodegenError::ConstEval)?;
                    let tt = crate::resolve::pom_eval_const(time_tol, &resolve).map_err(CodegenError::ConstEval)?;
                    let et = crate::resolve::pom_eval_const(expr_tol, &resolve).map_err(CodegenError::ConstEval)?;
                    let en = crate::resolve::pom_eval_const(enable, &resolve).map_err(CodegenError::ConstEval)?;
                    Ok((d, tt, et, en))
                }
                _ => Ok((0.0, 0.0, 0.0, 0.0)),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let detectors = kernel
            .events()
            .iter()
            .zip(&periods)
            .map(|(_, &(_period, first_fire))| EventDetector::new(first_fire))
            .collect();
        let abs_delta = kernel
            .events()
            .iter()
            .map(|e| matches!(e.trigger, CompiledTrigger::AbsDelta { .. }).then(AbsDeltaDetector::new))
            .collect();
        Ok(Self { detectors, periods, abs_delta, abs_delta_args })
    }

    /// Absolute landing points this instance's `@timer` events fire at
    /// within `(from, end]` — each timer fires every `period` (its current
    /// `next_fire` advanced into the window). Non-timer events (crossings)
    /// are detected reactively and contribute no static breakpoints here.
    pub(super) fn next_breakpoints(&self, from: f64, end: f64, out: &mut Vec<f64>) {
        for (det, &(period, _first_fire)) in self.detectors.iter().zip(&self.periods) {
            det.next_breakpoints(period, from, end, out);
        }
    }

    /// Evaluate trigger values and detect transitions, returning the
    /// per-event fired flags. Does not apply actions — the caller owns the
    /// vars bank that the fired actions write into.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn detect(
        &mut self,
        kernel: &AnalogKernel,
        volts: &[f64],
        params: &[f64],
        state: &[f64],
        vars: &[f64],
        sim: &SimCtx,
        time: f64,
    ) -> Vec<bool> {
        let num_events = kernel.events().len();
        if num_events == 0 {
            return Vec::new();
        }
        let mut triggers = vec![0.0; num_events];
        kernel.eval_event_triggers(volts, params, state, vars, sim, &mut triggers);
        let mut fired = vec![false; num_events];
        for (i, detector) in self.detectors.iter_mut().enumerate() {
            // `AbsDelta` runs through its own `AbsDeltaDetector`, not the
            // tier-1 `EventDetector`/`Trigger` shape below — dispatched
            // first and skipped in the tier-1 loop.
            if let CompiledTrigger::AbsDelta { .. } = &kernel.events()[i].trigger {
                let (delta, time_tol, expr_tol, enable) = self.abs_delta_args[i];
                if let Some(det) = self.abs_delta[i].as_mut() {
                    fired[i] = det.fired(triggers[i], time, delta, time_tol, expr_tol, enable);
                }
                continue;
            }
            // `Initial`/`Final` are lifecycle dispatch (`fire_initial_events`/
            // `fire_final_events`), not detectors — short-circuited here
            // exactly as the old inline `Initial => false` arm did, never
            // given a `signal::events::Trigger` counterpart.
            let Some(trigger) = (match &kernel.events()[i].trigger {
                CompiledTrigger::Initial(_) | CompiledTrigger::Final(_) => None,
                CompiledTrigger::Above => Some(Trigger::Above),
                CompiledTrigger::Cross(dir) => Some(Trigger::Cross(to_signal_cross_dir(*dir))),
                CompiledTrigger::Timer { .. } => Some(Trigger::Timer),
                CompiledTrigger::Named => Some(Trigger::AnyChange),
                CompiledTrigger::AbsDelta { .. } => unreachable!("handled above"),
            }) else {
                continue;
            };
            fired[i] = detector.fired(trigger, triggers[i], time, self.periods[i].0);
        }
        fired
    }
}

impl AnalogInstance {
    /// Execute the BARE `@(initial_step)` form's event actions once, at
    /// zero volts (before any solve, only parameters and power-on variable
    /// values are visible) — unchanged from before LRMW2-08 (T12): this is
    /// the pre-existing, already-working case (LRM Table 5-1's
    /// unqualified `initial_step` row — "every analysis"), called once at
    /// construction (`AnalogInstance::new`).
    ///
    /// The QUALIFIED form (`initial_step("dc", ...)`) is NOT handled
    /// here — at construction time no analysis has started yet, so there
    /// is nothing correct to filter against (LRM Table 5-1 draws a real
    /// distinction: `initial_step("tran")` must NOT fire during a `dc`
    /// run). It fires instead from [`Self::fire_initial_events_for_analysis`],
    /// called once per analysis at `Element::start_analysis` time, when
    /// `self.sim.current_analysis` is finally known.
    pub(super) fn fire_initial_events(&mut self) {
        let fired: Vec<bool> = self
            .kernel
            .events()
            .iter()
            .map(|e| matches!(&e.trigger, CompiledTrigger::Initial(analyses) if analyses.is_empty()))
            .collect();
        if fired.iter().any(|&f| f) {
            let volts = vec![0.0; self.num_terminals()];
            self.apply_event_actions(&fired, &volts);
        }
    }

    /// The QUALIFIED `@(initial_step("dc", ...))` form (LRM §5.10.2,
    /// LRMW2-08) — called once per analysis, at `Element::start_analysis`
    /// time, with `analysis` (`"dc"`/`"tran"`/`"ac"` —
    /// `AnalysisKind::as_str()`) naming the analysis that is actually
    /// about to run (an explicit parameter, not `self.sim.current_analysis`
    /// — that field is only synced by `sync_sim` inside the first stamp
    /// call of the analysis, which hasn't happened yet here). Fires at
    /// zero volts, same as [`Self::fire_initial_events`] (no solve has
    /// happened yet for this analysis).
    pub fn fire_initial_events_for_analysis(&mut self, analysis: &str) {
        let fired: Vec<bool> = self
            .kernel
            .events()
            .iter()
            .map(|e| matches!(&e.trigger, CompiledTrigger::Initial(analyses)
                if !analyses.is_empty() && analyses.iter().any(|a| a == analysis)))
            .collect();
        if fired.iter().any(|&f| f) {
            let volts = vec![0.0; self.num_terminals()];
            self.apply_event_actions(&fired, &volts);
        }
    }

    /// `@(final_step)`/`@(final_step("dc", ...))` (LRM §5.10.2, LRMW2-08)
    /// — called once per analysis, at `Element::finish_analysis` time,
    /// after the last accepted point, with `analysis` naming the analysis
    /// that just ran (see [`Self::fire_initial_events_for_analysis`] for
    /// why this is an explicit parameter, not `self.sim.current_analysis`
    /// — same reasoning holds symmetrically at the END of an analysis:
    /// `finish_analysis` runs from the analysis driver itself, which knows
    /// its own kind directly, rather than round-tripping through a field
    /// only the JIT's own stamp calls keep in sync). Unlike `initial_step`,
    /// `final_step` had NO working form before this task (a pure no-op),
    /// so both the bare and qualified spellings are handled identically
    /// here — the bare form is just an always-true `analyses.is_empty()`
    /// match, LRM Table 5-1's "every analysis" default. Fires at
    /// `self.last_volts` (the last accepted solution), not zero — the body
    /// runs at the end of the analysis, with real state visible, not
    /// before it began.
    pub fn fire_final_events(&mut self, analysis: &str) {
        let fired: Vec<bool> = self
            .kernel
            .events()
            .iter()
            .map(|e| matches!(&e.trigger, CompiledTrigger::Final(analyses)
                if analyses.is_empty() || analyses.iter().any(|a| a == analysis)))
            .collect();
        if fired.iter().any(|&f| f) {
            let volts = self.last_volts().to_vec();
            self.apply_event_actions(&fired, &volts);
        }
    }

    /// Evaluate all action rows at `volts` and write the fired events'
    /// actions into the vars bank, in body order.
    pub(super) fn apply_event_actions(&mut self, fired: &[bool], volts: &[f64]) {
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

    /// Runtime events: evaluate the trigger values at the accepted solution,
    /// detect transitions, and execute fired events' variable updates.
    pub(super) fn detect_events(&mut self, volts: &[f64], time: f64) {
        let fired = self.events.detect(&self.kernel, volts, &self.params, &self.state, &self.vars, &self.sim, time);
        if fired.iter().any(|&f| f) {
            self.apply_event_actions(&fired, volts);
        }
    }
}
