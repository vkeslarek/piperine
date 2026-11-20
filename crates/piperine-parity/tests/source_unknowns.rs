//! Unknown semantics run from MSL source on the Rust backend (feature `unknown-semantics`): one
//! test per run-time criterion, each module written in `tests/unknowns/`, elaborated by the front
//! end, compiled on the transpiled backend and run on the solver. Every expected value is written
//! from the rule it cites (part I §3.3, §1 rule 13, §19), never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_api::{Field, Record, Streams, Verdict};
use piperine_solver::protocol::ObservationKind;
use piperine_parity::project::Project;

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "../../piperine-transpile/tests/support/mixed.rs"]
mod mixed;
#[allow(dead_code)]
#[path = "buck/support.rs"]
mod support;

use mixed::Seen;
use support::{build, series, transient};

/// The project of the lone file `file` of `tests/unknowns/`.
fn project(file: &str) -> Project {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/unknowns").join(file);
    Project::new(&path)
}

/// The last value the net at `path` took in a 10 ns transient of `top` of `file`, `None` if it
/// never took one: a net no one wrote is unknown (every byte 0).
fn settled(file: &str, top: &str, path: &str) -> Option<Vec<u8>> {
    let seen = build(&project(file), top).run(10e-9, 1e-9);
    series(&seen, path).pop().map(|(_, bytes)| bytes)
}

#[test]
fn a_function_and_a_definition_merge_an_unknown_if() {
    // US-04 AC 10, §3.3.3: `d` is never driven. `pick(d)` is `if d { 1 } else { 2 }` in a
    // function: the arms differ, so the merge is unknown (committed as every byte 0, or no
    // value at all); the definition `if d { 3 } else { 3 }` has equal arms, so its merge is 3,
    // known. Neither is a fault.
    let y = settled("pure_merge.msl", "PickBench", "PickBench.y");
    assert!(y.as_ref().is_none_or(|bytes| *bytes == [0, 0]), "different arms merge to unknown: {y:?}");
    assert_eq!(settled("pure_merge.msl", "PickBench", "PickBench.z"), Some(vec![3, 1]), "equal arms merge to that value");
}

/// Changes of the nets of `Merges`, from one 10 ns transient at 1 GHz (rising edges at 0.5, 1.5,
/// 2.5 ns, ...): instant in ticks and bytes.
fn merges() -> Seen {
    build(&project("reaction_merge.msl"), "Merges").run(10e-9, 0.5e-9)
}

/// The changes of the net `name` of `Merges`.
fn changes(seen: &Seen, name: &str) -> Vec<(i64, Vec<u8>)> {
    transient(seen, &format!("Merges.{name}"))
}

/// A known `u8` carrier value.
fn byte(v: u8) -> Vec<u8> {
    vec![v, 1]
}

/// An unknown carrier value: every byte 0.
const UNKNOWN: [u8; 2] = [0, 0];

#[test]
fn an_unknown_if_runs_both_arms_without_a_fault() {
    // US-04 AC 1: `r <- 9; if d { q <- 5 } else { q <- 6 }` with `d` never driven completes the
    // run: no fault, `before` commits 9 at the first edge (0.5 ns), and `differ`, which held 1,
    // takes the merge of 5 and 6, which is unknown
    let seen = merges();
    assert_eq!(changes(&seen, "before"), vec![(NS / 2, byte(9))]);
    assert_eq!(changes(&seen, "differ"), vec![(NS / 2, UNKNOWN.to_vec())]);
}

/// One nanosecond in ticks.
const NS: i64 = 1_000_000;

#[test]
fn both_arms_committing_commit_their_merge() {
    // §3.3.3: `if d { differ <- 5 } else { differ <- 6 }` merges to unknown (above); here the
    // arms commit 5 and 5, whose merge is 5, known, over the 1 the carrier held
    let seen = merges();
    assert_eq!(changes(&seen, "equal"), vec![(NS / 2, byte(5))]);
}

#[test]
fn equal_known_commits_stay_known() {
    // §22.1.2 rule 1: two equal known values merge to that value, in the statement form and in
    // the expression form of the `if`
    let seen = merges();
    assert_eq!(changes(&seen, "statement_equal"), vec![(NS / 2, byte(1))]);
    assert_eq!(changes(&seen, "expression_equal"), vec![(NS / 2, byte(1))]);
}

