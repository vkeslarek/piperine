//! T20 (LOV-16) — reactive state and operators: the `ddt` backward-Euler
//! companion model built from `backend::analog::reactive::Companion`.
//!
//! Per `reactive.rs`'s module doc, the companion model is expressed as
//! ordinary `Pure`/`CellRead`/`CellWrite`/`ContributeThrough` ops — no new
//! `Call`/intrinsic — so BOTH the reference interpreter (T13, which already
//! supports every op this pattern uses) and this backend evaluate it, and
//! the differential comparison is a real cross-check, not backend-vs-itself.
//! `q_prev`'s cell (`history`) is written by the SAME block that reads it —
//! exactly the "operator state slots... advance on accept" this task's
//! Done-when names — and is driven across MULTIPLE evaluation steps here
//! (unlike T18/T19's single-shot fixtures), proving the state genuinely
//! persists and advances.

use piperine_codegen::backend::analog::reactive::Companion;
use piperine_codegen::backend::analog::AnalogBackend;
use piperine_codegen::backend::CellLayout;
use piperine_ir::core::{Block, Cell, Const, Domain, Module, NatureInfo, NatureKind, Net, NetId, Op, PureOp, Trigger, Ty, ValueId};
use piperine_ir::interp::{CellValue, Interpreter};
use piperine_ir::verify::Verifier;

const CAP: f64 = 1.0e-6;
const H: f64 = 1.0e-3;

struct CapFixture {
    module: Module,
    n_out: NetId,
    n_gnd: NetId,
    h_val: ValueId,
    c_val: ValueId,
    block: piperine_ir::core::BlockId,
}

fn build_cap_fixture() -> CapFixture {
    let mut m = Module::new();
    let voltage = m.push_nature(NatureInfo { access: "voltage".into(), kind: NatureKind::Across });
    let current = m.push_nature(NatureInfo { access: "current".into(), kind: NatureKind::Through });
    let n_out = m.push_net(Net::node(voltage, current));
    let n_gnd = m.push_net(Net::node(voltage, current));

    let h_val = m.push_value(Ty::Real);
    let c_val = m.push_value(Ty::Real);
    let v = m.push_value(Ty::Real);
    let q_now = m.push_value(Ty::Real);

    // `history` (q_prev) is written by `block`, declared up front so the
    // block id is known — the standard "allocate block, then cell" order
    // `Module::block_can_be_allocated_before_its_ops_are_built` documents.
    let block = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    let init = m.push_const(Const::Real(0.0));
    let history = m.push_cell(Cell::new(Ty::Real, init, block));

    m.block_at_mut(block).push(Op::ReadAcross { a: n_out, b: n_gnd, out: v });
    m.block_at_mut(block).push(Op::Pure { op: PureOp::Mul, args: vec![c_val, v], out: q_now });
    let i = Companion::backward_euler(&mut m, block, q_now, history, h_val);
    m.block_at_mut(block).push(Op::ContributeThrough { a: n_out, b: n_gnd, value: i });

    CapFixture { module: m, n_out, n_gnd, h_val, c_val, block }
}

/// Hand-computed backward-Euler companion current at each step of the
/// externally-supplied voltage sequence, starting from `q_prev = 0`.
fn hand_computed_currents(v_sequence: &[f64]) -> Vec<f64> {
    let mut q_prev = 0.0f64;
    v_sequence
        .iter()
        .map(|&v| {
            let q_now = CAP * v;
            let i = (q_now - q_prev) / H;
            q_prev = q_now;
            i
        })
        .collect()
}

const V_SEQUENCE: [f64; 4] = [1.0, 1.0, 2.0, 3.5];

#[test]
fn ddt_companion_matches_hand_computed_backward_euler_across_steps() {
    let fx = build_cap_fixture();
    Verifier::new(&fx.module).run().expect("cap fixture must verify");
    let expected = hand_computed_currents(&V_SEQUENCE);

    // Reference interpreter: multiple `step()`s, state (the `history`
    // cell) persisting across them exactly as `Interpreter::state` design
    // intends.
    let mut interp = Interpreter::new(&fx.module);
    interp.seed_value(fx.h_val, CellValue::Real(H));
    interp.seed_value(fx.c_val, CellValue::Real(CAP));
    let mut interp_currents = Vec::new();
    for &v in &V_SEQUENCE {
        interp.set_node_values(fx.n_out, v, 0.0);
        interp.set_node_values(fx.n_gnd, 0.0, 0.0);
        interp.step().unwrap();
        interp_currents.push(interp.contribution(fx.n_out, fx.n_gnd).expect("contributed"));
    }

    for (k, (&exp, &got)) in expected.iter().zip(&interp_currents).enumerate() {
        assert!((exp - got).abs() < 1e-15, "interpreter step {k}: expected {exp}, got {got}");
    }

    // The JIT backend, driven across the SAME sequence with its own
    // persistent `cell_real` bank standing in for the interpreter's cell
    // map — the companion current at every step agrees with both the
    // hand computation AND the reference interpreter.
    let mut backend = AnalogBackend::new().expect("engine");
    let layout = CellLayout::build(&fx.module);
    let compiled = backend.compile(&fx.module, fx.block, &layout).expect("compile reactive block");

    let net_count = fx.module.net_count();
    let net_through = vec![0.0f64; net_count];
    let net_signal = vec![0u64; net_count];
    let value_count = fx.module.value_count();
    let mut seed_real = vec![0.0f64; value_count];
    seed_real[fx.h_val.0 as usize] = H;
    seed_real[fx.c_val.0 as usize] = CAP;
    let seed_bits = vec![0u64; value_count];

    let mut cell_real = vec![0.0f64; layout.real_len()];
    let mut cell_bits = vec![0u64; layout.bits_len()];

    for (k, &v) in V_SEQUENCE.iter().enumerate() {
        let mut net_across = vec![0.0f64; net_count];
        net_across[fx.n_out.0 as usize] = v;
        let out = compiled.run(&net_across, &net_through, &net_signal, &seed_real, &seed_bits, &mut cell_real, &mut cell_bits);
        let got = out.contribution(fx.n_out, fx.n_gnd).expect("contributed");
        assert!((expected[k] - got).abs() < 1e-15, "jit step {k}: expected {}, got {got}", expected[k]);
        assert!((interp_currents[k] - got).abs() < 1e-15, "jit vs interpreter step {k}: {got} vs {}", interp_currents[k]);
    }
}
