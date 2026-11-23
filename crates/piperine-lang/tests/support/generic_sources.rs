//! What the generics targets share (`types_generics`, `types_traits`; `#[path]`-included, never a
//! target itself): MSL sources over the natures every one of them places in the open, checked by
//! `Types::check` from a `Top` or by the whole-model check, and what the check says read back.

use piperine_lang::pom::{DeclId, DeclKind, Design, DesignDiagnostic};
use piperine_lang::types::{Types, UnfinishedForm};

use crate::support;

/// MSL sources of the generics targets and the checks that read them.
pub struct GenericSources;

impl GenericSources {
    /// The natures every source places in the open.
    pub const HEAD: &'static str = "nature E { potential: V { access: v, units: \"V\", abstol: 1u }, flow: I { access: i, units: \"A\", abstol: 1p } }\n";

    /// The binding (a `let`) named `name` of `design`.
    pub fn binding(design: &Design, name: &str) -> DeclId {
        (0..design.decl_count())
            .map(|i| DeclId(i as u32))
            .find(|decl| matches!(design.decl_data(*decl).kind, DeclKind::Binding(_)) && design.decl_data(*decl).name.as_ref().is_some_and(|n| n.text == name))
            .unwrap_or_else(|| panic!("no binding `{name}`"))
    }

    /// The types of the bindings `names` of the `analog` block of `Top` after `items`, as `name = type`,
    /// or the codes and messages of the check.
    pub fn analog_types(items: &str, body: &str, names: &[&str]) -> Result<Vec<String>, Vec<(String, String)>> {
        let design = support::build(&format!("{}{items}\nmodule Top() {{ }}\nanalog for Top {{ {body} }}", Self::HEAD));
        assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}", support::codes(&design));
        match Types::check(&design, "Top") {
            Ok(types) => Ok(names.iter().map(|name| format!("{name} = {}", types.local(Self::binding(&design, name)).map_or_else(|| "?".to_string(), |ty| ty.name(&design)))).collect()),
            Err(found) => Err(found.iter().map(|d| (d.diagnostic.code.to_string(), d.diagnostic.message.clone())).collect()),
        }
    }

    /// The diagnostics of the whole-model check of `src` after the natures, which must find some.
    pub fn model_errors(src: &str) -> Vec<DesignDiagnostic> {
        let design = support::build(&format!("{}{src}", Self::HEAD));
        assert!(design.diagnostics().is_empty(), "the source must parse and resolve: {:?}", support::codes(&design));
        Types::check_model(&design).expect_err("expected diagnostics")
    }

    /// The codes of `found`, in order.
    pub fn codes_of(found: &[DesignDiagnostic]) -> Vec<String> {
        found.iter().map(|d| d.diagnostic.code.to_string()).collect()
    }

    /// The diagnostics of the whole-model check of `src` over the embedded standard library.
    pub fn standard_found(src: &str) -> Vec<DesignDiagnostic> {
        let design = Design::builder().source("top.msl", format!("{}{src}", Self::HEAD)).build();
        Types::check_model(&design).err().unwrap_or_default()
    }

    /// The diagnostics of the whole-model check of `src` over the embedded standard library with `forms` admitted.
    pub fn admitting_found(src: &str, forms: &[UnfinishedForm]) -> Vec<DesignDiagnostic> {
        let design = Design::builder().source("top.msl", format!("{}{src}", Self::HEAD)).build();
        Types::check_model_admitting(&design, forms).err().unwrap_or_default()
    }

    /// The codes of [`GenericSources::standard_found`].
    pub fn standard_codes(src: &str) -> Vec<String> {
        Self::codes_of(&Self::standard_found(src))
    }

    /// The `E0187` messages of [`GenericSources::standard_found`], in order; any other code fails the test.
    pub fn e0187_messages(src: &str) -> Vec<String> {
        let found = Self::standard_found(src);
        assert!(found.iter().all(|d| d.diagnostic.code == "E0187"), "{:?}", found.iter().map(|d| (&d.diagnostic.code, &d.diagnostic.message)).collect::<Vec<_>>());
        found.iter().map(|d| d.diagnostic.message.clone()).collect()
    }
}
