//! What a dependency query is asked about, and the indexes it names.

/// The index of a branch in its body (`Body::branches`).
pub type BranchIndex = u32;
/// The index of a `limit` or `ac_stim` site in its body's list.
pub type SiteIndex = u32;
/// The index of a param in its body (`Body::params`).
pub type ParamIndex = u32;

/// What a dependency query is asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The direct addends contributed to a branch, whatever its quantity: the resistive part.
    Flow(BranchIndex),
    /// The `ddt` addends contributed to a branch: the reactive part.
    Charge(BranchIndex),
    /// A `limit` site: its input, its function and its seed.
    Limit(SiteIndex),
    /// An `ac_stim` site: its magnitude and phase.
    Stim(SiteIndex),
    /// The `where` expression of a param; empty when the param has none.
    Check(ParamIndex),
    /// The value of a drive (`Body::drives`), by index.
    Drive(u32),
    /// The delay of a drive; empty when the drive has no `after`.
    DriveAfter(u32),
    /// The guard of an event (`Body::events`); empty when it has none.
    Guard(u32),
    /// The crossed expression of a `Cross` event; empty for a `Changed` or an `At` one.
    Cross(u32),
    /// The time argument of an `At` event (`Body::events`); empty for any other event.
    At(u32),
    /// Everything a reaction (`Body::reactions`) reads.
    Reaction(u32),
    /// The step of a `bound_step` site (`Stmt::BoundStep::site`); empty for a site the body
    /// does not have.
    BoundStep(u32),
    /// The `ttol` of a `Cross` event; empty when the event has none or is not a `Cross`.
    Ttol(u32),
    /// The `etol` of a `Cross` event; empty when the event has none or is not a `Cross`.
    Etol(u32),
    /// The state of a derivative site (`Body::derivatives`): the quantity it differentiates.
    Derivative(u32),
    /// The `abstol` of a derivative site; empty when the site has none.
    DerivativeAbstol(u32),
    /// The delayed quantity of a delay site (`Body::delays`).
    DelayInput(u32),
    /// The delay time of a delay site.
    DelayTime(u32),
    /// The `max` of a delay site; empty when the site has none.
    DelayMaximum(u32),
    /// The power density of a noise source (`Body::noise_sources`).
    NoiseDensity(u32),
    /// Everything a trace site (`Body::traces`) writes: the values of its fields.
    Trace(u32),
}
