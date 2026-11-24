//! Natures of every kind in `piperine_lang::elab` (feature `resolved-nets`, RN-08): the potential
//! drivers of a signal-flow node (§6 rule 2 and its Resolution), `E0242` naming the node and each
//! driver, and a node any of whose natures has `flow` being conservative and not counted. Sources
//! are written over the embedded standard library.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use support::Elab;

/// A module that drives its `VSignal` port with a potential, and one that reads it.
const BLOCKS: &str = "module Source(o: VSignal) { }\nanalog for Source { v(o) +> 1.0; }\n\
                      module Reader(a: VSignal) { }\nanalog for Reader { let x = v(a); }\n\
                      module Load(p: Electrical) { }\nanalog for Load { let x = v(p); }\n";

#[test]
fn a_signal_flow_node_with_no_or_two_drivers_is_e0242() {
    // §6 Resolution of rule 2: a node every nature of which lacks `flow` takes a potential
    // contribution to `(n, gnd)` from exactly one instance. None, or two, is `E0242` naming the
    // node and each driver; one is fine.
    let one = format!("{BLOCKS}module Top() {{ signal s: VSignal; let a = Source(s); let r = Reader(s); }}");
    Elab::ir(&one, "Top");

    let none = format!("{BLOCKS}module Top() {{ signal s: VSignal; let r = Reader(s); }}");
    let diagnostics = Elab::run(&none, "Top").expect_err("no driver");
    assert_eq!(Elab::codes(&diagnostics), ["E0242"]);
    let message = &diagnostics[0].diagnostic.message;
    assert!(message.contains("`Top.s`") && message.contains("0 potential driver") && message.contains("none"), "{message}");

    let two = format!("{BLOCKS}module Top() {{ signal s: VSignal; let a = Source(s); let b = Source(s); }}");
    let diagnostics = Elab::run(&two, "Top").expect_err("two drivers");
    assert_eq!(Elab::codes(&diagnostics), ["E0242"]);
    let message = &diagnostics[0].diagnostic.message;
    assert!(message.contains("`Top.s`") && message.contains("2 potential driver") && message.contains("Top.a") && message.contains("Top.b"), "{message}");
}

#[test]
fn a_node_meeting_a_nature_with_flow_is_not_counted() {
    // §4 rule 13 Resolution, §6 Resolution of rule 2: a node where an `Electrical` port meets a
    // `VSignal` port is conservative; it is not counted, so no driver and two drivers are both
    // accepted here.
    let none = format!("{BLOCKS}module Top() {{ signal s: VSignal; let l = Load(s); let r = Reader(s); }}");
    Elab::ir(&none, "Top");
    let two = format!("{BLOCKS}module Top() {{ signal s: VSignal; let l = Load(s); let a = Source(s); let b = Source(s); }}");
    Elab::ir(&two, "Top");
}

/// A project nature resolved by summing its drives, and a driver and a reader of it.
const TOTAL: &str = "nature Total { value: f64 }\nimpl Resolve<f64> for Total { fn resolve<const N: usize>(d: [f64; N]) -> f64 { d.iter().sum() } }\n\
                     module Driver(w: Total) { w = 2.0; }\nmodule Reader(w: Total) { }\n";

#[test]
fn a_resolved_port_lowers_to_a_resolved_carrier_and_its_drive() {
    // §5.2: a resolved port or signal is a carrier of role `Resolved { driven }`, of the nature's `value` type; the
    // body's definition of it is its drive, of the driven type `X`.
    let ir = Elab::run(&format!("{TOTAL}module Top() {{ signal n: Total; let d = Driver(n); let r = Reader(n); }}"), "Top").unwrap_or_else(|d| panic!("{d:?}"));
    let text = ir.print_structural();
    assert!(text.contains("carrier w: f64 resolved f64"), "{text}");
    assert!(text.contains("carrier n: f64 resolved f64"), "{text}");
    let driver = text.split("body Driver").nth(1).expect("the driver body");
    assert!(driver.contains("drive w"), "{driver}");
}

#[test]
fn others_lowers_to_the_bodys_others_read() {
    // §5.2.7: `s.others()` is the read of what every other drive resolves to, of the value type.
    let source = format!("{TOTAL}module Echo(w: Total, in clk: Bit) {{ signal seen: Bit<f64> = 0.0; }}\ndigital for Echo {{ on clk.rise() {{ seen <- w.others(); }} }}\nmodule Top() {{ signal n: Total; signal c: Bit = false; let e = Echo(n, c); }}");
    let ir = Elab::run(&source, "Top").unwrap_or_else(|d| panic!("{d:?}"));
    let text = ir.print_structural();
    assert!(text.contains("others(w)") || text.contains("others("), "{text}");
}

