//! The host's view of an accepted step: the seam a host puts a recorder on, so it can keep the
//! rows it prints and not the steps the solver took (feature `long-run`, LR-01).
//!
//! The owner calls `StepObserver::step` once for the start of a transient and once for every
//! accepted step, right after the sink has been handed every observation up to the step's time
//! and both sides have settled the instant. A rejected or retaken attempt is never observed. The
//! view is by reference: a time, the analog solution and a reader of the discrete nets, nothing
//! allocated per step and nothing of probes, grids or tables.

use super::sides::{AnalogSide, DiscreteSide};

/// What a host receives for each accepted step.
pub trait StepObserver {
    /// One accepted step, final: the sink has every observation up to `step.time()`.
    fn step(&mut self, step: &StepView<'_>);
}

/// A reader of discrete nets by id, over a `DiscreteSide`.
pub(super) trait NetRead {
    fn net(&self, id: u32) -> Option<&[u8]>;
}

pub(super) struct Nets<'a, D: DiscreteSide>(pub(super) &'a D);

impl<D: DiscreteSide> NetRead for Nets<'_, D> {
    fn net(&self, id: u32) -> Option<&[u8]> {
        self.0.net(id)
    }
}

/// One accepted step, by reference: valid for the call of `StepObserver::step` only.
pub struct StepView<'a> {
    time: f64,
    solution: &'a [f64],
    nets: &'a dyn NetRead,
}

impl<'a> StepView<'a> {
    pub(super) fn new(time: f64, solution: &'a [f64], nets: &'a dyn NetRead) -> StepView<'a> {
        StepView { time, solution, nets }
    }

    /// The step's time in seconds.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// The solution at an unknown's index (`AnalogReference::idx`), `None` past the vector.
    pub fn unknown(&self, index: usize) -> Option<f64> {
        self.solution.get(index).copied()
    }

    /// A discrete net's bytes after the step's instant; `None` for a net the side does not know.
    pub fn net(&self, id: u32) -> Option<&[u8]> {
        self.nets.net(id)
    }
}

/// The call both sites make: the observer, if any, sees the sides at `time`.
pub(super) fn observe<D: DiscreteSide, A: AnalogSide>(
    observer: &mut Option<Box<dyn StepObserver>>,
    time: f64,
    discrete: &D,
    analog: &A,
) {
    if let Some(observer) = observer {
        let nets = Nets(discrete);
        observer.step(&StepView::new(time, analog.solution(), &nets));
    }
}
