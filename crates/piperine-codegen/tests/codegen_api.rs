//! API surface pinning tests for codegen's POM lowering.

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_codegen::DigitalKernel;
use piperine_codegen::ir::LoweredBody;

fn parse_ppr(body: &str) -> HashMap<String, LoweredBody> {
    let elab = parse_and_elaborate(body, &piperine_lang::SourceMap::dummy()).expect("PHDL parse/elab");
    piperine_codegen::ir::lower_bodies(&elab).expect("lowering failed")
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
    let bodies = piperine_codegen::ir::lower_bodies(&elab).expect("lowering failed");
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
