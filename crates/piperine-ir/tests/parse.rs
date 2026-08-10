//! Integration test for the IR parser (T9) — round-trip property:
//! `parse(print(m))` produces a module structurally equivalent to `m` for
//! every fixture. Plus the malformed-input cases asserting a named error
//! (no panic — the IR text is user input for hand-written fixtures).
//!
//! LOV-02 (round-trippable). The grammar is documented in
//! `crates/piperine-ir/src/print.rs`; the parser lives in
//! `crates/piperine-ir/src/parse.rs`.

use piperine_ir::core::{
    Block, Cell, Chunk, Const, CrossDir, Domain, EdgeKind, Module, Net, NatureInfo, NatureKind,
    Op, Pattern, PureOp, SigRef, Trigger, Ty,
};
use piperine_ir::parse::ParseError;
use piperine_ir::print::Names;

/// Common helper: parse the printed form of `m` and assert structural
/// equivalence with the original.
fn assert_roundtrips(m: &Module, names: &Names) {
    let text = m.print_with_names(names);
    let parsed = Module::parse(&text).expect("a well-formed print() must round-trip");
    assert!(
        parsed.module.structurally_equivalent(m),
        "round-trip mismatch.\n--- printed:\n{text}\n--- parsed equivalent: {}",
        parsed.module.print()
    );
    // The names table round-trips too: re-printing the parsed module under
    // the parsed names gives byte-identical text. (Names side-table
    // identity.)
    let reparsed_text = parsed.module.print_with_names(&parsed.names);
    assert_eq!(text, reparsed_text, "name-table round-trip not byte-identical");
}

/// Fixture 1: empty module — every section present, none populated.
#[test]
fn round_trip_empty_module() {
    let m = Module::new();
    assert_roundtrips(&m, &Names::new());
}

/// Fixture 2: only values (real, bits, array).
#[test]
fn round_trip_only_values() {
    let mut m = Module::new();
    m.push_value(Ty::Real);
    m.push_value(Ty::Bits(1));
    m.push_value(Ty::Bits(32));
    m.push_value(Ty::Array(Box::new(Ty::Bits(32)), 1024));
    m.push_value(Ty::Array(Box::new(Ty::Array(Box::new(Ty::Real), 4)), 4));
    assert_roundtrips(&m, &Names::new());
}

/// Fixture 3: only nets (nodes and signals, two disciplines).
#[test]
fn round_trip_only_nets() {
    let mut m = Module::new();
    let v = m.push_nature(NatureInfo { access: "voltage".into(), kind: NatureKind::Across });
    let i = m.push_nature(NatureInfo { access: "current".into(), kind: NatureKind::Through });
    let t = m.push_nature(NatureInfo { access: "temp".into(), kind: NatureKind::Across });
    let p = m.push_nature(NatureInfo { access: "pwr".into(), kind: NatureKind::Through });
    m.push_net(Net::node(v, i));
    m.push_net(Net::node(t, p));
    m.push_net(Net::signal(Ty::Bits(1)));
    m.push_net(Net::signal(Ty::Bits(8)));
    assert_roundtrips(&m, &Names::new());
}

