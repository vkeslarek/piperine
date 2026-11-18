//! The checker's side of `unknown-semantics` (part I §3.3, §22.1): `x.level(k)` and `T::WIDTH`
//! typed on `Repr` values, the five stdlib functions of §22.1 typed by their contract at the
//! concrete type (`E0701`, `E0702`), and the flag-total functions `analog` may read a carrier
//! through (§3.3.6). One positive and one negative source per rule, over the embedded standard
//! library (the five functions are the stdlib's own items).

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


#[test]
fn the_intrinsics_are_typed_at_the_concrete_type() {
    // §22.1.2 to §22.1.4: each function is typed at the concrete type of its arguments, its
    // turbofish or the type the context expects (US-03).
    let body = "let a = merge(1.0, 2.0); let b = same(3_u8, 4_u8); let c: u2 = cast::<Mode, u2>(Mode::Run); \
                let d = cast::<u8, i8>(3_u8); let e: u4 = logic(\"10xx\"); let f = logic::<Mode>(\"x1\"); \
                let g = matches_bits(5_u4, \"01x1\"); let h = merge(Mode::Idle, Mode::Halt);";
    let t = local_types(MODE, body, &["a", "b", "c", "d", "e", "f", "g", "h"]);
    let mode = t[5].clone();
    assert!(matches!(mode, Ty::Enum(_)), "logic::<Mode> is a Mode: {mode:?}");
    assert_eq!(t[0], Ty::F64);
    assert_eq!(t[1], Ty::Bool);
    assert_eq!(t[2], Ty::UInt(2));
    assert_eq!(t[3], Ty::Int(8));
    assert_eq!(t[4], Ty::UInt(4));
    assert_eq!(t[6], Ty::Bool);
    assert_eq!(t[7], mode);
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

#[test]
fn a_bad_pattern_is_e0702() {
    // §22.1.4 rule 3: a character other than `0`, `1`, `x`, or a length other than WIDTH, is
    // `E0702` at the pattern (US-03 AC 9).
    for (call, pattern) in [("logic::<u4>(\"10z0\")", "\"10z0\""), ("logic::<u4>(\"101\")", "\"101\""), ("matches_bits(3_u4, \"01x10\")", "\"01x10\"")] {
        let (src, d) = diagnostics("", &format!("let a = {call};"));
        assert_eq!(codes_of(&d), ["E0702"], "{call}");
        assert_eq!(&src[d[0].diagnostic.span().range()], pattern, "the error sits at the pattern");
    }
}

/// A stage reading `c` in `analog` through the function `f`, over the electrical nature.
fn flag_total_source(function: &str, call: &str) -> String {
    format!("{function}\nmodule Amp(x: f64, p: Electrical) {{ }}\nanalog for Amp {{ v(p) +> x; }}\nmodule Top(in c: Bit<f64>, p: Electrical) {{ }}\nanalog for Top {{ {call} }}")
}

#[test]
fn a_flag_total_function_may_take_a_carrier_in_analog() {
    // §3.3.6: every use of the parameter passes through `level`, directly, through `known_or`,
    // or as the argument of another flag-total function: `analog` may pass it a carrier.
    for function in [
        "fn low(x: f64) -> bool { x.level(0).is_some() }",
        "fn known_sum(x: f64) -> f64 { x.known_or(0.0) + x.known_or(1.0) }",
        "fn through(x: f64) -> bool { low(x) }\nfn low(x: f64) -> bool { x.level(0).is_some() }",
        "fn spin(x: f64) -> bool { spin(x) }",
    ] {
        let name = function.split('(').next().unwrap().rsplit(' ').next().unwrap().to_string();
        let src = flag_total_source(function, &format!("let a = {name}(c);"));
        let design = design_of(&src);
        Types::check(&design, "Top").unwrap_or_else(|d| panic!("{function}: {:?}", d.iter().map(|d| &d.diagnostic.message).collect::<Vec<_>>()));
    }
}

#[test]
fn a_function_that_reads_its_parameter_bare_is_not_flag_total() {
    // §3.3.6 and §3.3.5: one bare use of the parameter, beside a `level` read or on its own, keeps
    // the call `E0150`, naming the carrier at the argument.
    for function in [
        "fn twice(x: f64) -> f64 { x * 2.0 }",
        "fn mixed(x: f64) -> f64 { if x.level(0).is_some() { x } else { 0.0 } }",
        "fn forwards(x: f64) -> f64 { twice(x) }\nfn twice(x: f64) -> f64 { x * 2.0 }",
    ] {
        let name = function.split('(').next().unwrap().rsplit(' ').next().unwrap().to_string();
        let src = flag_total_source(function, &format!("let a = {name}(c);"));
        let (src, d) = {
            let design = design_of(&src);
            (src.clone(), Types::check(&design, "Top").expect_err("a bare read is refused"))
        };
        assert_eq!(codes_of(&d), ["E0150"], "{function}");
        assert_eq!(&src[d[0].diagnostic.span().range()], "c", "{function}");
        assert!(d[0].diagnostic.message.contains("`c`"), "the carrier is named: {}", d[0].diagnostic.message);
    }
}
