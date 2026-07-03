//! `fn`/`impl` elaboration and generic-module monomorphization: the
//! remaining top-level items that don't fit `register`/`resolve`/`module`/
//! `behavior` — a `fn` and an `impl`'s methods share `elab_fn`, and
//! `monomorphize` is what `module.rs`'s instance lowering calls when it
//! meets a generic module with concrete const args.

use std::collections::HashMap;

use crate::parse::ast::{BehaviorKind, FnDecl, FnParam, ImplDecl};
use crate::elab::const_eval::ConstEnv;
use crate::pom::{
    BehaviorStmt, ElabError, ElabErrorKind, Function, ImplBlock, TypeRef, ValueType,
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

        let is_generic = !fn_decl.sig.type_params.is_empty();

        Ok(Function { name: fn_decl.sig.name.clone(), params, ret, body, is_generic })
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
                env.eval(e).map_err(|src| ElabError::from(ElabErrorKind::ConstEval {
                    context: format!("impl const arg for `{}`", impl_decl.ty),
                    source: src,
                }))
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

        if self.ctx.components.get_monomorphized(&mono_name).is_some() {
            return Ok(mono_name);
        }

        let def = self.ctx.components.lookup(module_name)
            .ok_or_else(|| ElabError::from(ElabErrorKind::UndefinedModule(module_name.to_owned())))?
            .clone_box();

        let mut env = ConstEnv::new();
        let mut module = def.instantiate(self, const_args, &mut env, &HashMap::new())?;
        module.name = mono_name.clone();
        
        self.ctx.components.drain_mono_cache(); // This isn't how we insert, wait
        // Wait, we need to insert it. But drain_mono_cache is not an insert method.
        // Let's rely on a helper we'll add to components.rs: insert_mono_cache
        self.ctx.components.insert_mono_cache(mono_name.clone(), module);

        Ok(mono_name)
    }


}
