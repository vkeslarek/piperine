//! Margin evaluation with provenance (dv-core T16, DVC-10): the device
//! evaluates the margin kernels at each accepted point, tracks the worst
//! (minimum) margin and its argmin per declared margin per instance, and
//! records violations as `Error` findings — a `require` crossing zero, or
//! a non-finite margin (loud, never clamped). The instance path on a
//! finding is the authored hierarchical path (the flatten pass's spliced
//! label), stamped at the element seam. `checks=off` never calls a margin
//! kernel: the extremum bank stays at its never-evaluated state across a
//! violating point.

//! **MD-45 regression — present-and-red by design.** The fixtures below
//! still use the retired PHDL `constraint Mod { … }` grammar and
//! `lower_bodies` produces `constraints`/`cover` = `None` from the VAMS
//! frontend, so these tests fail until the `constraints-on-vams` feature
//! (`.specs/features/constraints-on-vams/`) rebuilds the producer with
//! `constraint begin … end`. They are the executable specification of the
//! margin/coverage machinery — restore them to green when that feature
//! lands; do not delete them.

use std::sync::Arc;

use piperine_codegen::device::{AnalogInstance, MarginExtremum};
use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{
    CheckPosture, CircularArrayBuffer2, ElementCapabilities, Netlist, NodeIdentifier,
    ValidationSeverity,
};
use piperine_solver::prelude::{CircuitInstance, Context, TransientAnalysisOptions};

const DISCIPLINE: &str = "discipline Electrical { potential v : Real; flow i : Real; }";

