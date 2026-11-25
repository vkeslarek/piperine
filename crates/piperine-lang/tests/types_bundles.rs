//! Bundle types and views of `piperine_lang::types` (feature `resolved-nets-bundles`, BN-02 to BN-04; spec
//! part II §7): a bundle port is not a value, `flip` and the observer view set which one-driver fields a module
//! drives (§7 rules 3 to 5), driving an input field is `E0222`, and a field is a carrier with the rules of §5.
//! Sources are written over the embedded standard library.

use piperine_lang::pom::{Design, DesignDiagnostic};
use piperine_lang::types::Types;

/// Builds and checks the test designs.
struct Bundles;

impl Bundles {
    /// The `Stream` the tests share: a producer's `valid` and `data`, a consumer's `ready`.
    const STREAM: &'static str = "struct Stream { out valid: Bit, in ready: Bit, out data: Bit<u8> }\n";

    /// A bundle whose field holds a type with a `Default` (`Logic`), so it may be committed in a reaction with no initial value (§5.2.8).
    const LEVEL: &'static str = "struct Latch { out flag: Bit<Logic>, in ack: Bit }\n";

    /// A design of `src` that parses and resolves without a diagnostic.
    fn design(src: &str) -> Design {
        let design = Design::builder().source("top.msl", src).build();
        assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", design.diagnostics().iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
        design
    }

    /// Checks `src` from `Top`, which must raise nothing.
    fn accepted(src: &str) {
        let design = Bundles::design(src);
        if let Err(found) = Types::check(&design, "Top") {
            panic!("expected a clean check: {:?}\n{src}", Bundles::summary(&found));
        }
    }

    /// Checks `src` from `Top`, which must raise diagnostics: their codes and the source text of their spans.
    fn refused(src: &str) -> Vec<(String, String)> {
        let design = Bundles::design(src);
        let found = Types::check(&design, "Top").expect_err("expected diagnostics");
        found.iter().map(|d| (d.diagnostic.code.to_string(), src[d.diagnostic.span().range()].to_string())).collect()
    }

    /// The message of each diagnostic of `src` from `Top`.
    fn messages(src: &str) -> Vec<String> {
        let design = Bundles::design(src);
        Types::check(&design, "Top").err().map(|found| found.iter().map(|d| d.diagnostic.message.clone()).collect()).unwrap_or_default()
    }

    /// The code and message of each diagnostic.
    fn summary(found: &[DesignDiagnostic]) -> Vec<(String, String)> {
        found.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect()
    }
}

#[test]
fn a_producer_drives_out_fields_and_reads_in_fields() {
    // §7 rule 3: `tx: Stream` drives the `out` fields and reads the `in` fields
    Bundles::accepted(&format!("{}module Top(tx: Stream) {{ signal go: Bit = false; signal seen: Bit; seen = tx.ready; tx.valid = go; tx.data = 7; }}", Bundles::STREAM));
}

#[test]
fn flip_swaps_every_one_driver_direction_nested_included() {
    // §7 rule 3: `rx: flip Stream` drives what the producer reads, and reads what it drives
    Bundles::accepted(&format!("{}module Top(rx: flip Stream) {{ signal seen: Bit; seen = rx.valid; rx.ready = seen; }}", Bundles::STREAM));
    let refused = Bundles::refused(&format!("{}module Top(rx: flip Stream) {{ rx.ready = true; rx.valid = true; }}", Bundles::STREAM));
    assert_eq!(refused, [("E0222".to_string(), "rx.valid".to_string())], "the consumer reads `valid`");
    // §7 rule 5: the views apply to a nested bundle, a plain field and a `flip` one
    let nested = "struct Inner { out x: Bit }\nstruct Outer { inner: Inner, back: flip Inner, k: Electrical }\n";
    Bundles::accepted(&format!("{nested}module Top(o: Outer) {{ o.inner.x = true; signal s: Bit; s = o.back.x; }}"));
    let refused = Bundles::refused(&format!("{nested}module Top(o: Outer) {{ o.inner.x = true; o.back.x = true; }}"));
    assert_eq!(refused, [("E0222".to_string(), "o.back.x".to_string())], "a flipped nested bundle is read by the producer");
    Bundles::accepted(&format!("{nested}module Top(o: flip Outer) {{ o.back.x = true; signal s: Bit; s = o.inner.x; }}"));
    // §7 rule 4: a field without a direction is the same in every view
    Bundles::accepted(&format!("{nested}module Top(p: Electrical, o: flip Outer) {{ o.back.x = true; }}\nanalog for Top {{ i(o.k) +> v(p); }}"));
}

