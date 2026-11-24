//! The session parameters of a run (analog-operators AO-07 and AO-08, design mechanism 5): what a
//! design reads with `simparam(name)`. The host gives every run `reltol`, `abstol`,
//! `vntol`, `trtol` (the run's, else the solver's defaults) and `temp` in degrees Celsius (27 when
//! absent), then each entry of the run's `simparam` table (D13). There is no `tnom` here: user
//! decision 2026-11-23, `tnom` is a per-instance model param, not a session value. A design that reads
//! none gets none: the circuit binds as it always did. A name the run lacks reads `None` (user
//! decision 2026-11-23: `simparam(name)` is an `Option<f64>`, no `E0456`).

use piperine_ir::Ir;

use crate::request::Options;
use crate::session::Session;

/// The session parameters a run gives its design.
pub(crate) struct SessionParameters;

impl SessionParameters {
    /// The Celsius temperature `temp` reads when the run gives none.
    const ROOM: f64 = 27.0;

    /// The parameters to hand the circuit: empty when no instance of `ir` reads one, else the
    /// five options and the run's `simparam` entries, in that order. A name the run does not give
    /// reads `None` in the design (user decision 2026-11-23: `simparam(name)` is an `Option<f64>`,
    /// no `E0456`).
    pub(crate) fn of(ir: &Ir, options: &Options) -> Vec<(String, f64)> {
        let reads: Vec<_> = ir.instances().flat_map(|instance| instance.body().session_parameters().iter().map(move |p| (instance.path().to_string(), p))).collect();
        if reads.is_empty() {
            return Vec::new();
        }
        let tolerances = Session::context_of(options).tolerances;
        let mut given = vec![
            ("reltol".to_string(), tolerances.reltol),
            ("abstol".to_string(), tolerances.abstol),
            ("vntol".to_string(), tolerances.vntol),
            ("trtol".to_string(), tolerances.trtol),
            ("temp".to_string(), options.temp.unwrap_or(Self::ROOM)),
        ];
        given.extend(options.session_parameters.iter().cloned());
        given
    }
}
