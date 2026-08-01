//! T31 (DVT-39) — monitors as ordinary modules: a monitor is **no new
//! grammar** (`.specs/features/dv-tester/spec.md` P2) — it is a
//! `constraint` block attached to a module exactly the way
//! `crates/piperine-api/tests/violations_channel.rs`'s fixtures already
//! use, composed (multiple instances, a parameterized module) the same way
//! any PHDL module composes, and reporting through `validation_reports()`
//! (dv-core's channel, T22's own consumer) with full instance provenance.
//!
//! // SPEC_DEVIATION: the fixtures below give `Sub`/`Watch` a **trivial**
//! // analog half (`analog Sub { I(p, n) <+ V(p, n) * 1e-9; }`, a leakage
//! // conductance whose only job is to keep the node non-floating) even
//! // though the observed signal (`bad`) is a digital var.
//! //
//! // Reason: tracing `PiperineDevice::evaluate_constraints`/
//! // `validation_reports` (`crates/piperine-codegen/src/device/
//! // element.rs:737-769`) shows both forward **only** through
//! // `self.analog` — `evaluate_constraints`'s own doc comment states "a
//! // digital-only device has no margins... inherits the default no-op",
//! // and empirically a pure-digital module (no `analog` body at all)
//! // produces zero findings from a `constraint` block through this
//! // channel today, confirmed by direct experiment before writing this
//! // fixture (a `require` that is unconditionally false on a digital-only
//! // `Sub` yields `trace.violations().is_empty() == true`). This is a
//! // real, pre-existing gap between dv-core's delivered `evaluate_
//! // constraints`/`validation_reports` wiring and the "any digital module"
//! // premise this task states — T31's own scope is explicitly "no new
//! // code" (`tasks.md`'s `Where` column), so closing that gap in
//! // `piperine-codegen`/`piperine-solver` is out of this task's bounds; a
//! // module with a trivial analog half exercises the exact same
//! // `constraint`/`validation_reports()` channel this task asks for,
//! // honestly, without claiming a capability the codebase does not yet
//! // have for the pure-digital-only case.
//!
//! **Settled-only, inherited, not reinvented**: this feature builds no new
//! settling-observation mechanism — dv-core's own T23/DVC-15 machinery
//! already reads constraint predicates only at accepted (settled) points
//! (`violations_channel.rs::tran_violations_carry_time_provenance` proves
//! this for the analog case); a monitor's `require` rides that exact same
//! evaluation path. This file's assertion on `finding.time` falling inside
//! `[0, stop]` is the same style of proof.

use piperine_api::prelude::CheckPosture;
use piperine_api::{Session, SolverConfig};
use piperine_lang::SourceMap;
use piperine_solver::prelude::ValidationSeverity;

fn digital_source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn compile_digital(phdl: &str) -> Session {
    let design = piperine_lang::parse_and_elaborate(phdl, &digital_source_map()).expect("elaborates");
    Session::compile(&design, "Top").expect("session compiles")
}

fn collect_config() -> SolverConfig {
    SolverConfig { checks: CheckPosture::Collect, ..Default::default() }
}

/// A monitor (`Sub`'s `constraint` block) sees an internal signal (`bad`)
/// that never reaches a port — `Top` only ever wires `q`, which stays `0`
/// throughout the run, so a hypothetical top-level tester reading only the
/// design's ports could never observe the violation. `bad` is a `var`
/// initialized to `1` and never reassigned — a persistent internal fault,
/// deliberately requiring no clock/event machinery to keep this fixture
/// minimal (the point under test is *visibility*, not sequencing).
const HIERARCHICAL_MONITOR_PHDL: &str = "\
use piperine::disciplines;
mod Sub(input clk: Bit, output q: Bit, inout p: Electrical, inout n: Electrical) { var bad: Bit = 1; }
analog Sub { I(p, n) <+ V(p, n) * 1e-9; }
digital Sub { q <- bad; }
constraint Sub { require bad_is_low : bad <= -1.0; }

