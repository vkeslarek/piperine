//! Arrays of carriers run from MSL source on the Rust backend (feature `composite-values` phase C, CV-23 to CV-26): one
//! test per run-time criterion, each model written in `tests/carrier_arrays/`, elaborated by the front end, compiled
//! on the transpiled backend and run on the solver. Every expected value is computed by Rust in the test from the rule
//! it cites (part II §5.1.8, part IV §12.1), never read back from the model.
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

/// The model files of `tests/carrier_arrays/`.
const RUN: SourceRun = SourceRun::in_directory("tests/carrier_arrays");

/// The edges of the 10 GHz oscillator a 4 ns run sees: rising edge `n` is at `0.05 + 0.1 n` ns.
struct Edges;

impl Edges {
    /// The number of rising edges in the run.
    const COUNT: usize = 40;

    /// The instant of rising edge `n`, in nanoseconds rounded to a hundredth.
    fn at(n: usize) -> f64 {
        (5.0 + 10.0 * n as f64) / 100.0
    }
}

/// Reading the records of a run.
struct Records;

impl Records {
    /// The records of stream `name` of `top` of `arrays.msl`, in order.
    fn of(top: &str, name: &str) -> Vec<Record> {
        RUN.records_of("arrays.msl", top).into_iter().filter(|record| record.stream == name).collect()
    }

    /// The integer field `field` of `record`.
    fn integer(record: &Record, field: &str) -> i128 {
        match record.fields.iter().find(|(name, _)| name == field) {
            Some((_, Field::Int(value))) => i128::from(*value),
            other => panic!("{field} is an integer: {other:?}"),
        }
    }

    /// The boolean field `field` of `record`.
    fn boolean(record: &Record, field: &str) -> bool {
        match record.fields.iter().find(|(name, _)| name == field) {
            Some((_, Field::Bool(value))) => *value,
            other => panic!("{field} is a bool: {other:?}"),
        }
    }

    /// The instant of `record` in nanoseconds, rounded to a hundredth.
    fn nanoseconds(record: &Record) -> f64 {
        (record.time * 1e11).round() / 100.0
    }
}

#[test]
fn each_element_of_an_array_of_carriers_has_its_own_events() {
    // §5.1.4 (CV-23 AC 1, CV-24 AC 2): at rising edge `n` element `n % 4` of `x` toggles, so `x[k].changed()` fires
    // after the edges `n` with `n % 4 == k` and no other: its instants are the edges `k, k + 4, ...` (the commit lands
    // in the next microstep of the instant of the edge), after the initial commit at `(0, 0)` that is a change of every
    // carrier (§13 rule 6)
    let changes = Records::of("TogglesBench", "changed");
    for element in 0..4usize {
        let seen: Vec<f64> = changes.iter().filter(|r| Records::integer(r, "element") == element as i128).map(Records::nanoseconds).collect();
        let wanted: Vec<f64> = std::iter::once(0.0).chain((element..Edges::COUNT).step_by(4).map(Edges::at).take_while(|at| *at < 4.0)).collect();
        assert_eq!(seen, wanted, "element {element} changes at the edges that toggle it, and only there");
    }
}

#[test]
fn arrays_and_elements_connect_element_to_element() {
    // CV-23 AC 2: `Reg(x, y)` connects element to element and `Cell(x[2], z)` one element to one carrier, so at the edge
    // after a toggle the copies equal the source. `x[k]` has toggled once for each edge `m < n` with `m % 4 == k`
    for probe in Records::of("TogglesBench", "probe") {
        let n = Records::integer(&probe, "n") as usize;
        let x: Vec<bool> = (0..4).map(|k| (0..n).filter(|m| m % 4 == k).count() % 2 == 1).collect();
        for (k, wanted) in x.iter().enumerate() {
            assert_eq!(Records::boolean(&probe, &format!("y{k}")), *wanted, "edge {n}: y[{k}] is x[{k}] once the commit has landed");
        }
        assert_eq!(Records::boolean(&probe, "z"), x[2], "edge {n}: z is x[2]");
    }
}

