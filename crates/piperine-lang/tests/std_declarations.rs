//! Scope: which declaration forms the standard library can be written in today (code-clarity T100, CC-11 AC 1;
//! design.md "What `std` must be able to write"). Each test declares one form in a `std`-like crate (the prelude of a
//! standard library written in the test, `StdCrate`) and either checks it clean or asserts today's refusal by its code;
//! the refused forms are the ones a later task implements as a general form, named on each test. The last test records
//! the field types of §19 rule 11 and checks each is a `Repr` type or `str`. The last three are the language items of
//! T101 (CC-11 AC 2): each found in the embedded `std` at its path, a model's own function shadowing one, and a `std`
//! that lacks one the compiler needs (`P0004`).

use piperine_lang::elab::Elaborator;
use piperine_lang::pom::{Design, LanguageItem};
use piperine_lang_fixtures::checked::CheckedSource;
use piperine_lang_fixtures::std_crate::StdCrate;

/// The nature every module source of this file ports through.
const NATURE: &str = "pub nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n";

/// The model every source of this file hangs its use on.
const TOP: &str = "module Top() {}\n";

/// What elaborating `Top` of a `std`-like crate refuses, as codes.
struct StdElaboration;

impl StdElaboration {
    /// The codes of the elaboration of `Top`, empty when it elaborates.
    fn codes(source: &StdCrate) -> Vec<String> {
        let design = source.design();
        match Elaborator::new(&design).run("Top") {
            Ok(_) => Vec::new(),
            Err(found) => found.iter().map(|d| d.diagnostic.code.to_string()).collect(),
        }
    }
}

#[test]
fn an_inherent_impl_on_a_primitive_type_checks_in_std() {
    // checks today: `impl f64 { fn double(self) -> f64 }` in std, called as a method from the root crate.
    let source = StdCrate::declaring("impl f64 { pub fn double(self) -> f64 { self + self } }", &format!("{TOP}analog for Top {{ let x = 1.0.double(); }}"));
    assert_eq!(source.findings(), Vec::<(String, String)>::new());
    assert_eq!(StdElaboration::codes(&source), Vec::<String>::new());
}

#[test]
fn an_extern_intrinsic_method_is_refused_p0001() {
    // missing: `extern "intrinsic"` on a method is `P0001`; T112 declares the methods of the scalars that way.
    let source = StdCrate::declaring("impl f64 { pub extern \"intrinsic\" fn root(self) -> f64; }", &format!("{TOP}analog for Top {{ let x = 4.0.root(); }}"));
    assert_eq!(source.codes(), ["P0001"]);
    assert!(source.findings()[0].1.contains("`extern` function"), "{:?}", source.findings());
}

#[test]
fn an_extern_intrinsic_function_declares_in_std_and_only_there_but_a_call_is_refused_p0001() {
    // checks today: the declaration, accepted in std and `E0125` elsewhere. missing: a call of one is `P0001` "`extern`
    // function" (the real std's five are language items, found by their `std` path since T101; T105 declares
    // the functions of §14 and gives every intrinsic call its lowering).
    let declared = StdCrate::declaring("pub extern \"intrinsic\" fn root(x: f64) -> f64;", TOP);
    assert_eq!(declared.codes(), Vec::<String>::new());
    let called = StdCrate::declaring("pub extern \"intrinsic\" fn root(x: f64) -> f64;", &format!("{TOP}analog for Top {{ let x = root(4.0); }}"));
    assert_eq!(called.codes(), ["P0001"]);
    assert!(called.findings()[0].1.contains("`extern` function"), "{:?}", called.findings());
    let in_root = StdCrate::declaring("", &format!("extern \"intrinsic\" fn root(x: f64) -> f64;\n{TOP}"));
    assert_eq!(in_root.codes(), ["E0125"]);
}

