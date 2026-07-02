use crate::parse::lexer::{Lexed, Tok};
use super::{FormatRule, FormatState};

pub struct BlockRule;

impl FormatRule for BlockRule {
    fn before_token(&mut self, t: &Lexed, state: &mut FormatState, output: &mut String) {
        match &t.tok {
            Tok::Ident(s) => {
                if !state.at_line_start && matches!(s.as_str(), "mod" | "fn" | "discipline" | "bundle" | "enum" | "capability" | "impl" | "const" | "use") {
                    if state.brace_depth == 0 {
                        state.push_newline(output);
                    }
                }
            }
            Tok::RBrace => {
                state.indent_level = state.indent_level.saturating_sub(1);
                state.brace_depth = state.brace_depth.saturating_sub(1);
                if !state.at_line_start {
                    state.push_newline(output);
                }
            }
            _ => {}
        }
    }

    fn after_token(&mut self, t: &Lexed, state: &mut FormatState, output: &mut String) {
        match &t.tok {
            Tok::LBrace => {
                state.indent_level += 1;
                state.brace_depth += 1;
                state.push_newline(output);
            }
            Tok::RBrace => {
                state.push_newline(output);
            }
            _ => {}
        }
    }
}
