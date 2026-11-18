//! The checker's side of `unknown-semantics` (part I §3.3, §22.1): `x.level(k)` and `T::WIDTH`
//! typed on `Repr` values, the five stdlib functions of §22.1 typed by their contract at the
//! concrete type (`E0701`, `E0702`), and the flag-total functions `analog` may read a carrier
//! through (§3.3.6). One positive and one negative source per rule.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{DeclId, DeclKind, Design, DesignDiagnostic, Reflect};
use piperine_lang::types::{Ty, Types};

/// A design of `Top` with `body` as its analog block and `items` before it.
fn source(items: &str, body: &str) -> String {
    format!("{items}\nmodule Top() {{ }}\nanalog for Top {{ {body} }}")
}

fn design_of(src: &str) -> Design {
    let design = support::build(src);
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", support::codes(&design));
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
fn level_is_typed_on_every_repr_type() {
    // §22.1: `x.level(k)` of a value with `Repr` is `Option<bool>` (US-02 AC 1).
    let body = "let a = true.level(0); let b = 3_u8.level(0); let c = (-3_i4).level(1); let d = 1.5.level(0); let e = Mode::Run.level(0);";
    let t = local_types(MODE, body, &["a", "b", "c", "d", "e"]);
    assert_eq!(t, vec![Ty::Option(Box::new(Ty::Bool)); 5]);
}

#[test]
fn level_on_a_value_without_repr_is_refused() {
    // A receiver without `Repr` (here an `Option`) has no `level` (§2): refused loudly, never typed.
    let (src, d) = diagnostics("", "let a = Some(1.0).level(0);");
    assert_eq!(codes_of(&d), ["P0001"], "{src}");
}

#[test]
fn width_is_typed_u32_on_a_concrete_type() {
    // §2: `T::WIDTH` is a const `u32` (US-02 AC 3).
    let t = local_types(MODE, "let a = bool::WIDTH; let b = u8::WIDTH; let c = i4::WIDTH; let d = f64::WIDTH; let e = Mode::WIDTH;", &["a", "b", "c", "d", "e"]);
    assert_eq!(t, vec![Ty::UInt(32); 5]);
}

