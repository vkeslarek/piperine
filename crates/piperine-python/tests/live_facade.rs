//! T7 — the typed facade `LiveSession` (LIVE-11): PHDL-name `set` with
//! error parity against the Rust solver path (identical messages), and
//! `schedule_set` reaching the transient queue (the mid-tran RC scenario
//! driven from Python).
//!
//! One `#[test]` — `run_script` shares the process-global interpreter.

use piperine_python::embed::run_script;

/// Write `body` to a temp file named `name`; return its path as a `String`.
fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp file");
    path.to_str().expect("non-utf8 temp path").to_string()
}

/// RC step-response fixture (the LIVE-06 independent-test circuit):
/// v1 (dc source) → r1 (2 kΩ) → out, c1 (1 nF) to gnd.
const RC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod R(inout p: Electrical, inout n: Electrical) {
    param r: Real = 2e3;
}
analog R { I(p, n) <+ V(p, n) / r; }

mod C(inout p: Electrical, inout n: Electrical) {
    param c: Real = 1e-9;
}
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Vsrc(inout p: Electrical, inout n: Electrical) {
    param dc: Real = 0.0;
}
analog Vsrc { V(p, n) <- dc; }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire out : Electrical;
    v1 : Vsrc(.p = vin, .n = gnd) {};
    r1 : R(.p = vin, .n = out) {};
    c1 : C(.p = out, .n = gnd) {};
}
";

/// The exact solver-path error message for a live set on the same compiled
/// circuit — the parity oracle the Python assertions compare against
/// (LIVE-11: "same errors" means the same strings, not lookalikes).
fn solver_set_error(label: &str, param: &str) -> String {
    let design = piperine_lang::parse_and_elaborate(RC_PHDL, &piperine_lang::SourceMap::dummy())
        .expect("fixture elaborates");
    let bodies = piperine_codegen::ir::lower_bodies(&design).expect("fixture lowers");
    let mut compiler = piperine_codegen::CircuitCompiler::new(&design, &bodies);
    let mut circuit = compiler.build_circuit("Top").expect("fixture builds");
    circuit
        .set_element_param(label, param, piperine_solver::abi::Value::Real(1.0))
        .expect_err("oracle set must fail")
        .to_string()
}

/// LIVE-11: facade `set` raises the Rust path's exact messages (`KeyError`
/// for unknown label/param), and `schedule_set` reaches the transient
/// queue — the RC 2k→1k switch at t = 5 µs lands exactly and the waveform
/// follows the new time constant (closed form, reltol 1e-3).
#[test]
fn facade_live_session_has_error_parity_and_scheduled_sets() {
    let phdl = write_temp("piperine_live_facade_rc.phdl", RC_PHDL);
    let err_label = solver_set_error("nope", "r");
    let err_param = solver_set_error("r1", "bogus");
    assert!(err_label.contains("nope"), "oracle names the label: {err_label}");
    assert!(err_param.contains("bogus"), "oracle names the param: {err_param}");

    let script = format!(
        r#"import numpy as np
import piperine

design = piperine.load("{phdl}")

# Both facade entry points build a LiveSession.
s2 = design.module("Top").compile()
assert type(s2).__name__ == "LiveSession"
s = design.compile()          # top-module shorthand (Top is the only root)
assert type(s).__name__ == "LiveSession"
assert s.rebuilds == 0

# ── LIVE-11: error parity with the Rust solver path (exact messages) ────
try:
    s.set("nope", "r", 1.0)
    raise AssertionError("unknown label must raise")
except KeyError as e:
    assert e.args[0] == {err_label:?}, e.args[0]

try:
    s.set("r1", "bogus", 1.0)
    raise AssertionError("unknown param must raise")
except KeyError as e:
    assert e.args[0] == {err_param:?}, e.args[0]

# ── schedule_set reaches the transient queue (mid-tran RC scenario) ─────
t_on, t_sw = 1e-6, 5e-6
tau1, tau2 = 2e3 * 1e-9, 1e3 * 1e-9
s.schedule_set(t_on, "v1", "dc", 5.0)
s.schedule_set(t_sw, "r1", "r", 1e3)
trace = s.tran(piperine.TranConfig(stop=12e-6, step=0.1e-6))
wf = trace.v("out")
t, v = wf.axis, wf.values

# Exact landing on each scheduled time (unified breakpoint table).
for ts in (t_on, t_sw):
    assert np.sum(np.abs(t - ts) < 1e-18) == 1, ts

# Closed form: charge with tau1 from t_on, then settle with tau2 from t_sw.
v_sw = 5.0 * (1.0 - np.exp(-(t_sw - t_on) / tau1))
ref = np.where(
    t <= t_on,
    0.0,
    np.where(
        t <= t_sw,
        5.0 * (1.0 - np.exp(-(t - t_on) / tau1)),
        5.0 + (v_sw - 5.0) * np.exp(-(t - t_sw) / tau2),
    ),
)
assert np.all(np.abs(v - ref) <= 1e-3 * 5.0 + 1e-6), np.max(np.abs(v - ref))

# The queue drained: a second tran has no scheduled sets, and the session
# keeps the final values (r1 = 1 kΩ) — flat settle at 5 V.
trace2 = s.tran(piperine.TranConfig(stop=2e-6, step=0.1e-6))
assert np.all(np.abs(trace2.v("out").values - 5.0) < 0.05)
"#
    );
    let script_path = write_temp("piperine_live_facade_script.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "facade live-session script must pass: {:?}", result.err());

    for p in [phdl, script_path] {
        let _ = std::fs::remove_file(p);
    }
}