#[test]
fn an_observer_reads_every_one_driver_field() {
    // §7 rule 3: `in s: Stream` makes every one-driver field an input
    Bundles::accepted(&format!("{}module Top(in s: Stream) {{ signal a: Bit; signal b: Bit; a = s.valid; b = s.ready; }}", Bundles::STREAM));
    let refused = Bundles::refused(&format!("{}module Top(in s: Stream) {{ s.valid = true; }}", Bundles::STREAM));
    assert_eq!(refused, [("E0222".to_string(), "s.valid".to_string())], "the observer drives nothing");
}

#[test]
fn driving_an_input_field_is_e0222() {
    // §7 rule 3, §5.1: a module that drives a field its view makes an input
    let refused = Bundles::refused(&format!("{}module Top(tx: Stream) {{ tx.valid = true; tx.data = 1; tx.ready = true; }}", Bundles::STREAM));
    assert_eq!(refused, [("E0222".to_string(), "tx.ready".to_string())]);
    // and in a reaction, a commit
    let refused = Bundles::refused(&format!("{}module Top(in clk: Bit, tx: Stream) {{ tx.valid = true; tx.data = 1; }}\ndigital for Top {{ on clk.rise() {{ tx.ready <- true; }} }}", Bundles::STREAM));
    assert_eq!(refused, [("E0222".to_string(), "tx.ready".to_string())]);
}

#[test]
fn a_bundle_port_is_not_a_value_input() {
    // §7 rule 9: a bundle is no value; its port is neither typed as a value input nor refused as a struct item used as a type
    Bundles::accepted(&format!("{}module Top(in s: Stream, tx: Stream, rx: flip Stream) {{ tx.valid = s.valid; tx.data = s.data; rx.ready = s.ready; }}", Bundles::STREAM));
}

#[test]
fn a_bundle_field_accepts_carrier_writes_its_view_allows() {
    // §7 rule 12, §5: a commit and a definition of a field its view lets the module drive
    Bundles::accepted(&format!(
        "{}{}module Top(in clk: Bit, tx: Stream, l: Latch) {{ tx.data = 3; signal full: Bit = false; tx.valid = full; }}\ndigital for Top {{ on clk.rise() {{ full <- true; l.flag <- Logic::default(); }} }}",
        Bundles::STREAM, Bundles::LEVEL
    ));
}

#[test]
fn a_carrier_method_on_a_field_applies_to_that_field() {
    // §7 rule 12: `rx.valid.rise()` is the method of `Bit` on that field
    Bundles::accepted(&format!(
        "{}module Top(rx: flip Stream) {{ rx.ready = true; }}\ndigital for Top {{ on rx.valid.rise() {{ }} }}",
        Bundles::STREAM
    ));
}

