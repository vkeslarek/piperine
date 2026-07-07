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
                    self.disciplines.insert(d.name.clone(), d.clone());
                    self.ctx.types.register(d.clone());
                }
                Item::BundleDecl(b) => {
                    self.bundles.insert(b.name.clone(), b.clone());
                    self.ctx.types.register(b.clone());
                    // Register as an attribute schema if marked
                    // `@attribute(schema = "BundleName")`.
                    if let Some(schema_name) = b.attrs.iter().find_map(|a| {
                        if a.name == "attribute" {
                            a.args.iter().find(|arg| arg.name == "schema").and_then(|arg| {
                                if let crate::parse::ast::Expr::Literal(crate::parse::ast::Literal::String(s)) = &arg.expr {
                                    Some(s.clone())
                                } else {
                                    None
                                }
                            })
                        } else {
                            None
                        }
                    }) {
                        self.ctx.schemas.register(&schema_name);
                    }
                }
                Item::EnumDecl(e) => {
                    self.enums.insert(e.name.clone(), e.clone());
                    self.ctx.types.register(e.clone());
                }
                Item::ModuleDeclaration(m) => {
                    self.module_decls.insert(m.name.clone(), m.clone());
                    self.ctx.components.register(m.clone());
                }
                Item::BehaviorDecl(b) => {
                    self.behavior_decls.push(b.clone());
                }
                Item::BenchDecl(b) => {
                    self.bench_decls.push(b.clone());
                }
                Item::FnDecl(f) => {
                    self.fn_decls.insert(f.sig.name.clone(), f.clone());
                    self.ctx.callables.register(f.clone());
                }
                Item::CapabilityDecl(c) => {
                    self.capability_decls.insert(c.name.clone(), c.clone());
                }
                Item::ImplDecl(i) => {
                    self.impl_decls.push(i.clone());
                }
                Item::ConstDecl(c) => {
                    self.const_decls.insert(c.name.clone(), c.clone());
                }
                Item::UseDecl(_) => {} // already expanded by Resolver
            }
        }
        Ok(())
    }

}
