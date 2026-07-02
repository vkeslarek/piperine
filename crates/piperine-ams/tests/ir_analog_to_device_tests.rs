//! Phase 1.4 — IR → Device, TDD style.

use piperine_ams::Document;

use std::path::Path;

fn va_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join("piperine-solver/tests/va")
        .join(name)
}

fn resistor_va_ir() -> piperine_codegen::ir::IrProgram {
    let doc = Document::parse_file(&va_path("resistor.va"))
        .expect("resistor.va parses");
    piperine_ams::ams_to_ir(&doc)
}

#[test]
fn ir_analog_to_device_smoke_resistor() {
    let ir = resistor_va_ir();
    let device = ir_analog_to_device(&ir, "resistor_va")
        .expect("JitAnalogDevice compiles from IR");
    assert_eq!(device.num_terminals(), 2);
    assert!(device.num_params() >= 1, "should have at least R param");
}

#[test]
fn ir_analog_to_device_smoke_capacitor() {
    let ir = {
        let doc = Document::parse_file(&va_path("capacitor.va"))
            .expect("capacitor.va parses");
        piperine_ams::ams_to_ir(&doc)
    };
    let device = ir_analog_to_device(&ir, "capacitor_va")
        .expect("JitAnalogDevice compiles from IR");
    assert_eq!(device.num_terminals(), 2);
    // Capacitor's analog body has ddt → IrAnalogBody.state_vars non-empty.
    let m = ir.modules.iter().find(|m| m.name == "capacitor_va").unwrap();
    let body = m.analog.as_ref().expect("analog body present");
    assert!(!body.states.is_empty(), "expected ddt state var in capacitor");
}

#[test]
fn ir_analog_to_device_returns_err_for_unknown_module() {
    let ir = resistor_va_ir();
    assert!(ir_analog_to_device(&ir, "nonexistent_module").is_err());
}


fn ir_analog_to_device(
    prog: &piperine_codegen::ir::IrProgram,
    name: &str,
) -> Result<std::sync::Arc<piperine_codegen::AnalogKernel>, piperine_codegen::CodegenError> {
    let module = prog.module(name).ok_or_else(|| piperine_codegen::CodegenError::ModuleNotFound(name.into()))?;
    let compiled = piperine_codegen::CompiledModule::compile(module)?;
    compiled.analog().ok_or_else(|| piperine_codegen::CodegenError::Invalid("no analog body".into())).map(|a| a.clone())
}
