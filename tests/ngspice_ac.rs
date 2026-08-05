//! ngspice golden cross-validation of **AC small-signal** responses (NGP-01).
//!
//! Each `.cir`/`.phdl` twin in `tests/ngspice/` describes one network twice.
//! ngspice's `.control` block exports `v(<node>)` with `wrdata`, which renders
//! a complex vector as three columns (`freq real imag`); piperine sweeps the
//! same frequency grid in-process and the two are compared **per frequency
//! point**, on magnitude *and* phase, against `ToleranceContract::{AC_MAG,
//! AC_PHASE}` — never a number retyped here (NGP-07).
//!
//! Phase is meaningless where the magnitude has collapsed into `AC_MAG`'s
//! absolute floor, so it is compared only above that floor — and every skip is
//! counted, because a fixture that skipped every point would otherwise report
//! a pass having compared no phase at all (NGP-05, the vacuous-comparison
//! edge case).
//!
//! Without `ngspice` on PATH every golden case prints a SKIP notice and passes
//! (NGP-06); the comparator's own failure modes are unit-covered here with
//! synthetic data, so they hold with or without the binary.

use std::f64::consts::{PI, SQRT_2};
use std::path::PathBuf;

use num_complex::Complex64;
use piperine::SolverConfig;

mod support;

use support::ngspice::{NgspiceHarness, ToleranceContract};

/// One AC cross-check: the fixture, the node both sides export, and the sweep
/// its `.control` block runs.
///
/// The sweep is restated here because piperine must generate the *same* grid:
/// ngspice's `ac dec <n> <f1> <f2>` yields `f1·10^(i/n)` and piperine's
/// logarithmic sweep yields `f1·(f2/f1)^(i/(points−1))` — identical when
/// `points` is the decade count times `n`, plus one. A grid that drifts apart
/// is caught before any value is compared.
struct AcCase {
    circuit: &'static str,
    node: &'static str,
    fstart: f64,
    fstop: f64,
    points: usize,
}

impl AcCase {
    /// Run both sides and compare; panics with the loud failure text.
    fn run(&self, harness: &NgspiceHarness) {
        let export = format!("{}_ac", self.circuit);
        let golden = harness
            .run_export(self.circuit, "ac", &[(&export, 3)])
            .unwrap_or_else(|e| panic!("{e}"))
            .remove(0)
            .complex();

        let actual = self.piperine(harness);
        Self::compare(self.circuit, self.node, &golden, &actual)
            .unwrap_or_else(|e| panic!("{e}"));
        eprintln!("PASS {} (AC, {} frequency points)", self.circuit, golden.len());
    }

    /// Piperine's side of the sweep as `(freq, H)` samples.
    fn piperine(&self, harness: &NgspiceHarness) -> Vec<(f64, Complex64)> {
        let mut session = harness.session(self.circuit).unwrap_or_else(|e| panic!("{e}"));
        // A solver failure is its own named failure, never a tolerance miss.
        let trace = session
            .ac(self.fstart, self.fstop, self.points, true, &SolverConfig::default())
            .unwrap_or_else(|e| {
                panic!("{}: piperine AC analysis failed to solve: {e}", self.circuit)
            });
        trace
            .v(self.node.to_string())
            .unwrap_or_else(|e| panic!("{}: no v({}) in the AC trace: {e}", self.circuit, self.node))
            .points()
            .to_vec()
    }

