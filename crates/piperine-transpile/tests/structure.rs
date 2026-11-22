//! The generated code of each `analog-structure` form, on hand-built IR compiled once with `rustc`
//! and evaluated through the device ABI: contributions under a condition and switch branches
//! (AS-01, AS-03), derivative sites (AS-07), guarded `limit` and `bound_step` sites, param events
//! and reached carriers (AS-06, AS-14, AS-11). Every expected value is derived by hand from the
//! rule cited at its assertion (`docs/spec/` part IV §11 and §15, abi.md), never read back from the
//! printed source.
#![cfg(unix)]

use std::sync::Arc;

use piperine_abi::caller::{CallError, Handle};
use piperine_abi::device::{Access, cell_lens};
use piperine_ir::{BinaryOp, BodyId, BranchKind, Ir, IrBuilder, Operator, QuantityRole, Site, Stmt, ValueType};
use piperine_plan::Modules;
use piperine_transpile::{Settings, Transpiler};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/structure.rs"]
mod structure;

use structure::{ParamEvents, Switching, Written};

/// One body of a design compiled with `rustc` and instantiated at its defaults. The cache
/// directory lives as long as this does.
struct Device {
    handle: Handle,
    _cache: tempfile::TempDir,
}

impl Device {
    /// The body `Sw` of `ir`, compiled.
    fn of(ir: Ir) -> Device {
        Device::of_body(ir, "Sw")
    }

    /// The body `name` of `ir`, compiled.
    fn of_body(ir: Ir, name: &str) -> Device {
        let cache = tempfile::tempdir().expect("a temp dir");
        let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
        let ir = Arc::new(ir);
        let body: BodyId = ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body {name}")).id();
        let compiled = Transpiler::new(ir).unwrap_or_else(|e| panic!("{e}")).compile(&settings).unwrap_or_else(|e| panic!("{e}"));
        let module = Modules::module(&compiled, body).unwrap_or_else(|| panic!("{name} has a module"));
        let handle = Handle::create(module, &[]).unwrap_or_else(|e| panic!("create: {e}"));
        Device { handle, _cache: cache }
    }

    /// Runs function `name` on `reads`, one byte string per read cell, and returns each write as
    /// an `f64` (the first eight bytes of its cell).
    fn run(&self, name: &str, reads: &[&[u8]]) -> Result<Vec<f64>, CallError> {
        self.run_with_partials(name, reads).map(|(values, _)| values)
    }

    /// `run`, and the partials the function declares, in the shape's order.
    fn run_with_partials(&self, name: &str, reads: &[&[u8]]) -> Result<(Vec<f64>, Vec<f64>), CallError> {
        let (cells, jac) = self.run_cells(name, reads)?;
        Ok((cells.iter().map(|c| f64::from_le_bytes(c[..8].try_into().expect("an f64 cell"))).collect(), jac))
    }

    /// `run`, with each write as the bytes of its cell, and the partials.
    fn run_cells(&self, name: &str, reads: &[&[u8]]) -> Result<(Vec<Vec<u8>>, Vec<f64>), CallError> {
        let shape = self.handle.shape();
        let k = shape.functions.iter().position(|f| f.name.as_str() == name).unwrap_or_else(|| panic!("no function `{name}`"));
        let f = shape.functions.get(k).expect("the function");
        let lens = cell_lens(shape, &f.writes, Access::FunctionWrite).expect("write lens");
        let mut cells: Vec<Vec<u8>> = lens.iter().map(|len| vec![0u8; *len]).collect();
        let mut writes: Vec<&mut [u8]> = cells.iter_mut().map(Vec::as_mut_slice).collect();
        let seed = vec![1u8; f.partials.len()];
        let mut jac = vec![0.0; f.partials.len()];
        self.handle.eval(k as u32, reads, &mut writes, &seed, &mut jac)?;
        Ok((cells, jac))
    }
}

/// Bytes of the cells the tests feed.
struct Bytes;

