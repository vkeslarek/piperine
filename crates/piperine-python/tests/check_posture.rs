//! The Python `Solver` check posture (DVC-12, MD-22): `checks` defaults to
//! `"strict"`, `require_scope` to `"every_analysis"`; every posture reaches
//! the analysis through the `SolverConfig` mapping; an unknown spelling
//! fails loud naming the valid values — never a silent default.

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

/// DVC-12 / D2: the Python `Solver` dataclass defaults are `"strict"` and
/// `"every_analysis"` — the same defaults the Rust `SolverConfig`/`Context`
/// carry (MD-22).
#[test]
fn solver_posture_fields_default_to_strict_and_every_analysis() {
    let script = r#"
import piperine

s = piperine.Solver()
assert s.checks == "strict", s.checks
assert s.require_scope == "every_analysis", s.require_scope

derived = s.with_(checks="collect")
assert s.checks == "strict"  # original untouched
assert derived.checks == "collect"
assert derived.require_scope == "every_analysis"
"#;
    let script_path = write_temp("piperine_dvc12_defaults.py", script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "posture defaults script must pass: {:?}", result.err());
    let _ = std::fs::remove_file(script_path);
}

/// DVC-12: all three postures and the scope value reach the analysis
/// through the `Solver` → `SolverConfig` mapping (not silently ignored).
#[test]
fn every_posture_reaches_the_analysis() {
    let phdl = write_temp("piperine_dvc12_rc.phdl", RC_PHDL);
    let script = format!(
        r#"
import piperine

design = piperine.load("{phdl}")
module = design.top

for posture in ("strict", "collect", "off"):
    solver = piperine.Solver(checks=posture, require_scope="every_analysis")
    op = module.op(piperine.OpConfig(solver=solver))
    assert abs(op.v("a") - 1.0) < 1e-9, (posture, op.v("a"))
"#,
        phdl = phdl,
    );
    let script_path = write_temp("piperine_dvc12_postures.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "postures script must pass: {:?}", result.err());
    for p in [phdl, script_path] {
        let _ = std::fs::remove_file(p);
    }
}

/// DVC-12 / fail-loud: an unknown `checks` or `require_scope` spelling is
/// rejected naming the bad value and the valid candidates — never coerced
/// to a silent default.
#[test]
fn unknown_posture_spellings_fail_loud() {
    let phdl = write_temp("piperine_dvc12_rc_bad.phdl", RC_PHDL);
    let script = format!(
        r#"
import piperine

design = piperine.load("{phdl}")
module = design.top

try:
    module.op(piperine.OpConfig(solver=piperine.Solver(checks="lenient")))
    raise AssertionError("unknown checks posture accepted")
except ValueError as e:
    msg = str(e)
    assert "lenient" in msg, msg
    assert "strict" in msg and "collect" in msg and "off" in msg, msg

try:
    module.op(piperine.OpConfig(solver=piperine.Solver(require_scope="dc_only")))
    raise AssertionError("unknown require_scope accepted")
except ValueError as e:
    msg = str(e)
    assert "dc_only" in msg, msg
    assert "every_analysis" in msg, msg
"#,
        phdl = phdl,
    );
    let script_path = write_temp("piperine_dvc12_loud.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "fail-loud script must pass: {:?}", result.err());
    for p in [phdl, script_path] {
        let _ = std::fs::remove_file(p);
    }
}
