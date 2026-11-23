//! Variants: one IR function per `fn` and half of its result.
//!
//! A function is lowered once for each list of generic arguments (a value has no flag, §3.3): the
//! whole result, or, for a function returning an `Option`, one IR function for each half of the
//! pair. A function as written keeps its source name; an instance of a generic function is named
//! `item::<arguments>` and records the instance it is (§3.8.10).
//!
//! A variant is lowered when a call first reaches it, so the caller learns its result type, and
//! its id is handed out in that first-reach order; the functions are added to the IR in id order
//! once every body is lowered. A function only a const context reaches is folded there and gets
//! no IR function.

use std::collections::{BTreeMap, BTreeSet};

use piperine_ir::{FunctionId, FunctionSpec, GenericInstance, ValueType};

use super::elaborator::Elaboration;
use super::instances::{ConcreteValue, Instance};
use crate::msl::Diagnostic;
use crate::pom::{DeclId, DeclKind};
use crate::types::Ty;

/// The most instantiations of generic functions that may be open at once: a chain deeper than this never ends (D7).
const MOST_NESTED_INSTANTIATIONS: usize = 1024;

/// Which value of the function's result a variant computes. An `Option` result is no IR value, so
/// the function is two IR functions, one for each half of the pair `(is, val)` (MD-67 item 2): both
/// run the whole body, each returns its half.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Half {
    /// The result itself: any function that does not return an `Option`.
    Whole,
    /// Whether the `Option` is `Some`.
    Presence,
    /// The payload of the `Option`, the zero of its type for `None`.
    Payload,
}

impl Half {
    /// What the half adds to the function's name.
    fn suffix(self) -> &'static str {
        match self {
            Half::Whole => "",
            Half::Presence => ".is",
            Half::Payload => ".val",
        }
    }
}

/// The variants requested so far.
#[derive(Default)]
pub(super) struct Variants {
    ids: BTreeMap<(DeclId, Vec<ConcreteValue>, Half), FunctionId>,
    /// By id: the generic instance the variant is, `None` for a function as written.
    generic: Vec<Option<GenericInstance>>,
    /// By id: the result type once the variant is lowered; `None` while its body is being
    /// lowered (a call met then is a recursion) or after it failed.
    results: Vec<Option<ValueType>>,
    /// By id: the lowered function, added to the IR at the end.
    specs: Vec<Option<FunctionSpec>>,
    /// By id: whether the variant's body reads a continuous value.
    continuous: Vec<bool>,
    /// The variants whose bodies are being lowered.
    open: BTreeSet<FunctionId>,
    /// By id: the result type a call inside the variant's own body (a recursion) is given while
    /// its body is being lowered: the declared result, then the fixed point of the body's.
    assumed: Vec<ValueType>,
    /// The variants a recursive call was met in.
    recursive: BTreeSet<FunctionId>,
}

impl Variants {
    /// The name of the variant of `name` for `half`: the source name, with the half's suffix.
    pub(super) fn name(name: &str, half: Half) -> String {
        format!("{name}{}", half.suffix())
    }

    /// Whether the variant `id` reads a continuous value; `false` while it is being lowered.
    pub(super) fn continuous(&self, id: FunctionId) -> bool {
        self.continuous.get(id.index()).copied().unwrap_or(false)
    }

    /// The lowered functions in id order; `None` for one that failed (its diagnostics are out).
    pub(super) fn take(&mut self) -> Vec<Option<FunctionSpec>> {
        std::mem::take(&mut self.specs)
    }
}

impl Elaboration<'_, '_> {
    /// The variant of `function` for `half` and its result type, lowered the first time it is
    /// reached. `None` after a diagnostic: the variant failed to lower, or the call at `span`
    /// reaches a variant whose body is being lowered (the IR has no recursion).
    pub(super) fn request(&mut self, function: DeclId, half: Half) -> Option<(FunctionId, ValueType)> {
        self.request_instance(function, Vec::new(), half)
    }

