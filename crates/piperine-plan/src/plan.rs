//! The plan of one body: everything computed once per body (design "From a body to a shape").
//! Pins come first, in this order: one `PARAM` pin per param, one `PORT` continuous pin per body
//! node, one `PRIVATE` continuous pin per potential branch (its flow unknown), and one `GLOBAL`
//! `TEMPERATURE` pin when the body reads `temperature()`. Terms follow the branches: a flow
//! branch has a `FLOW` term for its direct addends and a `DDT` term for its `ddt` addends; a
//! potential branch has the two `FLOW` terms of abi.md A1, and a `DDT` term `{f, GROUND}` when it
//! has `ddt` addends. Inputs: one `LIMIT` per `limit` site, then one `STIM` per `ac_stim` site.
//!
//! Functions: `limit:<site>` per `limit` site, `stim:<k>` per `ac_stim` site, then `terms`,
//! which writes every term (abi.md A3 keeps limits and terms apart), then `check:<p>` per param
//! with a `where`: it reads the params the condition names and writes the param's private `BIT`
//! pin `check:<p>`, which the param's pin names as its `check` (`HAS_CHECK`). The check pins are
//! the last M1 pins of the shape; the discrete pins of `digital.rs` (one per carrier) follow
//! them. A function reads exactly
//! the cells of its dependency set (`msl-ir` IR-09) in one order: continuous pins (nodes by
//! index, then private flows), `LIMIT` values, `STIM` values (the differentiable reads, one
//! tangent slot each, in that order), then params, the temperature pin and the ambients
//! (`STATIC`, then `TIME`, the readings of `analysis(Static)` and `now()`: never a partial). A
//! `limit` function also reads its site's `LAST` and `AGE` and the ambient `STATIC`. Partials are
//! declared write by write, each against the differentiable reads of its own set, in read
//! order: the order the assembly folds `Σ g·x` in (INT-12).

use std::collections::BTreeSet;

use piperine_abi::layout::{BIT, F64};

use crate::digital::{CarrierLayout, Digital, ReactionPlan};
use piperine_abi::raw::{Cell, DEFAULT, GROUND, Partial, ambient, cell, global, input, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, InputSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_ir::{
    AnalysisKind, Body, BranchKind, Constant, Deps, Env, Operator, Origin, Stmt, Target as Query, Terminal, ValueType,
};

/// What one term of the shape computes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Term {
    /// A flow branch's direct addends, summed.
    Flow(u32),
    /// A branch's `ddt` addends, summed (negated on a potential branch, abi.md A1).
    Charge(u32),
    /// A potential branch's `FLOW {plus, minus}` term: its private flow pin.
    Current(u32),
    /// A potential branch's `FLOW {f, GROUND}` term: the potential difference minus the sum of
    /// its direct addends.
    Eq(u32),
}

/// What a function computes: one walk of the `analog` block in one mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The function of `limit` site `k`: its `PROPOSED` and `LIMITED`.
    Limit(u32),
    /// The function of `ac_stim` site `k`: its `RE` and `IM`.
    Stim(u32),
    /// Every term of the body.
    Terms,
    /// The `where` of param `p`: its check pin.
    Check(u32),
    /// Drive `k`: the value of its carrier, or of its undelayed pin when it has an `after`.
    Drive(u32),
    /// Event `k`: a `Changed` event fires its event pin when the carrier changed and the guard
    /// holds; a guarded `Cross` event, when its watch fired and the guard holds.
    Event(u32),
    /// The `cross` event `k`: the value of its `CROSS` watch.
    Cross(u32),
    /// `bound_step` site `k`: the value of its `CAP` watch.
    Cap(u32),
    /// The `ttol` of `cross` event `k`: its tolerance pin.
    Ttol(u32),
    /// The `etol` of `cross` event `k`: its tolerance pin.
    Etol(u32),
    /// The `changed` event of drive `k`'s undelayed pin.
    DelayedEvent(u32),
    /// The `changed` event of `carrier`, which the time of `at` event `event` reads: it fires
    /// that timer's arming reaction (abi.md R1).
    Rearm { event: u32, carrier: u32 },
}

