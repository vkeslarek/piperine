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
    // §7 rule 11, §8.2.23: each iteration of `for k in 0..3` joins the element `k` of the array: the array of bundles is one array
    // of carriers per field (struct of arrays, design D12), `lanes.a` and `lanes.b` of three elements each, and the producer and
    // the consumer of iteration `k` are given the element `k` of both
    let src = "struct Lane { out a: Bit, in b: Bit }\n\
               module Raise(tx: Lane) { tx.a = true; }\n\
               module Echo(rx: flip Lane) { rx.b = rx.a; }\n\
               module Top() { signal lanes: [Lane; 3]; for k in 0..3 { let r = Raise(lanes[k]); let e = Echo(lanes[k]); } }";
    let ir = Elab::ir(src, "Top");
    let top = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    let carriers: Vec<(&str, Option<u64>)> = top.carriers().iter().map(|c| (c.name.as_str(), c.count)).collect();
    assert_eq!(carriers, [("lanes.a", Some(3)), ("lanes.b", Some(3))]);
    assert_eq!(top.children().len(), 6, "a producer and a consumer per iteration");
    // the producers `raise[0..3]` come first, the consumers `echo[0..3]` after them; each joins both fields at one element
    let element = |at: u64| Some(piperine_ir::CarrierWindow { first: at, length: None });
    for child in 0..6 {
        assert_eq!(top.child_carriers(child), [Some(0), Some(1)], "child {child} joins `lanes.a` and `lanes.b`");
    }
    assert_eq!(top.child_windows(0), [element(0), element(0)], "producer 0 is element 0");
    assert_eq!(top.child_windows(1), [element(1), element(1)], "producer 1 is element 1");
    assert_eq!(top.child_windows(2), [element(2), element(2)], "producer 2 is element 2");
    assert_eq!(top.child_windows(4), [element(1), element(1)], "consumer 1 is element 1");
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


#[test]
fn a_constant_parameter_in_an_array_length_of_a_field_is_the_argument_of_each_port() {
    // §7 rule 11, §1.2: `Word<2>` and `Word<3>` are two bundles, each field written again with the parameter replaced: the carrier
    // of `bits` holds as many elements as the port's argument says, whether it is written (`Word<3>`) or a parameter of the module
    let words = |name: &str, src: &str| {
        let ir = Elab::ir(src, "Top");
        let top = ir.bodies().find(|b| b.name() == "Top").expect("Top");
        top.carriers().iter().filter(|c| c.name == name).map(|c| c.count).collect::<Vec<_>>()
    };
    let fill = "struct Word<const W: usize> { out bits: [Bit; W] }\nmodule Fill<const W: usize>(w: Word<W>) { for k in 0..W { w.bits[k] = k == 0; } }\n";
    assert_eq!(words("narrow.bits", &format!("{fill}module Top() {{ signal narrow: Word<2>; let f = Fill::<2>(narrow); }}")), [Some(2)], "`Word<2>` has two bits");
    assert_eq!(words("wide.bits", &format!("{fill}module Top() {{ signal wide: Word<5>; let f = Fill::<5>(wide); }}")), [Some(5)], "`Word<5>` has five");
    let written = "struct Word<const W: usize> { out bits: [Bit; W + 1] }\nmodule Fill(w: Word<2>) { w.bits[0] = true; w.bits[1] = false; w.bits[2] = true; }\nmodule Top() { signal word: Word<2>; let f = Fill(word); }";
    assert_eq!(words("word.bits", written), [Some(3)], "`[Bit; W + 1]` with `W = 2` has three");
}

#[test]
fn an_array_of_bundles_of_a_generic_length_has_the_length_of_each_instance() {
    // §7 rule 11: `[Lane; N]` placed with `N = 2` and with `N = 3` is one array of carriers per field of as many elements in each
    // instance, and the generation loop over `k in 0..N` gives the element `k` to the iteration's producer
    let lane = "struct Lane { out a: Bit, in b: Bit }\nmodule Raise(tx: Lane) { tx.a = true; }\nmodule Echo(rx: flip Lane) { rx.b = rx.a; }\n\
                module Bank<const N: usize>(ts: [Lane; N]) { for k in 0..N { let r = Raise(ts[k]); } }\n";
    for length in [2u64, 3] {
        let src = format!("{lane}module Top() {{ signal lanes: [Lane; {length}]; let b = Bank::<{length}>(lanes); for k in 0..{length} {{ let e = Echo(lanes[k]); }} }}");
        let ir = Elab::ir(&src, "Top");
        let top = ir.bodies().find(|b| b.name() == "Top").expect("Top");
        let carriers: Vec<(&str, Option<u64>)> = top.carriers().iter().map(|c| (c.name.as_str(), c.count)).collect();
        assert_eq!(carriers, [("lanes.a", Some(length)), ("lanes.b", Some(length))], "N = {length}");
        let bank = ir.bodies().find(|b| b.carriers().iter().any(|c| c.name == "ts.a")).expect("the body of `Bank`");
        let fields: Vec<(&str, Option<u64>)> = bank.carriers().iter().map(|c| (c.name.as_str(), c.count)).collect();
        assert_eq!(fields, [("ts.a", Some(length)), ("ts.b", Some(length))], "the port of `Bank` with N = {length}");
        assert_eq!(bank.children().len() as u64, length, "one producer per element");
        for k in 0..length {
            let element = Some(piperine_ir::CarrierWindow { first: k, length: None });
            assert_eq!(bank.child_windows(k as usize), [element, element], "producer {k} is element {k} (N = {length})");
        }
    }
}

