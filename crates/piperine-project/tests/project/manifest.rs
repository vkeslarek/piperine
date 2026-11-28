//! Scope: reading a package manifest, `Piperine.toml` (feature `msl-packages`, PKG-10): the
//! unreadable manifest (`E0170`) and the rendered `PackageError`, the `[package]` table and the
//! `[dependencies]` table.

use std::fs;

use piperine_lang::msl::{Diagnostic, Span};
use piperine_project::{Dependency, GitRef, Manifest, PackageError, Requirement, Source, Version};

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

// ---- the `[dependencies]` table (T18) ----

const PACKAGE: &str = "[package]\nname = \"app\"\nversion = \"1.0.0\"\n";

/// The manifest of `PACKAGE` followed by `[dependencies]` and these lines.
fn with_deps(lines: &str) -> String {
    format!("{PACKAGE}[dependencies]\n{lines}")
}

#[test]
fn dependencies_read_in_source_order() {
    let text = with_deps(
        "zeta = { path = \"../zeta\" }\n\
         alpha = { git = \"https://example.org/a.git\", tag = \"v1\" }\n\
         mid = { git = \"https://example.org/m.git\", package = \"middle\", version = \"1.2\" }\n\
         b = { git = \"https://example.org/b.git\", branch = \"dev\" }\n\
         r = { git = \"https://example.org/r.git\", rev = \"0123abc\", version = \"0.2.3\" }\n\
         d = { git = \"https://example.org/d.git\" }\n",
    );
    let manifest = Manifest::parse(&text).unwrap();
    let names: Vec<&str> = manifest.dependencies.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, ["zeta", "alpha", "mid", "b", "r", "d"], "source order, not alphabetical");
    let dep = |i: usize| -> &Dependency { &manifest.dependencies[i] };
    assert_eq!(dep(0).source, Source::Path("../zeta".to_string()));
    assert_eq!((dep(0).requirement, dep(0).package.as_deref()), (None, None));
    assert_eq!(dep(1).source, Source::Git { url: "https://example.org/a.git".to_string(), reference: GitRef::Tag("v1".to_string()) });
    assert_eq!(dep(2).package.as_deref(), Some("middle"));
    assert_eq!(dep(2).requirement, Some(Requirement { major: 1, minor: Some(2), patch: None }));
    assert!(matches!(&dep(3).source, Source::Git { reference: GitRef::Branch(b), .. } if b == "dev"));
    assert!(matches!(&dep(4).source, Source::Git { reference: GitRef::Rev(r), .. } if r == "0123abc"));
    assert_eq!(dep(4).requirement, Some(Requirement { major: 0, minor: Some(2), patch: Some(3) }));
    assert!(matches!(&dep(5).source, Source::Git { reference: GitRef::DefaultBranch, .. }));
    // Each entry carries the span of its name.
    for d in &manifest.dependencies {
        assert_eq!(&text[d.name_span.range()], d.name);
    }
    // A sub-table is the same entry, and no `[dependencies]` is no dependency.
    let sub = format!("{PACKAGE}[dependencies.util]\npath = \"../util\"\n");
    assert_eq!(Manifest::parse(&sub).unwrap().dependencies[0].name, "util");
    assert!(Manifest::parse(PACKAGE).unwrap().dependencies.is_empty());
    assert!(Manifest::parse(&format!("{PACKAGE}[dependencies]\n")).unwrap().dependencies.is_empty());
    // Every requirement form.
    for (text, expected) in [("1", (1, None, None)), ("1.2", (1, Some(2), None)), ("1.2.3", (1, Some(2), Some(3))), ("0", (0, None, None))] {
        let m = Manifest::parse(&with_deps(&format!("u = {{ path = \"u\", version = \"{text}\" }}\n"))).unwrap();
        let r = m.dependencies[0].requirement.unwrap();
        assert_eq!((r.major, r.minor, r.patch), expected, "{text}");
    }
}

#[test]
fn a_dependency_needs_exactly_one_source() {
    for (line, key) in [
        ("a = {}", "a"),
        ("b = { path = \"x\", git = \"y\" }", "b"),
        ("c = { version = \"1\" }", "c"),
        ("d = { path = \"x\", git = \"y\", branch = \"z\" }", "d"),
        ("e = { package = \"e\" }", "e"),
    ] {
        let text = with_deps(&format!("{line}\n"));
        let (error, at) = one_e0171(&text);
        assert_eq!(at, key, "{line}");
        assert!(error.diagnostic.message.contains("`path`") && error.diagnostic.message.contains("`git`"), "{}", error.diagnostic.message);
    }
    // A wrong type of a source is its own fault at the value.
    let (_, at) = one_e0171(&with_deps("f = { path = 3 }\n"));
    assert_eq!(at, "3");
    let (_, at) = one_e0171(&with_deps("g = { git = true }\n"));
    assert_eq!(at, "true");
}

#[test]
fn git_refs_are_exclusive_and_git_only() {
    for line in [
        "a = { git = \"u\", branch = \"b\", rev = \"abcdef0\" }",
        "a = { git = \"u\", branch = \"b\", tag = \"t\" }",
        "a = { git = \"u\", tag = \"t\", rev = \"abcdef0\" }",
        "a = { git = \"u\", branch = \"b\", tag = \"t\", rev = \"abcdef0\" }",
        "a = { path = \"p\", tag = \"t\" }",
        "a = { path = \"p\", branch = \"b\" }",
        "a = { path = \"p\", rev = \"abcdef0\" }",
    ] {
        let (error, _) = one_e0171(&with_deps(&format!("{line}\n")));
        let message = &error.diagnostic.message;
        assert!(message.contains("`branch`") || message.contains("`tag`") || message.contains("`rev`"), "{line}: {message}");
    }
    // One reference beside `git` is fine.
    for line in ["a = { git = \"u\", branch = \"b\" }", "a = { git = \"u\", tag = \"t\" }", "a = { git = \"u\", rev = \"abcdef0\" }"] {
        Manifest::parse(&with_deps(&format!("{line}\n"))).unwrap_or_else(|e| panic!("{line}: {e:?}"));
    }
}

