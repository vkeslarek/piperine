//! Scope: which declaration forms the standard library can be written in today (code-clarity T100, CC-11 AC 1;
//! design.md "What `std` must be able to write"). Each test declares one form in a `std`-like crate (the prelude of a
//! standard library written in the test, `StdCrate`) and either checks it clean or asserts today's refusal by its code;
//! the refused forms are the ones a later task implements as a general form, named on each test. One test records the
//! field types of §19 rule 11 and checks each is a `Repr` type or `str`. The rest are the ratchet guards of CC-11 AC 1
//! and AC 2 (T102): the builtin table, the string comparisons, Appendix H and the kept special cases, each judged
//! against `allowances/piperine-lang/<guard>.tsv` of the guards crate, with planted violations proving each guard fails. Three more are the language items of
//! T101 (CC-11 AC 2): each found in the embedded `std` at its path, a model's own function shadowing one, and a `std`
//! that lacks one the compiler needs (`P0004`).

use piperine_lang::elab::Elaborator;
use std::path::{Path, PathBuf};

use piperine_lang::pom::{BUILTINS, BuiltinInfo, BuiltinKind, Design, LanguageItem, Reflect};
use piperine_lang_fixtures::checked::CheckedSource;
use piperine_lang_fixtures::std_crate::StdCrate;
use piperine_lang_fixtures::spec_tables::SpecTables;
use piperine_repository_guards::{Allowance, RustSource, SourceTree};
use syn::visit::Visit;

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

// What the guards below judge, one type per finding; each takes its input as an argument, so a test can plant a violation.

/// The primitive types and the two bounds the language keeps without a declaration (design.md S10 and S17): the closed
/// set a builtin row may belong to.
pub struct PrimitiveNames;

impl PrimitiveNames {
    /// The spellings of the closed set; `uN`, `iN` and `fN` stand for each width of the family.
    pub const ALL: [&'static str; 14] = ["uN", "iN", "fN", "bool", "usize", "isize", "UInt", "Int", "Float", "str", "Event", "gnd", "Fn", "Nature"];

    /// Whether `name` is in the closed set.
    pub fn contains(name: &str) -> bool {
        Self::ALL.contains(&name)
    }
}

/// The rows of the builtin table a `std` declaration has not replaced yet.
pub struct BuiltinRows;

impl BuiltinRows {
    /// One key per row of `rows` outside the closed set, as `Kind name`.
    pub fn undeclared(rows: &[BuiltinInfo]) -> Vec<String> {
        rows.iter().filter(|row| !PrimitiveNames::contains(row.name)).map(|row| format!("{:?} {}", row.kind, row.name)).collect()
    }

    /// A row of the kind and name, for a test that plants one.
    pub fn planted(name: &'static str, kind: BuiltinKind) -> BuiltinInfo {
        BuiltinInfo { name, kind, section: "planted" }
    }
}

/// The comparisons of the front end's product sources with a string literal: `x == "name"`, a `match` arm with a literal
/// pattern, a literal inside `matches!`. A test cannot tell an item's name from other text, so every one is counted.
pub struct SpellingComparisons {
    found: usize,
}

impl SpellingComparisons {
    /// The one file allowed to hold the table from a path to a language item.
    const EXEMPT_FILE: &'static str = "crates/piperine-lang/src/pom/language_items.rs";

    /// One key, the file's path, per comparison in each product source of `piperine-lang` of `files`.
    pub fn in_files(files: &[RustSource]) -> Vec<String> {
        let mut keys = Vec::new();
        for file in files.iter().filter(|file| file.crate_name() == "piperine-lang" && !file.is_test() && file.path() != Path::new(Self::EXEMPT_FILE)) {
            let Ok(syntax) = file.syntax() else { continue };
            let mut scan = SpellingComparisons { found: 0 };
            scan.visit_file(syntax);
            keys.extend(std::iter::repeat_n(file.path().display().to_string(), scan.found));
        }
        keys
    }

    /// The same, over the sources of the workspace.
    pub fn in_workspace() -> Vec<String> {
        Self::in_files(SourceTree::of_workspace().files())
    }

    /// Whether `expression` is a string literal, seen through `&`, `*` and parentheses.
    fn is_literal(expression: &syn::Expr) -> bool {
        match expression {
            syn::Expr::Lit(literal) => matches!(literal.lit, syn::Lit::Str(_)),
            syn::Expr::Reference(inner) => Self::is_literal(&inner.expr),
            syn::Expr::Paren(inner) => Self::is_literal(&inner.expr),
            syn::Expr::Unary(inner) => Self::is_literal(&inner.expr),
            _ => false,
        }
    }

