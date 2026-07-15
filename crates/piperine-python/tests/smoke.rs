//! P12 — embedded smoke test: the uniform-shape proof (PY-17 / spec success
//! criteria). Runs a Python script through the embed path that exercises
//! `load → module → op/tran/ac/noise → numpy` via the typed public facade,
//! and asserts the Python results match the bench's results for the same
//! circuits (the binding invariant — PY-17 / spec §10).
//!
//! The divider (`vin=5 V`, `r_top=3 kΩ`, `r_bot=2 kΩ` → `mid=2.0 V`) is the
//! same circuit the bench tests solve; the AC fixture (`ac_stim(1.0)` into
//! `1 kΩ` → `|V_out|=1000 V`) mirrors `piperine-bench/tests/bench.rs`'s
//! `ac_stim_drives_a_low_pass_response`. Asserting those values from Python
//! IS the uniform-shape proof.
//!
//! One test function — `run_script` performs the global Python init; a single
//! call keeps the init sequential (avoids the process-global interpreter
//! race across tests).

use piperine_python::embed::run_script;

/// Write `body` to a temp file named `name`; return its path as a `String`.
fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp file");
    path.to_str().expect("non-utf8 temp path").to_string()
}

/// The divider circuit — same shape the bench tests solve. `mid` sits at
/// `5·2/(3+2) = 2.0 V`; staging is not exercised here (P6's unit test covers
/// it). Mirrors `piperine-python/src/lib.rs::ANALYSIS_PHDL`.
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

/// AC + noise fixture: an `ac_stim(1.0)` current source driving a 1 kΩ
/// resistor to ground → `|V_out| = 1 A × 1 kΩ = 1000 V` at every frequency
/// (resistive, flat). Mirrors the bench's AC test circuit + the P9 unit test.
const AC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod AcSource(inout p: Electrical, inout n: Electrical) { }
analog AcSource { I(p, n) <+ -ac_stim(1.0); }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}
analog Resistor { I(p, n) <+ V(p, n) / r; }

mod AcTest() {
    wire gnd : Electrical;
    wire out : Electrical;
    stim : AcSource (.p = out, .n = gnd);
    r1   : Resistor (.p = out, .n = gnd) { .r = 1e3 };
}
";

/// Noise fixture: a noisy resistor with explicit `white_noise` so the PSD is
/// non-zero and the integrated total is observable. Mirrors the P9 unit test.
const NOISE_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod NoisyResistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}
analog NoisyResistor { I(p, n) <+ V(p, n) / r + white_noise(4 * 8.617e-5 * 300.15 / r); }

mod NoiseTest() {
    wire gnd : Electrical;
    wire out : Electrical;
    nr : NoisyResistor (.p = out, .n = gnd) { .r = 1e3 };
}
";

