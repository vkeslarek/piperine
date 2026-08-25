//! LRM-COVERS: 1.3.1.2, 3.12
//! Wave-2 (T69) re-reading, no new tests: §1.3.1.2 "Reference directions"
//! is the `+`/`-` terminal convention (Figure 1-2) a branch's declared
//! `(pos, neg)` pair encodes directly — `named_branch_round_trips_
//! through_accessors` asserts `b.pos() == pos("a")`/`b.neg() ==
//! Some(pos("b"))` for `branch (a, b) br;`, i.e. `a` is the `+` terminal
//! and `b` the `-` terminal, matching declared order. §3.12 "Branches" is
//! this file's own subject, cited throughout its tests already; this adds
//! the file-header declaration to match.
//!
//! VAMS T18 (VAMS-02) — POM `Branch` node, LRM §3.12 / §5.
//!
//! Asserts the structural contract of the POM `Branch` type:
//! - named, implicit and ground-referenced branches are representable;
//! - the kind conflict (potential ↔ flow on the same branch) is a typed
//!   E4xxx error citing LRM §5.4.2 with both contribution spans labelled;
//! - the §5.6.5 switch-branch exception is modelled as a distinct kind,
//!   not by relaxing the conflict check.
//!
//! The lowering that produces these from `piperine_lang::ast::BranchDecl`
//! and from access-function call sites is T23; this target proves the POM
//! type's contract on its own.

use piperine_lang::pom::{
    Branch, BranchKind, ElabErrorKind, Kind, Kinded, Named, NetRef,
};

fn pos(p: &str) -> NetRef {
    NetRef::simple(p)
}
fn pos_idx(p: &str, i: u64) -> NetRef {
    NetRef::indexed(p, i)
}

fn named_branch(name: &str, p: &str, n: &str) -> Branch {
    Branch {
        span: None,
        doc: None,
        name: Some(name.into()),
        pos: pos(p),
        neg: Some(pos(n)),
        kind: BranchKind::Probe,
    }
}

fn implicit_branch(p: &str, n: &str) -> Branch {
    Branch {
        span: None,
        doc: None,
        name: None,
        pos: pos(p),
        neg: Some(pos(n)),
        kind: BranchKind::Probe,
    }
}

fn ground_branch(name: &str, p: &str) -> Branch {
    Branch {
        span: None,
        doc: None,
        name: Some(name.into()),
        pos: pos(p),
        neg: None,
        kind: BranchKind::Probe,
    }
}

#[test]
fn named_branch_round_trips_through_accessors() {
    // LRM §3.12: "branch (a, b) br;" — a named branch with two nets.
    let b = named_branch("br", "a", "b");
    assert_eq!(b.name(), Some("br"));
    assert_eq!(b.pos(), &pos("a"));
    assert_eq!(b.neg(), Some(&pos("b")));
    assert!(!b.is_implicit());
    assert!(!b.is_ground_referenced());
    assert_eq!(b.kind(), BranchKind::Probe);
}

#[test]
fn implicit_branch_has_no_name_and_is_marked_implicit() {
    // LRM §3.12: "There shall be at most one unnamed branch between any
    // two nets" — an access function V(a,b) creates an implicit branch.
    let b = implicit_branch("a", "b");
    assert_eq!(b.name(), None);
    assert!(b.is_implicit());
    assert_eq!(Named::name(&b), ""); // siblings' convention for unnamed nodes
}

#[test]
fn ground_referenced_branch_has_no_negative_terminal() {
    // LRM §3.12: "Only one net need be specified, in which case the
    // second net defaults to ground."
    let b = ground_branch("g", "a");
    assert_eq!(b.neg(), None);
    assert!(b.is_ground_referenced());
    assert_eq!(b.pos(), &pos("a"));
}

#[test]
fn kinded_returns_the_branch_discriminant() {
    let b = named_branch("br", "a", "b");
    assert_eq!(Kinded::kind(&b), Kind::Branch);
}

#[test]
fn indexed_terminals_are_preserved() {
    // LRM §3.12 vector terminals: a branch terminal may be a single
    // vector element (bus[3]). The POM NetRef carries the index.
    let b = Branch {
        span: None,
        doc: None,
        name: Some("tap".into()),
        pos: pos_idx("bus", 3),
        neg: Some(pos_idx("bus", 0)),
        kind: BranchKind::Probe,
    };
    assert_eq!(b.pos(), &pos_idx("bus", 3));
    assert_eq!(b.neg(), Some(&pos_idx("bus", 0)));
}

#[test]
fn first_potential_contribution_promotes_probe_to_potential() {
    // LRM §5.4.2: a branch with no contribution is a probe; the first
    // contribution promotes it to its source kind.
    let mut b = named_branch("br", "a", "b");
    assert_eq!(b.kind(), BranchKind::Probe);
    assert!(b.record_contribution(BranchKind::Potential, None, "m").is_ok());
    assert_eq!(b.kind(), BranchKind::Potential);
    assert!(b.kind().is_potential());
    assert!(b.kind().is_source());
}

