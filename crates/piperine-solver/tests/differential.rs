//! The differential oracle (SAI-13, SAI-02): each circuit is built twice, from
//! the `Element` doubles of `helpers/devices.rs` and from their `Device` ports
//! of `helpers/abi_devices.rs`, and the two are compared unknown by unknown at
//! 1e-12 relative, with equal accepted step counts and Newton iteration
//! counts. The target exists while both contracts do.

// Each library is used selectively.
#[allow(dead_code)]
#[path = "helpers/devices.rs"]
mod elements;
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod devices;

use std::collections::HashMap;
use std::sync::Arc;

use piperine_solver::abi::{AnalogReference, AnalogVariable, InitialValue, ParamValue, Port};
use piperine_solver::prelude::{CircuitBuilder, CircuitInstance, Context, TransientAnalysisOptions};

const REL: f64 = 1.0e-12;

/// `|a - b| <= REL * max(|a|, |b|)`, the floor being a number no circuit here can produce.
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= REL * a.abs().max(b.abs()) + 1.0e-30
}

fn assert_close(what: &str, a: f64, b: f64) {
    assert!(close(a, b), "{what}: element {a:e} vs device {b:e}");
}

/// A solution by unknown index: both contracts allocate nodes, then branches, in the same order.
fn by_index(circuit: &CircuitInstance, values: &HashMap<Arc<AnalogVariable>, f64>) -> Vec<f64> {
    let mut refs: Vec<&AnalogReference> = circuit.netlist().all_references();
    refs.retain(|r| r.idx().is_some());
    refs.sort_by_key(|r| r.idx());
    refs.iter().map(|r| values[r.variable()]).collect()
}

/// The circuits of the oracle, each on both contracts. Nodes: `top`, `mid`.
enum Fixture {
    Divider,
    Diode,
    DiodeSeries,
    Rc,
    SineRc,
}

fn element_circuit(f: &Fixture) -> (CircuitInstance, AnalogReference) {
    let mut b = CircuitBuilder::new("elements");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    match f {
        Fixture::Divider => {
            b.element(Box::new(elements::Vsrc::new("v1", 7.0, top.clone(), gnd.clone())));
            b.element(Box::new(elements::R::new("r1", 1.5e3, top.clone(), mid.clone())));
            b.element(Box::new(elements::R::new("r2", 4.5e3, mid.clone(), gnd.clone())));
        }
        Fixture::Diode => {
            b.element(Box::new(elements::Vsrc::new("v1", 1.0, top.clone(), gnd.clone())));
            b.element(Box::new(elements::R::new("r1", 1.0e3, top.clone(), mid.clone())));
            b.element(Box::new(elements::D::new("d1", 1.0e-14, mid.clone(), gnd.clone())));
        }
        Fixture::DiodeSeries => {
            let low = b.node("low");
            b.element(Box::new(elements::Vsrc::new("v1", 1.0, top.clone(), gnd.clone())));
            b.element(Box::new(elements::R::new("r1", 470.0, top.clone(), mid.clone())));
            b.element(Box::new(elements::D::new("d1", 1.0e-14, mid.clone(), low.clone())));
            b.element(Box::new(elements::R::new("r2", 22.0, low, gnd.clone())));
        }
        Fixture::Rc => {
            b.element(Box::new(elements::Vsrc::new("v1", 1.0, top.clone(), gnd.clone())));
            b.element(Box::new(elements::R::new("r1", 1.0e3, top.clone(), mid.clone())));
            b.element(Box::new(elements::C::new("c1", 1.0e-6, mid.clone(), gnd.clone())));
        }
        Fixture::SineRc => {
            b.element(Box::new(elements::Vsine::new("v1", 5.0, 1.0e3, top.clone(), gnd.clone())));
            b.element(Box::new(elements::R::new("r1", 1.0e3, top.clone(), mid.clone())));
            b.element(Box::new(elements::C::new("c1", 1.0e-6, mid.clone(), gnd.clone())));
        }
    }
    (b.build().expect("build"), mid)
}