/// Where a read of a function comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// A body node's pin.
    Node(u32),
    /// A potential branch's private flow pin.
    BranchFlow(u32),
    /// A `limit` site's input `VALUE`.
    LimitValue(u32),
    /// An `ac_stim` site's input `VALUE`.
    StimValue(u32),
    Param(u32),
    /// The `VALUE` of a carrier's pin.
    Carrier(u32),
    /// The `PREVIOUS` of a carrier's pin: its bytes before its most recent change (abi.md §4.1,
    /// msl-digital A7). Between `Carrier` and `CarrierChanged`, so a function that reads all
    /// three of one carrier reads `[VALUE, PREVIOUS, CHANGED]`, the order of abi.md §11.4.
    CarrierPrevious(u32),
    /// The `CHANGED` byte of a carrier's pin.
    CarrierChanged(u32),
    /// The `VALUE` of drive `k`'s undelayed pin.
    Delayed(u32),
    /// The `CHANGED` byte of drive `k`'s undelayed pin.
    DelayedChanged(u32),
    /// The `CHANGED` byte of the pin the watch of guarded `cross` event `k` fires.
    Crossed(u32),
    /// The `CHANGED` byte of the pin the timer of guarded `at` event `k` fires.
    Timer(u32),
    Temperature,
    Gmin,
    SourceScale,
    LimitLast(u32),
    LimitAge(u32),
    /// The ambient `STATIC`: an equilibrium solve, or an instant of one (abi.md §4.2).
    Static,
    /// The ambient `TIME`: the time of the solve or of the instant (abi.md §4.2).
    Time,
    /// The ambient `FREQ`: the analysis frequency, 0 outside an AC analysis (abi.md §4.2).
    Freq,
    /// The `GLOBAL` pin of an analysis kind: whether the running analysis is of this kind.
    Analysis(AnalysisKind),
    /// The `CHANGED` byte of the event pin of event `k`: whether the event fired in this
    /// microstep (`Expr::Fired`, abi.md §4.1).
    Fired(u32),
}

impl Source {
    /// A continuous pin: read as 0 by the affine rule (INT-12).
    pub fn is_pin(self) -> bool {
        matches!(self, Source::Node(_) | Source::BranchFlow(_))
    }

    /// A read the walk carries a tangent for.
    pub fn is_differentiable(self) -> bool {
        matches!(self, Source::Node(_) | Source::BranchFlow(_) | Source::LimitValue(_) | Source::StimValue(_))
    }
}

/// What a write of a function is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Term(Term),
    Proposed(u32),
    Limited(u32),
    StimRe(u32),
    StimIm(u32),
    /// The check pin of param `p`.
    Check(u32),
    /// The `VALUE` of a carrier's pin.
    Carrier(u32),
    /// The event pin of event `k`.
    EventPin(u32),
    /// Drive `k`'s undelayed pin.
    Delayed(u32),
    /// The event pin of the `changed` event of drive `k`'s undelayed pin.
    DelayedEventPin(u32),
    /// The `CROSS` watch of event `k`.
    Watch(u32),
    /// The pin the timer of guarded `at` event `k` fires (a statement of its arming reaction).
    Timer(u32),
    /// The event pin of the `changed` event of `carrier` that re-arms `at` event `event`.
    Rearm { event: u32, carrier: u32 },
    /// The `CAP` watch of `bound_step` site `k`.
    Cap(u32),
    /// The `ttol` pin of `cross` event `k`.
    Ttol(u32),
    /// The `etol` pin of `cross` event `k`.
    Etol(u32),
    /// The `RECORD` pin of stream `s` (a statement of a reaction that emits).
    Record(u32),
}

/// What one function of the shape computes, entry by entry parallel to the shape.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionPlan {
    pub mode: Mode,
    /// Parallel to the function's reads.
    pub reads: Vec<Source>,
    /// The differentiable reads: the leading `slots` entries of `reads`.
    pub slots: usize,
    /// Parallel to the function's writes.
    pub writes: Vec<Target>,
    /// `(write, slot)`, parallel to the function's partials.
    pub partials: Vec<(usize, usize)>,
    /// Per write: a `FLOW` term affine in the node potentials (INT-12).
    pub affine: Vec<bool>,
}

/// A set of reads, kept in read order by `Source`'s order.
#[derive(Clone, Debug, Default)]
pub(crate) struct ReadSet(pub(crate) BTreeSet<Source>);

impl ReadSet {
    pub(crate) fn of(deps: &Deps) -> ReadSet {
        let mut set = ReadSet::default();
        set.add(deps);
        set
    }

    fn add(&mut self, deps: &Deps) {
        self.0.extend(deps.nodes.iter().map(|i| Source::Node(*i)));
        self.0.extend(deps.limits.iter().map(|i| Source::LimitValue(*i)));
        self.0.extend(deps.stims.iter().map(|i| Source::StimValue(*i)));
        self.0.extend(deps.params.iter().map(|i| Source::Param(*i)));
        self.0.extend(deps.env.iter().filter_map(|e| match e {
            Env::Temperature => Some(Source::Temperature),
            Env::Gmin => Some(Source::Gmin),
            Env::SourceScale => Some(Source::SourceScale),
            // ambient reads (abi.md §4.2): `check()` lets only `terms`, `cross:*` and reactions
            // hold them, the readers abi.md gives `TIME` and `STATIC`
            Env::Time => Some(Source::Time),
            Env::Static => Some(Source::Static),
            Env::Freq => Some(Source::Freq),
            Env::Analysis(kind) => Some(Source::Analysis(*kind)),
        }));
    }

