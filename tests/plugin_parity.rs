//! PLG-12 — cross-host plugin-decorator parity: the canonical decorator,
//! hook-phase, `ctx`, and `staging` name lists are a single source of truth
//! checked against **both** plugin hosts (MD-22 literal parity, D14 —
//! name-level parity; the mechanisms differ).
//!
//! Mirrors `tests/host_parity.rs`'s technique: the Rust side is proven the
//! loudest way Rust can — the `#[pip::…]` macros are *used* below with every
//! canonical name (a drifted decorator or phase name fails to **compile**,
//! not just fails an assertion), and every canonical `Ctx`/`DesignStaging`
//! method is called by hand. The Python side is checked at runtime
//! (`hasattr`/`dir`) against the same canonical lists, embedded through
//! `piperine_python::embed::run_script`.
//! `cargo test -p piperine plugin_parity` (T9 full gate).

use std::path::Path;

use piperine_lang::{SourceMap, Value};
use piperine_plugin::{
    Ctx, DesignStaging, HookPhase, HostCtx, Permissions, Registry, SolveResultView,
};
use piperine_plugin::{DeviceKind, Element, PluginDevice, PluginDeviceSpec};
use piperine_solver::abi::{AnalogDevice, DigitalDevice, ElementCapabilities, Introspect};

/// The parity oracle all checks below read from (PLG-12): the decorator
/// names, the five frozen hook phases (D8), and the `ctx`/`staging` method
/// names — each must exist, spelled identically, on both hosts.
const DECORATORS: &[&str] = &["script", "hook", "device"];
const PHASES: &[&str] =
    &["after_parse", "after_elaborate", "transform_design", "before_lower", "after_solve"];
const CTX_METHODS: &[&str] = &["design", "project_root", "log", "fs_read", "fs_write"];
const STAGING_METHODS: &[&str] = &["design", "set_param", "add_instance", "add_connection"];

const SRC: &str = "mod Top() {}";

// ─── Rust decorator surface (compile-time proof) ────────────────────────────
//
// Every canonical decorator name used as a real `#[pip::…]` attribute; every
// canonical phase name used as a real `#[pip::hook(…)]` argument. A name
// drifted on the Rust side breaks this file's compilation.

#[pip::script("parity_script")]
fn parity_script(_args: &[String], _ctx: &Ctx) -> Result<i32, String> {
    Ok(0)
}

#[pip::hook(after_parse)]
fn parity_after_parse(_ctx: &Ctx, _source: &str) -> Result<(), String> {
    Ok(())
}

#[pip::hook(after_elaborate)]
fn parity_after_elaborate(_ctx: &Ctx) -> Result<(), String> {
    Ok(())
}

#[pip::hook(transform_design)]
fn parity_transform_design(_ctx: &Ctx, _staging: &DesignStaging) -> Result<(), String> {
    Ok(())
}

#[pip::hook(before_lower)]
fn parity_before_lower(_ctx: &Ctx) -> Result<(), String> {
    Ok(())
}

#[pip::hook(after_solve)]
fn parity_after_solve(_ctx: &Ctx, _result: &SolveResultView) -> Result<(), String> {
    Ok(())
}

/// A minimal device so `#[pip::device]` — the third canonical decorator —
// is used for real (the macro requires an `Element`).
#[pip::device("Parity::Probe")]
struct ParityProbe;

impl PluginDevice for ParityProbe {
    const KIND: DeviceKind = DeviceKind::Analog;

    fn from_spec(_spec: &PluginDeviceSpec) -> Result<Self, String> {
        Ok(Self)
    }
}

impl AnalogDevice for ParityProbe {}
impl DigitalDevice for ParityProbe {}
impl Introspect for ParityProbe {}

impl Element for ParityProbe {
    fn name(&self) -> &str {
        "parity"
    }

    fn capabilities(&self) -> ElementCapabilities {
        ElementCapabilities::ANALOG
    }
}

// ─── Rust ctx/staging surfaces (compile-time proof) ─────────────────────────

/// Every name in [`CTX_METHODS`], called on the Rust `Ctx` — fails to
/// COMPILE if the Rust ctx drifts.
fn call_every_ctx_method(ctx: &Ctx) {
    let _ = ctx.design();
    let _ = ctx.project_root();
    ctx.log("parity");
    let _ = ctx.fs_read("parity.txt");
    let _ = ctx.fs_write("parity.txt", "x");
}

/// Every name in [`STAGING_METHODS`], called on the Rust `DesignStaging` —
/// fails to COMPILE if the Rust staging surface drifts.
fn call_every_staging_method(staging: &DesignStaging) {
    let _ = staging.design();
    staging.set_param("", "r", Value::Real(1.0));
    let _ = staging.add_instance("Top", "p1", "NotDeclared", Vec::new(), Vec::new());
    let _ = staging.add_connection("Top", "a", "b");
}

// ─── Python probe ───────────────────────────────────────────────────────────

