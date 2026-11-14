//! The lockfile, `Piperine.lock`: the commit each git package resolved to. Read through the
//! spanned TOML table and written by a plain string builder, so that the bytes are fixed.

use std::ops::Range;
use std::path::Path;

use piperine_lang::msl::{Diagnostic, Span};
use toml::de::{DeTable, DeValue};

use crate::error::PackageError;
use crate::manifest::Version;

/// The file name of a lockfile.
pub const LOCKFILE: &str = "Piperine.lock";

/// One `[[package]]` entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LockedPackage {
    pub name: String,
    pub version: Version,
    /// `git+<url>`, then `?branch=<b>`, `?tag=<t>` or `?rev=<r>` when one was given.
    pub source: String,
    /// 40 hexadecimal digits.
    pub commit: String,
}

/// A lockfile as read, or as the resolver will write it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lockfile {
    pub packages: Vec<LockedPackage>,
}

impl Lockfile {
    /// Read `dir/Piperine.lock`: `None` when there is no such file. Text that is not TOML, a
    /// `version` other than `1` and an entry the format does not allow are each an `E0176`.
    pub fn load(dir: &Path) -> Result<Option<Lockfile>, PackageError> {
        let file = dir.join(LOCKFILE);
        let text = match std::fs::read(&file) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(error(&file, String::new(), Span::new(0, 0), format!("cannot read the lockfile `{}`: {e}", file.display()), "unreadable")),
        };
        Lockfile::parse_at(&file, &text).map(Some)
    }

    /// Read lockfile text.
    pub fn parse(text: &str) -> Result<Lockfile, PackageError> {
        Lockfile::parse_at(Path::new(LOCKFILE), text)
    }

    fn parse_at(file: &Path, text: &str) -> Result<Lockfile, PackageError> {
        let fail = |span: Range<usize>, message: String, label: &str| error(file, text.to_string(), widen(text, span), message, label);
        let table = DeTable::parse(text).map_err(|e| fail(e.span().unwrap_or(0..0), format!("the lockfile is not valid TOML: {}", e.message()), "not valid TOML"))?;
        let table = table.get_ref();
        match table.get("version") {
            None => return Err(fail(0..0, "the lockfile has no `version`".to_string(), "expected `version = 1`")),
            Some(v) => match v.get_ref().as_integer().map(|i| i.as_str()) {
                Some("1") => {}
                _ => return Err(fail(v.span(), "the lockfile `version` is not `1`".to_string(), "expected `1`")),
            },
        }
        let mut packages = Vec::new();
        for (key, value) in table.iter() {
            match key.get_ref().as_ref() {
                "version" => {}
                "package" => {
                    let Some(entries) = value.get_ref().as_array() else {
                        return Err(fail(value.span(), "`package` must be an array of tables".to_string(), "expected `[[package]]`"));
                    };
                    for entry in entries.iter() {
                        let Some(entry_table) = entry.get_ref().as_table() else {
                            return Err(fail(entry.span(), "a `package` entry must be a table".to_string(), "expected a table"));
                        };
                        let field = |name: &str| -> Result<String, PackageError> {
                            match entry_table.get(name).map(|v| (v, v.get_ref())) {
                                Some((_, DeValue::String(s))) => Ok(s.to_string()),
                                Some((v, _)) => Err(fail(v.span(), format!("`{name}` of a `package` entry must be a string"), "expected a string")),
                                None => Err(fail(entry.span(), format!("a `package` entry has no `{name}`"), "this entry is incomplete")),
                            }
                        };
                        for key in entry_table.keys() {
                            if !["name", "version", "source", "commit"].contains(&key.get_ref().as_ref()) {
                                return Err(fail(key.span(), format!("unknown key `{}` in a `package` entry", key.get_ref()), "not a lockfile key"));
                            }
                        }
                        let (name, version, source, commit) = (field("name")?, field("version")?, field("source")?, field("commit")?);
                        let at = entry_table.get("version").map_or(0..0, |v| v.span());
                        let version = crate::manifest::parse_version(&version).ok_or_else(|| fail(at, format!("`{version}` is not a version"), "expected `MAJOR.MINOR.PATCH`"))?;
                        if commit.len() != 40 || !commit.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
                            let at = entry_table.get("commit").map_or(0..0, |v| v.span());
                            return Err(fail(at, format!("`{commit}` is not a commit: 40 lowercase hexadecimal digits"), "expected a full commit id"));
                        }
                        packages.push(LockedPackage { name, version, source, commit });
                    }
                }
                other => return Err(fail(key.span(), format!("unknown key `{other}` in the lockfile"), "not a lockfile key")),
            }
        }
        Ok(Lockfile { packages })
    }

    /// The file's text: `version = 1`, then each entry sorted by `name`, then `source`.
    pub fn render(&self) -> String {
        let mut entries: Vec<&LockedPackage> = self.packages.iter().collect();
        entries.sort_by(|a, b| (&a.name, &a.source).cmp(&(&b.name, &b.source)));
        let mut out = String::from("version = 1\n");
        for entry in entries {
            out.push_str("\n[[package]]\n");
            out.push_str(&format!("name = {}\n", quote(&entry.name)));
            out.push_str(&format!("version = {}\n", quote(&entry.version.to_string())));
            out.push_str(&format!("source = {}\n", quote(&entry.source)));
            out.push_str(&format!("commit = {}\n", quote(&entry.commit)));
        }
        out
    }
}

/// A TOML basic string.
fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A span that is empty covers the next character, so that a report can underline it.
fn widen(text: &str, range: Range<usize>) -> Span {
    if range.start != range.end {
        return Span::new(range.start, range.end);
    }
    let at = range.start.min(text.len());
    match text[at..].chars().next() {
        Some(c) => Span::new(at, at + c.len_utf8()),
        None => Span::new(at, at),
    }
}

/// An `E0176` over the lockfile's own text.
pub(crate) fn error(file: &Path, text: String, span: Span, message: String, label: &str) -> PackageError {
    PackageError { file: file.to_path_buf(), text, diagnostic: Diagnostic::new("E0176", message, span, label) }
}
