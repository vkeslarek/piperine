//! Hook surfaces. In-process plugins reflect over **the real POM** —
//! `&Design` is the one structural interface (SPEC Part IV); there is no
//! parallel view model. The staging handle is the one mutable surface
//! (SPEC Part VI §8.2). Out-of-host tiers (WASM/process) receive the POM's
//! own serialized form (serde on `Design` itself, owned by `piperine-lang`).

use piperine_lang::pom::staging::{ConnectionSpec, InstanceSpec};
use piperine_lang::Design;
use piperine_lang::Value;

/// One analysis result summary for the `after_solve` hook. Analysis
/// metadata, not design structure: `node_voltages` is populated for `$op`;
/// swept analyses carry only their kind for now.
#[derive(Debug, Clone)]
pub struct SolveResultView {
    pub analysis: String,
    pub node_voltages: Vec<(String, f64)>,
}

/// The mutable surface of the `transform_design` hook (SPEC Part VI §8.2):
/// mutations go through the design's staging layer — the same rails bench
/// writes ride — and are consumed by the next pure re-elaboration. A plugin
/// never receives `&mut Design`; reads go through the real POM.
pub struct DesignStaging<'a> {
    design: &'a Design,
    /// The writer these stagings are attributed to (P0008 provenance).
    plugin: String,
}

impl<'a> DesignStaging<'a> {
    pub(crate) fn new(design: &'a Design, plugin: &str) -> Self {
        Self { design, plugin: plugin.to_string() }
    }

    /// The design being transformed — the full POM reflection surface,
    /// read-only.
    pub fn design(&self) -> &Design {
        self.design
    }

    /// Stage a parameter override on `instance` (empty label = the module's
    /// own params) — same semantics as a bench `inst.r = …` write.
    pub fn set_param(&self, instance: &str, param: &str, value: Value) {
        self.design.set_param(instance, param, value);
    }

    /// Stage an instance injection into `parent`. The module must be a type
    /// declared in the design (no-netlist-magic, Part VI §2) — P0005 with
    /// "type not declared"; a duplicate label with a different spec is a
    /// typed P0008 naming both plugins.
    pub fn add_instance(
        &self,
        parent: &str,
        label: &str,
        module: &str,
        ports: Vec<String>,
        params: Vec<(String, Value)>,
    ) -> Result<(), crate::PluginError> {
        use piperine_lang::pom::design::StageError;
        self.design
            .stage_instance(
                parent,
                InstanceSpec { label: label.to_string(), module: module.to_string(), ports, params },
                &self.plugin,
            )
            .map_err(|e| match e {
                StageError::Conflict(c) => crate::PluginError::StagingConflict {
                    a: c.first,
                    b: c.second,
                    path: format!("{}.{}", c.parent, c.label),
                },
                other => crate::PluginError::HookFailed {
                    hook: "transform_design",
                    plugin: self.plugin.clone(),
                    message: other.to_string(),
                },
            })
    }

    /// Stage a net connection into `parent`.
    pub fn add_connection(&self, parent: &str, lhs: &str, rhs: &str) -> Result<(), crate::PluginError> {
        self.design
            .stage_connection(
                parent,
                ConnectionSpec { lhs: lhs.to_string(), rhs: rhs.to_string() },
            )
            .map_err(|e| crate::PluginError::HookFailed {
                hook: "transform_design",
                plugin: self.plugin.clone(),
                message: e.to_string(),
            })
    }
}
