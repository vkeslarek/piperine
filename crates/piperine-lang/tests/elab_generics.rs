//! Scope: the elaborator's generic instances (generics GN-01, GN-02, GN-07, part I §3.8): the
//! elaborator reads the type table only through the substitution of the instance being lowered, a
//! generic function is instantiated once for each list of arguments, an instantiation that makes a
//! `where`, a length or a constant invalid is reported naming the item and its arguments, and the
//! POM keeps the generic item as written.

use std::path::{Path, PathBuf};

#[path = "support/elab.rs"]
#[allow(dead_code)]
mod support;

use support::Elab;

/// The source files of `piperine_lang::elab`.
struct ElabSources;

impl ElabSources {
    /// Every `.rs` file below `src/elab`, with its path relative to the crate.
    fn files() -> Vec<(PathBuf, String)> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/elab");
        let mut found = Vec::new();
        Self::collect(&root, &mut found);
        found.sort();
        found.into_iter().map(|path| (path.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap_or(&path).to_path_buf(), std::fs::read_to_string(&path).unwrap_or_default())).collect()
    }

    /// Appends the `.rs` files below `directory` to `found`.
    fn collect(directory: &Path, found: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(directory).expect("src/elab is readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                Self::collect(&path, found);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
    }
}

#[test]
fn the_elaborator_reads_types_only_through_the_substitution() {
    // GN-01 AC 1, C47: no `Ty::Param` reaches the IR because the elaborator reads the type table of an expression, a
    // binding and a written type only through `typed`, `typed_local` and `typed_written`, which apply the substitution
    // of the instance being lowered; the three raw reads live in `ConstEval` (`elab/consts.rs`) and nowhere else
    let mut direct: Vec<String> = Vec::new();
    for (path, text) in ElabSources::files() {
        for (number, line) in text.lines().enumerate() {
            let reads = [".types.ty(", ".types.local(", ".types.written("].iter().any(|read| line.contains(read));
            if reads && !path.ends_with("consts.rs") {
                direct.push(format!("{}:{}: {}", path.display(), number + 1, line.trim()));
            }
        }
    }
    assert!(direct.is_empty(), "a direct read of the type table outside `ConstEval`:\n{}", direct.join("\n"));
    let consts = ElabSources::files().into_iter().find(|(path, _)| path.ends_with("elab/consts.rs")).map(|(_, text)| text).unwrap_or_default();
    for helper in ["fn typed(", "fn typed_local(", "fn typed_written("] {
        assert!(consts.contains(helper), "ConstEval has `{helper}`");
    }
}

/// A design whose `analog` block calls the generic functions of `items` with `statement`, and the diagnostics of elaborating it.
struct GenericCall;

impl GenericCall {
    /// The `(code, message)` of each diagnostic of elaborating `Top`, whose `analog` runs `statement`, after `items`.
    fn found(items: &str, statement: &str) -> Vec<(String, String)> {
        let src = format!("{items}\nmodule Top() {{ signal o: Electrical; }}\nanalog for Top {{ {statement} }}\n");
        match Elab::run(&src, "Top") {
            Ok(_) => Vec::new(),
            Err(found) => found.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect(),
        }
    }
}

#[test]
fn a_false_where_is_e0127_naming_the_instantiation() {
    // §3 rule 8, GN-01 AC 4: the `where` is checked for each instantiation; a false one is E0127 naming `clamp::<1>`
    let items = "fn clamp<const N: usize>(x: f64) -> f64 where N > 2 { x }";
    let found = GenericCall::found(items, "v(o) +> clamp::<1>(1.0);");
    assert_eq!(found.iter().map(|(code, _)| code.as_str()).collect::<Vec<_>>(), ["E0127"], "{found:?}");
    assert!(found[0].1.contains("the `where` of `clamp::<1>` is false"), "{}", found[0].1);
    // a true `where` passes the instantiation; lowering the body waits for T34
    let held = GenericCall::found(items, "v(o) +> clamp::<3>(1.0);");
    assert_eq!(held.iter().map(|(code, _)| code.as_str()).collect::<Vec<_>>(), ["P0001"], "{held:?}");
}

#[test]
fn instantiation_errors_name_the_item_and_its_arguments() {
    // GN-07 AC 3: two lengths that differ at an instantiation, and a const expression that fails, are reported at the
    // instantiation naming the item and its arguments
    let items = "fn grown<const N: usize>(a: [f64; N]) -> [f64; N + 1] { a }\nfn shrunk<const N: usize>(a: [f64; N]) -> f64 { let b = [0.0; N - 1]; 1.0 }";
    let lengths = GenericCall::found(items, "let r = grown([1.0, 2.0]);");
    assert!(lengths.iter().any(|(code, message)| code == "E0180" && message.contains("`grown::<2>`") && message.contains("an array of 3 elements is expected, found one of 2")), "{lengths:?}");
    let constant = GenericCall::found(items, "let e: [f64; 0] = []; let r = shrunk(e);");
    assert!(constant.iter().any(|(code, message)| code == "E0104" && message.contains("`shrunk::<0>`")), "{constant:?}");
}
