use crate::parse::ast::*;
use crate::parse::lexer::Tok;
use super::Parser;

impl<'a> Parser<'a> {
    // ─────────────────────────── §4  Types ───────────────────────────────────

    /// Parses a type reference: `Name<Args...>[dim1][dim2]...` or `fn(Args...) -> Ret`.
    pub(crate) fn parse_type(&mut self) -> Result<Type, crate::parse::error::ParseError> {
        let name = self.parse_ident()?;
        let mut args = Vec::new();
        let mut dimensions = Vec::new();

        if name == "fn" && self.peek() == Some(&Tok::LParen) {
            // fn(T, U) -> R
            self.eat(&Tok::LParen);
            if !self.eat(&Tok::RParen) {
                args.push(self.parse_type()?);
                while self.eat(&Tok::Comma) {
                    if self.peek() == Some(&Tok::RParen) {
                        break;
                    }
                    args.push(self.parse_type()?);
                }
                self.expect(&Tok::RParen)?;
            }
            if self.eat(&Tok::Arrow) {
                args.push(self.parse_type()?);
            }
        } else {
            if self.eat(&Tok::Lt) {
                args.push(self.parse_type()?);
                while self.eat(&Tok::Comma) {
                    args.push(self.parse_type()?);
                }
                self.expect(&Tok::Gt)?;
            }
        }

        while self.eat(&Tok::LBrack) {
            dimensions.push(self.parse_expr()?);
            self.expect(&Tok::RBrack)?;
        }

        Ok(Type { name, args, dimensions })
    }
}
