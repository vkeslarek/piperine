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
        while self.eat(&Tok::At) {
            let name = self.parse_ident()?;
            let mut args = Vec::new();
            if self.eat(&Tok::LParen)
                && !self.eat(&Tok::RParen) {
                    let k = self.parse_ident()?;
                    self.expect(&Tok::Assign)?;
                    let v = crate::parse::ast::Expr::parse(self)?;
                    args.push(AttrArg { name: k, expr: v });
                    while self.eat(&Tok::Comma) {
                        if self.peek() == Some(&Tok::RParen) { break; }
                        let k = self.parse_ident()?;
                        self.expect(&Tok::Assign)?;
                        let v = crate::parse::ast::Expr::parse(self)?;
                        args.push(AttrArg { name: k, expr: v });
                    }
                    self.expect(&Tok::RParen)?;
                }
            attrs.push(Attribute { name, args });
        }
        Ok(attrs)
    }

    fn parse_attr_list(&mut self) -> Result<Vec<AttrArg>, ParseError> {
        let mut attrs = Vec::new();
        if self.eat(&Tok::LParen)
            && !self.eat(&Tok::RParen) {
                let aname = self.parse_ident()?;
                self.expect(&Tok::Assign)?;
                let expr = crate::parse::ast::Expr::parse(self)?;
                attrs.push(AttrArg { name: aname, expr });
                while self.eat(&Tok::Comma) {
                    if self.peek() == Some(&Tok::RParen) {
                        break;
                    }
                    let aname = self.parse_ident()?;
                    self.expect(&Tok::Assign)?;
                    let expr = crate::parse::ast::Expr::parse(self)?;
                    attrs.push(AttrArg { name: aname, expr });
                }
                self.expect(&Tok::RParen)?;
            }
        Ok(attrs)
    }
}
