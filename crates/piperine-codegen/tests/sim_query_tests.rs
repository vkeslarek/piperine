//! Negative- and positive-assertion tests for the `Sim` query emitter
//! (`$temperature`, `$abstime`, `$mfactor`, `$vt`, `$simparam`).
//!
//! Regression tests for `docs/GAPS.md` §A.2, §A.3, §A.14, §A.15.

use piperine_codegen::codegen::SimCtx;
use piperine_codegen::validate_ir_contrib;
use piperine_codegen::codegen::ir_emit::validate_ir_contrib_with;
use piperine_codegen::codegen::ir_emit::validate_ir_contrib_with2;
use piperine_codegen::ir::*;

/// A trivial simulator query that survives JIT should lower without error.
#[test]
fn temperature_vt_abstime_are_accepted_by_validator() {
    let expr = IrExpr::Sim(SimQuery::Temperature);
    assert!(validate_ir_contrib(&expr).is_ok());

    let expr = IrExpr::Sim(SimQuery::Abstime);
    assert!(validate_ir_contrib(&expr).is_ok());

    let expr = IrExpr::Sim(SimQuery::Mfactor);
    assert!(validate_ir_contrib(&expr).is_ok());

    let expr = IrExpr::Sim(SimQuery::Vt(None));
    assert!(validate_ir_contrib(&expr).is_ok());
}

/// `$simparam` is accepted by the validator; unknown keys fall back to
/// the default expression at JIT time. GAPS §A.14.
#[test]
fn simparam_is_accepted_by_validator() {
    let expr = IrExpr::Sim(SimQuery::Simparam {
        key: "gmin".into(),
        default: Box::new(IrExpr::Real(1.0e-12)),
    });
    assert!(validate_ir_contrib(&expr).is_ok());

    let expr = IrExpr::Sim(SimQuery::Simparam {
        key: "nonexistent".into(),
        default: Box::new(IrExpr::Real(0.0)),
    });
    assert!(validate_ir_contrib(&expr).is_ok());
}

/// `$param_given` is accepted by the validator. GAPS §A.15.
#[test]
fn param_given_is_accepted_by_validator() {
    let expr = IrExpr::Sim(SimQuery::ParamGiven("r".into()));
    assert!(validate_ir_contrib(&expr).is_ok());
}

/// `SimCtx` carries the runtime fields used by the `Sim` emitter with
/// the `#[repr(C)]` layout (`offset 0 = temperature`, etc.). This is a
/// cheap compile-time check via pointer offsets; an actual JVM-level
/// read would need Cranelift machinery (covered by other tests).
#[test]
fn sim_ctx_field_offsets_match_codegen_assumptions() {
    use std::mem::offset_of;

    // The codegen reads:
    //   offset 0  -> temperature
    //   offset 8  -> abstime
    //   offset 16 -> mfactor
    //   offset 24 -> gmin
    assert_eq!(offset_of!(SimCtx, temperature), 0);
    assert_eq!(offset_of!(SimCtx, abstime), 8);
    assert_eq!(offset_of!(SimCtx, mfactor), 16);
    assert_eq!(offset_of!(SimCtx, gmin), 24);

    // The codegen `Default` impl produces room-temperature, t=0, mfactor=1,
    // gmin=1e-12. Verify.
    let s = SimCtx::default();
    assert_eq!(s.temperature, 300.0);
    assert_eq!(s.abstime, 0.0);
    assert_eq!(s.mfactor, 1.0);
    assert_eq!(s.gmin, 1.0e-12);

    // K_B/q constant is the CODATA value used by $vt emission.
    let k_over_q = SimCtx::K_B_OVER_Q_EV_PER_K;
    let vt_300 = k_over_q * 300.0;
    assert!((vt_300 - 0.025852).abs() < 1e-6,
        "k*T/q at 300 K should match the spec's 0.025852 V: got {vt_300}");
}

// ───────────────────────── Part A — branch access / silent-zero failures ──

