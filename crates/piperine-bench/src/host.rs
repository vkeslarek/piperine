//! [`SimHost`] — the effectful [`Host`] backing a `bench`: resolves bare
//! names against the bench's module (piperine-bench/docs/SPEC.md §3), dispatches system
//! tasks (bench-only first, then the shared pure registry), and stages
//! assignments as POM overrides (§6.2).

use std::collections::HashMap;
use std::rc::Rc;

use piperine_lang::eval::tasks::dispatch_pure;
use piperine_lang::eval::{Callable, EvalError, Host, TaskRegistry, Value};
use piperine_lang::parse::ast::Expr;

use crate::objects::{InstanceRef, NetRef, SelectionRef};
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

    /// A `Record` of `bundle`'s field defaults, recursively: a field whose
    /// default is itself a bundle literal (`solver : Solver = Solver {}`)
    /// resolves to that bundle's own defaults overlaid with the literal's
    /// named fields. Fields with no default are simply absent — reading
    /// one from an under-filled literal is a fail-loud `Undefined`.
    fn bundle_defaults(&self, bundle: &str) -> Option<Value> {
        use piperine_lang::parse::ast::Expr;
        let decl = self.session.design().bundle(bundle)?;
        let env = piperine_lang::elab::const_eval::ConstEnv::new();
        let mut fields = HashMap::new();
        for field in &decl.fields {
            let Some(default) = &field.default else { continue };
            let value = match default {
                Expr::BundleLit { ty, fields: overrides } => {
                    let Some(Value::Record { fields: inner, .. }) = self.bundle_defaults(&ty.name) else {
                        continue;
                    };
                    for (name, expr) in overrides {
                        if let Ok(v) = env.eval(expr) {
                            inner.borrow_mut().insert(name.clone(), v);
                        }
                    }
                    Value::Record { ty: ty.name.clone(), fields: inner }
                }
                other => match other {
                    // `Map {}` default — the empty map is a compile-time
                    // constant (a non-empty Map literal holds Net keys, not
                    // const, and is never a declared default).
                    Expr::MapLit(entries) if entries.is_empty() => {
                        Value::Map(Rc::new(std::cell::RefCell::new(vec![])))
                    }
                    other => match env.eval(other) {
                        Ok(v) => v,
                        Err(_) => continue,
                    },
                },
            };
            fields.insert(field.name.clone(), value);
        }
        Some(Value::Record {
            ty: bundle.to_string(),
            fields: Rc::new(std::cell::RefCell::new(fields)),
        })
    }
}

