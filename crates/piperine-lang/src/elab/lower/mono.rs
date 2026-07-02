//! `fn`/`impl` elaboration and generic-module monomorphization: the
//! remaining top-level items that don't fit `register`/`resolve`/`module`/
//! `behavior` — a `fn` and an `impl`'s methods share `elab_fn`, and
//! `monomorphize` is what `module.rs`'s instance lowering calls when it
//! meets a generic module with concrete const args.

use std::collections::HashMap;

use crate::parse::ast::{BehaviorKind, FnDecl, FnParam, ImplDecl};
use crate::elab::const_eval::{ConstEnv, ConstVal};
use crate::pom::{
    BehaviorStmt, ElabError, Function, ImplBlock, Module, TypeRef, ValueType,
};

use super::Elaborator;

impl Elaborator {
    // ─────────────────────────── Function elaboration ─────────────────────────

    /// Elaborates a `FnDecl` into a `Function`. Resolves parameter types
    /// and return type, then lowers the function body from raw `Stmt` AST
    /// nodes into `BehaviorStmt`s. An implicit return from the trailing
    /// expression is appended if present.
    pub(crate) fn elab_fn(&self, fn_decl: &FnDecl) -> Result<Function, ElabError> {
        let mut env = ConstEnv::new();

        let params = fn_decl
            .sig
            .params
            .iter()
            .filter_map(|p| match p {
                FnParam::SelfParam => None,
                FnParam::Typed(name, ty) => {
                    let resolved = self.resolve_type(ty, &env, &HashMap::new()).ok()?;
                    Some(Ok((name.clone(), resolved)))
                }
            })
            .collect::<Result<Vec<_>, ElabError>>()?;

        let ret = self
            .resolve_type(&fn_decl.sig.ret, &env, &HashMap::new())
            .unwrap_or(TypeRef::Value(ValueType::Real));

        // Lower the body from raw Stmt AST to BehaviorStmt.
        // Functions use Analog as a placeholder kind (no behavior-specific ops allowed).
        let mut body: Vec<BehaviorStmt> = fn_decl
            .body
            .stmts
            .iter()
            .map(|s| self.lower_stmt_to_behavior(s, BehaviorKind::Analog, &mut env))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();

        // Trailing expression becomes an implicit return.
        if let Some(expr) = &fn_decl.body.expr {
            body.push(BehaviorStmt::Expr(*expr.clone()));
        }

        Ok(Function { name: fn_decl.sig.name.clone(), params, ret, body })
    }

    // ─────────────────────────── Impl elaboration ─────────────────────────────

    /// Elaborates an `ImplDecl` into an `ImplBlock`. Evaluates const args
    /// and elaborates each method via [`elab_fn`](Elaborator::elab_fn).
    pub(crate) fn elab_impl(&self, impl_decl: &ImplDecl) -> Result<ImplBlock, ElabError> {
        let env = ConstEnv::new();
        let const_args = impl_decl
            .const_args
            .iter()
            .map(|e| {
                env.eval(e).map_err(|src| ElabError::ConstEval {
                    context: format!("impl const arg for `{}`", impl_decl.ty),
                    source: src,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let methods = impl_decl
            .methods
            .iter()
            .map(|m| self.elab_fn(m))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ImplBlock {
            capability: impl_decl.capability.clone(),
            ty: impl_decl.ty.clone(),
            const_args,
            methods,
        })
    }

    // ──────────────────── Generic module monomorphization ─────────────────────

    /// Elaborate a generic module on demand with specific const substitutions.
    /// Caches the result in `mono_cache`. Returns the monomorphized name.
    pub fn monomorphize(
        &mut self,
        module_name: &str,
        const_args: &[u64],
    ) -> Result<String, ElabError> {
        let mono_name = if const_args.is_empty() {
            module_name.to_owned()
        } else {
            let suffix: Vec<String> = const_args.iter().map(|n| n.to_string()).collect();
            format!("{}__{}", module_name, suffix.join("_"))
        };

        if self.mono_cache.contains_key(&mono_name) {
            return Ok(mono_name);
        }

        let decl = self
            .module_decls
            .get(module_name)
            .cloned()
            .ok_or_else(|| ElabError::UndefinedModule(module_name.to_owned()))?;

        if decl.const_params.len() != const_args.len() {
            return Err(ElabError::Other(format!(
                "module `{}` expects {} const params, got {}",
                module_name,
                decl.const_params.len(),
                const_args.len()
            )));
        }

        let mut env = ConstEnv::new();
        for (param_name, &val) in decl.const_params.iter().zip(const_args.iter()) {
            env.define(param_name.clone(), ConstVal::Nat(val));
        }

        let mut mono_decl = decl.clone();
        mono_decl.name = mono_name.clone();

        // Insert a placeholder to break potential recursion.
        self.mono_cache.insert(mono_name.clone(), Module {
            attributes: Vec::new(),
            name: mono_name.clone(),
            ports: vec![],
            params: vec![],
            wires: vec![],
            vars: vec![],
            instances: vec![],
            connections: vec![],
            behaviors: vec![],
        });

        let elab_mod = self.elab_mod_inner(&mono_decl, &mut env, &HashMap::new())?;
        self.mono_cache.insert(mono_name.clone(), elab_mod);
        Ok(mono_name)
    }
}
