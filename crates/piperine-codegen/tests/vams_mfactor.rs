//! LRM-COVERS: 9.18
//! §9.18 (LRMC-C7-09, T49): `$mfactor` reads a module's own declared
//! `mfactor` parameter into `SimCtx.mfactor`, real per-instance effect
//! (not just name resolution) — an instance overriding `#(.mfactor(N))`
//! gets its own compiled kernel's `$mfactor` call return `N`, not the
//! ABI default `1.0`.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::Context;

const SRC: &str = "
    module dut (inout electrical p, inout electrical n);
        parameter real mfactor = 1.0;
        real m;
        analog begin
            m = $mfactor;
            I(p, n) <+ V(p, n);
        end
    endmodule

    module top ();
        electrical vin, gnd;
        dut #(.mfactor(3.0)) d1 (vin, gnd);
    endmodule
";

#[test]
fn mfactor_override_reaches_the_compiled_instance() {
    let design = parse_and_elaborate(SRC, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, _info) = compiler.build_circuit_mapped("top").expect("compiles");

    circuit.dc(Context::default()).expect("dc starts").solve().expect("dc converges");

    let d1 = circuit.all_devices().iter().find(|d| d.name() == "d1").expect("device `d1`");
    let opvars = d1.read_opvars();
    let m = opvars.iter().find(|(n, _)| n == "m").map(|(_, v)| *v).expect("opvar `m`");
    assert!((m - 3.0).abs() < 1e-9, "$mfactor should read the instance's own override (3.0), got {m}");
}
