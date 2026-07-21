//! Events capability: runtime analog event detection (`cross`/`above`/
//! `@timer`) and the `@initial`/fired-event variable-action dispatch.

use crate::emit::abi::SimCtx;
use crate::error::CodegenError;
use crate::kernel::analog::{AnalogKernel, CompiledTrigger};
use crate::resolve::CrossDir;

use super::AnalogInstance;

/// Per-event transition detector: remembers the previous accepted trigger
/// value (crossings) or the next fire time (timers).
struct EventDetector {
    /// A trigger value has been observed (crossing detection is armed).
    seeded: bool,
    prev: f64,
    next_fire: f64,
}

impl EventDetector {
    /// Whether the event fires given the trigger value at the accepted
    /// solution, updating the detector state.
    fn fired(&mut self, trigger: &CompiledTrigger, value: f64, time: f64, period: f64) -> bool {
        let fired = match trigger {
            // Fired once at instance creation, never here.
            CompiledTrigger::Initial => false,
            CompiledTrigger::Above => {
                let rose = if self.seeded { self.prev <= 0.0 } else { true };
                rose && value > 0.0
            }
            CompiledTrigger::Cross(dir) => {
                let rising = self.seeded && self.prev <= 0.0 && value > 0.0;
                let falling = self.seeded && self.prev >= 0.0 && value < 0.0;
                match dir {
                    CrossDir::Rising => rising,
                    CrossDir::Falling => falling,
                    CrossDir::Either => rising || falling,
                }
            }
            CompiledTrigger::Timer { .. } => {
                let fires = period > 0.0 && time >= self.next_fire;
                if fires {
                    while self.next_fire <= time {
                        self.next_fire += period;
                    }
                }
                fires
            }
        };
        self.prev = value;
        self.seeded = true;
        fired
    }
}

/// One detector per runtime event, in kernel event order, plus each event's
/// (parameter-constant) timer period (`0` for non-timers).
pub(super) struct EventSystem {
    detectors: Vec<EventDetector>,
    periods: Vec<(f64, f64)>,
}

impl EventSystem {
    pub(super) fn new(kernel: &AnalogKernel, params: &[f64]) -> Result<Self, CodegenError> {
        // Timer periods are parameter-constant, evaluated once.
        let periods = kernel
            .events()
            .iter()
            .map(|e| match &e.trigger {
                CompiledTrigger::Timer { period, phase } => {
                    let param_names = kernel.param_names();
                    let resolve = |name: &str| -> Option<f64> {
                        param_names.iter().position(|n| n == name)
                            .and_then(|i| params.get(i).copied())
                    };
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
        let detectors = kernel
            .events()
            .iter()
            .zip(&periods)
            .map(|(_, &(_period, first_fire))| EventDetector { seeded: false, prev: 0.0, next_fire: first_fire })
            .collect();
        Ok(Self { detectors, periods })
    }

    /// Absolute landing points this instance's `@timer` events fire at
    /// within `(from, end]` — each timer fires every `period` (its current
    /// `next_fire` advanced into the window). Non-timer events (crossings)
    /// are detected reactively and contribute no static breakpoints here.
    pub(super) fn next_breakpoints(&self, from: f64, end: f64, out: &mut Vec<f64>) {
        for (det, &(period, _first_fire)) in self.detectors.iter().zip(&self.periods) {
            if period <= 0.0 || !period.is_finite() {
                continue;
            }
            // First fire strictly after `from` (next_fire may lag if the timer
            // hasn't been advanced past the current step yet).
            let mut t = det.next_fire;
            while t <= from {
                t += period;
            }
            while t <= end {
                out.push(t);
                t += period;
            }
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
            let trigger = &kernel.events()[i].trigger;
            fired[i] = detector.fired(trigger, triggers[i], time, self.periods[i].0);
        }
        fired
    }
}

impl AnalogInstance {
    /// Execute `@ initial` event actions once, at zero volts (before any
    /// solve, only parameters and power-on variable values are visible).
    pub(super) fn fire_initial_events(&mut self) {
        let fired: Vec<bool> = self
            .kernel
            .events()
            .iter()
            .map(|e| matches!(e.trigger, CompiledTrigger::Initial))
            .collect();
        if fired.iter().any(|&f| f) {
            let volts = vec![0.0; self.num_terminals()];
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
