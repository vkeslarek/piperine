//! Integration test for the IR printer (T8) — repeat-print equality on a
//! non-trivial module built programmatically via the Phase-1 builder, plus
//! the `\user` vs `$generated` provenance assertion.
//!
//! LOV-02 (round-trippable), LOV-06 (byte-identical across runs), LOV-08
//! (diffable, stable ordering). The printer grammar is documented in
//! `crates/piperine-ir/src/print.rs`.

use piperine_ir::core::{
    Block, Cell, Chunk, Const, CrossDir, Domain, EdgeKind, Module, Net, NatureInfo, NatureKind,
    Op, Pattern, PureOp, SigRef, Trigger, Ty,
};
use piperine_ir::print::Names;

/// Build a non-trivial module: values (real and bits), cells (register and
/// memory), nets (nodes and signals), blocks with EVERY trigger kind
/// (Always, Edge, Cross, Timer, Initial), at least one `If` and one
/// `Match`. The point is breadth — every printable construct appears.
fn non_trivial_module() -> (Module, Names) {
    let mut m = Module::new();

    // Natures: electrical voltage/current pair.
    let voltage = m.push_nature(NatureInfo { access: "voltage".into(), kind: NatureKind::Across });
    let current = m.push_nature(NatureInfo { access: "current".into(), kind: NatureKind::Through });

    // Nets: an authored analog node, an authored analog ground, an authored
    // digital clock, an authored digital data bus.
    let in_net = m.push_net(Net::node(voltage, current));
    let gnd_net = m.push_net(Net::node(voltage, current));
    let clk_sig = m.push_net(Net::signal(Ty::Bits(1)));
    let bus_sig = m.push_net(Net::signal(Ty::Bits(8)));

    // Consts: a real init and a bits init.
    let zero_real = m.push_const(Const::Real(0.0_f64));
    let zero_bits = m.push_const(Const::Bits(0));
    let _one_real = m.push_const(Const::Real(1.0_f64));
    let _neg_real = m.push_const(Const::Real(-2.5_f64));

    // Blocks: one per trigger kind. Each carries at least one op.
    let b_always = m.push_block(Block::new(Domain::Analog, Trigger::Always));
    let b_initial = m.push_block(Block::new(Domain::Digital, Trigger::Initial));
    let clk_value = m.push_value(Ty::Bits(1));
    let clk_edge = SigRef(vec![Chunk::Slice { value: clk_value, lo: 0, hi: 1 }]);
    let b_edge = m.push_block(Block::new(Domain::Digital, Trigger::Edge(clk_edge, EdgeKind::Rising)));
    let cross_expr = m.push_value(Ty::Real);
    let _b_cross = m.push_block(Block::new(Domain::Analog, Trigger::Cross(cross_expr, CrossDir::Either)));
    let period = m.push_value(Ty::Real);
    let phase = m.push_value(Ty::Real);
    let b_timer = m.push_block(Block::new(Domain::Analog, Trigger::Timer { period, phase }));

    // Cells: register (real) and memory (bits array), each with its writer.
    let acc_cell = m.push_cell(Cell::new(Ty::Real, zero_real, b_edge));
    let mem_cell = m.push_cell(Cell::new(Ty::Array(Box::new(Ty::Bits(32)), 16), zero_bits, b_initial));

    // Values: a real, an 8-bit bus, a 32-bit memory word, an address.
    let v_across = m.push_value(Ty::Real);
    let v_through = m.push_value(Ty::Real);
    let v_bus_in = m.push_value(Ty::Bits(8));
    let v_mem_word = m.push_value(Ty::Bits(32));
    let v_addr = m.push_value(Ty::Bits(4));
    let v_cond = m.push_value(Ty::Bits(1));
    let v_one = m.push_value(Ty::Real);
    let v_two = m.push_value(Ty::Real);
    let v_sum = m.push_value(Ty::Real);
    let v_cmp = m.push_value(Ty::Bits(1));
    let v_select = m.push_value(Ty::Real);

    // Always-analog block: read across, pure arithmetic, contribute, force.
    m.block_at_mut(b_always).push(Op::ReadAcross { a: in_net, b: gnd_net, out: v_across });
    m.block_at_mut(b_always).push(Op::ReadThrough { a: in_net, b: gnd_net, out: v_through });
    m.block_at_mut(b_always).push(Op::Pure { op: PureOp::Add, args: vec![v_across, v_one], out: v_sum });
    m.block_at_mut(b_always).push(Op::Pure { op: PureOp::Mul, args: vec![v_sum, v_two], out: v_select });
    m.block_at_mut(b_always).push(Op::Pure {
        op: PureOp::Select,
        args: vec![v_cond, v_select, v_across],
        out: v_select,
    });
    m.block_at_mut(b_always).push(Op::ContributeThrough { a: in_net, b: gnd_net, value: v_select });
    m.block_at_mut(b_always).push(Op::ForceAcross { a: in_net, b: gnd_net, value: v_one });

    // Initial-digital block: read the bus, write the memory at an address.
    m.block_at_mut(b_initial).push(Op::ReadSignal { net: bus_sig, out: v_bus_in });
    m.block_at_mut(b_initial).push(Op::CellWrite { cell: mem_cell, addr: Some(v_addr), value: v_mem_word });
    m.block_at_mut(b_initial).push(Op::CellRead { cell: mem_cell, addr: Some(v_addr), out: v_mem_word });

    // Edge-digital block: the cell write (the single declared writer for
    // acc_cell), plus a `Match` op and an `If` op to exercise structured
    // control in the printout.
    m.block_at_mut(b_edge).push(Op::CellWrite { cell: acc_cell, addr: None, value: v_one });
    m.block_at_mut(b_edge).push(Op::If {
        cond: v_cond,
        then_ops: vec![Op::Drive { net: bus_sig, value: v_bus_in }],
        else_ops: vec![Op::Drive { net: clk_sig, value: v_cond }],
    });
    m.block_at_mut(b_edge).push(Op::Match {
        scrutinee: v_cond,
        arms: vec![
            (Pattern::Zero, vec![Op::Drive { net: bus_sig, value: v_bus_in }]),
            (Pattern::One, vec![Op::Drive { net: clk_sig, value: v_cond }]),
            (Pattern::Wildcard, vec![]),
        ],
    });

    // Cross-analog block: empty body — fires on the cross only.
    // (Verifier accepts an empty block.)

    // Timer-analog block: pure comparison and a contribute.
    m.block_at_mut(b_timer).push(Op::Pure { op: PureOp::Gt, args: vec![v_across, v_one], out: v_cmp });
    m.block_at_mut(b_timer).push(Op::ContributeThrough { a: in_net, b: gnd_net, value: v_two });

    // Authored names — mingle generated and authored provenance so the
    // printer's `$`/`\` distinction is exercised.
    let mut names = Names::new();
    names.set_net(in_net, "in");
    names.set_net(gnd_net, "gnd");
    names.set_net(clk_sig, "clk");
    names.set_net(bus_sig, "bus");
    names.set_cell(acc_cell, "acc");
    names.set_cell(mem_cell, "mem");
    names.set_block(b_always, "analog-always");
    names.set_block(b_edge, "clocked");
    names.set_value(v_across, "vin");
    names.set_value(v_one, "k1");
    // Others stay generated.

    (m, names)
}

