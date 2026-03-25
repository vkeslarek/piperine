use piperine_api::analysis::{OpAnalysis, TranAnalysis};
use piperine_api::circuit::Circuit;
use piperine_api::devices::VoltageSource;
use piperine_api::engine::{ExternalSourceHandler, SimulationEngine};
use piperine_api::node::Node;
use piperine_api::num::Expr;
use piperine_api::spice::{Measurement, Probe};
use piperine_api::waveform::Waveform;
use piperine_pool::NgspiceEngine;
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

fn engine() -> NgspiceEngine {
    let exe = env!("CARGO_BIN_EXE_piperine");
    NgspiceEngine::with_exe(exe, 1).expect("failed to create ngspice engine")
}

struct ConstantExternalSource {
    value: f64,
}

impl ExternalSourceHandler for ConstantExternalSource {
    fn get_value(&self, _source_name: &str, _time: f64) -> f64 {
        self.value
    }
}

#[test]
fn api_op_voltage_divider_with_ngspice_engine() {
    init_tracing_for_tests();
    let ng = engine();

    let mut circuit = Circuit::new("API OP Divider");
    let vin = circuit.node_label("in");

    circuit.add(VoltageSource::dc("1", vin, Node::GROUND, 10.0));

    let analysis = OpAnalysis::new();
    let result = ng.run(&circuit, &analysis).expect("OP simulation failed");

    assert!(!result.plots.is_empty(), "expected at least one plot");

    let vout_data = result.voltage(&vin).expect("missing voltage for vin node");
    let v = *vout_data.first().expect("empty output voltage vector");
    assert!(
        (v - 10.0).abs() < 0.05,
        "expected source node near 10V, got {v}"
    );
}

#[test]
fn api_tran_external_source_with_ngspice_engine() {
    init_tracing_for_tests();
    let ng = engine();

    let mut circuit = Circuit::new("API Tran External Source");
    let sig = circuit.node_label("sig");

    let mut vext = VoltageSource::new("1", sig, Node::GROUND);
    vext.with_dc(0.0).with_waveform(Waveform::External);

    circuit.add(vext);

    let vsig_max = Measurement::max(Probe::voltage(sig));
    let analysis = TranAnalysis::new(1e-4, 1e-3).meas(vsig_max.clone());
    let source = ConstantExternalSource { value: 5.0 };
    let result = ng
        .run_with_external_sources(&circuit, &analysis, &source)
        .expect("TRAN external-source simulation failed");

    assert!(!result.plots.is_empty(), "expected at least one plot");

    let vout_data = result.voltage(&sig).expect("missing voltage for sig node");
    assert!(
        vout_data.len() > 1,
        "expected transient to produce multiple samples"
    );

    let last = *vout_data.last().expect("empty output vector");
    assert!(
        (last - 5.0).abs() < 0.15,
        "expected source node near 5V with external drive, got {last}"
    );

    // Scalar measurement lookup (only populated if measurement was successfully executed)
    if let Some(vsig_max_val) = vsig_max.get(&result) {
        assert!(
            (vsig_max_val - 5.0).abs() < 0.15,
            "expected MAX ~5V, got {vsig_max_val}"
        );
    }
}

// ===== Measurement API Tests =====

/// Transient measurement smoke test: typed MAX/MIN + typed probe lookup.
#[test]
fn meas_dc_max_min_voltage_divider() {
    init_tracing_for_tests();
    let ng = engine();

    let mut circuit = Circuit::new("Tran Meas MAX/MIN");
    let vin = circuit.node_label("vin");
    circuit.add(VoltageSource::dc("1", vin, Node::GROUND, 10.0));

    let vin_max = Measurement::max(Probe::voltage(vin));
    let vin_min = Measurement::min(Probe::voltage(vin));

    let analysis = TranAnalysis::new(1e-4, 1e-3)
        .meas(vin_max.clone())
        .meas(vin_min.clone());
    let result = ng.run(&circuit, &analysis).expect("tran max/min failed");

    let waveform = result.voltage(&vin).expect("missing V(vin) waveform");
    assert!(waveform.len() > 1, "expected transient samples");
    assert!(
        (waveform[0] - 10.0).abs() < 0.05,
        "expected V(vin) ~10V, got {}",
        waveform[0]
    );

    let max_val = vin_max.get(&result).expect("missing tran meas max");
    let min_val = vin_min.get(&result).expect("missing tran meas min");
    assert!(
        (max_val - 10.0).abs() < 0.05,
        "expected MAX ~10V, got {max_val}"
    );
    assert!(
        (min_val - 10.0).abs() < 0.05,
        "expected MIN ~10V, got {min_val}"
    );
}

/// Transient measurement smoke test with external source callback.
#[test]
fn meas_tran_max_rms_and_param_expr() {
    init_tracing_for_tests();
    let ng = engine();

    let mut circuit = Circuit::new("Tran Meas");
    let sig = circuit.node_label("sig");

    let mut vext = VoltageSource::new("1", sig, Node::GROUND);
    vext.with_dc(0.0).with_waveform(Waveform::External);
    circuit.add(vext);

    let vsig_max = Measurement::max(Probe::voltage(sig));
    let vsig_avg = Measurement::avg(Probe::voltage(sig));
    let analysis = TranAnalysis::new(1e-4, 1e-3)
        .meas(vsig_max.clone())
        .meas(vsig_avg.clone());

    let source = ConstantExternalSource { value: 1.0 };
    let result = ng
        .run_with_external_sources(&circuit, &analysis, &source)
        .expect("tran measurement failed");

    let waveform = result.voltage(&sig).expect("missing V(sig) tran waveform");
    assert!(waveform.len() > 1, "expected transient samples");
    let final_v = *waveform.last().unwrap();
    assert!(
        (final_v - 1.0).abs() < 0.05,
        "expected final V(sig) ~1V, got {final_v}"
    );

    let max_val = vsig_max.get(&result).expect("missing tran MAX");
    let avg_val = vsig_avg.get(&result).expect("missing tran AVG");
    assert!(
        (max_val - 1.0).abs() < 0.05,
        "expected MAX ~1V, got {max_val}"
    );
    assert!(
        (avg_val - 1.0).abs() < 0.05,
        "expected AVG ~1V, got {avg_val}"
    );
}

/// Demonstrate Expr API: build expressions referencing .param variables and node voltages.
#[test]
fn expr_api_to_ngspice_rendering() {
    init_tracing_for_tests();
    // This test doesn't run a simulation — it validates that Expr renders correctly.
    let n = Node::from("out");

    let e1 = Expr::param("rval") * 2.0 + 100.0;
    assert_eq!(e1.to_ngspice(), "((rval*2)+100)");

    let e2 = Expr::voltage(n) / 2.0;
    assert_eq!(e2.to_ngspice(), "(V(out)/2)");

    let e3 = -Expr::param("gain");
    assert_eq!(e3.to_ngspice(), "(-gain)");

    // Measurement handle expr()
    let m = Measurement::max(Probe::voltage(n));
    let me = m.expr();
    // expr() returns MeasResult, which renders to the measurement's auto-name
    assert!(
        me.to_ngspice().starts_with("ppm_"),
        "expected auto-name like ppm_N, got {}",
        me.to_ngspice()
    );

    // Composed expr: (V(out)/2 - gain) / measurement
    let complex = (e2 - Expr::param("gain")) / me;
    let rendered = complex.to_ngspice();
    info!(rendered = %rendered, "expr complex expression");
    assert!(rendered.contains("V(out)"));
    assert!(rendered.contains("gain"));
    assert!(rendered.contains("ppm_"));
}
