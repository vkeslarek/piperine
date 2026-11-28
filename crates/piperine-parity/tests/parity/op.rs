//! Scope: operating-point parity (requirements PAR-06 AC 1 and 2, PAR-08 AC 2, RS-07): the node voltages
//! of a `Dc` solve of each twin, on each selected backend, agree with the values ngspice prints for
//! its `.cir`, under `DC_VOLTAGE`. One test per fixture, named after it; ngspice runs once per twin.

use piperine_lang::elab::Overrides;
use piperine_parity::backend::{Backend, BackendSelection};
use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::ngspice::{Lookup, Ngspice, Reference};
use piperine_parity::operating_point::OperatingPoint;
use piperine_solver::prelude::Context;

/// Runs ngspice once for the twin and holds every selected backend to it.
fn compare(twin: OperatingPoint) {
    let reference = Ngspice::from_environment().run(twin.fixture);
    for backend in BackendSelection::selected() {
        twin.compare_on(&reference, backend);
    }
}

#[test]
fn divider() {
    compare(OperatingPoint { fixture: "divider", nodes: &["vin", "mid"] });
}

#[test]
fn diode_series() {
    compare(OperatingPoint { fixture: "diode_series", nodes: &["a", "b"] });
}

#[test]
fn isrc_r() {
    compare(OperatingPoint { fixture: "isrc_r", nodes: &["a"] });
}

/// The backends that run a twin whose model selects its equations by a param: the compiled one (the
/// interpreter refuses a contribution under a selector by name, T15).
fn compiled_backends() -> Vec<Backend> {
    BackendSelection::selected().into_iter().filter(|backend| *backend == Backend::Rust).collect()
}

#[test]
fn diode_rs() {
    // `DiodeRs` with rs = 100 (`i(a, ai) +> v(a, ai) / rs`) against ngspice's diode with `rs=100`.
    let reference = Ngspice::from_environment().run("diode_rs");
    for backend in compiled_backends() {
        OperatingPoint { fixture: "diode_rs", nodes: &["vin", "a"] }.compare_on(&reference, backend);
    }
}

#[test]
fn diode_rs_with_rs_zero_merges_and_matches_rdiode() {
    // AS-02 AC 4 (section 11 rule 3): the before-run binding `d1.rs` = 0 selects the variant whose
    // series branch is the constant-zero potential `v(a, ai) +> 0.0`, so `a` and `ai` are one node
    // (one node fewer than the rs = 100 circuit) and the solve is `rdiode.cir`'s clamp: `v(a)` equals
    // ngspice's `v(out)` under `DC_VOLTAGE`.
    let reference = Ngspice::from_environment().run("rdiode");
    for backend in compiled_backends() {
        let series = Bench::load_on("diode_rs", backend);
        let mut merged = Bench::load_file_with_on("diode_rs", Bench::twin_path("diode_rs"), backend, &Overrides::new(&[("d1.rs".to_string(), 0.0)]));
        assert_eq!(merged.ir().nodes().count() + 1, series.ir().nodes().count(), "{}: rs = 0 merges a and ai", backend.name());
        let a = merged.node("a");
        let solved = merged.circuit().dc(Context::default()).expect("dc").solve().unwrap_or_else(|error| panic!("rs = 0 does not solve on {}: {error}", backend.name()));
        let actual = solved.get_node(&a).expect("v(a)");
        let Reference::Golden(golden) = &reference else { continue };
        let at = Point::new("diode_rs rs=0", "v(a)", Coordinate::OperatingPoint).with_ngspice(golden.version()).with_backend(backend.name());
        ToleranceContract::DC_VOLTAGE.assert(&at, golden.printed("v(out)"), actual);
    }
}

#[test]
fn diode_rs_runs_and_prints_the_skip_line_without_ngspice() {
    // AS-18 AC 3: with the comparison disabled the reference is `Skipped` (its skip line printed by
    // `Ngspice::run`) and the twin is still built and solved: both nodes are read, none is compared.
    let reference = Ngspice::new(Lookup::Skip).run("diode_rs");
    assert!(matches!(reference, Reference::Skipped));
    for backend in compiled_backends() {
        let outcome = OperatingPoint { fixture: "diode_rs", nodes: &["vin", "a"] }.compare_on(&reference, backend);
        assert_eq!((outcome.solved, outcome.compared), (2, 0), "{}", backend.name());
    }
}

#[test]
fn tnom_res() {
    // AO-08, withdrawn as a run key: the model's own `tnom` of 323.15 K (50 C) against ngspice's
    // `.options tnom=50`, both at 27 C, the lower leg 3k * (1 + 0.002 * (27 - 50))
    compare(OperatingPoint { fixture: "tnom_res", nodes: &["mid"] });
}
