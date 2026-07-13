//! Process-backend gates (Plugin plan Phase 5): the rc-parasitics case
//! served by a child process over stdio JSON-RPC passes the same divider
//! gate, guest bench tasks dispatch, and a crashed/exited guest is a loud
//! error — the isolation boundary the process tier exists for.

use std::path::PathBuf;
use std::rc::Rc;

use piperine_bench::{BenchOutcome, BenchRunner};
use piperine_lang::SourceMap;
use piperine_plugin::{PluginHost, TrustMode};

fn guest_bin() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir.parent().unwrap().parent().unwrap();
    let status = std::process::Command::new(env!("CARGO"))
        .args(["build", "-p", "piperine-plugin", "--example", "process_parasitics"])
        .current_dir(workspace)
        .status()
        .expect("cargo build process guest");
    assert!(status.success(), "process guest build failed");
    workspace.join("target").join("debug").join("examples").join("process_parasitics")
}

fn project_with_guest(dir: &std::path::Path, artifact: &std::path::Path) {
    let plugin_dir = dir.join("proc-plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    std::fs::copy(artifact, plugin_dir.join("process_parasitics")).unwrap();
    std::fs::write(
        plugin_dir.join("piperine-plugin.toml"),
        "[plugin]\nname = \"para\"\nabi = \"process\"\nentry = \"process_parasitics\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("Piperine.toml"),
        "[project]\nname = \"proc-smoke\"\nversion = \"0.1.0\"\nauthors = []\nedition = \"2024\"\n\n\
         [plugins.para]\npath = \"proc-plugin\"\n",
    )
    .unwrap();
}

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
    bench Top {
        fn divider() {
            var r = $op();
            $assert(r.v(out, gnd) > 2.49, \"divider low\");
            $assert(r.v(out, gnd) < 2.51, \"divider high\");
        }
        fn task_roundtrip() {
            $assert($pgain() == 42.0, \"guest task value\");
        }
    }
";

fn run(host: Rc<PluginHost>, entry: &str) -> BenchOutcome {
    let design = piperine_lang::parse_and_elaborate_seeded(DIVIDER, &SourceMap::dummy(), |ctx| {
        host.seed_schemas(ctx);
    })
    .expect("elaborate");
    BenchRunner::new(&design)
        .with_device_provider(host.clone())
        .with_plugins(host)
        .run_entry("Top", entry)
}

#[test]
fn process_parasitics_passes_the_divider_gate() {
    let artifact = guest_bin();
    let dir = tempfile::tempdir().unwrap();
    project_with_guest(dir.path(), &artifact);
    let host =
        Rc::new(PluginHost::load_for_project(dir.path(), TrustMode::AcceptAll).expect("load"));

    match run(host.clone(), "divider") {
        BenchOutcome::Passed => {}
        BenchOutcome::Failed(m) => panic!("bench assert failed: {m}"),
        BenchOutcome::Error(m) => panic!("bench errored: {m}"),
    }
    match run(host, "task_roundtrip") {
        BenchOutcome::Passed => {}
        BenchOutcome::Failed(m) => panic!("bench assert failed: {m}"),
        BenchOutcome::Error(m) => panic!("bench errored: {m}"),
    }
}

#[test]
fn dead_guest_is_a_loud_error() {
    // A guest that exits immediately (not speaking the protocol) must fail
    // the load loudly, never hang or no-op.
    let dir = tempfile::tempdir().unwrap();
    let plugin_dir = dir.path().join("proc-plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    let sh = plugin_dir.join("dead_guest.sh");
    std::fs::write(&sh, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&sh, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::fs::write(
        plugin_dir.join("piperine-plugin.toml"),
        "[plugin]\nname = \"dead\"\nabi = \"process\"\nentry = \"dead_guest.sh\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("Piperine.toml"),
        "[project]\nname = \"dead-smoke\"\nversion = \"0.1.0\"\nauthors = []\nedition = \"2024\"\n\n\
         [plugins.dead]\npath = \"proc-plugin\"\n",
    )
    .unwrap();

    let err = PluginHost::load_for_project(dir.path(), TrustMode::AcceptAll)
        .map(|_| ())
        .expect_err("dead guest must fail loud");
    let msg = err.to_string();
    assert!(msg.contains("exited") || msg.contains("stdout"), "{msg}");
}