    fn union(&mut self, other: &ReadSet) {
        self.0.extend(other.0.iter().copied());
    }

    /// The whole dependency set of `target`: its `Deps`, the carriers it reads, the flows of
    /// potential branches it reads (`i(a, b)`, msl-digital A5: the branch's private flow pin, a
    /// differentiable read) and the carriers whose previous value it reads (`c.prev()`, A7).
    pub(crate) fn target(body: Body<'_>, target: Query) -> ReadSet {
        let mut set = ReadSet::of(&body.dependencies(target));
        set.0.extend(body.carrier_reads(target).into_iter().map(Source::Carrier));
        set.0.extend(body.flow_reads(target).into_iter().map(Source::BranchFlow));
        set.0.extend(body.previous_reads(target).into_iter().map(Source::CarrierPrevious));
        set.0.extend(body.fired_reads(target).into_iter().map(Source::Fired));
        set
    }
}

/// What a branch receives from the `analog` block, and where it was first written.
#[derive(Clone, Debug, Default)]
struct Received {
    direct: bool,
    ddt: bool,
    origin: Option<Origin>,
}

/// The pin indices of a body's shape.
#[derive(Clone, Debug, PartialEq)]
pub struct Pins {
    /// The first body-node pin; param `i` is pin `i`.
    pub nodes: u32,
    /// The private flow pin of each branch; `None` for a flow branch.
    pub branches: Vec<Option<u32>>,
    pub temperature: Option<u32>,
    /// The `GLOBAL` pin of each analysis kind the body reads, in `AnalysisKind` order, after the
    /// temperature pin.
    pub analysis: Vec<(AnalysisKind, u32)>,
    /// The private check pin of each param with a `where`, after every other M1 pin.
    pub checks: Vec<Option<u32>>,
    /// The pin of carrier 0; carrier `i` is pin `carriers + i`, after every M1 pin.
    pub carriers: u32,
    /// The event pin of each event of the body, after the carrier pins.
    pub events: Vec<u32>,
    /// Per event, the pin its `CROSS` watch fires when it is a guarded `cross`, after the event
    /// pins; `None` for every other event (an unguarded watch fires the event pin itself).
    pub crossed: Vec<Option<u32>>,
    /// Per event, the index of its `CROSS` watch; `None` for a `Changed` event.
    pub watches: Vec<Option<u32>>,
    /// The two pins of each drive with an `after`, after the event pins; `None` for a drive
    /// without one.
    pub delayed: Vec<Option<DelayedPins>>,
    /// Per event, the private event pin `at:<k>` its timer fires when it is a guarded `at`,
    /// after the delayed pins; `None` for every other event (an unguarded timer fires the event
    /// pin itself).
    pub timers: Vec<Option<u32>>,
    /// The `GLOBAL` `START` pin, after the timer pins, when the body has an `at` event: every
    /// timer is armed at the first instant (abi.md R1).
    pub start: Option<u32>,
    /// Per event, `(carrier, pin)` for each carrier the time of an `at` event reads: the event
    /// pin `at:<k>.<carrier>` its `changed` event fires, after the `START` pin.
    pub rearms: Vec<Vec<(u32, u32)>>,
    /// The `RECORD` pin of each stream of the body, in stream order, after the re-arming pins.
    pub records: Vec<u32>,
    /// Per event, the `ttol` and `etol` pins of its `cross` (`ttol:<k>`, `etol:<k>`), after the
    /// record pins; `None` for a tolerance the event does not give.
    pub tolerances: Vec<(Option<u32>, Option<u32>)>,
    /// The `CAP` watch of each `bound_step` site, as `(site, watch)`, after the `CROSS` watches.
    pub caps: Vec<(u32, u32)>,
}

/// The pins of a drive with `after` (RS-12 AC 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DelayedPins {
    /// `drive:<carrier>.now`: the undelayed value, in the carrier's layout.
    pub value: u32,
    /// `drive:<carrier>.changed`: the event pin fired when `value` changes.
    pub event: u32,
}

impl Pins {
    /// The pin of body node `i`.
    pub fn node(&self, i: u32) -> u32 {
        self.nodes + i
    }

    /// The pin of carrier `i`.
    pub fn carrier(&self, i: u32) -> u32 {
        self.carriers + i
    }

    /// The pins of drive `k`; `None` for a drive without `after` or out of range.
    pub fn delayed(&self, k: u32) -> Option<DelayedPins> {
        self.delayed.get(k as usize).copied().flatten()
    }

