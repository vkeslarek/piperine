//! A fragment of MSL wrapped into the smallest context that parses it, shared by the printer and formatter targets.

use piperine_lang::msl::parser::SourceParser;

/// A fragment of MSL text wrapped into the smallest context that parses it.
pub struct FragmentSource;

impl FragmentSource {
    /// A fragment wrapped into the smallest context that parses it with no
    /// diagnostics: a file as it is, a module body, an `analog` body, a reaction
    /// body or a function body. `None` when no context accepts it.
    pub fn wrapped(text: &str) -> Option<String> {
        [
            text.to_string(),
            format!("module Fragment() {{\n{text}\n}}"),
            format!("analog for Fragment {{\n{text}\n}}"),
            format!("digital for Fragment {{\non event {{\n{text}\n}}\n}}"),
            format!("fn fragment() {{\n{text}\n}}"),
        ]
        .into_iter()
        .find(|src| SourceParser::parse(src).diagnostics.is_empty())
    }
}