fn device_circuit(f: &Fixture) -> (CircuitInstance, AnalogReference) {
    let mut b = CircuitBuilder::new("devices");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    match f {
        Fixture::Divider => {
            devices::add_vsrc(&mut b, "v1", 7.0, &top, &gnd);
            devices::add_resistor(&mut b, "r1", 1.5e3, &top, &mid);
            devices::add_resistor(&mut b, "r2", 4.5e3, &mid, &gnd);
        }
        Fixture::Diode => {
            devices::add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
            devices::add_resistor(&mut b, "r1", 1.0e3, &top, &mid);
            devices::add_diode(&mut b, "d1", 1.0e-14, &mid, &gnd);
        }
        Fixture::DiodeSeries => {
            let low = b.node("low");
            devices::add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
            devices::add_resistor(&mut b, "r1", 470.0, &top, &mid);
            devices::add_diode(&mut b, "d1", 1.0e-14, &mid, &low);
            devices::add_resistor(&mut b, "r2", 22.0, &low, &gnd);
        }
        Fixture::Rc => {
            devices::add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
            devices::add_resistor(&mut b, "r1", 1.0e3, &top, &mid);
            devices::add_capacitor(&mut b, "c1", 1.0e-6, &mid, &gnd);
        }
        Fixture::SineRc => {
            b.device(
                devices::vsine(),
                "v1",
                &[("amp", ParamValue::Real(5.0)), ("freq", ParamValue::Real(1.0e3))],
                &[Port::Node(top.clone()), Port::Node(gnd.clone())],
            );
            devices::add_resistor(&mut b, "r1", 1.0e3, &top, &mid);
            devices::add_capacitor(&mut b, "c1", 1.0e-6, &mid, &gnd);
        }
    }
    (b.build().expect("build"), mid)
}

fn dc(circuit: &mut CircuitInstance) -> (Vec<f64>, usize) {
    let result = circuit.dc(Context::default()).expect("dc").solve().expect("solve");
    (by_index(circuit, result.values()), result.stats.newton_iterations)
}

#[test]
fn dc_agrees_on_both_contracts() {
    for (name, f) in [("divider", Fixture::Divider), ("diode", Fixture::Diode), ("diode with series resistance", Fixture::DiodeSeries)] {
        let (mut e, _) = element_circuit(&f);
        let (mut d, _) = device_circuit(&f);
        let (xe, xd) = (dc(&mut e).0, dc(&mut d).0);
        assert_eq!(xe.len(), xd.len(), "{name}: unknown count");
        for (i, (a, b)) in xe.iter().zip(&xd).enumerate() {
            assert_close(&format!("{name}, unknown {i}"), *a, *b);
        }
    }
}

/// Every accepted step of an RC transient: (time, unknowns by index).
fn tran(circuit: &mut CircuitInstance, mid: &AnalogReference, seed: bool, stop: f64, dt: f64) -> (Vec<(f64, Vec<f64>)>, usize) {
    let mut solver = circuit.transient(TransientAnalysisOptions::new(stop, dt), Context::default()).expect("tran");
    if seed {
        solver.apply_initial_conditions(vec![InitialValue { reference: mid.clone(), value: 0.0 }]);
    }
    let result = solver.solve().expect("solve");
    let newton = result.stats.newton_iterations;
    drop(solver);
    let steps = result.iter().map(|s| (s.time(), by_index(circuit, s.values()))).collect();
    (steps, newton)
}

#[test]
fn transient_takes_the_same_steps_on_both_contracts() {
    for (name, f, seed, stop, dt) in [
        ("RC step", Fixture::Rc, true, 1.0e-3, 1.0e-5),
        ("sine-driven RC", Fixture::SineRc, false, 2.0e-3, 2.0e-5),
    ] {
        let (mut e, me) = element_circuit(&f);
        let (mut d, md) = device_circuit(&f);
        let (se, _) = tran(&mut e, &me, seed, stop, dt);
        let (sd, _) = tran(&mut d, &md, seed, stop, dt);
        assert_eq!(se.len(), sd.len(), "{name}: accepted step count");
        for (k, ((te, xe), (td, xd))) in se.iter().zip(&sd).enumerate() {
            assert!((te - td).abs() <= 1.0e-15 * te.abs().max(td.abs()), "{name}, step {k}: time {te:e} vs {td:e}");
            for (i, (a, b)) in xe.iter().zip(xd).enumerate() {
                assert_close(&format!("{name}, step {k}, unknown {i}"), *a, *b);
            }
        }
    }
}

