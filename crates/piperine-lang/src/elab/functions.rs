//! Functions: each `fn` the top reaches is one IR function, lowered once however many bodies
//! call it. Its parameters are its first locals, then every `let` and closure parameter of its
//! body in declaration order.

use piperine_ir::{FunctionSpec, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{DeclId, DeclKind, FileId, TypeId, TypeKind};

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
        let id = self.ir.function(FunctionSpec { name, origin: self.locator.decl(function), params, ret, locals: scope.locals, body: lowered });
        debug_assert_eq!(Some(id), self.function_ids.get(&function).copied(), "functions are built in first-reach order");
    }

    /// The IR type a type written in a signature names.
    pub(super) fn lower_type(&mut self, file: FileId, ty: TypeId) -> Option<ValueType> {
        let node = self.design.type_node(ty).clone();
        let lowered = match &node.kind {
            TypeKind::Tuple(items) if items.is_empty() => Some(ValueType::Unit),
            TypeKind::FnPtr { params, ret } => {
                let params: Option<Vec<ValueType>> = params.iter().map(|p| self.lower_type(file, *p)).collect();
                let ret = match ret {
                    Some(r) => self.lower_type(file, *r),
                    None => Some(ValueType::Unit),
                };
                Some(ValueType::Fn(params?, Box::new(ret?)))
            }
            _ => match self.design.type_resolution(ty) {
                crate::pom::Resolution::Builtin(id) if id.info().name == "fN" && id.width() == Some(64) => Some(ValueType::F64),
                crate::pom::Resolution::Builtin(id) if id.info().name == "bool" => Some(ValueType::Bool),
                _ => None,
            },
        };
        if lowered.is_none() {
            self.unsupported(file, node.span, "a type other than `f64`, `bool`, `()` and a function type");
        }
        lowered
    }
}
