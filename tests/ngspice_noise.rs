//! ngspice golden cross-validation of **output noise** (NGP-02).
//!
//! ⚠ The units differ, and that is the whole difficulty of this target.
//! ngspice's `onoise_spectrum` is an *amplitude* spectrum in **V/√Hz**;
//! piperine's `NoiseTrace::psd()` is a *power* spectral density in **V²/Hz**
//! (Part VII §12). Comparing them directly is wrong by a square and reads as a
//! broken noise model rather than a unit mismatch, so ngspice's value is
//! squared in exactly one place — [`NoiseCase::psd_from_spectrum`] — with the
//! derivation on it. `onoise_total` needs no such conversion: it and
//! `NoiseTrace::total()` are both an RMS in volts.
//!
//! The unit probe is what keeps that honest: a 1 kΩ resistor's spectrum must
//! equal the Johnson amplitude √(4kTR) ≈ 4.07e-9 V/√Hz. If a future ngspice or
//! a future piperine changed which side reports which quantity, that assertion
//! fails first and names the reason.
//!
//! Without `ngspice` on PATH every golden case prints a SKIP notice and passes
//! (NGP-06); the comparator and the conversion are unit-covered with synthetic
//! data, so they hold with or without the binary.

use piperine::SolverConfig;

mod support;

use support::ngspice::{NgspiceHarness, ToleranceContract};

/// Boltzmann's constant (J/K) and ngspice's nominal temperature (K) — the two
/// numbers the Johnson-noise probe below is derived from. They are physics,
/// not tolerances, so they belong here rather than in the tolerance contract.
const BOLTZMANN: f64 = 1.380649e-23;
const NOMINAL_KELVIN: f64 = 300.15;

/// One noise cross-check: the fixture, the output/reference pair both sides
/// measure between, and the sweep its `.control` block runs (piperine
/// generates the same logarithmic grid).
struct NoiseCase {
    circuit: &'static str,
    out: &'static str,
    reference: &'static str,
    fstart: f64,
    fstop: f64,
    points: usize,
}

impl NoiseCase {
    /// **The one unit-reconciliation site.**
    ///
    /// ngspice reports output noise as an amplitude spectral density
    /// `S(f)` in V/√Hz — for a resistor, `S = √(4kTR)`. Piperine reports the
    /// power spectral density `S²` in V²/Hz — for the same resistor, `4kTR`.
    /// The two therefore relate by exactly one squaring:
    ///
    /// ```text
    ///   psd [V²/Hz]  =  spectrum [V/√Hz]²
    /// ```
    ///
    /// Verified against ngspice-46: 1 kΩ at 1 kHz exports 4.071e-9, and
    /// √(4·1.380649e-23·300.15·1000) = 4.0712e-9 V/√Hz. Every comparison in
    /// this target goes through here; none converts inline.
    fn psd_from_spectrum(spectrum: f64) -> f64 {
        spectrum * spectrum
    }

    /// ngspice's side: the amplitude spectrum per frequency, and the
    /// integrated total. `onoise_total` is a scalar, which `wrdata` renders as
    /// a single row repeating the value in both columns.
    /// `run` names the caller, so two tests reading the same fixture get two
    /// scratch directories: the key is per (circuit, domain, process), and
    /// tests of one target run in parallel inside one process.
    fn golden(&self, harness: &NgspiceHarness, run: &str) -> (Vec<(f64, f64)>, f64) {
        let psd = format!("{}_psd", self.circuit);
        let total = format!("{}_total", self.circuit);
        let mut exports = harness
            .run_export(self.circuit, run, &[(&psd, 2), (&total, 2)])
            .unwrap_or_else(|e| panic!("{e}"));
        let integrated = exports[1].pairs();
        let integrated = integrated
            .first()
            .unwrap_or_else(|| panic!("{}: onoise_total export has no row", self.circuit))
            .1;
        (exports.remove(0).pairs(), integrated)
    }

    /// The spectrum export alone, for a fixture whose `.control` block exports
    /// only `onoise_spectrum`. A device fixture measures the *shape* one
    /// model's sources give the output noise; the integrated figure and its
    /// quadrature caveat belong to [`NOISE_RC`].
    fn golden_psd(&self, harness: &NgspiceHarness, run: &str) -> Vec<(f64, f64)> {
        let psd = format!("{}_psd", self.circuit);
        let mut exports = harness
            .run_export(self.circuit, run, &[(&psd, 2)])
            .unwrap_or_else(|e| panic!("{e}"));
        exports.remove(0).pairs()
    }

