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

use support::{build, transient};

/// One nanosecond in ticks.
const NS: i64 = 1_000_000;

/// A struct as the model `structs.msl` declares it, in Rust.
#[derive(Clone, Copy)]
struct P {
    x: f64,
    y: u8,
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
