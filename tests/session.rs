//! Root-crate host-API smoke (BRM-04): a Rust host drives
//! load → elaborate → compile → simulate through `piperine::session`
//! directly — no bench crate, no interpreter.

use std::path::PathBuf;

use piperine::{NetRef, SimSession, SolverConfig};
use piperine_lang::{SourceMap, Value};

/// Self-contained divider (defines its own discipline + devices — no prelude
/// dependency): `mid = 5·2k/(3k+2k) = 2.0 V` by default; staging
/// `r_top.r = 2e3` gives `5·2k/(2k+2k) = 2.5 V`.
const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VoltageSource(inout p: Electrical, inout n: Electrical) {
    param voltage: Real = 0.0;
}
analog VoltageSource { V(p, n) <- voltage; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}
analog Resistor { I(p, n) <+ V(p, n) / r; }

mod Divider() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire mid  : Electrical;
    src   : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
    r_top : Resistor      (.p = vin, .n = mid) { .r = 3e3 };
    r_bot : Resistor      (.p = mid, .n = gnd) { .r = 2e3 };
}
";

fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn divider_session() -> SimSession {
    let design = piperine_lang::parse_and_elaborate(DIVIDER_PHDL, &headers_source_map())
        .expect("divider elaborates");
    SimSession::new(design, "Divider".to_string())
}

/// Default divider: `v(mid) = 2.0 V`; the top resistor carries
/// `i(vin→mid) = 5 V / 5 kΩ = 1 mA` into the device, and the source branch
/// reads `−1 mA` (the source delivers the current — positive `i(a,b)` flows
/// from terminal `a` into the device, so a delivering source is negative).
#[test]
fn run_op_solves_and_reads_back_by_net_name() {
    let session = divider_session();
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    let mid = NetRef { name: "mid".into() };
    let vin = NetRef { name: "vin".into() };
    let gnd = NetRef { name: "gnd".into() };
    assert!((op.v(&mid, None).expect("v(mid)") - 2.0).abs() < 1e-9);
    assert!((op.v(&vin, Some(&gnd)).expect("v(vin,gnd)") - 5.0).abs() < 1e-9);
    assert!((op.i(&vin, Some(&mid)).expect("i(vin,mid)") - 1e-3).abs() < 1e-12);
    assert!((op.i(&vin, Some(&gnd)).expect("i(vin,gnd)") + 1e-3).abs() < 1e-12);
}

/// A staged override is consumed by the next analysis: `r_top.r = 2e3` →
/// `v(mid) = 2.5 V`.
#[test]
fn staged_override_reaches_the_next_analysis() {
    let session = divider_session();
    session.stage("r_top", "r", Value::Real(2e3));
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    let mid = NetRef { name: "mid".into() };
    assert!((op.v(&mid, None).expect("v(mid)") - 2.5).abs() < 1e-9);
}

/// An unknown net name fails loud — never a silent 0.0.
#[test]
fn unaddressable_net_is_a_loud_error() {
    let session = divider_session();
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    let bogus = NetRef { name: "bogus".into() };
    let err = op.v(&bogus, None).expect_err("bogus net must fail");
    assert!(err.to_string().contains("net `bogus` is not addressable"));
}
