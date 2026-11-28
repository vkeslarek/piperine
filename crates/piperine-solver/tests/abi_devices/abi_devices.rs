//! The device libraries on `Device` (the `circuit` and `mixed` modules of the fixtures crate, SAI-15):
//! every function of every device passes the partials verifier at a plain table of operating points,
//! and every shape passes `validate`. The generic points are the fixtures crate's own test
//! (`its_devices_pass_the_partials_verifier`); a guard here holds every hand-written device of the suite
//! to that crate.

use piperine_device_fixtures::circuit;
use piperine_device_fixtures::mixed;
use piperine_abi::caller::Handle;
use piperine_abi::raw::Module;
use piperine_abi::validate::Validator;
use piperine_abi::verify::Verdict;

#[test]
fn every_device_function_passes_the_partials_verifier() {
    for (name, module, k, points) in AbiDevicesCases::table() {
        let handle = Handle::create(module, &[]).unwrap_or_else(|e| panic!("create {name}: {e}"));
        for (i, point) in points.iter().enumerate() {
            let verdict = Verdict::of_partials(&handle, k, point, 1.0e-6)
                .unwrap_or_else(|e| panic!("{name} point {i}: the function faulted: {e}"));
            assert_eq!(verdict, Verdict::Exact, "{name} point {i}");
        }
    }
}

#[test]
fn every_device_shape_is_valid() {
    for (name, module, _, _) in AbiDevicesCases::table() {
        Validator::validate(module.prototype()).unwrap_or_else(|e| panic!("{name} prototype: {e}"));
        let handle = Handle::create(module, &[]).unwrap_or_else(|e| panic!("create {name}: {e}"));
        Validator::validate(handle.shape()).unwrap_or_else(|e| panic!("{name} instance: {e}"));
    }
}

/// HYG-16: every hand-written device of the suite lives in the fixtures crate, where
/// `its_devices_pass_the_partials_verifier` holds the whole list to a partials check, so no target of
/// this crate declares a `Device` of its own.
#[test]
fn every_hand_written_device_lives_in_the_fixtures_crate() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut own = Vec::new();
    for (path, text) in AbiDevicesCases::rust_files(&dir) {
        for name in AbiDevicesCases::device_impls(&text) {
            own.push(format!("{}: {name}", path.display()));
        }
    }
    assert!(own.is_empty(), "devices declared in a target instead of the fixtures crate:\n{}", own.join("\n"));
}

/// The helpers of the abi_devices tests.
pub struct AbiDevicesCases;

impl AbiDevicesCases {
    /// The Rust files under `directory`, with their text.
    fn rust_files(directory: &std::path::Path) -> Vec<(std::path::PathBuf, String)> {
        let mut found = Vec::new();
        for entry in std::fs::read_dir(directory).expect("read tests").map(|e| e.expect("entry").path()) {
            if entry.is_dir() {
                found.extend(AbiDevicesCases::rust_files(&entry));
            } else if entry.extension().is_some_and(|e| e == "rs") {
                found.push((entry.clone(), std::fs::read_to_string(&entry).expect("read target")));
            }
        }
        found
    }

    /// The bytes of each read cell when every read is one `F64`.
    fn reals(point: &[f64]) -> Vec<Vec<u8>> {
        point.iter().map(|v| v.to_le_bytes().to_vec()).collect()
    }

    /// A `BIT` read followed by `F64` reads.
    fn bit_then(bit: &[u8; 1], point: &[f64]) -> Vec<Vec<u8>> {
        std::iter::once(bit.to_vec()).chain(AbiDevicesCases::reals(point)).collect()
    }

    /// `F64` reads followed by a `BIT` read.
    fn reals_then(point: &[f64], bit: &[u8; 1]) -> Vec<Vec<u8>> {
        AbiDevicesCases::reals(point).into_iter().chain(std::iter::once(bit.to_vec())).collect()
    }

    /// `ROpt`'s third read is its `w`, an `F64` that is 1 mS when the point gives none.
    fn ropt_point(vp: f64, vn: f64, w: Option<f64>) -> Vec<Vec<u8>> {
        vec![vp.to_le_bytes().to_vec(), vn.to_le_bytes().to_vec(), w.unwrap_or(1.0e-3).to_le_bytes().to_vec()]
    }

