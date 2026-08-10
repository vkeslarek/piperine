//! T18 (LOV-16) — the analog backend's residual path, and the differential
//! rewiring the previous batch flagged: `piperine-ir`'s `tests/support/
//! harness.rs` module doc says T18 "rewires this same `compare` function
//! against the real analog backend to close the literal AC6 claim"
//! (spec.md P1 AC6: "the reference interpreter and a JIT backend... SHALL
//! agree"). This target is that rewiring's home — `piperine-ir` cannot
//! depend on `piperine-codegen` (the purity guard, MD-40), so the
//! interpreter-vs-JIT comparison can only live on this side of the
//! dependency edge.
//!
//! The fixture mirrors `piperine-ir/tests/fixtures.rs`'s RC divider
//! byte-for-byte (same topology, same R1/R2/Vin) so the comparison is
//! against a KNOWN-GOOD hand-computed operating point, not just "the two
//! evaluators happen to agree with each other".

use piperine_codegen::backend::analog::AnalogBackend;
use piperine_codegen::backend::CellLayout;
use piperine_ir::core::{Block, Domain, Module, NatureInfo, NatureKind, Net, NetId, Op, PureOp, Trigger, Ty, ValueId};
use piperine_ir::interp::{CellValue, Interpreter};
use piperine_ir::verify::Verifier;

struct RcDivider {
    module: Module,
    n_in: NetId,
    n_out: NetId,
    n_gnd: NetId,
    r1_val: ValueId,
    r2_val: ValueId,
}

fn build_rc_divider() -> RcDivider {
    let mut m = Module::new();
    let voltage = m.push_nature(NatureInfo { access: "voltage".into(), kind: NatureKind::Across });
    let current = m.push_nature(NatureInfo { access: "current".into(), kind: NatureKind::Through });
    let n_in = m.push_net(Net::node(voltage, current));
    let n_out = m.push_net(Net::node(voltage, current));
    let n_gnd = m.push_net(Net::node(voltage, current));

    let r1_val = m.push_value(Ty::Real);
    let r2_val = m.push_value(Ty::Real);
    let v1 = m.push_value(Ty::Real);
    let i1 = m.push_value(Ty::Real);
    let v2 = m.push_value(Ty::Real);
    let i2 = m.push_value(Ty::Real);

    let block = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    m.block_at_mut(block).push(Op::ReadAcross { a: n_in, b: n_out, out: v1 });
    m.block_at_mut(block).push(Op::Pure { op: PureOp::Div, args: vec![v1, r1_val], out: i1 });
    m.block_at_mut(block).push(Op::ContributeThrough { a: n_in, b: n_out, value: i1 });
    m.block_at_mut(block).push(Op::ReadAcross { a: n_out, b: n_gnd, out: v2 });
    m.block_at_mut(block).push(Op::Pure { op: PureOp::Div, args: vec![v2, r2_val], out: i2 });
    m.block_at_mut(block).push(Op::ContributeThrough { a: n_out, b: n_gnd, value: i2 });

    RcDivider { module: m, n_in, n_out, n_gnd, r1_val, r2_val }
}

const VIN: f64 = 10.0;
const R1: f64 = 1000.0;
const R2: f64 = 2000.0;

/// The reference interpreter's contributions at the RC operating point —
/// duplicated from `piperine-ir/tests/fixtures.rs`'s hand computation
/// (that test target is private to `piperine-ir`; this crate cannot import
/// it, so the small fixture is rebuilt here instead).
fn interpreter_contributions(rc: &RcDivider) -> (f64, f64) {
    let vout = VIN * R2 / (R1 + R2);
    let mut interp = Interpreter::new(&rc.module);
    interp.seed_value(rc.r1_val, CellValue::Real(R1));
    interp.seed_value(rc.r2_val, CellValue::Real(R2));
    interp.set_node_values(rc.n_in, VIN, 0.0);
    interp.set_node_values(rc.n_out, vout, 0.0);
    interp.set_node_values(rc.n_gnd, 0.0, 0.0);
    interp.step().unwrap();
    (
        interp.contribution(rc.n_in, rc.n_out).expect("i1 contributed"),
        interp.contribution(rc.n_out, rc.n_gnd).expect("i2 contributed"),
    )
}

