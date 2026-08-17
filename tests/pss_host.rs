//! SC-06 — PSS host surface: full-wave rectifier ripple vs a settled
//! transient, tstab equivalence, and the estimated-settle-time diagnostic
//! (all through `Session::run_pss`).

use std::path::PathBuf;

use piperine::{NetRef, Session, SolverConfig};
use piperine_lang::SourceMap;

const FIXTURE: &str = r#"
    module R (inout electrical p, inout electrical n);
        parameter real r = 1.0e3;
        analog I(p, n) <+ V(p, n) / r;
    endmodule

    module C (inout electrical p, inout electrical n);
        parameter real c = 1.0e-6;
        analog I(p, n) <+ c * ddt(V(p, n));
    endmodule

    module Dio (inout electrical p, inout electrical n);
        parameter real is_sat = 1.0e-9;
        analog I(p, n) <+ is_sat * (exp(V(p, n) / 0.02585) - 1.0);
    endmodule

    module Vsine (inout electrical p, inout electrical n);
        parameter real amp = 5.0;
        parameter real freq = 1.0e3;
        analog V(p, n) <+ amp * sin(2.0 * 3.14159265358979 * freq * $abstime);
    endmodule

    // Full-wave bridge: floating sine into d1..d4, RC load out→gnd.
    module Rectifier ();
        electrical gnd, acp, acn, out;
        Vsine vs (.p(acp), .n(acn));
        Dio d1 (.p(acp), .n(out));
        Dio d2 (.p(acn), .n(out));
        Dio d3 (.p(gnd), .n(acp));
        Dio d4 (.p(gnd), .n(acn));
        R rl (.p(out), .n(gnd));
        R #(.r(1.0e6)) b1 (.p(acp), .n(gnd));
        R #(.r(1.0e6)) b2 (.p(acn), .n(gnd));
        C #(.c(2.0e-6)) cl (.p(out), .n(gnd));
    endmodule

    module SineRc ();
        electrical gnd, top, out;
        Vsine v1 (.p(top), .n(gnd));
        R r1 (.p(top), .n(out));
        C c1 (.p(out), .n(gnd));
    endmodule
"#;

fn headers_source_map() -> SourceMap {
    let headers =
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.vams"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn session(top: &str) -> Session {
    let design = piperine_lang::parse_and_elaborate(FIXTURE, &headers_source_map())
        .expect("fixture elaborates");
    Session::compile(&design, top).expect("session compiles")
}

/// Full-wave rectifier: the PSS orbit's mean and ripple match the last
/// period of a long settled transient within the 10·reltol class.
#[test]
fn rectifier_ripple_matches_settled_transient() {
    let period = 1.0e-3;
    let config = SolverConfig::default();
    let out = NetRef { name: "out".into() };

    let mut s = session("Rectifier");
    let pss = s.pss(period, 0.0, &config).expect("pss orbit");
    let w_pss = pss.trace.v(&out).expect("v(out) over the orbit");

    // Reference: 14 periods of plain transient (tau = R*C = 2 ms = 2T, so
    // 12 periods is a 0.25% settle), statistics on the last one.
    let long = s
        .tran(14.0 * period, Some(period / 100.0), 13.0 * period, &config, None, false, &[])
        .expect("settled tran");
    let w_ref = long.v(&out).expect("v(out) settled");

    let tol = 10.0 * config.reltol * w_ref.mean().abs().max(1.0);
    assert!(
        (w_pss.mean() - w_ref.mean()).abs() < tol,
        "mean: pss {} vs settled {} (tol {tol})",
        w_pss.mean(),
        w_ref.mean()
    );
    assert!(
        (w_pss.peak_to_peak() - w_ref.peak_to_peak()).abs() < tol,
        "ripple: pss {} vs settled {} (tol {tol})",
        w_pss.peak_to_peak(),
        w_ref.peak_to_peak()
    );
    // Physically a rectifier: mean well above zero, ripple small vs mean.
    assert!(w_pss.mean() > 2.0, "rectified mean: {}", w_pss.mean());
}

/// `tstab > 0` converges to the same orbit as `tstab = 0` (within the
/// integration tolerance class), and the settle-time diagnostic on the
/// sine-RC matches the analytic `T·ln(reltol)/ln(e^{-T/τ})` ≈ 6.9 ms.
#[test]
fn tstab_equivalence_and_settle_estimate() {
    let period = 1.0e-3;
    let config = SolverConfig::default();
    let out = NetRef { name: "out".into() };

    let mut s = session("SineRc");
    let a = s.pss(period, 0.0, &config).expect("tstab=0");
    let b = s.pss(period, 2.0 * period, &config).expect("tstab=2T");
    let amp_a =
        a.trace.v(&out).expect("v out").max().abs();
    let amp_b =
        b.trace.v(&out).expect("v out").max().abs();
    assert!(
        (amp_a - amp_b).abs() < 1.0e-9 + 1.0e-3 * amp_a.abs(),
        "tstab equivalence: {amp_a} vs {amp_b}"
    );

    // τ = RC = 1 ms = T → ρ = e^{-1}; settle = T·ln(reltol)/1 ≈ 6.91 ms.
    let settle = a
        .stats
        .estimated_settle_time
        .expect("shooting needed a Jacobian on the cold start");
    let analytic = period * (config.reltol.ln() / (-1.0_f64)).abs();
    assert!(
        ((settle - analytic) / analytic).abs() < 0.2,
        "settle estimate {settle} vs analytic {analytic}"
    );
}
