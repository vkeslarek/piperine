use crate::parse::ast::*;
use super::attributes::ParseAttributesExt;
use crate::parse::lexer::Tok;
use super::{Parse, Parser};

impl Parse for ModuleDeclaration {
    /// Parses a `mod Name[CONST]<TYPE>(PORTS) { body }` or `mod Name[CONST]<TYPE>(PORTS);` declaration.
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        let attrs = parser.parse_attributes()?;
        let is_pub = parser.eat_ident("pub");
        parser.expect_ident_str("mod")?;
        let name = parser.parse_ident()?;

        let mut const_params = Vec::new();
        if parser.eat(&Tok::LBrack) {
            const_params.push(parser.parse_ident()?);
            while parser.eat(&Tok::Comma) {
                const_params.push(parser.parse_ident()?);
            }
            parser.expect(&Tok::RBrack)?;
        }

        let mut type_params = Vec::new();
        if parser.eat(&Tok::Lt) {
            type_params.push(TypeParam::parse(parser)?);
            while parser.eat(&Tok::Comma) {
                type_params.push(TypeParam::parse(parser)?);
            }
            parser.expect(&Tok::Gt)?;
        }

        let mut ports = Vec::new();
        if parser.eat(&Tok::LParen) {
            if !parser.eat(&Tok::RParen) {
                ports.push(Port::parse(parser)?);
                while parser.eat(&Tok::Comma) {
                    if parser.peek() == Some(&Tok::RParen) {
                        break;
                    }
                    ports.push(Port::parse(parser)?);
                }
                parser.expect(&Tok::RParen)?;
            }
        }

        let mut body = Vec::new();
        if parser.eat(&Tok::LBrace) {
            while !parser.eat(&Tok::RBrace) {
                body.push(ModuleStatement::parse(parser)?);
            }
        } else {
            parser.expect(&Tok::Semi)?;
        }

        Ok(ModuleDeclaration { attrs, is_pub, name, const_params, type_params, ports, body })
    }
}

impl Parse for TypeParam {
    /// Parses a generic type parameter: `Name` or `Name: Cap1 + Cap2 + ...`.
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        let name = parser.parse_ident()?;
        let mut bounds = Vec::new();
        if parser.eat(&Tok::Colon) {
            bounds.push(parser.parse_ident()?);
            while parser.eat(&Tok::Plus) {
                bounds.push(parser.parse_ident()?);
            }
        }
        Ok(TypeParam { name, bounds })
    }
}

impl Parse for Port {
    /// Parses a module port: `direction name : type`.
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        let attrs = parser.parse_attributes()?;
        let direction = if parser.eat_ident("input") {
            Direction::Input
        } else if parser.eat_ident("output") {
            Direction::Output
        } else if parser.eat_ident("inout") {
            Direction::Inout
        } else {
            return Err("Expected port direction (input/output/inout)".into());
        };
        let name = parser.parse_ident()?;
        parser.expect(&Tok::Colon)?;
        let ty = Type::parse(parser)?;
        Ok(Port { attrs, direction, name, ty })
    }
}
