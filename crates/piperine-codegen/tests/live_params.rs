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

/// LIVE-06 independent test: RC step response with a mid-transient live set
/// of R (2k → 1k) at t = 5 µs on JIT-compiled devices. The integrator lands
/// exactly on each scheduled set time (unified breakpoint table, LTE
/// skipped at the edge), and the waveform after the set follows the new
/// time constant — the closed-form solution of a fresh simulation started
/// from the pre-set state — within reltol 1e-3.
#[test]
fn mid_transient_r_set_switches_the_rc_time_constant_at_the_breakpoint() {
    const RC: &str = r#"
        discipline Electrical { potential v : Real; flow i : Real; }

        mod R (inout p : Electrical, inout n : Electrical) {
            param r : Real = 2.0e3;
        }
        analog R { I(p, n) <+ V(p, n) / r; }

        mod C (inout p : Electrical, inout n : Electrical) {
            param c : Real = 1.0e-9;
        }
        analog C { I(p, n) <+ c * ddt(V(p, n)); }

        mod Vsrc (inout p : Electrical, inout n : Electrical) {
            param dc : Real = 0.0;
        }
        analog Vsrc { V(p, n) <- dc; }

        mod Top () {
            wire gnd : Electrical;
            wire vin : Electrical;
            wire out : Electrical;
            v1 : Vsrc(.p = vin, .n = gnd) {};
            r1 : R(.p = vin, .n = out) {};
            c1 : C(.p = out, .n = gnd) {};
        }
    "#;
    let design = parse_and_elaborate(RC, &piperine_lang::SourceMap::dummy())
        .expect("rc elaborates");
    let bodies = piperine_codegen::ir::lower_bodies(&design).expect("rc lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("Top").expect("rc builds");
    let out = info.nets.get("out").expect("net `out`").clone();

    let (t_on, t_sw) = (1.0e-6, 5.0e-6);
    let (tau1, tau2) = (2.0e3 * 1.0e-9, 1.0e3 * 1.0e-9); // RC: 2 µs, then 1 µs
    let opts = piperine_solver::prelude::TransientAnalysisOptions::new(12e-6, 0.1e-6);
    let mut tran = circuit.transient(opts, Context::default()).unwrap();
    // Source turns on at 1 µs (step input), R switches 2k → 1k at 5 µs.
    tran.schedule_set(t_on, "v1", "dc", Value::Real(5.0));
    tran.schedule_set(t_sw, "r1", "r", Value::Real(1.0e3));
    let result = tran.solve().unwrap();

    // Both scheduled times are exact landing points (TRB-11).
    for ts in [t_on, t_sw] {
        assert_eq!(
            result.iter().filter(|s| (s.time() - ts).abs() < 1e-18).count(),
            1,
            "exactly one recorded landing at t = {ts:e}"
        );
    }

    // Closed-form reference: charge with τ1 = RC = 2 µs from t_on, then —
    // fresh run from the pre-set state — settle with τ2 = 1 µs from t_sw.
    let v_sw = 5.0 * (1.0 - (-(t_sw - t_on) / tau1).exp());
    let reference = |t: f64| -> f64 {
        if t <= t_on {
            0.0
        } else if t <= t_sw {
            5.0 * (1.0 - (-(t - t_on) / tau1).exp())
        } else {
            5.0 + (v_sw - 5.0) * (-(t - t_sw) / tau2).exp()
        }
    };

    let reltol = 1e-3;
    for step in result.iter() {
        let t = step.time();
        let got = step.get_node(&out).expect("v(out)");
        let want = reference(t);
        assert!(
            (got - want).abs() <= reltol * 5.0 + 1e-6,
            "t = {t:.4e}: v(out) = {got:.6} vs reference {want:.6}"
        );
    }

    // The new time constant is actually visible after the switch (the
    // waveform departs from the old-τ trajectory by far more than reltol).
    let probe_t = t_sw + 1.5e-6;
    let old_tau_value = 5.0 * (1.0 - (-(probe_t - t_on) / tau1).exp());
    let new_tau_value = reference(probe_t);
    assert!((old_tau_value - new_tau_value).abs() > 0.05, "trajectories separated");

    // No LTE rejection storm around the edges.
    assert!(
        result.stats.steps_rejected <= result.stats.steps_accepted / 5 + 5,
        "rejection storm: {} rejected vs {} accepted",
        result.stats.steps_rejected,
        result.stats.steps_accepted
    );
}


/// LIVE-07: a live set on a *reactive* element mid-transient (capacitance
/// jump) rides the discontinuity machinery — no NaN, no dt collapse, no
/// LTE rejection storm, and the waveform after t matches a fresh
/// simulation started from the pre-set state (closed form) within
/// reltol 1e-3.
#[test]
fn mid_transient_c_jump_stays_accurate_and_storm_free() {
    const RC: &str = r#"
        discipline Electrical { potential v : Real; flow i : Real; }

        mod R (inout p : Electrical, inout n : Electrical) {
            param r : Real = 1.0e3;
        }
        analog R { I(p, n) <+ V(p, n) / r; }

        mod C (inout p : Electrical, inout n : Electrical) {
            param c : Real = 1.0e-9;
        }
        analog C { I(p, n) <+ ddt(c * V(p, n)); }

        mod Vsrc (inout p : Electrical, inout n : Electrical) {
            param dc : Real = 0.0;
        }
        analog Vsrc { V(p, n) <- dc; }

        mod Top () {
            wire gnd : Electrical;
            wire vin : Electrical;
            wire out : Electrical;
            v1 : Vsrc(.p = vin, .n = gnd) {};
            r1 : R(.p = vin, .n = out) {};
            c1 : C(.p = out, .n = gnd) {};
        }
    "#;
    let design = parse_and_elaborate(RC, &piperine_lang::SourceMap::dummy()).expect("elab");
    let bodies = piperine_codegen::ir::lower_bodies(&design).expect("lower");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("Top").expect("build");
    let out = info.nets.get("out").expect("net `out`").clone();

    let (t_on, t_sw) = (1.0e-6, 4.0e-6);
    let (tau1, tau2) = (1.0e3 * 1.0e-9, 1.0e3 * 3.0e-9); // 1 µs, then 3 µs
    let opts = piperine_solver::prelude::TransientAnalysisOptions::new(14e-6, 0.1e-6);
    let mut tran = circuit.transient(opts, Context::default()).unwrap();
    tran.schedule_set(t_on, "v1", "dc", Value::Real(5.0));
    // Capacitance jump 1 nF → 3 nF at constant node voltage: v(out) is the
    // solver state and stays continuous; the time constant triples.
    tran.schedule_set(t_sw, "c1", "c", Value::Real(3.0e-9));
    let result = tran.solve().unwrap();

    let v_sw = 5.0 * (1.0 - (-(t_sw - t_on) / tau1).exp());
    let reference = |t: f64| -> f64 {
        if t <= t_on {
            0.0
        } else if t <= t_sw {
            5.0 * (1.0 - (-(t - t_on) / tau1).exp())
        } else {
            5.0 + (v_sw - 5.0) * (-(t - t_sw) / tau2).exp()
        }
    };

    for step in result.iter() {
        let t = step.time();
        let got = step.get_node(&out).expect("v(out)");
        assert!(got.is_finite(), "NaN at t = {t:e}");
        let want = reference(t);
        assert!(
            (got - want).abs() <= 1e-3 * 5.0 + 1e-6,
            "t = {t:.4e}: v(out) = {got:.6} vs reference {want:.6}"
        );
    }

    // No rejection storm, no dt collapse: the run finishes with a bounded
    // step count and rejections stay a small fraction of acceptances.
    assert!(
        result.stats.steps_rejected <= result.stats.steps_accepted / 5 + 5,
        "rejection storm: {} rejected vs {} accepted",
        result.stats.steps_rejected,
        result.stats.steps_accepted
    );
    assert!(
        result.stats.steps_accepted < 5000,
        "dt collapse: {} accepted steps for a 14 µs run",
        result.stats.steps_accepted
    );
}

/// LIVE-07 (flux leg): a live inductance jump mid-transient. The inductor
/// branch current is the solver state and stays continuous; the RL time
/// constant halves at the set.
#[test]
fn mid_transient_l_jump_stays_accurate_and_storm_free() {
    const RL: &str = r#"
        discipline Electrical { potential v : Real; flow i : Real; }

        mod R (inout p : Electrical, inout n : Electrical) {
            param r : Real = 1.0e3;
        }
        analog R { I(p, n) <+ V(p, n) / r; }

        mod L (inout p : Electrical, inout n : Electrical) {
            param l : Real = 1.0e-2;
        }
        analog L { V(p, n) <- l * ddt(I(p, n)); }

        mod Vsrc (inout p : Electrical, inout n : Electrical) {
            param dc : Real = 0.0;
        }
        analog Vsrc { V(p, n) <- dc; }

        mod Top () {
            wire gnd : Electrical;
            wire vin : Electrical;
            wire mid : Electrical;
            v1 : Vsrc(.p = vin, .n = gnd) {};
            r1 : R(.p = vin, .n = mid) {};
            l1 : L(.p = mid, .n = gnd) {};
        }
    "#;
    let design = parse_and_elaborate(RL, &piperine_lang::SourceMap::dummy()).expect("elab");
    let bodies = piperine_codegen::ir::lower_bodies(&design).expect("lower");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("Top").expect("build");
    let mid = info.nets.get("mid").expect("net `mid`").clone();

    // τ = L/R: 10 µs, then 5 µs after the set. Probe v(mid) = i·R_L-branch…
    // v(mid) tracks the inductor: v(mid) = 5 − i·R, and i(t) is the classic
    // RL exponential — continuous through the L jump.
    let (t_on, t_sw) = (2.0e-6, 20.0e-6);
    let (tau1, tau2) = (1.0e-2 / 1.0e3, 5.0e-3 / 1.0e3); // 10 µs, then 5 µs
    let opts = piperine_solver::prelude::TransientAnalysisOptions::new(60e-6, 0.2e-6);
    let mut tran = circuit.transient(opts, Context::default()).unwrap();
    tran.schedule_set(t_on, "v1", "dc", Value::Real(5.0));
    tran.schedule_set(t_sw, "l1", "l", Value::Real(5.0e-3));
    let result = tran.solve().unwrap();

    let i_inf = 5.0 / 1.0e3;
    let i_sw = i_inf * (1.0 - (-(t_sw - t_on) / tau1).exp());
    let i_ref = |t: f64| -> f64 {
        if t <= t_on {
            0.0
        } else if t <= t_sw {
            i_inf * (1.0 - (-(t - t_on) / tau1).exp())
        } else {
            i_inf + (i_sw - i_inf) * (-(t - t_sw) / tau2).exp()
        }
    };

    for step in result.iter() {
        let t = step.time();
        let v = step.get_node(&mid).expect("v(mid)");
        assert!(v.is_finite(), "NaN at t = {t:e}");
        // v(mid) = 5·(source on) − i(t)·R
        let src = if t > t_on { 5.0 } else { 0.0 };
        let want = src - i_ref(t) * 1.0e3;
        assert!(
            (v - want).abs() <= 1e-3 * 5.0 + 1e-6,
            "t = {t:.4e}: v(mid) = {v:.6} vs reference {want:.6}"
        );
    }

    assert!(
        result.stats.steps_rejected <= result.stats.steps_accepted / 5 + 5,
        "rejection storm: {} rejected vs {} accepted",
        result.stats.steps_rejected,
        result.stats.steps_accepted
    );
    assert!(
        result.stats.steps_accepted < 5000,
        "dt collapse: {} accepted steps for a 60 µs run",
        result.stats.steps_accepted
    );
}

