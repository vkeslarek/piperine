//! Bundles elaborated (feature `resolved-nets-bundles`, BN-02, BN-03; spec part II §7 rules 7, 14 and 15): a bundle port
//! or signal lowers to one carrier or node per leaf field named by its dotted path, a connection of a bundle joins each
//! field to the field of the same name, and the one-driver rule is judged field by field after the views.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_ir::{CarrierRole, Ir};
use support::Elab;

/// What a test reads of an elaborated design.
struct Read<'a> {
    ir: &'a Ir,
}

impl Read<'_> {
    /// The `(name, role)` of each carrier of the body `name`, in order.
    fn carriers(&self, name: &str) -> Vec<(String, CarrierRole)> {
        let body = self.ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body `{name}`:\n{}", self.ir.print_structural()));
        body.carriers().iter().map(|c| (c.name.clone(), c.role.clone())).collect()
    }

    /// The names of the nodes of the body `name`, in order.
    fn nodes(&self, name: &str) -> Vec<String> {
        let body = self.ir.bodies().find(|b| b.name() == name).unwrap_or_else(|| panic!("no body `{name}`"));
        body.nodes().iter().map(|n| n.name.clone()).collect()
    }
}

#[test]
fn a_bundle_port_lowers_to_one_carrier_per_leaf_field_by_path() {
    // §7 rule 14: a bundle has no semantics of its own, each field is the carrier or node it is, named by its path
    let src = "struct Stream { out valid: Bit, in ready: Bit, out data: Bit<u8> }\n\
               struct Pair { p: Electrical, n: Electrical }\n\
               struct Link { out up: Bit, pair: Pair }\n\
               module Top(tx: Stream, rx: flip Stream, in s: Stream, l: Link) {\n\
                   signal go: Bit = false;\n\
                   tx.valid = go; tx.data = 1; rx.ready = go; l.up = go;\n\
               }";
    let ir = Elab::ir(src, "Top");
    let read = Read { ir: &ir };
    let carriers = read.carriers("Top");
    let names: Vec<&str> = carriers.iter().map(|c| c.0.as_str()).collect();
    assert_eq!(names, ["tx.valid", "tx.ready", "tx.data", "rx.valid", "rx.ready", "rx.data", "s.valid", "s.ready", "s.data", "l.up", "go"], "ports in order, each bundle as its fields, then the signals");
    let role = |name: &str| carriers.iter().find(|c| c.0 == name).map(|c| c.1.clone()).unwrap_or_else(|| panic!("no carrier `{name}`"));
    // the views: the producer's `out` fields are `Out`, its `in` field `In`; the consumer's are the reverse; an observer reads all
    assert_eq!((role("tx.valid"), role("tx.ready")), (CarrierRole::Out, CarrierRole::In));
    assert_eq!((role("rx.valid"), role("rx.ready")), (CarrierRole::In, CarrierRole::Out));
    assert_eq!((role("s.valid"), role("s.ready"), role("s.data")), (CarrierRole::In, CarrierRole::In, CarrierRole::In));
    // a nested bundle's fields are named through it, and a continuous field is a node of the body
    assert_eq!(read.nodes("Top"), ["l.pair.p", "l.pair.n"]);
}

#[test]
fn a_bundle_signal_declares_every_carrier_and_drives_none_it_does_not_write() {
    // §7 rule 7: `signal a: Stream;` declares every carrier of the bundle; the declaring module drives none of its
    // one-driver fields unless it writes them
    let src = "struct Stream { out valid: Bit<Logic>, in ready: Bit<Logic> }\n\
               struct Bus { out v: Bit<Logic>, w: Electrical }\n\
               module Top(in clk: Bit) { signal a: Stream; signal b: Bus; }";
    let ir = Elab::ir(src, "Top");
    let read = Read { ir: &ir };
    let carriers = read.carriers("Top");
    assert_eq!(carriers.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["clk", "a.valid", "a.ready", "b.v"]);
    assert!(carriers[1..].iter().all(|c| c.1 == CarrierRole::Signal), "a signal's fields are signals of the body: {carriers:?}");
    assert_eq!(read.nodes("Top"), ["b.w"], "the continuous field of a signal is a node");
    let body = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    assert!(body.drives().is_empty(), "the declaring module drives no field it does not write");
}

