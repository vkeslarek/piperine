//! T23 — units (HOST-21): `Session::ac`'s `fstart`/`fstop` accept anything
//! `Into<Freq>`. Spec-derived from tasks.md's "Done when": `Freq::from
//! ("10MHz") == 1e7`; `f64` still accepted (existing call sites unchanged).

use piperine::{Freq, NetRef, Session, SolverConfig};
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

fn elaborate(src: &str) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates")
}

/// `Freq::from("10MHz") == 1e7` (the literal spec assertion).
#[test]
fn freq_from_si_suffixed_string() {
    assert_eq!(Freq::from("10MHz").0, 1e7);
}

/// A garbage `Freq` string fails loud (panics — `From` is infallible).
#[test]
#[should_panic(expected = "invalid Freq")]
fn freq_from_garbage_string_panics() {
    let _ = Freq::from("not-a-frequency");
}

/// `Session::ac` accepts a plain `f64` (unchanged for every existing
/// caller) and an SI-suffixed `&str`, producing identical results.
#[test]
fn session_ac_accepts_f64_and_si_string_identically() {
    let design = elaborate(RC_AC_PHDL);
    let out = NetRef { name: "out".into() };

    let mut session_f64 = Session::compile(&design, "Top").expect("session compiles");
    let ac_f64 = session_f64.ac(1.0, 1e6, 5, true, &SolverConfig::default()).expect("ac(f64) solves");
    let cw_f64 = ac_f64.v(&out).expect("v(out)");

    let mut session_str = Session::compile(&design, "Top").expect("session compiles");
    let ac_str = session_str.ac("1Hz", "1M", 5, true, &SolverConfig::default()).expect("ac(&str) solves");
    let cw_str = ac_str.v(&out).expect("v(out)");

    let mag_f64 = cw_f64.mag();
    let mag_str = cw_str.mag();
    let pts_f64 = mag_f64.points();
    let pts_str = mag_str.points();
    assert_eq!(pts_f64.len(), pts_str.len());
    assert!(!pts_f64.is_empty());
    for ((f0, m0), (f1, m1)) in pts_f64.iter().zip(pts_str.iter()) {
        assert!((f0 - f1).abs() < 1e-6, "frequency axis mismatch: {f0} vs {f1}");
        assert!((m0 - m1).abs() < 1e-12, "mag mismatch: {m0} vs {m1}");
    }
}