    /// The pin the timer of guarded `at` event `k` fires; `GROUND` for any other event.
    pub fn timer(&self, k: u32) -> u32 {
        self.timers.get(k as usize).copied().flatten().unwrap_or(GROUND)
    }

    /// The event pin of the `changed` event of `carrier` that re-arms `at` event `event`;
    /// `GROUND` when there is none.
    pub fn rearm(&self, event: u32, carrier: u32) -> u32 {
        self.rearms.get(event as usize).and_then(|r| r.iter().find(|(c, _)| *c == carrier)).map_or(GROUND, |(_, p)| *p)
    }

    /// The `CAP` watch of `bound_step` site `k`; `GROUND` for a site the body does not have.
    pub fn cap(&self, k: u32) -> u32 {
        self.caps.iter().find(|(site, _)| *site == k).map_or(GROUND, |(_, watch)| *watch)
    }

    /// The `ttol` pin of `cross` event `k`; `GROUND` when it has none.
    pub fn ttol(&self, k: u32) -> u32 {
        self.tolerances.get(k as usize).and_then(|t| t.0).unwrap_or(GROUND)
    }

    /// The `etol` pin of `cross` event `k`; `GROUND` when it has none.
    pub fn etol(&self, k: u32) -> u32 {
        self.tolerances.get(k as usize).and_then(|t| t.1).unwrap_or(GROUND)
    }

    /// The `RECORD` pin of stream `s`; `GROUND` out of range.
    pub fn record(&self, s: u32) -> u32 {
        self.records.get(s as usize).copied().unwrap_or(GROUND)
    }

    /// The pin a term end names: a node's pin, or `GROUND`.
    pub fn terminal(&self, t: Terminal) -> u32 {
        match t {
            Terminal::Node(i) => self.node(i),
            Terminal::Ground => GROUND,
        }
    }
}

/// Everything computed once per body: the shape and what each of its entries means.
#[derive(Debug)]
pub struct BodyPlan {
    shape: ShapeRef,
    /// What each function of the shape computes, in the shape's order.
    functions: Vec<FunctionPlan>,
    /// What each reaction of the shape does, in the shape's order.
    reactions: Vec<ReactionPlan>,
}

