//! host-library T11 (HOST-08): `tran(probe = [...])` selectively records a
//! device's opvar over time, and `Trace::opvar("instance.name")` reads it back
//! as a `Waveform`. The recording rides the shipped `ProbeSelection`
//! (ABI-33/34) + `record_device_state` plumbing; the read-back recomputes the
//! opvar per step from the recorded `(state, vars)` bank via the kernel's
//! `eval_opvars` (the same path `OpResult::instance(label).opvar(name)` walks
//! at a single point).
//!
//! Coverage: happy path (recorded opvar's time-mean matches the DC opvar at
//! the same held point), the ABI-35 fail-loud (unknown device / unknown
//! observable at setup), and the read-back's own fail-loud paths (opvar not
//! recorded because no `probe=` / unknown instance / unknown opvar name).

use std::path::PathBuf;

use piperine::{NetRef, SimSession, SolverConfig};
use piperine_lang::SourceMap;

/// A resistor that exports its conductance under the display name `cond`
/// (`@name(value = "cond")` — one declaration, both the opvar catalog and the
/// observable catalog surface it under that name; PIA-07). Driven by a
/// constant 5 V source: at DC, `cond = 1/r = 1e-3 S` and the value is
/// time-invariant, so a transient's `cond`-over-time is flat — its `.mean()`
/// must match the DC opvar (the design.md risk-row validation: "validate
/// against a DC opvar at a static point").
const PROBE_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VoltageSource(inout p: Electrical, inout n: Electrical) {
    param voltage: Real = 0.0;
}
analog VoltageSource { V(p, n) <- voltage; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
    @name(value = \"cond\") var g : Real = 0.0;
}
analog Resistor {
    g = 1.0 / r;
    I(p, n) <+ g * V(p, n);
}

mod Top() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    src : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
    r1  : Resistor      (.p = vin, .n = gnd) { .r = 1e3 };
}
";

fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn probe_session() -> SimSession {
    let design = piperine_lang::parse_and_elaborate(PROBE_PHDL, &headers_source_map())
        .expect("probe fixture elaborates");
    SimSession::new(design, "Top".to_string())
}

/// HOST-08 AC1 + AC3: `tran(probe = ["r1.cond"])` records the opvar per step,
/// `trace.opvar("r1.cond")` returns a `Waveform`, and its `.mean()` matches
/// the DC opvar `op.instance("r1").opvar("cond")` at the same held point
/// (5 V steady source → `cond = 1/r = 1e-3 S` for every step).
#[test]
fn recorded_opvar_over_time_matches_dc_opvar_at_a_held_point() {
    let session = probe_session();

    let dc_opvar = session
        .run_op(&SolverConfig::default(), None)
        .expect("op solves")
        .instance("r1")
        .expect("r1 exists")
        .opvar("cond")
        .expect("opvar `cond` reads back");
    assert!(
        (dc_opvar - 1.0e-3).abs() < 1.0e-12,
        "DC opvar cond = 1/r = 1e-3 S, got {dc_opvar:e}"
    );

    let trace = session
        .run_tran(1e-3, Some(1e-5), 0.0, &SolverConfig::default(), None, false, &["r1.cond"])
        .expect("tran with probe solves");
    let wf = trace.opvar("r1.cond").expect("recorded opvar reads back as a Waveform");

    let n = wf.points().len();
    assert!(n > 1, "waveform has more than one sample, got {n}");
    for &(_, v) in wf.points() {
        assert!((v - dc_opvar).abs() < 1.0e-9, "every step matches the DC opvar, got {v:e}");
    }
    let mean = wf.mean();
    assert!(
        (mean - dc_opvar).abs() < 1.0e-9,
        "mean of recorded opvar matches DC opvar: mean = {mean:e}, dc = {dc_opvar:e}"
    );
}

/// HOST-08 AC2 / ABI-35: an unknown observable on a known device fails loud
/// at solver setup (before any step). The error names the device and the
/// offending observable so a host can pinpoint the typo.
#[test]
fn unknown_observable_fails_loud_at_setup() {
    let session = probe_session();
    let err = session
        .run_tran(1e-3, Some(1e-5), 0.0, &SolverConfig::default(), None, false, &["r1.bogus"])
        .expect_err("unknown observable must fail loud");
    let msg = format!("{err}");
    assert!(
        msg.contains("r1") && msg.contains("bogus"),
        "error names the device + observable, got: {msg}"
    );
}

