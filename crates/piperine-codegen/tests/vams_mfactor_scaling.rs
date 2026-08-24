//! LRM-COVERS: 6.3.6, 9.18
//! T64/LRMW2-61: §6.3.6's automatic `$mfactor` scaling rule — "All
//! contributions to a branch flow quantity in the analog block shall be
//! multiplied by `$mfactor`" — applied by the simulator, never by the
//! model itself, plus §9.18/Table 9-29's hierarchical composition
//! (`$mfactor_specified * $mfactor_hier`, product along the instantiation
//! path). `dut`'s own contribution (`I(p,n) <+ V(p,n)/r`) never mentions
//! `$mfactor` — every scaling effect below comes solely from the automatic
//! rule.
//!
//! Measurement: a negligible sense resistor (`1e-9`, `rsense`) between the
//! ideal voltage source and `dut`, read back as a node-voltage difference
//! (`DcAnalysisResult::get_node`, the pattern `tests/dc_host_proof.rs`
//! already establishes) — so the observed current is (to ~1e-8 relative
//! precision) `dut`'s own branch current, no `$mfactor`-scaled flow-probe
//! read needed (§6.3.6's probe-division half of the rule is out of this
//! task's Done-when).

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::Context;

const CIRCUIT: &str = "
    module vsrc (inout electrical p, inout electrical n);
        parameter real v = 1.0;
        analog begin
            V(p, n) <+ v;
        end
    endmodule

    module rsense (inout electrical p, inout electrical n);
        parameter real r = 1.0;
        analog I(p, n) <+ V(p, n) / r;
    endmodule

    module dut (inout electrical p, inout electrical n);
        parameter real r = 1.0;
        parameter real mfactor = 1.0;
        analog I(p, n) <+ V(p, n) / r;
    endmodule
";

const R_SENSE: f64 = 1.0e-9;

/// Builds `src`'s `top` module, DC-solves it, and returns the current
/// flowing from `vin` to `mid` through the `1e-9` Ω sense resistor —
/// `dut`'s own branch current, to ~1e-8 relative precision.
fn measure_current(src: &str) -> f64 {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("top").expect("compiles");
    let result = circuit.dc(Context::default()).expect("dc starts").solve().expect("dc converges");
    let vin = result.get_node(info.nets.get("vin").expect("net `vin` mapped")).expect("v(vin)");
    let mid = result.get_node(info.nets.get("mid").expect("net `mid` mapped")).expect("v(mid)");
    (vin - mid) / R_SENSE
}

fn quadrupling_circuit(mfactor: f64) -> String {
    format!("{CIRCUIT}
        module top ();
            electrical vin, mid, gnd;
            vsrc #(.v(1.0)) src (vin, gnd);
            rsense #(.r({R_SENSE})) rs (vin, mid);
            dut #(.r(1.0), .mfactor({mfactor})) d1 (mid, gnd);
        endmodule
    ")
}

/// Done-when: "`$mfactor = 4` on an instance quadruples its branch current
/// without the model multiplying it by hand." With the sense resistor
/// negligible relative to `dut`'s own `r`, the measured current is (to
/// ~1e-8 relative precision) `dut`'s own branch current `v/(r/mfactor) =
/// v*mfactor/r` — a closed-form linear prediction, not "roughly
/// quadruple".
#[test]
fn mfactor_scales_branch_flow_without_the_model_reading_it() {
    let i_unity = measure_current(&quadrupling_circuit(1.0));
    let i_quad = measure_current(&quadrupling_circuit(4.0));

    let expected_unity = 1.0 / (R_SENSE + 1.0 / 1.0);
    let expected_quad = 1.0 / (R_SENSE + 1.0 / 4.0);

    assert!(
        (i_unity - expected_unity).abs() < 1e-6,
        "unity-mfactor baseline should be v/(r_sense + r/mfactor) = {expected_unity}, got {i_unity}"
    );
    assert!(
        (i_quad - expected_quad).abs() < 1e-6,
        "mfactor=4 should scale the branch current to v/(r_sense + r/mfactor) = {expected_quad}, got {i_quad}"
    );
    assert!(
        (i_quad / i_unity - 4.0).abs() < 1e-6,
        "mfactor=4 should quadruple dut's own branch current relative to mfactor=1, got ratio {}",
        i_quad / i_unity
    );
}

/// Done-when: "A nested instance's effective mfactor is the product along
/// the path." LRM §9.18's own worked example (Example 1, `test_module` →
/// `module_a` → `module_b` → `module_c`, `mfactor = 3 * 2 * 7 = 42`) —
/// reproduced with the printed text's own gap filled in: the LRM's Example
/// 1 source shows `module_a A1(p,n);` with no `#(.mfactor(3))` on `A1`
/// even though its own comment ("mfactor = 3 * 2") and the final comment
/// ("mfactor = 3 * 2 * 7 = 42") both require a `3` at that level — an
/// erratum in the printed LRM text, not a real ambiguity (Table 9-29's
/// rule is unambiguous once every level's `specified` term is known).
/// `mfactor` here is Piperine's disclosed vehicle for the system
/// parameter (a module-declared `parameter real mfactor`, not literal
/// `$mfactor(...)` override syntax — see `crates/piperine-codegen/src/
/// device/analog/instance.rs`), so only `module_c` (which uses it, playing
/// the LRM's `module_c`) declares it.
#[test]
fn nested_instance_mfactor_is_the_product_along_the_path() {
    let src = format!("
        module vsrc (inout electrical p, inout electrical n);
            parameter real v = 1.0;
            analog begin
                V(p, n) <+ v;
            end
        endmodule

        module rsense (inout electrical p, inout electrical n);
            parameter real r = 1.0;
            analog I(p, n) <+ V(p, n) / r;
        endmodule

        module module_c (inout electrical p, inout electrical n);
            parameter real r = 1.0;
            parameter real mfactor = 1.0;
            analog I(p, n) <+ V(p, n) / r;
        endmodule

        module module_b (inout electrical p, inout electrical n);
            module_c #(.mfactor(7.0)) C1 (p, n);
        endmodule

        module module_a (inout electrical p, inout electrical n);
            module_b #(.mfactor(2.0)) B1 (p, n);
        endmodule

        module top ();
            electrical vin, mid, gnd;
            vsrc #(.v(1.0)) src (vin, gnd);
            rsense #(.r({R_SENSE})) rs (vin, mid);
            module_a #(.mfactor(3.0)) A1 (mid, gnd);
        endmodule
    ");
    let i_meas = measure_current(&src);

    // v = 1.0, r = 1.0, effective mfactor = 3 * 2 * 7 = 42 (LRM §9.18
    // Example 1's own worked value).
    let expected = 1.0 / (R_SENSE + 1.0 / 42.0);
    assert!(
        (i_meas - expected).abs() < 1e-3,
        "nested mfactor should compose as 3 * 2 * 7 = 42, current v/(r_sense + r/mfactor) = {expected}, got {i_meas}"
    );
}
