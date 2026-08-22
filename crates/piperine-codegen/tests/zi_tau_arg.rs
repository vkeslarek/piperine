//! LRM-COVERS: 4.5.12, 4.5.14
//! LRMW2-32 (`lrm-conformance-wave2`, T40): `zi_*(expr, num, den, T[, τ[,
//! t0]])`'s trailing `τ`/`t0` arguments (§4.5.12: "All Z-transform filters
//! share three common arguments: T, τ, and t0 ... τ specifies the
//! transition time ... t0 specifies the time of the first transition")
//! used to be silently discarded entirely — `ZTransform::lower`
//! (`resolve/pom/analog_ops.rs`) only ever read `args[0..=3]`.
//!
//! Re-verification note: `t0` has no honest realization in this codegen —
//! `zi_*` lowers `expr` through the same `ddt`-shaped charge/resistive
//! split `laplace_*` does (no sample-and-hold state exists to shift a
//! "first transition" instant of), so an explicit `t0` fails loud instead
//! of being carried-and-ignored. `τ` genuinely is a step-control lever
//! (§4.5.12's own words) and is carried for real — see the discriminating
//! transient proof in root `tests/zi_tau_lte.rs`.

use piperine_codegen::resolve::{lower_bodies, LowerErrors, StateKind, StateVar};
use piperine_lang::parse_and_elaborate;
use piperine_lang::SourceMap;

fn zi_source(body: &str) -> String {
    format!(
        "module dut (inout electrical p, inout electrical n);
            analog begin
                {body}
            end
        endmodule"
    )
}

fn zi_state(src: &str) -> StateVar {
    let design = parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let bodies = lower_bodies(&design).expect("lowers");
    let body = &bodies["dut"];
    body.symbols
        .states()
        .find_map(|(_, s)| matches!(s.kind, StateKind::ZTransform { .. }).then(|| s.clone()))
        .expect("a zi_* state")
}

/// §4.5.12: an omitted `τ` carries no transition-time lever.
#[test]
fn zi_without_tau_has_none() {
    let src = zi_source("I(p, n) <+ zi_nd(V(p,n), '{0,1}, '{-1,0,1}, 1e-6);");
    let StateKind::ZTransform { tau, .. } = zi_state(&src).kind else { unreachable!() };
    assert!(tau.is_none(), "no 5th argument must leave tau unset");
}

/// §4.5.12: an explicit `τ` is carried, not discarded.
#[test]
fn zi_with_an_explicit_tau_carries_it() {
    let src = zi_source("I(p, n) <+ zi_nd(V(p,n), '{0,1}, '{-1,0,1}, 1e-6, 2.5e-7);");
    let StateKind::ZTransform { tau, .. } = zi_state(&src).kind else { unreachable!() };
    let piperine_lang::ast::Expr::Number { value, .. } = tau.expect("tau carried") else {
        panic!("a numeric tau should resolve to a literal at lowering time");
    };
    let v = match value {
        piperine_lang::lexer::NumberLit::Real(r) => r,
        piperine_lang::lexer::NumberLit::Integer(i) => i as f64,
        other => panic!("expected a real/integer literal, got {other:?}"),
    };
    assert!((v - 2.5e-7).abs() < 1e-15, "expected the declared tau 2.5e-7, got {v}");
}

/// §4.5.12: an explicit `t0` fails loud (typed `LowerErrors`, never a
/// silent carry-and-ignore) instead of resolving.
#[test]
fn zi_with_an_explicit_t0_fails_loud_by_typed_kind() {
    let src = zi_source("I(p, n) <+ zi_nd(V(p,n), '{0,1}, '{-1,0,1}, 1e-6, 2.5e-7, 1e-3);");
    let design = parse_and_elaborate(&src, &SourceMap::dummy()).expect("elaborates");
    let Err(LowerErrors(errors)) = lower_bodies(&design) else {
        panic!("an explicit zi_* t0 argument must fail lowering, not resolve silently");
    };
    assert!(
        errors.iter().any(|e| e.name == "zi_t0" && e.what.contains("4.5.12")),
        "expected a zi_t0 error citing §4.5.12, got: {errors:?}"
    );
}