    /// Compare two swept responses point by point. Magnitude is always
    /// compared; phase only where the magnitude clears `AC_MAG`'s absolute
    /// floor, and a run in which *every* point skipped is a vacuous
    /// comparison, not a pass.
    fn compare(
        circuit: &str,
        node: &str,
        golden: &[(f64, Complex64)],
        actual: &[(f64, Complex64)],
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
        let mut phase_skips = 0usize;
        for (&(f_ng, h_ng), &(f_pp, h_pp)) in golden.iter().zip(actual) {
            if !ToleranceContract::AC_MAG.holds(f_ng, f_pp) {
                return Err(format!(
                    "{circuit}: frequency grids diverge — ngspice={f_ng:.6e} Hz, \
                     piperine={f_pp:.6e} Hz"
                ));
            }
            let (m_ng, m_pp) = (h_ng.norm(), h_pp.norm());
            if !ToleranceContract::AC_MAG.holds(m_ng, m_pp) {
                mismatches.push(format!(
                    "    |v({node})| @ {f_ng:.6e} Hz: ngspice={m_ng:.6e}  piperine={m_pp:.6e}  \
                     Δ={:.3e}\n      {}",
                    (m_ng - m_pp).abs(),
                    ToleranceContract::AC_MAG.rationale()
                ));
            }
            // `AC_MAG.holds(0.0, m)` is true exactly when `m` sits inside the
            // contract's absolute floor — the deep stopband, where the phase
            // of a ~0 phasor is numerical noise on both sides.
            if ToleranceContract::AC_MAG.holds(0.0, m_ng)
                || ToleranceContract::AC_MAG.holds(0.0, m_pp)
            {
                phase_skips += 1;
                continue;
            }
            // The wrapped angle between the two phasors, so a pair straddling
            // ±π is not reported as a 2π miss.
            let delta = (h_ng * h_pp.conj()).arg().abs();
            if !ToleranceContract::AC_PHASE.holds(0.0, delta) {
                mismatches.push(format!(
                    "    ∠v({node}) @ {f_ng:.6e} Hz: ngspice={:.6e} rad  piperine={:.6e} rad  \
                     Δ={delta:.3e}\n      {}",
                    h_ng.arg(),
                    h_pp.arg(),
                    ToleranceContract::AC_PHASE.rationale()
                ));
            }
        }
        if phase_skips == golden.len() {
            return Err(format!(
                "{circuit}: |v({node})| stayed inside AC_MAG's floor at all {} point(s), so \
                 every phase comparison was skipped and the fixture compared no phase at all — \
                 a vacuous comparison, not a pass",
                golden.len()
            ));
        }
        if !mismatches.is_empty() {
            return Err(format!(
                "{circuit}: AC: {} of {} point(s) out of tolerance:\n{}",
                mismatches.len(),
                golden.len(),
                mismatches.join("\n")
            ));
        }
        Ok(())
    }
}

/// The RC lowpass, swept `dec 10` from 1 kHz to 10 MHz (41 points on both
/// sides). R = 1 kΩ and C = 1.5915494309189535 nF put the -3 dB corner at
/// exactly 100 kHz, which is a grid point rather than an interpolation.
const AC_RC: AcCase =
    AcCase { circuit: "ac_rc", node: "out", fstart: 1.0e3, fstop: 1.0e7, points: 41 };

/// NGP-01: magnitude and phase of `v(out)` agree with ngspice at every swept
/// frequency.
#[test]
fn ngspice_ac_rc() {
    match NgspiceHarness::detect() {
        Some(harness) => AC_RC.run(&harness),
        None => eprintln!("SKIP ac_rc: ngspice not on PATH"),
    }
}

/// NGP-16 + NGP-01: the tapered URC ladder. The fixture's `.cir` builds a
/// linear RC ladder by hand with per-segment R/C growing geometrically
/// (K = 2.0 toward the output), and the `.phdl` twin instantiates piperine's
/// `urc5` with `.k = 2.0` — piperine applies the same per-segment scaling.
/// The fixture fails when the taper is reverted (a uniform `urc5` ladder at
/// the same total R/C produces a measurably different AC response, since the
/// per-segment distribution shifts the poles).
const URC_TAPER: AcCase =
    AcCase { circuit: "urc_taper", node: "out", fstart: 1.0e2, fstop: 1.0e6, points: 41 };

#[test]
fn ngspice_urc_taper() {
    match NgspiceHarness::detect() {
        Some(harness) => URC_TAPER.run(&harness),
        None => eprintln!("SKIP urc_taper: ngspice not on PATH"),
    }
}

