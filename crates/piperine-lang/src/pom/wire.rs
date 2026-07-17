//! The plugin wire protocol (SPEC Part VI §6), built directly on the POM.
//!
//! **There is exactly one model.** What crosses the host↔guest boundary is
//! the real [`Design`] and the real [`Value`] — the POM types themselves,
//! which serialize as themselves (see the serde notes on [`Design`] and
//! [`Value`]). This module adds only *protocol*: registration shapes, hook
//! envelopes, the staging-patch actions, RPC framing, and the guest
//! runtimes for the WASM and process tiers. If the POM cannot express
//! something the language has, the POM gets extended — never shadowed here.
//!
//! In-process (native) plugins skip all of this: their hooks receive
//! `&Design` directly.
//!
//! ## Guest ABI (version [`WASM_ABI_VERSION`])
//!
//! A WASM guest module exports:
//!
//! ```text
//! pp_abi_version() -> i32                    // must equal WASM_ABI_VERSION
//! pp_alloc(len: i32) -> i32                  // guest buffer the host writes into
//! pp_register() -> i64                       // packed JSON Registration
//! pp_hook(in_ptr: i32, in_len: i32) -> i64   // JSON HookInput → packed JSON HookOutput
//! ```
//!
//! A packed `i64` return is `(ptr << 32) | len` into guest memory. A
//! process guest speaks the same shapes over line-delimited JSON-RPC
//! ([`serve_stdio`]).

use serde::{Deserialize, Serialize};

pub use super::design::Design;
pub use crate::value::Value;

/// Bumped on any breaking change to the shapes or the export set.
pub const WASM_ABI_VERSION: u32 = 3;

// ─── Registration ──────────────────────────────────────────────────────────────

/// What a guest contributes at load time (SPEC Part VI §6).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Registration {
    #[serde(default)]
    pub schemas: Vec<Schema>,
    /// Script names. Not yet runnable on the out-of-host tiers (the
    /// capability-gated fs imports are a follow-up) — declaring one is a
    /// load-time error.
    #[serde(default)]
    pub scripts: Vec<String>,
}

/// A guest-declared attribute schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub fields: Vec<SchemaField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaField {
    pub name: String,
    /// PHDL type name (`"String"`, `"Real"`, …).
    pub ty: String,
    pub required: bool,
}

// ─── Hooks ─────────────────────────────────────────────────────────────────────

/// Which hook is being asked to run (SPEC Part VI §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hook {
    AfterParse,
    AfterElaborate,
    TransformDesign,
    BeforeLower,
    AfterSolve,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookInput {
    pub hook: Hook,
    /// `AfterParse` only: the raw source text.
    #[serde(default)]
    pub source: Option<String>,
    /// Design-carrying hooks — the real POM `Design`, serialized as itself.
    #[serde(default)]
    pub design: Option<Design>,
    /// `AfterSolve` only.
    #[serde(default)]
    pub solve: Option<Solve>,
}

/// One analysis result summary. `node_voltages` is populated for `$op`;
/// swept analyses carry only their kind for now.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solve {
    pub analysis: String,
    pub node_voltages: Vec<(String, f64)>,
}

/// What a hook returns: an error aborts the run (fail loud); actions are
/// the staging patch (`TransformDesign` only — the host rejects actions
/// from read-only hooks).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookOutput {
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub actions: Vec<Action>,
}

/// The staging patch language — the same three verbs the in-process
/// staging surface offers (SPEC Part VI §8.2), carrying real POM [`Value`]s.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    SetParam { instance: String, param: String, value: Value },
    AddInstance {
        parent: String,
        label: String,
        module: String,
        ports: Vec<String>,
        params: Vec<(String, Value)>,
    },
    AddConnection { parent: String, lhs: String, rhs: String },
}

// ─── The guest contract ────────────────────────────────────────────────────────

/// The guest-side plugin contract — one trait for both out-of-host tiers
/// (WASM guests via the five `pp_*` exports; process guests via
/// [`serve_stdio`]). Every method defaults to a no-op. The `Design` a hook
/// receives is the deserialized POM — the same type, the same accessors
/// (`design.module("Top")`, `module.instances`, …) an in-process plugin
/// reflects over.
pub trait WirePlugin {
    fn register(&self) -> Registration {
        Registration::default()
    }

    fn after_parse(&self, _source: &str) -> Result<(), String> {
        Ok(())
    }

    fn after_elaborate(&self, _design: &Design) -> Result<(), String> {
        Ok(())
    }

    /// The one mutable hook: return the staging patch the host applies.
    fn transform_design(&self, _design: &Design) -> Result<Vec<Action>, String> {
        Ok(Vec::new())
    }

    fn before_lower(&self, _design: &Design) -> Result<(), String> {
        Ok(())
    }

    fn after_solve(&self, _solve: &Solve) -> Result<(), String> {
        Ok(())
    }
}

