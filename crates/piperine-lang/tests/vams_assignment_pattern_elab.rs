//! LRM-COVERS: 3.4.2, 3.4.4, 3.4.6, 3.4.8, 4.2.14
//! LRMC-C1-06 (FUNCTIONALITY half, T14) — elaborating assignment-pattern
//! `'{...}` initializers (parsed as `Expr::AssignmentPattern` by T13) for
//! parameter arrays (§3.4.4), string valid-value lists (§3.4.6), and
//! multidimensional initializers (§3.4.8). Every fixture is the LRM's own
//! worked example for its clause (spec.md's Edge Cases + Fixture rule), not
//! a synthetic minimal repro.

use piperine_lang::pom::{Value, ValueType};
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

fn list(v: &Value) -> Vec<Value> {
    match v {
        Value::List(items) => items.borrow().clone(),
        other => panic!("expected Value::List, got {other:?}"),
    }
}

#[test]
fn param_array_assignment_pattern_elaborates_from_lrm_3_4_4() {
    // §3.4.4's own worked example: `parameter real poles[0:3] = '{ 1.0,
    // 3.198, 4.554, 2.00 };`
    let d = elab(
        r#"
        module filt (inout electrical p, inout electrical n);
            parameter real poles[0:3] = '{ 1.0, 3.198, 4.554, 2.00 };
        endmodule
        "#,
    );
    let m = d.module("filt").expect("filt elaborated");
    let p = &m.params()[0];
    assert_eq!(p.ty, ValueType::Array(Box::new(ValueType::Real), 4));
    let items = list(p.default.as_ref().expect("default present"));
    let got: Vec<f64> = items
        .iter()
        .map(|v| match v {
            Value::Real(r) => *r,
            other => panic!("expected Real, got {other:?}"),
        })
        .collect();
    assert_eq!(got, vec![1.0, 3.198, 4.554, 2.00]);
}

#[test]
fn string_valid_value_list_constrains_param_from_lrm_3_4_6() {
    // §3.4.6's own worked example: `parameter string transistortype =
    // "NPN" from '{ "NPN", "PNP" };`
    let d = elab(
        r#"
        module q (inout electrical c, inout electrical b, inout electrical e);
            parameter string transistortype = "NPN" from '{ "NPN", "PNP" };
        endmodule
        "#,
    );
    let m = d.module("q").expect("q elaborated");
    assert_eq!(m.params()[0].default, Some(Value::Str("NPN".to_string())));
}

#[test]
fn string_value_outside_the_valid_value_list_fails_loud() {
    let src = r#"
        module q (inout electrical c, inout electrical b, inout electrical e);
            parameter string transistortype = "FET" from '{ "NPN", "PNP" };
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains("transistortype"), "names the parameter: {msg}");
    assert!(msg.contains("FET"), "names the offending value: {msg}");
}

#[test]
fn multidimensional_param_array_elaborates_from_lrm_3_4_8() {
    // §3.4.8's own worked example (adapted to a small, checkable shape):
    // `parameter real distort[0:1][0:1] = '{ '{1.0, 2.0}, '{3.0, 4.0} };`
    let d = elab(
        r#"
        module amp (inout electrical p, inout electrical n);
            parameter real distort[0:1][0:1] = '{ '{1.0, 2.0}, '{3.0, 4.0} };
        endmodule
        "#,
    );
    let m = d.module("amp").expect("amp elaborated");
    let p = &m.params()[0];
    assert_eq!(p.ty, ValueType::Array(Box::new(ValueType::Array(Box::new(ValueType::Real), 2)), 2));
    let rows = list(p.default.as_ref().expect("default present"));
    assert_eq!(rows.len(), 2);
    let row0: Vec<f64> = list(&rows[0])
        .iter()
        .map(|v| match v {
            Value::Real(r) => *r,
            other => panic!("expected Real, got {other:?}"),
        })
        .collect();
    let row1: Vec<f64> = list(&rows[1])
        .iter()
        .map(|v| match v {
            Value::Real(r) => *r,
            other => panic!("expected Real, got {other:?}"),
        })
        .collect();
    assert_eq!(row0, vec![1.0, 2.0]);
    assert_eq!(row1, vec![3.0, 4.0]);
}

#[test]
fn param_array_initializer_arity_mismatch_fails_loud() {
    let src = r#"
        module filt (inout electrical p, inout electrical n);
            parameter real poles[0:3] = '{ 1.0, 2.0 };
        endmodule
    "#;
    let msg = elab_err(src);
    assert!(msg.contains('4'), "names the expected count: {msg}");
    assert!(msg.contains('2'), "names the found count: {msg}");
}

#[test]
fn scalar_param_still_elaborates_unchanged() {
    // No dims: the ordinary scalar path must be untouched by the array
    // machinery this task adds.
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            parameter real r = 1.0e3;
        endmodule
        "#,
    );
    let m = d.module("res").expect("res elaborated");
    assert_eq!(m.params()[0].default, Some(Value::Real(1.0e3)));
    assert_eq!(m.params()[0].ty, ValueType::Real);
}