#[test]
fn a_bundle_connection_binds_each_field_by_name() {
    // §7 rule 14: passing a bundle to a port connects each field to the field of the same name; the producer's and the
    // consumer's views of one `signal` join the same three carriers, and a continuous field joins the same nodes
    let src = "struct Stream { out valid: Bit, in ready: Bit, out data: Bit<u8> }\n\
               struct Diff { p: Electrical, n: Electrical }\n\
               module Prod(tx: Stream) { signal go: Bit = false; tx.valid = go; tx.data = 1; signal r: Bit; r = tx.ready; }\n\
               module Cons(rx: flip Stream) { rx.ready = true; signal v: Bit; v = rx.valid; }\n\
               module Amp(d: Diff) { }\n\
               module Top() { signal s: Stream; signal pair: Diff; let p = Prod(s); let c = Cons(s); let a = Amp(pair); }";
    let ir = Elab::ir(src, "Top");
    let read = Read { ir: &ir };
    let top = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    assert_eq!(read.carriers("Top").iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["s.valid", "s.ready", "s.data"]);
    assert_eq!(top.child_carriers(0), [Some(0), Some(1), Some(2)], "the producer's `tx.valid`, `tx.ready`, `tx.data` are the signal's");
    assert_eq!(top.child_carriers(1), [Some(0), Some(1), Some(2)], "so are the consumer's, by name, whatever the direction");
    assert_eq!(top.children()[2].nodes, [piperine_ir::Terminal::Node(0), piperine_ir::Terminal::Node(1)], "`d.p` and `d.n` are `pair.p` and `pair.n`");
    assert_eq!(read.nodes("Top"), ["pair.p", "pair.n"]);
}

#[test]
fn a_literal_binds_each_named_field_to_its_carrier() {
    // §7 rule 8: a literal names each field once and gives it a carrier or a node, in any order
    let src = "struct Cell { in wl: Bit, bl: Electrical, in sel: Bit }\n\
               module Leaf(c: Cell) { }\n\
               module Top(in w: Bit, in x: Bit, b: Electrical) { let l = Leaf(Cell { sel: x, bl: b, wl: w }); }";
    let ir = Elab::ir(src, "Top");
    let top = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    let at = |name: &str| top.carriers().iter().position(|c| c.name == name).map(|i| i as u32);
    assert_eq!(top.child_carriers(0), [at("w"), at("x")], "the child's `c.wl` and `c.sel` are the carriers the literal names, whatever its order");
    assert_eq!(top.children()[0].nodes, [piperine_ir::Terminal::Node(0)], "`c.bl` is `b`");
}

#[test]
fn two_producers_of_one_field_is_e0220_naming_its_path() {
    // §7 rule 15: the one-driver rule is checked field by field after the views: two modules that see one one-driver field
    // as `out` are `E0220`, and the diagnostic names the field's path
    let src = "struct Stream { out valid: Bit, in ready: Bit }\n\
               module Prod(tx: Stream) { signal go: Bit = false; tx.valid = go; }\n\
               module Cons(rx: flip Stream) { rx.ready = true; }\n\
               module Top() { signal s: Stream; let a = Prod(s); let b = Prod(s); let c = Cons(s); }";
    let found = Elab::run(src, "Top").expect_err("two producers of `s.valid`");
    assert_eq!(Elab::codes(&found), ["E0220"], "{found:?}");
    assert!(found[0].diagnostic.message.contains("s.valid"), "names the field's path: {}", found[0].diagnostic.message);
    // a producer and a consumer of the opposite views drive different fields: no overlap
    let ok = "struct Stream { out valid: Bit, in ready: Bit }\n\
              module Prod(tx: Stream) { signal go: Bit = false; tx.valid = go; }\n\
              module Cons(rx: flip Stream) { rx.ready = true; }\n\
              module Top() { signal s: Stream; let a = Prod(s); let c = Cons(s); }";
    Elab::ir(ok, "Top");
}

