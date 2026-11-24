//! The environment read by compiled reactions and drives, run end to end on the solver (feature
//! `ir-rust`, F-B6; RS-11, check C109 for the compile half): one hand-built body whose reaction
//! commits `temperature()`, `gmin()` and `source_scale()` and whose three drives hold the same
//! three reads, placed under a top behind the compiled oscillator and run on a session at a
//! host temperature and `gmin` that are not the defaults. Expected values are the session's own
//! numbers written here (docs/spec part_iv §12.1.12; abi.md §4.2: `GMIN` holds the session's
//! `gmin` and `SOURCE_SCALE` holds 1.0 at every instant), never read back from the model.
#![cfg(unix)]

use piperine_ir::{BodyId, CarrierRole, Drive, Env, Event, EventSource, Expr, IrBuilder, ValueType};
use piperine_solver::prelude::Context;

#[allow(dead_code)]
#[path = "../../piperine-ir/tests/support/digital.rs"]
mod digital;
#[allow(dead_code)]
#[path = "support/mixed.rs"]
mod mixed;

use digital::Buck;
use mixed::{Built, Wire};

const A: &str = "env.msl";
/// The host's temperature and the session's `gmin`, neither a default.
const KELVIN: f64 = 350.0;
const GMIN: f64 = 3.0e-12;

fn f64_at(bytes: &[u8]) -> f64 {
    f64::from_le_bytes(bytes[..8].try_into().expect("eight bytes"))
}

/// ```text
/// module Ambient(in clk: Bit, out t: Bit<f64>, out g: Bit<f64>, out s: Bit<f64>,
///                out dt: Bit<f64>, out dg: Bit<f64>, out ds: Bit<f64>) {}
/// digital for Ambient {
///     dt = temperature(); dg = gmin(); ds = source_scale();                  // line 3
///     on clk.changed() { t <- temperature(); g <- gmin(); s <- source_scale(); }  // line 4
/// }
/// ```
fn ambient(b: &mut IrBuilder) -> BodyId {
    let body = b.body(Buck::spec("Ambient", A, 1));
    for (i, name) in ["clk", "t", "g", "s", "dt", "dg", "ds"].into_iter().enumerate() {
        let (ty, role) = if i == 0 { (ValueType::Bool, CarrierRole::In) } else { (ValueType::F64, CarrierRole::Out) };
        b.carrier(body, Buck::carrier(name, ty, role, None, A, 1, 20 + 10 * i as u32));
    }
    for (carrier, env, col) in [(4, Env::Temperature, 5), (5, Env::Gmin, 25), (6, Env::SourceScale, 40)] {
        let value = b.expr(Expr::Env(env), ValueType::F64, Buck::origin(A, 3, col));
        b.drive(body, Drive { carrier, value, after: None, origin: Buck::origin(A, 3, col), element: None, path: vec![] });
    }
    let changed = b.event(body, Event { source: EventSource::Changed(0), guard: None, origin: Buck::origin(A, 4, 8) });
    let mut commits = Vec::new();
    for (carrier, env, col) in [(1, Env::Temperature, 29), (2, Env::Gmin, 50), (3, Env::SourceScale, 64)] {
        let value = b.expr(Expr::Env(env), ValueType::F64, Buck::origin(A, 4, col));
        commits.push(b.stmt(piperine_ir::Stmt::Commit { carrier, value, after: None, target: None }));
    }
    Buck::reaction(b, body, vec![changed], commits, vec![], A, 4);
    body
}

#[test]
fn a_compiled_reaction_and_drive_read_the_hosts_temperature_gmin_and_source_scale() {
    let mut w = Wire::new();
    let osc = Buck::osc(&mut w.b);
    w.place("osc", osc, vec![piperine_ir::Constant::Real(piperine_ir::Real::new(200e6))], &[("clk", ValueType::Bool)]);
    let body = ambient(&mut w.b);
    let nets: Vec<(&str, ValueType)> = ["clk", "t", "g", "s", "dt", "dg", "ds"]
        .into_iter()
        .map(|n| (n, if n == "clk" { ValueType::Bool } else { ValueType::F64 }))
        .collect();
    w.place("amb", body, vec![], &nets);
    let built = Built::new(w.finish());
    let mut context = Context::default();
    context.tolerances.temperature = KELVIN;
    context.tolerances.gmin = GMIN;
    // 6 ns: the clock changes at 2.5 ns
    let (seen, _) = built.run_in(context, 6e-9, 1e-9);
    let last = |name: &str| f64_at(&seen.series(name).last().unwrap_or_else(|| panic!("net {name} has a value")).1);
    // the reaction commits what the solver holds: the host's temperature, the session's gmin and
    // a source scale of 1.0
    // the reaction commits what the solver holds, in the operating point (where `changed()`
    // fires on the initial clock) and again at the clock change at 2.5 ns: three commits, the
    // same three values, so each net holds one value for the whole run
    let made = seen.commits("Top.amb");
    assert_eq!(made.iter().filter(|t| **t == 2_500_000).count(), 3, "three commits at 2.5 ns: {made:?}");
    for net in ["t", "g", "s"] {
        assert_eq!(seen.series(net).len(), 1, "{net} keeps the one value");
    }
    // the host's temperature, the session's gmin and a source scale of 1.0
    assert_eq!([last("t"), last("g"), last("s")], [KELVIN, GMIN, 1.0]);
    // the drives hold the same three reads from the operating point on
    assert_eq!([last("dt"), last("dg"), last("ds")], [KELVIN, GMIN, 1.0]);
}
