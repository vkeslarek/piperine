//! Reactions and the functions of the discrete half: what a reaction runs, its statements, and the `drive:*`, `event:*` and `at:*` functions.

use piperine_abi::raw::{GROUND, Statement, part, statement_flag};
use piperine_abi::shape::{ReactionSpec, ShapeBuf};
use piperine_ir::{Body, EventSource, Expr, ExprId, Ir, Origin, Projection, Stmt, Target as Query, ValueType};

use crate::array::ArrayPin;
use crate::pin::PinLayout;
use crate::digital::{Digital, RecordLayout};
use crate::plan::{BodyPlan, FunctionPlan, Mode, Pins, ReadSet, Source, Target};

/// What a reaction of the shape runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReactionMode {
    /// Reaction `k` of the body.
    Reaction(u32),
    /// The commit of drive `k`'s undelayed value after its delay (RS-12 AC 5).
    Delayed(u32),
    /// The arming of `at` event `k`: a commit to its pin due at its time (msl-digital A3).
    Arm(u32),
}

/// What one reaction of the shape does, parallel to the shape's reaction.
#[derive(Clone, Debug, PartialEq)]
pub struct ReactionPlan {
    pub mode: ReactionMode,
    /// The event pins it listens to, parallel to the reaction's `on`.
    pub on: Vec<u32>,
    /// Parallel to the reaction's reads, each a `VALUE` of a pin.
    pub reads: Vec<Source>,
    /// Parallel to the reaction's statements.
    pub statements: Vec<StatementPlan>,
}

/// One statement of a reaction: the pin it commits to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatementPlan {
    /// `Target::Carrier` for a commit; `Target::EventPin` or `Target::Timer` for the statement
    /// of an arming reaction.
    pub target: Target,
    /// `Some`: the statement is `LATER` with this `min_after`, in seconds.
    pub min_after: Option<f64>,
    /// The statement is `REPLACE` (abi.md C3): a commit through it removes every pending one.
    pub replace: bool,
    /// The byte range `(offset, len)` the statement commits inside the pin, or inside one element of an array pin
    /// (`ArrayPin::range`); `None`: the whole pin, or the whole element.
    pub range: Option<(u32, u32)>,
}

impl StatementPlan {
    /// The statement of a commit to `carrier` delayed by `after`.
    pub fn commit(ir: &Ir, carrier: u32, after: Option<ExprId>) -> StatementPlan {
        StatementPlan { target: Target::Carrier(carrier), min_after: after.and_then(|a| Self::min_after(ir, a)), replace: false, range: None }
    }

    /// The statement of a commit to `carrier` (an element of type `ty`, an array of `count` of them when `Some`) delayed
    /// by `after`, through the steps `path` into its value: over the whole element, or over the byte range of the field
    /// or value-array element the path names (abi.md §3).
    pub fn commit_through(ir: &Ir, carrier: u32, ty: &ValueType, count: Option<u64>, after: Option<ExprId>, path: &[Projection]) -> StatementPlan {
        let range = if count.is_some() || !path.is_empty() { ArrayPin::range(ir, ty, path) } else { None };
        StatementPlan { range, ..Self::commit(ir, carrier, after) }
    }

    /// The statement of an emit site of stream `s`: the whole of its `RECORD` pin, due at the
    /// next microstep.
    pub fn record(s: u32) -> StatementPlan {
        StatementPlan { target: Target::Record(s), min_after: None, replace: false, range: None }
    }

