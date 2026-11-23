//! `Params`: the host's table of every instance's param values, and the recomputation of what
//! derives from a param a host sets (feature `host-ii`, X9, HII-18 to HII-20; LRM 23.1 rule 6). A
//! placement `Child(..) { r: rr }` gives the child a param that is an expression over its
//! parent's; the circuit holds each param as a net, so a `set` of `rr` alone would leave `r` at
//! the value it folded to at elaboration. `Params::set` walks the design's `DerivePlan` from the
//! changed param down (a parent before its children), asks the compiled device for each value,
//! and writes every changed param through the caller's writer, in instance order. A writer that
//! refuses one write undoes the writes already made: the set changes nothing.

use piperine_interp::{Dual, Value as Walked, Walk};
use piperine_ir::{Constant, Ir, ValueType};
use piperine_plan::{Binding, CompositeLayout, DerivePlan};
use piperine_solver::abi::Value;
use piperine_solver::prelude::ParamValue;
use piperine_transpile::Compiled;

use crate::failure::Failure;

/// What recomputes a derived param: the compiled device's generated function `k` over the
/// parent's params in declaration order (a `bool` as 0.0 or 1.0).
pub trait Derive {
    fn derive(&self, k: u32, params: &[f64]) -> f64;
}

impl Derive for Compiled {
    fn derive(&self, k: u32, params: &[f64]) -> f64 {
        Compiled::derive(self, k, params)
    }
}

/// A `Derive` that evaluates each placement expression with the reference interpreter's walk, so
/// a design that no backend has built (`piperine info`) still shows the values a `set` gives its
/// derived params: no `rustc`, the same expressions the generated functions compile.
pub struct Evaluated<'ir> {
    ir: &'ir Ir,
    bindings: Vec<Binding>,
}

impl<'ir> Evaluated<'ir> {
    /// The evaluator of `plan`'s bindings over `ir`.
    pub fn new(ir: &'ir Ir, plan: &DerivePlan) -> Evaluated<'ir> {
        Evaluated { ir, bindings: plan.bindings().to_vec() }
    }
}

impl Derive for Evaluated<'_> {
    /// NaN for a `k` the plan has no binding for, a count of params other than the parent's, or an
    /// expression the walk cannot evaluate: the caller reports it as a plan that does not match.
    fn derive(&self, k: u32, params: &[f64]) -> f64 {
        let Some(binding) = self.bindings.get(k as usize) else { return f64::NAN };
        let Some(parent) = self.ir.instance_at(&binding.parent) else { return f64::NAN };
        let declared = parent.body().params();
        if declared.len() != params.len() {
            return f64::NAN;
        }
        let values = declared.iter().zip(params).map(|(param, value)| if matches!(param.ty, ValueType::Bool) { Walked::Bool(*value != 0.0) } else { Walked::Real(Dual::constant(*value)) }).collect();
        match Walk::new(self.ir, values, 0).eval(binding.expr) {
            Ok(Walked::Real(real)) => real.v,
            Ok(Walked::Bool(flag)) => f64::from(u8::from(flag)),
            _ => f64::NAN,
        }
    }
}

/// One param a `set` changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Changed {
    pub instance: String,
    pub param: String,
    pub ty: ValueType,
    pub old: f64,
    pub value: f64,
    /// Whether the param was recomputed from another (not the one the host set).
    pub derived: bool,
}

impl Changed {
    /// `<instance>.<param>`.
    pub fn label(&self) -> String {
        format!("{}.{}", self.instance, self.param)
    }

    /// The value as the circuit takes it: what the host gave stays a real (as every `set` of a
    /// param has been), a recomputed `bool` is a bool.
    pub fn param_value(&self) -> ParamValue {
        self.encoded(self.value)
    }

    /// The value before the `set`, as the circuit takes it.
    pub fn old_value(&self) -> ParamValue {
        self.encoded(self.old)
    }

