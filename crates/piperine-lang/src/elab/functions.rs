//! Functions: each `fn` a body calls is one IR function (`variants.rs`), lowered when a call first reaches it. Its parameters are its first locals,
//! then every `let` and closure parameter of its body in declaration order.

use piperine_ir::{FunctionSpec, Local, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use super::instances::{ConcreteValue, Instance};
use super::variants::{Half, Variants};
use crate::pom::{DeclId, DeclKind, FileId, TypeId};
use crate::types::Ty;

impl Elaboration<'_, '_> {
    /// The variant of `function` for `half`: the result is the type of the body. `None` after a
    /// diagnostic.
    pub(super) fn lower_function(&mut self, function: DeclId, arguments: &[ConcreteValue], half: Half) -> Option<FunctionSpec> {
        let data = self.design.decl_data(function).clone();
        let DeclKind::Fn(f) = &data.kind else { return None };
        let written = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        // an instance of a generic function is named from the function and its arguments (D8)
        let name = if arguments.is_empty() { written } else { Instance { item: function, arguments: arguments.to_vec() }.name(self) };
        let mut scope = Scope::new(data.file);
        // `freq()` in a function is the caller's `freq_`: the sweep's in an analog caller, 0 in a reaction.
        scope.analog = true;
        let mut params = Vec::new();
        let mut ok = true;
        let declared = data.children.iter().copied().filter(|c| matches!(self.design.decl_data(*c).kind, DeclKind::FnParam(_)));
        for param in declared {
            // An `Option<T>` parameter is two: `name.is` and `name.val` (§3.2), read as the pair
            // of an `Option` local.
            if let Some(Ty::Option(payload)) = self.typed_local(param) {
                let Some(payload) = self.value_type(&payload) else {
                    self.unsupported(data.file, self.design.decl_data(param).span, "an `Option` parameter whose payload has no value in the IR");
                    ok = false;
                    continue;
                };
                let name = self.design.decl_data(param).name.as_ref().map_or_else(String::new, |n| n.text.clone());
                let is = scope.locals.len() as u32;
                for (suffix, ty) in [("is", ValueType::Bool), ("val", payload)] {
                    let local = Local { name: format!("{name}.{suffix}"), ty, mutable: false };
                    scope.locals.push(local.clone());
                    params.push(local);
                }
                scope.options.insert(param, (is, is + 1));
                continue;
            }
            match self.bind_local(&mut scope, param) {
                Some(index) => {
                    params.push(scope.locals[index as usize].clone());
                }
                None => ok = false,
            }
        }
        // The declared result is checked for a type the IR has; the variant's result is its body's.
        let declared_ret = match f.ret {
            // an `Option` result is two IR functions (`returns.rs`), each with the type of its half
            Some(ty) if matches!(self.typed_written(ty), Some(Ty::Option(_))) => true,
            Some(ty) => self.lower_type(data.file, ty).is_some(),
            None => true,
        };
        let Some(body) = &f.body else {
            self.unsupported(data.file, data.span, "a function without a body");
            return None;
        };
        let lowered = self.lower_function_body(&mut scope, function, half, data.file, body)?;
        if !(ok && declared_ret) {
            return None;
        }
        let ret = self.type_of(lowered);
        // Each half runs the whole body and uses only part of what it computes: its own locals are
        // spelled `_name`, so the generated Rust does not warn of the part a half leaves unread.
        if half != Half::Whole {
            for local in scope.locals.iter_mut().skip(params.len()) {
                local.name = format!("_{}", local.name);
            }
        }
        Some(FunctionSpec { name: Variants::name(&name, half), origin: self.locator.decl(function), params, ret, locals: scope.locals, body: lowered })
    }

    /// The IR type a type written in a signature names: the checker's type of it, as an IR type.
    pub(super) fn lower_type(&mut self, file: FileId, ty: TypeId) -> Option<ValueType> {
        let lowered = self.typed_written(ty).and_then(|t| self.value_type(&t));
        if lowered.is_none() {
            let span = self.design.type_node(ty).span;
            let what = match self.typed_written(ty) {
                Some(crate::types::Ty::Enum(_)) => "an enum of one variant, whose tag has no level,",
                _ => "a type that has no value in the IR",
            };
            self.unsupported(file, span, what);
        }
        lowered
    }
}
