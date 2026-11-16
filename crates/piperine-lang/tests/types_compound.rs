//! Compound values of `piperine_lang::types` (feature `msl-digital`, DIG-03, DIG-06 AC 8): unit
//! enums and their variants (part I §2 rule 5), `Option`, tuple `let`s, `match` and `if let`.
//! The positive cases are the buck's own enums and the stdlib's `Severity`, read as written.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{DeclId, DeclKind, Design, DesignDiagnostic, Reflect};
use piperine_lang::types::{Ty, Types};

/// A design of `Top` with `body` as its analog block and `items` before it, over an empty std.
fn source(items: &str, body: &str) -> String {
    format!("{items}\nmodule Top() {{ }}\nanalog for Top {{ {body} }}")
}

fn design_of(src: &str) -> Design {
    let design = support::build(src);
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", support::codes(&design));
    design
}

/// The same over the embedded standard library.
fn design_with_std(src: &str) -> Design {
    let design = Design::builder().source("top.msl", src).build();
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", support::codes(&design));
    design
}

fn checked<'d>(design: &'d Design, src: &str) -> Types<'d> {
    Types::check(design, "Top").unwrap_or_else(|d| panic!("expected a clean check: {:?}\n{src}", d.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>()))
}

fn diagnostics(items: &str, body: &str) -> (String, Vec<DesignDiagnostic>) {
    let src = source(items, body);
    let design = design_of(&src);
    let err = Types::check(&design, "Top").expect_err("expected diagnostics");
    (src, err)
}

fn codes(d: &[DesignDiagnostic]) -> Vec<String> {
    d.iter().map(|d| d.diagnostic.code.to_string()).collect()
}

fn snippet<'a>(src: &'a str, d: &DesignDiagnostic) -> &'a str {
    &src[d.diagnostic.span().range()]
}

fn decl(design: &Design, name: &str, enum_: bool) -> DeclId {
    design
        .decls()
        .into_iter()
        .find(|d| d.name() == Some(name) && (if enum_ { matches!(d.kind(), DeclKind::Enum(_)) } else { matches!(d.kind(), DeclKind::Binding(_)) }))
        .unwrap_or_else(|| panic!("no `{name}`"))
        .id()
}

/// The text of `item` (an `enum` or a `fn`) in `examples/buck/src/<file>`, its doc and attribute
/// lines included, through its closing brace.
fn buck_item(file: &str, head: &str) -> String {
    let path = format!("{}/../../examples/buck/src/{file}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let at = text.find(head).unwrap_or_else(|| panic!("no `{head}` in {file}"));
    let start = text[..at].rfind("#[derive").filter(|d| !text[*d..at].contains('}')).unwrap_or(at);
    let mut depth = 0usize;
    for (i, c) in text[at..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return text[start..at + i + 1].to_string();
                }
            }
            _ => {}
        }
    }
    panic!("`{head}` in {file} does not close")
}

#[test]
fn unit_enums_and_their_variants_are_typed() {
    // DIG-03 AC 1: an enum of unit variants deriving `Repr`, its variant paths, and `==`/`!=`
    // when it derives `PartialEq` (part I §2 rule 5). `Mode` and `Fault` as the buck writes them.
    let items = format!("{}\n{}", buck_item("supervisor.msl", "pub enum Mode"), buck_item("supervisor.msl", "pub enum Fault"));
    let body = "let m = Mode::Regulate; let f = Fault::Clear; let same = m == Mode::Off; let other = f != Fault::OverVoltage;";
    let src = source(&items, body);
    let design = design_of(&src);
    let types = checked(&design, &src);
    let (mode, fault) = (decl(&design, "Mode", true), decl(&design, "Fault", true));
    let got: Vec<Ty> = ["m", "f", "same", "other"].iter().map(|n| types.local(decl(&design, n, false)).cloned().expect("typed")).collect();
    assert_eq!(got, [Ty::Enum(mode), Ty::Enum(fault), Ty::Bool, Ty::Bool]);
    assert_eq!(Ty::Enum(mode).name(&design), "enum `Mode`");

    // `Severity` of the stdlib, through the prelude.
    let src = "module Top() { }\nanalog for Top { let s = Severity::Warning; let w = s == Severity::Error; }";
    let design = design_with_std(src);
    let types = checked(&design, src);
    let severity = design.decls().into_iter().find(|d| d.name() == Some("Severity") && matches!(d.kind(), DeclKind::Enum(_))).expect("std Severity").id();
    assert_eq!(types.local(decl(&design, "s", false)), Some(&Ty::Enum(severity)));
    assert_eq!(types.local(decl(&design, "w", false)), Some(&Ty::Bool));

    // Two enums do not compare, and one without `PartialEq` does not compare at all.
    let (src, d) = diagnostics(&items, "let x = Mode::Off == Fault::Clear;");
    assert_eq!(codes(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "Fault::Clear");
    let (_, d) = diagnostics("#[derive(Repr)]\nenum E { A, B }", "let x = E::A == E::B;");
    assert_eq!(codes(&d), ["P0002"]);
    assert!(d[0].diagnostic.message.contains("PartialEq"), "{}", d[0].diagnostic.message);
}

#[test]
fn an_enum_outside_the_subset_is_p0001() {
    // DIG-03 AC 2: a variant with a payload, an explicit discriminant or `#[repr]` is `P0001`
    // naming the form; so is an enum that does not derive `Repr`.
    for (items, names) in [
        ("#[derive(Repr, PartialEq)]\nenum E { A(f64), B }", "payload"),
        ("#[derive(Repr, PartialEq)]\nenum E { A { x: f64 }, B }", "payload"),
        ("#[derive(Repr, PartialEq)]\nenum E { A = 3, B }", "discriminant"),
        ("#[derive(Repr, PartialEq)]\n#[repr(u8)]\nenum E { A, B }", "#[repr]"),
        ("#[derive(PartialEq)]\nenum E { A, B }", "#[derive(Repr)]"),
    ] {
        let (src, d) = diagnostics(items, "let x = E::B;");
        assert_eq!(codes(&d), ["P0001"], "{src}");
        assert!(d[0].diagnostic.message.contains(names), "{src}: {}", d[0].diagnostic.message);
        assert_eq!(snippet(&src, &d[0]), "E::B", "{src}");
    }
}
