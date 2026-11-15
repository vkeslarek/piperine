//! The generic device: one `BodyFactory` per body, leaked once and turned into a `Module` by
//! `thunk::module_with`; each instance it creates (`Interpreted`) runs the body's functions by
//! walking its `analog` block.

use std::sync::Arc;

use piperine_abi::device::{Eval, Factory, Fault, Flow, React, Values};
use piperine_abi::raw::Shape;
use piperine_abi::shape::ShapeRef;
use piperine_ir::{BodyId, Ir};

use crate::plan::BodyPlan;

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

    fn eval(instance: &Interpreted, k: u32, _cx: &mut Eval<'_>) -> Result<(), Fault> {
        let name = instance.ir.body(instance.body).name();
        Err(Fault::new(Interpreted::UNRUN, format!("{name}: function {k} is not run by the interpreter yet")))
    }

    /// No M1 body has a reaction (the digital side is M2).
    fn react(instance: &Interpreted, k: u32, _cx: &mut React<'_>) -> Result<Flow, Fault> {
        let name = instance.ir.body(instance.body).name();
        Err(Fault::new(Interpreted::UNRUN, format!("{name}: reaction {k} does not exist")))
    }
}
