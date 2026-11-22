//! The explicit model of unknowns run from MSL source on the Rust backend (feature
//! `explicit-unknowns`): one test per run-time criterion, each model written in `tests/explicit/`,
//! elaborated by the front end, compiled on the transpiled backend and run on the solver. Every
//! expected value is written from the rule it cites (part I §3.3, §5.1, §13), never read back from
//! the model.
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

use std::path::Path;

use piperine_api::{Field, Record};
use piperine_parity::project::Project;
use mixed::Seen;
use run::SourceRun;
use support::{build, series, transient};

/// The model files of `tests/explicit/`.
const RUN: SourceRun = SourceRun::in_directory("tests/explicit");

#[test]
fn a_binary_carrier_is_read_as_its_value() {
    // §3.3.6 (EU-01 AC 1): `analog` reads a discrete carrier like any expression, with no `known_or`.
    // `r` holds 3.5 and `en` holds true, so the two forced nodes are 3.5 V and 2 V.
    let volts = RUN.operating_point("binary_read.msl", "Bench", &["a", "b"]);
    assert!((volts[0] - 3.5).abs() < 1e-9, "a `Bit<f64>` is read as its value: {}", volts[0]);
    assert!((volts[1] - 2.0).abs() < 1e-9, "a `Bit` is read as its value: {}", volts[1]);
}

#[test]
fn a_binary_carrier_of_any_shape_is_read_as_its_value_everywhere() {
    // §3.3.1, §3.3.6 (EU-01 AC 1): a `Bit<u8>`, a struct and an array carrier are read by `analog` directly and through a
    // function, and by a reaction directly and through a function. `n` is 5, `g` is `Gains { lo: 2, hi: 9 }`, `a` is
    // `[4, 6, 8]`: the nodes are 5, 10, 9, 9 - 2, 6 and 8 V, and every record reads the same values
    let volts = RUN.operating_point("binary_read.msl", "CountsBench", &["n_node", "f_node", "g_node", "s_node", "a_node", "t_node"]);
    for (read, (got, wanted)) in ["n", "twice(n)", "g.hi", "spread(g)", "a[1]", "third(a)"].iter().zip(volts.iter().zip([5.0, 10.0, 9.0, 7.0, 6.0, 8.0])) {
        assert!((got - wanted).abs() < 1e-9, "`{read}` in analog: {got} V, wanted {wanted}");
    }
    let seen: Vec<Record> = RUN.records_of("binary_read.msl", "CountsBench").into_iter().filter(|record| record.stream == "seen").collect();
    assert!(!seen.is_empty(), "the reaction runs at the rising edges");
    for record in seen {
        let values: Vec<Field> = record.fields.iter().map(|(_, field)| field.clone()).collect();
        assert_eq!(values, [Field::Int(5), Field::Int(10), Field::Int(9), Field::Int(7), Field::Int(6), Field::Int(8)], "n, twice(n), g.hi, spread(g), a[1], third(a) in a reaction");
    }
}