    /// Piperine's side: the output-noise PSD samples and the integrated total.
    fn piperine(&self, harness: &NgspiceHarness) -> (Vec<(f64, f64)>, f64) {
        let mut session = harness.session(self.circuit).unwrap_or_else(|e| panic!("{e}"));
        // A solver failure is its own named failure, never a tolerance miss.
        let trace = session
            .noise(
                self.out,
                self.reference,
                (self.fstart, self.fstop),
                self.points,
                true,
                &SolverConfig::default(),
            )
            .unwrap_or_else(|e| {
                panic!("{}: piperine noise analysis failed to solve: {e}", self.circuit)
            });
        (trace.psd().points().to_vec(), trace.total())
    }

    /// Compare ngspice's **amplitude** spectrum against piperine's **PSD**,
    /// squaring the former at the single conversion site.
    fn compare_psd(
        circuit: &str,
        node: &str,
        golden: &[(f64, f64)],
        actual: &[(f64, f64)],
    ) -> Result<(), String> {
        if golden.len() != actual.len() {
            return Err(format!(
                "{circuit}: ngspice swept {} point(s) and piperine {} — the two sides must run \
                 the same grid before any value is comparable",
                golden.len(),
                actual.len()
            ));
        }
        let mut mismatches = Vec::new();
        for (&(f_ng, s_ng), &(f_pp, psd_pp)) in golden.iter().zip(actual) {
            if !ToleranceContract::NOISE_PSD.holds(f_ng, f_pp) {
                return Err(format!(
                    "{circuit}: frequency grids diverge — ngspice={f_ng:.6e} Hz, \
                     piperine={f_pp:.6e} Hz"
                ));
            }
            let psd_ng = Self::psd_from_spectrum(s_ng);
            if !ToleranceContract::NOISE_PSD.holds(psd_ng, psd_pp) {
                mismatches.push(format!(
                    "    PSD v({node}) @ {f_ng:.6e} Hz: ngspice={psd_ng:.6e} V²/Hz \
                     (from {s_ng:.6e} V/√Hz)  piperine={psd_pp:.6e} V²/Hz  Δ={:.3e}",
                    (psd_ng - psd_pp).abs()
                ));
            }
        }
        if !mismatches.is_empty() {
            return Err(format!(
                "{circuit}: noise PSD: {} of {} point(s) out of tolerance:\n{}\n  tolerance: {}",
                mismatches.len(),
                golden.len(),
                mismatches.join("\n"),
                ToleranceContract::NOISE_PSD.rationale()
            ));
        }
        Ok(())
    }

    /// Compare the integrated totals — both an RMS in volts, so this one is
    /// deliberately *not* squared.
    fn compare_total(circuit: &str, golden: f64, actual: f64) -> Result<(), String> {
        if ToleranceContract::NOISE_TOTAL.holds(golden, actual) {
            return Ok(());
        }
        Err(format!(
            "{circuit}: integrated output noise: ngspice={golden:.6e} V  piperine={actual:.6e} V  \
             Δ={:.3e}\n  tolerance: {}",
            (golden - actual).abs(),
            ToleranceContract::NOISE_TOTAL.rationale()
        ))
    }
}

/// The thermal-noise-only RC network, swept `dec 100` from 1 kHz to 1 MHz
/// (301 points on both sides).
///
/// The density is deliberate and belongs to the *integrated* comparison: the
/// per-point PSD agrees at any density, but the two sides integrate the
/// spectrum with different quadrature rules, so on a coarse `dec 10` grid
/// their totals differ by ~0.5 % — a quadrature artefact, not a model
/// deviation (measured: ngspice 1.554429e-6 V vs piperine 1.562253e-6 V, and
/// ngspice's own total moves to 1.556251e-6 V at `dec 100`). At `dec 100` both
/// quadratures have converged and the totals agree inside `NOISE_TOTAL`.
/// Widening the tolerance instead would have hidden that.
const NOISE_RC: NoiseCase = NoiseCase {
    circuit: "noise_rc",
    out: "out",
    reference: "gnd",
    fstart: 1.0e3,
    fstop: 1.0e6,
    points: 301,
};

/// The three triode-biased MOSFET fixtures (NGP-08) — one per model level,
/// otherwise identical circuits. Each biases `vgs - vto = 2 V` against a
/// settled `vds ≈ 0.2 V`, which is the region the fix is about: ngspice's gm
/// there is `beta·vds`, while the saturation-only estimate the models used to
/// carry is `beta·(vgs - vto)`, ten times larger. The load resistor is sized so
/// the channel term and its own thermal noise are the same order, so that
/// tenfold gm error shows up as a ~2.6× error in the output spectrum rather
/// than disappearing under the resistor.
const NOISE_MOS_LINEAR: [NoiseCase; 3] = [
    NoiseCase {
        circuit: "noise_mos_linear",
        out: "d",
        reference: "gnd",
        fstart: 1.0e3,
        fstop: 1.0e5,
        points: 21,
    },
    NoiseCase {
        circuit: "noise_mos2_linear",
        out: "d",
        reference: "gnd",
        fstart: 1.0e3,
        fstop: 1.0e5,
        points: 21,
    },
    NoiseCase {
        circuit: "noise_mos3_linear",
        out: "d",
        reference: "gnd",
        fstart: 1.0e3,
        fstop: 1.0e5,
        points: 21,
    },
];

