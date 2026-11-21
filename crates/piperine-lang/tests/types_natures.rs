//! Natures of every kind in `piperine_lang::types` (feature `resolved-nets`, RN-01, RN-04, RN-08):
//! the carriers of a project's own discrete nature, generic with defaults or not (§4 rule 12), the
//! rules every discrete carrier shares (`E0310`, `E0220`, `E0251`, `E0112`), `others()` on a
//! one-driver carrier (`E0224`, §5.2.7), and signal-flow nodes (`E0413`, `E0206`). Sources are
//! written over the embedded standard library; every expected code comes from the rule it cites.

use piperine_lang::pom::{DeclKind, Design, DesignDiagnostic, Reflect};
use piperine_lang::types::{Ty, Types};

/// One MSL source under test and the checker's answer for its module `Top`.
struct Source {
    /// The text as the test wrote it.
    text: String,
    /// The design the text builds, which must parse and resolve cleanly.
    design: Design,
}

impl Source {
    /// Builds the design of `text`; a source that does not parse is the test's own mistake.
    fn new(text: &str) -> Source {
        let design = Design::builder().source("top.msl", text).build();
        assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{text}", design.diagnostics().iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
        Source { text: text.to_string(), design }
    }

    /// The diagnostics of checking `Top`, which must be there.
    fn diagnostics(&self) -> Vec<DesignDiagnostic> {
        Types::check(&self.design, "Top").expect_err("expected diagnostics")
    }

    /// The codes of the diagnostics, in order.
    fn codes(&self) -> Vec<String> {
        self.diagnostics().iter().map(|d| d.diagnostic.code.to_string()).collect()
    }

    /// The checked table of `Top`, which must be clean.
    fn checked(&self) -> Types<'_> {
        Types::check(&self.design, "Top").unwrap_or_else(|d| panic!("expected a clean check: {:?}\n{}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>(), self.text))
    }

    /// The type of the carrier port or signal named `name`.
    fn carrier(&self, name: &str) -> Ty {
        let types = self.checked();
        let decl = self.design.decls().into_iter().find(|d| d.name() == Some(name) && matches!(d.kind(), DeclKind::Port(_) | DeclKind::Signal(_))).unwrap_or_else(|| panic!("no port or signal `{name}`"));
        types.carrier(decl.id()).cloned().unwrap_or_else(|| panic!("`{name}` is no carrier"))
    }
}

/// A project's own discrete nature: one byte.
const BYTE: &str = "nature Byte { value: u8 }\n";

#[test]
fn a_project_discrete_port_without_a_direction_is_e0310() {
    // §8.1.5: a port of a discrete nature takes `in` or `out`, whoever declared the nature.
    let source = Source::new(&format!("{BYTE}module Top(a: Byte) {{ }}"));
    assert_eq!(source.codes(), ["E0310"]);
    let source = Source::new(&format!("{BYTE}module Top(in a: Byte, out b: Byte) {{ }}"));
    source.checked();
}

#[test]
fn connecting_byte_to_bit_u8_is_e0251_naming_both_natures() {
    // §6.1.3, §8.1.10: two carriers connect only when nature and value type agree; the message
    // names both natures, though `Byte` and `Bit<u8>` hold the same type.
    let source = Source::new(&format!("{BYTE}module Sink(in x: Bit<u8>) {{ }}\nmodule Top() {{ signal b: Byte; let s = Sink(b); }}"));
    let diagnostics = source.diagnostics();
    assert_eq!(diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect::<Vec<_>>(), ["E0251"]);
    let message = &diagnostics[0].diagnostic.message;
    assert!(message.contains("Byte<u8>") && message.contains("Bit<u8>"), "{message}");
    let source = Source::new(&format!("{BYTE}module Sink(in x: Byte) {{ }}\nmodule Top() {{ signal b: Byte; let s = Sink(b); }}"));
    source.checked();
}

#[test]
fn a_nature_value_without_repr_is_e0112() {
    // §2, §4 rule 11: a nature's `value` is a type with `Repr`; an enum that does not derive it is
    // `E0112` at the carrier.
    let source = Source::new("enum Mode { A, B }\nnature Moded { value: Mode }\nmodule Top(in m: Moded) { }");
    assert_eq!(source.codes(), ["E0112"]);
}

#[test]
fn a_second_driver_of_a_project_discrete_carrier_is_e0220() {
    // §5.2.1: a carrier of a one-driver nature has one drive, a project's nature included.
    let source = Source::new(&format!("{BYTE}module Source(out q: Byte) {{ }}\nmodule Top() {{ signal s: Byte; let a = Source(s); let b = Source(s); }}"));
    assert_eq!(source.codes(), ["E0220"]);
}

#[test]
fn a_generic_project_nature_takes_its_argument_or_its_default() {
    // §4 rule 12: `Cell<T: Repr = u8>` written bare holds the default `u8`; `Cell<u16>` holds `u16`.
    let source = Source::new("nature Cell<T: Repr = u8> { value: T }\nmodule Top(in a: Cell, in b: Cell<u16>) { }");
    let nature = |name: &str| match source.carrier(name) {
        Ty::Carrier { value, .. } => *value,
        other => panic!("not a carrier: {other:?}"),
    };
    assert_eq!(nature("a"), Ty::UInt(8));
    assert_eq!(nature("b"), Ty::UInt(16));
}

#[test]
fn others_on_a_one_driver_carrier_is_e0224() {
    // §5.2.7: `others()` is the read of a resolved carrier; on a carrier of a nature with no
    // `resolve` there is no other drive, `E0224` naming the carrier. `Bit` and a project's nature
    // alike.
    for (items, port) in [("", "Bit<u8>"), (BYTE, "Byte")] {
        let text = format!("{items}module Top(in s: {port}) {{ }}\ndigital for Top {{ on settle {{ let x = s.others(); }} }}");
        let source = Source::new(&text);
        let diagnostics = source.diagnostics();
        assert_eq!(diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect::<Vec<_>>(), ["E0224"], "{text}");
        assert!(diagnostics[0].diagnostic.message.contains("`s`"), "{}", diagnostics[0].diagnostic.message);
        assert_eq!(&text[diagnostics[0].diagnostic.span().range()], "s.others()");
    }
}
