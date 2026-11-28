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

use piperine_device_fixtures::circuits::CircuitBuilding;
use piperine_device_fixtures::solver_devices::failure_rules::{JumpOscillator, LateOscillator, LateSource, NanSource, OpenAboveDc, Oscillator};
use piperine_device_fixtures::digital;

use piperine_solver::abi::{Error, Port, SolverDomain};
use piperine_analog_solver::error::RunCode;
use piperine_solver::prelude::*;

const ORIGIN: &str = "failure_rules.rs";

// ─── §12 — noise output/reference node cannot be resolved ─────────────────────

#[test]
fn section_12_unresolvable_noise_output_node_is_an_analysis_error() {
    let mut circuit = FailureCircuit::resistor_circuit();
    let options = NoiseAnalysisOptions {
        sweep_options: FailureCircuit::sweep(),
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
    FailureCircuit::assert_rule(&err, SolverDomain::Noise, "output node");
}

#[test]
fn section_12_unresolvable_noise_reference_node_is_an_analysis_error() {
    let mut circuit = FailureCircuit::resistor_circuit();
    let options = NoiseAnalysisOptions {
        sweep_options: FailureCircuit::sweep(),
        output_node: FailureCircuit::node_a(&circuit),
        reference_node: NodeIdentifier::Anonymous(98),
        input_source_name: None,
        output_flow: None,
    };
    let err = circuit
        .noise(options, Context::default())
        .and_then(|mut n| n.solve())
        .map(|_| ())
        .expect_err("§12: an unresolvable reference node must fail");
    FailureCircuit::assert_rule(&err, SolverDomain::Noise, "reference node");
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
    CircuitBuilding::add_resistor(&mut b, "r", 1000.0, &a, &gnd);
    b.device(NanSource::module(), "nan", &[], &[Port::Node(a), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");

    let err = circuit
        .dc(Context::default())
        .and_then(|mut dc| dc.solve())
        .map(|_| ())
        .expect_err("§15/§7.3.2.1: a NaN contribution must fail, never converge");
    FailureCircuit::assert_rule(&err, SolverDomain::Device, "E0507");
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
    CircuitBuilding::add_resistor(&mut b, "r", 1000.0, &a, &gnd);
    b.device(NanSource::module(), "nan", &[], &[Port::Node(a), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");

    let err = FailureCircuit::dc_error(&mut circuit);
    assert_eq!(err.code(), Some(RunCode::E0507), "{err}");
    let text = err.to_string();
    assert!(text.contains("instance `nan`"), "the instance is named: {err}");
    assert!(!text.contains("7.3.2.1"), "no LRM section is cited: {err}");
}

#[test]
fn section_18_a_non_positive_period_is_an_analysis_error() {
    for period in [0.0, -1e-6] {
        let mut circuit = FailureCircuit::resistor_circuit();
        let err = circuit
            .pss(FailureCircuit::pss_options(period, 0.0), Context::default())
            .and_then(|p| p.solve())
            .map(|_| ())
            .expect_err("§18: a non-positive period must fail");
        FailureCircuit::assert_rule(&err, SolverDomain::Pss, "period must be positive");
    }
}

#[test]
fn section_18_a_negative_pre_roll_is_an_analysis_error() {
    let mut circuit = FailureCircuit::resistor_circuit();
    let err = circuit
        .pss(FailureCircuit::pss_options(1e-3, -1.0), Context::default())
        .and_then(|p| p.solve())
        .map(|_| ())
        .expect_err("§18: a negative pre-roll must fail");
    FailureCircuit::assert_rule(&err, SolverDomain::Pss, "tstab");
}

// ─── §9 — DC fails plain Newton, gmin stepping, and source stepping ────────────

#[test]
fn section_9_exhausting_every_dc_strategy_is_a_convergence_failure() {
    let mut b = CircuitBuilder::new("osc");
    let gnd = b.ground();
    let a = b.node("a");
    CircuitBuilding::add_resistor(&mut b, "r", 1e6, &a, &gnd);
    b.device(Oscillator::module(), "osc", &[], &[Port::Node(a), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");

    let err = circuit
        .dc(Context::default())
        .and_then(|mut dc| dc.solve())
        .map(|_| ())
        .expect_err("§9: plain Newton, gmin stepping and source stepping all fail here");
    // The exhausted plan reports its last attempt, so the domain is `Newton`
    // rather than `Dc` — §16 calls this a "convergence failure" without fixing
    // the domain, and this is the domain the code actually uses.
    FailureCircuit::assert_rule(&err, SolverDomain::Newton, "converge");
}

/// HYG-03 AC 4: an equilibrium no strategy settles is `E0505`, naming the
/// unknowns of the last attempt that failed the update or residual test.
#[test]
fn an_equilibrium_that_never_converges_is_e0505_with_its_unknowns() {
    let mut b = CircuitBuilder::new("osc");
    let gnd = b.ground();
    let a = b.node("a");
    CircuitBuilding::add_resistor(&mut b, "r", 1e6, &a, &gnd);
    b.device(Oscillator::module(), "osc", &[], &[Port::Node(a), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");

    let err = FailureCircuit::dc_error(&mut circuit);
    assert_eq!(err.code(), Some(RunCode::E0505), "{err}");
    let Error::Run(run) = &err else { panic!("a run error, got {err}") };
    assert_eq!(run.names, vec!["node of r.p, osc.p".to_string()], "the one unknown that never settles: {err}");
}

/// HYG-01 AC 1: a node reached only through capacitors stamps nothing at DC;
/// the error is `E0503` and names the node by the pins bound to it.
#[test]
fn a_singular_solve_names_the_pins_of_its_node() {
    let mut b = CircuitBuilder::new("floating");
    let gnd = b.ground();
    let a = b.node("a");
    CircuitBuilding::add_capacitor(&mut b, "c1", 1e-9, &a, &gnd);
    CircuitBuilding::add_capacitor(&mut b, "c2", 2e-9, &a, &gnd);
    let mut circuit = b.build().expect("build");

    let err = FailureCircuit::dc_error(&mut circuit);
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
    CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &a, &gnd);
    CircuitBuilding::add_voltage_source(&mut b, "v2", 2.0, &a, &gnd);
    let mut circuit = b.build().expect("build");

    let err = FailureCircuit::dc_error(&mut circuit);
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
    CircuitBuilding::add_capacitor(&mut b, "c1", 1e-9, &a, &gnd);
    CircuitBuilding::add_capacitor(&mut b, "c2", 2e-9, &a, &a);
    let mut circuit = b.build().expect("build");

    let err = FailureCircuit::dc_error(&mut circuit);
    let Error::Run(run) = &err else { panic!("a run error, got {err}") };
    assert_eq!(run.names, vec!["node of c1.p, c2.p+n".to_string()], "{err}");
    assert_eq!(err.to_string().matches("c2").count(), 1, "{err}");
}
const LATE_SOURCE_AT: f64 = 2.0e-7;

/// HYG-03 AC 5: a Newton failure at the floor is `E0508` with its time and the
/// unknowns that failed.
#[test]
fn a_step_that_fails_newton_at_the_floor_is_e0508_with_its_time() {
    let mut b = CircuitBuilder::new("late");
    let gnd = b.ground();
    let a = b.node("a");
    CircuitBuilding::add_resistor(&mut b, "r", 1e6, &a, &gnd);
    b.device(LateOscillator::module(), "late", &[], &[Port::Node(a), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");

    let err = FailureCircuit::tran_error(&mut circuit);
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
    CircuitBuilding::add_voltage_source(&mut b, "v1", 1.0, &a, &gnd);
    b.device(LateSource::module(), "late", &[], &[Port::Node(a), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");

    let err = FailureCircuit::tran_error(&mut circuit);
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
    b.device(OpenAboveDc::module(), "g1", &[], &[Port::Node(a), Port::Node(gnd)]);
    let mut circuit = b.build().expect("build");

    let err = circuit
        .ac(Context::default())
        .and_then(|mut ac| ac.solve_sweep(FailureCircuit::sweep()))
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
    CircuitBuilding::add_resistor(&mut b, "r", 1e6, &a, &gnd);
    b.device(digital::Timer::module(), "clk", &[("half", ParamValue::Real(5.0e-7))], &[Port::Net("go".to_string())]);
    b.device(
        JumpOscillator::module(),
        "jump",
        &[],
        &[Port::Node(a), Port::Node(gnd), Port::Net("go".to_string())],
    );
    let mut circuit = b.build().expect("build");

    let err = FailureCircuit::tran_error(&mut circuit);
    assert_eq!(err.code(), Some(RunCode::E0504), "{err}");
    let Error::Run(run) = &err else { panic!("a run error, got {err}") };
    assert!(run.detail.contains("go"), "the carrier is named: {err}");
    let t = run.time.expect("the time of the failing step");
    assert!(t > 5.0e-7 && t <= 1.0e-6, "the failing step lies after the delivery at 500 ns: {t:e}");
}

/// The resistor circuit the failure rules fail, and the errors they raise.
struct FailureCircuit;

impl FailureCircuit {
    // ─── Fixtures ─────────────────────────────────────────────────────────────────

    /// A one-resistor circuit between node 1 and ground.
    fn resistor_circuit() -> CircuitInstance {
        let mut b = CircuitBuilder::new("rules");
        let gnd = b.ground();
        let a = b.node("a");
        CircuitBuilding::add_resistor(&mut b, "r", 1000.0, &a, &gnd);
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

    /// The sweep helper `sweep`.
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

    // ─── §18 — non-positive period, negative pre-roll ─────────────────────────────

    /// The pss options helper `pss_options`.
    fn pss_options(period: f64, tstab: f64) -> PssAnalysisOptions {
        PssAnalysisOptions { period, tstab, max_shoot_iter: 4, shoot_tol: 1e-6, dt: None, dt_max: None, max_steps: None }
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

    /// The error of a transient to 1 µs that starts at 100 ns and may shrink to 10 ns.
    fn tran_error(circuit: &mut CircuitInstance) -> Error {
        let options = TransientAnalysisOptions::new(1.0e-6, 1.0e-7).with_dt_min(1.0e-8);
        circuit
            .transient(options, Context::default())
            .and_then(|mut tran| tran.solve())
            .map(|_| ())
            .expect_err("a step no Newton iteration settles must fail")
    }
}
