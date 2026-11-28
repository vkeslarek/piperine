//! Commit semantics from MSL source (feature `msl-digital`, T47; checks C138 to C141, DIG-14
//! AC 11): four modules, each the example of its rule in `docs/spec/part_iv_behavior.md`
//! §12.1, written in `tests/digital/commits.msl`, elaborated by the front end, compiled on the
//! transpiled backend and run on the solver. Every expected instant and value is written from
//! the rule cited at its assertion, in ticks of one femtosecond, never read back from the model.
#![cfg(unix)]

#[allow(dead_code)]
use crate::support;

use piperine_parity::project::Project;
use std::path::Path;
use support::{HIGH, LOW, build, transient};

/// One nanosecond in ticks.
const NS: i64 = 1_000_000;

fn project() -> Project {
    Project::new(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/digital/commits.msl"))
}

#[test]
fn two_commits_exchange_two_carriers() {
    let seen = build(&project(), "SwapBench").run(10e-9, 1e-9);
    // §12.1.1: every read sees the values at the start of the microstep and the commits of the
    // microstep apply together, so `a <- b; b <- a;` exchanges them. a = true, b = false at the
    // start; at the first rising edge (2.5 ns) a becomes false and b true; at the second
    // (7.5 ns) they exchange back. A `b <- a` that read the new `a` would leave both false at
    // 2.5 ns and give `b` no change.
    assert_eq!(transient(&seen, "SwapBench.a"), vec![(5 * NS / 2, LOW.to_vec()), (15 * NS / 2, HIGH.to_vec())]);
    assert_eq!(transient(&seen, "SwapBench.b"), vec![(5 * NS / 2, HIGH.to_vec()), (15 * NS / 2, LOW.to_vec())]);
}

#[test]
fn the_later_statement_wins() {
    let seen = build(&project(), "LaterBench").run(10e-9, 1e-9);
    // §12.1.2: commits to one level that apply in the same microstep are ordered by the instant
    // they were made, then by source order; the last one is the value. At 2.5 ns `x` takes 2,
    // and 1 is never seen. At 7.5 ns the same commits leave the value as it is: no change.
    assert_eq!(transient(&seen, "LaterBench.x"), vec![(5 * NS / 2, vec![2])]);
}

#[test]
fn two_statements_make_a_pulse() {
    // 7 ns: the second rising edge (7.5 ns) is not reached
    let seen = build(&project(), "PulseBench").run(7e-9, 1e-9);
    // §12.1.4: `p <- true after t; p <- false after t + w;` holds `p` true from `t` to `t + w`,
    // in either order of the two statements: the reaction runs at 2.5 ns, t = 1 ns, w = 2 ns, so
    // true from 3.5 ns to 5.5 ns.
    let pulse = vec![(7 * NS / 2, HIGH.to_vec()), (11 * NS / 2, LOW.to_vec())];
    assert_eq!(transient(&seen, "PulseBench.p"), pulse);
    assert_eq!(transient(&seen, "PulseBench.q"), pulse);
}

#[test]
fn an_unchanged_commit_fires_nothing() {
    // 20 ns: rising edges at 2.5, 7.5, 12.5 and 17.5 ns
    let seen = build(&project(), "UnchangedBench").run(20e-9, 1e-9);
    // §12.1.18: a commit that changes a level fires `changed()` where it applies, and one that
    // leaves the value as it was fires nothing. The latch commits true at four rising edges; it
    // changes `q` only at the first (2.5 ns)
    assert_eq!(transient(&seen, "UnchangedBench.q"), vec![(5 * NS / 2, HIGH.to_vec())]);
    // ... all four commits were made (a commit is a record of the reaction, changed or not)
    let made = seen.commits("UnchangedBench.latch");
    assert_eq!(made.iter().filter(|t| **t % (5 * NS) == 5 * NS / 2).count(), 4, "four commits: {made:?}");
    // ... and `changed()` fired once in the transient, so the counter, which adds one per
    // firing, moved once, at 2.5 ns (the operating point already counted the initial value of
    // `q`, so it reads 2; three more firings from the unchanged commits would read 5)
    let moved = transient(&seen, "UnchangedBench.n");
    assert_eq!(moved, vec![(5 * NS / 2, vec![2])], "the counter holds 2 after the one firing");
}