/// The fixture is anchored to its **closed form**, not only to ngspice: for a
/// unit AC stimulus `v(out) = H(f) = 1/(1 + j·2πfRC)`, so at the corner
/// `f = 1/(2πRC) = 100 kHz` the magnitude is exactly `1/√2` and the phase
/// exactly `−π/4`. Without this, two implementations could agree on a wrong
/// answer and the cross-check would still be green.
#[test]
fn the_rc_fixture_matches_its_analytic_minus_3db_point() {
    let harness = NgspiceHarness { exe: PathBuf::new() };
    let response = AC_RC.piperine(&harness);
    let corner = 1.0 / (2.0 * PI * 1000.0 * 1.5915494309189535e-9);
    let (f, h) = response
        .iter()
        .min_by(|a, b| (a.0 - corner).abs().total_cmp(&(b.0 - corner).abs()))
        .copied()
        .expect("the sweep produced points");
    assert!(
        ToleranceContract::AC_MAG.holds(corner, f),
        "the corner must be a grid point: corner={corner:.6e} Hz, nearest sample={f:.6e} Hz"
    );
    assert!(
        ToleranceContract::AC_MAG.holds(1.0 / SQRT_2, h.norm()),
        "|H| at the -3 dB corner must be 1/√2: got {:.9e}\n  {}",
        h.norm(),
        ToleranceContract::AC_MAG.rationale()
    );
    assert!(
        ToleranceContract::AC_PHASE.holds(-PI / 4.0, h.arg()),
        "∠H at the -3 dB corner must be −π/4 rad: got {:.9e}\n  {}",
        h.arg(),
        ToleranceContract::AC_PHASE.rationale()
    );
}

/// A fixture whose magnitude never leaves the floor compares no phase at all.
/// That is a contract violation, not a pass — the guard that keeps the phase
/// skip from turning into a silent hole.
#[test]
fn every_point_below_the_magnitude_floor_is_a_vacuous_failure() {
    let flat: Vec<(f64, Complex64)> =
        vec![(1.0e3, Complex64::new(1e-20, 0.0)), (1.0e4, Complex64::new(1e-20, 0.0))];
    let err = AcCase::compare("floored", "out", &flat, &flat)
        .expect_err("an all-skipped phase comparison must fail");
    assert!(err.contains("floored"), "names the circuit: {err}");
    assert!(err.contains("v(out)"), "names the node: {err}");
    assert!(err.contains("vacuous"), "names the violation: {err}");
}

/// NGP-05: a magnitude miss names the node, the frequency, both values, the
/// delta and the contract's rationale.
#[test]
fn a_magnitude_miss_is_actionable() {
    let golden = vec![(1.0e5, Complex64::new(0.5, -0.5))];
    let actual = vec![(1.0e5, Complex64::new(0.9, -0.9))];
    let err = AcCase::compare("offmag", "out", &golden, &actual)
        .expect_err("an out-of-tolerance magnitude must fail");
    assert!(err.contains("offmag"), "names the circuit: {err}");
    assert!(err.contains("|v(out)|"), "names the node and quantity: {err}");
    assert!(err.contains("1.000000e5"), "names the frequency: {err}");
    assert!(err.contains("7.071068e-1"), "shows ngspice's value: {err}");
    assert!(err.contains("1.272792e0"), "shows piperine's value: {err}");
    assert!(err.contains("Δ="), "shows the delta: {err}");
    assert!(
        err.contains(ToleranceContract::AC_MAG.rationale()),
        "carries the contract's rationale: {err}"
    );
}

/// NGP-05: a phase miss at an equal magnitude is reported on its own terms —
/// the magnitude check alone would pass this pair.
#[test]
fn a_phase_miss_is_reported_separately_from_magnitude() {
    let golden = vec![(1.0e5, Complex64::new(1.0, 0.0))];
    let actual = vec![(1.0e5, Complex64::from_polar(1.0, 0.01))];
    let err = AcCase::compare("offphase", "out", &golden, &actual)
        .expect_err("an out-of-tolerance phase must fail");
    assert!(err.contains("∠v(out)"), "names the phase of the node: {err}");
    assert!(err.contains("1.000000e5"), "names the frequency: {err}");
    assert!(
        err.contains(ToleranceContract::AC_PHASE.rationale()),
        "carries the phase contract's rationale: {err}"
    );
    assert!(!err.contains("|v(out)|"), "the magnitudes agree, so only phase is reported: {err}");
}

/// Two grids of different length are a setup error, reported before any value
/// is compared — never a comparison of the first N points.
#[test]
fn mismatched_grid_lengths_fail_loud() {
    let golden = vec![(1.0e3, Complex64::new(1.0, 0.0)), (1.0e4, Complex64::new(1.0, 0.0))];
    let actual = vec![(1.0e3, Complex64::new(1.0, 0.0))];
    let err = AcCase::compare("shortgrid", "out", &golden, &actual)
        .expect_err("differing point counts must fail");
    assert!(err.contains("shortgrid"), "names the circuit: {err}");
    assert!(err.contains("same grid"), "says why it cannot be compared: {err}");
}

/// NGP-06: without the binary on PATH the AC target takes the same SKIP
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
