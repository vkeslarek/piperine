//! PLG-06/11: `#[pip::script("name")]` and `#[pip::hook(phase)]` declare AND
//! bind a contribution in one attribute — the script dispatches `piperine
//! <name>` with its args + `&Ctx`; each hook fires with its phase's payload
//! (`ctx.design()` for the design hooks, `&DesignStaging` additionally for
//! `transform_design`, source for `after_parse`, the result view for
//! `after_solve`). This binary's registry contains exactly the declarations
//! below.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use piperine_lang::SourceMap;
use piperine_plugin::{
    Ctx, DesignStaging, HookCall, HookPhase, HostCtx, Permissions, Registry, SolveResultView,
};

const SRC: &str = "mod Top() {}";

fn host_ctx() -> HostCtx {
    HostCtx::new("probe", Path::new("."), Permissions::default())
}

// ─── #[pip::script] ───────────────────────────────────────────────────────────

#[piperine_plugin_macros::script("lint")]
fn lint(args: &[String], ctx: &Ctx) -> Result<i32, String> {
    assert_eq!(ctx.project_root(), Path::new("."), "the script ctx carries the project root");
    Ok(args.len() as i32)
}

#[test]
fn script_registers_under_its_name_and_dispatches_with_args() {
    let names: Vec<&str> = Registry::scripts().map(|s| s.name).collect();
    assert_eq!(names, ["lint"], "one decorator → one registration, keyed by name");

    let reg = Registry::scripts().next().expect("registered");
    let handler = (reg.make)();
    let mut cx = host_ctx();
    let code = handler.invoke(&["a.phdl".into(), "b.phdl".into()], &mut cx).expect("script ok");
    assert_eq!(code, 2, "the script receives its CLI args and its exit code propagates");
}

// ─── #[pip::hook] — the five frozen phases ────────────────────────────────────

static PARSED: AtomicUsize = AtomicUsize::new(0);
static ELABORATED: AtomicUsize = AtomicUsize::new(0);
static TRANSFORMED: AtomicUsize = AtomicUsize::new(0);
static LOWERED: AtomicUsize = AtomicUsize::new(0);
static SOLVED: AtomicUsize = AtomicUsize::new(0);

#[piperine_plugin_macros::hook(after_parse)]
fn on_parse(_ctx: &Ctx, source: &str) -> Result<(), String> {
    if !source.contains("mod Top") {
        return Err("after_parse must see the raw source".into());
    }
    PARSED.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

#[piperine_plugin_macros::hook(after_elaborate)]
fn on_elaborate(ctx: &Ctx) -> Result<(), String> {
    if ctx.design().module("Top").is_none() {
        return Err("after_elaborate's ctx.design() must expose the elaborated design".into());
    }
    ELABORATED.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

#[piperine_plugin_macros::hook(transform_design)]
fn on_transform(ctx: &Ctx, staging: &DesignStaging) -> Result<(), String> {
    if ctx.design().module("Top").is_none() || staging.design().module("Top").is_none() {
        return Err("transform_design gets both ctx.design() and the staging surface".into());
    }
    TRANSFORMED.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

#[piperine_plugin_macros::hook(before_lower)]
fn on_before_lower(ctx: &Ctx) -> Result<(), String> {
    if ctx.design().module("Top").is_none() {
        return Err("before_lower's ctx.design() must expose the design".into());
    }
    LOWERED.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

#[piperine_plugin_macros::hook(after_solve)]
fn on_solve(_ctx: &Ctx, result: &SolveResultView) -> Result<(), String> {
    if result.analysis != "op" {
        return Err(format!("after_solve must see the analysis kind, got `{}`", result.analysis));
    }
    SOLVED.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

#[test]
fn hook_registrations_cover_exactly_the_five_frozen_phases() {
    let mut phases: Vec<HookPhase> = Registry::hooks().map(|h| h.phase).collect();
    phases.sort_by_key(|p| p.as_str());
    let mut all = HookPhase::ALL.to_vec();
    all.sort_by_key(|p| p.as_str());
    assert_eq!(phases, all, "one decorator per frozen phase → five registrations");
}

#[test]
fn each_hook_fires_with_its_phase_payload() {
    let design = piperine_lang::parse_and_elaborate(SRC, &SourceMap::dummy()).expect("elaborates");
    let cx = host_ctx();
    let staging = DesignStaging::new(&design, "probe");
    let result = SolveResultView { analysis: "op".into(), node_voltages: Vec::new() };
    let call = HookCall {
        host: &cx,
        source: Some(SRC),
        design: Some(&design),
        staging: Some(&staging),
        result: Some(&result),
    };
    for reg in Registry::hooks() {
        (reg.invoke)(&call).unwrap_or_else(|e| panic!("{} hook failed: {e}", reg.phase.as_str()));
    }
    for (name, counter) in [
        ("after_parse", &PARSED),
        ("after_elaborate", &ELABORATED),
        ("transform_design", &TRANSFORMED),
        ("before_lower", &LOWERED),
        ("after_solve", &SOLVED),
    ] {
        assert_eq!(counter.load(Ordering::SeqCst), 1, "{name} fired exactly once with its payload");
    }
}