/// NGP-08: mos1/mos2/mos3 channel noise in the linear region agrees with
/// ngspice, which it can only do if the noise source reads the model's
/// analytic gm. Reverting any of the three `gm` sites to the saturation-only
/// estimate fails this test on that model, naming the frequency and both
/// spectra.
#[test]
fn ngspice_mos_linear_region_channel_noise() {
    let Some(harness) = NgspiceHarness::detect() else {
        eprintln!("SKIP noise_mos_linear: ngspice not on PATH");
        return;
    };
    for case in &NOISE_MOS_LINEAR {
        let golden = case.golden_psd(&harness, "noise-mos-linear");
        let (actual, _) = case.piperine(&harness);
        NoiseCase::compare_psd(case.circuit, case.out, &golden, &actual)
            .unwrap_or_else(|e| panic!("{e}"));
        eprintln!("PASS {} ({} frequency points)", case.circuit, golden.len());
    }
}

/// The two linear-region JFET fixtures (NGP-08, NGP-13) — the same circuit
/// twice, differing only in the model card's `nlev`. Below nlev 3 ngspice's
/// channel noise is `4kT·(2/3)·|gm|`, and the bias (`vgs - vto = 2 V` against
/// `vds ≈ 0.25 V`) puts the analytic gm eight times below the saturation-only
/// estimate. At nlev 3 ngspice abandons gm entirely for its own closed form in
/// the saturation parameter alpha, which at this bias is ~3.9x larger in power
/// — so a model that quietly kept the nlev<3 form cannot pass the second case.
const NOISE_JFET: [NoiseCase; 2] = [
    NoiseCase {
        circuit: "noise_jfet",
        out: "d",
        reference: "gnd",
        fstart: 1.0e3,
        fstop: 1.0e5,
        points: 21,
    },
    NoiseCase {
        circuit: "noise_jfet_nlev3",
        out: "d",
        reference: "gnd",
        fstart: 1.0e3,
        fstop: 1.0e5,
        points: 21,
    },
];

/// NGP-08 + NGP-13: the JFET's channel noise agrees with ngspice in the linear
/// region on both noise levels — the analytic gm below nlev 3, and the
/// nlev-3 closed form when the card asks for it.
#[test]
fn ngspice_jfet_linear_region_channel_noise() {
    let Some(harness) = NgspiceHarness::detect() else {
        eprintln!("SKIP noise_jfet: ngspice not on PATH");
        return;
    };
    for case in &NOISE_JFET {
        let golden = case.golden_psd(&harness, "noise-jfet");
        let (actual, _) = case.piperine(&harness);
        NoiseCase::compare_psd(case.circuit, case.out, &golden, &actual)
            .unwrap_or_else(|e| panic!("{e}"));
        eprintln!("PASS {} ({} frequency points)", case.circuit, golden.len());
    }
}

/// NGP-02: the output-noise PSD agrees with ngspice at every swept frequency,
/// once ngspice's V/√Hz is squared into piperine's V²/Hz.
#[test]
fn ngspice_noise_rc_psd_per_frequency() {
    let Some(harness) = NgspiceHarness::detect() else {
        eprintln!("SKIP noise_rc: ngspice not on PATH");
        return;
    };
    let (golden, _) = NOISE_RC.golden(&harness, "noise-psd");
    let (actual, _) = NOISE_RC.piperine(&harness);
    NoiseCase::compare_psd(NOISE_RC.circuit, NOISE_RC.out, &golden, &actual)
        .unwrap_or_else(|e| panic!("{e}"));
    eprintln!("PASS noise_rc (noise PSD, {} frequency points)", golden.len());
}

/// NGP-02: the integrated total agrees too — `onoise_total` against
/// `NoiseTrace::total()`, both an RMS in volts.
#[test]
fn ngspice_noise_rc_integrated_total() {
    let Some(harness) = NgspiceHarness::detect() else {
        eprintln!("SKIP noise_rc: ngspice not on PATH");
        return;
    };
    let (_, golden) = NOISE_RC.golden(&harness, "noise-total");
    let (_, actual) = NOISE_RC.piperine(&harness);
    NoiseCase::compare_total(NOISE_RC.circuit, golden, actual).unwrap_or_else(|e| panic!("{e}"));
    eprintln!("PASS noise_rc (integrated total {actual:.6e} V vs {golden:.6e} V)");
}

