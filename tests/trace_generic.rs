//! HOST-13 — `Trace<T>` generic: `AcTrace`/`NoiseTrace` are folded into the
//! same generic container as the transient `Trace`, not separate structs.
//! `cargo test -p piperine` (Phase 1 / T1 quick gate).

use piperine::waveform::NoiseSample;
use piperine::{AcTrace, NetRef, NoiseTrace, Session, SolverConfig, Trace, Waveform};
use piperine_lang::SourceMap;

const RC_PHDL: &str = "\
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

fn session() -> Session {
    let design = piperine_lang::parse_and_elaborate(RC_PHDL, &SourceMap::dummy()).expect("RC elaborates");
    Session::compile(&design, "Top").expect("session compiles")
}

/// `AcTrace` is literally `Trace<ComplexWaveform>` — the same generic
/// container as the transient/DC `Trace<Waveform>` (HOST-13 AC1/AC3), not a
/// distinct struct. Proven at the type level (`TypeId`) so a future
/// regression that reintroduces a separate `AcTrace` struct fails to
/// compile-time-unify here.
#[test]
fn ac_trace_is_the_same_generic_container_as_trace() {
    use std::any::TypeId;
    assert_eq!(
        TypeId::of::<AcTrace>(),
        TypeId::of::<Trace<piperine::ComplexWaveform>>(),
        "AcTrace must be the AC instantiation of the generic Trace<T>"
    );
    assert_ne!(
        TypeId::of::<AcTrace>(),
        TypeId::of::<Trace<Waveform>>(),
        "the AC instantiation must be distinct from the transient/DC instantiation"
    );
}

/// Same proof for the noise instantiation (HOST-13 AC2): `NoiseTrace` is
/// `Trace<NoiseSample>`, not a separate struct.
#[test]
fn noise_trace_is_the_same_generic_container_as_trace() {
    use std::any::TypeId;
    assert_eq!(
        TypeId::of::<NoiseTrace>(),
        TypeId::of::<Trace<NoiseSample>>(),
    );
}

/// The transient trace reads real samples; the AC trace over the same
/// circuit reads complex samples — one container, two sample types
/// (HOST-13 AC1's independent test).
#[test]
fn transient_is_real_and_ac_is_complex_on_the_same_container() {
    let mut s = session();
    let out = NetRef { name: "out".into() };

    let tran = s.tran(1e-3, Some(1e-5), 0.0, &SolverConfig::default(), None, false, &[]).expect("tran solves");
    let w = tran.v(&out).expect("v(out) real");
    assert!(w.len() > 1);

    let ac = s.ac(1.0, 1e6, 5, true, &SolverConfig::default()).expect("ac solves");
    let cw = ac.v(&out).expect("v(out) complex");
    assert!(cw.len() > 1);
    // A genuinely complex sample away from DC (nonzero imaginary part
    // somewhere in the sweep) — proves this is not just a real value
    // reinterpreted.
    assert!(cw.points().iter().any(|(_, c)| c.im.abs() > 1e-12), "AC response must have a reactive (complex) component");
}

/// The noise trace still exposes `psd`/`total` after the fold (HOST-13 AC2).
#[test]
fn noise_trace_still_exposes_psd_and_total() {
    let mut s = session();
    let nz = s.noise("out", "gnd", (1.0, 1e6), 5, true, &SolverConfig::default()).expect("noise solves");
    let psd = nz.psd();
    assert!(!psd.is_empty());
    assert!(nz.total() >= 0.0);
}
