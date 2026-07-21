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
use super::Parser;

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
