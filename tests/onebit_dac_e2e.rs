//! LRM-COVERS: 3.7, 7.3.1
//!
//! LRMW2-45 (`lrm-conformance-wave2`, T52): spec.md's "simulates, not
//! merely elaborates" standing rule, applied to §7.3.1 Table 7-1's own
//! `onebit_dac` worked example — a `wire`/`reg` net read from a
//! *continuous* (analog) context, via a bare identifier comparison
//! (`if (in == 0) ...`), not `V(...)`/`I(...)`. Before T52, `wire`/`reg`
//! silently became `NetType::Discipline("wire")`/`"reg"` — an undeclared
//! discipline, which `resolve/pom/structure.rs::domain_of` defaults to
//! `Domain::Analog` rather than erroring, so `in` would have been treated
//! as an ordinary analog branch voltage instead of routing through the
//! digital-domain shadow-variable bridge (§7.3.1) at all.
//!
//! Adapted from the LRM's own text only by merging the two modules into
//! one (a `reg`-declared port crossing to a `wire`-declared port on
//! another module hits a separate, pre-existing, unrelated gap — Annex F
//! discipline resolution treats `reg`/`wire`'s own `NetType::discipline_
//! name()` fallback labels as two distinct incompatible disciplines
//! rather than recognizing all four §3.7 keywords as mutually digital-
//! compatible; a deeper fix than this task's own file scope
//! (`grammar/item.rs`/`pom/net_type.rs`/"digital IR validation")) and by
//! giving `in` an initial value at elaboration time rather than driving
//! it with an external testbench signal (this codebase has no delay-
//! controlled-statement grammar support yet, `#1 in = ...;`, a separate
//! pre-existing gap) — neither adaptation touches the actual §7.3.1
//! mechanism under test.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::Context;

fn run(in_val: i64) -> f64 {
    let src = format!(
        r#"
        module res1k (inout electrical p, inout electrical n);
            analog I(p, n) <+ V(p, n) / 1e3;
        endmodule
        module top ();
            reg in;
            electrical out, gnd;
            real x;
            initial in = {in_val};
            analog begin
                if (in == 0)
                    x = 0.0;
                else
                    x = 3.0;
                V(out, gnd) <+ x;
            end
            res1k r1 (out, gnd);
        endmodule
        "#
    );
    let mut design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .unwrap_or_else(|e| panic!("fixture elaborates: {e:?}"));
    design.set_top("top");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) =
        compiler.build_circuit_mapped("top").unwrap_or_else(|e| panic!("top builds: {e:?}"));
    let dc = circuit.dc(Context::default()).unwrap().solve().expect("dc solve");
    let out = info.nets.get("out").expect("net `out`");
    dc.get_node(out).expect("V(out) at the operating point")
}

/// Table 7-1's own `onebit_dac`: `in == 0` reads `reg in` from the analog
/// body — `in = 0` must drive `x = 0.0` and therefore `out = 0.0 V`.
#[test]
fn discrete_reg_read_zero_drives_the_analog_output_to_zero() {
    let out = run(0);
    assert!((out - 0.0).abs() < 1e-9, "in=0 should read as 0 in the continuous context, giving out=0.0V, got {out}");
}

/// Discrimination: `in = 1` (any nonzero) must take the `else` branch,
/// giving `out = 3.0 V` — a shadow bridge that always reads `false`/`0`
/// (or always `true`) would pass only one of these two cases.
#[test]
fn discrete_reg_read_nonzero_drives_the_analog_output_to_three_volts() {
    let out = run(1);
    assert!((out - 3.0).abs() < 1e-9, "in=1 should read as nonzero in the continuous context, giving out=3.0V, got {out}");
}