#[test]
fn a_literal_field_of_another_nature_is_e0263() {
    // §7 rule 8: each field of a literal is a carrier of the field's nature (or a node of a compatible one)
    let cell = "struct Cell { in wl: Bit, bl: Electrical }\nmodule Leaf(c: Cell) { }\n";
    Bundles::accepted(&format!("{cell}module Top(in w: Bit, b: Electrical) {{ let c = Leaf(Cell {{ wl: w, bl: b }}); }}"));
    for (fields, at) in [("wl: b, bl: b", "wl: b"), ("wl: w, bl: w", "bl: w"), ("wl: wide, bl: b", "wl: wide")] {
        let refused = Bundles::refused(&format!("{cell}module Top(in w: Bit, in wide: Bit<u8>, b: Electrical) {{ let c = Leaf(Cell {{ {fields} }}); }}"));
        assert_eq!(refused, [("E0263".to_string(), at.to_string())], "{fields}");
    }
}

#[test]
fn a_literal_outside_a_connection_or_a_bundle_operand_is_e0264() {
    // §7 rules 8 and 9: a literal is written only as a connection, and a bundle is no operand, argument or value
    let cell = "struct Cell { in wl: Bit, bl: Electrical }\n";
    let refused = Bundles::refused(&format!("{cell}module Top(in w: Bit, b: Electrical) {{ let c = Cell {{ wl: w, bl: b }}; }}"));
    assert_eq!(refused, [("E0264".to_string(), "Cell { wl: w, bl: b }".to_string())]);
    let refused = Bundles::refused(&format!("{}module Top(in s: Stream) {{ let x = s; }}", Bundles::STREAM));
    assert_eq!(refused, [("E0264".to_string(), "s".to_string())]);
    let refused = Bundles::refused(&format!("{}module Top(in s: Stream) {{ let x = s == s; }}", Bundles::STREAM));
    assert_eq!(refused.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["E0264", "E0264"], "both operands: {refused:?}");
    // a field is a carrier, not a bundle: reading it is fine
    Bundles::accepted(&format!("{}module Top(in s: Stream) {{ let x = s.valid; }}", Bundles::STREAM));
}

#[test]
fn bundles_of_different_types_do_not_connect() {
    // §7 rule 14: a bundle connects to a port of its own `struct`, whatever the names of the fields
    let items = "struct A { out x: Bit }\nstruct B { out x: Bit }\nmodule Child(p: A) { p.x = true; }\n";
    Bundles::accepted(&format!("{items}module Top() {{ signal s: A; let c = Child(s); }}"));
    let refused = Bundles::refused(&format!("{items}module Top(q: B) {{ q.x = true; let c = Child(q); }}"));
    assert_eq!(refused, [("E0251".to_string(), "q".to_string())]);
}

#[test]
fn self_on_a_bundle_is_a_read_only_handle() {
    // §7 rule 13, §3.1.1: `self` of an `impl` on a bundle is a handle: its fields are read, probed and passed on
    let diff = "struct Diff { p: Electrical, n: Electrical }\nimpl Diff { fn vd(self) -> f64 { v(self.p, self.n) } }\n";
    Bundles::accepted(&format!("{diff}module Top(d: Diff) {{ }}\nanalog for Top {{ i(d.p, d.n) +> d.vd(); }}"));
    // the receiver is a bundle port, a flipped one, an observer's, or a signal; a field of a discrete nature is read as its value
    let stream = format!("{}impl Stream {{ fn taken(self) -> bool {{ self.valid && self.ready }} }}\n", Bundles::STREAM);
    Bundles::accepted(&format!("{stream}module Top(rx: flip Stream, in s: Stream) {{ signal ok: Bit; rx.ready = true; ok = rx.taken() && s.taken(); }}"));
    // a bundle handle is no value (§7 rule 9)
    let refused = Bundles::refused(&format!("{stream}impl Stream {{ fn whole(self) -> bool {{ let k = self; true }} }}\nmodule Top(rx: flip Stream) {{ rx.ready = rx.whole(); }}"));
    assert_eq!(refused.iter().map(|(code, _)| code.as_str()).collect::<Vec<_>>(), ["E0133"], "the handle is not kept (E0133, §3.1.5): {refused:?}");
}