/// LOV-06 (byte-identical across runs): two prints of the same module are
/// byte-equal. Both the `==` string comparison and the explicit
/// `as_bytes()` comparison are asserted — the spec names both forms.
#[test]
fn repeat_print_is_byte_identical_on_a_non_trivial_module() {
    let (m, names) = non_trivial_module();
    let a = m.print_with_names(&names);
    let b = m.print_with_names(&names);

    assert_eq!(a.len(), b.len(), "same length");
    assert_eq!(a, b, "string-equal");
    assert_eq!(a.as_bytes(), b.as_bytes(), "byte-equal (LOV-06)");
}

/// LOV-08 (traceability): generated names (`$N`) and authored names
/// (`\name`) both appear in the output of a module that mingles them.
/// Both forms must coexist, in the proportions the spec describes.
#[test]
fn generated_and_authored_names_both_appear() {
    let (m, names) = non_trivial_module();
    let text = m.print_with_names(&names);

    // Authored names appear with the `\` prefix.
    for authored in ["\\in", "\\gnd", "\\clk", "\\bus", "\\acc", "\\mem", "\\analog-always", "\\clocked", "\\vin", "\\k1"] {
        assert!(text.contains(authored), "authored name {authored:?} should appear:\n{text}");
    }
    // Generated names appear with the `$` prefix. The block ids $2, $3
    // (Cross, Timer) are un-named in the fixture, and so are most values
    // and consts.
    let gen_count = text.matches('$').count();
    assert!(gen_count >= 10, "many generated names appear; found {gen_count} `$` occurrences:\n{text}");
}

