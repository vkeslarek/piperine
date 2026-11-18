//! Scope: the deviation table (HYG-06): each of the 11 twins is run with the policy of its own
//! `parity_*` target and the worst `|golden - actual| / bound` of every compared quantity is
//! printed (`-- --nocapture`), with the accepted and rejected steps and `dt_min` floor hits of a
//! transient, and, beside ngspice's own point count, the steps of the same transient run free (the
//! step cap ngspice's own: its print step, not a tenth of it), which the capped run hides: the twins
//! run at a cap where every step sits at it. It measures; the gate stays the `parity_*` targets
//! under `ToleranceContract`.

use std::collections::BTreeMap;
use std::path::Path;

use piperine_parity::backend::Backend;
use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, ToleranceContract};
use piperine_parity::deviation::Deviation;
use piperine_parity::ngspice::{Ngspice, Reference};
use piperine_parity::operating_point::OperatingPoint;
use piperine_parity::waveform::Waveform;
use piperine_solver::prelude::{AcSweepAnalysisOptions, Context, TransientAnalysisOptions};

/// What a run of one twin on one backend produced: its deviations and its transient's counters.
struct Measured {
    deviations: Vec<Deviation>,
    steps: Option<(usize, usize, usize)>,
    /// A transient's cost with the step free: ngspice's points, and the accepted steps of the run
    /// capped at ngspice's own maximum step (its print step).
    free_running: Option<FreeRunning>,
}

/// The step count of a transient run at ngspice's own maximum step, beside ngspice's point count.
struct FreeRunning {
    ngspice_points: usize,
    steps: usize,
}

fn operating_point(twin: &OperatingPoint, golden: &piperine_parity::golden::Golden, backend: Backend) -> Measured {
    let mut bench = Bench::load_on(twin.fixture, backend);
    let ids: Vec<_> = twin.nodes.iter().map(|node| bench.node(node)).collect();
    let result = bench.circuit().dc(Context::default()).expect("dc").solve().expect("the operating point solves");
    let deviations = twin
        .nodes
        .iter()
        .zip(&ids)
        .map(|(node, id)| {
            let actual = result.get_node(id).expect("a node voltage");
            let wanted = golden.printed(&format!("v({node})"));
            Deviation::of(twin.fixture, &format!("v({node})"), &ToleranceContract::DC_VOLTAGE, &[(Coordinate::OperatingPoint, wanted, actual)])
        })
        .collect();
    Measured { deviations, steps: None, free_running: None }
}

fn sweep(fixture: &str, export: &str, golden: &piperine_parity::golden::Golden, backend: Backend) -> Measured {
    let mut bench = Bench::load_on(fixture, backend);
    let source = bench.source("v1");
    let mut rows = Vec::new();
    for (value, wanted) in golden.real(export) {
        bench.set("v1", "dc", value);
        let result = bench.circuit().dc(Context::default()).expect("dc").solve().expect("the sweep point solves");
        rows.push((Coordinate::Sweep(value), wanted, result.get_branch(source.clone()).expect("the flow of v1")));
    }
    Measured { deviations: vec![Deviation::of(fixture, "i(v1)", &ToleranceContract::DC_CURRENT, &rows)], steps: None, free_running: None }
}

fn ac(golden: &piperine_parity::golden::Golden, backend: Backend) -> Measured {
    let mut bench = Bench::load_on("ac_rc", backend);
    let out = bench.node("out");
    let rows = golden.complex("ac_rc_ac");
    let options = AcSweepAnalysisOptions { start_frequency: rows[0].0, stop_frequency: rows[rows.len() - 1].0, steps: rows.len(), logarithmic: true };
    let swept = bench.circuit().ac(Context::default()).expect("ac").solve_sweep(options).expect("the sweep solves");
    let (mut magnitude, mut phase) = (Vec::new(), Vec::new());
    for ((frequency, wanted), step) in rows.iter().zip(swept.iter()) {
        let actual = step.get_node(&out).expect("v(out)");
        magnitude.push((Coordinate::Frequency(*frequency), wanted.magnitude(), actual.norm()));
        if ToleranceContract::phase_applies(wanted.magnitude()) {
            phase.push((Coordinate::Frequency(*frequency), wanted.phase(), actual.arg()));
        }
    }
    Measured {
        deviations: vec![
            Deviation::of("ac_rc", "|v(out)|", &ToleranceContract::AC_MAGNITUDE, &magnitude),
            Deviation::of("ac_rc", "arg v(out)", &ToleranceContract::AC_PHASE, &phase),
        ],
        steps: None,
        free_running: None,
    }
}

/// The transient policy of `parity_tran.rs`: stop at 10 us, a step cap of a tenth of the print step.
fn tran(fixture: &str, export: &str, node: &str, golden: &piperine_parity::golden::Golden, backend: Backend) -> Measured {
    tran_to(fixture, export, node, golden, backend, 10e-6, f64::MIN_POSITIVE)
}

