use crate::parse::ast::*;
use super::attributes::ParseAttributesExt;
use crate::parse::lexer::Tok;
use super::{Parse, Parser};

impl Parse for ImplDecl {
    /// Parses an impl block: `impl [Capability for] Type[CONST]<TYPE> { fn ... }`.
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        let attrs = parser.parse_attributes()?;
        let is_pub = parser.eat_ident("pub");
        parser.expect_ident_str("impl")?;
        let mut ident1 = parser.parse_ident()?;
        let mut capability = None;
        if parser.eat_ident("for") {
            capability = Some(ident1);
            ident1 = parser.parse_ident()?;
        }
        let mut const_args = Vec::new();
        if parser.eat(&Tok::LBrack) {
            const_args.push(crate::parse::ast::Expr::parse(parser)?);
            while parser.eat(&Tok::Comma) {
                const_args.push(crate::parse::ast::Expr::parse(parser)?);
            }
            parser.expect(&Tok::RBrack)?;
        }
        let mut type_args = Vec::new();
        if parser.eat(&Tok::Lt) {
            type_args.push(Type::parse(parser)?);
            while parser.eat(&Tok::Comma) {
                type_args.push(Type::parse(parser)?);
            }
            parser.expect(&Tok::Gt)?;
        }
        parser.expect(&Tok::LBrace)?;
        let mut methods = Vec::new();
        while !parser.eat(&Tok::RBrace) {
            let _ = parser.parse_attributes()?;
            if let Some(crate::parse::lexer::Tok::Ident(s)) = parser.peek() {
                if s == "fn" || s == "pub" {
                    methods.push(FnDecl::parse(parser)?);
                    continue;
                }
            }
            return Err(format!("Expected `fn`, found {:?}", parser.peek()).into());
        }
        Ok(ImplDecl { attrs, is_pub, capability, ty: ident1, const_args, type_args, methods })
    }
}