#[test]
fn an_extern_intrinsic_module_is_refused_p0001_where_it_is_used() {
    // missing: the declaration alone parses and resolves; placing the module is `P0001`. T107 declares `ddt`, `delay`,
    // `limit`, `noise`, `cross` and `at` as `extern "intrinsic"` modules.
    let declared = StdCrate::declaring(&format!("{NATURE}pub extern \"intrinsic\" module Probe(p: E) {{}}"), "module Top(a: E) {}");
    assert_eq!(declared.codes(), Vec::<String>::new());
    let placed = StdCrate::declaring(&format!("{NATURE}pub extern \"intrinsic\" module Probe(p: E) {{}}"), "module Top(a: E) { let probe = Probe(a); }");
    assert_eq!(placed.codes(), ["P0001"]);
    assert!(placed.findings()[0].1.contains("`extern` module"), "{:?}", placed.findings());
    let plain = StdCrate::declaring(&format!("{NATURE}pub module Probe(p: E) {{}}"), "module Top(a: E) { let probe = Probe(a); }");
    assert_eq!(plain.codes(), Vec::<String>::new(), "the same module without `extern` places");
}

#[test]
fn a_trait_with_default_type_parameters_declares_but_an_omitted_argument_is_refused_p0001() {
    // checks today: the declaration, and an `impl` that writes both arguments. missing: an `impl` that omits them
    // (`impl BitAnd for Logic<N>`, the form `std/logic.msl` writes) is `P0001` "`trait` item used as a type"; T103
    // declares the six operator traits with their defaults and makes the omission mean the default.
    let trait_item = "pub trait Plus<Rhs = Self, Out = Self> { fn plus(self, rhs: Rhs) -> Out; }\n";
    let declared = StdCrate::declaring(trait_item, TOP);
    assert_eq!(declared.codes(), Vec::<String>::new());
    let explicit = StdCrate::declaring(&format!("{trait_item}impl Plus<u8, u8> for u8 {{ fn plus(self, rhs: u8) -> u8 {{ self }} }}"), &format!("{TOP}analog for Top {{ let x = 1u8.plus(2u8); }}"));
    assert_eq!(explicit.findings(), Vec::<(String, String)>::new());
    let omitted = StdCrate::declaring(&format!("{trait_item}impl Plus for u8 {{ fn plus(self, rhs: u8) -> u8 {{ self }} }}"), &format!("{TOP}analog for Top {{ let x = 1u8.plus(2u8); }}"));
    assert_eq!(omitted.codes(), ["P0001"]);
    assert!(omitted.findings()[0].1.contains("`trait` item used as a type"), "{:?}", omitted.findings());
}

#[test]
fn a_generic_function_with_an_fn_bound_checks_but_a_generic_method_is_refused_e0180() {
    // checks today: a free function `fn apply<F: Fn(f64) -> f64>`, called with a closure. missing: the same bound on a
    // method of an inherent `impl` is `E0180` ("expected `F`, found `fn(f64) -> f64`") and the closure's parameter is
    // `E0181` on a user type's method; a generic method with no bound at all is `E0180` too, so the gap is the method's
    // own generics. T113 declares the 14 methods typed from a closure (`map`, `filter`, `fold`, ...) this way.
    let free = StdCrate::declaring("pub fn apply<F: Fn(f64) -> f64>(x: f64, f: F) -> f64 { f(x) }", &format!("{TOP}analog for Top {{ let x = apply(1.0, |y| y + 1.0); }}"));
    assert_eq!(free.findings(), Vec::<(String, String)>::new());
    let method = StdCrate::declaring("impl f64 { pub fn apply<F: Fn(f64) -> f64>(self, f: F) -> f64 { f(self) } }", &format!("{TOP}analog for Top {{ let g = |y: f64| y + 1.0; let x = 1.0.apply(g); }}"));
    assert_eq!(method.codes(), ["E0180"]);
    assert!(method.findings()[0].1.contains("expected `F`"), "{:?}", method.findings());
    let on_a_struct = StdCrate::declaring("pub struct Cell { a: f64 }\nimpl Cell { pub fn apply<F: Fn(f64) -> f64>(self, f: F) -> f64 { f(self.a) } }", &format!("{TOP}analog for Top {{ let c = Cell {{ a: 1.0 }}; let x = c.apply(|y| y + 1.0); }}"));
    assert_eq!(on_a_struct.codes(), ["E0181"]);
    let unbounded = StdCrate::declaring("impl f64 { pub fn pick<T>(self, t: T) -> T { t } }", &format!("{TOP}analog for Top {{ let x = 1.0.pick(2u8); }}"));
    assert_eq!(unbounded.codes(), ["E0180"]);
}

