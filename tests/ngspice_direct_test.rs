//! Direct test of NgspiceInstance (in-process, no worker).
//! This tests that ngspice FFI + EXTERNAL sources work at all.

use piperine_ngspice::NgspiceInstance;

#[test]
fn direct_op() {
    let ng = NgspiceInstance::new().expect("init failed");
    ng.load_circuit(&[
        "Direct OP Test".into(),
        "V1 in 0 DC 10".into(),
        "R1 in out 1k".into(),
        "R2 out 0 1k".into(),
        ".end".into(),
    ]).expect("load failed");

    ng.command("op").expect("op failed");

    let plots = ng.all_plots();
    println!("Plots: {:?}", plots);

    let result = ng.collect_results().expect("collect failed");
    println!("Result plots: {}", result.plots.len());
    for (name, plot) in &result.plots {
        println!("  Plot '{}': {:?}", name, plot.vectors.keys().collect::<Vec<_>>());
    }
}

#[test]
fn direct_external_source() {
    let ng = NgspiceInstance::new().expect("init failed");

    // Set handler BEFORE loading circuit
    ng.set_vsrc_handler(|_name, _time| {
        5.0 // constant 5V
    });

    ng.load_circuit(&[
        "External Direct Test".into(),
        "V1 sig 0 DC 0 EXTERNAL".into(),
        "R1 sig out 1k".into(),
        "R2 out 0 1k".into(),
        ".end".into(),
    ]).expect("load failed");

    ng.command("tran 1e-4 1e-3").expect("tran failed");

    let result = ng.collect_results().expect("collect failed");
    println!("External source test: {} plots", result.plots.len());
    for (name, plot) in &result.plots {
        println!("  Plot '{}': {:?}", name, plot.vectors.keys().collect::<Vec<_>>());
    }

    ng.clear_external_handlers();
}
