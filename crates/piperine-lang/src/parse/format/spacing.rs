use crate::parse::lexer::{Lexed, Tok};
use super::{FormatRule, FormatState};

pub struct SpacingRule;

impl FormatRule for SpacingRule {
    fn space_after(&self, prev: Option<&Lexed>, t: &Lexed, next: Option<&Lexed>, _state: &FormatState) -> Option<bool> {
        if let Some(nxt) = next {
            if matches!(t.tok, Tok::LBrace) && matches!(nxt.tok, Tok::RBrace) {
                return Some(true);
            }
            match nxt.tok {
                Tok::Semi | Tok::Comma | Tok::RParen | Tok::Dot | Tok::Newline => return Some(false),
                Tok::LParen => {
                    if let Tok::Ident(s) = &t.tok {
                        if !matches!(s.as_str(), "if" | "for" | "while" | "match") {
                            return Some(false);
                        }
                    } else if matches!(t.tok, Tok::At) {
                        return Some(false);
                    }
                }
                Tok::RBrack | Tok::LBrack => {
                    // No space before bracket unless it's a type param? Actually `[CONST]` might not need spaces.
                    return Some(false);
                }
                _ => {}
            }
            match t.tok {
                Tok::LParen | Tok::LBrack | Tok::Dot | Tok::At | Tok::Not => return Some(false),
                Tok::Minus | Tok::Plus => {
                    let is_unary = match prev {
                        None => true,
                        Some(p) => {
                            if matches!(p.tok, Tok::LParen | Tok::Comma | Tok::Assign | Tok::EqEq | Tok::NotEq | Tok::Lt | Tok::Gt | Tok::Le | Tok::Ge | Tok::Plus | Tok::Minus | Tok::Star | Tok::Slash | Tok::Percent | Tok::And | Tok::Or | Tok::BitAnd | Tok::BitOr | Tok::BitXor | Tok::Colon | Tok::Contrib | Tok::Force) {
                                true
                            } else if let Tok::Ident(s) = &p.tok {
                                matches!(s.as_str(), "return" | "if" | "while" | "match")
                            } else {
                                false
                            }
                        }
                    };
                    if is_unary {
                        return Some(false);
                    }
                }
                Tok::DoubleColon => return Some(false),
                _ => {}
            }
            if matches!(nxt.tok, Tok::DoubleColon) {
                return Some(false);
            }
        }
        None
    }

    fn after_token(&mut self, t: &Lexed, _next: Option<&Lexed>, state: &mut FormatState, output: &mut String) {
        if let Tok::Semi = &t.tok {
            if state.paren_depth == 0 {
                state.push_newline(output);
            } else {
                output.push(' ');
            }
        }
    }
}
