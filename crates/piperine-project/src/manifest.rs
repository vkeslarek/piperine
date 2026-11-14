//! The manifest, `Piperine.toml`: read into spanned TOML, then walked by hand so that every fault
//! is a coded diagnostic at the text it is about.

use std::ops::Range;
use std::path::Path;

use piperine_lang::msl::{Diagnostic, Span};
use piperine_lang::pom::is_crate_name;
use toml::Spanned;
use toml::de::{DeString, DeTable, DeValue};

use crate::error::PackageError;

/// The file name of a manifest.
pub const MANIFEST: &str = "Piperine.toml";

/// `MAJOR.MINOR.PATCH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

/// The `[package]` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// The crate's name.
    pub name: String,
    pub version: Version,
    pub authors: Vec<String>,
    /// Empty when the manifest gives none.
    pub description: String,
}

/// A manifest as read and validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub package: Package,
}

impl Manifest {
    /// Read `dir/Piperine.toml`. A missing file, text that is not UTF-8 and text that is not
    /// TOML are each one `E0170`; every other fault is an `E0171`, in source order.
    pub fn load(dir: &Path) -> Result<Manifest, Vec<PackageError>> {
        let file = dir.join(MANIFEST);
        let fail = |text: String, diagnostic: Diagnostic| vec![PackageError { file: file.clone(), text, diagnostic }];
        let bytes = std::fs::read(&file).map_err(|e| {
            let message = format!("cannot read the manifest `{}`: {e}", file.display());
            fail(String::new(), Diagnostic::new("E0170", message, Span::new(0, 0), "a package holds a `Piperine.toml`"))
        })?;
        match String::from_utf8(bytes) {
            Ok(text) => Manifest::parse_at(&file, &text),
            Err(e) => {
                let at = e.utf8_error().valid_up_to();
                // The lossy text holds one U+FFFD (three bytes) for the offending sequence.
                let text = String::from_utf8_lossy(e.as_bytes()).into_owned();
                let message = format!("`{}` is not UTF-8 text", file.display());
                Err(fail(text, Diagnostic::new("E0170", message, Span::new(at, at + '\u{FFFD}'.len_utf8()), "not valid UTF-8")))
            }
        }
    }

    /// Read manifest text; its errors name the file `Piperine.toml`.
    pub fn parse(text: &str) -> Result<Manifest, Vec<PackageError>> {
        Manifest::parse_at(Path::new(MANIFEST), text)
    }

    fn parse_at(file: &Path, text: &str) -> Result<Manifest, Vec<PackageError>> {
        let table = match DeTable::parse(text) {
            Ok(table) => table,
            Err(e) => {
                let span = widen(text, e.span().unwrap_or(0..0));
                let diagnostic = Diagnostic::new("E0170", format!("the manifest is not valid TOML: {}", e.message()), span, "not valid TOML");
                return Err(vec![PackageError { file: file.to_path_buf(), text: text.to_string(), diagnostic }]);
            }
        };
        let mut walk = Walk { text, faults: Vec::new() };
        let package = walk.manifest(table.get_ref());
        let mut faults = walk.faults;
        faults.sort_by_key(|d| d.span().start);
        match package {
            Some(package) if faults.is_empty() => Ok(Manifest { package }),
            _ => Err(faults.into_iter().map(|diagnostic| PackageError { file: file.to_path_buf(), text: text.to_string(), diagnostic }).collect()),
        }
    }
}

/// A span `toml` reports empty (the place something was expected) covers the next character, so
/// that a report can underline it; at the end of the text, the last one.
fn widen(text: &str, range: Range<usize>) -> Span {
    if range.start != range.end {
        return Span::new(range.start, range.end);
    }
    let at = range.start.min(text.len());
    match text[at..].chars().next() {
        Some(c) => Span::new(at, at + c.len_utf8()),
        None => match text[..at].chars().next_back() {
            Some(c) => Span::new(at - c.len_utf8(), at),
            None => Span::new(0, 0),
        },
    }
}

type Key<'i> = Spanned<DeString<'i>>;
type Value<'i> = Spanned<DeValue<'i>>;

/// The entries of a table in source order (the table itself keeps them sorted by key).
fn entries<'a, 'i>(table: &'a DeTable<'i>) -> Vec<(&'a Key<'i>, &'a Value<'i>)> {
    let mut all: Vec<_> = table.iter().collect();
    all.sort_by_key(|(k, _)| k.span().start);
    all
}

const MANIFEST_KEYS: &[&str] = &["package", "dependencies"];
const PACKAGE_KEYS: &[&str] = &["name", "version", "authors", "description"];

/// The faults of one manifest, found by walking its spanned table.
struct Walk<'t> {
    text: &'t str,
    faults: Vec<Diagnostic>,
}