#[test]
fn one_arm_committing_merges_with_the_value_then() {
    // §3.3.3: `if d { changes <- 7; keeps <- 7 }` with no `else`: the path that does not run
    // leaves the carrier as it is, so each commit is merged with "the value the carrier would
    // hold then": `changes` held 3, merge(7, 3) is unknown; `keeps` held 7, merge(7, 7) is 7,
    // no change at all
    let seen = merges();
    assert_eq!(changes(&seen, "changes"), vec![(NS / 2, UNKNOWN.to_vec())]);
    assert_eq!(changes(&seen, "keeps"), Vec::<(i64, Vec<u8>)>::new());
}

#[test]
fn an_if_expression_and_an_if_statement_commit_the_same() {
    // US-04 AC 10: `x <- if d { 1 } else { 2 }` and `if d { x <- 1 } else { x <- 2 }` both give
    // the merge, unknown
    let seen = merges();
    assert_eq!(changes(&seen, "statement_differ"), changes(&seen, "expression_differ"));
    assert_eq!(changes(&seen, "expression_differ"), vec![(NS / 2, UNKNOWN.to_vec())]);
}

#[test]
fn an_unknown_match_merges_every_arm() {
    // US-04 AC 6: an unknown scrutinee takes every arm, the wildcard included. `differ` is 1, 2
    // or 3 by arm: unknown. `equal` is 4 in every arm: known 4
    let seen = merges();
    assert_eq!(changes(&seen, "match_differ"), vec![(NS / 2, UNKNOWN.to_vec())]);
    assert_eq!(changes(&seen, "match_equal"), vec![(NS / 2, byte(4))]);
}

#[test]
fn a_panic_under_unknown_control_makes_the_merge_unknown() {
    // US-04 AC 7, §1 rule 13: `if d { q <- 1 / zero } else { q <- 5 }` divides by zero on one
    // path; that is no fault, and every effect of the merge is unknown even though the other
    // arm agrees with the 5 `q` held. `before`, made outside the `if`, is not touched
    let seen = merges();
    assert_eq!(changes(&seen, "panic_q"), vec![(NS / 2, UNKNOWN.to_vec())]);
    assert_eq!(changes(&seen, "panic_before"), vec![(NS / 2, byte(9))]);
}

#[test]
fn definitions_lets_and_functions_merge_alike() {
    // US-04 AC 10: a definition, a body `let` and a function body deciding on an unknown `d`
    // apply the one merge. Arms 3 and 3 give 3; arms 3 and 4 give unknown
    let seen = merges();
    for name in ["equal_let", "equal_function"] {
        assert_eq!(changes(&seen, name), vec![(NS / 2, byte(3))], "{name}");
    }
    assert_eq!(series(&seen, "Merges.equal_definition").pop().map(|(_, bytes)| bytes), Some(byte(3)));
    for name in ["differ_let", "differ_function"] {
        assert_eq!(changes(&seen, name), vec![(NS / 2, UNKNOWN.to_vec())], "{name}");
    }
    assert!(series(&seen, "Merges.differ_definition").pop().is_none_or(|(_, bytes)| bytes == UNKNOWN), "an unknown definition");
}

#[test]
fn an_unknown_delay_under_unknown_control_commits_unknown_next() {
    // US-04 AC 13: `q <- 2 after t` with `t` unknown commits `Unknown` at the next microstep
    // (the same instant), in both arms of an unknown `if`: `q`, which held 1, is unknown
    let seen = merges();
    assert_eq!(changes(&seen, "late_q"), vec![(NS / 2, UNKNOWN.to_vec())]);
}

#[test]
fn neither_arm_committing_commits_nothing() {
    // US-04 AC 1: an `if` whose arms only bind locals makes no commit and no fault
    let seen = merges();
    assert_eq!(changes(&seen, "idle"), Vec::<(i64, Vec<u8>)>::new());
}

#[test]
fn delays_one_tick_apart_are_one_key() {
    // abi.md C1, §3.3.3: `after 1n` and `after 1.0000004n` both round to 1 000 000 ticks of 1 fs,
    // one instant: the two arms commit one key, merge(1, 1) = 1, known, at 0.5 + 1 = 1.5 ns. Two
    // keys would each merge with the value then (0) and give unknown
    let seen = merges();
    assert_eq!(changes(&seen, "ticks_q"), vec![(3 * NS / 2, byte(1))]);
}

