//! T16 — one fixture per historical seam defect (`GAPS.md` A0/A0b/A0c/A0d),
//! proving each construct is now rejected loudly (or simply cannot
//! disagree with itself) rather than silently mis-executed. These are
//! IR-level fixtures, hand-built via the `piperine_ir::core` API — PHDL
//! lowering does not exist until Phase 6.

use piperine_ir::core::{Block, Cell, Const, Domain, Net, NatureInfo, NatureKind, Op, Trigger, Ty};
use piperine_ir::interp::{CellValue, Interpreter};
use piperine_ir::verify::{VerifyError, Verifier};

fn electrical_node_pair(m: &mut piperine_ir::core::Module) -> (piperine_ir::core::NetId, piperine_ir::core::NetId) {
    let v = m.push_nature(NatureInfo { access: "voltage".into(), kind: NatureKind::Across });
    let i = m.push_nature(NatureInfo { access: "current".into(), kind: NatureKind::Through });
    (m.push_net(Net::node(v, i)), m.push_net(Net::node(v, i)))
}

// ─── A0: read_across and read_through on the same node pair must differ ────
//
// The historical defect: `emit_analog_branch` served "V" | "I" off the
// same `branch_voltages` lookup, so a contribution was silently read as a
// force. Here the two are separate `Op` variants (T5); this test asserts
// they compute DIFFERENT hand-computed values from the same node pair —
// distinctness is the proof, not a verifier rejection (Edge Cases in
// spec.md: "they SHALL produce distinct values").

#[test]
fn a0_read_across_and_read_through_yield_distinct_hand_computed_values() {
    let mut m = piperine_ir::core::Module::new();
    let (a, b) = electrical_node_pair(&mut m);
    let v_out = m.push_value(Ty::Real);
    let i_out = m.push_value(Ty::Real);
    let block = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    m.block_at_mut(block).push(Op::ReadAcross { a, b, out: v_out });
    m.block_at_mut(block).push(Op::ReadThrough { a, b, out: i_out });
    Verifier::new(&m).run().expect("fixture must verify");

    // Hand-computed: across = 5.0 V - 2.0 V = 3.0 V; through = 0.02 A -
    // 0.005 A = 0.015 A. These are DIFFERENT numbers precisely because
    // read_across and read_through are different ops over the same pair.
    let mut interp = Interpreter::new(&m);
    interp.set_node_values(a, 5.0, 0.02);
    interp.set_node_values(b, 2.0, 0.005);
    interp.step().unwrap();

    let got_v = interp.value(v_out).cloned().expect("v_out computed");
    let got_i = interp.value(i_out).cloned().expect("i_out computed");
    assert_eq!(got_v, CellValue::Real(3.0), "hand-computed across value");
    assert_eq!(got_i, CellValue::Real(0.015), "hand-computed through value");
    assert_ne!(got_v, got_i, "the A0 shape: across and through must differ, never collapse");
}

// ─── A0b: a Signal net handed to analog stamping is rejected BEFORE any
//          matrix (a driven signal-flow net cannot produce a symbolic-
//          singular matrix, because the verifier never lets it reach one).

#[test]
fn a0b_signal_net_rejected_for_analog_stamping_before_any_matrix() {
    let mut m = piperine_ir::core::Module::new();
    // A `Signal`, NOT a `Node` — handing it to analog stamping is the bug.
    let sig = m.push_net(Net::signal(Ty::Real));
    let gnd = m.push_net(Net::signal(Ty::Real));
    let value = m.push_value(Ty::Real);
    let block = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    m.block_at_mut(block).push(Op::ForceAcross { a: sig, b: gnd, value });

    // The rejection happens at VERIFY time — strictly before any backend
    // could build a matrix from this module (no analysis, no stamping,
    // no solve was attempted above).
    let err = Verifier::new(&m).run().expect_err("a Signal must never reach analog stamping");
    match err {
        VerifyError::AnalogOpOnSignal { net, .. } => assert_eq!(net, sig, "error names the offending Signal net"),
        other => panic!("expected AnalogOpOnSignal, got {other:?}"),
    }
}

