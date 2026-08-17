//! Findings and loud reads on the host surface (dv-core T23, DVC-15/18):
//! `r.violations` carries every finding with its severity and provenance
//! (label, message, value, evaluation coordinate, instance path), and the
//! margins channel fails loud with the *real* reason a declared name has
//! no entry — an unknown name lists the candidates, a reduced `var` read
//! outside AC is "not defined in this analysis", and a `require` on a
//! digital-only instance has no margin because margins are an analog
//! notion.
//!
//! **Migrated** (constraints-on-vams): the `require`-only divider fixture
//! runs against the `constraint begin … end` producer. The two remaining
//! red fixtures each name their blocker where they are declared:
//! `STAGE_PHDL` needs the deferred constraint `var`/reduced-metric
//! statement (`var ugbw : Real = ac_unity_gain_freq(…)`;
//! constraints-on-vams Out-of-Scope), and `DIGITAL_PHDL` needs PHDL
//! `Bit`-typed module vars (`var st : Bit = 0`) — module var initializers
//! have no VAMS producer and `Bit` is PHDL's value layer. Those stay in
//! PHDL and fail rather than being weakened, deleted, or `#[ignore]`d
//! (repo-banned by `tests/suite_hygiene.rs`).

use piperine_api::prelude::CheckPosture;
use piperine_api::{Session, SolverConfig};
use piperine_lang::SourceMap;
use piperine_solver::prelude::ValidationSeverity;

/// The `SourceMap` reaching `piperine-lang`'s bundled headers.
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// The divider (margins_channel.rs's fixture): `dc` volts across r1 +
/// dut1 puts `dc/2` across `dut1`; `require head : V(p, n) <= hi`
/// (hi = 0.4) has margin `0.4 − dc/2` — violated at dc = 1.0 (−0.1).
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

/// BLOCKED on the deferred constraint `var`/reduced-metric statement
/// (constraints-on-vams Out-of-Scope): `var ugbw : Real =
/// ac_unity_gain_freq(…)` has no `constraint begin … end` spelling yet,
/// so this fixture stays in PHDL and fails at parse.
///
/// A gain stage with a reduced `var` (reduced_metrics.rs's fixture
/// family): `ugbw` is evaluated host-side once per completed AC sweep —
/// no other analysis defines it.
const STAGE_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Vac(inout p: Electrical, inout n: Electrical) { param acmag: Real = 1.0; }
analog Vac { V(p, n) <+ ac_stim(acmag, 0.0); }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-6; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Stage(inout inp: Electrical, inout out: Electrical, inout gnd: Electrical) {
    param gm: Real = 0.1;
    param r: Real = 1e3;
    param c: Real = 1e-6;
}
analog Stage {
    I(gnd, out) <+ gm * V(inp, gnd);
    I(out, gnd) <+ V(out, gnd) / r;
    I(out, gnd) <+ c * ddt(V(out, gnd));
}
constraint Stage {
    var ugbw : Real = ac_unity_gain_freq(V(out, gnd));
}

mod Top() {
    wire gnd : Electrical;
    wire in  : Electrical;
    wire out : Electrical;
    v1 : Vac(.p = in, .n = gnd);
    x1 : Stage(.inp = in, .out = out, .gnd = gnd);
}
";

/// BLOCKED on PHDL Bit-typed module vars — no VAMS producer: `var st :
/// Bit = 0;` is a module var initializer (a known gap) over PHDL's `Bit`
/// value layer. This fixture stays in PHDL and fails at parse.
///
/// A digital-only inverter carrying a `require` over a digital quantity
/// (the stdlib prelude supplies `Bit`/`posedge`): the module has no
/// analog half, so the require produces no margin.
const DIGITAL_PHDL: &str = "\
use piperine::disciplines;
mod Inv(input a: Bit, output y: Bit) { var st : Bit = 0; }
digital Inv { y <- st; @ posedge(a) { st = a; } }
constraint Inv { require ok : a <= 1; }

mod Top() {
    wire a : Bit;
    wire y : Bit;
    u1 : Inv(.a = a, .y = y);
}
";

fn compile(src: &str) -> Session {
    let design = piperine_lang::parse_and_elaborate(src, &source_map()).expect("elaborates");
    Session::compile(&design, "Top").expect("session compiles")
}

fn collect_config() -> SolverConfig {
    SolverConfig { checks: CheckPosture::Collect, ..Default::default() }
}