    /// The value as a scheduled `set` takes it.
    pub fn scheduled(&self) -> Value {
        match (&self.ty, self.derived) {
            (ValueType::Bool, true) => Value::Boolean(self.value != 0.0),
            (ty, _) if Changed::holds_integer(ty) => Value::Integer(self.value as i64),
            _ => Value::Real(self.value),
        }
    }

    fn encoded(&self, v: f64) -> ParamValue {
        match (&self.ty, self.derived) {
            (ValueType::Bool, true) => ParamValue::Bool(v != 0.0),
            (ty, _) if Changed::holds_integer(ty) => ParamValue::Int(v as i64),
            _ => ParamValue::Real(v),
        }
    }

    /// Whether a param of `ty` is an integer pin: an integer, or a unit enum held as its tag (PI-01, PI-02).
    fn holds_integer(ty: &ValueType) -> bool {
        ty.is_integer() || matches!(ty, ValueType::Enum(_))
    }
}

/// The bytes of one leaf of a composite param.
struct LeafValue;

impl LeafValue {
    /// `value` as the leaf `leaf` (a `leaf::*` constant) holds it, little-endian; the reason it cannot when it cannot.
    fn encode(leaf: u32, value: f64) -> Result<Vec<u8>, String> {
        let whole = |low: f64, high: f64| {
            if value.fract() == 0.0 && (low..=high).contains(&value) { Ok(value) } else { Err(format!("the leaf holds a whole number from {low} to {high}")) }
        };
        Ok(match leaf {
            piperine_abi::raw::leaf::F64 => value.to_le_bytes().to_vec(),
            piperine_abi::raw::leaf::BOOL => vec![u8::from(whole(0.0, 1.0)? == 1.0)],
            piperine_abi::raw::leaf::U8 => (whole(0.0, 255.0)? as u8).to_le_bytes().to_vec(),
            piperine_abi::raw::leaf::U16 => (whole(0.0, 65535.0)? as u16).to_le_bytes().to_vec(),
            piperine_abi::raw::leaf::U32 => (whole(0.0, 4294967295.0)? as u32).to_le_bytes().to_vec(),
            piperine_abi::raw::leaf::U64 => (whole(0.0, 18446744073709551615.0)? as u64).to_le_bytes().to_vec(),
            piperine_abi::raw::leaf::I8 => (whole(-128.0, 127.0)? as i8).to_le_bytes().to_vec(),
            piperine_abi::raw::leaf::I16 => (whole(-32768.0, 32767.0)? as i16).to_le_bytes().to_vec(),
            piperine_abi::raw::leaf::I32 => (whole(-2147483648.0, 2147483647.0)? as i32).to_le_bytes().to_vec(),
            piperine_abi::raw::leaf::I64 => (whole(-9223372036854775808.0, 9223372036854775807.0)? as i64).to_le_bytes().to_vec(),
            other => return Err(format!("leaf kind {other} has no host value")),
        })
    }
}

/// The values of every param of every instance, by instance in the IR's order.
#[derive(Debug, Clone)]
pub struct Params {
    /// What derives from what.
    plan: DerivePlan,
    /// The params of every instance.
    instances: Vec<Slot>,
    /// The integers a host gave that no `f64` holds exactly (PI-01 AC 4), by `(instance, param)`: the table's own value is the
    /// nearest `f64`.
    exact: std::collections::BTreeMap<(String, String), i128>,
}

#[derive(Debug, Clone)]
struct Slot {
    path: String,
    params: Vec<(String, ValueType, f64)>,
    /// The composite params of the instance, each as the bytes its pin holds.
    composites: Vec<CompositeParam>,
}

/// A composite param of an instance: its layout and the bytes its pin holds (its value bytes).
#[derive(Debug, Clone)]
struct CompositeParam {
    /// The param's name.
    name: String,
    /// The layout of its pin.
    layout: CompositeLayout,
    /// The bytes its pin holds: its value.
    bytes: Vec<u8>,
}

