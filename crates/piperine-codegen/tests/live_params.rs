//! PHDL path parity for live parameter sets (LIVE-01): the solver's element
//! labels are exactly the POM instance paths `Design::set_param` accepts, so
//! one addressing scheme works before and after compilation.
//!
//! Path grammar note (spec assumption, verified here): elaboration produces
//! a flat top module — instance labels are the flat paths (`"r2"`), and
//! bundle-typed params flatten into `{param}_{field}` scalars
//! (`"model_r0"`). Nested hierarchy is fail-loud at circuit build
//! ("flatten during elaboration"), so every path the POM accepts for a
//! *compilable* design is a flat label — parity is asserted over that
//! grammar, bundle params included.

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_lang::pom::Design;
use piperine_codegen::ir::LoweredBody;
use piperine_codegen::CircuitCompiler;
use piperine_solver::prelude::{Context, Value};

/// Divider with bundle-param resistors: v1 (10 V) over r1 (top→mid) and
/// r2 (mid→gnd); each resistance is `model.r0 * model.k` from a bundle.
const DIVIDER: &str = r#"
    discipline Electrical { potential v : Real; flow i : Real; }

    bundle RModel { r0 : Real = 1.0e3, k : Real = 1.0 }

    mod R (inout p : Electrical, inout n : Electrical) {
        param model : RModel = RModel {};
    }
    analog R { I(p, n) <+ V(p, n) / (model.r0 * model.k); }

    mod Vsrc (inout p : Electrical, inout n : Electrical) {
        param dc : Real = 10.0;
    }
    analog Vsrc { V(p, n) <- dc; }

    mod Top () {
        wire gnd : Electrical;
        wire top : Electrical;
        wire mid : Electrical;
        v1 : Vsrc(.p = top, .n = gnd) {};
        r1 : R(.p = top, .n = mid) {};
        r2 : R(.p = mid, .n = gnd) {};
    }
"#;

fn elaborate() -> (Design, HashMap<String, LoweredBody>) {
    let design = parse_and_elaborate(DIVIDER, &piperine_lang::SourceMap::dummy())
        .expect("divider elaborates");
    let bodies = piperine_codegen::ir::lower_bodies(&design).expect("divider lowers");
    (design, bodies)
}

/// LIVE-01: the same path + param addresses the same instance through the
/// POM staging oracle (`Design::set_param` → re-elaborate → rebuild) and
/// through the live solver set on the already-compiled circuit — including
/// a bundle param flattened to `model_r0`.
#[test]
fn solver_set_matches_pom_path_for_flat_and_bundle_params() {
    let (design, bodies) = elaborate();

    // Element labels are exactly the POM instance paths.
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("Top").expect("circuit builds");
    let labels: Vec<&str> = circuit.all_devices().iter().map(|d| d.name()).collect();
    let pom_paths: Vec<&str> =
        design.module("Top").unwrap().instances().iter().map(|i| i.name()).collect();
    assert_eq!(labels, pom_paths, "solver labels == POM instance paths");
    assert_eq!(labels, vec!["v1", "r1", "r2"]);

    let mid = info.nets.get("mid").expect("top net `mid` mapped").clone();
    let read_mid = |r: &piperine_solver::prelude::DcAnalysisResult| -> f64 {
        r.get_node(&mid).expect("v(mid)")
    };

    // Baseline: 10·1k/2k = 5 V.
    let base = circuit.dc(Context::default()).unwrap().solve().unwrap();
    assert!((read_mid(&base) - 5.0).abs() < 1e-9);

    // Solver path: live set of the flattened bundle field on the compiled
    // circuit — no re-elaboration, no rebuild.
    circuit
        .set_element_param("r2", "model_r0", Value::Real(3000.0))
        .expect("live set by pom path");
    let live = circuit.dc(Context::default()).unwrap().solve().unwrap();

    // POM oracle: stage the same (path, param, value), re-elaborate, rebuild.
    design.set_param("r2", "model_r0", piperine_lang::pom::Value::Real(3000.0));
    let staged_design = design.with_overrides_applied("Top").expect("override applies");
    let staged_bodies =
        piperine_codegen::ir::lower_bodies(&staged_design).expect("staged design lowers");
    let mut staged_compiler = CircuitCompiler::new(&staged_design, &staged_bodies);
    let (mut staged_circuit, staged_info) =
        staged_compiler.build_circuit_mapped("Top").expect("staged circuit builds");
    let staged_mid = staged_info.nets.get("mid").unwrap().clone();
    let staged = staged_circuit.dc(Context::default()).unwrap().solve().unwrap();

    let v_live = read_mid(&live);
    let v_staged = staged.get_node(&staged_mid).expect("v(mid)");
    assert!((v_staged - 7.5).abs() < 1e-9, "oracle: 10·3k/4k = 7.5 V, got {v_staged}");
    assert!(
        (v_live - v_staged).abs() < 1e-9,
        "parity: live set {v_live} V vs POM-staged rebuild {v_staged} V"
    );

    // A plain scalar param addresses identically through both interfaces.
    circuit.set_element_param("v1", "dc", Value::Real(20.0)).expect("scalar live set");
    let live2 = circuit.dc(Context::default()).unwrap().solve().unwrap();
    assert!((read_mid(&live2) - 15.0).abs() < 1e-9, "20·3k/4k = 15 V");
}

/// The addressing errors stay loud on JIT-compiled devices too: unknown
/// labels echo the path, unknown params list the flattened candidates.
#[test]
fn jit_device_set_errors_are_loud_with_flattened_param_names() {
    let (design, bodies) = elaborate();
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let mut circuit = compiler.build_circuit("Top").expect("circuit builds");

    let err = circuit
        .set_element_param("x9", "model_r0", Value::Real(1.0))
        .expect_err("unknown label");
    assert!(err.to_string().contains("x9"), "{err}");

    let err = circuit
        .set_element_param("r1", "resistance", Value::Real(1.0))
        .expect_err("unknown param");
    let msg = err.to_string();
    assert!(msg.contains("resistance"), "{msg}");
    assert!(
        msg.contains("model_r0") && msg.contains("model_k"),
        "lists the flattened bundle params: {msg}"
    );
}
