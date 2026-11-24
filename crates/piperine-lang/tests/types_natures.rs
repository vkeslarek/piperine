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

    /// The codes the design itself reports for `text` (the POM's, before any checking).
    fn design_codes(text: &str) -> Vec<String> {
        Design::builder().source("top.msl", text).build().diagnostics().iter().map(|d| d.diagnostic.code.to_string()).collect()
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
    let source = Source::new(&format!("{BYTE}module Top(in a: Byte, out b: Byte = 0) {{ }}"));
    source.checked();
}

#[test]
fn connecting_byte_to_bit_u8_is_e0251_naming_both_natures() {
    // §6.1.3, §8.1.10: two carriers connect only when nature and value type agree; the message
    // names both natures, though `Byte` and `Bit<u8>` hold the same type.
    let source = Source::new(&format!("{BYTE}module Sink(in x: Bit<u8>) {{ }}\nmodule Top() {{ signal b: Byte = 0; let s = Sink(b); }}"));
    let diagnostics = source.diagnostics();
    assert_eq!(diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect::<Vec<_>>(), ["E0251"]);
    let message = &diagnostics[0].diagnostic.message;
    assert!(message.contains("`Byte`") && message.contains("`Bit<u8>`") && !message.contains("Byte<"), "{message}");
    let source = Source::new(&format!("{BYTE}module Sink(in x: Byte) {{ }}\nmodule Top() {{ signal b: Byte = 0; let s = Sink(b); }}"));
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
    let source = Source::new(&format!("{BYTE}module Source(out q: Byte = 0) {{ }}\nmodule Top() {{ signal s: Byte; let a = Source(s); let b = Source(s); }}"));
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

#[test]
fn a_flow_contribution_to_a_signal_flow_node_is_e0413() {
    // §11 rule 11, §6 Resolution of rule 2: a node of a nature without `flow` takes potential
    // contributions only; `flow(o, gnd) +> x` is `E0413`, while `v(o) +> x` is fine.
    let source = Source::new("module Top(a: VSignal, o: VSignal) { }\nanalog for Top { flow(o, gnd) +> v(a); }");
    assert_eq!(source.codes(), ["E0413"]);
    let source = Source::new("module Top(a: VSignal, o: VSignal) { }\nanalog for Top { v(o) +> 2.0 * v(a); }");
    source.checked();
    // RN-08 AC 4: any flow contribution to such a node is `E0413`, spelled with another nature's
    // access name (`i`) as well, which the POM would otherwise take for a probe of an access the
    // nature lacks (`E0206`, reserved for reads).
    let spelled = "module Top(a: VSignal, o: VSignal) { }\nanalog for Top { i(o) +> v(a); }";
    assert_eq!(Source::design_codes(spelled), ["E0413"]);
}

#[test]
fn a_flow_probe_of_a_signal_flow_node_is_e0206() {
    // §6 rule 10: `flow` applies to a nature with a `flow`. Spelled `flow(..)` the checker raises
    // `E0206`; spelled with another nature's access name (`i`) the POM raises it, since `VSignal`
    // declares no such access.
    let source = Source::new("module Top(a: VSignal, o: VSignal) { }\nanalog for Top { v(o) +> flow(a, gnd); }");
    assert_eq!(source.codes(), ["E0206"]);
    assert_eq!(Source::design_codes("module Top(a: VSignal, o: VSignal) { }\nanalog for Top { v(o) +> i(a); }"), ["E0206"]);
}

/// A project nature resolved by summing its drives (`f64`).
const TOTAL: &str = "nature Total { value: f64 }\nimpl Resolve<f64> for Total { fn resolve<const N: usize>(d: [f64; N]) -> f64 { d.iter().sum() } }\n";

#[test]
fn an_impl_resolve_makes_a_discrete_nature_resolved() {
    // §4 rule 8: the `impl` is the whole declaration; a resolved port takes no direction (§8.1.5)
    // and its carrier holds the nature's `value`.
    let source = Source::new(&format!("{TOTAL}module Top(a: Total) {{ signal s: Total; }}"));
    assert_eq!(source.carrier("a"), source.carrier("s"));
    assert!(matches!(source.carrier("s"), Ty::Carrier { value, .. } if *value == Ty::F64));
    // a one-argument `impl Resolve<V>` has `X = V`
    let single = "nature Level { value: bool }\nimpl Resolve<bool> for Level { fn resolve<const N: usize>(d: [bool; N]) -> bool { false } }\n";
    Source::new(&format!("{single}module Top(a: Level) {{ }}")).checked();
}

#[test]
fn resolve_may_call_a_library_or_project_function() {
    // §4 rule 8: the body of `resolve` is any pure function: a library function, a project one.
    let library = "nature Strongest { value: Driven }\nimpl Resolve<Driven> for Strongest { fn resolve<const N: usize>(d: [Driven; N]) -> Driven { Driven::strongest(d) } }\n";
    Source::new(&format!("{library}module Top(a: Strongest) {{ }}")).checked();
    let project = "fn add<const N: usize>(d: [f64; N]) -> f64 { d.iter().sum() }\nnature Total { value: f64 }\nimpl Resolve<f64> for Total { fn resolve<const N: usize>(d: [f64; N]) -> f64 { add(d) } }\n";
    Source::new(&format!("{project}module Top(a: Total) {{ }}")).checked();
}

#[test]
fn an_impl_resolve_of_another_shape_is_e0203() {
    // §4 rule 8: `N` discrete, `V` its `value`, `resolve` with one `N: usize` and one `[X; N]` that
    // gives `V`; every other shape is `E0203` naming the nature, at the `impl`, with no carrier of it.
    let wrong = [
        // `V` is not the nature's value
        "nature Total { value: f64 }\nimpl Resolve<f64, bool> for Total { fn resolve<const N: usize>(d: [f64; N]) -> bool { false } }\nmodule Top() { }",
        // a continuous nature
        "impl Resolve<f64> for Electrical { fn resolve<const N: usize>(d: [f64; N]) -> f64 { 0.0 } }\nmodule Top() { }",
        // the parameter is not `[X; N]`
        "nature Total { value: f64 }\nimpl Resolve<f64> for Total { fn resolve<const N: usize>(d: f64) -> f64 { d } }\nmodule Top() { }",
        // the result is not `V`
        "nature Total { value: f64 }\nimpl Resolve<f64> for Total { fn resolve<const N: usize>(d: [f64; N]) -> bool { true } }\nmodule Top() { }",
        // no const parameter
        "nature Total { value: f64 }\nimpl Resolve<f64> for Total { fn resolve(d: [f64; 2]) -> f64 { 0.0 } }\nmodule Top() { }",
        // no `resolve`
        "nature Total { value: f64 }\nimpl Resolve<f64> for Total { fn other<const N: usize>(d: [f64; N]) -> f64 { 0.0 } }\nmodule Top() { }",
    ];
    for text in wrong {
        let source = Source::new(text);
        let diagnostics = Types::check_model(&source.design).expect_err(text);
        assert_eq!(diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect::<Vec<_>>().first().map(String::as_str), Some("E0203"), "{text}");
    }
    let source = Source::new("nature Total { value: f64 }\nimpl Resolve<f64, bool> for Total { fn resolve<const N: usize>(d: [f64; N]) -> bool { false } }\nmodule Top() { }");
    let diagnostics = Types::check_model(&source.design).expect_err("E0203");
    assert!(diagnostics[0].diagnostic.message.contains("Total"), "{}", diagnostics[0].diagnostic.message);
}

#[test]
fn a_resolve_field_is_not_a_nature_field() {
    // the `resolve:` field is gone: the parser reports any word that is not a nature field.
    let codes = Source::design_codes("nature Total { value: f64, resolve: sum }\nmodule Top() { }");
    assert!(!codes.is_empty(), "a `resolve:` field must not parse");
}

#[test]
fn a_direction_on_a_resolved_port_is_e0310() {
    // §8.1.5: a port of a resolved nature takes no direction; a one-driver nature still needs one.
    let source = Source::new(&format!("{TOTAL}module Top(in a: Total) {{ }}"));
    assert_eq!(source.codes(), ["E0310"]);
    let source = Source::new(&format!("{TOTAL}module Top(out a: Total) {{ }}"));
    assert_eq!(source.codes(), ["E0310"]);
}

#[test]
fn others_of_a_resolved_carrier_has_the_value_type() {
    // §5.2.7: `s.others()` reads the drives of a resolved carrier but the module's own; it has the
    // nature's `value` type `V`, in a definition, in a reaction and in `analog`.
    let text = format!("{TOTAL}module Top(a: Total, in clk: Bit) {{ signal t: Bit<f64>; t = a.others() + 1.0; }}\ndigital for Top {{ on clk.rise() {{ let x: f64 = a.others(); }} }}");
    Source::new(&text).checked();
    let wrong = format!("{TOTAL}module Top(a: Total, in clk: Bit) {{ }}\ndigital for Top {{ on clk.rise() {{ let x: bool = a.others(); }} }}");
    assert_eq!(Source::new(&wrong).codes(), ["E0180"]);
    let analog = format!("{TOTAL}module Top(a: Total, p: Electrical) {{ }}\nanalog for Top {{ v(p) +> a.others(); }}");
    Source::new(&analog).checked();
}

#[test]
fn a_definition_and_a_commit_of_one_resolved_carrier_is_e0221() {
    // §5.2.4: a module drives a carrier by definitions or by commits, never both, resolved or not.
    let text = format!("{TOTAL}module Top(a: Total, in clk: Bit) {{ a = 1.0; }}\ndigital for Top {{ on clk.rise() {{ a <- 2.0; }} }}");
    assert_eq!(Source::new(&text).codes(), ["E0221"]);
    let commits = format!("{TOTAL}module Top(a: Total = 0.0, in clk: Bit) {{ }}\ndigital for Top {{ on clk.rise() {{ a <- 2.0; }} }}");
    Source::new(&commits).checked();
}

#[test]
fn a_module_local_loop_of_resolved_drives_is_accepted() {
    // §8.2.3: a loop through a resolved carrier is not `E0339`: the drives combine, and relaxation settles it.
    let text = format!("{TOTAL}module Top(a: Total, b: Total) {{ a = b.others() + 1.0; b = a.others() + 1.0; }}");
    Source::new(&text).checked();
    let one_driver = "module Top() { signal a: Bit; signal b: Bit; a = !b; b = !a; }";
    assert_eq!(Source::new(one_driver).codes(), ["E0339"]);
}

#[test]
fn a_second_drive_of_a_one_driver_carrier_names_the_nature_and_resolve() {
    // §5.2.1: a second drive of a nature with no `impl Resolve` is `E0220`, and says what would make it legal.
    let text = "module Source(out q: Bit = false) { }\nmodule Top() { signal s: Bit; let a = Source(s); let b = Source(s); }";
    let diagnostics = Source::new(text).diagnostics();
    assert_eq!(diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect::<Vec<_>>(), ["E0220"]);
    let message = &diagnostics[0].diagnostic.message;
    assert!(message.contains("`Bit`") && message.contains("Resolve"), "{message}");
}

/// A project nature with two `impl`s: `AndWire` takes `bool`, `Counted` takes `bool` or `u8`.
const TWO_IMPLS: &str = "nature Counted { value: u8 }\nimpl Resolve<bool, u8> for Counted { fn resolve<const N: usize>(d: [bool; N]) -> u8 { 0 } }\nimpl Resolve<u8> for Counted { fn resolve<const N: usize>(d: [u8; N]) -> u8 { 0 } }\nnature AndWire { value: bool }\nimpl Resolve<bool> for AndWire { fn resolve<const N: usize>(d: [bool; N]) -> bool { true } }\n";

#[test]
fn a_drive_picks_the_impl_of_its_type() {
    // §5.2.15: the type of a definition or a commit picks the `impl Resolve<X, V>` whose `X` it is.
    let text = format!("{TWO_IMPLS}module Top(c: Counted = false, in clk: Bit) {{ signal b: bool; }}\ndigital for Top {{ on clk.rise() {{ let flag: bool = true; c <- flag; }} }}");
    let source = Source::new(&text);
    let types = source.checked();
    let picks = (0..source.design.expr_count()).filter(|index| types.drive_impl(piperine_lang::pom::ExprId(*index as u32)).is_some()).count();
    assert_eq!(picks, 2, "the initial value and the commit each picked an `impl`");
    let text = format!("{TWO_IMPLS}module Top(c: Counted) {{ c = 7u8; }}");
    Source::new(&text).checked();
}

#[test]
fn a_drive_no_impl_accepts_is_e0180_and_an_ambiguous_literal_e0181() {
    // §5.2.15: a type no `impl` takes is `E0180` naming the nature and the driven types; a bare
    // literal that two `impl`s accept is `E0181`.
    let text = format!("{TWO_IMPLS}module Top(w: AndWire) {{ w = 1.5; }}");
    let diagnostics = Source::new(&text).diagnostics();
    assert_eq!(diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect::<Vec<_>>(), ["E0180"]);
    assert!(diagnostics[0].diagnostic.message.contains("AndWire") && diagnostics[0].diagnostic.message.contains("bool"), "{}", diagnostics[0].diagnostic.message);
    let text = format!("{TWO_IMPLS}module Top(c: Counted) {{ c = 1.5; }}");
    let diagnostics = Source::new(&text).diagnostics();
    assert_eq!(diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect::<Vec<_>>(), ["E0180"]);
    assert!(diagnostics[0].diagnostic.message.contains("Counted") && diagnostics[0].diagnostic.message.contains("u8"), "{}", diagnostics[0].diagnostic.message);
    let wide = "nature Wide { value: u8 }\nimpl Resolve<u8> for Wide { fn resolve<const N: usize>(d: [u8; N]) -> u8 { 0 } }\nimpl Resolve<u16, u8> for Wide { fn resolve<const N: usize>(d: [u16; N]) -> u8 { 0 } }\n";
    let text = format!("{wide}module Top(c: Wide) {{ c = 1; }}");
    assert_eq!(Source::new(&text).codes(), ["E0181"]);
    let text = format!("{wide}module Top(c: Wide) {{ c = 1u16; }}");
    Source::new(&text).checked();
}
