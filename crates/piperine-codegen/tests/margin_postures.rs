//! The three postures against one violated `require` (dv-core T17,
//! DVC-15): a divider whose `dut1` carries `require head : V(p, n) <= hi`
//! with hi = 0.4 across 0.5 V — margin −0.1, violated at every point.
//! `strict` aborts the analysis naming the constraint, the instance path,
//! the time, and the margin value; `collect` completes and records every
//! violation with its margin and time; `off` completes with nothing
//! called and nothing reported. A ramp variant crosses the limit
//! mid-transient, proving the accepted-step abort path and the
//! worst-margin-with-argmin record.

//! **MD-45 regression — present-and-red by design.** The fixtures below
//! still use the retired PHDL `constraint Mod { … }` grammar and
//! `lower_bodies` produces `constraints`/`cover` = `None` from the VAMS
//! frontend, so these tests fail until the `constraints-on-vams` feature
//! (`.specs/features/constraints-on-vams/`) rebuilds the producer with
//! `constraint begin … end`. They are the executable specification of the
//! margin/coverage machinery — restore them to green when that feature
//! lands; do not delete them.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{CheckPosture, ValidationSeverity};
use piperine_solver::prelude::{CircuitInstance, Context, TransientAnalysisOptions};

const DISCIPLINE: &str = "discipline Electrical { potential v : Real; flow i : Real; }";