impl Bytes {
    /// A known `f64` held by a continuous pin or an `F64` layout.
    fn real(v: f64) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }

    /// A `Bit` carrier's pin: the value byte, then the known byte (abi.md P3).
    fn bit(high: bool) -> Vec<u8> {
        vec![u8::from(high), 1]
    }
}

#[test]
fn a_conditional_flow_contribution_adds_only_when_its_arm_runs() {
    // `i(a, gnd) +> 1.0; if en.high() { i(a, gnd) +> 2.0; }` on a flow branch (§11 rule 4): the
    // branch's flow is the sum of the contributions that ran, 3.0 with `en` high and 1.0 with it
    // low. `terms` reads the carrier alone (nothing in the sum reads a node).
    let ir = Switching::ir(BranchKind::Flow, |b| {
        let always = Switching::contribute(b, QuantityRole::Flow, 1.0);
        let (cond, taken) = (Switching::carrier_high(b), Switching::contribute(b, QuantityRole::Flow, 2.0));
        Written { analog: vec![always, Switching::branch_on(b, cond, vec![taken], vec![])], ..Written::default() }
    });
    let device = Device::of(ir);
    assert_eq!(device.run("terms", &[&Bytes::bit(true)]).expect("terms"), [3.0]);
    assert_eq!(device.run("terms", &[&Bytes::bit(false)]).expect("terms"), [1.0]);
}

#[test]
fn a_switch_equation_follows_its_enabled_kind() {
    // `if en.high() { v(a, gnd) +> 1.0; } else { i(a, gnd) +> 2.0; }` on a switch branch (§11 rule
    // 2). `terms` reads `a`, the branch's flow pin `f` and `en`, and writes the term `b0.flow`
    // (the flow pin's value `f`) and `b0.eq`. With `a = 0.5`, `f = 0.25`:
    //   potential enabled: the row is `a - 1.0 = -0.5` (the branch is a potential source);
    //   flow enabled: the row is `f - 2.0 = -1.75` (the flow is the sum of the flow contributions).
    let ir = Switching::ir(BranchKind::Switch, Switching::on_the_carrier);
    let device = Device::of(ir);
    let (a, f) = (Bytes::real(0.5), Bytes::real(0.25));
    assert_eq!(device.run("terms", &[&a, &f, &Bytes::bit(true)]).expect("terms"), [0.25, -0.5]);
    assert_eq!(device.run("terms", &[&a, &f, &Bytes::bit(false)]).expect("terms"), [0.25, -1.75]);
}

#[test]
fn a_switch_with_neither_kind_has_zero_flow() {
    // `if en.high() { v(a, gnd) +> 1.0; }` on a switch branch with `en` low: no contribution runs,
    // so the branch has zero flow (§11 rule 4): the row is `f = 0`, whose value is `f` itself.
    let ir = Switching::ir(BranchKind::Switch, |b| {
        let (cond, potential) = (Switching::carrier_high(b), Switching::contribute(b, QuantityRole::Potential, 1.0));
        Written { analog: vec![Switching::branch_on(b, cond, vec![potential], vec![])], ..Written::default() }
    });
    let device = Device::of(ir);
    let (a, f) = (Bytes::real(0.5), Bytes::real(0.25));
    assert_eq!(device.run("terms", &[&a, &f, &Bytes::bit(false)]).expect("terms"), [0.25, 0.25], "the equation row is f");
    assert_eq!(device.run("terms", &[&a, &f, &Bytes::bit(true)]).expect("terms"), [0.25, -0.5], "enabled again: a - 1.0");
}

