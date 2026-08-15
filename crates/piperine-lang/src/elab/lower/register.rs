//! Top-level symbol registration: a first pass over `SourceFile::items`
//! that populates the `Elaborator`'s symbol tables before anything is
//! resolved or elaborated.

use crate::ast::Item;

use crate::pom::ElabError;

use super::Elaborator;

impl Elaborator {
    // ──────────────────────── Symbol registration ─────────────────────────────

    /// Populates the elaborator's symbol tables from the top-level items of
    /// a source file. Each `Item` variant is inserted into its
    /// corresponding map.
    pub(crate) fn register_items<'a>(
        &mut self,
        items: impl Iterator<Item = &'a Item>,
    ) -> Result<(), ElabError> {
        for item in items {
            match item {
                Item::Discipline(d) => {
                    self.syms.disciplines.insert(d.name.clone(), d.clone());
                }
                Item::Nature(n) => {
                    self.syms.natures.insert(n.name.clone(), n.clone());
                }
                Item::Module(m) => {
                    self.syms.module_decls.insert(m.name.clone(), m.clone());
                }
                // `connectrules` (LRM ch. 7) registered starting T52 —
                // `InsertConnectModules` reads `elab.syms.connectrules`.
                Item::Connectrules(cr) => {
                    self.syms.connectrules.insert(cr.name.clone(), cr.clone());
                }
                // `paramset` registered starting T29 (LRMC-C5-07) —
                // `lower_instance` reads `elab.syms.paramsets`.
                Item::Paramset(p) => {
                    self.syms.paramsets.insert(p.name.clone(), p.clone());
                }
                // `primitive` (UDP) registered starting T39 (LRMC-C7-13)
                // — `lower_instance` reads `elab.syms.primitives`.
                Item::Primitive(p) => {
                    self.syms.primitives.insert(p.name.clone(), p.clone());
                }
                // `config` (1364 configuration-selection) lands in a
                // later phase (spec Out of Scope).
                Item::Config(_) => {}
            }
        }
        Ok(())
    }
}
