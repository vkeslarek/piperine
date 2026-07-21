//! Ideal lossless transmission line (Branin companion over `delay`) — SC-15.
//!
//! Matched termination shows no reflection (the far end settles at the
//! launched half-amplitude); an open far end doubles (reflection coeff +1);
//! a non-positive `z0`/`td` is a loud elaboration error.

use std::path::PathBuf;

use piperine::{NetRef, SimSession, SolverConfig};
use piperine_lang::SourceMap;

fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// Step source (0→1 V, 10 ps linear ramp — a self-contained device, avoiding
/// the spice pulse waveform) through a 50 Ω source resistor into a 50 Ω line
/// (td = 1 ns). `rl` sets the far-end termination.
fn tline_phdl(rl_ohms: &str) -> String {
    format!(
        "use piperine::disciplines;
use spice::passives;
use spice::tline;

mod step_src(inout p: Electrical, inout n: Electrical) {{
    param v: Real = 1.0; param tr: Real = 1.0e-11;
}}
analog step_src {{ V(p, n) <+ v * min(1.0, $abstime / tr); }}

mod Top() {{
    wire gnd: Electrical; wire src: Electrical; wire a: Electrical; wire b: Electrical;
    vin: step_src (.p=src,.n=gnd) {{ .v = 1.0, .tr = 1.0e-11 }};
    rs: res (.p=src,.n=a) {{ .r = 50.0 }};
    t1: tline (.p1=a,.n1=gnd,.p2=b,.n2=gnd) {{ .z0 = 50.0, .td = 1.0e-9 }};
    rl: res (.p=b,.n=gnd) {{ .r = {rl_ohms} }};
}}"
    )
}

fn session(rl_ohms: &str) -> SimSession {
    let design = piperine_lang::parse_and_elaborate(&tline_phdl(rl_ohms), &headers_source_map())
        .expect("tline design elaborates");
    SimSession::new(design, "Top".to_string())
}

/// Matched termination: launched wave = 1 V · 50/(50+50) = 0.5 V arrives at
/// the far end after td and does NOT reflect (< 1 % residual ripple). Before
/// td the far end is quiet.
#[test]
fn tline_matched_no_reflection() {
    let sess = session("50.0");
    let trace = sess
        .run_tran(4e-9, Some(2e-12), 0.0, &SolverConfig::default(), None, false)
        .expect("matched tran solves");
    let b = NetRef { name: "b".into() };

    let v_before = trace.v(&b, None).expect("v(b)").at(0.5e-9);
    let v_after = trace.v(&b, None).expect("v(b)").at(2.0e-9);
    let v_late = trace.v(&b, None).expect("v(b)").at(3.5e-9);

    assert!(v_before.abs() < 0.02, "far end quiet before td: {v_before}");
    assert!((v_after - 0.5).abs() < 0.02, "far end at half-amplitude after td: {v_after}");
    // No reflection: the level after one more round trip is unchanged (< 1 %).
    assert!((v_late - 0.5).abs() < 0.005, "no reflection ripple: {v_late}");
}

/// Open far end (10 GΩ load): reflection coefficient +1 doubles the far-end
/// voltage to the full launched amplitude (≈ 1 V) after the wave arrives.
#[test]
fn tline_open_termination_doubles() {
    let sess = session("1.0e10");
    let trace = sess
        .run_tran(4e-9, Some(2e-12), 0.0, &SolverConfig::default(), None, false)
        .expect("open tran solves");
    let b = NetRef { name: "b".into() };

    let v_before = trace.v(&b, None).expect("v(b)").at(0.5e-9);
    let v_doubled = trace.v(&b, None).expect("v(b)").at(2.0e-9);

    assert!(v_before.abs() < 0.02, "far end quiet before td: {v_before}");
    // Incident 0.5 V + fully reflected 0.5 V = 1.0 V at the open end.
    assert!((v_doubled - 1.0).abs() < 0.03, "open end doubles to full amplitude: {v_doubled}");
}

/// A non-positive characteristic impedance is a loud elaboration error.
#[test]
fn tline_bad_z0_is_loud() {
    let phdl = "use piperine::disciplines;
use spice::sources;
use spice::passives;
use spice::tline;
mod Top() {
    wire gnd: Electrical; wire a: Electrical; wire b: Electrical;
    vin: vsrc (.p=a,.n=gnd) { .dc = 1.0 };
    t1: tline (.p1=a,.n1=gnd,.p2=b,.n2=gnd) { .z0 = -1.0, .td = 1.0e-9 };
    rl: res (.p=b,.n=gnd) { .r = 50.0 };
}";
    let err = piperine_lang::parse_and_elaborate(phdl, &headers_source_map())
        .expect_err("negative z0 must fail loud");
    let msg = format!("{err}");
    assert!(msg.contains("z0"), "loud error names z0: {msg}");
}

/// A non-positive transit delay is a loud elaboration error.
#[test]
fn tline_bad_td_is_loud() {
    let phdl = "use piperine::disciplines;
use spice::sources;
use spice::passives;
use spice::tline;
mod Top() {
    wire gnd: Electrical; wire a: Electrical; wire b: Electrical;
    vin: vsrc (.p=a,.n=gnd) { .dc = 1.0 };
    t1: tline (.p1=a,.n1=gnd,.p2=b,.n2=gnd) { .z0 = 50.0, .td = 0.0 };
    rl: res (.p=b,.n=gnd) { .r = 50.0 };
}";
    let err = piperine_lang::parse_and_elaborate(phdl, &headers_source_map())
        .expect_err("zero td must fail loud");
    let msg = format!("{err}");
    assert!(msg.contains("td"), "loud error names td: {msg}");
}