#[test]
fn an_array_of_carriers_reads_whole_as_an_array_value() {
    // CV-24 AC 1: `let w = y;` is the array of the elements' values, so `w[k]` is `y[k]` of the same edge
    let probes = Records::of("TogglesBench", "probe");
    let wholes = Records::of("TogglesBench", "whole");
    assert_eq!(probes.len(), wholes.len(), "one of each at every edge");
    for (probe, whole) in probes.iter().zip(&wholes) {
        for k in 0..4 {
            assert_eq!(Records::boolean(whole, &format!("w{k}")), Records::boolean(probe, &format!("y{k}")), "the whole read gives element {k}");
        }
    }
}

#[test]
fn element_history_and_events_apply_to_that_element_only() {
    // CV-24 AC 2: `x[1].prev()` and `x[1].changed()` are about element 1. At each change after `(0, 0)` its previous value
    // is the opposite of its value (it toggles), and element 0, which toggles at other edges, keeps its own history:
    // it has toggled once for each edge `m < n` with `m % 4 == 0`
    let history = Records::of("TogglesBench", "history");
    assert!(history.len() > 3, "element 1 changes more than once");
    let edges: Vec<usize> = (0..Edges::COUNT).filter(|n| n % 4 == 1).collect();
    for (record, n) in history.iter().skip(1).zip(edges) {
        assert_ne!(Records::boolean(record, "prev"), Records::boolean(record, "now"), "at edge {n} element 1 toggles: its previous value is the opposite");
        let x0 = (0..n).filter(|m| m % 4 == 0).count() % 2 == 1;
        assert_eq!(Records::boolean(record, "other"), x0, "at edge {n} element 0 holds the value its own commits gave");
        assert_ne!(Records::boolean(record, "other_prev"), x0, "and its previous value is its own, the opposite");
    }
}

#[test]
fn an_element_never_committed_reads_the_arrays_initial_value() {
    // §5.1.8, CV-24 AC 4, DC1: elements 500 and 1048575 are never committed: they read the initial value 9 at every
    // edge, and so does `prev()`; the committed elements read what was written
    for probe in Records::of("MemoryBench", "probe") {
        let n = Records::integer(&probe, "n");
        assert_eq!(Records::integer(&probe, "mid"), 9, "edge {n}: an element no model commits reads the initial value");
        assert_eq!(Records::integer(&probe, "mid_prev"), 9, "edge {n}: and so does its `prev()`");
        assert_eq!(Records::integer(&probe, "last"), 9, "edge {n}: the last of a million elements");
        let committed = |k: i128| if n > k { 100 + k } else { 9 };
        assert_eq!(Records::integer(&probe, "first"), committed(0), "edge {n}: element 0 holds what edge 0 wrote");
    }
}

#[test]
fn an_element_commit_changes_that_element_only() {
    // CV-25 AC 1: `mem[n] <- 100 + n` at edge `n` changes element `n` and fires `changed()` on that element only:
    // element 3 changes at `(0, 0)` and after edge 3, element 5 at `(0, 0)` and after edge 5, and no other time
    let third: Vec<f64> = Records::of("MemoryBench", "third").iter().map(Records::nanoseconds).collect();
    assert_eq!(third, vec![0.0, Edges::at(3)], "element 3 changes when edge 3 writes it");
    let fifth: Vec<f64> = Records::of("MemoryBench", "fifth").iter().map(Records::nanoseconds).collect();
    assert_eq!(fifth, vec![0.0, Edges::at(5)], "element 5 changes when edge 5 writes it");
    let written = Records::of("MemoryBench", "third");
    assert_eq!((Records::integer(&written[1], "value"), Records::integer(&written[1], "before")), (103, 9), "the value written and the one before");
}

#[test]
fn a_field_or_value_element_commit_changes_those_levels_only() {
    // CV-25 AC 2: `st.n <- ..` changes `n` and not `ok`; `word[2] <- true` changes bit 2 of the value array and no other
    let fields = Records::of("FieldsBench", "fields");
    let after = fields.iter().find(|r| Records::integer(r, "edge") == 1).expect("the edge after the commits");
    assert_eq!(Records::integer(after, "n"), 2, "the field written");
    assert!(Records::boolean(after, "w2") && !Records::boolean(after, "w0") && !Records::boolean(after, "w7"), "only bit 2 of the value array");
    let before = fields.iter().find(|r| Records::integer(r, "edge") == 0).expect("the first edge");
    assert_eq!((Records::integer(before, "n"), Records::boolean(before, "ok")), (0, false), "the initial value, before the commits");
}