    /// Whether `pattern` is, or alternates over, a string literal.
    fn is_literal_pattern(pattern: &syn::Pat) -> bool {
        match pattern {
            syn::Pat::Lit(literal) => matches!(literal.lit, syn::Lit::Str(_)),
            syn::Pat::Or(alternatives) => alternatives.cases.iter().any(Self::is_literal_pattern),
            syn::Pat::Reference(inner) => Self::is_literal_pattern(&inner.pat),
            _ => false,
        }
    }

    /// How many string literals a token stream holds.
    fn literals_in(tokens: proc_macro2::TokenStream) -> usize {
        tokens
            .into_iter()
            .map(|token| match token {
                proc_macro2::TokenTree::Literal(literal) => usize::from(literal.to_string().starts_with('"')),
                proc_macro2::TokenTree::Group(group) => Self::literals_in(group.stream()),
                _ => 0,
            })
            .sum()
    }

    /// Whether an item is written under `#[cfg(test)]`.
    fn is_test_only(attributes: &[syn::Attribute]) -> bool {
        attributes.iter().any(|attribute| attribute.path().is_ident("cfg") && matches!(&attribute.meta, syn::Meta::List(list) if list.tokens.to_string().contains("test")))
    }
}

impl<'ast> Visit<'ast> for SpellingComparisons {
    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if !Self::is_test_only(&module.attrs) {
            syn::visit::visit_item_mod(self, module);
        }
    }

    fn visit_expr_binary(&mut self, binary: &'ast syn::ExprBinary) {
        if matches!(binary.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_)) && (Self::is_literal(&binary.left) || Self::is_literal(&binary.right)) {
            self.found += 1;
        }
        syn::visit::visit_expr_binary(self, binary);
    }

    fn visit_arm(&mut self, arm: &'ast syn::Arm) {
        if Self::is_literal_pattern(&arm.pat) {
            self.found += 1;
        }
        syn::visit::visit_arm(self, arm);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if mac.path.is_ident("matches") {
            self.found += Self::literals_in(mac.tokens.clone());
        }
        syn::visit::visit_macro(self, mac);
    }
}

/// A receiver type of Appendix H and what its inherent `impl` in `std` is written over.
struct Receiver {
    /// The section of Appendix H.
    section: &'static str,
    /// The start of the `impl`'s self type, as written.
    spellings: &'static [&'static str],
}

/// The rows of Appendix H.1 to H.7, the 75 methods a model may call.
pub struct CatalogueRows;

impl CatalogueRows {
    /// Appendix H's sections of methods and the types their `impl` is written over.
    const RECEIVERS: [Receiver; 7] = [
        Receiver { section: "H.1", spellings: &["UInt", "Int", "u", "i"] },
        Receiver { section: "H.2", spellings: &["f64", "Float", "f32"] },
        Receiver { section: "H.3", spellings: &["bool"] },
        Receiver { section: "H.4", spellings: &["Option"] },
        Receiver { section: "H.5", spellings: &["["] },
        Receiver { section: "H.6", spellings: &["Range"] },
        Receiver { section: "H.7", spellings: &["Iterator"] },
    ];

    /// The text of Appendix H.
    pub fn appendix() -> String {
        std::fs::read_to_string(SourceTree::workspace_root().join("docs/spec/appendix_h_library.md")).expect("docs/spec/appendix_h_library.md")
    }

    /// One key, `section::method`, per method row of H.1 to H.7 of `appendix`.
    pub fn rows(appendix: &str) -> Vec<String> {
        let mut keys = Vec::new();
        for receiver in &Self::RECEIVERS {
            for row in SpecTables::table_rows(&SpecTables::section(appendix, &format!("## {} ", receiver.section))) {
                keys.push(format!("{}::{}", receiver.section, row[0].trim_matches('`')));
            }
        }
        keys
    }

    /// The keys of `rows` that no inherent `impl` of the standard library of `design` declares.
    pub fn undeclared(design: &Design, rows: &[String]) -> Vec<String> {
        let declared = Self::declared(design);
        rows.iter().filter(|row| !declared.contains(row)).cloned().collect()
    }

    /// The `section::method` of every method of an `impl` in `std` over a receiver of Appendix H.
    fn declared(design: &Design) -> Vec<String> {
        let Some(standard) = design.krate("std") else { return Vec::new() };
        let mut declared = Vec::new();
        for declaration in standard.decls() {
            let Some(implementation) = declaration.as_impl() else { continue };
            let receiver = implementation.self_ty().text().trim();
            for section in Self::RECEIVERS.iter().filter(|candidate| candidate.spellings.iter().any(|spelling| receiver.starts_with(spelling))) {
                for item in implementation.items() {
                    if let Some(name) = item.as_function().and_then(|function| function.name()) {
                        declared.push(format!("{}::{name}", section.section));
                    }
                }
            }
        }
        declared
    }
}

