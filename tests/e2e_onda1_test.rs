//! Wave 1 language features: brace blocks, ++/--/compound assign,
//! break/continue/return, repeat/forever, and math system functions.
//!
//! Each test parses a testbench `initial` block, runs it over a no-op backend,
//! and inspects the resulting scope (variables declared in the block land in the
//! flat scope) or captured `$display` output.

use std::sync::{Arc, Mutex};

use piperine_circuit::elaboration::ElaborationResult;
use piperine_circuit::registry::HardwareRegistry;
use piperine_common::EventAction;
use piperine_interpreter::backend::{AnalysisEvent, SimulatorBackend};
use piperine_interpreter::error::InterpreterError;
use piperine_interpreter::value::Value;
use piperine_interpreter::{Interpreter, Scope, SystemTaskRegistry};
use piperine_parser::parser::parse;

// ── Minimal backend (math/control-flow tests never touch the simulator) ───────

struct NoopBackend {
    output: Arc<Mutex<Vec<String>>>,
}
impl NoopBackend {
    fn new() -> Self { Self { output: Arc::new(Mutex::new(Vec::new())) } }
    fn lines(&self) -> Vec<String> { self.output.lock().unwrap().clone() }
}
impl SimulatorBackend for NoopBackend {
    fn load_circuit(&mut self, _: &[String]) -> Result<(), InterpreterError> { Ok(()) }
    fn run_command(&mut self, _: &str) -> Result<(), InterpreterError> { Ok(()) }
    fn get_vector(&mut self, _: &str) -> Result<Vec<f64>, InterpreterError> { Ok(vec![]) }
    fn list_vectors(&mut self, _: &str) -> Result<Vec<String>, InterpreterError> { Ok(vec![]) }
    fn print(&self, line: &str) { self.output.lock().unwrap().push(line.to_string()); }
    fn start_analysis(&mut self, _: &str, _: bool) -> Result<(), InterpreterError> { Ok(()) }
    fn poll_analysis(&mut self) -> Result<AnalysisEvent, InterpreterError> {
        Ok(AnalysisEvent::Done { plot_name: "noop".into(), had_run_errors: false })
    }
    fn respond_to_analysis_event(&mut self, _: EventAction) -> Result<(), InterpreterError> { Ok(()) }
}

fn parse_and_elaborate(src: &str) -> ElaborationResult {
    let doc = parse(src).expect("parse failed");
    let reg = HardwareRegistry::new();
    piperine_circuit::elaboration::elaborate(&doc, &reg).expect("elaborate failed")
}

/// Run an `initial` block and return its final scope.
fn run(src: &str) -> Scope {
    let result = parse_and_elaborate(src);
    let tasks = SystemTaskRegistry::default();
    let mut backend = NoopBackend::new();
    let mut interp = Interpreter::new(&mut backend, &tasks);
    let mut scope = Scope::default();
    interp.exec(&result.initial_statement, &mut scope).expect("exec failed");
    scope
}

fn real(scope: &Scope, name: &str) -> f64 {
    match scope.get(name) {
        Some(Value::Real(v))    => *v,
        Some(Value::Integer(i)) => *i as f64,
        other => panic!("variable `{name}` is {other:?}, expected numeric"),
    }
}

fn module(body: &str) -> String {
    format!("module tb;\n  initial begin\n    real x; integer n;\n{body}\n  end\nendmodule\n")
}

// ── Compound assignment & ++/-- ───────────────────────────────────────────────

#[test]
fn test_compound_assign() {
    let s = run(&module("x = 10.0; x += 5.0; x -= 2.0; x *= 3.0; x /= 2.0;"));
    // ((10+5-2)*3)/2 = 19.5
    assert!((real(&s, "x") - 19.5).abs() < 1e-9, "x={}", real(&s, "x"));
}

#[test]
fn test_inc_dec() {
    let s = run(&module("n = 5; n++; n++; n--;"));
    assert_eq!(real(&s, "n"), 6.0);
}

#[test]
fn test_mod_assign() {
    let s = run(&module("n = 17; n %= 5;"));
    assert_eq!(real(&s, "n"), 2.0);
}

// ── Brace blocks interchangeable with begin/end ───────────────────────────────

