//! SC-04 enabler — transient full-state re-entry: an RC charge run 0→T,
//! captured at T, re-entered T→2T equals a single 0→2T run within the
//! solver's tolerance class. This is the PSS shooting seam
//! (`TransientSolver::with_initial_state`).

use std::path::PathBuf;

use piperine_codegen::CircuitCompiler;
use piperine_lang::SourceMap;
use piperine_solver::abi::{AnalogVariable, InitialValue};
use piperine_solver::prelude::{Context, TransientAnalysisOptions};

/// `v(out) = 0` initial condition (discharged capacitor) so the runs show
/// the charge transient instead of the DC steady state.
fn discharged_ic(
    info: &piperine_codegen::device::CircuitBuildInfo,
    circuit: &piperine_solver::prelude::CircuitInstance,
) -> Vec<InitialValue<piperine_solver::abi::AnalogReference, f64>> {
    let out = info.nets.get("out").expect("net `out`").clone();
    let reference = circuit
        .netlist()
        .reference_for(&AnalogVariable::Node(out))
        .expect("out is solved")
        .clone();
    vec![InitialValue { reference, value: 0.0 }]
}

const RC: &str = r#"
    discipline Electrical { potential v : Real; flow i : Real; }

    mod R (inout p : Electrical, inout n : Electrical) {
        param r : Real = 1.0e3;
    }
    analog R { I(p, n) <+ V(p, n) / r; }

    mod C (inout p : Electrical, inout n : Electrical) {
        param c : Real = 1.0e-6;
    }
    analog C { I(p, n) <+ c * ddt(V(p, n)); }

    mod Vsrc (inout p : Electrical, inout n : Electrical) {
        param dc : Real = 5.0;
    }
    analog Vsrc { V(p, n) <- dc; }

    mod Top () {
        wire gnd : Electrical;
        wire top : Electrical;
        wire out : Electrical;
        v1 : Vsrc(.p = top, .n = gnd) {};
        r1 : R(.p = top, .n = out) {};
        c1 : C(.p = out, .n = gnd) {};
    }
"#;

fn headers_source_map() -> SourceMap {
    let headers =
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// τ = RC = 1 ms; T = 1 ms. Continuous 0→2T vs captured-at-T re-entry T→2T:
/// endpoints agree within the solver tolerance class (1e-9 + 1e-3·|v|), and
/// both sit near the analytic 5·(1−e^(−t/τ)).
#[test]
fn reentry_from_captured_state_matches_continuous_run() {
    let design =
        piperine_lang::parse_and_elaborate(RC, &headers_source_map()).expect("rc elaborates");
    let bodies = piperine_codegen::ir::lower_bodies(&design).expect("lower");
    let mut compiler = CircuitCompiler::new(&design, &bodies);

    let t_half = 1.0e-3;
    let read_out = |step: &piperine_solver::prelude::TransientStep,
                    info: &piperine_codegen::device::CircuitBuildInfo| {
        let out = info.nets.get("out").expect("net `out`");
        step.get_node(out).expect("v(out)")
    };

    // Reference: one continuous 0→2T run.
    let (mut circuit_a, info_a) = compiler.build_circuit_mapped("Top").expect("build A");
    circuit_a.init_digital().expect("digital");
    circuit_a.rebuild_digital_topology();
    let opts_a = TransientAnalysisOptions::new(2.0 * t_half, t_half / 100.0);
    let ic_a = discharged_ic(&info_a, &circuit_a);
    let mut solver_a = circuit_a.transient(opts_a, Context::default()).expect("tran A");
    solver_a.apply_initial_conditions(ic_a);
    let full = solver_a.solve().expect("solve A");
    let v_full_end = read_out(full.last().expect("A has steps"), &info_a);

    // Leg 1: 0→T, capture the final step.
    let (mut circuit_b, info_b) = compiler.build_circuit_mapped("Top").expect("build B");
    circuit_b.init_digital().expect("digital");
    circuit_b.rebuild_digital_topology();
    let ic_b = discharged_ic(&info_b, &circuit_b);
    let mut solver_b1 = circuit_b
        .transient(TransientAnalysisOptions::new(t_half, t_half / 100.0), Context::default())
        .expect("tran B1");
    solver_b1.apply_initial_conditions(ic_b);
    let leg1 = solver_b1.solve().expect("solve B1");
    let captured = leg1.last().expect("B1 has steps").clone();
    let v_captured = read_out(&captured, &info_b);
    let analytic_half = 5.0 * (1.0 - (-1.0_f64).exp());
    assert!(
        (v_captured - analytic_half).abs() < 5.0e-3,
        "capture point sanity: v(T)={v_captured} vs analytic {analytic_half}"
    );

    // Leg 2: re-enter from the captured state, T→2T on the same circuit.
    let opts_b2 = TransientAnalysisOptions::new(2.0 * t_half, t_half / 100.0).with_start(t_half);
    let mut solver_b2 = circuit_b.transient(opts_b2, Context::default()).expect("tran B2");
    solver_b2.with_initial_state(&captured);
    let leg2 = solver_b2.solve().expect("solve B2");
    let first_b2 = leg2.iter().next().expect("B2 has steps");
    assert!(
        (read_out(first_b2, &info_b) - v_captured).abs() < 1e-12,
        "re-entry starts exactly at the captured state"
    );
    let v_reentry_end = read_out(leg2.last().expect("B2 has steps"), &info_b);

    let tol = 1e-9 + 1e-3 * v_full_end.abs();
    assert!(
        (v_reentry_end - v_full_end).abs() <= tol,
        "re-entered end v={v_reentry_end} vs continuous v={v_full_end} (tol {tol})"
    );
    let analytic_end = 5.0 * (1.0 - (-2.0_f64).exp());
    assert!(
        (v_reentry_end - analytic_end).abs() < 1.0e-2,
        "analytic sanity: v(2T)={v_reentry_end} vs {analytic_end}"
    );
}
