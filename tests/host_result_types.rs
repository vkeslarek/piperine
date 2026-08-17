//! HOST-02/HOST-04 — the api result types that were previously either
//! re-exported solver structs or missing entirely: `PzResult`, `DistoResult`,
//! `SParamResult` (deprecated below) are now constructed by
//! `Session::run_pz`/`run_disto`/
//! `run_sp`; `TfResult` is exercised once `Session::tf` lands (T5).
//! `cargo test -p piperine` (Phase 1 / T2 quick gate).

use piperine::{DistoResult, PzResult, Session, SolverConfig};
use piperine_lang::SourceMap;

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

/// `PzResult` is the typed api struct (HOST-04): same `poles`/`zeros` field
/// shape the solver's own `PoleZeroResult` carries, on the series-RLC's known
/// analytic complex-conjugate pole pair.
#[test]
fn pz_result_is_the_typed_api_struct() {
    let design = piperine_lang::parse_and_elaborate(RLC_PHDL, &SourceMap::dummy()).expect("RLC elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let result: PzResult = session.pz("v1", "b", None, &SolverConfig::default()).expect("pz solves");
    assert_eq!(result.poles.len(), 2, "{:?}", result.poles);
    assert!(result.zeros.is_empty());
}

// `sparam_result_is_the_typed_api_struct_with_named_accessor` is gone with
// the PHDL frontend: `@rfport(num, z0)` has no reachable Verilog-AMS
// spelling (attribute instances resolve against `ElabContext::schemas`,
// which seeds only `device`/`port`), so no source can declare the RF ports
// `.sp` needs — the frontend-dark call ed575c85 already made for
// `piperine-lang`'s own `rfport` target.

/// `DistoResult` is the typed api struct (HOST-04): `hd2`/`hd3` populated,
/// `im2`/`im3` `None` for a single-tone run — same shape as the solver's
/// `DistoResult`.
#[test]
fn disto_result_is_the_typed_api_struct() {
    let design = piperine_lang::parse_and_elaborate(POLY_PHDL, &SourceMap::dummy()).expect("poly elaborates");
    let mut session = Session::builder(&design, "Top").disto(true).compile().expect("session compiles");
    let result: DistoResult =
        session.disto(1e6, None, 0.1, "vout", None, &SolverConfig::default()).expect("disto solves");
    assert!(result.hd2.is_some());
    assert!(result.hd3.is_some());
    assert!(result.im2.is_none());
    assert!(result.im3.is_none());
}
