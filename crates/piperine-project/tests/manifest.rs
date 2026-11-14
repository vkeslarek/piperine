//! Scope: reading a package manifest, `Piperine.toml` (feature `msl-packages`, PKG-10): the
//! unreadable manifest (`E0170`) and the rendered `PackageError`; the `[package]` table and the
//! `[dependencies]` table are added by the tasks that follow.

use std::fs;

use piperine_lang::msl::{Diagnostic, Span};
use piperine_project::{Manifest, PackageError, Version};

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

// ---- the `[package]` table (T17) ----

/// The errors of a manifest text, which must not be accepted.
fn faults(text: &str) -> Vec<PackageError> {
    Manifest::parse(text).expect_err(&format!("accepted:\n{text}"))
}

/// The single `E0171` of a manifest text and the text it points at.
fn one_e0171(text: &str) -> (PackageError, String) {
    let mut errors = faults(text);
    assert_eq!(errors.len(), 1, "{text:?}: {errors:?}");
    let error = errors.remove(0);
    assert_eq!(error.code(), "E0171", "{text:?}: {error}");
    assert_eq!(error.text, text);
    let at = text[error.diagnostic.span().range()].to_string();
    (error, at)
}

#[test]
fn a_valid_manifest_reads_every_package_field() {
    let text = "[package]\nname = \"app\"\nversion = \"1.20.3\"\nauthors = [\"Ada <ada@example.org>\", \"Bob\"]\ndescription = \"An app.\"\n";
    let manifest = Manifest::parse(text).unwrap();
    let package = &manifest.package;
    assert_eq!(package.name, "app");
    assert_eq!(package.version, Version { major: 1, minor: 20, patch: 3 });
    assert_eq!(package.authors, ["Ada <ada@example.org>", "Bob"]);
    assert_eq!(package.description, "An app.");
    // The optional keys default to empty.
    let minimal = Manifest::parse("[package]\nname = \"_x9\"\nversion = \"0.0.0\"\n").unwrap();
    assert_eq!(minimal.package.name, "_x9");
    assert_eq!(minimal.package.version, Version { major: 0, minor: 0, patch: 0 });
    assert!(minimal.package.authors.is_empty());
    assert_eq!(minimal.package.description, "");
    // Read from disk the same.
    let dir = dir_with(text.as_bytes());
    assert_eq!(Manifest::load(dir.path()).unwrap(), manifest);
    // `[dependencies]` is accepted here and read by the next task.
    Manifest::parse("[package]\nname = \"a\"\nversion = \"1.0.0\"\n[dependencies]\nutil = { path = \"../util\" }\n").unwrap();
}

#[test]
fn an_unknown_key_is_e0171_at_the_key() {
    let package = "[package]\nname = \"a\"\nversion = \"1.0.0\"\n";
    // A table the manifest does not have (the retired `[plugins]` and `[project]`) and a stray top-level key,
    // written first so that it is not a key of `[package]`.
    for (extra, key) in [("[plugins]\nx = 1\n", "plugins"), ("[project]\nname = \"a\"\n", "project"), ("edition = \"2024\"\n", "edition")] {
        let text = format!("{extra}{package}");
        let (error, at) = one_e0171(&text);
        assert_eq!(at, key, "{text}");
        assert!(error.diagnostic.message.contains(&format!("`{key}`")), "{}", error.diagnostic.message);
        let help = error.diagnostic.help.as_deref().unwrap_or_else(|| panic!("{key}: no help"));
        assert!(help.contains("package") && help.contains("dependencies"), "{help}");
    }
    // A key `[package]` does not have.
    let text = "[package]\nname = \"a\"\nversion = \"1.0.0\"\nedition = \"2024\"\n";
    let (error, at) = one_e0171(text);
    assert_eq!(at, "edition");
    assert!(error.diagnostic.message.contains("`edition`") && error.diagnostic.message.contains("package"), "{}", error.diagnostic.message);
    let help = error.diagnostic.help.as_deref().unwrap();
    for allowed in ["name", "version", "authors", "description"] {
        assert!(help.contains(allowed), "{help}");
    }
    // The old table alone is its own fault and the missing `[package]` is another.
    let errors = faults("[project]\nname = \"a\"\n");
    assert_eq!(errors.iter().map(|e| e.code()).collect::<Vec<_>>(), ["E0171", "E0171"]);
}

