//! API surface pinning tests for the IR-centric codegen (in piperine-lang).

use piperine_lang::{parse_and_elaborate, ppr_to_ir};
use piperine_codegen::DigitalKernel;
use piperine_codegen::ir::IrProgram;

fn parse_ppr(body: &str) -> IrProgram {
    let elab = parse_and_elaborate(body, &piperine_lang::SourceMap::dummy()).expect("PHDL parse/elab");
    ppr_to_ir(&elab)
}

#[test]
fn ppr_ir_produces_module_index() {
    let body = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod R (inout p : Electrical, inout n : Electrical) { param r : Real = 1.0e3; }
        analog R { I(p, n) <+ V(p, n) / r; }
    "#;
    let ir = parse_ppr(body);
    assert!(ir.modules.iter().any(|m| m.name == "R"), "missing R module");
}

#[test]
fn ir_analog_compile_resistor() {
    let ir = parse_ppr(r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod R (inout p : Electrical, inout n : Electrical) { param r : Real = 1.0e3; }
        analog R { I(p, n) <+ V(p, n) / r; }
    "#);
    let _dev = ir_analog_to_device(&ir, "R").expect("resistor");
}

#[test]
fn ir_analog_compile_capacitor() {
    let ir = parse_ppr(r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod Cap (inout p : Electrical, inout n : Electrical) { param c : Real = 1.0e-9; }
        analog Cap { I(p, n) <+ c * ddt(V(p, n)); }
    "#);
    let _dev = ir_analog_to_device(&ir, "Cap").expect("capacitor");
}

#[test]
fn ir_analog_compile_tanh_builtin() {
    // GAPS I.13 — sinh/cosh/tanh are builtin math (not user fns), so they
    // must compile through the JIT like any other builtin call.
    let ir = parse_ppr(r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod T (inout p : Electrical, inout n : Electrical) { param g : Real = 1.0; }
        analog T { I(p, n) <+ g * tanh(V(p, n)); }
    "#);
    let _dev = ir_analog_to_device(&ir, "T").expect("tanh compiles");
}

#[test]
fn ir_digital_compile_dff() {
    let src = "
        discipline Bit { storage Boolean; }
        mod DFF (input clk: Bit, input D: Bit, output Q: Bit) {}
        digital DFF { @ posedge(clk) { Q <- D; } }
    ";
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("parse_and_elaborate DFF");
    let ir = ppr_to_ir(&elab);
    let _interp = DigitalKernel::compile(ir.module("DFF").unwrap()).expect("DFF interp");
}

fn ir_analog_to_device(
    prog: &piperine_codegen::ir::IrProgram,
    name: &str,
) -> Result<std::sync::Arc<piperine_codegen::AnalogKernel>, piperine_codegen::CodegenError> {
    let module = prog.module(name).ok_or_else(|| piperine_codegen::CodegenError::ModuleNotFound(name.into()))?;
    let compiled = piperine_codegen::CompiledModule::compile(module)?;
    compiled.analog().ok_or_else(|| piperine_codegen::CodegenError::Invalid("no analog body".into())).map(|a| a.clone())
}