impl Host for SimHost {
    fn context_name(&self) -> &'static str {
        "a bench"
    }

    fn lookup(&mut self, name: &str) -> Option<Value> {
        // Interpreter protocol: `bundle:<Name>` asks for a Record of the
        // bundle's field defaults, so an omitted field in a bundle literal
        // (`OpConfig {}`) falls back to its declaration (SPEC §6.5).
        if let Some(bundle_name) = name.strip_prefix("bundle:") {
            return self.bundle_defaults(bundle_name);
        }
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
        // The bench module's own params, staged-override first (bench spec
        // §3 item 2 lists nets, instances, *and params*).
        if let Some(p) = module.params.iter().find(|p| p.name == name) {
            return self
                .session
                .design()
                .get_override("", &p.name)
                .or_else(|| p.value());
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
        // Sibling bench fns first (bench spec §2 "fn helper(x: T) -> U") —
        // effectful, so a helper may run analyses and stage overrides.
        if let Some(bench) = self.session.design().bench(self.session.module())
            && let Some(f) = bench.fn_by_name(name)
        {
            use piperine_lang::parse::ast::FnParam;
            let mut params = Vec::new();
            let mut defaults = Vec::new();
            for p in &f.sig.params {
                if let FnParam::Typed { name, default, .. } = p {
                    params.push(name.clone());
                    defaults.push(default.clone());
                }
            }
            return Some(Callable::BenchFn {
                params,
                defaults,
                body: f.body.stmts.clone(),
                tail: f.body.expr.as_deref().cloned(),
            });
        }
        let f = self.session.design().function(name)?;
        Some(Callable::Function {
            params: f.params().iter().map(|(n, _)| n.clone()).collect(),
            defaults: f.defaults().to_vec(),
            body: f.body().to_vec(),
        })
    }

    fn resolve_method(&mut self, ty: &str, method: &str) -> Option<Callable> {
        let m = self
            .session
            .design()
            .impls()
            .iter()
            .filter(|i| i.ty == ty)
            .flat_map(|i| i.methods.iter())
            .find(|m| m.name == method)?;
        Some(Callable::Function {
            params: m.params.iter().map(|(n, _)| n.clone()).collect(),
            defaults: m.defaults.clone(),
            body: m.body.to_vec(),
        })
    }

    fn call_host_fn(&mut self, name: &str, args: &[Value]) -> Option<Result<Value, EvalError>> {
        if name == "select" {
            Some(self.eval_select(args))
        } else {
            None
        }
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
            // `sw.ctrl = 1` — bare-name staging on one instance.
            if let Expr::Ident(label) = base.as_ref()
                && self.resolve_instance(label).is_some()
            {
                // One Value type end to end (SIMPLIFICATION.md P2) — a
                // staged override is the interpreter's value verbatim.
                // Non-scalars are rejected fail-loud when the next
                // analysis applies overrides.
                self.session.stage(label, param, value.clone());
                return Ok(true);
            }
            // `select("//resistor").resistance = 2e6` — bulk staging
            // across a selection (bench spec §7). The selector path must
            // be a string literal (milestone 1).
            if let Expr::Call(callee, sel_args) = base.as_ref()
                && let Expr::Ident(name) = callee.as_ref()
                && name == "select"
            {
                let Some(Expr::Literal(piperine_lang::parse::ast::Literal::String(path))) =
                    sel_args.first()
                else {
                    return Err(EvalError::TypeMismatch(
                        "select(...) staging takes a string-literal path".into(),
                    ));
                };
                return self.stage_selection(path, param, value);
            }
        }
        Ok(false)
    }

    fn assign_field_on(&mut self, target: &Value, field: &str, value: &Value) -> Result<bool, EvalError> {
        // `s.ctrl = 1` where `s` holds a `SelectionRef`: stage on every
        // instance in the (live) selection.
        if let Value::Object(obj) = target
            && let Some(sel) = obj.as_any().downcast_ref::<SelectionRef>()
        {
            if sel.labels.is_empty() {
                return Err(EvalError::Host(format!(
                    "cannot stage `{field}` on an empty selection"
                )));
            }
            for label in &sel.labels {
                self.session.stage(label, field, value.clone());
            }
            return Ok(true);
        }
        Ok(false)
    }
}

impl SimHost {
    /// Evaluate `select("path")` in expression position (piperine-bench/docs/SPEC.md
    /// §7/§13): return a [`SelectionRef`] of the matched instance labels
    /// with a param snapshot per instance.
    fn eval_select(&mut self, args: &[Value]) -> Result<Value, EvalError> {
        let Some(Value::Str(path)) = args.first() else {
            return Err(EvalError::TypeMismatch(
                "select(...) takes a string-literal path".into(),
            ));
        };
        use piperine_lang::pom::node::Node;
        let mut design = self.session.design().clone();
        design.set_top(self.session.module());
        let selection = design
            .select(path)
            .map_err(|e| EvalError::Host(format!("select(\"{path}\"): {e}")))?;
        let mut labels = Vec::new();
        let mut params = Vec::new();
        for node in selection.iter() {
            if let Node::Instance(inst) = node {
                let label = inst.name().to_string();
                let snap = self.resolve_instance(&label).map(|i| i.params).unwrap_or_default();
                labels.push(label);
                params.push(snap);
            }
        }
        // An empty selection is valid (not an error) — the spec says results
        // may be empty; callers use `.len()` or `.one()` to handle it.
        Ok(Value::Object(Rc::new(SelectionRef::new(labels, params))))
    }

    /// Stage `param = value` on every instance a selector path matches —
    /// fail-loud when the selection is empty or matches nothing stageable.
    fn stage_selection(&mut self, path: &str, param: &str, value: &Value) -> Result<bool, EvalError> {
        use piperine_lang::pom::node::Node;
        let mut design = self.session.design().clone();
        // Selector evaluation roots relative paths at the design top;
        // point it at the bench's module.
        design.set_top(self.session.module());
        let selection = design
            .select(path)
            .map_err(|e| EvalError::Host(format!("select(\"{path}\"): {e}")))?;
        let labels: Vec<String> = selection
            .iter()
            .filter_map(|node| match node {
                Node::Instance(inst) => Some(inst.name().to_string()),
                _ => None,
            })
            .collect();
        if labels.is_empty() {
            return Err(EvalError::Host(format!(
                "select(\"{path}\") matched no instances to stage `{param}` on"
            )));
        }
        for label in labels {
            self.session.stage(&label, param, value.clone());
        }
        Ok(true)
    }
}
