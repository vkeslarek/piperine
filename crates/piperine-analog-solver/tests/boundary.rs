//! The boundary of the analog crate (feature `solver-abi-analog`, SAN-01,
//! SAN-12, T10): its manifest names `piperine-abi` and no other workspace
//! crate, its device modules name no digital or owner item (C1) and hold no
//! `unsafe`, and the device assembly offers, as plain methods, everything an
//! adapter of the owner's `AnalogSide` needs (C22), driven here on an RC
//! with a DAC through the real Newton engine, with one rejected step.

#[allow(dead_code)]
#[path = "support/circuit.rs"]
mod circuit;
#[allow(dead_code)]
#[path = "support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "support/system.rs"]
mod system;

use std::path::{Path, PathBuf};

use circuit::{at, f64_bytes, instance, netlist};
use devices::modules;
use piperine_analog_solver::device::AnalogBinding::{Absent, Discrete, Ground};
use piperine_analog_solver::device::{AnalogPlan, Assembly, JumpPosition, Stage};
use piperine_analog_solver::math::integration::{TrBdf2, TrBdf2Phase};
use piperine_analog_solver::math::iv::InitialValue;
use piperine_analog_solver::netlist::Netlist;
use system::{Solver, Tran, at_rest, at_time, size, solve_dc, solve_stage};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The `piperine-*` dependency keys of a manifest.
fn workspace_dependencies(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| l.starts_with("piperine-") || l.starts_with("[dependencies.piperine-"))
        .map(|l| l.trim_start_matches("[dependencies.").split(['=', ' ', ']']).next().unwrap_or("").to_string())
        .collect()
}

#[test]
fn the_crate_depends_on_the_abi_crate_only() {
    let text = std::fs::read_to_string(crate_dir().join("Cargo.toml")).expect("read the manifest");
    assert_eq!(workspace_dependencies(&text), vec!["piperine-abi".to_string()]);
    let planted = "[dependencies]\npiperine-abi = { path = \"../piperine-abi\" }\npiperine-solver = { path = \"../x\" }\n";
    assert_eq!(workspace_dependencies(planted), vec!["piperine-abi".to_string(), "piperine-solver".to_string()]);
}

/// The Rust sources under `dir`, each with its code lines (comments dropped).
fn code_of(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).expect("read src/device") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).expect("read source");
            out.push((path, strip_comments(&text)));
        }
    }
    assert!(out.len() >= 8, "src/device holds its modules: {}", out.len());
    out
}

/// The text with `//` comments removed.
fn strip_comments(text: &str) -> String {
    text.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n")
}

/// The names of `names` that occur in `code`.
fn found(code: &str, names: &[String]) -> Vec<String> {
    names.iter().filter(|n| code.contains(n.as_str())).cloned().collect()
}

/// The digital and owner items the device modules must not name. Spelled in
/// pieces: the owner's own guard (`crate_boundaries.rs`) scans this crate's
/// files, tests included, for the digital type names.
fn foreign() -> Vec<String> {
    [["piperine_digital", "_solver"], ["piperine", "_solver"], ["Logic", "Value"], ["Digital", "Net"]]
        .iter()
        .map(|p| p.concat())
        .collect()
}

#[test]
fn device_modules_name_no_digital_or_owner_item() {
    let names = foreign();
    for (path, code) in code_of(&crate_dir().join("src/device")) {
        assert!(found(&code, &names).is_empty(), "{} names {:?}", path.display(), found(&code, &names));
    }
    let plant = format!("use {}::protocol; let v: {};", names[1], names[2]);
    assert_eq!(found(&plant, &names), vec![names[1].clone(), names[2].clone()], "the scan catches a plant");
}

#[test]
fn device_modules_hold_no_unsafe() {
    for (path, code) in code_of(&crate_dir().join("src/device")) {
        assert!(!code.contains("unsafe"), "{} holds unsafe code", path.display());
    }
    assert!(strip_comments("let x = unsafe { y };").contains("unsafe"), "the scan catches a plant");
    assert!(!strip_comments("// nothing here is unsafe").contains("unsafe"), "a comment is not code");
}

