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

// ── 2b: arrays / queues ───────────────────────────────────────────────────────

fn mod2b(body: &str) -> String {
    format!("module tb;\n  initial begin\n    real x; integer n; real q;\n{body}\n  end\nendmodule\n")
}

#[test]
fn test_array_literal_size_index() {
    let s = run(&mod2b("q = '{10.0, 20.0, 30.0}; n = q.size(); x = q[1];"));
    assert_eq!(real(&s, "n"), 3.0);
    assert_eq!(real(&s, "x"), 20.0);
}

#[test]
fn test_array_push_and_reduce() {
    let s = run(&mod2b(
        "q = '{}; q.push_back(3.0); q.push_back(4.0); q.push_back(5.0); \
         x = q.sum(); n = q.size();"));
    assert_eq!(real(&s, "x"), 12.0);
    assert_eq!(real(&s, "n"), 3.0);
}

#[test]
fn test_array_min_max() {
    let s = run(&mod2b("q = '{7.0, 2.0, 9.0, 4.0}; x = q.max(); n = q.min();"));
    assert_eq!(real(&s, "x"), 9.0);
    assert_eq!(real(&s, "n"), 2.0);
}

#[test]
fn test_array_indexed_assignment() {
    let s = run(&mod2b("q = '{1.0, 2.0, 3.0}; q[1] = 99.0; x = q[1] + q[0];"));
    assert_eq!(real(&s, "x"), 100.0);
}

#[test]
fn test_array_pop_front_back() {
    let s = run(&mod2b(
        "q = '{1.0, 2.0, 3.0, 4.0}; x = q.pop_front(); n = q.size();"));
    assert_eq!(real(&s, "x"), 1.0);
    assert_eq!(real(&s, "n"), 3.0);
}

#[test]
fn test_foreach_sum() {
    let s = run(&mod2b(
        "q = '{1.0, 2.0, 3.0, 4.0}; x = 0.0; foreach (q[i]) x += q[i];"));
    assert_eq!(real(&s, "x"), 10.0);
}

#[test]
fn test_foreach_with_break() {
    let s = run(&mod2b(
        "q = '{1.0, 2.0, 3.0, 4.0, 5.0}; x = 0.0; \
         foreach (q[i]) begin if (q[i] > 3.0) break; x += q[i]; end"));
    assert_eq!(real(&s, "x"), 6.0); // 1+2+3
}

#[test]
fn test_array_reference_semantics() {
    // q and r share storage (handle semantics): mutating r is visible via q.
    let s = run(&mod2b("q = '{1.0}; r = q; r.push_back(2.0); n = q.size();"));
    assert_eq!(real(&s, "n"), 2.0);
}

#[test]
fn test_array_build_in_loop() {
    let s = run(&mod2b(
        "q = '{}; for (n = 0; n < 5; n++) q.push_back(n * n); x = q.sum();"));
    assert_eq!(real(&s, "x"), 30.0); // 0+1+4+9+16
}

// ── 2c: inside operator ───────────────────────────────────────────────────────

#[test]
fn test_inside_scalar_set() {
    let s = run(&mod2b("n = 5 inside {1, 5, 9};"));
    assert_eq!(real(&s, "n"), 1.0);
    let s = run(&mod2b("n = 4 inside {1, 5, 9};"));
    assert_eq!(real(&s, "n"), 0.0);
}

#[test]
fn test_inside_range() {
    let s = run(&mod2b("n = 15 inside {[10:20]};"));
    assert_eq!(real(&s, "n"), 1.0);
    let s = run(&mod2b("n = 25 inside {[10:20]};"));
    assert_eq!(real(&s, "n"), 0.0);
}

#[test]
fn test_inside_mixed_and_open() {
    // scalar + closed range + open-upper range
    let s = run(&mod2b("n = 3 inside {3, [10:20], [100:$]};"));
    assert_eq!(real(&s, "n"), 1.0);
    let s = run(&mod2b("n = 150 inside {3, [10:20], [100:$]};"));
    assert_eq!(real(&s, "n"), 1.0);
    let s = run(&mod2b("n = 50 inside {3, [10:20], [100:$]};"));
    assert_eq!(real(&s, "n"), 0.0);
}

#[test]
fn test_inside_in_if_condition() {
    let s = run(&mod2b(
        "x = 0.0; n = 7; if (n inside {[1:10]}) x = 42.0;"));
    assert_eq!(real(&s, "x"), 42.0);
}

// ── Wave 3: randomization ($urandom, $dist_*) ─────────────────────────────────

#[test]
fn test_urandom_range_bounds() {
    // 200 draws must all land in [10, 20].
    let s = run(&mod2b(
        "n = 0; q = '{}; for (n = 0; n < 200; n++) q.push_back($urandom_range(20, 10)); \
         x = q.min(); n = q.max();"));
    assert!(real(&s, "x") >= 10.0, "min {}", real(&s, "x"));
    assert!(real(&s, "n") <= 20.0, "max {}", real(&s, "n"));
}

#[test]
fn test_srandom_reproducible() {
    // Same seed → same sequence.
    let a = run(&mod2b("$srandom(12345); x = $urandom_range(1000000); n = 0;"));
    let b = run(&mod2b("$srandom(12345); x = $urandom_range(1000000); n = 0;"));
    assert_eq!(real(&a, "x"), real(&b, "x"));
}

#[test]
fn test_dist_uniform_bounds() {
    let s = run(&mod2b(
        "q = '{}; for (n = 0; n < 200; n++) q.push_back($dist_uniform(0, 5, 8)); \
         x = q.min(); n = q.max();"));
    assert!(real(&s, "x") >= 5.0);
    assert!(real(&s, "n") <= 8.0);
}

#[test]
fn test_dist_normal_mean() {
    // Sample mean of a large draw should sit near the requested mean.
    let s = run(&mod2b(
        "$srandom(7); q = '{}; for (n = 0; n < 5000; n++) q.push_back($dist_normal(0, 100.0, 5.0)); \
         x = q.mean();"));
    let m = real(&s, "x");
    assert!((m - 100.0).abs() < 1.0, "sample mean {m} not near 100");
}

#[test]
fn test_dist_normal_in_function_tolerance() {
    // Realistic use: a function returning a resistor value with 1% sigma.
    let s = run(r#"
module tb;
    function real with_tol(input real nominal, input real sigma_pct);
        return $dist_normal(0, nominal, nominal * sigma_pct / 100.0);
    endfunction
    initial begin
        real r;
        $srandom(42);
        r = with_tol(1000.0, 1.0);
    end
endmodule
"#);
    // within ~6 sigma of nominal essentially always
    let r = real(&s, "r");
    assert!((r - 1000.0).abs() < 60.0, "r={r}");
}
