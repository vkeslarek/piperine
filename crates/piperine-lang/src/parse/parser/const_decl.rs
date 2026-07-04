use crate::parse::ast::*;
use super::attributes::ParseAttributesExt;
use crate::parse::lexer::Tok;
use super::{Parse, Parser};

impl Parse for ConstDecl {
    /// Parses a global const declaration: `const Name : Type = Expr;`.
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        let attrs = parser.parse_attributes()?;
        let is_pub = parser.eat_ident("pub");
        parser.expect_ident_str("const")?;
        let name = parser.parse_ident()?;
        parser.expect(&Tok::Colon)?;
        let ty = Type::parse(parser)?;
        parser.expect(&Tok::Assign)?;
        let value = crate::parse::ast::Expr::parse(parser)?;
        parser.expect(&Tok::Semi)?;
        Ok(ConstDecl { span: None, attrs, is_pub, name, ty, value })
    }
}
