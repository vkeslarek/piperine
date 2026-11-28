//! Scope: the two backends against each other (requirements RS-06 AC 5, RS-07; check C39): the
//! divider, the diode series, the AC RC and the transient RC are solved on the interpreter and on
//! the compiled library, and every result agrees under the `ToleranceContract` of its domain. No
//! bound is written here: each comparison is a contract's `assert`, the interpreter's value in
//! the reference place and the compiled one as the actual, so a rejection names both backends.
//! Every result is also equal bit for bit (verification F9): the two backends' functions give
//! the same bits (RS-06 AC 1), so the solver walks the same Newton path and time steps on both,
//! and any difference is a defect, never noise a contract may absorb.

use piperine_parity::backend::Backend;
use piperine_parity::bench::Bench;
use piperine_parity::contract::{Coordinate, Point, ToleranceContract};
use piperine_parity::waveform::Waveform;
use piperine_solver::prelude::{AcSweepAnalysisOptions, Context, TransientAnalysisOptions};

/// A twin on both backends.
struct Pair {
    fixture: &'static str,
    interp: Bench,
    rust: Bench,
}

impl Pair {
    fn load(fixture: &'static str) -> Pair {
        Pair { fixture, interp: Bench::load_on(fixture, Backend::Interp), rust: Bench::load_on(fixture, Backend::Rust) }
    }

    /// The point of a comparison: the interpreter is the reference, the compiled value the actual.
    fn point<'a>(&self, quantity: &'a str, at: Coordinate) -> Point<'a> {
        Point::new(self.fixture, quantity, at).with_backend("rust against interp")
    }

    /// Every node's operating point, under `DC_VOLTAGE`.
    fn operating_point(&mut self, nodes: &[&str]) {
        for node in nodes {
            let (a, b) = (self.interp.node(node), self.rust.node(node));
            let wanted = self.interp.circuit().dc(Context::default()).expect("dc").solve().expect("interp solves").get_node(&a).expect("v(node)");
            let actual = self.rust.circuit().dc(Context::default()).expect("dc").solve().expect("rust solves").get_node(&b).expect("v(node)");
            let quantity = format!("v({node})");
            ToleranceContract::DC_VOLTAGE.assert(&self.point(&quantity, Coordinate::OperatingPoint), wanted, actual);
            assert_eq!(actual.to_bits(), wanted.to_bits(), "{}: {quantity} is {actual:e} compiled and {wanted:e} interpreted, not the same bits", self.fixture);
        }
    }
}

#[test]
fn both_backends_agree_on_the_m1_circuits() {
    // divider: 10 V across 1 k and 3 k
    Pair::load("divider").operating_point(&["vin", "mid"]);

    // diode series: a source, a resistor and two diodes
    Pair::load("diode_series").operating_point(&["a", "b"]);

    // AC RC: magnitude under AC_MAGNITUDE, phase under AC_PHASE where the magnitude clears its floor
    let mut ac = Pair::load("ac_rc");
    let options = AcSweepAnalysisOptions { start_frequency: 1e3, stop_frequency: 10e6, steps: 41, logarithmic: true };
    let (out_i, out_r) = (ac.interp.node("out"), ac.rust.node("out"));
    let swept_i = ac.interp.circuit().ac(Context::default()).expect("ac").solve_sweep(options.clone()).expect("interp sweeps");
    let swept_r = ac.rust.circuit().ac(Context::default()).expect("ac").solve_sweep(options).expect("rust sweeps");
    assert_eq!(swept_i.len(), swept_r.len(), "the same grid");
    for (a, b) in swept_i.iter().zip(swept_r.iter()) {
        assert_eq!(a.frequency, b.frequency, "the same frequency");
        let (wanted, actual) = (a.get_node(&out_i).expect("v(out)"), b.get_node(&out_r).expect("v(out)"));
        let at = Coordinate::Frequency(a.frequency);
        ToleranceContract::AC_MAGNITUDE.assert(&ac.point("|v(out)|", at), wanted.norm(), actual.norm());
        if ToleranceContract::phase_applies(wanted.norm()) {
            ToleranceContract::AC_PHASE.assert(&ac.point("arg v(out)", at), wanted.arg(), actual.arg());
        }
        assert_eq!(
            (actual.re.to_bits(), actual.im.to_bits()),
            (wanted.re.to_bits(), wanted.im.to_bits()),
            "ac_rc: v(out) at {} Hz is {actual} compiled and {wanted} interpreted, not the same bits",
            a.frequency
        );
    }

    // transient RC to 10 us: the compiled waveform read at the interpreter's times
    let mut tran = Pair::load("tran_rc");
    let (out_i, out_r) = (tran.interp.node("out"), tran.rust.node("out"));
    let (stop, step) = (10e-6, 0.1e-6);
    let mut options = TransientAnalysisOptions::new(stop, step);
    options.dt_max = step;
    let run_i = tran.interp.circuit().transient(options.clone(), Context::default()).expect("transient").solve().expect("interp solves");
    let run_r = tran.rust.circuit().transient(options, Context::default()).expect("transient").solve().expect("rust solves");
    let (steps_i, steps_r): (Vec<_>, Vec<_>) = (run_i.iter().collect(), run_r.iter().collect());
    let compiled = Waveform::new(steps_r.iter().map(|s| s.time()).collect(), steps_r.iter().map(|s| s.get_node(&out_r).expect("v(out)")).collect());
    assert_ne!(steps_i.len(), 0, "the interpreter's transient has steps");
    // the same time steps and the same values, bit for bit
    assert_eq!(steps_r.len(), steps_i.len(), "tran_rc: the same number of steps on both backends");
    for (r, i) in steps_r.iter().zip(&steps_i) {
        let (vr, vi) = (r.get_node(&out_r).expect("v(out)"), i.get_node(&out_i).expect("v(out)"));
        assert_eq!((r.time().to_bits(), vr.to_bits()), (i.time().to_bits(), vi.to_bits()), "tran_rc: v(out) = {vr:e} at {:e} s compiled, {vi:e} at {:e} s interpreted", r.time(), i.time());
    }
    for s in &steps_i {
        let actual = compiled.at(s.time()).unwrap_or_else(|| panic!("tran_rc: the time {:e} s lies outside the compiled result", s.time()));
        let wanted = s.get_node(&out_i).expect("v(out)");
        ToleranceContract::TRAN_VOLTAGE.assert(&tran.point("v(out)", Coordinate::Time(s.time())), wanted, actual);
    }
}
