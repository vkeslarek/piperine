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

/// RC charge circuit: `out` charges through 1 kΩ / 1 µF (τ = 1 ms) toward
/// 5 V. Self-contained like the divider.
const RC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VoltageSource(inout p: Electrical, inout n: Electrical) {
    param voltage: Real = 0.0;
}
analog VoltageSource { V(p, n) <- voltage; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param resistance: Real = 1e3;
}
analog Resistor { I(p, n) <+ V(p, n) / resistance; }

mod Capacitor(inout p: Electrical, inout n: Electrical) {
    param c: Real = 1e-9;
}
analog Capacitor { I(p, n) <+ c * ddt(V(p, n)); }

mod RcCharge() {
    wire gnd : Electrical;
    wire vsrc : Electrical;
    wire out : Electrical;
    source : VoltageSource (.p = vsrc, .n = gnd) { .voltage = 5.0 };
    r1     : Resistor      (.p = vsrc, .n = out) { .resistance = 1e3 };
    c1     : Capacitor     (.p = out, .n = gnd) { .c = 1e-6 };
}
";

fn rc_session() -> SimSession {
    let design = piperine_lang::parse_and_elaborate(RC_PHDL, &headers_source_map())
        .expect("rc elaborates");
    SimSession::new(design, "RcCharge".to_string())
}

/// `run_tran` `start`: the solver integrates from t=0 (state evolution
/// matters) but only records steps with `t >= start` (ngspice `.tran tstart
/// tstop` semantics). After 5τ the node is settled, so a delayed start still
/// sees ~5 V at the first recorded sample — and that sample's time is
/// `>= start`, not 0.
#[test]
fn tran_delayed_start_records_from_start_not_zero() {
    let session = rc_session();
    let trace = session
        .run_tran(5e-3, None, 2.5e-3, &SolverConfig::default(), None)
        .expect("tran solves");
    let axis = trace.axis();
    assert!(axis.len() > 1, "delayed-start trace still has samples");
    let t0 = axis.at(0.0);
    assert!(t0 >= 2.5e-3, "recording starts at `start`, not t=0; got {t0}");
    let out = NetRef { name: "out".into() };
    let v = trace.v(&out, None).expect("v(out)");
    assert!(v.at(t0) > 4.9, "still settled at the delayed start; got {}", v.at(t0));
}

/// `run_op` accepts a nodeset hint and threads it to the DC solver as an
/// initial guess; a linear circuit converges to the same point regardless.
#[test]
fn op_nodeset_hint_is_accepted() {
    let session = rc_session();
    let nodeset = std::collections::HashMap::from([("out".to_string(), 5.0)]);
    let op = session
        .run_op(&SolverConfig::default(), Some(&nodeset))
        .expect("op with nodeset solves");
    let out = NetRef { name: "out".into() };
    assert!(op.v(&out, None).expect("v(out)") > 4.9);
}

/// `Trace::i` recomputes a two-terminal device current per step from the
/// solved terminal voltages (beyond ideal-source force branches). A pure
/// resistive divider settles instantly, so the series current is the DC
/// value: 5 V / (3k + 2k) = 1 mA at every sample, from t=0 on.
#[test]
fn trace_i_over_time_recomputes_a_resistor_current() {
    let session = divider_session();
    let trace = session
        .run_tran(1e-3, Some(1e-4), 0.0, &SolverConfig::default(), None)
        .expect("tran solves");
    let vin = NetRef { name: "vin".into() };
    let mid = NetRef { name: "mid".into() };
    let i = trace.i(&vin, Some(&mid)).expect("i(vin,mid)");
    assert!(i.len() > 1, "current waveform has samples");
    let i0 = i.at(0.0);
    assert!(
        (0.9e-3..1.1e-3).contains(&i0),
        "series current ~ 1 mA (5 V / 5 kΩ), got {i0}"
    );
}

/// The reactive recompute path (`dQ/dt` of `eval_charge`): a settled RC
/// starts at its DC operating point, so both the capacitor current and the
/// resistor current read ~0 — the path runs and reports the steady-state
/// zero (it does not only handle ideal sources).
#[test]
fn trace_i_over_time_exercises_the_reactive_path() {
    let session = rc_session();
    let trace = session
        .run_tran(1e-3, Some(1e-4), 0.0, &SolverConfig::default(), None)
        .expect("tran solves");
    let vsrc = NetRef { name: "vsrc".into() };
    let out = NetRef { name: "out".into() };
    let gnd = NetRef { name: "gnd".into() };
    let i_r = trace.i(&vsrc, Some(&out)).expect("i(vsrc,out)");
    let i_c = trace.i(&out, Some(&gnd)).expect("i(out,gnd)");
    assert!(i_r.at(0.0).abs() < 1e-6, "settled resistor current ≈ 0, got {}", i_r.at(0.0));
    assert!(i_c.at(0.0).abs() < 1e-6, "settled capacitor current ≈ 0, got {}", i_c.at(0.0));
}

/// A pure-digital design reads its logic values (0/1) straight off the DC
/// mixed-signal solve — no analog readback stage.
const DIGITAL_PHDL: &str = "\
discipline Bit { storage Boolean; }

mod BitDriver(output q : Bit) {
    param level : Real = 0.0;
    var b : Bit = 0;
}
digital BitDriver { b = level > 0.5; q <- b; }

mod Not1(input a : Bit, output y : Bit) { var r : Bit = 0; }
digital Not1 { r = !a; y <- r; }

mod Board() {
    wire na : Bit; wire ny : Bit;
    d : BitDriver(.q = na);
    g : Not1(.a = na, .y = ny);
}
";

/// `OpResult::v(bit_net)` returns the logic value: driver low → inverter
/// high; staging `d.level = 1.0` flips the chain on the next op.
#[test]
fn op_result_reads_digital_nets_directly() {
    let design = piperine_lang::parse_and_elaborate(DIGITAL_PHDL, &headers_source_map())
        .expect("digital elaborates");
    let session = SimSession::new(design, "Board".to_string());
    let na = NetRef { name: "na".into() };
    let ny = NetRef { name: "ny".into() };
    let op = session.run_op(&SolverConfig::default(), None).expect("op solves");
    assert!(op.v(&na, None).expect("v(na)").abs() < 1e-9, "driver low");
    assert!((op.v(&ny, None).expect("v(ny)") - 1.0).abs() < 1e-9, "inverter high");
    session.stage("d", "level", Value::Real(1.0));
    let op2 = session.run_op(&SolverConfig::default(), None).expect("op solves");
    assert!(op2.v(&ny, None).expect("v(ny)").abs() < 1e-9, "inverter follows the staged input");
}
