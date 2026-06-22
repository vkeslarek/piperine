use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use piperine_circuit::elaboration::ElaborationResult;
use piperine_circuit::registry::HardwareRegistry;
use piperine_common::{EventAction, SimEventKind};
use piperine_interpreter::backend::{AnalysisEvent, SimulatorBackend};
use piperine_interpreter::error::InterpreterError;
use piperine_interpreter::value::{AnalysisKind, ExternClass, Value, VectorData};
use piperine_interpreter::{Interpreter, Scope, SystemTaskRegistry};
use piperine_parser::parser::parse;
use piperine_parser::parser::parse_with_includes;
use piperine_ngspice::NgspicePlugin;
use piperine_interpreter::Plugin;

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
    let full_src = format!("`include \"ngspice.ppr\"\n{}", src);
    let dirs = vec![
        piperine_ngspice::ppr_dir(),
        piperine_parser::bundled_header_dir(),
    ];
    let doc = parse_with_includes(&full_src, &dirs).expect("parse failed");
    let mut reg = HardwareRegistry::new();
    let plugin = NgspicePlugin::default();
    plugin.register_hardware(&mut reg);
    piperine_circuit::elaboration::elaborate(&doc, &reg).expect("elaborate failed")
}

// ── Test 1: Device Handles and Operating Point Access ────────────────────────

#[test]
fn test_device_handle_op_access() {
    let src = r#"
module tb;
    nmos #(.model("nm"), .w(1e-6), .l(100e-9)) M1(.d(out), .g(in), .s(gnd), .b(gnd));
    initial begin
        $op();
        real gm_val = M1.gm();
        real id_val = M1.id;
    end
endmodule
"#;
    // We mock the backend to return 42.0 for @M1[gm] and 1.5 for @M1[id].
    let mut vectors = HashMap::new();
    vectors.insert("@M1[gm]".into(), vec![42.0]);
    vectors.insert("@M1[id]".into(), vec![1.5]);
    
    let mut backend = MockBackend::new(vec![
        AnalysisEvent::Done { plot_name: "op1".into(), had_run_errors: false },
    ]).with_vectors(vectors);
    
    let result = parse_and_elaborate(src);
    
    // Assert auto-save lines were added during elaboration.
    assert!(result.spice_lines.iter().any(|l| l == ".save @M1[gm]"));
    assert!(result.spice_lines.iter().any(|l| l == ".save @M1[id]"));
    
    let mut tasks = SystemTaskRegistry::default();
    let plugin = NgspicePlugin::default();
    plugin.register_tasks(&mut tasks);
    let mut interp = Interpreter::new(&mut backend, &tasks);
    
    // In actual use, run_analysis populates these, but here we just test exec().
    // We need $op() in tasks or we can comment out $op() from src and test manually.
    // Let's test the handle dispatch logic directly or use Interpreter::exec.
    // Wait, we need to register the system tasks to parse $op().
    
    // For now we will manually populate the scope and test interpreter paths
    let mut scope = Scope::default();
    interp.set_devices(result.instances.clone()); // need to add set_devices to Interpreter
    
    // Call exec to evaluate the initial statement
    interp.exec(&result.initial_statement, &mut scope).expect("exec failed");
    
    assert_eq!(scope.get("gm_val").unwrap(), &Value::Real(42.0));
    assert_eq!(scope.get("id_val").unwrap(), &Value::Real(1.5));
}

// ── Test 1b: device whose name lacks the SPICE prefix (load -> Rload) ─────────

