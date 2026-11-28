//! Param values, initial values and param history run from MSL source on the Rust backend (feature
//! `analog-structure`, AS-12, AS-15): each module is written in `tests/structure/params.msl`,
//! elaborated by the front end, compiled on the transpiled backend and run on the solver. Every
//! expected value is written from the criterion it cites, with its closed form beside the assertion.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::probe::OutputProbe;
use piperine_parity::project::Project;

/// The model file `tests/structure/params.msl`, loaded as a project.
struct ParamsModels;

impl ParamsModels {
    /// The project of `tests/structure/params.msl`.
    fn project() -> Project {
        let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/structure/params.msl");
        Project::new(&path)
    }
}


#[test]
fn a_body_let_as_a_param_value_sets_the_corner() {
    // AS-12 AC 1: `c = 1 / (2 pi r fc)` with the defaults r = 1 kOhm and fc = 1 kHz is the corner of
    // an RC low-pass: |v(o) / v(a)| = 1 / sqrt(1 + (f / fc)^2), which is 1 / sqrt 2 at f = fc = 1 kHz.
    let magnitudes = OutputProbe::new(ParamsModels::project()).ac_magnitudes("CornerBench", 1e2, 1e4, 3);
    for (frequency, magnitude) in magnitudes {
        let wanted = 1.0 / (1.0 + (frequency / 1e3).powi(2)).sqrt();
        ToleranceContract::AC_MAGNITUDE.assert(&Point::new("corner", "|v(o)|", Coordinate::Frequency(frequency)), wanted, magnitude);
    }
}

#[test]
fn a_node_initial_value_over_a_param_holds_the_operating_point() {
    // AS-15 AC 2 (§16): `signal o: Electrical = v0;` with v0 = 0.6 holds `o` at 0.6 V through the
    // transient's operating point, then the 1 uF into 1 V behind 1 kOhm (tau = 1 ms) charges it:
    // v(o) = 1 - 0.4 * exp(-t / tau): 0.6 at t = 0 and 1 - 0.4 / e = 0.8528 at 1 ms.
    let waveform = OutputProbe::new(ParamsModels::project()).transient("StartedBench", 3e-3, 10e-6);
    for time in [0.0, 1e-3, 2e-3, 3e-3] {
        let wanted = 1.0 - 0.4 * (-time / 1e-3_f64).exp();
        let actual = waveform.at(time).unwrap_or_else(|| panic!("no point at {time} s"));
        ToleranceContract::TRAN_VOLTAGE.assert(&Point::new("started", "v(o)", Coordinate::Time(time)), wanted, actual);
    }
}

#[test]
fn a_node_initial_value_over_a_param_starts_newton_in_an_operating_point() {
    // AS-15 AC 2 (§16): the node `o = v0` is where Newton starts in an `op`. The cell's current is
    // `v^3 - v`, zero at -1, 0 and 1 V: started at 0.6 V the iteration settles at +1 V, started at
    // -0.6 V at -1 V, so the solution tells the start.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("bistable", "v(o)", Coordinate::OperatingPoint), 1.0, OutputProbe::new(ParamsModels::project()).operating_point("BistableBench"));
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("bistable negative", "v(o)", Coordinate::OperatingPoint), -1.0, OutputProbe::new(ParamsModels::project()).operating_point("BistableNegBench"));
}

#[test]
fn an_integer_param_runs_with_its_value() {
    // PI-01 AC 1 to AC 3: the load is `n` mS behind 1 kOhm from 1 V, so v(o) = 1 / (1 + n): n = 4 (the default) gives 0.2 V and
    // the bound n = 7 gives 0.125 V.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("integer default", "v(o)", Coordinate::OperatingPoint), 1.0 / 5.0, OutputProbe::new(ParamsModels::project()).operating_point("IntegerDefaultBench"));
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("integer bound", "v(o)", Coordinate::OperatingPoint), 1.0 / 8.0, OutputProbe::new(ParamsModels::project()).operating_point("IntegerBoundBench"));
}

