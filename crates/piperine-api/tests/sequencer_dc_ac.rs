//! T25 (DVT-36) — what a sequencer-bearing circuit does at DC/OP and AC:
//! the coroutine does not run at DC (`run()` begins with the transient),
//! each installed `Wave` contributes its declared DC value (never
//! `value(0.0)` generically — asserted with a `sigmoid` chosen so the two
//! would visibly disagree), and AC refuses loud (D13) rather than silently
//! freezing the coroutine.

use std::rc::Rc;

use piperine_api::sequencer::install::InstallSite;
use piperine_api::sequencer::SequencerControl;
use piperine_api::{HostAdapter, HostDeviceRegistry, HostElement, Sequencer, SequencerProgram, Wake};
use piperine_lang::SourceMap;
use piperine_signal::wave::Wave;
use piperine_solver::abi::{Context, ElementCapabilities};
use piperine_solver::prelude::CircuitBuilder;

struct RampSource {
    site: InstallSite,
}
impl HostElement for RampSource {
    fn name(&self) -> &str {
        "wave_source"
    }
    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG | ElementCapabilities::LOADS_DC | ElementCapabilities::LOADS_TRAN
    }
    fn load_dc(&mut self, ctx: &piperine_api::Ctx<'_>, sink: &mut piperine_api::StampSink) {
        self.site.stamp(ctx, sink);
    }
    fn load_transient(&mut self, ctx: &piperine_api::Ctx<'_>, sink: &mut piperine_api::StampSink) {
        self.site.stamp(ctx, sink);
    }
}

/// DVT-36: `Wave::sigmoid(lo, hi, t0=0, tau)`'s DC value is `lo` (§12.10) —
/// deliberately chosen with `t0 = 0` so `value(0.0)` (the midpoint between
/// `lo` and `hi`) would visibly disagree with the declared DC convention,
/// discriminating a real fix from a no-op one.
#[test]
fn a_sigmoid_soft_start_dc_bias_matches_the_documented_convention() {
    let mut builder = CircuitBuilder::new("soft_start_dc");
    let gnd = builder.ground();
    let vdd = builder.node("vdd");

    let mut site = InstallSite::new(vdd.clone(), gnd, 1e-3);
    // t0 = 0.0: value(0.0) would be the sigmoid's midpoint (0.9V), NOT lo
    // (0.0V) — the two must disagree for this test to mean anything.
    site.install(Wave::sigmoid(0.0, 1.8, 0.0, 40e-6), 0.0);
    let midpoint_if_buggy = Wave::sigmoid(0.0, 1.8, 0.0, 40e-6).value(0.0);
    assert!((midpoint_if_buggy - 0.9).abs() < 1e-9, "sanity: value(0.0) really is the midpoint, not lo");

    builder.element(Box::new(HostAdapter(RampSource { site })));
    let mut circuit = builder.build().expect("builds");
    let mut dc = circuit.dc(Context::default()).expect("dc setup");
    let result = dc.solve().expect("dc solves");
    let v_vdd = result.get(vdd).expect("vdd is solved");
    assert!(
        (v_vdd - 0.0).abs() < 1e-6,
        "the DC operating point must see the sigmoid's declared DC value (lo = 0.0), not value(0.0) = {midpoint_if_buggy}: got {v_vdd}"
    );
}

/// DVT-36: at DC/OP the coroutine does not run — a program that panics if
/// `resume` is ever called proves this for the common case (no other
/// digital device in the circuit, so the DC solve never touches
/// `accept_timestep`/`on_digital_settled` at all).
#[test]
fn the_coroutine_does_not_run_at_the_dc_operating_point() {
    struct PanicsIfResumed;
    impl SequencerProgram for PanicsIfResumed {
        fn start(&mut self, _ctl: &mut SequencerControl) -> Wake {
            Wake::Advance(1e-6)
        }
        fn resume(&mut self, _t: f64, _ctl: &mut SequencerControl) -> Wake {
            panic!("the coroutine must never run during a DC/OP solve")
        }
    }

    let mut builder = CircuitBuilder::new("no_run_at_dc");
    let gnd = builder.ground();
    let vout = builder.node("vout");
    let mut site = InstallSite::new(vout, gnd, 1e-3);
    site.install(Wave::constant(1.0), 0.0);
    builder.element(Box::new(HostAdapter(RampSource { site })));
    builder.element(Box::new(HostAdapter(Sequencer::new("seq0", PanicsIfResumed, 0.0, 1e-3))));

    let mut circuit = builder.build().expect("builds");
    let mut dc = circuit.dc(Context::default()).expect("dc setup");
    let result = dc.solve();
    assert!(result.is_ok(), "a DC solve with a sequencer present must not panic or fail: {result:?}");
}

const HOST_SEQUENCER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

@device(plugin = \"none\", type = \"seq\")
mod Seq(inout node: Electrical);

mod Top() {
    wire gnd : Electrical;
    wire vout : Electrical;
    s : Seq(.node = vout);
}
";

/// D13 (T25): an AC sweep on a circuit whose device provider declared a
/// sequencer type refuses loud, naming the gap — never a silent freeze.
#[test]
fn ac_with_a_sequencer_present_fails_loud_naming_the_gap() {
    struct NeverRuns;
    impl SequencerProgram for NeverRuns {
        fn start(&mut self, _ctl: &mut SequencerControl) -> Wake {
            Wake::Done
        }
        fn resume(&mut self, _t: f64, _ctl: &mut SequencerControl) -> Wake {
            Wake::Done
        }
    }

    fn seq_factory(spec: piperine_api::HostDeviceSpec) -> Result<Box<dyn piperine_solver::abi::Element>, String> {
        let _ = spec;
        Ok(Box::new(HostAdapter(Sequencer::new("seq", NeverRuns, 0.0, 1e-3))))
    }

    let design = piperine_lang::parse_and_elaborate_seeded(
        HOST_SEQUENCER_PHDL,
        &SourceMap::dummy(),
        piperine_lang::seed_device_port_schemas,
    )
    .expect("elaborates");

    let registry = Rc::new(HostDeviceRegistry::new());
    registry.register_sequencer("seq", seq_factory);

    let mut session = piperine_api::Session::builder(&design, "Top")
        .host_registry(registry)
        .compile()
        .expect("a sequencer-bearing @device compiles");

    let err = session
        .ac(1.0, 1e6, 10, false, &piperine_api::SolverConfig::default())
        .expect_err("AC with a sequencer present must fail loud");
    let msg = err.to_string();
    assert!(msg.contains("D13") || msg.contains("Sequencer") || msg.contains("AC"), "names the gap: {msg}");
}
