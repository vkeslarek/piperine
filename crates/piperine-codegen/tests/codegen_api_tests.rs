//! Phase 1 — API surface pinning tests for the IR-centric codegen.
//!
//! Tests here document the public API of `piperine-codegen` once Phase 1.6
//! is complete:
//!   - `ams_to_ir(ams_doc)`     ✅ available now
//!   - `ppr_to_ir(phdl_prog)`   ✅ available now
//!   - `compile_analog_module(&IrProgram, name)` (NOT YET — expects &ElabProgram)
//!   - `compile_digital_module(&IrProgram, name)` (NOT YET)
//!   - `from_ir(&IrProgram, top)`                (NOT YET — Phase 1.6)
//!
//! Each test that depends on a future API is `#[ignore]` until the matching
//! sub-step is implemented.

use std::path::{Path, PathBuf};
use piperine_ams::Document;
use piperine_codegen::{ams_to_ir, ppr_to_ir};
use piperine_codegen::IrProgram;
use piperine_lang::parse_and_elaborate;

fn bundled_headers() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join("piperine-ams")
        .join("headers")
}

fn va_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join("piperine-solver/tests/va")
        .join(name)
}

fn parse_va(name: &str) -> Document {
    let path = va_fixture(name);
    let input = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {name}: {e}"));
    let dirs = vec![bundled_headers(), path.parent().unwrap().to_path_buf()];
    Document::parse_with_includes(&input, &dirs)
        .unwrap_or_else(|e| panic!("parse {name}: {e}"))
}

fn parse_ppr(body: &str) -> IrProgram {
    let elab = parse_and_elaborate(body).expect("PHDL parse/elab");
    ppr_to_ir(&elab)
}

#[test]
fn ams_ir_produces_module_index() {
    let doc = parse_va("resistor.va");
    let ir = ams_to_ir(&doc);
    assert!(
        ir.modules.iter().any(|m| m.name == "resistor_va"),
        "expected module resistor_va, got modules: {:?}",
        ir.modules.iter().map(|m| &m.name).collect::<Vec<_>>()
    );
}

#[test]
fn ppr_ir_produces_module_index() {
    let body = r#"
        discipline Electrical {
            potential v : Real;
            flow i : Real;
        }
        mod R (inout p : Electrical, inout n : Electrical) {
            param r : Real = 1.0e3;
        }
        analog R {
            I(p, n) <+ V(p, n) / r;
        }
    "#;
    let ir = parse_ppr(body);
    assert!(ir.modules.iter().any(|m| m.name == "R"), "missing R module");
}

// Future tests, marked `#[ignore]` until each phase step is implemented.

#[test]
fn ir_analog_compile_resistor() {
    let ir = ams_to_ir(&parse_va("resistor.va"));
    let _dev = piperine_codegen::ir_analog_to_device(&ir, "resistor_va").expect("resistor");
}

#[test]
fn ir_analog_compile_capacitor() {
    let ir = ams_to_ir(&parse_va("capacitor.va"));
    let _dev = piperine_codegen::ir_analog_to_device(&ir, "capacitor_va").expect("capacitor");
}

#[test]
fn ir_digital_compile_dff() {
    use piperine_codegen::{ir_digital_to_interp, ppr_to_ir};
    use piperine_lang::parse_and_elaborate;
    let src = "
        discipline Bit {}
        mod DFF (input clk: Bit, input D: Bit, output Q: Bit) {}
        digital DFF { @ posedge(clk) { Q <- D; } }
    ";
    let elab = parse_and_elaborate(src).expect("parse_and_elaborate DFF");
    let ir = ppr_to_ir(&elab);
    let _interp = ir_digital_to_interp(&ir, "DFF").expect("DFF interp");
}
