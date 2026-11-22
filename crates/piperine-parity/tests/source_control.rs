//! Mutation, loops, exits, patterns and function values run from MSL source on the Rust backend
//! (feature `composite-values`, CV-01 to CV-08): one test per run-time criterion, each model
//! written in `tests/control/`, elaborated by the front end, compiled on the transpiled backend
//! and run on the solver. Every expected value is computed here by Rust from the rule the test
//! cites, never read back from the model.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use piperine_parity::project::Project;
use piperine_solver::prelude::{AcSweepAnalysisOptions, Context, NodeIdentifier, Solver, TransientAnalysisOptions};

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

/// A `u16` carrier value: two bytes little-endian, the known byte, one byte of padding (abi.md §3.1).
fn word(v: u16) -> Vec<u8> {
    let mut bytes = v.to_le_bytes().to_vec();
    bytes.extend([1, 0]);
    bytes
}

#[test]
fn every_loop_form_gives_rusts_values() {
    // CV-02 AC 1 (§1.8.7 to §1.8.12): `while`, `while let`, `loop` with `break value`, `for` over
    // `a..b` and `a..=b` with `continue` and `break`, nested; each in a function and in the
    // reaction itself, bit for bit.
    let names = ["w", "l", "o", "inc", "m", "d", "nest", "rw", "rl", "ro", "rm", "rd"];
    let committed = first_commits("loops.msl", &names);
    let sum_to = |n: u16| (0..n).sum::<u16>();
    let first_square_over = |limit: u16| (0u16..).find(|k| k * k > limit).expect("a square over");
    let odd_sum = |n: u8| (0..n).filter(|k| k % 2 == 1).map(u16::from).sum::<u16>();
    let first_multiple = |step: u8, over: u8| (1..32u8).map(|k| k * step).find(|m| *m > over).unwrap_or(0);
    let nested = |n: u8| (0..n).flat_map(|a| (0..a).map(move |b| u16::from(a * b))).sum::<u16>();
    let wanted = [
        word(sum_to(10)),
        word(first_square_over(50)),
        word(odd_sum(9)),
        word((1..=6u16).sum()),
        byte(first_multiple(3, 20)),
        word((0..5u16).sum()),
        word(nested(5)),
        word(sum_to(10)),
        word(first_square_over(50)),
        word(odd_sum(9)),
        byte(first_multiple(3, 20)),
        word((0..5u16).sum()),
    ];
    for (k, name) in names.iter().enumerate() {
        assert_eq!(committed[k], wanted[k], "{name} is the value Rust gives");
    }
}

#[test]
fn a_commit_in_a_reaction_loop_is_one_statement_last_write_winning() {
    // CV-02 AC 5 (§12.1.14, §12.1.2): a commit in a loop is one statement run several times; the
    // writes of one instant to one carrier leave the last one, a single change.
    let seen = build(&project("loops.msl"), "CommitBench").run(10e-9, 1e-9);
    for (name, last) in [("last", 3u8), ("picked", 20), ("counted", 3)] {
        let changes = transient(&seen, &format!("CommitBench.{name}"));
        assert_eq!(changes, vec![(5 * NS / 2, byte(last))], "{name}: one change, to the last value written");
    }
}

#[test]
fn an_analog_loop_without_contribution_is_differentiated_through() {
    // CV-02 AC 4 (§11 rule 5): `let mut y = v(a); for _ in 0..3 { y = y * v(a); }` is v^4: at
    // v = 1.5 the operating point is v^4 and the AC gain is its partial 4 v^3, the same numbers
    // as the written-out product's.
    let (v, value, partial) = (1.5f64, 1.5f64.powi(4), 4.0 * 1.5f64.powi(3));
    for top in ["LoopedBench", "WrittenBench"] {
        let at = operating_point("loops.msl", top, &["o"]);
        assert!((at[0] - value).abs() <= SOLVE * value, "{top}: v^4 at {v} V is {value}, the operating point has {}", at[0]);
        let mut running = project("loops.msl").compile(top).unwrap_or_else(|r| panic!("{r}"));
        let node = NodeIdentifier::Anonymous(running.ir.node_at(&format!("{top}.o")).expect("a node").id().index());
        let mut ac = running.circuit.ac(Context::default()).expect("ac");
        let sweep = ac.solve_sweep(AcSweepAnalysisOptions { start_frequency: 1e3, stop_frequency: 1e5, steps: 3, logarithmic: true }).unwrap_or_else(|e| panic!("the sweep does not solve: {e}"));
        for step in sweep.iter() {
            let gain = step.get_node(&node).expect("v(o)").norm();
            assert!((gain - partial).abs() <= 1e-6 * partial, "{top}: the partial is 4 v^3 = {partial}, the AC gain is {gain}");
        }
    }
}

