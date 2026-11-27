//! The noise driver on devices (SAI-11): a divider of resistors with thermal
//! noise reads the analytic `4kT·(R1 ‖ R2)`, a noise input injected through
//! two terms is one correlated source and cancels, each contribution keeps its
//! instance and input names, and the per-source PSDs add up to the output.

use piperine_device_fixtures::circuit::K_B;
use piperine_device_fixtures::circuit;
use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::partials::PartialsCheck;
use std::result::Result;
use std::sync::{Arc, OnceLock};

use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, cell, input, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, InputSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{
    AcSweepAnalysisOptions, CircuitBuilder, Context, NodeIdentifier, NoiseAnalysisOptions, NoiseKind, Tolerances,
};

#[test]
fn a_resistor_divider_has_its_thermal_noise_on_devices() {
    let (r1, r2) = (1.0e3, 3.0e3);
    let mut b = CircuitBuilder::new("divider");
    let gnd = b.ground();
    let top = b.node("top");
    let mid = b.node("mid");
    CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &top, &gnd);
    for (path, r, p, n) in [("r1", r1, &top, &mid), ("r2", r2, &mid, &gnd)] {
        b.device(
            circuit::NoisyResistor::module(),
            path,
            &[("r", ParamValue::Real(r))],
            &[Port::Node(p.clone()), Port::Node(n.clone())],
        );
    }
    let mut circuit = b.build().expect("build");
    let out = CircuitBuilding::node_id(&mid);
    let result = circuit.noise(AbiNoiseCases::options(out), Context::default()).expect("noise").solve().expect("solve");

    let analytic = 4.0 * K_B * Tolerances::default().temperature * (r1 * r2 / (r1 + r2));
    for (i, density) in result.out_noise_sq.iter().enumerate() {
        assert!((density - analytic).abs() <= 1.0e-9 * analytic, "density {density:e} vs {analytic:e} at point {i}");
    }
    // Each contribution keeps its instance and input names.
    let mut names: Vec<(String, String)> =
        result.contributions.iter().map(|c| (c.element.clone(), c.source.clone())).collect();
    names.sort();
    assert_eq!(names, vec![("r1".into(), "thermal".into()), ("r2".into(), "thermal".into())]);
}

#[test]
fn a_shared_noise_input_cancels_across_the_pair() {
    let density = |k: f64| {
        let mut b = CircuitBuilder::new("pair");
        let gnd = b.ground();
        let node = b.node("node");
        CircuitBuilding::add_resistor(&mut b, "r1", 1.0e3, &node, &gnd);
        b.device(circuit::NoisePair::module(), "x1", &[("k", ParamValue::Real(k))], &[Port::Node(node.clone())]);
        let mut circuit = b.build().expect("build");
        let out = CircuitBuilding::node_id(&node);
        let result = circuit.noise(AbiNoiseCases::options(out), Context::default()).expect("noise").solve().expect("solve");
        result.out_noise_sq[0]
    };
    // Gains 1 and -1 on one source: the same current in and out, nothing reaches the node.
    assert!(density(-1.0) <= 1.0e-30, "a correlated pair leaks {:e}", density(-1.0));
    // One gain alone: 1e-18 A²/Hz into 1 kΩ is 1e-12 V²/Hz.
    let alone = density(0.0);
    assert!((alone - 1.0e-12).abs() <= 1.0e-9 * 1.0e-12, "{alone:e}");
}

/// A resistor whose branch carries two noise sources: a thermal one of density
/// `4kT/r` (T = 300 K) and a flicker one of `1e-24`.
/// pins: r=0 (PARAM F64, 1e3); p=1, n=2 (PORT)
/// inputs: I0 NOISE "thermal", I1 NOISE "flicker"; terms: T0 FLOW{p, n}
/// fn 0 "i": reads [p, n, r, I0, I1] writes [T0] partials against all five
/// fn 1 "thermal": reads [r] writes [I0.PSD]; fn 2 "flicker": reads [] writes [I1.PSD]
struct NoisyR;
static NOISY_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static NOISY: OnceLock<Module> = OnceLock::new();

