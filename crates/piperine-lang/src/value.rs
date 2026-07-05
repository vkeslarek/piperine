//! [`Value`] — **the** value type of the frontend (SIMPLIFICATION.md P2).
//!
//! One enum serves every phase: elaboration-time constant folding
//! (`ConstEnv`), POM storage (param defaults, instance overrides, global
//! consts, staged overrides), the `eval` interpreter, and `bench` results.
//! The former `ConstVal` and `pom::Value` were narrower copies of the same
//! scalars; they are gone — a site that must reject non-constants narrows
//! at the point of use instead of routing through a second type.
//!
//! Beyond the scalars this carries the value-layer collections (tuple,
//! list, option), closures, bundle-literal instances (`Record`), and host
//! objects (results, net/instance handles) behind the [`Object`] trait so
//! solver types never leak into this crate.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::parse::ast::Expr;

use crate::eval::error::EvalError;

/// A runtime value.
#[derive(Debug, Clone)]
pub enum Value {
    Unit,
    Int(i64),
    Nat(u64),
    Real(f64),
    Bool(bool),
    Str(String),
    /// Complex scalar `(re, im)` — POM attribute surface; no literal syntax yet.
    Complex(f64, f64),
    /// 8-level logic value (backed by a `u8`) — POM attribute surface.
    Quad(u8),
    EnumVariant(String, String),
    Tuple(Vec<Value>),
    /// A `Vec`-like value-layer list. Shared/mutable so `.push(...)` is visible
    /// through every alias of the same list (SPEC §6.1).
    List(Rc<RefCell<Vec<Value>>>),
    /// A bundle-literal instance (config bundles included). Field lookup
    /// falls back to the bundle's declared `FieldDecl.default` at
    /// construction time — there is no lazy default resolution here.
    Record {
        /// The bundle type this literal instantiates (`"DiodeModel"`) —
        /// what `impl` method dispatch resolves against.
        ty: String,
        fields: Rc<RefCell<HashMap<String, Value>>>,
    },
    /// A `Map<K, V>` association list (piperine-bench/docs/SPEC.md §5.1 — `ic`/`nodeset`
    /// per-node hints). Backed by a `Vec<(Value, Value)>`, not a `HashMap`:
    /// `Value` keys aren't `Hash`/`Eq`-clean, and N is tiny. Shared/mutable
    /// so `.insert(...)` is visible through every alias, like `List`.
    Map(Rc<RefCell<Vec<(Value, Value)>>>),
    Option(Option<Box<Value>>),
    Closure(Rc<Closure>),
    /// A host-defined object (e.g. `OpResult`, `NetRef`). Method calls on it
    /// are dispatched through [`Object::call_method`].
    Object(Rc<dyn Object>),
}

/// A closure value: captured lexical scope plus a lambda body.
#[derive(Debug)]
pub struct Closure {
    pub params: Vec<String>,
    pub body: Expr,
    pub captured: Vec<HashMap<String, Value>>,
}

/// A host-defined object reachable from `bench`/const-eval code.
///
/// Lets host crates (e.g. `piperine-bench`) hand PHDL-callable handles
/// (`OpResult`, `NetRef`, `InstanceRef`, ...) into the interpreter without
/// this crate knowing their concrete types.
pub trait Object: fmt::Debug {
    /// The type name as it should appear in diagnostics (e.g. `"OpResult"`).
    fn type_name(&self) -> &str;
    /// Downcast support, so a host can recover its concrete type from a
    /// `Value::Object` passed back to it as an argument (e.g. `.v(a, b)`
    /// receiving a `NetRef` produced by name resolution).
    fn as_any(&self) -> &dyn Any;
    /// Dispatch a method call `recv.name(args)`.
    fn call_method(&self, name: &str, args: Vec<Value>) -> Result<Value, EvalError>;

    /// Human-readable rendering for `$display` — result objects override
    /// this to print a table (a `Waveform` prints its samples, an
    /// `OpResult` its node voltages). The default is the diagnostic
    /// placeholder `<TypeName>`.
    fn render(&self) -> String {
        format!("<{}>", self.type_name())
    }

    /// Value-based equality for [`Value::PartialEq`] — returns true when two
    /// objects' data compares equal (e.g. two `NetRef`s with the same name).
    /// The default is identity (distinct objects compare unequal), which is
    /// the safe fallback; concrete object types that have meaningful value
    /// identity override this (piperine-bench/docs/SPEC.md §5.1 — `Map<Net, Real>` keys
    /// must compare by net name, not object pointer).
    fn equals(&self, _other: &dyn Any) -> bool {
        false
    }

    /// Dispatch a method call that receives a [`Value::Closure`] argument.
    /// Host objects can't invoke closures themselves (only the interpreter
    /// can), so when the interpreter sees a closure argument to a method on
    /// an `Object`, it routes here and hands over an `invoke` callback that
    /// re-enters the interpreter to call the closure. The default forwards
    /// to [`call_method`](Self::call_method), so objects that don't take
    /// callbacks are unaffected — the closure-taking methods (`Waveform::map`,
    /// ...) override this.
    fn call_method_with(
        &self,
        name: &str,
        args: Vec<Value>,
        _invoke: &mut dyn FnMut(&Closure, Vec<Value>) -> Result<Value, EvalError>,
    ) -> Result<Value, EvalError> {
        self.call_method(name, args)
    }
}

