//! §10.3/LRMC-C10-02 (T56): `` `default_transition `` feeds `transition()`'s
//! omitted `rise`/`fall` arguments. Asserted at the resolved-POM level
//! (the `__transition(...)` marker call's literal `rise`/`fall` args
//! after lowering) rather than through a full transient run — this
//! targets exactly the code changed (`analog_ops.rs::Transition::lower`'s
//! `arg(..., default_rise/fall)` substitution), not the shaping filter's
//! own already-tested runtime dynamics.

use piperine_lang::ast::{Expr, Stmt};
use piperine_lang::lexer::NumberLit;
use piperine_lang::parse_and_elaborate;

/// Walks a lowered analog body's statements to find the `__transition`
/// marker call and returns its `(rise, fall)` literal argument values.
fn transition_rise_fall(src: &str) -> (f64, f64) {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let body = &bodies["dut"];
    let analog = body.analog.as_ref().expect("has an analog body");

    fn find_call<'a>(e: &'a Expr, name: &str) -> Option<&'a [Expr]> {
        match e {
            Expr::Call { name: n, args, .. } if n == name => Some(args),
            Expr::Call { args, .. } => args.iter().find_map(|a| find_call(a, name)),
            Expr::Binary { lhs, rhs, .. } => find_call(lhs, name).or_else(|| find_call(rhs, name)),
            Expr::Unary { operand, .. } => find_call(operand, name),
            _ => None,
        }
    }

    fn as_real(e: &Expr) -> f64 {
        match e {
            Expr::Number { value: NumberLit::Real(r), .. } => *r,
            Expr::Number { value: NumberLit::Integer(i), .. } => *i as f64,
            _ => panic!("expected a literal number, got {e:?}"),
        }
    }

    for stmt in &analog.stmts {
        if let Stmt::Contribution { value, .. } = stmt {
            if let Some(args) = find_call(value, "__transition") {
                // marker(state_id, x, delay, rise, fall, tol)
                return (as_real(&args[3]), as_real(&args[4]));
            }
        }
    }
    panic!("no __transition marker call found in the lowered body");
}

#[test]
fn transition_without_explicit_times_uses_the_directive_value() {
    let src = "`default_transition 2.5e-9\n\
        module dut (inout electrical p, inout electrical n);\n\
        analog begin\n\
        I(p, n) <+ transition(V(p, n));\n\
        end\n\
        endmodule\n";
    assert_eq!(transition_rise_fall(src), (2.5e-9, 2.5e-9));
}

#[test]
fn explicit_rise_fall_arguments_still_win_over_the_directive() {
    let src = "`default_transition 2.5e-9\n\
        module dut (inout electrical p, inout electrical n);\n\
        analog begin\n\
        I(p, n) <+ transition(V(p, n), 0.0, 5e-9, 7e-9);\n\
        end\n\
        endmodule\n";
    assert_eq!(transition_rise_fall(src), (5e-9, 7e-9));
}

/// No `` `default_transition `` directive: the pre-existing `0.0` default
/// is unchanged (regression guard against this task silently changing the
/// no-directive case).
#[test]
fn without_a_directive_the_default_stays_zero() {
    let src = "module dut (inout electrical p, inout electrical n);\n\
        analog begin\n\
        I(p, n) <+ transition(V(p, n));\n\
        end\n\
        endmodule\n";
    assert_eq!(transition_rise_fall(src), (0.0, 0.0));
}
