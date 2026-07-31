//! `SequencerControl` (T19/T21/T22): the mutable handle a
//! [`super::SequencerProgram`] drives side effects through — the Rust
//! mirror of `t` in `ideal.md`'s `def run(self, t):` examples, minus the
//! drive/wait verbs themselves (those are free functions in
//! [`super::drive`] taking `&mut SequencerControl` explicitly, since a
//! program's analog drive sites and install sites are its own struct
//! fields, not something [`super::Sequencer`] owns on its behalf).

use piperine_solver::abi::{DigitalNet, LogicValue, ValidationSeverity};

use piperine_signal::events::Prediction;

use super::findings::Findings;

#[derive(Default)]
pub struct SequencerControl {
    digital_drives: Vec<(DigitalNet, LogicValue, f64)>,
    pub(super) findings: Findings,
    /// The tier-2 crossing prediction that satisfied the most recent
    /// `wait(above(...))` (T24) — read by the program from within
    /// `resume` to report the wake's honest error bound.
    last_crossing: Option<Prediction>,
}

impl SequencerControl {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Queue a digital drive: forwarded to the real `EventSink` once this
    /// accept's `accept_timestep` returns (never touches the MNA — T21,
    /// DVT-33).
    pub fn drive_digital(&mut self, net: DigitalNet, value: LogicValue, delay: f64) {
        self.digital_drives.push((net, value, delay));
    }

    /// The findings buffer `expect`/`warn` (T22) and internal-net drive
    /// notices (T21) record into.
    pub fn findings(&mut self) -> &mut Findings {
        &mut self.findings
    }

    pub(super) fn take_digital_drives(&mut self) -> Vec<(DigitalNet, LogicValue, f64)> {
        std::mem::take(&mut self.digital_drives)
    }

    /// The [`Prediction`] that satisfied the most recent crossing wait —
    /// `None` when the sequencer was not woken by one. `t.wait(above(...))`
    /// reads this from within `resume` to report the wake's error bound;
    /// the sequencer never claims an exactness it does not have.
    pub fn last_crossing(&self) -> Option<Prediction> {
        self.last_crossing
    }

    pub(super) fn set_last_crossing(&mut self, pred: Prediction) {
        self.last_crossing = Some(pred);
    }

    /// `t.expect(cond, msg)` (T22, DVT-29): a failing `cond` records an
    /// `Error` finding with `t`/`instance` provenance — never a print.
    /// Posture (strict aborts on the first `Error`, collect records all) is
    /// enforced by the existing `dv-core` channel
    /// (`CircuitInstance::consume_validation_reports`) once this finding is
    /// polled — `expect` itself only ever records; it never decides to
    /// abort.
    pub fn expect(&mut self, cond: bool, msg: impl Into<String>, t: f64, instance: &str) {
        if !cond {
            self.findings.push(ValidationSeverity::Error, "expect", msg, 0.0, t, instance);
        }
    }

    /// `t.warn(msg)`: always records a `Warning`, which never aborts under
    /// any posture (DVC-15's existing rule).
    pub fn warn(&mut self, msg: impl Into<String>, t: f64, instance: &str) {
        self.findings.push(ValidationSeverity::Warning, "warn", msg, 0.0, t, instance);
    }

    /// `expect` against a [`Measure`](piperine_signal::measure::handle::Measure)'s
    /// **lower** bound (`ideal.md` §4.3): compares `value - error_bound`
    /// against `threshold`, not the point estimate alone — a borderline
    /// pass whose bound-adjusted worst case would fail the threshold is
    /// still an `expect` failure.
    pub fn expect_measure_at_least<K: piperine_signal::measure::MeasureKind>(
        &mut self,
        measure: &piperine_signal::measure::handle::Measure<K>,
        threshold: f64,
        msg: impl Into<String>,
        t: f64,
        instance: &str,
    ) -> Result<(), piperine_signal::measure::handle::MeasureError> {
        let value = measure.value()?;
        let bound = measure.error_bound()?;
        self.expect(value - bound >= threshold, msg, t, instance);
        Ok(())
    }

