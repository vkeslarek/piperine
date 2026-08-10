//! Integration tests for the IR verifier (LOV-03/04/14, MD-31).
//!
//! Each check has ONE rejection fixture that proves the gate can fail
//! naming the offending entity, and ONE well-formed variant that proves
//! the same fixture passes once the violation is removed. Both directions
//! are required: a test that always fails proves nothing about the gate.
//!
//! Fixtures build small `Module`s directly (no POM lowering) so the test
//! reaches ONLY the verifier — the rest of the pipeline is out of scope.

use piperine_ir::core::{
    Block, Cell, Chunk, Const, Domain, EdgeKind, Module, Net, NatureInfo, NatureKind, Op,
    Pattern, PureOp, SigRef, Trigger, Ty,
};
use piperine_ir::verify::{VerifyError, Verifier};

// ─── Check 1: a Cell's writer must reference a declared block ────────────────

#[test]
fn a_cell_with_an_undeclared_writer_is_rejected_naming_the_cell_and_writer() {
    let mut m = Module::new();
    // Declare zero blocks; claim BlockId(99) as the writer.
    let init = m.push_const(Const::Real(0.0));
    let bad_writer = piperine_ir::core::BlockId(99);
    let cell = m.push_cell(Cell::new(Ty::Real, init, bad_writer));

    let err = Verifier::new(&m).run().expect_err("BlockId(99) is undeclared");
    match err {
        VerifyError::CellWriterNotDeclared { cell: c, writer, block_count: 0 } => {
            assert_eq!(c, cell, "error names the offending cell");
            assert_eq!(writer, bad_writer, "error names the bad writer id");
        }
        other => panic!("expected CellWriterNotDeclared, got {other:?}"),
    }
}

#[test]
fn a_cell_with_a_declared_writer_passes_the_same_check() {
    let mut m = Module::new();
    let writer = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    let init = m.push_const(Const::Real(0.0));
    m.push_cell(Cell::new(Ty::Real, init, writer));
    assert!(Verifier::new(&m).run().is_ok());
}

// ─── Check 4 (A0b): an analog op on a Signal net is rejected ────────────────

#[test]
fn an_analog_op_on_a_signal_is_rejected_naming_the_net() {
    let mut m = Module::new();
    // A Signal net, NOT a Node — handing it to analog stamping is the bug.
    let sig = m.push_net(Net::signal(Ty::Bits(1)));
    let g = m.push_net(Net::signal(Ty::Bits(1)));
    let out = m.push_value(Ty::Real);

    let mut block = Block::new(Domain::Analog, Trigger::Always);
    block.push(Op::ReadAcross { a: sig, b: g, out });
    let block_id = m.push_block(block);

    let err = Verifier::new(&m).run().expect_err("ReadAcross on a Signal must be rejected");
    match err {
        VerifyError::AnalogOpOnSignal { site, net } => {
            assert_eq!(net, sig, "error names the offending Signal net");
            assert_eq!(site.block, block_id);
        }
        other => panic!("expected AnalogOpOnSignal, got {other:?}"),
    }
}

#[test]
fn the_same_op_on_a_node_net_passes() {
    let mut m = Module::new();
    // Build a proper Node net pair (with across/through natures).
    let across = m.push_nature(NatureInfo { access: "voltage".into(), kind: NatureKind::Across });
    let through = m.push_nature(NatureInfo { access: "current".into(), kind: NatureKind::Through });
    let n = m.push_net(Net::node(across, through));
    let g = m.push_net(Net::node(across, through));
    let out = m.push_value(Ty::Real);

    let mut block = Block::new(Domain::Analog, Trigger::Always);
    block.push(Op::ReadAcross { a: n, b: g, out });
    m.push_block(block);
    assert!(Verifier::new(&m).run().is_ok());
}

// ─── Check 4 (mirror): a signal op on a Node net is rejected ────────────────

#[test]
fn a_signal_op_on_a_node_is_rejected_naming_the_net() {
    let mut m = Module::new();
    let across = m.push_nature(NatureInfo { access: "voltage".into(), kind: NatureKind::Across });
    let through = m.push_nature(NatureInfo { access: "current".into(), kind: NatureKind::Through });
    let node = m.push_net(Net::node(across, through));
    let out = m.push_value(Ty::Bits(1));

    let mut block = Block::new(Domain::Digital, Trigger::Always);
    block.push(Op::ReadSignal { net: node, out });
    let block_id = m.push_block(block);

    let err = Verifier::new(&m).run().expect_err("ReadSignal on a Node must be rejected");
    match err {
        VerifyError::SignalOpOnNode { site, net } => {
            assert_eq!(net, node, "error names the offending Node net");
            assert_eq!(site.block, block_id);
        }
        other => panic!("expected SignalOpOnNode, got {other:?}"),
    }
}

