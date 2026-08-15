//! T62 (LRMC-C10-13): `PluginHost::put_value` — `vpi_put_value()`'s native
//! equivalent, symmetric with the existing `SolveResultView` read path.

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;
use piperine_plugin::PluginHost;

const DIVIDER: &str = r#"
    module VoltageSource (inout electrical p, inout electrical n);
        parameter real voltage = 5.0;
        analog begin
            V(p, n) <+ voltage;
        end
    endmodule
    module Resistor (inout electrical p, inout electrical n);
        parameter real r = 1e3;
        analog begin
            I(p, n) <+ V(p, n) / r;
        end
    endmodule
    module Top ();
        electrical gnd, vin, out;
        VoltageSource src (.p(vin), .n(gnd));
        Resistor r1 (.p(vin), .n(out));
        Resistor r2 (.p(out), .n(gnd));
    endmodule
"#;

fn design() -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(DIVIDER, &SourceMap::dummy()).expect("elaborate")
}

/// Writing r2's `r` param through `put_value` changes the subsequent DC
/// solve numerically — a real effect on the running solve, not a no-op.
#[test]
fn put_value_changes_the_subsequent_solve_numerically() {
    let d = design();
    let mut session = Session::compile(&d, "Top").expect("compiles");
    let host = PluginHost::empty();

    // Equal 1k/1k divider: 2.5V at `out`.
    let before = session.op(&SolverConfig::default(), None).expect("op solves");
    let v_before = before.v("out".to_string()).expect("v(out)");
    assert!((v_before - 2.5).abs() < 0.01, "equal divider should read ~2.5V, got {v_before}");

    // r2 = 3k: out = 5 * 3k/(1k+3k) = 3.75V.
    host.put_value(&mut session, "r2", "r", 3.0e3).expect("put_value succeeds");
    let after = session.op(&SolverConfig::default(), None).expect("op solves after put_value");
    let v_after = after.v("out".to_string()).expect("v(out)");
    assert!((v_after - 3.75).abs() < 0.01, "3k/1k divider should read ~3.75V, got {v_after}");
}

/// MD-18: the write restamps the compiled circuit, never re-JITs — asserted
/// via `Session::rebuilds()` staying at zero across a `put_value` call and
/// a subsequent solve.
#[test]
fn put_value_never_triggers_a_rebuild() {
    let d = design();
    let mut session = Session::compile(&d, "Top").expect("compiles");
    let host = PluginHost::empty();
    assert_eq!(session.rebuilds(), 0);

    host.put_value(&mut session, "r1", "r", 2.0e3).expect("put_value succeeds");
    session.op(&SolverConfig::default(), None).expect("op solves");
    assert_eq!(session.rebuilds(), 0, "a parameter write must restamp, never rebuild (MD-18)");
}

/// An illegal write target (an unknown param) fails loud — no partial
/// apply, no silent success.
#[test]
fn put_value_on_an_unknown_param_fails_loud() {
    let d = design();
    let mut session = Session::compile(&d, "Top").expect("compiles");
    let host = PluginHost::empty();

    let err = host.put_value(&mut session, "r1", "bogus", 1.0).expect_err("unknown param must fail");
    assert!(format!("{err}").contains("bogus"), "error must name the offending param: {err}");
}

/// An illegal write target (an unknown device label) also fails loud.
#[test]
fn put_value_on_an_unknown_device_fails_loud() {
    let d = design();
    let mut session = Session::compile(&d, "Top").expect("compiles");
    let host = PluginHost::empty();

    let err = host.put_value(&mut session, "no_such_device", "r", 1.0).expect_err("unknown device must fail");
    assert!(!format!("{err}").is_empty());
}
