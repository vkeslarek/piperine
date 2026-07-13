use crate::parse::ast::*;
use crate::parse::lexer::Tok;
use super::Parser;

impl<'a> Parser<'a> {
    // ─────────────────────────── §4  Types ───────────────────────────────────

    /// Parses a type reference: `Name<Args...>[dim1][dim2]...?`, `fn(Args...) -> Ret`,
    /// or `(T, U)` for a tuple type.
    pub(crate) fn parse_type(&mut self) -> Result<Type, crate::parse::error::ParseError> {
        // Tuple type: `(T, U, ...)`.
        if self.peek() == Some(&Tok::LParen) {
            self.eat(&Tok::LParen);
            let mut elems = vec![self.parse_type()?];
            while self.eat(&Tok::Comma) {
                if self.peek() == Some(&Tok::RParen) {
                    break;
                }
                elems.push(self.parse_type()?);
            }
            self.expect(&Tok::RParen)?;
            let names: Vec<String> = elems.iter().map(|t| t.name.clone()).collect();
            return Ok(Type {
                name: format!("({})", names.join(",")),
                args: elems,
                dimensions: Vec::new(),
                optional: self.eat(&Tok::Question),
            });
        }

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

        // Trailing `?` marks an optional type (`Real?`).
        let optional = self.eat(&Tok::Question);

        Ok(Type { name, args, dimensions, optional })
    }
}