/// HOST-08 AC2 / ABI-35: an unknown device label fails loud at setup with a
/// "device not found"-shaped message.
#[test]
fn unknown_device_fails_loud_at_setup() {
    let session = probe_session();
    let err = session
        .run_tran(1e-3, Some(1e-5), 0.0, &SolverConfig::default(), None, false, &["ghost.cond"])
        .expect_err("unknown device must fail loud");
    let msg = format!("{err}");
    assert!(
        msg.contains("ghost"),
        "error names the missing device, got: {msg}"
    );
}

/// HOST-08 edge case: a malformed probe path (no `.`) fails loud with a
/// "must be `instance.name`" hint — never silently parses to `(path, "")`.
#[test]
fn malformed_probe_path_fails_loud() {
    let session = probe_session();
    let err = session
        .run_tran(1e-3, Some(1e-5), 0.0, &SolverConfig::default(), None, false, &["no-dot-here"])
        .expect_err("malformed probe path must fail loud");
    let msg = format!("{err}");
    assert!(
        msg.contains("instance.name") && msg.contains("no-dot-here"),
        "error hints the expected shape, got: {msg}"
    );
}

/// HOST-08 read-back fail-loud: `Trace::opvar` on a trace that did NOT
/// request the opvar via `probe=` (and doesn't run with
/// `record_device_state = true`) fails loud — never returns an empty/zero
/// waveform. The error names the opvar path and points the user at the
/// `probe = [...]` opt-in.
#[test]
fn opvar_read_fails_loud_when_not_recorded() {
    let session = probe_session();
    let trace = session
        .run_tran(1e-3, Some(1e-5), 0.0, &SolverConfig::default(), None, false, &[])
        .expect("tran without probe solves");
    let err = trace.opvar("r1.cond").expect_err("unrecorded opvar read must fail loud");
    let msg = format!("{err}");
    assert!(
        msg.contains("r1.cond") && msg.contains("probe"),
        "error names the opvar path + the probe opt-in, got: {msg}"
    );
}

/// HOST-08 read-back fail-loud: `Trace::opvar` on an unknown instance / opvar
/// name (when the opvar path IS being recorded for a different instance)
/// fails loud listing the available opvars — mirrors
/// `OpResult::instance(label).opvar(name)`'s not-found shape.
#[test]
fn opvar_read_fails_loud_on_unknown_name() {
    let session = probe_session();
    let trace = session
        .run_tran(1e-3, Some(1e-5), 0.0, &SolverConfig::default(), None, false, &["r1.cond"])
        .expect("tran with probe solves");
    let err = trace.opvar("r1.bogus").expect_err("unknown opvar name must fail loud");
    let msg = format!("{err}");
    assert!(
        msg.contains("r1") && msg.contains("cond") && msg.contains("bogus"),
        "error names the instance + lists the available opvar, got: {msg}"
    );
}

/// `record_device_state = true` (the "record everything" shorthand, ABI-34)
/// is equivalent to enumerating every observable in `probe=` — `Trace::opvar`
/// reads back successfully without an explicit `probe` list. Guards the
/// documented equivalence in `probe_selection.rs`'s contract comment.
#[test]
fn record_device_state_true_enables_opvar_read_without_explicit_probe() {
    let session = probe_session();
    let trace = session
        .run_tran(1e-3, Some(1e-5), 0.0, &SolverConfig::default(), None, true, &[])
        .expect("tran with full recording solves");
    let wf = trace.opvar("r1.cond").expect("record_device_state records every observable");
    let mean = wf.mean();
    assert!(
        (mean - 1.0e-3).abs() < 1.0e-9,
        "record_device_state path recovers the same opvar value, got {mean:e}"
    );
    // Sanity: the returned object really is a Waveform (axis + values), not
    // some default-constructed empty placeholder.
    let _net = NetRef { name: "vin".into() };
    assert!(!wf.points().is_empty(), "recorded waveform is non-empty");
}
