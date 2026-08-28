//! LRM-COVERS: 5.5.2
//! LRMW3-02 (`lrm-conformance-wave3`, T8): the LRM §5.5.2 N-bit-DAC shape
//! — a genvar-indexed vector access *inside* an `analog for` loop —
//! elaborates, compiles, and solves. Adapted from §5.9.3's own
//! for-loop-summing worked example the same way `analog_loop_unroll.rs`
//! (wave 2, T32) already adapted it: a literal loop bound instead of a
//! `parameter` bound (a `parameter` differs per instance and this
//! compiled-once-per-module-type kernel has no per-instance environment
//! at flatten time — SPEC_DEVIATION.md §2.7's own disclosed limit,
//! unchanged by this task). What *is* new here, closing the one gap
//! `analog_loop_unroll.rs` explicitly left open ("array analog nets are a
//! separate, pre-existing gap ... not this task's Where"): the loop body
//! itself indexes a vector net by the genvar, on both the contribution
//! destination and the read side.
//!
//! The elaboration-time seam this closes:
//! `elab/lower/vams.rs::resolve_or_create_branch` builds each module's
//! implicit branch list once, over raw (pre-unroll) source, so a
//! genvar-indexed contribution destination (`I(bus[k], g) <+ …`) used to
//! fail there ("undefined name: k") before codegen's own per-iteration
//! unroll (`flatten::analog::unroll_loop`) ever ran — `net_ref_for_branch`
//! (new, T8) tolerates it with a documented sentinel branch identity
//! instead of failing the whole module.

use piperine_codegen::kernel::analog::AnalogKernel;
use piperine_codegen::SimCtx;
use piperine_lang::parse_and_elaborate;

fn kernel(analog_src: &str) -> Result<AnalogKernel, String> {
    let src = format!(
        "module Dut(inout electrical g);
    electrical bus[0:3];
    inout electrical iout;
    integer k;
    {analog_src}
endmodule
"
    );
    let design = parse_and_elaborate(&src, &piperine_lang::SourceMap::dummy())
        .map_err(|e| format!("elaborate failed: {e:?}"))?;
    let bodies = piperine_codegen::resolve::lower_bodies(&design).map_err(|e| e.to_string())?;
    AnalogKernel::compile(&bodies["Dut"]).map_err(|e| e.to_string())
}

fn terminal_index(k: &AnalogKernel, name: &str) -> usize {
    (0..k.num_terminals())
        .find(|&i| k.terminal_name(i) == name)
        .unwrap_or_else(|| panic!("terminal `{name}` not found among: {:?}", (0..k.num_terminals()).map(|i| k.terminal_name(i)).collect::<Vec<_>>()))
}

fn jacobian(k: &AnalogKernel, volts: &[f64]) -> Vec<f64> {
    let n = k.num_terminals();
    let vars = vec![0.0; k.num_vars()];
    let state = vec![0.0; k.num_state_slots()];
    let mut out = vec![0.0; n * n];
    k.eval_jacobian(volts, &[], &state, &vars, &SimCtx::default(), &mut out);
    out
}

fn residual(k: &AnalogKernel, volts: &[f64]) -> Vec<f64> {
    let n = k.num_terminals();
    let vars = vec![0.0; k.num_vars()];
    let state = vec![0.0; k.num_state_slots()];
    let mut out = vec![0.0; n];
    k.eval_residual(volts, &[], &state, &vars, &SimCtx::default(), &mut out);
    out
}

/// Done-when #1: the §5.5.2-shaped N-bit-DAC weighted sum — `for (k = 0;
/// k < 4; k = k + 1) I(iout, g) <+ V(bus[k], g) * pow(2.0, k);` —
/// elaborates, compiles, and (Done-when #2, in the same test) resolves
/// each unrolled iteration to its *own* bit: `dI(iout)/dV(bus[k])` must
/// equal `2^k` for every `k`, read individually from the Jacobian, not
/// inferred from a single summed residual.
#[test]
fn genvar_indexed_read_produces_the_weighted_dac_sum() {
    let k = kernel(
        "analog begin
        for (k = 0; k < 4; k = k + 1) begin
            I(iout, g) <+ V(bus[k], g) * pow(2.0, k);
        end
    end",
    )
    .expect("genvar-indexed DAC read must compile");
    let n = k.num_terminals();
    let volts = vec![0.0; n];
    let jac = jacobian(&k, &volts);
    let i_iout = terminal_index(&k, "iout");

    // Each bit's own weight, read independently — a wrong/collapsed
    // resolution (e.g. every iteration wiring to bus[0]) would make these
    // four values collide instead of forming 1, 2, 4, 8.
    for (bit, expected_weight) in [(0, 1.0), (1, 2.0), (2, 4.0), (3, 8.0)] {
        let i_bit = terminal_index(&k, &format!("bus[{bit}]"));
        let got = jac[i_iout * n + i_bit];
        assert!(
            (got - expected_weight).abs() < 1e-9,
            "dI(iout)/dV(bus[{bit}]) must be 2^{bit} = {expected_weight}, got {got}"
        );
    }

    // A real transient-style DC point: bus = [1,0,1,1] (V) -> weighted
    // sum = 1*1 + 0*2 + 1*4 + 1*8 = 13 A into iout.
    let mut volts2 = vec![0.0; n];
    volts2[terminal_index(&k, "bus[0]")] = 1.0;
    volts2[terminal_index(&k, "bus[1]")] = 0.0;
    volts2[terminal_index(&k, "bus[2]")] = 1.0;
    volts2[terminal_index(&k, "bus[3]")] = 1.0;
    let out = residual(&k, &volts2);
    assert!(
        (out[i_iout] - 13.0).abs() < 1e-9,
        "the analytic weighted sum 1*1+0*2+1*4+1*8 = 13 must reach iout, got {}",
        out[i_iout]
    );
}

/// Done-when #2 (destination side): a genvar-indexed *contribution
/// destination* — `for (k = 0; k < 4; k = k + 1) I(bus[k], g) <+
/// V(bus[k], g) / 1e3;` — resolves each iteration to its own bit's node,
/// not a collapsed/shared one. This is the harder half of the seam
/// (`net_ref_for_branch`'s elaboration-time tolerance): the contribution
/// *destination* itself is genvar-indexed, which is exactly what used to
/// fail at elaboration before this task.
#[test]
fn genvar_indexed_contribution_destination_resolves_per_bit() {
    let k = kernel(
        "analog begin
        for (k = 0; k < 4; k = k + 1) begin
            I(bus[k], g) <+ V(bus[k], g) / 1e3;
        end
    end",
    )
    .expect("genvar-indexed contribution destination must compile");
    let n = k.num_terminals();
    let volts = vec![0.0; n];
    let jac = jacobian(&k, &volts);

    // Every bit gets its own 1/1e3 self-conductance and zero cross-talk
    // to any other bit — a collapsed resolution (every iteration wiring
    // the same node) would make the off-diagonal entries nonzero.
    for bit in 0..4 {
        let i_bit = terminal_index(&k, &format!("bus[{bit}]"));
        let self_g = jac[i_bit * n + i_bit];
        assert!((self_g - 1e-3).abs() < 1e-9, "bus[{bit}]'s own conductance must be 1/1e3, got {self_g}");
        for other in 0..4 {
            if other == bit {
                continue;
            }
            let i_other = terminal_index(&k, &format!("bus[{other}]"));
            let cross = jac[i_bit * n + i_other];
            assert!(cross.abs() < 1e-12, "bus[{bit}] must not depend on bus[{other}], got {cross}");
        }
    }
}
