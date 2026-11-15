//! Small-signal assembly (feature `solver-abi-analog`, SAN-08, T7): at the
//! operating point each term adds its partials times 1 (`FLOW`), `jω`
//! (`DDT`) or `exp(−jω·td)` (`DELAY`) (C14), a `STIM` input adds
//! `−g·(RE + j·IM)` to the right side, and an RC low pass solved at one
//! frequency with this crate's sparse LU reads `1/√2` at `1/(2πRC)`.
//!
//! Numerically sensitive: each weight is computed here by hand.

#[allow(dead_code)]
#[path = "support/circuit.rs"]
mod circuit;
#[allow(dead_code)]
#[path = "support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "support/system.rs"]
mod system;

use std::f64::consts::PI;

use circuit::{at, f64_bytes, instance, netlist};
use devices::modules;
use num_complex::Complex64;
use piperine_analog_solver::device::AnalogBinding::{Discrete, Ground};
use piperine_analog_solver::device::{AnalogPlan, Assembly};
use system::{at_freq, dense_complex, size, solve_complex};

fn near(a: Complex64, b: Complex64, rel: f64, what: &str) {
    assert!((a - b).norm() <= rel * a.norm().max(b.norm()), "{what}: {a} vs {b}");
}

#[test]
fn small_signal_weights_each_operator() {
    let m = modules();
    let (net, n) = netlist(3, &[]);
    let t = instance("t3", m.trio, vec![Discrete(0), Discrete(1), Discrete(2), Discrete(3), at(&n[0]), at(&n[1]), at(&n[2])]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![t], &net).expect("bind"));
    let (a, b, c, td) = (2e-3, 3e-9, 4e-3, 2.5e-7);
    for (slot, v) in [(0, a), (1, b), (2, c), (3, td)] {
        asm.set_discrete(slot, &f64_bytes(v));
    }
    for freq in [1.0e3, 1.0e6] {
        let (matrix, rhs) = asm.assemble_ac(&[0.4, 0.5, 0.6], at_freq(freq)).expect("assemble");
        let (y, s) = dense_complex(&matrix, &rhs, 3);
        let w = 2.0 * PI * freq;
        near(y[0][0], Complex64::new(a, 0.0), 1e-15, "FLOW: 1·a");
        near(y[1][1], Complex64::new(0.0, w * b), 1e-15, "DDT: jω·b");
        near(y[2][2], Complex64::new((w * td).cos() * c, -(w * td).sin() * c), 1e-14, "DELAY: exp(−jω·td)·c");
        assert_eq!((y[0][1], y[0][2], y[1][2]), (Complex64::default(), Complex64::default(), Complex64::default()));
        assert!(s.iter().all(|v| *v == Complex64::default()), "no stimulus");
    }
}

#[test]
fn a_stim_input_is_the_right_side() {
    let m = modules();
    let (net, n) = netlist(1, &["v1.f"]);
    let v = instance("v1", m.ac_vsource, vec![Discrete(0), Discrete(1), Discrete(2), at(&n[0]), Ground, at(&n[1])]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![v], &net).expect("bind"));
    let (re, im) = (0.3, -0.4);
    for (slot, x) in [(0, 2.0), (1, re), (2, im)] {
        asm.set_discrete(slot, &f64_bytes(x));
    }
    let (_, rhs) = asm.assemble_ac(&[2.0, 0.0], at_freq(1e3)).expect("assemble");
    let (_, s) = dense_complex(&[], &rhs, 2);
    // g = ∂eq/∂I0 = −1, so the branch row's right side is −g·(RE + j·IM).
    let g = -1.0;
    assert_eq!(s[1], -(g * Complex64::new(re, im)));
    assert_eq!(s[0], Complex64::default(), "the eq term's minus side is ground");
}

#[test]
fn an_rc_low_pass_has_its_corner() {
    let m = modules();
    let (net, n) = netlist(2, &["v1.f"]);
    let (inp, out, f) = (&n[0], &n[1], &n[2]);
    let (r, c) = (1.0e3, 1.0e-6);
    let v = instance("v1", m.ac_vsource, vec![Discrete(0), Discrete(1), Discrete(2), at(inp), Ground, at(f)]);
    let r1 = instance("r1", m.resistor, vec![Discrete(3), at(inp), at(out)]);
    let c1 = instance("c1", m.capacitor, vec![Discrete(4), at(out), Ground]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![v, r1, c1], &net).expect("bind"));
    for (slot, x) in [(0, 0.0), (1, 1.0), (2, 0.0), (3, r), (4, c)] {
        asm.set_discrete(slot, &f64_bytes(x));
    }
    let corner = 1.0 / (2.0 * PI * r * c);
    let (matrix, rhs) = asm.assemble_ac(&vec![0.0; size(&net)], at_freq(corner)).expect("assemble");
    let x = solve_complex(size(&net), matrix, rhs);
    let h = x[out.idx().expect("out")];
    let want = 1.0 / 2.0_f64.sqrt();
    assert!((h.norm() - want).abs() <= 1e-9, "|H| = {} vs 1/√2", h.norm());
    assert!((h.arg() + PI / 4.0).abs() <= 1e-9, "arg H = {} vs −π/4", h.arg());
}
