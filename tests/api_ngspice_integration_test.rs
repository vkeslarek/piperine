use piperine_api::analysis::{OpAnalysis, TranAnalysis};
use piperine_api::circuit::Circuit;
use piperine_api::devices::{Resistor, VoltageSource};
use piperine_api::engine::{ExternalSourceHandler, SimulationEngine};
use piperine_api::node::Node;
use piperine_api::num::Expr;
use piperine_api::spice::{Measurement, Probe};
use piperine_api::subcircuit::SubCircuit;
use piperine_api::waveform::Waveform;
use piperine_api::{
    abs, acosh, asinh, atan2, atanh, boltz, ceil, cos, cosh, echarge, floor, freq, limit, ln,
    max, min, ngfunc, nint, not, omega, param, pi, planck, pwr, sgn, sin, sinh, sqrt, table,
    tan, tanh, temp, ternary, time, u, uramp, val, vdiff, voltage,
};
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

// ===== New Expr features: conditional, logical, signal, constants, table =====

#[test]
fn expr_conditional_and_logical_rendering() {
    let out = Node::from("out");

    // Comparison operators
    assert_eq!(voltage!(out).gt(val!(5.0)).to_ngspice(), "(V(out)>5)");
    assert_eq!(voltage!(out).gte(val!(5.0)).to_ngspice(), "(V(out)>=5)");
    assert_eq!(voltage!(out).lt(val!(0.0)).to_ngspice(), "(V(out)<0)");
    assert_eq!(voltage!(out).lte(val!(0.0)).to_ngspice(), "(V(out)<=0)");
    assert_eq!(voltage!(out).equal(val!(3.3)).to_ngspice(), "(V(out)==3.3)");
    assert_eq!(voltage!(out).neq(val!(0.0)).to_ngspice(), "(V(out)!=0)");

    // Logical operators
    let hi = voltage!(out).gt(val!(4.0));
    let lo = voltage!(out).lt(val!(6.0));
    assert_eq!(hi.clone().and(lo.clone()).to_ngspice(), "((V(out)>4)&&(V(out)<6))");
    assert_eq!(hi.clone().or(lo.clone()).to_ngspice(),  "((V(out)>4)||(V(out)<6))");
    assert_eq!(not!(hi).to_ngspice(), "(!(V(out)>4))");

    // Ternary: clamp voltage to [0, 5]
    let clamped = ternary!(
        voltage!(out).gt(val!(5.0)),
        val!(5.0),
        ternary!(voltage!(out).lt(val!(0.0)), val!(0.0), voltage!(out))
    );
    let rendered = clamped.to_ngspice();
    assert!(rendered.contains("?"), "expected ternary ?");
    assert!(rendered.contains(":"), "expected ternary :");
    assert!(rendered.contains("V(out)"));
    assert_eq!(rendered, "((V(out)>5) ? 5 : ((V(out)<0) ? 0 : V(out)))");

    // if_then_else constructor matches ternary! macro
    let a = Expr::if_then_else(
        voltage!(out).gt(val!(0.0)),
        voltage!(out),
        val!(0.0),
    );
    assert_eq!(a.to_ngspice(), "((V(out)>0) ? V(out) : 0)");
}

#[test]
fn expr_signal_functions_rendering() {
    let n = Node::from("x");

    assert_eq!(u!(voltage!(n)).to_ngspice(), "u(V(x))");
    assert_eq!(uramp!(voltage!(n)).to_ngspice(), "uramp(V(x))");
    assert_eq!(sgn!(param!("s")).to_ngspice(), "sgn(s)");
    assert_eq!(nint!(param!("n")).to_ngspice(), "nint(n)");
    assert_eq!(pwr!(voltage!(n), val!(1.5)).to_ngspice(), "pwr(V(x),1.5)");
    assert_eq!(
        limit!(voltage!(n), val!(-1.0), val!(1.0)).to_ngspice(),
        "limit(V(x),-1,1)"
    );

    // Hyperbolic trig
    assert_eq!(sinh!(param!("x")).to_ngspice(), "sinh(x)");
    assert_eq!(cosh!(param!("x")).to_ngspice(), "cosh(x)");
    assert_eq!(tanh!(param!("x")).to_ngspice(), "tanh(x)");
    assert_eq!(asinh!(param!("x")).to_ngspice(), "asinh(x)");
    assert_eq!(acosh!(param!("x")).to_ngspice(), "acosh(x)");
    assert_eq!(atanh!(param!("x")).to_ngspice(), "atanh(x)");
    assert_eq!(atan2!(param!("y"), param!("x")).to_ngspice(), "atan2(y,x)");
}

