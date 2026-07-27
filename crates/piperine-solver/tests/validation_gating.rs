//! `EMITS_VALIDATION` gates validation polling (DVC-14): an element that
//! does not declare the bit is *never polled* — proven by a counting test
//! element, not by inspection — while an element with the bit is polled
//! once per evaluation point (the final converged DC solution; each
//! accepted transient step, design §C3). Declaring the bit and returning
//! `None` is legal: the bit promises capability, not content.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use piperine_solver::abi::{
    AnalogDevice, AnalogReference, BranchIdentifier, DcAnalysisState, DigitalDevice, Element,
    ElementCapabilities, Introspect, Netlist, NodeIdentifier, Stamp, TransientAnalysisContext,
    TransientAnalysisState, ValidationFinding, ValidationReport, ValidationSeverity,
};
use piperine_solver::prelude::{CircuitInstance, Context, Solver, TransientAnalysisOptions};

/// A linear conductance (g = 1 S to ground) carrying a validation hook that
/// counts its polls and replays a canned report. `emits_bit` controls
/// whether the element declares [`ElementCapabilities::EMITS_VALIDATION`].
struct CountingValidator {
    node: AnalogReference,
    emits_bit: bool,
    report: Option<ValidationReport>,
    polls: Arc<AtomicUsize>,
}

impl CountingValidator {
    fn stamps(&self) -> Vec<Stamp<AnalogReference, f64>> {
        vec![
            Stamp::Matrix(self.node.clone(), self.node.clone(), 1.0),
            Stamp::Rhs(self.node.clone(), 0.5),
        ]
    }
}

impl AnalogDevice for CountingValidator {
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        self.stamps()
    }
    fn load_transient(
        &mut self,
        _s: &TransientAnalysisState<'_>,
        _t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.stamps()
    }
}
impl DigitalDevice for CountingValidator {}
impl Introspect for CountingValidator {}
impl Element for CountingValidator {
    fn name(&self) -> &str {
        "cv"
    }
    fn capabilities(&self) -> ElementCapabilities {
        let mut caps = ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_TRAN;
        if self.emits_bit {
            caps |= ElementCapabilities::EMITS_VALIDATION;
        }
        caps
    }
    fn validation_reports(&self) -> Option<ValidationReport> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        self.report.clone()
    }
}

/// A pure voltage source forcing `node` to 1 V (branch-current unknown), so
/// the transient t=0 solve has a determined operating point.
struct Vdc {
    n1: AnalogReference,
    n2: AnalogReference,
    branch: AnalogReference,
}
impl AnalogDevice for Vdc {
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        self.stamps()
    }
    fn load_transient(
        &mut self,
        _s: &TransientAnalysisState<'_>,
        _t: &TransientAnalysisContext,
        _c: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.stamps()
    }
}
impl Vdc {
    fn stamps(&self) -> Vec<Stamp<AnalogReference, f64>> {
        vec![
            Stamp::Matrix(self.n1.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.n1.clone(), 1.0),
            Stamp::Matrix(self.n2.clone(), self.branch.clone(), -1.0),
            Stamp::Matrix(self.branch.clone(), self.n2.clone(), -1.0),
            Stamp::Rhs(self.branch.clone(), 1.0),
        ]
    }
}
impl DigitalDevice for Vdc {}
impl Introspect for Vdc {}
impl Element for Vdc {
    fn name(&self) -> &str {
        "v"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_TRAN
            | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
}

/// A one-node circuit: the voltage source pins the node, the counting
/// validator rides it.
fn counting_circuit(emits_bit: bool, report: Option<ValidationReport>) -> (CircuitInstance, Arc<AtomicUsize>) {
    let mut netlist = Netlist::new();
    let node = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));

    let polls = Arc::new(AtomicUsize::new(0));
    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(Vdc { n1: node.clone(), n2: gnd, branch }),
        Box::new(CountingValidator { node, emits_bit, report, polls: polls.clone() }),
    ];
    (CircuitInstance::from_devices_and_netlist("gate", elements, netlist), polls)
}

