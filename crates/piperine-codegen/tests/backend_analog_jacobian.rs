//! T19 (LOV-15/LOV-16) — the analog backend's Jacobian path: an entry
//! produced by `piperine_ir::diff::Differentiator` (T12's IR diff pass,
//! ported from the OLD `resolve/diff.rs` — never re-derived here) and
//! compiled through `backend::analog::jacobian::compile_jacobian_entry`.
//!
//! The fixture is a diode-shaped nonlinearity — `i = Is * (exp(V/Vt) - 1)`
//! — the smallest fixture that is BOTH nonlinear and exercises a `Call`
//! (the `exp` intrinsic), so the Jacobian path is proven on more than pure
//! arithmetic. Two independent checks, per the task's Done-when:
//!
//! 1. **Differential**: the residual (T18's path) agrees with the
//!    reference interpreter (which also evaluates `exp` — `interp.rs`'s
//!    `eval_call` has a case for it).
//! 2. **Finite-difference cross-check**: the compiled Jacobian entry
//!    agrees with a central-difference estimate computed by re-running the
//!    SAME compiled residual function at perturbed operating points.

use piperine_codegen::backend::analog::jacobian::compile_jacobian_entry;
use piperine_codegen::backend::analog::AnalogBackend;
use piperine_codegen::backend::CellLayout;
use piperine_ir::catalog::Catalog;
use piperine_ir::core::{Block, Domain, IntrinsicId, Module, NatureInfo, NatureKind, Net, NetId, Op, PureOp, Trigger, Ty, ValueId};
use piperine_ir::interp::{CellValue, Interpreter};
use piperine_ir::verify::Verifier;

const IS: f64 = 1.0e-14;
const VT: f64 = 0.025_85;

struct Diode {
    module: Module,
    block: piperine_ir::core::BlockId,
    n_a: NetId,
    n_b: NetId,
    is_val: ValueId,
    vt_val: ValueId,
    one_val: ValueId,
    i_out: ValueId,
}

fn exp_intrinsic_id() -> IntrinsicId {
    let idx = Catalog::entries().position(|e| e.name() == "exp").expect("catalog has `exp`");
    IntrinsicId(idx as u32)
}

fn build_diode() -> Diode {
    let mut m = Module::new();
    let voltage = m.push_nature(NatureInfo { access: "voltage".into(), kind: NatureKind::Across });
    let current = m.push_nature(NatureInfo { access: "current".into(), kind: NatureKind::Through });
    let n_a = m.push_net(Net::node(voltage, current));
    let n_b = m.push_net(Net::node(voltage, current));

    let is_val = m.push_value(Ty::Real);
    let vt_val = m.push_value(Ty::Real);
    let one_val = m.push_value(Ty::Real);
    let v = m.push_value(Ty::Real);
    let ratio = m.push_value(Ty::Real);
    let exp_v = m.push_value(Ty::Real);
    let expm1 = m.push_value(Ty::Real);
    let i_out = m.push_value(Ty::Real);

    let block = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: v });
    m.block_at_mut(block).push(Op::Pure { op: PureOp::Div, args: vec![v, vt_val], out: ratio });
    m.block_at_mut(block).push(Op::Call { intrinsic: exp_intrinsic_id(), args: vec![ratio], out: exp_v });
    m.block_at_mut(block).push(Op::Pure { op: PureOp::Sub, args: vec![exp_v, one_val], out: expm1 });
    m.block_at_mut(block).push(Op::Pure { op: PureOp::Mul, args: vec![is_val, expm1], out: i_out });
    m.block_at_mut(block).push(Op::ContributeThrough { a: n_a, b: n_b, value: i_out });

    Diode { module: m, block, n_a, n_b, is_val, vt_val, one_val, i_out }
}

fn seed_arrays(d: &Diode) -> (Vec<f64>, Vec<u64>) {
    let mut seed_real = vec![0.0f64; d.module.value_count()];
    seed_real[d.is_val.0 as usize] = IS;
    seed_real[d.vt_val.0 as usize] = VT;
    seed_real[d.one_val.0 as usize] = 1.0;
    (seed_real, vec![0u64; d.module.value_count()])
}

