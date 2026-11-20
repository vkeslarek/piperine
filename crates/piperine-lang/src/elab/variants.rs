//! Variants: one IR function per `fn` and flag pattern of its arguments.
//!
//! An IR function has fixed parameter types, and a value that may be unknown has the type
//! `Flagged` (`known.rs`). So a function is lowered once for each pattern of known and
//! possibly-unknown arguments a call reaches: `dithered_width(duty.known_or(0), frame + 1)`
//! reaches the pattern `01` and lowers `dithered_width~01`, whose `frame` is `flagged<u3>`; that
//! body's `rev3(frame)` reaches `rev3~1`. The pattern of all known arguments keeps the source
//! name, so a function no unknown value reaches prints as it always did. A variant's result is
//! the type its body has: flagged when an unknown value can reach it.
//!
//! A variant is lowered when a call first reaches it, so the caller learns its result type, and
//! its id is handed out in that first-reach order; the functions are added to the IR in id order
//! once every body is lowered. A function only a const context reaches is folded there and gets
//! no IR function.

use std::collections::{BTreeMap, BTreeSet};

use piperine_ir::{FunctionId, FunctionSpec, ValueType};

use super::elaborator::Elaboration;
use super::known::Flag;
use crate::pom::{DeclId, DeclKind};
use crate::types::Ty;

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
    ids: BTreeMap<(DeclId, Vec<Flag>, Half), FunctionId>,
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
    /// The name of the variant of `name` for `pattern`: the source name when every argument is
    /// known, else `name~bits`, a `1` for each argument that may be unknown.
    pub(super) fn name(name: &str, pattern: &[Flag], half: Half) -> String {
        let suffix = half.suffix();
        if pattern.iter().all(|f| *f == Flag::Plain) {
            return format!("{name}{suffix}");
        }
        let bits: String = pattern.iter().map(|f| if *f == Flag::Flagged { '1' } else { '0' }).collect();
        format!("{name}~{bits}{suffix}")
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
    /// The variant of `function` for `pattern` and its result type, lowered the first time it is
    /// reached. `None` after a diagnostic: the variant failed to lower, or the call at `span`
    /// reaches a variant whose body is being lowered (the IR has no recursion).
    pub(super) fn request(&mut self, function: DeclId, pattern: Vec<Flag>, half: Half) -> Option<(FunctionId, ValueType)> {
        if let Some(id) = self.variants.ids.get(&(function, pattern.clone(), half)).copied() {
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
        let id = FunctionId(self.variants.results.len() as u32);
        self.variants.ids.insert((function, pattern.clone(), half), id);
        self.variants.results.push(None);
        self.variants.specs.push(None);
        self.variants.continuous.push(false);
        self.variants.assumed.push(self.declared_result(function, half));
        self.variants.open.insert(id);
        let spec = self.lower_variant(id, function, &pattern, half);
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
            DeclKind::Fn(f) => f.ret.and_then(|t| self.types.written(t)),
            _ => None,
        };
        match (declared, half) {
            (Some(Ty::Option(_)), Half::Presence) => ValueType::Bool,
            (Some(Ty::Option(payload)), _) => self.value_type(payload).unwrap_or(ValueType::Unit),
            (Some(ty), _) => self.value_type(ty).unwrap_or(ValueType::Unit),
            (None, _) => ValueType::Unit,
        }
    }

    /// The body of the variant `id`, lowered again with the result it came out with while a
    /// recursive call took another (a result that may be unknown): the fixed point of the body.
    /// Each lowering leaves the expressions of the one before unused.
    fn lower_variant(&mut self, id: FunctionId, function: DeclId, pattern: &[Flag], half: Half) -> Option<FunctionSpec> {
        loop {
            let spec = self.lower_function(function, pattern, half)?;
            if !self.variants.recursive.contains(&id) || spec.ret == self.variants.assumed[id.index()] {
                return Some(spec);
            }
            self.variants.assumed[id.index()] = spec.ret.clone();
        }
    }

    /// Add the lowered variants to the IR, in the order of their ids. After a diagnostic no IR
    /// is built, and a variant that failed after reaching one that lowered leaves a gap in the
    /// ids: nothing is added.
    pub(super) fn add_functions(&mut self) {
        if !self.diagnostics.is_empty() {
            return;
        }
        for (k, spec) in self.variants.take().into_iter().enumerate() {
            let Some(spec) = spec else { continue };
            let id = self.ir.function(spec);
            debug_assert_eq!(id, FunctionId(k as u32), "a failed variant leaves diagnostics, so no Ir is built");
        }
    }
}
