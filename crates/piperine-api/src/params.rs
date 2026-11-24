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
use piperine_plan::{Binding, CompositeLayout, DerivePlan, PinLayout};
use piperine_solver::abi::Value;
use piperine_solver::prelude::ParamValue;
use piperine_transpile::Compiled;

use crate::composite::CellShape;
use crate::failure::Failure;
use crate::leaf_input::{LeafEncoding, LeafInput};

/// What recomputes a derived param (§8.1.19, §8.1.23): the compiled device's generated function `k`, over the pin bytes of the
/// parent's params one after another in declaration order, each as wide as its pin, giving the `width` pin bytes of the child's param. A param
/// crosses as the typed value it is, never as an `f64`.
pub trait Derive {
    /// The pin bytes of derived param `k`, or why there are none.
    fn derive(&self, k: u32, params: &[u8], width: usize) -> Result<Vec<u8>, String>;
}

impl Derive for Compiled {
    fn derive(&self, k: u32, params: &[u8], width: usize) -> Result<Vec<u8>, String> {
        Compiled::derive(self, k, params, width).ok_or_else(|| format!("the compiled design has no derive function {k} for {} bytes of params and a {width}-byte param: it does not match its plan", params.len()))
    }
}

/// A `Derive` that evaluates each placement expression with the reference interpreter's walk, so
/// a design that no backend has built (`piperine info`) still shows the values a `set` gives its
/// derived params: no `rustc`, the same expressions the generated functions compile. The walk
/// holds `f64` and `bool` params only, as the interpreter does (PI-05 AC 1: another type is the
/// pin of a compiled body); a binding over any other type is refused naming it and `--backend rust`.
pub struct Evaluated<'ir> {
    /// The design.
    ir: &'ir Ir,
    /// The plan's bindings, by `k`.
    bindings: Vec<Binding>,
}

impl<'ir> Evaluated<'ir> {
    /// The evaluator of `plan`'s bindings over `ir`.
    pub fn new(ir: &'ir Ir, plan: &DerivePlan) -> Evaluated<'ir> {
        Evaluated { ir, bindings: plan.bindings().to_vec() }
    }
}

impl Derive for Evaluated<'_> {
    fn derive(&self, k: u32, params: &[u8], _width: usize) -> Result<Vec<u8>, String> {
        let binding = self.bindings.get(k as usize).ok_or_else(|| format!("the plan has no binding {k}"))?;
        let label = format!("{}.{}", binding.child, binding.param);
        let refused = |what: &str| format!("`{label}` is derived by an expression over {what}, which the interpreter does not evaluate (it runs `f64` and `bool` params only): use --backend rust");
        let parent = self.ir.instance_at(&binding.parent).ok_or_else(|| format!("the plan names `{}`, which is no instance", binding.parent))?;
        let mut values = Vec::new();
        let mut at = 0usize;
        for declared in parent.body().params() {
            let width = PinLayout::in_ir(self.ir, &declared.ty).map(|layout| layout.size() as usize).ok_or_else(|| refused(&format!("the param `{}` of type {}", declared.name, declared.ty)))?;
            let bytes = params.get(at..at + width).ok_or_else(|| format!("the parent `{}` holds {} bytes of params, fewer than its params need", binding.parent, params.len()))?;
            values.push(match declared.ty {
                ValueType::F64 => Walked::Real(Dual::constant(f64::from_le_bytes(bytes.try_into().map_err(|_| refused("a malformed real"))?))),
                ValueType::Bool => Walked::Bool(bytes.first().is_some_and(|byte| *byte != 0)),
                // a param the expression does not read is left unread; one it reads is refused
                _ if binding.reads.contains(&declared.name) => return Err(refused(&format!("the param `{}` of type {}", declared.name, declared.ty))),
                _ => Walked::Unread,
            });
            at += width;
        }
        match (Walk::new(self.ir, values, 0).eval(binding.expr), &binding.ty) {
            (Ok(Walked::Real(real)), ValueType::F64) => Ok(real.v.to_le_bytes().to_vec()),
            (Ok(Walked::Bool(flag)), ValueType::Bool) => Ok(vec![u8::from(flag)]),
            (Ok(_), ty) => Err(refused(&format!("an expression giving a value of type {ty}"))),
            (Err(error), _) => Err(format!("`{label}` could not be evaluated by the interpreter: {error}")),
        }
    }
}

