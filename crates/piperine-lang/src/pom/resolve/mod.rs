//! Name resolution: the stages that run after lowering and write only into
//! resolution slots and the diagnostics list. Each stage computes its answers
//! from the immutable design into an [`Output`], then the answers are applied.

mod behaviors;
mod classify;
mod imports;
mod members;
mod names;
mod scopes;
mod shape;

use std::cell::Cell;
use std::collections::BTreeMap;

use super::design::Design;
use super::ids::{DeclId, FileId, PatId, PathId};
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
    /// Identifier patterns that name a constant, a unit variant or a unit struct.
    pub pats: Vec<(PatId, Resolution)>,
}

/// The read-only state every stage shares.
pub(super) struct Ctx<'a> {
    design: &'a Design,
    scopes: scopes::Scopes,
    imports: imports::Imports,
    /// The namespace `prelude` of the standard library's root: its `pub` items are implicit.
    prelude: Option<DeclId>,
    /// The root file's namespace: what `crate` names.
    root: DeclId,
    /// The module each behavior block is bound to (stage 4).
    links: BTreeMap<DeclId, DeclId>,
    /// The `impl` blocks of each declaration, by the declaration their self
    /// type names (stage 5 reads associated items through them).
    impls: BTreeMap<DeclId, Vec<DeclId>>,
    /// The access names quantities declare (`v`, `i`, ..): a call of one that
    /// no declaration names is an access call, not an unresolved name.
    access: BTreeMap<String, Vec<DeclId>>,
    /// Set by a lookup that found a name through two glob imports of
    /// different declarations: the walk reports it as `E0162`, not `E0161`.
    ambiguous: Cell<bool>,
}

/// Run every resolution stage over a freshly lowered design.
pub(super) fn run(design: &mut Design) {
    design.fill_parents();
    let out = {
        let mut ctx = Ctx {
            design,
            scopes: scopes::Scopes::build(design),
            imports: imports::Imports::build(design),
            prelude: std_prelude(design),
            root: design.root_id(),
            links: BTreeMap::new(),
            impls: BTreeMap::new(),
            access: classify::access_table(design),
            ambiguous: Cell::new(false),
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
    for (pat, res) in out.pats {
        design.set_pat_res(pat, res);
    }
    for (behavior, module) in out.links {
        design.bind_behavior(behavior, module);
    }
    for (file, diagnostic) in out.diagnostics {
        design.push_diagnostic(file, diagnostic);
    }
    let classified = classify::run(design);
    for (port, kind) in classified.port_kinds {
        design.set_port_kind(port, kind);
    }
    for (decl, bundle) in classified.bundles {
        design.set_bundle(decl, bundle);
    }
    for facts in classified.structs {
        design.set_struct(facts.expr, facts.class, &facts.fields);
    }
    for (pat, index, res) in classified.pattern_fields {
        design.set_pattern_field(pat, index, res);
    }
    for facts in classified.calls {
        design.set_call(facts.expr, facts.class, &facts.args, &facts.params);
    }
    for (path, index, res) in classified.segments {
        design.set_segment_res(path, index, res);
    }
    for (module, expr) in classified.instances {
        design.push_instance(module, expr);
    }
    for (file, diagnostic) in classified.diagnostics {
        design.push_diagnostic(file, diagnostic);
    }
    let members = members::run(design);
    for (expr, res) in members.res {
        design.set_member(expr, res);
    }
    for (file, diagnostic) in members.diagnostics {
        design.push_diagnostic(file, diagnostic);
    }
    for (file, diagnostic) in shape::run(design) {
        design.push_diagnostic(file, diagnostic);
    }
    design.sort_diagnostics();
    design.index = super::index::build(design);
}

/// The `pub` namespace `prelude` declared at the root of the standard library (its first file).
fn std_prelude(design: &Design) -> Option<DeclId> {
    let root = design.files.iter().find(|f| f.prelude)?.root;
    design.decl_data(root).children.iter().copied().find(|c| {
        let data = design.decl_data(*c);
        data.public && matches!(data.kind, super::decl::DeclKind::Namespace(_)) && data.name.as_ref().is_some_and(|n| n.text == "prelude")
    })
}
