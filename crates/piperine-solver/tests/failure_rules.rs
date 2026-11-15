//! LRM-COVERS: 7.3.2.1
//!
//! The normative failure rules of SPEC Part VII §16, enforced.
//!
//! Each rule in that table says a specific condition SHALL be an analysis or
//! device-load error. P6 found most of them had a failure site in the code but
//! no test reaching it (CLN-16/17), so this suite trips them: every test names
//! the `§n` row it enforces and asserts the typed `SolverDomain` plus the
//! message fragment that identifies the rule. (`Error` is
//! `{Simple,WithCause}{domain, detail}` — domain + fragment is the strongest
//! typed assertion the taxonomy offers.)

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;
#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use std::result::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use abi_devices::add_resistor;
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, cell, op, part, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{Error, Port, SolverDomain};
use piperine_solver::prelude::*;

const ORIGIN: &str = "failure_rules.rs";

// ─── Fixtures ─────────────────────────────────────────────────────────────────

fn cell_of(kind: u32, index: u32) -> Cell {
    Cell { kind, index, part: part::VALUE }
}

/// The shape of a two-port device `I(p, n) = f(V(p, n))`: one `FLOW` term and
/// the partials against both ports.
fn two_port_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
    s.function(FunctionSpec {
        name: "i",
        origin: ORIGIN,
        reads: vec![cell_of(cell::PIN, p), cell_of(cell::PIN, n)],
        writes: vec![cell_of(cell::TERM, t0)],
        partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }],
        ..FunctionSpec::default()
    });
    s.freeze()
}

/// The §15 trigger: a contribution that is not finite.
struct NanSource;
static NAN_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static NAN: OnceLock<Module> = OnceLock::new();

