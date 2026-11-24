//! Scope: constants and params of `piperine_lang::elab` (ELAB-02): `ConstEval` folds the
//! subset (a `const` item once, IEEE 754 throughout, a param read only through its binding);
//! the bound values, defaults and `where` conditions of placements join in T3 to T5.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_ir::{Constant, Expr, Real, ValueType};
use piperine_lang::elab::{Bindings, ConstEval, ConstFault};
use piperine_lang::pom::{Design, ExprId, Reflect};
use piperine_lang::types::Types;
use support::Elab;

const SOURCE: &str = "
const A: f64 = K_B / Q;
const B: f64 = A * A + A;
fn square(x: f64) -> f64 { let y = x * x; y }
const C: f64 = square(3.0) + 2.0_f64.sqrt();
const NAN_FREE: bool = 1.0 / 0.0 > 1.0e308 && !(2.0 < 1.0);
module Top() {
    p: f64 = B;
    q: f64 = B * 2.0;
    r: f64 = C;
    s: bool = NAN_FREE;
    t: f64 = 1.0 / 0.0;
}
";

/// The design and its checked types: every case below evaluates one expression of `SOURCE`.
struct Case {
    design: Design,
}

impl Case {
    fn new() -> Case {
        let design = Elab::design(SOURCE);
        assert!(design.diagnostics().is_empty(), "{:?}", Elab::codes(design.diagnostics()));
        Case { design }
    }

    /// The default of the param `name` of `Top`.
    fn default_of(&self, name: &str) -> ExprId {
        let param = self.design.module("Top").expect("Top").param(name).expect("a param of that name");
        param.default().expect("a default").id()
    }
}

fn real(value: f64) -> Constant {
    Constant::Real(Real::new(value))
}

#[test]
fn a_const_read_is_folded_once() {
    let case = Case::new();
    let types = Types::check(&case.design, "Top").expect("the source is in the subset");
    let mut eval = ConstEval::new(&case.design, &types);
    assert_eq!(eval.evaluations(), 0);

    // `B` reads `A` twice and `A` reads `K_B` and `Q`: four items, each folded once.
    let (p, q) = (case.default_of("p"), case.default_of("q"));
    let k_b_over_q = 1.380649e-23_f64 / 1.602176634e-19;
    let b = k_b_over_q * k_b_over_q + k_b_over_q;
    assert_eq!(eval.value(p, &Bindings::new()), Ok(real(b)), "bit-exact against Rust's own f64 arithmetic");
    assert_eq!(eval.value(q, &Bindings::new()), Ok(real(b * 2.0)));
    assert_eq!(eval.evaluations(), 4, "K_B, Q, A and B, once each however many reads");
    assert_eq!(eval.value(p, &Bindings::new()), Ok(real(b)));
    assert_eq!(eval.evaluations(), 4, "a third read folds nothing");
}

#[test]
fn functions_methods_and_logic_fold() {
    let case = Case::new();
    let types = Types::check(&case.design, "Top").expect("the source is in the subset");
    let mut eval = ConstEval::new(&case.design, &types);
    assert_eq!(eval.value(case.default_of("r"), &Bindings::new()), Ok(real(9.0 + 2.0_f64.sqrt())));
    assert_eq!(eval.value(case.default_of("s"), &Bindings::new()), Ok(Constant::Bool(true)));
}

#[test]
fn division_by_zero_is_inf_not_an_error() {
    let case = Case::new();
    let types = Types::check(&case.design, "Top").expect("the source is in the subset");
    let mut eval = ConstEval::new(&case.design, &types);
    assert_eq!(eval.value(case.default_of("t"), &Bindings::new()), Ok(real(f64::INFINITY)));
}

