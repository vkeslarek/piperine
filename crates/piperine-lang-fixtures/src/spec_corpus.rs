//! The extraction of the specification's code blocks, shared by the MSL
//! corpus and printer targets.

use std::path::PathBuf;

use piperine_lang::msl::parser::{ExpressionGrammar, SourceParser};

/// What a fenced block of the specification promises of its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockClass {
    /// The text parses and elaborates.
    Valid,
    /// The text is refused, as its fence says.
    CompileFail,
    /// The text is a piece of a file, parsed as an expression or a statement.
    Fragment,
}

/// One fenced block of a specification file.
pub struct SpecBlock {
    /// The file the block is in.
    pub file: String,
    /// 1-based line of the opening fence.
    pub line: usize,
    /// What the fence promises.
    pub class: BlockClass,
    /// The text inside the fence.
    pub text: String,
}

/// The fenced blocks of the specification's markdown, read from `docs/spec`.
pub struct SpecCorpus;

impl SpecCorpus {
    /// The directory of the specification's markdown files.
    fn specification_directory() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec")
    }

    /// Every ```` ```rust* ```` fence of every `docs/spec/*.md`, in file order.
    pub fn corpus() -> Vec<SpecBlock> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(SpecCorpus::specification_directory())
            .expect("docs/spec exists")
            .map(|e| e.expect("directory entry").path())
            .filter(|p| p.extension().is_some_and(|e| e == "md"))
            .collect();
        files.sort();
        let mut blocks = Vec::new();
        for path in files {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).expect("spec file is UTF-8");
            blocks.extend(SpecCorpus::extract(&name, &text));
        }
        blocks
    }

    /// The ```` ```rust* ```` fences of one markdown text, in order.
    pub fn extract(name: &str, text: &str) -> Vec<SpecBlock> {
        let mut blocks = Vec::new();
        let mut open: Option<(usize, BlockClass, String)> = None;
        for (i, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            match &mut open {
                None => {
                    if let Some(info) = trimmed.strip_prefix("```") {
                        let class = match info.trim() {
                            "rust" => Some(BlockClass::Valid),
                            "rust,compile_fail" => Some(BlockClass::CompileFail),
                            "rust,fragment" => Some(BlockClass::Fragment),
                            other if other.starts_with("rust") => {
                                panic!("{name}:{}: unknown fence class `{other}`", i + 1)
                            }
                            _ => None,
                        };
                        // A fence of another language or none is not the corpus,
                        // but its closing fence must still be consumed.
                        open = Some((i + 1, class.unwrap_or(BlockClass::Valid), String::new()));
                        if class.is_none() {
                            open.as_mut().unwrap().2.push('\u{0}');
                        }
                    }
                }
                Some((start, class, body)) => {
                    if trimmed.starts_with("```") {
                        if !body.starts_with('\u{0}') {
                            blocks.push(SpecBlock {
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
    pub fn fragment_diagnostics(text: &str) -> Vec<String> {
        let as_file = SourceParser::parse(text);
        if as_file.diagnostics.is_empty() {
            return Vec::new();
        }
        if ExpressionGrammar::parse_block(&format!("{{\n{text}\n}}")).1.is_empty() {
            return Vec::new();
        }
        if SourceParser::parse(&format!("module Fragment() {{\n{text}\n}}")).diagnostics.is_empty() {
            return Vec::new();
        }
        let lines: Vec<&str> = text
            .lines()
            .map(|l| l.split("//").next().unwrap_or("").trim())
            .filter(|l| !l.is_empty())
            .collect();
        if !lines.is_empty() && lines.iter().all(|l| ExpressionGrammar::parse(l).1.is_empty()) {
            return Vec::new();
        }
        as_file.diagnostics.iter().map(|d| format!("{}: {}", d.code, d.message)).collect()
    }
}

