//! The margins channel (dv-core T21, DVC-18, D3): margins travel as their
//! own result channel — `OpResult::margins`, `Trace::margins`,
//! `AcTrace::margins` — never folded into waveform rows. A **pointwise**
//! margin carries worst + argmin (time/frequency/swept coordinate) +
//! instance path; a **reduced** margin carries one value and **no**
//! argmin, and the channel states which kind it is. `requires_ok` and
//! `first_violation` are the common assertion shape on every result.
//!
//! **BLOCKED — fixtures left in PHDL syntax.** This target's fixture needs
//! PHDL's `constraint` block and its `require` clause.
//! Verilog-AMS Annex A has no production for any of them: MD-45 deleted
//! `pom::ConstraintBlock`/`QuantityClass`/`Module::constraints()` with the
//! PHDL AST (T21), `lower_bodies` now always sets `LoweredBody::constraints`
//! and `::cover` to `None`, `elab/lower/vams.rs` always writes
//! `Param::distribution = None`, and `MarginsCatalog::compute` returns an
//! empty catalog unconditionally. A future phase re-anchors the dv-core
//! surface on VAMS `(* attribute *)` syntax (`.specs/STATE.md` MD-45,
//! `resolve/pom/constraints.rs`'s module doc). Until then the fixtures below
//! cannot be retargeted and the tests are left failing rather than weakened,
//! deleted, or `#[ignore]`d (repo-banned by `tests/suite_hygiene.rs`).

use piperine_api::prelude::CheckPosture;
use piperine_api::{MarginEntry, MarginShape, MarginValue, MarginsResult, Session, SolverConfig};
use piperine_lang::SourceMap;
use piperine_solver::prelude::MarginKind;

