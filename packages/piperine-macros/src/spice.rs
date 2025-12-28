use syn::parse::{Parse, ParseStream};

mod keywords {
    syn::custom_keyword!(subckt);
    syn::custom_keyword!(ends);
}

pub enum SpiceStatement {
    SubCircuit {
        name: syn::Ident,
        params: Vec<(syn::Ident, Option<syn::Lit>)>,
        body: Vec<SpiceStatement>,
    },
}

#[derive(Debug)]
pub struct SpiceFile {}

impl Parse for SpiceFile {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        Ok(SpiceFile {})
    }
}