    /// [`Elaboration::request`] for the instance of a generic function with `arguments`: one IR function
    /// for each distinct list (§3.8.10), named from the function and its arguments, with the substitution
    /// of the arguments set while its body is lowered. A chain of instantiations deeper than 1 024 is
    /// `E0341` (D7).
    pub(super) fn request_instance(&mut self, function: DeclId, arguments: Vec<ConcreteValue>, half: Half) -> Option<(FunctionId, ValueType)> {
        let key = (function, arguments.clone(), half);
        if let Some(id) = self.variants.ids.get(&key).copied() {
            return match self.variants.results[id.index()].clone() {
                Some(ret) => Some((id, ret)),
                // a recursion: the variant is registered, its result is assumed until its body says
                None if self.variants.open.contains(&id) => {
                    self.variants.recursive.insert(id);
                    Some((id, self.variants.assumed[id.index()].clone()))
                }
                None => None,
            };
        }
        if self.variants.open.len() > MOST_NESTED_INSTANTIATIONS {
            let data = self.design.decl_data(function);
            let (file, span, name) = (data.file, data.span, data.name.as_ref().map_or_else(String::new, |n| n.text.clone()));
            self.report(file, Diagnostic::new("E0341", format!("instantiating `{name}` never ends: the chain of instantiations is deeper than {MOST_NESTED_INSTANTIATIONS} (D7)"), span, "an instantiation chain that does not end"));
            return None;
        }
        let id = FunctionId(self.variants.results.len() as u32);
        let generic = (!arguments.is_empty()).then(|| {
            let instance = Instance { item: function, arguments: arguments.clone() };
            GenericInstance { item: self.design.decl_data(function).name.as_ref().map_or_else(String::new, |n| n.text.clone()), arguments: instance.argument_names(self) }
        });
        self.variants.generic.push(generic);
        self.variants.ids.insert(key, id);
        self.variants.results.push(None);
        self.variants.specs.push(None);
        self.variants.continuous.push(false);
        self.variants.assumed.push(self.declared_result(function, half));
        self.variants.open.insert(id);
        let spec = crate::stack::wide(|| self.lower_variant(id, function, &arguments, half));
        self.variants.open.remove(&id);
        let spec = spec?;
        let ret = spec.ret.clone();
        self.variants.continuous[id.index()] = self.continuous(spec.body);
        self.variants.results[id.index()] = Some(ret.clone());
        self.variants.specs[id.index()] = Some(spec);
        Some((id, ret))
    }

    /// The result of `function` as its signature declares it (the half's type for an `Option`):
    /// what a recursive call is first taken to give.
    fn declared_result(&self, function: DeclId, half: Half) -> ValueType {
        let declared = match &self.design.decl_data(function).kind {
            DeclKind::Fn(f) => f.ret.and_then(|t| self.typed_written(t)),
            _ => None,
        };
        match (declared, half) {
            (Some(Ty::Option(_)), Half::Presence) => ValueType::Bool,
            (Some(Ty::Option(payload)), _) => self.value_type(&payload).unwrap_or(ValueType::Unit),
            (Some(ty), _) => self.value_type(&ty).unwrap_or(ValueType::Unit),
            (None, _) => ValueType::Unit,
        }
    }

    /// The body of the variant `id`, lowered again with the result it came out with while a
    /// recursive call took another: the fixed point of the body.
    /// Each lowering leaves the expressions of the one before unused.
    fn lower_variant(&mut self, id: FunctionId, function: DeclId, arguments: &[ConcreteValue], half: Half) -> Option<FunctionSpec> {
        // the body of an instance reads the type table through its own arguments; the caller's come back after
        let outer = self.consts.set_substitution(self.instance_substitution(function, arguments));
        let lowered = loop {
            let Some(spec) = self.lower_function(function, arguments, half) else { break None };
            if !self.variants.recursive.contains(&id) || spec.ret == self.variants.assumed[id.index()] {
                break Some(spec);
            }
            self.variants.assumed[id.index()] = spec.ret.clone();
        };
        self.consts.set_substitution(outer);
        lowered
    }

    /// Add the lowered variants to the IR, in the order of their ids. After a diagnostic no IR
    /// is built, and a variant that failed after reaching one that lowered leaves a gap in the
    /// ids: nothing is added.
    pub(super) fn add_functions(&mut self) {
        if !self.diagnostics.is_empty() {
            return;
        }
        let generic = std::mem::take(&mut self.variants.generic);
        for (k, spec) in self.variants.take().into_iter().enumerate() {
            let Some(spec) = spec else { continue };
            let id = self.ir.function(spec);
            debug_assert_eq!(id, FunctionId(k as u32), "a failed variant leaves diagnostics, so no Ir is built");
            if let Some(Some(generic)) = generic.get(k) {
                self.ir.mark_instance(id, generic.clone());
            }
        }
    }
}