#[test]
fn first_flow_contribution_promotes_probe_to_flow() {
    let mut b = named_branch("br", "a", "b");
    assert!(b.record_contribution(BranchKind::Flow, None, "m").is_ok());
    assert_eq!(b.kind(), BranchKind::Flow);
    assert!(b.kind().is_flow());
    assert!(b.kind().is_source());
}

#[test]
fn same_kind_contribution_does_not_conflict() {
    // Multiple V(p,n) <+ ... contributions to the same branch are normal
    // (LRM §5.6 — contributions accumulate). Only potential-vs-flow is
    // a conflict.
    let mut b = named_branch("br", "a", "b");
    assert!(b.record_contribution(BranchKind::Potential, None, "m").is_ok());
    assert!(b.record_contribution(BranchKind::Potential, None, "m").is_ok());
    assert_eq!(b.kind(), BranchKind::Potential);
}

#[test]
fn potential_then_flow_conflicts_loud_with_e4001_citing_lrm_5_4_2() {
    // LRM §5.4.2: "A branch cannot simultaneously be both a potential and
    // a flow source". The check fires on the second contribution, with
    // both spans labelled (E-STD 6) and the LRM clause cited.
    let mut b = Branch {
        span: Some(miette::SourceSpan::new(10usize.into(), 5usize)),
        doc: None,
        name: Some("br".into()),
        pos: pos("a"),
        neg: Some(pos("b")),
        kind: BranchKind::Probe,
    };
    assert!(b.record_contribution(BranchKind::Potential, None, "m").is_ok());

    let second_span = miette::SourceSpan::new(50usize.into(), 5usize);
    let err = b
        .record_contribution(BranchKind::Flow, Some(second_span), "m")
        .expect_err("potential→flow must conflict");

    let ElabErrorKind::BranchKindConflict {
        module,
        branch,
        first,
        second,
    } = &err.kind
    else {
        panic!("expected BranchKindConflict, got {:?}", err.kind);
    };

    assert_eq!(module, "m");
    assert_eq!(branch, "br");
    // Both spans carried and labelled (E-STD 6).
    assert_eq!(*first, Some(miette::SourceSpan::new(10usize.into(), 5usize)));
    assert_eq!(*second, Some(second_span));
    // The variant's #[error] message cites LRM §5.4.2 verbatim.
    let rendered = err.kind.to_string();
    assert!(rendered.contains("§5.4.2"), "message must cite LRM §5.4.2: {rendered}");
    assert!(rendered.contains("br"), "message must name the branch: {rendered}");
    assert!(rendered.contains("m"), "message must name the module: {rendered}");
    // E-STD 2: numbered diagnostic code.
    let code = miette::Diagnostic::code(&err.kind);
    assert_eq!(code.map(|c| c.to_string()), Some("E4001".to_string()));
}

#[test]
fn flow_then_potential_conflicts_symmetrically() {
    // The conflict is symmetric — flow source first, potential second.
    let mut b = named_branch("br", "a", "b");
    assert!(b.record_contribution(BranchKind::Flow, None, "m").is_ok());
    let err = b
        .record_contribution(BranchKind::Potential, None, "m")
        .expect_err("flow→potential must conflict");
    assert!(matches!(err.kind, ElabErrorKind::BranchKindConflict { .. }));
}

#[test]
fn switch_branch_allows_both_potential_and_flow() {
    // LRM §5.6.5: a switch branch may toggle between potential and flow
    // at runtime. Marking the branch as a switch disarms the conflict
    // check — modelled explicitly via BranchKind::Switch, never by
    // relaxing the regular Potential/Flow check.
    let mut b = named_branch("out", "p", "n");
    b.mark_switch();
    assert_eq!(b.kind(), BranchKind::Switch);
    assert!(b.kind().allows_both_potential_and_flow());

    // Both contributions accepted; kind stays Switch (not relaxed to one side).
    assert!(b.record_contribution(BranchKind::Potential, None, "m").is_ok());
    assert!(b.record_contribution(BranchKind::Flow, None, "m").is_ok());
    assert_eq!(b.kind(), BranchKind::Switch);
}

#[test]
fn unnamed_branch_label_synthesizes_pair_form() {
    // LRM §3.12: implicit branches are addressed by their net pair, not
    // a name. The error must produce a readable label.
    let mut b = implicit_branch("a", "b");
    assert!(b.record_contribution(BranchKind::Potential, None, "m").is_ok());
    let err = b
        .record_contribution(BranchKind::Flow, None, "m")
        .expect_err("implicit branch must still conflict");
    let ElabErrorKind::BranchKindConflict { branch, .. } = &err.kind else {
        panic!("expected BranchKindConflict");
    };
    assert!(!branch.is_empty());
    assert!(branch.contains('a') && branch.contains('b'));
}

#[test]
fn branch_preserves_doc_comment() {
    // MD-25 + LSP-07: every POM node preserves its authored `///` doc.
    let b = Branch {
        doc: Some("The output branch.".into()),
        ..named_branch("out", "p", "n")
    };
    assert_eq!(b.doc(), Some("The output branch."));
}