/// Compile `Dut` (two terminals, `hi`/`lo` params, 1 mS body) with the
/// given constraint block verbatim.
fn dut_kernel(constraint: &str) -> Arc<AnalogKernel> {
    let src = format!(
        "{DISCIPLINE}
mod Dut(inout p: Electrical, inout n: Electrical) {{ param hi: Real = 3.0; param lo: Real = 1.0; }}
analog Dut {{ I(p, n) <+ V(p, n) / 1e3; }}
{constraint}
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    Arc::new(AnalogKernel::compile(&bodies["Dut"]).expect("compile"))
}

/// One instance of `kernel` on two fresh nodes with the given params.
fn instance(label: &str, kernel: Arc<AnalogKernel>, params: Vec<f64>) -> AnalogInstance {
    let mut netlist = Netlist::new();
    let terminals = [NodeIdentifier::Anonymous(0), NodeIdentifier::Anonymous(1)];
    AnalogInstance::new(label, kernel, &terminals, params, 0b11, u64::MAX, &mut netlist).expect("instance")
}

/// Drive one accepted point: `V(p, n) = v` at time `t`. Margin evaluation
/// is decoupled from the accept hook (dv-core T18 — the drivers call
/// `evaluate_constraints` at design §C3's points, never the operator-
/// seeding accept), so an accepted *evaluation* point is the pair. The
/// default analysis kind is `Dc` (no `@ … { … }` scope in these fixtures
/// — every margin applies).
fn accept(inst: &mut AnalogInstance, v: f64, t: f64) {
    let mut state = CircularArrayBuffer2::new(1, 2);
    let row = ndarray::Array1::from_vec(vec![v, 0.0]);
    state.push(&row.view());
    inst.accept_timestep(&state, t);
    inst.evaluate_constraints(&state, t, piperine_solver::abi::AnalysisKind::Dc);
}

fn extremum(inst: &AnalogInstance, index: usize) -> (f64, Option<f64>) {
    let MarginExtremum { worst, argmin } = &inst.margin_extrema()[index];
    (*worst, *argmin)
}

/// DVC-10: the worst margin and its argmin are tracked per declared
/// margin — the head require's worst is its most-violated point, the
/// foot require's worst is its least-headroom point, each with the time
/// it occurred at. The findings of a point are drained by the next
/// evaluation: a clean final point reports `None` (the `Option` shape).
#[test]
fn worst_margin_and_argmin_tracked_per_constraint() {
    let kernel = dut_kernel(
        "constraint Dut {\n\
             require head : V(p, n) <= hi;\n\
             require foot : V(p, n) >= lo;\n\
         }",
    );
    let mut inst = instance("d1", kernel, vec![3.0, 1.0]);
    assert_eq!(inst.margin_extrema().len(), 2);
    assert_eq!(extremum(&inst, 0), (f64::INFINITY, None), "never evaluated yet");

    // t=1ns: head m = 3−2 = 1, foot m = 2−1 = 1 — both satisfied.
    accept(&mut inst, 2.0, 1e-9);
    assert!(inst.validation_report().is_none(), "a clean point reports nothing");

    // t=2ns: head m = 3−3.5 = −0.5 (violated, new worst), foot m = 2.5.
    accept(&mut inst, 3.5, 2e-9);
    let report = inst.validation_report().expect("the violating point reports");
    let findings = report.findings();
    assert_eq!(findings.len(), 1, "only the head require crossed zero");
    assert_eq!(findings[0].severity, ValidationSeverity::Error);
    assert_eq!(findings[0].label, "head");
    assert_eq!(findings[0].value, -0.5, "the finding carries the margin at the firing point");
    assert_eq!(findings[0].time, 2e-9);

    // t=3ns: head m = 0.5 (recovered — not worse), foot m = 1.5.
    accept(&mut inst, 2.5, 3e-9);
    assert_eq!(extremum(&inst, 0), (-0.5, Some(2e-9)), "head worst + argmin hold the worst point");
    assert_eq!(extremum(&inst, 1), (1.0, Some(1e-9)), "foot worst is its least-headroom point");
    assert!(inst.validation_report().is_none(), "a clean point drains the last point's findings");
}

/// DVC-10: the tracking is per instance — the same compiled kernel
/// (`Arc`-shared) serves two instances whose extrema advance
/// independently from their own params and points.
#[test]
fn extrema_are_tracked_per_instance_on_a_shared_kernel() {
    let kernel = dut_kernel("constraint Dut { require head : V(p, n) <= hi; }");
    let mut first = instance("d1", kernel.clone(), vec![3.0, 1.0]);
    let mut second = instance("d2", kernel, vec![5.0, 1.0]);

    // The same point violates `d1` (3−4.5 = −1.5) but satisfies `d2`
    // (5−4.5 = 0.5) — parameterization, not recompilation.
    accept(&mut first, 4.5, 1e-9);
    accept(&mut second, 4.5, 1e-9);
    assert_eq!(extremum(&first, 0), (-1.5, Some(1e-9)));
    assert_eq!(extremum(&second, 0), (0.5, Some(1e-9)));
    assert!(first.validation_report().is_some());
    assert!(second.validation_report().is_none(), "second instance satisfied at the same volts");
}

/// DVC-10 (spec margins AC5): a non-finite margin at an evaluated point
/// is a loud violation — an `Error` finding carrying the raw non-finite
/// value, never clamped to something finite — and it is never tracked as
/// an extremum (the worst stays at its never-evaluated `+inf`).
#[test]
fn non_finite_margin_is_a_loud_violation_never_clamped() {
    let kernel = dut_kernel("constraint Dut { require nz : 1.0 / V(p, n) >= 0.0; }");
    let mut inst = instance("d1", kernel, vec![3.0, 1.0]);

    // V = 0 → margin = 1/0 = +inf: non-finite at an evaluated point.
    accept(&mut inst, 0.0, 1e-9);
    let report = inst.validation_report().expect("a non-finite margin reports");
    let findings = report.findings();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, ValidationSeverity::Error);
    assert_eq!(findings[0].label, "nz");
    assert_eq!(findings[0].time, 1e-9);
    assert!(
        findings[0].value.is_infinite() && findings[0].value.is_sign_positive(),
        "the finding carries the raw +inf — clamping would hide the violation, got {}",
        findings[0].value
    );
    assert_eq!(extremum(&inst, 0), (f64::INFINITY, None), "non-finite is reported, never tracked");

    // A later finite point evaluates and tracks normally.
    accept(&mut inst, 2.0, 2e-9);
    assert_eq!(extremum(&inst, 0), (0.5, Some(2e-9)), "margin 1/2 at V=2");
    assert!(inst.validation_report().is_none(), "m = +0.5 ≥ 0 is satisfied");
}

/// DVC-12 AC3: `checks=off` means the margin kernels are never called —
/// across a violating point the extremum bank stays at its
/// never-evaluated state and nothing is reported. Flipping the posture
/// back (run data, MD-18 — no recompilation) evaluates the very next
/// point normally.
#[test]
fn checks_off_never_calls_the_margin_kernels() {
    let kernel = dut_kernel("constraint Dut { require head : V(p, n) <= hi; }");
    let mut inst = instance("d1", kernel, vec![3.0, 1.0]);

    inst.set_check_posture(CheckPosture::Off);
    accept(&mut inst, 4.5, 1e-9); // would violate (3−4.5 = −1.5) if evaluated
    assert_eq!(
        extremum(&inst, 0),
        (f64::INFINITY, None),
        "the kernels were not called — a call would have tracked the violation"
    );
    assert!(inst.validation_report().is_none());

    inst.set_check_posture(CheckPosture::Strict);
    accept(&mut inst, 4.5, 2e-9);
    assert_eq!(extremum(&inst, 0), (-1.5, Some(2e-9)), "posture change takes effect at once");
    assert!(inst.validation_report().is_some());
}