#[test]
fn newton_takes_the_same_iterations_on_both_contracts() {
    let (mut e, _) = element_circuit(&Fixture::Diode);
    let (mut d, _) = device_circuit(&Fixture::Diode);
    assert_eq!(dc(&mut e).1, dc(&mut d).1, "Newton iterations of the diode circuit");
}

// ---------------------------------------------------------------------------
// The small-signal family
// ---------------------------------------------------------------------------

use num_complex::Complex64;
use piperine_solver::abi::{
    AcAnalysisContext, AnalogDevice, DcAnalysisState, DigitalDevice, Element, ElementCapabilities, Introspect,
    Noise, NoiseKind, Stamp,
};
use piperine_solver::prelude::{AcSweepAnalysisOptions, AdjointOptions, DcAnalysisResult, NodeIdentifier, NoiseAnalysisOptions};

/// A resistor on the `Element` contract with its DC and AC conductance and its thermal noise
/// (`4kT/r` at the default 300.15 K).
struct AcR {
    label: &'static str,
    r: f64,
    p: AnalogReference,
    n: AnalogReference,
}

fn four_corners<T: Copy + std::ops::Neg<Output = T> + piperine_analog_solver::math::num::Scalar>(p: &AnalogReference, n: &AnalogReference, y: T) -> Vec<Stamp<AnalogReference, T>> {
    vec![
        Stamp::Matrix(p.clone(), p.clone(), y),
        Stamp::Matrix(n.clone(), n.clone(), y),
        Stamp::Matrix(p.clone(), n.clone(), -y),
        Stamp::Matrix(n.clone(), p.clone(), -y),
    ]
}

impl AnalogDevice for AcR {
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        four_corners(&self.p, &self.n, 1.0 / self.r)
    }
    fn load_ac(&mut self, _d: &DcAnalysisResult, _a: &AcAnalysisContext, _c: &Context) -> Vec<Stamp<AnalogReference, Complex64>> {
        four_corners(&self.p, &self.n, Complex64::new(1.0 / self.r, 0.0))
    }
    fn noise_current_psd(&mut self, _d: &DcAnalysisResult, _a: &AcAnalysisContext) -> Vec<Noise> {
        let psd = 4.0 * devices::K_B * 300.15 / self.r;
        vec![Noise::new((self.p.clone(), self.n.clone()), psd).named("thermal", NoiseKind::Thermal)]
    }
}
impl DigitalDevice for AcR {}
impl Introspect for AcR {}
impl Element for AcR {
    fn name(&self) -> &str {
        self.label
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_AC | ElementCapabilities::EMITS_NOISE
    }
}

/// A capacitor, an inductor (`Y = 1/(jωL)`) and a current source, AC only, to ground or between nodes.
enum AcKind {
    C(f64),
    L(f64),
    Isrc(f64),
}

struct AcPart {
    kind: AcKind,
    p: AnalogReference,
    n: AnalogReference,
}

impl AnalogDevice for AcPart {
    fn load_ac(&mut self, _d: &DcAnalysisResult, a: &AcAnalysisContext, _c: &Context) -> Vec<Stamp<AnalogReference, Complex64>> {
        let w = 2.0 * std::f64::consts::PI * a.frequency;
        match self.kind {
            AcKind::C(c) => four_corners(&self.p, &self.n, Complex64::new(0.0, w * c)),
            AcKind::L(l) => four_corners(&self.p, &self.n, Complex64::new(1.0, 0.0) / Complex64::new(0.0, w * l)),
            AcKind::Isrc(i) => vec![Stamp::Rhs(self.p.clone(), Complex64::new(i, 0.0))],
        }
    }
}
impl DigitalDevice for AcPart {}
impl Introspect for AcPart {}
impl Element for AcPart {
    fn name(&self) -> &str {
        "ac"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_AC
    }
}

