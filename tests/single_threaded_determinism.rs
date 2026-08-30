//! LRM-COVERS: 8.4.3.1
//!
//! lrm-conformance-wave3 (Phase 6, T39): §8.4.3.1 "Concurrency" — "Most
//! (current) simulators are single-threaded in execution... If an
//! implementation is genuinely multi-threaded, it should not evaluate
//! processes that directly share memory concurrently, as there are no
//! data locking semantics in Verilog-AMS." Piperine's solver is
//! single-threaded in its hot path (no thread spawning anywhere in
//! `piperine-solver`'s analysis loop) — the practical, observable
//! consequence is that solving the identical circuit twice gives
//! bit-identical results, with no data-race-induced nondeterminism. This
//! target proves that observable consequence directly rather than
//! asserting "no threads" as an unfalsifiable structural claim.

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

const SRC: &str = "
module R (inout electrical p, inout electrical n);
    parameter real r = 1.0e3;
    analog I(p, n) <+ V(p, n) / r;
endmodule

module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog V(p, n) <+ dc;
endmodule

module Top ();
    electrical gnd, top, mid;
    Vsrc v1 (.p(top), .n(gnd));
    R r1 (.p(top), .n(mid));
    R r2 (.p(mid), .n(gnd));
endmodule
";

fn v_mid_twice() -> (f64, f64) {
    let design = piperine_lang::parse_and_elaborate(SRC, &SourceMap::dummy()).expect("elaborates");
    let mut session1 = Session::compile(&design, "Top").expect("session compiles");
    let v1 = session1.op(&SolverConfig::default(), None).expect("op solves").v("mid").expect("v(mid)");
    let mut session2 = Session::compile(&design, "Top").expect("session compiles");
    let v2 = session2.op(&SolverConfig::default(), None).expect("op solves").v("mid").expect("v(mid)");
    (v1, v2)
}

#[test]
fn repeated_solves_of_the_same_circuit_are_bit_identical() {
    let (v1, v2) = v_mid_twice();
    assert_eq!(v1.to_bits(), v2.to_bits(), "no data-race-induced nondeterminism between two independent solves of the same circuit");
}
