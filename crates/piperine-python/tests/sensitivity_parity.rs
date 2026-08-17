//! SC-04 — uniform sensitivity/gradient shape (dv-gradients T12, DVG-13,
//! MD-22): the Python `module.sensitivity(...)` and `module.gradient(...)`
//! return the same values AND the same engine attribution as the Rust
//! `Module::sensitivity`/`Module::gradient` on the same design.

use piperine_python::embed::run_script;

const DIVIDER_PHDL: &str = r#"
nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
nature Current; units = "A"; access = I; abstol = 1e-12; endnature
discipline electrical;
    potential Voltage;
    flow Current;
enddiscipline

module VoltageSource (inout electrical p, inout electrical n);
    parameter real dc = 10.0;
    analog V(p, n) <+ dc;
endmodule

module Resistor (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog I(p, n) <+ V(p, n) / r;
endmodule

module Divider ();
    electrical gnd, vin, mid;
    VoltageSource v1 (.p(vin), .n(gnd));
    Resistor r1 (.p(vin), .n(mid));
    Resistor r2 (.p(mid), .n(gnd));
endmodule
"#;

#[test]
fn python_sensitivity_matches_rust_sensitivity() {
    let dir = std::env::temp_dir();
    let phdl = dir.join("piperine_sens_value_parity.phdl");
    std::fs::write(&phdl, DIVIDER_PHDL).expect("write phdl");
    let out_json = dir.join("piperine_sens_value_parity.txt");

    // Rust side.
    let module = piperine_api::model::Design::load_str(DIVIDER_PHDL)
        .expect("divider elaborates")
        .module("Divider")
        .expect("module present");
    let rust_s = module
        .sensitivity("mid", &("r2".to_string(), "r".to_string()), None)
        .expect("rust sensitivity");
    let rust_g = module
        .gradient("mid", &[("r2".to_string(), "r".to_string()), ("v1".to_string(), "dc".to_string())], None)
        .expect("rust gradient");

    // Python side — same design, same call shape, values + engine written
    // to a file.
    let script = format!(
        r#"
import piperine
design = piperine.load({phdl:?})
m = design.module("Divider")
s = m.sensitivity("mid", ("r2", "r"))
g = m.gradient("mid", [("r2", "r"), ("v1", "dc")])
with open({out:?}, "w") as f:
    f.write(repr(s.value) + "\n")
    f.write(s.method + "\n")
    f.write(repr(sorted(s.fallback_users)) + "\n")
    f.write(repr(g.d["r2.r"]) + "\n")
    f.write(repr(g.d["v1.dc"]) + "\n")
    f.write(g.method + "\n")
    f.write(repr(sorted(g.fallback_users)) + "\n")
"#,
        phdl = phdl.to_str().unwrap(),
        out = out_json.to_str().unwrap(),
    );
    let script_path = dir.join("piperine_sens_value_parity.py");
    std::fs::write(&script_path, script).expect("write script");
    run_script(script_path.to_str().unwrap()).expect("python sensitivity runs");

    let text = std::fs::read_to_string(&out_json).expect("python output");
    let mut lines = text.lines();
    let py_s_value: f64 = lines.next().unwrap().parse().expect("py s.value");
    let py_s_method = lines.next().unwrap().to_string();
    let _py_s_fallback = lines.next().unwrap();
    let py_g_r2r: f64 = lines.next().unwrap().parse().expect("py g.r2.r");
    let py_g_v1dc: f64 = lines.next().unwrap().parse().expect("py g.v1.dc");
    let py_g_method = lines.next().unwrap().to_string();
    let _py_g_fallback = lines.next().unwrap();

    // Value parity — both hosts run the same analytic adjoint over the
    // same compiled circuit, so the bar is exact equality.
    assert!(
        (py_s_value - rust_s.value).abs() < 1.0e-12,
        "sensitivity value parity: rust {} vs python {}",
        rust_s.value,
        py_s_value
    );
    assert_eq!(
        py_s_method,
        rust_s.method.to_string(),
        "sensitivity engine attribution parity (MD-22)"
    );
    assert!(
        (py_g_r2r - rust_g.get("r2", "r").unwrap()).abs() < 1.0e-12,
        "gradient r2.r value parity"
    );
    assert!(
        (py_g_v1dc - rust_g.get("v1", "dc").unwrap()).abs() < 1.0e-12,
        "gradient v1.dc value parity"
    );
    assert_eq!(
        py_g_method,
        rust_g.method.to_string(),
        "gradient engine attribution parity (MD-22)"
    );

    let _ = std::fs::remove_file(&phdl);
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&out_json);
}

#[test]
fn python_sensitivity_unknown_metric_fails_loud() {
    let dir = std::env::temp_dir();
    let phdl = dir.join("piperine_sens_unknown.phdl");
    std::fs::write(&phdl, DIVIDER_PHDL).expect("write phdl");

    let script = format!(
        r#"
import piperine
design = piperine.load({phdl:?})
m = design.module("Divider")
try:
    m.sensitivity("nope", ("r2", "r"))
    raise SystemExit("expected RuntimeError")
except RuntimeError as e:
    assert "nope" in str(e), str(e)
    assert "mid" in str(e) or "vin" in str(e), "lists candidates: " + str(e)
"#,
        phdl = phdl.to_str().unwrap(),
    );
    let script_path = dir.join("piperine_sens_unknown.py");
    std::fs::write(&script_path, script).expect("write script");
    run_script(script_path.to_str().unwrap()).expect("python unknown-metric refusal runs");

    let _ = std::fs::remove_file(&phdl);
    let _ = std::fs::remove_file(&script_path);
}
