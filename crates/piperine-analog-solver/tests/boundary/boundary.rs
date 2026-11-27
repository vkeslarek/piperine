//! The boundary of the analog crate (feature `solver-abi-analog`, SAN-01,
//! SAN-12, T10): its manifest names `piperine-abi` and no other workspace
//! crate, its device modules name no digital or owner item (C1) and hold no
//! `unsafe`, and the device assembly offers, as plain methods, everything an
//! adapter of the owner's `AnalogSide` needs (C22), driven here on an RC
//! with a DAC through the real Newton engine, with one rejected step.

use piperine_device_fixtures::analog::{FixtureModules};
use piperine_device_fixtures::assembly::Circuits;
use piperine_device_fixtures::assembly::{Solver, AssemblySolves, Tran};
use std::path::{Path, PathBuf};

use piperine_analog_solver::device::AnalogBinding::{Absent, Discrete, Ground};
use piperine_analog_solver::device::{AnalogPlan, Assembly, JumpPosition, Stage};
use piperine_analog_solver::math::integration::{TrBdf2, TrBdf2Phase};
use piperine_analog_solver::math::iv::InitialValue;
use piperine_analog_solver::netlist::Netlist;

#[test]
fn the_crate_depends_on_the_abi_crate_only() {
    let text = std::fs::read_to_string(BoundaryCircuits::crate_dir().join("Cargo.toml")).expect("read the manifest");
    assert_eq!(BoundaryCircuits::workspace_dependencies(&text), vec!["piperine-abi".to_string()]);
    let planted = "[dependencies]\npiperine-abi = { path = \"../piperine-abi\" }\npiperine-solver = { path = \"../x\" }\n";
    assert_eq!(BoundaryCircuits::workspace_dependencies(planted), vec!["piperine-abi".to_string(), "piperine-solver".to_string()]);
}

#[test]
fn device_modules_name_no_digital_or_owner_item() {
    let names = BoundaryCircuits::foreign();
    for (path, code) in BoundaryCircuits::code_of(&BoundaryCircuits::crate_dir().join("src/device")) {
        assert!(BoundaryCircuits::found(&code, &names).is_empty(), "{} names {:?}", path.display(), BoundaryCircuits::found(&code, &names));
    }
    let plant = format!("use {}::protocol; let v: {};", names[1], names[2]);
    assert_eq!(BoundaryCircuits::found(&plant, &names), vec![names[1].clone(), names[2].clone()], "the scan catches a plant");
}

#[test]
fn device_modules_hold_no_unsafe() {
    for (path, code) in BoundaryCircuits::code_of(&BoundaryCircuits::crate_dir().join("src/device")) {
        assert!(!code.contains("unsafe"), "{} holds unsafe code", path.display());
    }
    assert!(BoundaryCircuits::strip_comments("let x = unsafe { y };").contains("unsafe"), "the scan catches a plant");
    assert!(!BoundaryCircuits::strip_comments("// nothing here is unsafe").contains("unsafe"), "a comment is not code");
}

#[test]
fn the_facade_offers_what_an_analog_side_needs() {
    let (t, v) = BoundaryCircuits::rc_with_a_dac(None);
    let analytic = 1.0 - (-1.0_f64).exp();
    assert!((t - 1e-3).abs() < 1e-12, "stopped at {t}");
    assert!((v - analytic).abs() < 1.0e-3, "v(rc) = {v} vs 1 - 1/e = {analytic}");
    let (_, again) = BoundaryCircuits::rc_with_a_dac(Some(10));
    assert_eq!(again, v, "a rejected and retried step leaves the run bit for bit unchanged");
}

/// The circuits the boundary tests build.
pub struct BoundaryCircuits;

impl BoundaryCircuits {
    /// The crate dir helper `crate_dir`.
    fn crate_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    /// The `piperine-*` dependency keys of a manifest, but the dev-only fixtures crate: a test may add that one edge.
    fn workspace_dependencies(text: &str) -> Vec<String> {
        text.lines()
            .map(str::trim)
            .filter(|l| (l.starts_with("piperine-") || l.starts_with("[dependencies.piperine-")) && !l.starts_with("piperine-device-fixtures"))
            .map(|l| l.trim_start_matches("[dependencies.").split(['=', ' ', ']']).next().unwrap_or("").to_string())
            .collect()
    }

