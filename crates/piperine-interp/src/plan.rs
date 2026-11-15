//! The plan of one body: everything computed once per body (design "From a body to a shape").
//! Pins come first, in this order: one `PARAM` pin per param, one `PORT` continuous pin per body
//! node, one `PRIVATE` continuous pin per potential branch (its flow unknown), and one `GLOBAL`
//! `TEMPERATURE` pin when the body reads `temperature()`. Terms follow the branches: a flow
//! branch has a `FLOW` term for its direct addends and a `DDT` term for its `ddt` addends; a
//! potential branch has the two `FLOW` terms of abi.md A1, and a `DDT` term `{f, GROUND}` when it
//! has `ddt` addends. Inputs: one `LIMIT` per `limit` site, then one `STIM` per `ac_stim` site.
//!
//! Functions: `limit:<site>` per `limit` site, `stim:<k>` per `ac_stim` site, then `terms`,
//! which writes every term (abi.md A3 keeps limits and terms apart). A function reads exactly
//! the cells of its dependency set (`msl-ir` IR-09) in one order: continuous pins (nodes by
//! index, then private flows), `LIMIT` values, `STIM` values (the differentiable reads, one
//! tangent slot each, in that order), then params, the temperature pin and the ambients. A
//! `limit` function also reads its site's `LAST` and `AGE` and the ambient `STATIC`. Partials are
//! declared write by write, each against the differentiable reads of its own set, in read
//! order: the order the assembly folds `Σ g·x` in (INT-12).

use std::collections::BTreeSet;