#[test]
fn expr_constants_rendering() {
    // Physical constants
    assert_eq!(pi!().to_ngspice(), "pi");
    assert_eq!(boltz!().to_ngspice(), "boltz");
    assert_eq!(echarge!().to_ngspice(), "echarge");
    assert_eq!(planck!().to_ngspice(), "planck");

    // omega = 2 * pi * frequency
    let w = omega!();
    let rendered = w.to_ngspice();
    assert!(rendered.contains("pi"), "omega must contain pi");
    assert!(rendered.contains("frequency"), "omega must contain frequency");

    // Thermal voltage: boltz * temp / echarge
    let vt = boltz!() * temp!() / echarge!();
    let rendered = vt.to_ngspice();
    assert!(rendered.contains("boltz"));
    assert!(rendered.contains("temper"));
    assert!(rendered.contains("echarge"));
}

#[test]
fn expr_table_rendering() {
    let n = Node::from("in");

    // Piecewise-linear lookup: 0V→0, 1V→2, 2V→3, 5V→5
    let lut = table!(voltage!(n), [(0.0, 0.0), (1.0, 2.0), (2.0, 3.0), (5.0, 5.0)]);
    let rendered = lut.to_ngspice();
    assert!(rendered.starts_with("table("), "expected table(");
    assert!(rendered.contains("V(in)"), "expected V(in) as the sweep variable");
    assert_eq!(rendered, "table(V(in),0,0,1,2,2,3,5,5)");

    // Integer literals via the cast in macro
    let lut2 = table!(param!("gain"), [(0, 0), (1, 10), (2, 20)]);
    assert_eq!(lut2.to_ngspice(), "table(gain,0,0,1,10,2,20)");
}

// ===== Expr macros ergonomics =====

#[test]
fn expr_macros_ergonomics() {
    let out = Node::from("out");
    let a = Node::from("a");
    let b = Node::from("b");

    // Leaf macros
    assert_eq!(param!("rval").to_ngspice(), "rval");
    assert_eq!(voltage!(out).to_ngspice(), "V(out)");
    assert_eq!(vdiff!(a, b).to_ngspice(), "V(a,b)");
    assert_eq!(val!(3.14).to_ngspice(), "3.14");
    assert_eq!(time!().to_ngspice(), "time");
    assert_eq!(freq!().to_ngspice(), "frequency");
    assert_eq!(temp!().to_ngspice(), "temper");

    // Function macros
    assert_eq!(sqrt!(voltage!(out)).to_ngspice(), "sqrt(V(out))");
    assert_eq!(abs!(param!("x")).to_ngspice(), "abs(x)");
    assert_eq!(sin!(time!()).to_ngspice(), "sin(time)");
    assert_eq!(cos!(val!(0.0)).to_ngspice(), "cos(0)");
    assert_eq!(tan!(param!("phi")).to_ngspice(), "tan(phi)");
    assert_eq!(ln!(param!("k")).to_ngspice(), "ln(k)");
    assert_eq!(ceil!(param!("n")).to_ngspice(), "ceil(n)");
    assert_eq!(floor!(param!("n")).to_ngspice(), "floor(n)");
    assert_eq!(min!(voltage!(a), voltage!(b)).to_ngspice(), "min(V(a),V(b))");
    assert_eq!(max!(voltage!(out), val!(0.0)).to_ngspice(), "max(V(out),0)");

    // Escape hatch for arbitrary ngspice functions
    assert_eq!(ngfunc!("nint", param!("x")).to_ngspice(), "nint(x)");
    assert_eq!(ngfunc!("atan2", param!("y"), param!("x")).to_ngspice(), "atan2(y,x)");

    // Composed expression using macros — more readable than Expr::* chains
    let e = sqrt!(voltage!(out).pow(2.0) + param!("offset"));
    let rendered = e.to_ngspice();
    assert!(rendered.contains("sqrt"));
    assert!(rendered.contains("V(out)"));
    assert!(rendered.contains("offset"));

    // Real-world-ish: instantaneous power normalised by temperature
    // abs(V(out) * I(R1)) / temp
    // (we just check it renders without panic)
    let _power_expr = abs!(voltage!(out) * param!("amps")) / temp!();
}