/// The unit probe. R1 is the fixture's only noise source and the sweep's first
/// point (1 kHz) is two decades below the RC pole, so ngspice's spectrum there
/// must be the bare Johnson amplitude √(4kTR) — in **V/√Hz**. This is the
/// assertion that catches a unit regression on either side: if ngspice ever
/// exported V²/Hz, or piperine's PSD were compared unsquared, this fails
/// naming the formula rather than the model.
#[test]
fn the_thermal_probe_matches_the_johnson_amplitude() {
    let Some(harness) = NgspiceHarness::detect() else {
        eprintln!("SKIP noise_rc: ngspice not on PATH");
        return;
    };
    let (golden, _) = NOISE_RC.golden(&harness, "noise-probe");
    let (freq, spectrum) = golden[0];
    assert_eq!(freq, NOISE_RC.fstart, "the probe reads the sweep's first point");
    let johnson = (4.0 * BOLTZMANN * NOMINAL_KELVIN * 1000.0).sqrt();
    assert!(
        ToleranceContract::NOISE_PSD.holds(johnson, spectrum),
        "ngspice's onoise_spectrum at {freq:.6e} Hz must be the Johnson amplitude \
         √(4kTR) = {johnson:.6e} V/√Hz, got {spectrum:.6e} — a mismatch here is a UNIT \
         regression, not a model one\n  {}",
        ToleranceContract::NOISE_PSD.rationale()
    );
}

/// The conversion is a squaring, and comparing the two sides without it is
/// wrong by orders of magnitude — the failure mode the single named site
/// exists to prevent.
#[test]
fn the_spectrum_is_squared_into_a_psd() {
    let spectrum = (4.0 * BOLTZMANN * NOMINAL_KELVIN * 1000.0).sqrt();
    let psd = NoiseCase::psd_from_spectrum(spectrum);
    assert!(
        ToleranceContract::NOISE_PSD.holds(4.0 * BOLTZMANN * NOMINAL_KELVIN * 1000.0, psd),
        "√(4kTR) squared must be 4kTR: got {psd:.6e}"
    );
    assert!(
        !ToleranceContract::NOISE_PSD.holds(spectrum, psd),
        "the unconverted amplitude must NOT satisfy the PSD contract — if it did, the \
         squaring would be undetectable"
    );
}

/// NGP-05: a PSD miss names the node, the frequency, both values *in their
/// units*, the delta and the contract's rationale.
#[test]
fn a_psd_miss_is_actionable() {
    let golden = vec![(1.0e3, 4.0e-9)];
    let actual = vec![(1.0e3, 9.0e-17)];
    let err = NoiseCase::compare_psd("offnoise", "out", &golden, &actual)
        .expect_err("an out-of-tolerance PSD must fail");
    assert!(err.contains("offnoise"), "names the circuit: {err}");
    assert!(err.contains("PSD v(out)"), "names the node: {err}");
    assert!(err.contains("1.000000e3"), "names the frequency: {err}");
    assert!(err.contains("1.600000e-17"), "shows ngspice's converted value: {err}");
    assert!(err.contains("4.000000e-9 V/√Hz"), "shows the raw ngspice amplitude too: {err}");
    assert!(err.contains("9.000000e-17"), "shows piperine's value: {err}");
    assert!(
        err.contains(ToleranceContract::NOISE_PSD.rationale()),
        "carries the contract's rationale: {err}"
    );
}

/// NGP-05: an integrated-total miss is reported in volts, on its own contract.
#[test]
fn an_integrated_total_miss_is_actionable() {
    let err = NoiseCase::compare_total("offtotal", 1.5e-6, 3.0e-6)
        .expect_err("an out-of-tolerance total must fail");
    assert!(err.contains("offtotal"), "names the circuit: {err}");
    assert!(err.contains("1.500000e-6") && err.contains("3.000000e-6"), "both values: {err}");
    assert!(
        err.contains(ToleranceContract::NOISE_TOTAL.rationale()),
        "carries the contract's rationale: {err}"
    );
}

/// Two grids of different length are a setup error, reported before any value
/// is compared.
#[test]
fn mismatched_grid_lengths_fail_loud() {
    let err = NoiseCase::compare_psd("shortgrid", "out", &[(1.0e3, 1.0), (1.0e4, 1.0)], &[(1.0e3, 1.0)])
        .expect_err("differing point counts must fail");
    assert!(err.contains("shortgrid"), "names the circuit: {err}");
    assert!(err.contains("same grid"), "says why it cannot be compared: {err}");
}

/// NGP-06: without the binary on PATH the noise target takes the same SKIP
/// branch the DC oracle does.
#[test]
fn ngspice_absent_takes_the_skip_branch() {
    let empty = std::env::temp_dir();
    let path = Some(std::env::join_paths([&empty]).unwrap());
    assert!(
        NgspiceHarness::detect_with_path(path).is_none(),
        "no ngspice in an empty PATH must select the skip branch"
    );
}