/// Fixture 4: a module exercising every op kind — pure, call, all six
/// net accesses, cell read/write (with and without address), If, Match.
#[test]
fn round_trip_every_op_kind() {
    let mut m = Module::new();
    let v = m.push_nature(NatureInfo { access: "v".into(), kind: NatureKind::Across });
    let i = m.push_nature(NatureInfo { access: "i".into(), kind: NatureKind::Through });
    let n_in = m.push_net(Net::node(v, i));
    let n_gnd = m.push_net(Net::node(v, i));
    let n_sig = m.push_net(Net::signal(Ty::Bits(8)));
    let init_real = m.push_const(Const::Real(0.0));
    let init_bits = m.push_const(Const::Bits(0));
    let writer = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    let writer_d = m.push_block(Block::new(Domain::Digital, Trigger::Always));
    let reg = m.push_cell(Cell::new(Ty::Real, init_real, writer));
    let mem = m.push_cell(Cell::new(Ty::Array(Box::new(Ty::Bits(32)), 4), init_bits, writer_d));
    let ra = m.push_value(Ty::Real);
    let rt = m.push_value(Ty::Real);
    let pa = m.push_value(Ty::Real);
    let pb = m.push_value(Ty::Real);
    let sum = m.push_value(Ty::Real);
    let cmp = m.push_value(Ty::Bits(1));
    let sel = m.push_value(Ty::Real);
    let cond = m.push_value(Ty::Bits(1));
    let sig_in = m.push_value(Ty::Bits(8));
    let mem_word = m.push_value(Ty::Bits(32));
    let addr = m.push_value(Ty::Bits(2));
    let reg_out = m.push_value(Ty::Real);
    let _ = m.push_value(Ty::Real); // unused filler

    m.block_at_mut(writer).push(Op::ReadAcross { a: n_in, b: n_gnd, out: ra });
    m.block_at_mut(writer).push(Op::ReadThrough { a: n_in, b: n_gnd, out: rt });
    m.block_at_mut(writer).push(Op::Pure { op: PureOp::Add, args: vec![pa, pb], out: sum });
    m.block_at_mut(writer).push(Op::Pure { op: PureOp::Mul, args: vec![pa, pb], out: sum });
    m.block_at_mut(writer).push(Op::Pure { op: PureOp::Sub, args: vec![pa, pb], out: sum });
    m.block_at_mut(writer).push(Op::Pure { op: PureOp::Div, args: vec![pa, pb], out: sum });
    m.block_at_mut(writer).push(Op::Pure { op: PureOp::Neg, args: vec![pa], out: sum });
    m.block_at_mut(writer).push(Op::Pure { op: PureOp::Eq, args: vec![pa, pb], out: cmp });
    m.block_at_mut(writer).push(Op::Pure { op: PureOp::Ne, args: vec![pa, pb], out: cmp });
    m.block_at_mut(writer).push(Op::Pure { op: PureOp::Lt, args: vec![pa, pb], out: cmp });
    m.block_at_mut(writer).push(Op::Pure { op: PureOp::Le, args: vec![pa, pb], out: cmp });
    m.block_at_mut(writer).push(Op::Pure { op: PureOp::Gt, args: vec![pa, pb], out: cmp });
    m.block_at_mut(writer).push(Op::Pure { op: PureOp::Ge, args: vec![pa, pb], out: cmp });
    m.block_at_mut(writer).push(Op::Pure {
        op: PureOp::Select,
        args: vec![cond, pa, pb],
        out: sel,
    });
    m.block_at_mut(writer).push(Op::Call {
        intrinsic: piperine_ir::core::IntrinsicId(0),
        args: vec![pa],
        out: sum,
    });
    m.block_at_mut(writer).push(Op::ContributeThrough { a: n_in, b: n_gnd, value: sum });
    m.block_at_mut(writer).push(Op::ForceAcross { a: n_in, b: n_gnd, value: pa });
    m.block_at_mut(writer).push(Op::CellRead { cell: reg, addr: None, out: reg_out });
    m.block_at_mut(writer).push(Op::CellWrite { cell: reg, addr: None, value: sum });
    m.block_at_mut(writer).push(Op::If {
        cond,
        then_ops: vec![Op::ContributeThrough { a: n_in, b: n_gnd, value: pa }],
        else_ops: vec![Op::ForceAcross { a: n_in, b: n_gnd, value: pb }],
    });
    m.block_at_mut(writer).push(Op::Match {
        scrutinee: cond,
        arms: vec![
            (Pattern::Zero, vec![Op::ForceAcross { a: n_in, b: n_gnd, value: pa }]),
            (Pattern::One, vec![Op::ContributeThrough { a: n_in, b: n_gnd, value: pb }]),
            (Pattern::Wildcard, vec![]),
        ],
    });
    m.block_at_mut(writer_d).push(Op::ReadSignal { net: n_sig, out: sig_in });
    m.block_at_mut(writer_d).push(Op::Drive { net: n_sig, value: sig_in });
    m.block_at_mut(writer_d).push(Op::CellRead { cell: mem, addr: Some(addr), out: mem_word });
    m.block_at_mut(writer_d).push(Op::CellWrite { cell: mem, addr: Some(addr), value: mem_word });

    assert_roundtrips(&m, &Names::new());
}