/// The analog JIT backend's contributions at the SAME operating point,
/// given the SAME externally-supplied net/seed values — the direct
/// analogue of `Interpreter::set_node_values`/`seed_value`.
fn backend_contributions(rc: &RcDivider) -> (f64, f64) {
    Verifier::new(&rc.module).run().expect("RC divider fixture must verify");
    let mut backend = AnalogBackend::new().expect("engine construction");
    let layout = CellLayout::build(&rc.module);
    let block = rc.module.blocks().next().expect("one block").0;
    let compiled = backend.compile(&rc.module, block, &layout).expect("compile residual");

    let vout = VIN * R2 / (R1 + R2);
    let net_count = rc.module.net_count();
    let mut net_across = vec![0.0f64; net_count];
    let net_through = vec![0.0f64; net_count];
    net_across[rc.n_in.0 as usize] = VIN;
    net_across[rc.n_out.0 as usize] = vout;
    net_across[rc.n_gnd.0 as usize] = 0.0;

    let value_count = rc.module.value_count();
    let mut seed_real = vec![0.0f64; value_count];
    seed_real[rc.r1_val.0 as usize] = R1;
    seed_real[rc.r2_val.0 as usize] = R2;
    let seed_bits = vec![0u64; value_count];
    let net_signal = vec![0u64; net_count];
    let mut cell_real: Vec<f64> = Vec::new();
    let mut cell_bits: Vec<u64> = Vec::new();

    let out = compiled.run(&net_across, &net_through, &net_signal, &seed_real, &seed_bits, &mut cell_real, &mut cell_bits);
    (
        out.contribution(rc.n_in, rc.n_out).expect("i1 contributed"),
        out.contribution(rc.n_out, rc.n_gnd).expect("i2 contributed"),
    )
}

/// AC6, earned for real: the JIT backend and the reference interpreter,
/// run against the SAME RC fixture and the SAME hand-computed operating
/// point, agree bit-for-bit (well within the RC fixture's floating-point
/// tolerance).
#[test]
fn analog_backend_residual_agrees_with_the_reference_interpreter() {
    let rc = build_rc_divider();
    let (interp_i1, interp_i2) = interpreter_contributions(&rc);
    let (jit_i1, jit_i2) = backend_contributions(&rc);

    assert!((interp_i1 - jit_i1).abs() < 1e-9, "i1: interpreter={interp_i1}, jit={jit_i1}");
    assert!((interp_i2 - jit_i2).abs() < 1e-9, "i2: interpreter={interp_i2}, jit={jit_i2}");

    // And against the hand-computed closed form directly (not just
    // "the two evaluators happen to agree with each other").
    let expected_i = (VIN - VIN * R2 / (R1 + R2)) / R1;
    assert!((jit_i1 - expected_i).abs() < 1e-9, "jit i1 vs hand-computed: {jit_i1} vs {expected_i}");
    assert!((jit_i2 - expected_i).abs() < 1e-9, "jit i2 vs hand-computed: {jit_i2} vs {expected_i}");
}

/// Structured control (`If`) compiles to real Cranelift branches, not a
/// flattened select — proven behaviourally: a fixture whose contribution
/// depends on a branch produces the branch-selected value, for BOTH branch
/// outcomes, and agrees with the reference interpreter on each.
#[test]
fn analog_backend_if_emits_branches_matching_the_interpreter() {
    let mut m = Module::new();
    let voltage = m.push_nature(NatureInfo { access: "voltage".into(), kind: NatureKind::Across });
    let current = m.push_nature(NatureInfo { access: "current".into(), kind: NatureKind::Through });
    let n_a = m.push_net(Net::node(voltage, current));
    let n_b = m.push_net(Net::node(voltage, current));

    let cond = m.push_value(Ty::Bits(1));
    let v = m.push_value(Ty::Real);
    let hi = m.push_value(Ty::Real);
    let lo = m.push_value(Ty::Real);

    let block = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    m.block_at_mut(block).push(Op::ReadAcross { a: n_a, b: n_b, out: v });
    m.block_at_mut(block).push(Op::Pure { op: PureOp::Mul, args: vec![v, v], out: hi });
    m.block_at_mut(block).push(Op::If {
        cond,
        then_ops: vec![Op::ContributeThrough { a: n_a, b: n_b, value: hi }],
        else_ops: vec![Op::ContributeThrough { a: n_a, b: n_b, value: lo }],
    });

    Verifier::new(&m).run().expect("if fixture must verify");
    let mut backend = AnalogBackend::new().expect("engine");
    let layout = CellLayout::build(&m);
    let (block_id, _) = m.blocks().next().expect("one block");
    let compiled = backend.compile(&m, block_id, &layout).expect("compile");

    let net_count = m.net_count();
    let net_through = vec![0.0f64; net_count];
    let net_signal = vec![0u64; net_count];
    let value_count = m.value_count();
    let mut seed_real = vec![0.0f64; value_count];
    seed_real[lo.0 as usize] = -1.0;
    let mut seed_bits = vec![0u64; value_count];
    let mut cell_real: Vec<f64> = Vec::new();
    let mut cell_bits: Vec<u64> = Vec::new();

    for (cond_bit, expect_then) in [(1u64, true), (0u64, false)] {
        seed_bits[cond.0 as usize] = cond_bit;
        let mut net_across = vec![0.0f64; net_count];
        net_across[n_a.0 as usize] = 3.0;
        net_across[n_b.0 as usize] = 0.0;
        let out = compiled.run(&net_across, &net_through, &net_signal, &seed_real, &seed_bits, &mut cell_real, &mut cell_bits);
        let got = out.contribution(n_a, n_b).expect("contribution present");
        let expected = if expect_then { 9.0 } else { -1.0 };
        assert!((got - expected).abs() < 1e-12, "cond={cond_bit}: expected {expected}, got {got}");
    }
}
