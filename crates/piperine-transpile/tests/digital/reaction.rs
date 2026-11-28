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
use piperine_abi::device::Access;
use piperine_abi::raw::{Cell, List, Module, Statement, cell, status, statement_flag};
use piperine_abi::raw::{ambient, part};
use piperine_ir::{BinaryOp, BodyId, CarrierRole, Drive, Env, Event, EventSource, Expr, Ir, IrBuilder, MathFn, Origin, ValueType};
use piperine_plan::{Mode, Modules, Source};
use piperine_transpile::{Compiled, Settings, Transpiler};

use crate::pin_bytes::PinBytes;
use piperine_ir_fixtures::digital;
use piperine_ir_fixtures::alone;

use alone::Alone;
use digital::{Buck, Dx, ValueAliases};

const F: &str = "react.msl";

/// What the reaction was asked to commit: statement, bytes, delay.
type Made = (u32, Vec<u8>, f64);

/// The bodies whose reactions the tests run.
struct ReactionBodies;

impl ReactionBodies {
    /// The `Order` body (react.msl:1):
    ///
    /// ```text
    /// module Order(in clk: Bit, out a: Bit<u8>, out b: Bit<u8>, out c: Bit) {}
    /// digital for Order {
    ///     on clk.rise() {
    ///         a <- 1;
    ///         if clk { b <- 2; } else { b <- 3; }
    ///         c <- true after 1e-9;
    ///     }
    /// }
    /// ```
    fn order(b: &mut IrBuilder) -> BodyId {
        let body = b.body(Buck::spec("Order", F, 1));
        for (name, ty, role, col) in [
            ("clk", ValueType::Bool, CarrierRole::In, 20),
            ("a", ValueAliases::u(8), CarrierRole::Out, 30),
            ("b", ValueAliases::u(8), CarrierRole::Out, 40),
            ("c", ValueType::Bool, CarrierRole::Out, 50),
        ] {
            b.carrier(body, Buck::carrier(name, ty, role, None, F, 1, col));
        }
        let rise = Buck::rise(b, body, 0, F, 3, 8);
        let mut x = Dx::new(b, F, 4, 9);
        let one = x.int(1, ValueAliases::u(8));
        let first = x.commit(1, one, None);
        x.at(5, 9);
        let clk = x.carrier(0, ValueType::Bool);
        let two = x.int(2, ValueAliases::u(8));
        let then = x.commit(2, two, None);
        let three = x.int(3, ValueAliases::u(8));
        let otherwise = x.commit(2, three, None);
        let otherwise = x.block(vec![otherwise], None, ValueType::Unit);
        let branch = x.when(clk, vec![then], Some(otherwise));
        let branch = x.stmt(branch);
        x.at(6, 9);
        let yes = x.boolean(true);
        let delay = x.real(1e-9);
        let last = x.commit(3, yes, Some(delay));
        Buck::reaction(b, body, vec![rise], vec![first, branch, last], vec![], F, 3);
        body
    }

    /// The `Tables` body (react.msl:60): `same`, `cast` and a math function, committed through one
    /// statement each.
    ///
    /// ```text
    /// on clk.rise() {
    ///     sm <- same(a, b);
    ///     cs <- cast::<u8, i8>(a);
    ///     ex <- x.exp();
    /// }
    /// ```
    fn tables(b: &mut IrBuilder) -> BodyId {
        let body = b.body(Buck::spec("Tables", F, 60));
        for (name, ty, role, col) in [
            ("clk", ValueType::Bool, CarrierRole::In, 20),
            ("a", ValueAliases::u(8), CarrierRole::In, 21),
            ("b", ValueAliases::u(8), CarrierRole::In, 22),
            ("x", ValueType::F64, CarrierRole::In, 23),
            ("sm", ValueType::Bool, CarrierRole::Out, 24),
            ("cs", ValueAliases::i(8), CarrierRole::Out, 25),
            ("ex", ValueType::F64, CarrierRole::Out, 26),
        ] {
            b.carrier(body, Buck::carrier(name, ty, role, None, F, 60, col));
        }
        let rise = Buck::rise(b, body, 0, F, 61, 8);
        let mut x = Dx::new(b, F, 62, 9);
        let mut stmts = Vec::new();
        let (a, bb) = (x.carrier(1, ValueAliases::u(8)), x.carrier(2, ValueAliases::u(8)));
        let same = x.same(a, bb);
        stmts.push(x.commit(4, same, None));
        let recast = x.level_cast(a, ValueAliases::i(8), None, ValueType::Option(Box::new(ValueAliases::i(8))));
        let recast = x.option_value(recast, ValueAliases::i(8));
        stmts.push(x.commit(5, recast, None));
        let v = x.carrier(3, ValueType::F64);
        let exp = x.math(MathFn::Exp, v, ValueType::F64);
        stmts.push(x.commit(6, exp, None));
        Buck::reaction(b, body, vec![rise], stmts, vec![], F, 61);
        body
    }