#[test]
fn a_param_read_with_no_binding_is_not_constant() {
    let design = Elab::design("module Top() { a: f64 = 1.0; b: f64 = a * 2.0; }");
    assert!(design.diagnostics().is_empty(), "{:?}", Elab::codes(design.diagnostics()));
    let types = Types::check(&design, "Top").expect("the source is in the subset");
    let mut eval = ConstEval::new(&design, &types);
    let top = design.module("Top").expect("Top");
    let param = |name: &str| top.param(name).expect("the param").decl().id();
    let default = |name: &str| top.param(name).expect("the param").default().expect("a default").id();

    // Nothing bound: the read of `a` is reported to the caller, naming the param.
    match eval.value(default("b"), &Bindings::new()) {
        Err(ConstFault::UnboundParam { param: read, .. }) => assert_eq!(read, param("a")),
        other => panic!("expected an unbound param, got {other:?}"),
    }
    // Bound: the same expression folds.
    let mut bindings = Bindings::new();
    bindings.bind(param("a"), real(4.0));
    assert_eq!(eval.value(default("b"), &bindings), Ok(real(8.0)));
}

#[test]
fn an_environment_function_is_dynamic() {
    // A placement's param block is not a const context for the type checker, so `temperature()`
    // reaches the evaluator, which reports it as needing a run.
    let design = Elab::design("module Leaf() { x: f64 = 1.0; }\nmodule Top() { let l = Leaf { x: temperature().unwrap_or(1.0) }(); }");
    assert!(design.diagnostics().is_empty(), "{:?}", Elab::codes(design.diagnostics()));
    let types = Types::check(&design, "Top").expect("the source is in the subset");
    let top = design.module("Top").expect("Top");
    let value = top.instances()[0].callee().fields()[0].value().id();
    let mut eval = ConstEval::new(&design, &types);
    assert!(matches!(eval.value(value, &Bindings::new()), Err(ConstFault::Dynamic { .. })));
}

#[test]
fn a_where_is_recorded_over_the_body_params() {
    let ir = Elab::ir("module Top() { r: f64 = 1.0 where r > 0.0; k: f64 = 2.0 where k * r >= 0.0; }", "Top");
    let text = ir.print_structural();
    assert!(text.contains("param r: f64 = 1.0 where (r > 0.0)"), "{text}");
    assert!(text.contains("param k: f64 = 2.0 where ((k * r) >= 0.0)"), "{text}");
    // The condition reads the body's own params, not numbers: a host `set` of `r` reaches it.
    let body = ir.bodies().next().expect("one body");
    let check = body.params()[0].check.expect("a recorded condition");
    assert!(matches!(ir.expr(check), piperine_ir::Expr::Binary { lhs, .. } if matches!(ir.expr(*lhs), piperine_ir::Expr::Param(0))));
}

const LEAF: &str = "module Leaf(p: Electrical) { a: f64 = 3.0; b: f64 = 4.0; }\n";

#[test]
fn an_unset_param_takes_its_default() {
    let src = format!("{LEAF}module Top() {{ signal n: Electrical; let l = Leaf {{ b: 5.0 }}(n); let m = Leaf {{ }}(n); }}");
    let ir = Elab::ir(&src, "Top");
    let values = |path: &str| ir.instance_at(path).expect("the instance").params().iter().map(|c| c.to_string()).collect::<Vec<_>>();
    assert_eq!(values("Top.l"), ["3.0", "5.0"], "`a` takes its default, `b` the given value");
    assert_eq!(values("Top.m"), ["3.0", "4.0"], "both default");
}

#[test]
fn a_required_param_left_out_is_e0319() {
    let src = "module Leaf() { r: f64; }\nmodule Top() { let l = Leaf { }(); }";
    let err = Elab::run(src, "Top").expect_err("`r` has no default");
    assert_eq!(Elab::codes(&err), ["E0319"]);
    assert!(err[0].diagnostic.message.contains("Top.l"), "{}", err[0].diagnostic.message);
    let span = err[0].diagnostic.span();
    assert_eq!(&src[span.start..span.end], "Leaf { }()", "at the placement");

    // The top itself is a placement with no values: a required param of it is just as unbound.
    let src = "module Top() { r: f64; }";
    let err = Elab::run(src, "Top").expect_err("a top with a required param");
    assert_eq!(Elab::codes(&err), ["E0319"]);
}