fn interp_residual(d: &Diode, v_op: f64) -> f64 {
    let mut interp = Interpreter::new(&d.module);
    interp.seed_value(d.is_val, CellValue::Real(IS));
    interp.seed_value(d.vt_val, CellValue::Real(VT));
    interp.seed_value(d.one_val, CellValue::Real(1.0));
    interp.set_node_values(d.n_a, v_op, 0.0);
    interp.set_node_values(d.n_b, 0.0, 0.0);
    interp.step().unwrap();
    interp.contribution(d.n_a, d.n_b).expect("diode contributed")
}

/// AC6 extended to a nonlinear/`Call`-bearing fixture: the JIT residual
/// agrees with the reference interpreter.
#[test]
fn diode_residual_agrees_with_interpreter() {
    let d = build_diode();
    Verifier::new(&d.module).run().expect("diode fixture must verify");

    let mut backend = AnalogBackend::new().expect("engine");
    let layout = CellLayout::build(&d.module);
    let compiled = backend.compile(&d.module, d.block, &layout).expect("compile residual");

    let (seed_real, seed_bits) = seed_arrays(&d);
    let net_count = d.module.net_count();
    let net_through = vec![0.0f64; net_count];
    let net_signal = vec![0u64; net_count];
    let mut cell_real: Vec<f64> = Vec::new();
    let mut cell_bits: Vec<u64> = Vec::new();

    for v_op in [0.3, 0.5, 0.65] {
        let mut net_across = vec![0.0f64; net_count];
        net_across[d.n_a.0 as usize] = v_op;
        let out = compiled.run(&net_across, &net_through, &net_signal, &seed_real, &seed_bits, &mut cell_real, &mut cell_bits);
        let jit_i = out.contribution(d.n_a, d.n_b).expect("contributed");
        let ref_i = interp_residual(&d, v_op);
        assert!((jit_i - ref_i).abs() < 1e-15, "v={v_op}: jit={jit_i}, interp={ref_i}");
    }
}

/// The Done-when finite-difference cross-check: `di/dV` from
/// `compile_jacobian_entry` (the IR diff pass) agrees with a central
/// difference computed by re-running the SAME compiled residual at
/// perturbed operating points.
#[test]
fn diode_jacobian_matches_finite_difference_of_the_compiled_residual() {
    let mut d = build_diode();
    Verifier::new(&d.module).run().expect("diode fixture must verify");

    let mut backend = AnalogBackend::new().expect("engine");
    let layout = CellLayout::build(&d.module);

    // Compile the residual FIRST (captures the block's ops before the
    // diff pass appends derivative ops after them).
    let residual = backend.compile(&d.module, d.block, &layout).expect("compile residual");

    let (seed_real, seed_bits) = seed_arrays(&d);
    let net_count = d.module.net_count();
    let net_through = vec![0.0f64; net_count];
    let net_signal = vec![0u64; net_count];
    let mut cell_real: Vec<f64> = Vec::new();
    let mut cell_bits: Vec<u64> = Vec::new();

    let residual_at = |v_op: f64| -> f64 {
        let mut net_across = vec![0.0f64; net_count];
        net_across[d.n_a.0 as usize] = v_op;
        let mut cell_real = Vec::new();
        let mut cell_bits = Vec::new();
        let out = residual.run(&net_across, &net_through, &net_signal, &seed_real, &seed_bits, &mut cell_real, &mut cell_bits);
        out.contribution(d.n_a, d.n_b).expect("contributed")
    };

    let i_out = d.i_out;
    let jac = compile_jacobian_entry(&mut backend, &mut d.module, d.block, i_out, (d.n_a, d.n_b), &layout)
        .expect("compile jacobian entry");

    for v_op in [0.3, 0.5, 0.65] {
        let eps = 1e-6;
        let fd = (residual_at(v_op + eps) - residual_at(v_op - eps)) / (2.0 * eps);

        let mut net_across = vec![0.0f64; net_count];
        net_across[d.n_a.0 as usize] = v_op;
        let jit_jacobian = jac.run(&net_across, &net_through, &net_signal, &seed_real, &seed_bits, &mut cell_real, &mut cell_bits);

        // Relative tolerance: the diode's exponential slope spans orders
        // of magnitude across v_op, so a fixed absolute tolerance would
        // either be too loose at large v or impossible at small v.
        let rel = (jit_jacobian - fd).abs() / fd.abs().max(1e-30);
        assert!(rel < 1e-3, "v={v_op}: jacobian={jit_jacobian}, fd={fd}, rel_err={rel}");
    }
}
