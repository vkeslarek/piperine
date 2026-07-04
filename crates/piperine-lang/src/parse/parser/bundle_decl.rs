use crate::parse::ast::*;
use super::attributes::ParseAttributesExt;
use crate::parse::lexer::Tok;
use super::{Parse, Parser};

impl Parse for BundleDecl {
    // ─────────────────────────── §4.3  Bundles ───────────────────────────────

    /// Parses a bundle declaration: `bundle Name[CONST]<TYPE> { field: Type [= default], ... }`.
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        let start = parser.current_span_start();
        let attrs = parser.parse_attributes()?;
        let is_pub = parser.eat_ident("pub");
        parser.expect_ident_str("bundle")?;
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
        parser.expect(&Tok::LBrace)?;
        let mut fields = Vec::new();
        while !parser.eat(&Tok::RBrace) {
            let n = parser.parse_ident()?;
            parser.expect(&Tok::Colon)?;
            let ty = Type::parse(parser)?;
            let default = if parser.eat(&Tok::Assign) { Some(crate::parse::ast::Expr::parse(parser)?) } else { None };
            fields.push(FieldDecl { attrs: parser.parse_attributes()?, name: n, ty, default });
            if !parser.eat(&Tok::Comma) {
                parser.expect(&Tok::RBrace)?;
                break;
            }
        }
        let end = parser.previous_span_end();
        let span = Some((start, end - start).into());
        Ok(BundleDecl { span, attrs, is_pub, name, const_params, type_params, fields })
    }
}