/// One write of a leaf of a composite param: the whole value before and after, as the pin holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct LeafChange {
    /// The instance whose param it is.
    pub instance: String,
    /// The composite param.
    pub param: String,
    /// The leaf written, as the layout names it (`num[2]`, `p.x`, `0`).
    pub leaf: String,
    /// The bytes of the param before.
    pub old: Vec<u8>,
    /// The bytes of the param after.
    pub bytes: Vec<u8>,
}

impl LeafChange {
    /// `<instance>.<param>.<leaf>`.
    pub fn label(&self) -> String {
        format!("{}.{}.{}", self.instance, self.param, self.leaf)
    }

    /// The whole param as the circuit takes it.
    pub fn param_value(&self) -> ParamValue {
        ParamValue::Bytes(self.bytes.clone())
    }

    /// The whole param before the write, as the circuit takes it.
    pub fn old_value(&self) -> ParamValue {
        ParamValue::Bytes(self.old.clone())
    }
}

impl Params {
    /// The values the design elaborated to, and its derive plan.
    pub fn of(ir: &Ir) -> Params {
        let instances = ir
            .instances()
            .map(|instance| Slot {
                path: instance.path().to_string(),
                composites: instance
                    .body()
                    .params()
                    .iter()
                    .zip(instance.params())
                    .filter(|(param, _)| ir.is_composite_value(&param.ty))
                    .filter_map(|(param, given)| {
                        let layout = CompositeLayout::of(ir, &param.ty)?;
                        let bytes = CompositeLayout::bytes(ir, &param.ty, *given)?;
                        Some(CompositeParam { name: param.name.clone(), layout, bytes })
                    })
                    .collect(),
                params: instance
                    .body()
                    .params()
                    .iter()
                    .zip(instance.params())
                    .map(|(param, given)| {
                        let value = match given {
                            Constant::Real(r) => r.get(),
                            Constant::Bool(b) => f64::from(u8::from(*b)),
                            Constant::Int(v) => *v as f64,
                            // A unit enum is its tag (PI-02). The scalar table holds no leaf of a composite:
                            // its place is kept so the declaration order the derive functions read stays,
                            // and `get` reads none.
                            Constant::Aggregate(_) => ir.discriminant(&param.ty, *given).map_or(f64::NAN, |tag| tag as f64),
                        };
                        (param.name.clone(), param.ty.clone(), value)
                    })
                    .collect(),
            })
            .collect();
        Params { plan: DerivePlan::of(ir), instances, exact: std::collections::BTreeMap::new() }
    }

    /// The value of `param` of `instance`; `None` when the design has no such param.
    pub fn get(&self, instance: &str, param: &str) -> Option<f64> {
        self.slot(instance)?.params.iter().find(|(n, ty, _)| n == param && !ty.is_composite()).map(|(_, _, v)| *v).filter(|_| self.composite_bytes(instance, param).is_none())
    }

    /// The bytes of the composite param `param` of `instance`: its value.
    pub fn composite_bytes(&self, instance: &str, param: &str) -> Option<&[u8]> {
        self.slot(instance)?.composites.iter().find(|c| c.name == param).map(|c| c.bytes.as_slice())
    }

    /// The leaves of the composite param `param` of `instance`, as the layout names them; `None`
    /// when the instance has no such composite param.
    pub fn leaves(&self, instance: &str, param: &str) -> Option<Vec<String>> {
        let composite = self.slot(instance)?.composites.iter().find(|c| c.name == param)?;
        Some(composite.layout.leaves().iter().map(|l| l.name.clone()).collect())
    }

