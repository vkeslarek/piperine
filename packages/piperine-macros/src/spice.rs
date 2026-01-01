use proc_macro::TokenStream;
use syn::parse::{Parse, ParseStream};

pub fn spice_impl(input: TokenStream) -> TokenStream {
    // TODO
    input
}

mod keywords {
    syn::custom_keyword!(subckt);
    syn::custom_keyword!(ends);
}

enum SpiceStatement {
    SubCircuit {
        name: syn::Ident,
        params: Vec<(syn::Ident, Option<syn::Lit>)>,
        body: Vec<SpiceStatement>,
    },
}

#[derive(Debug)]
struct SpiceFile {}

impl Parse for SpiceFile {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        Ok(SpiceFile {})
    }
}