#[test]
fn the_pfd3_reaction_keeps_state_unknown_until_a_known_commit() {
    // §3.3's `Pfd3`, unreset, in the `if`-expression form §3.3 says is the same: `state` stays
    // unknown through the first two edges (0.5 and 1.5 ns), the third (2.5 ns) commits 0, known,
    // and the fourth (3.5 ns) steps it to 1, which later edges keep
    let seen = merges();
    assert_eq!(changes(&seen, "state"), vec![(5 * NS / 2, byte(0)), (7 * NS / 2, byte(1))]);
}

/// The instant of the last step of the transient of `top` of `reaction_merge.msl`, run to 4 ns.
fn last_step(top: &str) -> f64 {
    let (_, result) = build(&project("reaction_merge.msl"), top).run_with(4e-9, 0.5e-9);
    result.iter().last().expect("a time point").time()
}

#[test]
fn a_maybe_finish_does_not_end_the_run() {
    // US-05 AC 3, §19 Resolution: `if d { finish(); }` with `d` unknown is reached under unknown
    // control, so the run goes on to its stop (4 ns); the same `finish()` under known control
    // ends it at the first rising edge (0.5 ns)
    assert!((last_step("MaybeFinishBench") - 4e-9).abs() <= 1e-15, "a maybe finish runs to stop");
    assert!((last_step("SureFinishBench") - 0.5e-9).abs() <= 1e-15, "a sure finish ends at the edge");
}

/// Every record of the 4 ns transient of `RecordBench` (rising edges at 0.5, 1.5, 2.5, 3.5 ns),
/// decoded as the host decodes it.
fn recorded() -> Vec<Record> {
    let built = build(&project("records.msl"), "RecordBench");
    let streams = Streams::of(built.ir.clone());
    let seen = built.run(4e-9, 0.5e-9);
    seen.all().iter().filter_map(|o| streams.decode(o)).map(|r| r.unwrap_or_else(|f| panic!("{f}"))).collect()
}

/// The records of the stream `name`.
fn stream<'a>(records: &'a [Record], name: &str) -> Vec<&'a Record> {
    records.iter().filter(|r| r.stream == name).collect()
}

#[test]
fn a_record_under_unknown_control_is_maybe() {
    // US-05 AC 1, 2: `if d { emit!("unsure", ..) }` with `d` unknown is reached under unknown
    // control, so its record says `maybe`; the `emit!` under known control does not. One of each
    // per rising edge (0.5, 1.5, 2.5, 3.5 ns)
    let records = recorded();
    let (unsure, sure) = (stream(&records, "unsure"), stream(&records, "sure"));
    assert_eq!((unsure.len(), sure.len()), (4, 4));
    assert!(unsure.iter().all(|r| r.maybe), "reached under unknown control");
    assert!(sure.iter().all(|r| !r.maybe), "reached under known control");
}

#[test]
fn a_record_field_keeps_its_flag() {
    // US-05 AC 5: `v` is unknown and `k` is known 3, in a record that is `maybe` and in one that
    // is not; `maybe` is no field
    let records = recorded();
    let expected = [("v".to_string(), Field::Unknown), ("k".to_string(), Field::Int(3))];
    for name in ["unsure", "sure"] {
        for record in stream(&records, name) {
            assert_eq!(record.fields, expected, "{name}");
        }
    }
}

#[test]
fn a_maybe_check_is_judged_as_unknown() {
    // US-05 AC 4, §23.2: `if d { require("held", true) }` writes its `checks` record with
    // `maybe = true`, and the verdict fails it as it fails an unknown check, though its `pass` is
    // known true
    let records = recorded();
    let checks = stream(&records, "checks");
    assert_eq!(checks.len(), 4);
    assert!(checks.iter().all(|r| r.maybe && r.fields.iter().any(|(n, f)| n == "pass" && *f == Field::Bool(true))));
    let verdict = Verdict::judge(&records);
    assert!(!verdict.passed());
    assert_eq!((verdict.checks(), verdict.failed().len()), (4, 4));
}

/// The changes of the net `name` of `GuardBench` in a 4 ns transient.
fn guarded(name: &str) -> Vec<(i64, Vec<u8>)> {
    let seen = build(&project("guards.msl"), "GuardBench").run(4e-9, 0.5e-9);
    transient(&seen, &format!("GuardBench.{name}"))
}

