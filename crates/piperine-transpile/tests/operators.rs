//! The generated code of each `analog-operators` form, on hand-built IR compiled once with
//! `rustc` and evaluated through the device ABI: delay sites (AO-02), noise sources (AO-03),
//! session parameters (AO-07) and traces (AO-09). Every expected value is derived by hand from
//! the rule cited at its assertion (`docs/spec/` part IV §14, abi.md), never read back from the
//! printed source.
#![cfg(unix)]

use piperine_abi::raw::status;
use piperine_abi::caller::{Commit, Handle};
use piperine_ir::{BinaryOp, Origin, Carrier, CarrierRole, Constant, Env, Event, EventSource, Expr, Ir, NoiseSource, Reaction, Stmt, ValueType};
use piperine_plan::Modules;
use piperine_transpile::{Settings, Transpiler};
use std::sync::Arc;

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

/// `i(a, gnd) +> noise(<density>, "thermal")`, the density being `td` or `freq()`.
fn noisy(density: Expr) -> Ir {
    Operators::ir(|b| {
        let density = b.expr(density, ValueType::F64, Operators::at());
        let source = NoiseSource { name: "hiss".into(), group: Some("thermal".into()), density, origin: Operators::at() };
        Written { analog: vec![Operators::contribute_the_noise(b, 0)], noise_sources: vec![source], ..Written::default() }
    })
}

#[test]
fn the_noise_function_writes_its_density_and_faults_454() {
    // §14.8: the function `noise:0` writes the `PSD` of the input; a density that is negative or
    // not finite is the fault 454 naming the source (D9 of the density's own contract).
    let device = Device::of_body(noisy(Expr::Param(0)), "Op");
    // the function reads the param and the sweep's frequency, which a fault names
    let at = Bytes::real(2e3);
    assert_eq!(device.run("noise:0", &[&Bytes::real(4e-21), &at]).expect("a density"), [4e-21]);
    assert_eq!(device.run("noise:0", &[&Bytes::real(0.0), &at]).expect("zero is a density"), [0.0]);
    for bad in [-1e-21, f64::NAN, f64::INFINITY] {
        let fault = device.run("noise:0", &[&Bytes::real(bad), &at]).expect_err("a bad density");
        assert_eq!(fault.code, 454, "{bad}: {fault}");
        assert!(fault.message.contains("E0454") && fault.message.contains("`thermal`") && fault.message.contains("at 2000 Hz"), "{bad}: {fault}");
    }
    // a density that reads `freq()` names the frequency it was read at
    let flicker = Device::of_body(noisy(Expr::Env(Env::Freq)), "Op");
    assert_eq!(flicker.run("noise:0", &[&Bytes::real(1e3)]).expect("psd = freq"), [1e3]);
    let fault = flicker.run("noise:0", &[&Bytes::real(-5.0)]).expect_err("a negative frequency");
    assert!(fault.message.contains("at -5 Hz"), "{fault}");
    // `terms` reads the input's value: 0 in a solve, here whatever the call gives (a partial too)
    let (written, jac) = device.run_cells("terms", &[&Bytes::real(0.75)]).expect("terms");
    assert_eq!(f64::from_le_bytes(written[0][..8].try_into().expect("an f64")), 0.75);
    assert_eq!(jac, [1.0]);
}

#[test]
fn a_reaction_reads_noise_as_zero() {
    // §14.8.5: a reaction reads a noise source as 0. `on en.changed() { q <- noise > 0.5 }` is a
    // commit of `false`, whatever the source's density, and the reaction reads no input.
    let ir = Operators::ir(|b| {
        let source = {
            let density = b.expr(Expr::Param(0), ValueType::F64, Operators::at());
            NoiseSource { name: "hiss".into(), group: None, density, origin: Operators::at() }
        };
        let (read, half) = (b.expr(Expr::Noise(0), ValueType::F64, Operators::at()), Operators::ex(b).real(0.5));
        let above = b.expr(Expr::Binary { op: BinaryOp::Gt, lhs: read, rhs: half }, ValueType::Bool, Operators::at());
        let commit = b.stmt(Stmt::Commit { carrier: 1, value: above, after: None });
        let carrier = |name: &str, role| Carrier { name: name.into(), ty: ValueType::Bool, role, init: Some(Constant::Bool(false)), origin: Operators::at() };
        Written {
            analog: vec![Operators::contribute_the_noise(b, 0)],
            noise_sources: vec![source],
            carriers: vec![carrier("en", CarrierRole::In), carrier("q", CarrierRole::Out)],
            events: vec![Event { source: EventSource::Changed(0), guard: None, origin: Operators::at() }],
            reactions: vec![Reaction { on: vec![0], stmts: vec![commit], locals: vec![], origin: Operators::at() }],
            ..Written::default()
        }
    });
    let cache = tempfile::tempdir().expect("a temp dir");
    let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
    let ir = Arc::new(ir);
    let body = ir.bodies().find(|b| b.name() == "Op").expect("Op").id();
    let compiled = Transpiler::new(ir).unwrap_or_else(|e| panic!("{e}")).compile(&settings).unwrap_or_else(|e| panic!("{e}"));
    let mut handle = Handle::create(Modules::module(&compiled, body).expect("a module"), &[]).expect("an instance");
    let reaction = *handle.shape().reactions.get(0).expect("one reaction");
    assert_eq!(reaction.reads.len(), 0, "the reaction reads no cell, so no input");
    let mut made = Vec::new();
    handle.react(0, &[], &mut |c: Commit<'_>| {
        made.push(c.bytes.to_vec());
        status::OK
    }).expect("the reaction runs");
    assert_eq!(made, [vec![0u8]], "q <- 0.0 > 0.5 is false");
}