/// GAPS §A.1 — reading branch flow `I(a, b)` inside an analog contribution
/// must be rejected (not silently return 0.0). The validator has been
/// wired to do this; verify it names the gap in the message.
#[test]
fn flow_read_in_contribution_is_rejected_not_silently_zero() {
    let expr = IrExpr::BranchAccess {
        access: "I".into(),
        plus: "p".into(),
        minus: "0".into(),
    };
    let err = validate_ir_contrib(&expr).unwrap_err().to_string();
    assert!(
        err.contains("I(") || err.contains("not yet supported"),
        "expected rejection to name the `I(` access or 'not yet supported', got: {err}"
    );
    assert!(
        err.contains("A.1") || err.to_lowercase().contains("indirect"),
        "expected message to point at the A.1 gap or the indirect-contrib workaround, got: {err}"
    );
}

/// `V(a, b)` with unknown terminal names is rejected (GAPS §A.9). The
/// literal "0" is the implicit ground reference and is always allowed.
#[test]
fn v_with_unknown_terminal_is_rejected() {
    use std::collections::HashSet;
    let mut terms = HashSet::new();
    terms.insert("p".to_string());
    terms.insert("n".to_string());

    // Known terminals validate.
    let expr = IrExpr::BranchAccess {
        access: "V".into(),
        plus: "p".into(),
        minus: "n".into(),
    };
    assert!(validate_ir_contrib_with2(&expr, None, Some(&terms)).is_ok(),
        "known terminals should validate");

    // Unknown plus — rejected.
    let bad = IrExpr::BranchAccess {
        access: "V".into(),
        plus: "nonexistent".into(),
        minus: "n".into(),
    };
    let err = validate_ir_contrib_with2(&bad, None, Some(&terms)).unwrap_err().to_string();
    assert!(
        err.contains("nonexistent") && err.contains("A.9"),
        "expected rejection to name the bad terminal and the A.9 gap, got: {err}"
    );

    // Unknown minus (plus is known) — also rejected.
    let bad_minus = IrExpr::BranchAccess {
        access: "V".into(),
        plus: "p".into(),
        minus: "ghost".into(),
    };
    let err_m = validate_ir_contrib_with2(&bad_minus, None, Some(&terms))
        .unwrap_err().to_string();
    assert!(
        err_m.contains("ghost") && err_m.contains("A.9"),
        "expected rejection naming the bad minus terminal and the A.9 gap, got: {err_m}"
    );
}

/// `V(a)` with no second argument gets a default `0` for `minus`, which is
/// the implicit ground reference and is always allowed.
#[test]
fn v_with_implicit_ground_is_always_allowed() {
    use std::collections::HashSet;
    let mut terms = HashSet::new();
    terms.insert("only_one".to_string());

    let expr = IrExpr::BranchAccess {
        access: "V".into(),
        plus: "only_one".into(),
        minus: "0".into(),
    };
    assert!(
        validate_ir_contrib_with2(&expr, None, Some(&terms)).is_ok(),
        "literal minus `0` is always allowed as the implicit ground"
    );
}

/// GAPS §A.8 — `Param(name)` and `Var(name)` resolution rejects unknown
/// names when the known-names set is supplied.
#[test]
fn unresolved_param_var_is_rejected() {
    use std::collections::HashSet;
    let mut known = HashSet::new();
    known.insert("r".to_string());

    let good = IrExpr::Param("r".into());
    assert!(validate_ir_contrib_with(&good, Some(&known)).is_ok(),
        "known name should validate");

    let bad = IrExpr::Param("r_typo".into());
    let err = validate_ir_contrib_with(&bad, Some(&known)).unwrap_err().to_string();
    assert!(
        err.contains("r_typo") && err.contains("A.8"),
        "expected rejection to name the bad identifier and the A.8 gap, got: {err}"
    );

    // Unknown Var() should also be rejected.
    let bad_var = IrExpr::Var("another_typo".into());
    let err2 = validate_ir_contrib_with(&bad_var, Some(&known)).unwrap_err().to_string();
    assert!(
        err2.contains("another_typo") && err2.contains("A.8"),
        "expected Var() rejection to name the bad identifier and the A.8 gap, got: {err2}"
    );
}