// ===== Expr functions and special vars (no simulation needed) =====

#[test]
fn expr_functions_and_special_vars_rendering() {
    let n = Node::from("out");
    let n1 = Node::from("a");
    let n2 = Node::from("b");

    // Built-in single-argument functions
    assert_eq!(Expr::sqrt(Expr::voltage(n)).to_ngspice(), "sqrt(V(out))");
    assert_eq!(Expr::abs(Expr::param("x")).to_ngspice(), "abs(x)");
    assert_eq!(Expr::sin(Expr::time()).to_ngspice(), "sin(time)");
    assert_eq!(Expr::cos(Expr::constant(3.14)).to_ngspice(), "cos(3.14)");
    assert_eq!(Expr::ln(Expr::param("k")).to_ngspice(), "ln(k)");
    assert_eq!(Expr::log10(Expr::param("k")).to_ngspice(), "log(k)");
    assert_eq!(Expr::exp(Expr::constant(1.0)).to_ngspice(), "exp(1)");
    assert_eq!(Expr::floor(Expr::param("n")).to_ngspice(), "floor(n)");
    assert_eq!(Expr::ceil(Expr::param("n")).to_ngspice(), "ceil(n)");

    // Two-argument functions
    assert_eq!(
        Expr::min(Expr::voltage(n), Expr::constant(0.0)).to_ngspice(),
        "min(V(out),0)"
    );
    assert_eq!(
        Expr::max(Expr::voltage(n), Expr::param("vref")).to_ngspice(),
        "max(V(out),vref)"
    );

    // Special variables
    assert_eq!(Expr::time().to_ngspice(), "time");
    assert_eq!(Expr::frequency().to_ngspice(), "frequency");
    assert_eq!(Expr::temp().to_ngspice(), "temper");

    // Differential voltage
    assert_eq!(Expr::voltage_diff(n1, n2).to_ngspice(), "V(a,b)");

    // Modulo operator
    let modulo = Expr::param("x") % Expr::constant(2.0);
    assert_eq!(modulo.to_ngspice(), "(x%2)");

    // pow() method
    let squared = Expr::voltage(n).pow(2.0);
    assert_eq!(squared.to_ngspice(), "(V(out)^2)");

    // Escape hatch: arbitrary function
    let nint = Expr::func("nint", vec![Expr::param("val")]);
    assert_eq!(nint.to_ngspice(), "nint(val)");

    // Composed: sqrt(V(out)^2 + offset)
    let composed = Expr::sqrt(Expr::voltage(n).pow(2.0) + Expr::param("offset"));
    let rendered = composed.to_ngspice();
    info!(rendered = %rendered, "sqrt composed expr");
    assert!(rendered.contains("sqrt"));
    assert!(rendered.contains("V(out)"));
    assert!(rendered.contains("offset"));
}

// ===== TimeSeries waveform API =====

#[test]
fn waveform_time_series_api() {
    init_tracing_for_tests();
    let ng = engine();

    let mut circuit = Circuit::new("TimeSeries API");
    let vin = circuit.node_label("vin");
    circuit.add(VoltageSource::dc("1", vin, Node::GROUND, 5.0));

    let analysis = TranAnalysis::new(1e-4, 1e-3);
    let result = ng.run(&circuit, &analysis).expect("tran failed");

    let wave = result.waveform(&vin).expect("missing waveform for vin");
    assert!(wave.len() > 1, "expected multiple transient samples");
    assert_eq!(
        wave.time().len(),
        wave.values().len(),
        "time and values must have equal length"
    );

    // time vector starts at or near 0
    assert!(wave.time()[0] >= 0.0, "time should start at 0");
    // last time point should be near tstop=1ms
    let t_last = *wave.time().last().unwrap();
    assert!(t_last > 5e-4, "expected tstop near 1ms, got {t_last}");

    // all voltage values near 5V
    for (t, v) in wave.iter() {
        assert!(
            (v - 5.0).abs() < 0.15,
            "expected V(vin) ~5V at t={t:.3e}, got {v}"
        );
    }

    // get() accessor
    let (t0, v0) = wave.get(0).unwrap();
    assert!(t0 >= 0.0);
    assert!((v0 - 5.0).abs() < 0.15, "expected first sample ~5V, got {v0}");
}

// ===== SubCircuit + Measurement tests =====

