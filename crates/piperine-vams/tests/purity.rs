//! Mechanical proof that `piperine-vams` stays pure and stays clean of
//! third-party Verilog-A/AMS compiler provenance (design.md R1, C7). Three
//! checks: the manifest declares zero dependencies (in-tree or external),
//! no source file references POM/solver/JIT symbols, and no source file
//! (including comments/doc comments) references a third-party Verilog-A
//! compiler or an external grammar-file format. This crate is written from
//! `VAMS-LRM-2-4.md` Annex A only.

use std::fs;
use std::path::Path;

/// The `[dependencies]` table in `Cargo.toml` must be empty: no lines other
/// than blanks/comments between the `[dependencies]` header and the next
/// `[section]` (or end of file). Matches `piperine-ir`'s purity contract.
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
        "piperine-vams's [dependencies] table must be empty (purity contract), \
         found: {offending:?}"
    );
}

/// Family (a) — architectural leakage: this crate must never see the POM,
/// the solver ABI (`Element`), an MNA/matrix type, or the JIT (Cranelift).
/// Comments are prose and may describe what is banned, so only non-comment
/// lines are scanned (matches `piperine-ir`'s convention).
#[test]
fn sources_reference_no_pom_solver_or_jit_symbol() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let banned = architectural_banned_tokens();

    let mut hits = Vec::new();
    scan_dir(&src_dir, &banned, &mut hits, /* include_comments */ false);

    assert!(
        hits.is_empty(),
        "piperine-vams source files must not reference POM/Element/MNA-matrix/\
         Cranelift symbols (purity contract), found: {hits:?}"
    );
}

/// Family (b) — provenance: no third-party Verilog-A/AMS compiler is
/// consulted, referenced, or borrowed from — not its code, not its grammar
/// files, not its comments (design.md R1, user directive 2026-08-12). This
/// scan covers comments too, unlike family (a): a stray citation in a doc
/// comment is exactly the failure mode this gate exists to catch. The
/// deleted `piperine-ams`/`piperine-parser`/`cvaf` lineage is also banned by
/// name — it is history, not a library, per design.md F3.
#[test]
fn sources_reference_no_third_party_va_compiler() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let banned = provenance_banned_tokens();

    let mut hits = Vec::new();
    scan_dir(&src_dir, &banned, &mut hits, /* include_comments */ true);

    assert!(
        hits.is_empty(),
        "piperine-vams source files must not reference any third-party Verilog-A/AMS \
         compiler, grammar file, or the deleted piperine-ams/cvaf lineage — this crate \
         is written from VAMS-LRM-2-4.md Annex A only (design.md R1), found: {hits:?}"
    );
}

fn architectural_banned_tokens() -> Vec<String> {
    // Built from fragments so this guard file's own vocabulary (which must
    // describe what it bans) never self-trips the scan.
    vec![
        ["Elem", "ent"].concat(),
        ["cranel", "ift"].concat(),
        ["Cranel", "ift"].concat(),
        ["fa", "er"].concat(),
        ["Matr", "ix"].concat(),
        ["pom::", "Design"].concat(),
    ]
}

fn provenance_banned_tokens() -> Vec<String> {
    vec![
        ["Open", "VAF"].concat(),
        ["open", "vaf"].concat(),
        ["un", "gram"].concat(),
        ["Rel", "oaded"].concat(),
        ["cv", "af"].concat(),
        ["piperine", "-ams"].concat(),
        ["piperine", "_ams"].concat(),
        ["piperine", "-parser"].concat(),
        ["piperine", "_parser"].concat(),
    ]
}

fn scan_dir(dir: &Path, banned: &[String], hits: &mut Vec<String>, include_comments: bool) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, banned, hits, include_comments);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let Ok(contents) = fs::read_to_string(&path) else { continue };

        let scanned: String = if include_comments {
            contents.clone()
        } else {
            // Comments (including `//!`/`///` doc comments) are prose, not
            // code — the purity contract itself must be free to *describe*
            // what it bans. Only non-comment lines can violate family (a).
            contents
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        for token in banned {
            if scanned.contains(token.as_str()) {
                hits.push(format!("{}: {token}", path.display()));
            }
        }
    }
}
