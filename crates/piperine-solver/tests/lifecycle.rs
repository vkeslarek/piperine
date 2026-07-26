//! Lifecycle contract test (ABI-16): a recording `Element` instruments every
//! `Element`/`AnalogDevice`/`DigitalDevice`/`Introspect` hook, each analysis
//! is run against it, and the recorded hook ordering is asserted against the
//! documented chart in Part VII §19. A documentation-completeness assertion
//! guards Part VII §19 itself: every analysis subsection must be non-empty.
//!
//! The existing `LifecycleTestDevice` (setup/destroy counting) is retained
//! below; the recording device extends that pattern to cover the full hook
//! surface added across the element-abi-maturity feature (checkpoint/restore,
//! limiting_report, set_temperature, the per-analysis load methods).

use piperine_solver::abi::*;
use piperine_solver::prelude::{
    AcSweepAnalysisOptions, AnalogReference, Context, DcAnalysisResult, Net,
    Netlist, NodeIdentifier, NoiseAnalysisOptions, NoiseKind, PssAnalysisOptions,
    SensAnalysisOptions, Solver, TransientAnalysisOptions,
};
use num_complex::Complex64;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

// ── legacy setup/destroy counter (unchanged) ──────────────────────────────

struct LifecycleTestDevice {
    setup_calls: Arc<AtomicUsize>,
    destroy_calls: Arc<AtomicUsize>,
    fail_setup: bool,
}

impl AnalogDevice for LifecycleTestDevice {}

impl DigitalDevice for LifecycleTestDevice {}

impl Introspect for LifecycleTestDevice {}

impl Element for LifecycleTestDevice {
    fn name(&self) -> &str {
        "LifecycleTestDevice"
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC
    }

    fn setup(&mut self, _ctx: &Context) -> Result<()> {
        self.setup_calls.fetch_add(1, Ordering::SeqCst);
        if self.fail_setup {
            Err(Error::simple(SolverDomain::Element, "test setup error"))
        } else {
            Ok(())
        }
    }