#[test]
fn a_trait_bound_the_compiler_satisfies_checks_and_refuses_by_e0187() {
    // checks today: `T: Repr` is satisfied by a type with `Repr` and refused for `str` (`E0187`), the compiler deciding
    // both. `BitAnd` is not yet a trait a bound can name (`E0161`): T103 declares it.
    let declared = "pub fn carried<T: Repr>(x: T) -> bool { true }";
    let satisfied = StdCrate::declaring(declared, &format!("{TOP}analog for Top {{ let x = carried(1u8); }}"));
    assert_eq!(satisfied.findings(), Vec::<(String, String)>::new());
    let refused = StdCrate::declaring(declared, &format!("{TOP}analog for Top {{ let x = carried(\"text\"); }}"));
    assert_eq!(refused.codes(), ["E0187"]);
    let operator = StdCrate::declaring("pub fn both<T: BitAnd>(a: T, b: T) -> T { a }", &format!("{TOP}analog for Top {{ let x = both(1u8, 2u8); }}"));
    assert_eq!(operator.codes()[0], "E0161");
}

/// The kinds of value type of part I §1 (the table before rule 1), each with a type that spells it and the `Repr` it has.
struct ValueKind {
    /// The kind's name in the table.
    name: &'static str,
    /// A type of the kind.
    type_text: &'static str,
    /// Whether a type of the kind implements `Repr` today (§1 rule 4, rule 8).
    carried: bool,
}

impl ValueKind {
    /// Every kind a field expression of `emit!` may have (§19 rule 11): the five rows of the §1 table, the composites one by one.
    fn all() -> Vec<ValueKind> {
        vec![
            ValueKind { name: "boolean", type_text: "bool", carried: true },
            ValueKind { name: "integer", type_text: "u8", carried: true },
            ValueKind { name: "signed integer", type_text: "i16", carried: true },
            ValueKind { name: "float", type_text: "f64", carried: true },
            ValueKind { name: "string", type_text: "str", carried: false },
            ValueKind { name: "unit", type_text: "()", carried: false },
            ValueKind { name: "tuple", type_text: "(u8, bool)", carried: true },
            ValueKind { name: "array", type_text: "[u8; 3]", carried: true },
            ValueKind { name: "struct", type_text: "Pair", carried: true },
            ValueKind { name: "enum", type_text: "Mode", carried: true },
            ValueKind { name: "option", type_text: "Option<u8>", carried: true },
        ]
    }

    /// The items the `type_text` of some kinds names.
    fn items() -> &'static str {
        "#[derive(Repr)] struct Pair { a: u8, b: bool }\n#[derive(Repr)] enum Mode { Off, On }\n"
    }

    /// The carried check: `Bit<type_text>` is the type of an `in` port, which only a type with `Repr` may be.
    fn carried_findings(&self) -> Vec<String> {
        let src = format!("{}module Top(in p: Bit<{}>) {{ }}\n", Self::items(), self.type_text);
        CheckedSource::standard_outcome(&src).into_iter().map(|(code, _)| code).collect()
    }

    /// The field check: a function of this type's parameter that writes it as the field of an `emit!`.
    fn field_findings(&self) -> Vec<String> {
        let src = format!("{}fn note(x: {}) {{ emit!(\"trace\", field = x); }}\nmodule Top() {{ }}\n", Self::items(), self.type_text);
        CheckedSource::standard_outcome(&src).into_iter().map(|(code, _)| code).collect()
    }
}

