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
                Item::BenchDecl(b) => {
                    self.syms.bench_decls.push(b.clone());
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
            }
        }
        Ok(())
    }

}
