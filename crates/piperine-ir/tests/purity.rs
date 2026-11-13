//! Mechanical proof that `piperine-ir` stays pure (MD-31: a policy
//! invariant lives in the gate, not in inspection). Two checks: the manifest
//! declares zero in-tree dependencies, and no source file references
//! `Element`, an MNA/matrix type, or a Cranelift symbol.
//!

use std::fs;
use std::path::Path;

/// The `[dependencies]` table in `Cargo.toml` must be empty: no lines other
/// than blanks/comments between the `[dependencies]` header and the next
/// `[section]` (or end of file).
#[test]
fn manifest_has_no_dependencies() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path).expect("read Cargo.toml");

    let mut in_deps = false;
    let mut offending = Vec::new();
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed == "[dependencies]";
            continue;
        }
        if in_deps && !trimmed.is_empty() && !trimmed.starts_with('#') {
            offending.push(trimmed.to_string());
        }
    }

    assert!(
        offending.is_empty(),
        "piperine-ir's [dependencies] table must be empty (purity contract), \
         found: {offending:?}"
    );
}

/// Banned tokens: anything that would let the crate see the solver's ABI
/// (`Element`), an MNA/matrix type, or the JIT (Cranelift). Checked against
/// every `.rs` file under `src/` — this test itself lives in `tests/`, and
/// this file's own text must not trip the scan, so the banned words are
/// built at runtime rather than typed as literals here.
#[test]
fn sources_reference_no_solver_or_jit_symbol() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let banned = banned_tokens();

    let mut hits = Vec::new();
    scan_dir(&src_dir, &banned, &mut hits);

    assert!(
        hits.is_empty(),
        "piperine-ir source files must not reference Element/MNA-matrix/Cranelift \
         symbols (purity contract), found: {hits:?}"
    );
}

fn banned_tokens() -> Vec<String> {
    // Built from fragments so this guard file's own vocabulary (which must
    // describe what it bans) never self-trips the scan.
    vec![
        ["Elem", "ent"].concat(),
        ["cranel", "ift"].concat(),
        ["Cranel", "ift"].concat(),
        ["fa", "er"].concat(),
        ["Matr", "ix"].concat(),
    ]
}

fn scan_dir(dir: &Path, banned: &[String], hits: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, banned, hits);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let Ok(contents) = fs::read_to_string(&path) else { continue };
        // Comments (including `//!`/`///` doc comments) are prose, not code
        // — the purity contract itself must be free to *describe* what it
        // bans (`//! … never sees an Element …`). Only non-comment lines can
        // actually violate purity.
        let code_only: String = contents
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for token in banned {
            if code_only.contains(token.as_str()) {
                hits.push(format!("{}: {token}", path.display()));
            }
        }
    }
}