use piperine_abi::layout::{BIT, F64};
use piperine_abi::raw::{Cell, DEFAULT, GROUND, Partial, ambient, cell, global, input, op, part, pin_flag, role};
use piperine_abi::shape::{FunctionSpec, InputSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_ir::{
    Body, BranchKind, Constant, Deps, Env, Operator, Origin, Stmt, Target as Query, Terminal, ValueType,
};

/// What one term of the shape computes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Term {
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
pub(crate) enum Mode {
    /// The function of `limit` site `k`: its `PROPOSED` and `LIMITED`.
    Limit(u32),
    /// The function of `ac_stim` site `k`: its `RE` and `IM`.
    Stim(u32),
    /// Every term of the body.
    Terms,
}

/// Where a read of a function comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Source {
    /// A body node's pin.
    Node(u32),
    /// A potential branch's private flow pin.
    BranchFlow(u32),
    /// A `limit` site's input `VALUE`.
    LimitValue(u32),
    /// An `ac_stim` site's input `VALUE`.
    StimValue(u32),
    Param(u32),
    Temperature,
    Gmin,
    SourceScale,
    LimitLast(u32),
    LimitAge(u32),
    Static,
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
pub(crate) enum Target {
    Term(Term),
    Proposed(u32),
    Limited(u32),
    StimRe(u32),
    StimIm(u32),
}

/// What one function of the shape computes, entry by entry parallel to the shape.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FunctionPlan {
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
struct ReadSet(BTreeSet<Source>);

impl ReadSet {
    fn of(deps: &Deps) -> ReadSet {
        let mut set = ReadSet::default();
        set.add(deps);
        set
    }

    fn add(&mut self, deps: &Deps) {
        self.0.extend(deps.nodes.iter().map(|i| Source::Node(*i)));
        self.0.extend(deps.limits.iter().map(|i| Source::LimitValue(*i)));
        self.0.extend(deps.stims.iter().map(|i| Source::StimValue(*i)));
        self.0.extend(deps.params.iter().map(|i| Source::Param(*i)));
        self.0.extend(deps.env.iter().map(|e| match e {
            Env::Temperature => Source::Temperature,
            Env::Gmin => Source::Gmin,
            Env::SourceScale => Source::SourceScale,
        }));
    }

    fn union(&mut self, other: &ReadSet) {
        self.0.extend(other.0.iter().copied());
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
pub(crate) struct Pins {
    /// The first body-node pin; param `i` is pin `i`.
    pub nodes: u32,
    /// The private flow pin of each branch; `None` for a flow branch.
    pub branches: Vec<Option<u32>>,
    pub temperature: Option<u32>,
}

impl Pins {
    /// The pin of body node `i`.
    pub fn node(&self, i: u32) -> u32 {
        self.nodes + i
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
    pub(crate) shape: ShapeRef,
    /// What each function of the shape computes, in the shape's order.
    pub(crate) functions: Vec<FunctionPlan>,
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
                None => Vec::new(),
            })
            .collect();
        for (p, init) in body.params().iter().zip(&defaults) {
            let layout = if p.ty == ValueType::Bool { &BIT } else { &F64 };
            let mut flags = if init.is_empty() { 0 } else { pin_flag::HAS_INIT };
            if p.structural {
                flags |= pin_flag::STRUCTURAL;
            }
            let at = origin(&p.origin);
            s.pin(PinSpec { origin: &at, flags, init, ..PinSpec::discrete(&p.name, role::PARAM, layout) });
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
        let pins = Pins { nodes, branches, temperature };

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
            let set = ReadSet::of(&body.dependencies(Query::Limit(k)));
            let mut plan = Self::function(Mode::Limit(k), &set, vec![(Target::Proposed(k), Some(&set)), (Target::Limited(k), None)]);
            plan.reads.extend([Source::LimitLast(k), Source::LimitAge(k), Source::Static]);
            let name = format!("limit:{}", site.name);
            Self::declare(&mut s, &pins, &terms, body, &name, &origin(&site.origin), &plan);
            functions.push(plan);
        }
        for (k, site) in body.stims().iter().enumerate() {
            let k = k as u32;
            let set = ReadSet::of(&body.dependencies(Query::Stim(k)));
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

        BodyPlan { shape: s.freeze(), functions }
    }

    /// What term `t` reads.
    fn term_set(body: Body<'_>, t: Term) -> ReadSet {
        match t {
            Term::Flow(b) => ReadSet::of(&body.dependencies(Query::Flow(b))),
            Term::Charge(b) => ReadSet::of(&body.dependencies(Query::Charge(b))),
            Term::Current(b) => ReadSet([Source::BranchFlow(b)].into_iter().collect()),
            Term::Eq(b) => {
                let mut set = ReadSet::of(&body.dependencies(Query::Flow(b)));
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
    fn function(mode: Mode, all: &ReadSet, writes: Vec<(Target, Option<&ReadSet>)>) -> FunctionPlan {
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
    fn declare(s: &mut ShapeBuf, pins: &Pins, terms: &[Term], body: Body<'_>, name: &str, origin: &str, plan: &FunctionPlan) {
        let limits = body.limits().len() as u32;
        let at = |kind: u32, index: u32, part: u32| Cell { kind, index, part };
        let value = |kind: u32, index: u32| at(kind, index, part::VALUE);
        let reads = plan
            .reads
            .iter()
            .map(|r| match *r {
                Source::Node(i) => value(cell::PIN, pins.node(i)),
                Source::BranchFlow(b) => value(cell::PIN, pins.branches[b as usize].unwrap_or(GROUND)),
                Source::LimitValue(k) => value(cell::INPUT, k),
                Source::StimValue(k) => value(cell::INPUT, limits + k),
                Source::Param(i) => value(cell::PIN, i),
                Source::Temperature => value(cell::PIN, pins.temperature.unwrap_or(GROUND)),
                Source::Gmin => value(cell::AMBIENT, ambient::GMIN),
                Source::SourceScale => value(cell::AMBIENT, ambient::SOURCE_SCALE),
                Source::LimitLast(k) => at(cell::INPUT, k, part::LAST),
                Source::LimitAge(k) => at(cell::INPUT, k, part::AGE),
                Source::Static => value(cell::AMBIENT, ambient::STATIC),
            })
            .collect();
        let writes = plan
            .writes
            .iter()
            .map(|w| match *w {
                Target::Term(t) => value(cell::TERM, terms.iter().position(|x| *x == t).unwrap_or(usize::MAX) as u32),
                Target::Proposed(k) => at(cell::INPUT, k, part::PROPOSED),
                Target::Limited(k) => at(cell::INPUT, k, part::LIMITED),
                Target::StimRe(k) => at(cell::INPUT, limits + k, part::RE),
                Target::StimIm(k) => at(cell::INPUT, limits + k, part::IM),
            })
            .collect();
        let partials =
            plan.partials.iter().map(|(w, slot)| Partial { write: *w as u32, to: 0, read: *slot as u32, at: 0 }).collect();
        s.function(FunctionSpec { name, origin, reads, writes, partials, ..FunctionSpec::default() });
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

    /// Whether any target of the body reads `temperature()`.
    fn reads_temperature(body: Body<'_>) -> bool {
        Self::queries(body).iter().any(|q| body.dependencies(*q).env.contains(&Env::Temperature))
    }

    /// Every dependency query of a body: each branch's two parts, each site, each param.
    fn queries(body: Body<'_>) -> Vec<Query> {
        let n = |len: usize| 0..len as u32;
        n(body.branches().len())
            .flat_map(|b| [Query::Flow(b), Query::Charge(b)])
            .chain(n(body.limits().len()).map(Query::Limit))
            .chain(n(body.stims().len()).map(Query::Stim))
            .chain(n(body.params().len()).map(Query::Check))
            .collect()
    }

    /// The shape every instance of this body gets.
    pub fn shape(&self) -> &ShapeRef {
        &self.shape
    }
}
