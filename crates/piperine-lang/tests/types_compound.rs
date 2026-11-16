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

#[test]
fn option_values_and_their_methods_are_typed() {
    // DIG-03 AC 3: `Option<T>`, `Some(e)`, `None` and `is_some`, `is_none`, `unwrap_or`
    // (Appendix H). `None` takes the type its position expects: an annotation, the other arm of
    // an `if`, a parameter.
    let items = buck_item("supervisor.msl", "pub enum Fault");
    let body = "let a = Some(2.5); let b: Option<u8> = None; let c: Option<u8> = Some(7); \
                let f = if true { Some(Fault::OverCurrent) } else if false { Some(Fault::OverVoltage) } else { None }; \
                let g = if true { None } else { Some(1.0) }; \
                let s = a.is_some(); let n = f.is_none(); let u = b.unwrap_or(3); let v = a.unwrap_or(0.0);";
    let src = source(&items, body);
    let design = design_of(&src);
    let types = checked(&design, &src);
    let fault = decl(&design, "Fault", true);
    let opt = |t: Ty| Ty::Option(Box::new(t));
    let got: Vec<Ty> = ["a", "b", "c", "f", "g", "s", "n", "u", "v"].iter().map(|n| types.local(decl(&design, n, false)).cloned().expect("typed")).collect();
    assert_eq!(got, [opt(Ty::F64), opt(Ty::UInt(8)), opt(Ty::UInt(8)), opt(Ty::Enum(fault)), opt(Ty::F64), Ty::Bool, Ty::Bool, Ty::UInt(8), Ty::F64]);
    assert_eq!(opt(Ty::UInt(8)).name(&design), "Option<u8>");

    // `None` with nothing to take a type from, and `unwrap_or` of another type, are mismatches.
    let (src, d) = diagnostics("", "let x = None;");
    assert_eq!(codes(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "None");
    let (src, d) = diagnostics("", "let o: Option<u8> = Some(1); let x = o.unwrap_or(1.0);");
    assert_eq!(codes(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "1.0");
    // A method `Option` has no row for is named with the type.
    let (_, d) = diagnostics("", "let o: Option<u8> = Some(1); let x = o.unwrap();");
    assert_eq!(codes(&d), ["P0001"]);
    assert!(d[0].diagnostic.message.contains("unwrap") && d[0].diagnostic.message.contains("Option<u8>"), "{}", d[0].diagnostic.message);
}

#[test]
fn a_tuple_let_names_each_component() {
    // DIG-03 AC 7: `let (a, b) = (x, y);` names each component, in a function, an `analog` block
    // and a module body. `in_window` as the buck writes it.
    let items = format!("{}\nfn swap(a: u8, b: u8) -> u8 {{ let (x, y) = (b, a); x - y }}", buck_item("supervisor.msl", "pub fn in_window"));
    let body = "let w = in_window(130, 128); let s = swap(1, 2); let (p, q) = (1.5, true);";
    let src = source(&items, body);
    let design = design_of(&src);
    let types = checked(&design, &src);
    let got: Vec<Ty> = ["w", "s", "p", "q", "m", "r", "x", "y"].iter().map(|n| types.local(decl(&design, n, false)).cloned().expect("typed")).collect();
    assert_eq!(got, [Ty::Bool, Ty::UInt(8), Ty::F64, Ty::Bool, Ty::UInt(11), Ty::UInt(11), Ty::UInt(8), Ty::UInt(8)]);

    let src = "module Top() { let (a, b) = (2.0, 3.0); }";
    let design = design_of(src);
    let types = checked(&design, src);
    assert_eq!([types.local(decl(&design, "a", false)), types.local(decl(&design, "b", false))], [Some(&Ty::F64), Some(&Ty::F64)]);

    // A tuple anywhere but the two sides of a `let` is outside the subset; a count that differs
    // is a mismatch.
    for (body, names) in [("let t = (1.0, 2.0);", "tuple expression"), ("let x = (1.0, 2.0); let (a, b) = x;", "tuple expression"), ("let t: (f64, f64) = 1.0;", "tuple type")] {
        let (src, d) = diagnostics("", body);
        assert_eq!(codes(&d)[0], "P0001", "{src}");
        assert!(d[0].diagnostic.message.contains(names), "{src}: {}", d[0].diagnostic.message);
    }
    let (src, d) = diagnostics("", "let (a, b) = (1.0, 2.0, 3.0);");
    assert_eq!(codes(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "(1.0, 2.0, 3.0)");
}

#[test]
fn a_value_for_an_option_parameter_is_some_of_it() {
    // DIG-06 AC 8 (§3.2.3): an argument of type `T` for a parameter of type `Option<T>` is read
    // as `Some` of it; an `Option<T>`, `Some(..)` and `None` are passed as they are.
    let items = "fn pick(td: Option<f64>) -> f64 { td.unwrap_or(0.0) }\nfn count(n: Option<u8>) -> u8 { n.unwrap_or(1) }";
    let body = "let x = 2.0; let o: Option<f64> = None; let a = pick(x); let b = pick(o); let c = pick(None); let d = pick(Some(1.0)); let e = count(5);";
    let src = source(items, body);
    let design = design_of(&src);
    let types = checked(&design, &src);
    let calls: Vec<_> = (0..design.expr_count())
        .map(|i| piperine_lang::pom::ExprId(i as u32))
        .filter(|e| matches!(&design.expr_node(*e).kind, piperine_lang::pom::ExprKind::Call { .. }) && types.callee(*e).is_some())
        .collect();
    let argument = |call: piperine_lang::pom::ExprId| match &design.expr_node(call).kind {
        piperine_lang::pom::ExprKind::Call { args, .. } => args[0].value,
        _ => unreachable!("a call"),
    };
    let read_as_some: Vec<(String, bool)> = calls.iter().map(|c| (src[design.expr_node(argument(*c)).span.range()].to_string(), types.implicit_some(argument(*c)))).collect();
    let expected: Vec<(String, bool)> = [("x", true), ("o", false), ("None", false), ("Some(1.0)", false), ("5", true)].iter().map(|(t, s)| (t.to_string(), *s)).collect();
    assert_eq!(read_as_some, expected);
    assert_eq!(types.ty(argument(calls[4])), Some(&Ty::UInt(8)), "the literal takes the payload's type");

    // A value of another type is a mismatch naming the parameter's type.
    let (src, d) = diagnostics(items, "let a = pick(true);");
    assert_eq!(codes(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "true");
    assert!(d[0].diagnostic.message.contains("Option<f64>"), "{}", d[0].diagnostic.message);
}

/// `Mode` and `Fault` of the buck, and the supervisor's `match` and the setpoint register's
/// `if let` written as functions over values (`supervisor.msl`, `control.msl`).
fn supervisor_items() -> String {
    format!(
        "{}\n{}\nconst SETPOINT_MAX: u8 = 168;\n\
         fn next_mode(m: Mode, retry: u11, fault_now: Option<Fault>, at_target: bool) -> Mode {{\n\
             match m {{\n\
                 Mode::Off => Mode::SoftStart,\n\
                 Mode::Hiccup => if retry == 0 {{ Mode::SoftStart }} else {{ Mode::Hiccup }},\n\
                 Mode::SoftStart | Mode::Regulate => {{\n\
                     if let Some(f) = fault_now {{ Mode::Hiccup }} else if m == Mode::SoftStart && at_target {{ Mode::Regulate }} else {{ m }}\n\
                 }}\n\
             }}\n\
         }}\n\
         fn setpoint_next(data: Option<u8>, code: u8) -> u8 {{ if let Some(d) = data {{ d.min(SETPOINT_MAX) }} else {{ code }} }}",
        buck_item("supervisor.msl", "pub enum Mode"),
        buck_item("supervisor.msl", "pub enum Fault"),
    )
}

#[test]
fn match_is_typed_over_enums_bools_integers_and_options() {
    // DIG-03 AC 4: `match` over an enum, a `bool`, an integer and an `Option`, with path,
    // literal, `Some(p)`, `None`, or- and wildcard patterns, its arms of one type.
    let items = format!("{}\nconst TWO: u2 = 2;", supervisor_items());
    let body = "let m = next_mode(Mode::Off, 0, None, false); let s = setpoint_next(Some(200), 100); \
                let b = match true { true => 1.0, false => 2.0 }; \
                let k: u2 = 1; let i = match k { 0 => 5, 1 | TWO => 6, _ => 7 }; \
                let o: Option<Fault> = None; let fo = match o { Some(Fault::Clear) => 1, Some(_) => 2, None => 3 }; \
                let g = match o { Some(x) => x, None => Fault::Clear }; \
                let w: u9 = 4; let h = match w { 0 => 9, _ => w };";
    let src = source(&items, body);
    let design = design_of(&src);
    let types = checked(&design, &src);
    let (mode, fault) = (decl(&design, "Mode", true), decl(&design, "Fault", true));
    let got: Vec<Ty> = ["m", "s", "b", "i", "fo", "g", "h", "x"].iter().map(|n| types.local(decl(&design, n, false)).cloned().expect("typed")).collect();
    assert_eq!(got, [Ty::Enum(mode), Ty::UInt(8), Ty::F64, Ty::Int(32), Ty::Int(32), Ty::Enum(fault), Ty::UInt(9), Ty::Enum(fault)]);

    // DIG-01 AC 2, the `match` arm position: a bare literal arm takes the type of the first arm
    // that is not one (`9` is a `u9`, written before `w`).
    let lit = (0..design.expr_count())
        .map(|i| piperine_lang::pom::ExprId(i as u32))
        .find(|e| matches!(&design.expr_node(*e).kind, piperine_lang::pom::ExprKind::Lit(_)) && &src[design.expr_node(*e).span.range()] == "9")
        .expect("the literal 9");
    assert_eq!(types.literal(lit), Some(&Ty::UInt(9)));

    // §1 rule 14: a name that resolves to a constant is a path pattern and binds nothing; a name
    // that resolves to nothing would bind, which a `match` arm of the subset does not do.
    let (src, d) = diagnostics("", "let k: u2 = 1; let i = match k { x => 1.0 };");
    assert_eq!(codes(&d), ["P0001"], "{src}");
    assert_eq!(snippet(&src, &d[0]), "x");

    // Arms of different types, and a pattern of another type, are mismatches.
    let (src, d) = diagnostics("", "let b = match true { true => 1.0, false => false };");
    assert_eq!(codes(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "false");
    let (src, d) = diagnostics(&supervisor_items(), "let m = Mode::Off; let x = match m { Fault::Clear => 1.0, _ => 2.0 };");
    assert_eq!(codes(&d), ["P0002"]);
    assert_eq!(snippet(&src, &d[0]), "Fault::Clear");
}

#[test]
fn a_match_that_is_not_exhaustive_is_p0002() {
    // DIG-03 AC 5: a `match` that does not cover every value of its scrutinee is `P0002`.
    for body in [
        "let m = Mode::Off; let x = match m { Mode::Off => 1.0, Mode::SoftStart => 2.0, Mode::Regulate => 3.0 };",
        "let x = match true { true => 1.0 };",
        "let k: u9 = 1; let x = match k { 0 => 1.0, 1 => 2.0 };",
        "let o: Option<u8> = None; let x = match o { Some(1) => 1.0, None => 2.0 };",
        "let o: Option<u8> = None; let x = match o { Some(v) => 1.0 };",
    ] {
        let (src, d) = diagnostics(&supervisor_items(), body);
        assert_eq!(codes(&d), ["P0002"], "{src}");
        assert!(snippet(&src, &d[0]).starts_with("match "), "{src}: {}", snippet(&src, &d[0]));
    }
    // Every value, and only then, passes: all four variants; every value of a `u2` by literals.
    let ok = source(&supervisor_items(), "let m = Mode::Off; let x = match m { Mode::Off => 1.0, Mode::SoftStart => 2.0, Mode::Regulate => 3.0, Mode::Hiccup => 4.0 }; \
                                         let k: u2 = 1; let y = match k { 0 => 1.0, 1 => 2.0, 2 | 3 => 3.0 };");
    checked(&design_of(&ok), &ok);
}

#[test]
fn if_let_some_binds_the_payload() {
    // DIG-03 AC 6: `if let Some(x) = e` binds `x` to the payload in the first block; the
    // setpoint register's `if let` as `setpoint_next` writes it.
    let src = source(&supervisor_items(), "let s = setpoint_next(None, 100); let o: Option<f64> = Some(1.5); let y = if let Some(v) = o { v * 2.0 } else { 0.0 };");
    let design = design_of(&src);
    let types = checked(&design, &src);
    let got: Vec<Ty> = ["s", "y", "v", "d"].iter().map(|n| types.local(decl(&design, n, false)).cloned().expect("typed")).collect();
    assert_eq!(got, [Ty::UInt(8), Ty::F64, Ty::F64, Ty::UInt(8)]);

    // The value must be an `Option`; the pattern must be `Some(p)`.
    let (src, d) = diagnostics("", "let x = 1.0; let y = if let Some(v) = x { v } else { 0.0 };");
    assert_eq!(codes(&d)[0], "P0002", "{src}");
    assert_eq!(snippet(&src, &d[0]), "x");
    let (src, d) = diagnostics("", "let o: Option<f64> = None; let y = if let None = o { 1.0 } else { 0.0 };");
    assert_eq!(codes(&d), ["P0001"], "{src}");
}
