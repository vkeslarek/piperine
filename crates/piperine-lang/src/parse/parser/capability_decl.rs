use crate::parse::ast::*;
use super::attributes::ParseAttributesExt;
use crate::parse::lexer::Tok;
use super::{Parse, Parser};

impl Parse for CapabilityDecl {
    // ─────────────────────────── §5  Capabilities ────────────────────────────

    /// Parses a capability declaration: `capability Name[: Super, ...] { fn sig; | fn decl { } }`.
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
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
            if parser.eat_ident("fn") {
                let sig = FnSig::parse(parser)?;
                if parser.eat(&Tok::Semi) {
                    items.push(CapItem::FnSig(sig));
                } else {
                    let body = parser.parse_block()?;
                    items.push(CapItem::FnDecl(FnDecl { attrs: parser.parse_attributes()?, is_pub: false, sig, body }));
                }
            } else {
                return Err("Expected `fn` inside capability".into());
            }
        }
        Ok(CapabilityDecl { attrs, is_pub, name, supers, items })
    }
}
