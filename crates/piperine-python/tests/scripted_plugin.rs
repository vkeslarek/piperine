//! Scripted-plugin integration test (plugin-interface v2, PLG-06/10/11 —
//! T8): a `.py` plugin whose contributions are ALL declared by `@pip`
//! decorators loads through the embedded-CPython bridge; `@pip.script`
//! dispatches through the host's script table, and every one of the five
//! frozen `@pip.hook.<phase>` declarations fires with its phase payload.
//! The fixture (`scripted_plugin/plugin.py`) writes marker files through
//! the capability-gated ctx; this test reads them back as the dispatch
//! proof. `lint_tb.py` (run by `piperine test`) exercises the decorator
//! surface itself — the per-load registration table, the frozen hook
//! catalog, and declaration-time conflict errors.

use std::path::{Path, PathBuf};

use piperine_api::SimHooks;
use piperine_lang::SourceMap;
use piperine_plugin::{Manifest, PluginHost, ScriptedHost};
use piperine_python::scripted::EmbeddedScriptedHost;

/// The fixture plugin directory (its `plugin.py` is the scripted entry).
fn fixture_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/scripted_plugin"))
}

/// A manifest for the fixture: scripted shape, with the filesystem-write
/// capability the marker files need (deny-by-default otherwise — P0002).
fn fixture_manifest() -> Manifest {
    Manifest::parse(
        "glue",
        "[plugin]\nname = \"glue\"\npython = \"plugin.py\"\n\n\
         [permissions]\nfilesystem = [\"write *.txt\"]\n",
    )
    .expect("fixture manifest parses")
}

/// A unique, empty project root per test (marker files land here via the
/// capability-gated `ctx.fs_write`).
fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("piperine_scripted_{}_{}", std::process::id(), tag));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp project root");
    dir
}

/// Load the fixture plugin through the embedded scripted host into a
/// `PluginHost` rooted at `root`.
fn load_host(root: &Path) -> PluginHost {
    let plugin = EmbeddedScriptedHost
        .load_scripted(&fixture_dir(), &fixture_manifest())
        .expect("scripted plugin loads");
    PluginHost::from_plugins(vec![plugin]).expect("host").with_project_root(root)
}

fn read_marker(root: &Path, name: &str) -> String {
    std::fs::read_to_string(root.join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"))
}

/// A trivial design to hand the elaboration hooks.
fn design() -> piperine_lang::Design {
    piperine_lang::parse_and_elaborate(
        "discipline Electrical { potential v: Real; flow i: Real; }\n\
         mod Top() { wire gnd : Electrical; }\n",
        &SourceMap::dummy(),
    )
    .expect("elaborate")
}

/// PLG-06: declaration and binding are ONE decorator — the host's
/// registration table (read back after exec-ing the entry) names exactly
/// the scripts the fixture declared, with no separate register call.
#[test]
fn decorator_declarations_land_in_the_host_table() {
    let root = temp_root("table");
    let host = load_host(&root);
    let description = host.describe().join("\n");
    assert!(description.contains("glue (scripted)"), "shape inferred: {description}");
    assert!(description.contains("lint"), "declared script surfaced: {description}");
    assert!(description.contains("design_probe"), "declared script surfaced: {description}");
    let _ = std::fs::remove_dir_all(&root);
}

/// PLG-10: `@pip.script("lint")` dispatches `lint` with the CLI args and
/// its return value is the exit code; an undeclared name does not
/// dispatch (`None` — the CLI turns it into the loud P0009).
#[test]
fn script_dispatches_with_args_and_exit_code() {
    let root = temp_root("script");
    let host = load_host(&root);
    let args = vec!["--strict".to_string(), "a.phdl".to_string()];
    let outcome = host.run_script("lint", &args);
    assert!(matches!(outcome, Some(Ok(0))), "lint dispatches to exit 0: {outcome:?}");
    assert_eq!(
        read_marker(&root, "lint_args.txt"),
        "--strict a.phdl",
        "the handler observed the exact CLI args"
    );
    assert!(host.run_script("not_declared", &[]).is_none(), "unknown script: no dispatch");
    let _ = std::fs::remove_dir_all(&root);
}

/// PLG-11 (ctx surface): `ctx.design()` is the elaboration-hook surface —
/// calling it from a script fails loud, never a silent empty design.
#[test]
fn script_design_access_fails_loud() {
    let root = temp_root("probe");
    let host = load_host(&root);
    let outcome = host.run_script("design_probe", &[]);
    let Some(Err(err)) = outcome else {
        panic!("ctx.design() from a script must fail loud, got {outcome:?}");
    };
    assert!(
        err.to_string().contains("ctx.design()"),
        "error names the violated contract: {err}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// PLG-11: all five frozen `@pip.hook.<phase>` declarations fire at their
/// phase, each with its spec-defined payload — `after_parse` the source,
/// the design hooks a readable `ctx.design()`, `transform_design` a
/// staging handle, `after_solve` the analysis result.
#[test]
fn all_five_hooks_fire_with_their_payloads() {
    let root = temp_root("hooks");
    let host = load_host(&root);
    let design = design();

    let source = "mod SourceProbe() {}\n";
    host.fire_after_parse(source).expect("after_parse fires");
    assert_eq!(
        read_marker(&root, "phase_after_parse.txt"),
        format!("source:{}", source.len()),
        "after_parse received the raw source"
    );

    host.fire_after_elaborate(&design).expect("after_elaborate fires");
    assert_eq!(
        read_marker(&root, "phase_after_elaborate.txt"),
        "design:True",
        "after_elaborate received a readable design"
    );

    host.transform_design(&design).expect("transform_design fires");
    assert_eq!(
        read_marker(&root, "phase_transform_design.txt"),
        "staging:True",
        "transform_design received the staging surface"
    );

    host.before_lower(&design).expect("before_lower fires");
    assert_eq!(
        read_marker(&root, "phase_before_lower.txt"),
        "design:True",
        "before_lower received a readable design"
    );

    host.after_solve("op", &[("out".to_string(), 1.0)]).expect("after_solve fires");
    assert_eq!(
        read_marker(&root, "phase_after_solve.txt"),
        "analysis:op",
        "after_solve received the analysis result"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// T8 Done-when: the `*_tb.py` testbench exercises the decorator surface
/// itself — the per-load registration table contents, the frozen
/// five-phase catalog (an unknown phase raises), and declaration-time
/// duplicate-name conflicts.
#[test]
fn testbench_exercises_the_decorator_surface() {
    let tb = fixture_dir().join("lint_tb.py");
    piperine_python::embed::run_script(tb.to_str().expect("utf8 path"))
        .expect("lint_tb.py decorator assertions pass");
}
