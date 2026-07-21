//! The shared host-side adapter for wire-protocol plugin tiers (WASM,
//! process): a [`WireTransport`] moves `HookInput` payloads to the guest
//! and replies back; [`WireHosted`] presents the guest as an
//! ordinary [`Plugin`]. Everything above the transport — registration,
//! patch application, read-only enforcement — is identical across tiers.
//!
//! What crosses the boundary is the real POM: `Design` and `Value`
//! serialize as themselves (serde on the POM types, SPEC Part IV §7) —
//! there is no second model.

use std::sync::Arc;

use piperine_lang::elab::registry::AttrField;
use piperine_lang::pom::wire;
use piperine_lang::pom::Design;

use crate::capability::HostCtx;
use crate::contributions::Registrar;
use crate::error::{PluginError, PluginResult};
use crate::manifest::Manifest;
use crate::view::{DesignStaging, SolveResultView};
use crate::Plugin;

/// One round-trip to a wire-protocol guest. Implementations: the wasmtime
/// core (linear memory) and the process core (JSON-RPC over stdio).
pub trait WireTransport: Send + Sync {
    fn register(&self) -> PluginResult<wire::Registration>;
    fn hook(&self, input: &wire::HookInput) -> PluginResult<wire::HookOutput>;
}

/// Wrap a transport as a [`Plugin`]: run the guest's registration once and
/// hand back the adapter. A guest declaring scripts is a load-time error —
/// scripts need capability-gated fs the out-of-host tiers don't have yet.
pub fn host_wire(
    manifest: &Manifest,
    transport: Arc<dyn WireTransport>,
) -> PluginResult<Box<dyn Plugin>> {
    let contributions = transport.register()?;
    if !contributions.scripts.is_empty() {
        return Err(PluginError::Other {
            plugin: manifest.name.clone(),
            message: format!(
                "guest declares scripts {:?}, but scripts are not supported on the `{}` \
                 backend yet (capability-gated fs is a follow-up — use abi = \"native\")",
                contributions.scripts,
                manifest.abi.as_str()
            ),
        });
    }
    Ok(Box::new(WireHosted { manifest: manifest.clone(), transport, contributions }))
}

/// A wire-protocol guest presented as an ordinary [`Plugin`].
struct WireHosted {
    manifest: Manifest,
    transport: Arc<dyn WireTransport>,
    contributions: wire::Registration,
}

impl WireHosted {
    fn run_hook(&self, input: &wire::HookInput) -> PluginResult<wire::HookOutput> {
        let out = self.transport.hook(input)?;
        if let Some(error) = out.error {
            return Err(PluginError::Other { plugin: self.manifest.name.clone(), message: error });
        }
        Ok(out)
    }

    fn read_only_hook(&self, hook: wire::Hook, input: wire::HookInput) -> PluginResult<()> {
        let out = self.run_hook(&input)?;
        if !out.actions.is_empty() {
            return Err(PluginError::Other {
                plugin: self.manifest.name.clone(),
                message: format!("read-only hook {hook:?} returned staging actions"),
            });
        }
        Ok(())
    }
}

impl Plugin for WireHosted {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn register(&self, r: &mut Registrar) {
        for schema in &self.contributions.schemas {
            r.attr_schema(
                &schema.name,
                schema
                    .fields
                    .iter()
                    .map(|f| AttrField {
                        name: f.name.clone(),
                        ty: f.ty.clone(),
                        required: f.required,
                        default: None,
                        decl_span: None,
                    })
                    .collect(),
            );
        }
    }

    fn after_parse(&self, _cx: &mut HostCtx, source: &str) -> PluginResult<()> {
        self.read_only_hook(
            wire::Hook::AfterParse,
            wire::HookInput {
                hook: wire::Hook::AfterParse,
                source: Some(source.to_string()),
                design: None,
                solve: None,
            },
        )
    }

    fn after_elaborate(&self, _cx: &mut HostCtx, design: &Design) -> PluginResult<()> {
        self.read_only_hook(
            wire::Hook::AfterElaborate,
            wire::HookInput {
                hook: wire::Hook::AfterElaborate,
                source: None,
                design: Some(design.clone()),
                solve: None,
            },
        )
    }

    fn transform_design(&self, _cx: &mut HostCtx, staging: &DesignStaging<'_>) -> PluginResult<()> {
        let input = wire::HookInput {
            hook: wire::Hook::TransformDesign,
            source: None,
            design: Some(staging.design().clone()),
            solve: None,
        };
        let out = self.run_hook(&input)?;
        // The guest's patch applies through the same staging surface an
        // in-process plugin uses — same no-netlist-magic + P0008 checks.
        for action in out.actions {
            match action {
                wire::Action::SetParam { instance, param, value } => {
                    staging.set_param(&instance, &param, value);
                }
                wire::Action::AddInstance { parent, label, module, ports, params } => {
                    staging.add_instance(
                        &parent,
                        &label,
                        &module,
                        ports,
                        params,
                    )?;
                }
                wire::Action::AddConnection { parent, lhs, rhs } => {
                    staging.add_connection(&parent, &lhs, &rhs)?;
                }
            }
        }
        Ok(())
    }

    fn before_lower(&self, _cx: &mut HostCtx, design: &Design) -> PluginResult<()> {
        self.read_only_hook(
            wire::Hook::BeforeLower,
            wire::HookInput {
                hook: wire::Hook::BeforeLower,
                source: None,
                design: Some(design.clone()),
                solve: None,
            },
        )
    }

    fn after_solve(&self, _cx: &mut HostCtx, result: &SolveResultView) -> PluginResult<()> {
        self.read_only_hook(
            wire::Hook::AfterSolve,
            wire::HookInput {
                hook: wire::Hook::AfterSolve,
                source: None,
                design: None,
                solve: Some(wire::Solve {
                    analysis: result.analysis.clone(),
                    node_voltages: result.node_voltages.clone(),
                }),
            },
        )
    }
}
