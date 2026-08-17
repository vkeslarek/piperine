//! FOUR-05 — uniform `.four` shape (MD-22): the Python
//! `waveform.fourier(f0, n_harmonics)` returns the same values as the Rust
//! `Waveform::fourier` on the same synthesized transient signal.

use piperine_python::embed::run_script;

const RC_TRANSIENT: &str = r#"
nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
nature Current; units = "A"; access = I; abstol = 1e-12; endnature
discipline electrical;
    potential Voltage;
    flow Current;
enddiscipline

module Vsine (inout electrical p, inout electrical n);
    parameter real amp = 1.0;
    parameter real f0 = 1000.0;
    analog V(p, n) <+ amp * sin(6.283185307179586 * f0 * $abstime) + 0.1 * amp * sin(6.283185307179586 * 3.0 * f0 * $abstime);
endmodule

module R (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog I(p, n) <+ V(p, n) / r;
endmodule

module Top ();
    electrical gnd, out;
    Vsine v1 (.p(out), .n(gnd));
    R r1 (.p(out), .n(gnd));
endmodule
"#;

#[test]
fn python_fourier_matches_rust_fourier() {
    let dir = std::env::temp_dir();
    let phdl = dir.join("piperine_four_parity.phdl");
    std::fs::write(&phdl, RC_TRANSIENT).expect("write phdl");
    let out_txt = dir.join("piperine_four_parity.txt");

    let f0 = 1000.0_f64;
    let n_harmonics = 5;
    let stop = 6.0 / f0; // 6 fundamental periods

    // Rust side.
    let design =
        piperine_lang::parse_and_elaborate(RC_TRANSIENT, &piperine_lang::SourceMap::dummy())
            .expect("Top elaborates");
    let mut session = piperine_api::Session::compile(&design, "Top").expect("session compiles");
    let trace = session
        .tran(stop, None, 0.0, &piperine_api::SolverConfig::default(), None, false, &[])
        .expect("rust transient");
    let wf = trace.v("out").expect("v(out)");
    let rust = wf.fourier(f0, n_harmonics).expect("rust fourier");
    let rust_thd = rust.thd;
    let rust_hd3 = rust.harmonics[3].norm_magnitude;
    let rust_mag1 = rust.harmonics[1].magnitude;

    // Python side — same design, same call shape.
    let script = format!(
        r#"
import piperine
design = piperine.load({phdl:?})
r = design.module("Top").tran(piperine.TranConfig(stop={stop}))
wf = r.v("out")
four = wf.fourier({f0}, {n_harmonics})
with open({out:?}, "w") as f:
    f.write(f"{{four.thd!r}}\n{{four.harmonics[3].norm_magnitude!r}}\n{{four.harmonics[1].magnitude!r}}\n")
"#,
        phdl = phdl.to_str().unwrap(),
        stop = stop,
        f0 = f0,
        n_harmonics = n_harmonics,
        out = out_txt.to_str().unwrap(),
    );
    let script_path = dir.join("piperine_four_parity.py");
    std::fs::write(&script_path, script).expect("write script");
    run_script(script_path.to_str().unwrap()).expect("python fourier runs");

    let text = std::fs::read_to_string(&out_txt).expect("python output");
    let mut lines = text.lines();
    let py_thd: f64 = lines.next().unwrap().parse().expect("py thd");
    let py_hd3: f64 = lines.next().unwrap().parse().expect("py hd3");
    let py_mag1: f64 = lines.next().unwrap().parse().expect("py mag1");

    // Same engine, same design -> identical values (uniform shape, MD-22).
    assert!((py_thd - rust_thd).abs() < 1e-9, "thd parity: py={py_thd} rust={rust_thd}");
    assert!((py_hd3 - rust_hd3).abs() < 1e-9, "hd3 parity: py={py_hd3} rust={rust_hd3}");
    assert!((py_mag1 - rust_mag1).abs() < 1e-9, "mag1 parity: py={py_mag1} rust={rust_mag1}");
}
