//! Reading X/Z of a digital net in a continuous (analog) context is an
//! error, never a silent `2.0`/`3.0` (LRM §7.3.2, T25/LRMW2-23) — the D2A
//! bridge (`DigitalLayout::export_vars`) that feeds a digital `reg`'s value
//! into the analog var bank must fail loud when that value is genuinely
//! unknown/high-Z, naming the net.
//!
//! LRM-COVERS: 7.3.2

use piperine_codegen::device::DigitalInstance;
use piperine_codegen::resolve::*;
use piperine_codegen::{CodegenError, DigitalKernel};
use piperine_lang::ast::{Expr, Stmt};
use piperine_lang::error::PreprocessedSpan;
use piperine_lang::lexer::{BasedNumber, NumberBase, NumberLit};

const SP: PreprocessedSpan = PreprocessedSpan { start: 0, end: 0 };

fn ident(name: &str) -> Expr {
    Expr::Ident { span: SP, name: name.to_string() }
}

fn bit(digit: &str) -> Expr {
    Expr::Number {
        span: SP,
        value: NumberLit::Based(BasedNumber {
            size: Some(1),
            signed: false,
            base: NumberBase::Binary,
            digits: digit.to_string(),
        }),
    }
}

fn assign(dest: &str, value: Expr) -> Stmt {
    Stmt::Assign { span: SP, target: ident(dest), value }
}

/// A comb-only module with one `Quad` reg (`dreg`), one input (`a`), and no
/// outputs: `dreg = a ? <one_value> : <zero_value>;` is avoided in favor of
/// two fixed-value fixtures below (simplest to reason about — no ternary
/// dependency needed to pin the exported value).
fn quad_reg_module(name: &str, literal: &str) -> (LoweredBody, VarId) {
    let mut m = LoweredBody::new(name);
    let dreg = m.symbols.add_var("dreg", Type::Quad);
    m.digital = Some(DigitalBody {
        inputs: vec![],
        outputs: vec![],
        regs: vec![dreg],
        reg_inits: vec![],
        stmts: vec![assign("dreg", bit(literal))],
    });
    (m, dreg)
}

/// Runs `init` (comb runs once, unconditionally) and returns the instance.
fn run_once(module: LoweredBody) -> DigitalInstance {
    let kernel = std::sync::Arc::new(DigitalKernel::compile(&module).expect("compile"));
    let mut instance = DigitalInstance::new(kernel, 0, vec![], vec![], vec![]).expect("instance");
    let mut queue = Default::default();
    instance.init(&mut queue);
    instance
}

/// Spec AC: a `reg` driven to `1'bx` errors, citing §7.3.2 and naming the
/// net, when the D2A bridge tries to export it as a continuous (real) value.
#[test]
fn x_valued_reg_fails_loud_naming_the_net_and_the_clause() {
    let (module, _dreg) = quad_reg_module("x_export", "x");
    let instance = run_once(module);

    match instance.export_vars() {
        Err(CodegenError::Unsupported(msg)) => {
            assert!(msg.contains("dreg"), "error must name the net: {msg}");
            assert!(msg.contains("7.3.2"), "error must cite LRM §7.3.2: {msg}");
            assert!(msg.contains('X'), "error must say which 4-state value it saw: {msg}");
        }
        other => panic!("expected CodegenError::Unsupported naming `dreg`, got {other:?}"),
    }
}

/// `1'bz` (high impedance) is caught too — it currently reaches
/// `export_vars` as the same raw quad `X` encoding `1'bx` does (a
/// pre-existing, documented simplification in the based-literal decoder,
/// `emit/digital_expr.rs`: "any non-numeric digit... makes the whole
/// literal a Quad X" — genuinely distinct `Z`(3) values arise at runtime
/// from an undriven net/gate, not from a literal, so this test cannot
/// force the `Z`-worded branch of the error message through a literal;
/// the raw-value check in `DigitalLayout::export_vars` still covers both
/// `2` and `3` for when a real `Z` reaches it).
#[test]
fn z_literal_fails_loud_naming_the_net() {
    let (module, _dreg) = quad_reg_module("z_export", "z");
    let instance = run_once(module);

    match instance.export_vars() {
        Err(CodegenError::Unsupported(msg)) => {
            assert!(msg.contains("dreg"), "error must name the net: {msg}");
        }
        other => panic!("expected CodegenError::Unsupported naming `dreg`, got {other:?}"),
    }
}

/// Regression: a defined (`0`/`1`) value still exports normally — the fix
/// must not touch the ordinary D2A path.
#[test]
fn defined_zero_and_one_still_export_normally() {
    for (literal, expected) in [("0", 0.0_f64), ("1", 1.0_f64)] {
        let (module, dreg) = quad_reg_module(&format!("defined_export_{literal}"), literal);
        let instance = run_once(module);

        let vars = instance.export_vars().expect("defined 0/1 must not error");
        let value = vars.into_iter().find(|(v, _)| *v == dreg).map(|(_, value)| value);
        assert_eq!(value, Some(expected), "defined value `{literal}` must export as {expected}");
    }
}

/// An `Integer` (2-state, not `Quad`) var holding the literal value `2` or
/// `3` is an ordinary number, not an unknown/high-Z bit — the fix must not
/// mistake it for X/Z (only a genuinely 4-state `reg` triggers the error).
#[test]
fn an_integer_var_holding_the_literal_value_two_is_not_mistaken_for_x() {
    let mut m = LoweredBody::new("integer_two");
    let cnt = m.symbols.add_var("cnt", Type::Integer);
    m.digital = Some(DigitalBody {
        inputs: vec![],
        outputs: vec![],
        regs: vec![cnt],
        reg_inits: vec![],
        stmts: vec![assign("cnt", Expr::Number { span: SP, value: NumberLit::Integer(2) })],
    });
    let instance = run_once(m);

    let vars = instance.export_vars().expect("an Integer var holding 2 is not X");
    let value = vars.into_iter().find(|(v, _)| *v == cnt).map(|(_, value)| value);
    assert_eq!(value, Some(2.0), "Integer `cnt = 2` exports as the number 2.0, not an X error");
}

