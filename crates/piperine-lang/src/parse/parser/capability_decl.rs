use crate::parse::ast::*;
use super::attributes::ParseAttributesExt;
use crate::parse::lexer::Tok;
use super::{Parse, Parser};

impl Parse for CapabilityDecl {
    // ─────────────────────────── §5  Capabilities ────────────────────────────

    /// Parses a capability declaration: `capability Name[: Super, ...] { fn sig; | fn decl { } }`.
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        let start = parser.current_span_start();
        let attrs = parser.parse_attributes()?;
        let is_pub = parser.eat_ident("pub");
        parser.expect_ident_str("capability")?;
        let name = parser.parse_ident()?;
        let mut supers = Vec::new();
        if parser.eat(&Tok::Colon) {
            supers.push(parser.parse_ident()?);
            while parser.eat(&Tok::Comma) {
                supers.push(parser.parse_ident()?);
            }
        }
        parser.expect(&Tok::LBrace)?;
        let mut items = Vec::new();
        while !parser.eat(&Tok::RBrace) {
            let fn_start = parser.current_span_start();
            if parser.eat_ident("fn") {
                let sig = FnSig::parse(parser)?;
                if parser.eat(&Tok::Semi) {
                    items.push(CapItem::FnSig(sig));
                } else {
                    let body = parser.parse_block()?;
                    let fn_end = parser.previous_span_end();
                    let fn_span = Some((fn_start, fn_end - fn_start).into());
                    items.push(CapItem::FnDecl(FnDecl { span: fn_span, attrs: parser.parse_attributes()?, is_pub: false, sig, body }));
                }
            } else {
                return Err("Expected `fn` inside capability".into());
            }
        }
        let end = parser.previous_span_end();
        let span = Some((start, end - start).into());
        Ok(CapabilityDecl { span, attrs, is_pub, name, supers, items })
    }
}
