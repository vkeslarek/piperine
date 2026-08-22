//! LRM-COVERS: 5.6.5, 5.4.2
//!
//! LRMW2-40 (`lrm-conformance-wave2`, T45): the elaborator's
//! `promote_to_switch_if_conditional` (`elab/lower/vams.rs`) marks a
//! branch `BranchKind::Switch` the moment a conditional (`if`/`case`)
//! contribution conflicts in kind with the branch's already-recorded
//! kind — the §5.6.5 exception `pom_branch.rs` already proves the POM
//! `Branch` type *can* represent, but which had **no elaborator call
//! site** producing it before this task (`branch.rs`'s own doc comment:
//! "no call sites today"). This target proves the elaborator side: the
//! LRM's own relay pattern promotes to `Switch`, an *unconditional*
//! potential/flow conflict on the same branch still fails §5.4.2's
//! ordinary check (this fix must not over-relax it), and a conflict
//! reached only through an `@(...)` event-control body — §5.6.5's own
//! "not allowed within event control statements" restriction — also
//! stays a hard error.

use piperine_lang::parse_and_elaborate;
use piperine_lang::pom::{BranchKind, ElabError, ElabErrorKind};
use piperine_lang::SourceMap;

fn module_branch_kind(src: &str, module: &str) -> BranchKind {
    let design = parse_and_elaborate(src, &SourceMap::dummy())
        .unwrap_or_else(|e| panic!("elaborates: {e:?}"));
    let m = design.module(module).unwrap_or_else(|| panic!("module `{module}` exists"));
    assert_eq!(m.branches().len(), 1, "expected exactly one branch, got {:?}", m.branches());
    m.branches()[0].kind()
}

/// §5.6.5: the LRM's own relay pattern — a conditional potential
/// contribution on the `then` arm, a conditional flow contribution on the
/// `else` arm, same branch — elaborates and the branch is marked `Switch`.
#[test]
fn conditional_potential_flow_conflict_promotes_to_switch() {
    let src = "module relay (inout electrical p, n, cp, cn);
        parameter real thresh = 0;
        analog begin
            if (V(cp, cn) > thresh)
                V(p, n) <+ 0;
            else
                I(p, n) <+ 0;
        end
    endmodule";
    assert_eq!(module_branch_kind(src, "relay"), BranchKind::Switch);
}

/// Discrimination: swap which arm carries which nature — still `Switch`,
/// proving the promotion isn't order-dependent (an implementation that
/// only checked "potential-then-flow" would miss this).
#[test]
fn order_of_conflicting_arms_does_not_matter() {
    let src = "module relay (inout electrical p, n, cp, cn);
        parameter real thresh = 0;
        analog begin
            if (V(cp, cn) > thresh)
                I(p, n) <+ 0;
            else
                V(p, n) <+ 0;
        end
    endmodule";
    assert_eq!(module_branch_kind(src, "relay"), BranchKind::Switch);
}

/// §5.4.2: an *unconditional* potential/flow conflict on the same branch
/// is still the ordinary hard error — the switch exception must not
/// over-relax the general rule.
#[test]
fn unconditional_potential_flow_conflict_still_fails_by_typed_kind() {
    let src = "module dut (inout electrical p, inout electrical n);
        analog begin
            V(p, n) <+ 1.0;
            I(p, n) <+ 1.0;
        end
    endmodule";
    let report = parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("an unconditional potential/flow conflict must fail elaboration (LRM §5.4.2)");
    let err = report
        .downcast_ref::<ElabError>()
        .unwrap_or_else(|| panic!("elaboration failure is a typed ElabError, got: {report}"));
    assert!(
        matches!(&err.kind, ElabErrorKind::BranchKindConflict { .. }),
        "expected BranchKindConflict, got: {err:?}"
    );
}

/// §5.6.5: "Usage of contribution statements inside event control
/// statements is disallowed" for the switch mechanism — a conflict first
/// arising inside an `@(...)` body does not get the exception.
#[test]
fn conflict_inside_event_control_still_fails_by_typed_kind() {
    let src = "module dut (inout electrical p, inout electrical n, inout electrical c);
        analog begin
            V(p, n) <+ 1.0;
            @(cross(V(c), 0))
                I(p, n) <+ 1.0;
        end
    endmodule";
    let report = parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("a conflict reached only via event control must fail elaboration (LRM §5.4.2/§5.6.5)");
    let err = report
        .downcast_ref::<ElabError>()
        .unwrap_or_else(|| panic!("elaboration failure is a typed ElabError, got: {report}"));
    assert!(
        matches!(&err.kind, ElabErrorKind::BranchKindConflict { .. }),
        "expected BranchKindConflict, got: {err:?}"
    );
}