#[test]
fn drives_are_ordered_by_instance_path_segment_by_segment() {
    // §4 rule 2 (Resolution): the drives of a net are in the order of their holders' instance paths, segment by
    // segment, names by code point, whatever the order the instances are written in.
    let ir = Elab::ir(&format!("{TOTAL}module Top() {{ signal n: Total; let c = Driver(n); let a = Driver(n); let b = Driver(n); }}"), "Top");
    let text = ir.print_structural();
    assert!(text.contains("held by (Top.a#0, Top.b#0, Top.c#0)"), "{text}");
}

#[test]
fn resolve_is_instantiated_once_per_drive_count() {
    // §4 rule 8: a net of `N` drives calls `resolve::<k>` for each `k` in `0..=N`, one IR function each, and a
    // second net of the same nature and count shares them (one resolver body per count).
    let source = format!("{TOTAL}module Top() {{ signal n: Total; signal m: Total; let a = Driver(n); let b = Driver(n); let c = Driver(m); let d = Driver(m); }}");
    let ir = Elab::ir(&source, "Top");
    let text = ir.print_structural();
    assert!(text.contains("resolve [\"Total.resolve::<f64>::<0>\", \"Total.resolve::<f64>::<1>\", \"Total.resolve::<f64>::<2>\"]"), "{text}");
    assert_eq!(text.matches("body \"Total<f64>$resolve~2\"").count(), 1, "one resolver body for both nets:\n{text}");
    assert!(text.contains("resolved by Top.n$resolve") && text.contains("resolved by Top.m$resolve"), "{text}");
}

#[test]
fn others_depends_on_the_other_drives_only() {
    // §5.2.7: the others net of a holder is the `resolve` of every drive but its own: the resolver body of a net with a
    // reader has an `others` output for it, and a holder that reads nothing gets none.
    let echo = "module Echo(w: Total, in clk: Bit) { signal seen: Bit<f64> = 0.0; w = 1.0; }\ndigital for Echo { on clk.rise() { seen <- w.others(); } }\n";
    let source = format!("{TOTAL}{echo}module Top() {{ signal n: Total; signal c: Bit = false; let a = Driver(n); let e = Echo(n, c); }}");
    let ir = Elab::ir(&source, "Top");
    let text = ir.print_structural();
    assert!(text.contains("others (1: Top.n$others1)"), "only the reader (drive 1, `Top.e`) has an others net:\n{text}");
    assert!(text.contains("except"), "the resolver body resolves all but the reader's drive:\n{text}");
}

#[test]
fn two_driven_types_on_one_net_are_e0225() {
    // §5.2.15: every drive of a net holds one type; a `bool` from one module and a `u8` from another into the
    // same net is `E0225` naming both holders and both types.
    let natures = "nature Counted { value: u8 }\nimpl Resolve<bool, u8> for Counted { fn resolve<const N: usize>(d: [bool; N]) -> u8 { 0 } }\nimpl Resolve<u8> for Counted { fn resolve<const N: usize>(d: [u8; N]) -> u8 { 0 } }\n\
                    module Flag(c: Counted) { c = true; }\nmodule Byte(c: Counted) { c = 7u8; }\n";
    let source = format!("{natures}module Top() {{ signal n: Counted; let a = Flag(n); let b = Byte(n); }}");
    let diagnostics = Elab::run(&source, "Top").expect_err("two driven types");
    assert_eq!(Elab::codes(&diagnostics), ["E0225"]);
    let message = &diagnostics[0].diagnostic.message;
    assert!(message.contains("Top.a") && message.contains("Top.b") && message.contains("bool") && message.contains("u8"), "{message}");
    let agreeing = format!("{natures}module Top() {{ signal n: Counted; let a = Flag(n); let b = Flag(n); }}");
    Elab::ir(&agreeing, "Top");
}

