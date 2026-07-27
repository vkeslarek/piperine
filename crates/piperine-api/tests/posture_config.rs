//! The check posture on the host config (DVC-12, MD-22): `SolverConfig`
//! carries `checks` and `require_scope` with the `Context` defaults
//! (`Strict`, "every analysis"), `to_context()` maps them through, and
//! changing the posture between analyses in one session triggers **no**
//! recompilation (MD-18) — posture is pure run data.

use piperine_api::prelude::{CheckPosture, RequireScope};
use piperine_api::{Session, SolverConfig};
use piperine_lang::SourceMap;

/// Self-contained divider (own discipline + devices, no prelude):
/// `mid = 10·1k/(1k+1k) = 5.0 V`.
const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VoltageSource(inout p: Electrical, inout n: Electrical) {
    param voltage: Real = 0.0;
}
analog VoltageSource { V(p, n) <- voltage; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}
analog Resistor { I(p, n) <+ V(p, n) / r; }

mod Divider() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire mid  : Electrical;
    src   : VoltageSource (.p = vin, .n = gnd) { .voltage = 10.0 };
    r_top : Resistor      (.p = vin, .n = mid) { .r = 1e3 };
    r_bot : Resistor      (.p = mid, .n = gnd) { .r = 1e3 };
}
";

fn divider_session() -> Session {
    let headers =
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    let design =
        piperine_lang::parse_and_elaborate(DIVIDER_PHDL, &map).expect("divider elaborates");
    Session::compile(&design, "Divider").expect("session compiles")
}

/// DVC-12 / D2: `SolverConfig::default()` mirrors the `Context` defaults —
/// `Strict` and "every analysis" — and `to_context()` carries both fields
/// through unchanged for all three postures.
#[test]
fn solver_config_mirrors_the_context_posture_fields() {
    let config = SolverConfig::default();
    assert_eq!(config.checks, CheckPosture::Strict, "the default posture is strict (DVC-12)");
    assert_eq!(
        config.require_scope,
        RequireScope::EveryAnalysis,
        "an unscoped require holds in every analysis by default (D2)"
    );
    let ctx = config.to_context();
    assert_eq!(ctx.checks, CheckPosture::Strict);
    assert_eq!(ctx.require_scope, RequireScope::EveryAnalysis);

    for posture in [CheckPosture::Collect, CheckPosture::Off, CheckPosture::Strict] {
        let config = SolverConfig { checks: posture, ..Default::default() };
        assert_eq!(config.to_context().checks, posture, "to_context carries {posture:?}");
    }
}

/// DVC-12 AC7 / MD-18: changing the posture between analyses in one session
/// triggers no recompilation — the compile count does not move across a
/// strict → collect → off sequence.
#[test]
fn posture_changes_between_analyses_never_recompile() {
    let mut session = divider_session();
    let mut config = SolverConfig::default();

    let before = piperine_codegen::AnalogKernel::compile_count();
    let op = session.op(&config, None).expect("strict op solves");
    assert!((op.v("mid").expect("v(mid)") - 5.0).abs() < 1e-9);

    config.checks = CheckPosture::Collect;
    let op = session.op(&config, None).expect("collect op solves");
    assert!((op.v("mid").expect("v(mid)") - 5.0).abs() < 1e-9);

    config.checks = CheckPosture::Off;
    let op = session.op(&config, None).expect("off op solves");
    assert!((op.v("mid").expect("v(mid)") - 5.0).abs() < 1e-9);

    config.require_scope = RequireScope::EveryAnalysis;
    let _ = session.op(&config, None).expect("op solves after scope touch");

    assert_eq!(
        piperine_codegen::AnalogKernel::compile_count(),
        before,
        "a posture change between analyses recompiled kernels (MD-18 breach)"
    );
}