#[test]
fn an_empty_range_runs_zero_times_in_analog_too() {
    // CV-02 (§1.8.9): `0..0` and `0..count` with `count = 0` run the body zero times, as does
    // `while false`: the locals keep their values.
    let at = operating_point("loops.msl", "EmptyBench", &["o", "p"]);
    assert!((at[0] - 2.0).abs() <= SOLVE * 2.0, "`for _ in 0..0` keeps y: {}", at[0]);
    assert!((at[1] - 3.0).abs() <= SOLVE * 3.0, "an empty param range and `while false` keep z: {}", at[1]);
}

/// An `i16` carrier value: two bytes little-endian, the known byte, one byte of padding.
fn short(v: i16) -> Vec<u8> {
    let mut bytes = v.to_le_bytes().to_vec();
    bytes.extend([1, 0]);
    bytes
}

#[test]
fn an_early_return_gives_the_caller_its_value() {
    // CV-03 AC 1 (§1.8.13): `return e` from inside a loop, an `if` and a `match` arm gives the
    // caller `e` and skips the rest of the body.
    let names = ["a", "b", "c", "d", "e", "f"];
    let committed = first_commits("returns.msl", &names);
    let first_over = |limit: u8| (0..200u8).find(|k| k * k > limit).unwrap_or(255);
    let clamp_low = |x: i16| if x < 0 { 0 } else { x };
    let named = |m: u8| -> u8 {
        let v = match m {
            0 => return 100,
            1 => 7,
            _ => m * 2,
        };
        v + 1
    };
    let wanted = [byte(first_over(50)), short(clamp_low(-5)), short(clamp_low(5)), byte(named(0)), byte(named(1)), byte(named(5))];
    for (k, name) in names.iter().enumerate() {
        assert_eq!(committed[k], wanted[k], "{name} is the value Rust gives");
    }
}

#[test]
fn the_question_mark_on_none_returns_none() {
    // CV-03 AC 2 (§1.8.14): in a function whose result is an `Option`, `x?` is the payload of
    // `Some` and returns `None` from the function for `None`, also through a second call.
    let half = |x: Option<u8>| -> Option<u8> { Some(x? / 2) };
    let twice = |x: Option<u8>| -> Option<u8> {
        let a = half(x)?;
        half(Some(a))
    };
    let names = ["g", "h", "some", "none", "third"];
    let committed = first_commits("returns.msl", &names);
    let wanted = [
        byte(half(Some(8)).unwrap_or(99)),
        byte(half(None).unwrap_or(99)),
        byte(u8::from(half(Some(8)).is_some())),
        byte(u8::from(half(None).is_none())),
        byte(twice(Some(8)).unwrap_or(99)),
    ];
    for (k, name) in names.iter().enumerate() {
        assert_eq!(committed[k], wanted[k], "{name} is the value Rust gives");
    }
}

/// The commits of `patterns.msl`'s `Rows`, one `u8` each, in the order of `names`.
fn pattern_rows(names: &[&str]) -> Vec<u8> {
    first_commits("patterns.msl", names).into_iter().map(|bytes| bytes[0]).collect()
}

#[test]
fn a_match_guard_takes_the_arm_only_when_true() {
    // CV-04 AC 1 (§1.6.5): the arm is taken when its pattern matches and its guard holds; a guard
    // that fails falls through to the later arms, as Rust.
    let class = |x: i16| match x {
        n if n < 0 => 0u8,
        0 => 1,
        n if n % 2 == 0 => 2,
        _ => 3,
    };
    assert_eq!(pattern_rows(&["g0", "g1", "g2", "g3"]), vec![class(-3), class(0), class(4), class(5)]);
    // a guard on a name binds it in the reaction's own match
    let local = |n: u8| match n {
        n if n > 5 => n + 1,
        0..=5 => 0,
        _ => 99,
    };
    assert_eq!(pattern_rows(&["t0"]), vec![local(7)]);
}

#[test]
fn range_patterns_match_as_rust() {
    // CV-04 AC 2 (§1.6.4): `a..=b`, `a..b` and `a..` over integers and floats, bounds included
    // or excluded as written.
    let range = |x: u8| match x {
        0..=9 => 1u8,
        10..100 => 2,
        100.. => 3,
    };
    let real = |x: f64| match x {
        0.0..=1.0 => 1u8,
        -1.0..0.0 => 0,
        _ => 2,
    };
    let got = pattern_rows(&["r0", "r1", "r2", "r3", "r4", "r5", "q0", "q1", "q2", "q3"]);
    let wanted: Vec<u8> = [0u8, 9, 10, 99, 100, 255].into_iter().map(range).chain([0.5, -0.5, 2.0, 1.0].into_iter().map(real)).collect();
    assert_eq!(got, wanted);
}

