//! dv-core T29 (DVC-20) — event windows end-to-end: a settling
//! `require` scoped by `@ tran(after = …, dur = …) { … }` evaluates its
//! margin only inside the window. A window outside the analysis's time
//! range is "not exercised" (never a silent vacuous pass). The fixture
//! pairs a divider whose `out` sits at 0.5 V for the whole tran with a
//! `require vds_tran : V(p, n) <= 0.4` that is violated everywhere; the
//! window selects which evaluation points feed the worst-margin tracker.
//!
//! Math: margin = hi − V(p, n) = 0.4 − 0.5 = −0.1 (violated) at every
//! evaluated point. With `@ tran(after = 5e-3, dur = 2e-3)` and a tran
//! step of 1e-3 over `[0, 1e-2]`, the in-window points are t = 5e-3,
//! 6e-3, 7e-3 — the argmin is the first (5e-3), the worst is −0.1.

use piperine_api::prelude::CheckPosture;
use piperine_api::{Session, SolverConfig};
use piperine_lang::SourceMap;

/// A divider whose `out` sits at 0.5 V for the whole tran (DC source on a
/// resistor divider — no dynamics). The `@ tran(after = 5e-3, dur = 2e-3)`
/// window scopes `vds_tran` to `[5e-3, 7e-3]`.
const WINDOW_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Vsrc(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
analog Vsrc { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod Dut(inout p: Electrical, inout n: Electrical) { param hi: Real = 0.4; }
analog Dut { I(p, n) <+ V(p, n) / 1e3; }
constraint Dut {
    @ tran(after = 5e-3, dur = 2e-3) { require vds_tran : V(p, n) <= hi; }
}

mod Top() {
    wire gnd : Electrical;
    wire in  : Electrical;
    wire out : Electrical;
    v1   : Vsrc(.p = in,  .n = gnd);
    r1   : R(.p = in,     .n = out);
    dut1 : Dut(.p = out,  .n = gnd);
}
";

/// Same circuit, but the window sits outside the tran's time range
/// (`after = 100.0`). The `require` is never exercised — the host sees
/// "not exercised" rather than a silent vacuous pass.
const OUT_OF_RANGE_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod Vsrc(inout p: Electrical, inout n: Electrical) { param dc: Real = 1.0; }
analog Vsrc { V(p, n) <- dc; }

mod R(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog R { I(p, n) <+ V(p, n) / r; }

mod Dut(inout p: Electrical, inout n: Electrical) { param hi: Real = 0.4; }
analog Dut { I(p, n) <+ V(p, n) / 1e3; }
constraint Dut {
    @ tran(after = 100.0, dur = 1.0) { require vds_tran : V(p, n) <= hi; }
}

mod Top() {
    wire gnd : Electrical;
    wire in  : Electrical;
    wire out : Electrical;
    v1   : Vsrc(.p = in,  .n = gnd);
    r1   : R(.p = in,     .n = out);
    dut1 : Dut(.p = out,  .n = gnd);
}
";

fn compile(phdl: &str) -> Session {
    let design = piperine_lang::parse_and_elaborate(phdl, &SourceMap::dummy()).expect("elaborates");
    Session::compile(&design, "Top").expect("session compiles")
}

fn collect_config() -> SolverConfig {
    SolverConfig { checks: CheckPosture::Collect, ..Default::default() }
}

/// DVC-20 AC3 + acceptance: a settling fixture evaluates its margin only
/// inside the window. Under collect posture the worst margin's argmin
/// lands on the first in-window point (5e-3); the worst is the violated
/// `0.4 − 0.5 = −0.1`. Outside the window, no evaluation feeds the
/// tracker — never a silent pass.
#[test]
fn windowed_require_evaluates_only_inside_the_window() {
    let mut session = compile(WINDOW_PHDL);
    let tran = session
        .tran(1e-2, Some(1e-3), 0.0, &collect_config(), None, false, &[])
        .expect("collect tran completes");

    let margins = tran.margins();
    let vds_tran = margins
        .get("vds_tran")
        .expect("the windowed require is in the channel");
    assert!(
        !vds_tran.is_not_exercised(),
        "the windowed require was evaluated — the tran passes through [5e-3, 7e-3]"
    );
    // Worst margin = 0.4 − 0.5 = −0.1 everywhere in the window.
    let worst = vds_tran.scalar();
    assert!(
        (worst - (-0.1)).abs() < 1e-9,
        "worst margin = hi - V = 0.4 - 0.5 = -0.1: got {worst}"
    );
    // The argmin is the first in-window point: t = 5e-3. Every in-window
    // point has the same margin (the divider has no dynamics), so the
    // tracker holds the first one it sees.
    let argmin = vds_tran
        .argmin()
        .expect("the windowed require has an argmin (a point in the window was evaluated)");
    assert!(
        (argmin - 5e-3).abs() < 1e-12,
        "argmin = first in-window point (5e-3): got {argmin}"
    );
    // A violated windowed require makes `requires_ok` false — never a
    // silent pass.
    assert!(
        !margins.requires_ok(),
        "the violated windowed require makes requires_ok false"
    );
}

/// DVC-20 AC3: a window that selects no points is "not exercised". The
/// fixture's `after = 100.0` opens a window far past the tran's stop
/// time (1e-2); the worst stays at `+inf`, no argmin, and the host
/// reports `not_exercised`. `requires_ok` is false (never a silent
/// vacuous pass).
#[test]
fn window_outside_the_tran_range_is_not_exercised() {
    let mut session = compile(OUT_OF_RANGE_PHDL);
    let tran = session
        .tran(1e-2, Some(1e-3), 0.0, &collect_config(), None, false, &[])
        .expect("collect tran completes");

    let margins = tran.margins();
    let vds_tran = margins
        .get("vds_tran")
        .expect("the windowed require is still in the channel — not exercised, not absent");
    assert!(
        vds_tran.is_not_exercised(),
        "the window selects no tran point — worst = +inf, no argmin"
    );
    assert!(
        !margins.requires_ok(),
        "a not-exercised require makes requires_ok false — never a silent vacuous pass"
    );
    let first = margins
        .first_not_exercised()
        .expect("first_not_exercised surfaces the out-of-window require");
    assert_eq!(first.label, "vds_tran");
}
