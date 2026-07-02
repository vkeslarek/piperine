use crate::parse::lexer::{Lexed, Tok};
use super::{FormatRule, FormatState};

pub struct CommentRule;

impl FormatRule for CommentRule {
    fn before_token(&mut self, t: &Lexed, state: &mut FormatState, output: &mut String) {
        if let Tok::LineComment = &t.tok {
            if !state.at_line_start {
                output.push(' ');
            }
        }
    }
    
    fn space_after(&self, _prev: Option<&Lexed>, t: &Lexed, _next: Option<&Lexed>, _state: &FormatState) -> Option<bool> {
        if matches!(t.tok, Tok::LineComment | Tok::BlockComment) {
            return Some(false);
        }
        None
    }
}