impl BodyPlan {
    /// Plans `body`. The body comes from an `Ir` that passes `check()`.
    pub fn new(body: Body<'_>) -> BodyPlan {
        let ir = body.ir();
        let origin = |o: &Origin| format!("{}:{}:{}", o.file, o.line, o.col);
        let received = Self::received(body);
        let mut s = ShapeBuf::new();

        // Params, in declaration order (P5), each with its default as `init`.
        let defaults: Vec<Vec<u8>> = body
            .params()
            .iter()
            .map(|p| match p.default {
                Some(Constant::Real(r)) => r.get().to_le_bytes().to_vec(),
                Some(Constant::Bool(b)) => vec![u8::from(b), 1],
                // An integer param has a carrier's layout and bytes (`digital.rs`).
                Some(c @ Constant::Int(_)) => CarrierLayout::bytes(&p.ty, c).unwrap_or_default(),
                None => Vec::new(),
            })
            .collect();
        // A param with a `where` names its check pin, the last pins of the shape (P6): the pins
        // before them keep their indices.
        let pin_count = body.params().len()
            + body.nodes().len()
            + body.branches().iter().filter(|b| b.kind == BranchKind::Potential).count()
            + usize::from(Self::reads_temperature(body))
            + Self::reads_analysis(body).len();
        let mut next_check = pin_count as u32;
        let mut check_pins = Vec::with_capacity(body.params().len());
        for (p, init) in body.params().iter().zip(&defaults) {
            let layout = match &p.ty {
                ValueType::Bool => &BIT,
                ty if ty.is_integer() => CarrierLayout::of(ty).unwrap_or(&F64),
                _ => &F64,
            };
            let mut flags = if init.is_empty() { 0 } else { pin_flag::HAS_INIT };
            if p.structural {
                flags |= pin_flag::STRUCTURAL;
            }
            let check = p.check.map(|_| {
                flags |= pin_flag::HAS_CHECK;
                next_check += 1;
                next_check - 1
            });
            check_pins.push(check);
            let at = origin(&p.origin);
            s.pin(PinSpec { origin: &at, flags, init, check: check.unwrap_or(0), ..PinSpec::discrete(&p.name, role::PARAM, layout) });
        }

        // Body nodes, ports and signals alike, with their nature's tolerances.
        let nodes = body.params().len() as u32;
        let inits: Vec<Vec<u8>> =
            body.nodes().iter().map(|n| n.init.map_or_else(Vec::new, |r| r.get().to_le_bytes().to_vec())).collect();
        for (n, init) in body.nodes().iter().zip(&inits) {
            let nature = ir.nature(n.nature);
            let abstol = [nature.potential().abstol.get(), nature.flow().map_or(0.0, |q| q.abstol.get())];
            let flags = if init.is_empty() { 0 } else { pin_flag::HAS_INIT };
            let at = origin(&n.origin);
            s.pin(PinSpec { origin: &at, flags, init, abstol, ..PinSpec::continuous(&n.name, role::PORT) });
        }

        // One private flow pin per potential branch.
        let branch_origin = |b: usize| origin(received[b].origin.as_ref().unwrap_or(body.origin()));
        let mut branches = Vec::with_capacity(body.branches().len());
        for (b, branch) in body.branches().iter().enumerate() {
            branches.push(match branch.kind {
                BranchKind::Flow => None,
                BranchKind::Potential => {
                    let at = branch_origin(b);
                    Some(s.pin(PinSpec { origin: &at, ..PinSpec::continuous(&format!("b{b}.flow"), role::PRIVATE) }))
                }
            });
        }

        let temperature = Self::reads_temperature(body).then(|| {
            let at = origin(body.origin());
            s.pin(PinSpec {
                origin: &at,
                global: global::TEMPERATURE,
                ..PinSpec::discrete("temperature", role::GLOBAL, &F64)
            })
        });
        // One `GLOBAL` `BIT` pin per analysis kind the body reads (`analysis(kind)`).
        let analysis: Vec<(AnalysisKind, u32)> = Self::reads_analysis(body)
            .into_iter()
            .map(|kind| {
                let (global, name) = Self::analysis_pin(kind);
                let at = origin(body.origin());
                (kind, s.pin(PinSpec { origin: &at, global, ..PinSpec::discrete(name, role::GLOBAL, &BIT) }))
            })
            .collect();
        // The check pins: a private `BIT` per `where`, written by `check:<p>`.
        let mut checks = Vec::with_capacity(check_pins.len());
        for (p, check) in body.params().iter().zip(&check_pins) {
            checks.push(check.map(|expected| {
                let at = origin(&p.origin);
                let pin = s.pin(PinSpec { origin: &at, ..PinSpec::discrete(&format!("check:{}", p.name), role::PRIVATE, &BIT) });
                debug_assert_eq!(pin, expected);
                pin
            }));
        }
        let carriers = Digital::carrier_pins(&mut s, body);
        let events = Digital::event_pins(&mut s, body);
        let crossed = Digital::crossed_pins(&mut s, body);
        let delayed = Digital::delayed_pins(&mut s, body);
        let timers = Digital::timer_pins(&mut s, body);
        let start = Digital::start_pin(&mut s, body);
        let rearms = Digital::rearm_pins(&mut s, body);
        let records = Digital::record_pins(&mut s, body);
        let tolerances = Digital::tolerance_pins(&mut s, body);
        let watches = Digital::watches(&mut s, body, &events, &crossed, &tolerances);
        let caps = Digital::cap_watches(&mut s, body);
        let pins = Pins {
            nodes,
            branches,
            temperature,
            analysis,
            checks,
            carriers,
            events,
            crossed,
            watches,
            delayed,
            timers,
            start,
            rearms,
            records,
            tolerances,
            caps,
        };

        // Terms, branch by branch.
        let mut terms = Vec::new();
        for (b, branch) in body.branches().iter().enumerate() {
            let at = branch_origin(b);
            let (plus, minus) = (pins.terminal(branch.plus), pins.terminal(branch.minus));
            let mut term = |name: String, operator: u32, plus: u32, minus: u32, what: Term| {
                s.term(TermSpec { name: &name, origin: &at, op: operator, plus, minus, abstol: DEFAULT, max: DEFAULT });
                terms.push(what);
            };
            let b32 = b as u32;
            match (branch.kind, pins.branches[b]) {
                (BranchKind::Potential, Some(f)) => {
                    term(format!("b{b}.flow"), op::FLOW, plus, minus, Term::Current(b32));
                    term(format!("b{b}.eq"), op::FLOW, f, GROUND, Term::Eq(b32));
                    if received[b].ddt {
                        term(format!("b{b}.charge"), op::DDT, f, GROUND, Term::Charge(b32));
                    }
                }
                _ => {
                    if received[b].direct {
                        term(format!("b{b}.flow"), op::FLOW, plus, minus, Term::Flow(b32));
                    }
                    if received[b].ddt {
                        term(format!("b{b}.charge"), op::DDT, plus, minus, Term::Charge(b32));
                    }
                }
            }
        }

        // Inputs: limits, then stimuli, each named after its site.
        for site in body.limits() {
            let at = origin(&site.origin);
            s.input(InputSpec { name: &site.name, origin: &at, kind: input::LIMIT });
        }
        for site in body.stims() {
            let at = origin(&site.origin);
            s.input(InputSpec { name: &site.name, origin: &at, kind: input::STIM });
        }

        // Functions: one per site, then the terms.
        let mut functions = Vec::new();
        for (k, site) in body.limits().iter().enumerate() {
            let k = k as u32;
            let set = ReadSet::target(body, Query::Limit(k));
            let mut plan = Self::function(Mode::Limit(k), &set, vec![(Target::Proposed(k), Some(&set)), (Target::Limited(k), None)]);
            plan.reads.extend([Source::LimitLast(k), Source::LimitAge(k)]);
            // a site that reads `static()` itself already has the ambient among its reads
            if !plan.reads.contains(&Source::Static) {
                plan.reads.push(Source::Static);
            }
            let name = format!("limit:{}", site.name);
            Self::declare(&mut s, &pins, &terms, body, &name, &origin(&site.origin), &plan);
            functions.push(plan);
        }
        for (k, site) in body.stims().iter().enumerate() {
            let k = k as u32;
            let set = ReadSet::target(body, Query::Stim(k));
            let plan = Self::function(Mode::Stim(k), &set, vec![(Target::StimRe(k), None), (Target::StimIm(k), None)]);
            Self::declare(&mut s, &pins, &terms, body, &format!("stim:{k}"), &origin(&site.origin), &plan);
            functions.push(plan);
        }
        if !terms.is_empty() {
            let sets: Vec<ReadSet> = terms.iter().map(|t| Self::term_set(body, *t)).collect();
            let mut all = ReadSet::default();
            for set in &sets {
                all.union(set);
            }
            let writes = terms.iter().zip(&sets).map(|(t, set)| (Target::Term(*t), Some(set))).collect();
            let mut plan = Self::function(Mode::Terms, &all, writes);
            plan.affine = terms
                .iter()
                .map(|t| match t {
                    Term::Flow(b) | Term::Eq(b) => body.is_affine(*b),
                    Term::Current(_) => true,
                    Term::Charge(_) => false,
                })
                .collect();
            Self::declare(&mut s, &pins, &terms, body, "terms", &origin(body.origin()), &plan);
            functions.push(plan);
        }
        for (p, param) in body.params().iter().enumerate() {
            if param.check.is_none() {
                continue;
            }
            let p = p as u32;
            let set = ReadSet::target(body, Query::Check(p));
            let plan = Self::function(Mode::Check(p), &set, vec![(Target::Check(p), None)]);
            Self::declare(&mut s, &pins, &terms, body, &format!("check:{}", param.name), &origin(&param.origin), &plan);
            functions.push(plan);
        }

        for (site, _) in &pins.caps {
            let set = ReadSet::target(body, Query::BoundStep(*site));
            let plan = Self::function(Mode::Cap(*site), &set, vec![(Target::Cap(*site), None)]);
            let at = origin(body.origin());
            Self::declare(&mut s, &pins, &terms, body, &format!("cap:{site}"), &at, &plan);
            functions.push(plan);
        }
        functions.extend(Digital::functions(&mut s, &pins, body));
        let reactions = Digital::reactions(&mut s, &pins, body);

        BodyPlan { shape: s.freeze(), functions, reactions }
    }

