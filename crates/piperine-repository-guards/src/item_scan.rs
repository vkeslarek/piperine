//! The scan: one `ScannedItem` for each thing of the repository's Rust sources a guard may judge.

use std::path::PathBuf;

use crate::scan_visitor::ScanVisitor;
use crate::source_tree::SourceTree;

/// What a scanned item is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemKind {
    /// A source file.
    File,
    /// A `mod`, inline or in its own file.
    Module,
    /// A `struct`, `enum` or `union`.
    Type,
    /// A named or positional field of a `struct`.
    Field,
    /// A variant of an `enum`.
    Variant,
    /// A `trait` declaration.
    Trait,
    /// An `impl` block, inherent or of a trait.
    Impl,
    /// A function inside an `impl` or a `trait`.
    Method,
    /// A function at module level, outside any `impl`, `trait` or function body.
    FreeFunction,
    /// A function written inside another function's body.
    NestedFunction,
    /// A closure of 15 lines or more, counted as a function.
    Closure,
    /// A `#[allow]`, `#[expect]`, `#[warn]`, `#[deny]` or `#[forbid]` attribute, inner or outer.
    LintAttribute,
    /// A call of a method named `unwrap`.
    UnwrapCall,
    /// A call of a method named `expect`.
    ExpectCall,
}

/// Whether an item is part of the product or of a test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Code that ships.
    Product,
    /// A file under `tests/`, a `#[cfg(test)]` module or item, a file such a module declaration
    /// reaches, or a `#[test]` function.
    Test,
}

/// Why a free function is allowed to exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreeFunctionPermit {
    /// It is `main`.
    Main,
    /// It has an ABI, such as `extern "C"`.
    ForeignAbi,
    /// It carries `#[test]`.
    TestAttribute,
}

/// One scanned item, with where it is and how big it is.
#[derive(Clone, Debug)]
pub struct ScannedItem {
    /// What the item is.
    pub kind: ItemKind,
    /// Its name (a file's path, an `impl`'s self type, a lint attribute's `allow(dead_code)`).
    pub name: String,
    /// The inline modules, joined by `::`, or the `impl` or `trait` the item sits in.
    pub scope: String,
    /// The crate directory the item's file sits in.
    pub crate_name: String,
    /// The file, relative to the repository root.
    pub file: PathBuf,
    /// The line the item starts on (a function's signature, a type's name).
    pub line: usize,
    /// How many lines the item spans, counted from `line`.
    pub length: usize,
    /// Whether the item has a doc comment (a file: a `//!` header).
    pub documented: bool,
    /// Product or test code.
    pub origin: Origin,
    /// For an `impl`, and for each method in it: the trait it implements, when it does.
    pub trait_name: Option<String>,
    /// For a free function: the reason it is allowed, when it is.
    pub permit: Option<FreeFunctionPermit>,
    /// For a lint attribute: `inner` for `#![..]`, `outer` for `#[..]`.
    pub detail: String,
}

impl ScannedItem {
    /// Whether the item is a free function with no reason to exist as one.
    pub fn is_unpermitted_free_function(&self) -> bool {
        self.kind == ItemKind::FreeFunction && self.permit.is_none()
    }

    /// Whether the rules of doc comments leave the item out: a method of a trait `impl` is
    /// documented by the trait.
    pub fn is_documentation_exempt(&self) -> bool {
        self.kind == ItemKind::Method && self.trait_name.is_some()
    }
}

/// Every scanned item of a source tree.
pub struct ItemScan {
    items: Vec<ScannedItem>,
    unparsed: Vec<PathBuf>,
}

impl ItemScan {
    /// Scans every file of the tree; a file that does not parse is listed in `unparsed`.
    pub fn of(tree: &SourceTree) -> ItemScan {
        let mut items = Vec::new();
        let mut unparsed = Vec::new();
        for file in tree.files() {
            match file.syntax() {
                Ok(syntax) => ScanVisitor::scan_file(file, syntax, &mut items),
                Err(_) => unparsed.push(file.path().to_path_buf()),
            }
        }
        ItemScan { items, unparsed }
    }

    /// Every scanned item, file by file, in source order.
    pub fn items(&self) -> &[ScannedItem] {
        &self.items
    }

    /// The items of one kind.
    pub fn of_kind(&self, kind: ItemKind) -> impl Iterator<Item = &ScannedItem> {
        self.items.iter().filter(move |item| item.kind == kind)
    }