#[test]
fn a_false_where_is_e0317_with_the_path() {
    let src = "module Leaf() { r: f64 where r > 0.0; }\nmodule Top() { let r1 = Leaf { r: -1.0 }(); }";
    let err = Elab::run(src, "Top").expect_err("-1.0 breaks `r > 0.0`");
    assert_eq!(Elab::codes(&err), ["E0317"]);
    assert!(err[0].diagnostic.message.contains("Top.r1.r"), "names the param's path: {}", err[0].diagnostic.message);
    let span = err[0].diagnostic.span();
    assert_eq!(&src[span.start..span.end], "-1.0", "at the value that broke it");
    // A value inside the condition is accepted.
    Elab::ir("module Leaf() { r: f64 where r > 0.0; }\nmodule Top() { let r1 = Leaf { r: 1.0 }(); }", "Top");
}

#[test]
fn a_bound_value_that_is_not_constant_is_refused() {
    // A value that reads a param of the parent is lowered from msl-digital T32 on (DIG-09 AC 11);
    // the case still refused in its place is a form the constant evaluator does not fold.
    let src = "module Leaf() { r: f64 = 1.0; }\nmodule Top() { k: f64 = 2.0; let a = Leaf { r: (2.0).sin() }(); let b = Leaf { r: temperature().unwrap_or(1.0) }(); }";
    let err = Elab::run(src, "Top").expect_err("neither value is constant");
    assert_eq!(Elab::codes(&err), ["P0001", "E0321"]);
    let text = |i: usize| &src[err[i].diagnostic.span().start..err[i].diagnostic.span().end];
    assert_eq!(text(0), "(2.0).sin()", "a form the evaluator does not fold is P0001");
    assert_eq!(text(1), "temperature()", "anything that needs a run is E0321");
}

#[test]
fn a_phase_a_form_in_a_bound_value_is_evaluated() {
    // CV-36 AC 1 (replaces the `match` this test file pinned as a form the evaluator does not fold):
    // a `match` in a bound value is evaluated at elaboration, and the param of the parent it reads
    // is read as the value of the child (DIG-09 AC 11).
    let src = "module Leaf() { r: f64 = 1.0; }\nmodule Top() { k: f64 = 2.0; let a = Leaf { r: match 1 { _ => k } }(); }";
    Elab::ir(src, "Top");
}

#[test]
fn a_param_read_is_never_folded() {
    // A `const` is its value in the body; a param stays a read, however constant its default.
    let src = "const C: f64 = 2.0;\nmodule Top() { k: f64 = 3.0; }\nanalog for Top { let x = k * 2.0; let y = C * 4.0; }";
    let ir = Elab::ir(src, "Top");
    let text = ir.print_structural();
    assert!(text.contains("let x = (k * 2.0)"), "a param read prints as a read: {text}");
    assert!(text.contains("let y = (2.0 * 4.0)"), "a const read is its value: {text}");
    let body = ir.bodies().next().expect("one body");
    let first = body.analog().expect("an analog block")[0];
    let piperine_ir::Stmt::Let { init, .. } = ir.stmt(first) else { panic!("a `let`") };
    let piperine_ir::Expr::Binary { lhs, .. } = ir.expr(*init) else { panic!("a product") };
    assert_eq!(ir.expr(*lhs), &piperine_ir::Expr::Param(0));
}

/// The designs of the integer, enum and `Option` param tests, and what they place: the owner of the helpers they share.
struct Placed;

impl Placed {
    /// A `Child` with the params `params` read by the `analog` block `uses`, and a `Top` that places it with `binding`.
    fn child_with(params: &str, uses: &str, binding: &str) -> String {
        format!(
            "#[derive(Repr, PartialEq)]\nenum Mode {{ Fast, Slow }}\n\
             module Child(o: Electrical) {{ {params} }}\n\
             analog for Child {{ {uses} }}\n\
             module Top() {{ signal n: Electrical; let k = Child(n) {binding}; }}"
        )
    }

