//! The extraction of the specification's code blocks, shared by the MSL
//! corpus and printer targets (`#[path]`-included, never a target itself).

use std::path::PathBuf;

use piperine_lang::msl::parser::{parse, parse_block, parse_expr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Valid,
    CompileFail,
    Fragment,
}

pub struct Block {
    pub file: String,
    /// 1-based line of the opening fence.
    pub line: usize,
    pub class: Class,
    pub text: String,
}

fn spec_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec")
}

/// Every ```` ```rust* ```` fence of every `docs/spec/*.md`, in file order.
pub fn corpus() -> Vec<Block> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(spec_dir())
        .expect("docs/spec exists")
        .map(|e| e.expect("directory entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    files.sort();
    let mut blocks = Vec::new();
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).expect("spec file is UTF-8");
        blocks.extend(extract(&name, &text));
    }
    blocks
}

/// The ```` ```rust* ```` fences of one markdown text, in order.
pub fn extract(name: &str, text: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut open: Option<(usize, Class, String)> = None;
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        match &mut open {
            None => {
                if let Some(info) = trimmed.strip_prefix("```") {
                    let class = match info.trim() {
                        "rust" => Some(Class::Valid),
                        "rust,compile_fail" => Some(Class::CompileFail),
                        "rust,fragment" => Some(Class::Fragment),
                        other if other.starts_with("rust") => {
                            panic!("{name}:{}: unknown fence class `{other}`", i + 1)
                        }
                        _ => None,
                    };
                    // A fence of another language or none is not the corpus,
                    // but its closing fence must still be consumed.
                    open = Some((i + 1, class.unwrap_or(Class::Valid), String::new()));
                    if class.is_none() {
                        open.as_mut().unwrap().2.push('\u{0}');
                    }
                }
            }
            Some((start, class, body)) => {
                if trimmed.starts_with("```") {
                    if !body.starts_with('\u{0}') {
                        blocks.push(Block {
                            file: name.to_string(),
                            line: *start,
                            class: *class,
                            text: std::mem::take(body),
                        });
                    }
                    open = None;
                } else {
                    body.push_str(line);
                    body.push('\n');
                }
            }
        }
    }
    assert!(open.is_none(), "{name}: a code fence is never closed");
    blocks
}

/// Diagnostics of a fragment, or none when some entry accepts it. A fragment
/// is a piece of a file, so it is tried as a whole file, as the body of a
/// block (statements), as the body of a module (params, signals, `let`s,
/// definitions), and as a list of expressions, one per line.
#[allow(dead_code)] // shared by the targets that classify fences; not every includer does
pub fn fragment_diagnostics(text: &str) -> Vec<String> {
    let as_file = parse(text);
    if as_file.diagnostics.is_empty() {
        return Vec::new();
    }
    if parse_block(&format!("{{\n{text}\n}}")).1.is_empty() {
        return Vec::new();
    }
    if parse(&format!("module Fragment() {{\n{text}\n}}")).diagnostics.is_empty() {
        return Vec::new();
    }
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.split("//").next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .collect();
    if !lines.is_empty() && lines.iter().all(|l| parse_expr(l).1.is_empty()) {
        return Vec::new();
    }
    as_file.diagnostics.iter().map(|d| format!("{}: {}", d.code, d.message)).collect()
}
