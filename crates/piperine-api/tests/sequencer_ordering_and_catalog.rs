//! T26 (DVT-28/44) — ordering and conflicts among several host devices at
//! one instant, plus the feature's fail-loud catalog, cross-referenced
//! against what earlier tasks already built rather than re-implemented:
//!
//! | Catalog entry | Covered by |
//! |---|---|
//! | unknown net (listing candidates) | pre-existing host-surface `NetRef` lookup (`crates/piperine-api/src/session/entry.rs`, `results.rs`) — not sequencer-specific, reused as-is |
//! | driver conflict | `piperine_api::sequencer::DriveRegistry` (this task) — unit-tested in `conflicts.rs`; ordering independence proven there |
//! | non-yielding coroutine | `crates/piperine-api/src/sequencer/driver.rs::tests::a_non_yielding_coroutine_is_loud_not_a_hang` (T19) |
//! | rebuild-class `set_param` | proven below for a **host device** specifically — T14 claimed this "inherited for free" but never tested it end to end |
//! | unfed measure | `piperine-signal`'s `Measure::value`/`error_bound` (T13); adapted in `crates/piperine-api/src/device/level1.rs::tests::measure_is_fed_on_accept_and_untouched_otherwise` |
//! | AC with a sequencer | `crates/piperine-api/tests/sequencer_dc_ac.rs::ac_with_a_sequencer_present_fails_loud_naming_the_gap` (T25) |
//!
//! Ordering itself (§12.12): the authored POM instantiation order already
//! governs independent host-device actions at a shared instant — a
//! property of `Vec<Box<dyn Element>>` iteration order the circuit builder
//! already fills in source order. Proven below by construction order,
//! reversed and re-checked.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use piperine_api::sequencer::SequencerControl;
use piperine_api::{HostAdapter, Sequencer, SequencerProgram, Wake};
use piperine_lang::SourceMap;
use piperine_solver::prelude::{CircuitBuilder, Context, TransientAnalysisOptions};

/// Both sequencers ask for a single Advance to the SAME instant, then
/// finish — recording their own label into a shared log.
struct RecordOnce {
    label: &'static str,
    at: f64,
    log: Arc<Mutex<Vec<&'static str>>>,
}
impl SequencerProgram for RecordOnce {
    fn start(&mut self, _ctl: &mut SequencerControl) -> Wake {
        Wake::Advance(self.at)
    }
    fn resume(&mut self, _t: f64, _ctl: &mut SequencerControl) -> Wake {
        self.log.lock().unwrap().push(self.label);
        Wake::Done
    }
}

/// Several sequencers union their breakpoints (each is resumed
/// independently — proven by both appearing in the log at all), and at a
/// shared instant they act in **authored instantiation order** —
/// deterministic and visible: reversing the build order reverses the
/// observed resume order.
#[test]
fn several_sequencers_resume_independently_and_in_authored_instantiation_order() {
    let shared_instant = 10e-6;

    let run = |first: &'static str, second: &'static str| -> Vec<&'static str> {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut builder = CircuitBuilder::new("ordering");
        builder.element(Box::new(HostAdapter(Sequencer::new(
            first,
            RecordOnce { label: first, at: shared_instant, log: log.clone() },
            0.0,
            shared_instant,
        ))));
        builder.element(Box::new(HostAdapter(Sequencer::new(
            second,
            RecordOnce { label: second, at: shared_instant, log: log.clone() },
            0.0,
            shared_instant,
        ))));
        let mut circuit = builder.build().expect("builds");
        let opts = TransientAnalysisOptions::new(shared_instant, shared_instant / 10.0);
        let mut tran = circuit.transient(opts, Context::default()).expect("tran setup");
        let _ = tran.solve().expect("tran solves");
        drop(tran);
        drop(circuit);
        log.lock().unwrap().clone()
    };

    let forward = run("seq_a", "seq_b");
    assert_eq!(forward, vec!["seq_a", "seq_b"], "both resumed at the shared instant, in build order");

    let reversed = run("seq_b", "seq_a");
    assert_eq!(
        reversed,
        vec!["seq_b", "seq_a"],
        "reversing the authored instantiation order reverses the observed resume order — the \
         ordering is deterministic and comes from build order, not from labels or addresses"
    );
}

