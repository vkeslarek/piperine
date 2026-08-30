//! LRM-COVERS: 4.2.1.2, A.2.2, A.2.2.2
//!
//! lrm-conformance-wave3 (Phase 6, T43): originally two genuine gaps left
//! after T36-T41's backfill; the drive-strength gap below was closed by
//! a later gap-fix batch (LRMW3-09) — see that commit for the real
//! grammar/lowering/`DriverTable` implementation and
//! `crates/piperine-codegen/tests/vams_drive_strength_e2e.rs` for the
//! end-to-end Table 7-3 proof. This file keeps a smoke test here (parse +
//! elaborate succeeds) since §A.2.2.2 coverage lived here first.
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

/// LRMW3-09 (gap-fix batch): `drive_strength` on a continuous assign now
/// has a real grammar production and elaborates cleanly — this is the
/// language-layer smoke test; `vams_drive_strength_e2e.rs` (piperine-
/// codegen) proves the strength actually reaches the scheduler's
/// `DriverTable` and resolves per Table 7-3.
#[test]
fn drive_strength_on_a_continuous_assign_parses_and_elaborates() {
    // A `discipline ... domain discrete;` net (not the bare `wire`
    // keyword — a separate, narrower pre-existing gap in this same
    // lowering's digital/analog domain check does not special-case
    // `NetType::Digital1364`, unrelated to this task) is this codebase's
    // standard pure-digital fixture idiom (see `tests/
    // procedural_continuous_assign_e2e.rs`).
    let src = "\
        discipline logic; domain discrete; enddiscipline\n\
        module top ();\nlogic w, r;\nassign (strong1, weak0) w = r;\nendmodule\n";
    let _ = parse_and_elaborate(src, &SourceMap::dummy())
        .expect("drive_strength on a continuous assign must parse and elaborate (§A.2.2.2, LRMW3-09)");
}