// ─── A0c: a write with no matching net-access op fails by name — there is
//          no catch-all path for a bare-identifier destination to fall
//          into (`flatten/analog.rs::parse_dest`'s ground-to-ground
//          default, made unrepresentable). Concretely: writing through
//          the WRONG-domain op is rejected BY NAME, never silently
//          accepted or silently remapped to ground.

#[test]
fn a0c_wrong_domain_write_rejected_by_name_no_fallthrough_to_ground() {
    let mut m = piperine_ir::core::Module::new();
    let sig = m.push_net(Net::signal(Ty::Bits(1)));
    let value = m.push_value(Ty::Bits(1));
    // An ANALOG-domain block containing a `Drive` — the digital-write op.
    // The historical bug had no named failure here; a bare destination
    // silently became a ground-to-ground contribution. Here there is no
    // arm for "wrong op in this domain" to fall into unnoticed.
    let block = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    m.block_at_mut(block).push(Op::Drive { net: sig, value });

    let err = Verifier::new(&m).run().expect_err("Drive must not silently succeed in an Analog block");
    match err {
        VerifyError::DomainOpMismatch { domain: Domain::Analog, op_kind: "Drive", .. } => {}
        other => panic!("expected DomainOpMismatch naming Drive in Analog, got {other:?}"),
    }
}

// ─── A0d: a cell written by two blocks is rejected, naming both. `Cell`'s
//          `writer` is a single FIELD (T4) so a second CLAIMED writer
//          cannot even be constructed; the remaining way to get two
//          writers is a rogue `CellWrite` in a block that ISN'T the
//          declared writer — the verifier now catches that too (added in
//          this task; see `verify.rs::CellWrittenByWrongBlock`).

#[test]
fn a0d_cell_written_by_a_block_other_than_its_declared_writer_is_rejected_naming_both() {
    let mut m = piperine_ir::core::Module::new();
    let init = m.push_const(Const::Real(0.0));
    // Two blocks — `declared_writer` is the ONLY one the cell names.
    let declared_writer = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    let rogue_writer = m.push_block(Block::new(Domain::Digital, Trigger::Always));
    let cell = m.push_cell(Cell::new(Ty::Real, init, declared_writer));

    // The rogue block writes the same cell anyway — exactly the A0d shape:
    // one module-level state slot, two blocks both acting as if they own
    // it (historically: the analog var bank and the digital var bank).
    let value = m.push_value(Ty::Real);
    m.block_at_mut(rogue_writer).push(Op::CellWrite { cell, addr: None, value });

    let err = Verifier::new(&m).run().expect_err("a second writer must be rejected");
    match err {
        VerifyError::CellWrittenByWrongBlock { cell: c, declared_writer: dw, writing_block: wb, .. } => {
            assert_eq!(c, cell, "error names the offending cell");
            assert_eq!(dw, declared_writer, "error names the declared writer");
            assert_eq!(wb, rogue_writer, "error names the rogue writer");
        }
        other => panic!("expected CellWrittenByWrongBlock, got {other:?}"),
    }
}

/// Mirror: the SAME cell, written only by its declared writer, passes.
/// Both directions are required — a rejection test that always rejects
/// proves nothing about the check (`tests/verify.rs`'s own convention).
#[test]
fn a0d_mirror_a_cell_written_only_by_its_declared_writer_passes() {
    let mut m = piperine_ir::core::Module::new();
    let init = m.push_const(Const::Real(0.0));
    let writer = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    let cell = m.push_cell(Cell::new(Ty::Real, init, writer));
    let value = m.push_value(Ty::Real);
    m.block_at_mut(writer).push(Op::CellWrite { cell, addr: None, value });
    assert!(Verifier::new(&m).run().is_ok());
}