#[test]
fn a_write_through_a_bundle_handle_is_e0133() {
    // §7 rule 13: through a handle every field is read-only, whatever the view; the diagnostic names the field
    let source = format!("{}impl Stream {{ fn open(self) {{ self.ready = true; }} }}\nmodule Top(rx: flip Stream) {{ rx.ready = true; }}\ndigital for Top {{ on rx.valid.changed() {{ rx.open(); }} }}", Bundles::STREAM);
    let design = Bundles::design(&source);
    let found = Types::check(&design, "Top").expect_err("expected diagnostics");
    let summary = Bundles::summary(&found);
    assert_eq!(summary.len(), 1, "{summary:?}");
    assert_eq!(summary[0].0, "E0133");
    assert!(summary[0].1.contains("`self.ready`"), "names the field: {}", summary[0].1);
}

#[test]
fn a_trait_is_not_implemented_for_a_bundle() {
    // §3.1.9 as `explicit-unknowns` narrowed it: only an inherent `impl` gives a bundle functions
    let source = "struct Diff { p: Electrical, n: Electrical }\ntrait Gap { fn gap(self) -> f64; }\nimpl Gap for Diff { fn gap(self) -> f64 { v(self.p, self.n) } }\nmodule Top(d: Diff) { }\nanalog for Top { i(d.p, d.n) +> d.gap(); }";
    let refused = Bundles::refused(source);
    assert_eq!(refused.first().map(|(code, _)| code.as_str()), Some("E0134"), "{refused:?}");
}

#[test]
fn an_array_of_bundles_gives_each_element_its_fields() {
    // §7 rule 11: an element of `[Lane; 2]` is a bundle of its own at a literal index, the array a bundle of as many
    let lane = "struct Lane { out a: Bit, in b: Bit }\nmodule Raise(tx: Lane) { tx.a = true; }\nmodule Echo(rx: flip Lane) { rx.b = rx.a; }\nmodule Both(tx: [Lane; 2]) { tx[0].a = true; tx[1].a = false; }\n";
    Bundles::accepted(&format!("{lane}module Top() {{ signal lanes: [Lane; 2]; let r = Raise(lanes[0]); let q = Raise(lanes[1]); let e = Echo(lanes[0]); let f = Echo(lanes[1]); }}"));
    Bundles::accepted(&format!("{lane}module Top() {{ signal lanes: [Lane; 2]; let t = Both(lanes); let e = Echo(lanes[0]); let f = Echo(lanes[1]); }}"));
    // an array of another length does not connect (`E0251`)
    let refused = Bundles::refused(&format!("{lane}module Top() {{ signal lanes: [Lane; 3]; let t = Both(lanes); }}"));
    assert!(refused.contains(&("E0251".to_string(), "lanes".to_string())), "{refused:?}");
}

#[test]
fn an_array_field_of_a_bundle_is_an_array_of_carriers() {
    // §7 rule 1: a field may be an array of nodes; it connects whole and its elements are probed by index
    let column = "struct Column { in wl: Bit, bl: [Electrical; 2] }\nmodule Pull(c: Column) { }\nanalog for Pull { v(c.bl[0]) +> 0.25; }\nmodule Hold(out w: Bit = false) { }\nmodule Pass(c: Column) { let p = Pull(c); }\n";
    Bundles::accepted(&format!("{column}module Top() {{ signal column: Column; let h = Hold(column.wl); let q = Pass(column); }}"));
}

