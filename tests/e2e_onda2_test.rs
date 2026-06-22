//! Wave 2 language features: user-defined functions (2a), arrays/queues (2b),
//! and the `inside` operator (2c).

use std::sync::{Arc, Mutex};

use piperine_circuit::elaboration::ElaborationResult;
use piperine_circuit::registry::HardwareRegistry;
use piperine_common::EventAction;
use piperine_interpreter::backend::{AnalysisEvent, SimulatorBackend};
use piperine_interpreter::error::InterpreterError;
use piperine_interpreter::value::Value;
use piperine_interpreter::{Interpreter, Scope, SystemTaskRegistry};
use piperine_parser::parser::parse;

struct NoopBackend {
    output: Arc<Mutex<Vec<String>>>,
}
impl NoopBackend {
    fn new() -> Self { Self { output: Arc::new(Mutex::new(Vec::new())) } }
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

/// Run a full module (functions + initial block) and return the final scope.
fn run(src: &str) -> Scope {
    let result = parse_and_elaborate(src);
    let tasks = SystemTaskRegistry::default();
    let mut backend = NoopBackend::new();
    let mut interp = Interpreter::new(&mut backend, &tasks);
    interp.set_functions(result.functions);
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

// ── 2a: user-defined functions ────────────────────────────────────────────────

#[test]
fn test_function_va_return_convention() {
    // Verilog-A style: assign to a variable named after the function.
    let s = run(r#"
module tb;
    function real square(input real x);
        square = x * x;
    endfunction
    initial begin
        real y;
        y = square(3.0);
    end
endmodule
"#);
    assert_eq!(real(&s, "y"), 9.0);
}

#[test]
fn test_function_explicit_return() {
    let s = run(r#"
module tb;
    function real cube(input real x);
        return x * x * x;
    endfunction
    initial begin
        real y;
        y = cube(2.0);
    end
endmodule
"#);
    assert_eq!(real(&s, "y"), 8.0);
}

#[test]
fn test_function_multi_arg() {
    let s = run(r#"
module tb;
    function real wsum(input real a, input real b);
        return 2.0 * a + 3.0 * b;
    endfunction
    initial begin
        real y;
        y = wsum(5.0, 10.0);
    end
endmodule
"#);
    assert_eq!(real(&s, "y"), 40.0); // 10 + 30
}

#[test]
fn test_function_recursion() {
    let s = run(r#"
module tb;
    function integer fact(input integer n);
        if (n <= 1) return 1;
        else return n * fact(n - 1);
    endfunction
    initial begin
        integer y;
        y = fact(5);
    end
endmodule
"#);
    assert_eq!(real(&s, "y"), 120.0);
}

#[test]
fn test_function_locals_and_loop() {
    // Local var + loop inside a function; caller scope is untouched.
    let s = run(r#"
module tb;
    function real sumto(input integer n);
        integer i;
        real acc;
        acc = 0.0;
        for (i = 1; i <= n; i++) acc += i;
        sumto = acc;
    endfunction
    initial begin
        real y;
        y = sumto(4);
    end
endmodule
"#);
    assert_eq!(real(&s, "y"), 10.0); // 1+2+3+4
}

#[test]
fn test_function_calls_stdlib_math() {
    let s = run(r#"
module tb;
    function real norm(input real a, input real b);
        return $sqrt(a*a + b*b);
    endfunction
    initial begin
        real y;
        y = norm(3.0, 4.0);
    end
endmodule
"#);
    assert!((real(&s, "y") - 5.0).abs() < 1e-12);
}
