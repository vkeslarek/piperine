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
    AcSweepAnalysisOptions, BranchIdentifier, CircuitBuilder, Context, FlowOutput, NodeIdentifier, NoiseAnalysisOptions, NoiseAnalysisResult, Tolerances,
};

/// The options of the sweeps of this target.
struct Sweep;

impl Sweep {
    /// The options of a sweep from `start` to `stop` Hz over `steps` log-spaced points, output `out`.
    fn options(out: NodeIdentifier, start: f64, stop: f64, steps: usize) -> NoiseAnalysisOptions {
        NoiseAnalysisOptions {
            sweep_options: AcSweepAnalysisOptions { start_frequency: start, stop_frequency: stop, steps, logarithmic: true },
            output_node: out,
            reference_node: NodeIdentifier::Gnd,
            input_source_name: None,
            output_flow: None,
        }
    }
}

// ─── A resistor whose branch carries two flicker sources of one name ────────

/// Cells and partials of the hand-built shape of this target.
struct Cells;

impl Cells {
    fn at(kind: u32, index: u32, part: u32) -> Cell {
        Cell { kind, index, part }
    }

    fn partial(read: u32) -> Partial {
        Partial { write: 0, to: 0, read, at: 0 }
    }
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

impl TwinFlicker {
    fn shape() -> ShapeRef {
        let mut shape = ShapeBuf::new();
        shape.pin(PinSpec { flags: pin_flag::HAS_INIT, init: &1.0e3_f64.to_le_bytes(), ..PinSpec::discrete("r", role::PARAM, &F64) });
        let p = shape.pin(PinSpec::continuous("p", role::PORT));
        let n = shape.pin(PinSpec::continuous("n", role::PORT));
        let i0 = shape.input(InputSpec { name: "flicker", origin: "noise_report.rs", kind: input::NOISE });
        let i1 = shape.input(InputSpec { name: "flicker", origin: "noise_report.rs", kind: input::NOISE });
        let t0 = shape.term(TermSpec { name: "i", origin: "noise_report.rs", op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
        shape.function(FunctionSpec {
            name: "i",
            origin: "noise_report.rs",
            reads: vec![
                Cells::at(cell::PIN, p, part::VALUE),
                Cells::at(cell::PIN, n, part::VALUE),
                Cells::at(cell::PIN, 0, part::VALUE),
                Cells::at(cell::INPUT, i0, part::VALUE),
                Cells::at(cell::INPUT, i1, part::VALUE),
            ],
            writes: vec![Cells::at(cell::TERM, t0, part::VALUE)],
            partials: (0..5).map(Cells::partial).collect(),
            ..FunctionSpec::default()
        });
        for (name, input) in [("first", i0), ("second", i1)] {
            shape.function(FunctionSpec {
                name,
                origin: "noise_report.rs",
                reads: vec![Cells::at(cell::AMBIENT, ambient::FREQ, part::VALUE)],
                writes: vec![Cells::at(cell::INPUT, input, part::PSD)],
                ..FunctionSpec::default()
            });
        }
        shape.freeze()
    }
}

impl Device for TwinFlicker {
    fn prototype() -> &'static Shape<'static> {
        TWIN_SHAPE.get_or_init(TwinFlicker::shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((TwinFlicker, Arc::clone(TWIN_SHAPE.get_or_init(TwinFlicker::shape))))
    }
    fn eval(&self, function: u32, cx: &mut Eval) -> Result<(), Fault> {
        match function {
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

impl TwinFlicker {
    fn module() -> &'static Module {
        TWIN.get_or_init(|| module::<TwinFlicker>("TwinFlicker"))
    }
}

/// The partials verifier holds this target's own device to the same guard as the helpers.
#[test]
fn its_device_passes_the_partials_verifier() {
    partials::verify_every_function("TwinFlicker", Box::leak(Box::new(module::<TwinFlicker>("TwinFlicker"))));
}

impl TwinFlicker {
    /// The result of a noise run of `twin_flicker` (R = 1 kΩ to ground) from 1 kHz to 1 MHz over `steps`.
    fn run(steps: usize) -> NoiseAnalysisResult {
        let mut builder = CircuitBuilder::new("twin");
        let gnd = builder.ground();
        let top = builder.node("top");
        builder.device(TwinFlicker::module(), "x1", &[("r", ParamValue::Real(1.0e3))], &[Port::Node(top.clone()), Port::Node(gnd)]);
        let mut circuit = builder.build().expect("build");
        circuit.noise(Sweep::options(node_id(&top), 1.0e3, 1.0e6, steps), Context::default()).expect("noise").solve().expect("solve")
    }
}

#[test]
fn the_band_total_integrates_the_density_as_ngspice_does() {
    // Flat density: a divider of 1 kΩ over 3 kΩ at 4kT·(R1‖R2) V²/Hz over [f_first, f_last], so
    // the total is √(4kT·(R1‖R2)·(f_last − f_first)).
    let (r1, r2) = (1.0e3, 3.0e3);
    let mut builder = CircuitBuilder::new("divider");
    let gnd = builder.ground();
    let (top, mid) = (builder.node("top"), builder.node("mid"));
    add_vsrc(&mut builder, "v1", 1.0, &top, &gnd);
    for (path, r, p, n) in [("r1", r1, &top, &mid), ("r2", r2, &mid, &gnd)] {
        builder.device(resistor_noisy(), path, &[("r", ParamValue::Real(r))], &[Port::Node(p.clone()), Port::Node(n.clone())]);
    }
    let mut circuit = builder.build().expect("build");
    let result = circuit
        .noise(Sweep::options(node_id(&mid), 1.0e3, 1.0e5, 5), Context::default())
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
    let twin = TwinFlicker::run(3);
    let expected = ((FIRST + SECOND) * 1.0e6 * (1.0e6_f64 / 1.0e3).ln()).sqrt();
    assert!((twin.output_total - expected).abs() <= 1.0e-9 * expected, "{:e} against {expected:e}", twin.output_total);
    assert!((twin.integrated_noise - expected).abs() > 0.01 * expected, "the trapezoid total {:e} is not exact", twin.integrated_noise);
}

#[test]
fn two_inputs_sharing_a_name_sum_into_one_series() {
    let steps = 7;
    let result = TwinFlicker::run(steps);
    let contributions = result.contributions();
    assert_eq!(contributions.len(), 1, "one series for the one name");
    let series = &contributions[0];
    assert_eq!((series.element.as_str(), series.source.as_str()), ("x1", "flicker"));
    assert_eq!(series.psd.len(), steps, "one value per frequency");
    // Each frequency: (FIRST + SECOND)/f A²/Hz through R = 1 kΩ.
    for (frequency, value) in result.frequencies.iter().zip(&series.psd) {
        let expected = (FIRST + SECOND) / frequency * 1.0e6;
        assert!((value - expected).abs() <= 1.0e-9 * expected, "{value:e} against {expected:e} at {frequency} Hz");
    }
}

// ─── Input-referred noise ───────────────────────────────────────────────────

/// The noise run of a noisy resistor in parallel with a capacitor, driven by a current input.
struct RcWithCurrentInput;

impl RcWithCurrentInput {
    /// A 1 kΩ noisy resistor in parallel with a capacitor of 1.5915 nF (pole at 100 kHz) from `out`
    /// to ground, an AC current source `iin` injecting 1 A into `out`. Returns the circuit builder's
    /// result for a noise run from 1 kHz to 1 MHz over `steps` points with `input` as the input.
    fn run(input: Option<&str>, steps: usize) -> Result<NoiseAnalysisResult, String> {
        let mut builder = CircuitBuilder::new("rc");
        let gnd = builder.ground();
        let out = builder.node("out");
        builder.device(abi_devices::current_ac(), "iin", &[("i", ParamValue::Real(1.0))], &[Port::Node(out.clone())]);
        builder.device(resistor_noisy(), "r1", &[("r", ParamValue::Real(1.0e3))], &[Port::Node(out.clone()), Port::Node(gnd.clone())]);
        abi_devices::add_capacitor(&mut builder, "c1", 1.5915494309189535e-9, &out, &gnd);
        let mut circuit = builder.build().expect("build");
        let mut noise_options = Sweep::options(node_id(&out), 1.0e3, 1.0e6, steps);
        noise_options.input_source_name = input.map(str::to_string);
        let mut solver = circuit.noise(noise_options, Context::default()).map_err(|e| e.to_string())?;
        solver.solve().map_err(|e| e.to_string())
    }
}

#[test]
fn the_input_referred_density_divides_by_the_transfer_from_the_input() {
    // The transfer from the input current to v(out) is Z = R/(1 + jωRC), and the resistor's
    // current density 4kT/R gives onoise² = 4kT/R·|Z|², so inoise² = onoise²/|Z|² = 4kT/R A²/Hz at
    // every frequency, flat, and its band total is √(4kT/R·(f_last − f_first)).
    let (resistance, capacitance) = (1.0e3, 1.5915494309189535e-9);
    let result = RcWithCurrentInput::run(Some("iin"), 7).expect("noise");
    let current_density = 4.0 * K_B * Tolerances::default().temperature / resistance;
    let input_noise_sq = result.input_noise_sq.as_ref().expect("an input gives inoise");
    for (point, frequency) in result.frequencies.iter().enumerate() {
        let impedance_squared = resistance * resistance / (1.0 + (2.0 * std::f64::consts::PI * frequency * resistance * capacitance).powi(2));
        let expected_out = current_density * impedance_squared;
        assert!((result.out_noise_sq[point] - expected_out).abs() <= 1.0e-9 * expected_out, "onoise² at {frequency} Hz");
        assert!((input_noise_sq[point] - current_density).abs() <= 1.0e-9 * current_density, "inoise² at {frequency} Hz");
    }
    let width = result.frequencies[6] - result.frequencies[0];
    let expected_total = (current_density * width).sqrt();
    let total = result.input_total.expect("an input gives inoise_total");
    assert!((total - expected_total).abs() <= 1.0e-9 * expected_total, "{total:e} against {expected_total:e}");

    // Without an input the result has no input-referred quantity.
    let alone = RcWithCurrentInput::run(None, 7).expect("noise");
    assert!(alone.input_noise_sq.is_none() && alone.input_total.is_none());
}

#[test]
fn an_input_with_no_stimulus_is_refused_by_name() {
    // `r1` writes no `STIM` input, so no transfer exists to refer the noise to.
    let message = RcWithCurrentInput::run(Some("r1"), 3).err().expect("no stimulus");
    assert!(message.contains("r1") && message.contains("stimulus"), "{message}");
    let message = RcWithCurrentInput::run(Some("nope"), 3).err().expect("no such instance");
    assert!(message.contains("nope"), "{message}");
}

// ─── Flow and differential outputs ──────────────────────────────────────────

/// The noise run of a noisy resistor shorted by a 0 V source.
struct ShortedResistor;

impl ShortedResistor {
    /// A noisy 1 kΩ resistor `r1` from `top` to ground with a 0 V source `v1` across it, so the
    /// resistor's whole noise current goes through the source. Runs from 1 kHz to 100 kHz over 5
    /// points with the flow output `flow`.
    fn run(flow: FlowOutput) -> NoiseAnalysisResult {
        let mut builder = CircuitBuilder::new("short");
        let gnd = builder.ground();
        let top = builder.node("top");
        add_vsrc(&mut builder, "v1", 0.0, &top, &gnd);
        builder.device(resistor_noisy(), "r1", &[("r", ParamValue::Real(1.0e3))], &[Port::Node(top.clone()), Port::Node(gnd.clone())]);
        let mut circuit = builder.build().expect("build");
        let mut noise_options = Sweep::options(node_id(&top), 1.0e3, 1.0e5, 5);
        noise_options.output_flow = Some(flow);
        circuit.noise(noise_options, Context::default()).expect("noise").solve().expect("solve")
    }
}

#[test]
fn a_flow_output_reads_amperes_per_root_hertz() {
    // The 0 V source holds v(top) = 0, so the resistor's noise current 4kT/R A²/Hz flows whole
    // through the source (branch `f` of `v1`) and through the resistor's own term `i`
    // (value = v/r + noise, v = 0): both read √(4kT/R) A/√Hz at every frequency.
    let expected = (4.0 * K_B * Tolerances::default().temperature / 1.0e3).sqrt();
    for flow in [FlowOutput::Branch(BranchIdentifier::new("v1", "f")), FlowOutput::Term { instance: "r1".into(), term: "i".into() }] {
        let result = ShortedResistor::run(flow.clone());
        for (point, density) in result.out_noise_sq.iter().enumerate() {
            let amperes = density.sqrt();
            assert!((amperes - expected).abs() <= 1.0e-9 * expected, "{flow:?} point {point}: {amperes:e} against {expected:e}");
        }
    }
}

/// The noise run of the loop of three noisy resistors.
struct ResistorLoop;

impl ResistorLoop {
    /// The three-resistor loop ground–R1–a–R2–b–R3–ground (1 kΩ, 2 kΩ, 3 kΩ), noisy, from 1 kHz to
    /// 100 kHz over `steps` points, with `configure` applied to its options.
    fn run(steps: usize, configure: impl FnOnce(&mut NoiseAnalysisOptions, &[NodeIdentifier; 2])) -> NoiseAnalysisResult {
        let mut builder = CircuitBuilder::new("loop");
        let gnd = builder.ground();
        let (a, node_b) = (builder.node("a"), builder.node("b"));
        for (path, r, p, n) in [("r1", 1.0e3, &gnd, &a), ("r2", 2.0e3, &a, &node_b), ("r3", 3.0e3, &node_b, &gnd)] {
            builder.device(resistor_noisy(), path, &[("r", ParamValue::Real(r))], &[Port::Node(p.clone()), Port::Node(n.clone())]);
        }
        let mut circuit = builder.build().expect("build");
        let nodes = [node_id(&a), node_id(&node_b)];
        let mut noise_options = Sweep::options(nodes[0].clone(), 1.0e3, 1.0e5, steps);
        configure(&mut noise_options, &nodes);
        circuit.noise(noise_options, Context::default()).expect("noise").solve().expect("solve")
    }
}

#[test]
fn a_term_output_adds_its_direct_noise_to_the_circuit_path() {
    // The current of R2's term `i` in the loop below is `G2·(v(a) − v(b)) + n2`. With G = 1/R and
    // det = G1·G2 + G1·G3 + G2·G3, the transfers of the three noise currents (each entering the
    // loop at the term's far node, R2's own with its direct partial 1 added) are
    //   R1: G2·G3/det,  R2: 1 − G2·(G1 + G3)/det = G1·G3/det,  R3: G1·G2/det.
    // A wrong relative sign of the direct partial would give R2: (det + G2·(G1 + G3))/det.
    let g = [1.0e-3, 0.5e-3, 1.0 / 3.0e3];
    let det = g[0] * g[1] + g[0] * g[2] + g[1] * g[2];
    let transfers = [g[1] * g[2] / det, g[0] * g[2] / det, g[0] * g[1] / det];
    let expected: f64 =
        (0..3).map(|index| 4.0 * K_B * Tolerances::default().temperature * g[index] * transfers[index] * transfers[index]).sum();
    let result = ResistorLoop::run(3, |noise_options, _| noise_options.output_flow = Some(FlowOutput::Term { instance: "r2".into(), term: "i".into() }));
    for (point, got) in result.out_noise_sq.iter().enumerate() {
        assert!((got - expected).abs() <= 1.0e-9 * expected, "point {point}: {got:e} against {expected:e}");
    }
}

#[test]
fn a_differential_output_reads_the_pair() {
    // A loop ground–R1–a–R2–b–R3–ground (1 kΩ, 2 kΩ, 3 kΩ), output v(a) − v(b). With G = 1/R and
    // Y = [[G1+G2, −G2], [−G2, G2+G3]] over (a, b), det = (G1+G2)(G2+G3) − G2², each resistor's
    // noise current (density 4kT·G) enters the loop as e1 = (1, 0), e2 = (1, −1), e3 = (0, 1)
    // and Y⁻¹ = [[G2+G3, G2], [G2, G1+G2]]/det gives the transfers to v(a) − v(b):
    //   R1: G3/det,  R2: (G1+G3)/det,  R3: −G1/det.
    // The single-ended v(a) has the transfers (G2+G3)/det, G3/det and G2/det instead.
    let resistances = [1.0e3, 2.0e3, 3.0e3];
    let g: Vec<f64> = resistances.iter().map(|r| 1.0 / r).collect();
    let det = (g[0] + g[1]) * (g[1] + g[2]) - g[1] * g[1];
    let thermal = 4.0 * K_B * Tolerances::default().temperature;
    let density = |transfers: [f64; 3]| (0..3).map(|index| thermal * g[index] * (transfers[index] / det).powi(2)).sum::<f64>();
    let differential = density([g[2], g[0] + g[2], -g[0]]);
    let single_ended = density([g[1] + g[2], g[2], g[1]]);
    assert!((differential - single_ended).abs() > 0.05 * single_ended, "the two outputs must differ");

    let result = ResistorLoop::run(3, |noise_options, nodes| noise_options.reference_node = nodes[1].clone());
    for (point, got) in result.out_noise_sq.iter().enumerate() {
        assert!((got - differential).abs() <= 1.0e-9 * differential, "point {point}: {got:e} against {differential:e}");
    }
    let alone = ResistorLoop::run(3, |_, _| {});
    assert!((alone.out_noise_sq[0] - single_ended).abs() <= 1.0e-9 * single_ended, "{:e} against {single_ended:e}", alone.out_noise_sq[0]);
}
