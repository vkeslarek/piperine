//! Out-of-process backend (SPEC Part VI §6.3): the plugin is an executable
//! speaking line-delimited JSON-RPC over stdio (`piperine_lang::pom::wire::
//! serve_stdio` is the whole guest main). Real isolation — a plugin crash
//! cannot take down the host, and the process can be containerized — at
//! millisecond-per-call latency. No per-call timeout yet (unlike WASM's
//! fuel cap); the isolation story here is the crash boundary, not DoS
//! protection.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use piperine_lang::pom::wire;
use wire::{RpcRequest, RpcResponse};

use super::wire_hosted::{host_wire, WireTransport};
use crate::error::{PluginError, PluginResult};
use crate::manifest::Manifest;
use crate::Plugin;

/// The spawned child and its stdio pipes. One request in flight at a time
/// (the mutex serializes callers, matching the guest's sequential loop).
struct ProcessCore {
    name: String,
    io: Mutex<ProcessIo>,
}

struct ProcessIo {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Drop for ProcessIo {
    fn drop(&mut self) {
        // Best effort: closing stdin ends the guest's serve loop; kill is
        // the backstop for a guest that ignores it.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl ProcessCore {
    fn err(&self, message: String) -> PluginError {
        PluginError::Other { plugin: self.name.clone(), message }
    }

    /// One JSON-RPC round trip.
    fn rpc(&self, method: &str, params: Option<serde_json::Value>) -> PluginResult<serde_json::Value> {
        let mut io = self.io.lock().expect("process io poisoned");
        io.next_id += 1;
        let request = RpcRequest { id: io.next_id, method: method.to_string(), params };
        let encoded = serde_json::to_string(&request).map_err(|e| self.err(e.to_string()))?;
        writeln!(io.stdin, "{encoded}").map_err(|e| self.err(format!("guest stdin: {e}")))?;
        io.stdin.flush().map_err(|e| self.err(format!("guest stdin: {e}")))?;
        let mut line = String::new();
        let read = io
            .stdout
            .read_line(&mut line)
            .map_err(|e| self.err(format!("guest stdout: {e}")))?;
        if read == 0 {
            return Err(self.err("guest exited (stdout closed)".into()));
        }
        let response: RpcResponse =
            serde_json::from_str(&line).map_err(|e| self.err(format!("bad response: {e}")))?;
        if response.id != request.id {
            return Err(self.err(format!(
                "response id {} does not match request id {}",
                response.id, request.id
            )));
        }
        if let Some(error) = response.error {
            return Err(self.err(error));
        }
        response.result.ok_or_else(|| self.err("response carried neither result nor error".into()))
    }

    fn rpc_as<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> PluginResult<T> {
        let value = self.rpc(method, params)?;
        serde_json::from_value(value).map_err(|e| self.err(format!("bad {method} result: {e}")))
    }
}

impl WireTransport for ProcessCore {
    fn register(&self) -> PluginResult<wire::Registration> {
        self.rpc_as("register", None)
    }

    fn hook(&self, input: &wire::HookInput) -> PluginResult<wire::HookOutput> {
        let params = serde_json::to_value(input).map_err(|e| self.err(e.to_string()))?;
        self.rpc_as("hook", Some(params))
    }

    fn task(&self, input: &wire::TaskInput) -> PluginResult<wire::TaskOutput> {
        let params = serde_json::to_value(input).map_err(|e| self.err(e.to_string()))?;
        self.rpc_as("task", Some(params))
    }
}

/// Spawn the guest executable, verify the wire ABI version, and wrap it as
/// an ordinary [`Plugin`].
pub fn load(manifest: &Manifest, artifact: &std::path::Path) -> PluginResult<Box<dyn Plugin>> {
    let name = manifest.name.clone();
    let err = |message: String| PluginError::Other { plugin: name.clone(), message };

    let mut child = Command::new(artifact)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| err(format!("spawning {}: {e}", artifact.display())))?;
    let stdin = child.stdin.take().ok_or_else(|| err("no child stdin".into()))?;
    let stdout = BufReader::new(child.stdout.take().ok_or_else(|| err("no child stdout".into()))?);

    let core = Arc::new(ProcessCore {
        name: manifest.name.clone(),
        io: Mutex::new(ProcessIo { child, stdin, stdout, next_id: 0 }),
    });
    let version: u32 = core.rpc_as("abi_version", None)?;
    if version != wire::WASM_ABI_VERSION {
        return Err(err(format!(
            "wire ABI mismatch: guest has {version}, host expects {}",
            wire::WASM_ABI_VERSION
        )));
    }
    host_wire(manifest, core)
}