#[test]
fn an_initial_value_is_the_declaring_modules_first_drive_commit() {
    // §5.2.3 and §5.2.12: `= v` on a resolved port or signal is the declaring module's first drive of it, at `(0, 0)`,
    // of the driven type; a module that does not drive the carrier needs none; a drive with none starts at the
    // `Default` of its type (`Option<u8>` into a nature of `impl Resolve<Option<u8>, u8>`).
    let source = format!("{TOTAL}module Keeper(w: Total = 3.0) {{ }}\nmodule Top() {{ signal n: Total = 1.0; let k = Keeper(n); }}");
    let text = Elab::ir(&source, "Top").print_structural();
    assert!(text.contains("carrier w: f64 resolved f64 init 3.0") && text.contains("carrier n: f64 resolved f64 init 1.0"), "{text}");
    let silent = Elab::ir(&format!("{TOTAL}module Top() {{ signal n: Total; let r = Reader(n); }}"), "Top").print_structural();
    assert!(!silent.contains("resolved f64 init"), "a carrier nobody drives has no initial value:\n{silent}");
    let defaulted = "#[derive(Repr, PartialEq)]\nstruct Cell { v: u8 }\nimpl Default for Cell { fn default() -> Cell { Cell { v: 7 } } }\n\
                     nature Level { value: u8 }\nimpl Resolve<Cell, u8> for Level { fn resolve<const N: usize>(d: [Cell; N]) -> u8 { 0 } }\n\
                     module Top(in clk: Bit) { signal l: Level; }\ndigital for Top { on clk.rise() { l <- Cell { v: 1 }; } }";
    let text = Elab::ir(defaulted, "Top").print_structural();
    assert!(text.contains("carrier l: u8 resolved t0 init agg0") && text.contains("agg0: t0 = Cell(7)"), "a drive of a type with `Default` starts at it:\n{text}");
}

#[test]
fn a_resolved_drive_without_default_or_initial_value_is_e0187() {
    // §5.2.12: a drive has a value from `(0, 0)`: a type with no `Default` needs `= v`, or an undelayed definition.
    let commit = format!("{TOTAL}module Top(in clk: Bit) {{ signal n: Total; }}\ndigital for Top {{ on clk.rise() {{ n <- 1.0; }} }}");
    assert_eq!(Elab::codes(&Elab::run(&commit, "Top").expect_err("no default")), ["E0187"]);
    let delayed = format!("{TOTAL}module Top() {{ signal n: Total; n = 1.0 after 1n; }}");
    assert_eq!(Elab::codes(&Elab::run(&delayed, "Top").expect_err("no default")), ["E0187"]);
    Elab::ir(&format!("{TOTAL}module Top() {{ signal n: Total; n = 1.0; }}"), "Top");
    Elab::ir(&format!("{TOTAL}module Top(in clk: Bit) {{ signal n: Total = 0.0; }}\ndigital for Top {{ on clk.rise() {{ n <- 1.0; }} }}"), "Top");
}

#[test]
fn a_loop_through_a_one_driver_carrier_across_instances_is_e0339() {
    // §8.2.3: undelayed definitions that depend on each other through a one-driver carrier are `E0339`, the
    // loop closed by the placed tree: each inverter alone is fine, the ring of two is not. A delay breaks it.
    let inverter = "module Not(in a: Bit, out y: Bit) { y = !a; }\n";
    let ring = format!("{inverter}module Top() {{ signal p: Bit; signal q: Bit; let x = Not(p, q); let y = Not(q, p); }}");
    let diagnostics = Elab::run(&ring, "Top").expect_err("a loop");
    assert_eq!(Elab::codes(&diagnostics), ["E0339"], "{diagnostics:?}");
    let message = &diagnostics[0].diagnostic.message;
    assert!(message.contains("Top.x.y") && message.contains("Top.y.y"), "{message}");
    let chain = format!("{inverter}module Top(in p: Bit) {{ signal q: Bit; signal r: Bit; let x = Not(p, q); let y = Not(q, r); }}");
    Elab::ir(&chain, "Top");
    let delayed = "module Not(in a: Bit, out y: Bit = false) { y = !a after 1n; }\nmodule Top() { signal p: Bit; signal q: Bit; let x = Not(p, q); let y = Not(q, p); }";
    Elab::ir(delayed, "Top");
}

#[test]
fn a_loop_of_resolved_drives_across_instances_is_accepted() {
    // §8.2.3: a loop that passes through resolved nets only is left to relaxation: two passes over one net of
    // `Total` that each drive what the other drives (through `others()`) elaborate.
    let source = format!("{TOTAL}module Pass(w: Total) {{ w = w.others() + 1.0; }}\nmodule Top() {{ signal n: Total; let a = Pass(n); let b = Pass(n); }}");
    Elab::ir(&source, "Top");
}