    /// The write of `value` to `leaf` of the composite param `param` of `instance`, with nothing
    /// applied: the whole param's new bytes. `E0801` for a param or a leaf that does not exist
    /// (the leaves are named), `E0802` for a value the leaf cannot hold (a `bool` is 0 or 1, an
    /// integer a whole number in its range).
    pub fn leaf_change(&self, instance: &str, param: &str, leaf: &str, value: f64) -> Result<LeafChange, Failure> {
        let label = format!("{instance}.{param}.{leaf}");
        let composite = self
            .slot(instance)
            .and_then(|s| s.composites.iter().find(|c| c.name == param))
            .ok_or_else(|| Failure::Host { code: "E0801", message: format!("`{instance}.{param}` is no composite param of the design") })?;
        let found = composite.layout.leaves().iter().find(|l| l.name == leaf).ok_or_else(|| Failure::Host {
            code: "E0801",
            message: format!("`{label}` names no leaf of `{instance}.{param}`: its leaves are {}", composite.layout.leaves().iter().map(|l| format!("`{}`", l.name)).collect::<Vec<_>>().join(", ")),
        })?;
        let refused = |why: String| Failure::Host { code: "E0802", message: format!("`{label}` refuses the value {value}: {why}") };
        let mut bytes = composite.bytes.clone();
        let encoded = LeafValue::encode(found.leaf, value).map_err(refused)?;
        let at = found.offset as usize;
        bytes[at..at + encoded.len()].copy_from_slice(&encoded);
        Ok(LeafChange { instance: instance.to_string(), param: param.to_string(), leaf: leaf.to_string(), old: composite.bytes.clone(), bytes })
    }

    /// Takes `change` into the table.
    pub fn apply_leaf(&mut self, change: &LeafChange) {
        let found = self.instances.iter_mut().find(|s| s.path == change.instance).and_then(|s| s.composites.iter_mut().find(|c| c.name == change.param));
        if let Some(composite) = found {
            composite.bytes = change.bytes.clone();
        }
    }

    /// The derive plan of the design.
    pub fn plan(&self) -> &DerivePlan {
        &self.plan
    }

    /// Sets `param` of `instance` to `value` and every param derived from it, handing each
    /// changed param to `write` in instance order (the one set first). A write that fails is
    /// `E0802` naming the param; the writes already made are undone through `write` with the old
    /// values, and the table is as it was. `derive` is `None` on a backend with no generated
    /// functions: a param others derive from is then refused.
    pub fn set(
        &mut self,
        instance: &str,
        param: &str,
        value: f64,
        derive: Option<&dyn Derive>,
        write: &mut dyn FnMut(&Changed) -> Result<(), String>,
    ) -> Result<Vec<Changed>, Failure> {
        let changes = self.changes(instance, param, value, derive)?;
        for (done, change) in changes.iter().enumerate() {
            if let Err(error) = write(change) {
                for undone in changes[..done].iter().rev() {
                    let back = Changed { value: undone.old, old: undone.value, ..undone.clone() };
                    // the old value was accepted before: an undo that fails leaves the circuit's
                    // own error to be read, and the table is untouched either way
                    let _ = write(&back);
                }
                return Err(Failure::Host { code: "E0802", message: format!("`{}` refuses the value {}: {error}", change.label(), change.value) });
            }
        }
        self.apply(&changes);
        Ok(changes)
    }

    /// The params a `set` of `param` of `instance` to `value` changes, in instance order, with
    /// nothing written and the table untouched: what a timed `set` schedules. The refusals of
    /// `set` apply.
    /// `E0802` for a number a param of an integer or enum type cannot take (PI-01 AC 4, PI-02 AC 4): an integer param takes a whole number
    /// in its range, an enum takes a variant name as text, never a number. A real or a flag param takes any number.
    pub fn refuse_unfit(&self, instance: &str, param: &str, value: f64) -> Result<(), Failure> {
        let label = format!("{instance}.{param}");
        let refuse = |message: String| Failure::Host { code: "E0802", message };
        let Some(ty) = self.slot(instance).and_then(|s| s.params.iter().find(|(n, _, _)| n == param)).map(|(_, t, _)| t.clone()) else { return Ok(()) };
        if let Some((low, high)) = ty.integer_range()
            && (value.fract() != 0.0 || !(low as f64..=high as f64).contains(&value))
        {
            return Err(refuse(format!("`{label}` refuses the value {value}: the param is {ty}, a whole number from {low} to {high}")));
        }
        if matches!(ty, ValueType::Enum(_)) {
            return Err(refuse(format!("`{label}` is an enum: give a variant name as text, `--param {label}=<Variant>`")));
        }
        Ok(())
    }

