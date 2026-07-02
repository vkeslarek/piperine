use crate::parse::ast::*;
use super::attributes::ParseAttributesExt;
use crate::parse::lexer::Tok;
use super::{Parse, Parser};

impl Parse for DisciplineDecl {
    // ─────────────────────────── §4.1  Disciplines ───────────────────────────

    /// Parses a discipline declaration: `discipline Name { potential/flow/storage/resolve ... }`.
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        let attrs = parser.parse_attributes()?;
        let is_pub = parser.eat_ident("pub");
        parser.expect_ident_str("discipline")?;
        let name = parser.parse_ident()?;
        parser.expect(&Tok::LBrace)?;
        let mut items = Vec::new();
        while !parser.eat(&Tok::RBrace) {
            if parser.eat_ident("potential") {
                let n = parser.parse_ident()?;
                parser.expect(&Tok::Colon)?;
                let ty = Type::parse(parser)?;
                let attrs = parser.parse_attr_list()?;
                parser.expect(&Tok::Semi)?;
                items.push(DisciplineItem::Nature {
                    kind: NatureKind::Potential,
                    name: n,
                    ty,
                    attrs,
                });
            } else if parser.eat_ident("flow") {
                let n = parser.parse_ident()?;
                parser.expect(&Tok::Colon)?;
                let ty = Type::parse(parser)?;
                let attrs = parser.parse_attr_list()?;
                parser.expect(&Tok::Semi)?;
                items.push(DisciplineItem::Nature { kind: NatureKind::Flow, name: n, ty, attrs });
            } else if parser.eat_ident("storage") {
                let ty = Type::parse(parser)?;
                parser.expect(&Tok::Semi)?;
                items.push(DisciplineItem::Storage(ty));
            } else if parser.eat_ident("resolve") {
                let r = if parser.eat_ident("tri") {
                    ResolveKind::Tri
                } else if parser.eat_ident("or") {
                    ResolveKind::Or
                } else if parser.eat_ident("and") {
                    ResolveKind::And
                } else {
                    return Err("Unknown resolve kind (expected tri/or/and)".into());
                };
                parser.expect(&Tok::Semi)?;
                items.push(DisciplineItem::Resolve(r));
            } else {
                return Err("Unknown discipline item".into());
            }
        }
        Ok(DisciplineDecl { attrs, is_pub, name, items })
    }
}
