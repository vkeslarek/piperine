mod spice;
mod stamp;

use crate::spice::spice_impl;
use crate::stamp::stamps_impl;
use proc_macro::TokenStream;

#[proc_macro]
pub fn spice(input: TokenStream) -> TokenStream {
    spice_impl(input)
}

#[proc_macro]
pub fn stamps(input: TokenStream) -> TokenStream {
    stamps_impl(input)
}
