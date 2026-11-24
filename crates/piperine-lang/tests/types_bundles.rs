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
