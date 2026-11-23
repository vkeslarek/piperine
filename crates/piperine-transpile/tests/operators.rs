//! The generated code of each `analog-operators` form, on hand-built IR compiled once with
//! `rustc` and evaluated through the device ABI: delay sites (AO-02), noise sources (AO-03),
//! session parameters (AO-07) and traces (AO-09). Every expected value is derived by hand from
//! the rule cited at its assertion (`docs/spec/` part IV §14, abi.md), never read back from the
//! printed source.
#![cfg(unix)]

use piperine_ir::Ir;

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/operators.rs"]
mod fixtures;
#[allow(dead_code)]
#[path = "support/compiled.rs"]
mod compiled;

use compiled::{Bytes, Device};
use fixtures::{Operators, Written};

/// `i(a, gnd) +> 0.5 * delay(v(a), td, <maximum>)` as the body `Op`.
fn delayed(maximum: Option<f64>) -> Ir {
    Operators::ir(|b| {
        let site = Operators::delay_site(b, maximum);
        Written { analog: vec![Operators::contribute_half_the_delay(b, 0)], delays: vec![site], ..Written::default() }
    })
}

#[test]
fn the_delay_terms_clamp_td_to_max() {
    // §14.6.4: `td` above `max` reads as `max`. `terms` reads `a`, the output pin `u` and the
    // param `td`, and writes the flow `0.5·u`, the term `u`, the history term `-x = -a`, and the
    // `TIME` of that term, `min(td, max)`; with `max = 2e-9`:
    //   td = 1e-9 -> TIME 1e-9, td = 5e-9 -> TIME 2e-9 (held to `max`), td = 0 -> TIME 0.
    let device = Device::of_body(delayed(Some(2e-9)), "Op");
    let (a, u) = (Bytes::real(0.5), Bytes::real(0.25));
    for (td, time) in [(1e-9, 1e-9), (5e-9, 2e-9), (0.0, 0.0)] {
        let written = device.run("terms", &[&a, &u, &Bytes::real(td)]).expect("terms");
        assert_eq!(written, [0.125, 0.25, -0.5, time], "td = {td}");
    }
}

#[test]
fn the_delay_function_faults_453() {
    // D5: a negative `td` is the fault 453 of `terms`; a `max` that is not positive is the fault
    // 453 of `delay0.max`, and a positive one is written to the bound pin (§14.6.4).
    let device = Device::of_body(delayed(Some(2e-9)), "Op");
    let (a, u) = (Bytes::real(0.5), Bytes::real(0.25));
    let fault = device.run("terms", &[&a, &u, &Bytes::real(-1e-9)]).expect_err("a negative td");
    assert_eq!(fault.code, 453, "{fault}");
    assert!(fault.message.contains("E0453") && fault.message.contains("delay site 0"), "{fault}");
    assert_eq!(device.run("delay0.max", &[]).expect("a positive max"), [2e-9]);
    for bound in [0.0, -1e-9] {
        let fault = Device::of_body(delayed(Some(bound)), "Op").run("delay0.max", &[]).expect_err("a max that is not positive");
        assert_eq!(fault.code, 453, "max = {bound}: {fault}");
        assert!(fault.message.contains("max that is not positive"), "{fault}");
    }
    // without a `max` the bound is `td` itself (D4): written as it is, 0 allowed, a negative one a fault
    let unbounded = Device::of_body(delayed(None), "Op");
    assert_eq!(unbounded.run("delay0.max", &[&Bytes::real(3e-9)]).expect("td as the bound"), [3e-9]);
    assert_eq!(unbounded.run("delay0.max", &[&Bytes::real(0.0)]).expect("td = 0 is allowed"), [0.0]);
    assert_eq!(unbounded.run("delay0.max", &[&Bytes::real(-1e-9)]).expect_err("a negative td").code, 453);
}
