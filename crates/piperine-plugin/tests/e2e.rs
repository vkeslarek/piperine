//! End-to-end: PHDL `@device` instances built by the fixture plugin through
//! an in-process host — analog (`Fixture::Resistor` in a DC solve) and
//! digital (`Fixture::Inverter` through the event scheduler) — driven
//! through the root host API (`SimSession`). This is the Phase-2 gate of
//! `Plugin plan.md`.

use std::rc::Rc;

use piperine::{NetRef, OpResult, SimSession, SolverConfig};
use piperine_lang::SourceMap;
use piperine_plugin::PluginHost;

#[path = "../examples/fixture_plugin.rs"]
mod fixture_plugin;
use fixture_plugin::FixturePlugin;

fn host() -> Rc<PluginHost> {
    Rc::new(PluginHost::from_plugins(vec![Box::new(FixturePlugin::new())]).expect("host"))
}

fn elab(src: &str, host: &PluginHost) -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate_seeded(src, &SourceMap::dummy(), |ctx| {
        host.seed_schemas(ctx);
    })
    .expect("elaborate")
}

/// An operating point of `module` through a session wired with the host's
/// device provider (no lifecycle hooks — this gate is about `@device`).
fn run_op(src: &str, module: &str) -> Result<OpResult, piperine::Error> {
    let host = host();
    let design = elab(src, &host);
    let mut session = SimSession::new(design, module.to_string());
    session.set_device_provider(host);
    session.run_op(&SolverConfig::default(), None)
}

fn net(name: &str) -> NetRef {
    NetRef { name: name.to_string() }
}

const ANALOG: &str = "
    discipline Electrical { potential v: Real; flow i: Real; }

    mod VoltageSource(inout p: Electrical, inout n: Electrical) {
        param voltage: Real = 0.0;
    }
    analog VoltageSource { V(p, n) <- voltage; }

    @device(plugin = \"fixture\", type = \"Fixture::Resistor\")
    mod PluginResistor(inout p: Electrical, inout n: Electrical) {
        param r: Real = 100.0;
    }

    mod Top() {
        wire gnd : Electrical;
        wire vin : Electrical;
        source : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
        r1     : PluginResistor (.p = vin, .n = gnd);
    }
";

#[test]
fn plugin_resistor_solves_dc() {
    let op = run_op(ANALOG, "Top").expect("op solves");
    let vin = op.v(&net("vin"), Some(&net("gnd"))).expect("v(vin)");
    assert!((4.999..5.001).contains(&vin), "vin must sit at 5 V, got {vin}");
    // 5 V across the plugin's default 100R: |I| through the source branch
    // must be 50 mA.
    let i = op.i(&net("vin"), Some(&net("gnd"))).expect("i(vin,gnd)");
    assert!((0.0024..0.0026).contains(&(i * i)), "|i| ~ 50 mA, got {i}");
}

#[test]
fn plugin_resistor_honors_param_override() {
    // Same circuit, r overridden to 50R → 100 mA.
    let src = ANALOG.replace(
        "r1     : PluginResistor (.p = vin, .n = gnd);",
        "r1     : PluginResistor (.p = vin, .n = gnd) { .r = 50.0 };",
    );
    let op = run_op(&src, "Top").expect("op solves");
    let i = op.i(&net("vin"), Some(&net("gnd"))).expect("i(vin,gnd)");
    assert!((0.0099..0.0101).contains(&(i * i)), "|i| ~ 100 mA, got {i}");
}

const DIGITAL: &str = "
    discipline Bit { storage Boolean; }

    mod High(output y: Bit);
    digital High { y <- 1; }

    @device(plugin = \"fixture\", type = \"Fixture::Inverter\")
    mod PluginInv(
        @port(name = \"a\", kind = \"digital\") input a: Bit,
        @port(name = \"y\", kind = \"digital\") output y: Bit,
    );

    mod DTop() {
        wire a : Bit;
        wire y : Bit;
        hi  : High (.y = a);
        inv : PluginInv (.a = a, .y = y);
    }
";

#[test]
fn plugin_inverter_runs_through_scheduler() {
    let op = run_op(DIGITAL, "DTop").expect("op solves");
    assert_eq!(op.v(&net("a"), None).expect("v(a)"), 1.0, "driver output high");
    assert_eq!(op.v(&net("y"), None).expect("v(y)"), 0.0, "plugin inverter output low");
}

#[test]
fn device_without_host_fails_loud() {
    // No provider wired: the @device instance must be a loud error, never
    // a silently-missing device.
    let design = piperine_lang::parse_and_elaborate_seeded(ANALOG, &SourceMap::dummy(), |ctx| {
        host().seed_schemas(ctx);
    })
    .expect("elaborate");
    let session = SimSession::new(design, "Top".to_string());
    let msg = session
        .run_op(&SolverConfig::default(), None)
        .expect_err("must fail")
        .to_string();
    assert!(msg.contains("plugin"), "unexpected message: {msg}");
}

#[test]
fn unregistered_type_fails_loud() {
    let src = ANALOG.replace("Fixture::Resistor", "Fixture::DoesNotExist");
    let msg = run_op(&src, "Top").expect_err("must fail").to_string();
    assert!(msg.contains("DoesNotExist"), "unexpected message: {msg}");
}

#[test]
fn device_attribute_requires_seeded_schema() {
    // Without the host's schema seeding, `@device` is an unknown schema —
    // E2022, at elaboration.
    let err = piperine_lang::parse_and_elaborate(ANALOG, &SourceMap::dummy())
        .expect_err("must fail without seeded schemas");
    assert!(format!("{err:?}").contains("device"), "{err:?}");
}
