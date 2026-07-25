//! The macro registry (plugin-interface v2, PLG-05/06): `#[pip::…]`
//! expansions submit declaration records here (life-before-main, inside the
//! binary the plugin's code ships in). A native cdylib's registry holds
//! exactly that plugin's declarations — the host reads them through
//! `Plugin::collect`, whose default body is compiled into the plugin's own
//! code (vtable dispatch), so attribution across dlopen needs no extra
//! symbols. An in-process host (`PluginHost::from_plugins`) shares one
//! registry with the plugins compiled into its binary.

use piperine_lang::pom::Design;

use crate::capability::HostCtx;
use crate::contributions::{DeviceFactory, ScriptHandler};
use crate::view::{DesignStaging, SolveResultView};

/// One `#[pip::device("Type")]` declaration: the `@device(type = …)` id
/// plus a constructor for the generated factory.
pub struct DeviceRegistration {
    pub type_id: &'static str,
    pub make: fn() -> Box<dyn DeviceFactory>,
}

impl DeviceRegistration {
    pub const fn new(type_id: &'static str, make: fn() -> Box<dyn DeviceFactory>) -> Self {
        Self { type_id, make }
    }
}

inventory::collect!(DeviceRegistration);

/// One `#[pip::script("name")]` declaration: the CLI subcommand name plus a
/// constructor for the generated handler.
pub struct ScriptRegistration {
    pub name: &'static str,
    pub make: fn() -> Box<dyn ScriptHandler>,
}

impl ScriptRegistration {
    pub const fn new(name: &'static str, make: fn() -> Box<dyn ScriptHandler>) -> Self {
        Self { name, make }
    }
}

inventory::collect!(ScriptRegistration);

/// The five frozen lifecycle phases (D8/PLG-11) — no sixth hook without a
/// real consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookPhase {
    AfterParse,
    AfterElaborate,
    TransformDesign,
    BeforeLower,
    AfterSolve,
}

impl HookPhase {
    pub const ALL: [HookPhase; 5] = [
        HookPhase::AfterParse,
        HookPhase::AfterElaborate,
        HookPhase::TransformDesign,
        HookPhase::BeforeLower,
        HookPhase::AfterSolve,
    ];

    /// The decorator spelling (`#[pip::hook(after_parse)]`,
    /// `@pip.hook.after_parse`).
    pub fn as_str(&self) -> &'static str {
        match self {
            HookPhase::AfterParse => "after_parse",
            HookPhase::AfterElaborate => "after_elaborate",
            HookPhase::TransformDesign => "transform_design",
            HookPhase::BeforeLower => "before_lower",
            HookPhase::AfterSolve => "after_solve",
        }
    }

    /// The phase named by a decorator, or `None` (the macros/Python facade
    /// turn this into a loud unknown-phase error).
    pub fn from_name(name: &str) -> Option<HookPhase> {
        HookPhase::ALL.iter().copied().find(|p| p.as_str() == name)
    }
}

/// Everything a fired hook may need; each phase populates its own payload
/// (`after_parse` → `source`, design hooks → `design`, `transform_design` →
/// `design` + `staging`, `after_solve` → `result`). The macro-generated
/// wrapper unwraps exactly its phase's payload — a missing one is a loud
/// host bug, never a silent skip.
pub struct HookCall<'a> {
    pub host: &'a HostCtx,
    pub source: Option<&'a str>,
    pub design: Option<&'a Design>,
    pub staging: Option<&'a DesignStaging<'a>>,
    pub result: Option<&'a SolveResultView>,
}

/// One `#[pip::hook(phase)]` declaration.
pub struct HookRegistration {
    pub phase: HookPhase,
    pub invoke: fn(&HookCall<'_>) -> Result<(), String>,
}

impl HookRegistration {
    pub const fn new(phase: HookPhase, invoke: fn(&HookCall<'_>) -> Result<(), String>) -> Self {
        Self { phase, invoke }
    }
}

inventory::collect!(HookRegistration);

/// The read side of the macro registry. Zero-sized: every method reads the
/// calling binary's own registry (see the module doc for attribution).
pub struct Registry;

impl Registry {
    /// Every `#[pip::device]` declaration in the calling binary.
    pub fn devices() -> impl Iterator<Item = &'static DeviceRegistration> {
        inventory::iter::<DeviceRegistration>.into_iter()
    }

    /// Every `#[pip::script]` declaration in the calling binary.
    pub fn scripts() -> impl Iterator<Item = &'static ScriptRegistration> {
        inventory::iter::<ScriptRegistration>.into_iter()
    }

    /// Every `#[pip::hook]` declaration in the calling binary.
    pub fn hooks() -> impl Iterator<Item = &'static HookRegistration> {
        inventory::iter::<HookRegistration>.into_iter()
    }
}
