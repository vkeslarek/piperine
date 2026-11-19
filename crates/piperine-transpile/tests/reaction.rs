//! Compiled reactions and drives, called through the ABI (feature `ir-rust`, RS-11 AC 3, 5, 6,
//! RS-06 AC 6; checks C40, C71, C73, C74; and C109, the environment read by a reaction and a
//! drive, planned and compiled): each test compiles small hand-built bodies, the one
//! design shared by the five, and calls a reaction (or a drive) through `caller::Handle`,
//! recording each commit the reaction makes as the solver would receive it. Expected commits are
//! written from the source forms the bodies transcribe and from abi.md §5 and §6, never from
//! the printer's text.
#![cfg(unix)]

use std::sync::{Arc, Mutex, OnceLock};

use piperine_abi::caller::{CallError, Commit, Handle};
use piperine_abi::raw::{Cell, List, Module, Statement, cell, commit_flag, status, statement_flag};
use piperine_abi::raw::{ambient, part};
use piperine_ir::{BinaryOp, BodyId, CarrierRole, Drive, Env, Event, EventSource, Expr, Ir, IrBuilder, Origin, ValueType};
use piperine_plan::{Mode, Modules, Source};
use piperine_transpile::{Compiled, Settings, Transpiler};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "support/alone.rs"]
mod alone;

use alone::Alone;
use digital::{Buck, Dx, Ty};

const F: &str = "react.msl";

/// What the reaction was asked to commit: statement, bytes, delay.
type Made = (u32, Vec<u8>, f64);

/// The `Order` body (react.msl:1):
///
/// ```text
/// module Order(in clk: Bit, out a: Bit<u8>, out b: Bit<u8>, out c: Bit) {}
/// digital for Order {
///     on clk.rise() {
///         a <- 1;
///         if clk.high() { b <- 2; } else { b <- 3; }
///         c <- true after 1e-9;
///     }
/// }
/// ```
fn order(b: &mut IrBuilder) -> BodyId {
    let body = b.body(Buck::spec("Order", F, 1));
    for (name, ty, role, col) in [
        ("clk", ValueType::Bool, CarrierRole::In, 20),
        ("a", Ty::u(8), CarrierRole::Out, 30),
        ("b", Ty::u(8), CarrierRole::Out, 40),
        ("c", ValueType::Bool, CarrierRole::Out, 50),
    ] {
        b.carrier(body, Buck::carrier(name, ty, role, None, F, 1, col));
    }
    let rise = Buck::rise(b, body, 0, F, 3, 8);
    let mut x = Dx::new(b, F, 4, 9);
    let one = x.int(1, Ty::u(8));
    let first = x.commit(1, one, None);
    x.at(5, 9);
    let clk = x.carrier(0, ValueType::Bool);
    let high = x.high(clk);
    let two = x.int(2, Ty::u(8));
    let then = x.commit(2, two, None);
    let three = x.int(3, Ty::u(8));
    let otherwise = x.commit(2, three, None);
    let otherwise = x.block(vec![otherwise], None, ValueType::Unit);
    let branch = x.when(high, vec![then], Some(otherwise));
    let branch = x.stmt(branch);
    x.at(6, 9);
    let yes = x.boolean(true);
    let delay = x.real(1e-9);
    let last = x.commit(3, yes, Some(delay));
    Buck::reaction(b, body, vec![rise], vec![first, branch, last], vec![], F, 3);
    body
}