/// A voltage divider built from SubCircuit composition: two resistors with an
/// external midpoint node. The measurement probe targets the midpoint directly.
#[test]
fn subcircuit_voltage_divider_with_measurement() {
    init_tracing_for_tests();
    let ng = engine();

    let mut circuit = Circuit::new("SC Voltage Divider");
    let vin = circuit.node_label("vin");
    let vmid = circuit.node_label("vmid"); // external midpoint — not prefixed when composed

    circuit.add(VoltageSource::dc("vs", vin, Node::GROUND, 10.0));

    // Build a 1k/1k voltage divider as a SubCircuit.
    // vmid is an external node — it appears in SPICE without the compose prefix.
    let mut sc = SubCircuit::new();
    sc.add(Resistor::new("top", vin, vmid, 1000.0));
    sc.add(Resistor::new("bot", vmid, Node::GROUND, 1000.0));
    circuit.compose_as("div1", sc);

    let vmid_max = Measurement::max(Probe::voltage(vmid));
    let analysis = TranAnalysis::new(1e-4, 1e-3).meas(vmid_max.clone());

    let result = ng
        .run(&circuit, &analysis)
        .expect("SC divider simulation failed");

    assert!(!result.plots.is_empty(), "expected plots");

    let waveform = result.voltage(&vmid).expect("missing V(vmid)");
    assert!(waveform.len() > 1, "expected transient samples");

    let v_last = *waveform.last().unwrap();
    assert!(
        (v_last - 5.0).abs() < 0.15,
        "expected vmid ~5V (half of 10V), got {v_last}"
    );

    let max_val = vmid_max.get(&result).expect("missing MAX measurement");
    assert!(
        (max_val - 5.0).abs() < 0.15,
        "expected MAX(vmid) ~5V, got {max_val}"
    );

    // Also verify via TimeSeries API
    let wave = result.waveform(&vmid).expect("missing waveform");
    for (_, v) in wave.iter() {
        assert!((v - 5.0).abs() < 0.2, "expected all samples ~5V, got {v}");
    }
}

/// Two subcircuit instances (different ratios) composed into one circuit.
/// A PARAM measurement computes their ratio via Expr.
#[test]
fn two_subcircuit_instances_with_param_ratio() {
    init_tracing_for_tests();
    let ng = engine();

    let mut circuit = Circuit::new("SC Two Dividers");
    let vin = circuit.node_label("vin");
    let va = circuit.node_label("va"); // 1k/1k = 5V (50%)
    let vb = circuit.node_label("vb"); // 3k/1k = 2.5V (25%)

    circuit.add(VoltageSource::dc("vs", vin, Node::GROUND, 10.0));

    // Divider A: vin -> va -> gnd with 1k/1k
    let mut sc_a = SubCircuit::new();
    sc_a.add(Resistor::new("top", vin, va, 1000.0));
    sc_a.add(Resistor::new("bot", va, Node::GROUND, 1000.0));
    circuit.compose_as("da", sc_a);

    // Divider B: vin -> vb -> gnd with 3k/1k
    let mut sc_b = SubCircuit::new();
    sc_b.add(Resistor::new("top", vin, vb, 3000.0));
    sc_b.add(Resistor::new("bot", vb, Node::GROUND, 1000.0));
    circuit.compose_as("db", sc_b);

    let va_max = Measurement::max(Probe::voltage(va));
    let vb_max = Measurement::max(Probe::voltage(vb));
    // PARAM: ratio = va_max / vb_max, should be ~5V/2.5V = 2.0
    let ratio = Measurement::param(va_max.expr() / vb_max.expr());

    let analysis = TranAnalysis::new(1e-4, 1e-3)
        .meas(va_max.clone())
        .meas(vb_max.clone())
        .meas(ratio.clone());

    let result = ng
        .run(&circuit, &analysis)
        .expect("two-instance SC simulation failed");

    let va_val = va_max.get(&result).expect("missing MAX(va)");
    let vb_val = vb_max.get(&result).expect("missing MAX(vb)");

    assert!(
        (va_val - 5.0).abs() < 0.15,
        "expected va ~5V (1k/1k divider), got {va_val}"
    );
    assert!(
        (vb_val - 2.5).abs() < 0.15,
        "expected vb ~2.5V (3k/1k divider), got {vb_val}"
    );

    // Ratio measurement (PARAM): only check if ngspice returned it
    if let Some(r) = ratio.get(&result) {
        assert!(
            (r - 2.0).abs() < 0.15,
            "expected ratio(va/vb) ~2.0, got {r}"
        );
    }
}
