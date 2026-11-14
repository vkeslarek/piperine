//! Scope: reading a package manifest, `Piperine.toml` (feature `msl-packages`, PKG-10): the
//! unreadable manifest (`E0170`) and the rendered `PackageError`; the `[package]` table and the
//! `[dependencies]` table are added by the tasks that follow.

use std::fs;

use piperine_lang::msl::{Diagnostic, Span};
use piperine_project::{Manifest, PackageError};

/// A package directory holding exactly these bytes as its manifest.
fn dir_with(manifest: &[u8]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Piperine.toml"), manifest).unwrap();
    dir
}

fn only(errors: Result<Manifest, Vec<PackageError>>) -> PackageError {
    let mut errors = errors.expect_err("the manifest was accepted");
    assert_eq!(errors.len(), 1, "{errors:?}");
    errors.remove(0)
}

#[test]
fn a_missing_manifest_is_e0170() {
    let dir = tempfile::tempdir().unwrap();
    let error = only(Manifest::load(dir.path()));
    assert_eq!(error.code(), "E0170");
    let path = dir.path().join("Piperine.toml");
    assert!(error.diagnostic.message.contains(&path.display().to_string()), "{}", error.diagnostic.message);
    assert_eq!(error.file, path);
    assert_eq!(error.text, "");
    assert_eq!(error.diagnostic.span(), Span::new(0, 0));
}

#[test]
fn invalid_toml_is_e0170_at_its_span() {
    let text = "# the manifest\n[package\nname = \"a\"\n";
    let line = 15..24; // `[package\n`
    let error = only(Manifest::parse(text));
    assert_eq!(error.code(), "E0170");
    assert_eq!(error.text, text);
    let span = error.diagnostic.span();
    assert!(span.start >= line.start && span.end <= line.end && !span.is_empty(), "{span:?} is not on the `[package` line");
    // The same text read from disk.
    let dir = dir_with(text.as_bytes());
    let error = only(Manifest::load(dir.path()));
    assert_eq!((error.code(), error.diagnostic.span()), ("E0170", span));
    // Not UTF-8: the span is the offending byte, `\u{FFFD}` in the lossy text the error holds.
    let mut bytes = b"[package]\nname = \"a".to_vec();
    let at = bytes.len();
    bytes.extend([0xFF, b'"', b'\n']);
    let dir = dir_with(&bytes);
    let error = only(Manifest::load(dir.path()));
    assert_eq!(error.code(), "E0170");
    assert_eq!(error.diagnostic.span().start, at);
    assert_eq!(&error.text[error.diagnostic.span().range()], "\u{FFFD}");
    assert!(error.diagnostic.message.contains("UTF-8"), "{}", error.diagnostic.message);
}

#[test]
fn a_package_error_renders_as_a_miette_report() {
    let text = "[package]\nname = \"a\"\nplugins = 1\n";
    let start = text.find("plugins").unwrap();
    let diagnostic = Diagnostic::new("E0171", "unknown key `plugins`", Span::new(start, start + "plugins".len()), "not a key of `[package]`").with_help("keys: name, version");
    let error = PackageError { file: "pkg/Piperine.toml".into(), text: text.to_string(), diagnostic };
    assert_eq!(error.code(), "E0171");
    let out = error.render();
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.iter().any(|l| l.trim() == "E0171"), "code line in\n{out}");
    assert!(out.contains("× unknown key `plugins`"), "message in\n{out}");
    // The file name, not its directory, then line and column of the span start.
    assert!(out.contains("[Piperine.toml:3:1]"), "location in\n{out}");
    let at = lines.iter().position(|l| l.contains('│') && l.ends_with("plugins = 1")).unwrap_or_else(|| panic!("source line in\n{out}"));
    let underline: Vec<char> = lines[at + 1].chars().collect();
    let marks = |c: &char| matches!(c, '─' | '┬' | '┴' | '┼');
    let first = underline.iter().position(marks).unwrap_or_else(|| panic!("underline in\n{out}"));
    assert_eq!(first, lines[at].chars().count() - "plugins = 1".chars().count(), "the underline starts at the key:\n{out}");
    assert_eq!(underline[first..].iter().take_while(|c| marks(c)).count(), "plugins".len(), "the underline is the key's length:\n{out}");
    assert!(lines[at + 2].contains("not a key of `[package]`"), "label in\n{out}");
    assert!(lines.iter().any(|l| l.trim() == "help: keys: name, version"), "help in\n{out}");
    // An error the reader itself made renders the same way.
    let error = only(Manifest::parse("[package\n"));
    let out = error.render();
    assert!(out.contains("E0170") && out.contains("[Piperine.toml:1:"), "{out}");
}
