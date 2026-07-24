//! HOST-02/HOST-04 — the api result types that were previously either
//! re-exported solver structs or missing entirely: `PzResult`, `DistoResult`,
//! `SParamResult` are now constructed by `SimSession::run_pz`/`run_disto`/
//! `run_sp`; `TfResult` is exercised once `Session::tf` lands (T5).
//! `cargo test -p piperine` (Phase 1 / T2 quick gate).

use piperine::{DistoResult, PzResult, SParamResult, SimSession, SolverConfig};
use piperine_lang::SourceMap;

const RLC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
analog V { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod L(inout p: Electrical, inout n: Electrical) { param l: Real = 1e-3; }
analog L { V(p, n) <- l * ddt(I(p, n)); }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-6; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire a   : Electrical;
    wire b   : Electrical;
    v1 : V(.p = vin, .n = gnd) {};
    r1 : R(.p = vin, .n = a) { .r = 10.0 };
    l1 : L(.p = a, .n = b) { .l = 1e-3 };
    c1 : C(.p = b, .n = gnd) { .c = 1e-6 };
}
";

const SHUNT_C_LOWPASS: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1.0; }
analog R { I(p, n) <+ V(p, n) / r; }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-9; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Top() {
    wire gnd : Electrical;
    @rfport(num = 1, z0 = 50) wire p1 : Electrical;
    @rfport(num = 2, z0 = 50) wire p2 : Electrical;
    rs  : R(.p = p1, .n = p2) { .r = 1.0 };
    c1  : C(.p = p2, .n = gnd) { .c = 1e-9 };
    rb1 : R(.p = p1, .n = gnd) { .r = 1e9 };
    rb2 : R(.p = p2, .n = gnd) { .r = 1e9 };
}
";

const POLY_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 0.0; param acmag: Real = 0.0; }
analog V { V(p, n) <+ dc + ac_stim(acmag, 0.0); }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 50.0; }
analog R { I(p, n) <+ V(p, n) / r; }

mod PolyVccs(inout inp: Electrical, inout inn: Electrical,
             inout outp: Electrical, inout outn: Electrical) {
    param g1: Real = 0.1;
    param g2: Real = 0.02;
    param g3: Real = 0.003;
}
analog PolyVccs {
    I(outp, outn) <+ g1 * V(inp, inn)
                   + g2 * V(inp, inn) * V(inp, inn)
                   + g3 * V(inp, inn) * V(inp, inn) * V(inp, inn);
}

mod Top() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire vout : Electrical;
    v1 : V(.p = vin, .n = gnd) { .dc = 0.0, .acmag = 1.0 };
    n1 : PolyVccs(.inp = vin, .inn = gnd, .outp = vout, .outn = gnd) {};
    r1 : R(.p = vout, .n = gnd) { .r = 50.0 };
}
";

/// `PzResult` is the typed api struct (HOST-04): same `poles`/`zeros` field
/// shape the solver's own `PoleZeroResult` carries, on the series-RLC's known
/// analytic complex-conjugate pole pair.
#[test]
fn pz_result_is_the_typed_api_struct() {
    let design = piperine_lang::parse_and_elaborate(RLC_PHDL, &SourceMap::dummy()).expect("RLC elaborates");
    let session = SimSession::new(design, "Top".to_string());
    let result: PzResult = session.run_pz("v1", "b", None, &SolverConfig::default()).expect("pz solves");
    assert_eq!(result.poles.len(), 2, "{:?}", result.poles);
    assert!(result.zeros.is_empty());
}

/// `SParamResult` is the typed api struct (HOST-04): the same field shape as
/// the solver's `SpResult`, plus the named `s(k, i, j)` accessor over the raw
/// matrix — HOST-04's "s(i,j)" surface, not an untyped tuple.
#[test]
fn sparam_result_is_the_typed_api_struct_with_named_accessor() {
    let design =
        piperine_lang::parse_and_elaborate(SHUNT_C_LOWPASS, &SourceMap::dummy()).expect("shunt-C elaborates");
    let session = SimSession::new(design, "Top".to_string());
    let result: SParamResult = session.run_sp(1e3, 1e9, 5, true, &SolverConfig::default()).expect("sp solves");
    assert_eq!(result.n_ports, 2);
    assert_eq!(result.frequencies.len(), 5);
    for k in 0..result.frequencies.len() {
        assert_eq!(result.s(k, 0, 0), result.s[k][[0, 0]], "named accessor matches the raw matrix");
    }
}

/// `DistoResult` is the typed api struct (HOST-04): `hd2`/`hd3` populated,
/// `im2`/`im3` `None` for a single-tone run — same shape as the solver's
/// `DistoResult`.
#[test]
fn disto_result_is_the_typed_api_struct() {
    let design = piperine_lang::parse_and_elaborate(POLY_PHDL, &SourceMap::dummy()).expect("poly elaborates");
    let session = SimSession::new(design, "Top".to_string());
    let result: DistoResult =
        session.run_disto(1e6, None, 0.1, "vout", None, &SolverConfig::default()).expect("disto solves");
    assert!(result.hd2.is_some());
    assert!(result.hd3.is_some());
    assert!(result.im2.is_none());
    assert!(result.im3.is_none());
}
