//! The generic device: one `BodyFactory` per body, leaked once and turned into a `Module` by
//! `thunk::module_with`; each instance it creates (`Interpreted`) runs the body's functions by
//! walking its `analog` block.

use std::sync::Arc;

use piperine_abi::device::{Eval, Factory, Fault, Flow, React, Values};
use piperine_abi::layout::{HIGH, LOW};
use piperine_abi::raw::Shape;
use piperine_abi::shape::ShapeRef;
use piperine_ir::{Body, BodyId, BranchKind, Ir, ValueType};

use crate::dual::Dual;
use piperine_plan::{BodyPlan, FunctionPlan, Mode, Source, Target, Term};
use crate::walk::{Frame, Held, Sums, Value, Walk};

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
    /// and a param it does not read `Value::Unread` (see `walk`). With `pins_at_zero`, every continuous pin reads 0 (its
    /// tangent kept): the walk of the affine rule.
    fn inputs(&self, f: &FunctionPlan, cx: &Eval<'_>, pins_at_zero: bool) -> (Frame, Vec<Value>) {
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
            .map(|_| Value::Unread)
            .collect();
        for (i, source) in f.reads.iter().enumerate() {
            if matches!(source, Source::LimitLast(_) | Source::LimitAge(_) | Source::Static) {
                continue; // the site's own reads (`Held`), not values of the block
            }
            let read = || {
                let v = if pins_at_zero && source.is_pin() { 0.0 } else { cx.f64(i) };
                if i < f.slots { Dual::seeded(v, i, f.slots) } else { Dual::constant(v) }
            };
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
                // `now()`: `Interpreter::new` refuses every body that reads it (msl-digital T22).
                Source::Time => None,
                // `freq()` and `analysis(..)`: not a frame cell; the walk refuses the read by name.
                Source::Freq | Source::Analysis(_) | Source::Fired(_) | Source::TemperatureGiven => None,
                // the tick length of a reaction: no body with a reaction runs here (R1)
                // the settle pin of a `settle` event: refused by name (§13 rule 17)
                Source::Settle => None,
                // A carrier read: `Interpreter::new` refuses every body with a carrier (R1).
                Source::Carrier(_) | Source::CarrierChanged(_) | Source::Delayed(_) | Source::DelayedChanged(_) | Source::Crossed(_) => None,
                // a timer's pin, a previous value: `Interpreter::new` refuses every body with an
                // `at` or a carrier (msl-digital T22).
                Source::Timer(_) | Source::CarrierPrevious(_) => None,
                // an array carrier, whole or one element: refused by name in the same way (composite-values R1)
                Source::Whole(..) | Source::Element(..) => None,
                // a derivative site's rate: no body that has one reaches the interpreter, which
                // refuses it by name (analog-structure R1, AS-17).
                Source::Derivative(_) => None,
                // a delay site's output: no body that has one reaches the interpreter, which
                // refuses it by name (analog-operators R1, AO-11)
                Source::Delay(_) => None,
                // a noise source's input: refused by name likewise (AO-11)
                Source::NoiseValue(_) => None,
                // a session parameter: refused by name likewise (AO-11)
                Source::SessionParameter(_) => None,
                // a param's previous value or changed byte: refused by name likewise (T15)
                Source::ParamPrevious(_) | Source::ParamChanged(_) => None,
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
        let unread = |what: &str, b: u32| {
            Fault::new(Walk::UNDECLARED, format!("{}: branch {b}'s {what} is not a read of its function", body.name()))
        };
        Ok(match t {
            Term::Flow(b) => sum(&sums.direct, b).ok_or_else(|| missing("direct", b))?,
            Term::Charge(b) => {
                let q = sum(&sums.ddt, b).ok_or_else(|| missing("ddt", b))?;
                let potential = body.branches().get(b as usize).is_some_and(|x| x.kind == BranchKind::Potential);
                if potential { -q } else { q }
            }
            Term::Current(b) => frame.flow(b).ok_or_else(|| unread("flow pin", b))?,
            // the interpreter refuses a body with a derivative site by name (R1, AS-17)
            Term::DerivativeRate(k) | Term::DerivativeState(k) => {
                return Err(Fault::new(Walk::UNEXECUTED, format!("{}: derivative site {k} is not run by the interpreter", body.name())));
            }
            // the interpreter refuses a body with a delay site by name (R1, AO-11)
            Term::DelayOutput(k) | Term::DelayHistory(k) => {
                return Err(Fault::new(Walk::UNEXECUTED, format!("{}: delay site {k} is not run by the interpreter", body.name())));
            }
            Term::Eq(b) => {
                let branch = &body.branches()[b as usize];
                let v = frame.probe(branch.plus, branch.minus).ok_or_else(|| unread("probe", b))?;
                match sum(&sums.direct, b) {
                    Some(s) => v - s,
                    None => v,
                }
            }
        })
    }

    /// The `terms` function. A term the plan marks affine (INT-12) is written as `c + Σ g_j·x_j`:
    /// `c` and the pin partials `g` from a walk with every continuous pin read as 0, the sum
    /// folded from `0.0` over its pin partials in declared order, as the assembly folds the
    /// right side (`piperine-analog-solver`'s `Linear::dot`), so a linear row leaves no
    /// round-off there. Every other value and partial comes from the walk at the iterate.
    fn terms(&self, f: &FunctionPlan, cx: &mut Eval<'_>) -> Result<(), Fault> {
        let pin = |slot: usize| f.reads.get(slot).is_some_and(|r| r.is_pin());
        let affine = |w: usize| f.affine.get(w).copied().unwrap_or(false);
        let at_iterate = (0..f.writes.len()).any(|w| !affine(w))
            || f.partials.iter().any(|(w, slot)| affine(*w) && !pin(*slot));
        let iterate = if at_iterate { Some(self.walk_terms(f, cx, false)?) } else { None };
        let zero = if f.affine.iter().any(|a| *a) { Some(self.walk_terms(f, cx, true)?) } else { None };
        let pick = |w: usize, slot: Option<usize>| -> Result<&Dual, Fault> {
            let from_zero = affine(w) && slot.is_none_or(pin);
            let walk = if from_zero { zero.as_ref() } else { iterate.as_ref() };
            walk.and_then(|values| values.get(w))
                .ok_or_else(|| Fault::new(Walk::UNEXECUTED, format!("write {w} has no walk to read from")))
        };
        for w in 0..f.writes.len() {
            let value = pick(w, None)?;
            let v = if affine(w) {
                let fold = f
                    .partials
                    .iter()
                    .filter(|(pw, slot)| *pw == w && pin(*slot))
                    .fold(0.0, |s, (_, slot)| s + value.tangent(*slot) * cx.f64(*slot));
                value.v + fold
            } else {
                value.v
            };
            cx.set(w, v);
        }
        for (p, (w, slot)) in f.partials.iter().enumerate() {
            if cx.wanted(p) {
                let g = pick(*w, Some(*slot))?.tangent(*slot);
                cx.jac()[p] = g;
            }
        }
        Ok(())
    }

    /// The function of `limit` site `k`: `PROPOSED` with its partials and `LIMITED`. Nothing is
    /// written when the walk faults.
    fn limit(&self, f: &FunctionPlan, k: u32, cx: &mut Eval<'_>) -> Result<(), Fault> {
        let name = self.body().name();
        let at = |s: Source| f.reads.iter().position(|r| *r == s);
        let (Some(last), Some(age), Some(is_static)) = (at(Source::LimitLast(k)), at(Source::LimitAge(k)), at(Source::Static))
        else {
            return Err(Fault::new(Walk::UNEXECUTED, format!("{name}: limit {k} does not read LAST, AGE and STATIC")));
        };
        let held = Held { last: cx.f64(last), age: cx.u8(age), is_static: cx.bool(is_static) };
        let (frame, params) = self.inputs(f, cx, false);
        let mut walk = Walk::analog(self.body(), Mode::Limit(k), frame, params);
        walk.hold(held);
        walk.run()?;
        let Some(result) = walk.limited().cloned() else {
            return Err(Fault::new(Walk::UNEXECUTED, format!("{name}: limit {k} wrote nothing")));
        };
        for (j, w) in f.writes.iter().enumerate() {
            match *w {
                Target::Proposed(_) => cx.set(j, result.proposed.v),
                Target::Limited(_) => cx.set(j, result.limited),
                other => return Err(Fault::new(Walk::UNEXECUTED, format!("{name}: limit {k} writes {other:?}"))),
            }
        }
        for (p, (_, slot)) in f.partials.iter().enumerate() {
            if cx.wanted(p) {
                cx.jac()[p] = result.proposed.tangent(*slot);
            }
        }
        Ok(())
    }

    /// The function of `ac_stim` site `k`: `RE` and `IM`.
    fn stim(&self, f: &FunctionPlan, k: u32, cx: &mut Eval<'_>) -> Result<(), Fault> {
        let name = self.body().name();
        let (frame, params) = self.inputs(f, cx, false);
        let mut walk = Walk::analog(self.body(), Mode::Stim(k), frame, params);
        walk.run()?;
        let Some((re, im)) = walk.phasor() else {
            return Err(Fault::new(Walk::UNEXECUTED, format!("{name}: stim {k} wrote nothing")));
        };
        for (j, w) in f.writes.iter().enumerate() {
            match *w {
                Target::StimRe(_) => cx.set(j, re),
                Target::StimIm(_) => cx.set(j, im),
                other => return Err(Fault::new(Walk::UNEXECUTED, format!("{name}: stim {k} writes {other:?}"))),
            }
        }
        Ok(())
    }

    /// The `where` of param `p`: the condition over the params it reads, written as a `BIT`.
    fn check(&self, f: &FunctionPlan, p: u32, cx: &mut Eval<'_>) -> Result<(), Fault> {
        let body = self.body();
        let Some(condition) = body.params().get(p as usize).and_then(|x| x.check) else {
            return Err(Fault::new(Walk::UNEXECUTED, format!("{}: param {p} has no `where`", body.name())));
        };
        let (_, params) = self.inputs(f, cx, false);
        let ok = Walk::new(self.ir.as_ref(), params, body.locals().len()).eval_bool(condition)?;
        cx.out_bytes(0).copy_from_slice(if ok { &HIGH } else { &LOW });
        Ok(())
    }

    /// One `Terms` walk: the value of each write of `f`, in order.
    fn walk_terms(&self, f: &FunctionPlan, cx: &Eval<'_>, pins_at_zero: bool) -> Result<Vec<Dual>, Fault> {
        let (frame, params) = self.inputs(f, cx, pins_at_zero);
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
        Ok(values)
    }
}

