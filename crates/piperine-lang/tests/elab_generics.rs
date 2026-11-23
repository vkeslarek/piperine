//! Scope: the elaborator's generic instances (generics GN-01, GN-02, GN-07, part I §3.8): the
//! elaborator reads the type table only through the substitution of the instance being lowered, a
//! generic function is instantiated once for each list of arguments, an instantiation that makes a
//! `where`, a length or a constant invalid is reported naming the item and its arguments, and the
//! POM keeps the generic item as written.

use std::path::{Path, PathBuf};

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