    /// The `Guarded` body (react.msl:10): an `if` on an input.
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
            ("r", ValueAliases::u(8), CarrierRole::Out, 50),
        ] {
            b.carrier(body, Buck::carrier(name, ty, role, None, F, 10, col));
        }
        let rise = Buck::rise(b, body, 0, F, 11, 8);
        let mut x = Dx::new(b, F, 11, 20);
        let nine = x.int(9, ValueAliases::u(8));
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

    /// The `Late` body (react.msl:20): the delay is an input.
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

    /// The `Divide` body (react.msl:40): a drive that divides integers.
    ///
    /// ```text
    /// q = a / b;
    /// ```
    fn divide(b: &mut IrBuilder) -> BodyId {
        let body = b.body(Buck::spec("Divide", F, 40));
        for (name, ty, role, col) in [
            ("a", ValueAliases::u(8), CarrierRole::In, 20),
            ("b", ValueAliases::u(8), CarrierRole::In, 30),
            ("q", ValueAliases::u(8), CarrierRole::Out, 40),
        ] {
            b.carrier(body, Buck::carrier(name, ty, role, None, F, 40, col));
        }
        let mut x = Dx::new(b, F, 41, 9);
        let (a, c) = (x.carrier(0, ValueAliases::u(8)), x.carrier(1, ValueAliases::u(8)));
        let quotient = x.bin(BinaryOp::Div, a, c, ValueAliases::u(8));
        let origin = Origin { file: F.to_string(), line: 41, col: 5 };
        b.drive(body, Drive { carrier: 2, value: quotient, after: None, origin, element: None, path: vec![] });
        body
    }

    /// The `Settling` body (react.msl:50): a `settle` event gated by a guard (§13 rule 17).
    ///
    /// ```text
    /// on settle && ready { q <- true; }
    /// ```
    fn settling(b: &mut IrBuilder) -> BodyId {
        let body = b.body(Buck::spec("Settling", F, 50));
        for (name, role, col) in [("ready", CarrierRole::In, 20), ("q", CarrierRole::Out, 40)] {
            b.carrier(body, Buck::carrier(name, ValueType::Bool, role, None, F, 50, col));
        }
        let mut x = Dx::new(b, F, 51, 20);
        let ready = x.carrier(0, ValueType::Bool);
        let settle = b.event(body, Event { source: EventSource::Settle, guard: Some(ready), origin: Buck::origin(F, 51, 8) });
        let yes = Dx::new(b, F, 52, 9).boolean(true);
        let commit = Dx::new(b, F, 52, 9).commit(1, yes, None);
        Buck::reaction(b, body, vec![settle], vec![commit], vec![], F, 51);
        body
    }
}

/// The compiled design and the reactions and event functions run on it.
struct ReactionRun;

impl ReactionRun {
    fn design() -> &'static Mutex<Design> {
        static DESIGN: OnceLock<Mutex<Design>> = OnceLock::new();
        DESIGN.get_or_init(|| {
            let mut b = IrBuilder::new();
            for make in [ReactionBodies::order, ReactionBodies::guarded, ReactionBodies::late, ReactionBodies::divide, ReactionBodies::tables, ReactionBodies::settling] {
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
        let design = ReactionRun::design().lock().unwrap_or_else(|e| e.into_inner());
        let body = design.ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body {name}")).id();
        Modules::module(&design.compiled, body).unwrap_or_else(|| panic!("`{name}` has no module"))
    }

    fn items<T: Copy>(list: &List<'_, T>) -> Vec<T> {
        (0..list.len()).map(|i| *list.get(i).expect("a readable element")).collect()
    }

    /// Runs reaction `k` of `handle` with each read cell given by the name of its pin; returns what
    /// it committed in order, or the call's error. The reads a reaction declares are found in the
    /// shape, so the test does not depend on their order.
    fn react(handle: &mut Handle, k: u32, given: &[(&str, Vec<u8>)]) -> (Result<(), CallError>, Vec<Made>) {
        let shape = handle.shape();
        let reaction = ReactionRun::items(&shape.reactions)[k as usize];
        let mut input: Vec<Vec<u8>> = Vec::new();
        for c in ReactionRun::items(&reaction.reads) {
            assert_eq!(c.kind, cell::PIN, "a reaction of these bodies reads pins");
            let name = ReactionRun::items(&shape.pins)[c.index as usize].name.as_str().to_string();
            let bytes = given.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("the reaction reads `{name}`: not given"));
            input.push(bytes.1.clone());
        }
        let reads: Vec<&[u8]> = input.iter().map(Vec::as_slice).collect();
        let mut made: Vec<Made> = Vec::new();
        let result = handle
            .react(k, &reads, &mut |c: Commit<'_>| {
                made.push((c.statement, c.bytes.to_vec(), c.after));
                status::OK
            })
            .map(|_| ());
        (result, made)
    }

