use crate::parse::ast::*;
use super::attributes::ParseAttributesExt;
use crate::parse::lexer::Tok;
use super::{Parse, Parser};

impl Parse for FnSig {
    // ─────────────────────────── §6  Functions ───────────────────────────────

    /// Parses a function signature (without body): `fn name<TYPE>(params) -> RetType`.
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        let name = parser.parse_ident()?;
        let mut type_params = Vec::new();
        if parser.eat(&Tok::Lt) {
            type_params.push(TypeParam::parse(parser)?);
            while parser.eat(&Tok::Comma) {
                type_params.push(TypeParam::parse(parser)?);
            }
            parser.expect(&Tok::Gt)?;
        }
        parser.expect(&Tok::LParen)?;
        let mut params = Vec::new();
        let mut saw_default = false;
        if !parser.eat(&Tok::RParen) {
            if parser.eat_ident("self") {
                params.push(FnParam::SelfParam);
            } else {
                let n = parser.parse_ident()?;
                parser.expect(&Tok::Colon)?;
                let ty = Type::parse(parser)?;
                let default = if parser.eat(&Tok::Assign) {
                    saw_default = true;
                    Some(parser.parse_expr()?)
                } else {
                    None
                };
                params.push(FnParam::Typed { name: n, ty, default });
            }
            while parser.eat(&Tok::Comma) {
                if parser.peek() == Some(&Tok::RParen) {
                    break;
                }
                let n = parser.parse_ident()?;
                parser.expect(&Tok::Colon)?;
                let ty = Type::parse(parser)?;
                // SPEC_BENCH.md §10: defaults are trailing-only — a
                // non-defaulted parameter cannot follow a defaulted one.
                let default = if parser.eat(&Tok::Assign) {
                    saw_default = true;
                    Some(parser.parse_expr()?)
                } else if saw_default {
                    return Err(crate::parse::error::ParseError::from(
                        "a non-defaulted parameter cannot follow a defaulted one (defaults must be trailing)",
                    ));
                } else {
                    None
                };
                params.push(FnParam::Typed { name: n, ty, default });
            }
            parser.expect(&Tok::RParen)?;
        }
        // `-> RetType` is optional; an omitted return type is `Unit` (the
        // common case for a `bench` entry point, which is a procedure, not
        // a value computation — SPEC_BENCH.md §2).
        let ret = if parser.eat(&Tok::Arrow) {
            Type::parse(parser)?
        } else {
            Type { name: "Unit".into(), args: Vec::new(), dimensions: Vec::new() }
        };
        Ok(FnSig { name, type_params, params, ret })
    }
}

impl Parse for FnDecl {
    /// Parses a full function declaration: `fn name<TYPE>(params) -> RetType { body }`.
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        let attrs = parser.parse_attributes()?;
        let is_pub = parser.eat_ident("pub");
        parser.expect_ident_str("fn")?;
        let sig = FnSig::parse(parser)?;
        let body = parser.parse_block()?;
        Ok(FnDecl { attrs, is_pub, sig, body })
    }
}
