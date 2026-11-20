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
    // US-04 AC 6: an unknown scrutinee takes every arm, the wildcard included. `differ` is 1, 1
    // and 2 by arm: unknown only because the wildcard's 2 is merged in. `equal` is 4 in every
    // arm: known 4
    let seen = merges();
    assert_eq!(changes(&seen, "match_differ"), vec![(NS / 2, UNKNOWN.to_vec())]);
    assert_eq!(changes(&seen, "match_equal"), vec![(NS / 2, byte(4))]);
}

#[test]
fn a_nested_one_armed_commit_merges_with_the_path_before_it() {
    // §3.3.3: `if d { held <- 3; if e { held <- 5; } }` over a held 5 is merge(merge(3, 5), 5),
    // unknown; the else form and the form under a guard that may not hold are the same
    let seen = merges();
    for name in ["nested_held", "nested_other", "nested_guarded"] {
        assert_eq!(changes(&seen, name), vec![(NS / 2, UNKNOWN.to_vec())], "{name}");
    }
}

#[test]
fn a_merge_commit_meets_another_reactions_commit_of_the_same_microstep() {
    // §3.3.3, "the value the carrier would hold then": the first reaction commits 4 to `q`
    // (held 1), the second's `if d { q <- 4 }` merges with that 4, so `q` takes 4, known; merged
    // with the 1 held before the microstep it would be unknown
    let seen = merges();
    assert_eq!(changes(&seen, "pair_q"), vec![(NS / 2, byte(4))]);
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

#[test]
fn an_unknown_initial_value_starts_every_level_unknown() {
    // US-01 AC 1: `signal s: Bit<u8> = Unknown;` starts with every level unknown, so at every
    // rising edge `s.is_known()` is false and `s.known_or(9)` is the default
    let (_, records) = records_of("unknown_values.msl", "UnknownBench");
    let probes = stream(&records, "probe");
    assert_eq!(probes.len(), 4);
    for record in probes {
        assert_eq!(record.fields, [("known".to_string(), Field::Bool(false)), ("value".to_string(), Field::Int(9))]);
    }
}

#[test]
fn an_unknown_initial_value_fires_no_change_at_zero() {
    // US-01 AC 2, §3.3 "changes": unknown to unknown is no change, so `on s.changed()` never
    // runs for a carrier that starts unknown and is never written
    let (_, records) = records_of("unknown_values.msl", "UnknownBench");
    assert_eq!(stream(&records, "changed").len(), 0);
}

#[test]
fn a_const_holding_unknown_reads_unknown_everywhere() {
    // US-01 AC 3, §3.3.9: `const Z: Option<bool> = Some(Unknown)` is `Some` of an unknown value,
    // and `const W: u8 = Unknown` an unknown one; every read gives the same unknown levels
    let (_, records) = records_of("unknown_values.msl", "UnknownBench");
    let consts = stream(&records, "consts");
    assert_eq!(consts.len(), 4);
    let expected = [
        ("some".to_string(), Field::Bool(true)),
        ("z".to_string(), Field::Unknown),
        ("z_known".to_string(), Field::Bool(false)),
        ("w".to_string(), Field::Unknown),
        ("w_known".to_string(), Field::Bool(false)),
    ];
    assert!(consts.iter().all(|r| r.fields == expected), "{consts:?}");
}

#[test]
fn committing_unknown_makes_every_level_unknown() {
    // US-01 AC 5: `q <- Unknown;` makes every level of `q`, which held 5, unknown at the
    // microstep the commit applies: the first rising edge (0.5 ns)
    let (seen, _) = records_of("unknown_values.msl", "UnknownBench");
    assert_eq!(transient(&seen, "UnknownBench.q"), vec![(NS / 2, UNKNOWN.to_vec())]);
}

/// The fields every record of the stream `name` carries over the run, which must all be alike
/// (the probes read values that do not change), as `(name, field)` pairs.
fn constant_fields(records: &[Record], name: &str) -> Vec<(String, Field)> {
    let all = stream(records, name);
    assert_eq!(all.len(), 4, "one `{name}` record per rising edge: {all:?}");
    assert!(all.iter().all(|r| r.fields == all[0].fields), "the probe reads values that do not change: {all:?}");
    all[0].fields.clone()
}

/// The field `field` of the records of the stream `name` (alike, as `constant_fields` checks).
fn named(records: &[Record], name: &str, field: &str) -> Field {
    constant_fields(records, name).into_iter().find(|(n, _)| n == field).unwrap_or_else(|| panic!("no field `{field}` in `{name}`")).1
}

/// The fields of one record, named: `[("a", Field::Bool(true)), ..]`.
fn fields(list: &[(&str, Field)]) -> Vec<(String, Field)> {
    list.iter().map(|(name, field)| ((*name).to_string(), field.clone())).collect()
}

#[test]
fn level_reads_each_level_of_a_known_value() {
    // US-02 AC 1, §22.1: `x` holds 165 = 1010 0101, so level 0 is 1, 1 is 0, 2 is 1 and 7 is 1,
    // and level 7 is `Some`
    let (_, records) = records_of("levels.msl", "LevelsBench");
    let wanted = fields(&[("b0", Field::Bool(true)), ("b1", Field::Bool(false)), ("b2", Field::Bool(true)), ("b7", Field::Bool(true)), ("some7", Field::Bool(true))]);
    assert_eq!(constant_fields(&records, "known"), wanted);
}

#[test]
fn level_with_a_known_index_is_known() {
    // US-02 AC 1 and 2, §3.3.14: a level of an unknown value is `None`, and the answer is known:
    // `u.level(3).is_some()` is a known `false`, `unwrap_or(true)` the known default
    let (_, records) = records_of("levels.msl", "LevelsBench");
    assert_eq!(constant_fields(&records, "unknown"), fields(&[("some", Field::Bool(false)), ("value", Field::Bool(true))]));
}

#[test]
fn level_out_of_range_is_a_panic_naming_k() {
    // US-02 AC 4, §1 rule 11: `x.level(8)` of a `u8` is a run fault, whose message names the
    // level and the count
    let built = build(&project("levels.msl"), "OutsideBench");
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| built.run(2e-9, 0.5e-9)));
    let message = match run {
        Ok(_) => panic!("level 8 of a `u8` does not exist, the run must fault"),
        Err(payload) => payload.downcast_ref::<String>().cloned().unwrap_or_default(),
    };
    assert!(message.contains("level 8") && message.contains("out of range"), "{message}");
}

