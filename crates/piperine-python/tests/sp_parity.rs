//! SP-06 — uniform `.sp` shape (MD-22): the Python `module.sp(...)` returns
//! the same S matrix as the Rust `SimSession::run_sp` on the same shunt-C
//! low-pass design (ports declared via `@rfport(num, z0)`).

use piperine_python::embed::run_script;

const SHUNT_C_LOWPASS_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1.0; }
analog R { I(p, n) <+ V(p, n) / r; }

mod C(inout p: Electrical, inout n: Electrical) { param c: Real = 1e-9; }
analog C { I(p, n) <+ c * ddt(V(p, n)); }

mod Top() {
    wire gnd : Electrical;
    @rfport(num = 1, z0 = 50) wire p1 : Electrical;
    @rfport(num = 2, z0 = 50) wire p2 : Electrical;
    rs  : R(.p = p1, .n = p2) { .r = 1.0 };
    c1  : C(.p = p2, .n = gnd) { .c = 1e-9 };
    rb1 : R(.p = p1, .n = gnd) { .r = 1e9 };
    rb2 : R(.p = p2, .n = gnd) { .r = 1e9 };
}
";

#[test]
fn python_sp_matches_rust_sp() {
    let dir = std::env::temp_dir();
    let phdl = dir.join("piperine_sp_parity.phdl");
    std::fs::write(&phdl, SHUNT_C_LOWPASS_PHDL).expect("write phdl");
    let out_txt = dir.join("piperine_sp_parity.txt");

    // Rust side.
    let design = piperine_lang::parse_and_elaborate(SHUNT_C_LOWPASS_PHDL, &piperine_lang::SourceMap::dummy())
        .expect("shunt-C low-pass elaborates");
    let session = piperine_api::SimSession::new(design, "Top".to_string());
    let rust = session
        .run_sp(1e3, 1e9, 5, true, &piperine_api::SolverConfig::default())
        .expect("rust sp");

    // Python side — same design, same call shape.
    let script = format!(
        r#"
import piperine
design = piperine.load({phdl:?})
r = design.module("Top").sp(1e3, 1e9, 5, True)
with open({out:?}, "w") as f:
    f.write(f"{{len(r.frequencies)}} {{r.n_ports}}\n")
    for freq, mat, z0 in zip(r.frequencies, r.s, r.z0):
        f.write(f"{{freq!r}} {{z0!r}}\n")
    for mat in r.s:
        for row in mat:
            for val in row:
                f.write(f"{{val.real!r}} {{val.imag!r}}\n")
"#,
        phdl = phdl.to_str().unwrap(),
        out = out_txt.to_str().unwrap(),
    );
    let script_path = dir.join("piperine_sp_parity.py");
    std::fs::write(&script_path, script).expect("write script");
    run_script(script_path.to_str().unwrap()).expect("python sp runs");

    let text = std::fs::read_to_string(&out_txt).expect("python output");
    let mut lines = text.lines();

    let header = lines.next().expect("header line");
    let mut header_parts = header.split_whitespace();
    let py_freq_count: usize = header_parts.next().unwrap().parse().expect("py freq count");
    let py_n_ports: usize = header_parts.next().unwrap().parse().expect("py n_ports");
    assert_eq!(py_freq_count, rust.frequencies.len(), "frequency count parity");
    assert_eq!(py_n_ports, rust.n_ports, "n_ports parity");

    for (rust_f, rust_z0) in rust.frequencies.iter().zip(rust.z0.iter()) {
        let line = lines.next().expect("py freq/z0 line");
        let mut parts = line.split_whitespace();
        let py_f: f64 = parts.next().unwrap().parse().expect("py freq");
        let py_z0: f64 = parts.next().unwrap().parse().expect("py z0");
        assert!((py_f - rust_f).abs() < 1e-6, "freq parity: py={py_f} rust={rust_f}");
        assert!((py_z0 - rust_z0).abs() < 1e-9, "z0 parity: py={py_z0} rust={rust_z0}");
    }

    for (k, mat) in rust.s.iter().enumerate() {
        for i in 0..rust.n_ports {
            for j in 0..rust.n_ports {
                let line = lines.next().unwrap_or_else(|| panic!("py S entry [{k}][{i}][{j}]"));
                let mut parts = line.split_whitespace();
                let py_re: f64 = parts.next().unwrap().parse().expect("py re");
                let py_im: f64 = parts.next().unwrap().parse().expect("py im");
                let rust_val = mat[[i, j]];
                assert!(
                    (py_re - rust_val.re).abs() < 1e-9,
                    "S[{k}][{i}][{j}].re parity: py={py_re} rust={}",
                    rust_val.re
                );
                assert!(
                    (py_im - rust_val.im).abs() < 1e-9,
                    "S[{k}][{i}][{j}].im parity: py={py_im} rust={}",
                    rust_val.im
                );
            }
        }
    }
}