/// Fixture 5: chunk-list sigrefs in an `Edge` trigger — the multi-chunk
/// shape that motivated the SigRef-as-representation design.
#[test]
fn round_trip_chunk_list_sigrefs_in_edge_trigger() {
    let mut m = Module::new();
    let bus = m.push_value(Ty::Bits(32));
    let other = m.push_value(Ty::Bits(8));
    let zero_c = m.push_const(Const::Bits(0));
    // Three-chunk sigref: {zero_c, bus[31:28], other[7:0]} → 1+4+8 = 13.
    let sr = SigRef(vec![
        Chunk::Const(zero_c),
        Chunk::Slice { value: bus, lo: 28, hi: 32 },
        Chunk::Slice { value: other, lo: 0, hi: 8 },
    ]);
    let _ = m.push_block(Block::new(Domain::Digital, Trigger::Edge(sr.clone(), EdgeKind::Falling)));
    let _ = m.push_block(Block::new(Domain::Digital, Trigger::Edge(sr, EdgeKind::Any)));
    let init = m.push_const(Const::Real(0.0));
    let writer = m.push_block(Block::new(Domain::Digital, Trigger::Initial));
    let _ = m.push_cell(Cell::new(Ty::Real, init, writer));
    assert_roundtrips(&m, &Names::new());
}

/// Fixture 6: the same module as fixture 4 but with authored names mingled
/// — round-trip preserves the names table AND the module.
#[test]
fn round_trip_with_authored_names() {
    let mut m = Module::new();
    let v = m.push_nature(NatureInfo { access: "v".into(), kind: NatureKind::Across });
    let i = m.push_nature(NatureInfo { access: "i".into(), kind: NatureKind::Through });
    let n_in = m.push_net(Net::node(v, i));
    let n_gnd = m.push_net(Net::node(v, i));
    let init = m.push_const(Const::Real(0.0));
    let writer = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    let acc = m.push_cell(Cell::new(Ty::Real, init, writer));
    let v_in = m.push_value(Ty::Real);
    let v_one = m.push_value(Ty::Real);
    let v_sum = m.push_value(Ty::Real);
    m.block_at_mut(writer).push(Op::ReadAcross { a: n_in, b: n_gnd, out: v_in });
    m.block_at_mut(writer).push(Op::Pure { op: PureOp::Add, args: vec![v_in, v_one], out: v_sum });
    m.block_at_mut(writer).push(Op::CellWrite { cell: acc, addr: None, value: v_sum });

    let mut names = Names::new();
    names.set_net(n_in, "in");
    names.set_net(n_gnd, "gnd");
    names.set_cell(acc, "acc");
    names.set_block(writer, "loop");
    names.set_value(v_in, "vin");
    // v_one and v_sum stay generated.

    assert_roundtrips(&m, &names);
}

/// Fixture 7: every trigger kind, in one module, end-to-end.
#[test]
fn round_trip_every_trigger_kind() {
    let mut m = Module::new();
    let v = m.push_nature(NatureInfo { access: "v".into(), kind: NatureKind::Across });
    let i = m.push_nature(NatureInfo { access: "i".into(), kind: NatureKind::Through });
    let _ = m.push_net(Net::node(v, i));
    let _ = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    let _ = m.push_block(Block::new(Domain::Digital, Trigger::Initial));
    let clk_v = m.push_value(Ty::Bits(1));
    let clk_sr = SigRef(vec![Chunk::Slice { value: clk_v, lo: 0, hi: 1 }]);
    let _ = m.push_block(Block::new(Domain::Digital, Trigger::Edge(clk_sr, EdgeKind::Rising)));
    let cross_v = m.push_value(Ty::Real);
    let _ = m.push_block(Block::new(Domain::Analog, Trigger::Cross(cross_v, CrossDir::Either)));
    let p = m.push_value(Ty::Real);
    let ph = m.push_value(Ty::Real);
    let _ = m.push_block(Block::new(Domain::Analog, Trigger::Timer { period: p, phase: ph }));

    assert_roundtrips(&m, &Names::new());
}

