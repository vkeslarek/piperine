//! The analog device library on `Device` (`helpers/abi_devices.rs`, SAI-15):
//! every function of every device passes the partials verifier at a plain
//! table of operating points, and every shape passes `validate`.

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;

use abi_devices::{capacitor, current_ac, diode, inductor, resistor, resistor_opt, vramp, vsine, vsrc};
use piperine_abi::caller::Handle;
use piperine_abi::raw::Module;
use piperine_abi::validate::validate;
use piperine_abi::verify::{Verdict, verify_partials};

/// The bytes of each read cell when every read is one `F64`.
fn reals(point: &[f64]) -> Vec<Vec<u8>> {
    point.iter().map(|v| v.to_le_bytes().to_vec()).collect()
}

/// `ROpt`'s third read is a `REAL`: the value and its known byte.
fn ropt_point(vp: f64, vn: f64, w: Option<f64>) -> Vec<Vec<u8>> {
    let mut real = vec![0u8; 16];
    if let Some(w) = w {
        real[..8].copy_from_slice(&w.to_le_bytes());
        real[8] = 1;
    }
    vec![vp.to_le_bytes().to_vec(), vn.to_le_bytes().to_vec(), real]
}

/// (name, module, function, operating points): reads in the order the shape lists them.
fn table() -> Vec<(&'static str, &'static Module, u32, Vec<Vec<Vec<u8>>>)> {
    vec![
        ("R", resistor(), 0, vec![reals(&[0.7, 0.2, 2.0e3]), reals(&[-1.0, 3.0, 1.0e3]), reals(&[5.0, 0.0, 47.0])]),
        (
            "ROpt",
            resistor_opt(),
            0,
            vec![ropt_point(0.7, 0.2, None), ropt_point(0.7, 0.2, Some(4.0e-3)), ropt_point(-2.0, 1.0, Some(1.0e-6))],
        ),
        ("C", capacitor(), 0, vec![reals(&[0.4, 0.1, 1.0e-6]), reals(&[-2.0, 1.0, 1.0e-9])]),
        (
            "D",
            diode(),
            0,
            vec![reals(&[0.55, 0.0, 1.0e-14]), reals(&[0.1, 0.0, 1.0e-14]), reals(&[-0.3, 0.2, 1.0e-12])],
        ),
        // reads: p, n, f, dc, SOURCE_SCALE
        ("Vsrc", vsrc(), 0, vec![reals(&[3.0, 0.5, 0.01, 3.0, 1.0]), reals(&[0.0, 1.0, -0.2, 10.0, 0.25])]),
        // reads: p, n, f, amp, freq, TIME
        (
            "Vsine",
            vsine(),
            0,
            vec![reals(&[1.0, 0.2, 0.003, 5.0, 1.0e3, 1.25e-4]), reals(&[0.0, 0.0, 0.0, 2.0, 50.0, 0.0])],
        ),
        // reads: p, n, f, TIME
        ("Vramp", vramp(), 0, vec![reals(&[1.0, 0.0, 0.1, 1.0e-3]), reals(&[0.2, 0.2, 0.0, 0.0])]),
        // reads: p, n, f, l
        ("L", inductor(), 0, vec![reals(&[0.3, 0.1, 0.02, 1.0e-3]), reals(&[-1.0, 2.0, -0.5, 2.0e-6])]),
        // fn 1 "load" reads: I0.VALUE
        ("Iac load", current_ac(), 1, vec![reals(&[0.0]), reals(&[2.5])]),
    ]
}

#[test]
fn every_device_function_passes_the_partials_verifier() {
    for (name, module, k, points) in table() {
        let handle = Handle::create(module, &[]).unwrap_or_else(|e| panic!("create {name}: {e}"));
        for (i, point) in points.iter().enumerate() {
            let verdict = verify_partials(&handle, k, point, 1.0e-6)
                .unwrap_or_else(|e| panic!("{name} point {i}: the function faulted: {e}"));
            assert_eq!(verdict, Verdict::Exact, "{name} point {i}");
        }
    }
}

#[test]
fn every_device_shape_is_valid() {
    for (name, module, _, _) in table() {
        validate(module.prototype()).unwrap_or_else(|e| panic!("{name} prototype: {e}"));
        let handle = Handle::create(module, &[]).unwrap_or_else(|e| panic!("create {name}: {e}"));
        validate(handle.shape()).unwrap_or_else(|e| panic!("{name} instance: {e}"));
    }
}
