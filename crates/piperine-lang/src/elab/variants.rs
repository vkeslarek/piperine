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
use crate::msl::Span;
use crate::pom::{DeclId, FileId};

/// The variants requested so far.
#[derive(Default)]
pub(super) struct Variants {
    ids: BTreeMap<(DeclId, Vec<Flag>), FunctionId>,
    /// By id: the result type once the variant is lowered; `None` while its body is being
    /// lowered (a call met then is a recursion) or after it failed.
    results: Vec<Option<ValueType>>,
    /// By id: the lowered function, added to the IR at the end.
    specs: Vec<Option<FunctionSpec>>,
    /// By id: whether the variant's body reads a continuous value.
    continuous: Vec<bool>,
    /// The variants whose bodies are being lowered.
    open: BTreeSet<FunctionId>,
}

impl Variants {
    /// The name of the variant of `name` for `pattern`: the source name when every argument is
    /// known, else `name~bits`, a `1` for each argument that may be unknown.
    pub(super) fn name(name: &str, pattern: &[Flag]) -> String {
        if pattern.iter().all(|f| *f == Flag::Plain) {
            return name.to_string();
        }
        let bits: String = pattern.iter().map(|f| if *f == Flag::Flagged { '1' } else { '0' }).collect();
        format!("{name}~{bits}")
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
    pub(super) fn request(&mut self, function: DeclId, pattern: Vec<Flag>, file: FileId, span: Span) -> Option<(FunctionId, ValueType)> {
        if let Some(id) = self.variants.ids.get(&(function, pattern.clone())).copied() {
            return match self.variants.results[id.index()].clone() {
                Some(ret) => Some((id, ret)),
                None if self.variants.open.contains(&id) => {
                    self.unsupported(file, span, "a function that calls itself");
                    None
                }
                None => None,
            };
        }
        let id = FunctionId(self.variants.results.len() as u32);
        self.variants.ids.insert((function, pattern.clone()), id);
        self.variants.results.push(None);
        self.variants.specs.push(None);
        self.variants.continuous.push(false);
        self.variants.open.insert(id);
        let spec = self.lower_function(function, &pattern);
        self.variants.open.remove(&id);
        let spec = spec?;
        let ret = spec.ret.clone();
        self.variants.continuous[id.index()] = self.continuous(spec.body);
        self.variants.results[id.index()] = Some(ret.clone());
        self.variants.specs[id.index()] = Some(spec);
        Some((id, ret))
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
