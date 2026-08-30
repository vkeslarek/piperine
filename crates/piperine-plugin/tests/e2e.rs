//! End-to-end: Verilog-AMS `@device` instances built by the fixture plugin through
//! an in-process host — analog (`Fixture::Resistor` in a DC solve) and
//! digital (`Fixture::Inverter` through the event scheduler) — driven
//! through the root host API (`Session`). This is the Phase-2 gate of
//! `Plugin plan.md`.
//!
//! SPEC-COVERS: part_vi#1

use std::rc::Rc;

use piperine::{NetRef, OpResult, Session, SolverConfig};
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
/// device provider (no lifecycle hooks — this gate is about `@device`). The
/// `@device` instances are built when the session compiles, so a device error
/// surfaces from `compile()`, not from the solve.
fn run_op(src: &str, module: &str) -> Result<OpResult, piperine::Error> {
    let host = host();
    let design = elab(src, &host);
    let mut session = Session::builder(&design, module).provider(host).compile()?;
    session.op(&SolverConfig::default(), None)
}

fn net(name: &str) -> NetRef {
    NetRef { name: name.to_string() }
}

const ANALOG: &str = r#"
    nature Voltage; units = "V"; access = V; abstol = 1e-6; endnature
    nature Current; units = "A"; access = I; abstol = 1e-12; endnature
    discipline electrical;
        potential Voltage;
        flow Current;
    enddiscipline

    module VoltageSource (inout electrical p, inout electrical n);
        parameter real voltage = 0.0;
        analog V(p, n) <+ voltage;
    endmodule

    (* device = "fixture", type = "Fixture::Resistor" *)
    module PluginResistor (inout electrical p, inout electrical n);
        parameter real r = 100.0;
    endmodule

    module Top ();
        electrical gnd, vin;
        VoltageSource #(.voltage(5.0)) source (.p(vin), .n(gnd));
        PluginResistor r1 (.p(vin), .n(gnd));
    endmodule
"#;

#[test]
fn plugin_resistor_solves_dc() {
    let op = run_op(ANALOG, "Top").expect("op solves");
    let vin = op.v((net("vin"), net("gnd"))).expect("v(vin)");
    assert!((4.999..5.001).contains(&vin), "vin must sit at 5 V, got {vin}");
    // 5 V across the plugin's default 100R: |I| through the source branch
    // must be 50 mA.
    let i = op.i((net("vin"), net("gnd"))).expect("i(vin,gnd)");
    assert!((0.0024..0.0026).contains(&(i * i)), "|i| ~ 50 mA, got {i}");
}

#[test]
fn plugin_resistor_honors_param_override() {
    // Same circuit, r overridden to 50R → 100 mA.
    let src = ANALOG.replace(
        "PluginResistor r1 (.p(vin), .n(gnd));",
        "PluginResistor #(.r(50.0)) r1 (.p(vin), .n(gnd));",
    );
    let op = run_op(&src, "Top").expect("op solves");
    let i = op.i((net("vin"), net("gnd"))).expect("i(vin,gnd)");
    assert!((0.0099..0.0101).contains(&(i * i)), "|i| ~ 100 mA, got {i}");
}

const DIGITAL: &str = r#"
    discipline logic;
        domain discrete;
    enddiscipline

    module High (output logic y);
        always @(*) begin
            y <= 1;
        end
    endmodule

    (* device = "fixture", type = "Fixture::Inverter" *)
    module PluginInv (a, y);
        (* port = "a", kind = "digital" *) input logic a;
        (* port = "y", kind = "digital" *) output logic y;
    endmodule

    module DTop ();
        logic a, y;
        High hi (.y(a));
        PluginInv inv (.a(a), .y(y));
    endmodule
"#;

#[test]
fn plugin_inverter_runs_through_scheduler() {
    let op = run_op(DIGITAL, "DTop").expect("op solves");
    assert_eq!(op.v(net("a")).expect("v(a)"), 1.0, "driver output high");
    assert_eq!(op.v(net("y")).expect("v(y)"), 0.0, "plugin inverter output low");
}

#[test]
fn device_without_host_fails_loud() {
    // No provider wired: the @device instance must be a loud error, never
    // a silently-missing device.
    let design = piperine_lang::parse_and_elaborate_seeded(ANALOG, &SourceMap::dummy(), |ctx| {
        host().seed_schemas(ctx);
    })
    .expect("elaborate");
    let msg = match Session::compile(&design, "Top") {
        Err(e) => e.to_string(),
        Ok(_) => panic!("a @device instance with no provider must fail the build"),
    };
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