    pub fn changes(&self, instance: &str, param: &str, value: f64, derive: Option<&dyn Derive>) -> Result<Vec<Changed>, Failure> {
        let refuse = |message: String| Failure::Host { code: "E0802", message };
        let label = format!("{instance}.{param}");
        if let Some(giving) = self.plan.giving(instance, param) {
            return Err(refuse(format!(
                "`{label}` is derived from `{}` of `{}` by its placement: set that param instead",
                giving.reads.join("`, `"),
                giving.parent
            )));
        }
        let old = self.get(instance, param).ok_or_else(|| Failure::Host { code: "E0801", message: format!("`{label}` names no param of the design") })?;
        let readers = self.plan.readers(instance, param);
        let ty = self.slot(instance).and_then(|s| s.params.iter().find(|(n, _, _)| n == param)).map(|(_, t, _)| t.clone()).unwrap_or(ValueType::F64);
        let mut changes = vec![Changed { instance: instance.to_string(), param: param.to_string(), ty, old, value, derived: false }];
        if readers.is_empty() {
            return Ok(changes);
        }
        let Some(derive) = derive else {
            let derived: Vec<String> = readers.iter().filter_map(|k| self.plan.bindings().get(*k as usize)).map(|b| format!("{}.{}", b.child, b.param)).collect();
            return Err(refuse(format!("`{label}` is a param that {} derive from: the interpreter does not recompute them, use --backend rust", derived.join(", "))));
        };
        let mut working = self.clone();
        working.apply(&changes);
        for binding in self.plan.bindings() {
            let read_changed = binding.reads.iter().any(|r| changes.iter().any(|c| c.instance == binding.parent && &c.param == r));
            if !read_changed {
                continue;
            }
            if binding.ty.is_composite() || self.composite_bytes(&binding.child, &binding.param).is_some() {
                return Err(refuse(format!(
                    "`{label}` is read by the placement of the composite param `{}.{}`, which keeps the value it elaborated to: set `{}.{}` itself instead",
                    binding.child, binding.param, binding.child, binding.param
                )));
            }
            let parent = working.slot(&binding.parent).ok_or_else(|| Failure::Backend(format!("the derive plan names `{}`, which is no instance", binding.parent)))?;
            let inputs: Vec<f64> = parent.params.iter().map(|(_, _, v)| *v).collect();
            let derived = derive.derive(binding.k, &inputs);
            if derived.is_nan() {
                return Err(Failure::Backend(format!("the derive function {} of `{}.{}` gave no value (the compiled design does not match its plan)", binding.k, binding.child, binding.param)));
            }
            let change = working.derived(binding, derived)?;
            if change.old != change.value {
                working.apply(std::slice::from_ref(&change));
                changes.push(change);
            }
        }
        Ok(changes)
    }

    /// The change `binding` makes when its expression gives `value`.
    fn derived(&self, binding: &Binding, value: f64) -> Result<Changed, Failure> {
        let old = self.get(&binding.child, &binding.param).ok_or_else(|| Failure::Backend(format!("the derive plan names `{}.{}`, which is no param", binding.child, binding.param)))?;
        Ok(Changed { instance: binding.child.clone(), param: binding.param.clone(), ty: binding.ty.clone(), old, value, derived: true })
    }

    /// Takes `changes` into the table.
    pub fn apply(&mut self, changes: &[Changed]) {
        for change in changes {
            self.exact.remove(&(change.instance.clone(), change.param.clone()));
            let found = self.instances.iter_mut().find(|s| s.path == change.instance).and_then(|s| s.params.iter_mut().find(|(n, _, _)| *n == change.param));
            if let Some((_, _, v)) = found {
                *v = change.value;
            }
        }
    }

