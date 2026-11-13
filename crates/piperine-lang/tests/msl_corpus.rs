//! Scope: the specification's own code blocks as the acceptance corpus of the
//! MSL parser (requirement MSLP-03). Every ```` ```rust* ```` fence in
//! `docs/spec/*.md` is extracted at test time and classified by its info
//! string: `rust` must parse with zero diagnostics, `rust,compile_fail`
//! illustrates an error and must report at least one, `rust,fragment` is a
//! piece of a file (statements, items without their context) and must parse
//! as a file or as the body of a block. A `rust,*` info string of any other
//! class fails the suite, so a new class cannot slip in unchecked.

use std::path::PathBuf;

use piperine_lang::msl::parser::{parse, parse_block, parse_expr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Valid,
    CompileFail,
    Fragment,
}

struct Block {
    file: String,
    /// 1-based line of the opening fence.
    line: usize,
    class: Class,
    text: String,
}

fn spec_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec")
}

/// Every ```` ```rust* ```` fence of every `docs/spec/*.md`, in file order.
fn corpus() -> Vec<Block> {
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
                                file: name.clone(),
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
    }
    blocks
}

fn count(blocks: &[Block], class: Class) -> usize {
    blocks.iter().filter(|b| b.class == class).count()
}

/// Diagnostics of a fragment, or none when some entry accepts it. A fragment
/// is a piece of a file, so it is tried as a whole file, as the body of a
/// block (statements), as the body of a module (params, signals, `let`s,
/// definitions), and as a list of expressions, one per line.
fn fragment_diagnostics(text: &str) -> Vec<String> {
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

#[test]
fn every_valid_spec_block_parses_clean() {
    let blocks = corpus();
    println!(
        "corpus: {} rust, {} rust,compile_fail, {} rust,fragment",
        count(&blocks, Class::Valid),
        count(&blocks, Class::CompileFail),
        count(&blocks, Class::Fragment)
    );
    assert!(count(&blocks, Class::Valid) > 0, "the corpus is not empty");
    let mut failures = Vec::new();
    for b in &blocks {
        let diags: Vec<String> = match b.class {
            Class::Valid => parse(&b.text).diagnostics.iter().map(|d| format!("{}: {}", d.code, d.message)).collect(),
            Class::Fragment => fragment_diagnostics(&b.text),
            Class::CompileFail => continue,
        };
        if !diags.is_empty() {
            failures.push(format!("{}:{} ({:?}): {}", b.file, b.line, b.class, diags.join("; ")));
        }
    }
    assert!(failures.is_empty(), "{} block(s) do not parse:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn every_compile_fail_block_diagnoses() {
    let blocks = corpus();
    let fails: Vec<&Block> = blocks.iter().filter(|b| b.class == Class::CompileFail).collect();
    println!("compile_fail blocks: {}", fails.len());
    let silent: Vec<String> = fails
        .iter()
        .filter(|b| parse(&b.text).diagnostics.is_empty() && parse_block(&format!("{{\n{}\n}}", b.text)).1.is_empty())
        .map(|b| format!("{}:{}", b.file, b.line))
        .collect();
    assert!(silent.is_empty(), "compile_fail blocks that parse clean: {silent:?}");
}
