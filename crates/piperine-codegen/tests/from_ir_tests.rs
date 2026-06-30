//! Phase 1.6 — full IR → CircuitInstance glue, TDD style.
//!
//! Loads an [`IrProgram`] whose top module names the root of the netlist.
//! Walks the IR instances, dispatches each to either `ir_analog_to_device`
//! or `ir_digital_to_interp`, attaches wires to the `Netlist`, and
//! produces a [`CircuitInstance`] ready for the solver.

use piperine_codegen::{ams_to_ir, from_ir, ppr_to_ir};
use piperine_solver::circuit::CircuitInstance;
use piperine_ams::Document;
use piperine_lang::parse_and_elaborate;
use std::path::Path;

fn va_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join("piperine-solver/tests/va")
        .join(name)
}

#[test]
fn from_ir_resistor_va_yields_circuit() {
    // `resistor.va` is a leaf device; `from_ir` on the leaf yields an
    // empty netlist (it has no child instances).  Instead, construct a
    // small wrapper and instantiate `resistor_va` inside it.
    let doc = Document::parse_file(&va_path("resistor.va")).expect("resistor parses");
    let ir = ams_to_ir(&doc);
    // Sanity: the module is present in the IR.
    assert!(ir.modules.iter().any(|m| m.name == "resistor_va"));
    // `from_ir` on the leaf accepts but produces an empty netlist.
    let ci: CircuitInstance = from_ir(&ir, "resistor_va").expect("from_ir on leaf");
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
    let elab = parse_and_elaborate(src).expect("PHDL parse_and_elaborate");
    let ir = ppr_to_ir(&elab);
    let ci: CircuitInstance = from_ir(&ir, "Top").expect("from_ir compiles top");
    assert!(ci.all_devices().len() >= 1);
}

#[test]
fn from_ir_unknown_top_returns_err() {
    let doc = Document::parse_file(&va_path("resistor.va")).unwrap();
    let ir = ams_to_ir(&doc);
    assert!(from_ir(&ir, "no-such-module").is_err());
}
