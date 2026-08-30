//! LRM-COVERS: 4.2.1.2, A.2.2, A.2.2.2
//!
//! lrm-conformance-wave3 (Phase 6, T43): the final three genuine gaps
//! left after T36-T41's backfill — neither a deliberate scope decision
//! (so neither belongs in `coverage_exclusions.toml`, per spec §0 rule 1:
//! an OutOfScope row needs a reason naming what *replaces* the LRM
//! mechanism, not "not implemented yet") nor a bare-claim opportunity.
//! Both are declared limitations with a pinning test, the same pattern
//! this phase already used for §4.6.4.6/§9.5.9/§9.7.1/§9.7.2/§7.8.5.1.
//!
//! **§4.2.1.2 "Integer to real conversion"** — "Implicit conversion
//! shall take place when an expression is assigned to a real. Individual
//! bits that are `x` or `z` in the net or the variable shall be an
//! error." The implicit-conversion half already works (proven below,
//! `integer_to_real_conversion_is_implicit`); the x/z-bits-are-an-error
//! half is a **declared limitation**: an integral value carrying `x`/`z`
//! bits assigned to a `real` is silently accepted (coerced, not
//! rejected) — `assigning_an_x_valued_integral_to_real_is_not_rejected_
//! declared_limitation` pins the current behavior. Implementing the real
//! rule needs runtime four-state-value inspection at assignment time (the
//! value is not always an elaboration constant), a genuine runtime
//! feature beyond this coverage-backfill phase's scope.
//!
//! **§A.2.2/§A.2.2.2 "Declaration data types" / "Strengths"** — the
//! `drive_strength` grammar production (`(strong1, weak0)` etc. on a
//! continuous `assign`) is not implemented at all: no grammar production
//! parses it. §A.2.2.1 (net/variable types, the sibling subsection under
//! the same §A.2.2 parent) is already covered elsewhere
//! (`annex_a_grammar.rs`, `vams_lower_var.rs`); §A.2.2 itself is purely
//! organizational over both subsections, so it is declared here
//! alongside §A.2.2.2 rather than needing its own separate proof.
//! `drive_strength_on_a_continuous_assign_is_not_yet_parsed_declared_
//! limitation` pins the current (loud, not silent — a plain parse error)
//! rejection. Implementing this needs a new grammar production plus
//! threading the parsed strength pair through to `DriverTable` (spec
//! LRMW3-09) — a real language feature, not a coverage-backfill task.

use piperine_lang::{parse_and_elaborate, SourceMap};

#[test]
fn integer_to_real_conversion_is_implicit() {
    let src = "module dut ();\ninteger i;\nreal r;\ninitial begin\ni = 5;\nr = i;\nend\nendmodule\n";
    let _ = parse_and_elaborate(src, &SourceMap::dummy()).expect("implicit integer-to-real assignment must elaborate");
}

#[test]
fn assigning_an_x_valued_integral_to_real_is_not_rejected_declared_limitation() {
    let src = "module dut ();\nreg [3:0] q;\nreal r;\ninitial begin\nq = 4'bxxxx;\nr = q;\nend\nendmodule\n";
    // Current (limited) behavior: elaborates without error — §4.2.1.2's
    // own "shall be an error" rule is not enforced. Pinned so a future
    // implementation of the real rule is a deliberate, reviewed test
    // change, not a silent regression the other direction.
    let _ = parse_and_elaborate(src, &SourceMap::dummy())
        .expect("current behavior: an x-valued integral assigned to real is accepted, not rejected");
}

#[test]
fn drive_strength_on_a_continuous_assign_is_not_yet_parsed_declared_limitation() {
    let src = "module top ();\nwire w, r;\nassign (strong1, weak0) w = r;\nendmodule\n";
    let err = parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("drive_strength on assign has no grammar production — a parse error, not a silent drop");
    let msg = format!("{err}");
    assert!(!msg.is_empty(), "the rejection must at least carry a real (non-empty) parse-error message");
}