impl Factory for BodyFactory {
    type Instance = Interpreted;

    fn prototype(&self) -> &Shape<'_> {
        self.plan.shape().shape()
    }

    /// Every instance of a body gets the plan's shape: no M1 param is structural (abi.md
    /// concept 2). The values seed the param nets; the solver, not the shape, holds them (P5).
    fn create(&self, _values: &Values<'_>) -> Result<(Interpreted, ShapeRef), Fault> {
        let instance = Interpreted { ir: Arc::clone(&self.ir), body: self.body, plan: Arc::clone(&self.plan) };
        Ok((instance, Arc::clone(self.plan.shape())))
    }

    fn eval(instance: &Interpreted, k: u32, cx: &mut Eval<'_>) -> Result<(), Fault> {
        let name = instance.body().name();
        let Some(f) = instance.plan.functions().get(k as usize) else {
            return Err(Fault::new(Interpreted::UNRUN, format!("{name}: function {k} does not exist")));
        };
        match f.mode {
            Mode::Terms => instance.terms(f, cx),
            Mode::Limit(site) => instance.limit(f, site, cx),
            Mode::Stim(site) => instance.stim(f, site, cx),
            Mode::Check(param) => instance.check(f, param, cx),
            // `Interpreter::new` refuses every body with a carrier, so no digital function runs (R1).
            Mode::Cross(_) | Mode::Drive(_) | Mode::Event(_) | Mode::DelayedEvent(_) | Mode::Rearm { .. } => {
                Err(Fault::new(Interpreted::UNRUN, format!("{name}: function {k} is digital; the interpreter runs none")))
            }
            // `bound_step` and a `cross` tolerance: refused by name (T12).
            Mode::Cap(_) | Mode::Ttol(_) | Mode::Etol(_) | Mode::DerivativeAbstol(_) | Mode::DelayMaximum(_) | Mode::Noise(_) => {
                Err(Fault::new(Interpreted::UNRUN, format!("{name}: function {k} is digital; the interpreter runs none")))
            }
        }
    }

    /// No M1 body has a reaction (the digital side is M2).
    fn react(instance: &Interpreted, k: u32, _cx: &mut React<'_>) -> Result<Flow, Fault> {
        let name = instance.ir.body(instance.body).name();
        Err(Fault::new(Interpreted::UNRUN, format!("{name}: reaction {k} does not exist")))
    }
}
