//! Functions: each `fn` a body calls is one IR function (`variants.rs`), lowered when a call first reaches it. Its parameters are its first locals,
//! then every `let` and closure parameter of its body in declaration order.

use piperine_ir::{FunctionSpec, Local, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use super::instances::{ConcreteValue, Instance};
use super::variants::{Half, Variants};
use crate::refusal::forms::ValueForm;
use crate::pom::{DeclId, DeclKind, FileId, TypeId};
use crate::types::Ty;

impl Elaboration<'_, '_> {
    /// The variant of `function` for `half`: the result is the type of the body. `None` after a
    /// diagnostic.
    pub(super) fn lower_function(&mut self, function: DeclId, arguments: &[ConcreteValue], half: Half) -> Option<FunctionSpec> {
        let data = self.state.design.decl_data(function).clone();
        let DeclKind::Fn(f) = &data.kind else { return None };
        let written = self.impl_qualified_name(function);
        // an instance of a generic function is named from the function and its arguments (D8)
        let name = if arguments.is_empty() { written } else { Instance { item: function, arguments: arguments.to_vec() }.name(self) };
        self.register_function_types(function);
        let mut scope = Scope::new(data.file);
        // `freq()` in a function is the caller's `freq_`: the sweep's in an analog caller, 0 in a reaction.
        scope.analog.in_analog_code = true;
        let mut params = Vec::new();
        let mut ok = true;
        let declared = data.children.iter().copied().filter(|c| matches!(self.state.design.decl_data(*c).kind, DeclKind::FnParam(_)));
        for param in declared {
            // An `Option<T>` parameter is two: `name.is` and `name.val` (§3.2), read as the pair
            // of an `Option` local.
            if let Some(Ty::Option(payload)) = self.typed_local(param) {
                let Some(payload) = self.value_type(&payload) else {
                    self.refuse(data.file, self.state.design.decl_data(param).span, ValueForm::OptionParameterWhosePayloadHasNoValue);
                    ok = false;
                    continue;
                };
                let name = self.state.design.decl_data(param).name.as_ref().map_or_else(String::new, |n| n.text.clone());
                let is = scope.values.locals.len() as u32;
                for (suffix, ty) in [("is", ValueType::Bool), ("val", payload)] {
                    let local = Local { name: format!("{name}.{suffix}"), ty, mutable: false };
                    scope.values.locals.push(local.clone());
                    params.push(local);
                }
                scope.values.options.insert(param, (is, is + 1));
                continue;
            }
            match self.bind_local(&mut scope, param) {
                Some(index) => {
                    params.push(scope.values.locals[index as usize].clone());
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
            self.refuse(data.file, data.span, ValueForm::LoweredFunctionWithoutBody);
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
            for local in scope.values.locals.iter_mut().skip(params.len()) {
                local.name = format!("_{}", local.name);
            }
        }
        Some(FunctionSpec { name: Variants::name(&name, half), origin: self.state.locator.decl(function), params, ret, locals: scope.values.locals, body: lowered })
    }

    /// The arena entry of every instance of a generic type the body and the signature of `function` hold, under the
    /// substitution of the instance being lowered (the types of a body that is not generic were registered with the
    /// arena; a generic body names its instances only here, T38).
    fn register_function_types(&mut self, function: DeclId) {
        let mut types: Vec<Ty> = (0..self.state.design.expr_count() as u32).map(crate::pom::ExprId).filter(|expr| self.state.design.expr_node(*expr).owner == function).filter_map(|expr| self.typed(expr)).collect();
        types.extend(self.state.design.decl_data(function).children.iter().filter_map(|child| self.typed_local(*child)));
        if let DeclKind::Fn(signature) = &self.state.design.decl_data(function).kind {
            types.extend(signature.ret.and_then(|ret| self.typed_written(ret)));
        }
        types.iter().for_each(|ty| self.register_instances(ty));
    }

    /// The name of `function` as written; a function of an `impl` with no parameters is named by the type the `impl` is for, `Sq.area`, so
    /// two types giving one method name stay two IR functions.
    pub(super) fn impl_qualified_name(&self, function: DeclId) -> String {
        let data = self.state.design.decl_data(function);
        let own = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        let Some(parent) = data.parent else { return own };
        // an `impl` with parameters is told apart by the arguments of its instances
        let DeclKind::Impl(owner) = &self.state.design.decl_data(parent).kind else { return own };
        // an `impl` with parameters is told apart by the arguments of its instances, and a trait `impl` also by its type and
        // its trait arguments (`impl BitAnd for Logic<N>` and `impl BitAnd<UInt<N>, Logic<N>> for Logic<N>` give two `bitand`)
        if self.state.design.decl_data(parent).generics_span.is_some() && owner.trait_.is_none() {
            return own;
        }
        let suffix = self.trait_arguments_suffix(parent);
        // the type as written, under no substitution: the name of an instance is the same wherever it is asked
        match self.state.consts.written_as_declared(owner.self_ty) {
            Some(ty) => format!("{}.{own}{suffix}", ty.source_name(self.state.design)),
            None => match self.nature_target_name(parent) {
                Some(nature) => format!("{nature}.{own}{suffix}"),
                None => own,
            },
        }
    }

    /// The name of the nature a trait `impl` is written for (`impl Resolve<f64> for Total`): a nature is no written
    /// type, so two `impl`s of one trait for two natures tell their methods apart by it.
    fn nature_target_name(&self, implementation: DeclId) -> Option<String> {
        let entry = self.state.impls.impls().iter().find(|entry| entry.decl == implementation)?;
        match &entry.target {
            crate::types::ImplTarget::Declared(decl) => self.state.design.decl_data(*decl).name.as_ref().map(|n| n.text.clone()),
            _ => None,
        }
    }

    /// `::<f64>` for an `impl` of a trait written with arguments (`impl Convert<f64> for Cell`), empty for any other: two
    /// `impl`s of one trait for one type, written with different arguments, each give a function of one name, and the
    /// arguments tell them apart. Not inlined: it runs once for every instantiation, deep in a recursion.
    #[inline(never)]
    fn trait_arguments_suffix(&self, owner: DeclId) -> String {
        let arguments = self.state.impls.impls().iter().find(|entry| entry.decl == owner).map(|entry| entry.trait_arguments.clone()).unwrap_or_default();
        let written: Vec<String> = arguments.iter().filter_map(|argument| self.state.consts.written_as_declared(*argument)).map(|ty| ty.source_name(self.state.design)).collect();
        if written.is_empty() { String::new() } else { format!("::<{}>", written.join(", ")) }
    }

    /// The IR type a type written in a signature names: the checker's type of it, as an IR type.
    pub(super) fn lower_type(&mut self, file: FileId, ty: TypeId) -> Option<ValueType> {
        let lowered = self.typed_written(ty).and_then(|t| self.value_type(&t));
        if lowered.is_none() {
            let span = self.state.design.type_node(ty).span;
            match self.typed_written(ty) {
                Some(crate::types::Ty::Enum(_)) => self.refuse(file, span, ValueForm::EnumOfOneVariantWhoseTagHas),
                _ => self.refuse(file, span, ValueForm::TypeThatHasNoValueInIr),
            }
        }
        lowered
    }
}
