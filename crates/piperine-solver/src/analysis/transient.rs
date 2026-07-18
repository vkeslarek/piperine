#![allow(dead_code)]
use crate::analog::AnalogReference;
use crate::digital::LogicValue;
use crate::math::circular_array::CircularArrayBuffer2;
use crate::math::iv::InitialValue;
use crate::math::linear::Stamp;
use crate::math::unit::Second;
use crate::solver::Context;
use std::ops::Deref;

/// The read-only state an element sees while stamping the transient system: the
/// analog solution history **and** the digital net snapshot it may read (D2A,
/// no device-side cache). Derefs to the analog history buffer.
pub struct TransientAnalysisState<'a> {
    history: &'a CircularArrayBuffer2<f64>,
    /// Every digital net's logic value for this step, indexed by `DigitalNet`.
    pub digital: &'a [LogicValue],
}

impl<'a> TransientAnalysisState<'a> {
    pub fn new(history: &'a CircularArrayBuffer2<f64>, digital: &'a [LogicValue]) -> Self {
        Self { history, digital }
    }

    /// The analog solution history buffer.
    pub fn history(&self) -> &CircularArrayBuffer2<f64> {
        self.history
    }
}

impl Deref for TransientAnalysisState<'_> {
    type Target = CircularArrayBuffer2<f64>;
    fn deref(&self) -> &Self::Target {
        self.history
    }
}

#[derive(Clone)]
pub struct TransientAnalysisOptions {
    /// Simulation stop time
    pub stop_time: Second,

    /// Initial timestep for the adaptive stepper (SPICE has been adaptive
    /// since v2; the integrator varies `dt` from here via the PI controller).
    /// A user-supplied `.step` becomes this initial value.
    pub dt: Second,

    /// Minimum allowed timestep (default: 1e-15 seconds)
    pub dt_min: Second,

    /// Maximum allowed timestep (default: stop_time / 100)
    pub dt_max: Second,

    /// Earliest time at which a step is *recorded* (host `run_tran` `start`
    /// `TranConfig.start`). The solver still integrates from t=0 — the state
    /// evolution matters — but steps with `t < record_from` are dropped from
    /// the result (ngspice `.tran tstart tstop` semantics). Defaults to 0
    /// (record everything, the pre-existing behavior).
    pub record_from: Second,

    /// Simulation start time (default 0). The integrator's clock starts
    /// here — `$abstime`, breakpoints, and scheduled sets are all absolute
    /// times. Used by a host restarting a transient from `t` after a
    /// structural rebuild (LIVE-16); the starting state comes from the
    /// initial operating point overlaid with `apply_initial_conditions`.
    pub start_time: Second,
}

impl TransientAnalysisOptions {
    /// Create transient options. The integrator is always adaptive (PI
    /// controller); `dt` is the initial step size, grown/shrunk from there.
    pub fn new(stop_time: Second, dt: Second) -> Self {
        Self {
            stop_time,
            dt,
            dt_min: 1e-15,
            dt_max: (stop_time / 100.0),
            record_from: 0.0,
            start_time: 0.0,
        }
    }

    /// Set the simulation start time (restart-from-`t` semantics).
    pub fn with_start(mut self, start_time: Second) -> Self {
        self.start_time = start_time;
        self
    }

    /// Set minimum timestep
    pub fn with_dt_min(mut self, dt_min: Second) -> Self {
        self.dt_min = dt_min;
        self
    }

    /// Set maximum timestep
    pub fn with_dt_max(mut self, dt_max: Second) -> Self {
        self.dt_max = dt_max;
        self
    }

    /// Set the earliest recorded time (`TranConfig.start`).
    pub fn with_record_from(mut self, record_from: Second) -> Self {
        self.record_from = record_from;
        self
    }
}

/// Per-analysis config for transient. Built from
/// [`TransientAnalysisOptions`] via `From`. Carries the tunables that
/// used to be on the global `Context` (MD-03).
#[derive(Debug, Clone)]
pub struct TransientContext {
    pub dt: f64,
    pub dt_min: f64,
    pub dt_max: f64,
    pub record_from: f64,
    pub stop_time: f64,
}

impl From<TransientAnalysisOptions> for TransientContext {
    fn from(opts: TransientAnalysisOptions) -> Self {
        Self {
            dt: opts.dt,
            dt_min: opts.dt_min,
            dt_max: opts.dt_max,
            record_from: opts.record_from,
            stop_time: opts.stop_time,
        }
    }
}

/// Per-step transient context handed to the kernel. Carries the TR-BDF2
/// phase being stamped and the step sizes; the kernel calls
/// `TrBdf2::phase_coeffs(phase, h)` for the reactive companion — there is no
/// method-selection surface (TR-BDF2 is the sole integration scheme).
#[derive(Clone, Copy)]
pub struct TransientAnalysisContext {
    pub time: Second,
    pub tfinal: Second,
    /// Which sub-step the kernel is stamping: [`Trapezoidal`][TrBdf2Phase::Trapezoidal]
    /// over `γh` (solving for `x_{n+γ}`) or [`Bdf2`][TrBdf2Phase::Bdf2] over
    /// `(1−γ)h` (solving for `x_{n+1}` from `x_{n+γ}` and `x_n`).
    pub phase: crate::math::integration::TrBdf2Phase,
    /// The full step size `h = t_{n+1} − t_n`. The companion sub-step (`γh` or
    /// `(1−γ)h`) is derived from `phase` inside `TrBdf2::phase_coeffs`.
    pub h: Second,
    /// The previous accepted step size. The TR stage's trapezoidal companion
    /// needs the capacitor current at `t_n`, which the kernel re-derives from
    /// the prior step's BDF2 formula using this. Zero on the first step (no
    /// history → no current, matching the DC operating point).
    pub prev_h: Second,
}

pub trait TransientAnalysis {
    fn load_transient(
        &mut self,
        circuit_states: &TransientAnalysisState<'_>,
        transient_analysis_context: &TransientAnalysisContext,
        context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>>;

    fn load_transient_dynamic(
        &mut self,
        _circuit_states: &TransientAnalysisState<'_>,
        _transient_analysis_context: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        vec![]
    }

    fn initial_transient_values(
        &mut self,
        _context: &Context,
    ) -> Vec<InitialValue<AnalogReference, f64>> {
        Vec::new()
    }
}



#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use crate::prelude::TransientStep;
    use crate::core::net::Net;
    use crate::digital::LogicValue;
    use std::collections::HashMap;
    use crate::analog::{AnalogReference, AnalogVariable, NodeIdentifier};
    use std::sync::Arc;

    #[test]
    fn transient_step_lookup_by_net_returns_analog_and_digital_values() {
        let var: Arc<AnalogVariable> = Arc::new(AnalogVariable::Node(NodeIdentifier::Anonymous(0)));
        let mut values = HashMap::new();
        values.insert(var.clone(), 1.25);
        let step = TransientStep::new(0.0, values).with_digital(vec![LogicValue::One, LogicValue::Zero]);

        let analog_net: Net = (&AnalogReference::new(var.clone(), 0)).into();
        assert_eq!(step.get_net(&analog_net), Some(1.25));

        let digital_net = Net::digital(1, "top.clk");
        assert_eq!(step.digital_net(&digital_net), Some(LogicValue::Zero));
        assert_eq!(step.digital_net(&Net::digital(0, "d0")), Some(LogicValue::One));

        // Wrong kind returns None — analog_net is not a digital net.
        assert_eq!(step.digital_net(&analog_net), None);

        // Digital net past the recorded snapshot returns None.
        assert_eq!(step.digital_net(&Net::digital(99, "x")), None);
    }
}
