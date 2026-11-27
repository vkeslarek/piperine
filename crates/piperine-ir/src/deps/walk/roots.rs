//! The roots of a body's walk: its param checks, drives, events, sites and reactions, and the
//! analog block. Each root is walked in a scope of its own, the analog block in one scope.

use super::{Scope, Task, Walk};
use crate::ids::ExprId;
use crate::model::{BodyData, EventSource, Ir};

use super::super::{BodyDeps, Sources};

/// The sources of the drives of a body: each value, and each `after` delay.
struct DriveSources {
    values: Vec<Sources>,
    afters: Vec<Sources>,
}

/// The sources of the events of a body: what each guard, `cross`, `at` and tolerance reads.
struct EventSources {
    guards: Vec<Sources>,
    time_tolerances: Vec<Sources>,
    error_tolerances: Vec<Sources>,
    crosses: Vec<Sources>,
    times: Vec<Sources>,
}

impl<'a> Walk<'a> {
    /// The dependency walk of `data`: one entry per branch, site, param, drive, event and reaction.
    pub(in crate::deps) fn body(ir: &'a Ir, data: &'a BodyData) -> BodyDeps {
        let mut walk = Walk::empty(ir, data);
        let owner = format!("body {}", data.name);
        let checks = walk.param_checks(data, &owner);
        let drives = walk.drive_sources(data, &owner);
        let events = walk.event_sources(data, &owner);
        let derivative_states: Vec<Sources> = data.derivatives.iter().map(|d| walk.root_sources(data, &owner, Some(d.state))).collect();
        let derivative_abstols: Vec<Sources> = data.derivatives.iter().map(|d| walk.root_sources(data, &owner, d.abstol)).collect();
        let delay_inputs: Vec<Sources> = data.delays.iter().map(|d| walk.root_sources(data, &owner, Some(d.input))).collect();
        let delay_times: Vec<Sources> = data.delays.iter().map(|d| walk.root_sources(data, &owner, Some(d.time))).collect();
        let delay_maxima: Vec<Sources> = data.delays.iter().map(|d| walk.root_sources(data, &owner, d.maximum)).collect();
        let noise_densities: Vec<Sources> = data.noise_sources.iter().map(|n| walk.root_sources(data, &owner, Some(n.density))).collect();
        let reactions = walk.reaction_sources(data, &owner);
        walk.limit_conditions(data, &owner);
        walk.analog_block(data, owner);
        BodyDeps {
            flow: walk.flow,
            charge: walk.charge,
            affine: walk.affine,
            limits: walk.limits,
            stims: walk.stims,
            checks,
            drives: drives.values,
            drive_afters: drives.afters,
            guards: events.guards,
            crosses: events.crosses,
            ats: events.times,
            reactions,
            caps: walk.caps,
            ttols: events.time_tolerances,
            etols: events.error_tolerances,
            derivative_states,
            derivative_abstols,
            delay_inputs,
            delay_times,
            delay_maxima,
            noise_densities,
            traces: walk.traces,
        }
    }

    /// A walk with no scope yet, its per-branch and per-site tables sized for `data`.
    fn empty(ir: &'a Ir, data: &BodyData) -> Self {
        let branches = data.branches.len();
        Walk {
            ir,
            calls: Vec::new(),
            scopes: Vec::new(),
            returns: std::collections::HashMap::new(),
            values: Vec::new(),
            flow: vec![Sources::default(); branches],
            charge: vec![Sources::default(); branches],
            affine: vec![true; branches],
            limits: vec![Sources::default(); data.limits.len()],
            stims: vec![Sources::default(); data.stims.len()],
            reads: None,
            caps: Vec::new(),
            traces: vec![Sources::default(); data.traces.len()],
            held: std::collections::HashMap::new(),
            enclosing: Vec::new(),
            frames: Vec::new(),
            recursing: std::collections::HashSet::new(),
            approximate: std::collections::HashMap::new(),
        }
    }

