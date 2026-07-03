//! [`SimHost`] — the effectful [`Host`] backing a `bench`: resolves bare
//! names against the bench's module (SPEC_BENCH.md §3), dispatches system
//! tasks (bench-only first, then the shared pure registry), and stages
//! assignments as POM overrides (§6.2).

use std::collections::HashMap;
use std::rc::Rc;

use piperine_lang::eval::tasks::dispatch_pure;
use piperine_lang::eval::{Callable, EvalError, Host, TaskRegistry, Value};
use piperine_lang::parse::ast::Expr;

use crate::objects::{InstanceRef, NetRef};
use crate::session::SimSession;
use crate::tasks::SimTaskRegistry;

const GROUND_NAMES: &[&str] = &["gnd", "GND", "vss", "VSS"];

pub struct SimHost {
    session: SimSession,
    tasks: TaskRegistry,
    sim_tasks: SimTaskRegistry,
}

impl SimHost {
    pub fn new(session: SimSession) -> Self {
        Self { session, tasks: TaskRegistry::with_builtins(), sim_tasks: SimTaskRegistry::with_builtins() }
    }

    pub fn session(&self) -> &SimSession {
        &self.session
    }

    /// Resolve `label` to its `InstanceRef` (port → net map, param → value
    /// map), or `None` if it isn't an instance of the bench's module.
    fn resolve_instance(&self, label: &str) -> Option<InstanceRef> {
        let module = self.session.design().module(self.session.module())?;
        let instance = module.instances.iter().find(|i| i.name() == label)?;
        let child = self.session.design().module(instance.module_name())?;

        let mut ports = HashMap::new();
        for (i, port) in child.ports.iter().enumerate() {
            if let Some(net) = instance.ports.get(i) {
                ports.insert(port.name.clone(), net.net.clone());
            }
        }

        let mut params = HashMap::new();
        for p in &child.params {
            let value = self
                .session
                .design()
                .get_override(label, &p.name)
                .or_else(|| p.value());
            if let Some(v) = value {
                params.insert(p.name.clone(), v);
            }
        }
        for (name, v) in &instance.params {
            params.insert(name.clone(), v.clone());
        }

        Some(InstanceRef { label: label.to_string(), ports, params })
    }
}

impl Host for SimHost {
    fn context_name(&self) -> &'static str {
        "a bench"
    }

    fn lookup(&mut self, name: &str) -> Option<Value> {
        if GROUND_NAMES.contains(&name) {
            return Some(Value::Object(Rc::new(NetRef { name: "gnd".to_string() })));
        }
        let module = self.session.design().module(self.session.module())?;
        if module.wires.iter().any(|w| w.name() == name) || module.ports.iter().any(|p| p.name == name) {
            return Some(Value::Object(Rc::new(NetRef { name: name.to_string() })));
        }
        if let Some(instance) = self.resolve_instance(name) {
            return Some(Value::Object(Rc::new(instance)));
        }
        if let Some(value) = self.session.design().const_(name) {
            return Some(value.clone());
        }
        let enum_values = self.session.design().enum_value_map();
        if let Some(v) = enum_values.get(name) {
            return Some(Value::Int(*v));
        }
        None
    }

    fn resolve_callable(&mut self, name: &str) -> Option<Callable> {
        let f = self.session.design().function(name)?;
        Some(Callable::Function {
            params: f.params().iter().map(|(n, _)| n.clone()).collect(),
            body: f.body().to_vec(),
        })
    }

    fn syscall(&mut self, name: &str, args: Vec<Value>) -> Result<Value, EvalError> {
        if let Some(task) = self.sim_tasks.lookup(name) {
            return task.run(args, &self.session);
        }
        dispatch_pure(&self.tasks, name, args)
            .unwrap_or_else(|| Err(EvalError::TaskUnavailable { name: name.to_string(), context: self.context_name() }))
    }

    fn assign(&mut self, target: &Expr, value: &Value) -> Result<bool, EvalError> {
        if let Expr::Field(base, param) = target {
            if let Expr::Ident(label) = base.as_ref() {
                if self.resolve_instance(label).is_some() {
                    // One Value type end to end (SIMPLIFICATION.md P2) — a
                    // staged override is the interpreter's value verbatim.
                    // Non-scalars are rejected fail-loud when the next
                    // analysis applies overrides.
                    self.session.stage(label, param, value.clone());
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}
