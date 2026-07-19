//! The solver's config home: every tunable schedule/gain/trace knob as typed,
//! defaulted, documented fields. This module is **data** — no logic beyond
//! construction. `GminSchedule`/`SourceSchedule` parameterize the homotopy
//! strategies (owned by [`ConvergencePlan`](crate::solver::convergence::ConvergencePlan)),
//! `StepperGains` parameterizes the transient PI stepper, and `TraceFlags`
//! carries the diagnostic toggles that used to be read from `PIPERINE_TRACE_*`
//! env vars inline. Every default equals the literal it replaced (SS-09 parity).

/// Gmin-stepping homotopy schedule (SPICE gmin stepping). The defaults are the
/// literals that used to live inline in `GminStepping::converge`.
#[derive(Debug, Clone, Copy)]
pub struct GminSchedule {
    /// Starting extra conductance to ground (100 mS — the "easy" problem).
    pub start_g: f64,
    /// Initial multiplicative step (one decade per converged solve).
    pub decade_factor: f64,
    /// Growth applied to the step factor after each converged solve.
    pub relax_growth: f64,
    /// Cap on the relaxed step factor.
    pub relax_cap: f64,
    /// Growth applied to the step factor when a solve fails (back-off: raise
    /// g, shrink the step).
    pub backoff_growth: f64,
    /// Cap on the backed-off step factor.
    pub backoff_cap: f64,
    /// Bound on stepping iterations so a truly non-convergent circuit still
    /// terminates.
    pub max_steps: usize,
    /// Stop ramping once the extra conductance is below `gmin_floor ×` this
    /// margin (negligible next to the real device gmin).
    pub floor_margin: f64,
}

impl Default for GminSchedule {
    fn default() -> Self {
        Self {
            start_g: 0.1,
            decade_factor: 0.1,
            relax_growth: 1.3,
            relax_cap: 0.5,
            backoff_growth: 3.0,
            backoff_cap: 0.7,
            max_steps: 200,
            floor_margin: 10.0,
        }
    }
}

/// Source-stepping homotopy schedule (SPICE source stepping). The defaults are
/// the literals that used to live inline in `SourceStepping::converge`.
#[derive(Debug, Clone, Copy)]
pub struct SourceSchedule {
    /// Shunt conductance conditioning the exponential turn-on knee (the
    /// BJT/MOS threshold), held through the ramp then itself ramped out.
    pub knee_gmin: f64,
    /// First source-scale increment.
    pub start_step: f64,
    /// Growth applied to the step after each converged solve.
    pub step_growth: f64,
    /// Cap on the grown step.
    pub step_cap: f64,
    /// Factor a failed step shrinks by (back-off toward the last converged
    /// scale).
    pub backoff_factor: f64,
    /// Below this step the back-off is exhausted and the strategy gives up.
    pub min_step: f64,
    /// Bound on ramping iterations.
    pub max_steps: usize,
    /// The nested knee ramp-out stops once the shunt is below `gmin_floor ×`
    /// this margin.
    pub floor_margin: f64,
    /// Multiplicative decay of the nested knee ramp-out (one decade per
    /// converged solve).
    pub knee_decay: f64,
}

impl Default for SourceSchedule {
    fn default() -> Self {
        Self {
            knee_gmin: 1e-6,
            start_step: 0.1,
            step_growth: 1.5,
            step_cap: 0.25,
            backoff_factor: 0.5,
            min_step: 1e-6,
            max_steps: 300,
            floor_margin: 10.0,
            knee_decay: 0.1,
        }
    }
}

/// The two homotopy schedules as one family — the config a [`ConvergencePlan`]
/// owns and hands to each [`HomotopyStrategy`] it drives.
///
/// [`ConvergencePlan`]: crate::solver::convergence::ConvergencePlan
/// [`HomotopyStrategy`]: crate::solver::convergence::HomotopyStrategy
#[derive(Debug, Clone, Copy, Default)]
pub struct Schedules {
    pub gmin: GminSchedule,
    pub source: SourceSchedule,
}

