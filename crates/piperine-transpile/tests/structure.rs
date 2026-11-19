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
use piperine_ir::{BinaryOp, BodyId, BranchKind, Ir, IrBuilder, QuantityRole};
use piperine_plan::Modules;
use piperine_transpile::{Settings, Transpiler};

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/structure.rs"]
mod structure;

use structure::{Switching, Written};

/// One body of a design compiled with `rustc` and instantiated at its defaults. The cache
/// directory lives as long as this does.
struct Device {
    handle: Handle,
    _cache: tempfile::TempDir,
}

impl Device {
    /// The body `Sw` of `ir`, compiled.
    fn of(ir: Ir) -> Device {
        let cache = tempfile::tempdir().expect("a temp dir");
        let settings = Settings { rustc: None, cache: cache.path().to_path_buf() };
        let ir = Arc::new(ir);
        let body: BodyId = ir.bodies().find(|b| b.name() == "Sw").expect("the body Sw").id();
        let compiled = Transpiler::new(ir).unwrap_or_else(|e| panic!("{e}")).compile(&settings).unwrap_or_else(|e| panic!("{e}"));
        let module = Modules::module(&compiled, body).expect("Sw has a module");
        let handle = Handle::create(module, &[]).unwrap_or_else(|e| panic!("create: {e}"));
        Device { handle, _cache: cache }
    }

    /// Runs function `name` on `reads`, one byte string per read cell, and returns each write as
    /// an `f64` (the first eight bytes of its cell).
    fn run(&self, name: &str, reads: &[&[u8]]) -> Result<Vec<f64>, CallError> {
        let shape = self.handle.shape();
        let k = shape.functions.iter().position(|f| f.name.as_str() == name).unwrap_or_else(|| panic!("no function `{name}`"));
        let f = shape.functions.get(k).expect("the function");
        let lens = cell_lens(shape, &f.writes, Access::FunctionWrite).expect("write lens");
        let mut cells: Vec<Vec<u8>> = lens.iter().map(|len| vec![0u8; *len]).collect();
        let mut writes: Vec<&mut [u8]> = cells.iter_mut().map(Vec::as_mut_slice).collect();
        let seed = vec![1u8; f.partials.len()];
        let mut jac = vec![0.0; f.partials.len()];
        self.handle.eval(k as u32, reads, &mut writes, &seed, &mut jac)?;
        Ok(cells.iter().map(|c| f64::from_le_bytes(c[..8].try_into().expect("an f64 cell"))).collect())
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