impl Walk<'_> {
    fn fault(&mut self, span: Range<usize>, message: String, label: &str, help: Option<String>) {
        let mut d = Diagnostic::new("E0171", message, widen(self.text, span), label);
        d.help = help;
        self.faults.push(d);
    }

    fn unknown(&mut self, key: &Key<'_>, table: Option<&str>, allowed: &[&str]) {
        let name = key.get_ref();
        let (message, label) = match table {
            Some(t) => (format!("unknown key `{name}` in `[{t}]`"), format!("not a key of `[{t}]`")),
            None => (format!("unknown table or key `{name}` in the manifest"), "not part of the manifest".to_string()),
        };
        let help = format!("{} takes: {}", table.map_or("the manifest".to_string(), |t| format!("`[{t}]`")), allowed.join(", "));
        self.fault(key.span(), message, &label, Some(help));
    }

    fn mistyped(&mut self, key: &str, value: &Value<'_>, wants: &str) {
        let message = format!("`{key}` must be {wants}, found {}", value.get_ref().type_str());
        self.fault(value.span(), message, &format!("expected {wants}"), None);
    }

    fn manifest(&mut self, table: &DeTable<'_>) -> Option<Package> {
        let mut package = None;
        let mut seen = false;
        for (key, value) in entries(table) {
            match key.get_ref().as_ref() {
                "package" => {
                    seen = true;
                    match value.get_ref().as_table() {
                        Some(t) => package = self.package(key, t),
                        None => self.mistyped("package", value, "a table"),
                    }
                }
                // Read by the next task.
                "dependencies" => {}
                _ => self.unknown(key, None, MANIFEST_KEYS),
            }
        }
        if !seen {
            self.fault(0..0, "the manifest has no `package` table".to_string(), "a manifest starts with `[package]`", Some("add a `[package]` table with a `name` and a `version`".to_string()));
        }
        package
    }

    fn package(&mut self, header: &Key<'_>, table: &DeTable<'_>) -> Option<Package> {
        let (mut name, mut version) = (None, None);
        let (mut authors, mut description) = (Vec::new(), String::new());
        let mut broken = false;
        for (key, value) in entries(table) {
            match key.get_ref().as_ref() {
                "name" => match value.get_ref().as_str() {
                    Some(s) if is_crate_name(s) => name = Some(s.to_string()),
                    Some(s) => {
                        let message = format!("`{s}` is not a package name");
                        let help = "a package name is an identifier that is no keyword of the language and not `std`".to_string();
                        self.fault(value.span(), message, "not a package name", Some(help));
                        broken = true;
                    }
                    None => {
                        self.mistyped("name", value, "a string");
                        broken = true;
                    }
                },
                "version" => match value.get_ref().as_str() {
                    Some(s) => match parse_version(s) {
                        Some(v) => version = Some(v),
                        None => {
                            self.fault(value.span(), format!("`{s}` is not a version"), "expected `MAJOR.MINOR.PATCH`", Some("a version is three decimal integers separated by dots, such as `1.0.0`".to_string()));
                            broken = true;
                        }
                    },
                    None => {
                        self.mistyped("version", value, "a string");
                        broken = true;
                    }
                },
                "authors" => match value.get_ref().as_array() {
                    Some(items) => {
                        for item in items.iter() {
                            match item.get_ref().as_str() {
                                Some(s) => authors.push(s.to_string()),
                                None => {
                                    self.mistyped("authors", item, "an array of strings");
                                    broken = true;
                                }
                            }
                        }
                    }
                    None => {
                        self.mistyped("authors", value, "an array of strings");
                        broken = true;
                    }
                },
                "description" => match value.get_ref().as_str() {
                    Some(s) => description = s.to_string(),
                    None => {
                        self.mistyped("description", value, "a string");
                        broken = true;
                    }
                },
                _ => {
                    self.unknown(key, Some("package"), PACKAGE_KEYS);
                    broken = true;
                }
            }
        }
        for (key, found) in [("name", name.is_some()), ("version", version.is_some())] {
            // A key that is there but faulty was reported at its value.
            let present = table.keys().any(|k| k.get_ref() == key);
            if !found && !present {
                self.fault(header.span(), format!("`[package]` has no `{key}`"), &format!("`{key}` is required"), None);
                broken = true;
            }
        }
        match (name, version) {
            (Some(name), Some(version)) if !broken => Some(Package { name, version, authors, description }),
            _ => None,
        }
    }
}

/// Three decimal integers separated by dots.
fn parse_version(text: &str) -> Option<Version> {
    let mut parts = text.split('.');
    let mut next = || {
        let part = parts.next()?;
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        part.parse::<u64>().ok()
    };
    let version = Version { major: next()?, minor: next()?, patch: next()? };
    parts.next().is_none().then_some(version)
}
