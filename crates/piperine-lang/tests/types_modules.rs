//! Modules, params, nodes and placements of `piperine_lang::types` (TYP-03): params of `f64` and
//! `bool` with their defaults and `where`, ports and signals as nodes of a conservative nature,
//! `gnd`, node initial values, and the checks on a placement (count, nature, param values). Every
//! source declares its own natures: the support design has an empty standard library.

#[path = "support/pom.rs"]
mod support;

use piperine_lang::pom::{DeclId, Design, DesignDiagnostic, ExprId, Reflect};
use piperine_lang::types::{Callee, Ty, Types};

const NATURES: &str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n\
nature T { potential: Tp { access: temp, units: \"K\", abstol: 1m }, flow: P { access: pwr, units: \"W\", abstol: 1n } }\n\
module Resistor(p: E, n: E) { r: f64; }\n";

fn design_of(src: &str) -> Design {
    let design = support::build(src);
    assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}\n{src}", support::codes(&design));
    design
}

/// The diagnostics of checking `Top` in `NATURES` followed by `body`, and the source.
fn diagnostics(body: &str) -> (String, Vec<DesignDiagnostic>) {
    let src = format!("{NATURES}{body}");
    let design = design_of(&src);
    let err = Types::check(&design, "Top").expect_err("expected diagnostics");
    (src, err)
}

fn codes_of(diagnostics: &[DesignDiagnostic]) -> Vec<String> {
    diagnostics.iter().map(|d| d.diagnostic.code.to_string()).collect()
}

fn snippet<'a>(src: &'a str, d: &DesignDiagnostic) -> &'a str {
    &src[d.diagnostic.span().range()]
}

/// The expression of the design whose source text is exactly `text` (the first one).
fn expr_of(design: &Design, src: &str, text: &str) -> ExprId {
    (0..design.expr_count()).map(|i| ExprId(i as u32)).find(|e| &src[design.expr_node(*e).span.range()] == text).unwrap_or_else(|| panic!("no expression `{text}`"))
}

#[test]
fn params_are_f64_or_bool() {
    let src = format!("{NATURES}module Top() {{ a: f64 = 1.0; b: bool = true; }}");
    let design = design_of(&src);
    let types = Types::check(&design, "Top").expect("f64 and bool params are in the subset");
    assert_eq!(types.ty(expr_of(&design, &src, "1.0")), Some(&Ty::F64));
    assert_eq!(types.ty(expr_of(&design, &src, "true")), Some(&Ty::Bool));

    // Any other type is outside the subset.
    for ty in ["u32", "Option<f64>", "(f64, f64)"] {
        let (src, d) = diagnostics(&format!("module Top() {{ a: {ty}; }}"));
        assert_eq!(codes_of(&d), ["P0001"], "{ty}");
        assert!(snippet(&src, &d[0]).starts_with(ty.split('<').next().unwrap_or(ty)), "{ty}: {}", snippet(&src, &d[0]));
    }
}

#[test]
fn a_default_of_the_wrong_type_is_rejected() {
    let (src, d) = diagnostics("module Top() { a: f64 = true; }");
    assert_eq!(codes_of(&d), ["E0180"]);
    assert_eq!(snippet(&src, &d[0]), "true");
    let message = &d[0].diagnostic.message;
    assert!(message.contains("f64") && message.contains("bool"), "{message}");

    let (src, d) = diagnostics("module Top() { a: bool = 1.0; }");
    assert_eq!(snippet(&src, &d[0]), "1.0");
}

#[test]
fn a_where_that_is_not_bool_is_e0318() {
    let (src, d) = diagnostics("module Top() { a: f64 = 1.0 where a + 1.0; }");
    assert_eq!(codes_of(&d), ["E0318"]);
    assert_eq!(snippet(&src, &d[0]), "a + 1.0");

    // A `bool` condition checks.
    let src = format!("{NATURES}module Top() {{ a: f64 = 1.0 where a > 0.0; }}");
    let design = design_of(&src);
    Types::check(&design, "Top").expect("a bool where");
}

