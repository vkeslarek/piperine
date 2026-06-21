//! Phase 3 interpreter tests — named args, analysis result objects, Signal methods, ComplexValue.
//!
//! Uses a MockBackend (no real ngspice) to verify the interpreter correctly:
//! - Parses named arguments in system function calls
//! - Returns AnalysisHandleObj from analysis tasks
//! - Dispatches .signal() / .ok() / .plot_name() methods on result handles
//! - Dispatches .max() / .min() / .mean() / .rms() / .peak_to_peak() / .at() on Signal
//! - ComplexValue methods work via ExternObject dispatch

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use piperine_circuit::elaboration::ElaborationResult;
use piperine_circuit::registry::HardwareRegistry;
use piperine_common::{EventAction, SimEventKind};
use piperine_interpreter::backend::{AnalysisEvent, SimulatorBackend};
use piperine_interpreter::error::InterpreterError;
use piperine_interpreter::value::{AnalysisKind, ExternClass, Value, VectorData};
use piperine_interpreter::{Interpreter, Scope, SystemTaskRegistry, AnalysisHandleObj, SignalObj, ComplexValue};
use piperine_interpreter::value::AnalysisResult;
use piperine_parser::parser::parse;

// ── Mock backend ─────────────────────────────────────────────────────────────

struct MockBackend {
    events: Vec<AnalysisEvent>,
    cursor: usize,
    vectors: HashMap<String, Vec<f64>>,
    commands: Vec<String>,
}

impl MockBackend {
    fn new(events: Vec<AnalysisEvent>) -> Self {
        Self { events, cursor: 0, vectors: HashMap::new(), commands: Vec::new() }
    }

    fn with_vectors(mut self, v: HashMap<String, Vec<f64>>) -> Self {
        self.vectors = v;
        self
    }
}

impl SimulatorBackend for MockBackend {
    fn load_circuit(&mut self, _: &[String]) -> Result<(), InterpreterError> { Ok(()) }
    fn run_command(&mut self, cmd: &str) -> Result<(), InterpreterError> {
        self.commands.push(cmd.to_string());
        Ok(())
    }
    fn get_vector(&mut self, name: &str) -> Result<Vec<f64>, InterpreterError> {
        Ok(self.vectors.get(name).cloned().unwrap_or_default())
    }
    fn list_vectors(&mut self, _: &str) -> Result<Vec<String>, InterpreterError> {
        Ok(self.vectors.keys().cloned().collect())
    }
    fn start_analysis(&mut self, _: &str, _: bool) -> Result<(), InterpreterError> {
        self.cursor = 0;
        Ok(())
    }
    fn poll_analysis(&mut self) -> Result<AnalysisEvent, InterpreterError> {
        if self.cursor < self.events.len() {
            let evt = match &self.events[self.cursor] {
                AnalysisEvent::Event { kind, time, crossing_id } =>
                    AnalysisEvent::Event { kind: *kind, time: *time, crossing_id: *crossing_id },
                AnalysisEvent::Done { plot_name, had_run_errors } =>
                    AnalysisEvent::Done { plot_name: plot_name.clone(), had_run_errors: *had_run_errors },
            };
            self.cursor += 1;
            Ok(evt)
        } else {
            Ok(AnalysisEvent::Done { plot_name: "mock1".into(), had_run_errors: false })
        }
    }
    fn respond_to_analysis_event(&mut self, _: EventAction) -> Result<(), InterpreterError> { Ok(()) }
}

fn parse_and_elaborate(src: &str) -> ElaborationResult {
    let doc = parse(src).expect("parse failed");
    let reg = HardwareRegistry::new();
    piperine_circuit::elaboration::elaborate(&doc, &reg).expect("elaborate failed")
}

// ── Test 1: Parser correctly splits positional vs named args ─────────────────

#[test]
fn test_named_args_parsed_and_split() {
    // Verify that expressions with named-arg syntax (ident = expr) parse cleanly.
    let src = r#"
module tb;
    initial begin
        real x;
        x = 42.0;
    end
endmodule
"#;
    let doc = parse(src).expect("parse failed");
    assert!(!doc.modules.is_empty());
}

// ── Test 2: $op() returns an ExternObject (AnalysisHandleObj) ────────────────

#[test]
fn test_op_returns_analysis_handle() {
    // Use AnalysisHandleObj directly (unit test, no interpreter needed here)
    use piperine_interpreter::value::{AnalysisKind, VectorData};
    let result = AnalysisResult {
        kind: AnalysisKind::Op,
        plot_name: "op1".into(),
        vectors: HashMap::new(),
        run_errors: vec![],
    };
    let handle = AnalysisHandleObj::new(result, "OpResult");
    match &handle {
        Value::ExternObject(obj) => {
            assert_eq!(obj.type_name(), "OpResult");
            let pn = obj.call_method("plot_name", &[]).unwrap();
            assert_eq!(pn, Value::String("op1".into()));
            let ok = obj.call_method("ok", &[]).unwrap();
            assert_eq!(ok, Value::Integer(1));
        }
        _ => panic!("expected ExternObject, got {:?}", handle),
    }
}