/// The `Guarded` body (react.msl:10): an `if` on an input that may be unknown.
///
/// ```text
/// on clk.rise() { r <- 9; if d { q <- true; } else { q <- false; } }   // the `if` at 12:9
/// ```
fn guarded(b: &mut IrBuilder) -> BodyId {
    let body = b.body(Buck::spec("Guarded", F, 10));
    for (name, ty, role, col) in [
        ("clk", ValueType::Bool, CarrierRole::In, 20),
        ("d", ValueType::Bool, CarrierRole::In, 30),
        ("q", ValueType::Bool, CarrierRole::Out, 40),
        ("r", Ty::u(8), CarrierRole::Out, 50),
    ] {
        b.carrier(body, Buck::carrier(name, ty, role, None, F, 10, col));
    }
    let rise = Buck::rise(b, body, 0, F, 11, 8);
    let mut x = Dx::new(b, F, 11, 20);
    let nine = x.int(9, Ty::u(8));
    let first = x.commit(3, nine, None);
    x.at(12, 9);
    let d = x.carrier(1, ValueType::Bool);
    let yes = x.boolean(true);
    let then = x.commit(2, yes, None);
    let no = x.boolean(false);
    let otherwise = x.commit(2, no, None);
    let otherwise = x.block(vec![otherwise], None, ValueType::Unit);
    let branch = x.when(d, vec![then], Some(otherwise));
    let branch = x.stmt(branch);
    Buck::reaction(b, body, vec![rise], vec![first, branch], vec![], F, 11);
    body
}

/// The `Late` body (react.msl:20): the delay is an input that may be unknown.
///
/// ```text
/// on clk.rise() { q <- true after t; }
/// ```
fn late(b: &mut IrBuilder) -> BodyId {
    let body = b.body(Buck::spec("Late", F, 20));
    for (name, ty, role, col) in [
        ("clk", ValueType::Bool, CarrierRole::In, 20),
        ("t", ValueType::F64, CarrierRole::In, 30),
        ("q", ValueType::Bool, CarrierRole::Out, 40),
    ] {
        b.carrier(body, Buck::carrier(name, ty, role, None, F, 20, col));
    }
    let rise = Buck::rise(b, body, 0, F, 21, 8);
    let mut x = Dx::new(b, F, 21, 20);
    let yes = x.boolean(true);
    let t = x.carrier(1, ValueType::F64);
    let commit = x.commit(2, yes, Some(t));
    Buck::reaction(b, body, vec![rise], vec![commit], vec![], F, 21);
    body
}

/// The `Seven` body (react.msl:30): an input read with a default.
///
/// ```text
/// on clk.rise() { out <- data.known_or(7); }
/// ```
fn seven(b: &mut IrBuilder) -> BodyId {
    let body = b.body(Buck::spec("Seven", F, 30));
    for (name, ty, role, col) in [
        ("clk", ValueType::Bool, CarrierRole::In, 20),
        ("data", Ty::u(8), CarrierRole::In, 30),
        ("out", Ty::u(8), CarrierRole::Out, 40),
    ] {
        b.carrier(body, Buck::carrier(name, ty, role, None, F, 30, col));
    }
    let rise = Buck::rise(b, body, 0, F, 31, 8);
    let mut x = Dx::new(b, F, 31, 20);
    let data = x.carrier(1, Ty::u(8));
    let default = x.int(7, Ty::u(8));
    let value = x.known_or(data, default, Ty::u(8));
    let commit = x.commit(2, value, None);
    Buck::reaction(b, body, vec![rise], vec![commit], vec![], F, 31);
    body
}

/// The `Divide` body (react.msl:40): a drive that divides integers.
///
/// ```text
/// q = a / b;
/// ```
fn divide(b: &mut IrBuilder) -> BodyId {
    let body = b.body(Buck::spec("Divide", F, 40));
    for (name, ty, role, col) in [
        ("a", Ty::u(8), CarrierRole::In, 20),
        ("b", Ty::u(8), CarrierRole::In, 30),
        ("q", Ty::u(8), CarrierRole::Out, 40),
    ] {
        b.carrier(body, Buck::carrier(name, ty, role, None, F, 40, col));
    }
    let mut x = Dx::new(b, F, 41, 9);
    let (a, c) = (x.carrier(0, Ty::u(8)), x.carrier(1, Ty::u(8)));
    let quotient = x.bin(BinaryOp::Div, a, c, Ty::flagged(Ty::u(8)));
    let origin = Origin { file: F.to_string(), line: 41, col: 5 };
    b.drive(body, Drive { carrier: 2, value: quotient, after: None, origin });
    body
}