#[test]
fn ports_and_signals_are_nodes_and_gnd_fits_any() {
    let src = format!("{NATURES}module Top(a: E, b: T) {{ signal s: E; let r1 = Resistor {{ r: 1.0 }}(a, gnd); let r2 = Resistor {{ r: 1.0 }}(s, gnd); }}");
    let design = design_of(&src);
    let types = Types::check(&design, "Top").expect("ports, signals and gnd are nodes");
    let nature = design.find("E").expect("nature E").id();
    assert_eq!(types.ty(expr_of(&design, &src, "a")), Some(&Ty::Node(nature)));
    assert_eq!(types.ty(expr_of(&design, &src, "s")), Some(&Ty::Node(nature)));
    assert_eq!(types.ty(expr_of(&design, &src, "gnd")), Some(&Ty::AnyNode));
    // `gnd` fits a node of the other nature as well.
    let src = format!("{NATURES}module Top(b: T) {{ let r1 = Resistor {{ r: 1.0 }}(gnd, gnd); }}");
    let design = design_of(&src);
    Types::check(&design, "Top").expect("gnd of any nature");

    // A discrete nature is a carrier, not a node: its port takes a direction (`E0310`, §8.1.5; the
    // carriers of a project's nature are `types_natures.rs`). A value input of another type is
    // refused by name.
    let (_, d) = diagnostics("nature Bit2 { value: bool }\nmodule Top(a: Bit2) { }");
    assert_eq!(codes_of(&d), ["E0310"]);
    let (_, d) = diagnostics("module Top(a: &str) { }");
    assert_eq!(codes_of(&d), ["P0001"]);
}

#[test]
fn a_node_init_that_is_not_f64_is_e0210() {
    let (src, d) = diagnostics("module Top() { signal s: E = true; }");
    assert_eq!(codes_of(&d), ["E0210"]);
    assert_eq!(snippet(&src, &d[0]), "true");

    let src = format!("{NATURES}module Top() {{ signal s: E = 0.5; }}");
    let design = design_of(&src);
    Types::check(&design, "Top").expect("an f64 initial value");
}

#[test]
fn a_placement_checks_nodes_count_and_params() {
    let src = format!("{NATURES}module Top(a: E, b: E) {{ let r1 = Resistor {{ r: 1k }}(a, b); }}");
    let design = design_of(&src);
    let types = Types::check(&design, "Top").expect("a well-formed placement");
    let placement = expr_of(&design, &src, "Resistor { r: 1k }(a, b)");
    let resistor: DeclId = design.module("Resistor").expect("Resistor").decl().id();
    assert_eq!(types.callee(placement), Some(Callee::Module(resistor)));
    assert_eq!(types.ty(placement), Some(&Ty::Module(resistor)));
    assert!(types.reached().contains(&resistor));

    // A param of the wrong type: at the value.
    let (src, d) = diagnostics("module Top(a: E, b: E) { let r1 = Resistor { r: true }(a, b); }");
    assert_eq!(codes_of(&d), ["E0180"]);
    assert_eq!(snippet(&src, &d[0]), "true");

    // Too few and too many connections are `E0312`, at the placement.
    for args in ["a", "a, b, b"] {
        let (src, d) = diagnostics(&format!("module Top(a: E, b: E) {{ let r1 = Resistor {{ r: 1k }}({args}); }}"));
        assert_eq!(codes_of(&d), ["E0312"], "{args}");
        assert!(snippet(&src, &d[0]).starts_with("Resistor"), "{}", snippet(&src, &d[0]));
    }

    // A param the module does not have, and one written twice, are `E0320`: the POM reports
    // them, and a design with diagnostics is given back as it is.
    for block in ["{ q: 1.0 }", "{ r: 1.0, r: 2.0 }"] {
        let src = format!("{NATURES}module Top(a: E, b: E) {{ let r1 = Resistor {block}(a, b); }}");
        let design = support::build(&src);
        assert_eq!(support::codes(&design), ["E0320"], "{block}");
        let err = Types::check(&design, "Top").expect_err("a design with diagnostics");
        assert_eq!(codes_of(&err), ["E0320"], "{block}");
    }
}

#[test]
fn a_node_of_another_nature_is_e0251() {
    let (src, d) = diagnostics("module Top(a: E, t: T) { let r1 = Resistor { r: 1.0 }(a, t); }");
    assert_eq!(codes_of(&d), ["E0251"]);
    assert_eq!(snippet(&src, &d[0]), "t");

    // A value where a node is wanted is `E0315` (§8.1.11), proved by
    // `an_expression_on_a_carrier_port_that_is_not_in_is_e0315`; no longer a plain mismatch.
    let (src, d) = diagnostics("module Top(a: E) { let r1 = Resistor { r: 1.0 }(a, 1.0); }");
    assert_eq!(codes_of(&d), ["E0315"]);
    assert_eq!(snippet(&src, &d[0]), "1.0");
}