#[test]
fn an_enum_param_runs_its_arm() {
    // PI-02 AC 1, AC 2: `Fast` is 1 mS (v = 1 / 2) and `Slow` is 3 mS (v = 1 / 4).
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("enum default", "v(o)", Coordinate::OperatingPoint), 0.5, OutputProbe::new(ParamsModels::project()).operating_point("EnumDefaultBench"));
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("enum bound", "v(o)", Coordinate::OperatingPoint), 0.25, OutputProbe::new(ParamsModels::project()).operating_point("EnumBoundBench"));
}

#[test]
fn an_option_param_runs_its_case() {
    // PI-03 AC 1 to AC 3: left out or `None` it is 1 kOhm by `unwrap_or` (v = 1 / 2); `Some(3k)` is 3 kOhm (v = 3 / 4).
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("option left out", "v(o)", Coordinate::OperatingPoint), 0.5, OutputProbe::new(ParamsModels::project()).operating_point("OptionLeftOutBench"));
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("option none", "v(o)", Coordinate::OperatingPoint), 0.5, OutputProbe::new(ParamsModels::project()).operating_point("OptionNoneBench"));
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("option some", "v(o)", Coordinate::OperatingPoint), 0.75, OutputProbe::new(ParamsModels::project()).operating_point("OptionSomeBench"));
}

#[test]
fn a_payload_enum_param_runs_its_variant() {
    // PI-02, §8.1.15 ("a param's type is a value type"): an enum whose variants carry fields is a param too. The load is
    // `Conductance(2m)` by default, so v(o) = 1 / (1 + 2) = 1/3 behind 1 kOhm; bound to `Resistance(250.0)` it is 4 mS, so
    // v(o) = 1 / (1 + 4) = 0.2.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("payload default", "v(o)", Coordinate::OperatingPoint), 1.0 / 3.0, OutputProbe::new(ParamsModels::project()).operating_point("RatedDefaultBench"));
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("payload bound", "v(o)", Coordinate::OperatingPoint), 0.2, OutputProbe::new(ParamsModels::project()).operating_point("RatedResistanceBench"));
}

#[test]
fn every_read_of_an_option_param_gives_the_bound_case() {
    // PI-03 AC 3 (§3.2): `match`, `is_some` with `unwrap`, and `if let Some(r)` read the bound case of `rs` behind 1 kOhm from 1 V:
    // bound to 3k (a plain value, `Some` of it by §3.2.3) the load is 3 kOhm and v(o) = 3 / 4; left out it is `None`, the
    // 1 kOhm of the other arm, and v(o) = 1 / 2.
    for form in ["Match", "IsSome", "IfLet"] {
        ToleranceContract::DC_VOLTAGE.assert(&Point::new(form, "v(o) bound", Coordinate::OperatingPoint), 0.75, OutputProbe::new(ParamsModels::project()).operating_point(&format!("{form}SomeBench")));
        ToleranceContract::DC_VOLTAGE.assert(&Point::new(form, "v(o) none", Coordinate::OperatingPoint), 0.5, OutputProbe::new(ParamsModels::project()).operating_point(&format!("{form}NoneBench")));
    }
}

#[test]
fn a_composite_param_needs_no_repr_and_no_read_by_analog() {
    // §8.1.15 ("a param's type is a value type"; only a carried type needs `Repr`, §1): a struct that derives nothing, holding an `f64`,
    // is a param no `analog` block reads, and the design compiles and runs: v(o) = 1 kOhm / 2 kOhm = 1/2.
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("held rating", "v(o)", Coordinate::OperatingPoint), 0.5, OutputProbe::new(ParamsModels::project()).operating_point("HeldRatingBench"));
}

#[test]
fn a_unit_enum_with_no_derive_is_a_param() {
    // §8.1.15 (a param's type is a value type; only a carried type needs `Repr`), PI-02 AC 1: `enum Gear { Low, High }` derives nothing
    // and is read by `match`: `Low` is 1 mS behind 1 kOhm (v(o) = 1/2), `High` is 3 mS (v(o) = 1/4).
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("gear low", "v(o)", Coordinate::OperatingPoint), 0.5, OutputProbe::new(ParamsModels::project()).operating_point("GearLowBench"));
    ToleranceContract::DC_VOLTAGE.assert(&Point::new("gear high", "v(o)", Coordinate::OperatingPoint), 0.25, OutputProbe::new(ParamsModels::project()).operating_point("GearHighBench"));
}