#[test]
fn level_with_an_unknown_index_merges_every_level() {
    // US-02 AC 5, §3.3.13: `k` is unknown, so `x.level(k)` is the merge of every level: `x` =
    // 1010 0101 has both bits, so the value is unknown (`mixed`), while `ones` = 255 has only 1s,
    // so it is a known `true` (`all`); every level exists, so the level is `Some` (`some`)
    let (_, records) = records_of("levels.msl", "LevelsBench");
    assert_eq!(
        constant_fields(&records, "indexed"),
        fields(&[("mixed", Field::Unknown), ("all", Field::Bool(true)), ("some", Field::Bool(true))])
    );
}

#[test]
fn merge_of_equal_known_values_is_that_value() {
    // US-03 AC 1, §22.1.2 rule 1: `merge(5, 5)` is 5 and `merge(true, true)` is true, known
    let (_, records) = records_of("contracts.msl", "ContractsBench");
    assert_eq!(named(&records, "merge", "equal"), Field::Int(5));
    assert_eq!(named(&records, "merge", "same_bits"), Field::Bool(true));
}

#[test]
fn merge_of_different_values_is_unknown_and_commutative() {
    // US-03 AC 2, §22.1.2 rules 1 and 2: `merge(5, 6)` and `merge(6, 5)` differ in a level, so
    // both are unknown, and `merge(true, false)` is unknown; they have the same levels and flags
    let (_, records) = records_of("contracts.msl", "ContractsBench");
    assert_eq!(named(&records, "merge", "differ"), Field::Unknown);
    assert_eq!(named(&records, "merge", "swapped"), Field::Unknown);
    assert_eq!(named(&records, "merge", "bits"), Field::Unknown);
    assert_eq!(named(&records, "merge", "commutes"), Field::Bool(true));
}

#[test]
fn merge_of_a_value_with_itself_is_that_value() {
    // US-03 edge case, §22.1.2 rule 2: `merge(a, a)` has the levels and flags of `a`: 5 for a
    // known `a`, unknown for an unknown one; `merge(5, unknown)` is unknown
    let (_, records) = records_of("contracts.msl", "ContractsBench");
    assert_eq!(named(&records, "merge", "equal"), Field::Int(5));
    assert_eq!(named(&records, "merge", "of_itself"), Field::Unknown);
    assert_eq!(named(&records, "merge", "with_unknown"), Field::Unknown);
}

#[test]
fn same_compares_flags_and_values() {
    // US-03 AC 3, §22.1.4, §3.3.10: `same` is a known `bool`: equal known values are the same,
    // different ones are not, two unknown values are the same, a known and an unknown one are
    // not, and `Unknown == Unknown` is unknown
    let (_, records) = records_of("contracts.msl", "ContractsBench");
    assert_eq!(
        constant_fields(&records, "same"),
        fields(&[
            ("equal", Field::Bool(true)),
            ("differ", Field::Bool(false)),
            ("both_unknown", Field::Bool(true)),
            ("one_unknown", Field::Bool(false)),
            ("eq_unknown", Field::Unknown),
            ("flags", Field::Bool(false)),
        ])
    );
}

#[test]
fn cast_keeps_each_level_and_its_flag() {
    // US-03 AC 4, §22.1.3: 5 as an `i8` is 5, back to a `u8` is 5, an unknown value stays unknown,
    // and a tag the enum has is its variant (`Mode::Run` is 1)
    let (_, records) = records_of("contracts.msl", "ContractsBench");
    assert_eq!(named(&records, "cast", "signed"), Field::Int(5));
    assert_eq!(named(&records, "cast", "unsigned"), Field::Int(5));
    assert_eq!(named(&records, "cast", "unknown"), Field::Unknown);
    assert_eq!(named(&records, "cast", "mode"), Field::Bool(true));
}

