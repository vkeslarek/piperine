//! Mutation, loops, exits, patterns and function values run from MSL source on the Rust backend
//! (feature `composite-values`, CV-01 to CV-08): one test per run-time criterion, each model
//! written in `tests/control/`, elaborated by the front end, compiled on the transpiled backend
//! and run on the solver. Every expected value is computed here by Rust from the rule the test
//! cites, never read back from the model.
#![cfg(unix)]

use piperine_solver::prelude::{AcSweepAnalysisOptions, Context, NodeIdentifier};

use piperine_parity::source::bytes;
use piperine_parity::source::operators;
use piperine_parity::source::run;

use bytes::CarrierBytes;
use operators::PlaceOperators;
use run::SourceRun;
use piperine_parity::buck_fixture::NetChanges;
use piperine_parity::mixed::BuiltDesign;

/// One nanosecond in ticks.
const NS: i64 = 1_000_000;

/// The model files of `tests/control/`.
const RUN: SourceRun = SourceRun::in_directory("tests/control");

/// How far the solver may move a value it was given as a current into 1 ohm: its `gmin` and
/// round-off, not a tolerance of the rules (a reaction's values are bit for bit).
const SOLVE: f64 = 1e-9;

#[test]
fn let_mut_and_every_compound_operator_give_rusts_values() {
    // CV-01 AC 1 (§1.5.12): `x = e` and every compound operator of level 15, in a function, in a
    // reaction (bit for bit) and in an `analog` block (after the solve).
    let names = ["f", "g", "h", "d1", "d2", "r", "local_n", "local_x", "local_b"];
    let committed = RUN.first_commits("let_mut.msl", &names);
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
        CarrierBytes::real(PlaceOperators::real(2.0)),
        CarrierBytes::byte(PlaceOperators::integer(20)),
        CarrierBytes::byte(u8::from(flag)),
        CarrierBytes::byte(1),
        CarrierBytes::byte(2),
        CarrierBytes::real(reassigned),
        CarrierBytes::byte(PlaceOperators::integer(20)),
        CarrierBytes::real(PlaceOperators::real(2.0)),
        CarrierBytes::byte(u8::from(local)),
    ];
    for (k, name) in names.iter().enumerate() {
        assert_eq!(committed[k], wanted[k], "{name} is the value Rust gives");
    }

    let currents = RUN.operating_point("let_mut.msl", "AnalogBench", &["a", "b", "c"]);
    let analog = [PlaceOperators::real(2.0), {
        let mut y = 20.0f64;
        y -= 7.0;
        y += 1.0;
        y
    }, PlaceOperators::real(2.0)];
    for (k, want) in analog.iter().enumerate() {
        assert!((currents[k] - want).abs() <= SOLVE * want.abs().max(1.0), "analog node {k}: {want:?}, the operating point has {:?}", currents[k]);
    }
}

#[test]
fn every_loop_form_gives_rusts_values() {
    // CV-02 AC 1 (§1.8.7 to §1.8.12): `while`, `while let`, `loop` with `break value`, `for` over
    // `a..b` and `a..=b` with `continue` and `break`, nested; each in a function and in the
    // reaction itself, bit for bit.
    let names = ["w", "l", "o", "inc", "m", "d", "nest", "rw", "rl", "ro", "rm", "rd"];
    let committed = RUN.first_commits("loops.msl", &names);
    let sum_to = |n: u16| (0..n).sum::<u16>();
    let first_square_over = |limit: u16| (0u16..).find(|k| k * k > limit).expect("a square over");
    let odd_sum = |n: u8| (0..n).filter(|k| k % 2 == 1).map(u16::from).sum::<u16>();
    let first_multiple = |step: u8, over: u8| (1..32u8).map(|k| k * step).find(|m| *m > over).unwrap_or(0);
    let nested = |n: u8| (0..n).flat_map(|a| (0..a).map(move |b| u16::from(a * b))).sum::<u16>();
    let wanted = [
        CarrierBytes::word(sum_to(10)),
        CarrierBytes::word(first_square_over(50)),
        CarrierBytes::word(odd_sum(9)),
        CarrierBytes::word((1..=6u16).sum()),
        CarrierBytes::byte(first_multiple(3, 20)),
        CarrierBytes::word((0..5u16).sum()),
        CarrierBytes::word(nested(5)),
        CarrierBytes::word(sum_to(10)),
        CarrierBytes::word(first_square_over(50)),
        CarrierBytes::word(odd_sum(9)),
        CarrierBytes::byte(first_multiple(3, 20)),
        CarrierBytes::word((0..5u16).sum()),
    ];
    for (k, name) in names.iter().enumerate() {
        assert_eq!(committed[k], wanted[k], "{name} is the value Rust gives");
    }
}

