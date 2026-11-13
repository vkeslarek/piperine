//! Name resolution: the stages that run after lowering and write only into
//! resolution slots and the diagnostics list. Each stage computes its answers
//! from the immutable design into an [`Output`], then the answers are applied.

mod behaviors;
mod imports;
mod names;
mod scopes;

use std::collections::BTreeMap;

use super::design::Design;
use super::ids::{DeclId, FileId, PathId};
use super::resolution::Resolution;
use crate::msl::Diagnostic;

/// What the stages computed, not yet written into the design.
#[derive(Default)]
pub(super) struct Output {
    /// `(path, segment index, what the segment names)`.
    pub segments: Vec<(PathId, usize, Resolution)>,
    pub diagnostics: Vec<(FileId, Diagnostic)>,
    /// `(behavior block, module)`.
    pub links: BTreeMap<DeclId, DeclId>,
}

/// The read-only state every stage shares.
pub(super) struct Ctx<'a> {
    design: &'a Design,
    scopes: scopes::Scopes,
    imports: imports::Imports,
    /// The root namespace of each prelude source.
    prelude: Vec<DeclId>,
    /// The root file's namespace: what `crate` names.
    root: DeclId,
    /// The module each behavior block is bound to (stage 4).
    links: BTreeMap<DeclId, DeclId>,
    /// The `impl` blocks of each declaration, by the declaration their self
    /// type names (stage 5 reads associated items through them).
    impls: BTreeMap<DeclId, Vec<DeclId>>,
}

/// Run every resolution stage over a freshly lowered design.
pub(super) fn run(design: &mut Design) {
    let out = {
        let mut ctx = Ctx {
            design,
            scopes: scopes::Scopes::build(design),
            imports: imports::Imports::build(design),
            prelude: design.files.iter().filter(|f| f.prelude).map(|f| f.root).collect(),
            root: design.root_id(),
            links: BTreeMap::new(),
            impls: BTreeMap::new(),
        };
        let mut out = Output::default();
        out.diagnostics.extend(ctx.scopes.diagnostics(design));
        ctx.resolve_imports(&mut out);
        ctx.links = ctx.bind_behaviors(&mut out);
        ctx.impls = ctx.collect_impls();
        ctx.resolve_names(&mut out);
        out.links = ctx.links;
        out
    };
    for (path, index, res) in out.segments {
        design.set_segment_res(path, index, res);
    }
    for (behavior, module) in out.links {
        design.bind_behavior(behavior, module);
    }
    for (file, diagnostic) in out.diagnostics {
        design.push_diagnostic(file, diagnostic);
    }
    design.sort_diagnostics();
}