    /// The variant and the parts of the aggregate `constant` of `ir`.
    fn aggregate(ir: &piperine_ir::Ir, constant: &Constant) -> (Option<u32>, Vec<Constant>) {
        match constant {
            Constant::Aggregate(id) => (ir.aggregate(*id).variant, ir.aggregate(*id).parts.clone()),
            other => panic!("expected an aggregate, found {other:?}"),
        }
    }

    /// The IR of `src` and the params of its placed instance `Top.k`.
    fn params(src: &str) -> (piperine_ir::Ir, Vec<Constant>) {
        let ir = Elab::ir(src, "Top");
        let params = ir.instance_at("Top.k").expect("the placed instance").params().iter().copied().collect();
        (ir, params)
    }
}

#[test]
fn an_integer_param_is_bound_and_read() {
    // PI-01 AC 1, AC 2: `n: u8 = 4 where n >= 1` is 4 when nothing binds it and 7 when a placement does.
    let params = "n: u8 = 4 where n >= 1;";
    let uses = "i(o) +> 1.0;";
    assert_eq!(Placed::params(&Placed::child_with(params, uses, "{ }")).1, [Constant::Int(4)]);
    assert_eq!(Placed::params(&Placed::child_with(params, uses, "{ n: 7 }")).1, [Constant::Int(7)]);
    let src = Placed::child_with(params, uses, "{ n: 0 }");
    let err = Elab::run(&src, "Top").expect_err("0 breaks `n >= 1`");
    assert_eq!(Elab::codes(&err), ["E0317"], "the `where` holds for an integer");
}

#[test]
fn n_as_f64_reaches_the_contribution() {
    // PI-01 AC 3: `n as f64` in `analog` is a conversion of the param read, in the body of every instance.
    let ir = Elab::ir(&Placed::child_with("n: u8 = 4;", "i(o) +> n as f64;", "{ n: 7 }"), "Top");
    assert!(ir.expressions().any(|e| matches!(e, Expr::Cast { to: ValueType::F64, .. })), "the conversion is lowered");
    assert_eq!(ir.instance_at("Top.k").expect("instance").params().iter().copied().collect::<Vec<_>>(), [Constant::Int(7)]);
}

#[test]
fn an_integer_default_converts_with_as() {
    // Part I §1 rule 1: an `as` in a default is the one conversion; `m: u16 = 3.0 as u16` folds to 3.
    let params = "m: u16 = 3.0 as u16; w: i8 = 0 - 2;";
    assert_eq!(Placed::params(&Placed::child_with(params, "i(o) +> 1.0;", "{ }")).1, [Constant::Int(3), Constant::Int(-2)]);
}

#[test]
fn an_enum_param_selects_its_arm() {
    // PI-02 AC 1, AC 2: the param holds the bound variant (its index in the enum), and a `match` on it is in the body.
    let params = "md: Mode = Mode::Fast;";
    let uses = "i(o) +> match md { Mode::Fast => 1.0, Mode::Slow => 2.0 };";
    let (ir, defaults) = Placed::params(&Placed::child_with(params, uses, "{ }"));
    let (bound_ir, bound) = Placed::params(&Placed::child_with(params, uses, "{ md: Mode::Slow }"));
    assert_eq!(Placed::aggregate(&ir, &defaults[0]).0, Some(0), "`Fast` is the first variant");
    assert_eq!(Placed::aggregate(&bound_ir, &bound[0]).0, Some(1), "`Slow` is the second");
}

#[test]
fn an_option_param_left_out_is_none() {
    // PI-03 AC 1, explicit-unknowns EU-06 AC 1 (check C20): an `Option<T>` param no instance binds is `None`.
    let (ir, params) = Placed::params(&Placed::child_with("rs: Option<f64>;", "i(o) +> 1.0;", "{ }"));
    assert_eq!(Placed::aggregate(&ir, &params[0]), (None, vec![]), "no part: `None`");
}

