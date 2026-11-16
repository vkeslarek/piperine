//! Functions: each `fn` the top reaches is one IR function, lowered once however many bodies
//! call it. Its parameters are its first locals, then every `let` and closure parameter of its
//! body in declaration order.

use piperine_ir::{FunctionSpec, ValueType};

use super::elaborator::Elaboration;
use super::known::Flag;
use super::lower::{Scope, ValueTypes};
use crate::pom::{DeclId, DeclKind, FileId, TypeId};

impl Elaboration<'_, '_> {
    /// Lower `functions`, in first-reach order: the ids a call names were handed out by position.
    pub(super) fn lower_functions(&mut self, functions: &[DeclId]) {
        for function in functions {
            self.lower_function(*function);
        }
    }

    fn lower_function(&mut self, function: DeclId) {
        let data = self.design.decl_data(function).clone();
        let DeclKind::Fn(f) = &data.kind else { return };
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let mut scope = Scope::new(data.file);
        let mut params = Vec::new();
        for param in data.children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::FnParam(_))) {
            if let Some(index) = self.bind_local(&mut scope, param) {
                params.push(scope.locals[index as usize].clone());
            }
        }
        let ret = match f.ret {
            Some(ty) => self.lower_type(data.file, ty),
            None => Some(ValueType::Unit),
        };
        let Some(body) = &f.body else {
            self.unsupported(data.file, data.span, "a function without a body");
            return;
        };
        let lowered = self.lower_block(&mut scope, data.file, &body.stmts, body.span);
        let (Some(ret), Some(lowered)) = (ret, lowered) else { return };
        if self.flag(lowered) == Flag::Flagged {
            // A result that may be unknown has its own variant from msl-digital T25 on.
            self.unsupported(data.file, data.span, "a function whose result may be unknown");
            return;
        }
        let id = self.ir.function(FunctionSpec { name, origin: self.locator.decl(function), params, ret, locals: scope.locals, body: lowered });
        debug_assert_eq!(Some(id), self.function_ids.get(&function).copied(), "functions are built in first-reach order");
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