    /// (name, module, function, operating points): reads in the order the shape lists them.
    fn table() -> Vec<(&'static str, &'static Module, u32, Vec<Vec<Vec<u8>>>)> {
        vec![
            ("R", circuit::Resistor::module(), 0, vec![AbiDevicesCases::reals(&[0.7, 0.2, 2.0e3]), AbiDevicesCases::reals(&[-1.0, 3.0, 1.0e3]), AbiDevicesCases::reals(&[5.0, 0.0, 47.0])]),
            (
                "ROpt",
                circuit::OptionalResistor::module(),
                0,
                vec![AbiDevicesCases::ropt_point(0.7, 0.2, None), AbiDevicesCases::ropt_point(0.7, 0.2, Some(4.0e-3)), AbiDevicesCases::ropt_point(-2.0, 1.0, Some(1.0e-6))],
            ),
            ("C", circuit::Capacitor::module(), 0, vec![AbiDevicesCases::reals(&[0.4, 0.1, 1.0e-6]), AbiDevicesCases::reals(&[-2.0, 1.0, 1.0e-9])]),
            (
                "D",
                circuit::Diode::module(),
                0,
                vec![AbiDevicesCases::reals(&[0.55, 0.0, 1.0e-14]), AbiDevicesCases::reals(&[0.1, 0.0, 1.0e-14]), AbiDevicesCases::reals(&[-0.3, 0.2, 1.0e-12])],
            ),
            // reads: p, n, f, dc, SOURCE_SCALE
            ("Vsrc", circuit::VoltageSource::module(), 0, vec![AbiDevicesCases::reals(&[3.0, 0.5, 0.01, 3.0, 1.0]), AbiDevicesCases::reals(&[0.0, 1.0, -0.2, 10.0, 0.25])]),
            // reads: p, n, f, amp, freq, TIME
            (
                "Vsine",
                circuit::SineSource::module(),
                0,
                vec![AbiDevicesCases::reals(&[1.0, 0.2, 0.003, 5.0, 1.0e3, 1.25e-4]), AbiDevicesCases::reals(&[0.0, 0.0, 0.0, 2.0, 50.0, 0.0])],
            ),
            // reads: p, n, f, TIME
            ("Vramp", circuit::RampSource::module(), 0, vec![AbiDevicesCases::reals(&[1.0, 0.0, 0.1, 1.0e-3]), AbiDevicesCases::reals(&[0.2, 0.2, 0.0, 0.0])]),
            // reads: p, n, f, l
            ("L", circuit::Inductor::module(), 0, vec![AbiDevicesCases::reals(&[0.3, 0.1, 0.02, 1.0e-3]), AbiDevicesCases::reals(&[-1.0, 2.0, -0.5, 2.0e-6])]),
            // reads: p, n, r, I0.VALUE
            ("Rn", circuit::NoisyResistor::module(), 0, vec![AbiDevicesCases::reals(&[0.7, 0.2, 2.0e3, 0.0]), AbiDevicesCases::reals(&[-1.0, 3.0, 47.0, 1.0e-6])]),
            // fn 0 "a" reads: I0.VALUE
            ("Pair a", circuit::NoisePair::module(), 0, vec![AbiDevicesCases::reals(&[0.0]), AbiDevicesCases::reals(&[2.0])]),
            // fn 1 "b" reads: I0.VALUE, k
            ("Pair b", circuit::NoisePair::module(), 1, vec![AbiDevicesCases::reals(&[0.0, -1.0]), AbiDevicesCases::reals(&[2.0, 0.5])]),
            // fn 1 "load" reads: I0.VALUE
            ("Iac load", circuit::AcCurrentSource::module(), 1, vec![AbiDevicesCases::reals(&[0.0]), AbiDevicesCases::reals(&[2.5])]),
            // abi_mixed.rs. reads: inp, inn
            ("Comparator", mixed::Comparator::module(), 0, vec![AbiDevicesCases::reals(&[0.7, 0.5]), AbiDevicesCases::reals(&[-1.0, 2.0])]),
            // reads: code (BIT), out, f, vref
            ("Dac", mixed::BitDigitalToAnalog::module(), 0, vec![AbiDevicesCases::bit_then(&[1], &[0.4, 0.01, 1.0]), AbiDevicesCases::bit_then(&[0], &[-0.2, 0.0, 3.3])]),
            // reads: clk_a, gnd, f, hi (BIT)
            ("ClockSrc", mixed::ClockSource::module(), 0, vec![AbiDevicesCases::reals_then(&[1.0, 0.0, 0.02], &[1]), AbiDevicesCases::reals_then(&[0.3, 0.1, -0.5], &[0])]),
            // reads: p
            ("Tracer", mixed::ConductanceTracer::module(), 0, vec![AbiDevicesCases::reals(&[0.7]), AbiDevicesCases::reals(&[-3.0])]),
            // reads: p, n, TIME (never armed in this target)
            ("FlakyG", mixed::FlakyConductance::module(), 0, vec![AbiDevicesCases::reals(&[0.7, 0.2, 1.0e-4]), AbiDevicesCases::reals(&[-1.0, 1.0, 0.0])]),
            // reads: p, cap
            ("Capped", mixed::Capped::module(), 0, vec![AbiDevicesCases::reals(&[0.7, 1.0e-9])]),
        ]
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
}