    fn instance(name: &str) -> Handle {
        Handle::create(ReactionRun::module(name), &[]).unwrap_or_else(|e| panic!("create {name}: {e}"))
    }

    /// Runs the event function `name` of `handle`, each cell it reads given by the name of its pin and
    /// its part (`CHANGED` is one `bool` byte, a carrier's `VALUE` its pin bytes); returns the
    /// `CHANGED` byte of the event pin it writes.
    fn run_event(handle: &Handle, name: &str, given: &[(&str, u32, Vec<u8>)]) -> u8 {
        let shape = handle.shape();
        let k = ReactionRun::items(&shape.functions).iter().position(|f| f.name.as_str() == name).unwrap_or_else(|| panic!("no function `{name}`"));
        let function = ReactionRun::items(&shape.functions)[k];
        let pins = ReactionRun::items(&shape.pins);
        let input: Vec<Vec<u8>> = ReactionRun::items(&function.reads)
            .iter()
            .map(|c| {
                let pin = pins[c.index as usize].name.as_str();
                let (_, _, bytes) = given.iter().find(|(n, part, _)| *n == pin && *part == c.part).unwrap_or_else(|| panic!("the function reads `{pin}`: not given"));
                bytes.clone()
            })
            .collect();
        let reads: Vec<&[u8]> = input.iter().map(Vec::as_slice).collect();
        let lens = shape.cell_lens(&function.writes, Access::FunctionWrite).expect("write lens");
        let mut cells: Vec<Vec<u8>> = lens.iter().map(|len| vec![0u8; *len]).collect();
        let mut writes: Vec<&mut [u8]> = cells.iter_mut().map(Vec::as_mut_slice).collect();
        handle.eval(k as u32, &reads, &mut writes, &[], &mut []).unwrap_or_else(|e| panic!("{e}"));
        cells[0][0]
    }
}

/// The bodies, compiled once for the binary: rustc is the cost of every test here.
struct Design {
    compiled: Compiled,
    ir: Arc<Ir>,
    _cache: tempfile::TempDir,
}

#[test]
fn a_reaction_commits_in_source_order() {
    // abi.md §6: statement `i` is the `i`-th commit of a depth-first walk, `then` before `else`:
    // a (0), b <- 2 (1), b <- 3 (2), c (3); the reaction is flagged `LATER` only where the delay
    // is a literal constant above 0
    let mut h = ReactionRun::instance("Order");
    let shape = h.shape();
    let reaction = ReactionRun::items(&shape.reactions)[0];
    let statements: Vec<Statement> = ReactionRun::items(&reaction.writes);
    let pins: Vec<String> = statements.iter().map(|s| ReactionRun::items(&shape.pins)[s.pin as usize].name.as_str().to_string()).collect();
    assert_eq!(pins, ["a", "b", "b", "c"], "one statement per commit, in source order");
    let later: Vec<(bool, f64)> = statements.iter().map(|s| (s.flags & statement_flag::LATER != 0, s.min_after)).collect();
    assert_eq!(later, [(false, 0.0), (false, 0.0), (false, 0.0), (true, 1e-9)]);

    // clk high: the `then` arm; every commit in the order the source makes them, the bytes of
    // each carrier's pin (its value), the delay as written
    let (result, made) = ReactionRun::react(&mut h, 0, &[("clk", PinBytes::pin_bytes(&[1]))]);
    result.expect("the reaction runs");
    assert_eq!(
        made,
        [(0, PinBytes::pin_bytes(&[1]), 0.0), (1, PinBytes::pin_bytes(&[2]), 0.0), (3, PinBytes::pin_bytes(&[1]), 1e-9)],
        "a <- 1; b <- 2 (statement 1); c <- true after 1e-9"
    );

    // clk low: the `else` arm is statement 2; `c` is still statement 3
    let (result, made) = ReactionRun::react(&mut h, 0, &[("clk", PinBytes::pin_bytes(&[0]))]);
    result.expect("the reaction runs");
    assert_eq!(made, [(0, PinBytes::pin_bytes(&[1]), 0.0), (2, PinBytes::pin_bytes(&[3]), 0.0), (3, PinBytes::pin_bytes(&[1]), 1e-9)]);
}

