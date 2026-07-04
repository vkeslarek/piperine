use crate::parse::ast::*;
use super::attributes::ParseAttributesExt;
use crate::parse::lexer::Tok;
use super::{Parse, Parser};

impl Parse for EnumDecl {
    // ─────────────────────────── §4.2  Enums ─────────────────────────────────

    /// Parses an enum declaration: `enum Name[: Repr] { Variant [= expr], ... }`.
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        let start = parser.current_span_start();
        let attrs = parser.parse_attributes()?;
        let is_pub = parser.eat_ident("pub");
        parser.expect_ident_str("enum")?;
        let name = parser.parse_ident()?;
        let repr = if parser.eat(&Tok::Colon) { Some(Type::parse(parser)?) } else { None };
        parser.expect(&Tok::LBrace)?;
        let mut variants = Vec::new();
        while !parser.eat(&Tok::RBrace) {
            let n = parser.parse_ident()?;
            let value = if parser.eat(&Tok::Assign) { Some(crate::parse::ast::Expr::parse(parser)?) } else { None };
            variants.push(EnumVariant { name: n, value });
            if !parser.eat(&Tok::Comma) {
                parser.expect(&Tok::RBrace)?;
                break;
            }
        }
        let end = parser.previous_span_end();
        let span = Some((start, end - start).into());
        Ok(EnumDecl { span, attrs, is_pub, name, repr, variants })
    }
}
