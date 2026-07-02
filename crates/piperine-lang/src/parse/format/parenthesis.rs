use crate::parse::lexer::{Lexed, Tok};
use super::{FormatRule, FormatState};

pub struct ParenthesisRule;

impl FormatRule for ParenthesisRule {
    fn before_token(&mut self, t: &Lexed, state: &mut FormatState, _output: &mut String) {
        match t.tok {
            Tok::LParen => state.paren_depth += 1,
            Tok::RParen => state.paren_depth = state.paren_depth.saturating_sub(1),
            _ => {}
        }
    }
}
