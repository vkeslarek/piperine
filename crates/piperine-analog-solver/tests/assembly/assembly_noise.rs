//! Noise columns (feature `solver-abi-analog`, SAN-09, T8): each `NOISE`
//! input is one column through every term that reads it, with its density
//! `PSD`; one input on two terms is one correlated source and two inputs add
//! their densities (C15); a density that reads `FREQ` is evaluated per
//! frequency and never outside a noise sweep (A4).
//!
//! Numerically sensitive: the output density `Σ |H·b|²·PSD` is formed here
//! with this crate's sparse LU and compared with `4kTR`.

use piperine_device_fixtures::analog::{Call, Modules, T_NOISE};
use piperine_device_fixtures::assembly::Circuits;
use piperine_device_fixtures::assembly::{AssemblySolves};
use piperine_device_fixtures::examples::{KF, K_B};
use num_complex::Complex64;
use piperine_analog_solver::device::AnalogBinding::{Discrete, Ground};
use piperine_analog_solver::device::{AnalogPlan, Assembly, NoiseColumn, JumpPosition, Stage};
use piperine_analog_solver::math::integration::TrBdf2Phase;
use piperine_analog_solver::math::linear::Stamp;

#[test]
fn a_noise_input_is_one_column_with_its_psd() {
    let m = Modules::all();
    let (net, n) = Circuits::netlist(2, &[]);
    let r = 2.0e3;
    let res = Circuits::instance("r1", m.noisy, vec![Discrete(0), Circuits::at(&n[0]), Circuits::at(&n[1])]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![res], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(r));
    let cols = assembly.noise_sources(&[0.3, 0.1], AssemblySolves::at_frequency(1e3)).expect("noise");
    assert_eq!(cols.len(), 1);
    let c = &cols[0];
    assert_eq!(c.name, "r1.thermal");
    AssemblySolves::close(c.psd, 4.0 * K_B * T_NOISE / r, 1e-15, "4kT/R");
    let rows: Vec<(usize, Complex64)> = c.rows.iter().map(|(r, v)| (r.idx().expect("row"), *v)).collect();
    assert_eq!(rows, vec![(0, Complex64::new(1.0, 0.0)), (1, Complex64::new(-1.0, 0.0))]);
}

#[test]
fn one_noise_input_on_two_terms_is_correlated() {
    let m = Modules::all();
    let (net, n) = Circuits::netlist(1, &[]);
    let r = 1.0e3;
    for (shared, want) in [(1.0, 0.0), (0.0, 4.0 * K_B * T_NOISE * r)] {
        let s = Circuits::instance("s1", m.split, vec![Discrete(0), Discrete(1), Circuits::at(&n[0]), Ground]);
        let mut assembly = Assembly::new(AnalogPlan::bind(vec![s], &net).expect("bind"));
        assembly.set_discrete(0, &Circuits::real_bytes(r));
        assembly.set_discrete(1, &Circuits::real_bytes(shared));
        let cols = assembly.noise_sources(&[0.0], AssemblySolves::at_frequency(1e3)).expect("noise");
        assert_eq!(cols.len(), 2, "one column per NOISE input");
        let density = NoiseCircuits::output_density(&mut assembly, &cols, 1, 1e3);
        if shared == 1.0 {
            assert_eq!(cols[0].rows[0].1, Complex64::default(), "+1 and −1 on one source cancel");
            assert_eq!(density, 0.0, "a shared source across the pair has no density");
        } else {
            // Each half: 4kT/(2R) through H = R, twice: 4kTR.
            AssemblySolves::close(density, want, 1e-12, "two sources add their densities");
        }
    }
}

#[test]
fn a_psd_that_reads_freq_is_evaluated_per_frequency() {
    let m = Modules::all();
    let (net, n) = Circuits::netlist(1, &[]);
    let fl = Circuits::tagged("fl", m.flicker, 701, vec![Discrete(0), Circuits::at(&n[0])]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![fl], &net).expect("bind"));
    assembly.assemble_dc(&[0.2], AssemblySolves::at_rest()).expect("dc");
    let stage = Stage { phase: TrBdf2Phase::Trapezoidal, h: 1e-9, prev_h: 0.0, jump: JumpPosition::None };
    assembly.assemble_tran(&[0.2], AssemblySolves::at_time(1e-9), stage).expect("tran");
    assembly.assemble_ac(&[0.2], AssemblySolves::at_frequency(1e3)).expect("ac");
    assert!(Call::recorded(701).is_empty(), "KF/freq is not evaluated outside a noise sweep (at FREQ = 0 it is not finite)");

    for freq in [10.0, 100.0, 100.0] {
        let cols = assembly.noise_sources(&[0.2], AssemblySolves::at_frequency(freq)).expect("noise");
        AssemblySolves::close(cols[0].psd, KF / freq, 1e-15, "KF/f");
    }
    let c = Call::recorded(701);
    assert_eq!(c.len(), 2, "once per new frequency");
    assert_eq!((Circuits::real_of(&c[0].reads[0]), Circuits::real_of(&c[1].reads[0])), (10.0, 100.0));
}

/// The circuits the noise tests build.
pub struct NoiseCircuits;

impl NoiseCircuits {
    /// `Σ_k |v(p)|²·PSD_k` with `v = H·b_k` from the small-signal matrix.
    fn output_density(assembly: &mut Assembly, cols: &[NoiseColumn], n: usize, freq: f64) -> f64 {
        let (matrix, _) = assembly.assemble_ac(&vec![0.0; n], AssemblySolves::at_frequency(freq)).expect("assemble");
        cols.iter()
            .map(|c| {
                let rhs = c.rows.iter().map(|(r, v)| Stamp::Rhs(r.clone(), *v)).collect();
                let v = AssemblySolves::solve_complex(n, matrix.clone(), rhs);
                v[0].norm_sqr() * c.psd
            })
            .sum()
    }
}