#[test]
fn a_rev_is_a_commit_id_of_seven_to_forty_hex_digits() {
    let long = "0123456789abcdef0123456789abcdef01234567";
    for bad in ["main", "HEAD", "abc12", "0123456789abcdef0123456789abcdef012345678", "g123456", "abc123!", "", "v1.0.0"] {
        let (error, at) = one_e0171(&with_deps(&format!("a = {{ git = \"u\", rev = \"{bad}\" }}\n")));
        assert_eq!(at, format!("\"{bad}\""), "the error is at the string");
        assert!(error.diagnostic.message.contains("`rev`") && error.diagnostic.message.contains("commit"), "{}", error.diagnostic.message);
    }
    for good in ["abcdef0", "ABCDEF0", long, &long[..12]] {
        let manifest = Manifest::parse(&with_deps(&format!("a = {{ git = \"u\", rev = \"{good}\" }}\n"))).unwrap_or_else(|e| panic!("{good}: {e:?}"));
        assert!(matches!(&manifest.dependencies[0].source, Source::Git { reference: GitRef::Rev(r), .. } if r == good));
    }
}

#[test]
fn a_bad_dependency_name_or_requirement_is_e0171() {
    // Names: `std`, a hyphen, a keyword, a digit first.
    for (key, written) in [("std", "std"), ("my-dep", "\"my-dep\""), ("fn", "fn"), ("9x", "9x")] {
        let (error, at) = one_e0171(&with_deps(&format!("{written} = {{ path = \"p\" }}\n")));
        assert!(at.contains(key), "{key}: the error is at {at:?}");
        assert!(error.diagnostic.message.contains("dependency name"), "{}", error.diagnostic.message);
    }
    // Requirements.
    for version in [">=1.0", "1.0.0.0", "", "x", "1.x", "^1.0", "1.", "v1", "-1", "1.0.0-rc1"] {
        let text = with_deps(&format!("u = {{ path = \"p\", version = \"{version}\" }}\n"));
        let (error, at) = one_e0171(&text);
        assert_eq!(at, format!("\"{version}\""), "the error is at the string");
        assert!(error.diagnostic.message.contains("version requirement"), "{}", error.diagnostic.message);
    }
    // `package` is a package name.
    for package in ["my-pkg", "std", "9a"] {
        let (_, at) = one_e0171(&with_deps(&format!("u = {{ path = \"p\", package = \"{package}\" }}\n")));
        assert_eq!(at, format!("\"{package}\""));
    }
    // Any other key, and a bare string (there is no registry).
    for key in ["subdir", "features", "registry"] {
        let (error, at) = one_e0171(&with_deps(&format!("u = {{ path = \"p\", {key} = \"x\" }}\n")));
        assert_eq!(at, key);
        assert!(error.diagnostic.message.contains(&format!("`{key}`")), "{}", error.diagnostic.message);
    }
    let (error, at) = one_e0171(&with_deps("util = \"1.0\"\n"));
    assert_eq!(at, "\"1.0\"");
    assert!(error.diagnostic.message.contains("table"), "{}", error.diagnostic.message);
    // `[dependencies]` itself is a table.
    let (_, at) = one_e0171(&format!("dependencies = 3\n{PACKAGE}"));
    assert_eq!(at, "3");
    // Faults across entries are all reported, in source order.
    let text = with_deps("a = {}\nmy-dep = { path = \"p\" }\nc = { path = \"p\", version = \"x\" }\n");
    let errors = faults(&text);
    assert_eq!(errors.len(), 3, "{errors:?}");
    assert!(errors.windows(2).all(|w| w[0].diagnostic.span().start < w[1].diagnostic.span().start));
}

#[test]
fn a_run_table_is_accepted_and_located() {
    // CLI-02 AC 4: a manifest holding `[run.<name>]` tables is accepted, and `Manifest::run` is
    // the span of the key `run`; the keys inside are the host's and are not read here, so even
    // a key no run knows passes
    let text = "[package]\nname = \"a\"\nversion = \"1.0.0\"\n\n[run.step]\nanalysis = \"transient\"\nstop = 5e-6\nstopp = 1\n\n[run.op]\nanalysis = \"op\"\n";
    let manifest = Manifest::parse(text).unwrap_or_else(|e| panic!("{e:?}"));
    let span = manifest.run.expect("the run table is located");
    assert_eq!(&text[span.range()], "run");
    assert_eq!(span.range().start, text.find("[run.step]").unwrap() + 1, "the key of the first `[run.<name>]` header");
    // a manifest without the table has none
    assert_eq!(Manifest::parse("[package]\nname = \"a\"\nversion = \"1.0.0\"\n").unwrap().run, None);
    // `run` that is not a table is E0171 at its value
    let text = "run = \"step\"\n[package]\nname = \"a\"\nversion = \"1.0.0\"\n";
    let (error, at) = one_e0171(text);
    assert_eq!(at, "\"step\"");
    assert!(error.diagnostic.message.contains("`run`") && error.diagnostic.message.contains("a table"), "{}", error.diagnostic.message);
}