#[test]
fn a_where_on_a_module_or_fn_is_bool() {
    // Part III §8 errors: the `where` after the port list that is not a `bool` const expression.
    let (src, d) = diagnostics("module Top() where 1.0 { }");
    assert_eq!(codes_of(&d), ["E0304"]);
    assert_eq!(snippet(&src, &d[0]), "1.0");

    // Part I §3 rule 8: a `fn` `where` is a boolean const expression over const generics and
    // associated consts only; M1 has neither, so a read of a value parameter is outside it.
    for cond in ["x", "x > 0.0"] {
        let (src, d) = diagnostics(&format!("fn f(x: f64) -> f64 where {cond} {{ x }}\nmodule Top() {{ }}\nanalog for Top {{ let y = f(1.0); }}"));
        assert_eq!(codes_of(&d), ["P0001"], "{cond}");
        assert_eq!(snippet(&src, &d[0]), "x", "{cond}");
    }
    // A `fn` `where` that is not `bool` is a plain mismatch (the specification gives it no code).
    let (src, d) = diagnostics("fn f(x: f64) -> f64 where 1.0 { x }\nmodule Top() { }\nanalog for Top { let y = f(1.0); }");
    assert_eq!(codes_of(&d), ["E0180"]);
    assert_eq!(snippet(&src, &d[0]), "1.0");

    // A `bool` const condition on each checks.
    let src = format!("{NATURES}fn f(x: f64) -> f64 where true {{ x }}\nmodule Top() where true {{ }}\nanalog for Top {{ let y = f(1.0); }}");
    let design = design_of(&src);
    Types::check(&design, "Top").expect("bool where clauses");
}

#[test]
fn a_core_primitive_is_checked_where_it_is_written() {
    // §14 "Allowed in": `ac_stim` is `analog` only, so a module body is refused.
    // `ddt` and `limit` are core modules: a placement in a const context is `E0352` (§8.3.4).
    for (body, text, code) in [
        ("module Top() { let x = ac_stim(1.0); }", "ac_stim(1.0)", "E0450"),
        ("module Top() { a: f64 = ddt(1.0); }", "ddt(1.0)", "E0352"),
        ("fn l(n: f64, o: f64) -> f64 { n }\nmodule Top() { a: f64 = limit(1.0, l); }", "limit(1.0, l)", "E0352"),
        ("module Top() { a: f64 = 1.0 where ac_stim(1.0) > 0.0; }", "ac_stim(1.0)", "E0450"),
        ("module Top() where ddt(1.0) > 0.0 { }", "ddt(1.0)", "E0352"),
    ] {
        let (src, d) = diagnostics(body);
        assert_eq!(codes_of(&d), [code], "{body}");
        assert_eq!(snippet(&src, &d[0]), text, "{body}");
    }

    // A probe is not allowed in a param default either.
    let (src, d) = diagnostics("module Top(a: E) { x: f64 = v(a); }");
    assert_eq!(codes_of(&d), ["E0450"]);
    assert_eq!(snippet(&src, &d[0]), "v(a)");

    // Where the table allows them they check: `ddt` and a probe in a module-body `let`.
    let src = format!("{NATURES}module Top(a: E) {{ let x = ddt(1.0); let y = v(a); }}");
    let design = design_of(&src);
    Types::check(&design, "Top").expect("ddt and a probe in a module-body let");
}

#[test]
fn an_environment_function_in_a_const_context_is_e0323() {
    for (body, text) in [
        ("module Top() { t: f64 = temperature(); }", "temperature()"),
        ("module Top() { a: f64 = 1.0 where temperature() > 0.0; }", "temperature()"),
        ("module Top() where gmin() > 0.0 { }", "gmin()"),
        ("const T0: f64 = source_scale();\nmodule Top() { a: f64 = T0; }", "source_scale()"),
    ] {
        let (src, d) = diagnostics(body);
        assert_eq!(codes_of(&d), ["E0323"], "{body}");
        assert_eq!(snippet(&src, &d[0]), text, "{body}");
    }

    // A run-time position takes it: a module-body `let` and an `analog` block.
    let src = format!("{NATURES}module Top() {{ let x = temperature(); }}\nanalog for Top {{ let y = temperature(); }}");
    let design = design_of(&src);
    Types::check(&design, "Top").expect("the environment at run time");
}

#[test]
fn a_where_that_reads_a_later_param_is_e0318() {
    // FIX-03 AC 1: whatever the two values, a `where` over a param declared after it is E0318.
    for d_value in ["0.0", "5.0"] {
        let body = format!("module Top() {{ c: f64 = 1.0 where c > d; d: f64 = {d_value}; }}");
        let (src, d) = diagnostics(&body);
        assert_eq!(codes_of(&d), ["E0318"], "d = {d_value}");
        assert_eq!(snippet(&src, &d[0]), "d", "d = {d_value}");
        assert!(d[0].diagnostic.message.contains("the param `d`"), "names the later param: {}", d[0].diagnostic.message);
    }
}

