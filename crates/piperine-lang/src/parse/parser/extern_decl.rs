//! # `extern` declarations
//!
//! Parses the six `extern`-modified declaration forms (SPEC "declared
//! language surface" P2): `extern type`, `extern fn`, `extern task`,
//! `extern operator`, `extern attribute`, `extern impl`. Every form is
//! signature-only — a body on any of them (or on an individual method
//! inside `extern impl`) is a parse error naming the offending declaration.
//!
//! Dispatched from [`super::item`]'s `Item::parse`, which peeks the
//! `extern` modifier before delegating here.

use crate::parse::ast::*;
use crate::parse::error::ParseError;
use crate::parse::lexer::Tok;
use super::{Parse, Parser};

/// `extern type Name;`
pub(crate) fn parse_extern_type(parser: &mut Parser) -> Result<ExternDecl, ParseError> {
    let start = parser.current_span_start();
    parser.expect_ident_str("extern")?;
    parser.expect_ident_str("type")?;
    let name = parser.parse_ident()?;
    if parser.peek() == Some(&Tok::LBrace) {
        return Err(parser.make_error(format!(
            "`extern type {name}` may not have a body — extern declarations are signature-only"
        )));
    }
    parser.expect(&Tok::Semi)?;
    let end = parser.previous_span_end();
    Ok(ExternDecl::Type { span: Some((start, end - start).into()), name })
}

/// `extern fn name(params) -> RetType;`
pub(crate) fn parse_extern_fn(parser: &mut Parser) -> Result<ExternDecl, ParseError> {
    let start = parser.current_span_start();
    parser.expect_ident_str("extern")?;
    parser.expect_ident_str("fn")?;
    let name = parser.parse_ident()?;
    let (params, ret) = parse_sig_tail(parser)?;
    if parser.peek() == Some(&Tok::LBrace) {
        return Err(parser.make_error(format!(
            "`extern fn {name}` may not have a body — extern declarations are signature-only"
        )));
    }
    parser.expect(&Tok::Semi)?;
    let end = parser.previous_span_end();
    Ok(ExternDecl::Fn(ExternSig { span: Some((start, end - start).into()), name, params, ret }))
}

/// Parses `(params) -> RetType` — the tail shared by every `extern`
/// signature form (`fn`/`task`/`operator`, and each `extern impl` method).
/// Mirrors [`FnSig::parse`]'s parameter-list grammar (no defaults — not
/// part of the `extern` surface).
pub(crate) fn parse_sig_tail(parser: &mut Parser) -> Result<(Vec<FnParam>, Type), ParseError> {
    parser.expect(&Tok::LParen)?;
    let mut params = Vec::new();
    if !parser.eat(&Tok::RParen) {
        params.push(parse_extern_param(parser)?);
        while parser.eat(&Tok::Comma) {
            if parser.peek() == Some(&Tok::RParen) {
                break;
            }
            params.push(parse_extern_param(parser)?);
        }
        parser.expect(&Tok::RParen)?;
    }
    let ret = if parser.eat(&Tok::Arrow) {
        Type::parse(parser)?
    } else {
        Type { name: "Unit".into(), args: Vec::new(), dimensions: Vec::new(), optional: false }
    };
    Ok((params, ret))
}

fn parse_extern_param(parser: &mut Parser) -> Result<FnParam, ParseError> {
    if parser.eat_ident("self") {
        return Ok(FnParam::SelfParam);
    }
    let name = parser.parse_ident()?;
    parser.expect(&Tok::Colon)?;
    let ty = Type::parse(parser)?;
    Ok(FnParam::Typed { name, ty, default: None })
}