    /// The upper-bound twin of [`Self::expect_measure_at_least`]: compares
    /// `value + error_bound` against `threshold`.
    pub fn expect_measure_at_most<K: piperine_signal::measure::MeasureKind>(
        &mut self,
        measure: &piperine_signal::measure::handle::Measure<K>,
        threshold: f64,
        msg: impl Into<String>,
        t: f64,
        instance: &str,
    ) -> Result<(), piperine_signal::measure::handle::MeasureError> {
        let value = measure.value()?;
        let bound = measure.error_bound()?;
        self.expect(value + bound <= threshold, msg, t, instance);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expect_true_records_nothing() {
        let mut ctl = SequencerControl::new();
        ctl.expect(true, "fine", 0.0, "seq0");
        assert!(ctl.findings().to_report().is_none());
    }

    #[test]
    fn expect_false_records_an_error_with_time_and_instance() {
        let mut ctl = SequencerControl::new();
        ctl.expect(false, "vout too low", 5e-6, "seq0");
        let report = ctl.findings().to_report().expect("a failing expect must record");
        let f = &report.findings()[0];
        assert_eq!(f.severity, ValidationSeverity::Error);
        assert_eq!(f.time, 5e-6);
        assert_eq!(f.instance_path, "seq0");
        assert_eq!(f.message, "vout too low");
    }

    #[test]
    fn warn_always_records_a_warning_never_an_error() {
        let mut ctl = SequencerControl::new();
        ctl.warn("just a note", 1e-6, "seq0");
        let report = ctl.findings().to_report().unwrap();
        assert_eq!(report.findings()[0].severity, ValidationSeverity::Warning);
    }

    /// DVT-29 / `ideal.md` §4.3: `expect` against a `Measure` compares
    /// against the error bound, not the point estimate. A peak of 5.0 with
    /// a published bound of 0.3 passes a naive `value >= 4.8` check, but
    /// its bound-adjusted worst case (4.7) does not — `expect_measure_at_least`
    /// must catch the case the naive check would silently let through.
    #[test]
    fn expect_measure_at_least_uses_the_error_bound_not_the_point_estimate() {
        use piperine_signal::measure::handle::Measure;
        use piperine_signal::measure::kinds::Peak;

        let mut peak = Measure::arm(Peak::new());
        peak.feed(0.0, 5.0, 0.3);
        assert_eq!(peak.value().unwrap(), 5.0);
        assert_eq!(peak.error_bound().unwrap(), 0.3);

        // The naive check the bound-aware form must NOT reduce to:
        assert!(peak.value().unwrap() >= 4.8, "sanity: the naive point-estimate check would pass");

        let mut ctl = SequencerControl::new();
        ctl.expect_measure_at_least(&peak, 4.8, "peak must clear 4.8 with margin", 3e-6, "seq0").unwrap();
        let report = ctl.findings().to_report().expect(
            "the bound-adjusted worst case (5.0 - 0.3 = 4.7) is below 4.8 — expect_measure_at_least must fail",
        );
        assert_eq!(report.findings()[0].severity, ValidationSeverity::Error);
    }

    /// The contrast case: a threshold the bound-adjusted worst case still
    /// clears records nothing.
    #[test]
    fn expect_measure_at_least_passes_when_the_bound_adjusted_value_clears_the_threshold() {
        use piperine_signal::measure::handle::Measure;
        use piperine_signal::measure::kinds::Peak;

        let mut peak = Measure::arm(Peak::new());
        peak.feed(0.0, 5.0, 0.3);

        let mut ctl = SequencerControl::new();
        ctl.expect_measure_at_least(&peak, 4.5, "peak must clear 4.5", 3e-6, "seq0").unwrap();
        assert!(ctl.findings().to_report().is_none(), "4.7 (bound-adjusted) clears 4.5 — no finding");
    }
}
