//! T24 — `SimulationError` hierarchy (HOST-22). Spec-derived from
//! tasks.md's "Done when": `SimulationError` base + `ConvergenceError
//! (node/iteration/analysis)`/`ElaborationError`/`UnknownModule`/
//! `UnknownNet`; a non-converging run raises `ConvergenceError`.

use piperine_python::embed::run_script;

fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp file");
    path.to_str().expect("non-utf8 temp path").to_string()
}

const RC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 5.0; }
analog V { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire out : Electrical;
    v1 : V(.p = vin, .n = gnd) {};
    r1 : R(.p = vin, .n = out) { .r = 1e3 };
}
";

/// The hierarchy exists with the exact named classes, `SimulationError` as
/// the common base for all of them.
#[test]
fn hierarchy_classes_exist_and_share_a_base() {
    let script = r#"
import piperine as pip

for name in ("SimulationError", "ElaborationError", "UnknownModule", "UnknownNet", "ConvergenceError"):
    assert hasattr(pip, name), name

assert issubclass(pip.ElaborationError, pip.SimulationError)
assert issubclass(pip.UnknownModule, pip.SimulationError)
assert issubclass(pip.UnknownNet, pip.SimulationError)
assert issubclass(pip.ConvergenceError, pip.SimulationError)

err = pip.ConvergenceError("boom", node="vout", iteration=42, analysis="op")
assert err.node == "vout"
assert err.iteration == 42
assert err.analysis == "op"
"#;
    let script_path = write_temp("piperine_t24_hierarchy.py", script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "hierarchy script must pass: {:?}", result.err());
    let _ = std::fs::remove_file(script_path);
}

/// `load()` on a bad source raises `ElaborationError`.
#[test]
fn load_failure_raises_elaboration_error() {
    let script = r#"
import piperine as pip

try:
    pip.load("/nonexistent/path/definitely_missing.phdl")
    raised = None
except pip.ElaborationError as e:
    raised = type(e).__name__
assert raised == "ElaborationError", raised
"#;
    let script_path = write_temp("piperine_t24_elab.py", script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "elaboration script must pass: {:?}", result.err());
    let _ = std::fs::remove_file(script_path);
}

/// `Design.module` on an unknown name raises `UnknownModule`.
#[test]
fn unknown_module_lookup_raises_unknown_module() {
    let phdl = write_temp("piperine_t24_rc.phdl", RC_PHDL);
    let script = format!(
        r#"
import piperine as pip

design = pip.load("{phdl}")
try:
    design.module("DoesNotExist")
    raised = None
except pip.UnknownModule as e:
    raised = type(e).__name__
assert raised == "UnknownModule", raised
"#,
        phdl = phdl,
    );
    let script_path = write_temp("piperine_t24_unknown_module.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "unknown module script must pass: {:?}", result.err());
    for p in [phdl, script_path] {
        let _ = std::fs::remove_file(p);
    }
}

/// A sensitivity analysis referencing an unaddressable output net raises
/// `UnknownNet`.
#[test]
fn unaddressable_net_raises_unknown_net() {
    let phdl = write_temp("piperine_t24_rc_net.phdl", RC_PHDL);
    let script = format!(
        r#"
import piperine as pip

design = pip.load("{phdl}")
session = design.compile()
try:
    session.sens(["bogus_net"], [("r1", "r")])
    raised = None
except pip.UnknownNet as e:
    raised = type(e).__name__
assert raised == "UnknownNet", raised
"#,
        phdl = phdl,
    );
    let script_path = write_temp("piperine_t24_unknown_net.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "unknown net script must pass: {:?}", result.err());
    for p in [phdl, script_path] {
        let _ = std::fs::remove_file(p);
    }
}

/// A non-converging run (`Solver(max_iter=0)` — the Newton loop cannot
/// execute a single iteration) raises `ConvergenceError`, not a bare
/// `RuntimeError` — the literal spec assertion.
#[test]
fn non_converging_run_raises_convergence_error() {
    let phdl = write_temp("piperine_t24_rc_conv.phdl", RC_PHDL);
    let script = format!(
        r#"
import piperine as pip

design = pip.load("{phdl}")
module = design.top
solver = pip.Solver(max_iter=0)
raised = None
analysis = None
is_runtime_error = False
try:
    module.op(pip.OpConfig(solver=solver))
except pip.ConvergenceError as e:
    raised = type(e).__name__
    analysis = e.analysis
    is_runtime_error = isinstance(e, RuntimeError)
assert raised == "ConvergenceError", raised
assert is_runtime_error  # backward-compatible with generic RuntimeError catches
assert analysis == "op", analysis
"#,
        phdl = phdl,
    );
    let script_path = write_temp("piperine_t24_convergence.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "convergence script must pass: {:?}", result.err());
    for p in [phdl, script_path] {
        let _ = std::fs::remove_file(p);
    }
}