/// `i(a, gnd) +> simparam("gmin", <default>)` as the body `Op`, the parameter required without one.
fn parameterised(default: Option<f64>) -> Ir {
    Operators::ir(|b| {
        let mut parameter = Operators::gmin();
        parameter.required = default.is_none();
        Written { analog: vec![Operators::contribute_the_session_parameter(b, 0, default)], session_parameters: vec![parameter], ..Written::default() }
    })
}

/// The bytes of an `Option<f64>` pin: the tag byte, then the value (all zero for `None`).
fn option_pin(value: Option<f64>) -> Vec<u8> {
    let mut bytes = vec![0u8; 9];
    if let Some(v) = value {
        bytes[0] = 1;
        bytes[1..].copy_from_slice(&v.to_le_bytes());
    }
    bytes
}

#[test]
fn a_session_parameter_reads_its_pin_or_its_default() {
    // §14.4: `simparam(name, default)` is the parameter when the host gave it, else `default`
    // (`0.0` with none: the host refuses before binding, E0456). `terms` reads the pin, an
    // `Option<f64>`, and writes the flow `simparam(..)`.
    let with_default = Device::of_body(parameterised(Some(1e-12)), "Op");
    assert_eq!(with_default.run("terms", &[&option_pin(Some(7.0))]).expect("given"), [7.0]);
    assert_eq!(with_default.run("terms", &[&option_pin(None)]).expect("not given"), [1e-12], "the default of the read");
    let required = Device::of_body(parameterised(None), "Op");
    assert_eq!(required.run("terms", &[&option_pin(Some(-3.5))]).expect("given"), [-3.5]);
    assert_eq!(required.run("terms", &[&option_pin(None)]).expect("not given"), [0.0], "no default reads 0.0");
}

/// `if v(a, gnd) > 0.5 { emit!("newton", x = v(a, gnd)) }`, the only statement of the `analog` block.
fn traced_above_half() -> Ir {
    Operators::ir(|b| {
        let (probe, half) = (Operators::ex(b).probe(piperine_ir::Terminal::Node(0), piperine_ir::Terminal::Ground), Operators::ex(b).real(0.5));
        let cond = b.expr(Expr::Binary { op: BinaryOp::Gt, lhs: probe, rhs: half }, ValueType::Bool, Operators::at());
        let trace = Operators::trace_the_probe(b, 0);
        let at = Origin { file: Operators::FILE.into(), line: 3, col: 5 };
        let then = b.expr(Expr::Block { stmts: vec![trace], value: None }, ValueType::Unit, at.clone());
        let otherwise = b.expr(Expr::Block { stmts: vec![], value: None }, ValueType::Unit, at.clone());
        let branch = b.expr(Expr::If { cond, then, otherwise }, ValueType::Unit, at);
        let statement = b.stmt(Stmt::Expr(branch));
        Written { analog: vec![statement], streams: vec![Operators::newton()], traces: vec![Operators::trace_site()], ..Written::default() }
    })
}

#[test]
fn a_trace_writes_its_watch_only_where_it_executes() {
    // abi.md A5: the watch's record is a `BOOL` presence byte, then the stream's fields; where the
    // statement does not execute the record is empty (all zero). With `a = 0.75` the arm runs and
    // the record is presence 1, padding, `x = 0.75` at offset 8; with `a = 0.25` it is all zero.
    let device = Device::of_body(traced_above_half(), "Op");
    let (written, _) = device.run_cells("terms", &[&Bytes::real(0.75)]).expect("the arm runs");
    let mut expected = vec![0u8; 16];
    expected[0] = 1;
    expected[8..].copy_from_slice(&0.75f64.to_le_bytes());
    assert_eq!(written, [expected]);
    let (written, _) = device.run_cells("terms", &[&Bytes::real(0.25)]).expect("the arm does not run");
    assert_eq!(written, [vec![0u8; 16]], "no record where the statement does not execute");
}