/// Run one hook against a [`WirePlugin`] — shared by the WASM guest's
/// `pp_hook` body and the process tier's [`serve_stdio`].
pub fn run_hook(plugin: &impl WirePlugin, input: HookInput) -> HookOutput {
    let design = input.design.unwrap_or_default();
    let result: Result<Vec<Action>, String> = match input.hook {
        Hook::AfterParse => plugin
            .after_parse(input.source.as_deref().unwrap_or(""))
            .map(|()| Vec::new()),
        Hook::AfterElaborate => plugin.after_elaborate(&design).map(|()| Vec::new()),
        Hook::TransformDesign => plugin.transform_design(&design),
        Hook::BeforeLower => plugin.before_lower(&design).map(|()| Vec::new()),
        Hook::AfterSolve => {
            let solve = input
                .solve
                .unwrap_or(Solve { analysis: String::new(), node_voltages: Vec::new() });
            plugin.after_solve(&solve).map(|()| Vec::new())
        }
    };
    match result {
        Ok(actions) => HookOutput { error: None, actions },
        Err(e) => HookOutput { error: Some(e), actions: vec![] },
    }
}

// ─── Process tier framing ──────────────────────────────────────────────────────

/// One line-delimited JSON-RPC request (the process tier, SPEC Part VI §6.4).
#[derive(Debug, Deserialize, Serialize)]
pub struct RpcRequest {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

/// One line-delimited JSON-RPC response.
#[derive(Debug, Deserialize, Serialize)]
pub struct RpcResponse {
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Serve a [`WirePlugin`] over stdin/stdout — the whole main() of a
/// process-tier plugin executable. Methods: `abi_version`, `register`,
/// `hook`. Returns when stdin closes (the host dropped the plugin).
pub fn serve_stdio(plugin: &impl WirePlugin) {
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<RpcRequest>(&line) {
            Err(e) => RpcResponse { id: 0, result: None, error: Some(format!("bad request: {e}")) },
            Ok(req) => {
                let id = req.id;
                let result: Result<serde_json::Value, String> = match req.method.as_str() {
                    "abi_version" => Ok(serde_json::json!(WASM_ABI_VERSION)),
                    "register" => serde_json::to_value(plugin.register()).map_err(|e| e.to_string()),
                    "hook" => req
                        .params
                        .ok_or("hook needs params".to_string())
                        .and_then(|p| serde_json::from_value::<HookInput>(p).map_err(|e| e.to_string()))
                        .map(|input| run_hook(plugin, input))
                        .and_then(|out| serde_json::to_value(out).map_err(|e| e.to_string())),
                    other => Err(format!("unknown method `{other}`")),
                };
                match result {
                    Ok(value) => RpcResponse { id, result: Some(value), error: None },
                    Err(e) => RpcResponse { id, result: None, error: Some(e) },
                }
            }
        };
        let Ok(encoded) = serde_json::to_string(&response) else { break };
        if writeln!(stdout, "{encoded}").is_err() || stdout.flush().is_err() {
            break;
        }
    }
}

// ─── WASM tier: packing + guest glue ───────────────────────────────────────────

/// Pack a guest buffer address for the `i64` return convention.
pub fn pack(ptr: u32, len: u32) -> i64 {
    ((ptr as i64) << 32) | (len as i64)
}

/// Unpack an `i64` return into `(ptr, len)`.
pub fn unpack(packed: i64) -> (u32, u32) {
    ((packed >> 32) as u32, packed as u32)
}

/// `pp_abi_version` body for a WASM guest.
pub fn wasm_abi_version() -> i32 {
    WASM_ABI_VERSION as i32
}

/// `pp_alloc` body: a buffer the host writes an input payload into. Leaked —
/// per-call payloads, a handful per run, never in a solver loop.
pub fn wasm_alloc(len: i32) -> i32 {
    let buf = vec![0u8; len.max(0) as usize];
    Box::leak(buf.into_boxed_slice()).as_mut_ptr() as i32
}

/// Leak `bytes` and pack its address for the `i64` return convention.
fn wasm_reply(bytes: Vec<u8>) -> i64 {
    let len = bytes.len() as u32;
    let ptr = Box::leak(bytes.into_boxed_slice()).as_ptr() as u32;
    pack(ptr, len)
}

/// Read the host-written input payload. `ptr`/`len` come from the host,
/// which wrote them through `pp_alloc` — trusted by construction of the
/// protocol.
fn wasm_input(ptr: i32, len: i32) -> &'static [u8] {
    unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) }
}

/// `pp_register` body for a WASM guest.
pub fn wasm_register(plugin: &impl WirePlugin) -> i64 {
    wasm_reply(serde_json::to_vec(&plugin.register()).expect("serialize registration"))
}

/// `pp_hook` body for a WASM guest: decode the input, run the matching
/// hook, encode the output. A decode failure or hook error becomes
/// `HookOutput.error` — the host fails loud with it.
pub fn wasm_hook(plugin: &impl WirePlugin, ptr: i32, len: i32) -> i64 {
    let out = match serde_json::from_slice::<HookInput>(wasm_input(ptr, len)) {
        Err(e) => HookOutput { error: Some(format!("bad hook input: {e}")), actions: vec![] },
        Ok(input) => run_hook(plugin, input),
    };
    wasm_reply(serde_json::to_vec(&out).expect("serialize hook output"))
}