/// The five bodies, compiled once for the binary: rustc is the cost of every test here.
struct Design {
    compiled: Compiled,
    ir: Arc<Ir>,
    _cache: tempfile::TempDir,
}

fn design() -> &'static Mutex<Design> {
    static DESIGN: OnceLock<Mutex<Design>> = OnceLock::new();
    DESIGN.get_or_init(|| {
        let mut b = IrBuilder::new();
        for make in [order, guarded, late, seven, divide] {
            make(&mut b);
        }
        let ir = Arc::new(Alone::finish(b));
        let cache = tempfile::tempdir().expect("a temp dir");
        let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
        let compiled = Transpiler::new(ir.clone())
            .unwrap_or_else(|e| panic!("{e}"))
            .compile(&settings)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(compiled.report().warnings, "", "the generated source compiles with no warning");
        Mutex::new(Design { compiled, ir, _cache: cache })
    })
}

/// The module of body `name`.
fn module(name: &str) -> &'static Module {
    let design = design().lock().unwrap_or_else(|e| e.into_inner());
    let body = design.ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body {name}")).id();
    Modules::module(&design.compiled, body).unwrap_or_else(|| panic!("`{name}` has no module"))
}

fn items<T: Copy>(list: &List<'_, T>) -> Vec<T> {
    (0..list.len()).map(|i| *list.get(i).expect("a readable element")).collect()
}

/// The bytes of a `Bit<T>` pin holding `value` (known), or unknown: its layout's size, value
/// first, the known byte after it (abi.md P3).
fn known(value: &[u8]) -> Vec<u8> {
    let mut bytes = value.to_vec();
    bytes.push(1);
    bytes
}

/// The bytes of a `Bit<f64>` pin holding `value`: the value, the known byte, padding to 16.
fn known_real(value: f64) -> Vec<u8> {
    let mut bytes = value.to_le_bytes().to_vec();
    bytes.push(1);
    bytes.resize(16, 0);
    bytes
}

/// An unwritten pin: every byte 0, known byte included (RS-11 AC 4).
fn unknown(size: usize) -> Vec<u8> {
    vec![0; size]
}

/// Runs reaction `k` of `handle` with each read cell given by the name of its pin; returns what
/// it committed in order, or the call's error. The reads a reaction declares are found in the
/// shape, so the test does not depend on their order.
fn react(handle: &mut Handle, k: u32, given: &[(&str, Vec<u8>)]) -> (Result<(), CallError>, Vec<Made>) {
    let (result, made) = react_flagged(handle, k, given);
    (result, made.into_iter().map(|(statement, bytes, after, _)| (statement, bytes, after)).collect())
}

/// What the reaction was asked to commit with the `commit_flag` bits it was made with (abi.md C7):
/// statement, bytes, delay, flags.
type MadeFlagged = (u32, Vec<u8>, f64, u32);

/// `react`, each commit with its flags.
fn react_flagged(handle: &mut Handle, k: u32, given: &[(&str, Vec<u8>)]) -> (Result<(), CallError>, Vec<MadeFlagged>) {
    let shape = handle.shape();
    let reaction = items(&shape.reactions)[k as usize];
    let mut input: Vec<Vec<u8>> = Vec::new();
    for c in items(&reaction.reads) {
        assert_eq!(c.kind, cell::PIN, "a reaction of these bodies reads pins");
        let name = items(&shape.pins)[c.index as usize].name.as_str().to_string();
        let bytes = given.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("the reaction reads `{name}`: not given"));
        input.push(bytes.1.clone());
    }
    let reads: Vec<&[u8]> = input.iter().map(Vec::as_slice).collect();
    let mut made: Vec<MadeFlagged> = Vec::new();
    let result = handle
        .react(k, &reads, &mut |c: Commit<'_>| {
            made.push((c.statement, c.bytes.to_vec(), c.after, c.flags));
            status::OK
        })
        .map(|_| ());
    (result, made)
}

