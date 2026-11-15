//! The partials verifier over every function of a hand-written device
//! (SAI-15, Port AC 4): each function of the instance's shape is run at
//! generic points built from the byte length of each read cell, and every
//! declared partial must agree with its central difference. A target that
//! defines its own devices calls `verify_every_function` for each of them
//! (the guard `every_hand_written_device_is_in_a_partials_check` of
//! `abi_devices.rs` enforces it).

use piperine_abi::caller::Handle;
use piperine_abi::device::{Access, cell_lens};
use piperine_abi::raw::Module;
use piperine_abi::verify::{Verdict, verify_partials};

/// One read cell's bytes at a point: an `F64` (8 bytes) is `real`; a byte is
/// 1 (a `CHANGED`, a flag, an age); a `BIT` is `bit`; a 16-byte cell is a
/// `REAL` (`real`, known); any other layout is zero bytes.
fn read_bytes(len: usize, real: f64, bit: [u8; 2]) -> Vec<u8> {
    match len {
        8 => real.to_le_bytes().to_vec(),
        1 => vec![1],
        2 => bit.to_vec(),
        16 => {
            let mut b = vec![0u8; 16];
            b[..8].copy_from_slice(&real.to_le_bytes());
            b[8] = 1;
            b
        }
        n => vec![0u8; n],
    }
}

/// Two points with positive, moderate, distinct reals (a param read as a
/// divisor or under a logarithm stays meaningful) and both `BIT` values.
fn points(lens: &[usize]) -> [Vec<Vec<u8>>; 2] {
    let a = lens.iter().enumerate().map(|(j, len)| read_bytes(*len, 0.3 + 0.13 * j as f64, [1, 1])).collect();
    let b = lens.iter().enumerate().map(|(j, len)| read_bytes(*len, 0.05 + 0.07 * j as f64, [0, 1])).collect();
    [a, b]
}

/// A function that is a fixture of a failure path: what it does at every point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fixture {
    /// It writes a non-finite value (the finite difference is non-finite).
    NonFinite,
    /// Its limit is active at every point (§8: nothing to compare).
    AlwaysLimited,
    /// It breaks purity (F1) by design, so its finite difference disagrees.
    Impure,
}

/// Every function of `module` (created with its defaults) passes the partials
/// verifier at both generic points: no mismatch, no fault, and at least one
/// point where every limit it writes is inactive (`Exact`). A module whose
/// prototype has no function has nothing to verify and is not created.
pub fn verify_every_function(name: &str, module: &'static Module) {
    verify(name, module, &[], &[]);
}

/// As `verify_every_function`, with read `read` of function `k` held at
/// `value` at both points (`(k, read, value)`): a read the fixture guards
/// (a time window, a threshold) stays on the side the circuit uses.
pub fn verify_every_function_with(name: &str, module: &'static Module, fixed: &[(u32, usize, f64)]) {
    verify(name, module, fixed, &[]);
}

/// As `verify_every_function`, but the functions in `fixtures` are fixtures
/// of a failure path and must behave as declared at both points.
pub fn verify_every_function_except(name: &str, module: &'static Module, fixtures: &[(u32, Fixture)]) {
    verify(name, module, &[], fixtures);
}

fn verify(name: &str, module: &'static Module, fixed: &[(u32, usize, f64)], fixtures: &[(u32, Fixture)]) {
    if module.prototype().functions.is_empty() {
        return;
    }
    let handle = Handle::create(module, &[]).unwrap_or_else(|e| panic!("create {name}: {e}"));
    let shape = handle.shape();
    for (k, f) in shape.functions.iter().enumerate() {
        let k = k as u32;
        let lens = cell_lens(shape, &f.reads, Access::FunctionRead)
            .unwrap_or_else(|| panic!("{name} fn {k}: a read cell breaks a shape rule"));
        let fixture = fixtures.iter().find(|(f, _)| *f == k).map(|(_, x)| *x);
        let mut exact = false;
        for (i, mut point) in points(&lens).into_iter().enumerate() {
            for (_, read, value) in fixed.iter().filter(|(f, _, _)| *f == k) {
                point[*read] = read_bytes(lens[*read], *value, [1, 1]);
            }
            let verdict = verify_partials(&handle, k, &point, 1.0e-6);
            match (fixture, verdict) {
                (Some(Fixture::NonFinite), Ok(Verdict::Mismatch(m))) if m.iter().all(|m| !m.finite.is_finite()) => {}
                (Some(Fixture::AlwaysLimited), Ok(Verdict::Skipped { .. })) => {}
                (Some(Fixture::Impure), Ok(Verdict::Mismatch(_))) => {}
                (Some(x), v) => panic!("{name} fn {k} point {i}: the {x:?} fixture gave {v:?}"),
                (None, Err(e)) => panic!("{name} fn {k} point {i}: the function faulted: {e}"),
                (None, Ok(Verdict::Exact)) => exact = true,
                (None, Ok(Verdict::Skipped { .. })) => {}
                (None, Ok(Verdict::Mismatch(m))) => panic!("{name} fn {k} point {i}: declared partials disagree: {m:?}"),
            }
        }
        assert!(exact || fixture.is_some(), "{name} fn {k}: no point with every limit inactive");
    }
}