/// One special case the language keeps, and the id of the design.md case it is.
pub struct KeptCase {
    /// `S10`, `S17`, ...
    pub identifier: &'static str,
    /// What is kept.
    pub subject: &'static str,
}

/// The reasons written in `std` for the cases the language keeps (design.md S10, S17 to S20).
pub struct WrittenReason;

impl WrittenReason {
    /// The words a reason must have at least: a bare name is not a reason.
    const MINIMUM_WORDS: usize = 6;

    /// The kept cases.
    pub fn cases() -> Vec<KeptCase> {
        vec![
            KeptCase { identifier: "S10", subject: "`Fn` and `Nature`" },
            KeptCase { identifier: "S17", subject: "the primitive types and `gnd`" },
            KeptCase { identifier: "S18", subject: "the nature quantity keys" },
            KeptCase { identifier: "S19", subject: "the closed `#[derive]` set" },
            KeptCase { identifier: "S20", subject: "`extern \"intrinsic\"` in `std` only" },
        ]
    }

    /// The directory of the standard library's sources.
    pub fn std_directory() -> PathBuf {
        SourceTree::workspace_root().join("crates/piperine-lang/std")
    }

    /// The text of every file of the standard library.
    pub fn std_texts() -> Vec<String> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(Self::std_directory()).expect("std directory").flatten().map(|entry| entry.path()).collect();
        files.sort();
        files.iter().filter(|path| path.extension().is_some_and(|extension| extension == "msl")).map(|path| std::fs::read_to_string(path).expect("std file")).collect()
    }

    /// The cases of `cases` with no doc comment in `texts` of the form `Kept <id>: <reason>`, the reason of at least six words.
    pub fn without_reason(texts: &[String], cases: &[KeptCase]) -> Vec<String> {
        cases.iter().filter(|case| !texts.iter().any(|text| Self::has_reason(text, case.identifier))).map(|case| format!("{} {}", case.identifier, case.subject)).collect()
    }

    /// Whether `text` holds a doc-comment line `Kept <identifier>: <reason>`.
    fn has_reason(text: &str, identifier: &str) -> bool {
        let marker = format!("Kept {identifier}:");
        text.lines().filter(|line| line.trim_start().starts_with("//!") || line.trim_start().starts_with("///")).any(|line| {
            line.split_once(marker.as_str()).is_some_and(|(_, reason)| reason.split_whitespace().count() >= Self::MINIMUM_WORDS)
        })
    }
}

/// The guards' names: the file of each allowance is `allowances/piperine-lang/<name>.tsv`.
struct GuardName;

impl GuardName {
    /// The builtin rows outside the closed set of primitives.
    const BUILTINS: &'static str = "builtins_not_declared_in_std";
    /// The comparisons with a string literal, by file.
    const SPELLINGS: &'static str = "std_items_found_by_spelling";
    /// The rows of Appendix H with no declaration in `std`.
    const METHODS: &'static str = "appendix_h_methods_not_declared_in_std";
}

/// The allowance of a guard for `piperine-lang`, judged against what was found.
struct LangAllowance;

impl LangAllowance {
    /// Fails when `found` differs from `allowances/piperine-lang/<guard>.tsv`.
    fn assert_clean(guard: &str, found: &[String]) {
        Allowance::read("piperine-lang", guard).judge(found).assert_clean(guard);
    }
}

#[test]
fn every_builtin_is_declared_in_std() {
    // CC-11 AC 1: a name the resolver returns as a builtin is a primitive of the closed set (design.md S10, S17) or a row
    // of the allowance, which each task of phases 15 to 17 shortens and T114 empties.
    LangAllowance::assert_clean(GuardName::BUILTINS, &BuiltinRows::undeclared(BUILTINS));
}

#[test]
fn no_std_item_is_found_by_its_spelling() {
    // CC-11 AC 2: the front end compares no name with a string literal outside `pom/language_items.rs`; today's
    // comparisons are the allowance, by file and count (every comparison with a literal counts: a test cannot tell a
    // name from other text).
    LangAllowance::assert_clean(GuardName::SPELLINGS, &SpellingComparisons::in_workspace());
}

#[test]
fn every_method_of_appendix_h_is_declared_in_std() {
    // CC-11 AC 1: each of the 75 rows of H.1 to H.7 is a method of an inherent `impl` in `std`; today none is, and the
    // allowance lists the rows T112 and T113 declare.
    let rows = CatalogueRows::rows(&CatalogueRows::appendix());
    assert_eq!(rows.len(), 75, "Appendix H holds 75 method rows: {rows:?}");
    LangAllowance::assert_clean(GuardName::METHODS, &CatalogueRows::undeclared(&Design::builder().source("top.msl", "").build(), &rows));
}