/// The pin bytes of a scalar param and the value they hold: a real, a flag, an integer exactly, an enum's tag.
struct PinBytes;

impl PinBytes {
    /// The pin bytes of a param of type `ty` holding `value` (`exact` when no `f64` holds the integer), `width` bytes wide.
    fn encode(ty: &ValueType, value: f64, exact: Option<i128>, width: usize) -> Vec<u8> {
        match ty {
            ValueType::F64 => value.to_le_bytes().to_vec(),
            ValueType::Bool => vec![u8::from(value != 0.0)],
            _ => exact.unwrap_or(value as i128).to_le_bytes()[..width.min(16)].to_vec(),
        }
    }

    /// The value pin bytes hold as the table keeps it, and the integer when no `f64` holds it.
    fn decode(ty: &ValueType, bytes: &[u8]) -> (f64, Option<i128>) {
        match ty {
            ValueType::F64 => (bytes.try_into().map_or(f64::NAN, f64::from_le_bytes), None),
            ValueType::Bool => (f64::from(u8::from(bytes.first().is_some_and(|byte| *byte != 0))), None),
            ValueType::Int(_) => {
                let mut wide = [if bytes.last().is_some_and(|byte| byte & 0x80 != 0) { 0xff } else { 0 }; 16];
                wide[..bytes.len().min(16)].copy_from_slice(&bytes[..bytes.len().min(16)]);
                let value = i128::from_le_bytes(wide);
                (value as f64, (value.unsigned_abs() > 1 << 53).then_some(value))
            }
            _ => {
                let mut wide = [0u8; 16];
                wide[..bytes.len().min(16)].copy_from_slice(&bytes[..bytes.len().min(16)]);
                let value = i128::from_le_bytes(wide);
                (value as f64, (value.unsigned_abs() > 1 << 53).then_some(value))
            }
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
    /// The bytes a composite param holds after the change, when it is one (§8.1.23: a param bound from another follows it); `value` is
    /// then unused.
    pub bytes: Option<Vec<u8>>,
    /// The bytes it held before.
    pub old_bytes: Option<Vec<u8>>,
    /// The value as the host wrote it, when `value` cannot show it (an exact integer, a leaf of a composite): what a refusal names.
    pub text: Option<String>,
}

impl Changed {
    /// The value as a refusal names it: the host's own text, else the number.
    pub fn shown(&self) -> String {
        self.text.clone().unwrap_or_else(|| self.value.to_string())
    }

    /// `<instance>.<param>`.
    pub fn label(&self) -> String {
        format!("{}.{}", self.instance, self.param)
    }

    /// The value as the circuit takes it: what the host gave stays a real (as every `set` of a
    /// param has been), a recomputed `bool` is a bool.
    pub fn param_value(&self) -> ParamValue {
        match &self.bytes {
            Some(bytes) => ParamValue::Bytes(bytes.clone()),
            None => self.encoded(self.value),
        }
    }

    /// The value before the `set`, as the circuit takes it.
    pub fn old_value(&self) -> ParamValue {
        match &self.old_bytes {
            Some(bytes) => ParamValue::Bytes(bytes.clone()),
            None => self.encoded(self.old),
        }
    }

    /// The value as a scheduled `set` takes it.
    pub fn scheduled(&self) -> Value {
        if let Some(bytes) = &self.bytes {
            return Value::Bytes(bytes.clone());
        }
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
    /// The instance's path.
    path: String,
    /// Each param: its name, its type and its value as an `f64` (a unit enum's tag, a flag as 0 or 1).
    params: Vec<(String, ValueType, f64)>,
    /// The width of each param's pin in bytes, in the order of `params`.
    widths: Vec<usize>,
    /// The composite params of the instance, each as the bytes its pin holds.
    composites: Vec<CompositeParam>,
    /// The params of a number width (`f32`, `f16`, `f128`, an integer wider than 64 bits), each as the bytes its pin holds: they
    /// have no `f64` view, so the exact cell reads these.
    exacts: Vec<ExactParam>,
}

/// A param of a number width: the bytes its pin holds.
#[derive(Debug, Clone)]
struct ExactParam {
    /// The param's name.
    name: String,
    /// The bytes its pin holds: its value.
    bytes: Vec<u8>,
}

/// A composite param of an instance: its layout and the bytes its pin holds (its value bytes).
#[derive(Debug, Clone)]
struct CompositeParam {
    /// The param's name.
    name: String,
    /// The layout of its pin.
    layout: CompositeLayout,
    /// Its shape, which names the type of each scalar leaf (`None` for a type the host's decoder has no shape of).
    shape: Option<CellShape>,
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
                        Some(CompositeParam { name: param.name.clone(), layout, shape: CellShape::of(ir, &param.ty), bytes })
                    })
                    .collect(),
                exacts: instance
                    .body()
                    .params()
                    .iter()
                    .zip(instance.params())
                    .filter(|(param, _)| crate::number_cell::NumberCell::is_number_width(&param.ty))
                    .filter_map(|(param, given)| Some(ExactParam { name: param.name.clone(), bytes: PinLayout::bytes_in(ir, &param.ty, *given)? }))
                    .collect(),
                widths: instance.body().params().iter().map(|param| PinLayout::in_ir(ir, &param.ty).map_or(0, |layout| layout.size() as usize)).collect(),
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
                            // A float of another width and a wide integer have no `f64` view either: the exact cell reads their bytes.
                            Constant::FloatBits(_) | Constant::Wide(_) => f64::NAN,
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
        self.slot(instance)?.params.iter().find(|(n, ty, _)| n == param && !ty.is_composite()).map(|(_, _, v)| *v).filter(|_| self.held_bytes(instance, param).is_none())
    }