fn instance(name: &str) -> Handle {
    Handle::create(module(name), &[]).unwrap_or_else(|e| panic!("create {name}: {e}"))
}

#[test]
fn a_reaction_commits_in_source_order() {
    // abi.md §6: statement `i` is the `i`-th commit of a depth-first walk, `then` before `else`:
    // a (0), b <- 2 (1), b <- 3 (2), c (3); the reaction is flagged `LATER` only where the delay
    // is a literal constant above 0
    let mut h = instance("Order");
    let shape = h.shape();
    let reaction = items(&shape.reactions)[0];
    let statements: Vec<Statement> = items(&reaction.writes);
    let pins: Vec<String> = statements.iter().map(|s| items(&shape.pins)[s.pin as usize].name.as_str().to_string()).collect();
    assert_eq!(pins, ["a", "b", "b", "c"], "one statement per commit, in source order");
    let later: Vec<(bool, f64)> = statements.iter().map(|s| (s.flags & statement_flag::LATER != 0, s.min_after)).collect();
    assert_eq!(later, [(false, 0.0), (false, 0.0), (false, 0.0), (true, 1e-9)]);

    // clk high: the `then` arm; every commit in the order the source makes them, the bytes of
    // each carrier's pin (value, known byte 1), the delay as written
    let (result, made) = react(&mut h, 0, &[("clk", known(&[1]))]);
    result.expect("the reaction runs");
    assert_eq!(
        made,
        [(0, known(&[1]), 0.0), (1, known(&[2]), 0.0), (3, known(&[1]), 1e-9)],
        "a <- 1; b <- 2 (statement 1); c <- true after 1e-9"
    );

    // clk low: the `else` arm is statement 2; `c` is still statement 3
    let (result, made) = react(&mut h, 0, &[("clk", known(&[0]))]);
    result.expect("the reaction runs");
    assert_eq!(made, [(0, known(&[1]), 0.0), (2, known(&[3]), 0.0), (3, known(&[1]), 1e-9)]);
}

#[test]
fn an_unknown_condition_merges_both_arms() {
    // US-04 AC 1, §3.3.3 (this replaces RS-11 AC 5, the fault of an unknown `if`, which the spec
    // removes): `if d { q <- true } else { q <- false }` with `d` unwritten runs both arms. Both
    // paths commit `q` at the next microstep, so the join is one key: `merge(true, false)`,
    // which is unknown, committed through each statement an arm used (1 and 2, so transport sees
    // both) flagged `MAYBE`; `r <- 9`, made before the `if`, is a plain commit
    let maybe = commit_flag::MAYBE;
    let mut h = instance("Guarded");
    let tick = known_real(1e-15);
    let (result, made) = react_flagged(&mut h, 0, &[("clk", known(&[1])), ("d", unknown(2)), ("resolution", tick.clone())]);
    result.expect("an unknown condition is no fault");
    assert_eq!(made, [(0, known(&[9]), 0.0, 0), (1, unknown(2), 0.0, maybe), (2, unknown(2), 0.0, maybe)]);

    // `d` known true commits `r <- 9` (statement 0) and `q <- true` (statement 1), no flag
    let (result, made) = react_flagged(&mut h, 0, &[("clk", known(&[1])), ("d", known(&[1])), ("resolution", tick)]);
    result.expect("a known condition runs");
    assert_eq!(made, [(0, known(&[9]), 0.0, 0), (1, known(&[1]), 0.0, 0)]);
}