#[test]
fn both_kinds_enabled_return_e0418_naming_the_branch() {
    // `if en.high() { v(a, gnd) +> 1.0; i(a, gnd) +> 2.0; }`: with `en` high both kinds are enabled
    // in one solve, §11 rule 2 makes that `E0418` naming the branch; with it low neither is.
    let both = Switching::ir(BranchKind::Switch, |b| {
        let (cond, potential, flow) = (Switching::carrier_high(b), Switching::contribute(b, QuantityRole::Potential, 1.0), Switching::contribute(b, QuantityRole::Flow, 2.0));
        Written { analog: vec![Switching::branch_on(b, cond, vec![potential, flow], vec![])], ..Written::default() }
    });
    let device = Device::of(both);
    let (a, f) = (Bytes::real(0.5), Bytes::real(0.25));
    let fault = device.run("terms", &[&a, &f, &Bytes::bit(true)]).expect_err("both kinds enabled");
    assert_eq!(fault.code, 418, "{fault}");
    assert!(fault.message.contains("E0418") && fault.message.contains("v(a)"), "{fault}");
    assert_eq!(device.run("terms", &[&a, &f, &Bytes::bit(false)]).expect("neither kind enabled"), [0.25, 0.25]);

    // E0418 is a fault of the solve at the iterate. `if v(a) < 0.5 { v(a) +> 1.0 }` and
    // `if v(a) < 0.9 { i(a) +> 2.0 }` enable both kinds only at `v(a) < 0.5`, where the evaluation
    // at 0 that gives the affine term `b0.flow` its slope reads `v(a)` as 0: at `a = 0.75` only the
    // flow is enabled, and the solve must give `f - 2.0 = -1.75` and no fault.
    let at_zero = Switching::ir(BranchKind::Switch, |b| {
        let below = |b: &mut IrBuilder, limit: f64| {
            let (probe, edge) = (Switching::probe(b), Switching::ex(b).real(limit));
            Switching::ex(b).binary(BinaryOp::Lt, probe, edge)
        };
        let (c1, potential) = (below(b, 0.5), Switching::contribute(b, QuantityRole::Potential, 1.0));
        let first = Switching::branch_on(b, c1, vec![potential], vec![]);
        let (c2, flow) = (below(b, 0.9), Switching::contribute(b, QuantityRole::Flow, 2.0));
        let second = Switching::branch_on(b, c2, vec![flow], vec![]);
        Written { analog: vec![first, second], ..Written::default() }
    });
    let device = Device::of(at_zero);
    assert_eq!(device.run("terms", &[&Bytes::real(0.75), &Bytes::real(0.25)]).expect("only the flow is enabled"), [0.25, -1.75]);
}

/// `i(a, gnd) +> 2.0 * rate(D0)` under a condition when asked, on a flow branch, with the site
/// `ddt(v(a, gnd))`.
fn rate_in_a_flow(under_a_condition: bool, abstol: bool) -> Ir {
    Switching::ir(BranchKind::Flow, |b| {
        let tolerance = abstol.then(|| {
            let (tol, two) = (Switching::ex(b).param(0), Switching::ex(b).real(2.0));
            Switching::ex(b).binary(BinaryOp::Mul, tol, two)
        });
        let site = Switching::site(b, tolerance);
        let contribution = Switching::contribute_twice_the_rate(b, QuantityRole::Flow, 0);
        let analog = if under_a_condition {
            let cond = Switching::carrier_high(b);
            vec![Switching::branch_on(b, cond, vec![contribution], vec![])]
        } else {
            vec![contribution]
        };
        Written { analog, derivatives: vec![site], ..Written::default() }
    })
}

#[test]
fn a_derivative_state_is_written_outside_every_condition() {
    // `if en.high() { i(a, gnd) +> 2.0 * rate(D0); }` with the site `ddt(v(a, gnd))` (§14 rule 9: the
    // state is evaluated at every accepted solution whatever control flow surrounds the `ddt`).
    // `terms` reads `a`, the rate pin `u` and `en` and writes `b0.flow`, `d0.rate` (`u`) and
    // `d0.state` (`-a`). With `a = 0.5`, `u = 0.125`: `en` high gives flow `2u = 0.25`, `en` low gives
    // flow 0 (the arm does not run); the rate and state terms are the same in both: `0.125`, `-0.5`.
    let device = Device::of(rate_in_a_flow(true, false));
    let (a, u) = (Bytes::real(0.5), Bytes::real(0.125));
    assert_eq!(device.run("terms", &[&a, &u, &Bytes::bit(true)]).expect("terms"), [0.25, 0.125, -0.5]);
    assert_eq!(device.run("terms", &[&a, &u, &Bytes::bit(false)]).expect("terms"), [0.0, 0.125, -0.5]);
}

