//! The text of items of `examples/buck/src`, read as written, so a test checks the buck's own modules and never a retyped copy.

/// The text of items of `examples/buck/src`, read as written, so a test checks the buck's own modules and never a retyped copy.
pub struct BuckSource;

impl BuckSource {
    fn read(file: &str) -> String {
        let path = format!("{}/../../examples/buck/src/{file}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    /// The constants of `examples/buck/src/spec.msl`, the whole file (its inner docs are at its top).
    pub fn spec() -> String {
        BuckSource::read("spec.msl")
    }

    /// The text of the braces that open at or after `from` in `text`, through the matching `}`.
    fn through_closing_brace(text: &str, from: usize) -> usize {
        let mut depth = 0usize;
        for (i, c) in text[from..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return from + i + 1;
                    }
                }
                _ => {}
            }
        }
        panic!("an item does not close")
    }

    /// The item of `file` that starts with `head` (`pub module Controller`, `pub enum Mode`,
    /// `pub fn rev3`), through its closing brace, with the `#[derive(..)]` line before it.
    pub fn item(file: &str, head: &str) -> String {
        let text = BuckSource::read(file);
        let at = text.find(head).unwrap_or_else(|| panic!("no `{head}` in {file}"));
        let start = text[..at].rfind("#[derive").filter(|d| !text[*d..at].contains('}') && !text[*d..at].contains(";\n")).unwrap_or(at);
        text[start..BuckSource::through_closing_brace(&text, at)].to_string()
    }

    /// The `digital for name { .. }` block of `file`.
    pub fn digital(file: &str, name: &str) -> String {
        let text = BuckSource::read(file);
        let head = format!("digital for {name} ");
        let at = text.find(&head).unwrap_or_else(|| panic!("no `{head}` in {file}"));
        text[at..BuckSource::through_closing_brace(&text, at)].to_string()
    }

    /// A module of `file` with its `digital` block: the module item, then the block.
    pub fn module_with_digital(file: &str, name: &str) -> String {
        format!("{}\n{}", BuckSource::item(file, &format!("pub module {name}(")), BuckSource::digital(file, name))
    }

    /// The `analog for name { .. }` block of `file`.
    pub fn analog(file: &str, name: &str) -> String {
        let text = BuckSource::read(file);
        let head = format!("analog for {name} ");
        let at = text.find(&head).unwrap_or_else(|| panic!("no `{head}` in {file}"));
        text[at..BuckSource::through_closing_brace(&text, at)].to_string()
    }

    /// A module of `file` with its `analog` block: the module item, then the block.
    pub fn module_with_analog(file: &str, name: &str) -> String {
        format!("{}\n{}", BuckSource::item(file, &format!("pub module {name}(")), BuckSource::analog(file, name))
    }

    /// The hand fixture `name`: the structural print of the buck's body, `crates/piperine-ir/tests/golden/buck`.
    pub fn golden(name: &str) -> String {
        let path = format!("{}/../piperine-ir/tests/golden/buck/{name}.txt", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }
}