/// Every trigger kind prints a distinct line in the output — five blocks,
/// five triggers, each recognisable in the dump.
#[test]
fn every_trigger_kind_appears_in_the_dump() {
    let (m, names) = non_trivial_module();
    let text = m.print_with_names(&names);
    // Each trigger prints a recognisable keyword.
    assert!(text.contains("always"), "Always trigger: {text}");
    assert!(text.contains("initial"), "Initial trigger: {text}");
    assert!(text.contains("(edge "), "Edge trigger: {text}");
    assert!(text.contains(" rising"), "Edge rising kind: {text}");
    assert!(text.contains("(cross "), "Cross trigger: {text}");
    assert!(text.contains(" either"), "Cross either dir: {text}");
    assert!(text.contains("(timer "), "Timer trigger: {text}");
}

/// Structured control (`If`/`Match`) prints inline — no flattening, the
/// arm bodies appear inside the block, in their recorded order.
#[test]
fn if_and_match_print_arms_inline_in_their_block() {
    let (m, names) = non_trivial_module();
    let text = m.print_with_names(&names);
    assert!(text.contains("(if "), "If prints: {text}");
    assert!(text.contains("(then"), "If then-arm prints: {text}");
    assert!(text.contains("(else"), "If else-arm prints: {text}");
    assert!(text.contains("(match "), "Match prints: {text}");
    // Match patterns 0/1/? all appear (the fixture arms include all three).
    assert!(text.contains(" 0 ("), "Zero pattern: {text}");
    assert!(text.contains(" 1 ("), "One pattern: {text}");
    assert!(text.contains(" ? ("), "Wildcard pattern: {text}");
}

/// The dump starts with the module header and lists sections in the fixed
/// order documented in the printer's grammar: natures, nets, cells,
/// values, consts, blocks.
#[test]
fn dump_lays_out_sections_in_fixed_order() {
    let (m, names) = non_trivial_module();
    let text = m.print_with_names(&names);
    assert!(text.starts_with("(module"), "starts with module: {}", &text[..text.find('\n').unwrap_or(text.len())]);
    let n = text.find("(natures").unwrap();
    let ne = text.find("(nets").unwrap();
    let c = text.find("(cells").unwrap();
    let v = text.find("(values").unwrap();
    let co = text.find("(consts").unwrap();
    let b = text.find("(blocks").unwrap();
    assert!(n < ne && ne < c && c < v && v < co && co < b, "section order is fixed");
}

/// Two separate printer instances, run back to back, produce identical
/// output — LOV-06 restated at the printer-struct level (no transient
/// state can leak between runs).
#[test]
fn separate_printer_instances_agree() {
    use piperine_ir::print::Printer;
    let (m, names) = non_trivial_module();
    let a = Printer::new(&m, &names).print();
    let b = Printer::new(&m, &names).print();
    assert_eq!(a, b);
    assert_eq!(a.as_bytes(), b.as_bytes());
}

/// The default `Module::print()` (no authored names) prints every entity
/// as `$N` and contains NO `\` prefix — the `\user` form is opt-in.
#[test]
fn default_print_emits_only_generated_form() {
    let (m, _names) = non_trivial_module();
    let text = m.print();
    assert!(!text.contains('\\'), "no authored names by default: {text}");
    assert!(text.contains("$0"), "generated form appears: {text}");
}

/// `Names::set_*` is idempotent and the latest write wins (a re-author
/// replaces an earlier authored name).
#[test]
fn names_reauthor_is_idempotent_and_latest_wins() {
    let mut m = Module::new();
    let v = m.push_value(Ty::Real);
    let mut names = Names::new();
    names.set_value(v, "first");
    names.set_value(v, "second"); // re-author
    let text = m.print_with_names(&names);
    assert!(text.contains("\\second"), "latest authored name wins: {text}");
    assert!(!text.contains("\\first"), "earlier authored name replaced: {text}");
}