    /// Remembers the exact integer `value` of the param, which no `f64` holds.
    pub fn keep_exact(&mut self, instance: &str, param: &str, value: i128) {
        self.exact.insert((instance.to_string(), param.to_string()), value);
    }

    /// The exact integer a host gave the param, when no `f64` holds it.
    pub fn exact_integer(&self, instance: &str, param: &str) -> Option<i128> {
        self.exact.get(&(instance.to_string(), param.to_string())).copied()
    }

    fn slot(&self, instance: &str) -> Option<&Slot> {
        self.instances.iter().find(|s| s.path == instance)
    }
}

#[cfg(test)]
mod tests {
    use piperine_ir::{BinaryOp, BodyId, BodySpec, Child, ChildValue, Expr, InstanceSpec, IrBuilder, Origin, Param, Real};

    use super::*;

    fn origin() -> Origin {
        Origin { file: "params.rs".into(), line: 1, col: 1 }
    }

    fn param(name: &str) -> Param {
        Param { name: name.into(), ty: ValueType::F64, default: Some(Constant::Real(Real::new(1.0))), check: None, structural: false, origin: origin() }
    }

    fn body(b: &mut IrBuilder, name: &str, params: Vec<Param>, children: Vec<Child>) -> BodyId {
        b.body(BodySpec { name: name.into(), origin: origin(), params, nodes: vec![], branches: vec![], limits: vec![], stims: vec![], locals: vec![], analog: None, children })
    }

    fn placed(name: &str, body: BodyId) -> Child {
        Child { name: name.into(), body, nodes: vec![], params: vec![], origin: origin() }
    }

    fn scaled(b: &mut IrBuilder, owner: BodyId, scale: f64) {
        let k = b.expr(Expr::Real(Real::new(scale)), ValueType::F64, origin());
        let p = b.expr(Expr::Param(0), ValueType::F64, origin());
        let product = b.expr(Expr::Binary { op: BinaryOp::Mul, lhs: p, rhs: k }, ValueType::F64, origin());
        b.child_param(owner, 0, 0, ChildValue::Expr(product));
    }

    /// `Top { t }` places `Mid { rr: t * 3.0 }`, which places `Leaf { r: rr * 2.0 }`; the
    /// elaborated values are those of `t = 1`.
    fn chain() -> Ir {
        let mut b = IrBuilder::new();
        let leaf = body(&mut b, "Leaf", vec![param("r")], vec![]);
        let mid = body(&mut b, "Mid", vec![param("rr")], vec![placed("l", leaf)]);
        let top = body(&mut b, "Top", vec![param("t")], vec![placed("m", mid)]);
        scaled(&mut b, mid, 2.0);
        scaled(&mut b, top, 3.0);
        let instance = |b: &mut IrBuilder, path: &str, name: &str, body, parent, v: f64| {
            b.instance(InstanceSpec { path: path.into(), name: name.into(), body, parent, nodes: vec![], params: vec![Constant::Real(Real::new(v))], origin: origin() })
        };
        let t = instance(&mut b, "Top", "Top", top, None, 1.0);
        let m = instance(&mut b, "Top.m", "m", mid, Some(t), 3.0);
        instance(&mut b, "Top.m.l", "l", leaf, Some(m), 6.0);
        b.finish(t)
    }

    /// The generated functions the fixture's expressions print as.
    struct Scales;

    impl Derive for Scales {
        fn derive(&self, k: u32, params: &[f64]) -> f64 {
            match k {
                0 => params[0] * 3.0,
                1 => params[0] * 2.0,
                _ => f64::NAN,
            }
        }
    }

