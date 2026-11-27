//! The DC sweep: one operating point per value of a param.

use super::Session;
use super::run::AnalysisRun;
use crate::failure::Failure;
use crate::params::Derive;
use crate::table::Table;

/// Runs a `dc` request.
pub(super) struct DcSweepRunner<'s> {
    run: AnalysisRun<'s>,
}

impl<'s> DcSweepRunner<'s> {
    /// A runner over `run`.
    pub(super) fn new(run: AnalysisRun<'s>) -> Self {
        DcSweepRunner { run }
    }

    /// Sets the swept param to `from + k*step` for every k while the point is not past `to`, and
    /// solves the operating point at each.
    pub(super) fn solve(self, from: f64, to: f64, step: f64) -> Result<Table, Failure> {
        let AnalysisRun { session, mut circuit, design, .. } = self.run;
        let swept = session.swept.as_ref().ok_or_else(|| Failure::Backend("a dc run resolved no param to sweep".into()))?;
        // the tolerance on k absorbs the rounding of `(to - from) / step` (a whole number of steps
        // that lands a hair under)
        let count = ((to - from) / step + 1e-9).floor() as usize + 1;
        let mut points = Vec::with_capacity(count);
        for k in 0..count {
            let value = from + k as f64 * step;
            Session::set_param(&mut session.params, session.compiled.as_ref().map(|c| c as &dyn Derive), &mut circuit, swept, value)?;
            let result = circuit
                .dc(session.context.clone())
                // the settle commits of a point land at the next one (§15 S6)
                .and_then(|dc| dc.followed_by_another_point(k + 1 < count).solve())
                .map_err(|error| Failure::from_fault("dc", error, None, &design))?;
            let held = session.held_params(&circuit, &[]);
            let net = |path: &str| Session::discrete(&circuit, path).or_else(|| Session::held_bytes(&held, path));
            let term = |instance: &str, name: &str| circuit.bound().analog.term_value(instance, name);
            points.push((value, Table::point(&session.probes, &result, &net, &term)?));
        }
        Ok(Table::sweep(&swept.label(), &session.probes, points))
    }
}