#[test]
fn a_derivative_read_is_its_rate_pin() {
    // `i(a, gnd) +> 2.0 * rate(D0)`: the contribution is `2u` with `u` the pin `d0.rate`, so its
    // partial against the pin is 2; the rate term `u` has partial 1 against it; the state term
    // `-a` has partial -1 against `a`. The plan declares them as (b0.flow, u), (d0.rate, u),
    // (d0.state, a), in that order.
    let device = Device::of(rate_in_a_flow(false, false));
    let (values, partials) = device.run_with_partials("terms", &[&Bytes::real(0.5), &Bytes::real(0.125)]).expect("terms");
    assert_eq!(values, [0.25, 0.125, -0.5]);
    assert_eq!(partials, [2.0, 1.0, -1.0]);
}

#[test]
fn a_derivative_abstol_function_writes_its_pin() {
    // `ddt(v(a, gnd), 2.0 * tol)` with `tol = 0.25`: the function `d0.abstol` reads the param alone
    // and writes `0.5` to the site's `abstol` pin.
    let device = Device::of(rate_in_a_flow(false, true));
    assert_eq!(device.run("d0.abstol", &[&Bytes::real(0.25)]).expect("d0.abstol"), [0.5]);
}

#[test]
fn a_disabled_limit_writes_its_last_value() {
    // `if v(a, gnd) > 0.5 { i(a, gnd) +> limit(v(a, gnd), |new, old| old + (new - old) * 0.5); }`
    // with the site's `enabled` the same condition (§14.7.4). `limit:lim` reads `a`, `LAST`, `AGE`
    // and `STATIC` and writes `PROPOSED` and `LIMITED`. With `LAST = 0.75`, `AGE = 1`, not static:
    //   `a = 1.0`, enabled: `PROPOSED = 1.0`, `LIMITED = 0.75 + (1.0 - 0.75) * 0.5 = 0.875`, and the
    //   partial of `PROPOSED` against `a` is 1;
    //   `a = 0.25`, not enabled: the limiter does not run, both writes are `LAST = 0.75`, the
    //   partial is 0 (the held value stays, abi.md A3).
    let ir = Switching::ir(BranchKind::Flow, |b| {
        let read = Switching::halving_limit(b);
        let (cond, enabled) = (Switching::compare(b, BinaryOp::Gt, 0.5), Switching::compare(b, BinaryOp::Gt, 0.5));
        let taken = Switching::contribute_expr(b, QuantityRole::Flow, Operator::Direct, read);
        let site = Site { name: "lim".into(), origin: Switching::at(), enabled: Some(enabled) };
        Written { analog: vec![Switching::branch_on(b, cond, vec![taken], vec![])], limits: vec![site], locals: Switching::limit_locals(), ..Written::default() }
    });
    let device = Device::of(ir);
    let (last, age, held) = (Bytes::real(0.75), [1u8], [0u8]);
    let enabled = device.run_with_partials("limit:lim", &[&Bytes::real(1.0), &last, &age, &held]).expect("limit");
    assert_eq!(enabled, (vec![1.0, 0.875], vec![1.0]));
    let disabled = device.run_with_partials("limit:lim", &[&Bytes::real(0.25), &last, &age, &held]).expect("limit");
    assert_eq!(disabled, (vec![0.75, 0.75], vec![0.0]));
}

#[test]
fn a_disabled_cap_writes_infinity() {
    // `bound_step(0.5) when v(a, gnd) > 0.5;` (§14.10.2): `cap:0` reads `a`. With `a = 1.0` the
    // step is asked for and the watch is 0.5; with `a = 0.25` the call is not executed at this
    // solution, so it caps nothing: `+∞` (abi.md, the `CAP` watch).
    let ir = Switching::ir(BranchKind::Flow, |b| {
        let (step, when) = (Switching::ex(b).real(0.5), Switching::compare(b, BinaryOp::Gt, 0.5));
        let cap = b.stmt(Stmt::BoundStep { site: 0, value: step, when: Some(when) });
        Written { analog: vec![cap, Switching::contribute(b, QuantityRole::Flow, 1.0)], ..Written::default() }
    });
    let device = Device::of(ir);
    assert_eq!(device.run("cap:0", &[&Bytes::real(1.0)]).expect("cap"), [0.5]);
    assert_eq!(device.run("cap:0", &[&Bytes::real(0.25)]).expect("cap"), [f64::INFINITY]);
}

