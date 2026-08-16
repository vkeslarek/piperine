//! `piperine expand <file>` — developer/designer debug tooling
//! (`.specs/features/preprocessor-span-mapping/spec.md` P2.5,
//! SPANMAP-10/11): prints the fully preprocessed (`` `include ``/
//! `` `define ``-expanded, conditional-compilation-resolved) text a
//! designer or `piperine-lang` developer can read directly — the
//! Verilog-AMS-toolchain equivalent of `cargo expand`. Not part of the
//! compile pipeline itself; reuses `Preprocessor::run` and
//! `SourceProvenance` exactly as `check`/`build` do, adds no new
//! preprocessing logic of its own.

use std::path::{Path, PathBuf};

use thiserror::Error;

use super::check::is_recognized_source;

#[derive(Debug, Error)]
pub enum ExpandError {
    #[error("unrecognized source extension (expected .vams or .va)")]
    UnrecognizedExtension,
    #[error("read failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("preprocessing failed:\n{0:?}")]
    ParseReport(miette::Report),
}

/// Preprocesses `path`'s contents and returns the fully expanded text plus
/// the [`piperine_lang::source_provenance::SourceProvenance`] mapping it
/// back to real origins. Reuses `piperine_lang::Preprocessor` directly —
/// `expand` only needs ch.10 textual substitution, not the full
/// lex/grammar/elaborate pipeline `check`/`build` run.
pub fn expand_file(
    path: &Path,
    source_map: &piperine_lang::SourceMap,
) -> Result<(String, piperine_lang::source_provenance::SourceProvenance), ExpandError> {
    if !is_recognized_source(path) {
        return Err(ExpandError::UnrecognizedExtension);
    }
    let body = std::fs::read_to_string(path)?;
    let mut pp = piperine_lang::Preprocessor::with_include_resolver(
        piperine_lang::SourceMapIncludeResolver::new(source_map),
    );
    pp.run(&body).map_err(|e| {
        // SPANMAP-10 AC3: the same typed error `piperine check` would
        // report for the same underlying failure — reuses
        // `piperine_lang::ElabError`'s existing `From<ParseError>`
        // conversion and miette rendering, not a parallel error path.
        // `check`'s own render boundary (T8) hits this identical
        // raw-input fallback for a preprocessor-stage failure (its
        // second, resolving preprocessor pass would fail the same way,
        // deterministically, landing on the same fallback render).
        let elab_err = piperine_lang::ElabError::from(e);
        ExpandError::ParseReport(miette::Report::from(elab_err).with_source_code(body.clone()))
    })
}

pub fn execute(file: Option<String>) {
    let (source_map, _project_root) = super::utils::build_source_map();

    let Some(file) = file else {
        eprintln!("Error: `piperine expand` requires a file argument.");
        std::process::exit(1);
    };
    let path = PathBuf::from(file);

    match expand_file(&path, &source_map) {
        Ok((text, _prov)) => print!("{text}"),
        Err(ExpandError::ParseReport(report)) => {
            eprintln!("Error in file {}:\n{:?}", path.display(), report);
            std::process::exit(1);
        }
        Err(other) => {
            eprintln!("Error in file {}: {}", path.display(), other);
            std::process::exit(1);
        }
    }
}
