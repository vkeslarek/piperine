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

/// What `Bench` of `structs.msl` committed at its first rising edge (2.5 ns of a 200 MHz oscillator): the
/// bytes of each carrier in `names`, which must change exactly once.
fn first_commits(names: &[&str]) -> Vec<Vec<u8>> {
    let seen = build(&project("structs.msl"), "Bench").run(10e-9, 1e-9);
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
fn assert_commits(names: &[&str], wanted: &[Vec<u8>]) {
    let committed = first_commits(names);
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
        &["s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9"],
        &[byte(make_p.y), real_bytes(make_p.x), byte(shorthand.y), real_bytes(shorthand.x), real_bytes(pair.0), byte(u8::from(pair.1)), byte(line.to.y), byte(line.from.y), real_bytes(line.to.x), byte(9)],
    );
}

#[test]
fn field_reads_give_the_fields() {
    // CV-09 AC 3, CV-10 AC 1 (§1.3.4): `s.x` and `t.0` read a field, and a field of a field.
    let t = (1u8, 2.5f64, true);
    let nested = ((1u8, 2u8), 3u8);
    assert_commits(&["t0", "t1", "t2", "t7"], &[byte(t.0), real_bytes(t.1), byte(u8::from(t.2)), byte((nested.0).1 + nested.1)]);
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
