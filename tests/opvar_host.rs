//! HOST-07 (host-library T10): `OpResult::instance(label).opvar`/`.opvars()`
//! — the device introspection door over the shipped `read_opvars` bridge
//! (ABI-30), read back through the compiled center of gravity's `.op()`.

use std::path::PathBuf;

use piperine::{SimSession, SolverConfig};
use piperine_lang::SourceMap;

/// A divider whose resistors compute an opvar `g = 1/r` in their analog
/// body (the stdlib convention: `var g : Real = 0.0;` at module scope,
/// `g = 1.0 / r;` in `analog`) — mirrors
/// `piperine-codegen/tests/opvar_bridge.rs`'s fixture.
const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VoltageSource(inout p: Electrical, inout n: Electrical) {
    param voltage: Real = 0.0;
}
analog VoltageSource { V(p, n) <- voltage; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
    var g : Real = 0.0;
}
analog Resistor {
    g = 1.0 / r;
    I(p, n) <+ g * V(p, n);
}

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

/// `op.instance("r_top").opvar("g")` returns the post-solve `1/r` value —
/// the read-only bridge over the shipped `read_opvars` ABI (HOST-07 AC1).
#[test]
fn opvar_returns_the_devices_computed_operating_point_variable() {
    let session = divider_session();
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");

    let r_top = op.instance("r_top").expect("r_top is a labeled instance");
    let g_top = r_top.opvar("g").expect("r_top declares opvar g");
    assert!((g_top - 1.0 / 3e3).abs() < 1e-12, "g = 1/3000, got {g_top}");

    let r_bot = op.instance("r_bot").expect("r_bot is a labeled instance");
    let g_bot = r_bot.opvar("g").expect("r_bot declares opvar g");
    assert!((g_bot - 1.0 / 2e3).abs() < 1e-12, "g = 1/2000, got {g_bot}");
}

/// `opvars()` returns every declared opvar as `(name, value)` pairs.
#[test]
fn opvars_lists_every_declared_operating_point_variable() {
    let session = divider_session();
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    let vars = op.instance("r_top").expect("labeled instance").opvars();
    assert_eq!(vars.len(), 1, "Resistor declares exactly one opvar (g): {vars:?}");
    assert_eq!(vars[0].0, "g");
    assert!((vars[0].1 - 1.0 / 3e3).abs() < 1e-12);
}

/// An unknown opvar name fails loud (HOST-07 edge case) — never `None`/NaN.
#[test]
fn unknown_opvar_fails_loud() {
    let session = divider_session();
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    let r_top = op.instance("r_top").expect("labeled instance");
    let err = r_top.opvar("bogus").expect_err("unknown opvar must fail");
    assert!(err.to_string().contains("bogus"), "error names the bad opvar: {err}");
    assert!(err.to_string().contains("r_top"), "error names the instance: {err}");
}

/// An unknown instance label fails loud, never silently returning an empty
/// view.
#[test]
fn unknown_instance_label_fails_loud() {
    let session = divider_session();
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    let err = op.instance("nope").expect_err("unknown label must fail");
    assert!(err.to_string().contains("nope"), "error names the bad label: {err}");
}

/// A device with no declared opvars (the ideal `VoltageSource`, which has no
/// module `var`) returns an empty `opvars()` list — the ABI-30 default —
/// rather than failing.
#[test]
fn a_device_with_no_opvars_returns_an_empty_list() {
    let session = divider_session();
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    let src = op.instance("src").expect("src is a labeled instance");
    assert!(src.opvars().is_empty(), "VoltageSource declares no opvars");
}