#[test]
fn a_computed_length_of_an_array_of_bundles_is_folded_per_instance() {
    // §7 rule 11, §1.2: `[Lane; N + 1]` with `N = 2` is three elements
    let src = "struct Lane { out a: Bit, in b: Bit }\nmodule Raise(tx: Lane) { tx.a = true; }\nmodule Echo(rx: flip Lane) { rx.b = rx.a; }\n\
               module Wide<const N: usize>(ws: [Lane; N + 1]) { for k in 0..N + 1 { let r = Raise(ws[k]); } }\n\
               module Top() { signal lanes: [Lane; 3]; let w = Wide::<2>(lanes); for k in 0..3 { let e = Echo(lanes[k]); } }";
    let ir = Elab::ir(src, "Top");
    let wide = ir.bodies().find(|b| b.carriers().iter().any(|c| c.name == "ws.a")).expect("the body of `Wide`");
    assert_eq!(wide.carriers().iter().map(|c| c.count).collect::<Vec<_>>(), [Some(3), Some(3)]);
    assert_eq!(wide.children().len(), 3);
}

#[test]
fn a_second_producer_of_one_element_in_a_generic_instance_is_e0220_naming_it() {
    // §7 rule 15 judged per instance: the loop over `k in 0..N` already produces `tx[1]`, a second producer of it is `E0220`
    // naming the field of that element, once the length is known
    let src = "struct Lane { out a: Bit, in b: Bit }\nmodule Raise(tx: Lane) { tx.a = true; }\nmodule Echo(rx: flip Lane) { rx.b = rx.a; }\n\
               module Bank<const N: usize>(tx: [Lane; N]) { for k in 0..N { let r = Raise(tx[k]); } let again = Raise(tx[1]); }\n\
               module Top() { signal lanes: [Lane; 3]; let b = Bank::<3>(lanes); for k in 0..3 { let e = Echo(lanes[k]); } }";
    let found = Elab::run(src, "Top").expect_err("two producers of `tx[1].a`");
    assert!(Elab::codes(&found).contains(&"E0220".to_string()), "{found:?}");
    assert!(found.iter().any(|d| d.diagnostic.message.contains("`tx[1].a`")), "names the element: {found:?}");
    assert!(!found.iter().any(|d| d.diagnostic.message.contains("tx[0]") || d.diagnostic.message.contains("tx[2]")), "only element 1 has two: {found:?}");
}

#[test]
fn an_element_no_iteration_drives_in_a_generic_instance_is_e0187_naming_it() {
    // §3.3.4 judged per instance: the loop over `k in 0..N - 1` produces every element but the last, whose field `a` has no
    // value (a `Bit` has no `Default`): with `N = 3`, `E0187` names `tx[2].a` and no other element
    let src = "struct Lane { out a: Bit, in b: Bit }\nmodule Raise(tx: Lane) { tx.a = true; }\nmodule Echo(rx: flip Lane) { rx.b = rx.a; }\n\
               module Bank<const N: usize>(tx: [Lane; N]) { for k in 0..N - 1 { let r = Raise(tx[k]); } }\n\
               module Top() { signal lanes: [Lane; 3]; let b = Bank::<3>(lanes); for k in 0..3 { let e = Echo(lanes[k]); } }";
    let found = Elab::run(src, "Top").expect_err("the last element has no producer");
    let missing: Vec<&str> = found.iter().filter(|d| d.diagnostic.code == "E0187").map(|d| d.diagnostic.message.as_str()).collect();
    assert_eq!(missing.len(), 1, "{found:?}");
    assert!(missing[0].starts_with("`tx[2].a` in `"), "{missing:?}");
}

#[test]
fn a_node_field_of_an_array_of_bundles_is_one_array_of_nodes() {
    // §7 rule 12, struct of arrays (design D12): the field `p` of `d: [Diff; 2]` is the array of nodes `d.p`, and the element
    // `d[1]` given to a port is the node `1` of each field (the host reads it as `d[1].p`, `piperine-api`)
    let src = "struct Diff { p: Electrical, n: Electrical }\n\
               module Load(d: Diff) { }\nanalog for Load { i(d.p, d.n) +> v(d.p, d.n) / 1000.0; }\n\
               module Top() { signal d: [Diff; 2]; for k in 0..2 { let l = Load(d[k]); } }\nanalog for Top { v(d[0].p, d[0].n) +> 1.0; v(d[1].p, d[1].n) +> 2.0; }";
    let ir = Elab::ir(src, "Top");
    let read = Read { ir: &ir };
    assert_eq!(read.nodes("Top"), ["d.p[0]", "d.p[1]", "d.n[0]", "d.n[1]"]);
    let top = ir.bodies().find(|b| b.name() == "Top").expect("Top");
    assert_eq!(top.children()[1].nodes, [piperine_ir::Terminal::Node(1), piperine_ir::Terminal::Node(3)], "load 1 is `d[1].p` and `d[1].n`");
}