    /// The statements of reaction `k` of `body`, one per commit and per emit site, in ABI order.
    pub fn of_reaction(body: Body<'_>, k: u32) -> Vec<StatementPlan> {
        let ir = body.ir();
        body.reaction_statements(k)
            .into_iter()
            .filter_map(|id| match ir.stmt(id) {
                Stmt::Commit { carrier, after, target, .. } => {
                    let held = body.carriers().get(*carrier as usize)?;
                    let path = target.as_ref().map_or(&[][..], |t| t.path.as_slice());
                    let statement = StatementPlan::commit_through(ir, *carrier, &held.ty, held.count, *after, path);
                    // a commit of a resolved carrier writes its drive pin
                    Some(if Digital::is_resolved(body, *carrier) { StatementPlan { target: Target::ResolvedDrive(*carrier), ..statement } } else { statement })
                }
                Stmt::Emit { stream, .. } => Some(StatementPlan::record(*stream)),
                Stmt::Let { .. } | Stmt::Expr(_) | Stmt::Assign { .. } | Stmt::Contribute { .. } | Stmt::BoundStep { .. } | Stmt::Trace { .. } => None,
            })
            .collect()
    }

    /// The statement of the arming reaction of `at` event `k` of `body`: `REPLACE` on the pin the
    /// timer fires.
    pub fn arm(body: Body<'_>, k: u32) -> StatementPlan {
        let guarded = body.events().get(k as usize).is_some_and(|e| e.guard.is_some());
        StatementPlan { target: if guarded { Target::Timer(k) } else { Target::EventPin(k) }, min_after: None, replace: true, range: None }
    }

    /// The `min_after` of a commit delayed by `after`: the delay itself when it is a real
    /// literal greater than 0, else none (abi.md §6 C1).
    pub fn min_after(ir: &Ir, after: ExprId) -> Option<f64> {
        match ir.expr(after) {
            Expr::Real(r) if r.get() > 0.0 => Some(r.get()),
            _ => None,
        }
    }
}

impl Digital {
    /// Declares the discrete functions of `body`: `drive:*`, `event:*`, `drive:*.changed`,
    /// `at:*.*`.
    pub(crate) fn functions(s: &mut ShapeBuf, pins: &Pins, body: Body<'_>) -> Vec<FunctionPlan> {
        let mut functions = Vec::new();
        let mut add = |s: &mut ShapeBuf, name: String, origin: &Origin, plan: FunctionPlan| {
            BodyPlan::declare(s, pins, &[], body, &name, &Self::at(origin), &plan);
            functions.push(plan);
        };
        for (k, e) in body.events().iter().enumerate() {
            let EventSource::Cross { .. } = e.source else { continue };
            let k = k as u32;
            let set = ReadSet::target(body, Query::Cross(k));
            add(s, format!("cross:{k}"), &e.origin, BodyPlan::function(Mode::Cross(k), &set, vec![(Target::Watch(k), Some(&set))]));
        }
        for (k, e) in body.events().iter().enumerate() {
            let k = k as u32;
            let EventSource::Cross { ttol, etol, .. } = e.source else { continue };
            if ttol.is_some() {
                let set = ReadSet::target(body, Query::Ttol(k));
                add(s, format!("ttol:{k}"), &e.origin, BodyPlan::function(Mode::Ttol(k), &set, vec![(Target::Ttol(k), None)]));
            }
            if etol.is_some() {
                let set = ReadSet::target(body, Query::Etol(k));
                add(s, format!("etol:{k}"), &e.origin, BodyPlan::function(Mode::Etol(k), &set, vec![(Target::Etol(k), None)]));
            }
        }
        for (k, d) in body.drives().iter().enumerate() {
            let k = k as u32;
            let set = ReadSet::target(body, Query::Drive(k));
            let write = match (d.after.is_some(), d.element.is_some()) {
                (true, _) => Target::Delayed(k),
                (false, true) => Target::ElementDrive(k),
                (false, false) if Self::is_resolved(body, d.carrier) => Target::ResolvedDrive(d.carrier),
                (false, false) => Target::Carrier(d.carrier),
            };
            let name = Self::drive_name(body, k);
            add(s, name, &d.origin, BodyPlan::function(Mode::Drive(k), &set, vec![(write, None)]));
        }
        for (k, e) in body.events().iter().enumerate() {
            let k = k as u32;
            let mut set = ReadSet::target(body, Query::Guard(k));
            match e.source {
                EventSource::Changed(c) => set.0.extend([Source::Carrier(c), Source::CarrierChanged(c)]),
                // `p.changed()`: the param pin's value and the byte the host's `set` stamps
                EventSource::ParamChanged(p) => set.0.extend([Source::Param(p), Source::ParamChanged(p)]),
                EventSource::Cross { .. } if e.guard.is_some() => {
                    set.0.insert(Source::Crossed(k));
                }
                EventSource::Cross { .. } => continue,
                // a guarded timer fires its own pin, which the event function gates
                EventSource::At(_) if e.guard.is_some() => {
                    set.0.insert(Source::Timer(k));
                }
                // an unguarded one is fired by its arming reaction's statement
                EventSource::At(_) => continue,
                // `settle` (§13 rule 17): the settle pin's `CHANGED` byte, gated by the guard
                EventSource::Settle => {
                    set.0.insert(Source::Settle);
                }
                // `lines[k].changed()` (§13 rule 9): the element's value and `CHANGED` byte, gated by the guard
                EventSource::ElementChanged { carrier, element } => {
                    set.0.extend([Source::Element(carrier, element, part::VALUE), Source::Element(carrier, element, part::CHANGED)]);
                }
            }
            add(s, format!("event:{k}"), &e.origin, BodyPlan::function(Mode::Event(k), &set, vec![(Target::EventPin(k), None)]));
        }
        for (k, d) in body.drives().iter().enumerate() {
            if d.after.is_none() {
                continue;
            }
            let k = k as u32;
            let set = ReadSet([Source::Delayed(k), Source::DelayedChanged(k)].into_iter().collect());
            let name = format!("drive:{}.changed", Self::carrier_name(body, d.carrier));
            add(s, name, &d.origin, BodyPlan::function(Mode::DelayedEvent(k), &set, vec![(Target::DelayedEventPin(k), None)]));
        }
        for (k, e) in body.events().iter().enumerate() {
            let event = k as u32;
            for &(carrier, _) in pins.rearms.get(k).map_or(&[][..], Vec::as_slice) {
                let set = ReadSet([Source::CarrierChanged(carrier)].into_iter().collect());
                let name = format!("at:{k}.{}", Self::carrier_name(body, carrier));
                let plan = BodyPlan::function(Mode::Rearm { event, carrier }, &set, vec![(Target::Rearm { event, carrier }, None)]);
                add(s, name, &e.origin, plan);
            }
        }
        functions
    }

