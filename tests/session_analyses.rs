//! HOST-02 — every analysis is a method on `Session` with a typed return:
//! `tran`/`ac`/`noise`/`sens`/`pss`/`pz`/`disto`/`sp`, all on the one
//! compilation `Session::compile` produced. `cargo test -p piperine`
//! (Phase 1 / T4 quick gate).

use piperine::{DistoResult, NetRef, PzResult, Session, SolverConfig};
use piperine_lang::SourceMap;

const RC_AC_PHDL: &str = "\
module V(inout electrical p, inout electrical n);
    parameter real dc = 5.0;
    parameter real acmag = 1.0;
    analog V(p, n) <+ dc + ac_stim(acmag, 0.0);
endmodule

module R(inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog I(p, n) <+ V(p, n) / r;
endmodule

module C(inout electrical p, inout electrical n);
    parameter real c = 1e-6;
    analog I(p, n) <+ c * ddt(V(p, n));
endmodule

module Top();
    electrical gnd, vin, out;
    V v1 (.p(vin), .n(gnd));
    R #(.r(1e3)) r1 (.p(vin), .n(out));
    C #(.c(1e-6)) c1 (.p(out), .n(gnd));
endmodule
";

const RLC_PHDL: &str = "\
module V(inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog V(p, n) <+ dc;
endmodule

module R(inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog I(p, n) <+ V(p, n) / r;
endmodule

module L(inout electrical p, inout electrical n);
    parameter real l = 1e-3;
    analog V(p, n) <+ l * ddt(I(p, n));
endmodule

module C(inout electrical p, inout electrical n);
    parameter real c = 1e-6;
    analog I(p, n) <+ c * ddt(V(p, n));
endmodule

module Top();
    electrical gnd, vin, a, b;
    V v1 (.p(vin), .n(gnd));
    R #(.r(10.0)) r1 (.p(vin), .n(a));
    L #(.l(1e-3)) l1 (.p(a), .n(b));
    C #(.c(1e-6)) c1 (.p(b), .n(gnd));
endmodule
";


const POLY_PHDL: &str = "\
module V(inout electrical p, inout electrical n);
    parameter real dc = 0.0;
    parameter real acmag = 0.0;
    analog V(p, n) <+ dc + ac_stim(acmag, 0.0);
endmodule

module R(inout electrical p, inout electrical n);
    parameter real r = 50.0;
    analog I(p, n) <+ V(p, n) / r;
endmodule

module PolyVccs(inout electrical inp, inout electrical inn,
                inout electrical outp, inout electrical outn);
    parameter real g1 = 0.1;
    parameter real g2 = 0.02;
    parameter real g3 = 0.003;
    analog I(outp, outn) <+ g1 * V(inp, inn)
                          + g2 * V(inp, inn) * V(inp, inn)
                          + g3 * V(inp, inn) * V(inp, inn) * V(inp, inn);
endmodule

module Top();
    electrical gnd, vin, vout;
    V #(.dc(0.0), .acmag(1.0)) v1 (.p(vin), .n(gnd));
    PolyVccs n1 (.inp(vin), .inn(gnd), .outp(vout), .outn(gnd));
    R #(.r(50.0)) r1 (.p(vout), .n(gnd));
endmodule
";

fn elaborate(src: &str) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates")
}

/// `tran`/`ac`/`noise` on `Session` return the same typed containers as
/// `Session`'s (`Trace<Waveform>`/`Trace<ComplexWaveform>`/
/// `Trace<NoiseSample>`), read the same way.
#[test]
fn session_tran_ac_noise_return_typed_traces() {
    let design = elaborate(RC_AC_PHDL);
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let out = NetRef { name: "out".into() };

    let tran = session.tran(5e-3, Some(1e-4), 0.0, &SolverConfig::default(), None, false, &[]).expect("tran solves");
    let w = tran.v(&out).expect("v(out)");
    assert!(w.at(5e-3) > 4.9, "RC settles near 5 V, got {}", w.at(5e-3));

    let ac = session.ac(1.0, 1e6, 5, true, &SolverConfig::default()).expect("ac solves");
    let cw = ac.v(&out).expect("v(out) complex");
    assert!(cw.len() == 5);

    let nz = session.noise("out", "gnd", (1.0, 1e6), 5, true, &SolverConfig::default()).expect("noise solves");
    assert!(nz.total() >= 0.0);
}

/// `sens`/`pss`/`pz`/`disto` on `Session` return the same typed results
/// `Session` does (HOST-04's uniform shape), on the same fixtures. `.sp`
/// left this list with the PHDL frontend: `@rfport(num, z0)` has no
/// reachable Verilog-AMS spelling (attribute instances resolve against
/// `ElabContext::schemas`, which seeds only `device`/`port`), so no source
/// can declare the RF ports `.sp` needs — the frontend-dark call ed575c85
/// made for `piperine-lang`'s own `rfport` target. `.sp`'s fail-loud path
/// stays covered by root `sp.rs`.
#[test]
fn session_sens_pss_pz_disto_return_typed_results() {
    let rc_design = elaborate(RC_AC_PHDL);
    let mut rc_session = Session::compile(&rc_design, "Top").expect("session compiles");
    let sens = rc_session
        .sens(&["out"], &[("r1".to_string(), "r".to_string())], 1e-6, &SolverConfig::default())
        .expect("sens solves");
    assert!(sens.get("out", "r1", "r").is_some());

    let pss = rc_session.pss(1e-3, 0.0, &SolverConfig::default()).expect("pss solves");
    assert!(pss.trace.v("out").is_ok());

    let rlc_design = elaborate(RLC_PHDL);
    let mut rlc_session = Session::compile(&rlc_design, "Top").expect("session compiles");
    let pz: PzResult = rlc_session.pz("v1", "b", None, &SolverConfig::default()).expect("pz solves");
    assert_eq!(pz.poles.len(), 2);

    let poly_design = elaborate(POLY_PHDL);
    let mut poly_session =
        Session::builder(&poly_design, "Top").disto(true).compile().expect("session compiles");
    let disto: DistoResult =
        poly_session.disto(1e6, None, 0.1, "vout", None, &SolverConfig::default()).expect("disto solves");
    assert!(disto.hd2.is_some());
}

/// `Session::set` on an unknown parameter fails loud with the solver's own
/// message — no partial apply, no silent success (moved here from
/// `session_compile.rs`, which isolates the compile-count-sensitive test in
/// its own binary/process).
#[test]
fn session_set_on_unknown_param_is_a_loud_error() {
    let design = elaborate(RC_AC_PHDL);
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let err = session.set("r1", "bogus", 1.0).expect_err("unknown param must fail");
    assert!(format!("{err}").contains("bogus"));
}
