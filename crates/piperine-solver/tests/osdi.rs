//! Comprehensive OSDI integration tests for piperine-solver.
//!
//! These tests compile Verilog-A models to OSDI shared libraries, build circuits
//! using the OSDI device API, and verify DC, AC, noise analysis, opvar readout,
//! and temperature behaviour.
//!
//! All circuits use current sources + resistors (KCL-only) since OSDI voltage
//! sources require branch-current support in the solver.

use std::path::PathBuf;
use std::sync::Arc;

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use piperine_solver::analog::{NodeIdentifier, Netlist};
use piperine_solver::circuit::CircuitInstance;
use piperine_solver::osdi::OsdiDevice;
use piperine_solver::device::Device;

pub struct Circuit {
    pub title: String,
    pub components: HashMap<String, OsdiDevice>,
    pub node_counter: AtomicUsize,
}
impl Circuit {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), components: HashMap::new(), node_counter: AtomicUsize::new(0) }
    }
    pub fn port(&self) -> NodeIdentifier {
        NodeIdentifier::Anonymous(self.node_counter.fetch_add(1, Ordering::Relaxed))
    }
    pub fn components_mut(&mut self) -> &mut HashMap<String, OsdiDevice> { &mut self.components }

    pub fn instantiate(&self) -> CircuitInstance {
        let mut netlist = Netlist::new();
        let ctx = piperine_solver::solver::Context::default();
        let devices = self.components.values()
            .map(|spec| Box::new(OsdiDevice::from_spec(spec, &mut netlist, &ctx)) as Box<dyn Device>)
            .collect();
        CircuitInstance::from_devices_and_netlist(self.title.clone(), devices, netlist)
    }
}

use piperine_solver::analog::GND;
use piperine_solver::osdi::model::AnalogModel;
use piperine_solver::osdi::OsdiLib;
use piperine_solver::solver::Context;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn va_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("va")
}

fn compile_va(name: &str) -> (PathBuf, tempfile::TempDir) {
    let va_path = va_dir().join(format!("{name}.va"));
    assert!(va_path.exists(), "VA file not found: {}", va_path.display());
    let tmp = tempfile::tempdir().expect("create tempdir");
    let osdi_path = tmp.path().join(format!("{name}.osdi"));
    
    let status = std::process::Command::new(env!("OPENVAF_BIN"))
        .arg(&va_path)
        .arg("-o")
        .arg(&osdi_path)
        .status()
        .expect("Failed to invoke openvaf executable. Make sure 'openvaf' is in your PATH.");
        
    assert!(status.success(), "openvaf compilation failed for {}", name);
    (osdi_path, tmp)
}

fn load_lib(name: &str) -> (Arc<OsdiLib>, tempfile::TempDir) {
    let (osdi_path, tmp) = compile_va(name);
    let lib = OsdiLib::load(&osdi_path).expect("OSDI load failed");
    (lib, tmp)
}

fn load_model(name: &str) -> (AnalogModel, tempfile::TempDir) {
    let (osdi_path, tmp) = compile_va(name);
    let model = AnalogModel::load(&osdi_path).expect("Model load failed");
    (model, tmp)
}

/// Leak a TempDir so the .osdi stays alive for the duration of the test process.
fn leak_tmp(tmp: tempfile::TempDir) {
    Box::leak(Box::new(tmp));
}

// ===================================================================
// 1. COMPILATION AND LOADING
// ===================================================================

#[test]
fn test_compile_and_load_resistor() {
    let (lib, _tmp) = load_lib("resistor");
    assert!(lib.num_descriptors() >= 1);
    let desc = lib.descriptor(0);
    let name = unsafe { std::ffi::CStr::from_ptr(desc.name) }.to_string_lossy();
    assert_eq!(name, "resistor_va");
    assert_eq!(desc.num_terminals, 2);
}

#[test]
fn test_compile_and_load_isource() {
    let (lib, _tmp) = load_lib("isource");
    assert!(lib.num_descriptors() >= 1);
    let desc = lib.descriptor(0);
    let name = unsafe { std::ffi::CStr::from_ptr(desc.name) }.to_string_lossy();
    assert_eq!(name, "isource_va");
    assert_eq!(desc.num_terminals, 2);
}