#[test]
fn an_element_index_that_is_a_loop_variable_is_an_element_of_the_array() {
    // §7 rule 11, §8.2.23: in `for k in 0..3` the element `lanes[k]` is a bundle of its own, given to a producer and a consumer,
    // and `tx[k].a` is a field of it, written in the loop
    let lane = "struct Lane { out a: Bit, in b: Bit }\nmodule Raise(tx: Lane) { tx.a = true; }\nmodule Echo(rx: flip Lane) { rx.b = rx.a; }\nmodule Both(tx: [Lane; 3]) { for k in 0..3 { tx[k].a = true; } }\n";
    Bundles::accepted(&format!("{lane}module Top() {{ signal lanes: [Lane; 3]; for k in 0..3 {{ let r = Raise(lanes[k]); let e = Echo(lanes[k]); }} }}"));
    Bundles::accepted(&format!("{lane}module Top() {{ signal lanes: [Lane; 3]; let t = Both(lanes); for k in 0..3 {{ let e = Echo(lanes[k]); }} }}"));
    // an element of another type still does not connect (`E0251`), and an element that is no bundle is no field
    let refused = Bundles::refused(&format!("{lane}struct Other {{ out a: Bit }}\nmodule Take(o: Other) {{ o.a = true; }}\nmodule Top() {{ signal lanes: [Lane; 3]; for k in 0..3 {{ let t = Take(lanes[k]); }} }}"));
    assert!(refused.iter().any(|(code, _)| code == "E0251"), "{refused:?}");
}

#[test]
fn an_array_of_carriers_as_a_field_is_driven_and_read_by_index() {
    // §7 rule 1, rule 12: a field may be an array of carriers; `c.d[0]` is an element of it, driven by the module that sees the
    // field as `out`, and the whole field is handed on with the bundle
    let word = "struct Word { out d: [Bit; 2] }\nmodule Fill(w: Word) { w.d[0] = true; w.d[1] = false; }\nmodule Relay(w: Word) { let f = Fill(w); }\nmodule Look(in w: Word) { signal s: Bit = false; }\ndigital for Look { on w.d[1].changed() { s <- w.d[0]; } }\n";
    Bundles::accepted(&format!("{word}module Top() {{ signal word: Word; let r = Relay(word); let l = Look(word); }}"));
    // the element of the field is read through a consumer's view too, and driving it from the wrong view is `E0222`
    let refused = Bundles::refused(&format!("{word}module Top(w: flip Word) {{ w.d[0] = true; }}"));
    assert_eq!(refused.first().map(|(code, _)| code.as_str()), Some("E0222"), "{refused:?}");
}

#[test]
fn a_constant_parameter_in_an_array_length_of_a_field_is_replaced_by_its_argument() {
    // §7 rule 11: `Word<2>` has a field of two carriers: the element `1` is in range and the element `2` is not
    let word = "struct Word<const W: usize> { out bits: [Bit; W] }\n";
    Bundles::accepted(&format!("{word}module Fill(w: Word<2>) {{ w.bits[0] = true; w.bits[1] = false; }}\nmodule Top() {{ signal word: Word<2>; let f = Fill(word); }}"));
    Bundles::accepted(&format!("{word}module Fill<const W: usize>(w: Word<W>) {{ for k in 0..W {{ w.bits[k] = true; }} }}\nmodule Top() {{ signal word: Word<2>; let f = Fill::<2>(word); }}"));
    // the two lengths do not connect (`E0251`)
    let refused = Bundles::refused(&format!("{word}module Fill(w: Word<3>) {{ w.bits[0] = true; w.bits[1] = true; w.bits[2] = true; }}\nmodule Top() {{ signal word: Word<2>; let f = Fill(word); }}"));
    assert!(refused.contains(&("E0251".to_string(), "word".to_string())), "{refused:?}");
}