    /// The sources `root` reads, walked in a fresh scope of `data`'s body; none for an absent root.
    fn root_sources(&mut self, data: &BodyData, owner: &str, root: Option<ExprId>) -> Sources {
        match root {
            Some(expression) => {
                self.scopes = vec![Scope::new(owner.to_string(), true, data.locals.len())];
                self.run(Task::Expr(expression));
                self.pop().sources
            }
            None => Sources::default(),
        }
    }

    /// What each param's `where` check reads.
    fn param_checks(&mut self, data: &BodyData, owner: &str) -> Vec<Sources> {
        data.params.iter().map(|param| self.root_sources(data, owner, param.check)).collect()
    }

    /// What each drive's value and `after` delay read.
    fn drive_sources(&mut self, data: &BodyData, owner: &str) -> DriveSources {
        let mut drives = DriveSources { values: Vec::with_capacity(data.drives.len()), afters: Vec::with_capacity(data.drives.len()) };
        for drive in &data.drives {
            drives.values.push(self.root_sources(data, owner, Some(drive.value)));
            drives.afters.push(self.root_sources(data, owner, drive.after));
        }
        drives
    }

    /// What each event's `cross`, tolerances, `at` and guard read.
    fn event_sources(&mut self, data: &BodyData, owner: &str) -> EventSources {
        let count = data.events.len();
        let mut events = EventSources { guards: Vec::with_capacity(count), time_tolerances: Vec::with_capacity(count), error_tolerances: Vec::with_capacity(count), crosses: Vec::with_capacity(count), times: Vec::with_capacity(count) };
        for event in &data.events {
            let cross = match event.source {
                EventSource::Cross { e, .. } => Some(e),
                EventSource::Changed(_) | EventSource::At(_) | EventSource::ParamChanged(_) | EventSource::Settle | EventSource::ElementChanged { .. } => None,
            };
            events.crosses.push(self.root_sources(data, owner, cross));
            let ttol = match event.source {
                EventSource::Cross { ttol, .. } => ttol,
                EventSource::Changed(_) | EventSource::At(_) | EventSource::ParamChanged(_) | EventSource::Settle | EventSource::ElementChanged { .. } => None,
            };
            events.time_tolerances.push(self.root_sources(data, owner, ttol));
            let etol = match event.source {
                EventSource::Cross { etol, .. } => etol,
                EventSource::Changed(_) | EventSource::At(_) | EventSource::ParamChanged(_) | EventSource::Settle | EventSource::ElementChanged { .. } => None,
            };
            events.error_tolerances.push(self.root_sources(data, owner, etol));
            let at = match event.source {
                EventSource::At(e) => Some(e),
                EventSource::Changed(_) | EventSource::Cross { .. } | EventSource::ParamChanged(_) | EventSource::Settle | EventSource::ElementChanged { .. } => None,
            };
            events.times.push(self.root_sources(data, owner, at));
            events.guards.push(self.root_sources(data, owner, event.guard));
        }
        events
    }

    /// Every source of every expression each reaction's statements evaluate.
    fn reaction_sources(&mut self, data: &BodyData, owner: &str) -> Vec<Sources> {
        let mut reactions = Vec::with_capacity(data.reactions.len());
        for reaction in &data.reactions {
            self.scopes = vec![Scope::new(owner.to_string(), true, reaction.locals.len())];
            self.reads = Some(Sources::default());
            for &stmt in &reaction.stmts {
                self.run(Task::Stmt(stmt));
            }
            reactions.push(self.reads.take().unwrap_or_default());
        }
        reactions
    }

    /// What enables a `limit` under a condition is read by its site, as its input is.
    fn limit_conditions(&mut self, data: &BodyData, owner: &str) {
        for (i, site) in data.limits.iter().enumerate() {
            let Some(enabled) = site.enabled else { continue };
            let sources = self.root_sources(data, owner, Some(enabled));
            self.limits[i].join(&sources);
        }
    }

    /// The statements of the `analog` block, walked in one scope.
    fn analog_block(&mut self, data: &BodyData, owner: String) {
        if let Some(analog) = &data.analog {
            self.scopes = vec![Scope::new(owner, true, data.locals.len())];
            for &stmt in analog {
                self.run(Task::Stmt(stmt));
            }
        }
    }
}