/// One AC fixture on both contracts: (nodes, parts) with the node count.
#[derive(Clone, Copy)]
enum SmallSignal {
    /// A current source into a node loaded by R, C: a low-pass.
    RcLowPass,
    /// The parallel RLC of `parity_baseline.rs`.
    Rlc,
    /// Three resistors and a capacitor, no source: noise only. Output node `b`.
    NoiseLadder,
}

fn element_small(f: SmallSignal) -> CircuitInstance {
    let mut b = CircuitBuilder::new("elements");
    let gnd = b.ground();
    let a = b.node("a");
    match f {
        SmallSignal::RcLowPass => {
            b.element(Box::new(AcPart { kind: AcKind::Isrc(1.0), p: a.clone(), n: gnd.clone() }));
            b.element(Box::new(AcR { label: "r1", r: 1.0e3, p: a.clone(), n: gnd.clone() }));
            b.element(Box::new(AcPart { kind: AcKind::C(1.0e-9), p: a.clone(), n: gnd.clone() }));
        }
        SmallSignal::Rlc => {
            b.element(Box::new(AcPart { kind: AcKind::Isrc(1.0), p: a.clone(), n: gnd.clone() }));
            b.element(Box::new(AcR { label: "r1", r: 1.0e4, p: a.clone(), n: gnd.clone() }));
            b.element(Box::new(AcPart { kind: AcKind::C(1.0e-9), p: a.clone(), n: gnd.clone() }));
            b.element(Box::new(AcPart { kind: AcKind::L(1.0e-3), p: a.clone(), n: gnd.clone() }));
        }
        SmallSignal::NoiseLadder => {
            let nb = b.node("b");
            b.element(Box::new(AcR { label: "r1", r: 1.0e3, p: a.clone(), n: gnd.clone() }));
            b.element(Box::new(AcR { label: "r2", r: 2.0e3, p: a.clone(), n: nb.clone() }));
            b.element(Box::new(AcR { label: "r3", r: 3.0e3, p: nb.clone(), n: gnd.clone() }));
            b.element(Box::new(AcPart { kind: AcKind::C(1.0e-9), p: nb, n: gnd.clone() }));
        }
    }
    b.build().expect("build")
}

fn device_small(f: SmallSignal) -> CircuitInstance {
    let mut b = CircuitBuilder::new("devices");
    let gnd = b.ground();
    let a = b.node("a");
    let to = |p: &AnalogReference, n: &AnalogReference| [Port::Node(p.clone()), Port::Node(n.clone())];
    match f {
        SmallSignal::RcLowPass => {
            b.device(devices::current_ac(), "i1", &[("i", ParamValue::Real(1.0))], &[Port::Node(a.clone())]);
            devices::add_resistor(&mut b, "r1", 1.0e3, &a, &gnd);
            devices::add_capacitor(&mut b, "c1", 1.0e-9, &a, &gnd);
        }
        SmallSignal::Rlc => {
            b.device(devices::current_ac(), "i1", &[("i", ParamValue::Real(1.0))], &[Port::Node(a.clone())]);
            devices::add_resistor(&mut b, "r1", 1.0e4, &a, &gnd);
            devices::add_capacitor(&mut b, "c1", 1.0e-9, &a, &gnd);
            b.device(devices::inductor(), "l1", &[("l", ParamValue::Real(1.0e-3))], &to(&a, &gnd));
        }
        SmallSignal::NoiseLadder => {
            let nb = b.node("b");
            for (path, r, p, n) in [("r1", 1.0e3, &a, &gnd), ("r2", 2.0e3, &a, &nb), ("r3", 3.0e3, &nb, &gnd)] {
                b.device(devices::resistor_noisy(), path, &[("r", ParamValue::Real(r))], &to(p, n));
            }
            devices::add_capacitor(&mut b, "c1", 1.0e-9, &nb, &gnd);
        }
    }
    b.build().expect("build")
}

