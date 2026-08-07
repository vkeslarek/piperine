//! ngspice golden cross-validation of **transient** responses (NGP-03).
//!
//! The two sides integrate on different grids: ngspice records its print grid
//! (the fixture's `.options interp` makes the `wrdata` export exactly that),
//! piperine returns an adaptively sampled [`Waveform`]. Comparing raw samples
//! would be meaningless, so the comparison evaluates piperine's waveform on
//! ngspice's print times via `Waveform::at` — interpolation in one direction
//! only — against `ToleranceContract::TRAN` (never a number retyped here,
//! NGP-07).
//!
//! A piperine run that does not converge is reported as *failed to converge*,
//! naming the analysis and the fixture — never as a tolerance miss (the spec's
//! partial-failure edge case). A run whose waveform ends before ngspice's grid
//! does is equally loud: `Waveform::at` clamps beyond the recorded range, and a
//! silent clamp would read a truncated run as agreement.
//!
//! Without `ngspice` on PATH the golden case prints a SKIP notice and passes
//! (NGP-06); the comparator's own failure modes are unit-covered here with
//! synthetic data, so they hold with or without the binary.

use std::collections::HashMap;

use piperine::{SolverConfig, Waveform};

mod support;

use support::ngspice::{NgspiceHarness, ToleranceContract};

/// One transient cross-check: the fixture, the node both sides record, and
/// the stop time its `.control` block integrates to (the print grid comes from
/// the ngspice export itself).
struct TranCase {
    circuit: &'static str,
    node: &'static str,
    stop: f64,
}

impl TranCase {
    /// Run both sides and compare; panics with the loud failure text.
    fn run(&self, harness: &NgspiceHarness) {
        let export = format!("{}_tran", self.circuit);
        let golden = harness
            .run_export(self.circuit, "tran", &[(&export, 2)])
            .unwrap_or_else(|e| panic!("{e}"))
            .remove(0)
            .pairs();

        let actual = self.piperine(harness).unwrap_or_else(|e| panic!("{e}"));
        Self::compare(self.circuit, self.node, &golden, &actual)
            .unwrap_or_else(|e| panic!("{e}"));
        eprintln!("PASS {} (transient, {} print points)", self.circuit, golden.len());
    }

    /// Piperine's side: the adaptively sampled waveform of the probed node.
    /// `ic = {out: 0}` starts the capacitor discharged — the DC operating
    /// point would otherwise leave it charged from t=0 and the run would
    /// carry no transient at all (ngspice's `uic`).
    fn piperine(&self, harness: &NgspiceHarness) -> Result<Waveform, String> {
        let mut session = harness.session(self.circuit)?;
        let ic = HashMap::from([(self.node.to_string(), 0.0)]);
        // A solver failure is its own named failure, never a tolerance miss.
        let trace = session
            .tran(self.stop, None, 0.0, &SolverConfig::default(), Some(&ic), false, &[])
            .map_err(|e| Self::convergence_failure(self.circuit, e))?;
        trace
            .v(self.node.to_string())
            .map_err(|e| format!("{}: no v({}) in the transient trace: {e}", self.circuit, self.node))
    }

    /// A non-convergent run names the analysis and the fixture and says what
    /// happened — it is never phrased like a tolerance miss (the spec's
    /// partial-failure edge case).
    fn convergence_failure(circuit: &str, error: impl std::fmt::Display) -> String {
        format!(
            "{circuit}: piperine transient failed to converge: {error} — \
             a solver failure, not a tolerance miss"
        )
    }

    /// Compare node voltages on ngspice's print grid, evaluating piperine's
    /// adaptive waveform at each print time — never the raw samples.
    fn compare(
        circuit: &str,
        node: &str,
        golden: &[(f64, f64)],
        actual: &Waveform,
    ) -> Result<(), String> {
        // `Waveform::at` clamps outside the recorded range, so a piperine run
        // that ended early would silently compare its final value against
        // every later print point. Refuse that comparison instead.
        let grid_end = golden
            .last()
            .map(|(t, _)| *t)
            .expect("WrData rejects an empty export");
        let run_end = actual
            .points()
            .last()
            .map(|(t, _)| *t)
            .expect("a solved transient has samples");
        if grid_end > run_end {
            return Err(format!(
                "{circuit}: ngspice's print grid runs to {grid_end:.6e} s but piperine's \
                 waveform ends at {run_end:.6e} s — a truncated run, not a comparison"
            ));
        }
        let mut mismatches = Vec::new();
        for &(t, v_ng) in golden {
            let v_pp = actual.at(t);
            if !ToleranceContract::TRAN.holds(v_ng, v_pp) {
                mismatches.push(format!(
                    "    v({node}) @ t={t:.6e} s: ngspice={v_ng:.6e}  piperine={v_pp:.6e}  \
                     Δ={:.3e}",
                    (v_ng - v_pp).abs()
                ));
            }
        }
        if !mismatches.is_empty() {
            return Err(format!(
                "{circuit}: transient: {} of {} print point(s) out of tolerance:\n{}\n  tolerance: {}",
                mismatches.len(),
                golden.len(),
                mismatches.join("\n"),
                ToleranceContract::TRAN.rationale()
            ));
        }
        Ok(())
    }
}