    /// The name of drive `k`'s function: `drive:<carrier>`, `drive:<carrier>[<element>]` for one element.
    pub fn drive_name(body: Body<'_>, k: u32) -> String {
        let Some(drive) = body.drives().get(k as usize) else { return format!("drive:{k}") };
        match drive.element {
            Some(element) => format!("drive:{}[{element}]", Self::carrier_name(body, drive.carrier)),
            None => format!("drive:{}", Self::carrier_name(body, drive.carrier)),
        }
    }

    /// What reaction `k` of `body` reads: its own dependency set.
    pub(super) fn reaction_reads(body: Body<'_>, k: u32) -> ReadSet {
        ReadSet::target(body, Query::Reaction(k))
    }

    /// Declares the reactions of `body`: its own, then one per drive with `after`.
    pub(crate) fn reactions(s: &mut ShapeBuf, pins: &Pins, body: Body<'_>) -> Vec<ReactionPlan> {
        let ir = body.ir();
        let mut reactions = Vec::new();
        for (k, r) in body.reactions().iter().enumerate() {
            let plan = ReactionPlan {
                mode: ReactionMode::Reaction(k as u32),
                on: r.on.iter().map(|e| pins.events.get(*e as usize).copied().unwrap_or(u32::MAX)).collect(),
                // abi.md §4.2: a reaction reads no `FREQ`; the `freq()` of a function it calls is 0
                // (§14.4.4), which the printer binds
                reads: Self::reaction_reads(body, k as u32).0.into_iter().filter(|s| *s != Source::Freq).collect(),
                statements: StatementPlan::of_reaction(body, k as u32),
            };
            Self::declare(s, pins, body, &format!("reaction:{k}"), &Self::at(&r.origin), &plan);
            reactions.push(plan);
        }
        for (k, d) in body.drives().iter().enumerate() {
            let k = k as u32;
            let Some(delayed) = pins.delayed(k) else { continue };
            let mut set = ReadSet::target(body, Query::DriveAfter(k));
            set.0.insert(Source::Delayed(k));
            let plan = ReactionPlan {
                mode: ReactionMode::Delayed(k),
                on: vec![delayed.event],
                reads: set.0.into_iter().collect(),
                statements: vec![match StatementPlan::commit(ir, d.carrier, d.after) {
                    statement if Self::is_resolved(body, d.carrier) => StatementPlan { target: Target::ResolvedDrive(d.carrier), ..statement },
                    statement => statement,
                }],
            };
            let name = format!("drive:{}", Self::carrier_name(body, d.carrier));
            Self::declare(s, pins, body, &name, &Self::at(&d.origin), &plan);
            reactions.push(plan);
        }
        for (k, e) in body.events().iter().enumerate() {
            let EventSource::At(_) = e.source else { continue };
            let event = k as u32;
            let mut set = ReadSet::target(body, Query::At(event));
            set.0.insert(Source::Time);
            let mut on: Vec<u32> = pins.start.into_iter().collect();
            on.extend(pins.rearms.get(k).map_or(&[][..], Vec::as_slice).iter().map(|(_, pin)| *pin));
            let plan = ReactionPlan {
                mode: ReactionMode::Arm(event),
                on,
                reads: set.0.into_iter().collect(),
                statements: vec![StatementPlan::arm(body, event)],
            };
            Self::declare(s, pins, body, &format!("at:{k}"), &Self::at(&e.origin), &plan);
            reactions.push(plan);
        }
        reactions
    }