#[test]
fn a_template_placed_through_a_bound_has_the_bundle_in_the_bounds_view() {
    // §8.4.5, §7 rules 3 to 5: through `Fn(flip Pipe)` the cell is a consumer: it reads `q` and drives `d`, and the bank gives it `s`
    // and takes `r`; through `Fn(Pipe)` the cell is a producer, the roles reversed. The leaves of the cell are named by path.
    let src = "struct Pipe { out q: Bit<u8>, in d: Bit<u8> }\n\
               module Drv(p: Pipe) { p.q = p.d + 1; }\n\
               module Snk(p: flip Pipe) { p.d = p.q + 2; }\n\
               module Up<C: Fn(Pipe)>() { cell: C; signal s: Bit<u8>; signal r: Bit<u8> = 0; let one = cell(Pipe { q: s, d: r }); }\n\
               module Down<C: Fn(flip Pipe)>() { cell: C; signal s: Bit<u8> = 1; signal r: Bit<u8>; let one = cell(Pipe { q: s, d: r }); }\n\
               module Top() { let up = Up { cell: Drv {} }(); let down = Down { cell: Snk {} }(); }";
    let ir = Elab::ir(src, "Top");
    let read = Read { ir: &ir };
    let roles = |body: &str| read.carriers(body).into_iter().filter(|c| c.0.starts_with("p.")).collect::<Vec<_>>();
    assert_eq!(roles("Drv"), [("p.q".to_string(), CarrierRole::Out), ("p.d".to_string(), CarrierRole::In)]);
    assert_eq!(roles("Snk"), [("p.q".to_string(), CarrierRole::In), ("p.d".to_string(), CarrierRole::Out)]);
}

#[test]
fn a_field_default_is_the_initial_value_of_the_leaf_a_view_makes_out() {
    // §7 rule 10: `out q: Bit<u8> = 9` is the initial value of `q` of the port at a producer and of a signal, not of `q` at a consumer
    // (`flip` makes it an input, which the parent drives); a field with none starts at its `Default`
    let pipe = "struct Pipe { out q: Bit<u8> = 9, in d: Bit<Logic>, out r: Bit<Logic> }\nmodule Snk(p: flip Pipe) { }\n";
    let init = |ir: &Ir, body: &str, name: &str| ir.bodies().find(|b| b.name() == body).expect("a body").carriers().iter().find(|c| c.name == name).unwrap_or_else(|| panic!("no carrier `{name}`")).init;
    let ir = Elab::ir(&format!("{pipe}module Top(tx: Pipe) {{ }}"), "Top");
    assert_eq!(init(&ir, "Top", "tx.q"), Some(piperine_ir::Constant::Int(9)), "the producer's port leaf starts at the field's value");
    assert_eq!(init(&ir, "Top", "tx.r").map(|c| c != piperine_ir::Constant::Int(9)), Some(true), "no default: the `Default` of the type");
    let ir = Elab::ir(&format!("{pipe}module Top() {{ signal s: Pipe; signal t: Bit<Logic>; let k = Snk(s); }}"), "Top");
    assert_eq!(init(&ir, "Top", "s.q"), Some(piperine_ir::Constant::Int(9)), "a signal of the bundle starts there too");
    assert_eq!(init(&ir, "Snk", "p.q"), None, "an input leaf takes no initial value: its parent gives it");
}

#[test]
fn an_in_field_default_is_the_initial_value_of_the_consumers_leaf() {
    // §7 rule 10: `in d: Bit<u8> = 4` starts the carrier on the side that drives `d`: the `flip` consumer's port leaf, and a signal
    // of the bundle; a producer's `d` is an input its parent drives, so it takes none
    let pipe = "struct Pipe { out q: Bit<u8> = 9, in d: Bit<u8> = 4 }\nmodule Snk(p: flip Pipe) { }\n";
    let init = |ir: &Ir, body: &str, name: &str| ir.bodies().find(|b| b.name() == body).expect("a body").carriers().iter().find(|c| c.name == name).unwrap_or_else(|| panic!("no carrier `{name}`")).init;
    let ir = Elab::ir(&format!("{pipe}module Top() {{ signal s: Pipe; let k = Snk(s); }}"), "Top");
    assert_eq!(init(&ir, "Snk", "p.d"), Some(piperine_ir::Constant::Int(4)), "the consumer's leaf starts at the `in` field's value");
    assert_eq!(init(&ir, "Snk", "p.q"), None, "the consumer's `q` is an input");
    assert_eq!(init(&ir, "Top", "s.d"), Some(piperine_ir::Constant::Int(4)), "a signal of the bundle starts there too");
    let ir = Elab::ir(&format!("{pipe}module Top(tx: Pipe) {{ }}"), "Top");
    assert_eq!(init(&ir, "Top", "tx.d"), None, "the producer's `d` is an input");
}