#[test]
fn an_unknown_delay_commits_unknown_at_the_next_microstep() {
    // RS-11 AC 6, §3.3.3: `q <- true after t` with `t` unwritten commits `Unknown` (every byte
    // 0) after 0 seconds, the next microstep (abi.md §6)
    let mut h = instance("Late");
    let (result, made) = react(&mut h, 0, &[("clk", known(&[1])), ("t", unknown(16))]);
    result.expect("the reaction runs");
    assert_eq!(made, [(0, unknown(2), 0.0)]);

    // with `t` known the value is committed after it
    let (result, made) = react(&mut h, 0, &[("clk", known(&[1])), ("t", known_real(2.5e-9))]);
    result.expect("the reaction runs");
    assert_eq!(made, [(0, known(&[1]), 2.5e-9)]);
}

#[test]
fn a_known_or_over_an_unwritten_input_gives_the_default() {
    // RS-11 AC 3 and its independent test: `out <- data.known_or(7)` over an unwritten `data`
    // commits 7, known
    let mut h = instance("Seven");
    let (result, made) = react(&mut h, 0, &[("clk", known(&[1])), ("data", unknown(2))]);
    result.expect("the reaction runs");
    assert_eq!(made, [(0, known(&[7]), 0.0)]);
    // a written `data` is committed as it is
    let (result, made) = react(&mut h, 0, &[("clk", known(&[1])), ("data", known(&[5]))]);
    result.expect("the reaction runs");
    assert_eq!(made, [(0, known(&[5]), 0.0)]);
}

#[test]
fn a_panic_in_compiled_code_is_a_fault_and_the_instance_survives() {
    // RS-06 AC 6: `q = a / b` with `b` = 0 panics in the integer runtime (§1 rule 11); the ABI
    // thunk turns it into a `FAULT` carrying the message, and the next call on the same instance
    // gives `q = 8 / 2`
    let h = instance("Divide");
    let shape = h.shape();
    let function = items(&shape.functions)[0];
    let lens: Vec<usize> = items(&function.writes).iter().map(|w| {
        let pin = items(&shape.pins)[w.index as usize];
        pin.layout.size as usize
    }).collect();
    let call = |a: u8, b: u8| -> Result<Vec<u8>, CallError> {
        let reads: Vec<Vec<u8>> = items(&function.reads)
            .iter()
            .map(|c: &Cell| match items(&shape.pins)[c.index as usize].name.as_str() {
                "a" => known(&[a]),
                _ => known(&[b]),
            })
            .collect();
        let reads: Vec<&[u8]> = reads.iter().map(Vec::as_slice).collect();
        let mut out = vec![vec![0u8; lens[0]]];
        let mut writes: Vec<&mut [u8]> = out.iter_mut().map(Vec::as_mut_slice).collect();
        h.eval(0, &reads, &mut writes, &[], &mut [])?;
        Ok(out.remove(0))
    };
    let error = call(8, 0).expect_err("a division by zero is a fault");
    assert_eq!(error.status, status::FAULT);
    assert!(error.message.contains("division by zero"), "the fault carries the panic message: {}", error.message);
    assert_eq!(call(8, 2).expect("the instance survives"), known(&[4]));
}

