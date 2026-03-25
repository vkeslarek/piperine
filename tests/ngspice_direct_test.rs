//! Direct test of NgspiceInstance (in-process, no worker).
//! This tests that ngspice FFI + EXTERNAL sources work at all.

use piperine_ngspice::NgspiceInstance;
use std::sync::Once;
use tracing::info;

fn init_tracing_for_tests() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::INFO)
            .try_init();
    });
}

#[test]
fn direct_ngspice_smoke() {
    init_tracing_for_tests();
    let ng = NgspiceInstance::new().expect("init failed");
    ng.load_circuit(&[
        "Direct OP Test".into(),
        "V1 in 0 DC 10".into(),
        "R1 in out 1k".into(),
        "R2 out 0 1k".into(),
        ".end".into(),
    ])
    .expect("load failed");

    ng.command("op").expect("op failed");

    let plots = ng.all_plots();
    info!(plots = ?plots, "direct_op plots");

    let result = ng.collect_results().expect("collect failed");
    info!(plot_count = result.plots.len(), "direct_op result plots");
    for (name, plot) in &result.plots {
        info!(plot = %name, vectors = ?plot.vectors.keys().collect::<Vec<_>>(), "direct_op vectors");
    }

    let _ = ng.command("destroy all");

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
    ])
    .expect("load failed");

    ng.command("tran 1e-4 1e-3").expect("tran failed");

    let result = ng.collect_results().expect("collect failed");
    info!(
        plot_count = result.plots.len(),
        "direct_external_source plots"
    );
    for (name, plot) in &result.plots {
        info!(plot = %name, vectors = ?plot.vectors.keys().collect::<Vec<_>>(), "direct_external_source vectors");
    }

    ng.clear_external_handlers();
}
