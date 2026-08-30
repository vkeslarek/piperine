//! LRM-COVERS: 3.6.2.7
//!
//! lrm-conformance-wave3 T25: a `discipline` may declare user-defined
//! attributes — "Like natures, a `discipline` can specify user-defined
//! attributes ... useful for the same reasons as nature user-defined
//! attributes (see 3.6.1.3)" (§3.6.2.7). A bare `attr_name = expr;` in the
//! discipline body (anything that is not `domain`/`potential`/`flow` or a
//! slot-qualified `potential.attr`/`flow.attr` override, T23's own
//! §3.6.2.5 form) lands in `DisciplineDecl::attributes` and is readable
//! through the POM — never a parse error. §3.6.2.7 reserves `domain`,
//! `potential`, and `flow` for their own binding forms, so a user
//! attribute sharing one of those names collides and fails loud.

use piperine_lang::elab::const_eval::ConstEnv;
use piperine_lang::value::Value;
use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn parse_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected a parse/elaboration error")
        .to_string()
}

const BASE: &str = r#"
    nature ttl_volt;
        units = "V";
        access = V;
        abstol = 100u;
    endnature
    nature ttl_curr;
        units = "A";
        access = I;
        abstol = 1e-3;
    endnature
"#;

/// AC1 (T25 Done-when #1): a user-defined attribute is not a parse error
/// and survives to `DisciplineDecl::attributes` — the exact evaluated
/// constant declared in source.
#[test]
fn user_attribute_survives_to_discipline_decl_attributes() {
    let src = format!(
        "{BASE}
        discipline ttl;
            potential ttl_volt;
            flow ttl_curr;
            maxfreq = 100e6;
        enddiscipline
        module m (inout ttl p);
        endmodule
        "
    );
    let d = elab(&src);
    let disc = d.discipline("ttl").expect("discipline `ttl` elaborated");
    assert_eq!(disc.attributes.len(), 1, "exactly one user-defined attribute declared");
    let (name, expr) = &disc.attributes[0];
    assert_eq!(name, "maxfreq");
    let env = ConstEnv::new();
    let value = env.eval(expr).expect("attribute value is a constant expression");
    assert_eq!(value, Value::Real(100e6), "the declared value round-trips through the POM");
}

/// A discipline may declare more than one user attribute, each landing as
/// its own entry (not merged/overwritten).
#[test]
fn multiple_user_attributes_all_survive() {
    let src = format!(
        "{BASE}
        discipline ttl;
            potential ttl_volt;
            flow ttl_curr;
            maxfreq = 100e6;
            vendor_id = 42;
        enddiscipline
        module m (inout ttl p);
        endmodule
        "
    );
    let d = elab(&src);
    let disc = d.discipline("ttl").expect("discipline `ttl` elaborated");
    assert_eq!(disc.attributes.len(), 2, "both user-defined attributes declared");
    let names: Vec<&str> = disc.attributes.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"maxfreq"));
    assert!(names.contains(&"vendor_id"));
}

/// AC2 (T25 Done-when #2): an attribute named `domain` collides with the
/// reserved discipline binding name and fails loud, naming the offending
/// attribute and citing §3.6.2.7.
#[test]
fn attribute_named_domain_fails_loud() {
    let src = format!(
        "{BASE}
        discipline ttl;
            potential ttl_volt;
            flow ttl_curr;
            domain = 5;
        enddiscipline
        module m (inout ttl p);
        endmodule
        "
    );
    let msg = parse_err(&src);
    assert!(msg.contains("domain"), "error must name the reserved attribute, got: {msg}");
    assert!(msg.contains("3.6.2.7"), "error must cite §3.6.2.7, got: {msg}");
}

/// Same collision, for `potential` — the other reserved discipline
/// binding name.
#[test]
fn attribute_named_potential_fails_loud() {
    let src = format!(
        "{BASE}
        discipline ttl;
            flow ttl_curr;
            potential = 5;
        enddiscipline
        module m (inout ttl p);
        endmodule
        "
    );
    let msg = parse_err(&src);
    assert!(msg.contains("potential"), "error must name the reserved attribute, got: {msg}");
    assert!(msg.contains("3.6.2.7"), "error must cite §3.6.2.7, got: {msg}");
}

/// Same collision, for `flow`.
#[test]
fn attribute_named_flow_fails_loud() {
    let src = format!(
        "{BASE}
        discipline ttl;
            potential ttl_volt;
            flow = 5;
        enddiscipline
        module m (inout ttl p);
        endmodule
        "
    );
    let msg = parse_err(&src);
    assert!(msg.contains("flow"), "error must name the reserved attribute, got: {msg}");
    assert!(msg.contains("3.6.2.7"), "error must cite §3.6.2.7, got: {msg}");
}

/// The ordinary keyword forms (`domain continuous;`, `potential
/// ttl_volt;`, `flow ttl_curr;`) are unaffected by the collision check —
/// only the bare `name = expr;` attribute-assignment shape triggers it.
#[test]
fn ordinary_binding_keywords_still_parse() {
    let src = format!(
        "{BASE}
        discipline ttl;
            domain continuous;
            potential ttl_volt;
            flow ttl_curr;
        enddiscipline
        module m (inout ttl p);
        endmodule
        "
    );
    let d = elab(&src);
    let disc = d.discipline("ttl").expect("discipline `ttl` elaborated");
    assert_eq!(disc.domain, Some(piperine_lang::ast::Domain::Continuous));
    assert_eq!(disc.potential.as_deref(), Some("ttl_volt"));
    assert_eq!(disc.flow.as_deref(), Some("ttl_curr"));
    assert!(disc.attributes.is_empty(), "no user attributes declared here");
}
