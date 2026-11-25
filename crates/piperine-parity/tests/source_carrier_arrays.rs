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
    for part in ["small: the element 4 is out of range for an array of 4 carriers", "ReachBench.reach", "tick 50000"] {
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

#[test]
fn a_ladder_over_an_array_of_nodes_solves_as_written_out() {
    // CV-26 AC 1, CV-24 AC 2 (DC3, QC6): the ladder contributed in a loop over `bl[k]` has the operating point of the same
    // ladder written out node by node, to the last bit, and the closed form the test computes: with a 2 kOhm shunt at each
    // of four nodes behind a 1 kOhm series resistor, the node voltages follow from the transfer `r' = 1k + (2k || r)`
    let looped = RUN.operating_point("ladder.msl", "Ladder", &["bl[0]", "bl[1]", "bl[2]", "bl[3]", "bl[4]"]);
    let written = RUN.operating_point("ladder.msl", "WrittenOut", &["n0", "n1", "n2", "n3", "n4"]);
    assert_eq!(looped.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), written.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), "the same circuit, the same solution");
    // closed form: the load seen from node `k + 1` toward the end, `r_k` ohms; the voltage divides at each section
    let (series, shunt) = (1000.0_f64, 2000.0_f64);
    let mut load = [0.0_f64; 5];
    load[4] = shunt;
    for k in (1..4).rev() {
        let beyond = series + load[k + 1];
        load[k] = shunt * beyond / (shunt + beyond);
    }
    let mut wanted = [5.0_f64; 5];
    for k in 0..4 {
        wanted[k + 1] = wanted[k] * load[k + 1] / (series + load[k + 1]);
    }
    for (k, (got, want)) in looped.iter().zip(wanted).enumerate() {
        assert!((got - want).abs() < 1e-9, "node {k}: {got} V, the closed form gives {want} V");
    }
    let probed = RUN.operating_point("ladder.msl", "LadderProbed", &["tap"]);
    assert!((probed[0] - wanted[3]).abs() < 1e-9, "`v(bl[3])` is element 3 only: {} V, wanted {}", probed[0], wanted[3]);
}

/// The chain of `tests/carrier_arrays/element_readers.msl`, run for 4 ns.
struct ElementReaders;

impl ElementReaders {
    /// One nanosecond in the ticks the changes are stamped in.
    const NS: i64 = 1_000_000;

    /// The changes of the net `Chain.<name>` during the transient: instant in femtoseconds and bytes, in order.
    fn changes(name: &str) -> Vec<(i64, Vec<u8>)> {
        let built = support::build(&RUN.project("element_readers.msl"), "Chain");
        let seen = built.run(4e-9, 0.5e-9);
        support::transient(&seen, &format!("Chain.{name}"))
    }

    /// The value `s` holds after `rises` rising edges of `tick` (1 ns and 3 ns): `2 + 10 * rises`.
    fn s(rises: u8) -> u8 {
        2 + 10 * rises
    }

    /// `y[1]` after `rises` edges: `2 * s + 1` (§8.2.1).
    fn y1(rises: u8) -> u8 {
        2 * Self::s(rises) + 1
    }

    /// The changes a reader of `y[1]` makes, its value `of(rises)` after each of the two edges.
    fn following(of: impl Fn(u8) -> u8) -> Vec<(i64, Vec<u8>)> {
        vec![(Self::NS, vec![of(1)]), (3 * Self::NS, vec![of(2)])]
    }
}

#[test]
fn a_definition_reading_an_element_a_definition_drives_follows_it() {
    // §8.2.1, §5.1.8: `total = y[1] + 1` with `y[1] = s * 2 + 1` is recomputed in the microstep `y[1]` changes (it stayed at
    // its (0, 0) value before the fix, the carrier's pin being apart from the net its element drives write)
    assert_eq!(ElementReaders::changes("y[1]"), ElementReaders::following(ElementReaders::y1));
    assert_eq!(ElementReaders::changes("total"), ElementReaders::following(|rises| ElementReaders::y1(rises) + 1));
    // a chain over two arrays: `z[0] = y[1] + y[0]`, `z[1] = z[0] + 1` (two elements of one array, one reading the other,
    // with no cycle) and `last = z[1]`
    assert_eq!(ElementReaders::changes("z[0]"), ElementReaders::following(|rises| ElementReaders::y1(rises) + 2));
    assert_eq!(ElementReaders::changes("z[1]"), ElementReaders::following(|rises| ElementReaders::y1(rises) + 3));
    assert_eq!(ElementReaders::changes("last"), ElementReaders::following(|rises| ElementReaders::y1(rises) + 3));
}

#[test]
fn whole_array_run_time_element_and_reaction_readers_follow_the_elements() {
    // §5.1.8: `sum = sum3(y)` reads the whole array, `picked = y[s / 10]` an element decided at run time (`y[1]` after the
    // first edge, `y[2] = 8` after the second), and `on y[1].changed() { seen <- y[1]; }` reacts to the element
    assert_eq!(ElementReaders::changes("sum"), ElementReaders::following(|rises| 2 + ElementReaders::y1(rises) + 8));
    assert_eq!(ElementReaders::changes("picked"), vec![(ElementReaders::NS, vec![ElementReaders::y1(1)]), (3 * ElementReaders::NS, vec![8])]);
    assert_eq!(ElementReaders::changes("seen"), ElementReaders::following(ElementReaders::y1));
}