/// The RC charging curve, integrated to 10 µs (`tran 0.1u 10u uic` on the
/// ngspice side, a 100-point print grid).
const TRAN_RC: TranCase = TranCase { circuit: "tran_rc", node: "out", stop: 10.0e-6 };

/// NGP-03: `v(out)` agrees with ngspice at every print time, with piperine's
/// adaptive waveform interpolated onto ngspice's grid.
#[test]
fn ngspice_tran_rc() {
    match NgspiceHarness::detect() {
        Some(harness) => TRAN_RC.run(&harness),
        None => eprintln!("SKIP tran_rc: ngspice not on PATH"),
    }
}

/// NGP-10 part 1 (cap `ic`/UIC): the cap's per-instance `.ic = 2.0` seeds
/// the t=0 voltage, the way ngspice's `IC=2.0 … uic` does. The fixture's
/// response is `v(t) = 5 − 3·exp(−t/RC)` (a step from 2 V toward 5 V) — a
/// discriminating check that fails when the seed is dropped (v(out) starts
/// at 0 V) or when a default-ic clamp misfires (v(out) pinned to 0 V for
/// the whole run). The per-instance `.ic` reaches the solver through the
/// cap body's `@ initial { V(p,n) <- ic; }` and the
/// `AnalogInstance::initial_conditions` filter, so no host-side `ic` map
/// is passed (the device-level seed is what drives UIC here).
#[test]
fn ngspice_uic_rc() {
    let Some(harness) = NgspiceHarness::detect() else {
        eprintln!("SKIP uic_rc: ngspice not on PATH");
        return;
    };
    let export = "uic_rc_tran";
    let golden = harness
        .run_export("uic_rc", "tran", &[(export, 2)])
        .unwrap_or_else(|e| panic!("{e}"))
        .remove(0)
        .pairs();

    let mut session = harness.session("uic_rc").unwrap_or_else(|e| panic!("{e}"));
    // No host-side `ic` map: the cap's per-instance `.ic = 2.0` is the
    // UIC seed source. The solver still computes the DC operating point
    // and overlays the device's initial-condition clamps on top.
    let trace = session
        .tran(10.0e-6, None, 0.0, &SolverConfig::default(), None, false, &[])
        .map_err(|e| TranCase::convergence_failure("uic_rc", e))
        .unwrap_or_else(|e| panic!("{e}"));
    let actual = trace
        .v("out".to_string())
        .unwrap_or_else(|e| panic!("uic_rc: no v(out) in the transient trace: {e}"));

    TranCase::compare("uic_rc", "out", &golden, &actual)
        .unwrap_or_else(|e| panic!("{e}"));
    // The closed-form anchor: v(0) = 2 V (the ic seed); v(∞) → 5 V.
    let v0 = actual.at(0.0);
    assert!(
        (v0 - 2.0).abs() < 1.0e-3,
        "v(out)@0 = {v0:.6e}, expected the ic seed 2.0 V \
         (a default-ic clamp would give 0, a dropped seed would give the DC OP)"
    );
    eprintln!("PASS uic_rc (transient, {} print points, UIC seed = 2 V)", golden.len());
}

/// The fixture is anchored to its **closed form**, not only to ngspice: the
/// charging curve is `v(t) = 1 − exp(−t/RC)` with `RC = 1.5915494309189535 µs`,
/// so at `t = RC` the waveform reads `1 − e⁻¹` and at `t = 5·RC` it reads
/// `1 − e⁻⁵` — both read through `Waveform::at`, the same interpolation the
/// golden comparison uses.
#[test]
fn the_rc_fixture_matches_its_analytic_charging_curve() {
    let harness = NgspiceHarness { exe: std::path::PathBuf::new() };
    let waveform = TRAN_RC.piperine(&harness).unwrap_or_else(|e| panic!("{e}"));
    let tau = 1000.0 * 1.5915494309189535e-9;
    for (t, expected) in [(tau, 1.0 - (-1.0f64).exp()), (5.0 * tau, 1.0 - (-5.0f64).exp())] {
        let v = waveform.at(t);
        assert!(
            ToleranceContract::TRAN.holds(expected, v),
            "v(out) at t={t:.6e} s must be the analytic {expected:.9e}, got {v:.9e}\n  {}",
            ToleranceContract::TRAN.rationale()
        );
    }
}

