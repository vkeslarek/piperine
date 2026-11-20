//! Mutation, loops, exits, patterns and function values run from MSL source on the Rust backend
//! (feature `composite-values`, CV-01 to CV-08): one test per run-time criterion, each model
//! written in `tests/control/`, elaborated by the front end, compiled on the transpiled backend
//! and run on the solver. Every expected value is computed here by Rust from the rule the test
//! cites, never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::project::Project;
use piperine_solver::prelude::{Context, NodeIdentifier};

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

/// How far the solver may move a value it was given as a current into 1 ohm: its `gmin` and
/// round-off, not a tolerance of the rules (a reaction's values are bit for bit).
const SOLVE: f64 = 1e-9;

/// The lone file `file` of `tests/control/`, as a project.
fn project(file: &str) -> Project {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/control").join(file);
    Project::new(&path)
}

/// The bytes of a known `f64` carrier value: the value, then the known byte.
fn real_bytes(v: f64) -> Vec<u8> {
    let mut bytes = vec![0u8; 16];
    bytes[..8].copy_from_slice(&v.to_le_bytes());
    bytes[8] = 1;
    bytes
}

/// The bytes of a known one-byte carrier value (`u8`, `bool`).
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

/// The operating point of `top` of `file`: the voltage of each node in `names`.
fn operating_point(file: &str, top: &str, names: &[&str]) -> Vec<f64> {
    let mut running = project(file).compile(top).unwrap_or_else(|r| panic!("{r}"));
    let solved = running.circuit.dc(Context::default()).expect("dc").solve().expect("the operating point solves");
    names
        .iter()
        .map(|name| {
            let node = running.ir.node_at(&format!("{top}.{name}")).unwrap_or_else(|| panic!("node {name}"));
            solved.get_node(&NodeIdentifier::Anonymous(node.id().index())).expect("v(n)")
        })
        .collect()
}

/// The sequence of `let_mut.msl`'s integer operators, in Rust.
fn integer_operators(start: u8) -> u8 {
    let mut n = start;
    n += 3;
    n -= 1;
    n *= 2;
    n /= 3;
    n %= 5;
    n &= 7;
    n |= 8;
    n ^= 5;
    n <<= 1;
    n >>= 1;
    n
}

/// The sequence of `let_mut.msl`'s real operators, in Rust.
fn real_operators(start: f64) -> f64 {
    let mut x = start;
    x += 1.5;
    x -= 0.25;
    x *= 3.0;
    x /= 2.0;
    x
}

#[test]
fn let_mut_and_every_compound_operator_give_rusts_values() {
    // CV-01 AC 1 (§1.5.12): `x = e` and every compound operator of level 15, in a function, in a
    // reaction (bit for bit) and in an `analog` block (after the solve).
    let names = ["f", "g", "h", "d1", "d2", "r", "local_n", "local_x", "local_b"];
    let committed = first_commits("let_mut.msl", &names);
    let mut flag = true;
    flag &= false;
    flag |= !false;
    flag ^= true;
    let mut local = true;
    local &= false;
    local |= true;
    local ^= true;
    let reassigned = {
        let mut x = 3.0f64;
        x = x * x;
        x = x + 3.0;
        x
    };
    let wanted = [
        real_bytes(real_operators(2.0)),
        byte(integer_operators(20)),
        byte(u8::from(flag)),
        byte(1),
        byte(2),
        real_bytes(reassigned),
        byte(integer_operators(20)),
        real_bytes(real_operators(2.0)),
        byte(u8::from(local)),
    ];
    for (k, name) in names.iter().enumerate() {
        assert_eq!(committed[k], wanted[k], "{name} is the value Rust gives");
    }

    let currents = operating_point("let_mut.msl", "AnalogBench", &["a", "b", "c"]);
    let analog = [real_operators(2.0), {
        let mut y = 20.0f64;
        y -= 7.0;
        y += 1.0;
        y
    }, real_operators(2.0)];
    for (k, want) in analog.iter().enumerate() {
        assert!((currents[k] - want).abs() <= SOLVE * want.abs().max(1.0), "analog node {k}: {want:?}, the operating point has {:?}", currents[k]);
    }
}
