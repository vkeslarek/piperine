//! The transient: a run in time, recorded step by step.

use std::cell::RefCell;
use std::rc::Rc;

use piperine_solver::prelude::Solver;

use super::Session;
use super::run::AnalysisRun;
use crate::failure::Failure;
use crate::host::HostAction;
use crate::probe::Probe;
use crate::recorder::{Recorder, Tap};
use crate::statistics::TransientStatistics;
use crate::table::Table;

/// Runs a `transient` request.
pub(super) struct TransientRunner<'s> {
    run: AnalysisRun<'s>,
}

impl<'s> TransientRunner<'s> {
    /// A runner over `run`.
    pub(super) fn new(run: AnalysisRun<'s>) -> Self {
        TransientRunner { run }
    }

    /// Runs to `stop`, keeping what is at or after `from`, on the grid `print_step` when given.
    pub(super) fn solve(self, stop: f64, max_step: Option<f64>, print_step: Option<f64>, from: Option<f64>) -> Result<Table, Failure> {
        let AnalysisRun { session, circuit, mut delivery, design, held, timed } = self.run;
        // every printed signal finds its place in a step before the first one is taken
        let columns = session.probes.iter().map(|probe| probe.column_with(&circuit, &held)).collect::<Result<Vec<_>, Failure>>()?;
        let labels = session.probes.iter().map(Probe::label).collect();
        let mut recorder = Recorder::new(columns, labels, from.unwrap_or(0.0));
        if let Some(step) = print_step {
            recorder = recorder.with_grid(step, stop);
        }
        let recorder = Rc::new(RefCell::new(recorder));
        let finished = delivery.finished.clone();
        delivery.recorder = Some(recorder.clone());
        let options = Session::transient_options(stop, max_step);
        let options = match session.spec.options.max_steps {
            Some(budget) => options.with_max_steps(budget as usize),
            None => options,
        };
        let mut solver = Solver::new(circuit)
            .with_context(session.context.clone())
            .with_tran_opts(options)
            .with_sink(delivery)
            .with_step_observer(Box::new(Tap(recorder.clone())))
            .build();
        let result = solver
            .tran()
            .and_then(|mut tran| {
                for action in &timed {
                    match action {
                        HostAction::Drive { net, value, at } => tran.schedule_write(*at, net.clone(), value.clone()),
                        HostAction::Set { path, value, at } => tran.schedule_set(*at, path.instance.clone(), path.param.clone(), value.clone()),
                    }
                }
                tran.solve()
            })
            .map_err(|error| Failure::from_fault("transient", error, recorder.borrow().reached(), &design))?;
        session.statistics = Some(TransientStatistics::new(result.statistics().clone(), result.stats.clone()));
        Self::table(solver, recorder, finished.borrow().map_or(stop, |at| at.min(stop)))
    }

    /// The table of a finished run. A run that stopped short has no table to give (CLI-05 AC 11):
    /// how far it got is the last step the observer saw, which a design with no analog unknown
    /// reaches too (its result holds no analog accept, HII-17). A model that wrote `finish` ended
    /// the run at that instant: it is the run's end (FIX-02 AC 1), and the grid stops there.
    fn table(solver: Solver, recorder: Rc<RefCell<Recorder>>, end: f64) -> Result<Table, Failure> {
        recorder.borrow_mut().end_at(end);
        let reached = recorder.borrow().reached();
        if reached.is_none_or(|last| last < end * (1.0 - 1e-9)) {
            let at = reached.map_or_else(|| "no step".to_string(), |t| format!("the last step at {t:e}"));
            return Err(Failure::Backend(format!("the transient ended with {at}, before its stop at {end:e}")));
        }
        // the solver held the observers; with it gone the recorder has one owner
        drop(solver);
        let recorder = Rc::try_unwrap(recorder).map_err(|_| Failure::Backend("the run's recorder is still shared after the run".into()))?;
        recorder.into_inner().finish()
    }
}