#[test]
fn the_buck_elaborated_from_source_holds_no_flag_and_no_merge() {
    // EU-01 AC 3 (C3): the buck, elaborated from `examples/buck` (every digital and mixed form the model uses), prints
    // an IR and generates Rust that name no flag, no merge and no known byte: not in a type, a read, a commit or a
    // reaction. The words are the removed forms of the old model (`flagged`, `known_or`, `commit_with`, `maybe`, ...)
    let project = Project::new(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/buck/src/lib.msl"));
    let ir = std::sync::Arc::new(project.elaborate("Buck").unwrap_or_else(|rejection| panic!("{rejection}")));
    let generated = piperine_transpile::Transpiler::new(ir.clone()).expect("the buck transpiles").source().expect("its source");
    let printed = format!("{}\n{}", ir.print_structural(), ir);
    assert!(printed.lines().count() > 200 && generated.lines().count() > 200, "a real model: {} IR lines, {} generated lines", printed.lines().count(), generated.lines().count());
    let removed = ["flagged", "known_or", "is_known", ".high()", "Unknown", "merge(", "level_is", "level_value", "commit_with", "commit_flag", "Flagged", "HAS_KNOWN", "known_of", "maybe", "MAYBE", "unknown"];
    for form in removed {
        assert!(!printed.contains(form), "the IR prints `{form}`");
        assert!(!generated.contains(form), "the generated Rust holds `{form}`: {}", generated.lines().find(|l| l.contains(form)).unwrap_or_default());
    }
}

/// The records of the stream `name` the transient of `top` of `initial.msl` made, in order.
fn stream(top: &str, name: &str) -> Vec<Record> {
    RUN.records_of("initial.msl", top).into_iter().filter(|record| record.stream == name).collect()
}

#[test]
fn a_written_initial_value_is_the_value_at_the_start() {
    // §3.3.4, §5.1.1 (EU-04 AC 1): `signal s: Bit<u8> = 7;` is 7 from `(0, 0)`: every probe at a rising edge reads 7
    let probes = stream("WrittenBench", "written");
    assert!(!probes.is_empty(), "the probe runs at the rising edges");
    for record in probes {
        assert_eq!(record.fields, [("value".to_string(), Field::Int(7))], "the carrier holds its written value");
    }
}

/// The integer fields of `record`, in order.
fn integers(record: &Record) -> Vec<i128> {
    record.fields.iter().map(|(name, field)| match field {
        Field::Int(value) => i128::from(*value),
        other => panic!("{name} is an integer: {other:?}"),
    }).collect()
}

#[test]
fn a_carrier_without_a_value_starts_at_its_default() {
    // §3.3.4, H.8 rule 4 (EU-04 AC 2): `signal o: Bit<Option<u8>>;` starts at `None`, so every read before a commit
    // takes the `None` arm: `match` 99, `if let` leaves 98, `unwrap_or` 97
    let probes = stream("DefaultBench", "optional");
    assert!(!probes.is_empty(), "the probe runs at the rising edges");
    for record in &probes {
        assert_eq!(integers(record), [99, 98, 97], "a carrier of `Option<u8>` with no value reads `None`");
    }
}

#[test]
fn an_option_carrier_is_read_by_matching() {
    // §1.7, §5.1.1 (EU-06 AC 2): `match`, `if let` and `unwrap_or` read a `Bit<Option<u8>>` carrier. The commit of
    // `Some(5)` at the first rising edge lands in the next microstep, so that edge reads `None` and every later
    // one reads 5
    let probes = stream("SetBench", "optional");
    assert!(probes.len() > 1, "more than one rising edge");
    assert_eq!(integers(&probes[0]), [99, 98, 97], "the commit has not landed at its own edge");
    for record in &probes[1..] {
        assert_eq!(integers(record), [5, 5, 5], "`Some(5)` is read by the three forms");
    }
}

#[test]
fn the_first_commit_is_a_change() {
    // §5.3.4, D8 (EU-13 AC 2): the commit of the initial value at `(0, 0)` is a change of every carrier, for a written
    // value (`= 7`) and for the default of a type (`None`): `changed()` fires once, at time 0
    let written = stream("WrittenBench", "changed");
    assert_eq!(written.len(), 1, "one change, at the start: {written:?}");
    assert_eq!(written[0].time, 0.0, "at (0, 0)");
    assert_eq!(written[0].fields[1].1, Field::Int(7), "the value committed");
    let defaulted = stream("DefaultBench", "optional_changed");
    assert_eq!(defaulted.len(), 1, "one change, at the start: {defaulted:?}");
    assert_eq!(defaulted[0].time, 0.0, "at (0, 0)");
}

#[test]
fn prev_is_the_initial_value_before_a_change() {
    // §5.3.5, D8 (EU-13 AC 3): `q` starts at 4 and is committed 9 at the first rising edge. `prev()` reads 4 at that
    // edge, before any change after the start, and 4 again at the later edges, the value before the one change
    let probes = stream("PreviousBench", "previous");
    assert!(probes.len() > 1, "more than one rising edge");
    assert_eq!(integers(&probes[0]), [4, 4], "`prev()` and the value are the initial value at the first edge");
    for record in &probes[1..] {
        assert_eq!(integers(record), [4, 9], "`prev()` is the value before the change");
    }
}

#[test]
fn an_element_never_committed_reads_its_initial_value() {
    // §5.1.8 (EU-04 AC 3): an element no model commits reads the initial value written for it. Arrays of carriers
    // (`[Bit<Option<u8>>; 1 << 20]`, `GAPS.md` L24) are not built yet, so the elements are those of a composite
    // `Bit<[u8; 4]> = [5, 6, 7, 8]`, read at their first and last positions
    let probes = stream("ElementsBench", "elements");
    assert!(!probes.is_empty(), "the probe runs at the rising edges");
    for record in &probes {
        assert_eq!(integers(record), [5, 8], "each element reads its initial value");
    }
}

#[test]
fn an_unbound_option_param_waits_for_params_and_io() {
    // §8.1 (EU-06 AC 1, `an_unbound_option_param_is_none` once it can run): a param of type `Option<u8>` is `P0001`
    // today, `GAPS.md` L28, owned by `params-and-io`, which builds the param pins. The test fails when the param
    // elaborates, so the day it runs the criterion's own test replaces it
    let project = RUN.project("initial.msl");
    let refused = match project.compile("ParamBench") {
        Ok(_) => panic!("an `Option` param now elaborates: replace this test by `an_unbound_option_param_is_none`"),
        Err(report) => report.to_string(),
    };
    assert!(refused.contains("P0001") && refused.contains("param of type `Option<u8>` is not supported yet"), "{refused}");
}

/// The records of the stream `name` the transient of `top` of `edges.msl` made, in order.
fn edge_stream(top: &str, name: &str) -> Vec<Record> {
    RUN.records_of("edges.msl", top).into_iter().filter(|record| record.stream == name).collect()
}

/// The instants of `records`, in nanoseconds rounded to a tenth.
fn nanoseconds(records: &[Record]) -> Vec<f64> {
    records.iter().map(|record| (record.time * 1e10).round() / 10.0).collect()
}

#[test]
fn a_change_is_a_different_value() {
    // §5.3.4, D8 (EU-13 AC 1): `s` starts at 1, and is committed 2, 2 and 3 at the first three rising edges. The first
    // commit of each value is a change; the second 2 is the held value and is not. So `changed()` fires at `(0, 0)`
    // for the initial 1, after the first edge for 2, and after the third for 3
    let changes = edge_stream("Bench", "changed");
    assert_eq!(nanoseconds(&changes), [0.0, 0.5, 2.5], "the instants of the changes: {changes:?}");
    let values: Vec<Field> = changes.iter().map(|record| record.fields.iter().find(|(name, _)| name == "value").map(|(_, field)| field.clone()).expect("a value field")).collect();
    assert_eq!(values, [Field::Int(1), Field::Int(2), Field::Int(3)], "the value each change committed");
}

#[test]
fn an_edge_is_a_transition() {
    // §22.1.5 (EU-13 AC 4): `b` starts false and toggles at each rising edge of the clock (0.5, 1.5, 2.5, 3.5 ns).
    // `rise()` fires on false to true, at the first and third; `fall()` on true to false, at the second and fourth
    assert_eq!(nanoseconds(&edge_stream("Bench", "rise")), [0.5, 2.5], "false to true");
    assert_eq!(nanoseconds(&edge_stream("Bench", "fall")), [1.5, 3.5], "true to false");
}

#[test]
fn a_carrier_that_starts_high_has_not_risen() {
    // §5.3, D9 (EU-13 AC 5): a `Bit` that starts true fires no `rise()` at `(0, 0)`, and one that starts false no
    // `fall()`; nothing commits either, so no edge fires at all. `init()` still fires once, at `(0, 0)`: the first
    // commit of its carrier is a change (§22.1.6)
    for stream in ["hi_rise", "hi_fall", "lo_rise", "lo_fall"] {
        assert!(edge_stream("Quiet", stream).is_empty(), "{stream} fires on no transition");
    }
    assert_eq!(nanoseconds(&edge_stream("Quiet", "init")), [0.0], "`init()` fires once at the start");
}

#[test]
fn a_carrier_a_definition_drives_has_not_risen_at_the_start() {
    // §8.2.21, §5.3 (EU-13 AC 5): `n = !c` with `c` starting false gives `n` the value true from `(0, 0)`, with no
    // initial value of its own (an undelayed definition takes none). Its first value is its initial one, so it is no
    // rise from a value it never held
    for stream in ["defined_rise", "defined_fall"] {
        assert!(edge_stream("Defined", stream).is_empty(), "{stream} fires on no transition");
    }
}

/// One nanosecond in ticks.
const NS: i64 = 1_000_000;

/// The byte of a one-byte carrier.
fn byte(value: u8) -> Vec<u8> {
    vec![value]
}

/// The 4 ns transient of `Paths` of `paths.msl`.
fn paths() -> Seen {
    build(&RUN.project("paths.msl"), "Paths").run(4e-9, 0.5e-9)
}

/// The changes of the net `name` of `Paths`.
fn changes(seen: &Seen, name: &str) -> Vec<(i64, Vec<u8>)> {
    transient(seen, &format!("Paths.{name}"))
}

#[test]
fn control_on_a_carrier_runs_one_path() {
    // §3.3.5, EU-01 AC 2 (replaces the 14 tests of `source_unknowns` that held a merge of two arms: a_function_and_a_definition_merge_an_unknown_if,
    // an_unknown_if_runs_both_arms_without_a_fault, both_arms_committing_commit_their_merge, equal_known_commits_stay_known,
    // one_arm_committing_merges_with_the_value_then, an_if_expression_and_an_if_statement_commit_the_same,
    // an_unknown_match_merges_every_arm, a_nested_one_armed_commit_merges_with_the_path_before_it,
    // a_merge_commit_meets_another_reactions_commit_of_the_same_microstep, a_panic_under_unknown_control_makes_the_merge_unknown,
    // definitions_lets_and_functions_merge_alike, neither_arm_committing_commits_nothing,
    // an_unknown_delay_under_unknown_control_commits_unknown_next, delays_one_tick_apart_are_one_key): `d` holds true, `e` false, `sel` 1,
    // so every `if`, `match` and function takes one path and commits what that path commits
    let seen = paths();
    let at_first_edge = |name: &str| changes(&seen, name);
    assert_eq!(at_first_edge("differ"), vec![(NS / 2, byte(5))], "`if d` takes its first arm");
    assert_eq!(at_first_edge("equal"), vec![(NS / 2, byte(5))]);
    assert_eq!(at_first_edge("before"), vec![(NS / 2, byte(9))], "a commit outside the `if` is made once");
    assert_eq!(at_first_edge("changes"), vec![(NS / 2, byte(7))], "the one arm commits 7 over the 3 held");
    assert_eq!(at_first_edge("keeps"), Vec::<(i64, Vec<u8>)>::new(), "the held 7 committed again is no change");
    assert_eq!(at_first_edge("idle"), Vec::<(i64, Vec<u8>)>::new(), "arms that only bind commit nothing");
    assert_eq!(at_first_edge("statement"), vec![(NS / 2, byte(1))]);
    assert_eq!(at_first_edge("expression"), at_first_edge("statement"), "an `if` expression and an `if` statement commit the same");
    assert_eq!(at_first_edge("picked"), vec![(NS / 2, byte(11))], "`match sel` takes the arm of 1");
    assert_eq!(at_first_edge("nested_held"), vec![(NS / 2, byte(3))], "the inner `if e` does not run");
    assert_eq!(at_first_edge("nested_other"), Vec::<(i64, Vec<u8>)>::new(), "the else arm is not reached");
    assert_eq!(at_first_edge("nested_guarded"), Vec::<(i64, Vec<u8>)>::new(), "a guard that does not hold runs nothing");
    assert_eq!(at_first_edge("pair_q"), vec![(NS / 2, byte(4))]);
    assert_eq!(at_first_edge("panic_q"), vec![(NS / 2, byte(5))], "the arm that divides by zero is not taken: no fault");
    assert_eq!(at_first_edge("panic_before"), vec![(NS / 2, byte(9))]);
    for name in ["by_let", "by_function"] {
        assert_eq!(at_first_edge(name), vec![(NS / 2, byte(3))], "{name}");
    }
    assert_eq!(series(&seen, "Paths.by_definition").pop().map(|(_, bytes)| bytes), Some(byte(3)), "a definition decides alike");
    assert_eq!(at_first_edge("late_q"), vec![(3 * NS / 2, byte(2))], "a delay is an f64: `after 1n` commits 1 ns after the edge");
    assert_eq!(at_first_edge("ticks_q"), vec![(3 * NS / 2, byte(1))], "only the arm `d` names commits");
    assert_eq!(at_first_edge("state"), vec![(NS / 2, byte(1)), (5 * NS / 2, byte(0)), (7 * NS / 2, byte(1))], "a phase detector with an initial state");
}

/// The instant of the last step of the transient of `top` of `paths.msl`, run to 4 ns.
fn last_step(top: &str) -> f64 {
    let (_, result) = build(&RUN.project("paths.msl"), top).run_with(4e-9, 0.5e-9);
    result.iter().last().expect("a time point").time()
}

#[test]
fn a_finish_ends_the_run_where_its_guard_holds() {
    // §19 (replaces a_maybe_finish_does_not_end_the_run): `if d { finish(); }` with `d` true ends the run at the first rising
    // edge (0.5 ns); with `d` false the run goes on to its stop (4 ns)
    assert!((last_step("FinishBench") - 0.5e-9).abs() <= 1e-15, "a finish whose guard holds ends at the edge");
    assert!((last_step("KeepGoingBench") - 4e-9).abs() <= 1e-15, "a finish whose guard does not hold runs to stop");
}

#[test]
fn settle_runs_as_before() {
    // §15 (EU-14 AC 1): `on settle` runs once per time point and its commit lands at the next instant, so `s` changes once, to 1
    let seen = build(&RUN.project("paths.msl"), "SettleBench").run(4e-9, 0.5e-9);
    let changes = transient(&seen, "SettleBench.s");
    assert_eq!(changes.len(), 1, "one change: {changes:?}");
    assert_eq!(changes[0].1, byte(1));
}

#[test]
fn an_event_read_is_a_bool() {
    // §13 rule 2 (EU-14 AC 2): `fired <- clk.rise()` reads whether the edge fired in this microstep, so it is true at the first
    // edge and holds there
    let seen = build(&RUN.project("paths.msl"), "GuardBench").run(4e-9, 0.5e-9);
    assert_eq!(transient(&seen, "GuardBench.fired"), vec![(NS / 2, byte(1))]);
}

#[test]
fn a_false_guard_runs_nothing() {
    // §13 rule 1 (EU-14 AC 3, replaces a_known_false_guard_does_not_run, an_unknown_guard_runs_the_reaction_as_a_maybe and
    // two_commits_under_an_unknown_guard_merge_exactly): `on clk.rise() && en` with `en` true runs at each of the four rising
    // edges and `on clk.rise() && off` never runs, so `blocked`, which held 1, never changes
    let seen = build(&RUN.project("paths.msl"), "GuardBench").run(4e-9, 0.5e-9);
    let runs: Vec<i64> = transient(&seen, "GuardBench.runs").iter().map(|(at, _)| *at).collect();
    assert_eq!(runs, [NS / 2, 3 * NS / 2, 5 * NS / 2, 7 * NS / 2]);
    assert_eq!(transient(&seen, "GuardBench.blocked"), Vec::<(i64, Vec<u8>)>::new());
}

/// The instants of the records of `top` of `cross_if.msl`, in a 1 us transient.
fn hits(top: &str) -> Vec<f64> {
    let seen = build(&RUN.project("cross_if.msl"), top).run(1e-6, 0.05e-6);
    seen.all().iter().filter(|o| matches!(o.kind, piperine_solver::protocol::ObservationKind::Record { .. })).map(|o| o.time).collect()
}

#[test]
fn an_if_in_a_cross_argument_picks_its_arm() {
    // §13 (replaces an_unknown_if_in_a_cross_argument_merges): `cross(if d { v(p) - 0.3 } else { v(p) - 0.7 })` with `d` true
    // watches `v(p) - 0.3`, which a 1 V/us ramp crosses at 0.3 us
    let known = hits("KnownBench");
    assert_eq!(known.len(), 1, "one crossing: {known:?}");
    assert!((known[0] - 0.3e-6).abs() <= 1e-12, "at 0.3 us: {known:?}");
}

/// Every record of the 4 ns transient of `top` of `plain_values.msl`, decoded as the host decodes it.
fn plain_records(top: &str) -> Vec<Record> {
    RUN.records_of("plain_values.msl", top)
}

/// The fields every record of the stream `name` carries (the probes read values that do not change).
fn constant_fields(records: &[Record], name: &str) -> Vec<(String, Field)> {
    let all: Vec<&Record> = records.iter().filter(|r| r.stream == name).collect();
    assert_eq!(all.len(), 4, "one `{name}` record per rising edge: {all:?}");
    assert!(all.iter().all(|r| r.fields == all[0].fields), "the probe reads values that do not change: {all:?}");
    all[0].fields.clone()
}

/// The fields of one record, named.
fn fields(list: &[(&str, Field)]) -> Vec<(String, Field)> {
    list.iter().map(|(name, field)| ((*name).to_string(), field.clone())).collect()
}

#[test]
fn a_math_function_of_a_value_is_a_value() {
    // §3.3, §1.5 (replaces a_math_function_of_an_unknown_value_is_unknown and known_or_with_an_unknown_default_gives_that_default):
    // `sqrt` of 4.0 is 2.0, `abs` of -9.0 is 9.0, `exp(0.0)` is 1.0 and `ln(4.0) * 0.0` is 0.0
    let records = plain_records("MathBench");
    assert_eq!(constant_fields(&records, "math"), fields(&[("root", Field::Real(2.0)), ("abs", Field::Real(9.0)), ("exp", Field::Real(1.0)), ("ln", Field::Real(0.0))]));
}

#[test]
fn short_circuit_operands_decide_as_bools() {
    // §1.5, §1.8 (replaces short_circuit_operands_follow_the_merge): a left operand that decides never runs the right one
    // (`f && 10 / 0 > 1` is false, `t || 10 / 0 > 1` is true), one that does not gives the right operand's value; a block that
    // emits runs only then
    let records = plain_records("ShortBench");
    assert_eq!(constant_fields(&records, "and"), fields(&[("skipped", Field::Bool(false)), ("run", Field::Bool(true))]));
    assert_eq!(constant_fields(&records, "or"), fields(&[("skipped", Field::Bool(true)), ("run", Field::Bool(false))]));
    assert_eq!(constant_fields(&records, "emits"), fields(&[("skipped", Field::Bool(false)), ("run", Field::Bool(false))]));
    let ran = records.iter().filter(|r| r.stream == "ran").count();
    assert_eq!(ran, 4, "the one emitting right operand that runs, once per rising edge");
}

#[test]
fn an_emit_under_a_guard_is_a_plain_record() {
    // §19 (replaces a_record_under_unknown_control_is_maybe, a_record_field_keeps_its_flag and a_maybe_check_is_judged_as_unknown):
    // `if d { emit!(..) }` with `d` true writes at each edge, `if e` with `e` false never, and `emit!` itself writes a record
    // with the value it names; a `require` that holds is a pass
    let records = plain_records("RecordBench");
    let count = |name: &str| records.iter().filter(|r| r.stream == name).count();
    assert_eq!((count("taken"), count("skipped"), count("plain")), (4, 0, 4));
    assert_eq!(constant_fields(&records, "taken"), fields(&[("v", Field::Int(6)), ("k", Field::Int(3))]));
    let verdict = piperine_api::Verdict::judge(&records);
    assert!(verdict.passed());
    assert_eq!((verdict.checks(), verdict.failed().len()), (4, 0));
}

#[test]
fn an_emitting_function_runs_in_a_value_position_of_a_reaction() {
    // §19 rule 10: `sure <- heard()` commits 7 and writes its record; `if d { chosen <- heard(); }` with `d` true does too;
    // `u && ping()` with `u` true runs `ping`. Per rising edge: `heard` twice, `pinged` once
    let built = build(&RUN.project("plain_values.msl"), "CallsBench");
    let streams = piperine_api::Streams::of(built.ir.clone());
    let seen = built.run(4e-9, 0.5e-9);
    let records: Vec<Record> = seen.all().iter().filter_map(|o| streams.decode(o)).map(|r| r.unwrap_or_else(|f| panic!("{f}"))).collect();
    assert_eq!(transient(&seen, "CallsBench.sure"), vec![(NS / 2, byte(7))]);
    assert_eq!(transient(&seen, "CallsBench.chosen"), vec![(NS / 2, byte(7))]);
    assert_eq!(records.iter().filter(|r| r.stream == "heard").count(), 8);
    assert_eq!(records.iter().filter(|r| r.stream == "pinged").count(), 4);
    assert_eq!(constant_fields(&records, "both"), fields(&[("r", Field::Bool(true))]));
}
