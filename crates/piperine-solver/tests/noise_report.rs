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

// ─── Input-referred noise ───────────────────────────────────────────────────

/// A 1 kΩ noisy resistor in parallel with a capacitor of 1.5915 nF (pole at 100 kHz) from `out`
/// to ground, an AC current source `iin` injecting 1 A into `out`. Returns the circuit builder's
/// result for a noise run from 1 kHz to 1 MHz over `steps` points with `input` as the input.
fn rc_with_current_input(input: Option<&str>, steps: usize) -> Result<NoiseAnalysisResult, String> {
    let mut b = CircuitBuilder::new("rc");
    let gnd = b.ground();
    let out = b.node("out");
    b.device(abi_devices::current_ac(), "iin", &[("i", ParamValue::Real(1.0))], &[Port::Node(out.clone())]);
    b.device(resistor_noisy(), "r1", &[("r", ParamValue::Real(1.0e3))], &[Port::Node(out.clone()), Port::Node(gnd.clone())]);
    abi_devices::add_capacitor(&mut b, "c1", 1.5915494309189535e-9, &out, &gnd);
    let mut circuit = b.build().expect("build");
    let mut noise_options = options(node_id(&out), 1.0e3, 1.0e6, steps);
    noise_options.input_source_name = input.map(str::to_string);
    let mut solver = circuit.noise(noise_options, Context::default()).map_err(|e| e.to_string())?;
    solver.solve().map_err(|e| e.to_string())
}

#[test]
fn the_input_referred_density_divides_by_the_transfer_from_the_input() {
    // The transfer from the input current to v(out) is Z = R/(1 + jωRC), and the resistor's
    // current density 4kT/R gives onoise² = 4kT/R·|Z|², so inoise² = onoise²/|Z|² = 4kT/R A²/Hz at
    // every frequency, flat, and its band total is √(4kT/R·(f_last − f_first)).
    let (r, c) = (1.0e3, 1.5915494309189535e-9);
    let result = rc_with_current_input(Some("iin"), 7).expect("noise");
    let current_density = 4.0 * K_B * Tolerances::default().temperature / r;
    let input_noise_sq = result.input_noise_sq.as_ref().expect("an input gives inoise");
    for (i, f) in result.frequencies.iter().enumerate() {
        let z_sq = r * r / (1.0 + (2.0 * std::f64::consts::PI * f * r * c).powi(2));
        let expected_out = current_density * z_sq;
        assert!((result.out_noise_sq[i] - expected_out).abs() <= 1.0e-9 * expected_out, "onoise² at {f} Hz");
        assert!((input_noise_sq[i] - current_density).abs() <= 1.0e-9 * current_density, "inoise² at {f} Hz");
    }
    let width = result.frequencies[6] - result.frequencies[0];
    let expected_total = (current_density * width).sqrt();
    let total = result.input_total.expect("an input gives inoise_total");
    assert!((total - expected_total).abs() <= 1.0e-9 * expected_total, "{total:e} against {expected_total:e}");

    // Without an input the result has no input-referred quantity.
    let alone = rc_with_current_input(None, 7).expect("noise");
    assert!(alone.input_noise_sq.is_none() && alone.input_total.is_none());
}

#[test]
fn an_input_with_no_stimulus_is_refused_by_name() {
    // `r1` writes no `STIM` input, so no transfer exists to refer the noise to.
    let err = rc_with_current_input(Some("r1"), 3).err().expect("no stimulus");
    assert!(err.contains("r1") && err.contains("stimulus"), "{err}");
    let err = rc_with_current_input(Some("nope"), 3).err().expect("no such instance");
    assert!(err.contains("nope"), "{err}");
}
