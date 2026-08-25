//! LRM-COVERS: 9.19, 6.5.6
//! §9.19 (LRMC-C7-09, T50): `$port_connected` distinguishes connected
//! from unconnected ports — a real per-instance compiled-kernel effect,
//! not just name resolution. Two instances of the same module, one
//! leaving a port unconnected, get different `$port_connected` values
//! for the same port position.

use piperine_codegen::CircuitCompiler;
use piperine_lang::parse_and_elaborate;
use piperine_solver::prelude::Context;

const SRC: &str = "
    module leaf (inout electrical p, inout electrical n);
        real connected_flag;
        analog begin
            connected_flag = $port_connected(n);
            V(p, n) <+ 1.0;
        end
    endmodule

    module bleed (inout electrical p, inout electrical n);
        analog I(p, n) <+ V(p, n) / 1.0e6;
    endmodule

    module top ();
        electrical vin, vin2, gnd;
        leaf both (vin, gnd);
        leaf partial (vin2);
        bleed r1 (vin2, gnd);
    endmodule
";

#[test]
fn port_connected_distinguishes_connected_from_unconnected() {
    let design = parse_and_elaborate(SRC, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, _info) = compiler.build_circuit_mapped("top").expect("compiles");

    circuit.dc(Context::default()).expect("dc starts").solve().expect("dc converges");

    let both = circuit.all_devices().iter().find(|d| d.name() == "both").expect("`both`");
    let both_flag = both.read_opvars().iter().find(|(n, _)| n == "connected_flag").map(|(_, v)| *v).unwrap();
    assert_eq!(both_flag, 1.0, "both instance connects both ports");

    let partial = circuit.all_devices().iter().find(|d| d.name() == "partial").expect("`partial`");
    let partial_flag = partial.read_opvars().iter().find(|(n, _)| n == "connected_flag").map(|(_, v)| *v).unwrap();
    assert_eq!(partial_flag, 0.0, "partial instance leaves port `n` unconnected");
}

/// §9.20: dynamic node aliasing fails loud citing MD-18, not a generic
/// "unsupported" message.
#[test]
fn analog_node_alias_fails_loud_citing_md18() {
    let src = r#"
        module dut (inout electrical p, inout electrical n);
            real ok;
            analog begin
                ok = $analog_node_alias(p, "top.other.net");
                V(p, n) <+ 1.0;
            end
        endmodule
    "#;
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    let err = match piperine_codegen::kernel::analog::AnalogKernel::compile(&bodies["dut"]) {
        Ok(_) => panic!("$analog_node_alias must fail loud"),
        Err(e) => e,
    };
    let msg = format!("{err}").to_lowercase();
    assert!(msg.contains("md-18"), "should cite the architectural reason: {msg}");
}
