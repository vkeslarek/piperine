//! The margins channel (dv-core T21, DVC-18, D3): margins travel as their
//! own result channel — `OpResult::margins`, `Trace::margins`,
//! `AcTrace::margins` — never folded into waveform rows. A **pointwise**
//! margin carries worst + argmin (time/frequency/swept coordinate) +
//! instance path; a **reduced** margin carries one value and **no**
//! argmin, and the channel states which kind it is. `requires_ok` and
//! `first_violation` are the common assertion shape on every result.
//!
//! **Migrated** (constraints-on-vams): the fixture runs against the
//! `constraint begin … end` producer.

use piperine_api::prelude::CheckPosture;
use piperine_api::{MarginEntry, MarginShape, MarginValue, MarginsResult, Session, SolverConfig};
use piperine_lang::SourceMap;
use piperine_solver::prelude::MarginKind;

/// The divider: `dc` volts across r1 + dut1 puts `dc/2` across `dut1`
/// (dc = 1.0 → 0.5 V); `require head : V(p, n) <= hi` (hi = 0.4) has
/// margin −0.1 at the operating point.
/// The `SourceMap` reaching `piperine-lang`'s bundled headers.
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

const DIVIDER: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    parameter real hi = 0.4;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require head : V(p, n) <= hi;
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;

fn compile(phdl: &str) -> Session {
    let design = piperine_lang::parse_and_elaborate(phdl, &source_map()).expect("elaborates");
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
    let mut session = compile(DIVIDER);
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
    let mut session = compile(DIVIDER);
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
    let mut session = compile(DIVIDER);
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
    const FILTER: &str = r#"`include "disciplines.vams"
module Vac (inout electrical p, inout electrical n);
    parameter real acmag = 1.0;
    analog begin
        V(p, n) <+ ac_stim(acmag, 0.0);
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module C (inout electrical p, inout electrical n);
    parameter real c = 1e-6;
    analog begin
        I(p, n) <+ c * ddt(V(p, n));
    end
    constraint begin
        require amp : V(p, n) <= 0.5;
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vac v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    C c1 (.p(out), .n(gnd));
endmodule
"#;
    let mut session = compile(FILTER);
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

/// CONVAMS-13 (constraints-on-vams): the catalog classifies a `require`
/// on a digital-only instance as `digital` — margins are an analog
/// notion — so reading it fails loud naming the constraint and saying
/// why, never "unknown constraint".
#[test]
fn a_require_on_a_digital_instance_reads_loud_as_digital() {
    const DIGITAL: &str = r#"`include "disciplines.vams"
discipline logic;
    domain discrete;
enddiscipline
module Dut (input logic a, output logic y);
    always begin
        y <= a;
    end
    constraint begin
        require stable : y <= 1;
    end
endmodule
module Top (input logic a, output logic y);
    Dut d1 (.a(a), .y(y));
endmodule
"#;
    let design = piperine_lang::parse_and_elaborate(DIGITAL, &source_map()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let op = session.op(&collect_config(), None).expect("collect completes");
    let err = op.margins().get("stable").expect_err("no analog margin exists");
    let msg = err.to_string();
    assert!(msg.contains("stable"), "names the constraint: {msg}");
    assert!(
        msg.contains("no analog half"),
        "says why — margins are an analog notion and this instance is digital: {msg}"
    );
}

/// CONSTMT-03 (`constraint-statements`): a violated `target` is a soft
/// goal — the channel tracks its worst margin (readable, negative) but no
/// violation is recorded and `requires_ok` ignores targets entirely.
#[test]
fn a_violated_target_is_tracked_but_never_a_violation() {
    const SOFT: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    parameter real hi = 0.4;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        require head : V(p, n) <= hi;
        target V(p, n) >= 1.5;
    end
endmodule
module Top ();
    electrical gnd, out;
    Vsrc v1 (.p(out), .n(gnd));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;
    // V(dut) = 1.0 V: head (V ≤ 0.4) violated at −0.6; the target
    // (V ≥ 1.5) also violated at 1.0 − 1.5 = −0.5 — both negative, only
    // the require reports.
    let design = piperine_lang::parse_and_elaborate(SOFT, &source_map()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("compiles");
    let op = session.op(&collect_config(), None).expect("collect completes");
    let margins = op.margins();
    let target = margins.get("target#1").expect("the target is in the channel");
    assert!(
        target.scalar() < 0.0,
        "an unsatisfiable target carries its (negative) worst margin, got {}",
        target.scalar()
    );
    assert!(target.is_satisfied() == (target.scalar() >= 0.0), "satisfaction follows the sign");
    // The violated require reports; the violated target does not, and
    // requires_ok reflects requires only (here false because `head` is
    // violated — the target adds nothing).
    assert!(!op.requires_ok(), "gated by the violated require");
    assert_eq!(op.first_violation().expect("a violation").label, "head", "the require, never the target");
    let findings: Vec<&str> = op.violations().iter().map(|f| f.label.as_str()).collect();
    assert!(!findings.contains(&"target#1"), "targets never produce findings: {findings:?}");
}

/// CONSTMT-06 value-level: set-bins membership counts each accepted point
/// into its matching value's bin — and nothing for a non-member value.
#[test]
fn set_bins_count_by_membership_value_level() {
    use piperine_codegen::CircuitCompiler;
    let src = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        cover c1 : V(p, n) bins in {0.25, 0.5, 1.0};
    end
endmodule
module Top ();
    electrical gnd, out;
    Vsrc v1 (.p(out), .n(gnd));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = piperine_lang::SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    let design = piperine_lang::parse_and_elaborate(src, &map).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let mut circuit = compiler.build_circuit("Top").expect("builds");
    circuit
        .dc(piperine_solver::prelude::Context::default())
        .expect("dc setup")
        .solve()
        .expect("dc solves");
    // V(dut) = 1.0 V — exactly one of the set's members.
    let (points, _) = circuit.coverage_reports();
    let c1 = points.iter().find(|p| p.label.ends_with("c1")).expect("cover point c1");
    assert_eq!(c1.counts, vec![0, 0, 1], "only the 1.0 bin counts the operating point");
}