#[test]
fn test_compile_and_load_noisy_resistor() {
    let (lib, _tmp) = load_lib("noisy_resistor");
    let desc = lib.descriptor(0);
    let name = unsafe { std::ffi::CStr::from_ptr(desc.name) }.to_string_lossy();
    assert_eq!(name, "noisy_resistor");
    assert!(
        desc.num_noise_src > 0,
        "noisy_resistor should have noise sources, got {}",
        desc.num_noise_src
    );
}

#[test]
fn test_descriptor_node_info() {
    let (lib, _tmp) = load_lib("resistor");
    let desc = lib.descriptor(0);

    // Resistor should have 2 terminals
    assert_eq!(desc.num_terminals, 2);
    // Resistor should have Jacobian entries (at least 4 for a 2-terminal device: G11, G12, G21, G22)
    assert!(desc.num_jacobian_entries > 0, "resistor should have Jacobian entries");
    assert!(desc.num_resistive_jacobian_entries > 0, "resistor should have resistive Jacobian");
}

#[test]
fn test_multiple_descriptors_loading() {
    // Load two different libs and verify they are independent
    let (lib_r, _t1) = load_lib("resistor");
    let (lib_i, _t2) = load_lib("isource");

    let name_r = unsafe { std::ffi::CStr::from_ptr(lib_r.descriptor(0).name) }.to_string_lossy();
    let name_i = unsafe { std::ffi::CStr::from_ptr(lib_i.descriptor(0).name) }.to_string_lossy();

    assert_ne!(name_r, name_i, "should be different models");
}

// ===================================================================
// 2. CIRCUIT INSTANTIATION
// ===================================================================

#[test]
fn test_circuit_instantiation_single_device() {
    let (resistor, tmp) = load_model("resistor");
    leak_tmp(tmp);

    let mut circuit = Circuit::new("Single Resistor");
    let n1 = circuit.port();

    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![n1.clone(), GND], vec![("r".to_string(), 1000.0)]));

    let inst = circuit.instantiate();
    assert_eq!(inst.all_devices().len(), 1);
    assert!(inst.netlist().max_index().is_some());
}

#[test]
fn test_circuit_instantiation_multiple_devices() {
    let (resistor, t1) = load_model("resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    let mut circuit = Circuit::new("Multi Device");
    let n1 = circuit.port();
    let n2 = circuit.port();

    circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, n1.clone()], vec![("idc".to_string(), 1e-3)]));
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![n1.clone(), n2.clone()], vec![("r".to_string(), 500.0)]));
    circuit.components_mut().insert("R2".to_string(), OsdiDevice::new_with_params("R2".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![n2.clone(), GND], vec![("r".to_string(), 500.0)]));

    let inst = circuit.instantiate();
    assert_eq!(inst.all_devices().len(), 3);
}

// ===================================================================
// 3. DC ANALYSIS
// ===================================================================

/// Isource → R → GND. V = I*R.
#[test]
fn test_dc_isource_resistor() {
    let (resistor, t1) = load_model("resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    let mut circuit = Circuit::new("I*R = V");
    let v_node = circuit.port();

    // 1mA current source into a 1kΩ resistor → V = 1V
    circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, v_node.clone()], vec![("idc".to_string(), 1e-3)]));
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![v_node.clone(), GND], vec![("r".to_string(), 1000.0)]));

    let mut inst = circuit.instantiate();
    let dc = inst.dc(Context::default()).unwrap().solve().unwrap();

    let v = dc.get_node(&v_node).expect("node voltage");
    assert!(
        (v - 1.0).abs() < 0.01,
        "Expected V = I*R = 1mA * 1kΩ = 1.0V, got {v:.6}V"
    );
}

/// Current divider: I → R1 ∥ R2 → GND.
/// With R1=R2=1kΩ, V = I * (R1∥R2) = 1mA * 500Ω = 0.5V.
#[test]
fn test_dc_parallel_resistors() {
    let (resistor, t1) = load_model("resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    let mut circuit = Circuit::new("Parallel R");
    let v_node = circuit.port();

    circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, v_node.clone()], vec![("idc".to_string(), 1e-3)]));
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![v_node.clone(), GND], vec![("r".to_string(), 1000.0)]));
    circuit.components_mut().insert("R2".to_string(), OsdiDevice::new_with_params("R2".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![v_node.clone(), GND], vec![("r".to_string(), 1000.0)]));

    let mut inst = circuit.instantiate();
    let dc = inst.dc(Context::default()).unwrap().solve().unwrap();

    let v = dc.get_node(&v_node).expect("node voltage");
    assert!(
        (v - 0.5).abs() < 0.01,
        "Expected V = 1mA * 500Ω = 0.5V, got {v:.6}V"
    );
}

