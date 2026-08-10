//! T15 — the first two completeness fixtures for P1 (LOV-02, LOV-11): a
//! hand-written RC (resistive) divider exercising the analog path, and a
//! hand-written 4-bit counter exercising the digital path. Both are driven
//! through the reference interpreter against HAND-COMPUTED expected
//! values (not just "the interpreter agrees with itself"), and both
//! round-trip through the printer/parser (`parse(print(m))` is
//! structurally equivalent to `m`).
//!
//! The RC divider is purely resistive (no `ddt`/reactive companion state):
//! T13's interpreter evaluates the op graph given EXTERNALLY-SUPPLIED node
//! values (it does not solve KCL — see `interp.rs`'s module doc), so a
//! "DC point" fixture at this layer is a resistor network whose node
//! voltages we supply already balanced, and whose Ohm's-law contributions
//! we hand-compute and check. Reactive dynamics (the `ddt` companion
//! model) are a Phase 4 backend concern (T20); nothing at T15 depends on
//! them.

use piperine_ir::core::{
    Block, Cell, Chunk, Const, Domain, EdgeKind, Module, Net, NatureInfo, NatureKind, Op,
    PureOp, SigRef, Trigger, Ty,
};
use piperine_ir::interp::{CellValue, Interpreter};
use piperine_ir::verify::Verifier;

// ─── RC (resistive) divider ──────────────────────────────────────────────

/// `n_in --[R1]-- n_out --[R2]-- n_gnd`, R1 = 1000 Ω, R2 = 2000 Ω.
/// `i = ContributeThrough` on each resistor; the module computes
/// `i1 = (Vin - Vout) / R1` and `i2 = Vout / R2` from externally-supplied
/// node values, via `ReadAcross` + `Pure::Div` + `ContributeThrough`.
struct RcDivider {
    module: Module,
    n_in: piperine_ir::core::NetId,
    n_out: piperine_ir::core::NetId,
    n_gnd: piperine_ir::core::NetId,
    i1: piperine_ir::core::ValueId,
    i2: piperine_ir::core::ValueId,
    r1_val: piperine_ir::core::ValueId,
    r2_val: piperine_ir::core::ValueId,
}

fn build_rc_divider() -> RcDivider {
    let mut m = Module::new();
    let voltage = m.push_nature(NatureInfo { access: "voltage".into(), kind: NatureKind::Across });
    let current = m.push_nature(NatureInfo { access: "current".into(), kind: NatureKind::Through });
    let n_in = m.push_net(Net::node(voltage, current));
    let n_out = m.push_net(Net::node(voltage, current));
    let n_gnd = m.push_net(Net::node(voltage, current));

    let r1_val = m.push_value(Ty::Real); // externally seeded: R1 = 1000
    let r2_val = m.push_value(Ty::Real); // externally seeded: R2 = 2000
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

    RcDivider { module: m, n_in, n_out, n_gnd, i1, i2, r1_val, r2_val }
}

/// The hand-computed DC point: Vin = 10 V, R1 = 1000 Ω, R2 = 2000 Ω.
/// A resistive divider settles at `Vout = Vin * R2 / (R1 + R2)`, so the
/// two resistor currents are equal (KCL, by construction of the operating
/// point we hand over): `i1 = i2 = (Vin - Vout) / R1`.
#[test]
fn rc_divider_hand_computed_dc_point_matched() {
    let rc = build_rc_divider();
    Verifier::new(&rc.module).run().expect("RC divider fixture must verify");

    let vin = 10.0_f64;
    let r1 = 1000.0_f64;
    let r2 = 2000.0_f64;
    let vout = vin * r2 / (r1 + r2); // 6.666...
    let expected_i = (vin - vout) / r1; // 0.003333...

    let mut interp = Interpreter::new(&rc.module);
    interp.seed_value(rc.r1_val, CellValue::Real(r1));
    interp.seed_value(rc.r2_val, CellValue::Real(r2));
    interp.set_node_values(rc.n_in, vin, 0.0);
    interp.set_node_values(rc.n_out, vout, 0.0);
    interp.set_node_values(rc.n_gnd, 0.0, 0.0);
    interp.step().unwrap();

    let got_i1 = match interp.value(rc.i1) {
        Some(CellValue::Real(x)) => *x,
        other => panic!("i1 must be a Real value, got {other:?}"),
    };
    let got_i2 = match interp.value(rc.i2) {
        Some(CellValue::Real(x)) => *x,
        other => panic!("i2 must be a Real value, got {other:?}"),
    };
    assert!((got_i1 - expected_i).abs() < 1e-12, "i1: expected {expected_i}, got {got_i1}");
    assert!((got_i2 - expected_i).abs() < 1e-12, "i2: expected {expected_i}, got {got_i2}");
    // KCL: the two resistor currents balance at the operating point we
    // supplied — this is what makes it a valid DC point, not just two
    // independent Ohm's-law computations.
    assert!((got_i1 - got_i2).abs() < 1e-12, "the operating point must balance KCL");

    // Contributions were recorded too (the analog-write half of the op
    // pair), same values.
    assert_eq!(interp.contribution(rc.n_in, rc.n_out), Some(got_i1));
    assert_eq!(interp.contribution(rc.n_out, rc.n_gnd), Some(got_i2));
}