#[test]
fn a_where_over_its_own_and_earlier_params_is_accepted() {
    // FIX-03 AC 2.
    let src = format!("{NATURES}module Top() {{ a: f64 = 1.0; c: f64 = 2.0 where c > a && c > 0.0; }}");
    let design = design_of(&src);
    Types::check(&design, "Top").expect("a where over its own and earlier params");
}

#[test]
fn an_expression_on_a_carrier_port_that_is_not_in_is_e0315() {
    // §8.1.11 (`E0315`): an expression connected to a node port, which is no `in` port of a
    // discrete nature, names the port; a node and `gnd` stay accepted.
    for args in ["2.0 * v(a), gnd", "a, 1.0", "true, gnd"] {
        let (src, d) = diagnostics(&format!("module Top(a: E) {{ let r1 = Resistor {{ r: 1k }}({args}); }}\nanalog for Top {{ }}"));
        assert_eq!(codes_of(&d), ["E0315"], "{args}");
        assert!(d[0].diagnostic.message.contains("`p`") || d[0].diagnostic.message.contains("`n`"), "{}", d[0].diagnostic.message);
        assert!(["2.0 * v(a)", "1.0", "true"].contains(&snippet(&src, &d[0])), "{}", snippet(&src, &d[0]));
    }
}

#[test]
fn an_instance_without_a_result_used_as_a_value_is_e0333() {
    // §8.2.14 (`E0333`): the name of an instance of a module without a result read as a value
    // names the instance.
    let (src, d) = diagnostics("module Top(a: E, b: E) { let r1 = Resistor { r: 1k }(a, b); let q = r1; }");
    assert_eq!(codes_of(&d), ["E0333"], "{:?}", d.iter().map(|x| &x.diagnostic.message).collect::<Vec<_>>());
    assert_eq!(snippet(&src, &d[0]), "r1");
    assert!(d[0].diagnostic.message.contains("`r1`"), "{}", d[0].diagnostic.message);
}

/// Two modules with one param each, and a top with a param of module type `Cell` (PI-04).
const CELLS: &str = "module Cell(a: E) { cs: f64 = 1.0; }\nmodule Other(a: E) { cs: f64 = 1.0; }\n";

#[test]
fn a_module_typed_param_takes_a_template() {
    // PI-04 AC 1, §8.4.4: a param of module type has a template `Cell { cs: 30.0 }` as its default
    // and as its binding, and `cell(x)` places it, its ports checked like `Cell(x)`
    let src = format!("{NATURES}{CELLS}module Top(x: E) {{ cell: Cell = Cell {{ cs: 30.0 }}; let c1 = cell(x); }}\nmodule Parent(y: E) {{ let t = Top {{ cell: Cell {{ cs: 10.0 }} }}(y); }}");
    let design = design_of(&src);
    let types = Types::check(&design, "Top").expect("a param of module type, placed through");
    let cell = types.ty(expr_of(&design, &src, "Cell { cs: 30.0 }")).cloned();
    assert!(matches!(cell, Some(Ty::Module(_))), "the template is typed as its module: {cell:?}");
    // a wrong connection through the param is a port error, as for `Cell(..)`
    let (_, d) = diagnostics(&format!("{CELLS}module Top(x: E, y: E) {{ cell: Cell = Cell {{ cs: 30.0 }}; let c1 = cell(x, y); }}"));
    assert_eq!(codes_of(&d), ["E0312"], "the count of connections is the module's");
}

#[test]
fn a_param_block_on_a_placement_through_a_param_is_e0361() {
    // PI-04 AC 3, §8.4.4: the params belong to the template, so `cell(x) { cs: 2.0 }` is E0361
    let (src, d) = diagnostics(&format!("{CELLS}module Top(x: E) {{ cell: Cell = Cell {{ cs: 30.0 }}; let c1 = cell(x) {{ cs: 2.0 }}; }}"));
    assert_eq!(codes_of(&d), ["E0361"]);
    assert!(snippet(&src, &d[0]).starts_with("cell(x)"), "{}", snippet(&src, &d[0]));
}

#[test]
fn a_template_of_another_module_is_e0180_naming_both() {
    // PI-04 AC 5, Q2 (a): `Other { .. }` bound to a param of type `Cell` names both modules
    let (_, d) = diagnostics(&format!("{CELLS}module Top(x: E) {{ cell: Cell = Other {{ cs: 1.0 }}; let c1 = cell(x); }}"));
    assert_eq!(codes_of(&d), ["E0180"]);
    let message = &d[0].diagnostic.message;
    assert!(message.contains("Cell") && message.contains("Other"), "{message}");
}