// ── rebuild-class set_param from a host device ──────────────────────────────

const HOST_DEVICE_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

@device(plugin = \"none\", type = \"host_r\")
mod HostR(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }

mod Top() {
    wire gnd : Electrical;
    wire vout : Electrical;
    r : HostR(.p = vout, .n = gnd);
}
";

struct HostR {
    label: String,
    p: piperine_solver::abi::AnalogReference,
    n: piperine_solver::abi::AnalogReference,
    g: f64,
}
impl piperine_api::HostElement for HostR {
    fn name(&self) -> &str {
        &self.label
    }
    fn capabilities(&self) -> piperine_solver::abi::ElementCapabilities {
        piperine_solver::abi::ElementCapabilities::ANALOG | piperine_solver::abi::ElementCapabilities::LOADS_DC
    }
    fn load_dc(&mut self, _ctx: &piperine_api::Ctx<'_>, sink: &mut piperine_api::StampSink) {
        sink.g(&self.p, &self.p, self.g);
        sink.g(&self.p, &self.n, -self.g);
        sink.g(&self.n, &self.p, -self.g);
        sink.g(&self.n, &self.n, self.g);
    }
    fn list_params(&self) -> Vec<piperine_solver::abi::ParamDescriptor> {
        vec![]
    }
    fn get_param(&self, name: &str) -> Option<piperine_solver::abi::Value> {
        if name == "topology" {
            Some(piperine_solver::abi::Value::Real(0.0))
        } else {
            None
        }
    }
    fn set_param(
        &mut self,
        name: &str,
        _value: piperine_solver::abi::Value,
    ) -> Result<piperine_solver::abi::Invalidation, piperine_solver::abi::ParamError> {
        match name {
            // A restamp-class write: fine, the live-parameter path handles it.
            "r" => Ok(piperine_solver::abi::Invalidation::Restamp),
            // A rebuild-class write: this host device is declaring that
            // changing "topology" needs a structural rebuild it cannot do
            // in place — MD-18.
            "topology" => Ok(piperine_solver::abi::Invalidation::Rebuild),
            _ => Err(piperine_solver::abi::ParamError::Unknown(name.to_string())),
        }
    }
}

fn host_r_factory(
    spec: piperine_api::HostDeviceSpec,
) -> Result<Box<dyn piperine_solver::abi::Element>, String> {
    let node = |b: &piperine_codegen::device::PortBinding| match b {
        piperine_codegen::device::PortBinding::Analog(r) => r.clone(),
        piperine_codegen::device::PortBinding::Digital(_) => panic!("no digital ports"),
    };
    let p = node(&spec.ports[0]);
    let n = node(&spec.ports[1]);
    Ok(Box::new(HostAdapter(HostR { label: spec.instance_label.clone(), p, n, g: 1e-3 })))
}

/// T14 claimed a host device's rebuild-class `set_param` is "inherited for
/// free" through `Session::set`'s existing generic path — proven here for
/// a real host device rather than assumed: a restamp-class write succeeds,
/// a rebuild-class write on the SAME device fails loud (MD-18), never a
/// silent mid-analysis re-elaboration.
#[test]
fn rebuild_class_set_param_on_a_host_device_fails_loud() {
    let design = piperine_lang::parse_and_elaborate_seeded(
        HOST_DEVICE_PHDL,
        &SourceMap::dummy(),
        piperine_lang::seed_device_port_schemas,
    )
    .expect("elaborates");

    let registry = Rc::new(piperine_api::HostDeviceRegistry::new());
    registry.register("host_r", host_r_factory);

    let mut session =
        piperine_api::Session::builder(&design, "Top").provider(registry).compile().expect("compiles");

    // Restamp-class: succeeds.
    session.set("r", "r", 2e3).expect("a restamp-class write on a host device must succeed");

    // Rebuild-class: fails loud, never a silent re-elaboration.
    let err = session
        .set("r", "topology", 1.0)
        .expect_err("a rebuild-class write on a host device must fail loud, not silently rebuild");
    let msg = err.to_string();
    assert!(msg.contains("rebuild"), "names the reason: {msg}");
}
