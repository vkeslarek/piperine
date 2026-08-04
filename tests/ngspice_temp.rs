//! ngspice golden cross-validation of the **operating point across
//! temperature** (NGP-04).
//!
//! Each fixture compares at least **two** temperatures — one below and one
//! above ngspice's 27 °C nominal — so a model whose temperature path is
//! pinned to nominal agrees at one point at most and must fail the other.
//! The ngspice side is a `.control` block doing `set temp = <C>` + `op` per
//! point; the piperine side solves the same operating point with
//! `SolverConfig.temperature` set to the same temperature (Celsius + 273.15,
//! converted in exactly one place — [`TempCase::kelvin`]). The comparison
//! itself is a DC comparison and reads `ToleranceContract::TEMP` — never a
//! number retyped here (NGP-07).
//!
//! Without `ngspice` on PATH the golden case prints a SKIP notice and passes
//! (NGP-06); the comparator's own failure modes are unit-covered here with
//! synthetic data, so they hold with or without the binary.

use piperine::SolverConfig;

mod support;

use support::ngspice::{NgspiceHarness, ToleranceContract};

/// ngspice's nominal temperature in °C — the point a broken model is pinned
/// to. The golden points must straddle it, or the pinned model would pass.
const NOMINAL_CELSIUS: f64 = 27.0;

/// One temperature cross-check: the fixture, the node both sides report, and
/// the `(celsius, export suffix)` points its `.control` block exports — at
/// least two, straddling nominal.
struct TempCase {
    circuit: &'static str,
    node: &'static str,
    points: &'static [(f64, &'static str)],
}

impl TempCase {
    /// The one Celsius → Kelvin site. ngspice's `.temp` is in °C; piperine's
    /// `SolverConfig.temperature` is in kelvin (300.15 K nominal = 27 °C).
    /// Every comparison in this target converts here; none converts inline.
    fn kelvin(celsius: f64) -> f64 {
        celsius + 273.15
    }

    /// Run both sides and compare; panics with the loud failure text.
    fn run(&self, harness: &NgspiceHarness) {
        self.check_points_straddle_nominal();
        let golden = self.golden(harness);
        let actual = self.piperine(harness);
        Self::compare(self.circuit, self.node, &golden, &actual)
            .unwrap_or_else(|e| panic!("{e}"));
        eprintln!("PASS {} (temperature, {} points)", self.circuit, golden.len());
    }

    /// The cross-check's reason to exist: at least two points, on opposite
    /// sides of nominal. A single point — or two on the same side — could be
    /// passed by a model pinned to nominal, and the fixture would prove
    /// nothing.
    fn check_points_straddle_nominal(&self) {
        assert!(
            self.points.len() >= 2,
            "{}: a temperature cross-check needs ≥2 points, got {}",
            self.circuit,
            self.points.len()
        );
        let below = self.points.iter().any(|(t, _)| *t < NOMINAL_CELSIUS);
        let above = self.points.iter().any(|(t, _)| *t > NOMINAL_CELSIUS);
        assert!(
            below && above,
            "{}: the points must straddle the {NOMINAL_CELSIUS} °C nominal, got {:?} — \
             a model pinned to nominal would pass this fixture",
            self.circuit,
            self.points.iter().map(|(t, _)| *t).collect::<Vec<_>>()
        );
    }

    /// ngspice's side: one `v(<node>)` export per temperature, read back as
    /// `(celsius, value)` pairs. Each export is a single scalar row.
    fn golden(&self, harness: &NgspiceHarness) -> Vec<(f64, f64)> {
        let exports: Vec<(String, usize)> = self
            .points
            .iter()
            .map(|(_, suffix)| (format!("{}_{}", self.circuit, suffix), 2))
            .collect();
        let refs: Vec<(&str, usize)> =
            exports.iter().map(|(file, cols)| (file.as_str(), *cols)).collect();
        // `temperature`, not `temp`: the scratch key is per (circuit, domain,
        // process) and the support module's own cleanup unit test already
        // holds `temp_diode`/`temp` — sharing it would let that test's Drop
        // remove this run's directory mid-export.
        let data = harness
            .run_export(self.circuit, "temperature", &refs)
            .unwrap_or_else(|e| panic!("{e}"));
        self.points
            .iter()
            .zip(&data)
            .map(|((celsius, _), export)| {
                let pairs = export.pairs();
                let (_, value) = pairs
                    .first()
                    .copied()
                    .unwrap_or_else(|| panic!("{}: temperature export has no row", self.circuit));
                (*celsius, value)
            })
            .collect()
    }

