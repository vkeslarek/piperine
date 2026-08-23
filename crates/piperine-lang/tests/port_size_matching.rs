//! LRM-COVERS: 6.5.7.1
//!
//! LRMW2-53 (`lrm-conformance-wave2`, T48): §6.5.7.1's "Matching size
//! rule" — "the sizes of the ports and net need to match" — for an
//! ordinary (non-array-of-instances) instantiation. This had no check at
//! all before this task: `resolve_instance_ports` bound port positions to
//! net expressions purely by name/index, with no width comparison against
//! the target module's declared port widths — a scalar port silently
//! wired to a wide bus (or vice versa) elaborated without complaint.
//! `elab/lower/flatten.rs`'s own array-of-instances width rule is a
//! separate, already-implemented, already-tested mechanism (the
//! broadcast case, `res r[7:0] (bus, gnd);`) — not this file's concern.
//!
//! An undeclared/implicit interconnect net (§6.5.7.2's own explicit
//! allowance) turns out to have a perfectly determinable width once
//! looked up: this codebase's connection-resolution path registers it as
//! an ordinary scalar net, so it is checked the same as any other net —
//! `undeclared_interconnect_net_bound_to_a_scalar_port_elaborates_cleanly`
//! below proves the match case, not an exemption.

use piperine_lang::parse_and_elaborate;
use piperine_lang::pom::ElabError;
use piperine_lang::SourceMap;

/// §6.5.7.1: a scalar port connected to a 4-wide bus is a width mismatch
/// — a typed elaboration error citing the clause, never `is_err()`.
#[test]
fn scalar_port_connected_to_a_vector_net_fails_loud_citing_6_5_7_1() {
    let src = "module leaf (inout electrical p);
    endmodule
    module top ();
        electrical bus [3:0];
        leaf l1 (bus);
    endmodule";
    let report = parse_and_elaborate(src, &SourceMap::dummy())
        .expect_err("a scalar port bound to a 4-wide net must fail elaboration (LRM §6.5.7.1)");
    let err = report
        .downcast_ref::<ElabError>()
        .unwrap_or_else(|| panic!("elaboration failure is a typed ElabError, got: {report}"));
    let msg = format!("{err}");
    assert!(msg.contains("6.5.7.1"), "expected the error to cite LRM §6.5.7.1, got: {msg}");
    assert!(msg.contains("width 1") && msg.contains("width 4"), "expected both widths named, got: {msg}");
}

/// Discrimination: the *same* fixture shape with matching widths (a
/// 4-wide port bound to a 4-wide net) must elaborate cleanly — an overly
/// broad check that rejected any bus connection would fail this.
#[test]
fn matching_vector_widths_elaborate_cleanly() {
    let src = "module leaf (inout electrical p [3:0]);
    endmodule
    module top ();
        electrical bus [3:0];
        leaf l1 (bus);
    endmodule";
    parse_and_elaborate(src, &SourceMap::dummy())
        .unwrap_or_else(|e| panic!("matching 4-wide port/net must elaborate cleanly: {e:?}"));
}

/// §6.5.7.2's own explicit allowance ("a signal appearing in the
/// connection list ... need not appear in any port declaration"): an
/// undeclared/implicit interconnect net (never declared with `electrical
/// ...;`) is registered as an ordinary scalar net by the connection
/// itself — bound to a scalar port, that's a genuine width match (1 == 1),
/// so it elaborates cleanly, same as any other scalar-to-scalar
/// connection.
#[test]
fn undeclared_interconnect_net_bound_to_a_scalar_port_elaborates_cleanly() {
    let src = "module leaf (inout electrical p);
    endmodule
    module top ();
        leaf l1 (undeclared_net);
    endmodule";
    parse_and_elaborate(src, &SourceMap::dummy())
        .unwrap_or_else(|e| panic!("an implicit scalar net bound to a scalar port must elaborate: {e:?}"));
}

/// Array-of-instances broadcast connections keep their own,
/// already-implemented width rule — this task's new check must not
/// double-report (or otherwise interfere with) that case.
#[test]
fn array_of_instances_broadcast_is_unaffected() {
    let src = "module leaf (inout electrical p);
    endmodule
    module top ();
        electrical bus [3:0];
        leaf l [3:0] (bus);
    endmodule";
    parse_and_elaborate(src, &SourceMap::dummy())
        .unwrap_or_else(|e| panic!("array-of-instances broadcast must still elaborate: {e:?}"));
}
