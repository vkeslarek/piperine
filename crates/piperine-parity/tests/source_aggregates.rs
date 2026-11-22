//! Structs and tuples run from MSL source on the Rust backend (feature `composite-values`, CV-09
//! and CV-10): one test per run-time criterion, each model written in `tests/aggregates/`,
//! elaborated by the front end, compiled on the transpiled backend and run on the solver. Every
//! expected value is computed here by Rust from the rule the test cites, with Rust's own structs
//! and tuples, never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

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
use piperine_solver::protocol::{AnalysisKind, ObservationKind};
use support::{build, transient};

/// One nanosecond in ticks.
const NS: i64 = 1_000_000;

/// A struct as the model `structs.msl` declares it, in Rust.
#[derive(Clone, Copy)]
struct P {
    x: f64,
    y: u8,
}

/// `struct Gains { a: u8, b: f64 }` of the model `params.msl`.
#[derive(Clone, Copy)]
struct Gains {
    a: u8,
    b: f64,
}

/// `struct Line { from: P, to: P }`.
#[derive(Clone, Copy)]
struct Line {
    from: P,
    to: P,
}

/// The lone file `file` of `tests/aggregates/`, as a project.
fn project(file: &str) -> Project {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/aggregates").join(file);
    Project::new(&path)
}

/// A known `f64` carrier value as the plan lays it out: the value, then the known byte.
fn real_bytes(v: f64) -> Vec<u8> {
    let mut bytes = vec![0u8; 16];
    bytes[..8].copy_from_slice(&v.to_le_bytes());
    bytes[8] = 1;
    bytes
}

/// A known one-byte carrier value (`u8`, `bool`).
fn byte(v: u8) -> Vec<u8> {
    vec![v, 1]
}

/// What `Bench` of `file` committed at its first rising edge (2.5 ns of a 200 MHz oscillator): the
/// bytes of each carrier in `names`, which must change exactly once.
fn first_commits(file: &str, names: &[&str]) -> Vec<Vec<u8>> {
    let seen = build(&project(file), "Bench").run(10e-9, 1e-9);
    names
        .iter()
        .map(|name| {
            let changes = transient(&seen, &format!("Bench.{name}"));
            assert_eq!(changes.len(), 1, "{name} changes once: {changes:?}");
            assert_eq!(changes[0].0, 5 * NS / 2, "{name} changes at the first rising edge");
            changes[0].1.clone()
        })
        .collect()
}

/// The commits of `names`, each compared with the bytes Rust gives.
fn assert_commits(file: &str, names: &[&str], wanted: &[Vec<u8>]) {
    let committed = first_commits(file, names);
    for (k, name) in names.iter().enumerate() {
        assert_eq!(committed[k], wanted[k], "{name} is the value Rust gives");
    }
}

#[test]
fn struct_expressions_give_rusts_values() {
    // CV-09 AC 1 (§1.3.3): named fields in any order, shorthand, a tuple struct built by a call, a
    // unit struct by its name, a struct inside a struct.
    let make_p = P { y: 3, x: 1.5 };
    let y: u8 = 7;
    let shorthand = P { y, x: 0.5 };
    let line = Line { from: P { x: 0.0, y: 1 }, to: shorthand };
    #[derive(Clone, Copy)]
    struct Pair(f64, bool);
    let pair = Pair(2.5, true);
    assert_commits(
        "structs.msl",
        &["s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9"],
        &[byte(make_p.y), real_bytes(make_p.x), byte(shorthand.y), real_bytes(shorthand.x), real_bytes(pair.0), byte(u8::from(pair.1)), byte(line.to.y), byte(line.from.y), real_bytes(line.to.x), byte(9)],
    );
}

#[test]
fn field_reads_give_the_fields() {
    // CV-09 AC 3, CV-10 AC 1 (§1.3.4): `s.x` and `t.0` read a field, and a field of a field.
    let t = (1u8, 2.5f64, true);
    let nested = ((1u8, 2u8), 3u8);
    assert_commits("structs.msl", &["t0", "t1", "t2", "t7"], &[byte(t.0), real_bytes(t.1), byte(u8::from(t.2)), byte((nested.0).1 + nested.1)]);
}

#[test]
fn assigning_a_field_changes_that_field_only() {
    // CV-09 AC 4 (§1.5.12, §1.4.6): `q.y = 9` changes `q.y` and nothing else; the value it was copied
    // from is unchanged; a nested field and a compound assignment do the same; a value passed to a
    // function is a copy, so the caller's is unchanged.
    let p = P { x: 1.0, y: 1 };
    let mut q = p;
    q.y = 9;
    let l = Line { from: P { x: 0.0, y: 1 }, to: P { x: 0.0, y: 2 } };
    let mut m = l;
    m.to.y = 5;
    let mut compound = P { x: 0.0, y: 4 };
    compound.y += 3;
    compound.y *= 2;
    let bumped = |p: P| {
        let mut q = p;
        q.y += 1;
        q
    };
    let argument = P { x: 0.0, y: 10 };
    let result = bumped(argument);
    assert_commits(
        "structs.msl",
        &["a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7"],
        &[byte(p.y), byte(q.y), real_bytes(q.x), byte(l.to.y), byte(m.to.y), byte(compound.y), byte(argument.y), byte(result.y)],
    );
}

#[test]
fn tuples_give_rusts_values() {
    // CV-10 AC 1 and 2 (§1.2.4, §1.4.6): a tuple is built, read by index, passed to and returned from a
    // function and copied where it is used.
    let t = (1u8, 2.5f64, true);
    let swap = |t: (u8, u8)| (t.1, t.0);
    let swapped = swap((1, 2));
    let original = (1u8, 2u8);
    let mut copy = original;
    copy.0 = 7;
    assert_commits(
        "structs.msl",
        &["t0", "t1", "t2", "t3", "t4", "t5", "t6"],
        &[byte(t.0), real_bytes(t.1), byte(u8::from(t.2)), byte(swapped.0), byte(swapped.1), byte(original.0), byte(copy.0)],
    );
}

