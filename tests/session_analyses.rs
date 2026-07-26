//! HOST-02 — every analysis is a method on `Session` with a typed return:
//! `tran`/`ac`/`noise`/`sens`/`pss`/`pz`/`disto`/`sp`, all on the one
//! compilation `Session::compile` produced. `cargo test -p piperine`
//! (Phase 1 / T4 quick gate).

use piperine::{DistoResult, NetRef, PzResult, SParamResult, Session, SolverConfig};
use piperine_lang::SourceMap;

const RC_AC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 5.0; param acmag: Real = 1.0; }
analog V { V(p, n) <+ dc + ac_stim(acmag, 0.0); }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-6; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire out : Electrical;
    v1 : V(.p = vin, .n = gnd) {};
    r1 : R(.p = vin, .n = out) { .r = 1e3 };
    c1 : C(.p = out, .n = gnd) { .c = 1e-6 };
}
";

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

fn elaborate(src: &str) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates")
}

/// `tran`/`ac`/`noise` on `Session` return the same typed containers as
/// `SimSession`'s (`Trace<Waveform>`/`Trace<ComplexWaveform>`/
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

/// `sens`/`pss`/`pz`/`disto`/`sp` on `Session` return the same typed results
/// `SimSession` does (HOST-04's uniform shape), on the same fixtures.
#[test]
fn session_sens_pss_pz_disto_sp_return_typed_results() {
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
    let mut poly_session = Session::compile(&poly_design, "Top").expect("session compiles");
    let disto: DistoResult =
        poly_session.disto(1e6, None, 0.1, "vout", None, &SolverConfig::default()).expect("disto solves");
    assert!(disto.hd2.is_some());

    let sp_design = elaborate(SHUNT_C_LOWPASS);
    let mut sp_session = Session::compile(&sp_design, "Top").expect("session compiles");
    let sp: SParamResult = sp_session.sp(1e3, 1e9, 5, true, &SolverConfig::default()).expect("sp solves");
    assert_eq!(sp.n_ports, 2);
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