/// Malformed 1: a missing `(module ...)` header is a named error, never a
/// panic.
#[test]
fn malformed_missing_module_header_is_named_error() {
    // Bare text with no parens: the parser sees an unexpected ident at the
    // top — a typed error, not a panic.
    let err = Module::parse("not-a-module").unwrap_err();
    let _ = err; // assertion: typed error, not panic
    // Wrong top-level keyword: a typed MissingModuleHeader.
    let err = Module::parse("(not-a-module)").unwrap_err();
    assert_eq!(err, ParseError::MissingModuleHeader);
}

/// Malformed 2: an unknown section name is a named error.
#[test]
fn malformed_unknown_section_is_named_error() {
    let err = Module::parse("(module (bogus-section))").unwrap_err();
    assert!(matches!(err, ParseError::UnknownSection(_)));
    let msg = err.to_string();
    assert!(msg.contains("bogus-section"), "message names the section: {msg}");
}

/// Malformed 3: an id declared out of arena order is a named error.
#[test]
fn malformed_id_out_of_order_is_named_error() {
    let err = Module::parse("(module (values (value $1 real)))").unwrap_err();
    assert!(matches!(err, ParseError::IdOutOfOrder { .. }));
}

/// Malformed 4: an unbalanced paren is a named error (the parser never
/// panics on truncated input).
#[test]
fn malformed_unbalanced_parens_is_named_error() {
    let err = Module::parse("(module (natures").unwrap_err();
    let _ = err; // assertion: a typed error, not a panic
    // Any of the EOF/Token variants is acceptable; the contract is "named
    // error, never panic".
    let err = Module::parse("(module (natures (nature N0 (access \"v\") across))").unwrap_err();
    let _ = err;
}

/// Malformed 5: a reference to an undeclared value is a named error.
#[test]
fn malformed_undeclared_value_reference_is_named_error() {
    let text = "(module
      (values)
      (blocks
        (block $0 analog always
          ($5 = pure add $0 $0))))";
    let err = Module::parse(text).unwrap_err();
    assert!(matches!(err, ParseError::UndeclaredReference { .. } |
                      ParseError::IdOutOfOrder { .. }));
}

/// Malformed 6: a duplicate section is a named error.
#[test]
fn malformed_duplicate_section_is_named_error() {
    let err = Module::parse("(module (natures) (natures))").unwrap_err();
    assert!(matches!(err, ParseError::DuplicateSection(_)));
}

/// Malformed 7: an invalid number literal is a named error.
#[test]
fn malformed_invalid_number_literal_is_named_error() {
    let err = Module::parse("(module (consts (const K0 real not-a-number)))").unwrap_err();
    assert!(matches!(err, ParseError::InvalidNumber(_)));
}

/// Empty input is a named error, not a panic.
#[test]
fn empty_input_is_a_named_error() {
    let err = Module::parse("").unwrap_err();
    let _ = err; // assertion: typed error, not panic
}

/// Whitespace-only input is a named error.
#[test]
fn whitespace_only_input_is_a_named_error() {
    let err = Module::parse("   \n\t  \n").unwrap_err();
    let _ = err;
}

/// A parse error displays its reason — for the diagnostic surface.
#[test]
fn parse_error_displays_its_reason() {
    let err = Module::parse("(module (bogus))").unwrap_err();
    let msg = err.to_string();
    assert!(!msg.is_empty());
    assert!(msg.contains("bogus"), "message names the offender: {msg}");
}
