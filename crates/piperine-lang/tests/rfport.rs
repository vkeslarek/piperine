/// Tests for the `@rfport(num, z0)` attribute (SP-01, SP-05) — the `.sp`
/// S-parameter port declaration, resolved via the existing attribute-schema
/// machinery (Part VI). See `.specs/features/spectral-analyses/spec.md`
/// (P2 `.sp`) and `design.md` ("Port primitive — `@rfport` attribute").
use piperine_lang::{parse_str, pom::RfPort};

fn elab(src: &str) -> piperine_lang::pom::Design {
    parse_str(src).expect("parse failed").elaborate(&piperine_lang::SourceMap::dummy()).expect("elaborate failed")
}

#[test]
fn test_rfport_attribute_elaborates_on_wire() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) {
            @rfport(num = 1, z0 = 50) wire rf_in : Electrical;
        }
    ";
    let design = elab(src);
    let ports = design.rfports("M").expect("rfports() should resolve");
    assert_eq!(ports, vec![RfPort { num: 1, z0: 50.0, node: "rf_in".into() }]);
}

#[test]
fn test_rfport_z0_defaults_to_50() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) {
            @rfport(num = 1) wire rf_in : Electrical;
        }
    ";
    let design = elab(src);
    let ports = design.rfports("M").expect("rfports() should resolve");
    assert_eq!(ports.len(), 1);
    assert_eq!(ports[0].z0, 50.0);
}

#[test]
fn test_rfport_on_port() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( @rfport(num = 1, z0 = 75) inout rf : Electrical ) { }
    ";
    let design = elab(src);
    let ports = design.rfports("M").expect("rfports() should resolve");
    assert_eq!(ports, vec![RfPort { num: 1, z0: 75.0, node: "rf".into() }]);
}

#[test]
fn test_rfport_two_ports() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout gnd : Electrical ) {
            @rfport(num = 1, z0 = 50) wire rf_in : Electrical;
            @rfport(num = 2, z0 = 50) wire rf_out : Electrical;
        }
    ";
    let design = elab(src);
    let mut ports = design.rfports("M").expect("rfports() should resolve");
    ports.sort_by_key(|p| p.num);
    assert_eq!(
        ports,
        vec![
            RfPort { num: 1, z0: 50.0, node: "rf_in".into() },
            RfPort { num: 2, z0: 50.0, node: "rf_out".into() },
        ]
    );
}

#[test]
fn test_rfport_non_positive_z0_fails_loud() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) {
            @rfport(num = 1, z0 = 0) wire rf_in : Electrical;
        }
    ";
    let design = elab(src);
    let err = design.rfports("M").expect_err("z0=0 must fail loud (SP-05)").to_string();
    assert!(err.contains("z0"), "expected z0 in error, got: {err}");
    assert!(err.contains("positive"), "expected 'positive' in error, got: {err}");
}

#[test]
fn test_rfport_negative_z0_fails_loud() {
    // `-50` is a unary-negated literal — the shared attribute-value
    // evaluator (`eval_attr_value`, out of T7's scope) only accepts bare
    // literals, so this fails loud during elaboration itself rather than at
    // `rfports()`. Either way, SP-05 ("non-positive z0 fails loud") holds.
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) {
            @rfport(num = 1, z0 = -50) wire rf_in : Electrical;
        }
    ";
    let err = parse_str(src)
        .expect("parse failed")
        .elaborate(&piperine_lang::SourceMap::dummy())
        .err()
        .expect("negative z0 must fail loud (SP-05)")
        .to_string();
    assert!(err.contains("z0"), "expected z0 in error, got: {err}");
}

#[test]
fn test_rfport_duplicate_num_fails_loud() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) {
            @rfport(num = 1, z0 = 50) wire rf_in : Electrical;
            @rfport(num = 1, z0 = 50) wire rf_out : Electrical;
        }
    ";
    let design = elab(src);
    let err = design.rfports("M").expect_err("duplicate num must fail loud (SP-05)").to_string();
    assert!(err.contains("duplicate"), "expected 'duplicate' in error, got: {err}");
    assert!(err.contains('1'), "expected the duplicate num in error, got: {err}");
}

#[test]
fn test_rfport_unknown_module_fails_loud() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) { }
    ";
    let design = elab(src);
    let err = design.rfports("DoesNotExist").expect_err("unknown module must fail loud (SP-05)").to_string();
    assert!(err.contains("DoesNotExist"), "expected the module name in error, got: {err}");
}

#[test]
fn test_rfport_bad_arg_type_fails_loud() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) {
            @rfport(num = 1, z0 = \"not-a-number\") wire rf_in : Electrical;
        }
    ";
    let err = parse_str(src)
        .expect("parse failed")
        .elaborate(&piperine_lang::SourceMap::dummy())
        .err()
        .expect("expected elaboration error");
    let msg = err.to_string();
    assert!(msg.contains("z0"), "expected z0 in error, got: {msg}");
}

#[test]
fn test_rfport_module_with_no_ports_returns_empty() {
    let src = "
        discipline Electrical { potential v: Real; flow i: Real; }
        mod M ( inout p : Electrical ) { wire w : Electrical; }
    ";
    let design = elab(src);
    let ports = design.rfports("M").expect("rfports() should resolve");
    assert!(ports.is_empty());
}
