//! Time and timers from MSL source (feature `msl-digital`, T51; checks C110, C115, C162,
//! DIG-12 AC 1 and 4, DIG-14 AC 13): the stdlib `every(t0, period)` placed in an `on` head, a
//! reaction that commits `now()` and `analysis(Static)`, and `rise()`/`fall()` over transitions, from `tests/digital/time.msl`. Times are compared in ticks (one femtosecond) or
//! with the exact tick product, `now()` being `tick * resolution` (abi.md §4.2).
#![cfg(unix)]

use piperine_parity::source::bytes::CarrierBytes;
use piperine_parity::project::Project;
use piperine_solver::protocol::{AnalysisKind, ObservationKind};

use piperine_ir_fixtures::mixed::TopDesign;

use std::path::Path;
use piperine_parity::buck_fixture::{NetChanges, HIGH, LOW};
use piperine_parity::mixed::BuiltDesign;

/// One nanosecond and one microsecond in ticks.
const NS: i64 = 1_000_000;
const US: i64 = 1_000 * NS;
/// A solver tick in seconds (the default resolution of one femtosecond).
const TICK: f64 = 1e-15;

/// The model file `tests/digital/time.msl`, loaded as a project.
struct TimeModels;

impl TimeModels {
    fn project() -> Project {
        Project::new(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/digital/time.msl"))
    }
}

#[test]
fn every_from_the_stdlib_fires_once_per_period() {
    let seen = BuiltDesign::of(&TimeModels::project(), "TimerBench").run(10e-6, 1e-7);
    // DIG-12 AC 1: `every(1u, 2u)` fires at t0 = 1 us and every 2 us after: 1, 3, 5, 7 and 9 us
    // in 10 us, and at no other time; the reaction toggles `hit`, so each firing is one change
    let wanted = vec![
        (US, HIGH.to_vec()),
        (3 * US, LOW.to_vec()),
        (5 * US, HIGH.to_vec()),
        (7 * US, LOW.to_vec()),
        (9 * US, HIGH.to_vec()),
    ];
    assert_eq!(NetChanges::transient(&seen, "TimerBench.hit"), wanted);
}

#[test]
fn a_reaction_reads_now_and_static_from_source() {
    let seen = BuiltDesign::of(&TimeModels::project(), "StampBench").run(20e-9, 1e-9);
    // part IV §12.1.12 and abi.md §4.2: `now()` in a reaction is the time of its instant, a tick
    // of the session's resolution, so the reaction fired by the clock's change at 15 ns commits
    // the exact tick product `15e6 * 1e-15` (which is not the literal 15e-9)
    let t = NetChanges::transient(&seen, "StampBench.t");
    let fifteen = t.iter().find(|(at, _)| *at == 15 * NS);
    assert_eq!(fifteen, Some(&(15 * NS, CarrierBytes::real((15 * NS) as f64 * TICK))), "t at 15 ns: {t:?}");
    // `analysis(Static)` is true in the equilibrium solve and false at a transient instant: `s`
    // starts at its initial value, false, and the clock's initial value is a change at (0, 0), so
    // the reaction commits true there; its first change in the transient (2.5 ns) commits false
    let at_rest: Vec<Vec<u8>> = seen
        .all()
        .into_iter()
        .filter(|o| o.path == "StampBench.s" && o.analysis == AnalysisKind::OperatingPoint)
        .filter_map(|o| match o.kind {
            ObservationKind::Change { bytes } => Some(bytes),
            _ => None,
        })
        .collect();
    assert_eq!(at_rest, [LOW.to_vec(), HIGH.to_vec()], "s at the operating point: its initial value, then true");
    assert_eq!(NetChanges::transient(&seen, "StampBench.s").first(), Some(&(5 * NS / 2, LOW.to_vec())), "s at the first transient instant");
}

#[test]
fn rise_and_fall_fire_on_transitions() {
    // `q` starts false and is committed false, true, false, ... (odd edges false) at the rising edges 1 to 8 of the
    // oscillator in the 40 ns run (explicit-unknowns T27: a Bit passes through no unknown; the earlier name was
    // `rise_and_fall_fire_on_edges_to_and_from_unknown`). `rise()` is a change that ends true: false to true at edges
    // 2, 4, 6 and 8; `fall()` a change that ends false: true to false at edges 3, 5 and 7; the commit of false at edge 1
    // is no change. Each run commits once to its counter, at its instant.
    let seen = BuiltDesign::of(&TimeModels::project(), "EdgeBench").run(40e-9, 1e-9);
    assert_eq!(seen.commits("EdgeBench.rises"), vec![TopDesign::rising_edge(2), TopDesign::rising_edge(4), TopDesign::rising_edge(6), TopDesign::rising_edge(8)], "rise() runs");
    assert_eq!(seen.commits("EdgeBench.falls"), vec![TopDesign::rising_edge(3), TopDesign::rising_edge(5), TopDesign::rising_edge(7)], "fall() runs");
}
