//! The checker's side of the explicit model of unknowns (part I §3.3, §22.1): `analog` reads a
//! discrete carrier like any expression (§3.3.6), directly, through a `let`, a function or a
//! placement; `T::WIDTH` typed on `Repr` values, `same` and `cast` typed by their contract at the
//! concrete type (`E0701`), the flag methods that resolve to nothing (`E0161`), and `logic` and
//! `matches_bits` refused until `Logic` (`P0001`). One positive and one negative source per rule, over the embedded
//! standard library (the five functions are the stdlib's own items).

#[path = "support/pom.rs"]
mod support;
#[allow(dead_code)]
#[path = "support/checked.rs"]
mod checked_source;

use piperine_lang::pom::{DeclId, DeclKind, Design, DesignDiagnostic, Reflect};
use piperine_lang::types::{Ty, Types};

/// A design of `Top` with `body` as its analog block and `items` before it.
fn source(items: &str, body: &str) -> String {
    format!("{items}\nmodule Top() {{ }}\nanalog for Top {{ {body} }}")
}

fn design_of(src: &str) -> Design {
    let design = Design::builder().source("top.msl", src).build();
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", design.diagnostics().iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    design
}

fn binding(design: &Design, name: &str) -> DeclId {
    design.decls().into_iter().find(|d| d.name() == Some(name) && matches!(d.kind(), DeclKind::Binding(_))).unwrap_or_else(|| panic!("no binding `{name}`")).id()
}

