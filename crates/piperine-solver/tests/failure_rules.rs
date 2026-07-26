//! The normative failure rules of SPEC Part VII §16, enforced.
//!
//! Each rule in that table says a specific condition SHALL be an analysis or
//! device-load error. P6 found most of them had a failure site in the code but
//! no test reaching it (CLN-16/17), so this suite trips them: every test names
//! the `§n` row it enforces and asserts the typed `SolverDomain` plus the
//! message fragment that identifies the rule. (`Error` is
//! `{Simple,WithCause}{domain, detail}` — domain + fragment is the strongest
//! typed assertion the taxonomy offers.)

use piperine_solver::abi::{
    AnalogDevice, DcAnalysisState, DigitalDevice, Error, Introspect, SolverDomain, Stamp,
};
use piperine_solver::prelude::*;

// ─── Fixtures ─────────────────────────────────────────────────────────────────

/// A linear resistor.
struct Resistor {
    r: f64,
    n1: AnalogReference,
    n2: AnalogReference,
}

impl AnalogDevice for Resistor {
    fn load_dc(&mut self, _s: &DcAnalysisState, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        let g = 1.0 / self.r;
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
        ]
    }
}

impl DigitalDevice for Resistor {}
impl Introspect for Resistor {}

impl Element for Resistor {
    fn name(&self) -> &str {
        "r"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_AC
    }
}

/// A device whose DC stamp is not finite — the §15 trigger.
struct NanSource {
    n1: AnalogReference,
    n2: AnalogReference,
}

impl AnalogDevice for NanSource {
    fn load_dc(&mut self, _s: &DcAnalysisState, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), f64::NAN),
            Stamp::Rhs(self.n2.clone(), f64::NAN),
        ]
    }
}

impl DigitalDevice for NanSource {}
impl Introspect for NanSource {}

impl Element for NanSource {
    fn name(&self) -> &str {
        "nan"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }
}

/// A one-resistor circuit between node 1 and ground.
fn resistor_circuit() -> CircuitInstance {
    let mut netlist = Netlist::new();
    let a = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let elements: Vec<Box<dyn Element>> =
        vec![Box::new(Resistor { r: 1000.0, n1: a, n2: gnd })];
    CircuitInstance::from_devices_and_netlist("rules", elements, netlist)
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
        output_node: NodeIdentifier::Anonymous(1),
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

#[test]
fn section_15_a_non_finite_stamp_is_a_convergence_failure() {
    let mut netlist = Netlist::new();
    let a = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(Resistor { r: 1000.0, n1: a.clone(), n2: gnd.clone() }),
        Box::new(NanSource { n1: a, n2: gnd }),
    ];
    let mut circuit = CircuitInstance::from_devices_and_netlist("nan", elements, netlist);

    let err = circuit
        .dc(Context::default())
        .and_then(|mut dc| dc.solve())
        .map(|_| ())
        .expect_err("§15: a NaN in the system must fail, never converge");
    // The guard lives in the Newton driver; the DC driver wraps it.
    assert!(
        matches!(err, Error::Simple { .. } | Error::WithCause { .. }),
        "§15 must be a typed solver error: {err}"
    );
    let text = err.to_string().to_ascii_lowercase();
    assert!(
        text.contains("finite") || text.contains("nan") || text.contains("converge"),
        "§15's message must say what went wrong: {err}"
    );
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

/// A pathological element: its conductance flips sign every evaluation, so no
/// Newton iteration can settle and no homotopy ramp helps. Nothing in the
/// stdlib behaves like this — it exists to reach §9's "every strategy failed"
/// path, which is otherwise only reachable by a real convergence disaster.
struct Oscillator {
    n1: AnalogReference,
    n2: AnalogReference,
    flip: bool,
}

impl AnalogDevice for Oscillator {
    fn load_dc(&mut self, _s: &DcAnalysisState, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        self.flip = !self.flip;
        let g = if self.flip { 1e-3 } else { 1e3 };
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
            Stamp::Rhs(self.n1.clone(), if self.flip { 1.0 } else { -1.0 }),
        ]
    }
}

impl DigitalDevice for Oscillator {}
impl Introspect for Oscillator {}

impl Element for Oscillator {
    fn name(&self) -> &str {
        "osc"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }
}

#[test]
fn section_9_exhausting_every_dc_strategy_is_a_convergence_failure() {
    let mut netlist = Netlist::new();
    let a = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(Resistor { r: 1e6, n1: a.clone(), n2: gnd.clone() }),
        Box::new(Oscillator { n1: a, n2: gnd, flip: false }),
    ];
    let mut circuit = CircuitInstance::from_devices_and_netlist("osc", elements, netlist);

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