/// DVC-15: `r.violations` on the operating point carries the finding with
/// its severity and full provenance — label, instance path, the offending
/// margin value, and the evaluation coordinate.
#[test]
fn op_violations_carry_severity_and_provenance() {
    let mut session = compile(DIVIDER);
    let op = session.op(&collect_config(), None).expect("collect completes");
    let violations = op.violations();
    assert_eq!(violations.len(), 1, "the violated require emits exactly one finding at the DC point");
    let finding = &violations[0];
    assert_eq!(finding.severity, ValidationSeverity::Error, "a require crossing zero is an Error");
    assert_eq!(finding.label, "head", "the constraint's authored label");
    assert_eq!(finding.instance_path, "dut1", "the authored hierarchical instance path");
    assert!((finding.value - (0.4 - 0.5)).abs() < 1e-12, "the offending margin value, got {}", finding.value);
    assert_eq!(finding.time, 0.0, "the DC evaluation coordinate");
    assert!(finding.message.contains("head") || finding.message.contains("margin"), "a human-readable message: {}", finding.message);
}

/// DVC-15: the transient trace's findings channel carries the same
/// finding with the accepted-step time as its coordinate.
#[test]
fn tran_violations_carry_time_provenance() {
    let mut session = compile(DIVIDER);
    let trace = session
        .tran(1e-6, Some(1e-7), 0.0, &collect_config(), None, false, &[])
        .expect("collect completes");
    let violations = trace.violations();
    assert!(!violations.is_empty(), "a violated require records findings across the run");
    for finding in violations {
        assert_eq!(finding.severity, ValidationSeverity::Error);
        assert_eq!(finding.label, "head");
        assert_eq!(finding.instance_path, "dut1");
        assert!((0.0..=1e-6).contains(&finding.time), "the coordinate is an accepted step's time: {}", finding.time);
        assert!(finding.value < 0.0, "the recorded value is the violating margin: {}", finding.value);
    }
}

/// DVC-18 AC3: an unknown constraint name fails loud listing the
/// candidates — the `Error::Measurement` family's contract.
#[test]
fn unknown_constraint_name_lists_candidates() {
    let mut session = compile(DIVIDER);
    let op = session.op(&collect_config(), None).expect("collect completes");
    let err = op.margins().get("hed").expect_err("an unknown name fails loud");
    let msg = err.to_string();
    assert!(msg.contains("unknown constraint `hed`"), "names the unknown: {msg}");
    assert!(msg.contains("dut1.head"), "lists the candidates: {msg}");
}

/// DVC-08 AC6 / spec P1 "Margins" AC6: a reduced `var` read from an
/// analysis where it is not defined fails loud with "not defined in this
/// analysis" — never an "unknown constraint" for a name the design
/// declares, and never an invented `t = 0` value.
///
/// BLOCKED on the deferred constraint `var`/reduced-metric statement
/// (constraints-on-vams Out-of-Scope) — see `STAGE_PHDL`.
#[test]
fn a_reduced_var_read_outside_ac_is_not_defined_in_this_analysis() {
    let mut session = compile(STAGE_PHDL);
    let op = session.op(&collect_config(), None).expect("op completes");
    for name in ["ugbw", "x1.ugbw"] {
        let err = op.margins().get(name).expect_err("a reduced var has no entry on an op result");
        assert!(
            err.to_string().contains("not defined in this analysis"),
            "the cross-analysis read names the real reason: {err}"
        );
    }
    // The same name IS defined on the AC result — the catalog never
    // shadows a real entry.
    let trace = session.ac(1.0, 1e6, 11, true, &collect_config()).expect("ac completes");
    assert!(trace.margins().get("ugbw").is_ok(), "defined on AC");
}

/// Spec P1 "validation_reports()" story / design §C4: a `require` over a
/// digital quantity has no margin — asking for one fails loud with
/// "margins are an analog notion", not "unknown constraint".
///
/// BLOCKED on PHDL Bit-typed module vars — no VAMS producer — see
/// `DIGITAL_PHDL`.
#[test]
fn a_digital_require_has_no_margin_and_asking_for_one_is_loud() {
    let mut session = compile(DIGITAL_PHDL);
    let trace = session
        .tran(1e-9, Some(1e-10), 0.0, &collect_config(), None, false, &[])
        .expect("collect completes");
    assert!(trace.margins().is_empty(), "a digital-only instance contributes no margins");
    let err = trace.margins().get("ok").expect_err("asking for the digital margin fails loud");
    assert!(
        err.to_string().contains("margins are an analog notion"),
        "the read names the real reason: {err}"
    );
}
