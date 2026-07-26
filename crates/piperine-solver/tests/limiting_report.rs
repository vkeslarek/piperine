//! LimitingReport convergence-gate tests (ABI-10): the Newton loop consults
//! `limiting_report()` each iteration — applying the `limited_value` to the
//! guess (steering) and vetoing convergence while a report is active.
//!
//! A test device caches a `LimitingReport` during `load_dc` (the same pattern
//! the codegen `Limiter` uses), so the solver's consumption of the report is
//! observable through the recorded guess sequence.

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

use piperine_solver::abi::{
    AnalogDevice, AnalogReference, BranchIdentifier, DcAnalysisState, DigitalDevice, Element,
    ElementCapabilities, Introspect, LimitReason, LimitingReport, Netlist, NodeIdentifier, Stamp,
};
use piperine_solver::prelude::{CircuitInstance, Context, Solver};

/// A device whose load is a linear conductance + matched current (natural
/// solution `V_node = lim`), plus a `$limit`-style step limiter that clamps
/// the Newton guess to `lim` while it is far away. Caches the report in
/// `load_dc` (reading the live guess) and exposes it via `limiting_report()`,
/// mirroring the codegen `Limiter` producer.
struct StepLimiter {
    node: AnalogReference,
    lim: f64,
    /// Cached report (set in `load_dc`, read in `limiting_report`).
    report: Option<LimitingReport>,
    /// Count of `limiting_report()` calls (the solver consulting the report).
    consultations: Arc<AtomicUsize>,
    /// Guess values seen at each `load_dc` call — proves whether the limited
    /// value was applied to the Newton guess.
    guesses: Arc<Mutex<Vec<f64>>>,
}

impl AnalogDevice for StepLimiter {
    fn load_dc(&mut self, s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        let v = self
            .node
            .idx()
            .and_then(|i| s.latest().and_then(|row| row.get(i).copied()))
            .unwrap_or(0.0);
        self.guesses.lock().unwrap().push(v);
        // Step limiter: while the guess is far from the target, clamp it there
        // and report active (vetoing convergence until it lands).
        if (v - self.lim).abs() > 0.05 {
            self.report = Some(LimitingReport {
                device: self.name().to_string(),
                net: self.node.clone(),
                proposed: v,
                limited_value: self.lim,
                limiter_name: "testlim",
                reason: LimitReason::VoltageStep,
            });
        } else {
            self.report = None;
        }
        // Linear load: G = 1 S to gnd, I = lim → natural solution V = lim.
        vec![
            Stamp::Matrix(self.node.clone(), self.node.clone(), 1.0),
            Stamp::Rhs(self.node.clone(), self.lim),
        ]
    }

    fn limiting_report(&self) -> Option<LimitingReport> {
        self.consultations.fetch_add(1, Ordering::SeqCst);
        self.report.clone()
    }
}

impl DigitalDevice for StepLimiter {}
impl Introspect for StepLimiter {}
impl Element for StepLimiter {
    fn name(&self) -> &str { "steplim" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }
}

/// A pure voltage source forcing `node` to `vforce` (branch-current unknown).
struct Vdc {
    vforce: f64,
    n1: AnalogReference,
    n2: AnalogReference,
    branch: AnalogReference,
}
impl AnalogDevice for Vdc {
    fn load_dc(&mut self, _s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
        vec![
            Stamp::Matrix(self.n1.clone(), self.branch.clone(), 1.0),
            Stamp::Matrix(self.branch.clone(), self.n1.clone(), 1.0),
            Stamp::Matrix(self.n2.clone(), self.branch.clone(), -1.0),
            Stamp::Matrix(self.branch.clone(), self.n2.clone(), -1.0),
            Stamp::Rhs(self.branch.clone(), self.vforce),
        ]
    }
}
impl DigitalDevice for Vdc {}
impl Introspect for Vdc {}
impl Element for Vdc {
    fn name(&self) -> &str { "v" }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::HAS_INTERNAL_UNKNOWNS
    }
}

