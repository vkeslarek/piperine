use crate::parse::ast::{AttrArg, Attribute};
use crate::parse::parser::{Parse, Parser};
use crate::parse::lexer::Tok;
use crate::parse::error::ParseError;

pub trait ParseAttributesExt {
    fn parse_attributes(&mut self) -> Result<Vec<Attribute>, ParseError>;
    fn parse_attr_list(&mut self) -> Result<Vec<AttrArg>, ParseError>;
}

impl<'a> ParseAttributesExt for Parser<'a> {
    fn parse_attributes(&mut self) -> Result<Vec<Attribute>, ParseError> {
        let mut attrs = Vec::new();
        while self.peek() == Some(&Tok::At) {
            let attr_start = self.current_span_start();
            self.eat(&Tok::At);
            let name = self.parse_ident()?;
            let mut args = Vec::new();
            if self.eat(&Tok::LParen)
                && !self.eat(&Tok::RParen) {
                    args.push(self.parse_attr_arg()?);
                    while self.eat(&Tok::Comma) {
                        if self.peek() == Some(&Tok::RParen) { break; }
                        args.push(self.parse_attr_arg()?);
                    }
                    self.expect(&Tok::RParen)?;
                }
            let attr_end = self.previous_span_end();
            attrs.push(Attribute { name, args, span: Some((attr_start, attr_end - attr_start).into()) });
        }
        Ok(attrs)
    }

    fn parse_attr_list(&mut self) -> Result<Vec<AttrArg>, ParseError> {
        let mut attrs = Vec::new();
        if self.eat(&Tok::LParen)
            && !self.eat(&Tok::RParen) {
                attrs.push(self.parse_attr_arg()?);
                while self.eat(&Tok::Comma) {
                    if self.peek() == Some(&Tok::RParen) {
                        break;
                    }
                    attrs.push(self.parse_attr_arg()?);
                }
                self.expect(&Tok::RParen)?;
            }
        Ok(attrs)
    }
}

impl<'a> Parser<'a> {
    /// One `name = expr` attribute argument, with its own span (T19/LSP-21).
    fn parse_attr_arg(&mut self) -> Result<AttrArg, ParseError> {
        let start = self.current_span_start();
        let name = self.parse_ident()?;
        self.expect(&Tok::Assign)?;
        let expr = crate::parse::ast::Expr::parse(self)?;
        let end = self.previous_span_end();
        Ok(AttrArg { name, expr, span: Some((start, end - start).into()) })
    }
}
