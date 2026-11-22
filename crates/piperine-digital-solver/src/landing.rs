//! Why a look-ahead stops before an instant (abi.md §5 R4 and R6), and which rule decides it.

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
