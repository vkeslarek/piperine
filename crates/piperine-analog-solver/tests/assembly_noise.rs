//! Noise columns (feature `solver-abi-analog`, SAN-09, T8): each `NOISE`
//! input is one column through every term that reads it, with its density
//! `PSD`; one input on two terms is one correlated source and two inputs add
//! their densities (C15); a density that reads `FREQ` is evaluated per
//! frequency and never outside a noise sweep (A4).
//!
//! Numerically sensitive: the output density `Σ |H·b|²·PSD` is formed here
//! with this crate's sparse LU and compared with `4kTR`.

#[allow(dead_code)]
#[path = "support/circuit.rs"]
mod circuit;
#[allow(dead_code)]
#[path = "support/devices.rs"]
mod devices;
#[allow(dead_code)]
#[path = "support/system.rs"]
mod system;

use circuit::{at, f64_bytes, f64_of, instance, netlist, tagged};
use devices::{K_B, KF, T_NOISE, calls, modules};
use num_complex::Complex64;
use piperine_analog_solver::device::AnalogBinding::{Discrete, Ground};
use piperine_analog_solver::device::{AnalogPlan, Assembly, NoiseColumn, Stage};
use piperine_analog_solver::math::integration::TrBdf2Phase;
use piperine_analog_solver::math::linear::Stamp;
use system::{at_freq, at_rest, at_time, close, solve_complex};

#[test]
fn a_noise_input_is_one_column_with_its_psd() {
    let m = modules();
    let (net, n) = netlist(2, &[]);
    let r = 2.0e3;
    let res = instance("r1", m.noisy, vec![Discrete(0), at(&n[0]), at(&n[1])]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![res], &net).expect("bind"));
    asm.set_discrete(0, &f64_bytes(r));
    let cols = asm.noise_sources(&[0.3, 0.1], at_freq(1e3)).expect("noise");
    assert_eq!(cols.len(), 1);
    let c = &cols[0];
    assert_eq!(c.name, "r1.thermal");
    close(c.psd, 4.0 * K_B * T_NOISE / r, 1e-15, "4kT/R");
    let rows: Vec<(usize, Complex64)> = c.rows.iter().map(|(r, v)| (r.idx().expect("row"), *v)).collect();
    assert_eq!(rows, vec![(0, Complex64::new(1.0, 0.0)), (1, Complex64::new(-1.0, 0.0))]);
}

/// `Σ_k |v(p)|²·PSD_k` with `v = H·b_k` from the small-signal matrix.
fn output_density(asm: &mut Assembly, cols: &[NoiseColumn], n: usize, freq: f64) -> f64 {
    let (matrix, _) = asm.assemble_ac(&vec![0.0; n], at_freq(freq)).expect("assemble");
    cols.iter()
        .map(|c| {
            let rhs = c.rows.iter().map(|(r, v)| Stamp::Rhs(r.clone(), *v)).collect();
            let v = solve_complex(n, matrix.clone(), rhs);
            v[0].norm_sqr() * c.psd
        })
        .sum()
}

#[test]
fn one_noise_input_on_two_terms_is_correlated() {
    let m = modules();
    let (net, n) = netlist(1, &[]);
    let r = 1.0e3;
    for (shared, want) in [(1.0, 0.0), (0.0, 4.0 * K_B * T_NOISE * r)] {
        let s = instance("s1", m.split, vec![Discrete(0), Discrete(1), at(&n[0]), Ground]);
        let mut asm = Assembly::new(AnalogPlan::bind(vec![s], &net).expect("bind"));
        asm.set_discrete(0, &f64_bytes(r));
        asm.set_discrete(1, &f64_bytes(shared));
        let cols = asm.noise_sources(&[0.0], at_freq(1e3)).expect("noise");
        assert_eq!(cols.len(), 2, "one column per NOISE input");
        let density = output_density(&mut asm, &cols, 1, 1e3);
        if shared == 1.0 {
            assert_eq!(cols[0].rows[0].1, Complex64::default(), "+1 and −1 on one source cancel");
            assert_eq!(density, 0.0, "a shared source across the pair has no density");
        } else {
            // Each half: 4kT/(2R) through H = R, twice: 4kTR.
            close(density, want, 1e-12, "two sources add their densities");
        }
    }
}

#[test]
fn a_psd_that_reads_freq_is_evaluated_per_frequency() {
    let m = modules();
    let (net, n) = netlist(1, &[]);
    let fl = tagged("fl", m.flicker, 701, vec![Discrete(0), at(&n[0])]);
    let mut asm = Assembly::new(AnalogPlan::bind(vec![fl], &net).expect("bind"));
    asm.assemble_dc(&[0.2], at_rest()).expect("dc");
    let stage = Stage { phase: TrBdf2Phase::Trapezoidal, h: 1e-9, prev_h: 0.0 };
    asm.assemble_tran(&[0.2], at_time(1e-9), stage).expect("tran");
    asm.assemble_ac(&[0.2], at_freq(1e3)).expect("ac");
    assert!(calls(701).is_empty(), "KF/freq is not evaluated outside a noise sweep (at FREQ = 0 it is not finite)");

    for freq in [10.0, 100.0, 100.0] {
        let cols = asm.noise_sources(&[0.2], at_freq(freq)).expect("noise");
        close(cols[0].psd, KF / freq, 1e-15, "KF/f");
    }
    let c = calls(701);
    assert_eq!(c.len(), 2, "once per new frequency");
    assert_eq!((f64_of(&c[0].reads[0]), f64_of(&c[1].reads[0])), (10.0, 100.0));
}
