//! The Phase-4 gate guest (Plugin plan): the rc-parasitics case compiled to
//! WASM — `transform_design` returns an `AddInstance` patch staging a
//! declared `Resistor` from `out` to `gnd`. Also registers two bench tasks:
//! `$wgain()` (returns 42.0, proves task dispatch) and `$spin()` (loops
//! forever, proving the host's fuel cap kills a runaway guest).
//!
//! Build: `cargo build -p piperine-plugin-wasm --example wasm_parasitics
//!         --target wasm32-unknown-unknown`

use piperine_plugin_wasm as sdk;
use sdk::{Action, Design, Registration, Value};

struct Parasitics;

impl sdk::WirePlugin for Parasitics {
    fn register(&self) -> Registration {
        Registration {
            schemas: Vec::new(),
            bench_tasks: vec!["wgain".into(), "spin".into()],
            scripts: Vec::new(),
        }
    }

    fn transform_design(&self, design: &Design) -> Result<Vec<Action>, String> {
        // Same no-netlist-magic contract as in-process plugins: the patch
        // names a declared type; the host validates before applying.
        if design.module("Top").is_none() {
            return Err("expected a `Top` module in the design".into());
        }
        Ok(vec![Action::AddInstance {
            parent: "Top".into(),
            label: "r_par".into(),
            module: "Resistor".into(),
            ports: vec!["out".into(), "gnd".into()],
            params: vec![("r".into(), Value::Real(1e3))],
        }])
    }

    fn bench_task(&self, name: &str, _args: Vec<Value>) -> Result<Value, String> {
        match name {
            "wgain" => Ok(Value::Real(42.0)),
            "spin" => {
                // Deliberate runaway: the host's fuel cap must kill this.
                let mut x = 0u64;
                loop {
                    x = x.wrapping_add(1);
                    if x == u64::MAX {
                        return Ok(Value::Nat(x));
                    }
                }
            }
            other => Err(format!("unknown task `{other}`")),
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pp_abi_version() -> i32 {
    sdk::wasm_abi_version()
}

#[unsafe(no_mangle)]
pub extern "C" fn pp_alloc(len: i32) -> i32 {
    sdk::wasm_alloc(len)
}

#[unsafe(no_mangle)]
pub extern "C" fn pp_register() -> i64 {
    sdk::wasm_register(&Parasitics)
}

#[unsafe(no_mangle)]
pub extern "C" fn pp_hook(ptr: i32, len: i32) -> i64 {
    sdk::wasm_hook(&Parasitics, ptr, len)
}

#[unsafe(no_mangle)]
pub extern "C" fn pp_task(ptr: i32, len: i32) -> i64 {
    sdk::wasm_task(&Parasitics, ptr, len)
}
