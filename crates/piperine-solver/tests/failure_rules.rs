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
#[path = "helpers/abi_mixed.rs"]
mod abi_mixed;
#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use std::result::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use abi_devices::{add_capacitor, add_resistor, add_vsrc};
use piperine_abi::device::{Device, Eval, Fault, Flow, React, Values};
use piperine_abi::layout::BIT;
use piperine_abi::raw::{Cell, DEFAULT, Module, Partial, Shape, ambient, cell, op, part, role};
use piperine_abi::shape::{FunctionSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_abi::thunk::module;
use piperine_solver::abi::{Error, Port, SolverDomain};
use piperine_analog_solver::error::RunCode;
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
        Error::Run(run) => (run.domain, run.to_string()),
        Error::Simple { domain, detail } => (*domain, detail.clone()),
        Error::Unsettled { domain, .. } => (*domain, err.to_string()),
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
        output_flow: None,
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
        output_flow: None,
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
    assert_rule(&err, SolverDomain::Device, "E0507");
    let text = err.to_string();
    assert!(
        text.contains("instance `nan`"),
        "E0507's message must name the offending instance (`nan`): {err}"
    );
    assert!(text.contains(ORIGIN), "E0507's message must name the cell's origin ({ORIGIN}): {err}");
}

/// HYG-02 AC 3: the error is `E0507`, names the instance, and cites no LRM section.
#[test]
fn a_non_finite_contribution_is_e0507_and_names_the_instance() {
    let mut b = CircuitBuilder::new("nan");
    let gnd = b.ground();
    let a = b.node("a");
    add_resistor(&mut b, "r", 1000.0, &a, &gnd);
    b.device(NAN.get_or_init(|| module::<NanSource>("NanSource")), "nan", &[], &[Port::Node(a), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");

    let err = dc_error(&mut circuit);
    assert_eq!(err.code(), Some(RunCode::E0507), "{err}");
    let text = err.to_string();
    assert!(text.contains("instance `nan`"), "the instance is named: {err}");
    assert!(!text.contains("7.3.2.1"), "no LRM section is cited: {err}");
}

// ─── §18 — non-positive period, negative pre-roll ─────────────────────────────

fn pss_options(period: f64, tstab: f64) -> PssAnalysisOptions {
    PssAnalysisOptions { period, tstab, max_shoot_iter: 4, shoot_tol: 1e-6, dt: None, dt_max: None, max_steps: None }
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

/// HYG-03 AC 4: an equilibrium no strategy settles is `E0505`, naming the
/// unknowns of the last attempt that failed the update or residual test.
#[test]
fn an_equilibrium_that_never_converges_is_e0505_with_its_unknowns() {
    let mut b = CircuitBuilder::new("osc");
    let gnd = b.ground();
    let a = b.node("a");
    add_resistor(&mut b, "r", 1e6, &a, &gnd);
    b.device(OSC.get_or_init(|| module::<Oscillator>("Oscillator")), "osc", &[], &[Port::Node(a), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");

    let err = dc_error(&mut circuit);
    assert_eq!(err.code(), Some(RunCode::E0505), "{err}");
    let Error::Run(run) = &err else { panic!("a run error, got {err}") };
    assert_eq!(run.names, vec!["node of r.p, osc.p".to_string()], "the one unknown that never settles: {err}");
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
    // The fixtures of the step floor: their conductance flips once they trip.
    // (checked at time 0, where it is a plain resistor)
    partials::verify_every_function_with("LateOscillator", LATE.get_or_init(|| module::<LateOscillator>("LateOscillator")), &[(0, 2, 0.0)]);
    // (checked at time 0, where the jump fixture is a plain resistor)
    partials::verify_every_function_with("JumpOscillator", JUMP.get_or_init(|| module::<JumpOscillator>("JumpOscillator")), &[(0, 3, 0.0)]);
    // (checked at time 0, where the late source holds its branch at zero)
    partials::verify_every_function_with("LateSource", LATE_SOURCE.get_or_init(|| module::<LateSource>("LateSource")), &[(0, 3, 0.0)]);
    // (checked at frequency 0, where it is a plain conductance)
    partials::verify_every_function_with("OpenAboveDc", OPEN.get_or_init(|| module::<OpenAboveDc>("OpenAboveDc")), &[(0, 2, 0.0)]);
}

// ─── §17.1 E0503 — a singular solve names the unknowns of the singular part ───

/// The error of the operating point of `circuit`.
fn dc_error(circuit: &mut CircuitInstance) -> Error {
    circuit
        .dc(Context::default())
        .and_then(|mut dc| dc.solve())
        .map(|_| ())
        .expect_err("a singular circuit must fail")
}

/// HYG-01 AC 1: a node reached only through capacitors stamps nothing at DC;
/// the error is `E0503` and names the node by the pins bound to it.
#[test]
fn a_singular_solve_names_the_pins_of_its_node() {
    let mut b = CircuitBuilder::new("floating");
    let gnd = b.ground();
    let a = b.node("a");
    add_capacitor(&mut b, "c1", 1e-9, &a, &gnd);
    add_capacitor(&mut b, "c2", 2e-9, &a, &gnd);
    let mut circuit = b.build().expect("build");

    let err = dc_error(&mut circuit);
    assert_eq!(err.code(), Some(RunCode::E0503), "{err}");
    let Error::Run(run) = &err else { panic!("a run error, got {err}") };
    assert_eq!(run.names, vec!["node of c1.p, c2.p".to_string()], "{err}");
}

/// HYG-01 AC 1: two potential sources across the same pair of nodes are a
/// loop; both branch flows are the singular part.
#[test]
fn a_loop_of_sources_names_both_instances() {
    let mut b = CircuitBuilder::new("loop");
    let gnd = b.ground();
    let a = b.node("a");
    add_vsrc(&mut b, "v1", 1.0, &a, &gnd);
    add_vsrc(&mut b, "v2", 2.0, &a, &gnd);
    let mut circuit = b.build().expect("build");

    let err = dc_error(&mut circuit);
    assert_eq!(err.code(), Some(RunCode::E0503), "{err}");
    let Error::Run(run) = &err else { panic!("a run error, got {err}") };
    assert_eq!(
        run.names,
        vec!["flow of v1 (f)".to_string(), "flow of v2 (f)".to_string()],
        "both flows, each instance once: {err}"
    );
}

/// HYG-01 edge case 2: an instance with two pins on the singular node is named
/// once, its pins joined.
#[test]
fn an_instance_is_named_once_in_a_singular_report() {
    let mut b = CircuitBuilder::new("selfloop");
    let gnd = b.ground();
    let a = b.node("a");
    add_capacitor(&mut b, "c1", 1e-9, &a, &gnd);
    add_capacitor(&mut b, "c2", 2e-9, &a, &a);
    let mut circuit = b.build().expect("build");

    let err = dc_error(&mut circuit);
    let Error::Run(run) = &err else { panic!("a run error, got {err}") };
    assert_eq!(run.names, vec!["node of c1.p, c2.p+n".to_string()], "{err}");
    assert_eq!(err.to_string().matches("c2").count(), 1, "{err}");
}

// ─── §17.1 E0508 / E0504 — a step that fails Newton at the floor ──────────────

/// A resistor that turns into the oscillator (its conductance flips sign every
/// evaluation, so no Newton iteration settles) once it trips: after 2e-7 s
/// (`LateOscillator`) or while its `go` pin is HIGH in a run (`JumpOscillator`).
struct LateOscillator {
    flip: AtomicBool,
}
struct JumpOscillator {
    flip: AtomicBool,
}
static LATE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static LATE: OnceLock<Module> = OnceLock::new();
static JUMP_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static JUMP: OnceLock<Module> = OnceLock::new();

/// pins: p, n (PORT continuous), go (PORT BIT, jump variant only); one FLOW term.
/// The function reads [p, n, TIME] or [p, n, go, TIME].
fn tripping_shape(reads_go: bool) -> ShapeRef {
    let mut s = ShapeBuf::new();
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let time = Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE };
    let mut extra = Vec::new();
    if reads_go {
        let go = s.pin(PinSpec::discrete("go", role::PORT, &BIT));
        extra.push(cell_of(cell::PIN, go));
    }
    extra.push(time);
    let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
    s.function(FunctionSpec {
        name: "i",
        origin: ORIGIN,
        reads: [vec![cell_of(cell::PIN, p), cell_of(cell::PIN, n)], extra].concat(),
        writes: vec![cell_of(cell::TERM, t0)],
        partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }],
        ..FunctionSpec::default()
    });
    s.freeze()
}

/// The current of a resistor of 1 kOhm, or of the oscillator when `trips`.
fn tripping_current(flip: &AtomicBool, trips: bool, cx: &mut Eval) {
    let v = cx.f64(0) - cx.f64(1);
    let (g, rhs) = if trips {
        let flipped = !flip.fetch_xor(true, Ordering::SeqCst);
        if flipped { (1e-3, 1.0) } else { (1e3, -1.0) }
    } else {
        (1e-3, 0.0)
    };
    cx.set(0, g * v - rhs);
    cx.jac().copy_from_slice(&[g, -g]);
}

impl Device for LateOscillator {
    fn prototype() -> &'static Shape<'static> {
        LATE_SHAPE.get_or_init(|| tripping_shape(false)).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((LateOscillator { flip: AtomicBool::new(false) }, Arc::clone(LATE_SHAPE.get_or_init(|| tripping_shape(false)))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let trips = cx.f64(2) > 2.0e-7;
        tripping_current(&self.flip, trips, cx);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

impl Device for JumpOscillator {
    fn prototype() -> &'static Shape<'static> {
        JUMP_SHAPE.get_or_init(|| tripping_shape(true)).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((JumpOscillator { flip: AtomicBool::new(false) }, Arc::clone(JUMP_SHAPE.get_or_init(|| tripping_shape(true)))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        // `go` HIGH once the run is under way (the operating point is at time 0).
        let trips = cx.bytes(2)[0] != 0 && cx.f64(3) > 0.0;
        tripping_current(&self.flip, trips, cx);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A potential source that exists only from `LATE_SOURCE_AT` on: before it the branch carries no
/// current (`f = 0`), after it `V(p, n) = 1 V` is imposed, so a second source across the same nodes
/// makes the Jacobian singular at a transient step while the operating point is fine.
/// pins: p=0, n=1 (PORT); f=2 (PRIVATE branch current)
/// terms: T0 FLOW{p, n} "i" = f, T1 FLOW{f, ground} "eq"
/// fn 0 "v": reads [p, n, f, TIME] writes [T0, T1] partials (0<-f) (1<-p) (1<-n) (1<-f)
struct LateSource;
static LATE_SOURCE_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static LATE_SOURCE: OnceLock<Module> = OnceLock::new();
const LATE_SOURCE_AT: f64 = 2.0e-7;

fn late_source_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let f = s.pin(PinSpec::continuous("f", role::PRIVATE));
    let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
    let t1 = s.term(TermSpec { name: "eq", origin: ORIGIN, op: op::FLOW, plus: f, minus: piperine_abi::raw::GROUND, abstol: DEFAULT, max: DEFAULT });
    let time = Cell { kind: cell::AMBIENT, index: ambient::TIME, part: part::VALUE };
    s.function(FunctionSpec {
        name: "v",
        origin: ORIGIN,
        reads: vec![cell_of(cell::PIN, p), cell_of(cell::PIN, n), cell_of(cell::PIN, f), time],
        writes: vec![cell_of(cell::TERM, t0), cell_of(cell::TERM, t1)],
        partials: vec![
            Partial { write: 0, to: 0, read: 2, at: 0 },
            Partial { write: 1, to: 0, read: 0, at: 0 },
            Partial { write: 1, to: 0, read: 1, at: 0 },
            Partial { write: 1, to: 0, read: 2, at: 0 },
        ],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for LateSource {
    fn prototype() -> &'static Shape<'static> {
        LATE_SOURCE_SHAPE.get_or_init(late_source_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((LateSource, Arc::clone(LATE_SOURCE_SHAPE.get_or_init(late_source_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let (vp, vn, f, t) = (cx.f64(0), cx.f64(1), cx.f64(2), cx.f64(3));
        cx.set(0, f);
        if t > LATE_SOURCE_AT {
            cx.set(1, vp - vn - 1.0);
            cx.jac().copy_from_slice(&[1.0, 1.0, -1.0, 0.0]);
        } else {
            cx.set(1, f);
            cx.jac().copy_from_slice(&[1.0, 0.0, 0.0, 1.0]);
        }
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// A 1 mS conductance at DC that is open at any frequency: a node it alone holds is fine at the
/// operating point and has a zero row in the small-signal system.
/// pins: p=0, n=1 (PORT); fn 0 "i": reads [p, n, FREQ] writes [T0] partials (0<-p) (0<-n)
struct OpenAboveDc;
static OPEN_SHAPE: OnceLock<ShapeRef> = OnceLock::new();
static OPEN: OnceLock<Module> = OnceLock::new();

fn open_shape() -> ShapeRef {
    let mut s = ShapeBuf::new();
    let p = s.pin(PinSpec::continuous("p", role::PORT));
    let n = s.pin(PinSpec::continuous("n", role::PORT));
    let t0 = s.term(TermSpec { name: "i", origin: ORIGIN, op: op::FLOW, plus: p, minus: n, abstol: DEFAULT, max: DEFAULT });
    let frequency = Cell { kind: cell::AMBIENT, index: ambient::FREQ, part: part::VALUE };
    s.function(FunctionSpec {
        name: "i",
        origin: ORIGIN,
        reads: vec![cell_of(cell::PIN, p), cell_of(cell::PIN, n), frequency],
        writes: vec![cell_of(cell::TERM, t0)],
        partials: vec![Partial { write: 0, to: 0, read: 0, at: 0 }, Partial { write: 0, to: 0, read: 1, at: 0 }],
        ..FunctionSpec::default()
    });
    s.freeze()
}

impl Device for OpenAboveDc {
    fn prototype() -> &'static Shape<'static> {
        OPEN_SHAPE.get_or_init(open_shape).shape()
    }
    fn create(_: &Values<'_>) -> Result<(Self, ShapeRef), Fault> {
        Ok((OpenAboveDc, Arc::clone(OPEN_SHAPE.get_or_init(open_shape))))
    }
    fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
        let g = if cx.f64(2) > 0.0 { 0.0 } else { 1.0e-3 };
        cx.set(0, g * (cx.f64(0) - cx.f64(1)));
        cx.jac().copy_from_slice(&[g, -g]);
        Ok(())
    }
    fn react(&self, _k: u32, _cx: &mut React) -> Result<Flow, Fault> {
        Ok(Flow::Continue)
    }
}

/// The error of a transient to 1 µs that starts at 100 ns and may shrink to 10 ns.
fn tran_error(circuit: &mut CircuitInstance) -> Error {
    let options = TransientAnalysisOptions::new(1.0e-6, 1.0e-7).with_dt_min(1.0e-8);
    circuit
        .transient(options, Context::default())
        .and_then(|mut tran| tran.solve())
        .map(|_| ())
        .expect_err("a step no Newton iteration settles must fail")
}

/// HYG-03 AC 5: a Newton failure at the floor is `E0508` with its time and the
/// unknowns that failed.
#[test]
fn a_step_that_fails_newton_at_the_floor_is_e0508_with_its_time() {
    let mut b = CircuitBuilder::new("late");
    let gnd = b.ground();
    let a = b.node("a");
    add_resistor(&mut b, "r", 1e6, &a, &gnd);
    b.device(LATE.get_or_init(|| module::<LateOscillator>("LateOscillator")), "late", &[], &[Port::Node(a), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");

    let err = tran_error(&mut circuit);
    assert_eq!(err.code(), Some(RunCode::E0508), "{err}");
    let Error::Run(run) = &err else { panic!("a run error, got {err}") };
    let t = run.time.expect("the time of the failing step");
    assert!(t > 2.0e-7 && t <= 1.0e-6, "the failing step lies after the trip time: {t:e}");
    assert_eq!(run.names, vec!["node of r.p, late.p".to_string()], "{err}");
}

/// HYG-01 AC 1, the transient step: a second potential source appears across the nodes of the first
/// at 200 ns; the operating point and the steps before it solve, the step that reads it is singular.
/// The error is `E0503` at that time and names both branch flows.
#[test]
fn a_singular_transient_step_is_e0503_naming_both_sources() {
    let mut b = CircuitBuilder::new("late loop");
    let gnd = b.ground();
    let a = b.node("a");
    add_vsrc(&mut b, "v1", 1.0, &a, &gnd);
    b.device(LATE_SOURCE.get_or_init(|| module::<LateSource>("LateSource")), "late", &[], &[Port::Node(a), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");

    let err = tran_error(&mut circuit);
    assert_eq!(err.code(), Some(RunCode::E0503), "{err}");
    let Error::Run(run) = &err else { panic!("a run error, got {err}") };
    let t = run.time.expect("the time of the singular step");
    assert!(t > LATE_SOURCE_AT && t <= 1.0e-6, "the singular step lies after the second source appears: {t:e}");
    assert_eq!(run.names, vec!["flow of v1 (f)".to_string(), "flow of late (f)".to_string()], "both sources, each once: {err}");
}

/// HYG-01 AC 1, the AC point: a node held only by a conductance that is open above DC solves at the
/// operating point and is singular at the first frequency. The error is `E0503` and names the pin.
#[test]
fn a_singular_ac_point_is_e0503_naming_the_node() {
    let mut b = CircuitBuilder::new("open at ac");
    let gnd = b.ground();
    let a = b.node("a");
    b.device(OPEN.get_or_init(|| module::<OpenAboveDc>("OpenAboveDc")), "g1", &[], &[Port::Node(a), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");

    let err = circuit
        .ac(Context::default())
        .and_then(|mut ac| ac.solve_sweep(sweep()))
        .map(|_| ())
        .expect_err("a node with no small-signal path must be singular");
    assert_eq!(err.code(), Some(RunCode::E0503), "{err}");
    let Error::Run(run) = &err else { panic!("a run error, got {err}") };
    assert_eq!(run.names, vec!["node of g1.p".to_string()], "{err}");
}

/// HYG-03 AC 6: the step that reads a jump (the first after a `Restart::Integrator`
/// delivery) fails as `E0504`, naming the net whose delivery it reads.
#[test]
fn a_jump_step_that_fails_at_the_floor_is_e0504_naming_the_carrier() {
    let mut b = CircuitBuilder::new("jump");
    let gnd = b.ground();
    let a = b.node("a");
    add_resistor(&mut b, "r", 1e6, &a, &gnd);
    b.device(abi_mixed::timer(), "clk", &[("half", ParamValue::Real(5.0e-7))], &[Port::Net("go".to_string())]);
    b.device(
        JUMP.get_or_init(|| module::<JumpOscillator>("JumpOscillator")),
        "jump",
        &[],
        &[Port::Node(a), Port::Node(gnd), Port::Net("go".to_string())],
    );
    let mut circuit = b.build().expect("build");

    let err = tran_error(&mut circuit);
    assert_eq!(err.code(), Some(RunCode::E0504), "{err}");
    let Error::Run(run) = &err else { panic!("a run error, got {err}") };
    assert!(run.detail.contains("go"), "the carrier is named: {err}");
    let t = run.time.expect("the time of the failing step");
    assert!(t > 5.0e-7 && t <= 1.0e-6, "the failing step lies after the delivery at 500 ns: {t:e}");
}