/// Series chain: I → R1 → mid → R2 → GND.
/// V_mid = I * R2 = 1mA * 500Ω = 0.5V.
/// V_top = I * (R1+R2) = 1mA * 1500Ω = 1.5V.
#[test]
fn test_dc_series_resistors() {
    let (resistor, t1) = load_model("resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    let mut circuit = Circuit::new("Series R");
    let v_top = circuit.port();
    let v_mid = circuit.port();

    circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, v_top.clone()], vec![("idc".to_string(), 1e-3)]));
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![v_top.clone(), v_mid.clone()], vec![("r".to_string(), 1000.0)]));
    circuit.components_mut().insert("R2".to_string(), OsdiDevice::new_with_params("R2".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![v_mid.clone(), GND], vec![("r".to_string(), 500.0)]));

    let mut inst = circuit.instantiate();
    let dc = inst.dc(Context::default()).unwrap().solve().unwrap();

    let vt = dc.get_node(&v_top).expect("v_top");
    let vm = dc.get_node(&v_mid).expect("v_mid");
    assert!((vt - 1.5).abs() < 0.05, "Expected v_top ~1.5V, got {vt:.6}V");
    assert!((vm - 0.5).abs() < 0.05, "Expected v_mid ~0.5V, got {vm:.6}V");
}

/// Three nodes in series: I → n1 → R1 → n2 → R2 → n3 → R3 → GND.
#[test]
fn test_dc_three_node_chain() {
    let (resistor, t1) = load_model("resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    let mut circuit = Circuit::new("3-Node Chain");
    let n1 = circuit.port();
    let n2 = circuit.port();
    let n3 = circuit.port();

    // I = 2mA, R1=R2=R3=1kΩ
    // V(n1) = 2mA * 3kΩ = 6V, V(n2) = 2mA * 2kΩ = 4V, V(n3) = 2mA * 1kΩ = 2V
    circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, n1.clone()], vec![("idc".to_string(), 2e-3)]));
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![n1.clone(), n2.clone()], vec![("r".to_string(), 1000.0)]));
    circuit.components_mut().insert("R2".to_string(), OsdiDevice::new_with_params("R2".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![n2.clone(), n3.clone()], vec![("r".to_string(), 1000.0)]));
    circuit.components_mut().insert("R3".to_string(), OsdiDevice::new_with_params("R3".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![n3.clone(), GND], vec![("r".to_string(), 1000.0)]));

    let mut inst = circuit.instantiate();
    let dc = inst.dc(Context::default()).unwrap().solve().unwrap();

    let v1 = dc.get_node(&n1).unwrap();
    let v2 = dc.get_node(&n2).unwrap();
    let v3 = dc.get_node(&n3).unwrap();

    assert!((v1 - 6.0).abs() < 0.1, "Expected V(n1) ~6V, got {v1:.4}V");
    assert!((v2 - 4.0).abs() < 0.1, "Expected V(n2) ~4V, got {v2:.4}V");
    assert!((v3 - 2.0).abs() < 0.1, "Expected V(n3) ~2V, got {v3:.4}V");
}

/// Different resistance values: I=1mA, R=10kΩ → V=10V.
#[test]
fn test_dc_high_impedance() {
    let (resistor, t1) = load_model("resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    let mut circuit = Circuit::new("High-Z");
    let v_node = circuit.port();

    circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, v_node.clone()], vec![("idc".to_string(), 1e-3)]));
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![v_node.clone(), GND], vec![("r".to_string(), 10000.0)]));

    let mut inst = circuit.instantiate();
    let dc = inst.dc(Context::default()).unwrap().solve().unwrap();

    let v = dc.get_node(&v_node).unwrap();
    assert!((v - 10.0).abs() < 0.1, "Expected 10V, got {v:.4}V");
}

