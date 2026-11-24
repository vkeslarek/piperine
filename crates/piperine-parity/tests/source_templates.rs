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

    /// The middle of a divider of the conductances `1 / upper` and `1 / lower` driven at `drive`.
    fn divider(drive: f64, upper: f64, lower: f64) -> f64 {
        drive * lower / (upper + lower)
    }

    /// The middle of a divider of the conductances `upper` and `lower` driven at `drive`.
    fn conductance_divider(drive: f64, upper: f64, lower: f64) -> f64 {
        drive * upper / (upper + lower)
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

#[test]
fn a_param_default_reads_the_params_before_it() {
    // §8.1.16: `g: f64 = 1.0 / r` is folded per instance over the `r` the placement binds, so the divider of `r = 1` over `r = 3`
    // sits at 3/4 of the drive (with the module's `g = 1` for both it would sit at 1/2); `level: f64 = fet.gain()` reads the
    // template param `fet` through the trait of `N: Mosfet + Fn(..)` (§8.4.5)
    let divider = RUN.operating_point("defaults.msl", "RigDivider", &["mid"]);
    let expected = RustOracle::divider(1.0, 1.0, 3.0);
    assert!((divider[0] - expected).abs() < TOLERANCE, "{divider:?} against {expected}");
    let gain = RUN.operating_point("defaults.msl", "RigGain", &["d", "s"]);
    let (response, level) = RustOracle::square(3.0, 0.5);
    assert!((gain[0] - response).abs() < TOLERANCE && (gain[1] - level).abs() < TOLERANCE, "{gain:?} against {response} and {level}");
}

#[test]
fn a_template_of_a_nature_generic_module_runs_through_its_type() {
    // §8.4.3: `Load<Electrical>` names the nature of the ports, so the `const` `HALF` (g = 0.5) and the param `leg` (g = 2) place
    // conductances between nodes of `Electrical`: the divider sits at 0.5 / (0.5 + 2) of the drive
    let mid = RUN.operating_point("natures.msl", "RigNatures", &["mid"]);
    let expected = RustOracle::conductance_divider(1.0, 0.5, 2.0);
    assert!((mid[0] - expected).abs() < TOLERANCE, "{mid:?} against {expected}");
}

#[test]
fn gnd_connects_through_a_callable_bound() {
    // §8.1.10, §8.4.5: `leg(mid, gnd)` through `C: Fn(Electrical, Electrical)` grounds the lower leg as a placement does, so the
    // divider sits where `RigNatures` does
    let mid = RUN.operating_point("natures.msl", "RigBound", &["mid"]);
    let expected = RustOracle::conductance_divider(1.0, 0.5, 2.0);
    assert!((mid[0] - expected).abs() < TOLERANCE, "{mid:?} against {expected}");
}

#[test]
fn the_items_of_a_template_placed_child_read_as_values() {
    // §8.4.8: `c0.cs + c0.twice` in the bank and `bank.c0.cs`, `bank.cells[1].cs` from the rig read the param and the `pub let` of
    // the `Dram` the template names, at its `cs = 3`: 3 + 2 * 3, 3 and 10 * 3
    let cs = 3.0;
    let seen = RUN.operating_point("items.msl", "RigItems", &["level", "own", "row"]);
    for (value, wanted) in seen.iter().zip([cs + 2.0 * cs, cs, 10.0 * cs]) {
        assert!((value - wanted).abs() < TOLERANCE, "{value} is not {wanted}: {seen:?}");
    }
}