impl Value {
    /// The type name as it should appear in diagnostics.
    pub fn type_name(&self) -> &str {
        match self {
            Self::Unit => "Unit",
            Self::Int(_) => "Integer",
            Self::Nat(_) => "Natural",
            Self::Real(_) => "Real",
            Self::Bool(_) => "Boolean",
            Self::Str(_) => "String",
            Self::Complex(..) => "Complex",
            Self::Quad(_) => "Quad",
            Self::EnumVariant(..) => "EnumVariant",
            Self::Tuple(_) => "Tuple",
            Self::List(_) => "List",
            Self::Record { .. } => "Record",
            Self::Map(_) => "Map",
            Self::Option(_) => "Option",
            Self::Closure(_) => "Closure",
            Self::Object(o) => o.type_name(),
        }
    }

    /// True if this value is "truthy" for `if`/structural conditions:
    /// `Bool` directly, `Nat`/`Int` nonzero (mirrors `ConstEnv`'s legacy
    /// integer-as-condition allowance).
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Nat(n) => *n != 0,
            Self::Int(n) => *n != 0,
            _ => false,
        }
    }

    /// Built-in methods shared by every value (list/option/tuple ops). Not
    /// dispatched for `Object` — those go through [`Object::call_method`].
    pub fn call_builtin_method(&self, name: &str, args: Vec<Value>) -> Result<Value, EvalError> {
        match (self, name) {
            (Value::List(items), "push") => {
                let [v] = take1(args)?;
                items.borrow_mut().push(v);
                Ok(Value::Unit)
            }
            (Value::List(items), "len") => Ok(Value::Nat(items.borrow().len() as u64)),
            (Value::List(items), "get") => {
                let [i] = take1(args)?;
                let idx = as_index(&i)?;
                Ok(Value::Option(items.borrow().get(idx).cloned().map(Box::new)))
            }
            // `is_present`/`get_or` are the optional-param sugars (SPEC §…);
            // `is_some`/`is_none`/`unwrap`/`unwrap_or` are the value-layer aliases.
            (Value::Option(inner), "is_some" | "is_present") => Ok(Value::Bool(inner.is_some())),
            (Value::Option(inner), "is_none") => Ok(Value::Bool(inner.is_none())),
            (Value::Option(inner), "unwrap") => inner
                .clone()
                .map(|v| *v)
                .ok_or_else(|| EvalError::Host("unwrap of an empty Option".into())),
            (Value::Option(inner), "unwrap_or" | "get_or") => {
                let [default] = take1(args)?;
                Ok(inner.clone().map(|v| *v).unwrap_or(default))
            }
            (Value::Map(entries), "insert") => {
                let mut it = args.into_iter();
                let k = it.next().ok_or_else(|| EvalError::TypeMismatch("insert needs 2 arguments".into()))?;
                let v = it.next().ok_or_else(|| EvalError::TypeMismatch("insert needs 2 arguments".into()))?;
                let mut e = entries.borrow_mut();
                if let Some(slot) = e.iter_mut().find(|(ek, _)| ek == &k) {
                    slot.1 = v;
                } else {
                    e.push((k, v));
                }
                Ok(Value::Unit)
            }
            (Value::Map(entries), "get") => {
                let [k] = take1(args)?;
                let found = entries.borrow().iter().find(|(ek, _)| ek == &k).map(|(_, v)| v.clone());
                Ok(Value::Option(found.map(Box::new)))
            }
            (Value::Map(entries), "len") => Ok(Value::Nat(entries.borrow().len() as u64)),
            (Value::Object(obj), _) => obj.call_method(name, args),
            (recv, other) => {
                Err(EvalError::Undefined(format!("method `{other}` on {}", recv.type_name())))
            }
        }
    }
}

fn take1(mut args: Vec<Value>) -> Result<[Value; 1], EvalError> {
    if args.len() != 1 {
        return Err(EvalError::TypeMismatch(format!("expected 1 argument, got {}", args.len())));
    }
    Ok([args.remove(0)])
}

fn as_index(v: &Value) -> Result<usize, EvalError> {
    match v {
        Value::Nat(n) => Ok(*n as usize),
        Value::Int(n) if *n >= 0 => Ok(*n as usize),
        other => Err(EvalError::TypeMismatch(format!("expected an index, got {}", other.type_name()))),
    }
}

impl Value {
    /// True for the compile-time-constant scalar subset (what the former
    /// `ConstVal` could hold). Const-eval call sites that must reject
    /// collections/closures narrow with this.
    pub fn is_const_scalar(&self) -> bool {
        match self {
            Value::Int(_)
            | Value::Nat(_)
            | Value::Real(_)
            | Value::Bool(_)
            | Value::Str(_)
            | Value::Complex(..)
            | Value::Quad(_)
            | Value::EnumVariant(..) => true,
            // An optional is a compile-time constant iff absent (`none`) or
            // wrapping a constant scalar — this lets `param x : Real? = none`
            // and `param x : Real? = 1.2` be elaboration constants.
            Value::Option(None) => true,
            Value::Option(Some(inner)) => inner.is_const_scalar(),
            _ => false,
        }
    }