#[test]
fn the_same_op_on_a_signal_passes() {
    let mut m = Module::new();
    let sig = m.push_net(Net::signal(Ty::Bits(1)));
    let out = m.push_value(Ty::Bits(1));

    let mut block = Block::new(Domain::Digital, Trigger::Always);
    block.push(Op::ReadSignal { net: sig, out });
    m.push_block(block);
    assert!(Verifier::new(&m).run().is_ok());
}

// ─── Check 3: a Chunk::Slice out of bounds is rejected ──────────────────────

#[test]
fn a_chunk_slice_past_the_source_width_is_rejected_naming_the_chunk() {
    let mut m = Module::new();
    let x = m.push_value(Ty::Bits(8)); // width 8
    let block = Block::new(
        Domain::Digital,
        Trigger::Edge(
            SigRef(vec![Chunk::Slice { value: x, lo: 0, hi: 16 }]), // hi > 8
            EdgeKind::Rising,
        ),
    );
    // Block needs no ops for this check — the trigger is what's broken.
    let _ = m.push_block(block);

    let err = Verifier::new(&m).run().expect_err("Slice [0:16] on Bits(8) is out of bounds");
    match err {
        VerifyError::ChunkOutOfBounds { chunk_index: 0, source, lo: 0, hi: 16, source_width: 8 } => {
            assert_eq!(source, x, "error names the source value");
        }
        other => panic!("expected ChunkOutOfBounds, got {other:?}"),
    }
}

#[test]
fn a_chunk_slice_within_bounds_passes() {
    let mut m = Module::new();
    let x = m.push_value(Ty::Bits(8));
    let mut block = Block::new(
        Domain::Digital,
        Trigger::Edge(
            SigRef(vec![Chunk::Slice { value: x, lo: 0, hi: 8 }]), // hi == width, OK
            EdgeKind::Rising,
        ),
    );
    block.push(Op::ReadSignal { net: m.push_net(Net::signal(Ty::Bits(1))), out: m.push_value(Ty::Bits(1)) });
    m.push_block(block);
    assert!(Verifier::new(&m).run().is_ok());
}

// ─── Check 2: op output type must match what the op requires ────────────────

#[test]
fn a_read_across_whose_output_is_bits_not_real_is_rejected() {
    let mut m = Module::new();
    let across = m.push_nature(NatureInfo { access: "voltage".into(), kind: NatureKind::Across });
    let through = m.push_nature(NatureInfo { access: "current".into(), kind: NatureKind::Through });
    let n = m.push_net(Net::node(across, through));
    let g = m.push_net(Net::node(across, through));
    let bad_out = m.push_value(Ty::Bits(32)); // should be Real

    let mut block = Block::new(Domain::Analog, Trigger::Always);
    block.push(Op::ReadAcross { a: n, b: g, out: bad_out });
    let block_id = m.push_block(block);

    let err = Verifier::new(&m).run().expect_err("ReadAcross output must be Real");
    match err {
        VerifyError::OutputTypeMismatch { site, out, expected: Ty::Real, found, .. } => {
            assert_eq!(out, bad_out, "error names the offending value");
            assert_eq!(found, Ty::Bits(32));
            assert_eq!(site.block, block_id);
        }
        other => panic!("expected OutputTypeMismatch, got {other:?}"),
    }
}

#[test]
fn a_read_across_with_a_real_output_passes() {
    let mut m = Module::new();
    let across = m.push_nature(NatureInfo { access: "voltage".into(), kind: NatureKind::Across });
    let through = m.push_nature(NatureInfo { access: "current".into(), kind: NatureKind::Through });
    let n = m.push_net(Net::node(across, through));
    let g = m.push_net(Net::node(across, through));
    let out = m.push_value(Ty::Real);
    let mut block = Block::new(Domain::Analog, Trigger::Always);
    block.push(Op::ReadAcross { a: n, b: g, out });
    m.push_block(block);
    assert!(Verifier::new(&m).run().is_ok());
}

// ─── Check 2 (mirror): CellRead output must match the cell's element type ───

