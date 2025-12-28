mod spice;

use proc_macro::TokenStream;
use syn::parse_macro_input;
use crate::spice::SpiceFile;

#[proc_macro]
pub fn spice(input: TokenStream) -> TokenStream {
    // let input_cln = input.clone();
    // let circuit = parse_macro_input!(input as SpiceFile);

    input
}