impl Device for NoisyR {
    fn prototype() -> &'static Shape<'static> {
        NOISY_SHAPE.get_or_init(AbiNoiseCases::noisy_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((NoisyR, Arc::clone(NOISY_SHAPE.get_or_init(AbiNoiseCases::noisy_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        match k {
            0 => {
                let (vp, vn, r, thermal, flicker) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3), cx.f64(4));
                let g = 1.0 / r;
                cx.set(0, g * vp - g * vn + thermal + flicker);
                cx.jac().copy_from_slice(&[g, -g, -(vp - vn) / (r * r), 1.0, 1.0]);
            }
            1 => cx.set(0, 4.0 * 1.380649e-23 * 300.0 / cx.f64(0)),
            _ => cx.set(0, 1.0e-24_f64),
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

#[test]
fn test_per_source_noise_reporting_and_conservation() {
    let mut b = CircuitBuilder::new("test");
    let gnd = b.ground();
    let top = b.node("top");
    for path in ["r1", "r2"] {
        b.device(
            NOISY.get_or_init(|| module::<NoisyR>("NoisyR")),
            path,
            &[("r", ParamValue::Real(1000.0))],
            &[Port::Node(top.clone()), Port::Node(gnd.clone())],
        );
    }
    let mut circuit = b.build().expect("build");
    let sweep = AcSweepAnalysisOptions { start_frequency: 1.0, stop_frequency: 100.0, steps: 10, logarithmic: false };
    let opts = NoiseAnalysisOptions {
        sweep_options: sweep,
        output_node: CircuitBuilding::node_id(&top),
        reference_node: NodeIdentifier::Gnd,
        input_source_name: None,
        output_flow: None,
    };
    let mut solver = circuit.noise(opts, Context::default()).unwrap();
    let res = solver.solve().unwrap();

    let contribs = res.contributions();
    assert_eq!(contribs.len(), 4);

    let mut found_thermal = 0;
    let mut found_flicker = 0;
    for c in contribs {
        if c.source == "thermal" && c.kind == NoiseKind::Thermal {
            found_thermal += 1;
            assert_eq!(c.psd.len(), 10);
        }
        if c.source == "flicker" && c.kind == NoiseKind::Flicker {
            found_flicker += 1;
            assert_eq!(c.psd.len(), 10);
        }
    }
    assert_eq!(found_thermal, 2);
    assert_eq!(found_flicker, 2);

    for i in 0..res.frequencies.len() {
        let sum_psd: f64 = contribs.iter().map(|c| c.psd[i]).sum();
        let total = res.out_noise_sq[i];

        let err = (sum_psd - total).abs();
        let rel_err = if total > 1e-30 { err / total } else { err };
        assert!(rel_err < 1e-9, "freq idx {}, sum {} != total {}", i, sum_psd, total);
    }

    let mut manual_int = 0.0;
    for i in 0..res.frequencies.len() - 1 {
        let df = res.frequencies[i + 1] - res.frequencies[i];
        manual_int += 0.5 * df * (res.out_noise_sq[i] + res.out_noise_sq[i + 1]);
    }
    assert!((res.integrated_noise - manual_int.sqrt()).abs() < 1e-12);
}

/// SAI-15: every function of this target's own devices passes the partials
/// verifier (the guard of `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    PartialsCheck::verify_every_function("NoisyR", NOISY.get_or_init(|| module::<NoisyR>("NoisyR")));
}

/// The helpers of the abi_noise tests.
pub struct AbiNoiseCases;

impl AbiNoiseCases {
    /// The options helper `options`.
    fn options(output: NodeIdentifier) -> NoiseAnalysisOptions {
        NoiseAnalysisOptions {
            sweep_options: AcSweepAnalysisOptions { start_frequency: 1.0e3, stop_frequency: 1.0e5, steps: 5, logarithmic: true },
            output_node: output,
            reference_node: NodeIdentifier::Gnd,
            input_source_name: None,
            output_flow: None,
        }
    }

    // ─── A resistor with a thermal and a flicker source ─────────────────────────

    /// The cell of helper `cell_of`.
    fn cell_of(kind: u32, index: u32, part: u32) -> Cell {
        Cell { kind, index, part }
    }

    /// The partial helper `partial`.
    fn partial(read: u32) -> Partial {
        Partial { write: 0, to: 0, read, at: 0 }
    }

    /// The noisy shape helper `noisy_shape`.
    fn noisy_shape() -> ShapeRef {
        let mut s = ShapeBuf::new();
        s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &1.0e3_f64.to_le_bytes(), ..PinSpec::discrete("r", role::PARAM, &F64) });
        let p = s.pin(PinSpec::continuous("p", role::PORT));
        let n = s.pin(PinSpec::continuous("n", role::PORT));
        let i0 = s.input(InputSpec { name: "thermal", origin: "abi_noise.rs", kind: input::NOISE });
        let i1 = s.input(InputSpec { name: "flicker", origin: "abi_noise.rs", kind: input::NOISE });
        let t0 = s.term(TermSpec { name: "i", origin: "abi_noise.rs", op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        s.function(FunctionSpec {
            name: "i",
            origin: "abi_noise.rs",
            reads: vec![
                AbiNoiseCases::cell_of(cell::PIN, p, part::VALUE),
                AbiNoiseCases::cell_of(cell::PIN, n, part::VALUE),
                AbiNoiseCases::cell_of(cell::PIN, 0, part::VALUE),
                AbiNoiseCases::cell_of(cell::INPUT, i0, part::VALUE),
                AbiNoiseCases::cell_of(cell::INPUT, i1, part::VALUE),
            ],
            writes: vec![AbiNoiseCases::cell_of(cell::TERM, t0, part::VALUE)],
            partials: (0..5).map(AbiNoiseCases::partial).collect(),
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "thermal",
            origin: "abi_noise.rs",
            reads: vec![AbiNoiseCases::cell_of(cell::PIN, 0, part::VALUE)],
            writes: vec![AbiNoiseCases::cell_of(cell::INPUT, i0, part::PSD)],
            ..FunctionSpec::default()
        });
        s.function(FunctionSpec {
            name: "flicker",
            origin: "abi_noise.rs",
            reads: vec![],
            writes: vec![AbiNoiseCases::cell_of(cell::INPUT, i1, part::PSD)],
            ..FunctionSpec::default()
        });
        s.freeze()
    }
}
