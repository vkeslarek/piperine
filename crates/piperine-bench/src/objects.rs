//! Value-layer objects a bench can hold and call methods on: net/instance
//! handles produced by name resolution, and [`OpResult`] — the immutable
//! snapshot `$op()` returns (SPEC_BENCH.md §4/§6).

use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;

use piperine_codegen::device::CircuitBuildInfo;
use piperine_lang::eval::{EvalError, Object, Value};
use piperine_solver::analog::{BranchIdentifier, NodeIdentifier};
use piperine_solver::analysis::dc::DcAnalysisResult;

/// A resolved top-level net (SPEC_BENCH.md §3 bare-name resolution). The
/// argument type `.v`/`.i` expect.
#[derive(Debug, Clone)]
pub struct NetRef {
    pub name: String,
}

impl Object for NetRef {
    fn type_name(&self) -> &str {
        "Net"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn call_method(&self, name: &str, _args: Vec<Value>) -> Result<Value, EvalError> {
        Err(EvalError::Undefined(format!("method `{name}` on a Net")))
    }
}

/// A resolved top-level instance. Field access (`resistor.p`) resolves to
/// the port's connected [`NetRef`] or a param's current (staged-or-default)
/// value — both baked in at construction time by
/// [`crate::host::SimHost::lookup`], which has the `Design` access needed
/// to resolve them (SPEC_BENCH.md §3).
#[derive(Debug, Clone)]
pub struct InstanceRef {
    pub label: String,
    /// port name → connected top-level net name
    pub ports: HashMap<String, String>,
    /// param name → current value (staged override, else the elaborated default)
    pub params: HashMap<String, Value>,
}

impl Object for InstanceRef {
    fn type_name(&self) -> &str {
        "Instance"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn call_method(&self, name: &str, _args: Vec<Value>) -> Result<Value, EvalError> {
        if let Some(net) = self.ports.get(name) {
            return Ok(Value::Object(Rc::new(NetRef { name: net.clone() })));
        }
        if let Some(value) = self.params.get(name) {
            return Ok(value.clone());
        }
        Err(EvalError::Undefined(format!("`{name}` is not a port or param of `{}`", self.label)))
    }
}

/// A selection of instances returned by `select("...")` in expression
/// position (SPEC_BENCH.md §7/§13). Holds the matched instance labels plus a
/// snapshot of each instance's params at `select()` time — result objects
/// must be `'static`, so it cannot borrow the `Design`.
///
/// Staging via a held selection (`s.ctrl = 1`) re-runs against the *live*
/// design through `SimHost::assign_field_on`. Field-reads (`.resistance`)
/// return the snapshot (milestone-1 liveness caveat: re-staging after
/// `select()` is not reflected in a field-read). Field-reads always yield a
/// `List` — one value per instance, no singleton-scalar coercion.
#[derive(Debug)]
pub struct SelectionRef {
    pub labels: Vec<String>,
    /// param snapshot per instance, parallel to `labels`.
    params: Vec<HashMap<String, Value>>,
}

impl SelectionRef {
    pub fn new(labels: Vec<String>, params: Vec<HashMap<String, Value>>) -> Self {
        Self { labels, params }
    }
}

impl Object for SelectionRef {
    fn type_name(&self) -> &str {
        "Selection"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn call_method(&self, name: &str, _args: Vec<Value>) -> Result<Value, EvalError> {
        match name {
            "len" => Ok(Value::Nat(self.labels.len() as u64)),
            "labels" => Ok(Value::List(Rc::new(std::cell::RefCell::new(
                self.labels.iter().map(|s| Value::Str(s.clone())).collect(),
            )))),
            // A field-read (`s.resistance`, no parens) → snapshot per
            // instance, as a List (always a List, even for a singleton).
            _ => {
                let values: Result<Vec<Value>, EvalError> = self
                    .params
                    .iter()
                    .map(|p| {
                        p.get(name).cloned().ok_or_else(|| {
                            EvalError::Undefined(format!(
                                "`{name}` is not a param of every selected instance"
                            ))
                        })
                    })
                    .collect();
                Ok(Value::List(Rc::new(std::cell::RefCell::new(values?))))
            }
        }
    }
}

/// The immutable snapshot returned by `$op()` (SPEC_BENCH.md §4/§6): DC
/// operating-point node potentials and branch currents, read by name
/// through [`CircuitBuildInfo`].
pub struct OpResult {
    dc: DcAnalysisResult,
    info: Rc<CircuitBuildInfo>,
}

impl std::fmt::Debug for OpResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpResult").finish_non_exhaustive()
    }
}

impl OpResult {
    pub fn new(dc: DcAnalysisResult, info: Rc<CircuitBuildInfo>) -> Self {
        Self { dc, info }
    }

