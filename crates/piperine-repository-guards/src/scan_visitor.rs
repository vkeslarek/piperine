//! The walk over one file's syntax that produces its `ScannedItem`s.

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, Fields, Meta, Type};

use crate::item_scan::{FreeFunctionPermit, ScannedKind, CodeOrigin, ScannedItem};
use crate::module_declaration::ModuleDeclaration;
use crate::source_tree::RustSource;

/// The shortest closure, in lines, that counts as a function.
const LONG_CLOSURE_LINES: usize = 15;

/// The lint levels whose attributes are listed.
const LINT_LEVELS: [&str; 5] = ["allow", "expect", "warn", "deny", "forbid"];

/// The `impl` or `trait` the walk is inside.
struct Enclosing {
    name: String,
    trait_name: Option<String>,
}

/// Walks one file and appends what it finds to `items`.
pub struct ScanVisitor<'a> {
    file: &'a RustSource,
    items: &'a mut Vec<ScannedItem>,
    modules: Vec<String>,
    enclosing: Vec<Enclosing>,
    test_depth: usize,
    function_depth: usize,
}

impl<'a> ScanVisitor<'a> {
    /// Scans a parsed file: its `File` item, then everything inside it.
    pub fn scan_file(file: &'a RustSource, syntax: &syn::File, items: &'a mut Vec<ScannedItem>) {
        let mut visitor = ScanVisitor { file, items, modules: Vec::new(), enclosing: Vec::new(), test_depth: 0, function_depth: 0 };
        let mut item = visitor.item(ScannedKind::File, file.path().to_string_lossy().to_string(), 1, file.line_count().max(1), &syntax.attrs);
        item.documented = Self::is_documented(&syntax.attrs);
        item.origin = if file.is_test() { CodeOrigin::Test } else { CodeOrigin::Product };
        visitor.items.push(item);
        visitor.visit_file(syntax);
    }

    /// Whether an attribute list holds a doc comment.
    fn is_documented(attributes: &[Attribute]) -> bool {
        attributes.iter().any(|attribute| attribute.path().is_ident("doc"))
    }

    /// Whether an attribute list holds `#[test]` or a path ending in `test`.
    fn has_test_attribute(attributes: &[Attribute]) -> bool {
        attributes.iter().any(|attribute| attribute.path().segments.last().is_some_and(|segment| segment.ident == "test"))
    }

    /// Test when the file, an enclosing module or the attributes say so, product otherwise.
    fn origin_of(&self, attributes: &[Attribute]) -> CodeOrigin {
        if self.file.is_test() || self.test_depth > 0 || ModuleDeclaration::is_cfg_test(attributes) || Self::has_test_attribute(attributes) {
            CodeOrigin::Test
        } else {
            CodeOrigin::Product
        }
    }

    /// A new item at `line`, `length` lines long, with the context the walk is in.
    fn item(&self, kind: ScannedKind, name: String, line: usize, length: usize, attributes: &[Attribute]) -> ScannedItem {
        ScannedItem {
            kind,
            name,
            scope: self.scope(),
            crate_name: self.file.crate_name().to_string(),
            file: self.file.path().to_path_buf(),
            line,
            length,
            documented: Self::is_documented(attributes),
            origin: self.origin_of(attributes),
            trait_name: self.enclosing.last().and_then(|enclosing| enclosing.trait_name.clone()),
            permit: None,
            detail: String::new(),
        }
    }

    /// The `impl`, `trait` or inline module path the walk is inside.
    fn scope(&self) -> String {
        match self.enclosing.last() {
            Some(enclosing) => enclosing.name.clone(),
            None => self.modules.join("::"),
        }
    }

    /// An item whose first line is `start`'s and whose last line is `end`'s.
    fn spanning(&self, kind: ScannedKind, name: String, start: Span, end: Span, attributes: &[Attribute]) -> ScannedItem {
        let line = start.start().line;
        self.item(kind, name, line, end.end().line.saturating_sub(line) + 1, attributes)
    }

    /// Runs `walk` with the test depth raised when the attributes say `#[cfg(test)]`.
    fn within_test_scope(&mut self, attributes: &[Attribute], walk: impl FnOnce(&mut Self)) {
        let raised = ModuleDeclaration::is_cfg_test(attributes);
        self.test_depth += usize::from(raised);
        walk(self);
        self.test_depth -= usize::from(raised);
    }

    /// The name of the type an `impl` is for, looking through references.
    fn type_name(ty: &Type) -> String {
        match ty {
            Type::Path(path) => path.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default(),
            Type::Reference(reference) => Self::type_name(&reference.elem),
            Type::Paren(paren) => Self::type_name(&paren.elem),
            Type::Group(group) => Self::type_name(&group.elem),
            _ => "<type>".to_string(),
        }
    }

    /// Appends the item of a function or method found at `signature`.
    fn function_item(&mut self, signature: &syn::Signature, end: Span, attributes: &[Attribute], kind: ScannedKind) {
        let mut item = self.spanning(kind, signature.ident.to_string(), signature.span(), end, attributes);
        if kind == ScannedKind::FreeFunction {
            item.permit = if signature.ident == "main" {
                Some(FreeFunctionPermit::Main)
            } else if signature.abi.is_some() {
                Some(FreeFunctionPermit::ForeignAbi)
            } else if Self::has_test_attribute(attributes) {
                Some(FreeFunctionPermit::TestAttribute)
            } else {
                None
            };
        }
        self.items.push(item);
    }

    /// Runs `walk` one function body deeper.
    fn visit_function_body(&mut self, walk: impl FnOnce(&mut Self)) {
        self.function_depth += 1;
        walk(self);
        self.function_depth -= 1;
    }
}