#[test]
fn a_missing_or_mistyped_package_key_is_e0171() {
    let missing = |text: &str, key: &str| {
        let (error, _) = one_e0171(text);
        assert!(error.diagnostic.message.contains(&format!("`{key}`")), "{text:?}: {}", error.diagnostic.message);
    };
    missing("", "package");
    missing("[dependencies]\n", "package");
    missing("[package]\nversion = \"1.0.0\"\n", "name");
    missing("[package]\nname = \"a\"\n", "version");
    // A wrong type is one fault at the value, not also a missing key.
    for (text, key, at) in [
        ("[package]\nname = 3\nversion = \"1.0.0\"\n", "name", "3"),
        ("[package]\nname = \"a\"\nversion = 1.0\n", "version", "1.0"),
        ("[package]\nname = \"a\"\nversion = \"1.0.0\"\nauthors = \"Ada\"\n", "authors", "\"Ada\""),
        ("[package]\nname = \"a\"\nversion = \"1.0.0\"\nauthors = [\"Ada\", 7]\n", "authors", "7"),
        ("[package]\nname = \"a\"\nversion = \"1.0.0\"\ndescription = true\n", "description", "true"),
        ("package = 3\n", "package", "3"),
    ] {
        let (error, found) = one_e0171(text);
        assert_eq!(found, at, "{text:?}");
        assert!(error.diagnostic.message.contains(&format!("`{key}`")), "{text:?}: {}", error.diagnostic.message);
    }
}

#[test]
fn a_bad_package_name_is_e0171() {
    for name in ["my-pkg", "9lives", "module", "fn", "std", ""] {
        let text = format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n");
        let (error, at) = one_e0171(&text);
        assert_eq!(at, format!("\"{name}\""), "the error is at the string");
        assert!(error.diagnostic.message.contains("package name"), "{}", error.diagnostic.message);
    }
}

#[test]
fn a_bad_version_is_e0171() {
    for version in ["1.0", "1.0.0-rc1", "v1.0.0", "1.0.x", "1", "1.0.0.0", "1..0", "+1.0.0", "-1.0.0", " 1.0.0", "99999999999999999999.0.0", ""] {
        let text = format!("[package]\nname = \"a\"\nversion = \"{version}\"\n");
        let (error, at) = one_e0171(&text);
        assert_eq!(at, format!("\"{version}\""), "the error is at the string");
        assert!(error.diagnostic.message.contains("version"), "{}", error.diagnostic.message);
    }
    for version in ["0.1.0", "10.20.30", "01.0.0"] {
        let text = format!("[package]\nname = \"a\"\nversion = \"{version}\"\n");
        assert!(Manifest::parse(&text).is_ok(), "{version}");
    }
}

#[test]
fn every_fault_of_a_manifest_is_reported() {
    let text = "[package]\nname = \"my-pkg\"\nedition = \"2024\"\nversion = \"1.0\"\n[plugins]\n";
    let errors = faults(text);
    let found: Vec<(&str, &str)> = errors.iter().map(|e| (e.code(), &text[e.diagnostic.span().range()])).collect();
    assert_eq!(found, [("E0171", "\"my-pkg\""), ("E0171", "edition"), ("E0171", "\"1.0\""), ("E0171", "plugins")]);
    let starts: Vec<usize> = errors.iter().map(|e| e.diagnostic.span().start).collect();
    assert!(starts.windows(2).all(|w| w[0] < w[1]), "source order: {starts:?}");
    // The same three of the task, with the unknown key last.
    let text = "[package]\nname = \"9a\"\nversion = \"x\"\nedition = \"2024\"\n";
    let errors = faults(text);
    assert_eq!(errors.iter().map(|e| &text[e.diagnostic.span().range()]).collect::<Vec<_>>(), ["\"9a\"", "\"x\"", "edition"]);
}
