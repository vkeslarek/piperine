//! Test-fixture plugin for declared-language-surface T25 (DLS-22): a plugin
//! that contributes its own custom attribute schema (`widget_meta`) via
//! `Registrar::attr_schema`, the dynamic runtime path — exercised
//! end-to-end alongside its published `extern.phdl` stub (the *textual*
//! anchor `@widget_meta(...)` actually resolves through) and, in a second
//! test scenario with the stub withheld, to prove `PluginHost::
//! load_for_project` refuses to load it (`PluginError::MissingExternStub`)
//! rather than silently falling back to the dynamic registration below.
//!
//! Deliberately separate from `fixture_plugin.rs` (which contributes no
//! schema) so `native_smoke.rs`/`extern_stub.rs`'s existing, stub-less
//! fixture-plugin projects are unaffected by T25's enforcement.

use piperine_plugin::{entry, Abi, AttrField, Manifest, Permissions, Plugin, Registrar};

pub struct SchemaPlugin {
    manifest: Manifest,
}

impl SchemaPlugin {
    pub fn new() -> Self {
        Self {
            manifest: Manifest {
                name: "schema-fixture".into(),
                abi: Abi::Native,
                entry: String::new(),
                description: Some("test fixture: a plugin-contributed attribute schema".into()),
                permissions: Permissions::default(),
            },
        }
    }
}

impl Default for SchemaPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for SchemaPlugin {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn register(&self, r: &mut Registrar) {
        r.attr_schema(
            "widget_meta",
            vec![AttrField {
                name: "rating".into(),
                ty: "Real".into(),
                required: true,
                default: None,
                decl_span: None,
            }],
        );
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn piperine_plugin_abi_version() -> u32 {
    piperine_plugin::ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn piperine_plugin_entry() -> *mut core::ffi::c_void {
    entry(SchemaPlugin::new())
}