#[test]
fn a_cell_read_whose_output_does_not_match_the_cell_element_is_rejected() {
    let mut m = Module::new();
    let writer = m.push_block(Block::new(Domain::Digital, Trigger::Always));
    let init = m.push_const(Const::Bits(0));
    // A memory of Bits(8) words; element type Bits(8).
    let mem = m.push_cell(Cell::new(Ty::Array(Box::new(Ty::Bits(8)), 16), init, writer));
    let addr = m.push_value(Ty::Bits(4));
    let bad_out = m.push_value(Ty::Bits(32)); // should be Bits(8) — element type

    let mut block = Block::new(Domain::Digital, Trigger::Always);
    block.push(Op::CellRead { cell: mem, addr: Some(addr), out: bad_out });
    let block_id = m.push_block(block);

    let err = Verifier::new(&m).run().expect_err("CellRead output must match element type");
    match err {
        VerifyError::OutputTypeMismatch { site, out, expected: Ty::Bits(8), found: Ty::Bits(32), .. } => {
            assert_eq!(out, bad_out, "error names the offending value");
            assert_eq!(site.block, block_id);
        }
        other => panic!("expected OutputTypeMismatch, got {other:?}"),
    }
}

#[test]
fn a_cell_read_with_a_matching_output_passes() {
    let mut m = Module::new();
    let writer = m.push_block(Block::new(Domain::Digital, Trigger::Always));
    let init = m.push_const(Const::Bits(0));
    let mem = m.push_cell(Cell::new(Ty::Array(Box::new(Ty::Bits(8)), 16), init, writer));
    let addr = m.push_value(Ty::Bits(4));
    let out = m.push_value(Ty::Bits(8));
    let mut block = Block::new(Domain::Digital, Trigger::Always);
    block.push(Op::CellRead { cell: mem, addr: Some(addr), out });
    m.push_block(block);
    assert!(Verifier::new(&m).run().is_ok());
}

// ─── Check 2 (Pure op widths): arithmetic arg widths must agree ─────────────

#[test]
fn an_add_op_with_mismatched_arg_widths_is_rejected() {
    let mut m = Module::new();
    let a = m.push_value(Ty::Bits(8));
    let b = m.push_value(Ty::Bits(16)); // mismatch
    let out = m.push_value(Ty::Bits(8));

    let mut block = Block::new(Domain::Digital, Trigger::Always);
    block.push(Op::Pure { op: PureOp::Add, args: vec![a, b], out });
    let block_id = m.push_block(block);

    let err = Verifier::new(&m).run().expect_err("Add arg widths must agree");
    assert!(matches!(
        err,
        VerifyError::OutputTypeMismatch { site, .. } if site.block == block_id
    ));
}

#[test]
fn an_add_op_with_matching_arg_widths_passes() {
    let mut m = Module::new();
    let a = m.push_value(Ty::Bits(8));
    let b = m.push_value(Ty::Bits(8));
    let out = m.push_value(Ty::Bits(8));
    let mut block = Block::new(Domain::Digital, Trigger::Always);
    block.push(Op::Pure { op: PureOp::Add, args: vec![a, b], out });
    m.push_block(block);
    assert!(Verifier::new(&m).run().is_ok());
}

// ─── Check 5 (domain/op): Drive only in Digital, ForceAcross only Analog ────

#[test]
fn a_drive_op_in_an_analog_block_is_rejected() {
    let mut m = Module::new();
    let sig = m.push_net(Net::signal(Ty::Bits(1)));
    let v = m.push_value(Ty::Bits(1));
    let mut block = Block::new(Domain::Analog, Trigger::Always);
    block.push(Op::Drive { net: sig, value: v });
    let block_id = m.push_block(block);

    let err = Verifier::new(&m).run().expect_err("Drive must not appear in an Analog block");
    match err {
        VerifyError::DomainOpMismatch { site, domain: Domain::Analog, op_kind: "Drive" } => {
            assert_eq!(site.block, block_id);
        }
        other => panic!("expected DomainOpMismatch, got {other:?}"),
    }
}

#[test]
fn a_drive_op_in_a_digital_block_passes() {
    let mut m = Module::new();
    let sig = m.push_net(Net::signal(Ty::Bits(1)));
    let v = m.push_value(Ty::Bits(1));
    let mut block = Block::new(Domain::Digital, Trigger::Always);
    block.push(Op::Drive { net: sig, value: v });
    m.push_block(block);
    assert!(Verifier::new(&m).run().is_ok());
}

// ─── Undeclared entity references ────────────────────────────────────────────