    /// The files that did not parse.
    pub fn unparsed(&self) -> &[PathBuf] {
        &self.unparsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds scans over small sources for the tests below.
    struct Scenario;

    impl Scenario {
        /// The scan of one library file at `crates/demo/src/lib.rs`.
        fn scan(source: &str) -> ItemScan {
            Self::scan_files(&[("crates/demo/src/lib.rs", source)])
        }

        /// The scan of several files, paths relative to the repository root.
        fn scan_files(sources: &[(&str, &str)]) -> ItemScan {
            ItemScan::of(&SourceTree::of_sources(sources))
        }

        /// The item of `kind` called `name`.
        fn find<'a>(scan: &'a ItemScan, kind: ItemKind, name: &str) -> &'a ScannedItem {
            scan.of_kind(kind).find(|item| item.name == name).unwrap_or_else(|| panic!("no {kind:?} named {name}"))
        }
    }

    #[test]
    fn a_free_function_and_a_nested_function_are_told_apart() {
        let scan = Scenario::scan("fn outer() {\n    fn inner() {}\n}\n");
        assert_eq!(Scenario::find(&scan, ItemKind::FreeFunction, "outer").permit, None);
        assert!(Scenario::find(&scan, ItemKind::FreeFunction, "outer").is_unpermitted_free_function());
        assert_eq!(Scenario::find(&scan, ItemKind::NestedFunction, "inner").kind, ItemKind::NestedFunction);
        assert_eq!(scan.of_kind(ItemKind::FreeFunction).count(), 1);
    }

    #[test]
    fn main_an_extern_function_and_a_test_are_permitted() {
        let scan = Scenario::scan("fn main() {}\nextern \"C\" fn hook() {}\n#[test]\nfn works() {}\n");
        assert_eq!(Scenario::find(&scan, ItemKind::FreeFunction, "main").permit, Some(FreeFunctionPermit::Main));
        assert_eq!(Scenario::find(&scan, ItemKind::FreeFunction, "hook").permit, Some(FreeFunctionPermit::ForeignAbi));
        assert_eq!(Scenario::find(&scan, ItemKind::FreeFunction, "works").permit, Some(FreeFunctionPermit::TestAttribute));
        assert_eq!(Scenario::find(&scan, ItemKind::FreeFunction, "works").origin, Origin::Test);
    }

    #[test]
    fn a_method_of_a_trait_impl_is_exempt_and_a_method_of_an_inherent_impl_is_not() {
        let scan = Scenario::scan("struct S;\nimpl S {\n    fn own(&self) {}\n}\nimpl Clone for S {\n    fn clone(&self) -> S { S }\n}\n");
        let own = Scenario::find(&scan, ItemKind::Method, "own");
        assert_eq!((own.scope.as_str(), own.trait_name.clone(), own.is_documentation_exempt()), ("S", None, false));
        let clone = Scenario::find(&scan, ItemKind::Method, "clone");
        assert_eq!((clone.trait_name.as_deref(), clone.is_documentation_exempt()), (Some("Clone"), true));
        assert_eq!(scan.of_kind(ItemKind::FreeFunction).count(), 0);
    }

    #[test]
    fn an_inline_cfg_test_module_marks_what_it_holds_as_test_code() {
        let scan = Scenario::scan("fn product() {}\n#[cfg(test)]\nmod tests {\n    fn helper() {}\n}\n");
        assert_eq!(Scenario::find(&scan, ItemKind::FreeFunction, "product").origin, Origin::Product);
        assert_eq!(Scenario::find(&scan, ItemKind::FreeFunction, "helper").origin, Origin::Test);
        assert_eq!(Scenario::find(&scan, ItemKind::FreeFunction, "helper").scope, "tests");
    }

    #[test]
    fn a_cfg_test_module_declaration_marks_the_file_it_reaches() {
        let scan = Scenario::scan_files(&[
            ("crates/demo/src/lib.rs", "#[cfg(test)]\nmod support;\nmod real;\n"),
            ("crates/demo/src/support.rs", "fn shared() {}\n"),
            ("crates/demo/src/real.rs", "fn shipped() {}\n"),
        ]);
        assert_eq!(Scenario::find(&scan, ItemKind::FreeFunction, "shared").origin, Origin::Test);
        assert_eq!(Scenario::find(&scan, ItemKind::FreeFunction, "shipped").origin, Origin::Product);
    }

    #[test]
    fn a_path_include_reaches_the_file_it_names_across_crates() {
        let scan = Scenario::scan_files(&[
            ("crates/demo/src/lib.rs", "#[cfg(test)]\n#[path = \"../../other/tests/support.rs\"]\nmod support;\n"),
            ("crates/other/tests/support.rs", "mod nested;\nfn shared() {}\n"),
            ("crates/other/tests/support/nested.rs", "fn deeper() {}\n"),
            ("crates/demo/src/included.rs", "fn plain() {}\n"),
        ]);
        assert_eq!(Scenario::find(&scan, ItemKind::FreeFunction, "shared").origin, Origin::Test);
        assert_eq!(Scenario::find(&scan, ItemKind::FreeFunction, "deeper").origin, Origin::Test);
        let included = Scenario::scan_files(&[
            ("crates/demo/src/lib.rs", "#[path = \"../../other/src/shared.rs\"]\nmod shared;\n"),
            ("crates/other/src/shared.rs", "fn product_include() {}\n"),
        ]);
        assert_eq!(Scenario::find(&included, ItemKind::FreeFunction, "product_include").origin, Origin::Product);
        assert_eq!(Scenario::find(&scan, ItemKind::FreeFunction, "plain").origin, Origin::Product);
    }

    #[test]
    fn a_closure_counts_only_from_fifteen_lines() {
        let short = "fn f() {\n    let c = |x: i32| {\n        x + 1\n    };\n}\n";
        assert_eq!(Scenario::scan(short).of_kind(ItemKind::Closure).count(), 0);
        let body: String = (0..13).map(|i| format!("        let v{i} = x;\n")).collect();
        let long = format!("fn f() {{\n    let c = |x: i32| {{\n{body}        x\n    }};\n}}\n");
        let scan = Scenario::scan(&long);
        let closure = scan.of_kind(ItemKind::Closure).next().expect("a closure of 16 lines is counted");
        assert_eq!((closure.line, closure.length), (2, 16));
    }

    #[test]
    fn a_function_length_runs_from_its_signature_to_its_closing_brace() {
        let scan = Scenario::scan("/// Doc.\n#[inline]\nfn three(\n) {\n    ()\n}\n");
        let function = Scenario::find(&scan, ItemKind::FreeFunction, "three");
        assert_eq!((function.line, function.length, function.documented), (3, 4, true));
    }

    #[test]
    fn documented_and_undocumented_items_are_told_apart() {
        let scan = Scenario::scan("//! The file.\n/// A type.\nstruct A {\n    /// A field.\n    x: i32,\n    y: i32,\n}\nenum B {\n    /// First.\n    One,\n    Two,\n}\n");
        assert!(Scenario::find(&scan, ItemKind::File, "crates/demo/src/lib.rs").documented);
        assert!(Scenario::find(&scan, ItemKind::Type, "A").documented);
        assert!(!Scenario::find(&scan, ItemKind::Type, "B").documented);
        assert!(Scenario::find(&scan, ItemKind::Field, "x").documented);
        assert!(!Scenario::find(&scan, ItemKind::Field, "y").documented);
        assert!(Scenario::find(&scan, ItemKind::Variant, "One").documented);
        assert!(!Scenario::find(&scan, ItemKind::Variant, "Two").documented);
    }

    #[test]
    fn lint_attributes_unwrap_and_expect_are_listed() {
        let scan = Scenario::scan("#![deny(missing_docs)]\n#[allow(dead_code)]\nfn f(o: Option<i32>) -> i32 {\n    o.unwrap() + o.expect(\"x\")\n}\n");
        let inner = Scenario::find(&scan, ItemKind::LintAttribute, "deny(missing_docs)");
        assert_eq!(inner.detail, "inner");
        assert_eq!(Scenario::find(&scan, ItemKind::LintAttribute, "allow(dead_code)").detail, "outer");
        assert_eq!(scan.of_kind(ItemKind::UnwrapCall).count(), 1);
        assert_eq!(scan.of_kind(ItemKind::ExpectCall).count(), 1);
    }

    #[test]
    fn a_file_that_does_not_parse_is_listed_not_hidden() {
        let scan = Scenario::scan("fn (");
        assert_eq!(scan.unparsed().len(), 1);
    }

    #[test]
    fn the_workspace_scan_reaches_this_file_and_parses_every_source() {
        let scan = ItemScan::of(&SourceTree::of_workspace());
        let this_file = Scenario::find(&scan, ItemKind::File, "crates/piperine-repository-guards/src/item_scan.rs");
        assert_eq!((this_file.crate_name.as_str(), this_file.documented), ("piperine-repository-guards", true));
        assert_eq!(scan.unparsed(), &[] as &[PathBuf], "every .rs file of the tree parses");
    }
}