/// PY-17 / spec success criteria: a Python script going through the typed
/// facade produces numpy arrays that match the bench's `$tran` + result
/// readouts for the same circuit. This is the binding uniform-shape proof.
///
/// Coverage: `load → module → op/tran/ac/noise` via the facade dataclasses
/// (`TranConfig`/`AcConfig`/`NoiseConfig`), result access (`.v/.i/.values/
/// .axis`), net-name `__getitem__` (AC5/10), and instance-path `__getitem__`
/// (AC13). Every asserted value is the spec-defined outcome for the circuit.
#[test]
fn uniform_shape_matches_bench() {
    let divider_path = write_temp("piperine_smoke_divider.phdl", DIVIDER_PHDL);
    let ac_path = write_temp("piperine_smoke_ac.phdl", AC_PHDL);
    let noise_path = write_temp("piperine_smoke_noise.phdl", NOISE_PHDL);

    // The Python script is the success-criteria script (spec §10): the full
    // load → reflect → analyze → read loop through the typed facade.
    // `format!` interpolates the temp PHDL paths; Python braces would collide
    // with Rust's format placeholders, so the script avoids dict/f-string
    // literals.
    let script = format!(
        r#"import piperine
import numpy as np

# ── Divider: op + tran + instance-path (uniform shape vs bench) ──────────
design = piperine.load("{divider_path}")
divider = design.module("Divider")

# op: mid = 5 * 2/(3+2) = 2.0 V — the same value the bench computes (PY-17).
op = divider.op()
assert abs(op.v("mid") - 2.0) < 1e-6, op.v("mid")
assert abs(op.v("vin") - 5.0) < 1e-6, op.v("vin")
assert abs(op.v("vin", "mid") - 3.0) < 1e-6, op.v("vin", "mid")
assert abs(op.i("vin", "mid") - 1e-3) < 1e-9, op.i("vin", "mid")

# AC5: op["net"] == op.v("net").
assert abs(op["mid"] - op.v("mid")) < 1e-12

# tran: the DC divider has no dynamics, so mid is flat at 2.0 V across the
# grid — the numpy array matches the bench's tran for the same circuit
# (PY-17, the uniform-shape proof).
trace = divider.tran(piperine.TranConfig(stop=5e-3, step=1e-5))
wf = trace.v("mid")
assert isinstance(wf.values, np.ndarray), type(wf.values)
assert wf.values.dtype == np.float64, wf.values.dtype
assert wf.axis.dtype == np.float64
assert len(wf.values) == len(wf.axis)
assert len(wf.values) > 1
assert np.allclose(wf.values, 2.0, atol=1e-3), wf.values

# AC10: trace["net"] returns the same waveform (values array equality).
assert np.allclose(trace["mid"].values, wf.values)

# AC13: op["r_top"] returns a terminal sub-view of the r_top instance.
view = op["r_top"]
assert view.label == "r_top"
assert abs(view.v("p") - 5.0) < 1e-6, view.v("p")      # connected net vin
assert abs(view.v("n") - 2.0) < 1e-6, view.v("n")      # connected net mid
assert abs(view.v("p", "n") - 3.0) < 1e-6, view.v("p", "n")  # drop across r_top
assert abs(view.i("p", "n") - 1e-3) < 1e-9, view.i("p", "n")  # branch current

# trace["r_top"] returns the same view over waveforms.
tview = trace["r_top"]
twf = tview.v("n")
assert isinstance(twf.values, np.ndarray)
assert np.allclose(twf.values, 2.0, atol=1e-3)

# ── AC fixture: ac + projections (uniform shape vs bench AC test) ────────
ac_design = piperine.load("{ac_path}")
ac_module = ac_design.module("AcTest")
ac = ac_module.ac(piperine.AcConfig(fstart=1.0, fstop=1e6, points=10))
cw = ac.v("out")
assert cw.values.dtype == np.complex128, cw.values.dtype
assert len(cw.values) == 10
# 1 A * 1 kΩ = 1000 V at every frequency (resistive, flat) — matches the
# bench's ac_stim_drives_a_low_pass_response (PY-17).
assert np.allclose(np.abs(cw.values), 1000.0, atol=1.0), np.abs(cw.values)
# Projections return real waveforms.
assert cw.mag.values.dtype == np.float64
assert cw.phase.values.dtype == np.float64
assert cw.db.values.dtype == np.float64
assert len(cw.mag.values) == 10

# ── Noise fixture: psd + total (uniform shape vs bench noise test) ───────
noise_design = piperine.load("{noise_path}")
noise_module = noise_design.module("NoiseTest")
noise = noise_module.noise(piperine.NoiseConfig(out="out", fstart=1.0, fstop=1e6, points=5))
psd = noise.psd()
assert isinstance(psd.values, np.ndarray)
assert len(psd.values) == 5
assert np.all(psd.values >= 0.0), psd.values
assert noise.total() >= 0.0
"#,
        divider_path = divider_path,
        ac_path = ac_path,
        noise_path = noise_path,
    );

    let script_path = write_temp("piperine_smoke_script.py", &script);
    let result = run_script(&script_path);

    // The Python script's assertions ARE the uniform-shape proof. If any
    // fails, the AssertionError propagates as Err carrying the diagnostic.
    assert!(
        result.is_ok(),
        "smoke script must pass (uniform shape vs bench): {:?}",
        result.err()
    );

    let _ = std::fs::remove_file(divider_path);
    let _ = std::fs::remove_file(ac_path);
    let _ = std::fs::remove_file(noise_path);
    let _ = std::fs::remove_file(script_path);
}