#[test]
fn cast_to_a_tag_with_no_variant_is_unknown() {
    // US-03 AC 5, §22.1.3 rule 2: tag 3 has no variant of a three-variant enum, so the cast is
    // `Unknown` and `known_or` gives the default `Mode::Idle`
    let (_, records) = records_of("contracts.msl", "ContractsBench");
    assert_eq!(named(&records, "cast", "missing"), Field::Bool(true));
}

#[test]
fn logic_builds_known_and_unknown_values() {
    // US-03 AC 6, §22.1.4: `logic("1010")` is 10 (the leftmost character is the highest level),
    // `"0000"` is 0, `"xxxx"` is unknown, a `bool` pattern `"1"` is true, and the pattern builds
    // an initial value too
    let (_, records) = records_of("patterns.msl", "PatternsBench");
    assert_eq!(
        constant_fields(&records, "logic"),
        fields(&[
            ("ones", Field::Int(10)),
            ("zeros", Field::Int(0)),
            ("skip", Field::Unknown),
            ("initial", Field::Int(10)),
            ("bit", Field::Bool(true)),
        ])
    );
}

#[test]
fn matches_bits_follows_its_table() {
    // US-03 AC 7, §22.1.4: `a` = 0101. An exact pattern, a wildcard one and an all-`x` one match;
    // a pattern that differs in a level does not; an unknown value matches only an all-`x`
    // pattern, and the answer is a known `bool` either way
    let (_, records) = records_of("patterns.msl", "PatternsBench");
    assert_eq!(
        constant_fields(&records, "matches"),
        fields(&[
            ("exact", Field::Bool(true)),
            ("wild", Field::Bool(true)),
            ("all_x", Field::Bool(true)),
            ("wrong", Field::Bool(false)),
            ("low_wild", Field::Bool(true)),
            ("unknown", Field::Bool(false)),
            ("unknown_all_x", Field::Bool(true)),
        ])
    );
}

#[test]
fn short_circuit_operands_follow_the_merge() {
    // US-04 AC 8, §3.3.12, §1 rule 13: with `u` unknown, `u && false` is false and `u || true` is
    // true, `u && true` and `u || false` are unknown, and a right operand that panics (`10 / 0`)
    // makes the merge unknown, no fault; a known left operand that decides never runs the right
    // one (`f && 10 / 0` is false, `t || 10 / 0` is true), and a known one that does not gives
    // the right operand's value.
    let (_, records) = records_of("short_circuit.msl", "ShortCircuitBench");
    assert_eq!(
        constant_fields(&records, "and"),
        fields(&[("decided", Field::Bool(false)), ("unknown", Field::Unknown), ("panics", Field::Unknown), ("skipped", Field::Bool(false)), ("run", Field::Bool(true))])
    );
    assert_eq!(
        constant_fields(&records, "or"),
        fields(&[("decided", Field::Bool(true)), ("unknown", Field::Unknown), ("panics", Field::Unknown), ("skipped", Field::Bool(true)), ("run", Field::Bool(false))])
    );
    // A right operand that emits runs under the merge: its record exists and says `maybe`, and
    // the values are the same table.
    assert_eq!(constant_fields(&records, "emits"), fields(&[("unknown_and", Field::Unknown), ("unknown_or", Field::Unknown)]));
    let ran = stream(&records, "ran");
    assert_eq!(ran.len(), 8, "two emitting right operands per rising edge: {ran:?}");
    assert!(ran.iter().all(|r| r.maybe), "reached under unknown control");
}

#[test]
fn a_math_function_of_an_unknown_value_is_unknown() {
    // US-06 AC 1, §3.3.2: `sqrt` of the known 4.0 is 2.0; `sqrt`, `abs`, `exp` and `ln` of a value
    // that is unknown are unknown, so `known_or` gives its default and the bare result is unknown
    let (_, records) = records_of("math.msl", "MathBench");
    assert_eq!(
        constant_fields(&records, "math"),
        fields(&[
            ("root", Field::Real(2.0)),
            ("no_root", Field::Real(-1.0)),
            ("abs", Field::Real(-2.0)),
            ("exp", Field::Real(-3.0)),
            ("ln", Field::Real(-4.0)),
            ("still_unknown", Field::Unknown),
        ])
    );
}

#[test]
fn known_or_with_an_unknown_default_gives_that_default() {
    // US-06 AC 2, §22.1.1 rule 4: `x.known_or(d)` is `x` when `x` is known (4.0, whatever `d`
    // is), else `d` with its flags: 9.0 for a known `d`, unknown for an unknown one
    let (_, records) = records_of("math.msl", "MathBench");
    assert_eq!(
        constant_fields(&records, "default"),
        fields(&[("held", Field::Real(4.0)), ("taken", Field::Real(9.0)), ("both_unknown", Field::Unknown), ("unknown_flag", Field::Bool(false))])
    );
}
