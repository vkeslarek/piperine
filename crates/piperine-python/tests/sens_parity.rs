//! SC-03 — uniform `.sens` shape (MD-22): the Python `module.sens(...)`
//! returns the same values as the Rust `SimSession::run_sens` on the same
//! design, keyed the same way (`(output, "label.param")`).

use piperine_python::embed::run_script;

const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VoltageSource(inout p: Electrical, inout n: Electrical) {
    param dc: Real = 10.0;
}
analog VoltageSource { V(p, n) <- dc; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}
analog Resistor { I(p, n) <+ V(p, n) / r; }

mod Divider() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire mid : Electrical;
    v1 : VoltageSource (.p = vin, .n = gnd) {};
    r1 : Resistor      (.p = vin, .n = mid) {};
    r2 : Resistor      (.p = mid, .n = gnd) {};
}
";

#[test]
fn python_sens_matches_rust_sens() {
    let dir = std::env::temp_dir();
    let phdl = dir.join("piperine_sens_parity.phdl");
    std::fs::write(&phdl, DIVIDER_PHDL).expect("write phdl");
    let out_json = dir.join("piperine_sens_parity.txt");

    // Rust side.
    let design = piperine_lang::parse_and_elaborate(
        DIVIDER_PHDL,
        &piperine_lang::SourceMap::dummy(),
    )
    .expect("divider elaborates");
    let session = piperine_api::SimSession::new(design, "Divider".to_string());
    let rust = session
        .run_sens(
            &["mid"],
            &[("r2".to_string(), "r".to_string()), ("v1".to_string(), "dc".to_string())],
            1.0e-6,
            &piperine_api::SolverConfig::default(),
        )
        .expect("rust sens");
    let rust_r2 = rust.get("mid", "r2", "r").expect("rust d/dr2");
    let rust_v1 = rust.get("mid", "v1", "dc").expect("rust d/dv1");

    // Python side — same design, same call shape, values dumped to a file.
    let script = format!(
        r#"
import piperine
design = piperine.load({phdl:?})
m = design.module("Divider")
s = m.sens(["mid"], [("r2", "r"), ("v1", "dc")])
with open({out:?}, "w") as f:
    f.write(f"{{s.get('mid', 'r2', 'r')!r}}\n{{s.get('mid', 'v1', 'dc')!r}}\n")
"#,
        phdl = phdl.to_str().unwrap(),
        out = out_json.to_str().unwrap(),
    );
    let script_path = dir.join("piperine_sens_parity.py");
    std::fs::write(&script_path, script).expect("write script");
    run_script(script_path.to_str().unwrap()).expect("python sens runs");

    let text = std::fs::read_to_string(&out_json).expect("python output");
    let mut lines = text.lines();
    let py_r2: f64 = lines.next().unwrap().parse().expect("py d/dr2");
    let py_v1: f64 = lines.next().unwrap().parse().expect("py d/dv1");

    // Same engine, same design → identical values (uniform shape, MD-22).
    assert_eq!(py_r2, rust_r2, "d v(mid)/d r2.r parity");
    assert_eq!(py_v1, rust_v1, "d v(mid)/d v1.dc parity");
    // And both sit on the analytic value.
    let analytic = 10.0 * 1.0e3 / (2.0e3_f64).powi(2);
    assert!(((rust_r2 - analytic) / analytic).abs() < 1.0e-6, "analytic anchor: {rust_r2}");
}

/// SC-06 — uniform PSS shape: python `module.pss` returns the same orbit
/// stats and waveform values as the Rust `run_pss` on the same design.
#[test]
fn python_pss_matches_rust_pss() {
    let dir = std::env::temp_dir();
    const SINE_RC: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }
mod Vsine(inout p: Electrical, inout n: Electrical) { param amp: Real = 5.0; }
analog Vsine { V(p, n) <- amp * sin(6283.185307179586 * $abstime); }
mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }
mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-6; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }
mod Top() {
    wire gnd : Electrical;
    wire top : Electrical;
    wire out : Electrical;
    v1 : Vsine(.p = top, .n = gnd) {};
    r1 : R(.p = top, .n = out) {};
    c1 : C(.p = out, .n = gnd) {};
}
";
    let phdl = dir.join("piperine_pss_parity.phdl");
    std::fs::write(&phdl, SINE_RC).expect("write phdl");
    let out_txt = dir.join("piperine_pss_parity.txt");

    let design =
        piperine_lang::parse_and_elaborate(SINE_RC, &piperine_lang::SourceMap::dummy())
            .expect("elaborates");
    let session = piperine_api::SimSession::new(design, "Top".to_string());
    let rust = session
        .run_pss(1.0e-3, 0.0, &piperine_api::SolverConfig::default())
        .expect("rust pss");
    let rust_max = rust
        .trace
        .v(&piperine_api::NetRef { name: "out".into() }, None)
        .expect("v(out)")
        .max();

    let script = format!(
        r#"
import piperine
design = piperine.load({phdl:?})
r = design.module("Top").pss(period=1e-3)
with open({out:?}, "w") as f:
    f.write(f"{{float(r.trace.v('out').max())!r}}\n{{r.stats.shoot_iterations}}\n")
"#,
        phdl = phdl.to_str().unwrap(),
        out = out_txt.to_str().unwrap(),
    );
    let script_path = dir.join("piperine_pss_parity.py");
    std::fs::write(&script_path, script).expect("write script");
    run_script(script_path.to_str().unwrap()).expect("python pss runs");

    let text = std::fs::read_to_string(&out_txt).expect("python output");
    let mut lines = text.lines();
    let py_max: f64 = lines.next().unwrap().parse().expect("py max");
    let py_iters: usize = lines.next().unwrap().parse().expect("py iters");
    assert_eq!(py_max, rust_max, "orbit max parity");
    assert_eq!(py_iters, rust.stats.shoot_iterations, "iterations parity");
}
