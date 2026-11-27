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

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
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

/// A version requirement: `M`, `M.m` or `M.m.p`, Cargo's caret rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Requirement {
    pub major: u64,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
}

impl Requirement {
    /// Cargo's caret rule: `>=` the requirement with its missing parts zero, and below the next
    /// version that changes the leftmost non-zero part that was written.
    pub fn admits(&self, version: Version) -> bool {
        let lower = Version { major: self.major, minor: self.minor.unwrap_or(0), patch: self.patch.unwrap_or(0) };
        // The leftmost non-zero part written (the major, or the last part written when all
        // before it are zero) is the one the version may not raise: the upper bound is its next
        // value, so with `lower <= version` the version equals the requirement up to and
        // including that part. Stated as equality, the bound needs no increment and cannot overflow.
        let kept = match (self.major, self.minor, self.patch) {
            (major, _, _) if major > 0 => 0,
            (_, None, _) => 0,
            (_, Some(minor), _) if minor > 0 => 1,
            (_, Some(_), None) => 1,
            (_, Some(_), Some(_)) => 2,
        };
        let own = [lower.major, lower.minor, lower.patch];
        let given = [version.major, version.minor, version.patch];
        lower <= version && given[..=kept] == own[..=kept]
    }
}

impl std::fmt::Display for Requirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.major)?;
        if let Some(minor) = self.minor {
            write!(f, ".{minor}")?;
            if let Some(patch) = self.patch {
                write!(f, ".{patch}")?;
            }
        }
        Ok(())
    }
}

/// The git reference a dependency asks for; none means the repository's default branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitRef {
    DefaultBranch,
    Branch(String),
    Tag(String),
    Rev(String),
}

/// Where a dependency is: a directory, or a git repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// As written: relative to the directory of the manifest that names it, or absolute.
    Path(String),
    Git { url: String, reference: GitRef },
}

/// An entry of `[dependencies]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// The dependency name: the key.
    pub name: String,
    /// The span of the key in the manifest text.
    pub name_span: Span,
    pub source: Source,
    pub requirement: Option<Requirement>,
    /// The located package's own name, when it differs from the key.
    pub package: Option<String>,
}

/// A manifest as read and validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub package: Package,
    /// In source order.
    pub dependencies: Vec<Dependency>,
    /// The span of the key `run` when the manifest holds a `[run]` table. Its content belongs to
    /// the host and is not read here.
    pub run: Option<Span>,
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
                let span = Manifest::widen(text, e.span().unwrap_or(0..0));
                let diagnostic = Diagnostic::new("E0170", format!("the manifest is not valid TOML: {}", e.message()), span, "not valid TOML");
                return Err(vec![PackageError { file: file.to_path_buf(), text: text.to_string(), diagnostic }]);
            }
        };
        let mut walk = Walk { text, faults: Vec::new() };
        let (package, dependencies, run) = walk.manifest(table.get_ref());
        let mut faults = walk.faults;
        faults.sort_by_key(|d| d.span().start);
        match package {
            Some(package) if faults.is_empty() => Ok(Manifest { package, dependencies, run }),
            _ => Err(faults.into_iter().map(|diagnostic| PackageError { file: file.to_path_buf(), text: text.to_string(), diagnostic }).collect()),
        }
    }
}

type Key<'i> = Spanned<DeString<'i>>;
type Value<'i> = Spanned<DeValue<'i>>;

const MANIFEST_KEYS: &[&str] = &["package", "dependencies", "run"];
const PACKAGE_KEYS: &[&str] = &["name", "version", "authors", "description"];
const DEPENDENCY_KEYS: &[&str] = &["path", "git", "branch", "tag", "rev", "version", "package"];

/// The faults of one manifest, found by walking its spanned table.
struct Walk<'t> {
    text: &'t str,
    faults: Vec<Diagnostic>,
}

