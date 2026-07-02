use crate::parse::lexer::{Lexed, Tok};
use super::{FormatRule, FormatState};

pub struct BlockRule;

impl FormatRule for BlockRule {
    fn before_token(&mut self, t: &Lexed, _next: Option<&Lexed>, state: &mut FormatState, output: &mut String) {
        match &t.tok {
            Tok::Ident(s) => {
                if matches!(s.as_str(), "mod" | "fn" | "discipline" | "bundle" | "enum" | "capability" | "impl" | "const" | "use" | "analog" | "digital") {
                    if state.brace_depth <= 1 && !output.is_empty() {
                        let mut newline_count = 0;
                        for c in output.chars().rev() {
                            if c == '\n' {
                                newline_count += 1;
                            } else if c != ' ' && c != '\t' {
                                break;
                            }
                        }
                        while newline_count < 2 {
                            state.push_newline(output);
                            newline_count += 1;
                        }
                    }
                }
            }
            Tok::RBrace => {
                state.indent_level = state.indent_level.saturating_sub(1);
                state.brace_depth = state.brace_depth.saturating_sub(1);
                if !output.trim_end().ends_with('{') {
                    if !state.at_line_start {
                        state.push_newline(output);
                    }
                }
            }
            _ => {}
        }
    }

    fn after_token(&mut self, t: &Lexed, _next: Option<&Lexed>, state: &mut FormatState, output: &mut String) {
        match &t.tok {
            Tok::LBrace => {
                state.indent_level += 1;
                state.brace_depth += 1;
                if let Some(n) = _next {
                    if n.tok != Tok::RBrace {
                        state.push_newline(output);
                    }
                } else {
                    state.push_newline(output);
                }
            }
            Tok::RBrace => {
                let mut push_nl = true;
                if let Some(n) = _next {
                    if let Tok::Ident(s) = &n.tok {
                        if s == "else" {
                            push_nl = false;
                        }
                    } else if matches!(n.tok, Tok::Semi | Tok::Comma) {
                        push_nl = false;
                    }
                }
                if push_nl {
                    state.push_newline(output);
                }
            }
            _ => {}
        }
    }
}
