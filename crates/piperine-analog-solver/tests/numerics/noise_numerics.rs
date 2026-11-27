//! The numerics the noise analysis rests on (feature `analog-operators`, AO-05): the power-law
//! integral of a noise density over a band, ngspice's quadrature, against closed forms.

use piperine_device_fixtures::analog::{Modules};
use piperine_device_fixtures::assembly::Circuits;
use piperine_device_fixtures::assembly::{AssemblySolves};
use piperine_analog_solver::math::integration::Integrator;

/// The frequencies a noise sweep reads.
struct Band;

impl Band {
    /// `count` frequencies from `start` to `stop`, geometrically spaced.
    fn geometric(start: f64, stop: f64, count: usize) -> Vec<f64> {
        let step = (stop / start).powf(1.0 / (count - 1) as f64);
        (0..count).map(|index| start * step.powi(index as i32)).collect()
    }
}

/// How near a computed value must be to its closed form.
struct Closeness;

impl Closeness {
    /// Asserts that `value` equals `expected` within the relative `tolerance`.
    fn assert(value: f64, expected: f64, tolerance: f64, what: &str) {
        assert!((value - expected).abs() <= tolerance * expected.abs(), "{what}: {value:e} against {expected:e}");
    }
}

#[test]
fn the_power_law_integral_is_exact_on_power_laws() {
    // Only 3 points: a power law between points is exact on any power-law density.
    let frequencies = Band::geometric(1.0e3, 1.0e6, 3);
    let (first, last) = (frequencies[0], frequencies[2]);

    // N = c: ∫ c df = c·(f₂ − f₁).
    let c = 4.0e-18;
    let flat: Vec<f64> = frequencies.iter().map(|_| c).collect();
    Closeness::assert(Integrator::power_law(&frequencies, &flat), c * (last - first), 1e-12, "N = c");

    // N = c/f: ∫ c/f df = c·ln(f₂/f₁), the a = −1 case.
    let flicker: Vec<f64> = frequencies.iter().map(|f| c / f).collect();
    Closeness::assert(Integrator::power_law(&frequencies, &flicker), c * (last / first).ln(), 1e-12, "N = c/f");

    // N = c·f⁻²: ∫ c·f⁻² df = c·(1/f₁ − 1/f₂).
    let steep: Vec<f64> = frequencies.iter().map(|f| c / (f * f)).collect();
    Closeness::assert(Integrator::power_law(&frequencies, &steep), c * (1.0 / first - 1.0 / last), 1e-12, "N = c·f⁻²");

    // N = c·f: ∫ c·f df = c/2·(f₂² − f₁²), a rising power law.
    let rising: Vec<f64> = frequencies.iter().map(|f| c * f).collect();
    Closeness::assert(Integrator::power_law(&frequencies, &rising), 0.5 * c * (last * last - first * first), 1e-12, "N = c·f");
}

#[test]
fn the_power_law_integral_tracks_a_two_pole_sum_against_a_fine_trapezoid() {
    // N = 1/(1 + (f/f₁)²) + 4/(1 + (f/f₂)²): two poles, 1 kHz and 100 kHz.
    let density = |f: f64| 1.0 / (1.0 + (f / 1.0e3).powi(2)) + 4.0 / (1.0 + (f / 1.0e5).powi(2));
    let sparse = Band::geometric(10.0, 1.0e7, 601);
    let fine = Band::geometric(10.0, 1.0e7, 600_001);
    let sparse_density: Vec<f64> = sparse.iter().map(|f| density(*f)).collect();
    let fine_density: Vec<f64> = fine.iter().map(|f| density(*f)).collect();
    // 100 points per decade: the power law follows the poles' slope, which the trapezoid at
    // the same grid does not (it overestimates a convex density).
    Closeness::assert(
        Integrator::power_law(&sparse, &sparse_density),
        Integrator::trapezoid(&fine, &fine_density),
        1.0e-3,
        "two-pole sum",
    );
    let power_law_error =
        (Integrator::power_law(&sparse, &sparse_density) - Integrator::trapezoid(&fine, &fine_density)).abs();
    let trapezoid_error = (Integrator::trapezoid(&sparse, &sparse_density) - Integrator::trapezoid(&fine, &fine_density)).abs();
    assert!(power_law_error < trapezoid_error, "power law {power_law_error:e} against trapezoid {trapezoid_error:e}");
}

#[test]
fn a_zero_density_takes_the_plain_rectangle() {
    // D8: either density 0 has no logarithm; the interval is N₁·(f₂ − f₁).
    assert_eq!(Integrator::power_law(&[1.0, 3.0], &[2.0, 0.0]), 4.0);
    assert_eq!(Integrator::power_law(&[1.0, 3.0], &[0.0, 2.0]), 0.0);
    assert_eq!(Integrator::power_law(&[1.0], &[2.0]), 0.0);
}

// ─── The assembly's reads for the noise sweep ───────────────────────────────

