//! Codegen consumes the flat netlist (FLAT-01): a mid-level module — one
//! that instantiates sub-modules — builds and simulates through the normal
//! `CircuitCompiler` path. Pre-flatten, `builder.rs` hard-errored on any
//! child with its own sub-instances; the `FlattenHierarchy` pass splices
//! those away, so the nested-hierarchy guard is now unreachable for valid
//! flattened input.

use std::collections::HashMap;

use piperine_lang::parse_and_elaborate;
use piperine_lang::pom::Design;
use piperine_codegen::resolve::LoweredBody;
use piperine_codegen::CircuitCompiler;
use piperine_solver::prelude::Context;

/// A two-segment ladder: `Seg` is a mid-level module (two leaf Resistors);
/// `Top` instantiates `Seg` — a 3-level hierarchy that codegen rejected
/// before flattening.
const LADDER: &str = r#"
    discipline Electrical { potential v : Real; flow i : Real; }

    mod Resistor (inout p : Electrical, inout n : Electrical) {
        param r : Real = 1.0e3;
    }
    analog Resistor { I(p, n) <+ V(p, n) / r; }

    mod Vsrc (inout p : Electrical, inout n : Electrical) {
        param dc : Real = 1.0;
    }
    analog Vsrc { V(p, n) <- dc; }

    mod Seg (inout p : Electrical, inout n : Electrical) {
        wire mid : Electrical;
        r1 : Resistor(.p = p,   .n = mid);
        r2 : Resistor(.p = mid, .n = n);
    }

    mod Top () {
        wire gnd : Electrical;
        wire in  : Electrical;
        wire out : Electrical;
        v1 : Vsrc(.p = in, .n = gnd);
        x  : Seg(.p = in, .n = out);
        rl : Resistor(.p = out, .n = gnd);
    }
"#;

fn elaborate(src: &str) -> (Design, HashMap<String, LoweredBody>) {
    let design = parse_and_elaborate(src, &piperine_lang::SourceMap::dummy()).expect("elaborates");
    let bodies = piperine_codegen::resolve::lower_bodies(&design).expect("lowers");
    (design, bodies)
}

/// A mid-level module (`Seg`, which itself instantiates `Resistor`) builds
/// without hitting the `builder.rs:156` nested-hierarchy guard. The flat
/// root has only leaf devices.
#[test]
fn mid_level_module_builds_through_codegen() {
    let (design, bodies) = elaborate(LADDER);
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (circuit, info) = compiler.build_circuit_mapped("Top").expect("mid-level Top builds");

    // The flat root has four leaf devices: v1, x.r1, x.r2, rl (Seg is gone).
    let labels: Vec<&str> = circuit.all_devices().iter().map(|d| d.name()).collect();
    assert_eq!(
        labels,
        vec!["v1", "x.r1", "x.r2", "rl"],
        "Seg inlined: flat root has leaf devices with prefixed labels"
    );

    // The lifted wire `x.mid` is a top-level net.
    assert!(info.nets.contains_key("x.mid"), "lifted wire x.mid is a top net");
    assert!(info.nets.contains_key("out"), "authored wire out survives");
    assert!(info.nets.contains_key("in"), "authored wire in survives");
}

/// The flattened ladder simulates correctly: Vsrc(1V) drives two 1k series
/// resistors (Seg) in series with a 1k load → `out` = 1V · 1k/(1k+2k) = 1/3 V
/// and `x.mid` (Seg's internal midpoint) = 2/3 V.
#[test]
fn flattened_ladder_simulates_correctly() {
    let (design, bodies) = elaborate(LADDER);
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (mut circuit, info) = compiler.build_circuit_mapped("Top").expect("builds");

    let result = circuit.dc(Context::default()).unwrap().solve().unwrap();

    let out = info.nets.get("out").expect("net `out`");
    let xmid = info.nets.get("x.mid").expect("net `x.mid`");
    let v_out = result.get_node(out).expect("v(out)");
    let v_mid = result.get_node(&xmid).expect("v(x.mid)");

    // Seg is a 2k series string (r1=r2=1k); load rl=1k from out to gnd.
    // Total seen from v1: r1 + (r2 ∥ rl) = 1k + (1k ∥ 1k) = 1.5k.
    // v(out) = 1V · 500/1500 = 1/3 V; v(x.mid) = 1V · 1000/1500 = 2/3 V.
    assert!(
        (v_out - 1.0 / 3.0).abs() < 1e-9,
        "v(out) = 1/3 V for the flattened ladder, got {v_out}"
    );
    assert!(
        (v_mid - 2.0 / 3.0).abs() < 1e-9,
        "v(x.mid) = 2/3 V (Seg's internal node lifted and simulated), got {v_mid}"
    );
}

/// Two instances of the same mid-level module coexist — distinct spliced
/// labels (`a.*` and `b.*`) and distinct lifted wires, each simulating
/// correctly.
#[test]
fn two_mid_level_instances_produce_distinct_spliced_labels() {
    let src = r#"
        discipline Electrical { potential v : Real; flow i : Real; }
        mod Resistor (inout p : Electrical, inout n : Electrical) { param r : Real = 1.0e3; }
        analog Resistor { I(p, n) <+ V(p, n) / r; }
        mod Seg (inout p : Electrical, inout n : Electrical) {
            wire mid : Electrical;
            r1 : Resistor(.p = p,   .n = mid);
            r2 : Resistor(.p = mid, .n = n);
        }
        mod Top () {
            wire gnd : Electrical;
            wire in  : Electrical;
            a : Seg(.p = in, .n = gnd);
            b : Seg(.p = in, .n = gnd);
        }
    "#;
    let (design, bodies) = elaborate(src);
    let mut compiler = CircuitCompiler::new(&design, &bodies);
    let (circuit, info) = compiler.build_circuit_mapped("Top").expect("builds");

    let labels: Vec<&str> = circuit.all_devices().iter().map(|d| d.name()).collect();
    assert_eq!(
        labels,
        vec!["a.r1", "a.r2", "b.r1", "b.r2"],
        "two Seg instances produce collision-free a.*/b.* labels"
    );
    assert!(info.nets.contains_key("a.mid"), "lifted wire a.mid");
    assert!(info.nets.contains_key("b.mid"), "lifted wire b.mid — distinct from a.mid");
}