#[test]
fn section_19_rule_11_field_types_are_repr_types_or_str() {
    // §19 rule 11: "A field expression has a value type (§1)": bool, the integers, the floats, `str`, `()`, tuples,
    // arrays, structs, enums and `Option<T>` (`Event`, `Analysis`, closures and templates are values without a field
    // type, §1 rule 9). Today every one is accepted as an `emit!` field and every one but two implements `Repr` (an `in`
    // port of type `Bit<type_text>` checks): `str`, which the spec excludes (§1 rule 8), and `()`, which the §2 table has no row
    // for and which is `E0112` today. `RecordValue` (T116) is implemented for `Repr` types and `str`, so `()` is the
    // one field type outside it: T116 either gives `()` a `Repr` of width 0 or refuses it as a field.
    let mut without_repr = Vec::new();
    for kind in ValueKind::all() {
        let carried = kind.carried_findings();
        if kind.carried {
            assert!(carried.is_empty(), "{} (`{}`) must be a Repr type: {carried:?}", kind.name, kind.type_text);
        } else {
            assert_eq!(carried, ["E0112"], "{} (`{}`) must be one without Repr", kind.name, kind.type_text);
            without_repr.push(kind.type_text);
        }
        assert_eq!(kind.field_findings(), Vec::<String>::new(), "{} (`{}`) must be accepted as an `emit!` field", kind.name, kind.type_text);
    }
    assert_eq!(without_repr, ["str", "()"], "`str` is outside `Repr` by the spec, `()` by a gap");
}

#[test]
fn every_language_item_is_found_in_the_embedded_std() {
    // T101: each item the compiler gives meaning to is a declaration of the embedded `std`, at its path.
    let design = Design::builder().source("top.msl", TOP).build();
    for item in LanguageItem::ALL {
        let declaration = design.language_items().declaration(item).unwrap_or_else(|| panic!("`std::{}` is not declared", item.path()));
        assert_eq!(design.std_crate().find(item.path()).map(|found| found.id()), Some(declaration), "{item:?}");
        assert_eq!(design.language_items().item(declaration), Some(item));
    }
}

#[test]
fn a_shadowed_intrinsic_is_the_models_own_function() {
    // T101: the model's own `parse_hex` is another declaration than `std::io::parse_hex`, so a call of it is an
    // ordinary call; the std one, called where no model item shadows it, is still the intrinsic (its contract refuses a
    // result type that is not written).
    let intrinsic = "pub const extern \"intrinsic\" fn parse_hex<T, const N: usize>(text: str) -> [T; N];\n";
    let shadowed = StdCrate::declaring("pub use crate::io::*;\n", &format!("fn parse_hex(x: f64) -> f64 {{ x + x }}\n{TOP}analog for Top {{ let y = parse_hex(2.0); }}"))
        .with_namespace("io", intrinsic);
    assert_eq!(shadowed.findings(), Vec::<(String, String)>::new());
    assert_eq!(StdElaboration::codes(&shadowed), Vec::<String>::new());
    let design = shadowed.design();
    let own = design.root_crate().find("parse_hex").expect("the model's own function").id();
    assert_eq!(design.language_items().item(own), None);
    assert_ne!(design.language_items().declaration(LanguageItem::ParseHex), Some(own));
    let unshadowed = StdCrate::declaring("pub use crate::io::*;\n", &format!("{TOP}analog for Top {{ let y = parse_hex(\"0a\"); }}")).with_namespace("io", intrinsic);
    assert_eq!(unshadowed.codes(), ["P0001"]);
    assert!(unshadowed.findings()[0].1.contains("`parse_hex` whose result type is not written"), "{:?}", unshadowed.findings());
}

#[test]
fn a_std_without_a_language_item_is_an_internal_error() {
    // T101: a `std` that declares `values::logic` but no `logic::Logic` lacks a language item the call of `logic`
    // needs: `P0004`, a defect of that standard library, never a fallback.
    let values = "pub struct Logic<const N: usize> { pub value: UInt<N> }\npub extern \"intrinsic\" fn logic<const N: usize>(pattern: str) -> Logic<N>;\n";
    let source = StdCrate::declaring("pub use crate::values::*;\n", &format!("{TOP}analog for Top {{ let x = logic(\"01\"); }}")).with_namespace("values", values);
    assert_eq!(source.codes(), ["P0004"]);
    assert!(source.findings()[0].1.contains("`std::logic::Logic`"), "{:?}", source.findings());
}