#[test]
fn reactions_and_drives_read_the_environment_as_analog_code_does() {
    // §12.1.12: a reaction may read the environment; a drive's value may too. Both are wired as
    // `analog` code reads them: `temperature` through the global temperature pin's `VALUE`,
    // `gmin` and `source_scale` through the `GMIN` and `SOURCE_SCALE` ambients (verification
    // F-B6, user ruling 2026-11-15). Running such a reaction on the solver is
    // `tests/environment.rs` (C111):
    //
    //   module Ambient(in clk: Bit, out t: Bit<f64>, out g: Bit<f64>, out s: Bit<f64>, out pos: Bit) {}
    //   digital for Ambient {
    //       pos = gmin() > 0.0;                                                         // amb.msl:3
    //       on clk.changed() { t <- temperature(); g <- gmin(); s <- source_scale(); }  // amb.msl:4
    //   }
    const A: &str = "amb.msl";
    let mut b = IrBuilder::new();
    let body = b.body(Buck::spec("Ambient", A, 1));
    for (name, ty, role, col) in [
        ("clk", ValueType::Bool, CarrierRole::In, 20),
        ("t", ValueType::F64, CarrierRole::Out, 33),
        ("g", ValueType::F64, CarrierRole::Out, 50),
        ("s", ValueType::F64, CarrierRole::Out, 67),
        ("pos", ValueType::Bool, CarrierRole::Out, 84),
    ] {
        b.carrier(body, Buck::carrier(name, ty, role, None, A, 1, col));
    }
    let gmin = b.expr(Expr::Env(Env::Gmin), ValueType::F64, Buck::origin(A, 3, 11));
    let zero = b.expr(Expr::Real(piperine_ir::Real::new(0.0)), ValueType::F64, Buck::origin(A, 3, 20));
    let positive = b.expr(Expr::Binary { op: BinaryOp::Gt, lhs: gmin, rhs: zero }, ValueType::Bool, Buck::origin(A, 3, 11));
    b.drive(body, Drive { carrier: 4, value: positive, after: None, origin: Buck::origin(A, 3, 5) });
    let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Buck::origin(A, 4, 8) });
    let mut commits = Vec::new();
    for (carrier, env, col) in [(1, Env::Temperature, 29), (2, Env::Gmin, 50), (3, Env::SourceScale, 64)] {
        let value = b.expr(Expr::Env(env), ValueType::F64, Buck::origin(A, 4, col));
        commits.push(b.stmt(piperine_ir::Stmt::Commit { carrier, value, after: None }));
    }
    Buck::reaction(&mut b, body, vec![changed], commits, vec![], A, 4);
    let ir = Arc::new(Alone::finish(b));

    let transpiler = Transpiler::new(ir.clone()).unwrap_or_else(|e| panic!("{e}"));
    let plan = transpiler.plan(body).expect("the body runs");
    let shape = plan.shape().shape();
    let temperature = items(&shape.pins).iter().position(|p| p.name.as_str() == "temperature").expect("a temperature pin") as u32;
    let value = |kind: u32, index: u32| Cell { kind, index, part: part::VALUE };
    let (temperature_read, gmin_read, scale_read) =
        (value(cell::PIN, temperature), value(cell::AMBIENT, ambient::GMIN), value(cell::AMBIENT, ambient::SOURCE_SCALE));

    // the reaction reads the three, as the plan's sources and as the cells of its shape
    let reaction = &plan.reactions()[0];
    for source in [Source::Temperature, Source::Gmin, Source::SourceScale] {
        assert!(reaction.reads.contains(&source), "the reaction reads {source:?}: {:?}", reaction.reads);
    }
    let reads = items(&items(&shape.reactions)[0].reads);
    for read in [temperature_read, gmin_read, scale_read] {
        assert!(reads.contains(&read), "the reaction's shape reads {read:?}: {reads:?}");
    }
    // the drive of `pos` reads `GMIN`
    let k = plan.functions().iter().position(|f| f.mode == Mode::Drive(0)).expect("the drive's function");
    assert!(plan.functions()[k].reads.contains(&Source::Gmin), "{:?}", plan.functions()[k].reads);
    assert!(items(&items(&shape.functions)[k].reads).contains(&gmin_read));

    // the body compiles with no warning, and its library's shape is the plan's (`compile` checks it)
    let cache = tempfile::tempdir().expect("a temp dir");
    let compiled = transpiler
        .compile(&Settings { rustc: None, cache: cache.path().to_path_buf() })
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(compiled.report().warnings, "", "the generated source compiles with no warning");
    assert!(Modules::module(&compiled, body).is_some(), "the body has a module");
}
