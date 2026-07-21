//! DISTO-06 — uniform `.disto` shape (MD-22): the Python
//! `module.disto(...)` returns the same HD2/HD3 as the Rust
//! `SimSession::run_disto` on the same cubic VCCS design.

use piperine_python::embed::run_script;

const POLY_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 0.0; param acmag: Real = 0.0; }
analog V { V(p, n) <+ dc + ac_stim(acmag, 0.0); }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 50.0; }
analog R { I(p, n) <+ V(p, n) / r; }

mod PolyVccs(inout inp: Electrical, inout inn: Electrical,
             inout outp: Electrical, inout outn: Electrical) {
    param g1: Real = 0.1;
    param g2: Real = 0.02;
    param g3: Real = 0.003;
}
analog PolyVccs {
    I(outp, outn) <+ g1 * V(inp, inn)
                   + g2 * V(inp, inn) * V(inp, inn)
                   + g3 * V(inp, inn) * V(inp, inn) * V(inp, inn);
}

mod Top() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire vout : Electrical;
    v1 : V(.p = vin, .n = gnd) { .dc = 0.0, .acmag = 1.0 };
    n1 : PolyVccs(.inp = vin, .inn = gnd, .outp = vout, .outn = gnd) {};
    r1 : R(.p = vout, .n = gnd) { .r = 50.0 };
}
";

#[test]
fn python_disto_matches_rust_disto() {
    let dir = std::env::temp_dir();
    let phdl = dir.join("piperine_disto_parity.phdl");
    std::fs::write(&phdl, POLY_PHDL).expect("write phdl");
    let out_txt = dir.join("piperine_disto_parity.txt");

    // Rust side.
    let design =
        piperine_lang::parse_and_elaborate(POLY_PHDL, &piperine_lang::SourceMap::dummy())
            .expect("poly stage elaborates");
    let session = piperine_api::SimSession::new(design, "Top".to_string());
    let rust = session
        .run_disto(1e6, None, 0.1, "vout", None, &piperine_api::SolverConfig::default())
        .expect("rust disto");

    // Python side — same design, same call shape.
    let script = format!(
        r#"
import piperine
design = piperine.load({phdl:?})
r = design.module("Top").disto(1e6, 0.1, "vout")
with open({out:?}, "w") as f:
    f.write(f"{{r.hd2:.18e}}\n{{r.hd3:.18e}}\n{{r.im2}}\n{{r.im3}}\n")
"#,
        phdl = phdl.to_str().unwrap(),
        out = out_txt.to_str().unwrap(),
    );
    let script_path = dir.join("piperine_disto_parity.py");
    std::fs::write(&script_path, script).expect("write script");
    run_script(script_path.to_str().unwrap()).expect("python disto runs");

    let text = std::fs::read_to_string(&out_txt).expect("read parity output");
    let lines: Vec<&str> = text.lines().collect();
    let py_hd2: f64 = lines[0].parse().expect("hd2 float");
    let py_hd3: f64 = lines[1].parse().expect("hd3 float");
    assert_eq!(lines[2], "None", "single-tone reports no IM2");
    assert_eq!(lines[3], "None", "single-tone reports no IM3");

    let rust_hd2 = rust.hd2.expect("rust HD2");
    let rust_hd3 = rust.hd3.expect("rust HD3");
    assert!(
        (py_hd2 - rust_hd2).abs() <= 1e-9,
        "HD2 parity: python {py_hd2} vs rust {rust_hd2}"
    );
    assert!(
        (py_hd3 - rust_hd3).abs() <= 1e-9,
        "HD3 parity: python {py_hd3} vs rust {rust_hd3}"
    );
}
