//! Digital-path throughput microbenchmark: a chain of K inverters driven by
//! T input toggles through the circuit-level event loop.
//!
//! Run: `cargo run --release -p piperine-lang --example bench_digital [K] [T]`

use std::time::Instant;

use piperine_codegen::CircuitCompiler;
use piperine_lang::{parse_and_elaborate, ppr_to_ir};
use piperine_solver::digital::{DigitalEvent, DigitalNet, LogicValue};

fn main() {
    let k: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(100);
    let t: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(10_000);

    let mut src = String::from(
        "discipline Bit { storage Boolean; }\n\
         mod Inv ( input a : Bit, output y : Bit );\n\
         digital Inv { y <- !a; }\n\
         mod Top ( input d : Bit, output q : Bit ) {\n",
    );
    for i in 0..k {
        src.push_str(&format!("    wire n{i} : Bit;\n"));
    }
    for i in 0..k {
        let input = if i == 0 { "d".to_string() } else { format!("n{}", i - 1) };
        let output = if i == k - 1 { "q".to_string() } else { format!("n{i}") };
        src.push_str(&format!("    inv{i} : Inv ( {input}, {output} );\n"));
    }
    src.push_str("}\n");

    let elab = parse_and_elaborate(&src).expect("elaborate");
    let ir = ppr_to_ir(&elab);
    let mut compiler = CircuitCompiler::new(&ir);
    let mut circuit = compiler.build_circuit("Top").expect("build");
    circuit.init_digital();
    circuit.rebuild_digital_topology();

    // Net 0 is `d` (first digital net registered).
    let d = DigitalNet(0);
    let start = Instant::now();
    let mut time = 0.0;
    for i in 0..t {
        time += 1.0e-9;
        circuit.digital_state.schedule(DigitalEvent {
            time,
            net: d,
            value: if i % 2 == 0 { LogicValue::One } else { LogicValue::Zero },
            source: usize::MAX,
            seq: i as u64,
        });
        circuit.run_digital_at(time);
    }
    let elapsed = start.elapsed();

    let device_evals = k as u64 * t as u64;
    println!(
        "chain K={k} toggles T={t}: {:.3}s total, {:.0} device-evals/s, {:.1} ns/device-eval",
        elapsed.as_secs_f64(),
        device_evals as f64 / elapsed.as_secs_f64(),
        elapsed.as_nanos() as f64 / device_evals as f64,
    );
}
