//! The generic device: one `BodyFactory` per body, leaked once and turned into a `Module` by
//! `thunk::module_with`; each instance it creates (`Interpreted`) runs the body's functions by
//! walking its `analog` block.

use std::sync::Arc;

use piperine_abi::device::{Eval, Factory, Fault, Flow, React, Values};
use piperine_abi::raw::Shape;
use piperine_abi::shape::ShapeRef;
use piperine_ir::{Body, BodyId, BranchKind, Ir, ValueType};

use crate::dual::Dual;
use crate::plan::{BodyPlan, FunctionPlan, Mode, Source, Target, Term};
use crate::walk::{Frame, Sums, Value, Walk};

/// The factory of one body: the design, the body, its plan.
#[derive(Debug)]
pub struct BodyFactory {
    pub(crate) ir: Arc<Ir>,
    pub(crate) body: BodyId,
    pub(crate) plan: Arc<BodyPlan>,
}

/// One instance of a body. It holds no state between calls (abi.md F1).
#[derive(Debug)]
pub struct Interpreted {
    pub(crate) ir: Arc<Ir>,
    pub(crate) body: BodyId,
    pub(crate) plan: Arc<BodyPlan>,
}

impl Interpreted {
    /// The fault code of a function the interpreter does not run.
    pub const UNRUN: u32 = 2;

    /// The plan of the instance's body.
    pub fn plan(&self) -> &BodyPlan {
        &self.plan
    }

    fn body(&self) -> Body<'_> {
        self.ir.body(self.body)
    }

    /// The frame and params of function `f` from the call's reads: each differentiable read
    /// seeded in its slot, every other read a plain value; a cell `f` does not read is `None`
    /// (a real param `NaN`, see `walk`).
    fn inputs(&self, f: &FunctionPlan, cx: &Eval<'_>) -> (Frame, Vec<Value>) {
        let body = self.body();
        let mut frame = Frame {
            nodes: vec![None; body.nodes().len()],
            flows: vec![None; body.branches().len()],
            limits: vec![None; body.limits().len()],
            stims: vec![None; body.stims().len()],
            ..Frame::default()
        };
        let mut params: Vec<Value> = body
            .params()
            .iter()
            .map(|p| if p.ty == ValueType::Bool { Value::Unit } else { Value::Real(Dual::constant(f64::NAN)) })
            .collect();
        for (i, source) in f.reads.iter().enumerate() {
            let read = || if i < f.slots { Dual::seeded(cx.f64(i), i, f.slots) } else { Dual::constant(cx.f64(i)) };
            let cell = match *source {
                Source::Node(n) => frame.nodes.get_mut(n as usize),
                Source::BranchFlow(b) => frame.flows.get_mut(b as usize),
                Source::LimitValue(k) => frame.limits.get_mut(k as usize),
                Source::StimValue(k) => frame.stims.get_mut(k as usize),
                Source::Temperature => Some(&mut frame.temperature),
                Source::Gmin => Some(&mut frame.gmin),
                Source::SourceScale => Some(&mut frame.source_scale),
                Source::Param(p) => {
                    if let Some(slot) = params.get_mut(p as usize) {
                        let bool_param = body.params().get(p as usize).is_some_and(|x| x.ty == ValueType::Bool);
                        *slot = if bool_param { Value::Bool(cx.bool(i)) } else { Value::Real(read()) };
                    }
                    None
                }
                Source::LimitLast(_) | Source::LimitAge(_) | Source::Static => None,
            };
            if let Some(cell) = cell {
                *cell = Some(read());
            }
        }
        (frame, params)
    }

    /// The value of term `t` after a `Terms` walk.
    fn term(&self, t: Term, sums: &Sums, frame: &Frame) -> Result<Dual, Fault> {
        let body = self.body();
        let sum = |list: &[Option<Dual>], b: u32| list.get(b as usize).cloned().flatten();
        let missing = |what: &str, b: u32| {
            Fault::new(Walk::UNEXECUTED, format!("{}: branch {b} has no {what} sum after the walk", body.name()))
        };
        Ok(match t {
            Term::Flow(b) => sum(&sums.direct, b).ok_or_else(|| missing("direct", b))?,
            Term::Charge(b) => {
                let q = sum(&sums.ddt, b).ok_or_else(|| missing("ddt", b))?;
                let potential = body.branches().get(b as usize).is_some_and(|x| x.kind == BranchKind::Potential);
                if potential { -q } else { q }
            }
            Term::Current(b) => frame.flow(b),
            Term::Eq(b) => {
                let branch = &body.branches()[b as usize];
                let v = frame.probe(branch.plus, branch.minus);
                match sum(&sums.direct, b) {
                    Some(s) => v - s,
                    None => v,
                }
            }
        })
    }

    /// The `terms` function: one walk, each term's value and its seeded partials.
    fn terms(&self, f: &FunctionPlan, cx: &mut Eval<'_>) -> Result<(), Fault> {
        let (frame, params) = self.inputs(f, cx);
        let mut walk = Walk::analog(self.body(), Mode::Terms, frame, params);
        walk.run()?;
        let Some((sums, frame)) = walk.finish() else {
            return Err(Fault::new(Walk::UNEXECUTED, "the walk lost its frame"));
        };
        let mut values = Vec::with_capacity(f.writes.len());
        for w in &f.writes {
            let Target::Term(t) = *w else {
                return Err(Fault::new(Walk::UNEXECUTED, format!("{}: the terms function writes {w:?}", self.body().name())));
            };
            values.push(self.term(t, &sums, &frame)?);
        }
        for (j, v) in values.iter().enumerate() {
            cx.set(j, v.v);
        }
        for (p, (w, slot)) in f.partials.iter().enumerate() {
            if cx.wanted(p) {
                cx.jac()[p] = values[*w].tangent(*slot);
            }
        }
        Ok(())
    }
}

impl Factory for BodyFactory {
    type Instance = Interpreted;

    fn prototype(&self) -> &Shape<'_> {
        self.plan.shape.shape()
    }

    /// Every instance of a body gets the plan's shape: no M1 param is structural (abi.md
    /// concept 2). The values seed the param nets; the solver, not the shape, holds them (P5).
    fn create(&self, _values: &Values<'_>) -> Result<(Interpreted, ShapeRef), Fault> {
        let instance = Interpreted { ir: Arc::clone(&self.ir), body: self.body, plan: Arc::clone(&self.plan) };
        Ok((instance, Arc::clone(&self.plan.shape)))
    }

    fn eval(instance: &Interpreted, k: u32, cx: &mut Eval<'_>) -> Result<(), Fault> {
        let name = instance.body().name();
        let Some(f) = instance.plan.functions.get(k as usize) else {
            return Err(Fault::new(Interpreted::UNRUN, format!("{name}: function {k} does not exist")));
        };
        match f.mode {
            Mode::Terms => instance.terms(f, cx),
            Mode::Limit(_) | Mode::Stim(_) => {
                Err(Fault::new(Interpreted::UNRUN, format!("{name}: function {k} ({:?}) is not run by the interpreter yet", f.mode)))
            }
        }
    }

    /// No M1 body has a reaction (the digital side is M2).
    fn react(instance: &Interpreted, k: u32, _cx: &mut React<'_>) -> Result<Flow, Fault> {
        let name = instance.ir.body(instance.body).name();
        Err(Fault::new(Interpreted::UNRUN, format!("{name}: reaction {k} does not exist")))
    }
}
