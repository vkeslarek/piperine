//! LRM-COVERS: C.7, C.15
//! VAMS T27 (VAMS-25) — Annex C analog-subset enforcement: a digital-only
//! construct used inside an `analog` behavior body fails loud citing the
//! clause. Ongoing gate (VAMS-25): later phases add more constructs to
//! the same walk (`elab::typecheck::check_analog_subset`).
//!
//! C.7 ("Analog behavior... No digital behavior or events are supported
//! in Verilog-A[;] `casex`/`casez` are not supported in Verilog-A") is
//! this file's core subject matter: `non_blocking_assignment_inside_
//! analog_block_fails_loud_citing_annex_c` and `casex_inside_analog_
//! block_fails_loud` are exactly those two named exceptions.

use piperine_lang::{parse_and_elaborate, SourceMap};

fn elab(src: &str) -> piperine_lang::Design {
    parse_and_elaborate(src, &SourceMap::dummy()).unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

fn elab_err(src: &str) -> String {
    parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("expected elaboration error")
        .to_string()
}

#[test]
fn non_blocking_assignment_inside_analog_block_fails_loud_citing_annex_c() {
    let src = "
        module m (inout electrical p, inout electrical n);
            analog begin
                x <= 1.0;
            end
        endmodule
    ";
    let msg = elab_err(src);
    assert!(msg.contains("non-blocking"), "names the offending construct: {msg}");
    assert!(msg.contains("Annex C"), "cites the LRM annex: {msg}");
}

#[test]
fn posedge_event_control_inside_analog_block_is_legal_per_5_10_5() {
    // LRMC-C5-05 (T27): §5.10.5 explicitly permits "analog behavior ...
    // made sensitive to digital events, including posedge events" — this
    // was a real, undisclosed over-broad rejection before T27 (the whole
    // point of this LRMC-ID), not a construct the LRM ever restricted.
    let src = "
        module m (inout electrical p, inout electrical n);
            analog begin
                @(posedge clk);
            end
        endmodule
    ";
    let d = elab(src);
    d.module("m").expect("posedge inside analog elaborates per §5.10.5");
}

#[test]
fn casex_inside_analog_block_fails_loud() {
    let src = "
        module m (inout electrical p, inout electrical n);
            analog begin
                casex (sel)
                    1: V(p, n) <+ 1.0;
                    default: V(p, n) <+ 0.0;
                endcase
            end
        endmodule
    ";
    let msg = elab_err(src);
    assert!(msg.contains("casex"), "names the offending construct: {msg}");
    assert!(msg.contains("Annex C"), "cites the LRM annex: {msg}");
}

#[test]
fn ordinary_analog_contribution_is_unaffected() {
    let d = elab(
        r#"
        module res (inout electrical p, inout electrical n);
            analog begin
                V(p, n) <+ 1.0;
            end
        endmodule
        "#,
    );
    assert!(d.module("res").is_some());
}

#[test]
fn non_blocking_assignment_inside_a_digital_always_block_is_unaffected() {
    // Annex C restricts *analog* blocks only — the same construct is
    // ordinary and legal inside `always`.
    let d = elab(
        r#"
        module m ();
            always begin
                q <= 1;
            end
        endmodule
        "#,
    );
    assert!(d.module("m").is_some());
}