#[test]
fn an_if_commits_through_the_arm_it_takes() {
    // `if d { q <- true } else { q <- false }`: one path, the arm the condition names; `r <- 9`,
    // made before the `if`, is statement 0
    let mut h = ReactionRun::instance("Guarded");
    let (result, made) = ReactionRun::react(&mut h, 0, &[("clk", PinBytes::pin_bytes(&[1])), ("d", PinBytes::pin_bytes(&[1]))]);
    result.expect("the reaction runs");
    assert_eq!(made, [(0, PinBytes::pin_bytes(&[9]), 0.0), (1, PinBytes::pin_bytes(&[1]), 0.0)]);
    let (result, made) = ReactionRun::react(&mut h, 0, &[("clk", PinBytes::pin_bytes(&[1])), ("d", PinBytes::pin_bytes(&[0]))]);
    result.expect("the reaction runs");
    assert_eq!(made, [(0, PinBytes::pin_bytes(&[9]), 0.0), (2, PinBytes::pin_bytes(&[0]), 0.0)], "the `else` arm is statement 2");
}

#[test]
fn same_cast_and_math_compute_their_tables() {
    // §22.1.3, §22.1.4 and §3.3.2, one commit per form, statements 0 to 2: `sm`, `cs`, `ex`.
    // a = 5 (0b101), b = 6, x = 0.0: `same` of different values is false; the cast keeps the
    // bits, so 5 as an `i8`; `exp(0.0)` is 1.0
    let mut h = ReactionRun::instance("Tables");
    let mut run = |a: u8, b: u8, x: f64| {
        let (result, made) = ReactionRun::react(&mut h, 0, &[("clk", PinBytes::pin_bytes(&[1])), ("a", PinBytes::pin_bytes(&[a])), ("b", PinBytes::pin_bytes(&[b])), ("x", PinBytes::real_pin_bytes(x))]);
        result.expect("the reaction runs");
        made.into_iter().map(|(statement, bytes, _)| (statement, bytes)).collect::<Vec<_>>()
    };
    assert_eq!(run(5, 6, 0.0), [(0, PinBytes::pin_bytes(&[0])), (1, PinBytes::pin_bytes(&[5])), (2, PinBytes::real_pin_bytes(1.0))]);
    // equal values are the same (§22.1.4); a byte above 127 is negative as an `i8`
    assert_eq!(run(200, 200, 0.0), [(0, PinBytes::pin_bytes(&[1])), (1, PinBytes::pin_bytes(&[200])), (2, PinBytes::real_pin_bytes(1.0))]);
}

#[test]
fn a_commit_is_made_after_the_delay_it_names() {
    // `q <- true after t` commits `true` after `t` seconds
    let mut h = ReactionRun::instance("Late");
    let (result, made) = ReactionRun::react(&mut h, 0, &[("clk", PinBytes::pin_bytes(&[1])), ("t", PinBytes::real_pin_bytes(2.5e-9))]);
    result.expect("the reaction runs");
    assert_eq!(made, [(0, PinBytes::pin_bytes(&[1]), 2.5e-9)]);
    let (result, made) = ReactionRun::react(&mut h, 0, &[("clk", PinBytes::pin_bytes(&[1])), ("t", PinBytes::real_pin_bytes(0.0))]);
    result.expect("the reaction runs");
    assert_eq!(made, [(0, PinBytes::pin_bytes(&[1]), 0.0)], "a delay of 0 is the next microstep");
}