/// The divider fixture: 1 V across r1 + dut1 (both 1 kΩ) puts 0.5 V
/// across `dut1`; its `require head : V(p, n) <= hi` (hi = 0.4) is
/// violated with margin 0.4 − 0.5 = −0.1 at every point.
fn divider_circuit() -> CircuitInstance {
    let src = format!(
        "{DISCIPLINE}
mod Vsrc(inout p: Electrical, inout n: Electrical) {{ param dc: Real = 1.0; }}
analog Vsrc {{ V(p, n) <- dc; }}
mod R(inout p: Electrical, inout n: Electrical) {{ param r: Real = 1.0e3; }}
analog R {{ I(p, n) <+ V(p, n) / r; }}
mod Dut(inout p: Electrical, inout n: Electrical) {{ param hi: Real = 0.4; }}
analog Dut {{ I(p, n) <+ V(p, n) / 1e3; }}
constraint Dut {{ require head : V(p, n) <= hi; }}
mod Top() {{
    wire gnd : Electrical;
    wire in  : Electrical;
    wire out : Electrical;
    v1   : Vsrc(.p = in,  .n = gnd);
    r1   : R(.p = in,     .n = out);
    dut1 : Dut(.p = out,  .n = gnd);
}}
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    compiler.build_circuit("Top").expect("circuit builds")
}

/// The ramp fixture: the source sweeps 0.3 V → 0.7 V over 1 µs, so the
/// same `require head` (hi = 0.4) is satisfied at the t = 0 operating
/// point (margin +0.1) and crosses zero at t = 0.25 µs — the violation
/// happens mid-transient, on an accepted step.
fn ramp_circuit() -> CircuitInstance {
    let src = format!(
        "{DISCIPLINE}
mod Ramp(inout p: Electrical, inout n: Electrical) {{}}
analog Ramp {{ V(p, n) <- 0.3 + 0.4 * ($abstime / 1e-6); }}
mod Dut(inout p: Electrical, inout n: Electrical) {{ param hi: Real = 0.4; }}
analog Dut {{ I(p, n) <+ V(p, n) / 1e3; }}
constraint Dut {{ require head : V(p, n) <= hi; }}
mod Top() {{
    wire gnd : Electrical;
    wire in  : Electrical;
    v1   : Ramp(.p = in, .n = gnd);
    dut1 : Dut(.p = in,  .n = gnd);
}}
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    compiler.build_circuit("Top").expect("circuit builds")
}

fn context(posture: CheckPosture) -> Context {
    Context { checks: posture, ..Default::default() }
}

fn tran(circuit: &mut CircuitInstance, posture: CheckPosture) -> piperine_solver::prelude::TransientAnalysisResult {
    let opts = TransientAnalysisOptions::new(1e-6, 1e-7);
    circuit.transient(opts, context(posture)).unwrap().solve().expect("the analysis completes")
}

/// DVC-15 AC1: `strict` (the default) aborts the analysis on the violated
/// `require`, failing loud with the constraint name, the instance path,
/// the time, and the margin value.
#[test]
fn strict_aborts_naming_constraint_instance_time_and_margin() {
    let mut circuit = divider_circuit();
    let opts = TransientAnalysisOptions::new(1e-6, 1e-7);
    let err = circuit
        .transient(opts, Context::default())
        .unwrap()
        .solve()
        .expect_err("strict + a violated require fails the analysis");
    let msg = err.to_string();
    assert!(msg.contains("head"), "names the constraint: {msg}");
    assert!(msg.contains("dut1"), "names the instance path: {msg}");
    assert!(msg.contains("at t ="), "names the time: {msg}");
    assert!(
        msg.contains(&format!("margin = {:.6}", 0.4 - 0.5)),
        "names the margin value (−0.1): {msg}"
    );
}

/// DVC-15 AC2: `collect` completes the same run and records the worst
/// margin with its argmin — every accepted step's finding carries the
/// margin (−0.1) and the step's time, so the worst across the run is
/// −0.1 at an accepted-step coordinate.
#[test]
fn collect_completes_and_records_the_worst_margin_with_argmin() {
    let mut circuit = divider_circuit();
    let result = tran(&mut circuit, CheckPosture::Collect);

    let findings = &result.stats.validation;
    assert_eq!(
        findings.len(),
        result.stats.steps_accepted,
        "one finding per accepted step — the run was not cut short"
    );
    for f in findings {
        assert_eq!(f.severity, ValidationSeverity::Error);
        assert_eq!(f.label, "head");
        assert_eq!(f.instance_path, "dut1");
        assert!((f.value - (0.4 - 0.5)).abs() < 1e-9, "margin −0.1 at every point, got {}", f.value);
    }
    let worst = findings.iter().map(|f| f.value).fold(f64::INFINITY, f64::min);
    assert!((worst - (0.4 - 0.5)).abs() < 1e-9, "the recorded worst margin is −0.1");
    let argmin = findings.iter().find(|f| f.value == worst).map(|f| f.time).expect("the worst has a coordinate");
    assert!(
        (0..result.len()).any(|i| result.get(i).is_some_and(|s| s.time() == argmin)),
        "the argmin is an accepted step's time"
    );
}

/// DVC-15 AC3: `off` completes the same run with nothing reported — the
/// kernels are never called (proven at the device level in
/// `margin_evaluation.rs`), so there is nothing to record.
#[test]
fn off_completes_and_reports_nothing() {
    let mut circuit = divider_circuit();
    let result = tran(&mut circuit, CheckPosture::Off);
    assert!(result.stats.validation.is_empty(), "off reports nothing");
    assert!(result.stats.steps_accepted > 1, "the run completed normally");
}

/// DVC-15 AC1 on the accepted-step path: with the OP satisfied, the
/// crossing happens mid-transient — `strict` aborts there (not at the
/// operating point), still naming the constraint and instance.
#[test]
fn strict_aborts_mid_transient_when_the_op_is_satisfied() {
    let mut circuit = ramp_circuit();
    let opts = TransientAnalysisOptions::new(1e-6, 1e-7);
    let err = circuit
        .transient(opts, Context::default())
        .unwrap()
        .solve()
        .expect_err("the ramp crosses hi = 0.4 mid-run — strict aborts");
    let msg = err.to_string();
    assert!(msg.contains("head"), "names the constraint: {msg}");
    assert!(msg.contains("dut1"), "names the instance path: {msg}");
    // The crossing is at V = 0.4 → t = 0.25 µs; the abort fires at the
    // first accepted step past it, never at t = 0 (the OP was clean).
    // t = 0 formats as `at t = 0:` in the abort message.
    assert!(!msg.contains("at t = 0:"), "the satisfied OP did not abort: {msg}");
}

/// DVC-15 AC2 with a moving worst: `collect` on the ramp records the
/// worst margin across the whole run — 0.4 − 0.7 = −0.3 at the final
/// point, t = 1 µs — with the swept coordinate as its argmin.
#[test]
fn collect_records_the_worst_margin_across_the_run_with_its_argmin() {
    let mut circuit = ramp_circuit();
    let result = tran(&mut circuit, CheckPosture::Collect);

    let findings = &result.stats.validation;
    assert!(!findings.is_empty(), "the crossing produced findings");
    assert!(findings.iter().all(|f| f.severity == ValidationSeverity::Error && f.label == "head"));
    // Satisfied points produce no findings: the earliest finding is past
    // the crossing at t = 0.25 µs.
    let first_t = findings.iter().map(|f| f.time).fold(f64::INFINITY, f64::min);
    assert!(first_t > 0.0, "no finding at the satisfied OP");

    let worst = findings.iter().map(|f| f.value).fold(f64::INFINITY, f64::min);
    assert!(
        (worst - (0.4 - 0.7)).abs() < 1e-9,
        "worst margin = hi − V(1 µs) = 0.4 − 0.7 = −0.3, got {worst}"
    );
    let argmin = findings.iter().find(|f| f.value == worst).map(|f| f.time).expect("the worst has a coordinate");
    assert!(
        (argmin - 1e-6).abs() < 1e-12,
        "the worst margin occurs at the final point t = 1 µs, got {argmin}"
    );
}