    /// What term `t` reads.
    fn term_set(body: Body<'_>, t: Term) -> ReadSet {
        match t {
            Term::Flow(b) => ReadSet::target(body, Query::Flow(b)),
            Term::Charge(b) => ReadSet::target(body, Query::Charge(b)),
            Term::Current(b) => ReadSet([Source::BranchFlow(b)].into_iter().collect()),
            Term::Eq(b) => {
                let mut set = ReadSet::target(body, Query::Flow(b));
                let branch = &body.branches()[b as usize];
                for end in [branch.plus, branch.minus] {
                    if let Terminal::Node(i) = end {
                        set.0.insert(Source::Node(i));
                    }
                }
                set
            }
        }
    }

    /// A function reading `all`, writing `writes`; a write with a set gets one partial per
    /// differentiable read of that set.
    pub(crate) fn function(mode: Mode, all: &ReadSet, writes: Vec<(Target, Option<&ReadSet>)>) -> FunctionPlan {
        let reads: Vec<Source> = all.0.iter().copied().collect();
        let slots = reads.iter().take_while(|s| s.is_differentiable()).count();
        let mut partials = Vec::new();
        for (w, (_, set)) in writes.iter().enumerate() {
            let Some(set) = set else { continue };
            for (slot, source) in reads[..slots].iter().enumerate() {
                if set.0.contains(source) {
                    partials.push((w, slot));
                }
            }
        }
        let affine = vec![false; writes.len()];
        FunctionPlan { mode, reads, slots, writes: writes.into_iter().map(|(t, _)| t).collect(), partials, affine }
    }

