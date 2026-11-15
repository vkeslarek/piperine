//! The device libraries on `Device` (`helpers/abi_devices.rs`,
//! `helpers/abi_mixed.rs`, SAI-15): every function of every device passes the
//! partials verifier at a plain table of operating points and at the generic
//! points of `helpers/partials.rs`, and every shape passes `validate`. A guard
//! holds every hand-written device of the suite, in a helper or in a target,
//! to a partials check.

// The target uses the library selectively.
#[allow(dead_code)]
#[path = "helpers/abi_devices.rs"]
mod abi_devices;

#[allow(dead_code)]
#[path = "helpers/abi_mixed.rs"]
mod abi_mixed;

#[allow(dead_code)]
#[path = "helpers/partials.rs"]
mod partials;

use abi_devices::{
    capacitor, current_ac, diode, inductor, noise_pair, resistor, resistor_noisy, resistor_opt, vramp, vsine, vsrc,
};
use abi_mixed::{
    capped, clock_src, comparator, counter, dac, delay, dff, finisher, flaky_g, globals, inverter, recorder, sampler,
    settle_count, settle_time, stub, timer, toggle_dff, tracer,
};
use partials::verify_every_function;
use piperine_abi::caller::Handle;
use piperine_abi::raw::Module;
use piperine_abi::validate::validate;
use piperine_abi::verify::{Verdict, verify_partials};

/// The bytes of each read cell when every read is one `F64`.
fn reals(point: &[f64]) -> Vec<Vec<u8>> {
    point.iter().map(|v| v.to_le_bytes().to_vec()).collect()
}

/// A `BIT` read followed by `F64` reads.
fn bit_then(bit: &[u8; 2], point: &[f64]) -> Vec<Vec<u8>> {
    std::iter::once(bit.to_vec()).chain(reals(point)).collect()
}

/// `F64` reads followed by a `BIT` read.
fn reals_then(point: &[f64], bit: &[u8; 2]) -> Vec<Vec<u8>> {
    reals(point).into_iter().chain(std::iter::once(bit.to_vec())).collect()
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
        // reads: p, n, r, I0.VALUE
        ("Rn", resistor_noisy(), 0, vec![reals(&[0.7, 0.2, 2.0e3, 0.0]), reals(&[-1.0, 3.0, 47.0, 1.0e-6])]),
        // fn 0 "a" reads: I0.VALUE
        ("Pair a", noise_pair(), 0, vec![reals(&[0.0]), reals(&[2.0])]),
        // fn 1 "b" reads: I0.VALUE, k
        ("Pair b", noise_pair(), 1, vec![reals(&[0.0, -1.0]), reals(&[2.0, 0.5])]),
        // fn 1 "load" reads: I0.VALUE
        ("Iac load", current_ac(), 1, vec![reals(&[0.0]), reals(&[2.5])]),
        // abi_mixed.rs. reads: inp, inn
        ("Comparator", comparator(), 0, vec![reals(&[0.7, 0.5]), reals(&[-1.0, 2.0])]),
        // reads: code (BIT), out, f, vref
        ("Dac", dac(), 0, vec![bit_then(&[1, 1], &[0.4, 0.01, 1.0]), bit_then(&[0, 1], &[-0.2, 0.0, 3.3])]),
        // reads: clk_a, gnd, f, hi (BIT)
        ("ClockSrc", clock_src(), 0, vec![reals_then(&[1.0, 0.0, 0.02], &[1, 1]), reals_then(&[0.3, 0.1, -0.5], &[0, 1])]),
        // reads: p
        ("Tracer", tracer(), 0, vec![reals(&[0.7]), reals(&[-3.0])]),
        // reads: p, n, TIME (never armed in this target)
        ("FlakyG", flaky_g(), 0, vec![reals(&[0.7, 0.2, 1.0e-4]), reals(&[-1.0, 1.0, 0.0])]),
        // reads: p, cap
        ("Capped", capped(), 0, vec![reals(&[0.7, 1.0e-9])]),
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

#[test]
fn every_helper_device_passes_at_generic_points() {
    let helpers: [(&str, &'static Module); 30] = [
        ("R", resistor()),
        ("ROpt", resistor_opt()),
        ("C", capacitor()),
        ("D", diode()),
        ("Vsrc", vsrc()),
        ("Vsine", vsine()),
        ("Vramp", vramp()),
        ("Iac", current_ac()),
        ("L", inductor()),
        ("Rn", resistor_noisy()),
        ("Pair", noise_pair()),
        ("Comparator", comparator()),
        ("Dff", dff()),
        ("Inverter", inverter()),
        ("Dac", dac()),
        ("SettleCount", settle_count()),
        ("Globals", globals()),
        ("Timer", timer()),
        ("Recorder", recorder()),
        ("Sampler", sampler()),
        ("Stub", stub()),
        ("Counter", counter()),
        ("SettleTime", settle_time()),
        ("Delay", delay()),
        ("Finisher", finisher()),
        ("Tracer", tracer()),
        ("FlakyG", flaky_g()),
        ("Capped", capped()),
        ("ClockSrc", clock_src()),
        ("ToggleDff", toggle_dff()),
    ];
    for (name, module) in helpers {
        verify_every_function(name, module);
    }
}

/// The names of the devices `text` implements `Device` for.
fn device_impls(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines().map(str::trim_start) {
        let Some(rest) = line.strip_prefix("impl") else { continue };
        let rest = if rest.starts_with('<') { rest.split_once('>').map_or("", |(_, r)| r) } else { rest };
        if let Some(name) = rest.trim_start().strip_prefix("Device for ") {
            let name: String = name.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            names.push(name);
        }
    }
    names
}

#[test]
fn every_hand_written_device_is_in_a_partials_check() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let this = std::fs::read_to_string(dir.join("abi_devices.rs")).expect("read abi_devices.rs");
    // A check names its device as the first argument of a `verify_every_function*`
    // call (the helpers' devices may also be a row of `table()`).
    let checked = |text: &str, name: &str| {
        let called = text.match_indices("verify_every_function").any(|(at, _)| {
            let rest = &text[at..];
            let args = rest.split_once('(').map_or("", |(_, a)| a).trim_start();
            args.strip_prefix('"').and_then(|a| a.split_once('"')).is_some_and(|(n, _)| n == name)
        });
        called || (std::ptr::eq(text, this.as_str()) && text.contains(&format!("(\"{name}\", ")))
    };
    let mut missing = Vec::new();
    let mut seen = 0;
    for sub in ["", "helpers"] {
        for entry in std::fs::read_dir(dir.join(sub)).expect("read tests") {
            let path = entry.expect("entry").path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read target");
            // A helper's devices are checked here; a target's, in the target.
            let home = if sub == "helpers" { &this } else { &text };
            for name in device_impls(&text) {
                seen += 1;
                if !checked(home, &name) {
                    missing.push(format!("{}: {name}", path.display()));
                }
            }
        }
    }
    assert!(seen > 50, "the scan found the suite's devices ({seen})");
    assert!(missing.is_empty(), "devices no partials check names:\n{}", missing.join("\n"));
}