#[test]
fn an_executed_cap_that_is_not_finite_and_positive_is_e0509() {
    // §14.10.2 rule 5 and its Resolution: an executed `bound_step(dt)` with `dt` not finite or
    // `dt <= 0` is `E0509`. `bound_step(bad) when v(a, gnd) > 0.5;`, one body per `bad`: with
    // `a = 1.0` the call is executed and faults for 0, a negative step and `+∞`; with `a = 0.25`
    // it is not executed, caps nothing (`+∞`) and does not fault, whatever its value.
    for bad in [0.0, -1.0e-9, f64::INFINITY] {
        let ir = Switching::ir(BranchKind::Flow, |b| {
            let (step, when) = (Switching::ex(b).real(bad), Switching::compare(b, BinaryOp::Gt, 0.5));
            let cap = b.stmt(Stmt::BoundStep { site: 0, value: step, when: Some(when) });
            Written { analog: vec![cap, Switching::contribute(b, QuantityRole::Flow, 1.0)], ..Written::default() }
        });
        let device = Device::of(ir);
        let fault = device.run("cap:0", &[&Bytes::real(1.0)]).expect_err("an executed cap that is not valid");
        assert_eq!(fault.code, 509, "step {bad}: {fault}");
        assert!(fault.message.contains("E0509") && fault.message.contains("bound_step"), "step {bad}: {fault}");
        assert_eq!(device.run("cap:0", &[&Bytes::real(0.25)]).expect("not executed"), [f64::INFINITY], "step {bad}");
    }
    let unguarded = Switching::ir(BranchKind::Flow, |b| {
        let step = Switching::ex(b).real(f64::INFINITY);
        let cap = b.stmt(Stmt::BoundStep { site: 0, value: step, when: None });
        Written { analog: vec![cap, Switching::contribute(b, QuantityRole::Flow, 1.0)], ..Written::default() }
    });
    let fault = Device::of(unguarded).run("cap:0", &[]).expect_err("an unguarded bound_step(inf) is executed");
    assert_eq!(fault.code, 509, "{fault}");
}

#[test]
fn a_param_change_fires_its_event() {
    // `on tol.changed() { .. }`: the event function `event:0` reads the param `tol` and its
    // `CHANGED` byte and writes the event pin's `CHANGED` byte: nonzero exactly when the param's
    // pin changed (abi.md §4.1, §5.3.1).
    let ir = ParamEvents::ir(ValueType::Bool, |b| b.expr(piperine_ir::Expr::Fired(0), ValueType::Bool, ParamEvents::at()));
    let device = Device::of_body(ir, "Lag");
    let tol = Bytes::real(1e-9);
    assert_eq!(device.run_cells("event:0", &[&tol, &[1u8]]).expect("event").0, [vec![1u8]]);
    assert_eq!(device.run_cells("event:0", &[&tol, &[0u8]]).expect("event").0, [vec![0u8]]);
}

#[test]
fn a_param_previous_decodes_its_previous_bytes() {
    // `seen = tol.prev();` as a drive of an `out seen: f64` carrier: `drive:seen` reads the
    // `PREVIOUS` part of the param pin, in the param's own layout (an `F64`), and writes the value
    // `0.375` into the carrier's pin (its eight bytes, abi.md P3).
    let device = Device::of_body(ParamEvents::previous_drive(), "Lag");
    let (cells, _) = device.run_cells("drive:seen", &[&Bytes::real(0.375)]).expect("drive");
    assert_eq!(f64::from_le_bytes(cells[0][..8].try_into().expect("the value")), 0.375);
}