    /// The bytes of the composite param `param` of `instance`: its value.
    pub fn composite_bytes(&self, instance: &str, param: &str) -> Option<&[u8]> {
        self.slot(instance)?.composites.iter().find(|c| c.name == param).map(|c| c.bytes.as_slice())
    }

    /// The bytes of the param `param` of `instance` when its type is a number width: its exact value.
    pub fn exact_bytes(&self, instance: &str, param: &str) -> Option<&[u8]> {
        self.slot(instance)?.exacts.iter().find(|e| e.name == param).map(|e| e.bytes.as_slice())
    }

    /// The bytes a param holds when no `f64` shows them: a composite's or a number width's.
    fn held_bytes(&self, instance: &str, param: &str) -> Option<&[u8]> {
        self.composite_bytes(instance, param).or_else(|| self.exact_bytes(instance, param))
    }

    /// The leaves of the composite param `param` of `instance`, as the layout names them; `None`
    /// when the instance has no such composite param.
    pub fn leaves(&self, instance: &str, param: &str) -> Option<Vec<String>> {
        let composite = self.slot(instance)?.composites.iter().find(|c| c.name == param)?;
        Some(composite.layout.leaves().iter().map(|l| l.name.clone()).collect())
    }

    /// The write of `input` to `leaf` of the composite param `param` of `instance`, with nothing applied: the whole param's new
    /// bytes. `E0801` for a param or a leaf that does not exist (the leaves are named), `E0802` for a value the leaf cannot hold
    /// (a `bool` is 0 or 1, `true` or `false`; an integer a whole number in the range of the leaf's own type, written exactly at any
    /// width).
    pub fn leaf_change(&self, instance: &str, param: &str, leaf: &str, input: &LeafInput) -> Result<LeafChange, Failure> {
        let label = format!("{instance}.{param}.{leaf}");
        let composite = self
            .slot(instance)
            .and_then(|s| s.composites.iter().find(|c| c.name == param))
            .ok_or_else(|| Failure::Host { code: "E0801", message: format!("`{instance}.{param}` is no composite param of the design") })?;
        let found = composite.layout.leaves().iter().find(|l| l.name == leaf).ok_or_else(|| Failure::Host {
            code: "E0801",
            message: format!("`{label}` names no leaf of `{instance}.{param}`: its leaves are {}", composite.layout.leaves().iter().map(|l| format!("`{}`", l.name)).collect::<Vec<_>>().join(", ")),
        })?;
        let encoding = LeafEncoding { leaf: found, ty: composite.shape.as_ref().and_then(|shape| shape.leaf_type(leaf)) };
        let encoded = encoding.bytes(input).map_err(|why| Failure::Host { code: "E0802", message: format!("`{label}` refuses the value {}: {why}", input.shown()) })?;
        let mut bytes = composite.bytes.clone();
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
    pub fn set(&mut self, instance: &str, param: &str, value: f64, derive: Option<&dyn Derive>, write: &mut dyn FnMut(&Changed) -> Result<(), String>) -> Result<Vec<Changed>, Failure> {
        let changes = self.changes(instance, param, value, derive)?;
        self.write_changes(&changes, write)?;
        Ok(changes)
    }

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

    /// The params a `set` of `param` of `instance` to `value` changes, in instance order, with
    /// nothing written and the table untouched: what a timed `set` schedules. The refusals of
    /// `set` apply.
    pub fn changes(&self, instance: &str, param: &str, value: f64, derive: Option<&dyn Derive>) -> Result<Vec<Changed>, Failure> {
        self.refuse_derived(instance, param)?;
        let old = self.get(instance, param).ok_or_else(|| Failure::Host { code: "E0801", message: format!("`{instance}.{param}` names no param of the design") })?;
        let ty = self.type_of(instance, param);
        self.changes_from(Changed { instance: instance.to_string(), param: param.to_string(), ty, old, value, derived: false, bytes: None, old_bytes: None, text: None }, derive)
    }

    /// [`Params::changes`] for an integer, held exactly (PI-01 AC 4): the pin gets its own bytes and what a placement derives from it is
    /// recomputed from them, not from an `f64`.
    pub fn changes_integer(&self, instance: &str, param: &str, value: i128, derive: Option<&dyn Derive>) -> Result<Vec<Changed>, Failure> {
        self.refuse_derived(instance, param)?;
        let old = self.get(instance, param).ok_or_else(|| Failure::Host { code: "E0801", message: format!("`{instance}.{param}` names no param of the design") })?;
        let ty = self.type_of(instance, param);
        let width = self.width_of(instance, param);
        let first = Changed { instance: instance.to_string(), param: param.to_string(), ty, old, value: value as f64, derived: false, bytes: Some(value.to_le_bytes()[..width].to_vec()), old_bytes: None, text: Some(value.to_string()) };
        self.changes_from(first, derive)
    }

    /// The changes of writing `bytes` to the composite param `param` of `instance`, and of every value bound from it (§8.1.23).
    /// Bytes of another length than the param's are `E0802`: they are no value of its type.
    pub fn changes_composite(&self, instance: &str, param: &str, bytes: Vec<u8>, shown: &str, derive: Option<&dyn Derive>) -> Result<Vec<Changed>, Failure> {
        self.refuse_derived(instance, param)?;
        let old = self.composite_bytes(instance, param).map(<[u8]>::to_vec);
        if let Some(held) = old.as_ref().filter(|held| held.len() != bytes.len()) {
            return Err(Failure::Host { code: "E0802", message: format!("`{instance}.{param}` refuses the value {shown}: it is {} bytes, and the param holds {}", bytes.len(), held.len()) });
        }
        let ty = self.type_of(instance, param);
        self.changes_from(Changed { instance: instance.to_string(), param: param.to_string(), ty, old: f64::NAN, value: f64::NAN, derived: false, bytes: Some(bytes), old_bytes: old, text: Some(shown.to_string()) }, derive)
    }

    /// The changes writing the exact `bytes` of a param of a number width (`shown` is the text the host gave).
    pub fn changes_exact(&self, instance: &str, param: &str, bytes: Vec<u8>, shown: &str, derive: Option<&dyn Derive>) -> Result<Vec<Changed>, Failure> {
        self.refuse_derived(instance, param)?;
        let old = self.exact_bytes(instance, param).map(<[u8]>::to_vec);
        let ty = self.type_of(instance, param);
        self.changes_from(Changed { instance: instance.to_string(), param: param.to_string(), ty, old: f64::NAN, value: f64::NAN, derived: false, bytes: Some(bytes), old_bytes: old, text: Some(shown.to_string()) }, derive)
    }

    /// `E0802` for a param a placement gives: a host sets the param it derives from instead.
    fn refuse_derived(&self, instance: &str, param: &str) -> Result<(), Failure> {
        match self.plan.giving(instance, param) {
            Some(giving) => Err(Failure::Host {
                code: "E0802",
                message: format!("`{instance}.{param}` is derived from `{}` of `{}` by its placement: set that param instead", giving.reads.join("`, `"), giving.parent),
            }),
            None => Ok(()),
        }
    }

    /// `first`, a param the host sets, and every param recomputed from it down the plan, a parent before its children.
    fn changes_from(&self, first: Changed, derive: Option<&dyn Derive>) -> Result<Vec<Changed>, Failure> {
        let readers = self.plan.readers(&first.instance, &first.param);
        let mut changes = vec![first];
        if readers.is_empty() {
            return Ok(changes);
        }
        let label = changes[0].label();
        let Some(derive) = derive else {
            let derived: Vec<String> = readers.iter().filter_map(|k| self.plan.bindings().get(*k as usize)).map(|b| format!("{}.{}", b.child, b.param)).collect();
            return Err(Failure::Host { code: "E0802", message: format!("`{label}` is a param that {} derive from: the interpreter does not recompute them, use --backend rust", derived.join(", ")) });
        };
        let mut working = self.clone();
        working.apply(&changes);
        for binding in self.plan.bindings() {
            let read_changed = binding.reads.iter().any(|r| changes.iter().any(|c| c.instance == binding.parent && &c.param == r));
            if !read_changed {
                continue;
            }
            let inputs = working.parent_bytes(&binding.parent)?;
            let derived = derive.derive(binding.k, &inputs, working.width_of(&binding.child, &binding.param)).map_err(|reason| Failure::Host { code: "E0802", message: format!("`{}.{}` cannot be recomputed after `{}`: {reason}", binding.child, binding.param, changes[0].label()) })?;
            let change = working.derived(binding, derived);
            if change.bytes != change.old_bytes {
                working.apply(std::slice::from_ref(&change));
                changes.push(change);
            }
        }
        Ok(changes)
    }

    /// The type of `param` of `instance`.
    fn type_of(&self, instance: &str, param: &str) -> ValueType {
        self.slot(instance).and_then(|s| s.params.iter().find(|(n, _, _)| n == param)).map(|(_, t, _)| t.clone()).unwrap_or(ValueType::F64)
    }

    /// The width in bytes of the pin of `param` of `instance`.
    fn width_of(&self, instance: &str, param: &str) -> usize {
        self.slot(instance).and_then(|s| s.params.iter().position(|(n, _, _)| n == param).and_then(|i| s.widths.get(i).copied())).unwrap_or(8)
    }

    /// The pin bytes of every param of `instance`, one after another in declaration order: what a derive function reads.
    fn parent_bytes(&self, instance: &str) -> Result<Vec<u8>, Failure> {
        let slot = self.slot(instance).ok_or_else(|| Failure::Backend(format!("the derive plan names `{instance}`, which is no instance")))?;
        let mut bytes = Vec::new();
        for ((name, ty, value), width) in slot.params.iter().zip(&slot.widths) {
            match self.held_bytes(instance, name) {
                Some(held) => bytes.extend(held.iter().copied().chain(std::iter::repeat(0)).take(*width)),
                None => bytes.extend(PinBytes::encode(ty, *value, self.exact_integer(instance, name), *width)),
            }
        }
        Ok(bytes)
    }

    /// The change `binding` makes when its expression gives the pin bytes `bytes`.
    fn derived(&self, binding: &Binding, bytes: Vec<u8>) -> Changed {
        let old_bytes = match self.held_bytes(&binding.child, &binding.param) {
            Some(held) => Some(held.to_vec()),
            None => self.slot(&binding.child).and_then(|s| s.params.iter().zip(&s.widths).find(|((n, _, _), _)| *n == binding.param)).map(|((n, ty, value), width)| PinBytes::encode(ty, *value, self.exact_integer(&binding.child, n), *width)),
        };
        let value = PinBytes::decode(&binding.ty, &bytes).0;
        let old = self.get(&binding.child, &binding.param).unwrap_or(f64::NAN);
        Changed { instance: binding.child.clone(), param: binding.param.clone(), ty: binding.ty.clone(), old, value, derived: true, bytes: Some(bytes), old_bytes, text: None }
    }

    /// Writes `changes` through `write`, undoing the writes already made when one is refused, then takes them into the table.
    pub fn write_changes(&mut self, changes: &[Changed], write: &mut dyn FnMut(&Changed) -> Result<(), String>) -> Result<(), Failure> {
        for (done, change) in changes.iter().enumerate() {
            if let Err(error) = write(change) {
                for undone in changes[..done].iter().rev() {
                    let back = Changed { value: undone.old, old: undone.value, bytes: undone.old_bytes.clone(), old_bytes: undone.bytes.clone(), ..undone.clone() };
                    let _ = write(&back);
                }
                return Err(Failure::Host { code: "E0802", message: format!("`{}` refuses the value {}: {error}", change.label(), change.shown()) });
            }
        }
        self.apply(changes);
        Ok(())
    }

    /// Takes `changes` into the table: a composite param its bytes, any other its value (and the exact integer when no `f64` holds it).
    pub fn apply(&mut self, changes: &[Changed]) {
        for change in changes {
            self.exact.remove(&(change.instance.clone(), change.param.clone()));
            let Some(slot) = self.instances.iter_mut().find(|s| s.path == change.instance) else { continue };
            if let Some(bytes) = &change.bytes
                && let Some(composite) = slot.composites.iter_mut().find(|c| c.name == change.param)
            {
                composite.bytes = bytes.clone();
                continue;
            }
            if let Some(bytes) = &change.bytes
                && let Some(exact) = slot.exacts.iter_mut().find(|e| e.name == change.param)
            {
                exact.bytes = bytes.clone();
                continue;
            }
            let held = match &change.bytes {
                Some(bytes) => PinBytes::decode(&change.ty, bytes),
                None => (change.value, None),
            };
            if let Some((_, _, value)) = slot.params.iter_mut().find(|(n, _, _)| *n == change.param) {
                *value = held.0;
            }
            if let Some(exact) = held.1 {
                self.exact.insert((change.instance.clone(), change.param.clone()), exact);
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
        fn derive(&self, k: u32, params: &[u8], _width: usize) -> Result<Vec<u8>, String> {
            let first = f64::from_le_bytes(params.get(..8).and_then(|head| head.try_into().ok()).ok_or("no params")?);
            match k {
                0 => Ok((first * 3.0).to_le_bytes().to_vec()),
                1 => Ok((first * 2.0).to_le_bytes().to_vec()),
                _ => Err("no such k".to_string()),
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
