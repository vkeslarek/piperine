//! Templates run from MSL source on the Rust backend (feature `generics` phase D, GN-21 to GN-23, part III §8.4): each model is
//! written in `tests/templates/`, elaborated by the front end, compiled on the transpiled backend and run on the solver. Every
//! expected value is computed here by Rust from the rule the test cites, never read back from the model.
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

/// The model files of `tests/templates/`.
const RUN: SourceRun = SourceRun::in_directory("tests/templates");

/// The tolerance of an operating point of sources and unit gains.
const TOLERANCE: f64 = 1e-9;

/// Rust's own value of the models: what a run is compared with.
struct RustOracle;

impl RustOracle {
    /// `Dram`: the storage node follows the bit line scaled by `cs`.
    fn storage(cs: f64, bit_line: f64) -> f64 {
        cs * bit_line
    }

    /// `nch` of the model `binning.msl`: the `vth0` the length bins to (§8.4.7).
    fn bin(length: f64) -> f64 {
        if length < 0.18e-6 {
            0.42
        } else if length < 1e-6 {
            0.45
        } else {
            0.47
        }
    }

    /// `Square`: `k * g^2` at the gate voltage, and the gain its `impl Mosfet` gives, `2 k`.
    fn square(k: f64, gate: f64) -> (f64, f64) {
        (k * gate * gate, k * 2.0)
    }

    /// `Linear`: `k * g` at the gate voltage, and the gain its `impl Mosfet` gives, `k / 2`.
    fn linear(k: f64, gate: f64) -> (f64, f64) {
        (k * gate, k * 0.5)
    }
}

#[test]
fn a_memory_bank_places_its_cell_template() {
    // GN-21, GN-23 AC 3 (§8.4.8): the bank places `cell` once per row and column; `bank.cells[r][c].sn` of two different
    // cells reads the storage node of the `Dram` the template names, at its own `cs`
    let seen = RUN.operating_point("memory.msl", "Rig", &["a", "first", "last", "bank.cells[0][0].sn", "bank.cells[1][2].sn", "bank.cells[0][1].sn"]);
    let wanted = RustOracle::storage(30.0, 0.5);
    for (value, name) in seen.iter().skip(1).zip(["first", "last", "cells[0][0]", "cells[1][2]", "cells[0][1]"]) {
        assert!((value - wanted).abs() < TOLERANCE, "{name}: {value} is not {wanted}: {seen:?}");
    }
}

#[test]
fn binning_places_the_chosen_model() {
    // GN-22 AC 3 (§8.4.7): each length places the `Bsim` of its own bin, and the bin's `vth0` is the node its model drives
    let seen = RUN.operating_point("binning.msl", "Rig", &["short", "middle", "long"]);
    for (value, length) in seen.iter().zip([0.13e-6, 0.5e-6, 2e-6]) {
        assert!((value - RustOracle::bin(length)).abs() < TOLERANCE, "length {length}: {value}: {seen:?}");
    }
}

#[test]
fn the_swappable_mosfet_runs_each_model() {
    // GN-23 (the decision of 2026-11-22): `DiffPair` over `N: Mosfet + Fn(..)` gives each model its own operating point and its
    // own `gain()`, the `impl` of the module the template names
    let gate = 0.5;
    let square = RUN.operating_point("mosfet.msl", "RigSquare", &["d1", "d2", "s"]);
    let linear = RUN.operating_point("mosfet.msl", "RigLinear", &["d1", "d2", "s"]);
    for (seen, (response, gain)) in [(square, RustOracle::square(3.0, gate)), (linear, RustOracle::linear(3.0, gate))] {
        assert!((seen[0] - response).abs() < TOLERANCE && (seen[1] - response).abs() < TOLERANCE, "{seen:?} against {response}");
        assert!((seen[2] - gain).abs() < TOLERANCE, "{seen:?} against gain {gain}");
    }
}