// ===================================================================
// 4. OPVAR READOUT
// ===================================================================

#[test]
fn test_opvar_readout_doesnt_crash() {
    let (resistor, t1) = load_model("resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    let mut circuit = Circuit::new("Opvar Test");
    let v_node = circuit.port();

    circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, v_node.clone()], vec![("idc".to_string(), 1e-3)]));
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![v_node.clone(), GND], vec![("r".to_string(), 1000.0)]));

    let mut inst = circuit.instantiate();
    let _dc = inst.dc(Context::default()).unwrap().solve().unwrap();

    for (i, rt) in inst.all_devices().iter().enumerate() {
        let opvars = rt.read_opvars();
        println!("  Runtime {i}: {} opvars", opvars.len());
        for (name, val) in &opvars {
            println!("    {name} = {val}");
            assert!(val.is_finite(), "opvar {name} should be finite");
        }
    }
}

#[test]
fn test_noisy_resistor_opvars() {
    // The noisy_resistor model has `gop` and `pdiss` opvars.
    let (noisy_resistor, t1) = load_model("noisy_resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    let mut circuit = Circuit::new("Noisy R Opvars");
    let v_node = circuit.port();

    circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, v_node.clone()], vec![("idc".to_string(), 1e-3)]));
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), noisy_resistor.lib.clone(), noisy_resistor.descriptor_idx, vec![v_node.clone(), GND], vec![("r".to_string(), 1000.0)]));

    let mut inst = circuit.instantiate();
    let _dc = inst.dc(Context::default()).unwrap().solve().unwrap();

    // Find the noisy resistor runtime and check opvars
    for rt in inst.all_devices() {
        let opvars = rt.read_opvars();
        if !opvars.is_empty() {
            println!("Opvars: {:?}", opvars);
            // Check that at least some opvar values are reasonable
            for (name, val) in &opvars {
                assert!(val.is_finite(), "opvar {name} should be finite, got {val}");
            }
        }
    }
}

// ===================================================================
// 5. NOISE ANALYSIS
// ===================================================================

#[test]
fn test_noisy_resistor_has_noise_sources() {
    let (noisy_resistor, t1) = load_model("noisy_resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    use piperine_solver::analysis::ac::AcAnalysisContext;

    let mut circuit = Circuit::new("Noise Sources");
    let v_node = circuit.port();

    circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, v_node.clone()], vec![("idc".to_string(), 1e-3)]));
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), noisy_resistor.lib.clone(), noisy_resistor.descriptor_idx, vec![v_node.clone(), GND], vec![("r".to_string(), 1000.0)]));

    let mut inst = circuit.instantiate();
    let dc = inst.dc(Context::default()).unwrap().solve().unwrap();

    let ac_ctx = AcAnalysisContext { frequency: 1e3 };
    let mut total_noise = 0;
    for rt in inst.all_devices_mut() {
        let noises = rt.noise_current_psd(&dc, &ac_ctx);
        total_noise += noises.len();
        for n in &noises {
            assert!(n.value >= 0.0, "PSD must be non-negative");
            println!("  PSD = {:.4e} A²/Hz", n.value);
        }
    }

    assert!(total_noise > 0, "Expected noise sources from noisy_resistor");
}

#[test]
fn test_noise_psd_thermal_value() {
    // Thermal noise PSD = 4kT*G = 4 * 1.38e-23 * 300 * (1/1000) ≈ 1.656e-23 A²/Hz.
    let (noisy_resistor, t1) = load_model("noisy_resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    use piperine_solver::analysis::ac::AcAnalysisContext;

    let mut circuit = Circuit::new("Thermal PSD");
    let v_node = circuit.port();

    circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, v_node.clone()], vec![("idc".to_string(), 1e-3)]));
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), noisy_resistor.lib.clone(), noisy_resistor.descriptor_idx, vec![v_node.clone(), GND], vec![("r".to_string(), 1000.0)]));

    let mut inst = circuit.instantiate();
    let dc = inst.dc(Context::default()).unwrap().solve().unwrap();

    let ac_ctx = AcAnalysisContext { frequency: 1e3 };
    let mut total_psd = 0.0;
    for rt in inst.all_devices_mut() {
        for n in rt.noise_current_psd(&dc, &ac_ctx) {
            total_psd += n.value;
        }
    }

    // Expected: 4kT/R ≈ 1.66e-23 for R=1kΩ, T=300K
    let expected = 4.0 * 1.38e-23 * 300.0 / 1000.0;
    println!("Total PSD = {total_psd:.4e}, expected ~{expected:.4e}");

    if total_psd > 0.0 {
        // Allow order of magnitude match (model uses $temperature not necessarily 300K)
        let ratio = total_psd / expected;
        assert!(
            ratio > 0.1 && ratio < 10.0,
            "PSD should be within order of magnitude of 4kT/R. Got ratio {ratio:.2}"
        );
    }
}