    /// Adds `plan` to the shape as function `name`.
    pub(crate) fn declare(s: &mut ShapeBuf, pins: &Pins, terms: &[Term], body: Body<'_>, name: &str, origin: &str, plan: &FunctionPlan) {
        let limits = body.limits().len() as u32;
        let at = |kind: u32, index: u32, part: u32| Cell { kind, index, part };
        let value = |kind: u32, index: u32| at(kind, index, part::VALUE);
        let reads = plan.reads.iter().map(|r| Self::read(pins, limits, *r)).collect();
        let writes = plan
            .writes
            .iter()
            .map(|w| match *w {
                Target::Term(t) => value(cell::TERM, terms.iter().position(|x| *x == t).unwrap_or(usize::MAX) as u32),
                Target::Proposed(k) => at(cell::INPUT, k, part::PROPOSED),
                Target::Limited(k) => at(cell::INPUT, k, part::LIMITED),
                Target::StimRe(k) => at(cell::INPUT, limits + k, part::RE),
                Target::StimIm(k) => at(cell::INPUT, limits + k, part::IM),
                Target::Check(p) => value(cell::PIN, pins.checks.get(p as usize).copied().flatten().unwrap_or(GROUND)),
                Target::Carrier(c) => value(cell::PIN, pins.carrier(c)),
                // abi.md §4.1: an event pin is written through its `CHANGED` byte (nonzero fires it).
                Target::EventPin(k) => at(cell::PIN, pins.events.get(k as usize).copied().unwrap_or(GROUND), part::CHANGED),
                Target::Delayed(k) => value(cell::PIN, pins.delayed(k).map_or(GROUND, |d| d.value)),
                Target::DelayedEventPin(k) => at(cell::PIN, pins.delayed(k).map_or(GROUND, |d| d.event), part::CHANGED),
                Target::Watch(k) => value(cell::WATCH, pins.watches.get(k as usize).copied().flatten().unwrap_or(GROUND)),
                Target::Timer(k) => at(cell::PIN, pins.timer(k), part::CHANGED),
                Target::Rearm { event, carrier } => at(cell::PIN, pins.rearm(event, carrier), part::CHANGED),
                Target::Record(stream) => value(cell::PIN, pins.record(stream)),
                Target::Cap(site) => value(cell::WATCH, pins.cap(site)),
                Target::Ttol(k) => value(cell::PIN, pins.ttol(k)),
                Target::Etol(k) => value(cell::PIN, pins.etol(k)),
            })
            .collect();
        let partials =
            plan.partials.iter().map(|(w, slot)| Partial { write: *w as u32, to: 0, read: *slot as u32, at: 0 }).collect();
        s.function(FunctionSpec { name, origin, reads, writes, partials, ..FunctionSpec::default() });
    }

    /// The cell a read names; `limits` is the body's count of `limit` sites (the `STIM` inputs
    /// follow them).
    pub(crate) fn read(pins: &Pins, limits: u32, r: Source) -> Cell {
        let at = |kind: u32, index: u32, part: u32| Cell { kind, index, part };
        let value = |kind: u32, index: u32| at(kind, index, part::VALUE);
        match r {
            Source::Node(i) => value(cell::PIN, pins.node(i)),
            Source::BranchFlow(b) => value(cell::PIN, pins.branches[b as usize].unwrap_or(GROUND)),
            Source::LimitValue(k) => value(cell::INPUT, k),
            Source::StimValue(k) => value(cell::INPUT, limits + k),
            Source::Param(i) => value(cell::PIN, i),
            Source::Carrier(c) => value(cell::PIN, pins.carrier(c)),
            Source::CarrierPrevious(c) => at(cell::PIN, pins.carrier(c), part::PREVIOUS),
            Source::CarrierChanged(c) => at(cell::PIN, pins.carrier(c), part::CHANGED),
            Source::Delayed(k) => value(cell::PIN, pins.delayed(k).map_or(GROUND, |d| d.value)),
            Source::DelayedChanged(k) => at(cell::PIN, pins.delayed(k).map_or(GROUND, |d| d.value), part::CHANGED),
            Source::Crossed(k) => at(cell::PIN, pins.crossed.get(k as usize).copied().flatten().unwrap_or(GROUND), part::CHANGED),
            Source::Timer(k) => at(cell::PIN, pins.timer(k), part::CHANGED),
            Source::Temperature => value(cell::PIN, pins.temperature.unwrap_or(GROUND)),
            Source::Gmin => value(cell::AMBIENT, ambient::GMIN),
            Source::SourceScale => value(cell::AMBIENT, ambient::SOURCE_SCALE),
            Source::LimitLast(k) => at(cell::INPUT, k, part::LAST),
            Source::LimitAge(k) => at(cell::INPUT, k, part::AGE),
            Source::Static => value(cell::AMBIENT, ambient::STATIC),
            Source::Time => value(cell::AMBIENT, ambient::TIME),
            Source::Freq => value(cell::AMBIENT, ambient::FREQ),
            Source::Analysis(kind) => value(cell::PIN, pins.analysis.iter().find(|(k, _)| *k == kind).map_or(GROUND, |(_, pin)| *pin)),
            Source::Fired(k) => at(cell::PIN, pins.events.get(k as usize).copied().unwrap_or(GROUND), part::CHANGED),
        }
    }