/// PI timestep-controller gains (transient). The defaults are the literals
/// that used to live inline in `PiController::{propose_dt, reject_dt}` and its
/// `Default` impl.
#[derive(Debug, Clone, Copy)]
pub struct StepperGains {
    /// Proportional gain (ngspice lineage).
    pub kp: f64,
    /// Integral gain on the error history.
    pub ki: f64,
    /// dt growth when there is no usable error signal (non-reactive step or
    /// short history).
    pub grow_factor: f64,
    /// Divisor applied to a rejected step (aggressive backtracking).
    pub reject_divisor: f64,
    /// Safe per-step clamp on the PI-computed growth/shrink factor.
    pub factor_clamp: (f64, f64),
}

impl Default for StepperGains {
    fn default() -> Self {
        Self {
            kp: 0.7,
            ki: 0.4,
            grow_factor: 1.5,
            reject_divisor: 8.0,
            factor_clamp: (0.2, 1.5),
        }
    }
}

/// Diagnostic trace toggles (SS-08). Replaces the inline
/// `PIPERINE_TRACE_{GMIN,SRC,TRAN}` env reads: the env vars seed the flags via
/// [`TraceFlags::from_env`], and the code paths read the typed fields.
/// Default-off, matching the previous unset-env behavior.
#[derive(Debug, Clone, Copy, Default)]
pub struct TraceFlags {
    /// Trace gmin-stepping solves (`PIPERINE_TRACE_GMIN`).
    pub gmin: bool,
    /// Trace source-stepping ramp steps (`PIPERINE_TRACE_SRC`).
    pub source: bool,
    /// Trace transient LTE rejections (`PIPERINE_TRACE_TRAN`).
    pub transient: bool,
}

impl TraceFlags {
    /// Seed the flags from the `PIPERINE_TRACE_{GMIN,SRC,TRAN}` env vars
    /// (present = on), preserving the pre-config toggle mechanism.
    pub fn from_env() -> Self {
        Self {
            gmin: std::env::var("PIPERINE_TRACE_GMIN").is_ok(),
            source: std::env::var("PIPERINE_TRACE_SRC").is_ok(),
            transient: std::env::var("PIPERINE_TRACE_TRAN").is_ok(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parity by construction: every config default equals the inline literal
    /// it replaces (SS-09).
    #[test]
    fn defaults_equal_the_literals_they_replace() {
        let gmin = GminSchedule::default();
        assert_eq!(gmin.start_g, 0.1);
        assert_eq!(gmin.decade_factor, 0.1);
        assert_eq!(gmin.relax_growth, 1.3);
        assert_eq!(gmin.relax_cap, 0.5);
        assert_eq!(gmin.backoff_growth, 3.0);
        assert_eq!(gmin.backoff_cap, 0.7);
        assert_eq!(gmin.max_steps, 200);
        assert_eq!(gmin.floor_margin, 10.0);

        let source = SourceSchedule::default();
        assert_eq!(source.knee_gmin, 1e-6);
        assert_eq!(source.start_step, 0.1);
        assert_eq!(source.step_growth, 1.5);
        assert_eq!(source.step_cap, 0.25);
        assert_eq!(source.backoff_factor, 0.5);
        assert_eq!(source.min_step, 1e-6);
        assert_eq!(source.max_steps, 300);
        assert_eq!(source.floor_margin, 10.0);
        assert_eq!(source.knee_decay, 0.1);

        let gains = StepperGains::default();
        assert_eq!(gains.kp, 0.7);
        assert_eq!(gains.ki, 0.4);
        assert_eq!(gains.grow_factor, 1.5);
        assert_eq!(gains.reject_divisor, 8.0);
        assert_eq!(gains.factor_clamp, (0.2, 1.5));

        // Tracing is default-off (the previous unset-env behavior); the env
        // vars only seed the flags through `from_env`.
        let trace = TraceFlags::default();
        assert!(!trace.gmin);
        assert!(!trace.source);
        assert!(!trace.transient);
    }
}