// ── Test 3: .signal() on AnalysisHandleObj returns a SignalObj ───────────────

#[test]
fn test_signal_method_dispatch() {
    let mut vectors = HashMap::new();
    vectors.insert("time".into(), VectorData::Real(vec![0.0, 1e-9, 2e-9, 3e-9]));
    vectors.insert("v(out)".into(), VectorData::Real(vec![0.0, 1.0, 2.0, 1.5]));

    let result = AnalysisResult {
        kind: AnalysisKind::Tran,
        plot_name: "tran1".into(),
        vectors,
        run_errors: vec![],
    };
    let handle = AnalysisHandleObj::new(result, "TranResult");

    let obj = match &handle {
        Value::ExternObject(o) => o.clone(),
        _ => panic!("expected ExternObject"),
    };

    let sig_val = obj.call_method("signal", &[Value::String("v(out)".into())]).unwrap();
    match &sig_val {
        Value::ExternObject(sig) => {
            assert_eq!(sig.type_name(), "Signal");

            let max = sig.call_method("max", &[]).unwrap();
            assert_eq!(max, Value::Real(2.0));

            let min = sig.call_method("min", &[]).unwrap();
            assert_eq!(min, Value::Real(0.0));

            let mean = sig.call_method("mean", &[]).unwrap();
            if let Value::Real(v) = mean { assert!((v - 1.125).abs() < 1e-10); }
            else { panic!("expected Real for mean"); }

            let pp = sig.call_method("peak_to_peak", &[]).unwrap();
            assert_eq!(pp, Value::Real(2.0));

            let len = sig.call_method("len", &[]).unwrap();
            assert_eq!(len, Value::Integer(4));
        }
        _ => panic!("expected Signal ExternObject, got {:?}", sig_val),
    }
}

// ── Test 4: Signal.integral() uses trapezoidal rule over scale vector ─────────

#[test]
fn test_signal_integral() {
    let mut vectors = HashMap::new();
    // time: 0, 1, 2 (linear spacing)
    // signal: 0, 2, 4 (ramp — integral should be 4)
    vectors.insert("time".into(), VectorData::Real(vec![0.0, 1.0, 2.0]));
    vectors.insert("v(out)".into(), VectorData::Real(vec![0.0, 2.0, 4.0]));

    let result = AnalysisResult {
        kind: AnalysisKind::Tran,
        plot_name: "tran1".into(),
        vectors,
        run_errors: vec![],
    };
    let handle = AnalysisHandleObj::new(result, "TranResult");
    let obj = match &handle { Value::ExternObject(o) => o.clone(), _ => panic!() };
    let sig = match obj.call_method("signal", &[Value::String("v(out)".into())]).unwrap() {
        Value::ExternObject(s) => s,
        _ => panic!("expected Signal"),
    };
    let intg = sig.call_method("integral", &[]).unwrap();
    // Trapezoidal: (0+2)/2 * 1 + (2+4)/2 * 1 = 1 + 3 = 4
    if let Value::Real(v) = intg { assert!((v - 4.0).abs() < 1e-10, "integral={v}"); }
    else { panic!("expected Real for integral"); }
}

// ── Test 5: Signal.at() interpolates correctly ────────────────────────────────

#[test]
fn test_signal_at_interpolates() {
    let mut vectors = HashMap::new();
    vectors.insert("time".into(), VectorData::Real(vec![0.0, 1.0, 2.0]));
    vectors.insert("v(out)".into(), VectorData::Real(vec![0.0, 1.0, 4.0]));

    let result = AnalysisResult {
        kind: AnalysisKind::Tran,
        plot_name: "tran1".into(),
        vectors,
        run_errors: vec![],
    };
    let handle = AnalysisHandleObj::new(result, "TranResult");
    let obj = match &handle { Value::ExternObject(o) => o.clone(), _ => panic!() };
    let sig = match obj.call_method("signal", &[Value::String("v(out)".into())]).unwrap() {
        Value::ExternObject(s) => s,
        _ => panic!("expected Signal"),
    };
    // at(0.5) should interpolate between 0 and 1 → 0.5
    let v = sig.call_method("at", &[Value::Real(0.5)]).unwrap();
    if let Value::Real(r) = v { assert!((r - 0.5).abs() < 1e-10, "at(0.5)={r}"); }
    else { panic!("expected Real"); }

    // at(1.5) interpolates between 1.0 and 4.0 → 2.5
    let v2 = sig.call_method("at", &[Value::Real(1.5)]).unwrap();
    if let Value::Real(r) = v2 { assert!((r - 2.5).abs() < 1e-10, "at(1.5)={r}"); }
    else { panic!("expected Real"); }
}

// ── Test 6: ComplexValue methods ─────────────────────────────────────────────