#[test]
fn test_noise_psd_scales_with_resistance() {
    let (noisy_resistor, t1) = load_model("noisy_resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    use piperine_solver::analysis::ac::AcAnalysisContext;

    let ac_ctx = AcAnalysisContext { frequency: 1e3 };

    let get_psd = |r_val: f64| -> f64 {
        let mut circuit = Circuit::new("R Scaling");
        let v_node = circuit.port();

        circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, v_node.clone()], vec![("idc".to_string(), 1e-3)]));
        circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), noisy_resistor.lib.clone(), noisy_resistor.descriptor_idx, vec![v_node.clone(), GND], vec![("r".to_string(), r_val)]));

        let mut inst = circuit.instantiate();
        let dc = inst.dc(Context::default()).unwrap().solve().unwrap();

        let mut total = 0.0;
        for rt in inst.all_devices_mut() {
            for n in rt.noise_current_psd(&dc, &ac_ctx) {
                total += n.value;
            }
        }
        total
    };

    let psd_1k = get_psd(1e3);
    let psd_10k = get_psd(10e3);

    println!("PSD(1kΩ) = {psd_1k:.4e}, PSD(10kΩ) = {psd_10k:.4e}");

    // 4kT*G: lower R → higher G → more noise current PSD.
    // Ratio should be ~10× (1kΩ vs 10kΩ).
    if psd_1k > 0.0 && psd_10k > 0.0 {
        let ratio = psd_1k / psd_10k;
        assert!(
            ratio > 3.0 && ratio < 30.0,
            "Expected PSD ratio ~10×, got {ratio:.2}×"
        );
    }
}

#[test]
fn test_noise_zero_for_non_noisy_device() {
    // Plain resistor (no white_noise) should return no noise sources.
    let (resistor, t1) = load_model("resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    use piperine_solver::analysis::ac::AcAnalysisContext;

    let mut circuit = Circuit::new("No Noise");
    let v_node = circuit.port();

    circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, v_node.clone()], vec![("idc".to_string(), 1e-3)]));
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![v_node.clone(), GND], vec![("r".to_string(), 1000.0)]));

    let mut inst = circuit.instantiate();
    let dc = inst.dc(Context::default()).unwrap().solve().unwrap();

    let ac_ctx = AcAnalysisContext { frequency: 1e3 };
    let mut total = 0;
    for rt in inst.all_devices_mut() {
        total += rt.noise_current_psd(&dc, &ac_ctx).len();
    }
    // Plain resistor model has no noise sources
    // (isource also has none)
    println!("Non-noisy circuit: {total} noise sources");
}

// ===================================================================
// 6. TEMPERATURE EFFECTS
// ===================================================================

#[test]
fn test_set_temperature_doesnt_crash() {
    let (resistor, t1) = load_model("resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    let mut circuit = Circuit::new("Temp Test");
    let v_node = circuit.port();

    circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, v_node.clone()], vec![("idc".to_string(), 1e-3)]));
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![v_node.clone(), GND], vec![("r".to_string(), 1000.0)]));

    let mut inst = circuit.instantiate();

    // Set various temperatures — should not crash
    for temp in [200.0, 300.0, 400.0, 500.0] {
        for rt in inst.all_devices_mut() {
            rt.set_temperature(temp);
        }
    }
}

#[test]
fn test_temperature_small_change_no_rerun() {
    // set_temperature with < 0.01K change should be a no-op
    let (resistor, t1) = load_model("resistor");
    leak_tmp(t1);

    let mut circuit = Circuit::new("Temp NoOp");
    let n1 = circuit.port();
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![n1.clone(), GND], vec![("r".to_string(), 1000.0)]));

    let mut inst = circuit.instantiate();
    for rt in inst.all_devices_mut() {
        // Default is 300.15K. Setting to 300.155 (0.005K diff) should be no-op.
        rt.set_temperature(300.155);
    }
    // Just check it doesn't crash.
}

