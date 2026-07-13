//! End-to-end: PHDL `@device` instances built by the fixture plugin through
//! an in-process host — analog (`Fixture::Resistor` in a DC solve) and
//! digital (`Fixture::Inverter` through the event scheduler). This is the
//! Phase-2 gate of `Plugin plan.md`.

use std::rc::Rc;

use piperine_bench::{BenchOutcome, BenchRunner};
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

fn run(src: &str, module: &str, entry: &str) -> BenchOutcome {
    let host = host();
    let design = elab(src, &host);
    BenchRunner::new(&design)
        .with_device_provider(host)
        .run_entry(module, entry)
}

fn assert_passed(outcome: BenchOutcome) {
    match outcome {
        BenchOutcome::Passed => {}
        BenchOutcome::Failed(m) => panic!("bench assert failed: {m}"),
        BenchOutcome::Error(m) => panic!("bench errored: {m}"),
    }
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
    bench Top {
        fn node_is_pinned() {
            var res = $op();
            $assert(res.v(vin, gnd) > 4.999, \"vin must sit at 5V\");
            $assert(res.v(vin, gnd) < 5.001, \"vin must sit at 5V\");
        }
        fn source_current_matches_r() {
            var res = $op();
            // 5V across the plugin's default 100R: |I| through the source
            // branch must be 50mA.
            var i = res.i(vin, gnd);
            $assert(i * i > 0.0024, \"|i| ~ 50mA\");
            $assert(i * i < 0.0026, \"|i| ~ 50mA\");
        }
    }
";

#[test]
fn plugin_resistor_solves_dc() {
    assert_passed(run(ANALOG, "Top", "node_is_pinned"));
    assert_passed(run(ANALOG, "Top", "source_current_matches_r"));
}

#[test]
fn plugin_resistor_honors_param_override() {
    // Same circuit, r overridden to 50R → 100mA.
    let src = ANALOG.replace(
        "r1     : PluginResistor (.p = vin, .n = gnd);",
        "r1     : PluginResistor (.p = vin, .n = gnd) { .r = 50.0 };",
    );
    let src = src.replace("0.0024", "0.0099").replace("0.0026", "0.0101");
    assert_passed(run(&src, "Top", "source_current_matches_r"));
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
    bench DTop {
        fn inverts() {
            var r = $op();
            $assert(r.v(a) == 1.0, \"driver output high\");
            $assert(r.v(y) == 0.0, \"plugin inverter output low\");
        }
    }
";

#[test]
fn plugin_inverter_runs_through_scheduler() {
    assert_passed(run(DIGITAL, "DTop", "inverts"));
}

#[test]
fn device_without_host_fails_loud() {
    let host = host();
    let design = elab(ANALOG, &host);
    // No provider wired: the @device instance must be a loud error, never
    // a silently-missing device.
    let outcome = BenchRunner::new(&design).run_entry("Top", "node_is_pinned");
    match outcome {
        BenchOutcome::Error(msg) => {
            assert!(msg.contains("plugin"), "unexpected message: {msg}");
        }
        other => panic!("expected a loud error, got {other:?}"),
    }
}

#[test]
fn unregistered_type_fails_loud() {
    let src = ANALOG.replace("Fixture::Resistor", "Fixture::DoesNotExist");
    match run(&src, "Top", "node_is_pinned") {
        BenchOutcome::Error(msg) => {
            assert!(msg.contains("DoesNotExist"), "unexpected message: {msg}");
        }
        other => panic!("expected a loud error, got {other:?}"),
    }
}

#[test]
fn device_attribute_requires_seeded_schema() {
    // Without the host's schema seeding, `@device` is an unknown schema —
    // E2022, at elaboration.
    let err = piperine_lang::parse_and_elaborate(ANALOG, &SourceMap::dummy())
        .expect_err("must fail without seeded schemas");
    assert!(format!("{err:?}").contains("device"), "{err:?}");
}