/// The divider: `dc` volts across r1 + dut1 puts `dc/2` across `dut1`
/// (dc = 1.0 → 0.5 V); `require head : V(p, n) <= hi` (hi = 0.4) has
/// margin −0.1 at the operating point.
const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Vsrc(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
analog Vsrc { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod Dut(inout p: Electrical, inout n: Electrical) { param hi: Real = 0.4; }
analog Dut { I(p, n) <+ V(p, n) / 1e3; }
constraint Dut { require head : V(p, n) <= hi; }

mod Top() {
    wire gnd : Electrical;
    wire in  : Electrical;
    wire out : Electrical;
    v1   : Vsrc(.p = in,  .n = gnd);
    r1   : R(.p = in,     .n = out);
    dut1 : Dut(.p = out,  .n = gnd);
}
";

fn compile(phdl: &str) -> Session {
    let design = piperine_lang::parse_and_elaborate(phdl, &SourceMap::dummy()).expect("elaborates");
    Session::compile(&design, "Top").expect("session compiles")
}

fn collect_config() -> SolverConfig {
    SolverConfig { checks: CheckPosture::Collect, ..Default::default() }
}

/// DVC-18: the operating point's margin channel carries the pointwise
/// shape — worst value, argmin (the DC coordinate), and the instance
/// path — with `requires_ok` / `first_violation` as the assertion shape.
#[test]
fn op_result_margins_carry_worst_argmin_and_instance_path() {
    let mut session = compile(DIVIDER_PHDL);
    let op = session.op(&collect_config(), None).expect("collect completes");
    let margins = op.margins();
    let head = margins.get("head").expect("the require is in the channel");
    assert_eq!(
        head.value,
        MarginValue::Pointwise { worst: 0.4 - 0.5, argmin: Some(0.0) },
        "worst + argmin (the DC coordinate)"
    );
    assert_eq!(head.instance_path, "dut1", "the authored instance path");
    assert_eq!(head.shape(), MarginShape::Pointwise, "the channel states the kind");

    assert!(!op.requires_ok(), "the violated require gates requires_ok");
    let violation = op.first_violation().expect("a violation is present");
    assert_eq!(violation.label, "head");
    assert_eq!(violation.instance_path, "dut1");
}

/// DVC-18: the transient trace reads the same channel with time
/// coordinates — a satisfied require leaves `requires_ok` true and
/// `first_violation` empty.
#[test]
fn tran_trace_margins_read_with_time_argmins() {
    let mut session = compile(DIVIDER_PHDL);
    // dc = 0.6 → 0.3 V across dut1, margin +0.1 — satisfied everywhere.
    session.set("v1", "dc", 0.6).expect("restamp");
    let trace = session
        .tran(1e-6, Some(1e-7), 0.0, &collect_config(), None, false, &[])
        .expect("collect completes");
    let margins = trace.margins();
    let head = margins.get("head").expect("the require is in the channel");
    assert!(
        (head.scalar() - 0.1).abs() < 1e-9,
        "worst margin = 0.4 − 0.3 = +0.1, got {}",
        head.scalar()
    );
    assert!(head.argmin().is_some(), "a pointwise margin carries its time argmin");
    assert!(trace.requires_ok(), "satisfied: requires_ok");
    assert!(trace.first_violation().is_none(), "satisfied: no violation");
}

/// D3: margins never appear as waveform rows — the constraint label is
/// not a net, and asking the trace for it as a waveform fails loud.
#[test]
fn margins_are_not_waveform_rows() {
    let mut session = compile(DIVIDER_PHDL);
    session.set("v1", "dc", 0.6).expect("restamp");
    let trace = session
        .tran(1e-6, Some(1e-7), 0.0, &collect_config(), None, false, &[])
        .expect("collect completes");
    let err = trace.v("head").expect_err("a constraint label is not a net");
    assert!(
        err.to_string().contains("not addressable"),
        "margins live only on the margins channel: {err}"
    );
    assert!(
        !trace.margins().is_empty(),
        "the margin is present on its own channel"
    );
}

/// DVC-18: the AC trace's channel carries frequency argmins.
#[test]
fn ac_trace_margins_read_with_frequency_argmins() {
    const FILTER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Vac(inout p: Electrical, inout n: Electrical) { param acmag: Real = 1.0; }
analog Vac { V(p, n) <+ ac_stim(acmag, 0.0); }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-6; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }
constraint C { require amp : V(p, n) <= 0.5; }

mod Top() {
    wire gnd : Electrical;
    wire in  : Electrical;
    wire out : Electrical;
    v1 : Vac(.p = in, .n = gnd);
    r1 : R(.p = in, .n = out);
    c1 : C(.p = out, .n = gnd);
}
";
    let mut session = compile(FILTER_PHDL);
    let trace = session
        .ac(1.0, 1e6, 6, true, &collect_config())
        .expect("collect completes");
    let margins = trace.margins();
    let amp = margins.get("amp").expect("the require is in the channel");
    assert!(amp.scalar() < 0.0, "violated at low frequencies (|V| ≈ 1 > 0.5)");
    assert_eq!(amp.argmin(), Some(1.0), "the worst is at the lowest frequency");
    assert!(!trace.requires_ok(), "the violation gates requires_ok");
    assert_eq!(trace.first_violation().map(|e| e.label), Some("amp".to_string()));
}

/// D3/§C2: a **reduced** margin carries one value and **no** argmin —
/// the channel states the kind rather than inventing a `t = 0`.
#[test]
fn a_reduced_margin_has_one_value_and_no_argmin() {
    let channel = MarginsResult::from_entries(vec![
        MarginEntry {
            label: "head".to_string(),
            instance_path: "dut1".to_string(),
            kind: MarginKind::Require,
            value: MarginValue::Pointwise { worst: 0.1, argmin: Some(2e-9) },
        },
        MarginEntry {
            label: "ugbw".to_string(),
            instance_path: "x1".to_string(),
            kind: MarginKind::Require,
            value: MarginValue::Reduced { value: 12.5e6 },
        },
    ]);
    let pointwise = channel.get("head").expect("pointwise entry");
    assert_eq!(pointwise.shape(), MarginShape::Pointwise);
    assert_eq!(pointwise.argmin(), Some(2e-9), "pointwise carries its argmin");
    assert_eq!(pointwise.instance_path, "dut1", "the instance path travels with the entry");

    let reduced = channel.get("ugbw").expect("reduced entry");
    assert_eq!(reduced.shape(), MarginShape::Reduced, "the channel states which kind it is");
    assert_eq!(reduced.scalar(), 12.5e6, "one value");
    assert_eq!(reduced.argmin(), None, "a reduced margin has NO argmin — none is invented");

    assert!(channel.requires_ok(), "both margins satisfied");
    assert!(channel.first_violation().is_none());
}
