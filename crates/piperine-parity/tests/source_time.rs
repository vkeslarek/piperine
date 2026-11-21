//! Time and timers from MSL source (feature `msl-digital`, T51; checks C110, C115, C162,
//! DIG-12 AC 1 and 4, DIG-14 AC 13): the stdlib `every(t0, period)` placed in an `on` head, a
//! reaction that commits `now()` and `analysis(Static)`, and `rise()`/`fall()` over edges to and
//! from unknown, from `tests/digital/time.msl`. Times are compared in ticks (one femtosecond) or
//! with the exact tick product, `now()` being `tick * resolution` (abi.md §4.2).
#![cfg(unix)]

use piperine_parity::project::Project;
use piperine_solver::protocol::{AnalysisKind, ObservationKind};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use mixed::rise;
use std::path::Path;
use support::{HIGH, LOW, build, transient};

/// One nanosecond and one microsecond in ticks.
const NS: i64 = 1_000_000;
const US: i64 = 1_000 * NS;
/// A solver tick in seconds (the default resolution of one femtosecond).
const TICK: f64 = 1e-15;

fn project() -> Project {
    Project::new(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/digital/time.msl"))
}

#[test]
fn every_from_the_stdlib_fires_once_per_period() {
    let seen = build(&project(), "TimerBench").run(10e-6, 1e-7);
    // DIG-12 AC 1: `every(1u, 2u)` fires at t0 = 1 us and every 2 us after: 1, 3, 5, 7 and 9 us
    // in 10 us, and at no other time; the reaction toggles `hit`, so each firing is one change
    let wanted = vec![
        (US, HIGH.to_vec()),
        (3 * US, LOW.to_vec()),
        (5 * US, HIGH.to_vec()),
        (7 * US, LOW.to_vec()),
        (9 * US, HIGH.to_vec()),
    ];
    assert_eq!(transient(&seen, "TimerBench.hit"), wanted);
}

fn real_bytes(v: f64) -> Vec<u8> {
    let mut bytes = vec![0u8; 16];
    bytes[..8].copy_from_slice(&v.to_le_bytes());
    bytes[8] = 1;
    bytes
}

#[test]
fn a_reaction_reads_now_and_static_from_source() {
    let seen = build(&project(), "StampBench").run(20e-9, 1e-9);
    // part IV §12.1.12 and abi.md §4.2: `now()` in a reaction is the time of its instant, a tick
    // of the session's resolution, so the reaction fired by the clock's change at 15 ns commits
    // the exact tick product `15e6 * 1e-15` (which is not the literal 15e-9)
    let t = transient(&seen, "StampBench.t");
    let fifteen = t.iter().find(|(at, _)| *at == 15 * NS);
    assert_eq!(fifteen, Some(&(15 * NS, real_bytes((15 * NS) as f64 * TICK))), "t at 15 ns: {t:?}");
    // `analysis(Static)` is true in the equilibrium solve and false at a transient instant: the
    // clock's initial value is a change at (0, 0), so the reaction commits true there; its first
    // change in the transient (2.5 ns) commits false
    let at_rest: Vec<Vec<u8>> = seen
        .all()
        .into_iter()
        .filter(|o| o.path == "StampBench.s" && o.analysis == AnalysisKind::OperatingPoint)
        .filter_map(|o| match o.kind {
            ObservationKind::Change { bytes } => Some(bytes),
            _ => None,
        })
        .collect();
    assert_eq!(at_rest, [HIGH.to_vec()], "s at the operating point");
    assert_eq!(transient(&seen, "StampBench.s").first(), Some(&(5 * NS / 2, LOW.to_vec())), "s at the first transient instant");
}

#[test]
fn rise_and_fall_fire_on_edges_to_and_from_unknown() {
    // `q` starts false and is committed false, unknown, true, unknown, false and true at the
    // rising edges 1 to 6 of the oscillator (the commit of false is no change)
    let seen = build(&project(), "EdgeBench").run(40e-9, 1e-9);
    // §22.1: `rise()` is IEEE 1364 posedge: false to unknown (edge 2), unknown to true (edge 3)
    // and false to true (edge 6), and no other edge; `fall()` is negedge: true to unknown (edge
    // 4) and unknown to false (edge 5), and no other edge (a true to false is not in the
    // sequence). Each run commits once to its counter, at its instant.
    assert_eq!(seen.commits("EdgeBench.rises"), vec![rise(2), rise(3), rise(6)], "rise() runs");
    assert_eq!(seen.commits("EdgeBench.falls"), vec![rise(4), rise(5)], "fall() runs");
}