/// [`tran`] to `stop`, with `dt_min` as the smallest step unless it is not above `f64::MIN_POSITIVE`
/// (the options' own default).
fn tran_to(fixture: &str, export: &str, node: &str, golden: &piperine_parity::golden::Golden, backend: Backend, stop: f64, dt_min: f64) -> Measured {
    let mut bench = Bench::load_on(fixture, backend);
    let out = bench.node(node);
    let rows = golden.real(export);
    let print_step = rows[1].0 - rows[0].0;
    let mut options = TransientAnalysisOptions::new(stop, print_step / 10.0);
    options.dt_max = print_step / 10.0;
    if dt_min > f64::MIN_POSITIVE {
        options.dt_min = dt_min;
    }
    let result = bench.circuit().transient(options, Context::default()).expect("transient").solve().expect("the transient solves");
    let steps: Vec<_> = result.iter().collect();
    let waveform = Waveform::new(steps.iter().map(|s| s.time()).collect(), steps.iter().map(|s| s.get_node(&out).expect("a node voltage")).collect());
    let compared: Vec<_> = rows.iter().map(|(time, wanted)| (Coordinate::Time(*time), *wanted, waveform.at(*time).expect("inside the result"))).collect();
    let stats = &result.stats;
    Measured {
        deviations: vec![Deviation::of(fixture, &format!("v({node})"), &ToleranceContract::TRAN_VOLTAGE, &compared)],
        steps: Some((stats.steps_accepted, stats.steps_rejected, stats.dt_min_floor_hits)),
        free_running: Some(FreeRunning { ngspice_points: rows.len(), steps: free_running_steps(fixture, backend, stop, print_step, dt_min) }),
    }
}

/// The accepted steps of the twin `fixture` run to `stop` with the step cap at `print_step`, ngspice's
/// own maximum step (`tstep`), instead of a tenth of it.
fn free_running_steps(fixture: &str, backend: Backend, stop: f64, print_step: f64, dt_min: f64) -> usize {
    let mut bench = Bench::load_on(fixture, backend);
    let mut options = TransientAnalysisOptions::new(stop, print_step / 10.0);
    options.dt_max = print_step;
    if dt_min > f64::MIN_POSITIVE {
        options.dt_min = dt_min;
    }
    let result = bench.circuit().transient(options, Context::default()).expect("transient").solve().expect("the free transient solves");
    result.stats.steps_accepted
}

/// `tran_floor`: no number to compare. ngspice stops with "Timestep too small"; the run of the twin
/// must fail (`parity_tran.rs` asserts the code and the time).
fn floor(golden: &piperine_parity::golden::Golden) -> Measured {
    assert!(golden.said("Timestep too small"), "ngspice stops at the floor");
    Measured { deviations: Vec::new(), steps: None, free_running: None }
}

/// The twins whose source reads the time, which only the compiled backend runs.
const COMPILED_ONLY: [&str; 3] = ["tran_jump", "tran_corner", "tran_floor"];

/// Every twin, in the order of the `parity_*` targets, measured on `backend`; `None` when ngspice is absent.
fn measure(backend: Backend) -> Option<BTreeMap<&'static str, Measured>> {
    let ngspice = Ngspice::from_environment();
    let mut table = BTreeMap::new();
    for (fixture, nodes) in [("divider", &["vin", "mid"][..]), ("diode_series", &["a", "b"][..]), ("isrc_r", &["a"][..])] {
        let Reference::Golden(golden) = ngspice.run(fixture) else { return None };
        table.insert(fixture, operating_point(&OperatingPoint { fixture, nodes }, &golden, backend));
    }
    for (fixture, export) in [("diode_iv", "diode_iv_sweep"), ("diode_rev", "diode_rev_sweep")] {
        let Reference::Golden(golden) = ngspice.run(fixture) else { return None };
        table.insert(fixture, sweep(fixture, export, &golden, backend));
    }
    let Reference::Golden(golden) = ngspice.run("ac_rc") else { return None };
    table.insert("ac_rc", ac(&golden, backend));
    let Reference::Golden(golden) = ngspice.run("tran_rc") else { return None };
    table.insert("tran_rc", tran("tran_rc", "tran_rc_tran", "out", &golden, backend));
    let Reference::Golden(golden) = ngspice.run("tran_dq") else { return None };
    table.insert("tran_dq", tran("tran_dq", "tran_dq_tran", "a", &golden, backend));
    if backend == Backend::Rust {
        let Reference::Golden(golden) = ngspice.run("tran_jump") else { return None };
        table.insert("tran_jump", tran_to("tran_jump", "tran_jump_tran", "top", &golden, backend, 2e-6, f64::MIN_POSITIVE));
        let Reference::Golden(golden) = ngspice.run("tran_corner") else { return None };
        table.insert("tran_corner", tran_to("tran_corner", "tran_corner_tran", "top", &golden, backend, 2e-6, 1e-9));
        let Reference::Golden(golden) = ngspice.run("tran_floor") else { return None };
        table.insert("tran_floor", floor(&golden));
    }
    Some(table)
}

#[test]
fn the_table_covers_every_twin() {
    let mut msl: Vec<String> = std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/ngspice"))
        .expect("tests/ngspice exists")
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "msl"))
        .filter_map(|path| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .filter(|stem| stem != "devices")
        .collect();
    msl.sort();
    for backend in Backend::selected() {
        let Some(table) = measure(backend) else { return };
        let covered: Vec<&str> = table.keys().copied().collect();
        let expected: Vec<&str> = msl.iter().map(String::as_str).filter(|name| backend == Backend::Rust || !COMPILED_ONLY.contains(name)).collect();
        assert_eq!(covered, expected, "one row per twin on {}", backend.name());
        let deviations: Vec<Deviation> = table.values().flat_map(|m| m.deviations.clone()).collect();
        println!("deviation table on {} (ratio of |golden - actual| to the contract bound; 1 = at the bound)", backend.name());
        print!("{}", Deviation::table(&deviations));
        for (fixture, measured) in &table {
            if let Some((accepted, rejected, floor)) = measured.steps {
                println!("{fixture} on {}: steps accepted {accepted}, rejected {rejected}, dt_min floor hits {floor}", backend.name());
            }
            if let Some(free) = &measured.free_running {
                println!("{fixture} on {}: free-running steps {} against ngspice's {} points", backend.name(), free.steps, free.ngspice_points);
            }
        }
    }
}
