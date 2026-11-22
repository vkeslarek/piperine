//! Why a look-ahead stops before an instant (abi.md §5 R4 and R6), and which rule decides it.

use crate::net::NetId;
use crate::plan::DigitalPlan;
use crate::run::Firing;
use crate::store::Store;
use piperine_abi::raw::reaction_flag;

/// Why the look-ahead stopped before an instant: the instant is a landing, untouched, and the
/// analog step must end on its time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LandingCause {
    /// A carrier the analog side reads changed at a microstep of the instant.
    AnalogReadChanged,
    /// A firing reaction reads a continuous pin itself.
    ContinuousRead,
    /// A firing reaction is flagged by the static reach analysis only: a statement without `LATER`
    /// on a net that reaches the analog side.
    DeclaredReach,
    /// A firing reaction is `IRREVERSIBLE` (R6).
    Irreversible,
    /// A host waiter's event fired (R7).
    HostWaiter,
}

impl LandingCause {
    /// Whether the instant is a time point of §17.2 rule 2 (R4). An `IRREVERSIBLE` reaction and a
    /// host waiter land the step without making the instant a time point, so `settle` does not
    /// run there.
    pub fn is_time_point(&self) -> bool {
        !matches!(self, LandingCause::Irreversible | LandingCause::HostWaiter)
    }
}

/// Which rule makes a scheduled instant a landing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimePointRule {
    /// Land only where something observable happens: a carrier the analog side reads changed, or a
    /// firing reaction reads a continuous pin. Not yet wired: the engine applies `Declared`.
    #[default]
    Observable,
    /// Land on every instant the static reach analysis marks (R4 as declared).
    Declared,
}

/// The test of a look-ahead: after the functions of one microstep, is the instant a landing, and
/// why? Borrows what it reads, so the engine builds one per look-ahead without copying.
pub struct LandingTest<'a> {
    plan: &'a DigitalPlan,
    waited: &'a [NetId],
}

impl<'a> LandingTest<'a> {
    /// The test over `plan` and the event nets a host waiter listens to (abi.md R7).
    pub fn new(plan: &'a DigitalPlan, waited: &'a [NetId]) -> LandingTest<'a> {
        LandingTest { plan, waited }
    }

    /// The cause that makes the instant a landing after the functions of a microstep, or `None`
    /// when it may run ahead. `firing` are the reactions about to run, `analog_read_changed` says
    /// a carrier the analog side reads changed. The causes are tried in the order of the enum.
    pub fn after_functions(&self, store: &Store, firing: &[Firing], analog_read_changed: bool) -> Option<LandingCause> {
        let analysis = self.plan.analysis();
        let any = |test: &dyn Fn(&Firing) -> bool| firing.iter().any(test);
        if analog_read_changed {
            Some(LandingCause::AnalogReadChanged)
        } else if any(&|f| analysis.samples(f.instance as u32, f.reaction as u32)) {
            Some(LandingCause::ContinuousRead)
        } else if any(&|f| analysis.probes(f.instance as u32, f.reaction as u32)) {
            Some(LandingCause::DeclaredReach)
        } else if any(&|f| self.is_irreversible(f)) {
            Some(LandingCause::Irreversible)
        } else if self.waited.iter().any(|w| store.fired(*w)) {
            Some(LandingCause::HostWaiter)
        } else {
            None
        }
    }

    /// Whether the firing reaction is flagged `IRREVERSIBLE` (R6).
    fn is_irreversible(&self, firing: &Firing) -> bool {
        let shape = self.plan.instances[firing.instance].handle.shape();
        shape.reactions.get(firing.reaction).is_some_and(|r| r.flags & reaction_flag::IRREVERSIBLE != 0)
    }
}
