//! Posture behavior on validation findings (dv-core T17, DVC-15): the
//! check posture decides what an analysis does with the findings channel.
//! `strict` (the default) fails loud on the first `Error` finding —
//! naming the constraint, the instance path, the time, and the margin
//! value — while a `Warning` never aborts, even in strict. `collect`
//! completes the analysis and records every finding on the result stats.
//! `off` never polls the hook and reports nothing. Proven with a counting
//! test element replaying canned reports through real DC and transient
//! solves.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use piperine_solver::abi::{
    AnalogDevice, AnalogReference, BranchIdentifier, CheckPosture, DcAnalysisState, DigitalDevice,
    Element, ElementCapabilities, Introspect, Netlist, NodeIdentifier, Stamp,
    TransientAnalysisContext, TransientAnalysisState, ValidationFinding, ValidationReport,
    ValidationSeverity,
};
use piperine_solver::prelude::{CircuitInstance, Context, TransientAnalysisOptions};

/// A linear conductance (g = 1 S to ground) replaying a canned report and
/// counting its polls.
struct CannedValidator {
    node: AnalogReference,
    report: ValidationReport,
    polls: Arc<AtomicUsize>,
}

impl CannedValidator {
    fn stamps(&self) -> Vec<Stamp<AnalogReference, f64>> {
        vec![
            Stamp::Matrix(self.node.clone(), self.node.clone(), 1.0),
            Stamp::Rhs(self.node.clone(), 0.5),
        ]
    }
}

impl AnalogDevice for CannedValidator {
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
impl DigitalDevice for CannedValidator {}
impl Introspect for CannedValidator {}
impl Element for CannedValidator {
    fn name(&self) -> &str {
        "cv"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::LOADS_TRAN
            | ElementCapabilities::EMITS_VALIDATION
    }
    fn validation_reports(&self) -> Option<ValidationReport> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        Some(self.report.clone())
    }
}

/// A pure voltage source forcing `node` to 1 V (branch-current unknown),
/// so the transient t=0 solve has a determined operating point.
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

fn finding(severity: ValidationSeverity) -> ValidationFinding {
    ValidationFinding {
        severity,
        label: "soa".to_string(),
        message: "safe-operating limit exceeded".to_string(),
        value: -0.5,
        time: 3e-9,
        instance_path: "/gate/cv".to_string(),
    }
}

/// A one-node circuit: the source pins the node, the canned validator
/// rides it.
fn canned_circuit(report: ValidationReport) -> (CircuitInstance, Arc<AtomicUsize>) {
    let mut netlist = Netlist::new();
    let node = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let branch = netlist.connect_branch(BranchIdentifier::from_component("v1"));

    let polls = Arc::new(AtomicUsize::new(0));
    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(Vdc { n1: node.clone(), n2: gnd, branch }),
        Box::new(CannedValidator { node, report, polls: polls.clone() }),
    ];
    (CircuitInstance::from_devices_and_netlist("gate", elements, netlist), polls)
}

fn context(posture: CheckPosture) -> Context {
    Context { checks: posture, ..Default::default() }
}

/// DVC-15: `strict` (the default posture) fails loud on an `Error`
/// finding — the error names the constraint, the instance path, the
/// time, and the margin value.
#[test]
fn strict_aborts_on_an_error_finding_naming_all_four_fields() {
    let (mut circuit, _) = canned_circuit(ValidationReport::new(vec![finding(ValidationSeverity::Error)]));
    let err = circuit
        .dc(Context::default())
        .unwrap()
        .solve()
        .expect_err("strict + Error finding aborts the analysis");
    let msg = err.to_string();
    assert!(msg.contains("soa"), "names the constraint: {msg}");
    assert!(msg.contains("/gate/cv"), "names the instance path: {msg}");
    assert!(msg.contains(&format!("at t = {}", 3e-9)), "names the time: {msg}");
    assert!(msg.contains(&format!("margin = {:.6}", -0.5)), "names the margin value: {msg}");
}

/// DVC-15: a `Warning` finding never aborts, even in `strict` — only
/// `Error` aborts. The warning is still recorded on the result.
#[test]
fn a_warning_never_aborts_even_in_strict() {
    let (mut circuit, _) =
        canned_circuit(ValidationReport::new(vec![finding(ValidationSeverity::Warning)]));
    let dc = circuit
        .dc(Context::default())
        .unwrap()
        .solve()
        .expect("a Warning must not abort a strict DC");
    assert_eq!(dc.stats.validation.len(), 1, "the warning is recorded");
    assert_eq!(dc.stats.validation[0].severity, ValidationSeverity::Warning);
    assert_eq!(dc.stats.validation[0].label, "soa");

    let opts = TransientAnalysisOptions::new(1e-6, 1e-7);
    let tran = circuit
        .transient(opts, Context::default())
        .unwrap()
        .solve()
        .expect("a Warning must not abort a strict transient");
    assert_eq!(
        tran.stats.validation.len(),
        tran.stats.steps_accepted,
        "one recorded warning per accepted step"
    );
    assert!(tran.stats.validation.iter().all(|f| f.severity == ValidationSeverity::Warning));
}

/// DVC-15: `collect` completes the analysis even with `Error` findings,
/// recording each one on the result stats with every field intact.
#[test]
fn collect_completes_and_records_error_findings() {
    let (mut circuit, _) = canned_circuit(ValidationReport::new(vec![finding(ValidationSeverity::Error)]));
    let dc = circuit
        .dc(context(CheckPosture::Collect))
        .unwrap()
        .solve()
        .expect("collect completes despite the Error finding");
    assert_eq!(dc.stats.validation.len(), 1, "the converged DC solution's finding");
    let recorded = &dc.stats.validation[0];
    assert_eq!(recorded.severity, ValidationSeverity::Error);
    assert_eq!(recorded.label, "soa");
    assert_eq!(recorded.value, -0.5);
    assert_eq!(recorded.time, 3e-9);
    assert_eq!(recorded.instance_path, "/gate/cv");

    let opts = TransientAnalysisOptions::new(1e-6, 1e-7);
    let tran = circuit
        .transient(opts, context(CheckPosture::Collect))
        .unwrap()
        .solve()
        .expect("collect completes the whole transient");
    assert!(tran.stats.steps_accepted > 1, "the run took several steps");
    assert_eq!(
        tran.stats.validation.len(),
        tran.stats.steps_accepted,
        "one recorded finding per accepted step — the run was not cut short"
    );
}

/// DVC-15: `off` never polls the hook — the poll counter stays at zero
/// across both analyses — and nothing is reported.
#[test]
fn off_never_polls_and_reports_nothing() {
    let (mut circuit, polls) =
        canned_circuit(ValidationReport::new(vec![finding(ValidationSeverity::Error)]));
    let dc = circuit
        .dc(context(CheckPosture::Off))
        .unwrap()
        .solve()
        .expect("off completes");
    assert!(dc.stats.validation.is_empty());

    let opts = TransientAnalysisOptions::new(1e-6, 1e-7);
    let tran = circuit
        .transient(opts, context(CheckPosture::Off))
        .unwrap()
        .solve()
        .expect("off completes");
    assert!(tran.stats.validation.is_empty());
    assert_eq!(
        polls.load(Ordering::SeqCst),
        0,
        "off calls nothing — the hook was never polled"
    );
}