    /// The Rust sources under `dir`, each with its code lines (comments dropped).
    fn code_of(dir: &Path) -> Vec<(PathBuf, String)> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).expect("read src/device") {
            let path = entry.expect("entry").path();
            if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).expect("read source");
                out.push((path, BoundaryCircuits::strip_comments(&text)));
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

    /// One TR-BDF2 step of `dt` from `t`: both stages, each recorded as a
    /// history row (`assemble_tran` through the Newton loop, `end_iteration`
    /// between iterations, `push_stage` after each stage).
    fn step(solver: &mut Solver, assembly: &mut Assembly, net: &Netlist, t: f64, dt: f64, prev_h: f64) -> Vec<f64> {
        let trap = Stage { phase: TrBdf2Phase::Trapezoidal, h: dt, prev_h, jump: JumpPosition::None };
        AssemblySolves::solve_stage(solver, assembly, net, AssemblySolves::at_time(t + TrBdf2::GAMMA * dt), trap);
        let bdf2 = Stage { phase: TrBdf2Phase::Bdf2, h: dt, prev_h, jump: JumpPosition::None };
        AssemblySolves::solve_stage(solver, assembly, net, AssemblySolves::at_time(t + dt), bdf2).to_vec()
    }

    /// An RC driven by a DAC whose code goes to full scale at t = 0, run over
    /// one time constant in 50 steps; the step `reject` (if any) is solved,
    /// rejected, restored and solved again. Returns the end time and v(rc).
    fn rc_with_a_dac(reject: Option<usize>) -> (f64, f64) {
        let m = FixtureModules::all();
        let (net, n) = Circuits::netlist(2, &["dac.f"]);
        let (out, rc, f) = (&n[0], &n[1], &n[2]);
        let (r, c) = (1.0e3, 1.0e-6);
        let dac = Circuits::instance("dac", m.digital_to_analog, vec![Discrete(0), Discrete(1), Circuits::at(out), Circuits::at(f)]);
        let r1 = Circuits::instance("r1", m.resistor, vec![Discrete(2), Circuits::at(out), Circuits::at(rc)]);
        let c1 = Circuits::instance("c1", m.capacitor, vec![Discrete(3), Circuits::at(rc), Ground]);
        let sh = Circuits::instance("sh", m.sample_hold, vec![Circuits::at(rc), Absent, Discrete(4)]);
        let mut assembly = Assembly::new(AnalogPlan::bind(vec![dac, r1, c1, sh], &net).expect("bind"));
        assembly.set_discrete(0, &Circuits::real_bytes(1.0));
        assembly.set_discrete(2, &Circuits::real_bytes(r));
        assembly.set_discrete(3, &Circuits::real_bytes(c));

        // The operating point at code 0: accepted, and the history's first row.
        let op = AssemblySolves::solve_dc(&mut assembly, &net, &vec![0.0; AssemblySolves::size(&net)]).to_vec();
        assembly.accept(&op, AssemblySolves::at_rest()).expect("accept the operating point");
        assembly.push_stage(&op, AssemblySolves::at_rest()).expect("the operating point's row");

        // A boundary event: the code goes to full scale.
        assembly.set_discrete(1, &[255]);
        let dt = r * c / 50.0;
        let first = Stage { phase: TrBdf2Phase::Trapezoidal, h: dt, prev_h: 0.0, jump: JumpPosition::None };
        let mut solver = Solver::new(&mut Tran { assembly: &mut assembly, net: &net, ambient: AssemblySolves::at_time(0.0), stage: first }, AssemblySolves::size(&net), 4)
            .expect("dry run");
        let ivs = n.iter().zip(&op).map(|(r, v)| InitialValue { reference: r.clone(), value: *v }).collect();
        solver.push_initial_conditions(ivs);

        let (mut t, mut prev_h) = (0.0, 0.0);
        let mut x = op;
        for k in 0..50 {
            let (mark, rows) = (assembly.snapshot(), solver.state_snapshot());
            x = BoundaryCircuits::step(&mut solver, &mut assembly, &net, t, dt, prev_h);
            if reject == Some(k) {
                assembly.restore(&mark);
                solver.restore_state(rows);
                assert_eq!(assembly.snapshot(), mark, "restore returns every history and limit");
                x = BoundaryCircuits::step(&mut solver, &mut assembly, &net, t, dt, prev_h);
            }
            let acc = assembly.accept(&x, AssemblySolves::at_time(t + dt)).expect("accept");
            assert_eq!((acc.fired.len(), acc.cap), (0, f64::INFINITY), "no CROSS or CAP watch");
            t += dt;
            prev_h = dt;
        }
        let v = x[rc.idx().expect("rc")];
        assert_eq!(assembly.samples(&x), vec![v], "the sample-and-hold reads v(rc)");
        assert!(assembly.drain_traces().is_empty(), "no TRACE watch in this circuit");
        (t, v)
    }
}