/// The type of each named `let` binding of the analog block.
fn local_types(items: &str, body: &str, names: &[&str]) -> Vec<Ty> {
    let src = source(items, body);
    let design = design_of(&src);
    let types = Types::check(&design, "Top").unwrap_or_else(|d| panic!("expected a clean check: {:?}\n{src}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()));
    names.iter().map(|n| types.local(binding(&design, n)).unwrap_or_else(|| panic!("no type for `{n}`")).clone()).collect()
}

fn diagnostics(items: &str, body: &str) -> (String, Vec<DesignDiagnostic>) {
    let src = source(items, body);
    let design = design_of(&src);
    let err = Types::check(&design, "Top").expect_err("expected diagnostics");
    (src, err)
}

fn codes_of(diagnostics: &[DesignDiagnostic]) -> Vec<String> {
    diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect()
}

/// Mode, a unit enum with `Repr`, for the receivers below.
const MODE: &str = "#[derive(Repr)] enum Mode { Idle, Run, Halt }";

#[test]
fn maybe_is_an_ordinary_field_name() {
    // EU-02 AC 3 (C6, §19 rule 11): no record has a `maybe` any more, so an `emit!` field may be
    // named `maybe`; only a repeated name in one `emit!` is `E0604`
    let src = "module Top(in clk: Bit) { }\ndigital for Top { on clk.rise() { emit!(\"s\", maybe = true, x = 1.0); } }";
    let design = design_of(src);
    if let Err(d) = Types::check(&design, "Top") {
        panic!("`maybe` is an ordinary field name: {:?}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
    }
    let twice = "module Top(in clk: Bit) { }\ndigital for Top { on clk.rise() { emit!(\"s\", maybe = true, maybe = false); } }";
    let design = design_of(twice);
    let err = Types::check(&design, "Top").expect_err("a repeated field");
    assert_eq!(codes_of(&err), ["E0604"]);
}

#[test]
fn the_flag_methods_do_not_resolve() {
    // EU-02 AC 2 (C5): `level`, `known`, `known_or`, `is_known`, `high` and `merge` resolve to nothing: `E0161`
    // naming the method and the type, with the help that names an initial value, `Option<T>` and `Logic`
    for (call, method, ty) in [("3_u8.level(0)", "level", "u8"), ("3_u8.known()", "known", "u8"), ("3_u8.known_or(1)", "known_or", "u8"), ("true.is_known()", "is_known", "bool"), ("true.high()", "high", "bool"), ("1.5.known_or(0.0)", "known_or", "f64")] {
        let (src, d) = diagnostics("", &format!("let a = {call};"));
        assert_eq!(codes_of(&d), ["E0161"], "{call}\n{src}");
        let found = &d[0].diagnostic;
        assert!(found.message.contains(&format!("`{method}`")) && found.message.contains(&format!("`{ty}`")), "{call}: names the method and the type: {}", found.message);
        let help = found.help.as_deref().unwrap_or_default();
        assert!(["initial value", "Option<T>", "Logic"].iter().all(|word| help.contains(word)), "{call}: the help: {help}");
    }
    // `merge` is a function of the earlier std: it is not found at all
    let design = Design::builder().source("top.msl", "module Top() { }\nanalog for Top { let a = merge(1.0, 2.0); }").build();
    assert_eq!(codes_of(design.diagnostics()), ["E0161"]);
}

#[test]
fn unknown_is_a_name_that_does_not_resolve() {
    // EU-02 AC 1 (C4): `Unknown` in a value position is no name: `E0161` at the name, whose help names the three ways to
    // say what a carrier holds before a drive, an initial value, `Option<T>` and `Logic`
    for (src, name) in [
        ("module Top() { }\nanalog for Top { let a = Unknown; }", "Unknown"),
        ("module Top() { signal s: Bit<u8> = Unknown; }", "Unknown"),
    ] {
        let design = Design::builder().source("top.msl", src).build();
        assert_eq!(codes_of(design.diagnostics()), ["E0161"], "{src}");
        let found = &design.diagnostics()[0].diagnostic;
        let span = found.span();
        assert_eq!(&src[span.start..span.end], name, "the error sits at the name: {src}");
        let help = found.help.as_deref().unwrap_or_default();
        assert!(["initial value", "Option<T>", "Logic"].iter().all(|word| help.contains(word)), "{src}: the help: {help}");
    }
}

#[test]
fn logic_and_matches_bits_wait_for_logic() {
    // EU-02 (GAPS L68): until `Logic` (T45) the two functions of §22.1.4 are refused by name with `P0001`
    for call in ["let e: u4 = logic(\"10xx\");", "let g = matches_bits(5_u4, \"01x1\");"] {
        let (src, d) = diagnostics("", call);
        assert!(!d.is_empty() && codes_of(&d).iter().all(|code| code == "P0001"), "{src}: {:?}", codes_of(&d));
    }
}

#[test]
fn width_is_typed_u32_on_a_concrete_type() {
    // §2: `T::WIDTH` is a const `u32` (US-02 AC 3).
    let t = local_types(MODE, "let a = bool::WIDTH; let b = u8::WIDTH; let c = i4::WIDTH; let d = f64::WIDTH; let e = Mode::WIDTH;", &["a", "b", "c", "d", "e"]);
    assert_eq!(t, vec![Ty::UInt(32); 5]);
}


#[test]
fn the_intrinsics_are_typed_at_the_concrete_type() {
    // §22.1.3 and §22.1.4: each function is typed at the concrete type of its arguments, its
    // turbofish or the type the context expects (US-03).
    // `cast` is an `Option<B>` (§22.1.3): `None` when `B` rejects the levels
    let body = "let b = same(3_u8, 4_u8); let c: Option<u2> = cast::<Mode, u2>(Mode::Run); let d = cast::<u8, i8>(3_u8); let e: Option<Mode> = cast(2_u2);";
    let t = local_types(MODE, body, &["b", "c", "d", "e"]);
    assert_eq!(t[0], Ty::Bool);
    assert_eq!(t[1], Ty::Option(Box::new(Ty::UInt(2))));
    assert_eq!(t[2], Ty::Option(Box::new(Ty::Int(8))));
    assert!(matches!(&t[3], Ty::Option(inner) if matches!(**inner, Ty::Enum(_))), "{:?}", t[3]);
}

#[test]
fn a_cast_into_a_sequence_that_holds_an_enum_is_refused() {
    // GAPS L58: a tag that names no variant is judged for an enum and a struct, not yet inside a tuple or an array
    let (src, d) = diagnostics(MODE, "let a = cast::<u4, (Mode, u2)>(5_u4);");
    assert_eq!(codes_of(&d), ["P0001"], "{src}");
    let (src, d) = diagnostics(MODE, "let a = cast::<u4, [Mode; 2]>(5_u4);");
    assert_eq!(codes_of(&d), ["P0001"], "{src}");
}

#[test]
fn a_project_function_named_like_an_intrinsic_is_an_ordinary_function() {
    // §22 rule 3: a project's own `merge` is not intercepted; it is typed from its signature.
    let t = local_types("fn merge(a: f64, b: f64) -> f64 { a + b }", "let a = merge(1.0, 2.0);", &["a"]);
    assert_eq!(t, [Ty::F64]);
}

#[test]
fn cast_between_widths_is_e0701() {
    // §22.1.3 rule 3: a width mismatch is `E0701`, naming both types (US-03 AC 8).
    let (src, d) = diagnostics(MODE, "let a: u8 = cast::<Mode, u8>(Mode::Run);");
    assert_eq!(codes_of(&d), ["E0701"]);
    let message = &d[0].diagnostic.message;
    assert!(message.contains("Mode") && message.contains("u8"), "both types are named: {message}\n{src}");
}

/// A stage reading `c` in `analog` through the function `f`, over the electrical nature.
fn carrier_read_source(function: &str, call: &str) -> String {
    format!("{function}\nmodule Amp(x: f64, p: Electrical) {{ }}\nanalog for Amp {{ v(p) +> x; }}\nmodule Top(in c: Bit<f64>, p: Electrical) {{ }}\nanalog for Top {{ {call} }}")
}

#[test]
fn a_function_that_reads_its_parameter_takes_a_carrier_in_analog() {
    // §3.3.6, EU-01 AC 1: `analog` hands a function a discrete carrier whatever the function does with it: a bare
    // use of the parameter, a forwarded one, a recursive one
    for function in [
        "fn twice(x: f64) -> f64 { x * 2.0 }",
        "fn forwards(x: f64) -> f64 { twice(x) }\nfn twice(x: f64) -> f64 { x * 2.0 }",
        "fn spin(x: f64) -> bool { spin(x) }",
    ] {
        let name = function.split('(').next().unwrap().rsplit(' ').next().unwrap().to_string();
        let src = carrier_read_source(function, &format!("let a = {name}(c);"));
        let design = design_of(&src);
        Types::check(&design, "Top").unwrap_or_else(|d| panic!("{function}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
    }
}

use checked_source::CheckedSource;

#[test]
fn the_types_that_have_a_default() {
    // §3.3.4, H.8 rules 4 and 7 (EU-05 AC 1): `()`, `Option<T>` for every `T`, a tuple or an array of types that
    // have it, and a struct that derives it have `Default`; `T::default()` is typed `T`
    let items = "#[derive(Default)]\nstruct Inner { held: Option<u8>, mark: () }\n\
                 #[derive(Default)]\nstruct Outer { inner: Inner, pair: (Option<bool>, ()), row: [Option<u8>; 3], none: Option<f64> }";
    let body = "let outer = Outer::default(); let inner = Inner::default(); let held: Option<u8> = Option::<u8>::default();";
    let src = CheckedSource::source(items, body);
    CheckedSource::assert_clean(&src);
    assert_eq!(CheckedSource::binding_or_parameter_type(&src, "outer"), Ty::Struct(CheckedSource::type_decl(&src, "Outer")), "`T::default()` has the type `T`");
}

#[test]
fn deriving_default_over_a_number_is_e0187() {
    // §3.3.4, H.8 rule 4 (EU-05 AC 2): a derive over a field of a type without `Default` is `E0187` naming the
    // field and its type, for each of the types the rule excludes
    for (field, ty) in [("count", "u8"), ("level", "i16"), ("up", "bool"), ("gain", "f64"), ("name", "str"), ("taps", "[u8; 2]")] {
        let items = format!("#[derive(Default)]\nstruct Holder {{ {field}: {ty} }}");
        let found = CheckedSource::outcome(&CheckedSource::source(&items, "let h = Holder::default();"));
        assert_eq!(found.len(), 1, "{ty}: {found:?}");
        assert_eq!(found[0].0, "E0187", "{ty}: {found:?}");
        assert!(found[0].1.contains(&format!("`{field}`")) && found[0].1.contains(&format!("`{ty}`")), "{ty}: the field and its type are named: {}", found[0].1);
    }
}

#[test]
fn no_primitive_numeric_type_has_a_default() {
    // §3.3.4, H.8 rule 4 (EU-05 AC 3): `u8::default()`, `bool::default()`, `f64::default()` and `str::default()`
    // are `E0187` naming the type
    for ty in ["u8", "i16", "bool", "f64", "str"] {
        let found = CheckedSource::outcome(&CheckedSource::source("", &format!("let x = {ty}::default();")));
        assert_eq!(found.len(), 1, "{ty}: {found:?}");
        assert_eq!(found[0].0, "E0187", "{ty}: {found:?}");
        assert!(found[0].1.contains(&format!("`{ty}`")), "{ty}: the type is named: {}", found[0].1);
    }
}

#[test]
fn an_in_port_may_carry_an_initial_value() {
    // §3.3.4, §23.1 rule 9 (EU-04): a top-level `in` port starts at its written value, like an `out` port or a signal
    let src = "module Top(in en: Bit = false, in data: Bit<u8> = 100, out ready: Bit = true) { }";
    let design = Design::builder().source("top.msl", src).build();
    assert!(design.diagnostics().is_empty(), "{:?}", design.diagnostics());
    Types::check(&design, "Top").unwrap_or_else(|d| panic!("{:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
}

/// The `(code, message)` of each diagnostic of checking `Top` of `src` over the embedded standard library.
fn top_outcome(src: &str) -> Vec<(String, String)> {
    match Types::check(&design_of(src), "Top") {
        Ok(_) => Vec::new(),
        Err(found) => found.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect(),
    }
}

#[test]
fn a_missing_initial_value_is_e0187_naming_the_carrier() {
    // §3.3.4, EU-04 AC 4 (C14): a signal or an `out` port with no initial value, whose type has no `Default`, is `E0187`
    // at the declaration naming the carrier and `Default`, once per declaration
    for (src, name) in [
        ("module Top() { signal en: Bit; }", "en"),
        ("module Top(out q: Bit) { }", "q"),
        ("module Top() { signal count: Bit<u8>; }", "count"),
        ("module Top() { signal gain: Bit<f64>; signal also: Bit<f64>; }", "gain"),
    ] {
        let found = top_outcome(src);
        assert_eq!(found.first().map(|(code, _)| code.as_str()), Some("E0187"), "{src}: {found:?}");
        assert!(found[0].1.contains(&format!("`{name}`")) && found[0].1.contains("`Default`"), "{src}: names the carrier and `Default`: {}", found[0].1);
        assert_eq!(found.len(), src.matches(": Bit").count(), "{src}: one error per declaration: {found:?}");
    }
    // a type that prints as `enum `Mode`` elsewhere is named bare, with no backtick inside the backticks
    let found = top_outcome("#[derive(Repr)] enum Mode { Idle, Run }\nmodule Top() { signal m: Bit<Mode>; }");
    assert_eq!(found.first().map(|(code, _)| code.as_str()), Some("E0187"), "{found:?}");
    assert!(found[0].1.contains("its type `Mode` does not implement `Default`"), "the bare type name: {}", found[0].1);
    // the carriers that have a value: written, or a type with `Default`
    let clean = "#[derive(Default, Repr)] struct Pair { a: Option<u8>, b: Option<bool> }\n\
                 module Top(in clk: Bit, out q: Bit = false) { signal en: Bit = false; signal last: Bit<Option<u8>>; signal both: Bit<Pair>; }";
    assert!(top_outcome(clean).is_empty(), "{:?}", top_outcome(clean));
}

#[test]
fn an_undriven_carrier_without_default_is_e0187() {
    // §5.2.8, EU-04 AC 5 (C15): a one-driver carrier nothing drives holds its initial value in every analysis; with none
    // written and no `Default`, it is `E0187` naming the carrier
    let found = top_outcome("module Top(in a: Bit<u8>) { signal idle: Bit<u8>; }");
    assert_eq!(found.iter().map(|(code, _)| code.as_str()).collect::<Vec<_>>(), ["E0187"], "{found:?}");
    assert!(found[0].1.contains("`idle`"), "{}", found[0].1);
    // an `in` port is its caller's carrier, not this module's
    assert!(top_outcome("module Top(in a: Bit<u8>) { }").is_empty());
}

#[test]
fn a_definition_or_a_child_out_port_specifies_the_value() {
    // §3.3.4, EU-04 AC 7 (C92): the one driver gives the value: an undelayed definition, or the `out` port of a child
    // the carrier is handed to; a delayed definition does not
    let undelayed = "module Top(in d: Bit) { signal s: Bit; s = d; }";
    assert!(top_outcome(undelayed).is_empty(), "{:?}", top_outcome(undelayed));
    let child = "module Child(in a: Bit, out q: Bit = false) { }\nmodule Top(in a: Bit) { signal s: Bit; let c = Child(a, s); }";
    assert!(top_outcome(child).is_empty(), "{:?}", top_outcome(child));
    let delayed = top_outcome("module Top(in d: Bit) { signal s: Bit; s = d after 1n; }");
    assert_eq!(delayed.iter().map(|(code, _)| code.as_str()).collect::<Vec<_>>(), ["E0187"], "{delayed:?}");
    // the child's own `out` port is a declaration of its own: it needs its value or a `Default`
    let bare_child = top_outcome("module Child(in a: Bit, out q: Bit) { }\nmodule Top(in a: Bit) { signal s: Bit; let c = Child(a, s); }");
    assert_eq!(bare_child.iter().map(|(code, _)| code.as_str()).collect::<Vec<_>>(), ["E0187"], "{bare_child:?}");
}
