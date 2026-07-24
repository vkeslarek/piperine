//! HOST-05 — `Session::dc(label, param, values)` returns a swept
//! `Trace<Waveform>` (not a bare `Vec<OpResult>`), restamping on the one
//! compilation (MD-18). `cargo test -p piperine` (Phase 1 / T6 quick gate).

use piperine::{NetRef, Session, SimSession, SolverConfig};
use piperine_codegen::AnalogKernel;
use piperine_lang::SourceMap;

const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 5.0; }
analog V { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire mid : Electrical;
    v1    : V(.p = vin, .n = gnd) {};
    r_top : R(.p = vin, .n = mid) { .r = 3e3 };
    r_bot : R(.p = mid, .n = gnd) { .r = 2e3 };
}
";

fn design() -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(DIVIDER_PHDL, &SourceMap::dummy()).expect("divider elaborates")
}

/// `session.dc("v1", "dc", values)` sweeps the source's DC value on ONE
/// compilation, returning a `Trace<Waveform>` whose `.axis()` is the swept
/// values and whose `.v("mid")` matches independent fresh builds at every
/// point — the compile-once restamp path (MD-18), not per-point
/// elaboration.
#[test]
fn dc_sweep_returns_a_trace_over_the_swept_axis_matching_fresh_builds() {
    let mid = NetRef { name: "mid".into() };
    let values = [1.0_f64, 3.0, 5.0, 7.5, 12.0];

    let mut session = Session::compile(&design(), "Top").expect("session compiles");
    let before = AnalogKernel::compile_count();
    let trace = session.dc("v1", "dc", &values, &SolverConfig::default(), None).expect("dc sweep solves");
    let sweep_compiles = AnalogKernel::compile_count() - before;
    assert_eq!(sweep_compiles, 0, "the dc sweep must never re-JIT (MD-18), got {sweep_compiles}");

    let axis = trace.axis();
    assert_eq!(axis.len(), values.len());
    for (i, &v) in values.iter().enumerate() {
        assert!((axis.at(v) - v).abs() < 1e-9 || axis.points()[i].0 == v, "axis must carry the swept value");
    }

    let w = trace.v(&mid).expect("v(mid) over the sweep");
    assert_eq!(w.len(), values.len());
    for (i, &v_dc) in values.iter().enumerate() {
        let live = w.points()[i].1;
        let fresh_session = SimSession::new(design(), "Top".to_string());
        fresh_session.stage("v1", "dc", piperine_lang::Value::Real(v_dc));
        let fresh = fresh_session
            .run_op(&SolverConfig::default(), None)
            .expect("fresh op")
            .v(&mid)
            .expect("v(mid)");
        assert!((live - fresh).abs() < 1e-9, "v1.dc = {v_dc}: swept v(mid)={live} vs fresh build {fresh}");
    }
}
