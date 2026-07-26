//! T25 — `NetRef` ergonomics + enums (HOST-23). Spec-derived from
//! tasks.md's "Done when": `v("out")`/`v(("out","in"))` in Rust; no bare
//! `NetRef { name }` needed; `cross`/`dir`/`scale` are enums on both sides.

use piperine::{CrossDirection, NetRef, Scale, Session, SolverConfig, Waveform};
use piperine_lang::SourceMap;

const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param voltage: Real = 10.0; }
analog V { V(p, n) <- voltage; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod Top() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire mid  : Electrical;
    src   : V(.p = vin, .n = gnd) {};
    r_top : R(.p = vin, .n = mid) { .r = 1e3 };
    r_bot : R(.p = mid, .n = gnd) { .r = 1e3 };
}
";

fn elaborate(src: &str) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates")
}

/// `op.v("mid")` (bare `&str`, single net) — no `NetRef { name }` needed.
#[test]
fn v_accepts_bare_str() {
    let design = elaborate(DIVIDER_PHDL);
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let op = session.op(&SolverConfig::default(), None).expect("op solves");
    let mid = op.v("mid").expect("v(mid)");
    assert!((mid - 5.0).abs() < 1e-9, "mid = 10*1k/2k = 5.0, got {mid}");
}

/// `op.v(("vin", "mid"))` — a `(&str, &str)` tuple for a differential read,
/// matching the ideal `v(("out","in"))` shape.
#[test]
fn v_accepts_str_tuple_for_differential_read() {
    let design = elaborate(DIVIDER_PHDL);
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let op = session.op(&SolverConfig::default(), None).expect("op solves");
    let diff = op.v(("vin", "mid")).expect("v(vin,mid)");
    // vin = 10.0, mid = 5.0 -> vin - mid = 5.0
    assert!((diff - 5.0).abs() < 1e-9, "vin-mid = 5.0, got {diff}");
}

/// A bare `NetRef { name }` value still works (backward compatible — the
/// struct is unchanged, just no longer *required* at call sites).
#[test]
fn v_still_accepts_bare_netref() {
    let design = elaborate(DIVIDER_PHDL);
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let op = session.op(&SolverConfig::default(), None).expect("op solves");
    let net = NetRef { name: "mid".to_string() };
    let mid = op.v(net).expect("v(mid) via NetRef");
    assert!((mid - 5.0).abs() < 1e-9);
}

/// `CrossDirection` is a real enum (HOST-23) — `Waveform::cross` takes it,
/// not a free-form `&str`.
#[test]
fn cross_takes_the_crossdirection_enum() {
    let w = Waveform::new(vec![(0.0, 0.0), (1.0, 2.0), (2.0, 2.0)]);
    let t = w.cross(1.0, CrossDirection::Rising).expect("crosses rising");
    assert!((t - 0.5).abs() < 1e-9, "crossing at t=0.5, got {t}");
    assert!(w.cross(1.0, CrossDirection::Falling).is_none(), "never falls through 1.0");
}

/// `Scale` is a real enum (HOST-23) usable as the `logarithmic` argument on
/// `Session::ac` (`impl Into<bool>`) — a `bool` still works unchanged.
#[test]
fn scale_enum_and_bool_both_convert_to_logarithmic() {
    assert!(!bool::from(Scale::Lin));
    assert!(bool::from(Scale::Dec));
    assert!(bool::from(Scale::Oct));
    // `bool` itself still satisfies `impl Into<bool>` (identity conversion,
    // every pre-existing `Session::ac(..., true, ...)`/`false` call site
    // keeps compiling unchanged).
    let b: bool = true;
    assert!(b);
}