/// One TR-BDF2 step of `dt` from `t`: both stages, each recorded as a
/// history row (`assemble_tran` through the Newton loop, `end_iteration`
/// between iterations, `push_stage` after each stage).
fn step(solver: &mut Solver, asm: &mut Assembly, net: &Netlist, t: f64, dt: f64, prev_h: f64) -> Vec<f64> {
    let trap = Stage { phase: TrBdf2Phase::Trapezoidal, h: dt, prev_h, jump: JumpPosition::None };
    solve_stage(solver, asm, net, at_time(t + TrBdf2::GAMMA * dt), trap);
    let bdf2 = Stage { phase: TrBdf2Phase::Bdf2, h: dt, prev_h, jump: JumpPosition::None };
    solve_stage(solver, asm, net, at_time(t + dt), bdf2).to_vec()
}

/// An RC driven by a DAC whose code goes to full scale at t = 0, run over
/// one time constant in 50 steps; the step `reject` (if any) is solved,
/// rejected, restored and solved again. Returns the end time and v(rc).
fn rc_with_a_dac(reject: Option<usize>) -> (f64, f64) {
    let m = modules();
    let (net, n) = netlist(2, &["dac.f"]);
    let (out, rc, f) = (&n[0], &n[1], &n[2]);
    let (r, c) = (1.0e3, 1.0e-6);
    let dac = instance("dac", m.dac, vec![Discrete(0), Discrete(1), at(out), at(f)]);
    let r1 = instance("r1", m.resistor, vec![Discrete(2), at(out), at(rc)]);
    let c1 = instance("c1", m.capacitor, vec![Discrete(3), at(rc), Ground]);
    let sh = instance("sh", m.sample_hold, vec![at(rc), Absent, Discrete(4)]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![dac, r1, c1, sh], &net).expect("bind"));
    asm.set_discrete(0, &f64_bytes(1.0));
    asm.set_discrete(2, &f64_bytes(r));
    asm.set_discrete(3, &f64_bytes(c));

    // The operating point at code 0: accepted, and the history's first row.
    let op = solve_dc(&mut asm, &net, &vec![0.0; size(&net)]).to_vec();
    asm.accept(&op, at_rest()).expect("accept the operating point");
    asm.push_stage(&op, at_rest()).expect("the operating point's row");

    // A boundary event: the code goes to full scale.
    asm.set_discrete(1, &[255, 1]);
    let dt = r * c / 50.0;
    let first = Stage { phase: TrBdf2Phase::Trapezoidal, h: dt, prev_h: 0.0, jump: JumpPosition::None };
    let mut solver = Solver::new(&mut Tran { asm: &mut asm, net: &net, amb: at_time(0.0), stage: first }, size(&net), 4)
        .expect("dry run");
    let ivs = n.iter().zip(&op).map(|(r, v)| InitialValue { reference: r.clone(), value: *v }).collect();
    solver.push_initial_conditions(ivs);

    let (mut t, mut prev_h) = (0.0, 0.0);
    let mut x = op;
    for k in 0..50 {
        let (mark, rows) = (asm.snapshot(), solver.state_snapshot());
        x = step(&mut solver, &mut asm, &net, t, dt, prev_h);
        if reject == Some(k) {
            asm.restore(&mark);
            solver.restore_state(rows);
            assert_eq!(asm.snapshot(), mark, "restore returns every history and limit");
            x = step(&mut solver, &mut asm, &net, t, dt, prev_h);
        }
        let acc = asm.accept(&x, at_time(t + dt)).expect("accept");
        assert_eq!((acc.fired.len(), acc.cap), (0, f64::INFINITY), "no CROSS or CAP watch");
        t += dt;
        prev_h = dt;
    }
    let v = x[rc.idx().expect("rc")];
    assert_eq!(asm.samples(&x), vec![v], "the sample-and-hold reads v(rc)");
    assert!(asm.drain_traces().is_empty(), "no TRACE watch in this circuit");
    (t, v)
}

#[test]
fn the_facade_offers_what_an_analog_side_needs() {
    let (t, v) = rc_with_a_dac(None);
    let analytic = 1.0 - (-1.0_f64).exp();
    assert!((t - 1e-3).abs() < 1e-12, "stopped at {t}");
    assert!((v - analytic).abs() < 1.0e-3, "v(rc) = {v} vs 1 - 1/e = {analytic}");
    let (_, again) = rc_with_a_dac(Some(10));
    assert_eq!(again, v, "a rejected and retried step leaves the run bit for bit unchanged");
}
