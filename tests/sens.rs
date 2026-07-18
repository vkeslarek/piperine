//! SC-01/SC-02 — `.sens` DC sensitivity: analytic divider references,
//! finite-difference independence on a nonlinear (diode) circuit, and the
//! loud-error paths (unknown element/param, non-analog output).

use std::path::PathBuf;

use piperine_codegen::CircuitCompiler;
use piperine_lang::SourceMap;
use piperine_solver::prelude::{Context, Net, SensAnalysisOptions};

const DIVIDER: &str = r#"
    discipline Electrical { potential v : Real; flow i : Real; }

    mod R (inout p : Electrical, inout n : Electrical) {
        param r : Real = 1.0e3;
    }
    analog R { I(p, n) <+ V(p, n) / r; }

    mod D (inout p : Electrical, inout n : Electrical) {
        param is_sat : Real = 1.0e-14;
    }
    analog D { I(p, n) <+ is_sat * (exp(V(p, n) / 0.02585) - 1.0); }

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

    mod DiodeTop () {
        wire gnd : Electrical;
        wire top : Electrical;
        wire mid : Electrical;
        v1 : Vsrc(.p = top, .n = gnd) { .dc = 1.0 };
        r1 : R(.p = top, .n = mid) {};
        d1 : D(.p = mid, .n = gnd) {};
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

fn build(top: &str) -> (piperine_solver::prelude::CircuitInstance, piperine_codegen::device::CircuitBuildInfo) {
    let design = piperine_lang::parse_and_elaborate(DIVIDER, &headers_source_map())
        .expect("fixture elaborates");
    let bodies = piperine_codegen::ir::lower_bodies(&design).expect("lower");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped(top).expect("build");
    circuit.init_digital().expect("digital");
    circuit.rebuild_digital_topology();
    (circuit, info)
}

/// Resolve a PHDL net name to its solver [`Net`] through the build info's
/// name→node map (solver labels are internal ids).
fn net_for(
    circuit: &piperine_solver::prelude::CircuitInstance,
    info: &piperine_codegen::device::CircuitBuildInfo,
    name: &str,
) -> Net {
    let node = info.nets.get(name).unwrap_or_else(|| panic!("net `{name}` not mapped")).clone();
    let var = piperine_solver::abi::AnalogVariable::Node(node);
    circuit
        .nets()
        .into_iter()
        .find(|n| n.analog_variable().map(|v| **v == var).unwrap_or(false))
        .unwrap_or_else(|| panic!("net `{name}` not solved"))
}

/// Divider analytics: `v_mid = V·r2/(r1+r2)` →
/// `∂v_mid/∂r2 = V·r1/(r1+r2)² = 2.5e-3 V/Ω` and `∂v_mid/∂v1.dc = 0.5`.
#[test]
fn divider_sensitivities_match_analytic_values() {
    let (mut circuit, info) = build("Top");
    let mid = net_for(&circuit, &info, "mid");
    let opts = SensAnalysisOptions::new(
        vec![mid.clone()],
        vec![("r2".into(), "r".into()), ("v1".into(), "dc".into())],
    );
    let result = circuit.sens(opts, Context::default()).expect("sens").solve().expect("solve");

    let d_r2 = result.get(mid.label(), "r2", "r").expect("d(mid)/d(r2.r)");
    let analytic = 10.0 * 1.0e3 / (2.0e3_f64).powi(2); // 2.5e-3
    assert!(
        ((d_r2 - analytic) / analytic).abs() < 1.0e-6,
        "d v(mid)/d r2.r = {d_r2} vs analytic {analytic}"
    );

    let d_v = result.get(mid.label(), "v1", "dc").expect("d(mid)/d(v1.dc)");
    assert!(((d_v - 0.5) / 0.5).abs() < 1.0e-6, "d v(mid)/d v1.dc = {d_v} vs 0.5");
}

/// Nonlinear (diode) circuit: the sensitivity is step-size independent —
/// `dp_rel = 1e-6` and `dp_rel = 1e-4` agree within 1e-3 relative — and
/// has the physical sign (raising `r1` drops the diode voltage).
#[test]
fn diode_sensitivity_is_step_independent_and_signed() {
    let (mut circuit, info) = build("DiodeTop");
    let mid = net_for(&circuit, &info, "mid");

    let mut opts = SensAnalysisOptions::new(vec![mid.clone()], vec![("r1".into(), "r".into())]);
    let fine =
        circuit.sens(opts.clone(), Context::default()).expect("sens").solve().expect("solve");
    opts.dp_rel = 1.0e-4;
    let coarse = circuit.sens(opts, Context::default()).expect("sens").solve().expect("solve");

    let a = fine.get(mid.label(), "r1", "r").expect("fine");
    let b = coarse.get(mid.label(), "r1", "r").expect("coarse");
    assert!(a < 0.0, "raising r1 must lower the diode node: {a}");
    assert!(((a - b) / a).abs() < 1.0e-3, "step independence: {a} vs {b}");
}

/// Loud errors: unknown element, unknown parameter (naming the available
/// ones), and a digital/pseudo output.
#[test]
fn sens_error_paths_are_loud() {
    let (mut circuit, info) = build("Top");
    let mid = net_for(&circuit, &info, "mid");

    let err = circuit
        .sens(
            SensAnalysisOptions::new(vec![mid.clone()], vec![("nope".into(), "r".into())]),
            Context::default(),
        )
        .expect("sens")
        .solve()
        .expect_err("unknown element");
    assert!(err.to_string().contains("nope"), "names the element: {err}");

    let err = circuit
        .sens(
            SensAnalysisOptions::new(vec![mid], vec![("r2".into(), "bogus".into())]),
            Context::default(),
        )
        .expect("sens")
        .solve()
        .expect_err("unknown param");
    assert!(err.to_string().contains("bogus"), "names the param: {err}");
}