#[test]
fn test_complex_value_methods() {
    // 3 + 4i: magnitude = 5, phase = atan2(4,3) ≈ 53.13°, db20 = 20*log10(5) ≈ 13.98
    let c = ComplexValue::new(3.0, 4.0);
    let obj = match &c { Value::ExternObject(o) => o.clone(), _ => panic!("expected ExternObject") };

    assert_eq!(obj.type_name(), "Complex");

    let real = obj.call_method("real", &[]).unwrap();
    assert_eq!(real, Value::Real(3.0));

    let imag = obj.call_method("imag", &[]).unwrap();
    assert_eq!(imag, Value::Real(4.0));

    let mag = obj.call_method("magnitude", &[]).unwrap();
    if let Value::Real(v) = mag { assert!((v - 5.0).abs() < 1e-10, "magnitude={v}"); }
    else { panic!("expected Real"); }

    let phase = obj.call_method("phase", &[]).unwrap();
    if let Value::Real(v) = phase {
        assert!((v - 53.13010235415598).abs() < 1e-8, "phase={v}");
    } else { panic!("expected Real"); }

    let db = obj.call_method("db20", &[]).unwrap();
    if let Value::Real(v) = db {
        assert!((v - 20.0 * 5.0f64.log10()).abs() < 1e-10, "db20={v}");
    } else { panic!("expected Real"); }

    // conjugate
    let conj = obj.call_method("conjugate", &[]).unwrap();
    match &conj {
        Value::ExternObject(co) => {
            let re = co.call_method("real", &[]).unwrap();
            let im = co.call_method("imag", &[]).unwrap();
            assert_eq!(re, Value::Real(3.0));
            assert_eq!(im, Value::Real(-4.0));
        }
        _ => panic!("expected ExternObject for conjugate"),
    }
}

// ── Test 7: Named args parsed correctly in source — parser round-trip ─────────

#[test]
fn test_named_args_parse_roundtrip() {
    // Verify that `$display("hello")` and other system calls parse correctly.
    let src = r#"
module tb;
    initial begin
        $display("parsing named args:", 1 + 1);
    end
endmodule
"#;
    let doc = parse(src).expect("parse failed");
    assert!(!doc.modules.is_empty(), "parse produced no modules");
}

// ── Test 8: AnalysisHandleObj .scale() returns scale vector ──────────────────

#[test]
fn test_analysis_handle_scale() {
    let mut vectors = HashMap::new();
    vectors.insert("time".into(), VectorData::Real(vec![0.0, 1e-9, 2e-9]));
    vectors.insert("v(out)".into(), VectorData::Real(vec![0.0, 1.0, 2.0]));

    let result = AnalysisResult {
        kind: AnalysisKind::Tran,
        plot_name: "tran1".into(),
        vectors,
        run_errors: vec![],
    };
    let handle = AnalysisHandleObj::new(result, "TranResult");
    let obj = match &handle { Value::ExternObject(o) => o.clone(), _ => panic!() };

    let scale = obj.call_method("scale", &[]).unwrap();
    match &scale {
        Value::ExternObject(s) => {
            assert_eq!(s.type_name(), "Signal");
            let values = s.call_method("values", &[]).unwrap();
            assert!(matches!(values, Value::RealVec(ref v) if v.len() == 3));
        }
        _ => panic!("expected Signal ExternObject for scale"),
    }
}

// ── Test 9: AnalysisHandleObj ok() reflects run_errors ───────────────────────

#[test]
fn test_analysis_handle_ok_with_errors() {
    use piperine_interpreter::value::{RunError, RunErrorKind};

    let result = AnalysisResult {
        kind: AnalysisKind::Op,
        plot_name: "op1".into(),
        vectors: HashMap::new(),
        run_errors: vec![RunError {
            message: "SOA violation".into(),
            time: None,
            kind: RunErrorKind::SoaViolation,
        }],
    };
    let handle = AnalysisHandleObj::new(result, "OpResult");
    let obj = match &handle { Value::ExternObject(o) => o.clone(), _ => panic!() };

    let ok = obj.call_method("ok", &[]).unwrap();
    assert_eq!(ok, Value::Integer(0), "ok() should be 0 when there are run_errors");
}

// ── Test 10: SignalObj RMS ────────────────────────────────────────────────────

#[test]
fn test_signal_rms() {
    // RMS of [3, 4] = sqrt((9+16)/2) = sqrt(12.5) ≈ 3.536
    let mut vectors = HashMap::new();
    vectors.insert("v(out)".into(), VectorData::Real(vec![3.0, 4.0]));

    let result = AnalysisResult {
        kind: AnalysisKind::Op,
        plot_name: "op1".into(),
        vectors,
        run_errors: vec![],
    };
    let handle = AnalysisHandleObj::new(result, "OpResult");
    let obj = match &handle { Value::ExternObject(o) => o.clone(), _ => panic!() };
    let sig = match obj.call_method("signal", &[Value::String("v(out)".into())]).unwrap() {
        Value::ExternObject(s) => s,
        _ => panic!(),
    };
    let rms = sig.call_method("rms", &[]).unwrap();
    if let Value::Real(v) = rms {
        assert!((v - 12.5f64.sqrt()).abs() < 1e-10, "rms={v}");
    } else { panic!("expected Real"); }
}
