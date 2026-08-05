//! ngspice golden cross-validation of the **source-stepping homotopy**
//! (NGP-11): the independent sources honour ngspice's `CKTsrcFact`.
//!
//! Both fixtures here are bias points neither plain Newton nor gmin stepping
//! can reach — measured on each: gmin stepping reports "GMIN step
//! g=1.000e-1 -> fail" and the operating point is then reached by the
//! source-stepping ramp. A ramp only exists while the independent sources
//! scale with it, so each fixture falsifies one half of the contract:
//!
//! * `homotopy_bias` — `vsrc` (`vsrcload.c`): the forced value is scaled on
//!   its MNA force row. Stop scaling it and every ramp step solves the
//!   fully-driven ladder from a cold start; the fixture fails.
//! * `homotopy_sink` — `isrc` (`isrcload.c`): the DC value is scaled through
//!   `$simparam("sourceScaleFactor")` in the model body. Stop scaling it and
//!   the sink drains a ladder the ramp has only partially driven; the fixture
//!   fails.
//!
//! The two need different ladder stiffness because the sources break the ramp
//! in different ways, so neither fixture alone falsifies both halves.
//!
//! The comparison is a DC operating point and reads `ToleranceContract::DC`
//! through the shared harness — no tolerance number is retyped here (NGP-07).
//! Alongside it each case asserts, from piperine's own `OpResult`, that the
//! ladder is *conducting* at the solution: a fixture whose diodes sat off
//! would agree with ngspice at ~0 V everywhere and would not be the hard bias
//! point this target exists to hold. Without `ngspice` on PATH both cases
//! print a SKIP notice and pass (NGP-06).

use piperine::OpResult;

mod support;

use support::ngspice::NgspiceHarness;

/// The forward drop each ladder tap must clear for the diodes below it to be
/// in the exponential region — the property that makes these bias points
/// hard, and so the property that makes the fixtures discriminating. Not a
/// comparison tolerance: agreement with ngspice is judged only by the DC
/// contract, inside the harness.
const LADDER_CONDUCTING_MIN_V: f64 = 0.5;

/// The ladder taps both fixtures share, from the top of the string down.
const LADDER_TAPS: [&str; 3] = ["a", "e", "h"];

/// Cross-check one homotopy fixture against live ngspice, or skip (and pass)
/// without it.
fn homotopy_case(circuit: &str) {
    match NgspiceHarness::detect() {
        Some(harness) => {
            harness.op_case(circuit);
            let op = harness.piperine_op(circuit).unwrap_or_else(|e| panic!("{e}"));
            assert_ladder_conducts(circuit, &op);
        }
        None => eprintln!("SKIP {circuit}: ngspice not on PATH"),
    }
}

/// The fixture's reason to exist: the eight-diode ladder divides the rail,
/// every tap sitting above the forward-conduction floor and strictly below
/// the tap above it. A ladder that had collapsed to ~0 V would still match
/// ngspice — and would prove nothing about a homotopy, because an off ladder
/// is what plain Newton solves on its first iteration.
fn assert_ladder_conducts(circuit: &str, op: &OpResult) {
    let mut previous = f64::INFINITY;
    for tap in LADDER_TAPS {
        let v = op
            .v(tap.to_string())
            .unwrap_or_else(|e| panic!("{circuit}: no v({tap}) in the operating point: {e}"));
        assert!(
            v > LADDER_CONDUCTING_MIN_V,
            "{circuit}: v({tap}) = {v:.6e} V is below the {LADDER_CONDUCTING_MIN_V} V \
             conduction floor — the ladder is off, so this bias point is not the hard \
             one the source-stepping proof needs"
        );
        assert!(
            v < previous,
            "{circuit}: v({tap}) = {v:.6e} V does not sit below the tap above it \
             ({previous:.6e} V) — the ladder is not dividing the rail"
        );
        previous = v;
    }
}

/// NGP-11, the `vsrc` half: the stiff ladder (10 Ω, Is = 1e-20) whose ramp
/// collapses when the forced voltage stops scaling.
#[test]
fn ngspice_homotopy_bias() {
    homotopy_case("homotopy_bias");
}

/// NGP-11, the `isrc` half: the softer ladder (100 Ω, Is = 1e-16) with a
/// 25 mA sink at its middle node, whose ramp collapses when the sink's DC
/// value stops scaling.
#[test]
fn ngspice_homotopy_sink() {
    homotopy_case("homotopy_sink");
}
