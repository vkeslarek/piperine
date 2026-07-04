use crate::parse::ast::*;
use super::attributes::ParseAttributesExt;
use crate::parse::lexer::Tok;
use super::{Parse, Parser};

impl Parse for BenchDecl {
    /// Parses `bench Name { fn ... }` (piperine-bench/docs/SPEC.md §2) — modeled on
    /// [`ImplDecl`][super::impl_decl], since a bench body is the same
    /// `fn`-only grammar.
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        let attrs = parser.parse_attributes()?;
        let is_pub = parser.eat_ident("pub");
        parser.expect_ident_str("bench")?;
        let name = parser.parse_ident()?;
        parser.expect(&Tok::LBrace)?;
        let mut fns = Vec::new();
        while !parser.eat(&Tok::RBrace) {
            let _ = parser.parse_attributes()?;
            if let Some(Tok::Ident(s)) = parser.peek()
                && (s == "fn" || s == "pub") {
                    fns.push(FnDecl::parse(parser)?);
                    continue;
                }
            return Err(format!("Expected `fn`, found {:?}", parser.peek()).into());
        }
        Ok(BenchDecl { span: None, attrs, is_pub, name, fns })
    }
}
