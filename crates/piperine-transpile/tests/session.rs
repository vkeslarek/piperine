//! A session on the compiled backend never calls the compiler (feature `ir-rust`, RS-04 AC 6 and
//! 7; checks C22, C23): the compiled RC and diode circuits run `Dc`, `Ac` and a transient, and a
//! `set` of a non-structural param between analyses is applied and read in the next result,
//! while the log of a counting compiler wrapper, which saw the build, does not grow by a line.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use piperine_ir::Ir;
use piperine_solver::prelude::{
    AcSweepAnalysisOptions, CircuitInstance, Context, NodeIdentifier, ParamValue, Tolerances, TransientAnalysisOptions,
};
use piperine_transpile::{Compiled, Settings, Transpiler};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "../../piperine-interp/tests/support/circuits.rs"]
mod circuits;

use circuits::Circuits;

/// Writing an executable and exec'ing it races with another thread's `fork` holding the write
/// descriptor (ETXTBSY); the tests here take turns.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// A compiler wrapper in a directory of its own: logs each call's arguments to `rustc.log`, then
/// runs the real `rustc`.
struct Counting {
    _dir: tempfile::TempDir,
    program: PathBuf,
    log: PathBuf,
    cache: PathBuf,
}

impl Counting {
    fn new() -> Counting {
        let dir = tempfile::tempdir().expect("a temp dir");
        let program = dir.path().join("rustc");
        let log = dir.path().join("rustc.log");
        let text = format!("#!/bin/sh\nREAL=$(command -v rustc)\necho \"$@\" >> {}\nexec \"$REAL\" \"$@\"\n", log.display());
        std::fs::write(&program, text).expect("write the wrapper");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let cache = dir.path().join("cache");
        Counting { _dir: dir, program, log, cache }
    }

    fn compile(&self, ir: &Ir) -> Compiled {
        let settings = Settings { rustc: Some(self.program.clone()), cache: self.cache.clone() };
        Transpiler::new(Arc::new(ir.clone())).unwrap_or_else(|e| panic!("{e}")).compile(&settings).unwrap_or_else(|e| panic!("{e}"))
    }

    /// The compiler calls so far, `-vV` probes included.
    fn calls(&self) -> usize {
        std::fs::read_to_string(&self.log).unwrap_or_default().lines().count()
    }
}

/// The solver node of the design node at `path`.
fn node(ir: &Ir, path: &str) -> NodeIdentifier {
    NodeIdentifier::Anonymous(ir.node_at(path).unwrap_or_else(|| panic!("no node {path}")).id().index())
}

fn dc(circuit: &mut CircuitInstance, id: &NodeIdentifier) -> f64 {
    let result = circuit.dc(Context::default()).expect("dc").solve().expect("solve");
    result.get_node(id).expect("the node is solved")
}

fn ac_magnitude(circuit: &mut CircuitInstance, id: &NodeIdentifier, hz: f64) -> f64 {
    let opts = AcSweepAnalysisOptions { start_frequency: hz, stop_frequency: hz, steps: 1, logarithmic: false };
    let mut ac = circuit.ac(Context::default()).expect("ac");
    ac.solve_sweep(opts).expect("sweep").get(0).expect("one point").get_node(id).expect("the node").norm()
}

fn follow(circuit: &mut CircuitInstance, id: &NodeIdentifier, stop: f64) -> Vec<(f64, f64)> {
    let opts = TransientAnalysisOptions::new(stop, stop / 400.0);
    let result = circuit.transient(opts, Context::default()).expect("transient").solve().expect("solve");
    result.iter().map(|s| (s.time(), s.get_node(id).expect("the node"))).collect()
}

