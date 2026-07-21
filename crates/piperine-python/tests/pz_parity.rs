//! PZ-07 — uniform `.pz` shape (MD-22): the Python `module.pz(...)` returns
//! the same poles/zeros as the Rust `SimSession::run_pz` on the same series
//! RLC design.

use piperine_python::embed::run_script;

const RLC_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod V(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
analog V { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod L(inout p: Electrical, inout n: Electrical) { param l: Real = 1e-3; }
analog L { V(p, n) <- l * ddt(I(p, n)); }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-6; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire a   : Electrical;
    wire b   : Electrical;
    v1 : V(.p = vin, .n = gnd) {};
    r1 : R(.p = vin, .n = a) { .r = 10.0 };
    l1 : L(.p = a, .n = b) { .l = 1e-3 };
    c1 : C(.p = b, .n = gnd) { .c = 1e-6 };
}
";

#[test]
fn python_pz_matches_rust_pz() {
    let dir = std::env::temp_dir();
    let phdl = dir.join("piperine_pz_parity.phdl");
    std::fs::write(&phdl, RLC_PHDL).expect("write phdl");
    let out_txt = dir.join("piperine_pz_parity.txt");

    // Rust side.
    let design =
        piperine_lang::parse_and_elaborate(RLC_PHDL, &piperine_lang::SourceMap::dummy())
            .expect("RLC elaborates");
    let session = piperine_api::SimSession::new(design, "Top".to_string());
    let rust = session
        .run_pz("v1", "b", None, &piperine_api::SolverConfig::default())
        .expect("rust pz");
    let mut rust_poles = rust.poles.clone();
    rust_poles.sort_by(|a, b| a.im.partial_cmp(&b.im).unwrap());

    // Python side — same design, same call shape.
    let script = format!(
        r#"
import piperine
design = piperine.load({phdl:?})
r = design.module("Top").pz("v1", "b")
poles = sorted(r.poles, key=lambda c: c.imag)
with open({out:?}, "w") as f:
    f.write(f"{{len(poles)}}\n{{len(r.zeros)}}\n")
    for p in poles:
        f.write(f"{{p.real!r}} {{p.imag!r}}\n")
"#,
        phdl = phdl.to_str().unwrap(),
        out = out_txt.to_str().unwrap(),
    );
    let script_path = dir.join("piperine_pz_parity.py");
    std::fs::write(&script_path, script).expect("write script");
    run_script(script_path.to_str().unwrap()).expect("python pz runs");

    let text = std::fs::read_to_string(&out_txt).expect("python output");
    let mut lines = text.lines();
    let py_pole_count: usize = lines.next().unwrap().parse().expect("py pole count");
    let py_zero_count: usize = lines.next().unwrap().parse().expect("py zero count");

    assert_eq!(py_pole_count, rust_poles.len(), "pole count parity");
    assert_eq!(py_zero_count, rust.zeros.len(), "zero count parity");

    for rust_pole in &rust_poles {
        let line = lines.next().expect("py pole line");
        let mut parts = line.split_whitespace();
        let py_re: f64 = parts.next().unwrap().parse().expect("py re");
        let py_im: f64 = parts.next().unwrap().parse().expect("py im");
        assert!((py_re - rust_pole.re).abs() < 1e-9, "Re parity: py={py_re} rust={}", rust_pole.re);
        assert!((py_im - rust_pole.im).abs() < 1e-9, "Im parity: py={py_im} rust={}", rust_pole.im);
    }

    // And both sit on the analytic series-RLC pole pair.
    let (r, l, c) = (10.0_f64, 1e-3_f64, 1e-6_f64);
    let sigma = -r / (2.0 * l);
    let omega_d = (1.0 / (l * c) - (r / (2.0 * l)).powi(2)).sqrt();
    assert!((rust_poles[0].re - sigma).abs() / sigma.abs() < 1e-6);
    assert!((rust_poles[0].im + omega_d).abs() / omega_d < 1e-6);
    assert!((rust_poles[1].im - omega_d).abs() / omega_d < 1e-6);
}
