//! Host-surface proof of SOA-on-model (dv-core T27, DVC-16): the shipped
//! spice models (`mos1`, `dio`, `bjt`) carry their own `constraint` blocks,
//! so a designer who never writes a `constraint` declaration gets SOA
//! enforcement for free the moment their model card sets a real limit.
//!
//! Two end-to-end contracts are proven here, both with **zero user-written
//! constraint code** — the requires came from `headers/spice/*`:
//!
//!   * **Inert by default (DVC-16 AC1, design §C5):** every unset limit is
//!     the documented `1e99` sentinel, so each `require` is trivially
//!     satisfied and the default `strict` posture completes cleanly on a
//!     stock card. The shipped gallery does not regress.
//!   * **Loud on violation (DVC-16 AC2/AC4):** gone with the PHDL frontend
//!     — see the note at the bottom of this file.
//!
//! This is the host-side half of DVC-16: cross-crate (lang headers →
//! codegen → solver → host API), driven through [`Session`].

use std::path::PathBuf;

use piperine::{Session, SolverConfig};
use piperine_lang::{Design, SourceMap};

/// The source map a real `piperine` invocation builds: the bundled stdlib
/// headers under the `piperine`/`spice` namespaces, so the test's
/// `use spice::…` resolves exactly as it does on the CLI.
fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

fn elaborate(src: &str) -> Design {
    piperine_lang::parse_and_elaborate(src, &headers_source_map())
        .unwrap_or_else(|e| panic!("elaboration failed: {e:?}"))
}

/// DVC-16 AC1 + design §C5: the **stock gallery** is inert. A mos1 with
/// the default model card (no SOA field overridden) elaborates and
/// simulates unchanged in default `strict` posture — the sentinel-default
/// limits keep every `require` trivially satisfied, so no finding fires
/// and the analysis completes. This is the acceptance test that an
/// invented limit did not slip in: any require that binds to a finite
/// default would trip on the gallery.
#[test]
fn stock_card_is_inert_under_default_strict_posture() {
    const STOCK: &str = r#"`include "disciplines.vams"
`include "spice/sources.va"
`include "spice/passives.va"
`include "spice/mos.va"

module Bias();
    electrical gnd, vdd, d;
    vsrc #(.dc(5.0)) vd (.p(vdd), .n(gnd));
    res  #(.r(10.0e3)) r1 (.p(vdd), .n(d));
    mos1 #(.vto(1.0), .kp(2.0e-5)) m1 (.d(d), .g(d), .s(gnd), .b(gnd));
endmodule
"#;
    let design = elaborate(STOCK);
    let mut session = Session::compile(&design.fork(), "Bias").expect("compiles");
    // Default SolverConfig carries checks = CheckPosture::Strict (DVC-12).
    let op = session.op(&SolverConfig::default(), None).expect("strict op completes clean");
    assert!(op.requires_ok(), "no finding fires on the stock card");
    assert!(op.first_violation().is_none(), "no violation recorded");
    assert!(
        op.v("d".to_string()).unwrap_or_else(|e| panic!("v(d): {e}")) > 0.0,
        "the diode-connected NMOS sits above threshold"
    );
}

// The four "loud on violation" cases (`mos1`/`diode`/`bjt` cards driven past
// a real `vds_max`/`bv_max`/`vce_max`, in `strict` and in `collect`) are gone
// with the PHDL frontend. Their producer is dark end to end: Annex A has no
// `constraint` block production, so `headers/spice/mos.va` says in its own
// header that `vgs_max`/`vds_max`/… "stay declared parameters (unused) …
// the constraint runtime that read it is dark"; T21 deleted
// `pom::ConstraintBlock`/`Module::constraints()`, `lower_bodies` always sets
// `LoweredBody::constraints` to `None`, and `MarginsCatalog::compute`
// returns an empty catalog unconditionally. Nothing a Verilog-AMS model card
// can say produces a finding to assert on. DVC-16 AC1 (the stock gallery is
// inert) is kept and green above — it is the half that never needed the
// constraint runtime to fire.