#[test]
fn test_device_handle_non_prefixed_name() {
    // A resistor named `load` becomes SPICE element `Rload`. The handle must be
    // bound under the piperine name `load`, but query `@Rload[i]`.
    let src = r#"
module tb;
    res #(.r(1e3)) load(.p(out), .n(gnd));
    initial begin
        $op();
        real cur = load.i;
    end
endmodule
"#;
    let result = parse_and_elaborate(src);
    // Auto-save must use the SPICE name, not the piperine name.
    assert!(result.spice_lines.iter().any(|l| l == ".save @Rload[i]"),
        "expected `.save @Rload[i]`, got: {:?}", result.spice_lines);
    assert!(!result.spice_lines.iter().any(|l| l == ".save @load[i]"));

    let mut vectors = HashMap::new();
    vectors.insert("@Rload[i]".into(), vec![0.001]);
    let mut backend = MockBackend::new(vec![
        AnalysisEvent::Done { plot_name: "op1".into(), had_run_errors: false },
    ]).with_vectors(vectors);

    let mut tasks = SystemTaskRegistry::default();
    NgspicePlugin::default().register_tasks(&mut tasks);
    let mut interp = Interpreter::new(&mut backend, &tasks);
    interp.set_devices(result.instances.clone());
    let mut scope = Scope::default();
    interp.exec(&result.initial_statement, &mut scope).expect("exec failed");

    assert_eq!(scope.get("cur").unwrap(), &Value::Real(0.001));
}

// ── Test 2: Physical Constants ───────────────────────────────────────────────

#[test]
fn test_physical_constants() {
    let src = r#"
module tb;
    initial begin
        real thermal_voltage = BOLTZMANN * 300.0 / ECHARGE;
        real double_pi = M_PI * 2.0;
    end
endmodule
"#;
    let mut backend = MockBackend::new(vec![]);
    let result = parse_and_elaborate(src);
    
    let tasks = SystemTaskRegistry::default();
    let mut interp = Interpreter::new(&mut backend, &tasks);
    
    let mut scope = Scope::default();
    interp.exec(&result.initial_statement, &mut scope).expect("exec failed");
    
    let thermal_v = scope.get("thermal_voltage").unwrap().as_f64().unwrap();
    // 1.380649e-23 * 300 / 1.602176634e-19 ≈ 0.0258519997...
    assert!((thermal_v - 0.025852).abs() < 1e-5);
    
    let dpi = scope.get("double_pi").unwrap().as_f64().unwrap();
    assert!((dpi - std::f64::consts::TAU).abs() < 1e-10);
}

// ── Test 3: System Tasks ─────────────────────────────────────────────────────

#[test]
fn test_system_tasks() {
    let src = r#"
module tb;
    initial begin
        $set_option("reltol", 1e-5);
        $set_temp(50.0);
        $set_tnom(25.0);
        
        $alter("R1", "resistance", 2000.0);
        $altermod("NMOS_SVT", "vth0", 0.5);
        $alterparam("mc_runs", 100);
        
        real v1 = $V("n1");
        real vdiff = $V("n1", "n2");
        array vlist = $get_vec("v(n1)");
    end
endmodule
"#;
    let mut backend = MockBackend::new(vec![]);
    
    // Add fake vector data for $V and $get_vec
    let mut vectors = HashMap::new();
    vectors.insert("v(n1)".into(), vec![1.2, 2.4, 3.6]);
    vectors.insert("v(n1, n2)".into(), vec![0.5]);
    backend = backend.with_vectors(vectors);
    
    let result = parse_and_elaborate(src);
    
    let mut tasks = SystemTaskRegistry::default();
    let plugin = NgspicePlugin::default();
    plugin.register_tasks(&mut tasks);
    
    let mut interp = Interpreter::new(&mut backend, &tasks);
    
    let mut scope = Scope::default();
    interp.exec(&result.initial_statement, &mut scope).expect("exec failed");
    
    assert_eq!(scope.get("v1").unwrap(), &Value::Real(3.6));
    assert_eq!(scope.get("vdiff").unwrap(), &Value::Real(0.5));
    
    if let Value::RealVec(v) = scope.get("vlist").unwrap() {
        assert_eq!(v, &vec![1.2, 2.4, 3.6]);
    } else {
        panic!("vlist is not RealVec");
    }
    
    let expected_cmds = vec![
        "option reltol=0.00001",
        "set temp = 50",
        "set tnom = 25",
        "alter R1 resistance = 2000",
        "altermod NMOS_SVT vth0 = 0.5",
        "alterparam mc_runs = 100",
        "reset",
    ];
    assert_eq!(backend.commands, expected_cmds);
}