#[test]
fn tuple_patterns_bind_each_component() {
    // CV-10 AC 3 (§1.6.5, §1.7): a tuple, a struct and a tuple-struct pattern bind each field in a `let`, a
    // `match` (with guards and `..`) and an `if let`.
    let swap = |t: (u8, u8)| (t.1, t.0);
    let (a, b) = swap((1, 2));
    let P { x, y } = P { x: 2.0, y: 6 };
    let (ta, tb) = (3.0f64, true);
    let match_tuple = |a: u8, b: u8| match (a, b) {
        (0, n) => n,
        (1, n) if n > 4 => n * 2,
        (_, n) => n + 100,
    };
    let match_struct = |a: u8, b: u8| match (P { x: 0.0, y: b }) {
        P { y: 4, .. } if a == 0 => 1u8,
        P { y, .. } => y + 50,
    };
    let if_let = |a: u8, b: u8| if let (1, n) = (a, b) { n } else { 0 };
    let rest = match (1u8, 2u8, 3u8) {
        (1, .., c) => c,
        _ => 0,
    };
    assert_commits(
        "structs.msl",
        &["d0", "d1", "d2", "d3", "d4", "d5", "d6", "d7", "d8", "d9", "d10"],
        &[
            byte(a * 10 + b),
            byte(y + (x as u8)),
            byte((ta as u8) + if tb { 1 } else { 0 }),
            byte(match_tuple(1, 5)),
            byte(match_tuple(0, 7)),
            byte(match_tuple(2, 9)),
            byte(match_struct(0, 4)),
            byte(match_struct(3, 4)),
            byte(if_let(1, 6)),
            byte(if_let(2, 6)),
            byte(rest),
        ],
    );
}

/// `enum Tx { Idle, Start, Data { n: u3 }, Stop }` of `enums.msl`, in Rust.
#[derive(Clone, Copy, PartialEq)]
enum Tx {
    Idle,
    Start,
    Data { n: u8 },
    Stop,
}

/// `enum Msg { Quit, Move(u8, u8), Say { len: u8 } }`.
#[derive(Clone, Copy)]
enum Msg {
    Quit,
    Move(u8, u8),
    Say { len: u8 },
}

/// `#[repr(u8)] enum Op { Add = 0, Sub = 1, Mul = 0x80 }`.
#[derive(Clone, Copy)]
#[repr(u8)]
enum Op {
    Add = 0,
    Sub = 1,
    Mul = 0x80,
}

#[test]
fn payload_variants_build_and_match() {
    // CV-11 AC 1 (§1.3.5, §1.3.6, §1.6.5): a variant is built by its name, by a call and by a struct
    // expression, matched with the bindings of its fields in a `match` and an `if let`, and two
    // values are equal when they are the same variant with equal fields (H.8 rule 2).
    let size = |t: Tx| match t {
        Tx::Idle => 0u8,
        Tx::Start => 1,
        Tx::Data { n } => n + 10,
        Tx::Stop => 2,
    };
    let build = |k: u8| match k {
        0 => Tx::Idle,
        1 => Tx::Data { n: 5 },
        _ => Tx::Stop,
    };
    let weight = |m: Msg| match m {
        Msg::Quit => 0u8,
        Msg::Move(a, b) => a * 10 + b,
        Msg::Say { len } => len + 100,
    };
    let data = |t: Tx| if let Tx::Data { n } = t { n } else { 99 };
    assert_commits(
        "enums.msl",
        &["v0", "v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8", "v9", "v10", "v11", "o2", "o3"],
        &[
            byte(size(Tx::Idle)),
            byte(size(Tx::Start)),
            byte(size(Tx::Data { n: 5 })),
            byte(size(Tx::Stop)),
            byte(size(build(0))),
            byte(size(build(1))),
            byte(size(build(2))),
            byte(weight(Msg::Quit)),
            byte(weight(Msg::Move(3, 4))),
            byte(weight(Msg::Say { len: 6 })),
            byte(u8::from(build(1) == Tx::Data { n: 5 })),
            byte(u8::from(build(1) == Tx::Data { n: 4 })),
            byte(data(build(1))),
            byte(data(build(0))),
        ],
    );
}

#[test]
fn discriminants_are_the_tag_and_the_cast() {
    // CV-11 AC 2 (§1.3.7, §2 rule 5): `as` of an enum without fields is its discriminant, written or counted
    // from 0 in declaration order, and `#[repr(u8)]` makes the tag eight levels wide.
    #[derive(Clone, Copy)]
    enum Level {
        Low,
        Mid,
        High,
    }
    let op_code = |o: Op| match o {
        Op::Mul => o as u8,
        _ => 0,
    };
    let _ = (Level::Low, Level::Mid);
    assert_commits(
        "enums.msl",
        &["c0", "c1", "c2", "c3", "c4"],
        &[byte(Op::Add as u8), byte(Op::Sub as u8), byte(Op::Mul as u8), byte(op_code(Op::Mul)), byte(Level::High as u8)],
    );
}

#[test]
fn options_of_aggregates_behave_as_enums() {
    // CV-11 AC 5 (§3.2): an `Option` of a struct is matched, tested and unwrapped like any `Option`, and one
    // held as a field of a struct (one value, not a pair) reads back as it was written.
    struct P {
        _x: f64,
        y: u8,
    }
    let pick = |b: bool| if b { Some(P { _x: 0.5, y: 8 }) } else { None };
    let pick_y = |b: bool| match pick(b) {
        Some(P { y, .. }) => y,
        None => 0,
    };
    struct Wrapped {
        inner: Option<P>,
    }
    let wrapped = Wrapped { inner: Some(P { _x: 1.0, y: 3 }) };
    let empty = Wrapped { inner: None };
    assert_commits(
        "enums.msl",
        &["o0", "o1", "o4", "o5"],
        &[byte(pick_y(true)), byte(pick_y(false)), byte(match wrapped.inner { Some(p) => p.y, None => 0 }), byte(u8::from(empty.inner.is_none()))],
    );
}

