//! The numerics the noise analysis rests on (feature `analog-operators`, AO-05): the power-law
//! integral of a noise density over a band, ngspice's quadrature, against closed forms.

use piperine_analog_solver::math::integration::Integrator;

/// `count` frequencies from `start` to `stop`, geometrically spaced.
fn geometric_grid(start: f64, stop: f64, count: usize) -> Vec<f64> {
    let step = (stop / start).powf(1.0 / (count - 1) as f64);
    (0..count).map(|i| start * step.powi(i as i32)).collect()
}

/// Asserts that `value` equals `expected` within the relative `tolerance`.
fn close(value: f64, expected: f64, tolerance: f64, what: &str) {
    assert!((value - expected).abs() <= tolerance * expected.abs(), "{what}: {value:e} against {expected:e}");
}

#[test]
fn the_power_law_integral_is_exact_on_power_laws() {
    // Only 3 points: a power law between points is exact on any power-law density.
    let frequencies = geometric_grid(1.0e3, 1.0e6, 3);
    let (f_first, f_last) = (frequencies[0], frequencies[2]);

    // N = c: ∫ c df = c·(f₂ − f₁).
    let c = 4.0e-18;
    let flat: Vec<f64> = frequencies.iter().map(|_| c).collect();
    close(Integrator::power_law(&frequencies, &flat), c * (f_last - f_first), 1e-12, "N = c");

    // N = c/f: ∫ c/f df = c·ln(f₂/f₁), the a = −1 case.
    let flicker: Vec<f64> = frequencies.iter().map(|f| c / f).collect();
    close(Integrator::power_law(&frequencies, &flicker), c * (f_last / f_first).ln(), 1e-12, "N = c/f");

    // N = c·f⁻²: ∫ c·f⁻² df = c·(1/f₁ − 1/f₂).
    let steep: Vec<f64> = frequencies.iter().map(|f| c / (f * f)).collect();
    close(Integrator::power_law(&frequencies, &steep), c * (1.0 / f_first - 1.0 / f_last), 1e-12, "N = c·f⁻²");

    // N = c·f: ∫ c·f df = c/2·(f₂² − f₁²), a rising power law.
    let rising: Vec<f64> = frequencies.iter().map(|f| c * f).collect();
    close(Integrator::power_law(&frequencies, &rising), 0.5 * c * (f_last * f_last - f_first * f_first), 1e-12, "N = c·f");
}

#[test]
fn the_power_law_integral_tracks_a_two_pole_sum_against_a_fine_trapezoid() {
    // N = 1/(1 + (f/f₁)²) + 4/(1 + (f/f₂)²): two poles, 1 kHz and 100 kHz.
    let density = |f: f64| 1.0 / (1.0 + (f / 1.0e3).powi(2)) + 4.0 / (1.0 + (f / 1.0e5).powi(2));
    let sparse = geometric_grid(10.0, 1.0e7, 601);
    let fine = geometric_grid(10.0, 1.0e7, 600_001);
    let sparse_density: Vec<f64> = sparse.iter().map(|f| density(*f)).collect();
    let fine_density: Vec<f64> = fine.iter().map(|f| density(*f)).collect();
    // 100 points per decade: the power law follows the poles' slope, which the trapezoid at
    // the same grid does not (it overestimates a convex density).
    close(
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
