//! `piperine expand` on a real Verilog-AMS fixture: the command prints the
//! fully preprocessed text (SPANMAP-10) and reuses the same typed error
//! path `check` would for an unresolvable target
//! (`.specs/features/preprocessor-span-mapping/tasks.md` T14).
//!
//! Exercises the `expand` command's front-door wiring (file detection +
//! `Preprocessor::run` + error rendering), matching `check_cmd.rs`'s
//! convention (call the command's own `_file`-suffixed function directly).

use piperine_cli::commands::expand::{expand_file, ExpandError};
use std::io::Write;
use std::path::PathBuf;

fn tmp_ams(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    let mut f = std::fs::File::create(&p).expect("create");
    f.write_all(body.as_bytes()).expect("write");
    p
}

/// SPANMAP-10 AC4: a file with no directives at all expands to output
/// byte-identical to the input file.
#[test]
fn expand_with_no_directives_is_byte_identical_to_input() {
    let dir = tempfile::tempdir().expect("tempdir");
    let body = "module R (inout electrical p, n);\nendmodule\n";
    let p = tmp_ams(dir.path(), "plain.vams", body);

    let (text, _prov) = expand_file(&p, &piperine_lang::SourceMap::dummy()).expect("expand_file ok");
    assert_eq!(text, body, "no directives present, output must be byte-identical to the input file");
}

/// SPANMAP-10 AC1: a file with `` `include ``/`` `define ``s prints the
/// fully resolved/expanded text — the included file's own content spliced
/// in, and the macro invocation replaced by its substituted body.
#[test]
fn expand_resolves_includes_and_macro_invocations() {
    let dir = tempfile::tempdir().expect("tempdir");
    tmp_ams(dir.path(), "defs.vams", "parameter real included_default = 42.0;\n");
    let top_body = "`define WIDTH 8\n`include \"defs.vams\"\nmodule R;\n  parameter integer w = `WIDTH;\nendmodule\n";
    let p = tmp_ams(dir.path(), "top.vams", top_body);

    let source_map = piperine_lang::SourceMap::new(dir.path().to_path_buf());
    let (text, _prov) = expand_file(&p, &source_map).expect("expand_file ok");

    assert!(text.contains("included_default"), "the `include`d file's own content must appear in the expanded text, got:\n{text}");
    assert!(text.contains("w = 8"), "the macro invocation must be replaced by its substituted body, got:\n{text}");
    assert!(!text.contains("`WIDTH"), "the raw macro invocation text must not survive expansion, got:\n{text}");
    assert!(!text.contains("`include"), "the raw `include` directive text must not survive expansion, got:\n{text}");
}

/// SPANMAP-10 AC3: an unresolvable `` `include `` reports the same typed
/// error `piperine check` would (reusing `ElabError::from(ParseError)`),
/// not a parallel error path.
#[test]
fn expand_on_unresolvable_include_reports_the_same_typed_error_as_check() {
    let dir = tempfile::tempdir().expect("tempdir");
    let top_body = "`include \"does_not_exist.vams\"\n";
    let p = tmp_ams(dir.path(), "top.vams", top_body);

    let source_map = piperine_lang::SourceMap::new(dir.path().to_path_buf());
    let err = expand_file(&p, &source_map).expect_err("unresolvable include must fail");
    match err {
        ExpandError::ParseReport(report) => {
            let rendered = format!("{report:?}");
            assert!(
                rendered.contains("does_not_exist.vams"),
                "rendered report should name the unresolved include path, got:\n{rendered}"
            );
            assert!(
                rendered.contains("could not be resolved") || rendered.contains("E1"),
                "rendered report should carry the same typed IncludeNotFound diagnostic `check` would show, got:\n{rendered}"
            );
        }
        other => panic!("expected ExpandError::ParseReport, got: {other:?}"),
    }
}

/// Unrecognized extensions are rejected the same way `check` rejects them.
#[test]
fn expand_rejects_unrecognized_extension() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = tmp_ams(dir.path(), "not_vams.txt", "module R; endmodule\n");
    let err = expand_file(&p, &piperine_lang::SourceMap::dummy()).expect_err("must reject .txt");
    assert!(matches!(err, ExpandError::UnrecognizedExtension));
}