#[test]
fn an_array_of_bundles_of_a_generic_length_is_typed_per_instance() {
    // §7 rule 11: `[Lane; N]` is an array of bundles whose length is the parameter of the module, given by each placement, or
    // taught by the connection; `[flip Stream<u8>; N]` takes the views of its element, and a length computed from the parameter
    // (`N + 1`) is its value
    let lane = "struct Lane { out a: Bit, in b: Bit }\nmodule Raise(tx: Lane) { tx.a = true; }\nmodule Echo(rx: flip Lane) { rx.b = rx.a; }\n";
    let bank = "module Bank<const N: usize>(tx: [Lane; N]) { for k in 0..N { let r = Raise(tx[k]); } }\n";
    Bundles::accepted(&format!("{lane}{bank}module Top() {{ signal lanes: [Lane; 2]; let b = Bank::<2>(lanes); for k in 0..2 {{ let e = Echo(lanes[k]); }} }}"));
    Bundles::accepted(&format!("{lane}{bank}module Top() {{ signal lanes: [Lane; 3]; let b = Bank::<3>(lanes); for k in 0..3 {{ let e = Echo(lanes[k]); }} }}"));
    Bundles::accepted(&format!("{lane}{bank}module Top() {{ signal lanes: [Lane; 3]; let b = Bank(lanes); for k in 0..3 {{ let e = Echo(lanes[k]); }} }}"));
    let stream = "struct Stream<T: Repr> { out valid: Bit, in ready: Bit, out data: Bit<T> }\n";
    let sinks = "module Sinks<const N: usize>(rx: [flip Stream<u8>; N]) { for k in 0..N { rx[k].ready = rx[k].valid; } }\nmodule Offer(tx: Stream<u8>) { tx.valid = true; tx.data = 7; }\n";
    Bundles::accepted(&format!("{stream}{sinks}module Top() {{ signal s: [Stream<u8>; 2]; let k = Sinks::<2>(s); let a = Offer(s[0]); let b = Offer(s[1]); }}"));
    let wide = "module Wide<const N: usize>(tx: [Lane; N + 1]) { for k in 0..N + 1 { let r = Raise(tx[k]); } }\n";
    Bundles::accepted(&format!("{lane}{wide}module Top() {{ signal lanes: [Lane; 3]; let w = Wide::<2>(lanes); for k in 0..3 {{ let e = Echo(lanes[k]); }} }}"));
    // another length than the placement's argument does not connect (`E0251`)
    let refused = Bundles::refused(&format!("{lane}{bank}module Top() {{ signal lanes: [Lane; 3]; let b = Bank::<2>(lanes); }}"));
    assert!(refused.contains(&("E0251".to_string(), "lanes".to_string())), "{refused:?}");
}

#[test]
fn two_producers_of_one_element_of_an_array_of_bundles_is_e0220_naming_it() {
    // §7 rule 15: the one-driver rule is judged element by element: `lanes[0]` and `lanes[1]` have a producer each, a second
    // producer of `lanes[1]` is `E0220` naming the field of that element
    let lane = "struct Lane { out a: Bit, in b: Bit }\nmodule Raise(tx: Lane) { tx.a = true; }\nmodule Echo(rx: flip Lane) { rx.b = rx.a; }\n";
    Bundles::accepted(&format!("{lane}module Top() {{ signal lanes: [Lane; 2]; let r = Raise(lanes[0]); let q = Raise(lanes[1]); let e = Echo(lanes[0]); let f = Echo(lanes[1]); }}"));
    let src = format!("{lane}module Top() {{ signal lanes: [Lane; 2]; let r = Raise(lanes[0]); let q = Raise(lanes[1]); let t = Raise(lanes[1]); let e = Echo(lanes[0]); let f = Echo(lanes[1]); }}");
    let refused = Bundles::refused(&src);
    assert!(refused.iter().any(|(code, _)| code == "E0220"), "{refused:?}");
    let messages = Bundles::messages(&src);
    assert!(messages.iter().any(|m| m.contains("`lanes[1].a`")), "names the field of the element: {messages:?}");
    assert!(!messages.iter().any(|m| m.contains("lanes[0]")), "the element 0 has one producer: {messages:?}");
}

