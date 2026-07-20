//! API surface pinning tests for codegen's POM lowering.

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_codegen::DigitalKernel;
use piperine_codegen::resolve::LoweredBody;

fn parse_ppr(body: &str) -> HashMap<String, LoweredBody> {
    let elab = parse_and_elaborate(body, &piperine_lang::SourceMap::dummy()).expect("PHDL parse/elab");
    piperine_codegen::resolve::lower_bodies(&elab).expect("lowering failed")
}

#[test]
fn ppr_ir_produces_module_index() {
    let body = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod R (inout p : Electrical, inout n : Electrical) { param r : Real = 1.0e3; }
        analog R { I(p, n) <+ V(p, n) / r; }
    "#;
    let bodies = parse_ppr(body);
    assert!(bodies.contains_key("R"), "missing R module");
}

#[test]
fn ir_analog_compile_resistor() {
    let bodies = parse_ppr(r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod R (inout p : Electrical, inout n : Electrical) { param r : Real = 1.0e3; }
        analog R { I(p, n) <+ V(p, n) / r; }
    "#);
    let _dev = ir_analog_to_device(&bodies, "R").expect("resistor");
}

#[test]
fn ir_analog_compile_capacitor() {
    let bodies = parse_ppr(r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod Cap (inout p : Electrical, inout n : Electrical) { param c : Real = 1.0e-9; }
        analog Cap { I(p, n) <+ c * ddt(V(p, n)); }
    "#);
    let _dev = ir_analog_to_device(&bodies, "Cap").expect("capacitor");
}

#[test]
fn ir_analog_compile_tanh_builtin() {
    // GAPS I.13 — sinh/cosh/tanh are builtin math (not user fns), so they
    // must compile through the JIT like any other builtin call.
    let bodies = parse_ppr(r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod T (inout p : Electrical, inout n : Electrical) { param g : Real = 1.0; }
        analog T { I(p, n) <+ g * tanh(V(p, n)); }
    "#);
    let _dev = ir_analog_to_device(&bodies, "T").expect("tanh compiles");
}

#[test]
fn ir_digital_compile_dff() {
    let src = "
        discipline Bit { storage Boolean; }
        mod DFF (input clk: Bit, input D: Bit, output Q: Bit) {}
        digital DFF { @ posedge(clk) { Q <- D; } }
    ";
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("parse_and_elaborate DFF");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering failed");
    let _interp = DigitalKernel::compile(bodies.get("DFF").unwrap()).expect("DFF interp");
}

fn ir_analog_to_device(
    bodies: &HashMap<String, LoweredBody>,
    name: &str,
) -> Result<std::sync::Arc<piperine_codegen::AnalogKernel>, piperine_codegen::CodegenError> {
    let body = bodies.get(name).ok_or_else(|| piperine_codegen::CodegenError::ModuleNotFound(name.into()))?;
    let compiled = piperine_codegen::CompiledModule::compile(body)?;
    compiled.analog().ok_or_else(|| piperine_codegen::CodegenError::Invalid("no analog body".into())).map(|a| a.clone())
}

#[test]
fn test_purely_resistive_device_capabilities() {
    use piperine_solver::abi::{Element, ElementCapabilities};
    let bodies = parse_ppr(r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod R (inout p : Electrical, inout n : Electrical) { param r : Real = 1.0e3; }
        analog R { I(p, n) <+ V(p, n) / r; }
    "#);
    let kernel = ir_analog_to_device(&bodies, "R").unwrap();
    let mut netlist = piperine_solver::abi::Netlist::new();
    let terms = vec![piperine_solver::abi::NodeIdentifier::Anonymous(1), piperine_solver::abi::NodeIdentifier::Anonymous(2)];
    let a_inst = piperine_codegen::device::AnalogInstance::new("testR", kernel, &terms, vec![1000.0], 1, &mut netlist).unwrap();
    let dev = piperine_codegen::device::PiperineDevice::new("testR", Some(a_inst), None);
    
    let caps = dev.capabilities();
    // A purely resistive analog device advertises the analog engine and no
    // digital participation (the removed ANALYTIC_JACOBIAN/STAMPS_CHARGE flags
    // had no solver consumer — SS-10).
    assert!(caps.contains(ElementCapabilities::ANALOG));
    assert!(!caps.contains(ElementCapabilities::DIGITAL));
}

#[test]
fn test_reactive_device_capabilities() {
    use piperine_solver::abi::{Element, ElementCapabilities};
    let bodies = parse_ppr(r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod Cap (inout p : Electrical, inout n : Electrical) { param c : Real = 1.0e-9; }
        analog Cap { I(p, n) <+ c * ddt(V(p, n)); }
    "#);
    let kernel = ir_analog_to_device(&bodies, "Cap").unwrap();
    let mut netlist = piperine_solver::abi::Netlist::new();
    let terms = vec![piperine_solver::abi::NodeIdentifier::Anonymous(1), piperine_solver::abi::NodeIdentifier::Anonymous(2)];
    let a_inst = piperine_codegen::device::AnalogInstance::new("testC", kernel, &terms, vec![1e-9], 1, &mut netlist).unwrap();
    let dev = piperine_codegen::device::PiperineDevice::new("testC", Some(a_inst), None);
    
    let caps = dev.capabilities();
    // A reactive analog device still advertises just the analog engine —
    // reactivity is an internal kernel property, no longer a capability flag
    // (the removed STAMPS_CHARGE had no solver consumer — SS-10).
    assert!(caps.contains(ElementCapabilities::ANALOG));
    assert!(!caps.contains(ElementCapabilities::DIGITAL));
}