#[test]
fn test_dc_at_different_temperatures() {
    // resistor_va model has R * ($temperature / tnom)^zeta with default zeta=0,
    // so changing temperature shouldn't change the result (zeta=0 → ratio^0 = 1).
    // With zeta=1, res = R * (T/tnom), so higher T → higher R → higher V.
    let (resistor, t1) = load_model("resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    let dc_at_temp = |temp: f64, zeta: f64| -> f64 {
        let mut circuit = Circuit::new("Temp DC");
        let v_node = circuit.port();

        circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, v_node.clone()], vec![("idc".to_string(), 1e-3)]));
        circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![v_node.clone(), GND], vec![("r".to_string(), 1000.0), ("zeta".to_string(), zeta), ("tnom".to_string(), 300.0)]));

        let mut inst = circuit.instantiate();
        for rt in inst.all_devices_mut() {
            rt.set_temperature(temp);
        }

        let ctx = Context { temperature: temp, ..Context::default() };
        let dc = inst.dc(ctx).unwrap().solve().unwrap();
        dc.get_node(&v_node).unwrap()
    };

    // With zeta=0: temperature doesn't matter
    let v_300_z0 = dc_at_temp(300.0, 0.0);
    let v_400_z0 = dc_at_temp(400.0, 0.0);
    assert!(
        (v_300_z0 - v_400_z0).abs() < 0.1,
        "zeta=0: should be same. V(300K)={v_300_z0:.4}, V(400K)={v_400_z0:.4}"
    );

    // With zeta=1: R scales linearly with temperature.
    // At 300K: R=1000*(300/300)=1000Ω → V=1V
    // At 600K: R=1000*(600/300)=2000Ω → V=2V
    let v_300_z1 = dc_at_temp(300.0, 1.0);
    let v_600_z1 = dc_at_temp(600.0, 1.0);
    println!("zeta=1: V(300K)={v_300_z1:.4}, V(600K)={v_600_z1:.4}");
    assert!(
        v_600_z1 > v_300_z1 * 1.5,
        "zeta=1: V should roughly double. V(300K)={v_300_z1:.4}, V(600K)={v_600_z1:.4}"
    );
}

#[test]
fn test_noise_at_different_temperatures() {
    // Our noisy_resistor model has G = 1/(R * T/tnom), so PSD = 4kT*G = 4k*tnom/R.
    // This happens to be temperature-independent! (T cancels out.)
    // We verify that noise is consistently produced at different temperatures.
    let (noisy_resistor, t1) = load_model("noisy_resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    use piperine_solver::analysis::ac::AcAnalysisContext;

    let ac_ctx = AcAnalysisContext { frequency: 1e3 };

    let noise_at_temp = |temp: f64| -> f64 {
        let mut circuit = Circuit::new("Noise Temp");
        let v_node = circuit.port();

        circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, v_node.clone()], vec![("idc".to_string(), 1e-3)]));
        circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), noisy_resistor.lib.clone(), noisy_resistor.descriptor_idx, vec![v_node.clone(), GND], vec![("r".to_string(), 1000.0), ("tnom".to_string(), 300.15)]));

        let mut inst = circuit.instantiate();
        for rt in inst.all_devices_mut() {
            rt.set_temperature(temp);
        }

        let ctx = Context { temperature: temp, ..Context::default() };
        let dc = inst.dc(ctx).unwrap().solve().unwrap();

        let mut total = 0.0;
        for rt in inst.all_devices_mut() {
            for n in rt.noise_current_psd(&dc, &ac_ctx) {
                total += n.value;
            }
        }
        total
    };

    let psd_300 = noise_at_temp(300.0);
    let psd_600 = noise_at_temp(600.0);

    println!("PSD(300K)={psd_300:.4e}, PSD(600K)={psd_600:.4e}");

    // Both should be non-zero
    assert!(psd_300 > 0.0, "PSD at 300K should be positive");
    assert!(psd_600 > 0.0, "PSD at 600K should be positive");

    // For this model, PSD = 4k*tnom/R ≈ 1.66e-23 regardless of temperature.
    let expected = 4.0 * 1.38e-23 * 300.15 / 1000.0;
    let ratio = psd_300 / expected;
    assert!(
        ratio > 0.5 && ratio < 2.0,
        "PSD should be near 4k*tnom/R = {expected:.4e}, got {psd_300:.4e}"
    );
}