#[test]
fn an_element_of_an_array_of_bundles_no_producer_drives_is_e0187_naming_it() {
    // §3.3.4, §7 rule 15: each element's field has its own driver: with producers of `lanes[0]` and `lanes[2]` only, the field `a`
    // of `lanes[1]` has no value (a `Bit` has no `Default`): `E0187` names that element and no other
    let lane = "struct Lane { out a: Bit, in b: Bit }\nmodule Raise(tx: Lane) { tx.a = true; }\nmodule Echo(rx: flip Lane) { rx.b = rx.a; }\n";
    let src = format!("{lane}module Top() {{ signal lanes: [Lane; 3]; let r = Raise(lanes[0]); let q = Raise(lanes[2]); for k in 0..3 {{ let e = Echo(lanes[k]); }} }}");
    let refused = Bundles::refused(&src);
    assert!(refused.iter().any(|(code, _)| code == "E0187"), "{refused:?}");
    let messages = Bundles::messages(&src);
    let missing: Vec<&String> = messages.iter().filter(|m| m.contains("no initial value")).collect();
    assert_eq!(missing.len(), 1, "{messages:?}");
    assert!(missing[0].starts_with("`lanes[1].a` has no initial value"), "{missing:?}");
}


/// A bundle with both directions, a template bounded over it and a placement of each view through the bound.
struct Templates;

impl Templates {
    /// `Pipe`, its three views as modules, and `Bank<C: Fn(Pipe)>` that places the template over a bundle literal.
    const BANK: &'static str = "struct Pipe { out q: Bit<u8>, in d: Bit<u8> }\n\
        module Drv(p: Pipe) { p.q = p.d + 1; }\n\
        module Snk(p: flip Pipe) { p.d = p.q + 2; }\n\
        module Obs(in p: Pipe) { signal t: Bit<u8>; t = p.q; }\n\
        module Bank<C: Fn(Pipe)>(in d: Bit<u8>) { cell: C; signal s: Bit<u8>; let one = cell(Pipe { q: s, d: d }); }\n";

    /// `Bank` of the bound `Fn(flip Pipe)`: the bank gives the cell what it reads and takes what it drives.
    const FLIPPED_BANK: &'static str = "module FlippedBank<C: Fn(flip Pipe)>(out r: Bit<u8>) { cell: C; signal s: Bit<u8> = 1; let one = cell(Pipe { q: s, d: r }); }\n";

    /// The code and message of each diagnostic of the placement `place` in `Top`.
    fn placing(prelude: &str, place: &str) -> Vec<(String, String)> {
        let src = format!("{prelude}module Top() {{ signal d: Bit<u8> = 5; {place} }}");
        let design = Bundles::design(&src);
        Types::check(&design, "Top").err().map(|found| Bundles::summary(&found)).unwrap_or_default()
    }
}

#[test]
fn a_template_of_the_bounds_own_view_is_accepted_through_the_bound() {
    // §8.4.5, §7 rules 3 to 5: `Drv` has `Pipe` as a producer, the view of `Fn(Pipe)`; `Snk` has it as a consumer, the view of `Fn(flip Pipe)`
    assert_eq!(Templates::placing(Templates::BANK, "let a = Bank { cell: Drv {} }(d);"), []);
    assert_eq!(Templates::placing(&format!("{}{}", Templates::BANK, Templates::FLIPPED_BANK), "signal r: Bit<u8>; let a = FlippedBank { cell: Snk {} }(r);"), []);
}

#[test]
fn a_template_of_another_view_than_the_bound_is_e0360_naming_the_port() {
    // §7 rules 3 to 5: through `Fn(Pipe)` the bank drives `q` and reads `d`; `Snk` drives `d`, which is the bank's `in d`, and `Obs`
    // drives nothing, which leaves `s` with no driver. Placed directly both are refused (E0314, E0187), so the bound refuses them too.
    for (module, view) in [("Snk", "a consumer"), ("Obs", "an observer")] {
        let found = Templates::placing(Templates::BANK, &format!("let a = Bank {{ cell: {module} {{}} }}(d);"));
        assert_eq!(found.len(), 1, "{module}: {found:?}");
        assert_eq!(found[0].0, "E0360", "{module}: {found:?}");
        assert!(found[0].1.contains("`p`") && found[0].1.contains(view) && found[0].1.contains("wants it as a producer"), "{module}: {found:?}");
    }
    let found = Templates::placing(&format!("{}{}", Templates::BANK, Templates::FLIPPED_BANK), "signal r: Bit<u8>; let a = FlippedBank { cell: Drv {} }(r);");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].0 == "E0360" && found[0].1.contains("takes the bundle as a producer") && found[0].1.contains("wants it as a consumer"), "{found:?}");
}

