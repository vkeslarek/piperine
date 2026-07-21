//! The process-tier gate guest (Plugin plan Phase 5): the same rc-parasitics
//! case as the WASM guest, served as an executable over stdio JSON-RPC —
//! `serve_stdio` is the whole main.
//!
//! Build: `cargo build -p piperine-plugin --example process_parasitics`

use piperine_lang::pom::wire::{Action, Design, Registration, Value, WirePlugin};

struct Parasitics;

impl WirePlugin for Parasitics {
    fn register(&self) -> Registration {
        Registration {
            schemas: Vec::new(),
            scripts: Vec::new(),
        }
    }

    fn transform_design(&self, design: &Design) -> Result<Vec<Action>, String> {
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
}

fn main() {
    piperine_lang::pom::wire::serve_stdio(&Parasitics);
}