/// Run a Python script (through the embedded facade) that checks every
/// canonical name on the Python host — decorators as `piperine.<name>`,
/// phases as `piperine.HOOK_PHASES` + `pip.hook.<phase>`, ctx/staging
/// methods on `piperine.Ctx`/`piperine.Staging` — writing one `kind:name`
/// line per MISSING name to `out_path` (empty file = full parity).
/// `extra_*` inject synthetic drift names (the negative test).
fn python_missing_surface(out_path: &Path, extra: &[(&str, &str)]) {
    let list = |names: &[&str]| names.iter().map(|n| format!("{n:?}")).collect::<Vec<_>>().join(", ");
    let mut decorators = list(DECORATORS);
    let mut phases = list(PHASES);
    let mut ctx_methods = list(CTX_METHODS);
    let mut staging_methods = list(STAGING_METHODS);
    for (kind, name) in extra {
        let target = match *kind {
            "decorator" => &mut decorators,
            "phase" => &mut phases,
            "ctx" => &mut ctx_methods,
            "staging" => &mut staging_methods,
            other => panic!("unknown probe kind {other}"),
        };
        if !target.is_empty() {
            target.push_str(", ");
        }
        target.push_str(&format!("{name:?}"));
    }
    let phases_tuple = format!("{}, ", list(PHASES));
    let script = format!(
        r#"
import piperine

def _probe():
    missing = []
    for name in [{decorators}]:
        if not hasattr(piperine, name):
            missing.append("decorator:" + name)
    if tuple(piperine.HOOK_PHASES) != ({phases_tuple}):
        missing.append("phases:" + repr(tuple(piperine.HOOK_PHASES)))
    for phase in [{phases}]:
        if not hasattr(piperine.hook, phase):
            missing.append("phase:" + phase)
    for m in [{ctx_methods}]:
        if m not in dir(piperine.Ctx):
            missing.append("ctx:" + m)
    for m in [{staging_methods}]:
        if m not in dir(piperine.Staging):
            missing.append("staging:" + m)
    with open({out:?}, "w") as f:
        f.write("\n".join(missing))

_probe()
"#,
        decorators = decorators,
        phases_tuple = phases_tuple,
        phases = phases,
        ctx_methods = ctx_methods,
        staging_methods = staging_methods,
        out = out_path.to_str().expect("utf8 out path"),
    );
    let script_path = std::env::temp_dir().join(format!(
        "piperine_plugin_parity_{}_{}.py",
        std::process::id(),
        out_path.file_stem().expect("out file stem").to_string_lossy()
    ));
    std::fs::write(&script_path, script).expect("write probe script");
    let _ = std::fs::remove_file(out_path);
    piperine_python::embed::run_script(script_path.to_str().expect("utf8 script path"))
        .expect("python parity probe runs");
    let _ = std::fs::remove_file(&script_path);
}

/// PLG-12's positive case: the decorator names, the five hook phase names,
/// and the `ctx`/`staging` method names are identical on both hosts.
#[test]
fn plugin_parity_surfaces_match_on_both_hosts() {
    // Rust side, compile-time: the macro usages + hand-called methods above
    // fail the BUILD on drift. Execute the call proofs (their results are
    // irrelevant — the proof is that they resolve).
    let design = piperine_lang::parse_and_elaborate(SRC, &SourceMap::dummy()).expect("elaborates");
    let host = HostCtx::new("parity", Path::new("."), Permissions::default());
    call_every_ctx_method(&Ctx::new(&host, Some(&design)));
    call_every_staging_method(&DesignStaging::new(&design, "parity"));

    // Rust side, runtime: the declarations above bound into this binary's
    // registry under exactly the canonical names/phases.
    let scripts: Vec<&str> = Registry::scripts().map(|s| s.name).collect();
    assert_eq!(scripts, ["parity_script"], "the #[pip::script] declaration bound by name");
    let devices: Vec<&str> = Registry::devices().map(|d| d.type_id).collect();
    assert_eq!(devices, ["Parity::Probe"], "the #[pip::device] declaration bound by type id");
    let mut phases: Vec<&str> = Registry::hooks().map(|h| h.phase.as_str()).collect();
    phases.sort_unstable();
    let mut canonical = PHASES.to_vec();
    canonical.sort_unstable();
    assert_eq!(phases, canonical, "one #[pip::hook] per frozen phase bound");
    let catalog: Vec<&str> = HookPhase::ALL.iter().map(|p| p.as_str()).collect();
    assert_eq!(catalog, PHASES, "the Rust hook catalog IS the canonical five");
    for phase in PHASES {
        assert!(HookPhase::from_name(phase).is_some(), "Rust parses phase name {phase}");
    }

    // Python side: every canonical name present, none missing.
    let out =
        std::env::temp_dir().join(format!("piperine_plugin_parity_{}.txt", std::process::id()));
    python_missing_surface(&out, &[]);
    let missing = std::fs::read_to_string(&out).expect("read parity probe output");
    let _ = std::fs::remove_file(&out);
    assert!(
        missing.trim().is_empty(),
        "Python plugin surface is missing names the Rust host has:\n{missing} (MD-22 breach)"
    );
}

/// PLG-12's negative case (the "synthetic one-sided drift fails it"
/// done-when criterion): a name added to only one side of each surface
/// category is flagged by the same probe — proving the parity check
/// discriminates rather than vacuously passing.
#[test]
fn plugin_parity_flags_a_synthetic_one_sided_drift() {
    let drift: [(&str, &str); 4] = [
        ("decorator", "transform"),
        ("phase", "after_compile"),
        ("ctx", "fs_delete"),
        ("staging", "remove_instance"),
    ];
    let out = std::env::temp_dir()
        .join(format!("piperine_plugin_parity_drift_{}.txt", std::process::id()));
    python_missing_surface(&out, &drift);
    let missing = std::fs::read_to_string(&out).expect("read drift probe output");
    let _ = std::fs::remove_file(&out);
    for (kind, name) in drift {
        assert!(
            missing.lines().any(|l| l == format!("{kind}:{name}")),
            "the probe must flag the one-sided {kind} name `{name}`, got:\n{missing}"
        );
    }
    // The Rust side equally rejects the drifted phase name — the catalog is
    // frozen at five on BOTH hosts.
    assert!(HookPhase::from_name("after_compile").is_none(), "no sixth phase on the Rust side");
}