    fn accept(written: &mut Vec<String>) -> impl FnMut(&Changed) -> Result<(), String> + '_ {
        move |change| {
            written.push(format!("{}={}", change.label(), change.value));
            Ok(())
        }
    }

    #[test]
    fn a_set_derives_its_children() {
        // HII-18 AC 1: `t = 5` gives `Top.m.rr = 15` and `Top.m.l.r = 30` (a chain derives top
        // down), written in instance order, the set param first
        let mut params = Params::of(&chain());
        let mut written = Vec::new();
        let changes = params.set("Top", "t", 5.0, Some(&Scales), &mut accept(&mut written)).expect("set");
        assert_eq!(written, ["Top.t=5", "Top.m.rr=15", "Top.m.l.r=30"]);
        assert_eq!(changes.iter().map(|c| c.derived).collect::<Vec<_>>(), [false, true, true]);
        assert_eq!(params.get("Top.m", "rr"), Some(15.0));
        assert_eq!(params.get("Top.m.l", "r"), Some(30.0));
    }

    #[test]
    fn a_refused_child_restores_every_param() {
        // HII-18 AC 4: the leaf refuses its recomputed value: the writes already made are undone
        // with their old values, and the table keeps what it had
        let mut params = Params::of(&chain());
        let mut written = Vec::new();
        let mut refuse_leaf = |change: &Changed| {
            written.push(format!("{}={}", change.label(), change.value));
            if change.instance == "Top.m.l" && change.value == 30.0 { Err("check 0 is false".to_string()) } else { Ok(()) }
        };
        let failure = params.set("Top", "t", 5.0, Some(&Scales), &mut refuse_leaf).expect_err("refused");
        match failure {
            Failure::Host { code, message } => assert!(code == "E0802" && message.contains("Top.m.l.r"), "{code}: {message}"),
            other => panic!("a host failure, found {other}"),
        }
        assert_eq!(written, ["Top.t=5", "Top.m.rr=15", "Top.m.l.r=30", "Top.m.rr=3", "Top.t=1"]);
        assert_eq!((params.get("Top", "t"), params.get("Top.m", "rr"), params.get("Top.m.l", "r")), (Some(1.0), Some(3.0), Some(6.0)));
    }

    #[test]
    fn a_derived_param_cannot_be_set_directly() {
        // HII-20 AC 1: `Top.m.rr` derives from `t` of `Top`; the refusal names it
        let mut params = Params::of(&chain());
        let mut written = Vec::new();
        let failure = params.set("Top.m", "rr", 7.0, Some(&Scales), &mut accept(&mut written)).expect_err("refused");
        match failure {
            Failure::Host { code, message } => assert!(code == "E0802" && message.contains("`t`") && message.contains("`Top`"), "{code}: {message}"),
            other => panic!("a host failure, found {other}"),
        }
        assert!(written.is_empty());
    }

    #[test]
    fn the_interpreter_refuses_a_set_others_derive_from() {
        // HII-20 AC 2: with no generated functions a set of `t` is refused naming the derived
        // params and `--backend rust`
        let mut params = Params::of(&chain());
        let mut written = Vec::new();
        let failure = params.set("Top", "t", 5.0, None, &mut accept(&mut written)).expect_err("refused");
        match failure {
            Failure::Host { code, message } => assert!(code == "E0802" && message.contains("Top.m.rr") && message.contains("--backend rust"), "{code}: {message}"),
            other => panic!("a host failure, found {other}"),
        }
        assert!(written.is_empty());
    }

    #[test]
    fn no_set_gives_the_folded_constants() {
        // HII-20 AC 3: the table is what the instance table holds, so a run with no `set` gives
        // the values it always did
        let ir = chain();
        let params = Params::of(&ir);
        for instance in ir.instances() {
            for (param, given) in instance.body().params().iter().zip(instance.params()) {
                let Constant::Real(r) = given else { panic!("a real") };
                assert_eq!(params.get(instance.path(), &param.name), Some(r.get()), "{}.{}", instance.path(), param.name);
            }
        }
    }
}
