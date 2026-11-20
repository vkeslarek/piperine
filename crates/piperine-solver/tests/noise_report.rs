//! The report of the noise analysis (feature `analog-operators`, AO-05): the band total by
//! ngspice's power-law rule, one series per source name, and the input-referred, flow and
//! differential forms of the output. Every expected value is a closed form written beside its
//! assertion, over the hand-built devices of `helpers/abi_devices.rs` and the two of this file.

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use std::result::Result;
use std::sync::{Arc, OnceLock};

use abi_devices::{K_B, add_vsrc, node_id, resistor_noisy};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::F64;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, ambient, cell, input, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, InputSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{ParamValue, Port};
use piperine_solver::prelude::{
    AcSweepAnalysisOptions, CircuitBuilder, Context, NodeIdentifier, NoiseAnalysisOptions, NoiseAnalysisResult, Tolerances,
};

/// The options of a sweep from `start` to `stop` Hz over `steps` log-spaced points, output `out`.
fn options(out: NodeIdentifier, start: f64, stop: f64, steps: usize) -> NoiseAnalysisOptions {
    NoiseAnalysisOptions {
        sweep_options: AcSweepAnalysisOptions { start_frequency: start, stop_frequency: stop, steps, logarithmic: true },
        output_node: out,
        reference_node: NodeIdentifier::Gnd,
        input_source_name: None,
    }
}

// ─── A resistor whose branch carries two flicker sources of one name ────────

fn cell_of(kind: u32, index: u32, part: u32) -> Cell {
    Cell { kind, index, part }
}

fn partial(read: u32) -> Partial {
    Partial { write: 0, to: 0, read, at: 0 }
}

/// The density constants of the two sources of [`TwinFlicker`]: `FIRST/f` and `SECOND/f` A²/Hz.
const FIRST: f64 = 1.0e-24;
const SECOND: f64 = 3.0e-24;

/// A resistor `r` from `p` to `n` carrying two `NOISE` inputs both named `flicker`, of densities
/// `FIRST/f` and `SECOND/f`.
/// pins: r=0 (PARAM F64, 1e3); p=1, n=2 (PORT)
/// inputs: I0 NOISE "flicker", I1 NOISE "flicker"; terms: T0 FLOW{p, n} "i"
/// fn 0 "i": reads [p, n, r, I0, I1] writes [T0] partials against all five
/// fn 1 "first": reads [FREQ] writes [I0.PSD]; fn 2 "second": reads [FREQ] writes [I1.PSD]
struct TwinFlicker;
static TWIN_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static TWIN: OnceLock<Module> = OnceLock::new();