#[test]
fn every_kept_special_case_has_its_written_reason() {
    // CC-11 AC 1 ("keep it with a written reason the user reviews"): each case the language keeps is a doc comment in
    // `std` of the form `Kept S<n>: <reason>`, in `std/lib.msl`.
    let missing = WrittenReason::without_reason(&WrittenReason::std_texts(), &WrittenReason::cases());
    assert!(missing.is_empty(), "no written reason in std for: {missing:?}");
}

#[test]
fn a_new_builtin_row_fails_the_guard() {
    // A row outside the closed set that the allowance (today's rows) does not list is unlisted; a primitive row is not a finding.
    let mut rows = BUILTINS.to_vec();
    rows.push(BuiltinRows::planted("Planted", BuiltinKind::Fn));
    rows.push(BuiltinRows::planted("bool", BuiltinKind::Type));
    let today = Allowance::parse("today", &Allowance::render(&BuiltinRows::undeclared(BUILTINS)));
    let verdict = today.judge(&BuiltinRows::undeclared(&rows));
    assert_eq!(verdict.unlisted, ["Fn Planted\t1"]);
    assert!(verdict.stale.is_empty());
}

#[test]
fn a_new_string_comparison_fails_the_guard() {
    // A comparison, a `match` arm and a `matches!` over literals, in a product file of the crate, are three findings in
    // that file; the exempt file and a test module are none.
    let planted = "fn f(n: &str) -> bool { n == \"ddt\" }\nfn g(n: &str) -> u8 { match n { \"at\" => 1, _ => 0 } }\nfn h(n: &str) -> bool { matches!(n, \"now\") }\n#[cfg(test)]\nmod tests { fn t(n: &str) -> bool { n == \"x\" } }\n";
    let tree = SourceTree::of_sources(&[
        ("crates/piperine-lang/src/planted.rs", planted),
        ("crates/piperine-lang/src/pom/language_items.rs", planted),
        ("crates/piperine-ir/src/other_crate.rs", planted),
    ]);
    let found = SpellingComparisons::in_files(tree.files());
    assert_eq!(found, ["crates/piperine-lang/src/planted.rs"; 3]);
    let no_files: Vec<RustSource> = Vec::new();
    assert!(SpellingComparisons::in_files(&no_files).is_empty());
    let verdict = Allowance::read("piperine-lang", GuardName::SPELLINGS).judge(&found);
    assert_eq!(verdict.unlisted, ["crates/piperine-lang/src/planted.rs\t3"]);
}

#[test]
fn a_stale_allowance_row_fails_each_guard() {
    // A listed row that is no longer found is stale: closing a finding deletes its row. With nothing found, every row of
    // each real allowance is stale; with one unlisted finding, that finding is the only unlisted row.
    for guard in [GuardName::BUILTINS, GuardName::SPELLINGS, GuardName::METHODS] {
        let rows = Allowance::read("piperine-lang", guard);
        let nothing_found = rows.judge(&[]);
        assert!(!nothing_found.stale.is_empty(), "{guard}: today's allowance lists rows");
        let one_found = rows.judge(&["a key no allowance lists".to_string()]);
        assert_eq!(one_found.unlisted, ["a key no allowance lists\t1"], "{guard}");
        assert_eq!(one_found.stale, nothing_found.stale, "{guard}");
    }
    let planted = Allowance::parse("planted", "crates/piperine-lang/src/closed.rs\t2\n");
    assert_eq!(planted.judge(&[]).stale, ["crates/piperine-lang/src/closed.rs\t2"]);
}

#[test]
fn a_missing_appendix_h_declaration_is_found() {
    // A standard library with an `impl f64 { fn sqrt }` declares `H.2::sqrt` and nothing else of the catalogue.
    let design = Design::builder().std_source("lib.msl", "pub mod prelude;\n").std_source("prelude.msl", "impl f64 { pub fn sqrt(self) -> f64 { self } }").source("top.msl", "").build();
    let rows = CatalogueRows::rows(&CatalogueRows::appendix());
    let undeclared = CatalogueRows::undeclared(&design, &rows);
    assert_eq!(undeclared.len(), 74);
    assert!(!undeclared.contains(&"H.2::sqrt".to_string()));
    assert!(undeclared.contains(&"H.2::sin".to_string()));
}

#[test]
fn a_kept_case_without_a_reason_is_found() {
    // A bare name, a marker with a short reason and a marker in an ordinary comment are not reasons.
    let texts = vec![
        "//! Kept S10: bounds on kinds of item, not traits a model implements.\n".to_string(),
        "//! Kept S17: because\n".to_string(),
        "// Kept S18: the nature declaration is its own grammar and no function.\n".to_string(),
    ];
    let missing = WrittenReason::without_reason(&texts, &WrittenReason::cases());
    assert_eq!(missing.iter().map(|case| &case[..3]).collect::<Vec<_>>(), ["S17", "S18", "S19", "S20"]);
}
