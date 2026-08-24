//! LRM-COVERS: 6.3.6, 9.18
//! T65/LRMW2-62: §9.18/Table 9-29's five geometrical hierarchical system
//! parameters (`$xposition`/`$yposition`/`$angle`/`$hflip`/`$vflip`) —
//! declared per-instance placement reaching a model's own reads, composed
//! hierarchically across nesting levels the same way `$mfactor` already is
//! (T64). Unlike `$mfactor`, these five have no automatic effect on the
//! simulation (LRM's own words), so the test reads them back straight as
//! module-var opvars (the same proven pattern `vams_mfactor.rs` already
//! uses for `$mfactor`) rather than through an observed circuit effect.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::Context;

const DUT: &str = "
    module dut (inout electrical p, inout electrical n);
        parameter real xposition = 0.0;
        parameter real yposition = 0.0;
        parameter real angle = 0.0;
        parameter real hflip = 1.0;
        parameter real vflip = 1.0;
        real xp, yp, ang, hf, vf;
        analog begin
            xp = $xposition;
            yp = $yposition;
            ang = $angle;
            hf = $hflip;
            vf = $vflip;
            I(p, gnd) <+ V(p, gnd);
        end
    endmodule
";

fn opvars(src: &str, top: &str, device: &str) -> std::collections::HashMap<String, f64> {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, _info) = compiler.build_circuit_mapped(top).expect("compiles");
    circuit.dc(Context::default()).expect("dc starts").solve().expect("dc converges");
    let d = circuit.all_devices().iter().find(|d| d.name() == device)
        .unwrap_or_else(|| panic!("device `{device}`"));
    d.read_opvars().into_iter().collect()
}

/// Done-when: "Declared placement reaches the model's reads." A direct
/// instance override of all five reaches `$xposition`/.../`$vflip` inside
/// the model's own analog body — no composition involved (a single
/// instantiation level).
#[test]
fn declared_placement_reaches_the_models_reads() {
    let src = format!("{DUT}
        module top ();
            electrical p, gnd;
            dut #(.xposition(1.5), .yposition(-2.5), .angle(75.0), .hflip(-1.0), .vflip(-1.0)) d1 (p, gnd);
        endmodule
    ");
    let vars = opvars(&src, "top", "d1");
    assert!((vars["xp"] - 1.5).abs() < 1e-12, "$xposition should read the instance's own override 1.5, got {}", vars["xp"]);
    assert!((vars["yp"] - -2.5).abs() < 1e-12, "$yposition should read the instance's own override -2.5, got {}", vars["yp"]);
    assert!((vars["ang"] - 75.0).abs() < 1e-12, "$angle should read the instance's own override 75.0, got {}", vars["ang"]);
    assert!((vars["hf"] - -1.0).abs() < 1e-12, "$hflip should read the instance's own override -1.0, got {}", vars["hf"]);
    assert!((vars["vf"] - -1.0).abs() < 1e-12, "$vflip should read the instance's own override -1.0, got {}", vars["vf"]);
}

/// Done-when: "Nested placement composes per §9.18/Table 9-29 (rotation/
/// flip combination asserted)." LRM §9.18 Example 2's own worked
/// `$xposition` pattern (`test_module` -> `module_a` -> `module_b` ->
/// `module_c`, `xposition = 1.1u + 1u + 2u = 4.1u`) — again with the
/// printed text's own gap filled in (`module_a A1(p,n);` carries no
/// `#(.$xposition(1.1u))` even though the example's own comments require
/// one at that level; the same erratum `vams_mfactor_scaling.rs` already
/// disclosed for Example 1). Extended here to `$yposition` (sum, same
/// rule) and `$angle`/`$hflip`/`$vflip` (Table 9-29's other two rules:
/// sum-modulo-360, and product over a `{+1,-1}` domain) with synthetic
/// values, since the LRM prints no worked example for those three.
#[test]
fn nested_instance_placement_is_composed_per_table_9_29() {
    let src = "
        module module_c (inout electrical p, inout electrical n);
            parameter real xposition = 0.0;
            parameter real yposition = 0.0;
            parameter real angle = 0.0;
            parameter real hflip = 1.0;
            parameter real vflip = 1.0;
            real xp, yp, ang, hf, vf;
            analog begin
                xp = $xposition;
                yp = $yposition;
                ang = $angle;
                hf = $hflip;
                vf = $vflip;
                I(p, gnd) <+ V(p, gnd);
            end
        endmodule

        module module_b (inout electrical p, inout electrical n);
            module_c #(.xposition(2.0), .yposition(20.0), .angle(200.0), .hflip(-1.0), .vflip(1.0)) C1 (p, gnd);
        endmodule

        module module_a (inout electrical p, inout electrical n);
            module_b #(.xposition(1.0), .yposition(10.0), .angle(150.0), .hflip(1.0), .vflip(-1.0)) B1 (p, gnd);
        endmodule

        module top ();
            electrical p, gnd;
            module_a #(.xposition(1.1), .yposition(5.0), .angle(100.0), .hflip(-1.0), .vflip(-1.0)) A1 (p, gnd);
        endmodule
    ";
    let vars = opvars(src, "top", "A1.B1.C1");

    // Table 9-29: x/y position add; angle adds modulo 360; hflip/vflip
    // multiply.
    let expected_x = 1.1 + 1.0 + 2.0; // 4.1 — LRM §9.18 Example 2's own value.
    let expected_y = 5.0 + 10.0 + 20.0; // 35.0
    let expected_angle = (100.0f64 + 150.0 + 200.0).rem_euclid(360.0); // 450 mod 360 = 90.0
    let expected_hflip = -1.0 * 1.0 * -1.0; // 1.0
    let expected_vflip = -1.0 * -1.0 * 1.0; // 1.0

    assert!((vars["xp"] - expected_x).abs() < 1e-9, "xposition should compose to {expected_x} (1.1+1.0+2.0), got {}", vars["xp"]);
    assert!((vars["yp"] - expected_y).abs() < 1e-9, "yposition should compose to {expected_y} (5.0+10.0+20.0), got {}", vars["yp"]);
    assert!((vars["ang"] - expected_angle).abs() < 1e-9, "angle should compose to {expected_angle} ((100+150+200) mod 360), got {}", vars["ang"]);
    assert!((vars["hf"] - expected_hflip).abs() < 1e-9, "hflip should compose to {expected_hflip} (-1 * 1 * -1), got {}", vars["hf"]);
    assert!((vars["vf"] - expected_vflip).abs() < 1e-9, "vflip should compose to {expected_vflip} (-1 * -1 * 1), got {}", vars["vf"]);
}