/// The nested-fixture circuit: `Top` drives `x : Seg`, which holds
/// `d : Dut` with `require head : V(p, n) <= hi` (hi = 0.4) across a
/// 0.5 V source — the require is violated (margin −0.1) at every point.
fn nested_violated_circuit() -> CircuitInstance {
    let src = format!(
        "{DISCIPLINE}
mod Vsrc(inout p: Electrical, inout n: Electrical) {{ param dc: Real = 0.5; }}
analog Vsrc {{ V(p, n) <- dc; }}
mod Dut(inout p: Electrical, inout n: Electrical) {{ param hi: Real = 0.4; }}
analog Dut {{ I(p, n) <+ V(p, n) / 1e3; }}
constraint Dut {{ require head : V(p, n) <= hi; }}
mod Seg(inout p: Electrical, inout n: Electrical) {{ d : Dut(.p = p, .n = n); }}
mod Top() {{
    wire gnd : Electrical;
    wire in  : Electrical;
    v1 : Vsrc(.p = in, .n = gnd);
    x  : Seg(.p = in, .n = gnd);
}}
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    compiler.build_circuit("Top").expect("circuit builds")
}

/// DVC-10: the instance path a finding reports is the authored
/// hierarchical path — the flatten pass's spliced `x.d`, not a
/// codegen-internal label — with the margin value and time of the
/// firing point. Proven end to end: a full transient over the nested
/// fixture, then the finding read back through the `Element` hook.
#[test]
fn findings_carry_the_authored_hierarchical_instance_path() {
    let mut circuit = nested_violated_circuit();
    let dut = circuit
        .all_devices()
        .iter()
        .find(|d| d.name() == "x.d")
        .expect("the flattened instance keeps its spliced authored path");

    // The device with declared margins asks to be polled (DVC-14); the
    // constraint-free source does not.
    assert!(dut.capabilities().contains(ElementCapabilities::EMITS_VALIDATION));
    let source = circuit
        .all_devices()
        .iter()
        .find(|d| d.name() == "v1")
        .expect("the source instance");
    assert!(
        !source.capabilities().contains(ElementCapabilities::EMITS_VALIDATION),
        "no declared margins — the bit stays clear"
    );

    let opts = TransientAnalysisOptions::new(1e-6, 1e-7);
    // Collect: the violated require must not cut the run short — the
    // finding is read back after the last accepted step (posture
    // behavior itself is T17's `margin_postures.rs`).
    let collect = Context { checks: CheckPosture::Collect, ..Default::default() };
    let result = circuit.transient(opts, collect).unwrap().solve().unwrap();
    let last = result.get(result.len() - 1).expect("accepted steps recorded");

    let dut = circuit
        .all_devices()
        .iter()
        .find(|d| d.name() == "x.d")
        .expect("the dut device");
    let report = dut.validation_reports().expect("the violated require reports");
    let findings = report.findings();
    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding.severity, ValidationSeverity::Error);
    assert_eq!(finding.label, "head");
    assert_eq!(finding.instance_path, "x.d", "the authored hierarchical path, never a flat label");
    assert!(
        (finding.value - (0.4 - 0.5)).abs() < 1e-9,
        "margin = hi − V = 0.4 − 0.5 = −0.1, got {}",
        finding.value
    );
    assert_eq!(finding.time, last.time(), "the finding fired at the last accepted point");
}

/// DVC-12 AC3 end to end: a transient over the violated fixture with
/// `checks=off` completes with nothing reported — the posture reaches the
/// device through the per-assemble `Context` handoff (`Element::update`),
/// so the accept hook never evaluates.
#[test]
fn checks_off_end_to_end_reports_nothing() {
    let mut circuit = nested_violated_circuit();
    let opts = TransientAnalysisOptions::new(1e-6, 1e-7);
    let off = Context { checks: CheckPosture::Off, ..Default::default() };
    circuit.transient(opts, off).unwrap().solve().unwrap();

    let dut = circuit
        .all_devices()
        .iter()
        .find(|d| d.name() == "x.d")
        .expect("the dut device");
    assert!(
        dut.validation_reports().is_none(),
        "checks=off: the kernels never ran, so there is nothing to report"
    );
}

/// A module with no constraint block allocates no margin runtime: the
/// extremum bank is empty and a point evaluates to nothing.
#[test]
fn no_constraint_block_allocates_nothing() {
    let kernel = dut_kernel("");
    let mut inst = instance("d1", kernel, vec![3.0, 1.0]);
    assert!(inst.margin_extrema().is_empty());
    accept(&mut inst, 99.0, 1e-9);
    assert!(inst.validation_report().is_none());
}
