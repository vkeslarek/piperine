//! Phase-4 gates (Plugin plan): the rc-parasitics case recompiled to WASM
//! passes the Phase-3 divider gate unmodified, and a runaway guest is
//! killed by the fuel cap — driven through the root host API.

use std::path::PathBuf;
use std::rc::Rc;

use piperine::{NetRef, SimSession, SolverConfig};
use piperine_lang::SourceMap;
use piperine_plugin::{PluginHost, TrustMode};

/// Build the guest example for wasm32 and return the artifact path.
fn guest_wasm() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir.parent().unwrap().parent().unwrap();
    let status = std::process::Command::new(env!("CARGO"))
        .args([
            "build",
            "-p",
            "piperine-plugin-wasm",
            "--example",
            "wasm_parasitics",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .current_dir(workspace)
        .status()
        .expect("cargo build wasm guest");
    assert!(status.success(), "wasm guest build failed");
    workspace
        .join("target")
        .join("wasm32-unknown-unknown")
        .join("debug")
        .join("examples")
        .join("wasm_parasitics.wasm")
}

/// A throwaway project whose `[plugins]` names the wasm guest by path.
/// `timeout_ms` is tiny so the runaway test traps fast (fuel = ms × 1e6).
fn project_with_guest(dir: &std::path::Path, artifact: &std::path::Path) {
    let plugin_dir = dir.join("wasm-plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    std::fs::copy(artifact, plugin_dir.join("wasm_parasitics.wasm")).unwrap();
    std::fs::write(
        plugin_dir.join("piperine-plugin.toml"),
        "[plugin]\nname = \"para\"\nabi = \"wasm\"\nentry = \"wasm_parasitics.wasm\"\n\n\
         [permissions]\ntimeout_ms = 200\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("Piperine.toml"),
        "[project]\nname = \"wasm-smoke\"\nversion = \"0.1.0\"\nauthors = []\nedition = \"2024\"\n\n\
         [plugins.para]\npath = \"wasm-plugin\"\n",
    )
    .unwrap();
}

fn load_host(dir: &std::path::Path) -> Rc<PluginHost> {
    Rc::new(PluginHost::load_for_project(dir, TrustMode::AcceptAll).expect("load wasm plugin"))
}

/// The Phase-3 divider source, verbatim (`r1` dangles until the plugin
/// injects `r_par` from `out` to `gnd`).
const DIVIDER: &str = "
    discipline Electrical { potential v: Real; flow i: Real; }

    mod VoltageSource(inout p: Electrical, inout n: Electrical) {
        param voltage: Real = 0.0;
    }
    analog VoltageSource { V(p, n) <- voltage; }

    mod Resistor(inout p: Electrical, inout n: Electrical) {
        param r: Real = 1e3;
    }
    analog Resistor { I(p, n) <+ V(p, n) / r; }

    mod Top() {
        wire gnd : Electrical;
        wire vin : Electrical;
        wire out : Electrical;
        src : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
        r1  : Resistor (.p = vin, .n = out);
    }
";

/// An operating point of `Top` through a session wired with the wasm host.
fn run_top_op(host: Rc<PluginHost>, src: &str) -> Result<f64, String> {
    let design = piperine_lang::parse_and_elaborate_seeded(src, &SourceMap::dummy(), |ctx| {
        host.seed_schemas(ctx);
    })
    .expect("elaborate");
    let mut session = SimSession::new(design, "Top".to_string());
    session.set_device_provider(host.clone());
    session.set_hooks(host);
    let op = session
        .run_op(&SolverConfig::default(), None)
        .map_err(|e| e.to_string())?;
    op.v(&NetRef { name: "out".to_string() }, None).map_err(|e| e.to_string())
}

#[test]
fn wasm_parasitics_passes_the_phase3_gate() {
    let artifact = guest_wasm();
    let dir = tempfile::tempdir().unwrap();
    project_with_guest(dir.path(), &artifact);
    let host = load_host(dir.path());

    let out = run_top_op(host, DIVIDER).expect("op solves");
    assert!(out > 2.49 && out < 2.51, "divider at 2.5 V, got {out}");
}

#[test]
fn runaway_guest_is_killed_by_the_fuel_cap() {
    let artifact = guest_wasm();
    let dir = tempfile::tempdir().unwrap();
    project_with_guest(dir.path(), &artifact);
    let host = load_host(dir.path());

    // The `Runaway` marker module tells the guest to spin in `before_lower`.
    let src = format!("{DIVIDER}\n    mod Runaway() {{}}\n");
    let msg = run_top_op(host, &src).expect_err("the fuel cap must trap");
    assert!(msg.contains("fuel"), "the trap must name the fuel cap: {msg}");
}
