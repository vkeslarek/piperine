use crate::parse::lexer::{Lexed, Tok};
use super::{FormatRule, FormatState};

pub struct BlankLineRule;

impl FormatRule for BlankLineRule {
    fn consume_token(&mut self, t: &Lexed, prev: Option<&Lexed>, state: &mut FormatState, output: &mut String) -> bool {
        if t.tok == Tok::Newline {
            if let Some(p) = prev {
                if matches!(p.tok, Tok::LineComment) || matches!(p.tok, Tok::BlockComment) {
                    state.force_newline(output);
                    return true;
                }
                
                if p.tok == Tok::Newline {
                    let double_newline = format!("{}{}", state.options.line_ending, state.options.line_ending);
                    if !output.ends_with(&double_newline) {
                        state.force_newline(output);
                    }
                }
            }
            return true;
        }
        false
    }
}
