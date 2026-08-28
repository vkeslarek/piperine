//! Top-level symbol registration: a first pass over `SourceFile::items`
//! that populates the `Elaborator`'s symbol tables before anything is
//! resolved or elaborated.

use crate::ast::Item;

use crate::pom::{ElabError, ElabErrorKind};

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
                // `lower_instance` reads `elab.syms.paramsets`. Pushed,
                // never inserted-overwriting: §6.4.2 paramset overloading
                // (LRMW2-22, wave-2 T29) lets several paramsets share a
                // name, so every declaration under that name is kept and
                // `select_paramset` chooses among them per instance.
                Item::Paramset(p) => {
                    self.syms.paramsets.entry(p.name.clone()).or_default().push(p.clone());
                }
                // `primitive` (UDP) registered starting T39 (LRMC-C7-13)
                // — `lower_instance` reads `elab.syms.primitives`.
                Item::Primitive(p) => {
                    self.syms.primitives.insert(p.name.clone(), p.clone());
                }
                // `config` (1364 configuration-selection, Annex A.1.5,
                // T22, LRMW3-08 / spec P2 AC4): fails loud instead of
                // silently accepting and discarding it — Piperine's own
                // `Piperine.toml`/`SourceMap` is the replacement (spec §0
                // rule 1: an exclusion names what to use instead).
                Item::Config(c) => {
                    let span = miette::SourceSpan::new(c.span.start.into(), c.span.end.saturating_sub(c.span.start));
                    return Err(ElabError::from(ElabErrorKind::UnsupportedConstruct {
                        construct: format!("config declaration `{}`", c.name),
                        clause: "A.1.5".to_string(),
                        replacement: "Piperine.toml / SourceMap (Piperine has no 1364 configuration-selection mechanism — build configuration lives in Piperine.toml)".to_string(),
                    })
                    .with_span(Some(span)));
                }
                // `const` (sanctioned extension, MD-50) is evaluated and
                // landed on `Design::consts` by the `LowerDeclarations`
                // pass — the eval needs the ConstEnv, not registration.
                Item::Const(c) => self.pending_consts.push(c.clone()),
            }
        }
        Ok(())
    }
}
