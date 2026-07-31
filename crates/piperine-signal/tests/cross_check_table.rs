//! Cross-check fixture (DVT-06, `design.md` §C1): `table(x, xs, ys)`, run
//! both through codegen's own JIT device (lowered to a nested-`If` POM
//! expression) and through `piperine-signal::behavior::table::Table`
//! directly, values asserted equal at every accepted point — including
//! points that fall inside the end-clamp regions.
//!
//! **This fixture is deleted by `codegen-simplification`** — once codegen
//! delegates to `piperine-signal` instead of carrying its own copy of
//! `resolve/pom/analog_ops.rs::Table`, there is only one implementation
//! left and nothing to cross-check.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_signal::behavior::table::Table;

const DISCIPLINE: &str = "discipline Electrical { potential v : Real; flow i : Real; }";

const XS: [f64; 4] = [0.0, 1e-3, 2e-3, 3e-3];
// Positive-only: codegen's `const_array` only accepts a bare `Literal::Real`
// per element, not a unary-negated one (`-2.0` parses as `Neg(2.0)`).
const YS: [f64; 4] = [0.0, 5.0, 1.0, 5.0];

/// `table($abstime, xs, ys)` as an ideal-forced source: `V(out)` equals the
/// table lookup exactly at every accepted point. The stop time (4e-3)
/// extends past `xs`'s last breakpoint so the sweep also exercises the
/// high-end clamp.
fn run(stop: f64, dt: f64) -> Vec<(f64, f64)> {
    let src = format!(
        "{DISCIPLINE}
mod TableLookup(inout p: Electrical, inout n: Electrical) {{}}
analog TableLookup {{
    V(p, n) <- table($abstime, [{}], [{}]);
}}
mod Top() {{
    wire gnd: Electrical;
    wire out: Electrical;
    t1: TableLookup(.p = out, .n = gnd);
}}
",
        XS.map(|x| x.to_string()).join(", "),
        YS.map(|y| y.to_string()).join(", "),
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy()).expect("elaborate");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowering");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("Top").expect("circuit builds");
    let out = info.nets.get("out").expect("net `out`").clone();

    let opts = piperine_solver::prelude::TransientAnalysisOptions::new(stop, dt);
    let ctx = piperine_solver::prelude::Context::default();
    let result = circuit.transient(opts, ctx).unwrap().solve().expect("transient completes");

    (0..result.len())
        .filter_map(|i| {
            let step = result.get(i)?;
            let v = step.get_node(&out)?;
            Some((step.time(), v))
        })
        .collect()
}

#[test]
fn table_matches_codegens_jit_device_on_the_same_accepted_points() {
    let jit = run(4e-3, 2e-4);
    assert!(jit.len() > 10, "the fixture must produce a non-trivial accepted-point sequence");

    let table = Table::new(XS.to_vec(), YS.to_vec()).unwrap();
    let mut saw_high_clamp = false;
    for &(t, jit_v) in &jit {
        // table is a pure algebraic expression (no runtime state), so
        // unlike the operators/events families there is no accept-lag: the
        // kernel evaluates it fresh on the current accepted solution.
        let signal_v = table.value(t);
        assert!(
            (signal_v - jit_v).abs() < 1e-9,
            "t={t}: piperine-signal={signal_v} codegen-jit={jit_v}"
        );
        if t > XS[3] {
            saw_high_clamp = true;
        }
    }
    assert!(saw_high_clamp, "the fixture must run past the last breakpoint to exercise the end clamp");
}
