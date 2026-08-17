//! T27 — `pip.extract` host helper (HOST-25). Spec-derived from tasks.md's
//! "Done when": returns the named-measurement dict; works over `Trace`/
//! `Waveform`.

use piperine_python::embed::run_script;

fn write_temp(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, body).expect("write temp file");
    path.to_str().expect("non-utf8 temp path").to_string()
}

const RC_PHDL: &str = r#"
nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
nature Current; units = "A"; access = I; abstol = 1e-12; endnature
discipline electrical;
    potential Voltage;
    flow Current;
enddiscipline

module V (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog V(p, n) <+ dc;
endmodule

module R (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog I(p, n) <+ V(p, n) / r;
endmodule

module C (inout electrical p, inout electrical n);
    parameter real c = 1e-6;
    analog I(p, n) <+ c * ddt(V(p, n));
endmodule

module Top ();
    electrical gnd, vin, out;
    V v1 (.p(vin), .n(gnd));
    R #(.r(1e3)) r1 (.p(vin), .n(out));
    C #(.c(1e-6)) c1 (.p(out), .n(gnd));
endmodule
"#;

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
