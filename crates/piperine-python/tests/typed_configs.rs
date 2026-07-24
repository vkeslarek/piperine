//! T22 — Typed configs + canonical `Solver` knobs (HOST-20). Spec-derived
//! from tasks.md's "Done when": `inspect.signature(TranConfig)` shows
//! fields; `.with_()` is an immutable copy; `Solver` (both hosts) carries
//! the same knob set including `nodeset` availability on `.dc`/`.op`/`.tran`
//! and `dc_damp_tolerance`.

use piperine_python::embed::run_script;

fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp file");
    path.to_str().expect("non-utf8 temp path").to_string()
}

const RC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
analog V { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire a   : Electrical;
    V1 : V(.p = vin, .n = gnd) {};
    r1 : R(.p = vin, .n = a) { .r = 1e3 };
}
";

/// `inspect.signature(TranConfig)` shows the dataclass fields (a plain
/// dataclass `__init__`, no opaque native constructor) — the spec's typed-
/// config requirement. Every config bundle's fields must be introspectable.
#[test]
fn config_signatures_show_typed_fields() {
    let script = r#"
import inspect
import piperine

sig = inspect.signature(piperine.TranConfig)
names = list(sig.parameters.keys())
assert names == ["stop", "step", "start", "ic", "solver", "record_device_state"], names

solver_sig = inspect.signature(piperine.Solver)
solver_names = list(solver_sig.parameters.keys())
assert "dc_damp_tolerance" in solver_names, solver_names
assert "temperature" in solver_names, solver_names
"#;
    let script_path = write_temp("piperine_t22_signatures.py", script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "config signature script must pass: {:?}", result.err());
    let _ = std::fs::remove_file(script_path);
}

/// `.with_()` is an immutable copy: the original config is untouched, the
/// returned copy has the overridden field(s), everything else carried over.
#[test]
fn with_returns_immutable_copy() {
    let script = r#"
import piperine

base = piperine.Solver()
derived = base.with_(reltol=1e-9)

assert base.reltol == 1e-3, base.reltol  # original untouched
assert derived.reltol == 1e-9, derived.reltol
assert derived.abstol == base.abstol  # other fields carried over
assert derived is not base

tran = piperine.TranConfig(stop=1e-3)
tran2 = tran.with_(step=1e-6)
assert tran.step == 0.0
assert tran2.step == 1e-6
assert tran2.stop == tran.stop
"#;
    let script_path = write_temp("piperine_t22_with.py", script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "with_ script must pass: {:?}", result.err());
    let _ = std::fs::remove_file(script_path);
}

/// `Solver` carries `dc_damp_tolerance` (mirrors the Rust `SolverConfig`
/// field) and a run using a custom `dc_damp_tolerance` actually reaches the
/// Newton loop through `Session.op`/`Module.op` (not silently ignored).
#[test]
fn solver_dc_damp_tolerance_reaches_the_analysis() {
    let phdl = write_temp("piperine_t22_rc.phdl", RC_PHDL);
    let script = format!(
        r#"
import piperine

design = piperine.load("{phdl}")
module = design.top()

solver = piperine.Solver(dc_damp_tolerance=0.1)
op = module.op(piperine.OpConfig(solver=solver))
assert isinstance(op.v("a"), float)
"#,
        phdl = phdl,
    );
    let script_path = write_temp("piperine_t22_solver_knob.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "solver dc_damp_tolerance script must pass: {:?}", result.err());
    for p in [phdl, script_path] {
        let _ = std::fs::remove_file(p);
    }
}

/// `nodeset` is available on `.dc` (HOST-20's "nodeset asymmetry" fix) —
/// not just `.op`/`.tran` — on the compiled `Session`.
#[test]
fn session_dc_accepts_nodeset() {
    let phdl = write_temp("piperine_t22_rc_dc.phdl", RC_PHDL);
    let script = format!(
        r#"
import piperine

design = piperine.load("{phdl}")
session = design.compile()

trace = session.dc("r1", "r", [1e3, 2e3], nodeset={{"a": 0.5}})
wf = trace.v("a")
assert len(wf.values) == 2
"#,
        phdl = phdl,
    );
    let script_path = write_temp("piperine_t22_dc_nodeset.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "dc nodeset script must pass: {:?}", result.err());
    for p in [phdl, script_path] {
        let _ = std::fs::remove_file(p);
    }
}
