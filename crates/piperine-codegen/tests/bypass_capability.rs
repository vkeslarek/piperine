//! Which compiled devices opt into the DC stamp bypass (P6/CLN-12).
//!
//! `ElementCapabilities::BYPASS_OK` promises that a device's DC stamps are a
//! pure function of its terminal voltages, and the solver now honours that
//! promise: one element without the bit disables the cache for the whole
//! circuit (`piperine-solver/tests/stamp_bypass.rs`). So codegen must declare
//! it exactly when the promise holds — every disqualifier below withholds it.

use piperine_lang::parse_and_elaborate;
use piperine_solver::abi::{Element, ElementCapabilities};

const DISCIPLINE: &str = "discipline Electrical { potential v : Real; flow i : Real; }";

/// Compile a one-device `Top` and report whether the device declares
/// `BYPASS_OK`.
fn declares_bypass_ok(module_body: &str, analog_body: &str) -> bool {
    let src = format!(
        "{DISCIPLINE}
mod Dut(inout p: Electrical, inout n: Electrical) {{ {module_body} }}
analog Dut {{ {analog_body} }}

mod Top() {{
    wire gnd : Electrical;
    wire a   : Electrical;
    d1 : Dut (.p = a, .n = gnd);
}}
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let circuit = piperine_codegen::CircuitCompiler::new(&design, &bodies)
        .build_circuit("Top")
        .expect("compile Top");
    let device: &dyn Element = circuit
        .devices
        .first()
        .expect("one device")
        .as_ref();
    device.capabilities().contains(ElementCapabilities::BYPASS_OK)
}

#[test]
fn a_plain_resistor_opts_in() {
    assert!(
        declares_bypass_ok("param r: Real = 1e3;", "I(p, n) <+ V(p, n) / r;"),
        "a linear resistor's stamps are a pure function of its terminal voltages"
    );
}

#[test]
fn a_capacitor_opts_in_because_charge_is_not_a_dc_stamp() {
    assert!(
        declares_bypass_ok("param c: Real = 1e-6;", "I(p, n) <+ ddt(c * V(p, n));"),
        "a `ddt` contribution is charge, which never enters the DC stamp"
    );
}

#[test]
fn a_history_dependent_operator_withholds_the_flag() {
    assert!(
        !declares_bypass_ok("", "I(p, n) <+ transition(V(p, n), 0.0, 1e-6, 1e-6);"),
        "`transition` carries a runtime state slot — its output depends on history, not just V"
    );
    assert!(
        !declares_bypass_ok("", "I(p, n) <+ idt(V(p, n));"),
        "`idt` accumulates across steps"
    );
}

#[test]
fn a_runtime_event_withholds_the_flag() {
    assert!(
        !declares_bypass_ok(
            "",
            "@ cross(V(p, n)) { I(p, n) <+ 1.0; } I(p, n) <+ V(p, n) / 1e3;"
        ),
        "an event action mutates state, so a skipped evaluation would skip it"
    );
}

#[test]
fn a_limiter_withholds_the_flag() {
    assert!(
        !declares_bypass_ok(
            "param vto: Real = 1.0;",
            "I(p, n) <+ $limit(\"fetlim\", V(p, n), 0.0, vto, 0.0);"
        ),
        "a `$limit` limiter advances its `vold` slot every evaluation"
    );
}

#[test]
fn a_diagnostic_withholds_the_flag() {
    assert!(
        !declares_bypass_ok("", "$strobe(\"v = %g\", V(p, n)); I(p, n) <+ V(p, n) / 1e3;"),
        "a diagnostic's side effect must not be silently skipped"
    );
}
