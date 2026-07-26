//! T27 — `pip.extract` host helper (HOST-25). Spec-derived from tasks.md's
//! "Done when": returns the named-measurement dict; works over `Trace`/
//! `Waveform`.

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

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-6; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire out : Electrical;
    v1 : V(.p = vin, .n = gnd) {};
    r1 : R(.p = vin, .n = out) { .r = 1e3 };
    c1 : C(.p = out, .n = gnd) { .c = 1e-6 };
}
";

/// `pip.extract(trace, {...})` returns a dict keyed by the measurement
/// names, each value the corresponding function applied to `trace`
/// (working over a `Trace`, reading `.v("out")` inside each measurement).
#[test]
fn extract_over_trace_returns_named_measurement_dict() {
    let phdl = write_temp("piperine_t27_rc.phdl", RC_PHDL);
    let script = format!(
        r#"
import piperine as pip

design = pip.load("{phdl}")
module = design.top
trace = module.tran(pip.TranConfig(stop=5e-3, step=1e-4, ic={{"out": 0.0}}))

m = pip.extract(trace, {{
    "peak": lambda tr: tr.v("out").max(),
    "cross_50pct": lambda tr: tr.v("out").cross(0.5, pip.CrossDirection.Rising),
}})
assert set(m.keys()) == {{"peak", "cross_50pct"}}, m.keys()
assert isinstance(m["peak"], float)
assert m["cross_50pct"] is not None
assert m["cross_50pct"] > 0.0
"#,
        phdl = phdl,
    );
    let script_path = write_temp("piperine_t27_trace.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "extract over trace script must pass: {:?}", result.err());
    for p in [phdl, script_path] {
        let _ = std::fs::remove_file(p);
    }
}

/// `pip.extract` also works directly over a bare `Waveform` (each
/// measurement function receives the waveform itself, not a `Trace`).
#[test]
fn extract_over_waveform_returns_named_measurement_dict() {
    let phdl = write_temp("piperine_t27_rc_wf.phdl", RC_PHDL);
    let script = format!(
        r#"
import piperine as pip

design = pip.load("{phdl}")
module = design.top
trace = module.tran(pip.TranConfig(stop=5e-3, step=1e-4, ic={{"out": 0.0}}))
wf = trace.v("out")

m = pip.extract(wf, {{
    "max": lambda w: w.max(),
    "min": lambda w: w.min(),
}})
assert m["max"] >= m["min"]
assert isinstance(m["max"], float)
"#,
        phdl = phdl,
    );
    let script_path = write_temp("piperine_t27_waveform.py", &script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "extract over waveform script must pass: {:?}", result.err());
    for p in [phdl, script_path] {
        let _ = std::fs::remove_file(p);
    }
}

/// An empty measurement dict returns an empty dict (edge case).
#[test]
fn extract_with_no_measurements_returns_empty_dict() {
    let script = r#"
import piperine as pip

m = pip.extract(object(), {})
assert m == {}
"#;
    let script_path = write_temp("piperine_t27_empty.py", script);
    let result = run_script(&script_path);
    assert!(result.is_ok(), "extract empty script must pass: {:?}", result.err());
    let _ = std::fs::remove_file(script_path);
}