// RS-04 AC 6: `Dc` on the diode circuit and the RC, `Ac` on the RC and a transient of the held
// RC invoke the compiler 0 times; the build before them did call it.
#[test]
fn no_analysis_runs_the_compiler() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let compiler = Counting::new();
    let (diode_ir, rc_ir, held_ir) = (Circuits::diode(), Circuits::rc_lowpass(), Circuits::rc_held());
    let diode = compiler.compile(&diode_ir);
    let rc = compiler.compile(&rc_ir);
    let held = compiler.compile(&held_ir);
    let built = compiler.calls();
    assert!(built >= 3, "the builds called the compiler ({built} calls)");

    let mut circuit = diode.circuit().unwrap_or_else(|e| panic!("circuit: {e}"));
    let a = node(&diode_ir, "DiodeCircuit.a");
    let vd = circuit.dc(Context { tolerances: Tolerances { reltol: 1e-12, vntol: 1e-14, abstol: 1e-16, ..Tolerances::default() }, ..Context::default() })
        .expect("dc").solve().expect("solve").get_node(&a).expect("v(a)");
    assert!(vd > 0.3 && vd < 0.8, "a forward-biased junction: v(a) = {vd}");

    let mut circuit = rc.circuit().unwrap_or_else(|e| panic!("circuit: {e}"));
    let out = node(&rc_ir, "RcLowpass.out");
    let m = ac_magnitude(&mut circuit, &out, 100e3);
    assert!((m - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-9, "-3 dB at the corner: |v(out)| = {m}");

    let mut circuit = held.circuit().unwrap_or_else(|e| panic!("circuit: {e}"));
    let out = node(&held_ir, "RcHeld.out");
    let rc_time = 1000.0 * Circuits::FARADS;
    let steps = follow(&mut circuit, &out, rc_time);
    let (_, v) = *steps.last().expect("steps");
    assert!((v - (1.0 - (-1.0_f64).exp())).abs() < 1e-3, "out at RC: {v}");

    assert_eq!(compiler.calls(), built, "no analysis ran the compiler");
}

// RS-04 AC 7: a `set` of a non-structural param between analyses is applied (the next result
// changes) and invokes the compiler 0 times: the diode's source voltage, then the RC's resistor.
#[test]
fn a_set_between_analyses_runs_no_compiler_and_applies() {
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let compiler = Counting::new();
    let diode_ir = Circuits::diode();
    let rc_ir = Circuits::rc_lowpass();
    let diode = compiler.compile(&diode_ir);
    let rc = compiler.compile(&rc_ir);
    let built = compiler.calls();

    let mut circuit = diode.circuit().unwrap_or_else(|e| panic!("circuit: {e}"));
    let a = node(&diode_ir, "DiodeCircuit.a");
    let before = dc(&mut circuit, &a);
    circuit.set("DiodeCircuit.v1", "dc", ParamValue::Real(0.5)).expect("set the source");
    let after = dc(&mut circuit, &a);
    assert!(after < before, "a lower source gives a lower junction voltage: {before} then {after}");
    circuit.set("DiodeCircuit.v1", "dc", ParamValue::Real(0.8)).expect("set the source back");
    assert_eq!(dc(&mut circuit, &a).to_bits(), before.to_bits(), "the same param gives the same result");

    // 1 k and 1.59 nF: -3 dB at 100 kHz; 2 k halves the corner, so at 100 kHz the gain falls.
    let mut circuit = rc.circuit().unwrap_or_else(|e| panic!("circuit: {e}"));
    let out = node(&rc_ir, "RcLowpass.out");
    let corner = ac_magnitude(&mut circuit, &out, 100e3);
    circuit.set("RcLowpass.r1", "r", ParamValue::Real(2000.0)).expect("set the resistor");
    let doubled = ac_magnitude(&mut circuit, &out, 100e3);
    let expected = 1.0 / (1.0 + (2.0 * std::f64::consts::PI * 100e3 * 2000.0 * Circuits::FARADS).powi(2)).sqrt();
    assert!((doubled - expected).abs() < 1e-9, "|v(out)| with 2 k: {doubled} vs {expected}");
    assert!(doubled < corner, "{doubled} < {corner}");

    assert_eq!(compiler.calls(), built, "a set ran no compiler");
}