impl Walk<'_> {
    fn fault(&mut self, span: Range<usize>, message: String, label: &str, help: Option<String>) {
        let mut d = Diagnostic::new("E0171", message, Manifest::widen(self.text, span), label);
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

    fn manifest(&mut self, table: &DeTable<'_>) -> (Option<Package>, Vec<Dependency>, Option<Span>) {
        let mut package = None;
        let mut dependencies = Vec::new();
        let mut run = None;
        let mut seen = false;
        for (key, value) in Manifest::entries(table) {
            match key.get_ref().as_ref() {
                "package" => {
                    seen = true;
                    match value.get_ref().as_table() {
                        Some(t) => package = self.package(key, t),
                        None => self.mistyped("package", value, "a table"),
                    }
                }
                "dependencies" => match value.get_ref().as_table() {
                    Some(t) => self.dependencies(t, &mut dependencies),
                    None => self.mistyped("dependencies", value, "a table"),
                },
                // The host's table: located, never read here.
                "run" => match value.get_ref().as_table() {
                    Some(_) => run = Some(Span::new(key.span().start, key.span().end)),
                    None => self.mistyped("run", value, "a table"),
                },
                _ => self.unknown(key, None, MANIFEST_KEYS),
            }
        }
        if !seen {
            self.fault(0..0, "the manifest has no `package` table".to_string(), "a manifest starts with `[package]`", Some("add a `[package]` table with a `name` and a `version`".to_string()));
        }
        (package, dependencies, run)
    }

    fn dependencies(&mut self, table: &DeTable<'_>, out: &mut Vec<Dependency>) {
        for (key, value) in Manifest::entries(table) {
            let name = key.get_ref().to_string();
            let mut broken = false;
            if !is_crate_name(&name) {
                let help = "a dependency name is an identifier that is no keyword of the language and not `std`".to_string();
                self.fault(key.span(), format!("`{name}` is not a dependency name"), "not a dependency name", Some(help));
                broken = true;
            }
            let Some(entry) = value.get_ref().as_table() else {
                let message = format!("dependency `{name}` must be a table with a `path` or a `git`, found {}", value.get_ref().type_str());
                self.fault(value.span(), message, "expected a table such as `{ path = \"..\" }`", Some("there is no registry: name a `path` or a `git` source".to_string()));
                continue;
            };
            if let Some(dependency) = self.dependency(key, &name, entry) {
                if !broken {
                    out.push(dependency);
                }
            }
        }
    }

    /// One entry's keys; `None` when it has a fault (each reported once).
    fn dependency(&mut self, key: &Key<'_>, name: &str, entry: &DeTable<'_>) -> Option<Dependency> {
        let mut ok = true;
        let (mut path, mut git, mut package, mut requirement) = (None, None, None, None);
        // Reference keys in source order, with the span of the key.
        let mut refs: Vec<(&str, Range<usize>, String)> = Vec::new();
        for (k, v) in Manifest::entries(entry) {
            let k_name = k.get_ref().as_ref();
            match k_name {
                "path" | "git" | "branch" | "tag" | "rev" | "version" | "package" => {
                    let Some(s) = v.get_ref().as_str() else {
                        self.mistyped(k_name, v, "a string");
                        ok = false;
                        continue;
                    };
                    match k_name {
                        "path" => path = Some(s.to_string()),
                        "git" => git = Some(s.to_string()),
                        "rev" if !GitRef::is_commit_id(s) => {
                            let help = "a `rev` is a commit id: 7 to 40 hexadecimal digits; use `branch` or `tag` for a name".to_string();
                            self.fault(v.span(), format!("`{s}` is not a commit id for `rev`"), "expected 7 to 40 hexadecimal digits", Some(help));
                            ok = false;
                        }
                        "branch" | "tag" | "rev" => refs.push((k_name, k.span(), s.to_string())),
                        "version" => match Requirement::parse(s) {
                            Some(r) => requirement = Some(r),
                            None => {
                                let help = "a version requirement is `MAJOR`, `MAJOR.MINOR` or `MAJOR.MINOR.PATCH`".to_string();
                                self.fault(v.span(), format!("`{s}` is not a version requirement"), "expected `1`, `1.2` or `1.2.3`", Some(help));
                                ok = false;
                            }
                        },
                        _ => {
                            if is_crate_name(s) {
                                package = Some(s.to_string());
                            } else {
                                let help = "a package name is an identifier that is no keyword of the language and not `std`".to_string();
                                self.fault(v.span(), format!("`{s}` is not a package name"), "not a package name", Some(help));
                                ok = false;
                            }
                        }
                    }
                }
                _ => {
                    self.unknown(k, Some(&format!("dependencies.{name}")), DEPENDENCY_KEYS);
                    ok = false;
                }
            }
        }
        // A source key of the wrong type was reported at its value: it is not also a missing source.
        let (has_path, has_git) = (entry.contains_key("path"), entry.contains_key("git"));
        let source = match (path, git) {
            (Some(path), None) => Some(Source::Path(path)),
            (None, Some(url)) => Some(Source::Git { url, reference: GitRef::DefaultBranch }),
            (None, None) if has_path != has_git => None,
            _ => {
                self.fault(key.span(), format!("dependency `{name}` needs exactly one of `path` and `git`"), "name a `path` or a `git` source", None);
                None
            }
        };
        let mut source = source?;
        match &mut source {
            Source::Git { reference, .. } => {
                if let Some((first, rest)) = refs.split_first() {
                    if let Some((second, span, _)) = rest.first() {
                        let message = format!("dependency `{name}` names `{}` and `{second}`: at most one of `branch`, `tag` and `rev`", first.0);
                        self.fault(span.clone(), message, "one reference only", None);
                        ok = false;
                    } else {
                        *reference = match first.0 {
                            "branch" => GitRef::Branch(first.2.clone()),
                            "tag" => GitRef::Tag(first.2.clone()),
                            _ => GitRef::Rev(first.2.clone()),
                        };
                    }
                }
            }
            Source::Path(_) => {
                if let Some((which, span, _)) = refs.first() {
                    self.fault(span.clone(), format!("`{which}` needs a `git` source, but dependency `{name}` has a `path`"), "a reference belongs to a `git` dependency", None);
                    ok = false;
                }
            }
        }
        ok.then(|| Dependency { name: name.to_string(), name_span: Span::new(key.span().start, key.span().end), source, requirement, package })
    }

    fn package(&mut self, header: &Key<'_>, table: &DeTable<'_>) -> Option<Package> {
        let (mut name, mut version) = (None, None);
        let (mut authors, mut description) = (Vec::new(), String::new());
        let mut broken = false;
        for (key, value) in Manifest::entries(table) {
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
                    Some(s) => match Version::parse(s) {
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

impl Version {
    /// Three decimal integers separated by dots.
    pub(crate) fn parse(text: &str) -> Option<Version> {
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
}

impl Requirement {
    /// `M`, `M.m` or `M.m.p`, each a decimal integer.
    pub(crate) fn parse(text: &str) -> Option<Requirement> {
        let parts: Vec<&str> = text.split('.').collect();
        if parts.len() > 3 {
            return None;
        }
        let mut numbers = Vec::new();
        for part in parts {
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            numbers.push(part.parse::<u64>().ok()?);
        }
        Some(Requirement { major: numbers[0], minor: numbers.get(1).copied(), patch: numbers.get(2).copied() })
    }
}

impl GitRef {
    /// A `rev`: a full or abbreviated commit id.
    pub(crate) fn is_commit_id(text: &str) -> bool {
        (7..=40).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_hexdigit())
    }
}

impl Manifest {
    /// A span `toml` reports empty (the place something was expected) covers the next character, so
    /// that a report can underline it; at the end of the text, the last one.
    pub(crate) fn widen(text: &str, range: Range<usize>) -> Span {
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

    /// The entries of a table in source order (the table itself keeps them sorted by key).
    fn entries<'a, 'i>(table: &'a DeTable<'i>) -> Vec<(&'a Key<'i>, &'a Value<'i>)> {
        let mut all: Vec<_> = table.iter().collect();
        all.sort_by_key(|(k, _)| k.span().start);
        all
    }
}