    /// Adds `plan` to the shape as reaction `name`: each statement commits the whole of its
    /// carrier's pin, or fires an event pin (`len` 0, abi.md C4).
    pub(super) fn declare(s: &mut ShapeBuf, pins: &Pins, body: Body<'_>, name: &str, origin: &str, plan: &ReactionPlan) {
        let limits = body.limits().len() as u32;
        let stims = body.stims().len() as u32;
        let writes = plan
            .statements
            .iter()
            .map(|st| {
                let (pin, len) = match st.target {
                    Target::Carrier(c) => {
                        (pins.carrier(c), body.carriers().get(c as usize).and_then(|x| PinLayout::in_ir(body.ir(), &x.ty)).map_or(0, |l| l.size()))
                    }
                    Target::ResolvedDrive(c) => (pins.resolved_drive(c), Self::drive_size(body, c)),
                    Target::EventPin(k) => (pins.events.get(k as usize).copied().unwrap_or(GROUND), 0),
                    Target::Timer(k) => (pins.timer(k), 0),
                    Target::Record(r) => {
                        (pins.record(r), body.streams().get(r as usize).and_then(|stream| RecordLayout::in_ir(body.ir(), stream)).map_or(0, |l| l.size))
                    }
                    _ => (GROUND, 0),
                };
                let (mut flags, min_after) = match st.min_after {
                    Some(m) => (statement_flag::LATER, m),
                    None => (0, 0.0),
                };
                if st.replace {
                    flags |= statement_flag::REPLACE;
                }
                let (offset, len) = st.range.unwrap_or((0, len));
                Statement { pin, offset, len, flags, min_after }
            })
            .collect();
        s.reaction(ReactionSpec {
            name,
            origin,
            on: plan.on.clone(),
            reads: plan.reads.iter().map(|r| BodyPlan::read(pins, limits, stims, *r)).collect(),
            writes,
            elements: BodyPlan::elements(&plan.reads),
            ..ReactionSpec::default()
        });
    }
}
