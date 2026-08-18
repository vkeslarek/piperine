//! dv-core T28 (DVC-19) — analysis-scoped event blocks, end-to-end: a
//! `@ dc { … }`-scoped `require` is evaluated only in DC analyses; under
//! strict posture, transient startup that violates the DC rule passes
//! tran and fails DC. A scoped require whose analysis never ran reports
//! "not exercised" on the host (never a silent vacuous pass).
//!
//! Fixture: a resistor divider with `v1.dc = 1.0` puts `out` at 0.5 V.
//! `@ dc { require vds_dc : V(p, n) <= 0.4 }` is violated at DC (margin
//! `0.4 − 0.5 = −0.1`); the same rule, scoped to DC, is **not** evaluated
//! during a transient whose solution converges to the same 0.5 V point —
//! strict tran passes. A top-level (unscoped) `require` reading the same
//! predicate would fail tran; this fixture separates the two scopes to
//! prove `@ dc` is honored.
//!
//! **Migrated** (constraint-scoping): the fixtures run against the
//! `constraint begin … end` producer with the analysis-scoped groups
//! spelled `@ dc { … }` / `@ tran { … }` / `@ ac { … }` inside the module.

use piperine_api::prelude::CheckPosture;
use piperine_api::{Session, SolverConfig};
use piperine_lang::SourceMap;

/// The `SourceMap` reaching `piperine-lang`'s bundled headers.
fn source_map() -> SourceMap {
    let headers = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone());
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

const DIVIDER: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    parameter real hi = 0.4;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        @ dc { require vds_dc : V(p, n) <= hi; }
        @ tran { require vds_tran : V(p, n) <= hi; }
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;

/// An AC-only constraint block on an instance the DC sweep touches — its
/// scoped require is never exercised at DC, and `requires_ok` reports
/// false (never a silent vacuous pass).
const AC_ONLY: &str = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    parameter real hi = 0.4;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        @ ac { require ac_headroom : V(p, n) <= hi; }
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;

fn compile(src: &str) -> Session {
    let design = piperine_lang::parse_and_elaborate(src, &source_map()).expect("elaborates");
    Session::compile(&design, "Top").expect("session compiles")
}

fn strict_config() -> SolverConfig {
    SolverConfig { checks: CheckPosture::Strict, ..Default::default() }
}

fn collect_config() -> SolverConfig {
    SolverConfig { checks: CheckPosture::Collect, ..Default::default() }
}

/// DVC-19 AC5 (the acceptance test): the `@ dc` rule is violated at DC
/// (margin `0.4 − 0.5 = −0.1`), so a strict DC analysis fails loud naming
/// the constraint and instance. The same fixture run as a transient
/// (whose solution passes through the same 0.5 V point but is scoped to
/// `@ tran` only) completes cleanly — the DC-scoped require is not
/// evaluated there.
#[test]
fn strict_dc_fails_when_dc_scoped_require_is_violated_strict_tran_passes() {
    let mut dc_session = compile(DIVIDER);
    let dc_err = session_dc_fails(&mut dc_session);
    assert!(
        dc_err.contains("vds_dc") && dc_err.contains("dut1"),
        "strict DC must fail naming the dc-scoped constraint and instance: {dc_err}"
    );

    // The transient startup passes through the same 0.5 V operating
    // point; under strict posture it completes because the DC-scoped
    // require is not evaluated during tran. The `@ tran` rule is also
    // violated at the same point — to prove `@ dc` is honored, run a
    // variant where the tran-scoped require's `hi` is loose enough to
    // always hold.
    let mut tran_session = compile(DIVIDER);
    let _ = tran_session
        .tran(1e-3, Some(1e-4), 0.0, &strict_config(), None, false, &[])
        .expect_err("the @ tran rule fires on the same point — strict tran fails too");

    // Now the variant: only `@ dc` carries the violated rule; tran has no
    // rule at all. Strict tran must pass even though the DC point that
    // lives under it would fail the DC rule.
    let tran_only = r#"`include "disciplines.vams"
module Vsrc (inout electrical p, inout electrical n);
    parameter real dc = 1.0;
    analog begin
        V(p, n) <+ dc;
    end
endmodule
module R (inout electrical p, inout electrical n);
    parameter real r = 1e3;
    analog begin
        I(p, n) <+ V(p, n) / r;
    end
endmodule
module Dut (inout electrical p, inout electrical n);
    parameter real hi = 0.4;
    analog begin
        I(p, n) <+ V(p, n) / 1e3;
    end
    constraint begin
        @ dc { require vds_dc : V(p, n) <= hi; }
    end
endmodule
module Top ();
    electrical gnd, in, out;
    Vsrc v1 (.p(in), .n(gnd));
    R r1 (.p(in), .n(out));
    Dut dut1 (.p(out), .n(gnd));
endmodule
"#;
    let mut tran_only_session = compile(tran_only);
    tran_only_session
        .tran(1e-3, Some(1e-4), 0.0, &strict_config(), None, false, &[])
        .expect("strict tran passes — the dc-scoped require is not evaluated in tran");
}

