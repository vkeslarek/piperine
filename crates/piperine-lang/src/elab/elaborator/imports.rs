//! Constants of the IR's arena read back as the trees the evaluator folds: a param bound taken apart, and the text that keys a body by its params.

use piperine_ir::{Constant, ValueType};
use crate::elab::consts::{Bindings, CompositeKind, ConstTree};
use super::Elaboration;

impl<'d, 't> Elaboration<'d, 't> {
    /// `value`, a param's constant, bound to `param` in `bindings`; a composite is also bound taken
    /// apart, for the evaluator that reads it (a `where`, a value over the param).
    pub(in crate::elab) fn bind_param(&self, bindings: &mut Bindings, param: crate::pom::DeclId, value: Constant) {
        match value {
            Constant::Aggregate(_) => bindings.bind_composite(param, value, self.imported_tree(value)),
            other => bindings.bind(param, other),
        }
    }

    /// The constant `value` of the IR's arena as the tree the evaluator reads: the reverse of
    /// `import_tree`. A unit variant of an enum with no field is its tag.
    pub(in crate::elab) fn imported_tree(&self, value: Constant) -> ConstTree {
        let Constant::Aggregate(id) = value else { return ConstTree::Scalar(value) };
        let data = self.state.builder.aggregate_at(id);
        let parts: Vec<ConstTree> = data.parts.iter().map(|part| self.imported_tree(*part)).collect();
        let kind = match (&data.ty, data.variant) {
            (ValueType::Tuple(_), _) => CompositeKind::Tuple,
            (ValueType::Array(..), _) => CompositeKind::Array,
            (ValueType::Option(_), _) => CompositeKind::Option,
            (ValueType::Enum(type_id), Some(position)) => {
                if self.state.values.type_arena.is_unit_only(*type_id) {
                    return ConstTree::Scalar(Constant::Int(i128::from(position)));
                }
                CompositeKind::Variant(position)
            }
            _ => CompositeKind::Struct,
        };
        ConstTree::Composite { kind, parts }
    }

    /// The text that tells the value `value` of a key param apart from every other, by what it holds: a composite is written out
    /// by its parts, so two equal composites (two equal arrays of templates) have one text and select one body variant, where the
    /// arena id of the aggregate (`agg2`) would give each its own.
    pub(in crate::elab) fn key_text(&self, value: Constant) -> String {
        Self::tree_text(&self.imported_tree(value))
    }

    /// `tree` written out: a scalar as its value, a composite as its parts in the brackets of its kind.
    pub(in crate::elab) fn tree_text(tree: &ConstTree) -> String {
        let (kind, parts) = match tree {
            ConstTree::Scalar(value) => return value.to_string(),
            ConstTree::Composite { kind, parts } => (kind, parts),
        };
        let written = parts.iter().map(Self::tree_text).collect::<Vec<_>>().join(",");
        match kind {
            CompositeKind::Tuple => format!("({written})"),
            CompositeKind::Array => format!("[{written}]"),
            CompositeKind::Struct => format!("{{{written}}}"),
            CompositeKind::Option => format!("option({written})"),
            CompositeKind::Variant(position) => format!("v{position}({written})"),
            CompositeKind::Range { .. } => format!("range({written})"),
        }
    }
}