#[test]
fn an_op_referencing_an_undeclared_value_is_rejected() {
    let mut m = Module::new();
    let undeclared = piperine_ir::core::ValueId(99);
    let mut block = Block::new(Domain::Digital, Trigger::Always);
    block.push(Op::Pure {
        op: PureOp::Add,
        args: vec![m.push_value(Ty::Bits(8)), undeclared],
        out: m.push_value(Ty::Bits(8)),
    });
    let block_id = m.push_block(block);

    let err = Verifier::new(&m).run().expect_err("ValueId(99) is undeclared");
    match err {
        VerifyError::UndeclaredValue { site, value, value_count: _ } => {
            assert_eq!(value, undeclared, "error names the undeclared value");
            assert_eq!(site.block, block_id);
        }
        other => panic!("expected UndeclaredValue, got {other:?}"),
    }
}

#[test]
fn an_op_referencing_an_undeclared_cell_is_rejected() {
    let mut m = Module::new();
    let undeclared = piperine_ir::core::CellId(99);
    let out = m.push_value(Ty::Real);
    let mut block = Block::new(Domain::Digital, Trigger::Always);
    block.push(Op::CellRead { cell: undeclared, addr: None, out });
    let block_id = m.push_block(block);

    let err = Verifier::new(&m).run().expect_err("CellId(99) is undeclared");
    match err {
        VerifyError::UndeclaredCell { site, cell, cell_count: _ } => {
            assert_eq!(cell, undeclared, "error names the undeclared cell");
            assert_eq!(site.block, block_id);
        }
        other => panic!("expected UndeclaredCell, got {other:?}"),
    }
}

// ─── If carries nested ops: domain mismatch is caught in the inner arm ──────

#[test]
fn a_domain_mismatch_in_a_nested_if_arm_is_rejected() {
    let mut m = Module::new();
    let sig = m.push_net(Net::signal(Ty::Bits(1)));
    let v = m.push_value(Ty::Bits(1));
    let cond = m.push_value(Ty::Bits(1));

    // An Analog block — but it contains a Drive (Digital-only op) in its
    // `then` arm. The verifier descends into the If and catches it.
    let mut block = Block::new(Domain::Analog, Trigger::Always);
    block.push(Op::If {
        cond,
        then_ops: vec![Op::Drive { net: sig, value: v }],
        else_ops: vec![],
    });
    let block_id = m.push_block(block);

    let err = Verifier::new(&m).run().expect_err("Drive in an If arm of an Analog block");
    match err {
        VerifyError::DomainOpMismatch { site, domain: Domain::Analog, op_kind: "Drive" } => {
            assert_eq!(site.block, block_id);
        }
        other => panic!("expected DomainOpMismatch, got {other:?}"),
    }
}

// ─── Sanity: a fully-formed small mixed-signal module verifies ──────────────

#[test]
fn a_small_well_formed_module_with_a_node_and_a_signal_verifies() {
    let mut m = Module::new();
    // A thermal node pair.
    let temp = m.push_nature(NatureInfo { access: "temp".into(), kind: NatureKind::Across });
    let pwr = m.push_nature(NatureInfo { access: "pwr".into(), kind: NatureKind::Through });
    let thermal = m.push_net(Net::node(temp, pwr));
    let thermal_ref = m.push_net(Net::node(temp, pwr));

    // A digital signal.
    let clk = m.push_net(Net::signal(Ty::Bits(1)));

    // Analog block: read temperature, contribute power.
    let temp_out = m.push_value(Ty::Real);
    let pwr_val = m.push_value(Ty::Real);
    let mut analog = Block::new(Domain::Analog, Trigger::Always);
    analog.push(Op::ReadAcross { a: thermal, b: thermal_ref, out: temp_out });
    analog.push(Op::ContributeThrough { a: thermal, b: thermal_ref, value: pwr_val });
    m.push_block(analog);

    // Digital block: drive the clock.
    let driven = m.push_value(Ty::Bits(1));
    let mut digital = Block::new(
        Domain::Digital,
        Trigger::Edge(SigRef(vec![Chunk::Slice { value: m.push_value(Ty::Bits(1)), lo: 0, hi: 1 }]), EdgeKind::Rising),
    );
    digital.push(Op::Drive { net: clk, value: driven });
    m.push_block(digital);

    // Match arm to exercise structured control.
    let cond = m.push_value(Ty::Bits(1));
    let mut with_match = Block::new(Domain::Digital, Trigger::Always);
    with_match.push(Op::Match {
        scrutinee: cond,
        arms: vec![
            (Pattern::Zero, vec![Op::Drive { net: clk, value: driven }]),
            (Pattern::One, vec![]),
            (Pattern::Wildcard, vec![]),
        ],
    });
    m.push_block(with_match);

    assert!(Verifier::new(&m).run().is_ok());
}