/// Spec ABI-10: the Newton loop applies `limiting_report().limited_value` to
/// the guess and the next iteration's load sees the steered value.
#[test]
fn newton_applies_limited_value_to_the_guess() {
    let consultations = Arc::new(AtomicUsize::new(0));
    let guesses = Arc::new(Mutex::new(Vec::new()));

    let mut netlist = Netlist::new();
    let node = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let _gnd = netlist.connect_node(NodeIdentifier::Gnd);

    let dev = StepLimiter {
        node: node.clone(),
        lim: 0.7,
        report: None,
        consultations: consultations.clone(),
        guesses: guesses.clone(),
    };
    let elements: Vec<Box<dyn Element>> = vec![Box::new(dev)];
    let circuit = CircuitInstance::from_devices_and_netlist("lim", elements, netlist);
    let mut solver = Solver::new(circuit).build();
    let res = solver.dc().unwrap().solve().unwrap();

    // The load's natural solution is lim; the limiter steers there faster.
    let v = res.get_node(&NodeIdentifier::Anonymous(1)).unwrap();
    assert!((v - 0.7).abs() < 1e-9, "converged to the limited value, got {v}");

    // ABI-10: the solver consulted limiting_report (apply + gate).
    assert!(
        consultations.load(Ordering::SeqCst) > 0,
        "Newton must call limiting_report each iteration"
    );

    // ABI-10 (apply): the limiter clamped the guess to lim. The recorded
    // guess sequence reaches lim within the first three load calls (dry-run +
    // cold-start + clamped) — a plain midpoint-damped linear solve would still
    // be at ~0.35 by then (sequence [0, 0, 0.35, …]). The early jump to lim
    // proves `apply_limiting_reports` overwrote the guess.
    let seq = guesses.lock().unwrap();
    assert!(
        seq.iter().take(3).any(|&v| (v - 0.7).abs() < 1e-9),
        "the clamped value (0.7) appears early in the guess sequence {:?}, \
         proving the report was applied to the Newton guess",
        *seq
    );
}

/// Spec ABI-10: an always-active limiter vetoes Newton convergence — the
/// solver does NOT declare convergence at a clamped non-solution. A voltage
/// source forcing a node to `vforce` while a limiter clamps the guess to a
/// different `lim` cannot settle, so the solve fails to converge (the limiter
/// keeps vetoing). This proves the gate reads `limiting_report`, not a stale
/// boolean.
#[test]
fn active_limiter_vetoes_newton_convergence() {
    let consultations = Arc::new(AtomicUsize::new(0));
    let guesses = Arc::new(Mutex::new(Vec::new()));

    let mut netlist = Netlist::new();
    let node = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let vb = netlist.connect_branch(BranchIdentifier::from_component("v1"));

    // Vsource forces node to 1.0 V; limiter permanently clamps the guess to
    // 0.3 V (never releases) — the two can never agree, so convergence is
    // permanently vetoed.
    struct AlwaysLim(StepLimiter);
    impl AnalogDevice for AlwaysLim {
        fn load_dc(&mut self, s: &DcAnalysisState<'_>, _c: &Context) -> Vec<Stamp<AnalogReference, f64>> {
            let v = self.0.node.idx().and_then(|i| s.latest().and_then(|r| r.get(i).copied())).unwrap_or(0.0);
            self.0.guesses.lock().unwrap().push(v);
            // Always active — clamp to lim regardless of proximity.
            self.0.report = Some(LimitingReport {
                device: self.name().to_string(),
                net: self.0.node.clone(),
                proposed: v,
                limited_value: self.0.lim,
                limiter_name: "alwolim",
                reason: LimitReason::VoltageStep,
            });
            Vec::new()
        }
        fn limiting_report(&self) -> Option<LimitingReport> {
            self.0.consultations.fetch_add(1, Ordering::SeqCst);
            self.0.report.clone()
        }
    }
    impl DigitalDevice for AlwaysLim {}
    impl Introspect for AlwaysLim {}
    impl Element for AlwaysLim {
        fn name(&self) -> &str { "alwolim" }
        fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC }
    }

    let dev = AlwaysLim(StepLimiter {
        node: node.clone(),
        lim: 0.3,
        report: None,
        consultations: consultations.clone(),
        guesses: guesses.clone(),
    });
    let elements: Vec<Box<dyn Element>> = vec![
        Box::new(Vdc { vforce: 1.0, n1: node.clone(), n2: gnd.clone(), branch: vb }),
        Box::new(dev),
    ];
    let circuit = CircuitInstance::from_devices_and_netlist("veto", elements, netlist);
    let mut solver = Solver::new(circuit).build();
    let res = solver.dc().unwrap().solve();

    // The limiter vetoes convergence permanently → the DC plan fails.
    assert!(res.is_err(), "an always-active limiter must veto convergence");
    assert!(
        consultations.load(Ordering::SeqCst) > 0,
        "the gate consulted limiting_report before vetoing"
    );
}