#[test]
fn a_commit_in_a_reaction_loop_is_one_statement_last_write_winning() {
    // CV-02 AC 5 (§12.1.14, §12.1.2): a commit in a loop is one statement run several times; the
    // writes of one instant to one carrier leave the last one, a single change.
    let seen = BuiltDesign::of(&RUN.project("loops.msl"), "CommitBench").run(10e-9, 1e-9);
    for (name, last) in [("last", 3u8), ("picked", 20), ("counted", 3)] {
        let changes = NetChanges::transient(&seen, &format!("CommitBench.{name}"));
        assert_eq!(changes, vec![(5 * NS / 2, CarrierBytes::byte(last))], "{name}: one change, to the last value written");
    }
}

#[test]
fn an_analog_loop_without_contribution_is_differentiated_through() {
    // CV-02 AC 4 (§11 rule 5): `let mut y = v(a); for _ in 0..3 { y = y * v(a); }` is v^4: at
    // v = 1.5 the operating point is v^4 and the AC gain is its partial 4 v^3, the same numbers
    // as the written-out product's.
    let (v, value, partial) = (1.5f64, 1.5f64.powi(4), 4.0 * 1.5f64.powi(3));
    for top in ["LoopedBench", "WrittenBench"] {
        let at = RUN.operating_point("loops.msl", top, &["o"]);
        assert!((at[0] - value).abs() <= SOLVE * value, "{top}: v^4 at {v} V is {value}, the operating point has {}", at[0]);
        let mut running = RUN.project("loops.msl").compile(top).unwrap_or_else(|r| panic!("{r}"));
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
    let at = RUN.operating_point("loops.msl", "EmptyBench", &["o", "p"]);
    assert!((at[0] - 2.0).abs() <= SOLVE * 2.0, "`for _ in 0..0` keeps y: {}", at[0]);
    assert!((at[1] - 3.0).abs() <= SOLVE * 3.0, "an empty param range and `while false` keep z: {}", at[1]);
}

#[test]
fn an_early_return_gives_the_caller_its_value() {
    // CV-03 AC 1 (§1.8.13): `return e` from inside a loop, an `if` and a `match` arm gives the
    // caller `e` and skips the rest of the body.
    let names = ["a", "b", "c", "d", "e", "f"];
    let committed = RUN.first_commits("returns.msl", &names);
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
    let wanted = [CarrierBytes::byte(first_over(50)), CarrierBytes::short(clamp_low(-5)), CarrierBytes::short(clamp_low(5)), CarrierBytes::byte(named(0)), CarrierBytes::byte(named(1)), CarrierBytes::byte(named(5))];
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
    let committed = RUN.first_commits("returns.msl", &names);
    let wanted = [
        CarrierBytes::byte(half(Some(8)).unwrap_or(99)),
        CarrierBytes::byte(half(None).unwrap_or(99)),
        CarrierBytes::byte(u8::from(half(Some(8)).is_some())),
        CarrierBytes::byte(u8::from(half(None).is_none())),
        CarrierBytes::byte(twice(Some(8)).unwrap_or(99)),
    ];
    for (k, name) in names.iter().enumerate() {
        assert_eq!(committed[k], wanted[k], "{name} is the value Rust gives");
    }
}

#[test]
fn an_option_function_that_logs_or_takes_a_str_runs_its_body_once() {
    // CV finding 13 (§19 rule 10, §1.2.9): a function returning an `Option` that reaches `emit!` or takes a
    // `str` is inlined where it stands, once per call, whichever way the `Option` is read (`unwrap_or`,
    // `match`, `if let`, `is_some`): its record appears once per call and the values are Rust's.
    let probe = |x: u8| if x > 1 { Some(x + 1) } else { None };
    let pick = |name: &str, x: u8| if name == "go" && x > 1 { Some(x * 2) } else { None };
    let noted = |name: &str, x: u8| if name == "go" { Some(x) } else { None };
    let matched = match probe(0) {
        Some(v) => v,
        None => 7,
    };
    let bound = if let Some(v) = probe(4) { v * 2 } else { 0 };
    // a `return` and a `?` inside the inlined body, each after a `log`, also inside a `for`
    let early_none = |x: u8| if x > 5 { None } else { Some(x) };
    let early_some = |x: u8| if x > 1 { Some(x + 10) } else { Some(0) };
    let through = |x: u8| -> Option<u8> {
        let inner = if x > 2 { Some(x) } else { None };
        let v = inner?;
        Some(v + 1)
    };
    let in_loop = |n: u8| -> Option<u8> {
        for k in 0..n {
            if k == 3 {
                return Some(k * 2);
            }
        }
        None
    };
    let names = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n"];
    let wanted = [
        CarrierBytes::byte(probe(3).unwrap_or(9)),
        CarrierBytes::byte(matched),
        CarrierBytes::byte(bound),
        CarrierBytes::byte(u8::from(probe(5).is_some())),
        CarrierBytes::byte(pick("go", 5).unwrap_or(1)),
        CarrierBytes::byte(pick("stop", 5).unwrap_or(1)),
        CarrierBytes::byte(noted("go", 6).unwrap_or(0)),
        CarrierBytes::byte(early_none(9).unwrap_or(77)),
        CarrierBytes::byte(early_none(2).unwrap_or(77)),
        CarrierBytes::byte(early_some(4).unwrap_or(0)),
        CarrierBytes::byte(through(4).unwrap_or(99)),
        CarrierBytes::byte(through(1).unwrap_or(99)),
        CarrierBytes::byte(in_loop(8).unwrap_or(1)),
        CarrierBytes::byte(in_loop(2).unwrap_or(1)),
    ];
    RUN.assert_commits("option_log.msl", &names, &wanted);
    // twelve calls of a function that logs (`probe` four times, `noted` once, seven that leave early), one record each
    let records = RUN.records_of("option_log.msl", "Bench");
    assert_eq!(records.len(), 12, "one record per call: {records:?}");
    assert!(records.iter().all(|r| r.stream == "log" && r.instance == "Bench.rows"), "{records:?}");
}

#[test]
fn locals_named_a_and_a_underscore_merge_under_unknown_control() {
    // finding 14: `a` and `a_` written in one `if` on an undriven (unknown) condition are merged and handed
    // back under distinct names, the generated source compiling with no warning; both arms agree, so the
    // merge is the value Rust computes, known.
    let (a, a_) = (5u8, 6u8);
    let names = ["p", "q", "s"];
    let wanted = [CarrierBytes::byte(a), CarrierBytes::byte(a_), CarrierBytes::byte(a + a_)];
    RUN.assert_commits("merge_names.msl", &names, &wanted);
}

/// The rows `patterns.msl` commits.
struct PatternRows;

impl PatternRows {
    /// The commits of `Rows`, one `u8` each, in the order of `names`.
    fn commits(names: &[&str]) -> Vec<u8> {
        RUN.first_commits("patterns.msl", names).into_iter().map(|bytes| bytes[0]).collect()
    }
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
    assert_eq!(PatternRows::commits(&["g0", "g1", "g2", "g3"]), vec![class(-3), class(0), class(4), class(5)]);
    // a guard on a name binds it in the reaction's own match
    let local = |n: u8| match n {
        n if n > 5 => n + 1,
        0..=5 => 0,
        _ => 99,
    };
    assert_eq!(PatternRows::commits(&["t0"]), vec![local(7)]);
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
    let got = PatternRows::commits(&["r0", "r1", "r2", "r3", "r4", "r5", "q0", "q1", "q2", "q3"]);
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
    let got = PatternRows::commits(&["o0", "o1", "o2", "o3", "a0", "a1", "a2", "p0", "p1", "p2", "p3"]);
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
    let got = PatternRows::commits(&["s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "s12", "s13"]);
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
fn a_match_or_if_let_yielding_an_option_runs_as_rust() {
    // CV-04 AC 1 and AC 4 (GAPS L66): a `match` or an `if let` whose value is an `Option` (arms by key, by a
    // guard over an `Option`, and `if let`) takes the arm Rust takes, flag and payload both.
    let by_key = |k: u8| match k {
        0 => Some(1u8),
        1 => Some(k + 10),
        _ => None,
    };
    let by_guard = |o: Option<u8>| match o {
        Some(n) if n > 5 => Some(n * 2),
        Some(_) => Some(0u8),
        None => None,
    };
    let if_let = |o: Option<u8>| if let Some(n) = o { Some(n + 1) } else { None };
    let got = PatternRows::commits(&["m0", "m1", "m2", "m3", "m4", "m5", "m6", "m7"]);
    let wanted = vec![by_key(0).unwrap_or(99), by_key(1).unwrap_or(99), by_key(2).unwrap_or(99), by_guard(Some(7)).unwrap_or(99), by_guard(Some(3)).unwrap_or(99), by_guard(None).unwrap_or(99), if_let(Some(4)).unwrap_or(99), if_let(None).unwrap_or(99)];
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
    let got = PatternRows::commits(&["l0", "l1", "l2", "l3", "l4", "l5", "l6", "l7", "l8", "l9", "w0", "t1"]);
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
    let committed = RUN.first_commits("closures.msl", &["a", "b", "c", "g"]);
    assert_eq!(committed, vec![CarrierBytes::byte(add(3, 4)), CarrierBytes::byte(plus_k(1)), CarrierBytes::byte(k), CarrierBytes::byte(twice(|x| x + 100, 1))]);
}

#[test]
fn a_function_value_calls_the_named_function() {
    // CV-06 AC 1 (§1.5.10): `let f = double; f(21)` and `apply(double, 5)` call `double`.
    let double = |x: u8| x * 2;
    let committed = RUN.first_commits("closures.msl", &["d", "e"]);
    assert_eq!(committed, vec![CarrierBytes::byte(double(21)), CarrierBytes::byte(double(5))]);
}

#[test]
fn a_closure_in_analog_is_differentiated_through() {
    // CV-05 AC 3 (§3 rule 1): a closure called in `analog` is differentiated as a direct call, and
    // so is a function value: v^3 at 1.5 V is 3.375 and its partial 3 v^2 = 6.75, as the
    // written-out product's.
    let (value, partial) = (1.5f64.powi(3), 3.0 * 1.5f64.powi(2));
    let at = RUN.operating_point("closures.msl", "ClosureBench", &["o", "p"]);
    for (k, got) in at.iter().enumerate() {
        assert!((got - value).abs() <= SOLVE * value, "node {k}: v^3 is {value}, the operating point has {got}");
    }
    let mut running = RUN.project("closures.msl").compile("ClosureBench").unwrap_or_else(|r| panic!("{r}"));
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

/// Rust's own recursion, the oracle of `recursion_from_a_reaction_runs`.
struct RustRecursion;

impl RustRecursion {
    // CV-07 AC 1 (§3 rule 12): a function that calls itself, and two that call each other, run from
    // a reaction and give the values Rust's recursion gives.
    fn fib(n: u8) -> u16 {
        if n < 2 { u16::from(n) } else { RustRecursion::fib(n - 1) + RustRecursion::fib(n - 2) }
    }

    fn is_even(n: u8) -> bool {
        n == 0 || RustRecursion::is_odd(n - 1)
    }

    fn is_odd(n: u8) -> bool {
        n != 0 && RustRecursion::is_even(n - 1)
    }
}

#[test]
fn recursion_from_a_reaction_runs() {
    let committed = RUN.first_commits("recursion.msl", &["a", "b", "c", "d"]);
    let wanted = vec![CarrierBytes::word(RustRecursion::fib(10)), CarrierBytes::byte(u8::from(RustRecursion::is_even(10))), CarrierBytes::byte(u8::from(RustRecursion::is_odd(7))), CarrierBytes::byte(u8::from(RustRecursion::is_even(7)))];
    assert_eq!(committed, wanted);
}

#[test]
fn runaway_loops_and_recursions_fail_the_analysis_by_name() {
    // CV-07 AC 4 (Q5 (b)): a recursion deeper than 1 024 calls and a loop of more than 2^24
    // iterations fail the analysis with a message naming the function or the loop (by its place
    // in the source), the instance and the instant; the process, which runs this test on, survives.
    let deep = RUN.the_fault("recursion.msl", "RunawayBench");
    assert!(deep.contains("deep") && deep.contains("1024"), "the fault names the function and the bound: {deep}");
    assert!(deep.contains("RunawayBench.runaway"), "the fault names the instance: {deep}");
    let endless = RUN.the_fault("recursion.msl", "EndlessBench");
    assert!(endless.contains("loop @ recursion.msl:12"), "the fault names the loop by its place in the source: {endless}");
    assert!(endless.contains("16777216"), "the fault gives the bound, 2^24: {endless}");
    assert!(endless.contains("EndlessBench.endless"), "the fault names the instance: {endless}");
}
