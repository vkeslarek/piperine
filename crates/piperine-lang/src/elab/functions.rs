//! Functions: each `fn` a body calls is one IR function per flag pattern of its arguments
//! (`variants.rs`), lowered when a call first reaches it. Its parameters are its first locals,
//! then every `let` and closure parameter of its body in declaration order.

use piperine_ir::{FunctionSpec, ValueType};

use super::elaborator::Elaboration;
use super::known::Flag;
use super::lower::{Scope, ValueTypes};
use super::variants::Variants;
use crate::pom::{DeclId, DeclKind, FileId, TypeId};

impl Elaboration<'_, '_> {
    /// The variant of `function` for `pattern`: a parameter is flagged where the pattern is, and
    /// the result is the type of the body. `None` after a diagnostic.
    pub(super) fn lower_function(&mut self, function: DeclId, pattern: &[Flag]) -> Option<FunctionSpec> {
        let data = self.design.decl_data(function).clone();
        let DeclKind::Fn(f) = &data.kind else { return None };
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let mut scope = Scope::new(data.file);
        let mut params = Vec::new();
        let mut ok = true;
        let declared = data.children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::FnParam(_)));
        for (param, flag) in declared.zip(pattern.iter().copied().chain(std::iter::repeat(Flag::Plain))) {
            match self.bind_local(&mut scope, param) {
                Some(index) => {
                    let local = &mut scope.locals[index as usize];
                    local.ty = flag.wrap(local.ty.clone());
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
        let lowered = self.types.written(ty).and_then(ValueTypes::of);
        if lowered.is_none() {
            let span = self.design.type_node(ty).span;
            self.unsupported(file, span, "a type that has no value in the IR");
        }
        lowered
    }
}
