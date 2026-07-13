use crate::parse::ast::*;
use super::attributes::ParseAttributesExt;
use crate::parse::lexer::Tok;
use super::{Parse, Parser};

impl Parse for Item {
    fn parse(parser: &mut Parser) -> Result<Self, crate::parse::error::ParseError> {
        let mut fork = parser.clone();
        let attrs = fork.parse_attributes()?;
        
        if fork.eat_ident("use") {
            if !attrs.is_empty() { return Err("Attributes not allowed on `use`".into()); }
            parser.parse_attributes()?;
            parser.eat_ident("use");
            let path = parser.parse_path()?;
            parser.expect(&Tok::Semi)?;
            return Ok(Item::UseDecl(path));
        }

        let _ = fork.eat_ident("pub");
        let _ = fork.eat_ident("extern");

        let ident = match fork.peek() {
            Some(Tok::Ident(s)) => s.as_str(),
            _ => return Err(format!("Unknown top-level item at {:?}", fork.peek()).into()),
        };

        match ident {
            "mod" => Ok(Item::ModuleDeclaration(ModuleDeclaration::parse(parser)?)),
            "analog" | "digital" => Ok(Item::BehaviorDecl(BehaviorDecl::parse(parser)?)),
            "discipline" => Ok(Item::DisciplineDecl(DisciplineDecl::parse(parser)?)),
            "bundle" => Ok(Item::BundleDecl(BundleDecl::parse(parser)?)),
            "enum" => Ok(Item::EnumDecl(EnumDecl::parse(parser)?)),
            "capability" => Ok(Item::CapabilityDecl(CapabilityDecl::parse(parser)?)),
            "impl" => Ok(Item::ImplDecl(ImplDecl::parse(parser)?)),
            "fn" => {
                // Consume `pub` / `extern` in the real parser, then parse the fn.
                parser.parse_attributes()?;
                let is_pub = parser.eat_ident("pub");
                let is_extern = parser.eat_ident("extern");
                Ok(Item::FnDecl(FnDecl::parse_with_extern(parser, is_pub, is_extern)?))
            }
            "const" => Ok(Item::ConstDecl(ConstDecl::parse(parser)?)),
            "bench" => Ok(Item::BenchDecl(BenchDecl::parse(parser)?)),
            _ => Err(format!("Unknown top-level item: {}", ident).into()),
        }
    }
}
