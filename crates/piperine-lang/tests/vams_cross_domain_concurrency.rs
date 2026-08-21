//! LRM-COVERS: 5.10.5, 7.3.6, 7.3.6.1, 7.3.6.2, 7.3.6.3, 7.3.6.4, 7.3.6.5, 7.3.7
//! LRM §7.3.6 "Concurrency" (LRMC-C7-06, T36) — the five named
//! synchronization forms between the continuous and discrete domains.
//! Every one of the five is a *permission/semantics* clause, not a
//! prohibition (re-read against `VAMS-LRM-2-4.md` lines 8899-8977 three
//! times over the course of this task to be sure): §7.3.6.1 and §7.3.6.2
//! describe scheduling semantics for an analog/digital event appearing in
//! the other domain's `@()`; §7.3.6.3 and §7.3.6.5 describe value
//! semantics (interpolation vs. real-promoted time) for a primary from
//! one domain read in the other's expressions; §7.3.6.4 permits an
//! analog-event-only-assigned variable in a continuous assign. None of
//! the five has an illegal form to reject — so unlike T27's Annex C gate
//! (a real prohibition list), this target's "check" is a fixture proving
//! each form elaborates, cited to its own sub-clause; two (7.3.6.1,
//! 7.3.6.2) reuse the LRM's own worked examples verbatim (7.3.6.2's is
//! also T27's `vams_annex_c.rs` fixture, viewed from this clause instead
//! of §5.10.5 — both cite the same real construct, legitimately, from two
//! different LRM angles).
//!
//! **§7.3.7 "Function calls" (SPEC_DEVIATION, disclosed not silently
//! dropped):** the immediately adjacent clause ("Digital functions cannot
//! be called from within the analog context. Analog functions cannot be
//! called from within the digital context.") *is* a genuine prohibition,
//! and the natural sixth check to pair with these five — but
//! `FunctionDecl`/`FunctionKind::{Digital,Analog}` (`ast/item.rs`) is
//! parsed and then dropped entirely: no elaboration pass registers a
//! function's name/kind into the POM (confirmed empirically — a
//! `function`/`analog function` pair both elaborate silently regardless
//! of which context calls which, since nothing tracks either). Enforcing
//! §7.3.7 needs a POM function registry this task's stated `Where`
//! (`elab/typecheck.rs` only) doesn't reach — out of scope here, not
//! silently claimed done.

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e}"))
}

/// §7.3.6.1 — the LRM's own worked example: an analog event
/// (`timer(0,100n)`) samples `V(in)` into `smpl`; a digital `always`
/// block is sensitive to `smpl`'s assignment.
#[test]
fn analog_event_sampled_variable_drives_a_digital_event_control() {
    let src = "
        module dut (inout electrical in);
            real smpl;
            analog @(timer(0, 100n))
                smpl = V(in);
            always @(smpl) begin
            end
        endmodule
    ";
    elab(src);
}

/// §7.3.6.2 — the LRM's own worked example: a digital edge (`posedge n`)
/// appears in an analog `@()`.
#[test]
fn digital_edge_appears_in_an_analog_event_control() {
    let src = "
        module dut ();
            reg n;
            real r;
            analog begin
                @(posedge n)
                    r = 3.14;
            end
        endmodule
    ";
    elab(src);
}

/// §7.3.6.3 — an analog primary (`v`, calculated in the continuous
/// domain) read from a digital expression (`always @(v)`).
#[test]
fn analog_primary_appears_in_a_digital_expression() {
    let src = "
        module dut (inout electrical p, inout electrical n);
            real v;
            analog v = V(p, n);
            always @(v) begin
            end
        endmodule
    ";
    elab(src);
}

/// §7.3.6.4 — an analog variable assigned only within an analog event
/// statement (`smpl`, exactly the §7.3.6.1 pattern) drives a continuous
/// assign onto a `wreal`.
#[test]
fn analog_event_only_variable_drives_a_continuous_assign() {
    let src = "
        module dut (inout electrical in);
            real smpl;
            wreal w;
            analog @(timer(0, 100n))
                smpl = V(in);
            assign w = smpl;
        endmodule
    ";
    elab(src);
}

/// §7.3.6.5 — a digital primary (`r`, a `reg`) read from an analog
/// expression (a contribution's RHS).
#[test]
fn digital_primary_appears_in_an_analog_expression() {
    let src = "
        module dut (inout electrical p, inout electrical n);
            reg r;
            analog begin
                V(p, n) <+ (r == 1) ? 1.0 : 0.0;
            end
        endmodule
    ";
    elab(src);
}