    /// What each branch receives: contributions sit at the top level of the `analog` block.
    fn received(body: Body<'_>) -> Vec<Received> {
        let ir = body.ir();
        let mut received = vec![Received::default(); body.branches().len()];
        for id in body.analog().unwrap_or(&[]) {
            let Stmt::Contribute { branch, addends, .. } = ir.stmt(*id) else { continue };
            let Some(r) = received.get_mut(*branch as usize) else { continue };
            for a in addends {
                match a.operator {
                    Operator::Direct => r.direct = true,
                    Operator::Ddt => r.ddt = true,
                }
                if r.origin.is_none() {
                    r.origin = Some(ir.expr_origin(a.value).clone());
                }
            }
        }
        received
    }

    /// The `bound_step` sites of the body's `analog` block, in order.
    pub(crate) fn bound_steps(body: Body<'_>) -> Vec<u32> {
        let ir = body.ir();
        body.analog()
            .unwrap_or(&[])
            .iter()
            .filter_map(|id| match ir.stmt(*id) {
                Stmt::BoundStep { site, .. } => Some(*site),
                _ => None,
            })
            .collect()
    }

    /// Whether any target of the body reads `temperature()`.
    fn reads_temperature(body: Body<'_>) -> bool {
        Self::queries(body).iter().any(|q| body.dependencies(*q).env.contains(&Env::Temperature))
    }

    /// The analysis kinds some target of the body reads, in `AnalysisKind` order.
    fn reads_analysis(body: Body<'_>) -> Vec<AnalysisKind> {
        let kinds: BTreeSet<AnalysisKind> = Self::queries(body)
            .iter()
            .flat_map(|q| body.dependencies(*q).env)
            .filter_map(|e| match e {
                Env::Analysis(kind) => Some(kind),
                _ => None,
            })
            .collect();
        kinds.into_iter().collect()
    }

    /// The `GLOBAL` and the name of the pin of analysis kind `kind`.
    fn analysis_pin(kind: AnalysisKind) -> (u32, &'static str) {
        match kind {
            AnalysisKind::Dc => (global::DC, "analysis:dc"),
            AnalysisKind::Transient => (global::TRAN, "analysis:tran"),
            AnalysisKind::Ac => (global::AC, "analysis:ac"),
            AnalysisKind::Noise => (global::NOISE, "analysis:noise"),
        }
    }

    /// Every dependency query of a body: each branch's two parts, each site, each param, and
    /// each drive, delay, guard, crossed expression, tolerance, `bound_step` site and reaction.
    fn queries(body: Body<'_>) -> Vec<Query> {
        let n = |len: usize| 0..len as u32;
        n(body.branches().len())
            .flat_map(|b| [Query::Flow(b), Query::Charge(b)])
            .chain(n(body.limits().len()).map(Query::Limit))
            .chain(n(body.stims().len()).map(Query::Stim))
            .chain(n(body.params().len()).map(Query::Check))
            .chain(n(body.drives().len()).flat_map(|k| [Query::Drive(k), Query::DriveAfter(k)]))
            .chain(n(body.events().len()).flat_map(|k| [Query::Guard(k), Query::Cross(k), Query::Ttol(k), Query::Etol(k)]))
            .chain(n(body.reactions().len()).map(Query::Reaction))
            .chain(Self::bound_steps(body).into_iter().map(Query::BoundStep))
            .collect()
    }

    /// Whether carrier `c` of `body` is a `PORT` pin of its shape, bound by the netlist to the
    /// instance's net: an `in` or `out` port, or a `signal` a child's carrier port is bound to.
    pub fn carrier_is_port(body: Body<'_>, c: u32) -> bool {
        Digital::is_port(body, c)
    }

    /// The shape every instance of this body gets.
    pub fn shape(&self) -> &ShapeRef {
        &self.shape
    }

    /// What each function of the shape computes, in the shape's order.
    pub fn functions(&self) -> &[FunctionPlan] {
        &self.functions
    }

    /// What each reaction of the shape does, in the shape's order.
    pub fn reactions(&self) -> &[ReactionPlan] {
        &self.reactions
    }
}