#[test]
fn an_unknown_guard_runs_the_reaction_as_a_maybe() {
    // US-04 AC 9, §13 rule 1: `on clk.rise() && en` with `en` unknown fires, and each effect of
    // the reaction is merged with not running it: `changes` held 1 and is committed 5, merge(5,
    // 1) is unknown at the first edge (0.5 ns); `keeps` held 5 and is committed 5, merge(5, 5) is
    // 5, no change at all
    assert_eq!(guarded("changes"), vec![(NS / 2, UNKNOWN.to_vec())]);
    assert_eq!(guarded("keeps"), Vec::<(i64, Vec<u8>)>::new());
}

#[test]
fn two_commits_under_an_unknown_guard_merge_exactly() {
    // §3.3.3, §13 rule 1: `settles` holds 7 and a reaction on an unknown guard commits it 5 and
    // then 7. The second commit replaces the first, so the reaction's effect is `7`, and the merge
    // with not running it is `merge(7, 7)` = 7 known: nothing changes. Merging each commit on
    // its own (`merge(5, 7)` unknown, then merged again) would leave it unknown
    assert_eq!(guarded("settles"), Vec::<(i64, Vec<u8>)>::new());
}

#[test]
fn a_known_false_guard_does_not_run() {
    // §13 rule 1: a guard that is known false stops the event, so the reaction does not run and
    // `blocked`, which held 1, never changes
    assert_eq!(guarded("blocked"), Vec::<(i64, Vec<u8>)>::new());
}

/// The instants (seconds) of the `hit` records of `top` of `cross_if.msl`, in a 1 us transient.
fn hits(top: &str) -> Vec<f64> {
    let seen = build(&project("cross_if.msl"), top).run(1e-6, 0.05e-6);
    seen.all().iter().filter(|o| matches!(o.kind, ObservationKind::Record { .. })).map(|o| o.time).collect()
}

#[test]
fn an_unknown_if_in_a_cross_argument_merges() {
    // §3.3.3: `cross(if d { v(p) - 0.3 } else { v(p) - 0.7 })`. With `d` known true it watches
    // `v(p) - 0.3`, which a 1 V/us ramp crosses at 0.3 us (within the 1 ps `ttol`); with `d`
    // unknown both arms run and the value differs, so it is unknown, which is NaN to the solver
    // (abi.md A4): the watch never crosses, and nothing is refused
    let known = hits("KnownBench");
    assert_eq!(known.len(), 1, "one crossing: {known:?}");
    assert!((known[0] - 0.3e-6).abs() <= 1e-12, "at 0.3 us: {known:?}");
    assert_eq!(hits("MaybeBench"), Vec::<f64>::new());
}

/// The records of the 4 ns transient of `top` of `file`, decoded as the host decodes them.
fn records_of(file: &str, top: &str) -> (Seen, Vec<Record>) {
    let built = build(&project(file), top);
    let streams = Streams::of(built.ir.clone());
    let seen = built.run(4e-9, 0.5e-9);
    let records = seen.all().iter().filter_map(|o| streams.decode(o)).map(|r| r.unwrap_or_else(|f| panic!("{f}"))).collect();
    (seen, records)
}

#[test]
fn an_if_whose_arms_commit_is_the_value_of_a_commit_and_of_an_emit_field() {
    // §3.3.3: `v <- if d { w <- 1; 5 } else { w <- 2; 5 }` with `d` unknown makes both paths'
    // commits: `v` is 5 on each, so it is known 5; `w` is 1 or 2, so it is unknown. The same in
    // an emit field: `x = if d { z <- 3; 7 } else { z <- 3; 7 }` commits `z` 3 and writes `x` 7,
    // known, in a record that is no maybe (the `emit!` itself is reached under known control,
    // only the value of its field decides on `d`). The generated source must compile at all: two
    // calls on one `Commits` cannot be nested
    let (seen, records) = records_of("hoisted.msl", "HoistBench");
    let changes = |name: &str| transient(&seen, &format!("HoistBench.{name}"));
    assert_eq!(changes("v"), vec![(NS / 2, byte(5))]);
    assert_eq!(changes("w"), vec![(NS / 2, UNKNOWN.to_vec())]);
    assert_eq!(changes("z"), vec![(NS / 2, byte(3))]);
    let seen_records = stream(&records, "seen");
    assert_eq!(seen_records.len(), 4);
    assert!(seen_records.iter().all(|r| !r.maybe && r.fields == [("x".to_string(), Field::Int(7))]), "{seen_records:?}");
}