#[test]
fn or_patterns_and_at_bindings_bind_as_rust() {
    // CV-04 AC 3 (§1.6.6, §1.6.7): `1 | 2 | 3`, `n @ 6..=9`, `n @ (1 | 2)` and `Some(n @ 1..=5)`
    // bind the matched value as Rust does.
    let or = |x: u8| match x {
        1 | 2 | 3 => 10u8,
        4 | 5 => 20,
        n @ 6..=9 => n,
        _ => 0,
    };
    let at = |x: u8| match x {
        n @ (1 | 2) => n * 10,
        n @ 3..=5 => n,
        _ => 0u8,
    };
    let option = |o: Option<u8>| match o {
        Some(n @ 1..=5) => n,
        Some(n) if n > 100 => 100u8,
        Some(_) => 50,
        None => 0,
    };
    let got = pattern_rows(&["o0", "o1", "o2", "o3", "a0", "a1", "a2", "p0", "p1", "p2", "p3"]);
    let wanted = vec![or(2), or(5), or(7), or(0), at(2), at(4), at(9), option(Some(3)), option(Some(200)), option(Some(50)), option(None)];
    assert_eq!(got, wanted);
}

#[test]
fn option_arms_take_rusts_patterns() {
    // CV-04 AC 1 and AC 4 (finding 4): `Some(1)`, `Some(2) | Some(3)`, `Some(n) if ..`, `_` and `None` arms, a name
    // that binds the whole `Option` (`whole @ Some(7)`, `other`) and a pattern nested in `Some` (`Some((n, true))`)
    // over an `Option` take the arm Rust takes.
    let exact = |o: Option<u8>| match o {
        Some(1) => 10u8,
        Some(2) | Some(3) => 20,
        Some(n) if n > 100 => 30,
        None => 40,
        _ => 50,
    };
    let wild = |o: Option<u8>| match o {
        Some(1) => 10u8,
        _ => 20,
    };
    let bound = |o: Option<u8>| match o {
        whole @ Some(7) => whole.unwrap_or(0) + 1,
        other => other.unwrap_or(9),
    };
    let nested = |o: Option<(u8, bool)>| match o {
        Some((n, true)) => n,
        Some((_, false)) => 1u8,
        None => 0,
    };
    let got = pattern_rows(&["s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "s12", "s13"]);
    let wanted = vec![
        exact(Some(1)),
        exact(Some(3)),
        exact(Some(200)),
        exact(None),
        exact(Some(5)),
        wild(Some(1)),
        wild(Some(2)),
        wild(None),
        bound(Some(7)),
        bound(Some(3)),
        bound(None),
        nested(Some((4, true))),
        nested(Some((4, false))),
        nested(None),
    ];
    assert_eq!(got, wanted);
}

#[test]
fn refutable_patterns_run_in_if_let_while_let_and_let_else() {
    // CV-04 AC 4 (§1.6.9, §1.7.6, §1.8.6): `if let`, `while let` and `let ... else` with literal,
    // range, `Some` and nested patterns run as Rust; a `let ... else` that fails runs its block.
    let if_let = |o: Option<u8>| if let Some(v) = o { v + 1 } else { 0 };
    let if_let_range = |x: u8| if let n @ 3..=5 = x { n * 2 } else { 0 };
    let if_let_option_range = |o: Option<u8>| if let Some(1..=3) = o { 1u8 } else { 0 };
    let let_else = |o: Option<u8>| -> u8 {
        let Some(v) = o else { return 77 };
        v * 2
    };
    let small_sum = |limit: u8| {
        let (mut i, mut sum) = (0u8, 0u8);
        while let Some(v @ 0..=4) = (i < limit).then_some(i) {
            sum += v;
            i += 1;
        }
        sum
    };
    let got = pattern_rows(&["l0", "l1", "l2", "l3", "l4", "l5", "l6", "l7", "l8", "l9", "w0", "t1"]);
    let wanted = vec![
        if_let(Some(4)),
        if_let(None),
        if_let_range(4),
        if_let_range(6),
        if_let_option_range(Some(2)),
        if_let_option_range(Some(9)),
        if_let_option_range(None),
        let_else(Some(5)),
        let_else(None),
        small_sum(10),
        small_sum(3),
        7, // `if let Some(n @ 6..=8) = Some(7)` gives `got = 7`
    ];
    assert_eq!(got, wanted);
}

#[test]
fn closures_run_and_capture_by_value() {
    // CV-05 AC 1 (§1.5.14, §3 rule 3): a closure bound by `let` and called runs; it captures by
    // value where it is written, so `k = 10` after it does not reach it; a non-capturing closure
    // is a function pointer (CV-06 AC 2).
    let add = |x: u8, y: u8| x + y;
    let mut k = 3u8;
    let plus_k = move |x: u8| x + k;
    k = 10;
    let twice = |f: fn(u8) -> u8, x: u8| f(f(x));
    let committed = first_commits("closures.msl", &["a", "b", "c", "g"]);
    assert_eq!(committed, vec![byte(add(3, 4)), byte(plus_k(1)), byte(k), byte(twice(|x| x + 100, 1))]);
}

#[test]
fn a_function_value_calls_the_named_function() {
    // CV-06 AC 1 (§1.5.10): `let f = double; f(21)` and `apply(double, 5)` call `double`.
    let double = |x: u8| x * 2;
    let committed = first_commits("closures.msl", &["d", "e"]);
    assert_eq!(committed, vec![byte(double(21)), byte(double(5))]);
}

#[test]
fn a_closure_in_analog_is_differentiated_through() {
    // CV-05 AC 3 (§3 rule 1): a closure called in `analog` is differentiated as a direct call, and
    // so is a function value: v^3 at 1.5 V is 3.375 and its partial 3 v^2 = 6.75, as the
    // written-out product's.
    let (value, partial) = (1.5f64.powi(3), 3.0 * 1.5f64.powi(2));
    let at = operating_point("closures.msl", "ClosureBench", &["o", "p"]);
    for (k, got) in at.iter().enumerate() {
        assert!((got - value).abs() <= SOLVE * value, "node {k}: v^3 is {value}, the operating point has {got}");
    }
    let mut running = project("closures.msl").compile("ClosureBench").unwrap_or_else(|r| panic!("{r}"));
    let mut ac = running.circuit.ac(Context::default()).expect("ac");
    let sweep = ac.solve_sweep(AcSweepAnalysisOptions { start_frequency: 1e3, stop_frequency: 1e5, steps: 3, logarithmic: true }).unwrap_or_else(|e| panic!("the sweep does not solve: {e}"));
    for name in ["o", "p"] {
        let node = NodeIdentifier::Anonymous(running.ir.node_at(&format!("ClosureBench.{name}")).expect("a node").id().index());
        for step in sweep.iter() {
            let gain = step.get_node(&node).expect("v").norm();
            assert!((gain - partial).abs() <= 1e-6 * partial, "{name}: the partial is 3 v^2 = {partial}, the AC gain is {gain}");
        }
    }
}

/// The message of the fault the transient of `top` of `file` runs into: the run is expected to fail.
fn the_fault(file: &str, top: &str) -> String {
    let built = build(&project(file), top);
    let circuit = built.compiled.circuit().unwrap_or_else(|e| panic!("{e}"));
    let mut options = TransientAnalysisOptions::new(10e-9, 1e-9);
    options.dt_max = 1e-9;
    let mut solver = Solver::new(circuit).with_context(Context::default()).with_tran_opts(options).build();
    match solver.tran().and_then(|mut t| t.solve()) {
        Ok(_) => panic!("the run was expected to fault"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn recursion_from_a_reaction_runs() {
    // CV-07 AC 1 (§3 rule 12): a function that calls itself, and two that call each other, run from
    // a reaction and give the values Rust's recursion gives.
    fn fib(n: u8) -> u16 {
        if n < 2 { u16::from(n) } else { fib(n - 1) + fib(n - 2) }
    }
    fn is_even(n: u8) -> bool {
        n == 0 || is_odd(n - 1)
    }
    fn is_odd(n: u8) -> bool {
        n != 0 && is_even(n - 1)
    }
    let committed = first_commits("recursion.msl", &["a", "b", "c", "d"]);
    let wanted = vec![word(fib(10)), byte(u8::from(is_even(10))), byte(u8::from(is_odd(7))), byte(u8::from(is_even(7)))];
    assert_eq!(committed, wanted);
}

#[test]
fn runaway_loops_and_recursions_fail_the_analysis_by_name() {
    // CV-07 AC 4 (Q5 (b)): a recursion deeper than 1 024 calls and a loop of more than 2^24
    // iterations fail the analysis with a message naming the function or the loop (by its place
    // in the source), the instance and the instant; the process, which runs this test on, survives.
    let deep = the_fault("recursion.msl", "RunawayBench");
    assert!(deep.contains("deep") && deep.contains("1024"), "the fault names the function and the bound: {deep}");
    assert!(deep.contains("RunawayBench.runaway"), "the fault names the instance: {deep}");
    let endless = the_fault("recursion.msl", "EndlessBench");
    assert!(endless.contains("loop @ recursion.msl:12"), "the fault names the loop by its place in the source: {endless}");
    assert!(endless.contains("16777216"), "the fault gives the bound, 2^24: {endless}");
    assert!(endless.contains("EndlessBench.endless"), "the fault names the instance: {endless}");
}
