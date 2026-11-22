//! The four analyses added by host-ii on the command line (HII-08): `piperine run` prints the
//! header with the analysis named `tf`, `pz`, `sens` or `pss`, the table, and with `--csv` the
//! file, from a project whose runs are written with the new keys (`input`, `output`, `wrt`,
//! `period`, `tstab`). Every child gets one cache directory shared by this target.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_piperine"))
}

fn shared_cache() -> &'static Path {
    static CACHE: OnceLock<tempfile::TempDir> = OnceLock::new();
    CACHE.get_or_init(|| tempfile::tempdir().expect("tmp")).path()
}

fn piperine_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(binary()).args(args).current_dir(dir).env("PIPERINE_CACHE_DIR", shared_cache()).env_remove("PIPERINE_BACKEND").output().unwrap_or_else(|e| panic!("cannot run {}: {e}", binary().display()))
}

/// The parts both benches share: a resistor, a capacitor and a voltage source.
const PARTS: &str = "pub module Resistor(p: Electrical, n: Electrical) { r: f64 where r > 0.0; }\nanalog for Resistor { i(p, n) +> v(p, n) / r; }\npub module Capacitor(p: Electrical, n: Electrical) { c: f64 where c >= 0.0; }\nanalog for Capacitor { i(p, n) +> ddt(c * v(p, n)); }\npub module Vsource(p: Electrical, n: Electrical) { dc: f64 = 1.0; }\nanalog for Vsource { v(p, n) +> dc; }\n";

/// A 10 V source over 1 kOhm, then 3 kOhm in parallel with 1 uF: gain 3/4, one pole.
fn lowpass() -> String {
    format!("{PARTS}module Top() {{\n    signal vin: Electrical;\n    signal vout: Electrical;\n    let v1 = Vsource(vin, gnd) {{ dc: 10.0 }};\n    let r1 = Resistor(vin, vout) {{ r: 1k }};\n    let r2 = Resistor(vout, gnd) {{ r: 3k }};\n    let c1 = Capacitor(vout, gnd) {{ c: 1u }};\n}}\n")
}

/// The square-wave RC of `tests/e2e/twins/pss_rc.msl`, period 4 us.
fn pulsed() -> String {
    format!("{PARTS}module Pulse(p: Electrical) {{ amp: f64 = 1.0; width: f64 = 2u; period: f64 = 4u; edge: f64 = 1p; }}\nanalog for Pulse {{\n    let ph = now() - period * (now() / period).floor();\n    let level = if ph < edge {{ ph / edge }} else if ph < width {{ 1.0 }} else if ph < width + edge {{ 1.0 - (ph - width) / edge }} else {{ 0.0 }};\n    v(p) +> amp * level;\n}}\nmodule Steady() {{\n    signal vin: Electrical;\n    signal vout: Electrical;\n    let src = Pulse(vin);\n    let r1 = Resistor(vin, vout) {{ r: 1k }};\n    let c1 = Capacitor(vout, gnd) {{ c: 1n }};\n}}\n")
}

const MANIFEST: &str = r#"[package]
name = "small"
version = "0.1.0"

[run.tf]
bench = "benches/lowpass.msl"
analysis = "tf"
input = "v1"
output = "vout"

[run.pz]
bench = "benches/lowpass.msl"
analysis = "pz"
input = "v1"
output = "vout"

[run.sens]
bench = "benches/lowpass.msl"
analysis = "sens"
wrt = ["r1.r", "r2.r"]
print = ["vout"]

[run.pss]
bench = "benches/pulsed.msl"
analysis = "pss"
period = 4e-6
tstab = 8e-6
max_step = 1e-9
print_step = 1e-6
print = ["vout"]
"#;

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::create_dir_all(dir.path().join("benches")).expect("mkdir");
    std::fs::create_dir_all(dir.path().join("src")).expect("mkdir");
    std::fs::write(dir.path().join("src/lib.msl"), "").expect("crate root");
    std::fs::write(dir.path().join("Piperine.toml"), MANIFEST).expect("manifest");
    std::fs::write(dir.path().join("benches/lowpass.msl"), lowpass()).expect("bench");
    std::fs::write(dir.path().join("benches/pulsed.msl"), pulsed()).expect("bench");
    dir
}

