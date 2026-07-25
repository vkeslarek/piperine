//! The `#[pip::…]` declaration-coupled plugin surface (plugin-interface v2,
//! PLG-05/06/24): every contribution is declared at the point of
//! contribution — a device is an attribute on its `Element` type, a script
//! or hook an attribute on its function — with no imperative `register()`
//! body. Expansions submit declaration records into the plugin binary's
//! registry (`piperine_plugin::Registry`); the host reads them through
//! `Plugin::collect` at load.
//!
//! Depend on this crate under the name `pip` so the attributes spell
//! exactly like the Python decorators (MD-22 literal parity):
//!
//! ```toml
//! pip = { package = "piperine-plugin-macros", version = "…" }
//! ```

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemStruct, LitStr};

/// `#[pip::device("Type")]` — declare a solver device (PLG-05). Applied to
/// an `Element` type implementing `piperine_plugin::PluginDevice`; the
/// expansion generates the `DeviceFactory` adapter and submits a
/// `piperine_plugin::DeviceRegistration` keyed by the type id — the same
/// id `@device(type = "Type")` names in the plugin's PHDL.
#[proc_macro_attribute]
pub fn device(attr: TokenStream, item: TokenStream) -> TokenStream {
    let strukt = parse_macro_input!(item as ItemStruct);
    let type_id = match syn::parse::<LitStr>(attr) {
        Ok(lit) => lit,
        Err(err) => {
            let err = err.to_compile_error();
            return quote! { #strukt #err }.into();
        }
    };
    let name = &strukt.ident;
    quote! {
        #strukt

        const _: () = {
            struct __Factory;

            impl ::piperine_plugin::DeviceFactory for __Factory {
                fn kind(&self) -> ::piperine_plugin::DeviceKind {
                    <#name as ::piperine_plugin::PluginDevice>::KIND
                }

                fn instantiate(
                    &self,
                    spec: &::piperine_plugin::PluginDeviceSpec,
                ) -> ::core::result::Result<
                    ::std::boxed::Box<dyn ::piperine_plugin::Element>,
                    ::std::string::String,
                > {
                    ::core::result::Result::Ok(::std::boxed::Box::new(
                        <#name as ::piperine_plugin::PluginDevice>::from_spec(spec)?,
                    ))
                }
            }

            fn __make() -> ::std::boxed::Box<dyn ::piperine_plugin::DeviceFactory> {
                ::std::boxed::Box::new(__Factory)
            }

            ::piperine_plugin::__private::submit! {
                ::piperine_plugin::DeviceRegistration::new(#type_id, __make)
            }
        };
    }
    .into()
}