    fn resolve_node(&self, arg: &Value) -> Result<NodeIdentifier, EvalError> {
        match arg {
            Value::Object(obj) => {
                let net = obj
                    .as_any()
                    .downcast_ref::<NetRef>()
                    .ok_or_else(|| EvalError::TypeMismatch(format!("expected a Net, got {}", obj.type_name())))?;
                if net.name == "gnd" || net.name == "GND" || net.name == "vss" || net.name == "VSS" {
                    return Ok(NodeIdentifier::Gnd);
                }
                self.info
                    .nets
                    .get(&net.name)
                    .cloned()
                    .ok_or_else(|| EvalError::Undefined(format!("net `{}` is not addressable", net.name)))
            }
            other => Err(EvalError::TypeMismatch(format!("expected a Net, got {}", other.type_name()))),
        }
    }

    fn v(&self, args: &[Value]) -> Result<Value, EvalError> {
        let a = self.resolve_node(args.first().ok_or_else(|| EvalError::TypeMismatch("v() needs at least 1 argument".into()))?)?;
        let va = if a == NodeIdentifier::Gnd { 0.0 } else { self.dc.get_node(&a).unwrap_or(0.0) };
        let vb = match args.get(1) {
            Some(b) => {
                let b = self.resolve_node(b)?;
                if b == NodeIdentifier::Gnd { 0.0 } else { self.dc.get_node(&b).unwrap_or(0.0) }
            }
            None => 0.0,
        };
        Ok(Value::Real(va - vb))
    }

    /// The unique two-terminal instance whose ports connect exactly to
    /// `(a, b)` — the branch a bare `.i(net_a, net_b)` names (SPEC_BENCH.md
    /// §14 node-reference question, resolved: the instance-port form
    /// `r.i(resistor.p, resistor.n)` is unambiguous by construction since
    /// both args already name the same instance; this two-net form is
    /// provided for completeness and errors on any ambiguity).
    fn find_branch_instance(&self, a: NodeIdentifier, b: NodeIdentifier) -> Result<&piperine_codegen::device::BuiltInstanceInfo, EvalError> {
        let matches: Vec<_> = self
            .info
            .instances
            .iter()
            .filter(|inst| {
                inst.terminals.len() == 2
                    && ((inst.terminals[0] == a && inst.terminals[1] == b)
                        || (inst.terminals[0] == b && inst.terminals[1] == a))
            })
            .collect();
        match matches.as_slice() {
            [one] => Ok(one),
            [] => Err(EvalError::TypeMismatch("no two-terminal instance connects those nets".into())),
            _ => Err(EvalError::TypeMismatch("more than one instance connects those nets — use the instance-port form".into())),
        }
    }

    fn i(&self, args: &[Value]) -> Result<Value, EvalError> {
        if args.is_empty() || args.len() > 2 {
            return Err(EvalError::TypeMismatch("i() takes 1 or 2 arguments".into()));
        }
        let a = self.resolve_node(&args[0])?;
        // `i(a)` — the omitted second terminal is ground (SPEC_BENCH §6).
        let b = match args.get(1) {
            Some(v) => self.resolve_node(v)?,
            None => NodeIdentifier::Gnd,
        };
        let instance = self.find_branch_instance(a.clone(), b)?;
        if instance.num_forces > 0 {
            let branch = BranchIdentifier::new(instance.label.clone(), "force0".to_string());
            return Ok(Value::Real(self.dc.get_branch(branch).unwrap_or(0.0)));
        }
        let volts: Vec<f64> = instance
            .terminals
            .iter()
            .map(|t| if *t == NodeIdentifier::Gnd { 0.0 } else { self.dc.get_node(t).unwrap_or(0.0) })
            .collect();
        let mut residual = vec![0.0; instance.terminals.len()];
        let sim = piperine_codegen::SimCtx::default();
        instance.kernel.eval_residual(&volts, &instance.params, &[], &[], &sim, &mut residual);
        // Sign convention: positive current flows from terminal `a` into
        // the device; `residual[0]` is the current out of terminal 0
        // (SPEC_BENCH.md §4 `.i(a, b)` — positive a → b).
        let current = if instance.terminals[0] == a { residual[0] } else { -residual[0] };
        Ok(Value::Real(current))
    }
}

impl Object for OpResult {
    fn type_name(&self) -> &str {
        "OpResult"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn call_method(&self, name: &str, args: Vec<Value>) -> Result<Value, EvalError> {
        match name {
            "v" => self.v(&args),
            "i" => self.i(&args),
            other => Err(EvalError::Undefined(format!("method `{other}` on OpResult"))),
        }
    }
}