#[test]
fn an_observer_view_adds_no_drive() {
    // §7 rule 3, edge case: an observer reads every field and so adds no drive to any
    let src = "struct Stream { out valid: Bit, in ready: Bit }\n\
               module Prod(tx: Stream) { signal go: Bit = false; tx.valid = go; }\n\
               module Cons(rx: flip Stream) { rx.ready = true; }\n\
               module Watch(in s: Stream) { signal seen: Bit; seen = s.valid; }\n\
               module Top() { signal s: Stream; let a = Prod(s); let c = Cons(s); let w = Watch(s); let w2 = Watch(s); }";
    let ir = Elab::ir(src, "Top");
    let top = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    assert_eq!(top.child_carriers(2), [Some(0), Some(1)], "the observer is joined to both fields");
}

#[test]
fn a_loop_variable_index_folds_to_the_element_of_each_iteration() {
    // §7 rule 11, §8.2.23: each iteration of `for k in 0..3` joins the element `k` of the array: the producer and the consumer of
    // iteration `k` are the carriers `lanes[k].a` and `lanes[k].b`, in the order of the array
    let src = "struct Lane { out a: Bit, in b: Bit }\n\
               module Raise(tx: Lane) { tx.a = true; }\n\
               module Echo(rx: flip Lane) { rx.b = rx.a; }\n\
               module Top() { signal lanes: [Lane; 3]; for k in 0..3 { let r = Raise(lanes[k]); let e = Echo(lanes[k]); } }";
    let ir = Elab::ir(src, "Top");
    let top = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    let names: Vec<&str> = top.carriers().iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["lanes[0].a", "lanes[0].b", "lanes[1].a", "lanes[1].b", "lanes[2].a", "lanes[2].b"]);
    assert_eq!(top.children().len(), 6, "a producer and a consumer per iteration");
    // the producers `raise[0..3]` come first, the consumers `echo[0..3]` after them
    let children = |index: usize| top.child_carriers(index).to_vec();
    assert_eq!(children(0), [Some(0), Some(1)], "producer 0 is element 0");
    assert_eq!(children(1), [Some(2), Some(3)], "producer 1 is element 1");
    assert_eq!(children(2), [Some(4), Some(5)], "producer 2 is element 2");
    assert_eq!(children(4), [Some(2), Some(3)], "consumer 1 is element 1");
}

#[test]
fn a_loop_that_gives_one_element_twice_is_e0220_naming_it() {
    // the iterations all name the same element when the index does not use the variable: two producers of `lanes[1].a`
    let src = "struct Lane { out a: Bit, in b: Bit }\n\
               module Raise(tx: Lane) { tx.a = true; }\n\
               module Echo(rx: flip Lane) { rx.b = rx.a; }\n\
               module Top() { signal lanes: [Lane; 2]; let e = Echo(lanes[0]); let f = Echo(lanes[1]); for k in 0..2 { let r = Raise(lanes[1]); } let q = Raise(lanes[0]); }";
    let found = Elab::run(src, "Top").expect_err("two producers of `lanes[1].a`");
    assert!(Elab::codes(&found).contains(&"E0220".to_string()), "{found:?}");
    assert!(found.iter().any(|d| d.diagnostic.message.contains("lanes[1].a")), "names the element: {found:?}");
}

#[test]
fn an_array_field_of_carriers_binds_whole_and_is_written_by_element() {
    // §7 rule 1, rule 14: the field `d: [Bit; 2]` of a word is one array of carriers, joined whole to the child's field, whose
    // elements the child drives one by one
    let src = "struct Word { out d: [Bit; 2] }\n\
               module Fill(w: Word) { w.d[0] = true; w.d[1] = false; }\n\
               module Relay(w: Word) { let f = Fill(w); }\n\
               module Top() { signal word: Word; let r = Relay(word); }";
    let ir = Elab::ir(src, "Top");
    let read = Read { ir: &ir };
    assert_eq!(read.carriers("Top").iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["word.d"]);
    let top = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    assert_eq!(top.child_carriers(0), [Some(0)], "the relay's `w.d` is the signal's");
    let fill = ir.bodies().find(|b| b.name() == "Fill").expect("Fill");
    assert_eq!(fill.drives().len(), 2, "one drive per element of the field");
}

