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
                // `paramset`/`config`/`primitive` (UDP) land in later
                // phases (T22 extends this list as their own POM
                // collections are wired up).
                Item::Paramset(_) | Item::Config(_) | Item::Primitive(_) => {}
            }
        }
        Ok(())
    }
}
