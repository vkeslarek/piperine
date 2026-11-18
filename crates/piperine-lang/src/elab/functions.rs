//! Functions: each `fn` a body calls is one IR function per flag pattern of its arguments
//! (`variants.rs`), lowered when a call first reaches it. Its parameters are its first locals,
//! then every `let` and closure parameter of its body in declaration order.

use piperine_ir::{FunctionSpec, Local, ValueType};

use super::elaborator::Elaboration;
use super::known::Flag;
use super::lower::Scope;
use super::variants::Variants;
use crate::pom::{DeclId, DeclKind, FileId, TypeId};
use crate::types::Ty;

impl Elaboration<'_, '_> {
    /// The variant of `function` for `pattern`: a parameter is flagged where the pattern is, and
    /// the result is the type of the body. `None` after a diagnostic.
    pub(super) fn lower_function(&mut self, function: DeclId, pattern: &[Flag]) -> Option<FunctionSpec> {
        let data = self.design.decl_data(function).clone();
        let DeclKind::Fn(f) = &data.kind else { return None };
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let mut scope = Scope::new(data.file);
        // `freq()` in a function is the caller's `freq_`: the sweep's in an analog caller, 0 in a reaction.
        scope.analog = true;
        let mut params = Vec::new();
        let mut ok = true;
        let declared = data.children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::FnParam(_)));
        let mut flags = pattern.iter().copied().chain(std::iter::repeat(Flag::Plain));
        for param in declared {
            // An `Option<T>` parameter is two: `name.is` and `name.val` (§3.2), read as the pair
            // of an `Option` local.
            if let Some(Ty::Option(payload)) = self.types.local(param).cloned() {
                let Some(payload) = self.value_type(&payload) else {
                    self.unsupported(data.file, self.design.decl_data(param).span, "an `Option` parameter whose payload has no value in the IR");
                    ok = false;
                    continue;
                };
                let name = self.design.decl_data(param).name.as_ref().map_or_else(String::new, |n| n.text.clone());
                let is = scope.locals.len() as u32;
                for (suffix, ty) in [("is", ValueType::Bool), ("val", payload)] {
                    let local = Local { name: format!("{name}.{suffix}"), ty: flags.next().unwrap_or(Flag::Plain).wrap(ty) };
                    scope.locals.push(local.clone());
                    params.push(local);
                }
                scope.options.insert(param, (is, is + 1));
                continue;
            }
            match self.bind_local(&mut scope, param) {
                Some(index) => {
                    let local = &mut scope.locals[index as usize];
                    local.ty = flags.next().unwrap_or(Flag::Plain).wrap(local.ty.clone());
                    params.push(local.clone());
                }
                None => ok = false,
            }
        }
        // The declared result is checked for a type the IR has; the variant's result is its body's.
        let declared_ret = match f.ret {
            Some(ty) => self.lower_type(data.file, ty).is_some(),
            None => true,
        };
        let Some(body) = &f.body else {
            self.unsupported(data.file, data.span, "a function without a body");
            return None;
        };
        let lowered = self.lower_block(&mut scope, data.file, &body.stmts, body.span)?;
        if !(ok && declared_ret) {
            return None;
        }
        let ret = self.type_of(lowered);
        Some(FunctionSpec { name: Variants::name(&name, pattern), origin: self.locator.decl(function), params, ret, locals: scope.locals, body: lowered })
    }

    /// The IR type a type written in a signature names: the checker's type of it, as an IR type.
    pub(super) fn lower_type(&mut self, file: FileId, ty: TypeId) -> Option<ValueType> {
        let lowered = self.types.written(ty).and_then(|t| self.value_type(t));
        if lowered.is_none() {
            let span = self.design.type_node(ty).span;
            let what = match self.types.written(ty) {
                Some(crate::types::Ty::Enum(_)) => "an enum of one variant, whose tag has no level,",
                _ => "a type that has no value in the IR",
            };
            self.unsupported(file, span, what);
        }
        lowered
    }
}
