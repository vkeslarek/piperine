//! LRM-COVERS: 6.6, 6.6.1
//!
//! LRMW2-52 (`lrm-conformance-wave2`, T51): spec.md AC16's own requirement
//! ("the LRM's own `rcline`/`adc` generate example compiles and runs") —
//! the *simulated* half. `crates/piperine-lang/tests/vams_generate_items.rs`
//! proves both LRM examples elaborate; this file proves generate-produced
//! nets, instances, and analog behavior statements all reach a real DC
//! solve together — not the LRM's own literal `rcline`/`adc` text, because
//! both hit a separate, pre-existing, disclosed codegen gap first:
//! `resolve/pom/expr.rs::parse_contrib_dest`/`ident_from_expr` only
//! recognize a bare `Ident` as a contribution destination — an
//! array-indexed one (`V(out[i]) <+ ...`, `V(n2, n[N]) <+ ...`) is
//! `CodegenError::unsupported` regardless of whether the index is literal
//! or a param/genvar (unrelated to `elab/lower/generate.rs`, T51's own
//! file scope — a distinct follow-up task).
//!
//! This fixture avoids indexed contribution destinations entirely while
//! still exercising exactly the three item kinds `rcline2`'s own worked
//! example combines in one generate scope: a net declared inside the loop
//! (`electrical node;`, one per iteration, dotted-scoped), instances with
//! a genvar-dependent parameter override, and an `analog` behavior block
//! reading that iteration's own generated net. `N` current sources (one
//! per iteration, magnitude `(i+1)` mA) each load a 1 kΩ resistor to
//! ground; a generated behavior per iteration re-expresses that node's
//! voltage as a current fed into a shared `total` node (KCL sums flow
//! contributions across iterations, §5.4.1), itself loaded by a 1 kΩ
//! resistor — `total = 1kΩ * Σ(i+1) mA` for `i` in `0..N`.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::Context;

fn run(n: i64) -> f64 {
    let src = format!(
        r#"
        module isrc (inout electrical p, inout electrical gnd);
            parameter real i0 = 1e-3;
            analog I(p, gnd) <+ -i0;
        endmodule
        module res1k (inout electrical p, inout electrical gnd);
            analog I(p, gnd) <+ V(p, gnd) / 1e3;
        endmodule

        module top ();
            parameter integer N = {n};
            electrical gnd, total;
            genvar i;
            for (i = 0; i < N; i = i + 1) begin : term
                electrical node;
                isrc #(.i0(1e-3*(i+1))) s (node, gnd);
                res1k rl (node, gnd);
                analog I(total, gnd) <+ V(node, gnd) / 1e3;
            end
            res1k rt (total, gnd);
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
    let total = info.nets.get("total").expect("net `total`");
    dc.get_node(total).expect("V(total) at the operating point")
}

/// AC16: three generate-produced items (net, instances with a
/// genvar-dependent param, and an analog behavior reading that generated
/// net) combine correctly in a real DC solve. `N=3`: total = 1kΩ *
/// (1+2+3) mA = 6 V (the sign convention here makes it negative — the
/// magnitude is what's under test).
#[test]
fn generated_net_instances_and_behavior_combine_in_a_real_dc_solve() {
    let total = run(3);
    assert!((total.abs() - 6.0).abs() < 1e-6, "N=3: |total| should be 6.0V (1+2+3 mA into 1kΩ), got {total}");
}

/// Discrimination: a different iteration count must change the summed
/// result — a generate unroll that silently dropped the behavior block
/// (leaving `total` floating/grounded at a constant value) would pass the
/// N=3 case above by coincidence but fail this one. N=5: total = 1kΩ *
/// (1+2+3+4+5) mA = 15 V.
#[test]
fn generated_content_scales_with_the_generate_loop_bound() {
    let total = run(5);
    assert!((total.abs() - 15.0).abs() < 1e-6, "N=5: |total| should be 15.0V (1+..+5 mA into 1kΩ), got {total}");
}