#[test]
fn ac_agrees_on_both_contracts() {
    for (name, f, sweep) in [
        ("RC low-pass", SmallSignal::RcLowPass, AcSweepAnalysisOptions { start_frequency: 1.0e3, stop_frequency: 1.0e7, steps: 25, logarithmic: true }),
        ("parallel RLC", SmallSignal::Rlc, AcSweepAnalysisOptions { start_frequency: 1.0e4, stop_frequency: 1.0e6, steps: 40, logarithmic: true }),
    ] {
        let run = |mut c: CircuitInstance| c.ac(Context::default()).expect("ac").solve_sweep(sweep.clone()).expect("sweep");
        let (e, d) = (run(element_small(f)), run(device_small(f)));
        assert_eq!(e.len(), d.len(), "{name}: points");
        let node = NodeIdentifier::Anonymous(0);
        for i in 0..e.len() {
            let (se, sd) = (e.get(i).unwrap(), d.get(i).unwrap());
            let (ve, vd) = (*se.get_node(&node).unwrap(), *sd.get_node(&node).unwrap());
            assert_close(&format!("{name}, point {i}, re"), ve.re, vd.re);
            assert_close(&format!("{name}, point {i}, im"), ve.im, vd.im);
        }
    }
}

#[test]
fn noise_agrees_on_both_contracts() {
    let options = NoiseAnalysisOptions {
        sweep_options: AcSweepAnalysisOptions { start_frequency: 1.0e3, stop_frequency: 1.0e6, steps: 7, logarithmic: true },
        output_node: NodeIdentifier::Anonymous(1),
        reference_node: NodeIdentifier::Gnd,
        input_source_name: None,
    };
    let run = |mut c: CircuitInstance| c.noise(options.clone(), Context::default()).expect("noise").solve().expect("solve");
    let (e, d) = (run(element_small(SmallSignal::NoiseLadder)), run(device_small(SmallSignal::NoiseLadder)));
    assert_eq!(e.out_noise_sq.len(), d.out_noise_sq.len());
    for (i, (a, b)) in e.out_noise_sq.iter().zip(&d.out_noise_sq).enumerate() {
        assert_close(&format!("output density, point {i}"), *a, *b);
    }
    assert_close("integrated noise", e.integrated_noise, d.integrated_noise);
    let by_name = |r: &piperine_solver::prelude::NoiseAnalysisResult| {
        let mut v: Vec<(String, String, Vec<f64>)> = r.contributions.iter().map(|c| (c.element.clone(), c.source.clone(), c.psd.clone())).collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    };
    let (ce, cd) = (by_name(&e), by_name(&d));
    assert_eq!(ce.len(), 3);
    for ((en, es, ep), (dn, ds, dp)) in ce.iter().zip(&cd) {
        assert_eq!((en, es), (dn, ds), "contribution names");
        for (i, (a, b)) in ep.iter().zip(dp).enumerate() {
            assert_close(&format!("{en}.{es} point {i}"), *a, *b);
        }
    }
}

#[test]
fn sensitivities_agree_on_both_contracts() {
    let wrt: Vec<(String, String)> =
        [("r1", "r"), ("r2", "r"), ("v1", "dc")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
    for (name, f) in [("divider", Fixture::Divider), ("diode", Fixture::Diode)] {
        let params: Vec<(String, String)> = if matches!(f, Fixture::Diode) {
            [("r1", "r"), ("d1", "is_sat"), ("v1", "dc")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
        } else {
            wrt.clone()
        };
        let run = |(mut c, mid): (CircuitInstance, AnalogReference)| {
            let var = mid.variable().clone();
            let net = c.nets().into_iter().find(|n| n.analog_variable().map(|v| **v == *var).unwrap_or(false)).expect("net");
            let label = net.label().to_string();
            let r = c.adjoint(AdjointOptions::new(net, params.clone()), Context::default()).expect("adjoint").solve().expect("solve");
            (label, r)
        };
        let ((le, e), (ld, d)) = (run(element_circuit(&f)), run(device_circuit(&f)));
        assert_eq!(le, ld);
        for (label, param) in &params {
            let (a, b) = (e.get(&le, label, param).expect("element"), d.get(&ld, label, param).expect("device"));
            assert_close(&format!("{name}: d/d{label}.{param}"), a, b);
        }
    }
}
