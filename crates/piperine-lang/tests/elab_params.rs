//! Scope: constants and params of `piperine_lang::elab` (ELAB-02): `ConstEval` folds the
//! subset (a `const` item once, IEEE 754 throughout, a param read only through its binding);
//! the bound values, defaults and `where` conditions of placements join in T3 to T5.

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use piperine_ir::{Constant, Real};
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
    let design = Elab::design("module Leaf() { x: f64 = 1.0; }\nmodule Top() { let l = Leaf { x: temperature() }(); }");
    assert!(design.diagnostics().is_empty(), "{:?}", Elab::codes(design.diagnostics()));
    let types = Types::check(&design, "Top").expect("the source is in the subset");
    let top = design.module("Top").expect("Top");
    let value = top.instances()[0].callee().fields()[0].value().id();
    let mut eval = ConstEval::new(&design, &types);
    assert!(matches!(eval.value(value, &Bindings::new()), Err(ConstFault::Dynamic { .. })));
}