#[test]
fn an_analog_block_reads_an_element_of_an_array_of_carriers() {
    // §3.3.6: the `analog` block reads an element a definition drives (`y[1] = 2 * 2 + 1`), and elements of an array with an
    // initial value of each (`y[1] + 10 * y[0]` of `[3, 4]`), at the operating point
    let level = RUN.operating_point("element_readers.msl", "Level", &["level"]);
    assert!((level[0] - 5.0).abs() < 1e-9, "{level:?}");
    let seeded = RUN.operating_point("element_readers.msl", "Seeded", &["level"]);
    assert!((seeded[0] - (4.0 + 10.0 * 3.0)).abs() < 1e-9, "{seeded:?}");
}

#[test]
fn an_analog_function_reads_an_element_a_bundle_leaf_element_and_an_array_port() {
    // §5.1.8, §3.3.6: a cell is `R` to ground with a gate that selects `R = 1k` (high) or `2k` (low) and pulls 1 mA out of its
    // node, so the node is at `R * 1 mA`: element 1 of `gates` is high and element 2 low, `lanes[2]` high and `lanes[1]` low
    // (an array of bundles is a struct of arrays: the gate leaf is `[Bit; 3]`), an array port read at its elements 0 and 3, high on both for `up` and not for `down`
    let rows = RUN.operating_point("gated.msl", "Rows", &["a", "b"]);
    assert!((rows[0] - 1e3 * 1e-3).abs() < 1e-9 && (rows[1] - 2e3 * 1e-3).abs() < 1e-9, "{rows:?}");
    let lanes = RUN.operating_point("gated.msl", "Lanes", &["x", "y"]);
    assert!((lanes[0] - 1e3 * 1e-3).abs() < 1e-9 && (lanes[1] - 2e3 * 1e-3).abs() < 1e-9, "{lanes:?}");
    let windows = RUN.operating_point("gated.msl", "Windows", &["both", "one"]);
    assert!((windows[0] - 1e3 * 1e-3).abs() < 1e-9 && (windows[1] - 2e3 * 1e-3).abs() < 1e-9, "{windows:?}");
}

#[test]
fn a_ram_read_port_of_65536_elements_builds_and_follows_its_address_and_its_element() {
    // §5.1.8, §8.2.1: `q = mem[at]` over `[Bit<u8>; 65536]` reads one view of the array (it was one read per element, a
    // build that did not finish): `tick` toggles every ns; a rise moves `at` by 1000 and writes 41 there in the same
    // microstep, a fall writes 50 at `at`, so `q` is 41 after each rise and 50 after each fall; the build and the run take
    // seconds, not minutes
    let started = std::time::Instant::now();
    let built = support::build(&RUN.project("ram.msl"), "Ram");
    let seen = built.run(4e-9, 0.5e-9);
    let q = support::transient(&seen, "Ram.q");
    let expected: Vec<(i64, Vec<u8>)> = (1..=4).map(|edge: i64| (edge * ElementReaders::NS, vec![if edge % 2 == 1 { 41 } else { 50 }])).collect();
    assert_eq!(q, expected);
    assert!(started.elapsed() < std::time::Duration::from_secs(120), "built and ran in {:?}", started.elapsed());
}

#[test]
fn a_transient_with_an_array_no_analog_function_reads_stays_sparse() {
    // §5.1.8: `mem` has 2^32 elements and no analog function reads it, so the analog side is given none of it (it would be 4 GiB
    // of bytes): the transient takes seconds, not minutes (the bound is a fifth of what the dense copy takes). The cell reads `gates[1]`, which rises at 2 ns: the node is at
    // `2k * 1 mA` = 2 V before and `1k * 1 mA` = 1 V after.
    let started = std::time::Instant::now();
    let built = support::build(&RUN.project("sparse.msl"), "Sparse");
    let node = built.ir.node_at("Sparse.a").expect("the node `a`").id();
    let (_, result) = built.run_with(6e-9, 0.5e-9);
    let at = |time: f64| result.iter().rev().find(|step| step.time() <= time).and_then(|step| step.get_node(&piperine_solver::prelude::NodeIdentifier::Anonymous(node.index()))).expect("a point");
    assert!((at(1e-9) - 2.0).abs() < 1e-6, "before the rise: {}", at(1e-9));
    assert!((at(5e-9) - 1.0).abs() < 1e-6, "after the rise: {}", at(5e-9));
    // 2 s alone, 8 s beside a loaded suite; the dense copy of 2^32 elements does not finish in 150 s (2^28 takes 17 s)
    assert!(started.elapsed() < std::time::Duration::from_secs(30), "built and ran in {:?}", started.elapsed());
}