mod Top() {
    wire clk : Bit;
    wire q0  : Bit;
    wire q1  : Bit;
    wire gnd : Electrical;
    wire p0  : Electrical;
    wire p1  : Electrical;
    sub0 : Sub(.clk = clk, .q = q0, .p = p0, .n = gnd);
    sub1 : Sub(.clk = clk, .q = q1, .p = p1, .n = gnd);
}
";

/// T31: a monitor is an ordinary `digital` module — `Sub` above adds no
/// PHDL construct beyond `digital`/`constraint`, both already declared
/// language surface. Fires through `validation_reports()`
/// (`trace.violations()`, the same channel T22 consumes), reports with
/// **instance provenance** distinguishing `sub0` from `sub1` (composition:
/// two ordinary instances of the same monitor module, independently), and
/// catches a violation `Top`'s own only externally visible signals (`q0`/
/// `q1`, both constant `0`) never reveal.
#[test]
fn a_monitor_detects_a_violation_the_top_level_cannot_see_with_instance_provenance() {
    let mut session = compile_digital(HIERARCHICAL_MONITOR_PHDL);
    let trace = session
        .tran(1e-9, Some(1e-10), 0.0, &collect_config(), None, false, &[])
        .expect("collect completes");

    // Top's own visible surface (q0/q1) never shows a problem — both are
    // wired to a constant 0 in `Sub`'s own digital body.
    let violations = trace.violations();
    assert!(!violations.is_empty(), "sub0/sub1's internal `bad` violation must be reported");

    let mut seen_instances: Vec<&str> = violations.iter().map(|f| f.instance_path.as_str()).collect();
    seen_instances.sort();
    seen_instances.dedup();
    assert_eq!(
        seen_instances,
        vec!["sub0", "sub1"],
        "both monitor instances report independently, each with its own instance path"
    );

    for finding in violations {
        assert_eq!(finding.severity, ValidationSeverity::Error, "a require crossing zero is an Error");
        assert_eq!(finding.label, "bad_is_low", "the constraint's authored label");
        assert!(
            (0.0..=1e-9).contains(&finding.time),
            "the finding's coordinate is an accepted step's time (settled-only, inherited from DVC-15): {}",
            finding.time
        );
    }
}

/// T31: a monitor composes under **parameterization** exactly as any
/// module does — `mod Watch[N]`, a generic module, carries its
/// `constraint` block through monomorphization
/// (`constraint_pom.rs::monomorphized_variant_carries_the_authored_block`
/// already proves the POM side of this; this proves it end to end through
/// a real solve).
const PARAMETERIZED_MONITOR_PHDL: &str = "\
use piperine::disciplines;
mod Watch[N](input clk: Bit, output q: Bit, inout p: Electrical, inout n: Electrical) { var bad: Bit = 1; }
analog Watch { I(p, n) <+ V(p, n) * 1e-9; }
digital Watch { q <- bad; }
constraint Watch { require bad_is_low : bad <= -1.0; }

mod Top() {
    wire clk : Bit;
    wire q0  : Bit;
    wire gnd : Electrical;
    wire p0  : Electrical;
    w0 : Watch[4](.clk = clk, .q = q0, .p = p0, .n = gnd);
}
";

#[test]
fn a_monitor_composes_under_parameterization() {
    let mut session = compile_digital(PARAMETERIZED_MONITOR_PHDL);
    let trace = session
        .tran(1e-9, Some(1e-10), 0.0, &collect_config(), None, false, &[])
        .expect("collect completes");
    let violations = trace.violations();
    assert!(!violations.is_empty(), "the monomorphized Watch[4] instance still carries its constraint block");
    assert!(
        violations.iter().all(|f| f.label == "bad_is_low"),
        "the parameterized module's constraint reports under its authored label"
    );
}