fn twin_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    s.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &1.0e3_f64.to_le_bytes(), ..PinSpec::discrete("r", role::PARAM, &F64) });
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let i0 = s.input(InputSpec { name: "flicker", origin: "noise_report.rs", kind: input::NOISE });
    let i1 = s.input(InputSpec { name: "flicker", origin: "noise_report.rs", kind: input::NOISE });
    let t0 = s.term(TermSpec { name: "i", origin: "noise_report.rs", op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
    s.function(FunctionSpec {
        name: "i",
        origin: "noise_report.rs",
        reads: vec![
            cell_of(cell::PIN, p, part::VALUE),
            cell_of(cell::PIN, n, part::VALUE),
            cell_of(cell::PIN, 0, part::VALUE),
            cell_of(cell::INPUT, i0, part::VALUE),
            cell_of(cell::INPUT, i1, part::VALUE),
        ],
        writes: vec![cell_of(cell::TERM, t0, part::VALUE)],
        partials: (0..5).map(partial).collect(),
        ..FunctionSpec::default()
    });
    for (name, input) in [("first", i0), ("second", i1)] {
        s.function(FunctionSpec {
            name,
            origin: "noise_report.rs",
            reads: vec![cell_of(cell::AMBIENT, ambient::FREQ, part::VALUE)],
            writes: vec![cell_of(cell::INPUT, input, part::PSD)],
            ..FunctionSpec::default()
        });
    }
    s.freeze()
}

impl Device for TwinFlicker {
    fn prototype() -> &'static Shape<'static> {
        TWIN_SHAPE.get_or_init(twin_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((TwinFlicker, Arc::clone(TWIN_SHAPE.get_or_init(twin_shape))))
    }
    fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
        match k {
            0 => {
                let (vp, vn, r, first, second) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3), cx.f64(4));
                let g = 1.0 / r;
                cx.set(0, g * vp - g * vn + first + second);
                cx.jac().copy_from_slice(&[g, -g, -(vp - vn) / (r * r), 1.0, 1.0]);
            }
            1 => cx.set(0, FIRST / cx.f64(0)),
            _ => cx.set(0, SECOND / cx.f64(0)),
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

fn twin_flicker() -> &'static Module {
    TWIN.get_or_init(|| module::<TwinFlicker>("TwinFlicker"))
}

/// The partials verifier holds this target's own device to the same guard as the helpers.
#[test]
fn its_device_passes_the_partials_verifier() {
    partials::verify_every_function("TwinFlicker", twin_flicker());
}

/// The result of a noise run of `twin_flicker` (R = 1 kΩ to ground) from 1 kHz to 1 MHz over `steps`.
fn twin_flicker_run(steps: usize) -> NoiseAnalysisResult {
    let mut b = CircuitBuilder::new("twin");
    let gnd = b.ground();
    let top = b.node("top");
    b.device(twin_flicker(), "x1", &[("r", ParamValue::Real(1.0e3))], &[Port::Node(top.clone()), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");
    circuit.noise(options(node_id(&top), 1.0e3, 1.0e6, steps), Context::default()).expect("noise").solve().expect("solve")
}

#[test]
fn the_band_total_integrates_the_density_as_ngspice_does() {
    // Flat density: a divider of 1 kΩ over 3 kΩ at 4kT·(R1‖R2) V²/Hz over [f_first, f_last], so
    // the total is √(4kT·(R1‖R2)·(f_last − f_first)).
    let (r1, r2) = (1.0e3, 3.0e3);
    let mut b = CircuitBuilder::new("divider");
    let gnd = b.ground();
    let (top, mid) = (b.node("top"), b.node("mid"));
    add_vsrc(&mut b, "v1", 1.0, &top, &gnd);
    for (path, r, p, n) in [("r1", r1, &top, &mid), ("r2", r2, &mid, &gnd)] {
        b.device(resistor_noisy(), path, &[("r", ParamValue::Real(r))], &[Port::Node(p.clone()), Port::Node(n.clone())]);
    }
    let mut circuit = b.build().expect("build");
    let result = circuit
        .noise(options(node_id(&mid), 1.0e3, 1.0e5, 5), Context::default())
        .expect("noise")
        .solve()
        .expect("solve");
    let density = 4.0 * K_B * Tolerances::default().temperature * (r1 * r2 / (r1 + r2));
    let width = result.frequencies[4] - result.frequencies[0];
    let expected = (density * width).sqrt();
    assert!((result.output_total - expected).abs() <= 1.0e-9 * expected, "{:e} against {expected:e}", result.output_total);

    // 1/f density: a 1 kΩ resistor to ground reads `(FIRST + SECOND)/f · R²` V²/Hz, and ngspice's
    // rule integrates it exactly to `(FIRST + SECOND)·R²·ln(f_last/f_first)` on a 3-point grid,
    // where the trapezoid is off by far more than a percent.
    let twin = twin_flicker_run(3);
    let expected = ((FIRST + SECOND) * 1.0e6 * (1.0e6_f64 / 1.0e3).ln()).sqrt();
    assert!((twin.output_total - expected).abs() <= 1.0e-9 * expected, "{:e} against {expected:e}", twin.output_total);
    assert!((twin.integrated_noise - expected).abs() > 0.01 * expected, "the trapezoid total {:e} is not exact", twin.integrated_noise);
}

#[test]
fn two_inputs_sharing_a_name_sum_into_one_series() {
    let steps = 7;
    let result = twin_flicker_run(steps);
    let contributions = result.contributions();
    assert_eq!(contributions.len(), 1, "one series for the one name");
    let series = &contributions[0];
    assert_eq!((series.element.as_str(), series.source.as_str()), ("x1", "flicker"));
    assert_eq!(series.psd.len(), steps, "one value per frequency");
    // Each frequency: (FIRST + SECOND)/f A²/Hz through R = 1 kΩ.
    for (f, value) in result.frequencies.iter().zip(&series.psd) {
        let expected = (FIRST + SECOND) / f * 1.0e6;
        assert!((value - expected).abs() <= 1.0e-9 * expected, "{value:e} against {expected:e} at {f} Hz");
    }
}
