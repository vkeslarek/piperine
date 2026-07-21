//! Top-level symbol registration: a first pass over `SourceFile::items`
//! that populates the `Elaborator`'s symbol tables before anything is
//! resolved or elaborated.

use crate::parse::ast::Item;
use crate::pom::ElabError;

use super::Elaborator;

impl Elaborator {
    // ──────────────────────── Symbol registration ─────────────────────────────

    /// Populates the elaborator's symbol tables from the top-level items
    /// of a source file. Each `Item` variant is inserted into the
    /// corresponding map (disciplines, bundles, enums, modules, functions,
    /// capabilities) or pushed into a vector (behaviors, impl blocks).
    /// `UseDecl` items are skipped as they are already expanded by the
    /// resolver.
    pub(crate) fn register_items<'a>(
        &mut self,
        items: impl Iterator<Item = &'a Item>,
    ) -> Result<(), ElabError> {
        for item in items {
            match item {
                Item::DisciplineDecl(d) => {
                    self.syms.disciplines.insert(d.name.clone(), d.clone());
                    self.ctx.types.register(crate::elab::registry::TypeDefKind::Discipline(d.clone()));
                }
                Item::BundleDecl(b) => {
                    self.syms.bundles.insert(b.name.clone(), b.clone());
                    self.ctx.types.register(crate::elab::registry::TypeDefKind::Bundle(b.clone()));
                    // Register as an attribute schema if marked
                    // `@attribute(schema = "name")`. The schema name is an
                    // alias — `@name(...)` is used in source, not the bundle
                    // name directly.
                    for attr in &b.attrs {
                        if attr.name == "attribute" {
                            for arg in &attr.args {
                                if arg.name == "schema"
                                    && let crate::parse::ast::Expr::Literal(crate::parse::ast::Literal::String(s)) = &arg.expr {
                                        self.ctx.schemas.register(s, &b.name);
                                    }
                            }
                        }
                    }
                }
                Item::EnumDecl(e) => {
                    self.syms.enums.insert(e.name.clone(), e.clone());
                    self.ctx.types.register(crate::elab::registry::TypeDefKind::Enum(e.clone()));
                }
                Item::ModuleDeclaration(m) => {
                    self.syms.module_decls.insert(m.name.clone(), m.clone());
                    self.ctx.components.register(m.clone());
                }
                Item::BehaviorDecl(b) => {
                    self.syms.behavior_decls.push(b.clone());
                }
                Item::FnDecl(f) => {
                    self.syms.fn_decls.insert(f.sig.name.clone(), f.clone());
                    self.ctx.callables.register(f.clone());
                }
                Item::CapabilityDecl(c) => {
                    self.syms.capability_decls.insert(c.name.clone(), c.clone());
                }
                Item::ImplDecl(i) => {
                    self.syms.impl_decls.push(i.clone());
                }
                Item::ConstDecl(c) => {
                    self.syms.const_decls.insert(c.name.clone(), c.clone());
                }
                Item::UseDecl(_) => {} // already expanded by Resolver
                // `extern type Name;` registers into TypeRegistry alongside
                // plain types (declared-language-surface T7, DLS-01
                // groundwork). Types are not overloadable — a name already
                // taken by any type declaration (plain or extern) is an
                // ordinary duplicate-declaration error (SPEC Edge Cases: no
                // shadowing of an `extern` declaration).
                Item::ExternDecl(crate::parse::ast::ExternDecl::Type { span, name }) => {
                    if self.ctx.types.lookup(name).is_some() {
                        return Err(ElabError::from(crate::pom::ElabErrorKind::Other(format!(
                            "type `{name}` is already declared (duplicate `extern type`/type declaration)"
                        ))).with_span(*span));
                    }
                    self.ctx.types.register(crate::elab::registry::TypeDefKind::Extern {
                        name: name.clone(),
                        decl_span: *span,
                    });
                }
                // `extern fn`/`extern task` register into `CallableRegistry`
                // as an overload candidate (declared-language-surface T11,
                // DLS-01/03 groundwork) — the `resolve.rs` fail-loud call
                // path (T11) is their first real consumer. `extern task`
                // shares `CallableRegistry`'s namespace with `extern fn`
                // (both are just named, typed callables); its `$`-prefixed
                // form keeps it distinct from any `fn` name in practice.
                Item::ExternDecl(crate::parse::ast::ExternDecl::Fn(sig))
                | Item::ExternDecl(crate::parse::ast::ExternDecl::Task(sig)) => {
                    let param_types = self.extern_sig_param_types(sig)?;
                    self.ctx.callables.register(crate::elab::registry::ExternFnDecl {
                        sig: sig.clone(),
                        param_types,
                    });
                }
                // `extern operator name(...) -> Ret;` registers into the
                // dedicated `OperatorRegistry` (T10) — real enforcement for
                // operator calls lands in T22; this task just completes
                // registration so `extern operator` declarations aren't
                // silently dropped.
                Item::ExternDecl(crate::parse::ast::ExternDecl::Operator(sig)) => {
                    self.ctx.operators.register(crate::elab::registry::ExternOperatorDecl(sig.clone()));
                }
                // `extern attribute name { field: Type, ... }` registers
                // into `SchemaRegistry` exactly like a bundle-backed schema
                // (T12) — `@name(...)` attribute validation (`elab/lower/
                // attrs.rs`) already fails loud (`UnknownAttrSchema`) for
                // any name not registered here, so this wiring alone closes
                // DLS-04 for the attribute-schema category.
                Item::ExternDecl(crate::parse::ast::ExternDecl::Attribute { name, fields, .. }) => {
                    let attr_fields = fields
                        .iter()
                        .map(|f| crate::elab::registry::AttrField {
                            name: f.name.clone(),
                            ty: f.ty.name.clone(),
                            required: true,
                            default: None,
                            decl_span: f.span,
                        })
                        .collect();
                    self.ctx.schemas.register_declared(name, attr_fields);
                }
                // `extern impl TypeName { fn method(...) -> Ret; ... }`
                // registers each method into the impl-method table (T10),
                // keyed by `(target, method.name)` — `resolve.rs`'s
                // `Type::method(...)` call path (T11) is its first real
                // consumer.
                Item::ExternDecl(crate::parse::ast::ExternDecl::Impl { target, methods, .. }) => {
                    for method in methods {
                        let param_types = self.extern_sig_param_types(method)?;
                        self.ctx.impl_methods.register_impl_method(
                            target,
                            crate::elab::registry::ExternFnDecl { sig: method.clone(), param_types },
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// Resolves an `extern fn`/`extern task`/`extern impl` method
    /// signature's declared parameter types to `ValueType`s, for storage in
    /// `ExternFnDecl::param_types` — skips `self` (not a call-site
    /// argument). Requires every referenced type to already be registered
    /// (an author declaring `extern fn foo(x: Bar)` before `Bar` exists
    /// gets the ordinary `UndefinedType` error, same as any other type use).
    fn extern_sig_param_types(
        &self,
        sig: &crate::parse::ast::ExternSig,
    ) -> Result<Vec<crate::pom::ValueType>, ElabError> {
        sig.params
            .iter()
            .filter_map(|p| match p {
                crate::parse::ast::FnParam::SelfParam => None,
                crate::parse::ast::FnParam::Typed { ty, .. } => {
                    Some(self.resolve_value_type(ty, &crate::elab::const_eval::ConstEnv::new()))
                }
            })
            .collect()
    }
}