fn session_dc_fails(session: &mut Session) -> String {
    session
        .op(&strict_config(), None)
        .expect_err("strict DC must fail on the violated dc-scoped require")
        .to_string()
}

/// DVC-19 AC3: a scoped require whose analysis never ran reports "not
/// exercised" — `requires_ok` is `false` (never a silent vacuous pass),
/// and `first_not_exercised` finds it. The fixture's `@ ac` rule never
/// runs during a DC op().
#[test]
fn scoped_require_whose_analysis_never_ran_is_not_exercised() {
    let mut session = compile(AC_ONLY);
    let op = session.op(&collect_config(), None).expect("collect completes");
    let margins = op.margins();

    // The ac-scoped require is in the catalog (the design declares it)...
    let ac_headroom = margins
        .get("ac_headroom")
        .expect("the scoped require is in the channel — `not exercised`, not absent");
    assert!(
        ac_headroom.is_not_exercised(),
        "the ac-scoped require was not evaluated during DC — worst = +inf, no argmin"
    );
    // ...and it is never a silent vacuous pass: `requires_ok` reports
    // false, and `first_not_exercised` surfaces the constraint.
    assert!(
        !margins.requires_ok(),
        "a not-exercised require makes requires_ok false — never a silent vacuous pass"
    );
    let first = margins
        .first_not_exercised()
        .expect("first_not_exercised surfaces the unrun scoped require");
    assert_eq!(first.label, "ac_headroom");
}

/// DVC-19 AC1 + AC2: a top-level (unscoped) `require` holds in every
/// analysis subject to D2's `Context` default; `@ dc`/`@ tran`/`@ ac`
/// scope only their own bodies. Run the same DC analysis under collect
/// posture and verify the dc-scoped require is exercised (not
/// "not exercised") and satisfied or not per the math.
#[test]
fn scoped_require_evaluates_only_in_its_analysis_under_collect() {
    let mut session = compile(DIVIDER);
    let op = session.op(&collect_config(), None).expect("collect completes");
    let margins = op.margins();

    let vds_dc = margins.get("vds_dc").expect("dc-scoped require is exercised during DC");
    assert!(!vds_dc.is_not_exercised(), "dc-scoped require was evaluated at DC");
    // Margin = hi − V(p,n) = 0.4 − 0.5 = −0.1, violated.
    assert!(
        vds_dc.scalar() < 0.0,
        "dc-scoped margin = 0.4 - 0.5 = -0.1, violated: got {}",
        vds_dc.scalar()
    );

    // The tran-scoped require is *not* exercised during a DC analysis.
    let vds_tran = margins.get("vds_tran").expect("tran-scoped require still in the channel");
    assert!(
        vds_tran.is_not_exercised(),
        "tran-scoped require was not evaluated during DC — not exercised"
    );
}
