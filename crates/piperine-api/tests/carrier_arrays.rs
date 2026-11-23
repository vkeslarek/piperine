//! Arrays of carriers read by a host (composite-values CV-28): `Top.mem[2]` reads one element of an array net as a scalar
//! cell, `Top.mem` every element as one array cell, `Top.bl[2]` one node of an array of nodes, an element no model committed
//! is the array's initial value, and a name that is no element of the array is `E0801`. The design is MSL written by the
//! test; an expected value is the one the model commits, cited at each assertion.
#![cfg(unix)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::OnceLock;

use piperine_api::{Cell, CompositeCell, Failure, Overrides, Project, Record, RecordSink, Session, Table};
use piperine_transpile::Settings;

/// Drops every record: the table is what a test reads.
struct Ignore;

impl RecordSink for Ignore {
    fn record(&mut self, _: &Record) {}
}

/// The slot `n` of `mem` takes `100 + n` at edge `n` (four edges), the others keep the initial value 9.
const DESIGN: &str = "pub module Osc(out clk: Bit = false) { f: f64 = 200M where f > 0.0; }\n\
digital for Osc { on clk.changed() { clk <- !clk after 0.5 / f; } }\n\
module Memory(in clk: Bit) {\n    signal n: Bit<u8> = 0;\n    signal mem: [Bit<u8>; 6] = [9; 6];\n}\n\
digital for Memory { on clk.rise() { n <- n + 1; if n < 4 { mem[n] <- 100 + n; } } }\n\
module Ladder() { signal bl: [Electrical; 3]; }\n\
analog for Ladder { v(bl[0], gnd) +> 3.0; for k in 0..2 { i(bl[k], bl[k + 1]) +> v(bl[k], bl[k + 1]) / 1000.0; } i(bl[2], gnd) +> v(bl[2], gnd) / 1000.0; }\n\
module Bank(in lanes: [Bit; 4], out cells: [Bit<u8>; 2] = [0; 2]) { }\n\
module Wide() { n: u32 = 4; signal lanes: [Bit; n]; }\n\
module Gap() { signal none: [Bit<u8>; 0]; signal bl: [Electrical; 0]; }\n\
module Hoist() -> f64 { signal e: [Bit<u8>; 0]; 1.0 }\n\
module Bench() { signal clk: Bit; let osc = Osc(clk) { f: 1G }; let memory = Memory(clk); let ladder = Ladder(); let wide = Wide(); let gap = Gap(); let p = Hoist(); }\n";

/// The last row of the transient of `DESIGN` over the probes `print`.
struct Run;

impl Run {
    /// The table of the transient, 20 ns long, over `print`; an entry that names nothing is the failure.
    fn table(print: &[&str]) -> Result<Table, Failure> {
        Run::session(print)?.run(Rc::new(RefCell::new(Ignore)))
    }

    /// The session of the transient, 20 ns long, over `print`, opened and not run.
    fn session(print: &[&str]) -> Result<Session, Failure> {
        Run::session_of("Bench", print)
    }

    /// [`session`](Self::session) with `top` as the top module.
    fn session_of(top: &str, print: &[&str]) -> Result<Session, Failure> {
        static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
        let cache = CACHE.get_or_init(|| tempfile::tempdir().expect("tmp"));
        let directory = tempfile::tempdir().expect("tmp");
        let file = directory.path().join("arrays.msl");
        std::fs::write(&file, DESIGN).expect("write the model");
        let project = Project::file(&file)?;
        let overrides = Overrides {
            analysis: Some("transient".into()),
            top: Some(top.into()),
            stop: Some(2e-8),
            print_step: Some(5e-9),
            max_step: Some(1e-9),
            print: print.iter().map(|p| p.to_string()).collect(),
            ..Overrides::default()
        };
        let spec = project.request(None, &overrides)?;
        Session::open(&project, &spec, &Settings { rustc: None, cache: cache.path().to_path_buf() })
    }

    /// The last row of `table`.
    fn last(table: &Table) -> &Vec<Cell> {
        table.rows.last().expect("a row")
    }
}

#[test]
fn array_probes_read_an_element_and_the_whole_array() {
    // CV-28 AC 1, AC 2: after the four edges `mem` is `[100, 101, 102, 103, 9, 9]`: `Bench.memory.mem[2]` is the scalar 102,
    // `Bench.memory.mem[5]` an element no edge committed (the initial 9), `Bench.memory.mem` the whole array; the node
    // `Bench.ladder.bl[2]` is one node of the node array
    let table = Run::table(&["Bench.memory.mem[2]", "Bench.memory.mem[5]", "Bench.memory.mem", "Bench.ladder.bl[2]"]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.columns[1..], ["Bench.memory.mem[2]", "Bench.memory.mem[5]", "Bench.memory.mem", "Bench.ladder.bl[2]"].map(String::from), "one column per entry");
    let row = Run::last(&table);
    assert_eq!(row[1], Cell::Int(102), "element 2 holds what edge 2 wrote");
    assert_eq!(row[2], Cell::Int(9), "an element no model commits reads the initial value");
    let Cell::Composite(whole) = &row[3] else { panic!("the whole array is one composite cell: {:?}", row[3]) };
    let leaves: Vec<Cell> = (0..6).map(|k| whole.leaf(&format!("[{k}]")).unwrap_or_else(|| panic!("element {k}"))).collect();
    assert_eq!(leaves, [100, 101, 102, 103, 9, 9].map(Cell::Int), "every element, in order");
    // the ladder is a chain of 1 kOhm resistors from 3 V: node 2 sits at 1 V (a divider of the end shunt)
    match row[4] {
        Cell::Real(volts) => assert!((volts - 1.0).abs() < 1e-9, "`bl[2]` is 3 V * 1 kOhm / 3 kOhm = 1 V: {volts} V"),
        ref other => panic!("a node is a real: {other:?}"),
    }
}

