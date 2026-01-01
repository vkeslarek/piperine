use proc_macro::TokenStream;
use quote::quote;
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Expr, Ident, Result, Token,
};

// Represents: node => value OR RHS => value
struct StampEntry {
    column: syn::Expr, // Changed from Ident to Expr
    _arrow: Token![=>],
    value: syn::Expr,
}

impl Parse for StampEntry {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(StampEntry {
            column: input.parse()?, // Now parses 'self.node_plus' correctly
            _arrow: input.parse()?,
            value: input.parse()?,
        })
    }
}

// Represents: KCL(target): { entries }
struct LawBlock {
    law_type: Ident, // KCL or KVL
    target: Expr,
    entries: Punctuated<StampEntry, Token![,]>,
}

impl Parse for LawBlock {
    fn parse(input: ParseStream) -> Result<Self> {
        let law_type: Ident = input.parse()?;

        let content_paren;
        parenthesized!(content_paren in input);
        let target: Expr = content_paren.parse()?;

        input.parse::<Token![:]>()?;

        let content_brace;
        braced!(content_brace in input);
        let entries = content_brace.parse_terminated(StampEntry::parse, Token![,])?;

        Ok(LawBlock {
            law_type,
            target,
            entries,
        })
    }
}

struct StampsMacroInput {
    blocks: Punctuated<LawBlock, Token![,]>,
}

impl Parse for StampsMacroInput {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(StampsMacroInput {
            blocks: input.parse_terminated(LawBlock::parse, Token![,])?,
        })
    }
}

pub fn stamps_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as StampsMacroInput);
    let mut expansions = Vec::new();

    for block in input.blocks {
        let target = &block.target;
        let row_idx = match block.law_type.to_string().as_str() {
            "KCL" | "KVL" | "Equation" => quote! { (#target).clone() }, // Added Equation
            _ => panic!("Expected KCL, KVL, or Equation"),
        };

        for entry in block.entries {
            let col_expr = &entry.column;
            let val = &entry.value;

            // 1. Check if RHS
            let is_rhs = if let syn::Expr::Path(p) = col_expr {
                p.path.is_ident("RHS")
            } else { false };

            if is_rhs {
                expansions.push(quote! { __stamps.push(Stamp::Rhs(#row_idx, #val)); });
            } else {
                expansions.push(quote! {
                    __stamps.push(Stamp::Matrix(#row_idx, (#col_expr).clone(), #val));
                });
            }
        }
    }

    let expanded = quote! {
        {
            let mut __stamps = Vec::new();
            #(#expansions)*
            __stamps
        }
    };

    TokenStream::from(expanded)
}