#[test]
fn a_literal_given_through_a_flipped_bound_is_handed_over_by_the_bounds_view() {
    // §7 rule 5: through `Fn(flip Pipe)` the cell drives `d` and reads `q`, so the bank may not hand over its `in` port `d` as the cell's
    // output (`E0314`, as placing `Snk` directly), and `s` is read by the cell, so the bank holds its driver
    let found = Templates::placing("struct Pipe { out q: Bit<u8>, in d: Bit<u8> }\nmodule Bank<C: Fn(flip Pipe)>(in d: Bit<u8>) { cell: C; signal s: Bit<u8> = 1; let one = cell(Pipe { q: s, d: d }); }\nmodule Snk(p: flip Pipe) { p.d = p.q + 2; }\n", "let a = Bank { cell: Snk {} }(d);");
    assert!(found.iter().any(|(code, _)| code == "E0314"), "{found:?}");
    assert!(!found.iter().any(|(code, _)| code == "E0223"), "`s` is the bank's, the cell reads it: {found:?}");
}

#[test]
fn a_field_default_is_the_initial_value_of_the_carrier_its_producer_commits() {
    // §7 rule 10, §5.2.8: a `Bit` field committed in a reaction needs an initial value, and a bundle port takes none: the field gives
    // it (`out q: Bit = false`). Without it the leaf is `E0187`, and the help sends the author to the field.
    let with = "struct Pipe { out q: Bit = false, in d: Bit }\nmodule Top(in clk: Bit, p: Pipe) { }\ndigital for Top { on clk.rise() { p.q <- p.d; } }";
    Bundles::accepted(with);
    let without = "struct Pipe { out q: Bit, in d: Bit }\nmodule Top(in clk: Bit, p: Pipe) { }\ndigital for Top { on clk.rise() { p.q <- p.d; } }";
    let design = Bundles::design(without);
    let found = Types::check(&design, "Top").expect_err("`p.q` has no initial value");
    let missing = found.iter().find(|d| d.diagnostic.code.as_ref() == "E0187").unwrap_or_else(|| panic!("{:?}", Bundles::summary(&found)));
    assert!(missing.diagnostic.message.contains("`p.q`"), "{}", missing.diagnostic.message);
    let help = missing.diagnostic.help.clone().unwrap_or_default();
    assert!(help.contains("field `q`") && help.contains("struct"), "the help names the field's default: {help}");
    // a bundle signal takes it the same way, and a child that holds the drive of it gives no second initial value (no E0223)
    Bundles::accepted("struct Pipe { out q: Bit = false }\nmodule Drv(in clk: Bit, p: Pipe) { }\ndigital for Drv { on clk.rise() { p.q <- true; } }\nmodule Top(in clk: Bit) { signal s: Pipe; let a = Drv(clk, s); }");
    // as an `out` port with an initial value, a definition of it would replace the value (§8.2.21)
    assert_eq!(Bundles::refused("struct Pipe { out q: Bit = false }\nmodule Top(p: Pipe) { p.q = true; }").first().map(|d| d.0.clone()), Some("E0338".to_string()));
    // the value is of the field's type
    assert_eq!(Bundles::refused("struct Pipe { out q: Bit = 3 }\nmodule Top(p: Pipe) { p.q = true; }").first().map(|d| d.0.clone()), Some("E0210".to_string()));
}
