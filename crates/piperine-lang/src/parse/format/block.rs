use crate::parse::lexer::{Lexed, Tok};
use super::{FormatRule, FormatState};

pub struct BlockRule;

impl FormatRule for BlockRule {
    fn before_token(&mut self, t: &Lexed, _next: Option<&Lexed>, state: &mut FormatState, output: &mut String) {
        match &t.tok {
            Tok::Ident(s) => {
                if matches!(s.as_str(), "pub" | "mod" | "fn" | "discipline" | "bundle" | "enum" | "capability" | "impl" | "const" | "use" | "analog" | "digital")
                    && state.brace_depth <= 1 && !output.is_empty() {
                        // A pre-existing declaration keyword (`mod`, `fn`, …)
                        // that directly continues a `pub` on the SAME line
                        // (`pub mod Foo`) is not a fresh item boundary — `pub`
                        // already is (or would be) the trigger for this same
                        // declaration. Forcing a blank line here would split
                        // `pub` from its keyword onto separate lines.
                        let continues_pub_on_this_line = output
                            .trim_end_matches([' ', '\t'])
                            .rsplit('\n')
                            .next()
                            .is_some_and(|line| line.trim() == "pub");
                        if continues_pub_on_this_line {
                            return;
                        }

                        // A `///` doc comment directly above this declaration
                        // must stay directly above it — the lexer drops a
                        // pending doc run on a blank line. Detect it BEFORE
                        // touching `output`: strip every trailing newline/
                        // space/tab (however many blank lines already
                        // accumulated — source-authored or left over from a
                        // previous, buggy formatter run) and check whether
                        // the line left behind is a `///` line.
                        let trimmed = output.trim_end_matches(['\n', ' ', '\t']);
                        let preceded_by_doc_comment = trimmed
                            .rsplit('\n')
                            .next()
                            .is_some_and(|line| line.trim_start().starts_with("///"));
                        if preceded_by_doc_comment {
                            // Collapse back to exactly one newline — heals
                            // an already-blank-separated doc comment, not
                            // just avoids introducing a new one.
                            output.truncate(trimmed.len());
                            state.push_newline(output);
                        } else {
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
                if !output.trim_end().ends_with('{')
                    && !state.at_line_start {
                        state.push_newline(output);
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