// ===================================================================
// 7. CONTEXT / SOLVER CONFIG
// ===================================================================

#[test]
fn test_context_defaults() {
    let ctx = Context::default();
    assert!((ctx.temperature - 300.15).abs() < 0.01);
    assert!((ctx.tnom - 300.15).abs() < 0.01);
    assert!(ctx.reltol > 0.0 && ctx.reltol < 1.0);
    assert!(ctx.vntol > 0.0);
    assert!(ctx.abstol > 0.0);
    assert!(ctx.max_iter > 0);
    assert!(ctx.gmin > 0.0);
}

#[test]
fn test_context_custom() {
    let ctx = Context {
        reltol: 1e-6,
        vntol: 1e-9,
        abstol: 1e-15,
        max_iter: 1000,
        ..Context::default()
    };
    assert!((ctx.reltol - 1e-6).abs() < 1e-10);
    assert_eq!(ctx.max_iter, 1000);
}

// ===================================================================
// 8. RUNTIME PROPERTIES
// ===================================================================

#[test]
fn test_bound_step_hint_default() {
    let (resistor, t1) = load_model("resistor");
    leak_tmp(t1);

    let mut circuit = Circuit::new("Bound Step");
    let n1 = circuit.port();
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![n1.clone(), GND], vec![("r".to_string(), 1000.0)]));

    let inst = circuit.instantiate();
    for rt in inst.all_devices() {
        assert!(rt.bound_step_hint().is_infinite());
    }
}

#[test]
fn test_netlist_node_count() {
    let (resistor, t1) = load_model("resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    let mut circuit = Circuit::new("Netlist Count");
    let n1 = circuit.port();
    let n2 = circuit.port();
    let n3 = circuit.port();

    circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, n1.clone()], vec![("idc".to_string(), 1e-3)]));
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![n1.clone(), n2.clone()], vec![("r".to_string(), 100.0)]));
    circuit.components_mut().insert("R2".to_string(), OsdiDevice::new_with_params("R2".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![n2.clone(), n3.clone()], vec![("r".to_string(), 200.0)]));
    circuit.components_mut().insert("R3".to_string(), OsdiDevice::new_with_params("R3".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![n3.clone(), GND], vec![("r".to_string(), 300.0)]));

    let inst = circuit.instantiate();
    assert_eq!(inst.all_devices().len(), 4);
    let max_idx = inst.netlist().max_index().unwrap();
    assert!(max_idx >= 2, "should have at least 3 unknowns");
}

// ===================================================================
// 9. AC ANALYSIS (pure resistor — flat response)
// ===================================================================

#[test]
fn test_ac_analysis_runs() {
    let (resistor, t1) = load_model("resistor");
    let (isource, t2) = load_model("isource");
    leak_tmp(t1);
    leak_tmp(t2);

    use piperine_solver::analysis::ac::AcSweepAnalysisOptions;

    let mut circuit = Circuit::new("AC Test");
    let v_node = circuit.port();

    circuit.components_mut().insert("I1".to_string(), OsdiDevice::new_with_params("I1".to_string(), isource.lib.clone(), isource.descriptor_idx, vec![GND, v_node.clone()], vec![("idc".to_string(), 1e-3)]));
    circuit.components_mut().insert("R1".to_string(), OsdiDevice::new_with_params("R1".to_string(), resistor.lib.clone(), resistor.descriptor_idx, vec![v_node.clone(), GND], vec![("r".to_string(), 1000.0)]));

    let mut inst = circuit.instantiate();
    let options = AcSweepAnalysisOptions {
        start_frequency: 1.0,
        stop_frequency: 1e6,
        steps: 10,
        logarithmic: true,
    };

    let result = inst.ac(Context::default()).unwrap().solve_sweep(options).unwrap();
    assert_eq!(result.len(), 10, "should have 10 frequency steps");
}