/// DVC-14: the bit occupies the documented position — `1 << 15`, one above
/// the previous highest used bit (`NUMERIC_JACOBIAN = 1 << 14`).
#[test]
fn the_bit_sits_at_1_shift_15() {
    assert_eq!(ElementCapabilities::EMITS_VALIDATION.bits(), 1u32 << 15);
}

/// DVC-14: an element without `EMITS_VALIDATION` is never polled — not in
/// DC, not across a whole transient — proven by the poll counter staying at
/// zero. This is also the "no validating elements ⇒ no per-step cost"
/// proof: nothing to call is the zero cost.
#[test]
fn element_without_the_bit_is_never_polled() {
    let (mut circuit, polls) = counting_circuit(false, None);
    circuit.dc(Context::default()).unwrap().solve().unwrap();
    assert_eq!(polls.load(Ordering::SeqCst), 0, "DC must not poll an element without the bit");

    let opts = TransientAnalysisOptions::new(1e-6, 1e-7);
    circuit.transient(opts, Context::default()).unwrap().solve().unwrap();
    assert_eq!(polls.load(Ordering::SeqCst), 0, "transient must not poll an element without the bit");
}

/// DVC-14: an element with the bit is polled exactly once per evaluation
/// point. DC evaluates at the final converged solution only (design §C3) —
/// one poll for the whole solve, however many Newton iterations it took.
#[test]
fn element_with_the_bit_is_polled_once_at_the_converged_dc_solution() {
    let (mut circuit, polls) = counting_circuit(true, None);
    circuit.dc(Context::default()).unwrap().solve().unwrap();
    assert_eq!(
        polls.load(Ordering::SeqCst),
        1,
        "DC polls once, at the final converged solution — not per Newton iteration"
    );
}

/// DVC-14: transient polls once per accepted step — the poll count equals
/// the result's accepted-step count exactly.
#[test]
fn element_with_the_bit_is_polled_once_per_accepted_step() {
    let (mut circuit, polls) = counting_circuit(true, None);
    let opts = TransientAnalysisOptions::new(1e-6, 1e-7);
    let result = circuit.transient(opts, Context::default()).unwrap().solve().unwrap();
    let accepted = result.stats.steps_accepted;
    assert!(accepted > 1, "the run took several steps, got {accepted}");
    // One poll per accepted step, plus one for the computed t = 0 operating
    // point (design §C3: with a computed OP, t = 0 is an evaluation point).
    assert_eq!(
        polls.load(Ordering::SeqCst),
        accepted + 1,
        "one poll per accepted step plus the computed t = 0 OP, never per rejected attempt"
    );
}

/// DVC-14: an element with the bit that always returns `None` is legal —
/// the analyses complete — and its polls produce no reports.
#[test]
fn none_returning_element_with_the_bit_is_legal() {
    let (mut circuit, polls) = counting_circuit(true, None);
    circuit.dc(Context::default()).unwrap().solve().unwrap();
    assert!(polls.load(Ordering::SeqCst) > 0, "the None-returning element was still polled");
    assert!(
        circuit.validation_reports().is_empty(),
        "a None report collects nothing"
    );
}

/// DVC-14: a report from a bit-declaring element is collected, paired with
/// the element's name and every finding field intact.
#[test]
fn the_collector_pairs_reports_with_element_names() {
    let report = ValidationReport::new(vec![ValidationFinding {
        severity: ValidationSeverity::Error,
        label: "soa".to_string(),
        message: "limit exceeded".to_string(),
        value: -0.1,
        time: 3e-9,
        instance_path: "/gate/cv".to_string(),
    }]);
    let (circuit, _polls) = counting_circuit(true, Some(report));
    let collected = circuit.validation_reports();
    assert_eq!(collected.len(), 1, "one reporting element");
    let (name, report) = &collected[0];
    assert_eq!(name, "cv", "the report travels with its element's name");
    let findings = report.findings();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, ValidationSeverity::Error);
    assert_eq!(findings[0].label, "soa");
    assert_eq!(findings[0].value, -0.1);
    assert_eq!(findings[0].time, 3e-9);
    assert_eq!(findings[0].instance_path, "/gate/cv");
}