    /// Piperine's side: one operating point per temperature, same node.
    fn piperine(&self, harness: &NgspiceHarness) -> Vec<(f64, f64)> {
        let mut session = harness.session(self.circuit).unwrap_or_else(|e| panic!("{e}"));
        self.points
            .iter()
            .map(|(celsius, _)| {
                let config =
                    SolverConfig { temperature: Self::kelvin(*celsius), ..Default::default() };
                // A solver failure is its own named failure, never a tolerance miss.
                let op = session.op(&config, None).unwrap_or_else(|e| {
                    panic!(
                        "{}: piperine DC solve failed at {celsius} °C: {e} — \
                         a solver failure, not a tolerance miss",
                        self.circuit
                    )
                });
                let value = op
                    .v(self.node.to_string())
                    .unwrap_or_else(|e| panic!("{}: no v({}) in the OP: {e}", self.circuit, self.node));
                (*celsius, value)
            })
            .collect()
    }

    /// Compare the operating point at each temperature.
    fn compare(
        circuit: &str,
        node: &str,
        golden: &[(f64, f64)],
        actual: &[(f64, f64)],
    ) -> Result<(), String> {
        if golden.len() != actual.len() {
            return Err(format!(
                "{circuit}: ngspice exported {} temperature point(s) and piperine {} — \
                 the two sides must run the same temperatures before any value is comparable",
                golden.len(),
                actual.len()
            ));
        }
        let mut mismatches = Vec::new();
        for (&(t_ng, v_ng), &(t_pp, v_pp)) in golden.iter().zip(actual) {
            assert_eq!(t_ng, t_pp, "{circuit}: temperature grids diverge — {t_ng} vs {t_pp} °C");
            if !ToleranceContract::TEMP.holds(v_ng, v_pp) {
                mismatches.push(format!(
                    "    v({node}) @ {t_ng} °C: ngspice={v_ng:.6e}  piperine={v_pp:.6e}  \
                     Δ={:.3e}",
                    (v_ng - v_pp).abs()
                ));
            }
        }
        // The discrimination check: if ngspice itself barely moves across the
        // sweep, a model pinned to nominal would pass and the fixture proves
        // nothing. The fixture must be built so the true answer *changes*.
        let spread = golden
            .iter()
            .map(|(_, v)| *v)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)));
        let pinned = (spread.0 + spread.1) / 2.0;
        assert!(
            golden.iter().any(|(_, v)| !ToleranceContract::TEMP.holds(*v, pinned)),
            "{circuit}: ngspice's own v({node}) spread ({:.3e} V) fits inside the TEMP \
             tolerance around its midpoint — a model pinned to nominal would pass this \
             fixture, so it cannot detect a dead temperature path",
            spread.1 - spread.0
        );
        if !mismatches.is_empty() {
            return Err(format!(
                "{circuit}: temperature: {} of {} point(s) out of tolerance:\n{}\n  tolerance: {}",
                mismatches.len(),
                golden.len(),
                mismatches.join("\n"),
                ToleranceContract::TEMP.rationale()
            ));
        }
        Ok(())
    }
}

/// The forward-biased diode at 0 °C and 75 °C — both away from the 27 °C
/// nominal, ~117 mV apart on the golden side.
const TEMP_DIODE: TempCase = TempCase {
    circuit: "temp_diode",
    node: "out",
    points: &[(0.0, "t0"), (75.0, "t75")],
};

/// The forced-base BJT pair at -25 °C and 100 °C. `q1` is forward active, so
/// its collector current is set by beta and the Early effect alone; `q2` is
/// driven hard into saturation, where the *reverse* Early voltage enters. The
/// card gives both `VAF`/`VAR` a temperature coefficient, so a model that pins
/// them to their nominal value — as this one did, with a hardcoded
/// `1 + 0.0·dt` — cannot agree at both points. Measured with the coefficients
/// zeroed on ngspice's side, v(c) moves 45 mV at -25 °C and 145 mV at 100 °C,
/// and v(c2) 0.22 mV; the TEMP contract admits 5.8 mV and 34 µV respectively.
const TEMP_BJT: TempCase = TempCase {
    circuit: "temp_bjt",
    node: "c",
    points: &[(-25.0, "tm25"), (100.0, "t100")],
};