    fn destroy(&mut self) {
        self.destroy_calls.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn test_lifecycle_hooks_called() {
    let setup_calls = Arc::new(AtomicUsize::new(0));
    let destroy_calls = Arc::new(AtomicUsize::new(0));

    let dev = LifecycleTestDevice {
        setup_calls: setup_calls.clone(),
        destroy_calls: destroy_calls.clone(),
        fail_setup: false,
    };

    let netlist = Netlist::new();
    let mut circuit = CircuitInstance::from_devices_and_netlist(
        "test_circuit",
        vec![Box::new(dev)],
        netlist,
    );

    let context = Context::default();

    let mut dc = circuit.dc(context.clone()).unwrap();
    assert_eq!(setup_calls.load(Ordering::SeqCst), 1);
    let _ = dc.solve().unwrap();

    let _ac = circuit.ac(context.clone()).unwrap();
    assert_eq!(setup_calls.load(Ordering::SeqCst), 1);
    assert_eq!(destroy_calls.load(Ordering::SeqCst), 0);

    drop(circuit);
    assert_eq!(destroy_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn test_setup_error_propagates() {
    let setup_calls = Arc::new(AtomicUsize::new(0));
    let destroy_calls = Arc::new(AtomicUsize::new(0));

    let dev = LifecycleTestDevice {
        setup_calls: setup_calls.clone(),
        destroy_calls: destroy_calls.clone(),
        fail_setup: true,
    };

    let netlist = Netlist::new();
    let mut circuit = CircuitInstance::from_devices_and_netlist(
        "test_circuit",
        vec![Box::new(dev)],
        netlist,
    );

    let context = Context::default();

    let result = circuit.dc(context);
    assert!(result.is_err());
    assert_eq!(setup_calls.load(Ordering::SeqCst), 1);
}

// ── recording device (ABI-16): full hook surface ──────────────────────────
//
// One `RecordingDevice` stamps a 1 kΩ conductance between a named node and
// ground (so DC/AC/tran/noise/sens all have a real unknown to solve/reference)
// and records every hook call into a shared, ordered log. Each analysis test
// builds a fresh circuit, runs the analysis, drops the circuit (destroy),
// and asserts the recorded ordering matches Part VII §19.

type HookLog = Arc<Mutex<Vec<&'static str>>>;

struct RecordingDevice {
    log: HookLog,
    n1: AnalogReference,
    n2: AnalogReference,
    r: f64,
}

impl RecordingDevice {
    fn new(log: HookLog, n1: AnalogReference, n2: AnalogReference) -> Self {
        Self { log, n1, n2, r: 1000.0 }
    }

    fn record(&self, hook: &'static str) {
        self.log.lock().expect("log lock").push(hook);
    }
}

impl AnalogDevice for RecordingDevice {
    fn set_temperature(&mut self, _t: f64) {
        self.record("set_temperature");
    }

    fn update(&mut self, _state: &CircularArrayBuffer2<f64>, _ctx: &Context) {
        self.record("update");
    }

    fn load_dc(
        &mut self,
        _state: &DcAnalysisState<'_>,
        _ctx: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.record("load_dc");
        let g = 1.0 / self.r;
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
        ]
    }

    fn load_ac(
        &mut self,
        _dc_op: &DcAnalysisResult,
        _ac_ctx: &AcAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, Complex64>> {
        self.record("load_ac");
        let g = Complex64::new(1.0 / self.r, 0.0);
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
        ]
    }

    fn load_transient(
        &mut self,
        _states: &TransientAnalysisState<'_>,
        _tran_ctx: &TransientAnalysisContext,
        _context: &Context,
    ) -> Vec<Stamp<AnalogReference, f64>> {
        self.record("load_transient");
        let g = 1.0 / self.r;
        vec![
            Stamp::Matrix(self.n1.clone(), self.n1.clone(), g),
            Stamp::Matrix(self.n2.clone(), self.n2.clone(), g),
            Stamp::Matrix(self.n1.clone(), self.n2.clone(), -g),
            Stamp::Matrix(self.n2.clone(), self.n1.clone(), -g),
        ]
    }

    fn limiting_report(&self) -> Option<LimitingReport> {
        self.record("limiting_report");
        None
    }

    fn noise_current_psd(
        &mut self,
        _dc_point: &DcAnalysisResult,
        _ac_context: &AcAnalysisContext,
    ) -> Vec<Noise> {
        self.record("noise_current_psd");
        let thermal_psd = 4.0 * 1.380649e-23 * 300.0 / self.r;
        vec![Noise::new(
            (self.n1.clone(), self.n2.clone()),
            thermal_psd,
        )
        .named("thermal", NoiseKind::Thermal)]
    }
}

impl DigitalDevice for RecordingDevice {}

impl Introspect for RecordingDevice {
    fn list_params(&self) -> Vec<ParamDescriptor> {
        vec![ParamDescriptor {
            name: "r".into(),
            kind: ValueKind::Real,
            default: Value::Real(1000.0),
            unit: Some("ohm".into()),
            bounds: Bounds { min: Some(0.0), max: None },
            scope: ParamScope::Instance,
            invalidation: Invalidation::Restamp,
        }]
    }

    fn get_param(&self, name: &str) -> Option<Value> {
        (name == "r").then_some(Value::Real(self.r))
    }

    fn set_param(&mut self, name: &str, value: Value) -> std::result::Result<Invalidation, ParamError> {
        self.record("set_param");
        if name != "r" {
            return Err(ParamError::Unknown(name.into()));
        }
        let Some(v) = value.as_real() else {
            return Err(ParamError::TypeMismatch {
                name: name.into(),
                expected: ValueKind::Real,
            });
        };
        if v <= 0.0 {
            return Err(ParamError::OutOfRange { name: name.into(), value });
        }
        self.r = v;
        Ok(Invalidation::Restamp)
    }
}

impl Element for RecordingDevice {
    fn name(&self) -> &str {
        "RecordingDevice"
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
            | ElementCapabilities::LOADS_DC
            | ElementCapabilities::LOADS_AC
            | ElementCapabilities::LOADS_TRAN
            | ElementCapabilities::EMITS_NOISE
    }

    fn setup(&mut self, _ctx: &Context) -> Result<()> {
        self.record("setup");
        Ok(())
    }

    fn destroy(&mut self) {
        self.record("destroy");
    }

    fn accept_timestep(
        &mut self,
        _state: &CircularArrayBuffer2<f64>,
        _t: f64,
        _nets: &[LogicValue],
        _sink: &mut dyn EventSink,
    ) {
        self.record("accept_timestep");
    }

    fn checkpoint_state(&self) -> Option<ElementCheckpoint> {
        self.record("checkpoint_state");
        None
    }

    fn restore_state(&mut self, _checkpoint: &ElementCheckpoint) {
        self.record("restore_state");
    }
}

/// Build a fresh circuit with one `RecordingDevice` stamping a conductance
/// between `top` and ground. Returns the circuit + the hook log + the top
/// node reference (for noise/sens output addressing).
fn recording_circuit() -> (
    CircuitInstance,
    HookLog,
    AnalogReference,
) {
    let log: HookLog = Arc::new(Mutex::new(Vec::new()));
    let mut netlist = Netlist::new();
    let top = netlist.connect_node(NodeIdentifier::Anonymous(1));
    let gnd = netlist.connect_node(NodeIdentifier::Gnd);
    let dev = RecordingDevice::new(log.clone(), top.clone(), gnd);
    let circuit = CircuitInstance::from_devices_and_netlist(
        "lifecycle-contract",
        vec![Box::new(dev)],
        netlist,
    );
    (circuit, log, top)
}

/// Drain the log into a `Vec<&'static str>` snapshot.
fn snapshot(log: &HookLog) -> Vec<&'static str> {
    let mut g = log.lock().expect("log lock");
    std::mem::take(&mut *g)
}

/// Assert `log` contains `expected` as an ordered subsequence (each element
/// appears at or after the position of the previous one). Asserts the chart
/// ordering from Part VII §19 holds for the hooks that actually fired.
fn assert_ordered_subsequence(log: &[&'static str], expected: &[&'static str]) {
    let mut from = 0;
    for want in expected {
        match log[from..].iter().position(|h| h == want) {
            Some(pos) => from += pos + 1,
            None => {
                panic!(
                    "hook `{want}` not found in remaining log after index {from}.\n\
                     full log: {log:?}\nexpected subsequence: {expected:?}"
                );
            }
        }
    }
}

// ── per-analysis ordering tests (Part VII §19) ────────────────────────────

/// §19.2 DC: setup → set_temperature → update → load_dc → limiting_report
/// → accept_timestep (mixed-signal settle) → destroy.
#[test]
fn dc_hook_ordering_matches_chart() {
    let (mut circuit, log, _top) = recording_circuit();
    {
        let mut dc = circuit.dc(Context::default()).unwrap();
        let _ = dc.solve().unwrap();
    }
    drop(circuit);
    let seq = snapshot(&log);
    assert_ordered_subsequence(
        &seq,
        &["setup", "set_temperature", "update", "load_dc", "limiting_report", "destroy"],
    );
}

/// §19.3 AC: DC operating point (setup → set_temperature → load_dc) then
/// load_ac per frequency point → destroy.
#[test]
fn ac_hook_ordering_matches_chart() {
    let (circuit, log, _top) = recording_circuit();
    {
        let mut solver = Solver::new(circuit).build();
        let _ = solver
            .ac()
            .unwrap()
            .solve_sweep(AcSweepAnalysisOptions {
                start_frequency: 1.0,
                stop_frequency: 10.0,
                steps: 2,
                logarithmic: false,
            })
            .unwrap();
    }
    let seq = snapshot(&log);
    assert_ordered_subsequence(
        &seq,
        &["setup", "set_temperature", "load_dc", "load_ac", "destroy"],
    );
}

/// §19.4 tran: setup → set_temperature → checkpoint_state → update →
/// load_transient → limiting_report → accept_timestep → destroy.
#[test]
fn tran_hook_ordering_matches_chart() {
    let (mut circuit, log, _top) = recording_circuit();
    {
        let opts = TransientAnalysisOptions::new(1e-6, 1e-7);
        let _ = circuit.transient(opts, Context::default()).unwrap().solve().unwrap();
    }
    drop(circuit);
    let seq = snapshot(&log);
    assert_ordered_subsequence(
        &seq,
        &[
            "setup",
            "set_temperature",
            "checkpoint_state",
            "update",
            "load_transient",
            "limiting_report",
            "accept_timestep",
            "destroy",
        ],
    );
}

/// §19.5 noise: DC operating point (setup → set_temperature → load_dc →
/// load_ac) then noise_current_psd per device per frequency → destroy.
#[test]
fn noise_hook_ordering_matches_chart() {
    let (mut circuit, log, top) = recording_circuit();
    {
        let opts = NoiseAnalysisOptions {
            sweep_options: AcSweepAnalysisOptions {
                start_frequency: 1.0,
                stop_frequency: 10.0,
                steps: 2,
                logarithmic: false,
            },
            output_node: match top.variable().as_ref() {
                AnalogVariable::Node(n) => n.clone(),
                _ => NodeIdentifier::Anonymous(1),
            },
            reference_node: NodeIdentifier::Gnd,
            input_source_name: None,
        };
        let _ = circuit.noise(opts, Context::default()).unwrap().solve().unwrap();
    }
    drop(circuit);
    let seq = snapshot(&log);
    assert_ordered_subsequence(
        &seq,
        &["setup", "set_temperature", "load_dc", "load_ac", "noise_current_psd", "destroy"],
    );
}

/// §19.6 PSS: setup → set_temperature → (per-shot transient load_transient)
/// → destroy. The shooting re-enters via digital_hidden_restore; the
/// recording device is pure-analog so that hook does not fire, but the
/// inner transient's load_transient must appear before destroy.
#[test]
fn pss_hook_ordering_matches_chart() {
    let (mut circuit, log, _top) = recording_circuit();
    {
        let opts = PssAnalysisOptions::new(1e-6);
        let _ = circuit.pss(opts, Context::default()).unwrap().solve().unwrap();
    }
    drop(circuit);
    let seq = snapshot(&log);
    assert_ordered_subsequence(
        &seq,
        &["setup", "set_temperature", "load_transient", "destroy"],
    );
}

/// §19.7 .sens: setup → set_temperature → set_param (central-difference
/// perturbation) → load_dc (re-solve at each side) → destroy.
#[test]
fn sens_hook_ordering_matches_chart() {
    let (mut circuit, log, top) = recording_circuit();
    {
        let output: Net = (&top).into();
        let opts = SensAnalysisOptions::new(
            vec![output],
            vec![("RecordingDevice".to_string(), "r".to_string())],
        );
        let _ = circuit.sens(opts, Context::default()).unwrap().solve().unwrap();
    }
    drop(circuit);
    let seq = snapshot(&log);
    assert_ordered_subsequence(
        &seq,
        &["setup", "set_temperature", "set_param", "load_dc", "destroy"],
    );
}

// ── Part VII §19 completeness assertion (ABI-15) ──────────────────────────
//
// The lifecycle chart lives in `docs/spec/part_vii_solver.md` §19. Each
// analysis subsection (§19.2–§19.7) must be present and non-empty — an
// external device author reads that chart to know when every hook fires.
// This test guards against a section being stubbed out or accidentally
// deleted.

#[test]
fn part_vii_section_19_has_nonempty_algorithm_flow_for_every_analysis() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/spec/part_vii_solver.md");
    let doc = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("could not read Part VII at {}: {e}", path.display())
    });

    for (analysis, section) in [
        ("DC", "19.2"),
        ("AC", "19.3"),
        ("tran", "19.4"),
        ("noise", "19.5"),
        ("PSS", "19.6"),
        (".sens", "19.7"),
    ] {
        let header = format!("### {section}");
        let start = doc.find(&header).unwrap_or_else(|| {
            panic!("Part VII §{section} ({analysis}) header missing")
        });
        let body_start = start + header.len();
        let next_section = doc[body_start..]
            .find("\n### ")
            .or_else(|| doc[body_start..].find("\n## " ))
            .map(|p| body_start + p)
            .unwrap_or(doc.len());
        let body = doc[body_start..next_section].trim();
        assert!(
            !body.is_empty(),
            "Part VII §{section} ({analysis}) is empty"
        );
        assert!(
            body.contains("Algorithm flow"),
            "Part VII §{section} ({analysis}) missing 'Algorithm flow' description"
        );
        assert!(
            body.contains("Hook ordering table"),
            "Part VII §{section} ({analysis}) missing 'Hook ordering table'"
        );
    }
}