#[test]
fn test_brace_block_initial() {
    // Whole initial body uses braces instead of begin/end.
    let src = "module tb;\n  initial {\n    real x;\n    x = 7.0;\n  }\nendmodule\n";
    let s = run(src);
    assert_eq!(real(&s, "x"), 7.0);
}

#[test]
fn test_brace_and_begin_mixed() {
    // begin/end outer, brace inner (an if-body), both in one testbench.
    let src = "module tb;\n  initial begin\n    real x; x = 0.0;\n    if (1) {\n      x = 42.0;\n    }\n  end\nendmodule\n";
    let s = run(src);
    assert_eq!(real(&s, "x"), 42.0);
}

// ── break / continue ──────────────────────────────────────────────────────────

#[test]
fn test_for_break() {
    // Sum 0..9 but break at 5 → 0+1+2+3+4 = 10.
    let s = run(&module(
        "x = 0.0; for (n = 0; n < 10; n++) begin if (n == 5) break; x += n; end"));
    assert_eq!(real(&s, "x"), 10.0);
}

#[test]
fn test_for_continue() {
    // Sum odd numbers 0..9: skip evens → 1+3+5+7+9 = 25.
    let s = run(&module(
        "x = 0.0; for (n = 0; n < 10; n++) { if (n % 2 == 0) continue; x += n; }"));
    assert_eq!(real(&s, "x"), 25.0);
}

#[test]
fn test_while_break() {
    let s = run(&module(
        "x = 0.0; n = 0; while (1) begin x += 1.0; n++; if (n == 3) break; end"));
    assert_eq!(real(&s, "x"), 3.0);
}

// ── return ────────────────────────────────────────────────────────────────────

#[test]
fn test_return_stops_block() {
    // return halts the initial block; x stays 1, never reaches 2.
    let s = run(&module("x = 1.0; return; x = 2.0;"));
    assert_eq!(real(&s, "x"), 1.0);
}

// ── repeat / forever ──────────────────────────────────────────────────────────

#[test]
fn test_repeat() {
    let s = run(&module("x = 0.0; repeat (4) x += 2.5;"));
    assert_eq!(real(&s, "x"), 10.0);
}

#[test]
fn test_forever_with_break() {
    let s = run(&module(
        "x = 0.0; n = 0; forever begin n++; x += 1.0; if (n >= 7) break; end"));
    assert_eq!(real(&s, "x"), 7.0);
}

// ── Math system functions ─────────────────────────────────────────────────────

#[test]
fn test_math_unary() {
    let s = run(&module("x = $sqrt(16.0);"));
    assert!((real(&s, "x") - 4.0).abs() < 1e-12);

    let s = run(&module("x = $ln($exp(1.0));"));
    assert!((real(&s, "x") - 1.0).abs() < 1e-12);

    let s = run(&module("x = $floor(3.7) + $ceil(3.2);"));
    assert_eq!(real(&s, "x"), 7.0); // 3 + 4
}

#[test]
fn test_math_binary() {
    let s = run(&module("x = $pow(2.0, 10.0);"));
    assert_eq!(real(&s, "x"), 1024.0);

    let s = run(&module("x = $hypot(3.0, 4.0);"));
    assert!((real(&s, "x") - 5.0).abs() < 1e-12);
}

#[test]
fn test_clog2() {
    for (n, expect) in [(1, 0), (2, 1), (4, 2), (5, 3), (8, 3), (256, 8)] {
        let s = run(&module(&format!("n = $clog2({n});")));
        assert_eq!(real(&s, "n"), expect as f64, "clog2({n})");
    }
}

// ── Combined: a small parametric sweep loop ───────────────────────────────────

#[test]
fn test_sweep_loop_realistic() {
    // Accumulate sqrt(i) for i in 1..=4, using ++ and += and a brace body.
    let s = run(&module(
        "x = 0.0; for (n = 1; n <= 4; ++n) { x += $sqrt(n); }"));
    let expect = 1.0_f64.sqrt() + 2.0_f64.sqrt() + 3.0_f64.sqrt() + 4.0_f64.sqrt();
    assert!((real(&s, "x") - expect).abs() < 1e-9, "x={}", real(&s, "x"));
}
