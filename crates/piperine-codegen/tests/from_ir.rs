//! Phase 1.6 — full POM → CircuitInstance glue, TDD style.
//!
//! Loads a `Design` whose top module names the root of the netlist. Walks
//! the top module's instances, dispatches each to either
//! `ir_analog_to_device` or `ir_digital_to_interp`, attaches wires to the
//! `Netlist`, and produces a [`CircuitInstance`] ready for the solver.

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_lang::pom::Design;
use piperine_codegen::resolve::LoweredBody;
use piperine_solver::prelude::CircuitInstance;
use piperine_codegen::CircuitCompiler;

fn from_ir(design: &Design, bodies: &HashMap<String, LoweredBody>, top: &str) -> Result<CircuitInstance, String> {
    let mut c = CircuitCompiler::new(design, bodies);
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
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering failed");
    // Sanity: the module is present in the lowered bodies.
    assert!(bodies.contains_key("R"));
    // `from_ir` on the leaf accepts but produces an empty netlist.
    let ci: CircuitInstance = from_ir(&elab, &bodies, "R").expect("from_ir on leaf");
    // No leaf instance expected — children come from the top module's instances.
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
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering failed");
    let ci: CircuitInstance = from_ir(&elab, &bodies, "Top").expect("from_ir compiles top");
    assert!(ci.all_devices().len() >= 1);
}

#[test]
fn jit_device_exposes_params_through_introspection() {
    use piperine_solver::prelude::{Invalidation, ParamError, Value};

    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) { param r: Real = 1.0e3; }
        analog R { I(p, n) <+ V(p, n) / r; }
        mod Top ( inout a: Electrical, inout b: Electrical ) { R(a, b); }
    ";
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("parse+elab");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering");
    let mut ci = from_ir(&elab, &bodies, "Top").expect("from_ir compiles Top");

    let dev = &mut ci.all_devices_mut()[0];

    // The JIT device advertises its `r` parameter through the introspection ABI.
    assert!(dev.list_params().iter().any(|p| p.name == "r"), "`r` should be listed");
    assert_eq!(dev.get_param("r"), Some(Value::Real(1000.0)));

    // A runtime write restamps (no rebuild) and is read back.
    assert_eq!(dev.set_param("r", Value::Real(2000.0)), Ok(Invalidation::Restamp));
    assert_eq!(dev.get_param("r"), Some(Value::Real(2000.0)));

    // Unknown parameters and non-real values are rejected loud.
    assert!(matches!(dev.set_param("nope", Value::Real(1.0)), Err(ParamError::Unknown(_))));
    assert!(matches!(
        dev.set_param("r", Value::Boolean(true)),
        Err(ParamError::TypeMismatch { .. })
    ));
}

#[test]
fn from_ir_unknown_top_returns_err() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod R (inout p: Electrical, inout n: Electrical) { param r: Real = 1.0e3; }
        analog R { I(p, n) <+ V(p, n) / r; }
    ";
    let elab = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("PHDL parses");
    let bodies = piperine_codegen::resolve::lower_bodies(&elab).expect("lowering failed");
    assert!(from_ir(&elab, &bodies, "no-such-module").is_err());
}

#[allow(dead_code)]
fn ir_analog_to_device(
    bodies: &HashMap<String, LoweredBody>,
    name: &str,
) -> Result<std::sync::Arc<piperine_codegen::AnalogKernel>, piperine_codegen::CodegenError> {
    let body = bodies.get(name).ok_or_else(|| piperine_codegen::CodegenError::ModuleNotFound(name.into()))?;
    let compiled = piperine_codegen::CompiledModule::compile(body)?;
    compiled.analog().ok_or_else(|| piperine_codegen::CodegenError::Invalid("no analog body".into())).map(|a| a.clone())
}

