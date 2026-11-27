//! The dependency queries of a [`Body`]: what a target reads, and whether a branch is affine.

use std::collections::BTreeSet;

use crate::handle::Body;
use crate::model::EventSource;

use super::walk::Walk;
use super::{BodyDeps, BranchIndex, Deps, Sources, Target};

impl<'a> Body<'a> {
    /// What `target` reads, through locals and function calls. A target out of range of the
    /// body panics, as a handle given an id out of range does.
    pub fn dependencies(&self, target: Target) -> Deps {
        self.sources_of(target).to_deps()
    }

    /// The carriers `target` reads, by index (`Body::carriers`), sorted, through locals and
    /// function calls: the discrete part of its dependency set.
    pub fn carrier_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).carriers.iter().copied().collect()
    }

    /// The branches whose flow `target` reads (`Expr::Flow`), by index (`Body::branches`),
    /// sorted, through locals and function calls: the flow part of its dependency set.
    pub fn flow_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).flows.iter().copied().collect()
    }

    /// The constant elements of array carriers whose value `target` reads (`mem[3]`), as
    /// `(carrier, element)`, sorted: a run-time index reads the whole carrier (`carrier_reads`).
    pub fn element_reads(&self, target: Target) -> Vec<(u32, u64)> {
        self.sources_of(target).elements.iter().copied().collect()
    }

    /// The constant elements whose previous value `target` reads (`mem[3].prev()`), sorted.
    pub fn element_previous_reads(&self, target: Target) -> Vec<(u32, u64)> {
        self.sources_of(target).element_previous.iter().copied().collect()
    }

    /// The constant elements whose `changed` byte `target` reads (`mem[3].changed()`), sorted.
    pub fn element_changed_reads(&self, target: Target) -> Vec<(u32, u64)> {
        self.sources_of(target).element_changed.iter().copied().collect()
    }

    /// The array carriers whose `changed` byte `target` reads at a run-time element, sorted.
    pub fn whole_changed_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).whole_changed.iter().copied().collect()
    }

    /// The carriers whose previous value (`Expr::Previous`) `target` reads, by index
    /// (`Body::carriers`), sorted, through locals and function calls. A read of a previous value
    /// is not in `carrier_reads`: it reads the carrier's `PREVIOUS` and not its value.
    pub fn previous_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).previous.iter().copied().collect()
    }

    /// The resolved carriers whose `others()` (`Expr::Others`) `target` reads, by index (`Body::carriers`), sorted,
    /// through locals and function calls.
    pub fn others_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).others.iter().copied().collect()
    }

    /// The carriers `target` reads as the drives of an `Expr::Resolve`, by index (`Body::carriers`), sorted: a resolver
    /// body's drive carriers, whose pins hold the driven type followed by the `driving` byte.
    pub fn resolve_drive_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).resolve_drives.iter().copied().collect()
    }

    /// Every dependency query of the body: each branch's two parts, each site, each param, and each drive, event,
    /// reaction, `bound_step` site and `cross` tolerance.
    pub fn targets(&self) -> Vec<Target> {
        let mut targets: Vec<Target> = Vec::new();
        for i in 0..self.branches().len() as u32 {
            targets.extend([Target::Flow(i), Target::Charge(i)]);
        }
        targets.extend((0..self.limits().len() as u32).map(Target::Limit));
        targets.extend((0..self.stims().len() as u32).map(Target::Stim));
        targets.extend((0..self.params().len() as u32).map(Target::Check));
        for i in 0..self.drives().len() as u32 {
            targets.extend([Target::Drive(i), Target::DriveAfter(i)]);
        }
        for i in 0..self.events().len() as u32 {
            targets.extend([Target::Guard(i), Target::Cross(i), Target::At(i)]);
        }
        targets.extend((0..self.reactions().len() as u32).map(Target::Reaction));
        let bound_steps = self.analog().unwrap_or(&[]).iter().filter_map(|s| match self.ir().stmt(*s) {
            crate::model::Stmt::BoundStep { site, .. } => Some(*site),
            _ => None,
        });
        targets.extend(bound_steps.map(Target::BoundStep));
        targets.extend((0..self.events().len() as u32).flat_map(|k| [Target::Ttol(k), Target::Etol(k)]));
        targets
    }

    /// Whether any query of the body reads `others()` of resolved carrier `carrier` (`Expr::Others`).
    pub fn reads_others(&self, carrier: u32) -> bool {
        self.targets().into_iter().any(|target| self.others_reads(target).contains(&carrier))
    }

    /// The carriers some query of the body reads as the drives of a `resolve`, by index, sorted: the pins that hold a
    /// drive net's layout.
    pub fn resolve_drive_carriers(&self) -> Vec<u32> {
        let mut carriers: Vec<u32> = self.targets().into_iter().flat_map(|target| self.resolve_drive_reads(target)).collect();
        carriers.sort_unstable();
        carriers.dedup();
        carriers
    }

    /// The params whose previous value (`Expr::ParamPrevious`) `target` reads, by index
    /// (`Body::params`), sorted, through locals and function calls.
    pub fn param_previous_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).param_previous.iter().copied().collect()
    }

    /// The params whose `changed` event (`EventSource::ParamChanged`) `target` reads through
    /// `Expr::Fired`, by index (`Body::params`), sorted and without repeats.
    /// Kept for a host that lists the param changes a body reacts to (`tests/structure.rs`).
    pub fn param_changes(&self, target: Target) -> Vec<u32> {
        let events = self.events();
        let params: BTreeSet<u32> = self
            .fired_reads(target)
            .into_iter()
            .filter_map(|e| match events.get(e as usize).map(|event| &event.source) {
                Some(EventSource::ParamChanged(p)) => Some(*p),
                _ => None,
            })
            .collect();
        params.into_iter().collect()
    }

    /// The derivative sites whose rate (`Expr::Derivative`) `target` reads, by index
    /// (`Body::derivatives`), sorted, through locals and function calls.
    pub fn derivative_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).derivatives.iter().copied().collect()
    }

    /// The session parameters `target` reads (`Expr::SessionParameter`), by index
    /// (`Body::session_parameters`), sorted, through locals and function calls.
    pub fn session_parameter_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).session_parameters.iter().copied().collect()
    }

    /// The noise sources whose value (`Expr::Noise`) `target` reads, by index
    /// (`Body::noise_sources`), sorted, through locals and function calls.
    pub fn noise_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).noises.iter().copied().collect()
    }

    /// The delay sites whose output (`Expr::Delay`) `target` reads, by index (`Body::delays`),
    /// sorted, through locals and function calls.
    pub fn delay_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).delays.iter().copied().collect()
    }

    /// The events whose `fired` byte `target` reads (`Expr::Fired`), by index (`Body::events`),
    /// sorted, through locals and function calls.
    pub fn fired_reads(&self, target: Target) -> Vec<u32> {
        self.sources_of(target).fired.iter().copied().collect()
    }

    /// The sources of `target`, by the memo of the body walk.
    fn sources_of(&self, target: Target) -> &'a Sources {
        let deps = self.deps();
        let (list, i, what) = match target {
            Target::Flow(i) => (&deps.flow, i, "branch"),
            Target::Charge(i) => (&deps.charge, i, "branch"),
            Target::Limit(i) => (&deps.limits, i, "limit site"),
            Target::Stim(i) => (&deps.stims, i, "ac_stim site"),
            Target::Check(i) => (&deps.checks, i, "param"),
            Target::Drive(i) => (&deps.drives, i, "drive"),
            Target::DriveAfter(i) => (&deps.drive_afters, i, "drive"),
            Target::Guard(i) => (&deps.guards, i, "event"),
            Target::Cross(i) => (&deps.crosses, i, "event"),
            Target::At(i) => (&deps.ats, i, "event"),
            Target::Reaction(i) => (&deps.reactions, i, "reaction"),
            Target::BoundStep(i) => (&deps.caps, i, "bound_step site"),
            Target::Ttol(i) => (&deps.ttols, i, "event"),
            Target::Etol(i) => (&deps.etols, i, "event"),
            Target::Derivative(i) => (&deps.derivative_states, i, "derivative site"),
            Target::DerivativeAbstol(i) => (&deps.derivative_abstols, i, "derivative site"),
            Target::DelayInput(i) => (&deps.delay_inputs, i, "delay site"),
            Target::DelayTime(i) => (&deps.delay_times, i, "delay site"),
            Target::DelayMaximum(i) => (&deps.delay_maxima, i, "delay site"),
            Target::NoiseDensity(i) => (&deps.noise_densities, i, "noise source"),
            Target::Trace(i) => (&deps.traces, i, "trace site"),
        };
        match list.get(i as usize) {
            Some(sources) => sources,
            None => panic!("body {}: {what} {i} out of range ({} in the body)", self.name(), list.len()),
        }
    }

    /// Whether every direct addend contributed to `branch` is affine in the node potentials: every
    /// probe reaches the sum only through `+`, `-`, unary `-`, and `*` or `/` by an expression
    /// that reads no probe and no `limit` site. A branch with no direct addend is affine.
    pub fn is_affine(&self, branch: BranchIndex) -> bool {
        let deps = self.deps();
        match deps.affine.get(branch as usize) {
            Some(affine) => *affine,
            None => panic!("body {}: branch {branch} out of range ({} in the body)", self.name(), deps.affine.len()),
        }
    }

    /// The memoized dependency walk of this body: made on first use.
    fn deps(&self) -> &'a BodyDeps {
        let ir = self.ir();
        let data = &ir.bodies[self.id().index()];
        data.deps.0.get_or_init(|| Walk::body(ir, data))
    }
}
