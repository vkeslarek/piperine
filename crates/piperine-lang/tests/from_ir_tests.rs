//! Phase 1.6 — full IR → CircuitInstance glue, TDD style.
//!
//! Loads an [`IrProgram`] whose top module names the root of the netlist.
//! Walks the IR instances, dispatches each to either `ir_analog_to_device`
//! or `ir_digital_to_interp`, attaches wires to the `Netlist`, and
//! produces a [`CircuitInstance`] ready for the solver.

use piperine_lang::{parse_and_elaborate, ppr_to_ir};
use piperine_solver::circuit::CircuitInstance;
use piperine_codegen::CircuitCompiler;
fn from_ir(prog: &piperine_codegen::ir::IrProgram, top: &str) -> Result<CircuitInstance, String> {
    let mut c = CircuitCompiler::new(prog);
    c.build_circuit(top).map_err(|e| e.to_string())
}


#[test]
fn from_ir_resistor_va_yields_circuit() {
    // Leaf device: PHDL resistor with no child instances
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) { param r: Real = 1.0e3; }
        analog R { I(p, n) <+ V(p, n) / r; }
    ";
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("PHDL parses + elaborates");
    let ir = ppr_to_ir(&elab);
    // Sanity: the module is present in the IR.
    assert!(ir.modules.iter().any(|m| m.name == "R"));
    // `from_ir` on the leaf accepts but produces an empty netlist.
    let ci: CircuitInstance = from_ir(&ir, "R").expect("from_ir on leaf");
    // No leaf instance expected — children come from `top_module.instances`.
    assert!(ci.all_devices().is_empty(),
        "leaf module has no instances; expected empty device list");
}

#[test]
fn from_ir_ppr_resistor_yields_circuit() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) { param r: Real = 1.0e3; }
        analog R { I(p, n) <+ V(p, n) / r; }
        mod Top ( inout a: Electrical, inout b: Electrical ) { R(a, b); }
    ";
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("PHDL parse_and_elaborate");
    let ir = ppr_to_ir(&elab);
    let ci: CircuitInstance = from_ir(&ir, "Top").expect("from_ir compiles top");
    assert!(ci.all_devices().len() >= 1);
}

#[test]
fn from_ir_unknown_top_returns_err() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) { param r: Real = 1.0e3; }
        analog R { I(p, n) <+ V(p, n) / r; }
    ";
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("PHDL parses");
    let ir = ppr_to_ir(&elab);
    assert!(from_ir(&ir, "no-such-module").is_err());
}


fn ir_analog_to_device(
    prog: &piperine_codegen::ir::IrProgram,
    name: &str,
) -> Result<std::sync::Arc<piperine_codegen::AnalogKernel>, piperine_codegen::CodegenError> {
    let module = prog.module(name).ok_or_else(|| piperine_codegen::CodegenError::ModuleNotFound(name.into()))?;
    let compiled = piperine_codegen::CompiledModule::compile(module)?;
    compiled.analog().ok_or_else(|| piperine_codegen::CodegenError::Invalid("no analog body".into())).map(|a| a.clone())
}
