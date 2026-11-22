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

use piperine_api::{Field, Record};
use run::SourceRun;

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
