//! Stage 8: the reverse tables. Everything here is derived from the resolved
//! design: where a declaration is named, who writes it, who instantiates it.
//! The tables are computed once at the end of the build and only read after.

use serde::{Deserialize, Serialize};

use super::body::{ExprKind, PatKind, StmtKind};
use super::decl::DeclKind;
use super::design::Design;
use super::ids::{DeclId, ExprId, PatId, PathId, StmtId};
use super::resolution::{CallClass, Resolution};

/// A node that names a declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferenceId {
    /// A path with a segment that resolved to the declaration.
    Path(PathId),
    /// A `.name` or `.method()` expression that resolved to it.
    Member(ExprId),
    /// An identifier pattern that names it (a constant, a unit variant, a unit struct).
    Pattern(PatId),
}

/// A statement or item that writes a place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WriterId {
    /// `place = value;` in a module body.
    Definition(DeclId),
    /// `place <- value;` in a reaction.
    Commit(StmtId),
}

/// The three tables, each indexed by `DeclId`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Index {
    references: Vec<Vec<ReferenceId>>,
    writers: Vec<Vec<WriterId>>,
    instantiated_by: Vec<Vec<ExprId>>,
}

impl Index {
    pub fn references(&self, decl: DeclId) -> &[ReferenceId] {
        self.references.get(decl.index()).map_or(&[], Vec::as_slice)
    }

    pub fn writers(&self, decl: DeclId) -> &[WriterId] {
        self.writers.get(decl.index()).map_or(&[], Vec::as_slice)
    }

    pub fn instantiated_by(&self, decl: DeclId) -> &[ExprId] {
        self.instantiated_by.get(decl.index()).map_or(&[], Vec::as_slice)
    }
}

/// The declaration at the root of a place: `Field`, `Index` and `Paren` are
/// stripped down to a path, whose last segment names it.
fn place_root(design: &Design, mut at: ExprId) -> Option<DeclId> {
    loop {
        match &design.expr_node(at).kind {
            ExprKind::Field { expr, .. } | ExprKind::Index { expr, .. } | ExprKind::Paren(expr) => at = *expr,
            ExprKind::Path(p) => {
                return match design.path_node(*p).segments.last()?.res {
                    Resolution::Decl(d) => Some(d),
                    _ => None,
                };
            }
            _ => return None,
        }
    }
}

/// Compute the tables of a resolved design.
pub(super) fn build(design: &Design) -> Index {
    let n = design.decl_count();
    let mut references: Vec<Vec<(usize, usize, ReferenceId)>> = vec![Vec::new(); n];
    let mut writers: Vec<Vec<(usize, usize, WriterId)>> = vec![Vec::new(); n];
    let mut instantiated_by: Vec<Vec<ExprId>> = vec![Vec::new(); n];
    let file_of = |owner: DeclId| design.decl_data(owner).file.index();

    for i in 0..design.path_count() {
        let id = PathId(i as u32);
        let node = design.path_node(id);
        let mut seen: Vec<DeclId> = Vec::new();
        for segment in &node.segments {
            if let Resolution::Decl(d) = segment.res {
                if !seen.contains(&d) {
                    seen.push(d);
                    references[d.index()].push((file_of(node.owner), node.span.start, ReferenceId::Path(id)));
                }
            }
        }
    }
    for i in 0..design.expr_count() {
        let id = ExprId(i as u32);
        let node = design.expr_node(id);
        if let ExprKind::Field { res: Resolution::Decl(d), .. } | ExprKind::MethodCall { res: Resolution::Decl(d), .. } = &node.kind {
            references[d.index()].push((file_of(node.owner), node.span.start, ReferenceId::Member(id)));
        }
    }
    for i in 0..design.pat_count() {
        let id = PatId(i as u32);
        let node = design.pat_node(id);
        if let PatKind::Ident { res: Resolution::Decl(d), .. } = &node.kind {
            references[d.index()].push((file_of(node.owner), node.span.start, ReferenceId::Pattern(id)));
        }
    }
    for i in 0..n {
        let id = DeclId(i as u32);
        let data = design.decl_data(id);
        match &data.kind {
            DeclKind::Definition(def) => {
                if let Some(root) = place_root(design, def.place) {
                    writers[root.index()].push((data.file.index(), data.span.start, WriterId::Definition(id)));
                }
            }
            DeclKind::Module(m) => {
                for e in &m.instances {
                    if let ExprKind::Call { class: CallClass::Instance { module: Some(target), .. }, .. } = &design.expr_node(*e).kind {
                        instantiated_by[target.index()].push(*e);
                    }
                }
            }
            _ => {}
        }
    }
    for i in 0..design.stmt_count() {
        let id = StmtId(i as u32);
        let node = design.stmt_node(id);
        if let StmtKind::Commit { place, .. } = &node.kind {
            if let Some(root) = place_root(design, *place) {
                writers[root.index()].push((file_of(node.owner), node.span.start, WriterId::Commit(id)));
            }
        }
    }

    fn ordered<T: Copy>(mut v: Vec<(usize, usize, T)>) -> Vec<T> {
        v.sort_by_key(|(file, start, _)| (*file, *start));
        v.into_iter().map(|(_, _, t)| t).collect()
    }
    Index {
        references: references.into_iter().map(ordered).collect(),
        writers: writers.into_iter().map(ordered).collect(),
        instantiated_by,
    }
}
