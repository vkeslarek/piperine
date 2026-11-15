//! The plan of one body: everything computed once per body (design "From a body to a shape").
//! Pins come first, in this order: one `PARAM` pin per param, one `PORT` continuous pin per body
//! node, one `PRIVATE` continuous pin per potential branch (its flow unknown), and one `GLOBAL`
//! `TEMPERATURE` pin when the body reads `temperature()`. Terms follow the branches: a flow
//! branch has a `FLOW` term for its direct addends and a `DDT` term for its `ddt` addends; a
//! potential branch has the two `FLOW` terms of abi.md A1, and a `DDT` term `{f, GROUND}` when it
//! has `ddt` addends. Inputs: one `LIMIT` per `limit` site, then one `STIM` per `ac_stim` site.

use piperine_abi::layout::{BIT, F64};
use piperine_abi::raw::{DEFAULT, GROUND, global, input, op, pin_flag, role};
use piperine_abi::shape::{InputSpec, PinSpec, ShapeBuf, ShapeRef, TermSpec};
use piperine_ir::{Body, BranchKind, Constant, Env, Operator, Origin, Stmt, Target as Query, Terminal, ValueType};

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
#[allow(dead_code, reason = "`pins` and `terms` are read by the function plan, T6")]
pub struct BodyPlan {
    pub(crate) shape: ShapeRef,
    pub(crate) pins: Pins,
    /// What each term of the shape computes, in the shape's order.
    pub(crate) terms: Vec<Term>,
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

        BodyPlan { shape: s.freeze(), pins, terms }
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
