//! T22 (DVT-29) — the parts of `expect`/`warn` that need a real analysis
//! run through the `dv-core` findings channel: posture (`strict` aborts on
//! the first `Error`, `collect` records all, `off` polls nothing), and a
//! coroutine exception surfacing as a time-carrying error with the
//! analysis stopped cleanly (never a raw panic escaping `solve()`).

use piperine_api::sequencer::SequencerControl;
use piperine_api::{HostAdapter, Sequencer, SequencerProgram, Wake};
use piperine_solver::abi::{
    AnalogDevice, AnalogReference, CheckPosture, Context, DcAnalysisState, DigitalDevice, Element,
    ElementCapabilities, Introspect, Stamp, TransientAnalysisContext, TransientAnalysisState, ValidationSeverity,
};
use piperine_solver::prelude::{CircuitBuilder, TransientAnalysisOptions};

/// A trivial resistor to ground — just enough to make the circuit
/// well-posed; the sequencer under test is the only interesting device.
struct GroundResistor {
    node: AnalogReference,
}
impl AnalogDevice for GroundResistor {
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
impl GroundResistor {
    fn stamps(&self) -> Vec<Stamp<AnalogReference, f64>> {
        vec![Stamp::Matrix(self.node.clone(), self.node.clone(), 1e-3)]
    }
}
impl DigitalDevice for GroundResistor {}
impl Introspect for GroundResistor {}
impl Element for GroundResistor {
    fn name(&self) -> &str {
        "gr"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_TRAN
    }
}

fn build_circuit(sequencer: Box<dyn Element>) -> piperine_solver::prelude::CircuitInstance {
    let mut builder = CircuitBuilder::new("posture");
    let n = builder.node("n");
    builder.element(Box::new(GroundResistor { node: n }));
    builder.element(sequencer);
    builder.build().expect("the posture fixture builds")
}

fn run(
    sequencer: Box<dyn Element>,
    checks: CheckPosture,
) -> piperine_solver::abi::Result<piperine_solver::prelude::TransientAnalysisResult> {
    let mut circuit = build_circuit(sequencer);
    let opts = TransientAnalysisOptions::new(20e-6, 1e-6);
    let ctx = Context { checks, ..Context::default() };
    let mut tran = circuit.transient(opts, ctx).expect("tran setup");
    tran.solve()
}

/// A program that always fails one `expect` at a fixed instant, then
/// finishes.
struct AlwaysFails;
impl SequencerProgram for AlwaysFails {
    fn start(&mut self, _ctl: &mut SequencerControl) -> Wake {
        Wake::Advance(5e-6)
    }
    fn resume(&mut self, t: f64, ctl: &mut SequencerControl) -> Wake {
        ctl.expect(false, "this always fails", t, "seq0");
        Wake::Done
    }
}

#[test]
fn strict_posture_aborts_on_the_first_expect_failure_naming_time_and_instance() {
    let sequencer = Box::new(HostAdapter(Sequencer::new("seq0", AlwaysFails, 0.0, 20e-6)));
    let err = run(sequencer, CheckPosture::Strict).expect_err("a failing expect under strict must abort");
    let msg = err.to_string();
    assert!(msg.contains("seq0"), "names the instance: {msg}");
    assert!(msg.contains("5e-6") || msg.contains("0.000005") || msg.contains("5.000000e-6") || msg.contains("5"), "names the time: {msg}");
}

#[test]
fn collect_posture_records_the_failure_and_completes() {
    let sequencer = Box::new(HostAdapter(Sequencer::new("seq0", AlwaysFails, 0.0, 20e-6)));
    let result = run(sequencer, CheckPosture::Collect).expect("collect never aborts the analysis");
    let errors: Vec<_> =
        result.stats.validation.iter().filter(|f| f.severity == ValidationSeverity::Error).collect();
    assert_eq!(errors.len(), 1, "the one expect failure must be recorded exactly once: {:?}", result.stats.validation);
    assert_eq!(errors[0].instance_path, "seq0");
}

/// DVT-29's explicit `checks=off` interaction: the same failing expect
/// records nothing and never aborts — `off` never even polls the channel.
#[test]
fn checks_off_makes_expect_failures_invisible_and_the_analysis_completes() {
    let sequencer = Box::new(HostAdapter(Sequencer::new("seq0", AlwaysFails, 0.0, 20e-6)));
    let result = run(sequencer, CheckPosture::Off).expect("off never aborts, regardless of what expect recorded");
    assert!(result.stats.validation.is_empty(), "off must poll nothing: {:?}", result.stats.validation);
}

/// A `warn` never aborts, even under strict — the contrast case to
/// `AlwaysFails`.
struct AlwaysWarns;
impl SequencerProgram for AlwaysWarns {
    fn start(&mut self, _ctl: &mut SequencerControl) -> Wake {
        Wake::Advance(5e-6)
    }
    fn resume(&mut self, t: f64, ctl: &mut SequencerControl) -> Wake {
        ctl.warn("just a note", t, "seq0");
        Wake::Done
    }
}

#[test]
fn warn_never_aborts_even_under_strict_posture() {
    let sequencer = Box::new(HostAdapter(Sequencer::new("seq0", AlwaysWarns, 0.0, 20e-6)));
    let result = run(sequencer, CheckPosture::Strict).expect("a Warning must never abort strict");
    assert_eq!(result.stats.validation.len(), 1);
    assert_eq!(result.stats.validation[0].severity, ValidationSeverity::Warning);
}

/// T22: a coroutine exception (a Rust panic inside `resume`) surfaces as a
/// time-carrying error rather than an unhandled panic unwinding through
/// the solver — proven two ways: `solve()` itself never panics (wrapped in
/// `catch_unwind` here as an outer safety net), and the resulting `Err`
/// names the simulation time.
#[test]
fn a_coroutine_panic_surfaces_as_a_time_carrying_error_not_a_raw_unwind() {
    struct Explodes;
    impl SequencerProgram for Explodes {
        fn start(&mut self, _ctl: &mut SequencerControl) -> Wake {
            Wake::Advance(7e-6)
        }
        fn resume(&mut self, _t: f64, _ctl: &mut SequencerControl) -> Wake {
            panic!("boom: a coroutine-level bug");
        }
    }

    let sequencer = Box::new(HostAdapter(Sequencer::new("seq0", Explodes, 0.0, 20e-6)));
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(sequencer, CheckPosture::Strict)));
    let result = outcome.expect("the panic must be caught inside the sequencer, never escape solve()");
    let err = result.expect_err("a coroutine panic must surface as an Err, not a silent success");
    let msg = err.to_string();
    assert!(msg.contains("seq0"), "names the instance: {msg}");
    assert!(msg.contains("boom"), "carries the panic message: {msg}");
    assert!(
        msg.contains("7e-6") || msg.contains("0.000007") || msg.contains("7.000000e-6") || msg.contains("7"),
        "names the simulation time: {msg}"
    );
}