/// The message of the fault the transient of `top` of `file` runs into: the run is expected to fail.
fn the_fault(file: &str, top: &str) -> String {
    let built = build(&project(file), top);
    let circuit = built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
    let mut options = piperine_solver::prelude::TransientAnalysisOptions::new(10e-9, 1e-9);
    options.dt_max = 1e-9;
    let mut solver = piperine_solver::prelude::Solver::new(circuit).with_context(piperine_solver::prelude::Context::default()).with_tran_opts(options).build();
    match solver.tran().and_then(|mut t| t.solve()) {
        Ok(_) => panic!("the run was expected to fault"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn arrays_give_rusts_values() {
    // CV-13 AC 1 (§1.2.5, §1.5.25): literals, `[x; N]`, indexing by a constant and by a variable, element
    // assignment, nested arrays, an array of structs, and an array passed to a function and returned
    // from it by value, so the caller's is unchanged.
    let a = [1u8, 2, 3];
    let repeated = [7u8; 4];
    let nested = [[1u8, 2], [3, 4]];
    let mut assigned = nested;
    assigned[1][1] = 9;
    let floats = [1.5f64, 2.5];
    let original = [1u8, 2, 3];
    let mut bumped = original;
    bumped[0] += 1;
    let ten = [10u8, 20, 30];
    let mut ps = [P { x: 0.0, y: 4 }; 2];
    ps[1].y = 6;
    assert_commits(
        "arrays.msl",
        &["a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7", "a8", "a9"],
        &[
            byte(a[0] + a[2]),
            byte(repeated[3]),
            byte(nested[1][0]),
            byte(assigned[1][1] + assigned[0][1]),
            byte((floats[0] + floats[1]) as u8),
            byte(original[0]),
            byte(bumped[0]),
            byte(ten[2]),
            byte(ten[0]),
            byte(ps[0].y + ps[1].y),
        ],
    );
    // array patterns: `[x, .., z]`, `[h, rest @ ..]`, literals and `..` in a `match`
    let four = [1u8, 2, 3, 4];
    let [x, .., z] = four;
    let [h, rest @ ..] = a;
    let match_literals = |k: u8| match [k, 5, 6] {
        [1, ..] => 10u8,
        [2, n, _] => n,
        _ => 0,
    };
    assert_commits(
        "arrays.msl",
        &["p0", "p1", "p2", "p3"],
        &[byte(x * 10 + z), byte(h * 100 + rest[0] * 10 + rest[1]), byte(match_literals(1)), byte(match_literals(2))],
    );
}

#[test]
fn a_range_index_gives_the_array_of_its_elements() {
    // CV-13 AC 2 (value semantics, §1.5.25): `a[i..j]` is the array of the elements `i` to `j - 1`, a
    // missing bound is the array's end, `..=` includes the last, and the result is a copy.
    let a = [1u8, 2, 3, 4];
    let mid: [u8; 2] = a[1..3].try_into().expect("two elements");
    let to: [u8; 2] = a[..2].try_into().expect("two elements");
    let inclusive: [u8; 2] = a[1..=2].try_into().expect("two elements");
    let three = [1u8, 2, 3];
    let mut copy: [u8; 2] = three[0..2].try_into().expect("two elements");
    copy[0] = 9;
    assert_commits(
        "arrays.msl",
        &["r0", "r1", "r2", "r3", "r4"],
        &[byte(mid[0] * 10 + mid[1]), byte(to[0] * 10 + to[1]), byte(inclusive[0] * 10 + inclusive[1]), byte(a[1..4][2]), byte(three[0] * 10 + copy[0])],
    );
}

#[test]
fn an_index_out_of_range_panics_at_run_time() {
    // CV-13 AC 3 (§1 rule 11, §1.5.25): `a[5]` over three elements fails the analysis, naming the instance, the
    // instant and the index.
    let fault = the_fault("arrays.msl", "OverrunBench");
    assert!(fault.contains("OverrunBench.overrun"), "the fault names the instance: {fault}");
    assert!(fault.contains('5'), "the fault names the index: {fault}");
    assert!(fault.contains("ns") || fault.contains("e-9") || fault.contains("instant") || fault.contains("time") || fault.contains("t="), "the fault names the instant: {fault}");
}

#[test]
fn an_unknown_index_reads_and_writes_by_merge() {
    // CV-13 AC 1 and US-04 AC 12 (§3.3.13): with `sel` never driven, reading `eq[sel]` over elements that are all
    // 5 gives 5, known; over 5, 5, 6 it is unknown; writing 5 into `w[sel]` merges 5 with each element,
    // which is 5 for the elements that held 5 and unknown for the one that held 6.
    let seen = build(&project("arrays.msl"), "MergeBench").run(10e-9, 1e-9);
    let at_edge = |name: &str| -> Option<Vec<u8>> { transient(&seen, &format!("MergeBench.{name}")).into_iter().next().map(|(_, bytes)| bytes) };
    assert_eq!(at_edge("same"), Some(byte(5)), "the elements all hold 5");
    assert_eq!(at_edge("differ"), Some(vec![0, 0]), "5, 5, 6 merge to unknown");
    assert_eq!(at_edge("w0"), Some(byte(5)), "merge(5, 5) is 5");
    assert_eq!(at_edge("w1"), Some(vec![0, 0]), "merge(6, 5) is unknown");
}

/// The enum `Level` of `compare.msl`, in Rust with the same derives.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Level {
    /// `Low`.
    Low,
    /// `Mid(u8)`.
    Mid(u8),
    /// `High { a, b }`.
    High {
        /// First field.
        a: u8,
        /// Second field.
        b: u8,
    },
}

/// `struct Wrap { inner: Option<u8>, tag: Level }` with the same derives.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
struct Wrap {
    /// The optional field.
    inner: Option<u8>,
    /// The enum field.
    tag: Level,
}

/// `struct Ordered { x: f64, y: u8 }` as `compare.msl` derives it.
#[derive(Clone, Copy, PartialEq, PartialOrd, Default)]
struct Ordered {
    /// First field.
    x: f64,
    /// Second field.
    y: u8,
}

#[test]
fn derived_comparisons_and_defaults_give_rusts_results() {
    // CV-12 AC 1 (H.8 rules 2 to 4, §1.5.23): derived `==`, `!=`, `<`, `<=`, `>`, `>=` on structs,
    // tuples, arrays, enums (discriminant first, then fields) and `Option`s (`None` lowest), and
    // `T::default()`, each the result Rust's derives give.
    let flag = |b: bool| byte(u8::from(b));
    let p = |x: f64, y: u8| Ordered { x, y };
    let mid = Level::Mid;
    let high = |a: u8, b: u8| Level::High { a, b };
    let none: Option<u8> = None;
    let wanted = vec![
        flag(p(1.5, 2) == p(1.5, 2)),
        flag(p(1.5, 2) != p(1.5, 3)),
        flag(p(1.0, 9) < p(2.0, 0)),
        flag(p(1.0, 1) < p(1.0, 2)),
        flag(p(1.0, 1) <= p(1.0, 1)),
        flag(p(2.0, 0) > p(1.0, 9)),
        flag(p(1.0, 1) >= p(1.0, 2)),
        flag((1u8, 2.5, true) == (1u8, 2.5, true)),
        flag((1u8, 2.5, true) != (1u8, 2.5, false)),
        flag((1u8, 9u8) < (2u8, 0u8)),
        flag((3u8, 1u8) >= (3u8, 2u8)),
        flag([1u8, 2, 3] == [1u8, 2, 3]),
        flag([1u8, 2, 3] != [1u8, 2, 4]),
        flag([1u8, 2, 3] < [1u8, 3, 0]),
        flag([1u8, 2, 3] > [1u8, 3, 0]),
        flag(mid(3) == mid(3)),
        flag(mid(3) != high(3, 0)),
        flag(Level::Low < mid(0)),
        flag(mid(200) < high(0, 0)),
        flag(high(1, 9) < high(2, 0)),
        flag(mid(5) > mid(4)),
        flag(none < Some(0u8)),
        flag(Some(3u8) < Some(4u8)),
        flag(Some(3u8) == Some(3u8)),
        flag(none == None),
        flag(Some(3u8) != none),
        flag(Wrap { inner: Some(1), tag: mid(2) } < Wrap { inner: Some(1), tag: mid(3) }),
        flag(Wrap { inner: None, tag: Level::Low } == Wrap { inner: None, tag: Level::Low }),
        byte(Ordered::default().y),
        flag(Ordered::default().x == 0.0),
        flag(Ordered::default() == p(0.0, 0)),
        flag(Option::<u8>::default() == none),
    ];
    let names = [
        "r0", "r1", "r2", "r3", "r4", "r5", "r6", "t0", "t1", "t2", "t3", "a0", "a1", "a2", "a3", "e0", "e1", "e2", "e3", "e4", "e5", "o0", "o1", "o2", "o3", "o4", "n0", "n1", "d0", "d1", "d2", "d3",
    ];
    assert_commits("compare.msl", &names, &wanted);
}

/// The rows of `iterators.msl`, each computed by Rust from the same text, and the names they commit.
struct IteratorRows;

impl IteratorRows {
    /// The `(name, bytes Rust gives)` of every row, in the order the model commits them.
    fn wanted() -> Vec<(&'static str, Vec<u8>)> {
        let flag = |b: bool| byte(u8::from(b));
        vec![
            ("len", byte({ let a = [3u8, 1, 4, 1, 5]; a.len() as u8 })),
            ("is_empty", flag({ let a = [3u8, 1, 4, 1, 5]; a.is_empty() })),
            ("contains_hit", flag({ let a = [3u8, 1, 4, 1, 5]; a.contains(&4) })),
            ("contains_miss", flag({ let a = [3u8, 1, 4, 1, 5]; a.contains(&9) })),
            ("first", byte({ let a = [3u8, 1, 4, 1, 5]; *a.first().unwrap_or(&0) })),
            ("last", byte({ let a = [3u8, 1, 4, 1, 5]; *a.last().unwrap_or(&0) })),
            ("get_in", byte({ let a = [3u8, 1, 4, 1, 5]; *a.get(2).unwrap_or(&99) })),
            ("get_out", byte({ let a = [3u8, 1, 4, 1, 5]; *a.get(7).unwrap_or(&99) })),
            ("get_end_is_none", flag({ let a = [3u8, 1, 4, 1, 5]; a.get(5).is_none() && a.len() == 5 })),
            ("range_in", flag((2..5).contains(&4))),
            ("range_at_end", flag((2..5).contains(&5))),
            ("range_inclusive", flag((2..=5).contains(&5))),
            ("range_below", flag((2..5).contains(&1))),
            ("range_float", flag((0.5..1.5).contains(&1.0))),
            ("range_empty", flag((5..2).is_empty())),
            ("range_single", flag((2..=2).is_empty())),
            ("range_named", flag({ let r = 1..4; r.contains(&3) })),
            ("range_to", flag((..3).contains(&2))),
            ("range_from", flag((2..).contains(&1))),
            ("map_sum", byte({ let a = [3u8, 1, 4, 1, 5]; a.iter().copied().map(|x| x * 2).sum::<u8>() })),
            ("filter_count", byte({ let a = [3u8, 1, 4, 1, 5]; a.iter().copied().filter(|x| *x > 1).count() as u8 })),
            ("fold", byte({ let a = [3u8, 1, 4, 1, 5]; a.iter().copied().fold(0u8, |acc, x| acc * 2 + x) })),
            ("sum", byte({ let a = [3u8, 1, 4, 1, 5]; a.iter().copied().sum::<u8>() })),
            ("count", byte({ let a = [3u8, 1, 4, 1, 5]; a.iter().count() as u8 })),
            ("zip", byte({ let a = [3u8, 1, 4, 1, 5]; let b = [2u8, 0, 1, 3, 1]; a.iter().copied().zip(b.iter().copied()).map(|(x, y)| x * y).sum::<u8>() })),
            ("zip_shorter", byte({ let a = [3u8, 1, 4, 1, 5]; a.iter().zip([7u8, 8].iter()).count() as u8 })),
            ("enumerate", byte({ let a = [3u8, 1, 4, 1, 5]; a.iter().copied().enumerate().fold(0u8, |acc, (i, x)| acc + (i as u8) * x) })),
            ("rev", byte({ let a = [3u8, 1, 4, 1, 5]; a.iter().copied().rev().fold(0u8, |acc, x| acc * 2 + x) })),
            ("enumerate_then_rev", byte({ let a = [3u8, 1, 4, 1, 5]; a.iter().copied().enumerate().rev().fold(0u8, |acc, (i, x)| acc * 2 + (i as u8) + x) })),
            ("rev_then_enumerate", byte({ let a = [3u8, 1, 4, 1, 5]; a.iter().copied().rev().enumerate().fold(0u8, |acc, (i, x)| acc * 2 + (i as u8) + x) })),
            ("any_hit", flag({ let a = [3u8, 1, 4, 1, 5]; a.iter().copied().any(|x| x == 4) })),
            ("any_miss", flag({ let a = [3u8, 1, 4, 1, 5]; a.iter().copied().any(|x| x > 9) })),
            ("all_hit", flag({ let a = [3u8, 1, 4, 1, 5]; a.iter().copied().all(|x| x > 0) })),
            ("all_miss", flag({ let a = [3u8, 1, 4, 1, 5]; a.iter().copied().all(|x| x > 1) })),
            ("empty_any", flag({ let e: [u8; 0] = []; e.iter().copied().any(|x| x > 0) })),
            ("empty_all", flag({ let e: [u8; 0] = []; e.iter().copied().all(|x| x > 0) })),
            ("empty_sum", byte({ let e: [u8; 0] = []; e.iter().copied().sum::<u8>() })),
            ("empty_fold", byte({ let e: [u8; 0] = []; e.iter().copied().fold(7u8, |acc, x| acc + x) })),
            ("empty_len", byte({ let e: [u8; 0] = []; e.len() as u8 })),
            ("empty_first_is_none", flag({ let e: [u8; 0] = []; e.first().is_none() && e.is_empty() })),
            ("empty_is_empty", flag({ let e: [u8; 0] = []; e.is_empty() })),
            ("filter_enumerate", byte({ let a = [3u8, 1, 4, 1, 5]; a.iter().copied().filter(|x| *x != 1).enumerate().fold(0u8, |acc, (i, x)| acc * 2 + (i as u8) * 3 + x) })),
            ("chain", byte({ let a = [3u8, 1, 4, 1, 5]; a.iter().copied().map(|x| x * 2).filter(|x| *x > 4).map(|x| x + 1).sum::<u8>() })),
            ("range_map_sum", byte((1u8..5).map(|k| k * k).sum::<u8>())),
            ("range_rev", byte((1u8..5).rev().fold(0u8, |acc, k| acc * 2 + k))),
            ("range_filter_count", byte((0u8..10).filter(|k| k % 3 == 0).count() as u8)),
            ("range_inclusive_sum", byte((1u8..=4).sum::<u8>())),
            ("range_empty_sum", byte((5u8..2).sum::<u8>())),
            ("range_zip", byte({ let a = [3u8, 1, 4, 1, 5]; (0u8..3).zip(a.iter().copied()).map(|(i, x)| i * x).sum::<u8>() })),
            ("let_iterator", byte({ let a = [3u8, 1, 4, 1, 5]; let it = a.iter().copied().map(|x| x + 1); it.sum::<u8>() })),
            ("signed_range_sum", byte({ let s: i8 = (-3i8..2).sum(); s as u8 })),
            ("signed_range_count", byte((-3i8..2).count() as u8)),
            ("for_array", byte({ let a = [3u8, 1, 4, 1, 5]; let mut acc = 0u8; for x in a { acc = acc * 2 + x; } acc })),
            ("for_iter_rev", byte({ let a = [3u8, 1, 4, 1, 5]; let mut acc = 0u8; for x in a.iter().copied().rev() { acc = acc * 2 + x; } acc })),
            ("for_enumerate", byte({ let a = [3u8, 1, 4, 1, 5]; let mut acc = 0u8; for (i, x) in a.iter().copied().enumerate() { acc = acc * 2 + (i as u8) + x; } acc })),
            ("for_zip", byte({ let a = [3u8, 1, 4, 1, 5]; let mut acc = 0u8; for (x, y) in a.iter().copied().zip([1u8, 2, 3].iter().copied()) { acc = acc * 2 + x + y; } acc })),
            ("for_filtered_range", byte({ let mut acc = 0u8; for k in (0u8..8).filter(|k| k % 2 == 0) { acc = acc * 2 + k; } acc })),
            ("for_range_rev", byte({ let mut acc = 0u8; for k in (0u8..4).rev() { acc = acc * 2 + k; } acc })),
            ("for_break", byte({ let a = [3u8, 1, 4, 1, 5]; let mut acc = 0u8; for x in a { if x == 4 { break; } acc = acc * 2 + x; } acc })),
            ("for_continue", byte({ let a = [3u8, 1, 4, 1, 5]; let mut acc = 0u8; for x in a.iter().copied().filter(|x| *x != 5) { if x == 1 { continue; } acc = acc * 2 + x; } acc })),
            ("for_named", byte({ let a = [3u8, 1, 4, 1, 5]; let it = a.iter().copied().map(|x| x * 3); let mut acc = 0u8; for x in it { acc = acc + x; } acc })),
            ("zi_nd", real_bytes(Self::zi_acc([0.5, 0.25, 0.125], [1.5, -2.0], [0.75, -0.5], [0.3, 0.7], 2.0))),
        ]
    }

    /// `zi_nd`'s reaction body value code, in the order the stdlib writes it.
    fn zi_acc(num: [f64; 3], xs: [f64; 2], den: [f64; 2], ys: [f64; 2], x: f64) -> f64 {
        let mut acc = num[0] * x;
        for k in 1..3 {
            acc += num[k] * xs[k - 1];
        }
        for k in 0..2 {
            acc -= den[k] * ys[k];
        }
        acc
    }
}

#[test]
fn every_row_of_h5_to_h7_gives_rusts_result() {
    // CV-15 AC 1 (H.5 rules 1 to 5, H.6 rules 1 to 3, H.7 rules 4 to 10): each row of the three tables, on
    // arrays, ranges and adaptor chains, gives the result Rust's own array, range and iterator give;
    // `get` past the end is `None`. The `for` rows are in `for_visits_elements_in_rusts_order`.
    let rows: Vec<_> = IteratorRows::wanted().into_iter().filter(|(name, _)| !name.starts_with("for_") && !name.starts_with("empty_") && *name != "zi_nd").collect();
    let names: Vec<&str> = rows.iter().map(|(name, _)| *name).collect();
    let wanted: Vec<Vec<u8>> = rows.into_iter().map(|(_, bytes)| bytes).collect();
    assert_commits("iterators.msl", &names, &wanted);
}

#[test]
fn for_visits_elements_in_rusts_order() {
    // CV-15 AC 2 (§1.8.9, H.7 rule 4): a `for` over an array, a range or an adaptor chain visits the elements
    // in the order Rust's `for` does, `break` and `continue` leaving as they do.
    let rows: Vec<_> = IteratorRows::wanted().into_iter().filter(|(name, _)| name.starts_with("for_")).collect();
    let names: Vec<&str> = rows.iter().map(|(name, _)| *name).collect();
    let wanted: Vec<Vec<u8>> = rows.into_iter().map(|(_, bytes)| bytes).collect();
    assert_commits("iterators.msl", &names, &wanted);
}

#[test]
fn an_empty_array_has_length_zero_and_no_first() {
    // Edge case (H.5 rules 1 and 4, H.7 rules 8 to 10): an array of no element has `len()` 0 and `first()`
    // `None`; `sum` of it is 0, `fold` gives `init`, `any` is false and `all` is true.
    let rows: Vec<_> = IteratorRows::wanted().into_iter().filter(|(name, _)| name.starts_with("empty_")).collect();
    let names: Vec<&str> = rows.iter().map(|(name, _)| *name).collect();
    let wanted: Vec<Vec<u8>> = rows.into_iter().map(|(_, bytes)| bytes).collect();
    assert_commits("iterators.msl", &names, &wanted);
}

#[test]
fn the_zi_nd_reaction_value_code_runs_as_a_function() {
    // Phase A Independent Test (D18): the value code of the `zi_nd` reaction body, `acc` over `num`, `xs`,
    // `den` and `ys`, runs as a monomorphic `fn` over arrays with the value Rust gives for the same loops.
    let zi = IteratorRows::wanted().into_iter().filter(|(name, _)| *name == "zi_nd").collect::<Vec<_>>();
    assert_commits("iterators.msl", &["zi_nd"], &[zi[0].1.clone()]);
}

/// A known `u64` carrier value as the plan lays it out: the value, then the known byte.
fn u64_bytes(v: u64) -> Vec<u8> {
    let mut bytes = vec![0u8; 16];
    bytes[..8].copy_from_slice(&v.to_le_bytes());
    bytes[8] = 1;
    bytes
}

/// §22.2.17's generator, written as the spec writes it and compiled by Rust.
struct SpecGenerator;

impl SpecGenerator {
    /// One step of SplitMix64: a uniform real in [0, 1) and the next state.
    fn uniform(rng: u64) -> (f64, u64) {
        let s = rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let z = (s ^ (s >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        let z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        let z = z ^ (z >> 31);
        ((z >> 11) as f64 / 9_007_199_254_740_992.0, s)
    }

    /// `normal`: two uniform draws and Box-Muller; the sample and the next state.
    fn normal(rng: u64, mean: f64, sigma: f64) -> (f64, u64) {
        let (a, s1) = Self::uniform(rng);
        let (b, s2) = Self::uniform(s1);
        let z = (-2.0 * (1.0 - a).ln()).sqrt() * (2.0 * std::f64::consts::PI * b).cos();
        (mean + sigma * z, s2)
    }
}

#[test]
fn normal_matches_the_spec_text_bit_for_bit() {
    // CV-16 AC 1 (§22.2.17 rules 1 and 2): the sample and the next state are those of the spec's SplitMix64 and
    // Box-Muller text compiled by Rust, bit for bit.
    let spec = |seed: u64| SpecGenerator::normal(seed, 1.5, 0.25);
    assert_commits(
        "normal.msl",
        &["s0", "s1", "s2", "n0", "n1"],
        &[real_bytes(spec(0).0), real_bytes(spec(42).0), real_bytes(spec(0xDEAD_BEEF_CAFE_F00D).0), u64_bytes(spec(0).1), u64_bytes(spec(42).1)],
    );
}

#[test]
fn normal_with_zero_sigma_returns_the_mean() {
    // CV-16 AC 2 (§22.2.17 rule 3): with `sigma == 0.0` the sample is `mean`, whatever the seed.
    assert_commits("normal.msl", &["f0", "f1"], &[real_bytes(3.0), real_bytes(3.0)]);
}

#[test]
fn the_jitter_clock_repeats_with_its_seed() {
    // CV-16 AC 3 (§22.2.17 rule 4): the clock whose edges the state of `normal` places gives the same edge times on
    // every run with one seed, and the edges are not on a regular grid (the jitter is there).
    let edges = |top: &str| transient(&build(&project("normal.msl"), top).run(200e-9, 1e-9), &format!("{top}.clk")).into_iter().map(|(tick, _)| tick).collect::<Vec<i64>>();
    let first = edges("JitterBench");
    let second = edges("JitterBench");
    assert!(first.len() > 10, "the clock runs: {first:?}");
    assert_eq!(first, second, "one seed, one sequence of edge times");
    let gaps: Vec<i64> = first.windows(2).map(|pair| pair[1] - pair[0]).collect();
    assert!(gaps.iter().any(|gap| *gap != gaps[0]), "the jitter moves the edges: {gaps:?}");
    // the same sequence Rust gives: each edge is placed half a period plus one draw after the last, each
    // delay rounded to the nearest tick (a femtosecond) when it is scheduled
    let (mut state, mut now, mut wanted) = (7u64, 0i64, Vec::new());
    for _ in 0..first.len() {
        let (jitter, next) = SpecGenerator::normal(state, 0.0, 1e-9);
        state = next;
        now += ((10e-9 / 2.0 + jitter) / 1e-15).round() as i64;
        wanted.push(now);
    }
    assert_eq!(first, wanted, "the edges are the spec's draws placed one after the other");
}

/// The carriers of the model `carriers.msl`, as the plan lays a composite pin out (`CompositeLayout`):
/// the value bytes of the parts one after another, the first lowest, then the known byte.
struct Carriers {
    seen: Seen,
}

impl Carriers {
    /// `Bench` of `carriers.msl`, run for two rising edges (2.5 ns and 7.5 ns).
    fn run() -> Carriers {
        Carriers { seen: build(&project("carriers.msl"), "Bench").run(10e-9, 1e-9) }
    }

    /// Every change of the carrier `name` during the transient: instant in ticks and bytes.
    fn changes(&self, name: &str) -> Vec<(i64, Vec<u8>)> {
        transient(&self.seen, &format!("Bench.{name}"))
    }

    /// The only change of `name`, which must be at the first rising edge.
    fn once(&self, name: &str) -> Vec<u8> {
        let changes = self.changes(name);
        assert_eq!(changes.len(), 1, "{name} changes once: {changes:?}");
        assert_eq!(changes[0].0, 5 * NS / 2, "{name} changes at the first rising edge");
        changes[0].1.clone()
    }

    /// `value`, the bytes of a known composite, as its pin holds it: the known byte after them.
    fn known(value: &[u8]) -> Vec<u8> {
        let mut bytes = value.to_vec();
        bytes.push(1);
        bytes
    }
}

/// A value whose levels §2 lays out, so a test computes what `T::WIDTH` and `level(k)` give.
struct Levels {
    bits: Vec<bool>,
}

impl Levels {
    /// The levels of an unsigned integer of `width` levels, level 0 the least significant bit.
    fn unsigned(value: u64, width: u32) -> Levels {
        Levels { bits: (0..width).map(|k| value >> k & 1 == 1).collect() }
    }

    /// `self` then `next`: the fields of a struct, first field lowest (§2 rule 2).
    fn then(mut self, next: Levels) -> Levels {
        self.bits.extend(next.bits);
        self
    }

    /// An enum value of `variant` of `variants`, its payload padded with false levels to `widest` (§2 rules 5 and 6).
    fn tagged(variant: u64, variants: u64, payload: Levels, widest: u32) -> Levels {
        let tag_width = u64::BITS - variants.saturating_sub(1).leading_zeros();
        let mut padded = payload;
        padded.bits.resize(widest as usize, false);
        Levels::unsigned(variant, tag_width).then(padded)
    }
}

#[test]
fn derived_repr_layout_and_width_follow_section_2() {
    // CV-17 AC 1 (§2 rules 2, 5 and 6): a struct is its fields' levels, first field lowest; an
    // enum a tag of ceil(log2 V) levels, then the widest payload; `T::WIDTH` is that width and
    // `level(k)` reads level k.
    let pair = Levels::unsigned(8, 8).then(Levels::unsigned(0, 1));
    let tuple_field = Levels::unsigned(0, 8).then(Levels::unsigned(0, 1));
    let array_field = Levels::unsigned(0, 24);
    let option_field = Levels::unsigned(0, 9);
    let wrap = tuple_field.then(array_field).then(option_field);
    let data = Levels::tagged(1, 3, Levels::unsigned(5, 3), 3);
    let carriers = Carriers::run();
    assert_eq!(carriers.once("w_pair"), vec![pair.bits.len() as u8, 1], "Pair is u8 then bool: 9 levels");
    assert_eq!(carriers.once("w_enum"), vec![data.bits.len() as u8, 1], "Tx is a 2-level tag then 3 levels of payload");
    assert_eq!(carriers.once("w_wrap"), vec![wrap.bits.len() as u8, 1], "Wrap is (u8, bool), [u8; 3] and Option<u8>: 9 + 24 + 9");
    // the levels of `Tx::Data { n: 5 }`, read one by one
    let read: Vec<u8> = carriers.once("r_levels");
    assert_eq!(read, Carriers::known(&data.bits.iter().map(|b| u8::from(*b)).collect::<Vec<_>>()), "level(k) of the enum value is level k of §2's layout");
    // the levels of a struct: level 0 of `Pair { a: 7, b: true }` is the least significant bit of `a`, level 8 is `b`
    let seven = Levels::unsigned(7, 8).then(Levels::unsigned(1, 1));
    assert_eq!(carriers.once("level_a0"), vec![u8::from(seven.bits[0]), 1]);
    assert_eq!(carriers.once("level_b"), vec![u8::from(seven.bits[8]), 1]);
    // the bytes a pin holds: the parts' values in field order (the tag first), then the known byte
    assert_eq!(carriers.once("s"), Carriers::known(&[7, 1]));
    assert_eq!(carriers.once("tx"), Carriers::known(&[1, 5]), "the tag of Data, then n");
}

#[test]
fn composite_carriers_initialize_define_commit_and_change() {
    // CV-17 AC 3 (§2, §12.1): an initial value, a definition, a commit of each composite shape, a
    // read of the whole value and of its parts, `changed()` and `prev()` over the whole value.
    let carriers = Carriers::run();
    let at_start = |name: &str| carriers.seen.series_of(&format!("Bench.{name}"))[0].clone();
    assert_eq!(at_start("first"), (0, Carriers::known(&[3, 1])), "the initial value Pair {{ a: 3, b: true }}");
    assert_eq!(at_start("defined"), (0, Carriers::known(&[1, 1])), "the definition Pair {{ a: 1, b: true }}");
    assert_eq!(carriers.once("s"), Carriers::known(&[7, 1]));
    assert_eq!(carriers.once("t"), Carriers::known(&[9, 0]));
    assert_eq!(carriers.once("arr"), Carriers::known(&[1, 2, 3]));
    assert_eq!(carriers.once("tx"), Carriers::known(&[1, 5]));
    assert_eq!(carriers.once("opt"), Carriers::known(&[1, 4]), "Some(4): the tag byte, then the payload");
    // a part of each, read where the carrier changed
    assert_eq!(carriers.once("r_t"), byte(9));
    assert_eq!(carriers.once("r_arr"), byte(2));
    assert_eq!(carriers.once("r_opt"), byte(4));
    assert_eq!(carriers.once("r_tx"), byte(5));
    // `changed()` fires for each change of the whole value, and `prev()` is the value before the change
    let rise = |k: i64| 5 * NS / 2 + 5 * NS * k;
    assert_eq!(carriers.changes("u"), vec![(rise(0), Carriers::known(&[7, 0])), (rise(1), Carriers::known(&[8, 0]))]);
    assert_eq!(carriers.changes("now_a"), vec![(rise(0), byte(7)), (rise(1), byte(8))], "u.a where u changed");
    // §5.1.8: before the first change `u.prev()` is unknown, so the model reads no field of it (a plain read of it is
    // refused by name, `an_unknown_composite_is_refused_by_name`) and `u.prev().a` is 7 only at the second change
    assert_eq!(carriers.changes("before_known"), vec![(rise(0), byte(0)), (rise(1), byte(1))], "u.prev().is_known(): unknown before the first change");
    assert_eq!(carriers.changes("before_a"), vec![(rise(1), byte(7))], "u.prev().a: nothing before the first change, then 7");
}

#[test]
fn an_unknown_composite_is_refused_by_name() {
    // §3.3.2, §5.1.1: a composite carrier nothing drove holds no value, so a field of it is no known
    // value; it fails the run naming the instance and the carrier, never a default.
    let fault = the_fault("carriers.msl", "UnknownBench");
    assert!(fault.contains("UnknownBench.read"), "the fault names the instance: {fault}");
    assert!(fault.contains("`un`"), "the fault names the carrier: {fault}");
    assert!(fault.contains("holds no value"), "the fault says why: {fault}");
}

#[test]
fn committing_an_equal_composite_fires_no_change() {
    // CV-17 AC 4 (§12.1): `s` is committed the same `Pair` at both rising edges; the second commit
    // changes nothing, so the net has one change and `s.changed()` fired once.
    let carriers = Carriers::run();
    assert_eq!(carriers.changes("s"), vec![(5 * NS / 2, Carriers::known(&[7, 1]))], "one change for two commits of one value");
    assert_eq!(carriers.once("s_changes"), byte(1), "`changed()` fired once");
}

#[test]
fn a_tag_naming_no_variant_reads_unknown() {
    // CV-17 AC 5 (§2 rule 9): `Tx` has the tags 0, 1 and 2. `cast::<u5, Tx>(13)` has tag 1 and n = 3,
    // a value; `cast::<u5, Tx>(3)` has tag 3, which names no variant, so it is unknown: the net never
    // holds a value and `is_known()` reads false.
    let carriers = Carriers::run();
    assert_eq!(carriers.once("good"), Carriers::known(&[1, 3]), "tag 1, n = 3");
    assert_eq!(carriers.changes("bad"), vec![], "an unknown value commits nothing the net had not");
    assert_eq!(carriers.once("good_known"), byte(1));
    assert_eq!(carriers.once("bad_known"), byte(0), "a tag naming no variant reads unknown");
}

#[test]
fn composite_params_bind_and_read_everywhere() {
    // CV-18 AC 1 (§8.1): a struct, a tuple and an array param bind their defaults or a placement's
    // value, and every block reads them: a digital reaction (fields, elements, a real field
    // converted), an `analog` block (a real field times Rust's constant) and the `where`.
    let built = build(&project("params.msl"), "Bench");
    let seen = built.run(10e-9, 1e-9);
    let at_edge = |name: &str| transient(&seen, &format!("Bench.{name}"))[0].1.clone();
    // the defaults: g = Gains { a: 3, b: 0.5 }, t = (4, 5), arr = [1, 2, 3]
    let defaults = (Gains { a: 3, b: 0.5 }, (4u8, 5u8), [1u8, 2, 3]);
    assert_eq!(at_edge("dq"), byte(defaults.0.a), "g.a");
    assert_eq!(at_edge("dr"), byte(defaults.1.1), "t.1");
    assert_eq!(at_edge("ds"), byte(defaults.2[2]), "arr[2]");
    assert_eq!(at_edge("dw"), byte((defaults.0.b * 10.0) as u8), "g.b converted");
    // the placement binds g, t and arr; `g.b` is over the parent's `scale = 0.5`
    let scale = 0.5f64;
    let bound = (Gains { a: 9, b: scale * 1.5 }, (7u8, 8u8), [4u8, 5, 6]);
    assert_eq!(at_edge("pq"), byte(bound.0.a));
    assert_eq!(at_edge("pr"), byte(bound.1.1));
    assert_eq!(at_edge("ps"), byte(bound.2[2]));
    assert_eq!(at_edge("pw"), byte((bound.0.b * 10.0) as u8));
    // the analog block: a source held at `g.b * 4.0`, by default and bound
    let (_, result) = built.run_with(10e-9, 1e-9);
    let volts = |name: &str| {
        let id = piperine_solver::prelude::NodeIdentifier::Anonymous(built.ir.node_at(&format!("Bench.{name}")).expect("a node").id().index());
        result.iter().last().and_then(|step| step.get_node(&id)).expect("a value")
    };
    assert!((volts("by_default") - 0.25 * 4.0).abs() < 1e-9, "the default b = 0.25");
    assert!((volts("by_placement") - 0.5 * 4.0).abs() < 1e-9, "the bound b = 0.5");
    // the `where` is checked on the value a placement binds (E0317)
    let refused = project("params_where.msl").elaborate("Bench").err().expect("a false where is refused");
    assert!(refused.to_string().contains("E0317"), "{refused}");
}

/// A sink that keeps the bytes of every record of the transient.
struct RecordBytes {
    out: std::rc::Rc<std::cell::RefCell<Vec<Vec<u8>>>>,
}

impl piperine_solver::protocol::Sink for RecordBytes {
    fn observe(&mut self, o: &piperine_solver::protocol::Observation) {
        if let (AnalysisKind::Transient, ObservationKind::Record { bytes, .. }) = (&o.analysis, &o.kind) {
            self.out.borrow_mut().push(bytes.clone());
        }
    }
}

#[test]
fn a_record_carries_a_composite_field_and_its_type() {
    // CV-19 AC 1 (§19, rule 13): `emit!` writes a struct, an array, a tuple and a payload enum as
    // fields of the stream `sample`; the record is delivered with each field's bytes laid out as a
    // composite pin (the parts' values, then the known byte) at the offset the layout gives, and the
    // stream's metadata names each field's type as the source writes it.
    let built = build(&project("records.msl"), "Bench");
    let out = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let circuit = built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
    let mut options = piperine_solver::prelude::TransientAnalysisOptions::new(10e-9, 1e-9);
    options.dt_max = 1e-9;
    let sink = RecordBytes { out: out.clone() };
    let mut solver = piperine_solver::prelude::Solver::new(circuit).with_tran_opts(options).with_sink(Box::new(sink)).build();
    solver.tran().unwrap_or_else(|e| panic!("{e}")).solve().unwrap_or_else(|e| panic!("{e}"));
    let records = out.borrow().clone();
    assert_eq!(records.len(), 2, "one record per rising edge: {records:?}");
    // the fields in order: t (f64, 16), pair (u8, f64 + known), arr (3 u8 + known), tup (u8, bool + known),
    // tx (tag 1 = Data, n = 5 + known), then `maybe` (bool + known): each at the next offset its alignment allows
    let mut wanted: Vec<u8> = Vec::new();
    let pad = |bytes: &mut Vec<u8>, align: usize| bytes.resize(bytes.len().next_multiple_of(align), 0);
    // `t` is the time of the edge: read it back from the record rather than compute it
    let time = &records[0][..8];
    wanted.extend_from_slice(time);
    wanted.push(1);
    // an `f64` field is the 16 bytes of `REAL`: the value, the known byte, padding to its alignment
    pad(&mut wanted, 16);
    wanted.push(7);
    wanted.extend_from_slice(&0.5f64.to_le_bytes());
    wanted.push(1);
    wanted.extend_from_slice(&[1, 2, 3, 1]);
    wanted.extend_from_slice(&[9, 1, 1]);
    wanted.extend_from_slice(&[1, 5, 1]);
    wanted.extend_from_slice(&[0, 1]);
    pad(&mut wanted, 8);
    assert_eq!(records[0], wanted, "the record of the first edge");
    assert_eq!(f64::from_le_bytes(time.try_into().expect("eight bytes")), 2.5e-9, "t = now() at the first rising edge");
    let body = built.ir.instance_at("Bench.logger").expect("the logger").body();
    let types: Vec<String> = body.streams()[0].fields.iter().map(|f| built.ir.written(&f.ty)).collect();
    assert_eq!(types, ["f64", "Pair", "[u8; 3]", "(u8, bool)", "Tx"], "the types as the source writes them");
}

#[test]
fn two_sites_giving_a_field_different_composite_types_are_refused() {
    // CV-19 AC 2 (§19 rule 2): the field `v` of `sample` is a `Pair` at one site and a `(u8, u8)` at
    // another, which is E0605
    let refused = project("records_conflict.msl").elaborate("Bench").err().expect("the conflict is refused");
    assert!(refused.to_string().contains("E0605"), "{refused}");
}
