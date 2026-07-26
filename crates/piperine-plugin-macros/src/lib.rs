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
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Ident, ItemFn, ItemStruct, LitStr};

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

/// `#[pip::script("name")]` — declare a CLI subcommand (PLG-06): `piperine
/// <name> …` dispatches to the annotated function when no builtin command
/// matches. The function takes the CLI arguments and a `&Ctx`, and returns
/// the process exit code (`Err` surfaces as a plugin script error).
#[proc_macro_attribute]
pub fn script(attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let script_name = match syn::parse::<LitStr>(attr) {
        Ok(lit) => lit,
        Err(err) => {
            let err = err.to_compile_error();
            return quote! { #func #err }.into();
        }
    };
    let name = &func.sig.ident;
    quote! {
        #func

        const _: () = {
            struct __Handler;

            impl ::piperine_plugin::ScriptHandler for __Handler {
                fn invoke(
                    &self,
                    args: &[::std::string::String],
                    cx: &mut ::piperine_plugin::HostCtx,
                ) -> ::core::result::Result<i32, ::std::string::String> {
                    let ctx = ::piperine_plugin::Ctx::new(cx, ::core::option::Option::None);
                    #name(args, &ctx)
                }
            }

            fn __make() -> ::std::boxed::Box<dyn ::piperine_plugin::ScriptHandler> {
                ::std::boxed::Box::new(__Handler)
            }

            ::piperine_plugin::__private::submit! {
                ::piperine_plugin::ScriptRegistration::new(#script_name, __make)
            }
        };
    }
    .into()
}

/// `#[pip::hook(phase)]` — declare a lifecycle hook for one of the five
/// frozen phases (PLG-06/11, D8): `after_parse`, `after_elaborate`,
/// `transform_design`, `before_lower`, `after_solve`. Any other phase name
/// is a compile error. The annotated function's payload matches its phase:
/// `after_parse` receives `(&Ctx, &str)`, the design hooks `&Ctx` (with
/// `ctx.design()`), `transform_design` `(&Ctx, &DesignStaging)`, and
/// `after_solve` `(&Ctx, &SolveResultView)`.
#[proc_macro_attribute]
pub fn hook(attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let phase = match syn::parse::<Ident>(attr) {
        Ok(ident) => ident,
        Err(err) => {
            let err = err.to_compile_error();
            return quote! { #func #err }.into();
        }
    };
    let name = &func.sig.ident;
    let (variant, body) = hook_body(&phase, name);
    let (variant, body) = match (variant, body) {
        (Some(variant), Some(body)) => (variant, body),
        _ => {
            let err = syn::Error::new(
                phase.span(),
                format!(
                    "unknown hook phase `{phase}` — the five frozen phases are: after_parse, \
                     after_elaborate, transform_design, before_lower, after_solve"
                ),
            )
            .to_compile_error();
            return quote! { #func #err }.into();
        }
    };
    quote! {
        #func

        const _: () = {
            fn __invoke(
                call: &::piperine_plugin::HookCall<'_>,
            ) -> ::core::result::Result<(), ::std::string::String> {
                #body
            }

            ::piperine_plugin::__private::submit! {
                ::piperine_plugin::HookRegistration::new(
                    ::piperine_plugin::HookPhase::#variant,
                    __invoke,
                )
            }
        };
    }
    .into()
}

/// The `HookPhase` variant + `__invoke` body for a known phase name, or
/// `(None, None)` for an unknown one (the caller emits the compile error).
fn hook_body(phase: &Ident, name: &Ident) -> (Option<TokenStream2>, Option<TokenStream2>) {
    let design_ctx = quote! {
        let ctx = ::piperine_plugin::Ctx::new(
            call.host,
            ::core::option::Option::Some(
                call.design
                    .ok_or_else(|| ::std::string::String::from("hook fired without a design"))?,
            ),
        );
    };
    match phase.to_string().as_str() {
        "after_parse" => (
            Some(quote!(AfterParse)),
            Some(quote! {
                let ctx = ::piperine_plugin::Ctx::new(call.host, call.design);
                let source = call
                    .source
                    .ok_or_else(|| ::std::string::String::from("after_parse fired without source"))?;
                #name(&ctx, source)
            }),
        ),
        "after_elaborate" => (
            Some(quote!(AfterElaborate)),
            Some(quote! {
                #design_ctx
                #name(&ctx)
            }),
        ),
        "transform_design" => (
            Some(quote!(TransformDesign)),
            Some(quote! {
                #design_ctx
                let staging = call
                    .staging
                    .ok_or_else(|| ::std::string::String::from("transform_design fired without staging"))?;
                #name(&ctx, staging)
            }),
        ),
        "before_lower" => (
            Some(quote!(BeforeLower)),
            Some(quote! {
                #design_ctx
                #name(&ctx)
            }),
        ),
        "after_solve" => (
            Some(quote!(AfterSolve)),
            Some(quote! {
                let ctx = ::piperine_plugin::Ctx::new(call.host, call.design);
                let result = call
                    .result
                    .ok_or_else(|| ::std::string::String::from("after_solve fired without a result"))?;
                #name(&ctx, result)
            }),
        ),
        _ => (None, None),
    }
}
