//! Handle functions run from MSL source on the Rust backend (feature `generics`, GN-10 to GN-12): each model is written in
//! `tests/handles/`, elaborated by the front end, compiled on the transpiled backend and run on the solver. Every expected
//! value is computed here by Rust from the rule the test cites, never read back from the model.
#![cfg(unix)]

#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;
#[allow(dead_code)]
#[path = "source/bytes.rs"]
mod bytes;
#[allow(dead_code)]
#[path = "source/operators.rs"]
mod operators;
#[allow(dead_code)]
#[path = "source/run.rs"]
mod run;

use bytes::CarrierBytes;
use run::SourceRun;

/// The model files of `tests/handles/`.
const RUN: SourceRun = SourceRun::in_directory("tests/handles");

/// The tolerance of a node voltage at a linear operating point.
const VOLT: f64 = 1e-9;

#[test]
fn impl_thermal_celsius_reads_temp_minus_273_15() {
    // §3.1, GN-10: `impl Thermal { fn celsius(self) }` gives `temp(self) - 273.15` at every point of a sweep of the held
    // temperature, and the `impl` of a project nature (`bar` of `Pressure`) runs the same way
    for (bench, kelvin) in [("Cold", 250.0), ("Room", 300.0), ("Warm", 350.0)] {
        let measured = RUN.operating_point("thermal.msl", bench, &["o"])[0];
        assert!((measured - (kelvin - 273.15)).abs() < VOLT, "{bench}: {measured}");
    }
    let bar = RUN.operating_point("thermal.msl", "Plant", &["o"])[0];
    assert!((bar - 250000.0 / 100000.0).abs() < VOLT, "{bar}");
}

#[test]
fn a_nature_generic_handle_function_instantiates_per_nature() {
    // GN-11 AC 4: `level<N: Nature>` called with an `Electrical` node gives its potential, with a `Thermal` one its
    // temperature; `diff(a, b)` is the solved difference of two nodes (GN-12 AC 1, from `analog`)
    let seen = RUN.operating_point("probing.msl", "AnalogBench", &["o1", "o2", "o3"]);
    for (found, wanted) in seen.iter().zip([3.0 - 1.25, 3.0, 321.0]) {
        assert!((found - wanted).abs() < VOLT, "{seen:?}");
    }
}

#[test]
fn a_handle_is_read_probed_passed_on_and_has_carrier_methods() {
    // GN-11 AC 1, §3.1.5: at the first rising edge `held(clk)` reads the carrier (true), `before(clk)` calls `prev()` on it
    // (false, the value before the change), `either` passes it on to both, and `diff` probes two nodes passed on
    let (current, previous) = (true, false);
    RUN.assert_commits(
        "discrete.msl",
        &["r0", "r1", "r2"],
        &[CarrierBytes::byte(u8::from(current)), CarrierBytes::byte(u8::from(previous)), CarrierBytes::byte(u8::from(current || previous))],
    );
}

#[test]
fn a_discrete_handle_reads_the_value_with_its_flags() {
    // GN-11 AC 3, §3.1.6, as MD-78 amends it (a value carries no flag): reading a handle of a discrete carrier gives the
    // carrier's value, `true` at a rising edge and not what it was before
    RUN.assert_commits("discrete.msl", &["r0"], &[CarrierBytes::byte(1)]);
}

#[test]
fn a_probing_function_runs_from_analog_and_a_reaction() {
    // GN-12 AC 1: `diff(a, b)` called from a reaction reads the solved nodes at the edge: 2.5 - 0.75
    RUN.assert_commits("discrete.msl", &["r3"], &[CarrierBytes::real(2.5 - 0.75)]);
}

#[test]
fn a_handle_generic_over_the_value_of_its_carrier_calls_prev() {
    // GN-11 AC 1: `earlier<T: Repr>(a: Bit<T>) -> T { a.prev() }` is a handle of any value type: for `clk` (`bool`) it is
    // the value before the rising edge, for a `Bit<u8>` that never changed the value it started with
    RUN.assert_commits("discrete.msl", &["r4", "r5"], &[CarrierBytes::byte(0), CarrierBytes::byte(7)]);
}
