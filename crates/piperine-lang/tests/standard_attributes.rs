//! LRM-COVERS: 2.9.2, 3.6.3.1
//!
//! lrm-conformance-wave3 T27: the four LRM §2.9.2 standard attributes —
//! `desc`, `units`, `op`, `multiplicity` — register as core-owned
//! attribute schemas (the same way `rfport` already does, `piperine_lang::
//! lib.rs`'s `parse_and_elaborate`) so a plain `parse_and_elaborate` call
//! (no plugin host, no schema seeding) accepts them on parameter,
//! variable, and net declarations, per the LRM's own worked example:
//! `(* desc="effective resistance", units="Ohms", op="yes",
//! multiplicity="divide" *) real reff;` (the LRM's own prose renders this
//! attr_spec list space-separated; Annex A's actual grammar,
//! `attr_spec {, attr_spec}`, requires the commas this file's fixtures
//! use) — and §3.6.3.1's own net example,
//! `(* desc="drain terminal" *) electrical d;`.

use piperine_lang::value::Value;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

/// AC4 (P2 story) + T27 Done-when #1, the LRM's own combined example
/// (§2.9.2): all four attributes are accepted with no plugin schema
/// seeding at all (`parse_and_elaborate`, not `_seeded`) and their values
/// are readable through the POM.
#[test]
fn all_four_standard_attributes_accepted_without_a_plugin_schema() {
    let src = r#"
        module m ();
            (* desc="effective resistance", units="Ohms", op="yes", multiplicity="divide" *)
            real reff;
        endmodule
        "#;
    let d = elab(src);
    let m = d.module("m").expect("m elaborated");
    let reff = m.vars().iter().find(|v| v.name() == "reff").expect("reff declared");
    let get = |schema: &str| {
        reff.attributes().iter().find(|a| a.schema() == schema).and_then(|a| a.field(schema)).cloned()
    };
    assert_eq!(get("desc"), Some(Value::Str("effective resistance".to_string())));
    assert_eq!(get("units"), Some(Value::Str("Ohms".to_string())));
    assert_eq!(get("op"), Some(Value::Str("yes".to_string())));
    assert_eq!(get("multiplicity"), Some(Value::Str("divide".to_string())));
}

/// T27 Done-when #1, §3.6.3.1's own worked example: `desc` on a plain net
/// (not a port) is readable through the POM.
#[test]
fn desc_on_a_net_is_readable_through_the_pom() {
    let src = r#"
        module m ();
            (* desc="drain terminal" *) electrical d;
        endmodule
        "#;
    let d = elab(src);
    let m = d.module("m").expect("m elaborated");
    let wire = m.wire("d").expect("d declared as a wire");
    let value = wire.attributes().iter().find(|a| a.schema() == "desc").and_then(|a| a.field("desc"));
    assert_eq!(value, Some(&Value::Str("drain terminal".to_string())));
}

/// T27 Done-when #2: `op` must be `"yes"` or `"no"` — both legal values
/// are accepted.
#[test]
fn op_accepts_yes_and_no() {
    let src = r#"
        module m ();
            (* op="yes" *) real a;
            (* op="no" *) real b;
        endmodule
        "#;
    let d = elab(src);
    let m = d.module("m").expect("m elaborated");
    assert_eq!(
        m.vars().iter().find(|v| v.name() == "a").unwrap().attributes()[0].field("op"),
        Some(&Value::Str("yes".to_string()))
    );
    assert_eq!(
        m.vars().iter().find(|v| v.name() == "b").unwrap().attributes()[0].field("op"),
        Some(&Value::Str("no".to_string()))
    );
}

/// T27 Done-when #2: an `op` value outside `{"yes", "no"}` fails loud,
/// naming the attribute and the illegal value.
#[test]
fn op_outside_yes_no_fails_loud() {
    let msg = elab_err(
        r#"
        module m ();
            (* op="maybe" *) real a;
        endmodule
        "#,
    );
    assert!(msg.contains("op"), "error must name the attribute, got: {msg}");
    assert!(msg.contains("maybe"), "error must name the illegal value, got: {msg}");
}

/// T27 Done-when #2: `multiplicity` accepts exactly `"multiply"`,
/// `"divide"`, or `"none"`.
#[test]
fn multiplicity_accepts_its_three_legal_values() {
    let src = r#"
        module m ();
            (* multiplicity="multiply" *) real a;
            (* multiplicity="divide" *) real b;
            (* multiplicity="none" *) real c;
        endmodule
        "#;
    let d = elab(src);
    let m = d.module("m").expect("m elaborated");
    for (name, expect) in [("a", "multiply"), ("b", "divide"), ("c", "none")] {
        let v = m.vars().iter().find(|v| v.name() == name).unwrap();
        assert_eq!(v.attributes()[0].field("multiplicity"), Some(&Value::Str(expect.to_string())));
    }
}

/// T27 Done-when #2: a `multiplicity` value outside the closed set fails
/// loud, naming the attribute and the illegal value.
#[test]
fn multiplicity_outside_its_closed_set_fails_loud() {
    let msg = elab_err(
        r#"
        module m ();
            (* multiplicity="scale" *) real a;
        endmodule
        "#,
    );
    assert!(msg.contains("multiplicity"), "error must name the attribute, got: {msg}");
    assert!(msg.contains("scale"), "error must name the illegal value, got: {msg}");
}

/// T27 Done-when #3, §3.6.3.1's own rule: a `desc` given on both a port
/// declaration and that same net's discipline declaration (the two-step
/// form) keeps the last (net-side) value and records a module lint.
#[test]
fn duplicate_desc_on_port_and_net_keeps_last_and_warns() {
    let src = r#"
        module m (p);
            (* desc="from the port" *) input p;
            (* desc="from the net" *) electrical p;
        endmodule
        "#;
    let d = elab(src);
    let m = d.module("m").expect("m elaborated");
    let port = m.port("p").expect("p declared as a port");
    let value = port.attributes().iter().find(|a| a.schema() == "desc").and_then(|a| a.field("desc"));
    assert_eq!(value, Some(&Value::Str("from the net".to_string())), "the net declaration's value must win");
    assert!(
        m.lints().iter().any(|l| l.contains("desc") && l.contains("p")),
        "a duplicate-desc lint must be recorded, got: {:?}",
        m.lints()
    );
}