fn stdout_of(out: &Output) -> String {
    assert_eq!(out.status.code(), Some(0), "stderr:\n{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn cells(line: &str) -> Vec<&str> {
    line.split_whitespace().collect()
}

/// For each analysis: the run, the header's top, the first line of its table and its row count.
const RUNS: [(&str, &str, &[&str], usize); 4] = [
    ("tf", "Top", &["quantity", "value"], 3),
    ("pz", "Top", &["kind", "real", "imag"], 1),
    ("sens", "Top", &["param", "d(Top.vout)"], 2),
    ("pss", "Steady", &["time", "Steady.vout"], 5),
];

#[test]
fn each_new_analysis_prints_header_records_and_table() {
    // HII-08 AC 1: the header names the analysis, an empty line, then the table
    let dir = project();
    for (run, top, columns, rows) in RUNS {
        let out = stdout_of(&piperine_in(dir.path(), &["run", run]));
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], format!("run {run}: {run}, top {top}, backend rust"), "{out}");
        let blank = lines.iter().position(|l| l.is_empty()).unwrap_or_else(|| panic!("no empty line in\n{out}"));
        assert!(lines[1..blank].iter().all(|l| l.starts_with('@')), "between the header and the empty line only records:\n{out}");
        assert_eq!(cells(lines[blank + 1]), columns, "{out}");
        assert_eq!(lines.len() - blank - 2, rows, "{run}: {out}");
    }
    // the values the analyses give: gain 3/4 and the pole at -(1/R1 + 1/R2)/C
    let tf = stdout_of(&piperine_in(dir.path(), &["run", "tf"]));
    let gain: f64 = tf.lines().find(|l| l.starts_with("gain")).and_then(|l| cells(l).get(1).and_then(|v| v.parse().ok())).unwrap_or_else(|| panic!("no gain in\n{tf}"));
    assert!((gain - 0.75).abs() < 1e-6, "{gain}");
}

#[test]
fn each_new_analysis_writes_csv() {
    // HII-08 AC 2: `--csv` writes the table as a file and says so, as for the other analyses
    let dir = project();
    for (run, _, columns, rows) in RUNS {
        let out = stdout_of(&piperine_in(dir.path(), &["run", run, "--csv", "out.csv"]));
        assert!(out.lines().any(|l| l == format!("wrote out.csv ({rows} rows)")), "{run}: {out}");
        let csv = std::fs::read_to_string(dir.path().join("out.csv")).expect("the csv");
        assert_eq!(csv.lines().next().expect("a header"), columns.join(","), "{run}");
        assert_eq!(csv.lines().count(), rows + 1, "{run}");
    }
}

#[test]
fn the_new_keys_have_flags() {
    // HII-08 AC 4: a flag gives the key a table gives, on a lone file too
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(dir.path().join("lp.msl"), lowpass()).expect("bench");
    let out = stdout_of(&piperine_in(dir.path(), &["run", "lp.msl", "--analysis", "tf", "--input", "v1", "--output", "vout"]));
    assert!(out.starts_with("run lp: tf, top Top, backend rust"), "{out}");
    let out = stdout_of(&piperine_in(dir.path(), &["run", "lp.msl", "--analysis", "sens", "--wrt", "r1.r", "--print", "vout"]));
    assert!(out.starts_with("run lp: sens, top Top"), "{out}");
}

#[test]
fn a_noise_run_prints_one_row_per_frequency_then_the_total() {
    // AO-05 AC 1 on the command line: `--analysis noise` prints the header, the table `freq`,
    // `onoise` with one row per frequency (1 to 1e3 at 1 point per decade: round(1 * 3) + 1 = 4),
    // then the summary line `onoise_total  <value>`; the resistors and capacitor of the lowpass
    // have no noise source, so the density and the total are 0. CSV writes the rows only.
    let dir = tempfile::tempdir().expect("tmp");
    std::fs::write(dir.path().join("lp.msl"), lowpass()).expect("bench");
    let args = ["run", "lp.msl", "--analysis", "noise", "--output", "vout", "--from", "1", "--to", "1e3", "--per-decade", "1"];
    let out = stdout_of(&piperine_in(dir.path(), &args));
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "run lp: noise, top Top, backend rust", "{out}");
    let blank = lines.iter().position(|l| l.is_empty()).unwrap_or_else(|| panic!("no empty line in\n{out}"));
    assert_eq!(cells(lines[blank + 1]), ["freq", "onoise"], "{out}");
    assert_eq!(lines.len() - blank - 2, 4 + 1, "four rows and the total: {out}");
    let total = cells(lines[lines.len() - 1]);
    assert_eq!(total[0], "onoise_total", "{out}");
    assert_eq!(total[1].parse::<f64>().expect("a number"), 0.0, "{out}");
    let mut with_csv = args.to_vec();
    with_csv.extend(["--csv", "noise.csv"]);
    let out = stdout_of(&piperine_in(dir.path(), &with_csv));
    assert!(out.lines().any(|l| l == "wrote noise.csv (4 rows)"), "{out}");
    assert!(!out.contains("onoise_total"), "CSV writes the rows only: {out}");
    let csv = std::fs::read_to_string(dir.path().join("noise.csv")).expect("the csv");
    assert_eq!(csv.lines().next(), Some("freq,onoise"));
    assert_eq!(csv.lines().count(), 5);
}