#[test]
fn a_name_that_is_no_element_of_the_array_is_e0801() {
    // §5.1.8, the host's naming: `mem[6]` is past the end of six elements, so no net, node or param has that path
    match Run::table(&["Bench.memory.mem[6]"]) {
        Err(Failure::Host { code, message }) => {
            assert_eq!(code, "E0801", "{message}");
            assert!(message.contains("mem[6]"), "the entry is named: {message}");
        }
        other => panic!("an element past the end is refused: {other:?}"),
    }
}

#[test]
fn a_node_array_prints_one_column_per_node() {
    // CV-28 AC 3: the name of an array of nodes is one column per node, `bl[0]` to `bl[2]`, each the node's potential
    let table = Run::table(&["Bench.ladder.bl"]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.columns[1..], ["Bench.ladder.bl[0]", "Bench.ladder.bl[1]", "Bench.ladder.bl[2]"].map(String::from));
    let row = Run::last(&table);
    assert!(matches!(row[1], Cell::Real(volts) if (volts - 3.0).abs() < 1e-9), "bl[0] is held at 3 V: {:?}", row[1]);
}

#[test]
fn a_set_of_a_param_an_array_length_reads_is_e0803() {
    // CV-36 AC 1 (§8.1.2, §23.1 rule 7): `Wide.n` is the length of `lanes`, so it is structural and a host `set` of it is
    // refused naming it, at any time; a param no length reads (`Osc.f`) is accepted
    let mut session = Run::session(&[]).unwrap_or_else(|f| panic!("{f}"));
    match session.set_at("wide.n", 8.0, 5e-9) {
        Err(Failure::Host { code: "E0803", message }) => assert!(message.contains("wide.n"), "{message}"),
        other => panic!("E0803, found {other:?}"),
    }
    session.set_at("osc.f", 2e9, 5e-9).expect("a param no array length reads is not structural");
}

#[test]
fn the_metadata_lists_the_length_of_an_array_port() {
    // CV-28: a discrete port that is an array of carriers is listed with its length in the type, not as one element
    let session = Run::session_of("Bank", &[]).unwrap_or_else(|f| panic!("{f}"));
    let ports: Vec<(String, &str, String)> = session.metadata().ports.into_iter().map(|p| (p.name, p.direction, p.ty)).collect();
    assert_eq!(ports, [("lanes".to_string(), "in", "[bool; 4]".to_string()), ("cells".to_string(), "out", "[u8; 2]".to_string())]);
}

#[test]
fn a_declared_empty_array_prints_as_the_empty_array() {
    // CV-28 AC 1 (§1.2.3: a length of 0 is legal): `Top.gap.none` of `[Bit<u8>; 0]` has no element, so it prints `[]` at
    // every row, and so does an array of no node; a name the design does not declare stays `E0801`
    let table = Run::table(&["Bench.gap.none", "Bench.gap.bl"]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.columns[1..], ["Bench.gap.none", "Bench.gap.bl"].map(String::from), "one column per entry");
    assert!(table.rows.len() > 1, "the run has rows");
    for row in &table.rows {
        for cell in &row[1..] {
            let Cell::Composite(array) = cell else { panic!("an empty array is a composite cell: {cell:?}") };
            assert_eq!(**array, CompositeCell::Array(Vec::new()), "no element");
        }
    }
    match Run::table(&["Bench.gap.other"]) {
        Err(Failure::Host { code, .. }) => assert_eq!(code, "E0801"),
        other => panic!("a name nothing declares is refused: {other:?}"),
    }
}

#[test]
fn an_empty_array_of_a_hoisted_module_prints_under_the_placement() {
    // CV-28 AC 1 (finding 20): `Hoist` has a result, so its placement `p` joins the body of `Bench` under the prefix
    // `p.`; its empty array `e` is `Bench.p.e` and prints `[]` at every row
    let table = Run::table(&["Bench.p.e"]).unwrap_or_else(|f| panic!("{f}"));
    assert_eq!(table.columns[1..], ["Bench.p.e"].map(String::from), "one column for the entry");
    for row in &table.rows {
        let Cell::Composite(array) = &row[1] else { panic!("an empty array is a composite cell: {:?}", row[1]) };
        assert_eq!(**array, CompositeCell::Array(Vec::new()), "no element");
    }
}