/// NGP-03's core: the comparison interpolates onto the grid rather than
/// comparing raw samples. The waveform below has no sample at the golden
/// time `t = 1.5`, so only the interpolated value (1.5, the midpoint) can
/// pass — a raw-sample comparison would fail or compare nothing.
#[test]
fn the_comparison_interpolates_onto_the_print_grid() {
    let waveform = Waveform::new(vec![(1.0, 1.0), (2.0, 2.0)]);
    let golden = vec![(1.5, 1.5)];
    TranCase::compare("interp", "out", &golden, &waveform)
        .unwrap_or_else(|e| panic!("the interpolated value must pass: {e}"));
    let off = vec![(1.5, 1.0)];
    assert!(
        TranCase::compare("interp", "out", &off, &waveform).is_err(),
        "the raw-sample value must NOT pass at a time with no raw sample"
    );
}

/// A piperine run that ended before ngspice's grid does is a truncated run,
/// reported as such — never silently clamped into agreement.
#[test]
fn a_grid_beyond_the_waveform_is_a_loud_failure() {
    let waveform = Waveform::new(vec![(0.0, 0.0), (1.0, 1.0)]);
    let golden = vec![(1.0, 1.0), (2.0, 1.0)];
    let err = TranCase::compare("truncated", "out", &golden, &waveform)
        .expect_err("a print grid beyond the run must fail");
    assert!(err.contains("truncated"), "names the circuit: {err}");
    assert!(err.contains("2.000000e0"), "names the grid end: {err}");
    assert!(err.contains("truncated run"), "says why it cannot be compared: {err}");
}

/// NGP-05: a transient miss names the node, the time, both values, the delta
/// and the contract's rationale.
#[test]
fn a_tolerance_miss_is_actionable() {
    let waveform = Waveform::new(vec![(0.0, 0.0), (1.0e-6, 0.5)]);
    let golden = vec![(1.0e-6, 0.9)];
    let err = TranCase::compare("offcurve", "out", &golden, &waveform)
        .expect_err("an out-of-tolerance point must fail");
    assert!(err.contains("offcurve"), "names the circuit: {err}");
    assert!(err.contains("v(out)"), "names the node: {err}");
    assert!(err.contains("t=1.000000e-6"), "names the time: {err}");
    assert!(err.contains("9.000000e-1") && err.contains("5.000000e-1"), "both values: {err}");
    assert!(err.contains("Δ="), "shows the delta: {err}");
    assert!(
        err.contains(ToleranceContract::TRAN.rationale()),
        "carries the contract's rationale: {err}"
    );
}

/// The spec's partial-failure edge case: a non-convergent run is reported as
/// *failed to converge*, phrased so it can never be read as a tolerance miss.
#[test]
fn a_convergence_failure_is_distinct_from_a_tolerance_miss() {
    let convergence = TranCase::convergence_failure("stiff", "Newton did not converge");
    assert!(convergence.contains("stiff"), "names the fixture: {convergence}");
    assert!(convergence.contains("failed to converge"), "names the failure: {convergence}");
    assert!(!convergence.contains("out of tolerance"), "never a tolerance phrasing: {convergence}");

    let waveform = Waveform::new(vec![(0.0, 0.0), (1.0e-6, 0.5)]);
    let miss = TranCase::compare("stiff", "out", &[(1.0e-6, 0.9)], &waveform)
        .expect_err("the tolerance miss for the same fixture");
    assert!(miss.contains("out of tolerance"), "the miss keeps its own phrasing: {miss}");
    assert!(!miss.contains("failed to converge"), "never a solver phrasing: {miss}");
}

/// NGP-06: without the binary on PATH the transient target takes the same
/// SKIP branch the DC oracle does.
#[test]
fn ngspice_absent_takes_the_skip_branch() {
    let empty = std::env::temp_dir();
    let path = Some(std::env::join_paths([&empty]).unwrap());
    assert!(
        NgspiceHarness::detect_with_path(path).is_none(),
        "no ngspice in an empty PATH must select the skip branch"
    );
}