/// The same fixture read at the saturated device, which is what exercises
/// `VAR` — the forward-active node above barely feels it.
const TEMP_BJT_SATURATED: TempCase = TempCase {
    circuit: "temp_bjt",
    node: "c2",
    points: &[(-25.0, "sat_tm25"), (100.0, "sat_t100")],
};

/// NGP-14: `VAF` and `VAR` follow their temperature dependence rather than
/// being pinned to nominal, in both the forward-active and the saturated
/// device.
#[test]
fn ngspice_temp_bjt_early_voltages() {
    match NgspiceHarness::detect() {
        Some(harness) => {
            TEMP_BJT.run(&harness);
            TEMP_BJT_SATURATED.run(&harness);
        }
        None => eprintln!("SKIP temp_bjt: ngspice not on PATH"),
    }
}

/// NGP-04: the operating point agrees with ngspice at two temperatures, one
/// below and one above nominal.
#[test]
fn ngspice_temp_diode() {
    match NgspiceHarness::detect() {
        Some(harness) => TEMP_DIODE.run(&harness),
        None => eprintln!("SKIP temp_diode: ngspice not on PATH"),
    }
}

/// The case really does straddle nominal — the guard that keeps a
/// single-sided fixture from passing a nominal-pinned model.
#[test]
fn the_case_straddles_the_nominal_temperature() {
    TEMP_DIODE.check_points_straddle_nominal();
}

/// ngspice's `.temp` is °C and piperine's `SolverConfig.temperature` is
/// kelvin; the conversion happens in one named site and nominal maps to the
/// 300.15 K the solver defaults to.
#[test]
fn celsius_converts_to_kelvin_in_one_named_site() {
    assert_eq!(TempCase::kelvin(NOMINAL_CELSIUS), 300.15, "nominal must map to the solver default");
    assert_eq!(TempCase::kelvin(0.0), 273.15);
    assert_eq!(TempCase::kelvin(75.0), 348.15);
}

/// A model pinned to nominal reads the same voltage at both temperatures;
/// the golden side does not. That pair must fail — the fixture's whole
/// purpose (the task's Done-when).
#[test]
fn a_model_pinned_to_nominal_fails() {
    let golden = vec![(0.0, 0.7264), (75.0, 0.6099)];
    let pinned = vec![(0.0, 0.668), (75.0, 0.668)];
    let err = TempCase::compare("temp_diode", "out", &golden, &pinned)
        .expect_err("a nominal-pinned model must fail");
    assert!(err.contains("75 °C"), "the failure lands away from nominal: {err}");
}

/// A fixture whose golden side barely moves across the sweep cannot detect a
/// dead temperature path — the discrimination guard rejects it as vacuous.
#[test]
fn a_fixture_with_no_golden_spread_is_rejected_as_vacuous() {
    let flat = vec![(0.0, 0.668_000_1), (75.0, 0.668_000_2)];
    let result = std::panic::catch_unwind(|| TempCase::compare("flat", "out", &flat, &flat));
    let panic = result.expect_err("a golden side inside the tolerance must be rejected");
    let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .unwrap_or("");
    assert!(message.contains("pinned to nominal"), "says why the fixture proves nothing: {message}");
}

/// NGP-05: a temperature miss names the node, the temperature, both values,
/// the delta and the contract's rationale.
#[test]
fn a_temperature_miss_is_actionable() {
    let golden = vec![(0.0, 0.7264), (75.0, 0.6099)];
    let actual = vec![(0.0, 0.7264), (75.0, 0.55)];
    let err = TempCase::compare("temp_diode", "out", &golden, &actual)
        .expect_err("an out-of-tolerance point must fail");
    assert!(err.contains("temp_diode"), "names the circuit: {err}");
    assert!(err.contains("v(out)"), "names the node: {err}");
    assert!(err.contains("75 °C"), "names the temperature: {err}");
    assert!(err.contains("6.099000e-1") && err.contains("5.500000e-1"), "both values: {err}");
    assert!(err.contains("Δ="), "shows the delta: {err}");
    assert!(
        err.contains(ToleranceContract::TEMP.rationale()),
        "carries the contract's rationale: {err}"
    );
    assert!(!err.contains("0 °C: ngspice"), "the agreeing point is not reported: {err}");
}

/// NGP-06: without the binary on PATH the temperature target takes the same
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