impl<'ast> Visit<'ast> for ScanVisitor<'_> {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let item = self.spanning(ScannedKind::Module, node.ident.to_string(), node.ident.span(), node.span(), &node.attrs);
        self.items.push(item);
        self.within_test_scope(&node.attrs, |visitor| {
            visitor.modules.push(node.ident.to_string());
            syn::visit::visit_item_mod(visitor, node);
            visitor.modules.pop();
        });
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        let item = self.spanning(ScannedKind::Type, node.ident.to_string(), node.ident.span(), node.span(), &node.attrs);
        self.items.push(item);
        let owner = node.ident.to_string();
        let fields: Vec<&syn::Field> = match &node.fields {
            Fields::Named(named) => named.named.iter().collect(),
            Fields::Unnamed(unnamed) => unnamed.unnamed.iter().collect(),
            Fields::Unit => Vec::new(),
        };
        self.within_test_scope(&node.attrs, |visitor| {
            for (position, field) in fields.iter().enumerate() {
                let name = field.ident.as_ref().map_or_else(|| position.to_string(), |ident| ident.to_string());
                let mut item = visitor.spanning(ScannedKind::Field, name, field.span(), field.span(), &field.attrs);
                item.scope = owner.clone();
                visitor.items.push(item);
            }
            syn::visit::visit_item_struct(visitor, node);
        });
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        let item = self.spanning(ScannedKind::Type, node.ident.to_string(), node.ident.span(), node.span(), &node.attrs);
        self.items.push(item);
        self.within_test_scope(&node.attrs, |visitor| {
            for variant in &node.variants {
                let mut item = visitor.spanning(ScannedKind::Variant, variant.ident.to_string(), variant.ident.span(), variant.span(), &variant.attrs);
                item.scope = node.ident.to_string();
                visitor.items.push(item);
            }
            syn::visit::visit_item_enum(visitor, node);
        });
    }

    fn visit_item_union(&mut self, node: &'ast syn::ItemUnion) {
        let item = self.spanning(ScannedKind::Type, node.ident.to_string(), node.ident.span(), node.span(), &node.attrs);
        self.items.push(item);
        self.within_test_scope(&node.attrs, |visitor| syn::visit::visit_item_union(visitor, node));
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        let item = self.spanning(ScannedKind::Trait, node.ident.to_string(), node.ident.span(), node.span(), &node.attrs);
        self.items.push(item);
        self.within_test_scope(&node.attrs, |visitor| {
            visitor.enclosing.push(Enclosing { name: node.ident.to_string(), trait_name: None });
            syn::visit::visit_item_trait(visitor, node);
            visitor.enclosing.pop();
        });
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        let name = Self::type_name(&node.self_ty);
        let trait_name = node.trait_.as_ref().and_then(|(_, path, _)| path.segments.last().map(|s| s.ident.to_string()));
        let mut item = self.spanning(ScannedKind::Impl, name.clone(), node.impl_token.span, node.span(), &node.attrs);
        item.trait_name = trait_name.clone();
        self.items.push(item);
        self.within_test_scope(&node.attrs, |visitor| {
            visitor.enclosing.push(Enclosing { name, trait_name });
            syn::visit::visit_item_impl(visitor, node);
            visitor.enclosing.pop();
        });
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.function_item(&node.sig, node.block.span(), &node.attrs, ScannedKind::Method);
        self.within_test_scope(&node.attrs, |visitor| visitor.visit_function_body(|inner| syn::visit::visit_impl_item_fn(inner, node)));
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        let end = node.default.as_ref().map_or_else(|| node.sig.span(), |block| block.span());
        self.function_item(&node.sig, end, &node.attrs, ScannedKind::Method);
        self.within_test_scope(&node.attrs, |visitor| visitor.visit_function_body(|inner| syn::visit::visit_trait_item_fn(inner, node)));
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let kind = if self.function_depth > 0 { ScannedKind::NestedFunction } else { ScannedKind::FreeFunction };
        self.function_item(&node.sig, node.block.span(), &node.attrs, kind);
        let saved = std::mem::take(&mut self.enclosing);
        self.within_test_scope(&node.attrs, |visitor| visitor.visit_function_body(|inner| syn::visit::visit_item_fn(inner, node)));
        self.enclosing = saved;
    }

    fn visit_attribute(&mut self, node: &'ast Attribute) {
        let Meta::List(list) = &node.meta else { return };
        let Some(level) = LINT_LEVELS.iter().find(|level| list.path.is_ident(level)) else { return };
        let name = format!("{level}({})", list.tokens);
        let mut item = self.spanning(ScannedKind::LintAttribute, name, node.span(), node.span(), &[]);
        item.detail = if matches!(node.style, syn::AttrStyle::Inner(_)) { "inner" } else { "outer" }.to_string();
        self.items.push(item);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let kind = match node.method.to_string().as_str() {
            "unwrap" => Some(ScannedKind::UnwrapCall),
            "expect" => Some(ScannedKind::ExpectCall),
            _ => None,
        };
        if let Some(kind) = kind {
            let item = self.spanning(kind, node.method.to_string(), node.method.span(), node.method.span(), &[]);
            self.items.push(item);
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_closure(&mut self, node: &'ast syn::ExprClosure) {
        let item = self.spanning(ScannedKind::Closure, "closure".to_string(), node.span(), node.span(), &node.attrs);
        if item.length >= LONG_CLOSURE_LINES {
            self.items.push(item);
        }
        syn::visit::visit_expr_closure(self, node);
    }
}