use num_complex::Complex64;
use piperine_analog_solver::device::AnalogBinding::{Discrete, Ground};
use piperine_analog_solver::device::{AnalogPlan, Assembly};

#[test]
fn a_stimulus_column_carries_its_partials_and_phase() {
    // An `AcVSource` (dc 0, re 1, im 1) from node 0 to ground: its `STIM` input "ac" enters the
    // branch row `f` of `eq = v(p) − v(n) − dc − I0` with partial −1, at unit magnitude, so the
    // column is one row (the branch unknown, 2) of −1, and the phasor 1 + j has phase π/4.
    let library = Modules::all();
    let (net, n) = Circuits::netlist(1, &["f"]);
    let source = Circuits::instance("top.v1", library.ac_voltage_source, vec![Discrete(0), Discrete(1), Discrete(2), Circuits::at(&n[0]), Ground, Circuits::at(&n[1])]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![source], &net).expect("bind"));
    for (slot, value) in [(0, 0.0), (1, 1.0), (2, 1.0)] {
        assembly.set_discrete(slot, &Circuits::real_bytes(value));
    }
    let columns = assembly.stimulus_columns(&[0.0, 0.0], AssemblySolves::at_frequency(1.0e3), "top.v1").expect("columns");
    assert_eq!(columns.len(), 1, "one column per STIM input");
    assert_eq!(columns[0].name, "top.v1.ac");
    assert!((columns[0].phase - std::f64::consts::FRAC_PI_4).abs() < 1e-15, "phase {}", columns[0].phase);
    let rows: Vec<(usize, Complex64)> = columns[0].rows.iter().map(|(r, v)| (r.idx().expect("row"), *v)).collect();
    assert_eq!(rows, vec![(1, Complex64::new(-1.0, 0.0))]);

    // An unknown instance is named.
    let message = assembly.stimulus_columns(&[0.0, 0.0], AssemblySolves::at_frequency(1.0e3), "top.nope").expect_err("no such instance").to_string();
    assert!(message.contains("top.nope"), "{message}");
}

#[test]
fn a_term_linearization_carries_its_unknown_and_noise_partials() {
    // `NoisyResistor` term `i = (v(p) − v(n))/r + I0` at r = 2 kΩ: ∂/∂v(p) = 1/r, ∂/∂v(n) = −1/r,
    // and a direct partial of 1 against its one NOISE input (position 0); the response of a
    // FLOW term is 1.
    let library = Modules::all();
    let (net, n) = Circuits::netlist(2, &[]);
    let r = 2.0e3;
    let res = Circuits::instance("r1", library.noisy, vec![Discrete(0), Circuits::at(&n[0]), Circuits::at(&n[1])]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![res], &net).expect("bind"));
    assembly.set_discrete(0, &Circuits::real_bytes(r));
    let linearization = assembly.term_linearization(&[0.3, 0.1], AssemblySolves::at_frequency(1.0e3), "r1", "i").expect("linearization");
    let unknowns: Vec<(usize, Complex64)> =
        linearization.unknowns.iter().map(|(c, g)| (c.idx().expect("unknown"), *g)).collect();
    assert_eq!(unknowns, vec![(0, Complex64::new(1.0 / r, 0.0)), (1, Complex64::new(-1.0 / r, 0.0))]);
    assert_eq!(linearization.noise, vec![(0, Complex64::new(1.0, 0.0))]);
    let message = assembly.term_linearization(&[0.3, 0.1], AssemblySolves::at_frequency(1.0e3), "r1", "nope").expect_err("no such term").to_string();
    assert!(message.contains("`r1`") && message.contains("`nope`"), "{message}");

    // The `DDT` term `q` of a `Trio` (b = 3, partial against p1) at 100 kHz carries the response
    // jω: ∂v/∂v(p1)·jω = 3·j·2π·1e5.
    let trio = Circuits::instance("t1", library.trio, vec![Discrete(0), Discrete(1), Discrete(2), Discrete(3), Circuits::at(&n[0]), Circuits::at(&n[1]), Ground]);
    let mut assembly = Assembly::new(AnalogPlan::bind(vec![trio], &net).expect("bind"));
    for (slot, value) in [(0, 1.0), (1, 3.0), (2, 1.0), (3, 0.0)] {
        assembly.set_discrete(slot, &Circuits::real_bytes(value));
    }
    let omega = 2.0 * std::f64::consts::PI * 1.0e5;
    let linearization = assembly.term_linearization(&[0.0, 0.0], AssemblySolves::at_frequency(1.0e5), "t1", "q").expect("linearization");
    assert_eq!(linearization.unknowns.len(), 1);
    assert_eq!(linearization.unknowns[0].0.idx(), Some(1));
    Closeness::assert(linearization.unknowns[0].1.im, 3.0 * omega, 1e-12, "jω·b");
    assert_eq!(linearization.unknowns[0].1.re, 0.0);
    assert!(linearization.noise.is_empty(), "no noise input on the term");
}