#[test]
fn overlapping_commits_in_one_microstep_keep_the_last_write_per_level() {
    // §12.1.2, CV-25 AC 2: `st.n <- 3; .. st.n <- 1; st.ok <- true; st.n <- 2;` leaves `n` 2 (the last write of its levels)
    // and `ok` true (a different level, written once); `bits[1] <- true; bits[1] <- false;` leaves the element false
    let fields = Records::of("FieldsBench", "fields");
    let after = fields.iter().find(|r| Records::integer(r, "edge") == 1).expect("the edge after the commits");
    assert_eq!((Records::integer(after, "n"), Records::boolean(after, "ok")), (2, true), "the last write per level");
    assert!(!Records::boolean(after, "b1"), "the later commit of one element wins");
}

#[test]
fn a_run_time_element_index_out_of_range_names_carrier_index_instance_and_instant() {
    // CV-25 AC 5, §1 rule 11: `small[n + 4] <- true` at the first edge (0.05 ns, tick 50000 of one femtosecond) is element 4
    // of 4: the analysis fails naming the carrier, the index, the instance and the instant
    let fault = RUN.the_fault("arrays.msl", "ReachBench");
    for part in ["small", "4", "ReachBench.reach", "tick 50000"] {
        assert!(fault.contains(part), "the fault names `{part}`: {fault}");
    }
}

#[test]
fn an_element_given_to_an_out_port_is_driven_by_the_child() {
    // CV-25, §5.2.6: `Cell(x[0], lines[3])` hands element 3 of `lines` to the child, which drives it with `x[0]`; the
    // parent drives `lines[1]` itself (it toggles at every edge), and element 3 follows `x[0]` one microstep behind
    for probe in Records::of("TogglesBench", "probe") {
        let n = Records::integer(&probe, "n") as usize;
        let x0 = (0..n).filter(|m| m % 4 == 0).count() % 2 == 1;
        assert_eq!(Records::boolean(&probe, "l3"), x0, "edge {n}: the child drives lines[3] with x[0]");
        assert_eq!(Records::boolean(&probe, "l1"), n % 2 == 1, "edge {n}: the parent drives lines[1] itself");
    }
}

/// The values the shift register and the RAM take, computed by Rust from the rule (reads see the start of the microstep,
/// the commits of the microstep apply together, §12.1.1).
struct Models;

impl Models {
    /// The pattern `rx` plays: `pat[n % 8]`.
    const PATTERN: [bool; 8] = [true, false, true, true, false, false, true, false];

    /// The shift register `sh` at the start of each of `edges` rising edges, and `rx` at the start of each.
    fn shift(edges: usize) -> Vec<([bool; 8], bool)> {
        let (mut sh, mut rx) = ([false; 8], false);
        let mut seen = Vec::new();
        for n in 0..edges {
            seen.push((sh, rx));
            let next_rx = Self::PATTERN[n % 8];
            let mut next = sh;
            next[7] = rx;
            for k in 0..7 {
                next[k] = sh[k + 1];
            }
            (sh, rx) = (next, next_rx);
        }
        seen
    }

    /// The word the RAM reads at each of `edges` rising edges: `(addr, read)`.
    fn ram(edges: usize) -> Vec<(usize, u32)> {
        let mut mem = [0u32; 256];
        let mut seen = Vec::new();
        for n in 0..edges {
            let a = n % 16;
            seen.push((a, mem[a]));
            if n % 2 == 0 {
                mem[a] = 1000 + n as u32;
            }
        }
        seen
    }
}

#[test]
fn the_shift_register_and_the_ram_give_their_expected_sequences() {
    // §8.2 (`UartShift`), E.5 (`Ram`): each edge's record is what the rule gives, computed above
    let shifts = Records::of("ShiftBench", "shift");
    assert!(shifts.len() >= 30, "the register runs for many edges");
    for (record, (sh, _)) in shifts.iter().zip(Models::shift(shifts.len())) {
        for (k, wanted) in sh.iter().enumerate() {
            assert_eq!(Records::boolean(record, &format!("s{k}")), *wanted, "edge {}: sh[{k}]", Records::integer(record, "n"));
        }
    }
    let reads = Records::of("RamBench", "ram");
    for (record, (addr, read)) in reads.iter().zip(Models::ram(reads.len())) {
        assert_eq!((Records::integer(record, "addr"), Records::integer(record, "read")), (addr as i128, i128::from(read)), "edge {}: the address and the word read before the write", Records::integer(record, "n"));
    }
}
