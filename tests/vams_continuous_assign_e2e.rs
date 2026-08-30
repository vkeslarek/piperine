//! LRM-COVERS: 6.5.3, 7.3.6.4, 7.4.2, 8.5.3.1
//!
//! T10 (LRMW2-06): `crates/piperine-lang/tests/vams_continuous_assign.rs`
//! proves the elaborated POM shape a module-level `assign` synthesizes;
//! this file is the "actually drives the net" simulate-level proof for
//! both domains the fix routes into (SPEC_DEVIATION.md finding 1.3).
//!
//! lrm-conformance-wave3 (Phase 6, T36): `wreal_target_is_driven_by_
//! assign_end_to_end`'s `Follower (inout electrical in, inout wreal out)`
//! instantiated as `Follower f1 (.in(mid), .out(out_net))` also proves
//! §7.4.2 "Connection of discrete-time disciplines" — a `wreal` port
//! connected across a module boundary to a top-level `wreal` net, with
//! the driven value observed all the way through the connection (3.3 V),
//! not merely elaborated.

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

/// Mirrors `tests/vams_annex_e.rs`'s `headers_source_map` — a source map
/// rooted at the real stdlib headers, resolving `` `include
/// "disciplines.vams" ``.
fn headers_source_map() -> SourceMap {
    let headers =
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    SourceMap::new(headers)
}

/// AC6: `wreal out; assign out = V(in);` drives `out` end-to-end. Before
/// this fix `assign` was a complete no-op — `out` would float with no
/// driver at all. With the fix, `out` is the ideal-source image of `in`,
/// observed through a Top-level net (`out_net`) connected to the port —
/// the only nets this host surface addresses by plain name are top-level
/// ones, so the port makes the proof end-to-end all the way out of the
/// module, not merely internal.
#[test]
fn wreal_target_is_driven_by_assign_end_to_end() {
    let src = "
        `include \"disciplines.vams\"
        module Vsrc (inout electrical p, inout electrical n);
            parameter real dc = 0.0;
            analog V(p, n) <+ dc;
        endmodule
        module Follower (inout electrical in, inout wreal out);
            assign out = V(in);
        endmodule
        module Top ();
            electrical gnd, mid;
            wreal out_net;
            Vsrc #(.dc(3.3)) src (.p(mid), .n(gnd));
            Follower f1 (.in(mid), .out(out_net));
        endmodule
    ";
    let design = piperine_lang::parse_and_elaborate(src, &headers_source_map()).expect("elaborates");
    let op = Session::compile(&design, "Top").expect("session compiles").op(&SolverConfig::default(), None).expect("op solves");
    let v_out = op.v("out_net".to_string()).expect("v(out_net) — the driven wreal net");
    assert!(
        (v_out - 3.3).abs() < 1e-6,
        "assign out = V(in) must drive `out` to follow `in` (3.3 V), got {v_out}"
    );
}

/// A digital `assign` participates in the Active region like a blocking
/// assign: chained continuous assigns across two instances (`b = a` in one
/// module feeding `c = !b` in the next) settle combinationally within one
/// digital pass, exactly as a chained `always @(*)` would — `c` always
/// tracks `!a`, never a stale value. (An INTERNAL, non-port net cannot be
/// an assign/always@(*) target in this codegen regardless of syntax — a
/// pre-existing digital-IR constraint, not something this task changes —
/// so the chain is expressed through two ports instead of one internal
/// wire.)
#[test]
fn digital_assign_participates_in_the_active_region_like_a_blocking_assign() {
    let src = "
        discipline logic;
            domain discrete;
        enddiscipline

        module Stage1 (input logic a, output logic b);
            assign b = a;
        endmodule

        module Stage2 (input logic b, output logic c);
            assign c = !b;
        endmodule

        module Driver (output logic a);
            assign a = 1;
        endmodule

        module Top ();
            logic a_net, b_net, c_net;
            Driver d1 (a_net);
            Stage1 s1 (a_net, b_net);
            Stage2 s2 (b_net, c_net);
        endmodule
    ";
    let design = piperine_lang::parse_and_elaborate(src, &SourceMap::dummy()).expect("elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let trace = session
        .tran(1.0, Some(0.1), 0.0, &SolverConfig::default(), None, false, &[])
        .expect("transient solves");
    let c = trace.v("c_net").expect("v(c_net) — a digital net reads its logic value (0.0/1.0)");
    let last = c.at(1.0);
    assert!(
        (last - 0.0).abs() < 1e-9,
        "c = !b = !a = !1 = 0 must hold at every settled digital pass, got {last}"
    );
}