impl Device for NanSource {
    fn prototype() -> &'static Shape<'static> {
        NAN_SHAPE.get_or_init(two_port_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((NanSource, Arc::clone(NAN_SHAPE.get_or_init(two_port_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        cx.set(0, f64::NAN);
        cx.jac().copy_from_slice(&[1.0, -1.0]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A pathological device: its conductance flips sign every evaluation, so no
/// Newton iteration can settle and no homotopy ramp helps. Nothing in the
/// stdlib behaves like this — it exists to reach §9's "every strategy failed"
/// path, which is otherwise only reachable by a real convergence disaster.
struct Oscillator {
    flip: AtomicBool,
}
static OSC_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static OSC: OnceLock<Module> = OnceLock::new();

impl Device for Oscillator {
    fn prototype() -> &'static Shape<'static> {
        OSC_SHAPE.get_or_init(two_port_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((Oscillator { flip: AtomicBool::new(false) }, Arc::clone(OSC_SHAPE.get_or_init(two_port_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let flip = !self.flip.fetch_xor(true, Ordering::SeqCst);
        let g = if flip { 1e-3 } else { 1e3 };
        let rhs = if flip { 1.0 } else { -1.0 };
        let v = cx.f64(0) - cx.f64(1);
        cx.set(0, g * v - rhs);
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A one-resistor circuit between node 1 and ground.
fn resistor_circuit() -> CircuitInstance {
    let mut b = CircuitBuilder::new("rules");
    let gnd = b.ground();
    let a = b.node("a");
    add_resistor(&mut b, "r", 1000.0, &a, &gnd);
    b.build().expect("build")
}

/// The identifier of the circuit's one node.
fn node_a(circuit: &CircuitInstance) -> NodeIdentifier {
    circuit
        .netlist()
        .all_references()
        .into_iter()
        .find_map(|r| match r.variable().as_ref() {
            piperine_solver::abi::AnalogVariable::Node(id) if *id != NodeIdentifier::Gnd => Some(id.clone()),
            _ => None,
        })
        .expect("the node")
}

fn sweep() -> AcSweepAnalysisOptions {
    AcSweepAnalysisOptions {
        start_frequency: 1.0,
        stop_frequency: 10.0,
        steps: 2,
        logarithmic: false,
    }
}

/// Assert an error carries `domain` and mentions `fragment`.
fn assert_rule(err: &Error, domain: SolverDomain, fragment: &str) {
    let (actual, text) = match err {
        Error::Simple { domain, detail } => (*domain, detail.clone()),
        Error::WithCause { domain, detail, cause } => {
            (*domain, format!("{detail}: {cause}"))
        }
    };
    assert_eq!(actual, domain, "wrong domain for `{fragment}`: {err}");
    assert!(
        text.to_ascii_lowercase().contains(&fragment.to_ascii_lowercase()),
        "message must identify the rule (`{fragment}`): {text}"
    );
}

// ─── §12 — noise output/reference node cannot be resolved ─────────────────────

#[test]
fn section_12_unresolvable_noise_output_node_is_an_analysis_error() {
    let mut circuit = resistor_circuit();
    let options = NoiseAnalysisOptions {
        sweep_options: sweep(),
        output_node: NodeIdentifier::Anonymous(99),
        reference_node: NodeIdentifier::Gnd,
        input_source_name: None,
    };
    let err = circuit
        .noise(options, Context::default())
        .and_then(|mut n| n.solve())
        .map(|_| ())
        .expect_err("§12: an unresolvable output node must fail");
    assert_rule(&err, SolverDomain::Noise, "output node");
}

#[test]
fn section_12_unresolvable_noise_reference_node_is_an_analysis_error() {
    let mut circuit = resistor_circuit();
    let options = NoiseAnalysisOptions {
        sweep_options: sweep(),
        output_node: node_a(&circuit),
        reference_node: NodeIdentifier::Anonymous(98),
        input_source_name: None,
    };
    let err = circuit
        .noise(options, Context::default())
        .and_then(|mut n| n.solve())
        .map(|_| ())
        .expect_err("§12: an unresolvable reference node must fail");
    assert_rule(&err, SolverDomain::Noise, "reference node");
}

// ─── §15 — a linear solve returning NaN or infinity ───────────────────────────
// ─── §7.3.2.1 (T35/LRMW3-18) — a non-finite contribution names the branch ─────

/// T35/LRMW3-18: retargets this test in place (never deleted) once the
/// non-finite check moved from `math/newton_raphson.rs`'s post-solve guard
/// (which only sees the assembled solution vector, with no way back to
/// which device produced a bad entry) to `analyses::non_finite_stamp_error`
/// — checked per-device, right where `NanSource::load_dc`'s own stamps are
/// collected (`analyses/dc.rs`'s `assemble`), before they are summed into
/// the shared matrix/RHS alongside every other device. The instance path
/// ("nan") is what makes the branch nameable.
#[test]
fn section_15_a_non_finite_stamp_is_a_convergence_failure() {
    let mut b = CircuitBuilder::new("nan");
    let gnd = b.ground();
    let a = b.node("a");
    add_resistor(&mut b, "r", 1000.0, &a, &gnd);
    b.device(NAN.get_or_init(|| module::<NanSource>("NanSource")), "nan", &[], &[Port::Node(a), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");

    let err = circuit
        .dc(Context::default())
        .and_then(|mut dc| dc.solve())
        .map(|_| ())
        .expect_err("§15/§7.3.2.1: a NaN contribution must fail, never converge");
    assert_rule(&err, SolverDomain::Device, "§7.3.2.1");
    let text = err.to_string();
    assert!(
        text.contains("instance `nan`"),
        "§7.3.2.1's message must name the offending instance (`nan`): {err}"
    );
    assert!(text.contains(ORIGIN), "§7.3.2.1's message must name the cell's origin ({ORIGIN}): {err}");
}

// ─── §18 — non-positive period, negative pre-roll ─────────────────────────────

fn pss_options(period: f64, tstab: f64) -> PssAnalysisOptions {
    PssAnalysisOptions { period, tstab, max_shoot_iter: 4, shoot_tol: 1e-6, dt: None }
}

#[test]
fn section_18_a_non_positive_period_is_an_analysis_error() {
    for period in [0.0, -1e-6] {
        let mut circuit = resistor_circuit();
        let err = circuit
            .pss(pss_options(period, 0.0), Context::default())
            .and_then(|p| p.solve())
            .map(|_| ())
            .expect_err("§18: a non-positive period must fail");
        assert_rule(&err, SolverDomain::Pss, "period must be positive");
    }
}

#[test]
fn section_18_a_negative_pre_roll_is_an_analysis_error() {
    let mut circuit = resistor_circuit();
    let err = circuit
        .pss(pss_options(1e-3, -1.0), Context::default())
        .and_then(|p| p.solve())
        .map(|_| ())
        .expect_err("§18: a negative pre-roll must fail");
    assert_rule(&err, SolverDomain::Pss, "tstab");
}

// ─── §9 — DC fails plain Newton, gmin stepping, and source stepping ────────────

#[test]
fn section_9_exhausting_every_dc_strategy_is_a_convergence_failure() {
    let mut b = CircuitBuilder::new("osc");
    let gnd = b.ground();
    let a = b.node("a");
    add_resistor(&mut b, "r", 1e6, &a, &gnd);
    b.device(OSC.get_or_init(|| module::<Oscillator>("Oscillator")), "osc", &[], &[Port::Node(a), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");

    let err = circuit
        .dc(Context::default())
        .and_then(|mut dc| dc.solve())
        .map(|_| ())
        .expect_err("§9: plain Newton, gmin stepping and source stepping all fail here");
    // The exhausted plan reports its last attempt, so the domain is `Newton`
    // rather than `Dc` — §16 calls this a "convergence failure" without fixing
    // the domain, and this is the domain the code actually uses.
    assert_rule(&err, SolverDomain::Newton, "converge");
}

/// SAI-15: every function of this target's own devices passes the partials
/// verifier (the guard of `abi_devices.rs` holds each device to it).
#[test]
fn its_devices_pass_the_partials_verifier() {
    use partials::Fixture;
    // The fixture of the non-finite rule: its value is NaN at every point.
    partials::verify_every_function_except("NanSource", NAN.get_or_init(|| module::<NanSource>("NanSource")), &[(0, Fixture::NonFinite)]);
    // The fixture of "every strategy failed": its conductance flips at every call.
    partials::verify_every_function_except("Oscillator", OSC.get_or_init(|| module::<Oscillator>("Oscillator")), &[(0, Fixture::Impure)]);
}
