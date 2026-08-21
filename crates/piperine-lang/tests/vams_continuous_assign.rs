//! LRM-COVERS: 6.5.3, 7.3.6.4, 8.5.3.1
//!
//! LRM §8.5.3.1/§8.5.3.2: `assign target = expr;` at module scope is a
//! continuous assignment. `elab/lower/vams.rs` used to fold
//! `ModuleItem::ContinuousAssign` into its no-op arm — the target compiled
//! and simulated with nothing ever driving it (SPEC_DEVIATION.md finding
//! 1.3, re-verified reproducing at HEAD before this fix: the item had zero
//! consumers anywhere in the tree). Also closes §6.5.3 (a `wreal` port's
//! only legal driver is a continuous assign) and §7.3.6.4 (an analog var
//! read digitally needs the same continuous-drive path) by routing the
//! lowering through the domain the target net already resolves to.
//!
//! These tests assert the elaborated POM shape directly (the synthesized
//! behavior); `tests/vams_continuous_assign_e2e.rs` (root) is the
//! "actually drives the net" simulate-level proof.

use piperine_lang::ast::{BehaviorKind, EventExpr, Expr, Stmt};
use piperine_lang::parse_and_elaborate;
use piperine_lang::pom::{ElabError, ElabErrorKind};
use piperine_lang::SourceMap;

fn headers_source_map() -> SourceMap {
    let headers =
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/headers"));
    SourceMap::new(headers)
}

/// A digital-domain `assign` target (declared discipline `domain
/// discrete`) lowers to an `always @(*)` shape: one synthesized `Always`
/// behavior with a single `EventControl{events: [ImplicitAll], body:
/// Assign}` statement — the SAME shape the LRM's own combinational
/// `always @(*)` idiom lowers to, so it participates in the Active region
/// exactly like a blocking assign (§8.5.3.3/.4's existing digital
/// semantics apply unchanged).
#[test]
fn digital_target_lowers_to_an_always_star_shape() {
    let src = "
        discipline logic;
            domain discrete;
        enddiscipline

        module m (input logic a, output logic b);
            assign b = a;
        endmodule
    ";
    let design = parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let m = design.modules().find(|m| m.name() == "m").expect("module m");
    let synthesized = m
        .behaviors()
        .iter()
        .find(|b| matches!(b.kind(), BehaviorKind::Always))
        .expect("assign synthesizes an Always behavior");
    assert_eq!(synthesized.body().len(), 1, "one synthesized statement");
    match &synthesized.body()[0] {
        Stmt::EventControl { events, body, .. } => {
            assert!(matches!(events.as_slice(), [EventExpr::ImplicitAll]), "watches @(*), got {events:?}");
            match body.as_ref() {
                Stmt::Assign { target, value, .. } => {
                    assert!(matches!(target, Expr::Ident { name, .. } if name == "b"));
                    assert!(matches!(value, Expr::Ident { name, .. } if name == "a"));
                }
                other => panic!("expected a blocking Assign body, got {other:?}"),
            }
        }
        other => panic!("expected an EventControl(@*) statement, got {other:?}"),
    }
}

/// An analog-domain `assign` target — `wreal`'s discipline is never
/// declared (LRM has no built-in `wreal` discipline block; T49 is the
/// still-open task that makes `wreal` a first-class net kind), so it hits
/// the SAME "unknown discipline defaults to analog"
/// (`resolve/pom/structure.rs::domain_of`) rule codegen already applies at
/// circuit-build time — lowers to the ideal-source contribution idiom this
/// codebase already uses pervasively for analog drivers: a single
/// implicit-ground `V(target) <+ expr;` in a synthesized `Analog` behavior.
#[test]
fn analog_domain_target_lowers_to_a_v_contribution() {
    let src = "
        `include \"disciplines.vams\"
        module m (inout electrical in);
            wreal out;
            assign out = V(in);
        endmodule
    ";
    let design = parse_and_elaborate(src, &headers_source_map()).expect("elaborates");
    let m = design.modules().find(|m| m.name() == "m").expect("module m");
    let synthesized = m
        .behaviors()
        .iter()
        .find(|b| matches!(b.kind(), BehaviorKind::Analog))
        .expect("assign synthesizes an Analog behavior");
    assert_eq!(synthesized.body().len(), 1, "one synthesized statement");
    match &synthesized.body()[0] {
        Stmt::Contribution { target, .. } => {
            assert!(
                matches!(target, Expr::AccessFunction { function, pos, neg, .. }
                    if function == "V"
                    && matches!(pos.as_ref(), Expr::Ident { name, .. } if name == "out")
                    && neg.is_none()),
                "expected V(out) with an implicit-ground branch, got {target:?}"
            );
        }
        other => panic!("expected a Contribution statement, got {other:?}"),
    }
}

/// An indexed/hierarchical `assign` target is not yet lowerable — must
/// fail loud (§8.5.3.1) rather than silently doing nothing, matching AC7's
/// blanket rule for every remaining P1 construct. Asserted by typed kind,
/// never `is_err()`.
#[test]
fn indexed_assign_target_fails_loud_by_typed_kind() {
    let src = "
        discipline logic;
            domain discrete;
        enddiscipline

        module m (input logic a);
            logic b [3:0];
            assign b[0] = a;
        endmodule
    ";
    let report = parse_and_elaborate(src, &SourceMap::dummy()).expect_err("indexed target must fail loud");
    let err = report
        .downcast_ref::<ElabError>()
        .unwrap_or_else(|| panic!("elaboration failure is a typed ElabError, got: {report}"));
    match &err.kind {
        ElabErrorKind::Other(msg) => {
            assert!(
                msg.contains("8.5.3.1"),
                "error must cite the LRM clause (§8.5.3.1): {msg}"
            );
        }
        other => panic!("expected ElabErrorKind::Other citing §8.5.3.1, got {other:?}"),
    }
}
