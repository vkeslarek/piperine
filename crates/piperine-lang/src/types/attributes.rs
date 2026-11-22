//! The attributes of a design against Appendix A.1's table: an attribute the table does not list,
//! or a listed one on the wrong kind of item, is `E0904` (`#[cfg(..)]` and `#[allow(..)]` included:
//! attributes are not an extension point).

use crate::pom::{AttrNode, DeclId, DeclKind, Design, StmtId};

use super::check::Checker;

/// The kind of item an attribute of A.1's table may sit on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttributeSite {
    /// A `struct` or an `enum`: `#[derive(..)]`.
    Type,
    /// An `enum`: `#[repr(uN)]`.
    Enum,
    /// An `extern "C" fn`: `#[derivative(..)]`.
    ExternFunction,
    /// An `extern` module: `#[link(..)]`.
    ExternModule,
    /// Any item (and any node that carries attributes): `#[doc = ".."]`.
    Any,
}

impl AttributeSite {
    /// Whether `decl` is an item of this kind.
    fn holds(self, design: &Design, decl: DeclId) -> bool {
        let kind = &design.decl_data(decl).kind;
        match self {
            AttributeSite::Type => matches!(kind, DeclKind::Struct(_) | DeclKind::Enum(_)),
            AttributeSite::Enum => matches!(kind, DeclKind::Enum(_)),
            AttributeSite::ExternFunction => matches!(kind, DeclKind::Fn(f) if f.abi.is_some()),
            AttributeSite::ExternModule => matches!(kind, DeclKind::Module(m) if m.abi.is_some()),
            AttributeSite::Any => true,
        }
    }

    /// How the kind reads in a message.
    fn describe(self) -> &'static str {
        match self {
            AttributeSite::Type => "a `struct` or an `enum`",
            AttributeSite::Enum => "an `enum`",
            AttributeSite::ExternFunction => "an `extern \"C\" fn`",
            AttributeSite::ExternModule => "an `extern` module",
            AttributeSite::Any => "any item",
        }
    }
}

/// Appendix A.1's table of recognized attributes, as data: each attribute name with the one kind of
/// item it may sit on.
pub(super) struct AttributeCatalogue;

impl AttributeCatalogue {
    /// The rows of A.1's table.
    const ROWS: [(&'static str, AttributeSite); 5] = [
        ("derive", AttributeSite::Type),
        ("repr", AttributeSite::Enum),
        ("derivative", AttributeSite::ExternFunction),
        ("link", AttributeSite::ExternModule),
        ("doc", AttributeSite::Any),
    ];

    /// The kind of item the attribute `name` may sit on, or `None` when A.1 does not list it.
    fn site(name: &str) -> Option<AttributeSite> {
        Self::ROWS.iter().find(|(row, _)| *row == name).map(|(_, site)| *site)
    }
}

impl Checker<'_> {
    /// One walk over every attribute of the design: each declaration's and each statement's. An
    /// attribute A.1 does not list, or one on the wrong kind of item, is `E0904` naming both.
    pub(super) fn check_attributes(&mut self) {
        for index in 0..self.design.decl_count() {
            let decl = DeclId(index as u32);
            for attr in self.design.decl_data(decl).meta.attrs.clone() {
                self.check_attribute(decl, &attr, true);
            }
        }
        for index in 0..self.design.stmt_count() {
            let node = self.design.stmt_node(StmtId(index as u32)).clone();
            for attr in &node.meta.attrs {
                self.check_attribute(node.owner, attr, false);
            }
        }
    }

    /// One attribute of `owner` (or of a statement in it, when `on_item` is false).
    fn check_attribute(&mut self, owner: DeclId, attr: &AttrNode, on_item: bool) {
        let path = self.design.path_node(attr.path).clone();
        let name = path.segments.iter().map(|s| s.name.text.as_str()).collect::<Vec<_>>().join("::");
        let file = self.design.decl_data(owner).file;
        let owner_name = self.design.decl_data(owner).name.as_ref().map_or_else(|| "an item".to_string(), |n| format!("`{}`", n.text));
        let message = match AttributeCatalogue::site(&name) {
            None => format!("the attribute `#[{name}]` is not one A.1 lists: attributes are not an extension point"),
            Some(site) if on_item && !site.holds(self.design, owner) => format!("the attribute `#[{name}]` sits on {owner_name}, but A.1 allows it only on {}", site.describe()),
            Some(AttributeSite::Any) => return,
            Some(site) if !on_item => format!("the attribute `#[{name}]` sits on a statement of {owner_name}, but A.1 allows it only on {}", site.describe()),
            Some(_) => return,
        };
        self.coded(file, "E0904", attr.span, message, "an attribute A.1 does not allow here");
    }
}
