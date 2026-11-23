//! Handle functions run from MSL source on the Rust backend (feature `generics`, GN-10 to GN-12): each model is written in
//! `tests/handles/`, elaborated by the front end, compiled on the transpiled backend and run on the solver. Every expected
//! value is computed here by Rust from the rule the test cites, never read back from the model.
#![cfg(unix)]

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
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

