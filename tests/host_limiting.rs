//! HOST-10 (host-library T13): `op.stats.limiting` exposes the shipped
//! `LimitingReport` diagnostics (ABI-09) — per-device structured limiting
//! feedback from the final Newton step. Empty slice when nothing limited
//! (the common case for linear / well-converged circuits); never `None`.
//!
//! The limiting state is transient — pnjlim/fetlim fire on intermediate
//! Newton steps and release once the junction voltage stabilises. So a
//! converged DC operating point typically has an empty `limiting` list at
//! the final step. These tests prove the surface exists and is accessible,
//! and that the empty case (the default for most circuits) is an empty
//! slice, not `None` or `NaN`.

use std::path::PathBuf;

use piperine::{Session, SolverConfig};
use piperine_lang::SourceMap;

const DIVIDER_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VoltageSource(inout p: Electrical, inout n: Electrical) {
    param voltage: Real = 0.0;
}
analog VoltageSource { V(p, n) <- voltage; }

mod Resistor(inout p: Electrical, inout n: Electrical) {
    param r: Real = 1e3;
}
analog Resistor { I(p, n) <+ V(p, n) / r; }

mod Divider() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire mid  : Electrical;
    src   : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
    r_top : Resistor      (.p = vin, .n = mid) { .r = 3e3 };
    r_bot : Resistor      (.p = mid, .n = gnd) { .r = 2e3 };
}
";

/// A diode with `$limit("pnjlim", …)` — a nonlinear circuit whose junction
/// limiter fires during the convergence walk. At the final converged step
/// the limiter typically releases (the junction voltage has stabilised), so
/// `limiting` may be empty — but the surface must still be accessible.
const DIODE_PHDL: &str = "\
discipline Electrical { potential v: Real; flow i: Real; }

mod VSource(inout p: Electrical, inout n: Electrical) { param dc: Real = 0.0; }
analog VSource { V(p, n) <- dc; }

mod Resistor(inout p: Electrical, inout n: Electrical) { param r: Real = 1e3; }
analog Resistor { I(p, n) <+ V(p, n) / r; }

mod Diode(inout a: Electrical, inout c: Electrical) {
    param is_sat : Real = 1e-14;
    param vte    : Real = 0.02585;
    param vcrit  : Real = 0.7;
}
analog Diode {
    var vd : Real = $limit(\"pnjlim\", V(a, c), 0.0, vte, vcrit);
    I(a, c) <+ is_sat * (limexp(vd / vte) - 1.0);
}

mod Top() {
    wire gnd  : Electrical;
    wire vin  : Electrical;
    wire vd   : Electrical;
    v1 : VSource   (vin, gnd) { .dc = 5.0 };
    r1 : Resistor  (vin, vd)  { .r = 1e3 };
    d1 : Diode     (vd, gnd);
}
";

fn headers_source_map() -> SourceMap {
    let headers = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/piperine-lang/headers"));
    let mut map = SourceMap::new(headers.clone()).with_prelude(headers.join("prelude.phdl"));
    map.add_namespace("piperine", headers.clone());
    map.add_namespace("spice", headers.join("spice"));
    map
}

/// HOST-10 AC: `op.stats().limiting` is accessible and returns an empty
/// slice when no device limited (a purely linear divider never limits).
#[test]
fn stats_limiting_is_empty_for_linear_circuit() {
    let design = piperine_lang::parse_and_elaborate(DIVIDER_PHDL, &headers_source_map())
        .expect("divider elaborates");
    let mut session = Session::compile(&design, "Divider").expect("session compiles");
    let op = session.op(&SolverConfig::default(), None).expect("op solves");
    let limiting = op.stats().limiting.as_slice();
    assert!(
        limiting.is_empty(),
        "a linear divider has no limiters; limiting must be empty, got {limiting:?}"
    );
}

/// HOST-10 surface: `op.stats().limiting` is accessible on a nonlinear diode
/// circuit (which uses `$limit("pnjlim", …)`). The list may be empty at the
/// final converged step (limiter releases once the junction stabilises) —
/// the test proves the surface works, not that limiting fires.
#[test]
fn stats_limiting_accessible_on_nonlinear_circuit() {
    let design = piperine_lang::parse_and_elaborate(DIODE_PHDL, &headers_source_map())
        .expect("diode elaborates");
    let mut session = Session::compile(&design, "Top").expect("session compiles");
    let op = session.op(&SolverConfig::default(), None).expect("op solves");
    // The diode converges; limiting is a `&[LimitingReport]` we can read.
    // It may be empty (limiter released) or non-empty (still active) — both
    // are valid. The assertion is that the field IS readable, not its value.
    let _limiting: &[piperine_solver::abi::LimitingReport] = op.stats().limiting.as_slice();
}