#[test]
fn an_unbound_option_param_is_none() {
    // explicit-unknowns C20 (EU-06 AC 1), owed to params-and-io B5 (GAPS L28): the top itself is a placement
    // with no values, so its `Option` param is `None` too.
    let src = "module Top() { rs: Option<f64>; k: Option<u8> = None; }";
    let ir = Elab::ir(src, "Top");
    let params: Vec<Constant> = ir.instance_at("Top").expect("the top instance").params().iter().copied().collect();
    assert_eq!(params.iter().map(|p| Placed::aggregate(&ir, p)).collect::<Vec<_>>(), [(None, vec![]), (None, vec![])]);
}

#[test]
fn an_option_param_bound_to_a_value_is_some() {
    // PI-03 AC 2 (§3.2.3, §3.2.10): `{ rs: 10.0 }`, a plain value bound to an `Option<f64>` param, is `Some(10.0)`, so is `Some(10.0)` written out, and `{ rs: None }` is `None`.
    let params = "rs: Option<f64>;";
    for binding in ["{ rs: 10.0 }", "{ rs: Some(10.0) }"] {
        let (ir, some) = Placed::params(&Placed::child_with(params, "i(o) +> 1.0;", binding));
        assert_eq!(Placed::aggregate(&ir, &some[0]), (None, vec![Constant::Real(Real::new(10.0))]), "{binding}");
    }
    let (ir, none) = Placed::params(&Placed::child_with(params, "i(o) +> 1.0;", "{ rs: None }"));
    assert_eq!(Placed::aggregate(&ir, &none[0]), (None, vec![]));
}

#[test]
fn option_param_methods_read_the_bound_case() {
    // PI-03 AC 3, the shape: `unwrap_or`, `is_some` and `match` over the param elaborate to its two halves, the presence and the
    // payload, read through the param pin. The value of the bound case in each form (`match`, `is_some`, `if let`) is run in
    // `piperine-parity`'s `source_params::every_read_of_an_option_param_gives_the_bound_case`.
    let params = "rs: Option<f64>;";
    let uses = "i(o) +> rs.unwrap_or(1.0) + match rs { Some(x) => x, None => 0.0 } + if rs.is_some() { 1.0 } else { 0.0 };";
    let ir = Elab::ir(&Placed::child_with(params, uses, "{ rs: Some(2.0) }"), "Top");
    let halves = ir.expressions().filter(|e| matches!(e, Expr::OptionPart { .. })).count();
    assert!(halves >= 2, "the `Option` param is read as its two halves, the presence and the payload: {halves}");
    assert!(ir.expressions().any(|e| matches!(e, Expr::Param(0))), "through the param pin");
}

#[test]
fn an_option_param_bound_over_the_parents_param_reads_some_of_it() {
    // PI-03 AC 2 (§3.2.3), F21: a placement binds an `Option<f64>` param to an expression over the parent's params, plain (`Some` of it
    // by §3.2.3) or written as `Some(expr)`: each instance reads `Some` of the parent's value.
    for binding in ["{ rs: rt }", "{ rs: Some(rt) }", "{ rs: rt * 2.0 }"] {
        let src = format!(
            "module Load(p: Electrical) {{ rs: Option<f64>; }}\nanalog for Load {{ i(p, gnd) +> v(p) / rs.unwrap_or(1.0); }}\n\
             module Mid(p: Electrical) {{ rt: f64 = 3.0; let l = Load(p) {binding}; }}\nmodule Top() {{ signal n: Electrical; let m = Mid(n); }}"
        );
        let ir = Elab::ir(&src, "Top");
        let params: Vec<Constant> = ir.instance_at("Top.m.l").expect("the child").params().iter().copied().collect();
        let wanted = if binding.contains("2.0") { 6.0 } else { 3.0 };
        assert_eq!(Placed::aggregate(&ir, &params[0]), (None, vec![Constant::Real(Real::new(wanted))]), "{binding}");
    }
}
