//! WASM backend (SPEC Part VI §6.1, Plugin plan Phase 4): the plugin is a
//! sandboxed `wasm32-unknown-unknown` module speaking the POM's wire
//! protocol (`piperine_lang::pom::wire`) over guest linear memory. The
//! shared [`WireHosted`](super::wire_hosted) adapter does everything above
//! the transport.
//!
//! - No host imports: the guest is pure — it receives snapshots and returns
//!   patches. Real capability-gated imports (fs for scripts) are a
//!   follow-up; a guest declaring a script is a load-time error.
//! - Runaway protection: every guest call runs under a **fuel cap** derived
//!   from the manifest's `timeout_ms` (1e6 fuel per ms — a coarse
//!   instructions-per-millisecond proxy). An infinite loop traps with
//!   "all fuel consumed", surfacing as a loud `PluginError`.
//! - Devices stay native/process-only (Plugin plan D9): a device sits in
//!   the Newton loop; snapshot-per-call WASM is unusable there.

use std::sync::{Arc, Mutex};

use piperine_lang::pom::wire;

use wasmtime::{Config, Engine, Instance, Memory, Store, TypedFunc};

use super::wire_hosted::{host_wire, WireTransport};
use crate::error::{PluginError, PluginResult};
use crate::manifest::Manifest;
use crate::Plugin;

/// One loaded guest: the wasmtime plumbing every call goes through.
/// `Store` is single-threaded; the mutex serializes calls (hooks
/// run a handful of times per run, never concurrently in practice).
struct WasmCore {
    name: String,
    store: Mutex<Store<()>>,
    memory: Memory,
    alloc: TypedFunc<i32, i32>,
    register: TypedFunc<(), i64>,
    hook: TypedFunc<(i32, i32), i64>,
    fuel_per_call: u64,
}

impl WasmCore {
    fn err(&self, message: String) -> PluginError {
        PluginError::Other { plugin: self.name.clone(), message }
    }

    /// Read a packed `(ptr, len)` reply out of guest memory.
    fn read_reply(&self, store: &Store<()>, packed: i64) -> PluginResult<Vec<u8>> {
        let (ptr, len) = wire::unpack(packed);
        let mut out = vec![0u8; len as usize];
        self.memory
            .read(store, ptr as usize, &mut out)
            .map_err(|e| self.err(e.to_string()))?;
        Ok(out)
    }

    /// Write `payload` into guest memory (via `pp_alloc`) and call `f`,
    /// reading back the packed reply. Fuel is reset per call.
    fn call(&self, f: &TypedFunc<(i32, i32), i64>, payload: &[u8]) -> PluginResult<Vec<u8>> {
        let mut store = self.store.lock().expect("wasm store poisoned");
        store.set_fuel(self.fuel_per_call).map_err(|e| self.err(e.to_string()))?;
        let ptr = self
            .alloc
            .call(&mut *store, payload.len() as i32)
            .map_err(|e| self.err(format!("pp_alloc: {e}")))?;
        self.memory
            .write(&mut *store, ptr as usize, payload)
            .map_err(|e| self.err(e.to_string()))?;
        let packed = f
            .call(&mut *store, (ptr, payload.len() as i32))
            .map_err(|e| self.err(format!("guest call failed (fuel cap = {}): {e}", self.fuel_per_call)))?;
        self.read_reply(&store, packed)
    }
}

impl WireTransport for WasmCore {
    fn register(&self) -> PluginResult<wire::Registration> {
        let mut store = self.store.lock().expect("wasm store poisoned");
        store.set_fuel(self.fuel_per_call).map_err(|e| self.err(e.to_string()))?;
        let packed = self
            .register
            .call(&mut *store, ())
            .map_err(|e| self.err(format!("pp_register: {e}")))?;
        let reply = self.read_reply(&store, packed)?;
        serde_json::from_slice(&reply).map_err(|e| self.err(format!("bad contributions: {e}")))
    }

    fn hook(&self, input: &wire::HookInput) -> PluginResult<wire::HookOutput> {
        let payload = serde_json::to_vec(input).map_err(|e| self.err(e.to_string()))?;
        let reply = self.call(&self.hook, &payload)?;
        serde_json::from_slice(&reply).map_err(|e| self.err(format!("bad hook output: {e}")))
    }
}

/// Load a `.wasm` artifact, verify the wire ABI version, and wrap it as an
/// ordinary [`Plugin`].
pub fn load(manifest: &Manifest, artifact: &std::path::Path) -> PluginResult<Box<dyn Plugin>> {
    let name = manifest.name.clone();
    let err = |message: String| PluginError::Other { plugin: name.clone(), message };

    let mut config = Config::new();
    config.consume_fuel(true);
    let engine = Engine::new(&config).map_err(|e| err(e.to_string()))?;
    let module = wasmtime::Module::from_file(&engine, artifact)
        .map_err(|e| err(format!("loading {}: {e}", artifact.display())))?;
    let mut store = Store::new(&engine, ());
    // 1e6 fuel per manifest millisecond — coarse, deterministic runaway cap.
    let fuel_per_call = manifest.permissions.timeout_ms.max(1) * 1_000_000;
    store.set_fuel(fuel_per_call).map_err(|e| err(e.to_string()))?;

    let instance = wasmtime::Linker::new(&engine)
        .instantiate(&mut store, &module)
        .map_err(|e| err(format!("instantiate: {e}")))?;
    let get = |store: &mut Store<()>, instance: &Instance, name: &str| {
        instance
            .get_func(&mut *store, name)
            .ok_or_else(|| PluginError::Other {
                plugin: manifest.name.clone(),
                message: format!("guest does not export `{name}`"),
            })
    };
    let abi: TypedFunc<(), i32> = get(&mut store, &instance, "pp_abi_version")?
        .typed(&store)
        .map_err(|e| err(e.to_string()))?;
    let version = abi.call(&mut store, ()).map_err(|e| err(e.to_string()))?;
    if version as u32 != wire::WASM_ABI_VERSION {
        return Err(err(format!(
            "wire ABI mismatch: guest has {version}, host expects {}",
            wire::WASM_ABI_VERSION
        )));
    }
    let alloc = get(&mut store, &instance, "pp_alloc")?.typed(&store).map_err(|e| err(e.to_string()))?;
    let register = get(&mut store, &instance, "pp_register")?.typed(&store).map_err(|e| err(e.to_string()))?;
    let hook = get(&mut store, &instance, "pp_hook")?.typed(&store).map_err(|e| err(e.to_string()))?;
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| err("guest does not export `memory`".into()))?;

    let core = Arc::new(WasmCore {
        name: manifest.name.clone(),
        store: Mutex::new(store),
        memory,
        alloc,
        register,
        hook,
        fuel_per_call,
    });
    host_wire(manifest, core)
}
