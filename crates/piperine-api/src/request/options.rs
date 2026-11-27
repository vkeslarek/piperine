//! The session options of a run, read for every analysis (HII-01).

use piperine_digital_solver::landing::TimePointRule;

/// The session options of a run, read for every analysis (HII-01): ngspice's `.options` names,
/// lowercase. `None` leaves the solver's own default, so a run that gives none runs as it did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Options {
    /// The relative tolerance of a Newton iterate and of a step's error.
    pub reltol: Option<f64>,
    /// The absolute tolerance of a flow, amperes.
    pub abstol: Option<f64>,
    /// The absolute tolerance of a potential, volts.
    pub vntol: Option<f64>,
    /// The factor on the local truncation error a transient accepts.
    pub trtol: Option<f64>,
    /// The temperature of the run in degrees Celsius.
    pub temp: Option<f64>,
    /// The session's tick length in seconds.
    pub resolution: Option<f64>,
    /// The bound on the rounds of an operating point.
    pub maxrounds: Option<u32>,
    /// The bound on the microsteps of one instant.
    pub maxmicro: Option<u32>,
    /// The budget of attempts a transient may make before it fails; the solver's 2,000,000 when
    /// not given.
    pub max_steps: Option<u32>,
    /// The node values that seed the first Newton iterate of an operating point: a path and a
    /// value each, in the order given.
    pub nodeset: Vec<(String, f64)>,
    /// The session parameters a design reads with `simparam("name")`, in the order given: a name
    /// and a value each (the key `simparam`, the flag `--simparam`).
    pub session_parameters: Vec<(String, f64)>,
    /// Which scheduled instants the digital engine lands the analog step on. A host's choice of
    /// the solver's reference mode (`Declared`): no run key, no flag names it, so a run table never
    /// gives it and it stays `None` (the solver's `Observable`).
    pub time_point_rule: Option<TimePointRule>,
}