    /// Extract the inner `f64` if this is a `Real`.
    pub fn as_real(&self) -> Option<f64> {
        match self { Self::Real(v) => Some(*v), _ => None }
    }
    /// Extract the inner `u64` if this is a `Natural`.
    pub fn as_natural(&self) -> Option<u64> {
        match self { Self::Nat(v) => Some(*v), _ => None }
    }
    /// Extract the inner `i64` if this is an `Integer`.
    pub fn as_integer(&self) -> Option<i64> {
        match self { Self::Int(v) => Some(*v), _ => None }
    }
    /// Extract the inner `bool` if this is a `Boolean`.
    pub fn as_boolean(&self) -> Option<bool> {
        match self { Self::Bool(v) => Some(*v), _ => None }
    }
    /// Extract the inner `&str` if this is a `String`.
    pub fn as_string(&self) -> Option<&str> {
        match self { Self::Str(v) => Some(v), _ => None }
    }
    /// Extract the inner `u8` if this is a `Quad`.
    pub fn as_quad(&self) -> Option<u8> {
        match self { Self::Quad(v) => Some(*v), _ => None }
    }
    /// Extract the `(re, im)` pair if this is a `Complex`.
    pub fn as_complex(&self) -> Option<(f64, f64)> {
        match self { Self::Complex(re, im) => Some((*re, *im)), _ => None }
    }
}

/// Structural equality on data; `Closure`/`Object` never compare equal
/// (they have no meaningful value identity — documented, not derived).
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Unit, Value::Unit) => true,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Nat(a), Value::Nat(b)) => a == b,
            (Value::Real(a), Value::Real(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Complex(ar, ai), Value::Complex(br, bi)) => ar == br && ai == bi,
            (Value::Quad(a), Value::Quad(b)) => a == b,
            (Value::EnumVariant(ae, av), Value::EnumVariant(be, bv)) => ae == be && av == bv,
            (Value::Tuple(a), Value::Tuple(b)) => a == b,
            (Value::List(a), Value::List(b)) => *a.borrow() == *b.borrow(),
            (Value::Record { fields: a, .. }, Value::Record { fields: b, .. }) => {
                *a.borrow() == *b.borrow()
            }
            (Value::Map(a), Value::Map(b)) => *a.borrow() == *b.borrow(),
            (Value::Option(a), Value::Option(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => a.equals(b.as_any()),
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unit => write!(f, "()"),
            Self::Real(v) => write!(f, "{v}"),
            Self::Nat(v) => write!(f, "{v}"),
            Self::Int(v) => write!(f, "{v}"),
            Self::Bool(v) => write!(f, "{v}"),
            Self::Quad(v) => write!(f, "0q{v}"),
            Self::Str(v) => write!(f, "\"{v}\""),
            Self::Complex(re, im) => write!(f, "{re}+{im}j"),
            Self::EnumVariant(e, v) => write!(f, "{e}::{v}"),
            other => write!(f, "<{}>", other.type_name()),
        }
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self { Self::Real(v) }
}
impl From<u64> for Value {
    fn from(v: u64) -> Self { Self::Nat(v) }
}
impl From<i64> for Value {
    fn from(v: i64) -> Self { Self::Int(v) }
}
impl From<bool> for Value {
    fn from(v: bool) -> Self { Self::Bool(v) }
}
impl From<String> for Value {
    fn from(v: String) -> Self { Self::Str(v) }
}
impl From<&str> for Value {
    fn from(v: &str) -> Self { Self::Str(v.into()) }
}
impl From<num_complex::Complex64> for Value {
    fn from(c: num_complex::Complex64) -> Self {
        Self::Complex(c.re, c.im)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_real_construction_and_access() {
        let v = Value::Real(3.14);
        assert_eq!(v.as_real(), Some(3.14));
        assert_eq!(v.as_integer(), None);
    }

    #[test]
    fn value_natural_construction_and_access() {
        let v = Value::Nat(42);
        assert_eq!(v.as_natural(), Some(42));
    }

    #[test]
    fn value_boolean_construction_and_access() {
        let v = Value::Bool(true);
        assert_eq!(v.as_boolean(), Some(true));
    }

    #[test]
    fn value_string_construction_and_access() {
        let v = Value::Str("hello".into());
        assert_eq!(v.as_string(), Some("hello"));
    }

    #[test]
    fn value_integer_construction_and_access() {
        let v = Value::Int(-7);
        assert_eq!(v.as_integer(), Some(-7));
    }

    #[test]
    fn value_type_name() {
        assert_eq!(Value::Real(0.0).type_name(), "Real");
        assert_eq!(Value::Nat(0).type_name(), "Natural");
        assert_eq!(Value::Bool(false).type_name(), "Boolean");
        assert_eq!(Value::Str("".into()).type_name(), "String");
    }
}