#[test]
fn rc_divider_round_trips_through_print_and_parse() {
    let rc = build_rc_divider();
    let text = rc.module.print();
    let parsed = Module::parse(&text).expect("printed RC divider must reparse");
    assert!(
        parsed.module.structurally_equivalent(&rc.module),
        "parse(print(m)) must be structurally equivalent to m:\n{text}"
    );
}

// ─── 4-bit counter ────────────────────────────────────────────────────────

struct Counter4 {
    module: Module,
    clk_v: piperine_ir::core::ValueId,
    one: piperine_ir::core::ValueId,
    count: piperine_ir::core::CellId,
}

/// `count <= count + 1` on every rising edge of an externally-driven
/// clock value — `CellRead` -> `Pure::Add` (width 4, wraps at 16) ->
/// `CellWrite`, gated by `Trigger::Edge`.
fn build_counter4() -> Counter4 {
    let mut m = Module::new();
    let clk_v = m.push_value(Ty::Bits(1));
    let one = m.push_value(Ty::Bits(4));
    let init = m.push_const(Const::Bits(0));
    let block = m.push_block(Block::new(
        Domain::Digital,
        Trigger::Edge(SigRef(vec![Chunk::Slice { value: clk_v, lo: 0, hi: 1 }]), EdgeKind::Rising),
    ));
    let count = m.push_cell(Cell::new(Ty::Bits(4), init, block));
    let old = m.push_value(Ty::Bits(4));
    let next = m.push_value(Ty::Bits(4));
    m.block_at_mut(block).push(Op::CellRead { cell: count, addr: None, out: old });
    m.block_at_mut(block).push(Op::Pure { op: PureOp::Add, args: vec![old, one], out: next });
    m.block_at_mut(block).push(Op::CellWrite { cell: count, addr: None, value: next });

    Counter4 { module: m, clk_v, one, count }
}

/// 20 rising edges of a 4-bit (mod-16) counter starting at 0: the
/// hand-computed sequence is `(k % 16)` for `k` in `1..=20`, so the FINAL
/// value after all 20 edges is `20 % 16 = 4`. The test also samples the
/// value after every edge (not just the end) against `k % 16` — the
/// Done-when's "sequence over >= 8 cycles matched", not just one point.
#[test]
fn counter4_hand_computed_sequence_over_twenty_cycles_matched() {
    let c4 = build_counter4();
    Verifier::new(&c4.module).run().expect("counter4 fixture must verify");

    let mut interp = Interpreter::new(&c4.module);
    interp.seed_value(c4.one, CellValue::Bits(1));

    // Arm the edge detector: first sample must be low.
    interp_set_clk(&mut interp, c4.clk_v, 0);
    interp.step().unwrap();

    for k in 1..=20u128 {
        // Rising edge: low -> high fires; the block increments `count`.
        interp_set_clk(&mut interp, c4.clk_v, 1);
        interp.step().unwrap();
        let got = match interp.cell_value(c4.count) {
            Some(CellValue::Bits(b)) => *b,
            other => panic!("count must be Bits, got {other:?}"),
        };
        assert_eq!(got, k % 16, "after edge {k}, hand-computed count is {} (mod 16)", k % 16);
        // Fall back low so the NEXT iteration's rising edge can fire.
        interp_set_clk(&mut interp, c4.clk_v, 0);
        interp.step().unwrap();
    }
}

#[test]
fn counter4_round_trips_through_print_and_parse() {
    let c4 = build_counter4();
    let text = c4.module.print();
    let parsed = Module::parse(&text).expect("printed counter4 must reparse");
    assert!(
        parsed.module.structurally_equivalent(&c4.module),
        "parse(print(m)) must be structurally equivalent to m:\n{text}"
    );
}

/// Small local helper: seed the clock value for one step (kept local to
/// this test file — not shared harness machinery).
fn interp_set_clk(interp: &mut Interpreter, clk_v: piperine_ir::core::ValueId, bit: u128) {
    interp.seed_value(clk_v, CellValue::Bits(bit));
}
