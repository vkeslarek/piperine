//! The explicit model of unknowns run from MSL source on the Rust backend (feature
//! `explicit-unknowns`): one test per run-time criterion, each model written in `tests/explicit/`,
//! elaborated by the front end, compiled on the transpiled backend and run on the solver. Every
//! expected value is written from the rule it cites (part I §3.3, §5.1, §13), never read back from
//! the model.
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
#[path = "source/run.rs"]
mod run;

use run::SourceRun;

/// The model files of `tests/explicit/`.
const RUN: SourceRun = SourceRun::in_directory("tests/explicit");

#[test]
fn a_binary_carrier_is_read_as_its_value() {
    // §3.3.6 (EU-01 AC 1): `analog` reads a discrete carrier like any expression, with no `known_or`.
    // `r` holds 3.5 and `en` holds true, so the two forced nodes are 3.5 V and 2 V.
    let volts = RUN.operating_point("binary_read.msl", "Bench", &["a", "b"]);
    assert!((volts[0] - 3.5).abs() < 1e-9, "a `Bit<f64>` is read as its value: {}", volts[0]);
    assert!((volts[1] - 2.0).abs() < 1e-9, "a `Bit` is read as its value: {}", volts[1]);
}