#[test]
fn a_panic_in_compiled_code_is_a_fault_and_the_instance_survives() {
    // RS-06 AC 6: `q = a / b` with `b` = 0 panics in the integer runtime (§1 rule 11); the ABI
    // thunk turns it into a `FAULT` carrying the message, and the next call on the same instance
    // gives `q = 8 / 2`
    let h = ReactionRun::instance("Divide");
    let shape = h.shape();
    let function = ReactionRun::items(&shape.functions)[0];
    let lens: Vec<usize> = ReactionRun::items(&function.writes).iter().map(|w| {
        let pin = ReactionRun::items(&shape.pins)[w.index as usize];
        pin.layout.size as usize
    }).collect();
    let call = |a: u8, b: u8| -> Result<Vec<u8>, CallError> {
        let reads: Vec<Vec<u8>> = ReactionRun::items(&function.reads)
            .iter()
            .map(|c: &Cell| match ReactionRun::items(&shape.pins)[c.index as usize].name.as_str() {
                "a" => PinBytes::pin_bytes(&[a]),
                _ => PinBytes::pin_bytes(&[b]),
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
    assert_eq!(call(8, 2).expect("the instance survives"), PinBytes::pin_bytes(&[4]));
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
    b.drive(body, Drive { carrier: 4, value: positive, after: None, origin: Buck::origin(A, 3, 5), element: None, path: vec![] });
    let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Buck::origin(A, 4, 8) });
    let mut commits = Vec::new();
    for (carrier, env, col) in [(1, Env::Temperature, 29), (2, Env::Gmin, 50), (3, Env::SourceScale, 64)] {
        let value = b.expr(Expr::Env(env), ValueType::F64, Buck::origin(A, 4, col));
        commits.push(b.stmt(piperine_ir::Stmt::Commit { carrier, value, after: None, target: None }));
    }
    Buck::reaction(&mut b, body, vec![changed], commits, vec![], A, 4);
    let ir = Arc::new(Alone::finish(b));

    let transpiler = Transpiler::new(ir.clone()).unwrap_or_else(|e| panic!("{e}"));
    let plan = transpiler.plan(body).expect("the body runs");
    let shape = plan.shape().shape();
    let temperature = ReactionRun::items(&shape.pins).iter().position(|p| p.name.as_str() == "temperature").expect("a temperature pin") as u32;
    let value = |kind: u32, index: u32| Cell { kind, index, part: part::VALUE };
    let (temperature_read, gmin_read, scale_read) =
        (value(cell::PIN, temperature), value(cell::AMBIENT, ambient::GMIN), value(cell::AMBIENT, ambient::SOURCE_SCALE));

    // the reaction reads the three, as the plan's sources and as the cells of its shape
    let reaction = &plan.reactions()[0];
    for source in [Source::Temperature, Source::Gmin, Source::SourceScale] {
        assert!(reaction.reads.contains(&source), "the reaction reads {source:?}: {:?}", reaction.reads);
    }
    let reads = ReactionRun::items(&ReactionRun::items(&shape.reactions)[0].reads);
    for read in [temperature_read, gmin_read, scale_read] {
        assert!(reads.contains(&read), "the reaction's shape reads {read:?}: {reads:?}");
    }
    // the drive of `pos` reads `GMIN`
    let k = plan.functions().iter().position(|f| f.mode == Mode::Drive(0)).expect("the drive's function");
    assert!(plan.functions()[k].reads.contains(&Source::Gmin), "{:?}", plan.functions()[k].reads);
    assert!(ReactionRun::items(&ReactionRun::items(&shape.functions)[k].reads).contains(&gmin_read));

    // the body compiles with no warning, and its library's shape is the plan's (`compile` checks it)
    let cache = tempfile::tempdir().expect("a temp dir");
    let compiled = transpiler
        .compile(&Settings { rustc: None, cache: cache.path().to_path_buf() })
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(compiled.report().warnings, "", "the generated source compiles with no warning");
    assert!(Modules::module(&compiled, body).is_some(), "the body has a module");
}

#[test]
fn a_settle_event_function_fires_on_the_settle_pin_and_its_guard() {
    // §13 rule 17 and rule 1: the event of `settle && ready` fires in a microstep exactly
    // when the `SETTLE` pin fired (its `CHANGED` byte, abi.md §4) and the guard holds there
    let handle = ReactionRun::instance("Settling");
    let fire = |settle: u8, ready_high: bool| {
        let ready = PinBytes::pin_bytes(&[u8::from(ready_high)]);
        ReactionRun::run_event(&handle, "event:0", &[("ready", part::VALUE, ready), ("settle", part::CHANGED, vec![settle])])
    };
    assert_eq!(fire(1, true), 1, "the settle phase, the guard holds");
    assert_eq!(fire(1, false), 0, "the settle phase, the guard is false");
    assert_eq!(fire(0, true), 0, "an ordinary microstep: SETTLE did not fire");
    assert_eq!(fire(0, false), 0);
}
