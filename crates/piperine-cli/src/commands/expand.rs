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

pub fn execute(file: Option<String>, trace: bool) {
    let (source_map, _project_root) = super::utils::build_source_map();

    let Some(file) = file else {
        eprintln!("Error: `piperine expand` requires a file argument.");
        std::process::exit(1);
    };
    let path = PathBuf::from(file);

    match expand_file(&path, &source_map) {
        Ok((text, prov)) => {
            if trace {
                print!("{}", render_trace(&text, &prov, &path.display().to_string()));
            } else {
                print!("{text}");
            }
        }
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

/// `--trace` mode (T15, SPANMAP-11): walks `prov`'s segments in buffer
/// order (`SourceProvenance::segments_in_order`); at the *start* of each
/// contiguous run of segments sharing the same (non-empty) expansion
/// chain — a macro's own substituted body is recorded as one micro-segment
/// per pushed byte range, T6/T9's paired-recording granularity, so a run
/// of many segments shares one chain and must produce exactly one
/// annotation, not one per micro-segment — emits an annotation line: the
/// macro's name, its `param=value` argument pairs (read from
/// `ExpansionFrame::args` via `SourceProvenance::text()`), and the
/// invocation's real file:line, before the expanded text it produced. A
/// plain/`` `include ``-only run gets no annotation (SPANMAP-09's "no
/// noise on the common case" convention, same as T9's `ExpansionFrame`
/// recording).
///
/// `top_name` is the real on-disk path of the file being expanded —
/// substituted in place of `SourceProvenance`'s synthetic `"<input>"`
/// top-level source name (design.md's documented placeholder for a
/// caller that only ever hands the preprocessor a raw string, never a
/// real path) so the invocation location `--trace` prints is actually
/// useful to a developer reading it, not an opaque placeholder.
pub fn render_trace(text: &str, prov: &piperine_lang::source_provenance::SourceProvenance, top_name: &str) -> String {
    let mut out = String::new();
    let mut last_chain: Option<&[piperine_lang::source_provenance::ExpansionFrame]> = None;
    for (buffer_range, chain) in prov.segments_in_order() {
        let start = buffer_range.start.min(text.len());
        let end = buffer_range.end.min(text.len());
        let is_new_run = !chain.is_empty() && last_chain != Some(chain);
        if is_new_run {
            // Innermost frame (index 0, per T9's "innermost first" chain
            // ordering) is the one that directly produced this run's text.
            let frame = &chain[0];
            let args: Vec<String> = frame
                .args
                .iter()
                .map(|(name, span)| {
                    let value = &prov.text(span.source_id)[span.start..span.end];
                    format!("{name}={value}")
                })
                .collect();
            let inv_text = prov.text(frame.invocation.source_id);
            let inv_start = frame.invocation.start.min(inv_text.len());
            let line = 1 + inv_text[..inv_start].matches('\n').count();
            let inv_name = prov.name(frame.invocation.source_id);
            let display_name = if inv_name == "<input>" { top_name } else { inv_name };
            out.push_str(&format!(
                "// expanded from macro `{}`({}) at {}:{}\n",
                frame.macro_name,
                args.join(", "),
                display_name,
                line,
            ));
        }
        last_chain = Some(chain);
        out.push_str(&text[start..end]);
    }
    out
}
