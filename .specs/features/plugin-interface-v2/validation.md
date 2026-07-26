# Plugin Interface v2 — Validation

**Date**: 2026-07-25
**Spec**: `.specs/features/plugin-interface-v2/spec.md` (PLG-01..26)
**Diff range**: `65ddd73..HEAD` (T1–T17 committed as `65ddd73`…`15f28cf`, plus
the audit fixes below)
**Audit**: independent post-hoc read of the delivered surface against the
spec's acceptance criteria (author ≠ auditor), not a re-run of the per-task
Verifier.

## Gate

| Gate | Command | Result |
|---|---|---|
| Full suite | `cargo test --workspace` | ✅ 1123 passed, 0 failed (179 green targets) |
| Zero warnings | `cargo build --workspace` | ✅ no code warnings (only the `piperine-cli` build-script notice that the `piperine-python` `.so` is not built yet) |

---

## Requirement-Anchored Evidence

| Req | Criterion | Evidence | Result |
|---|---|---|---|
| PLG-01 | WASM + process backends and `piperine-plugin-wasm` gone | `crates/piperine-plugin/src/backend/mod.rs` (only `native`); no `piperine-plugin-wasm` crate; repo-wide grep for `wasm`/`wire_hosted`/`process.rs` in plugin code: zero | ✅ |
| PLG-02 | `abi = "wasm"\|"process"` → targeted removal error | `manifest.rs:167-178` `PluginError::RemovedBackend`; `tests/manifest.rs::removed_backends_are_a_targeted_error`, `::any_other_abi_field_is_rejected` | ✅ |
| PLG-03 | native device path still solves | `tests/e2e.rs::plugin_resistor_solves_dc`, `::plugin_inverter_runs_through_scheduler`, `::plugin_resistor_honors_param_override` | ✅ |
| PLG-04 | no imperative `Registrar` | grep `Registrar`/`attr_schema` over `crates/**/*.rs`: zero; `contributions.rs` exposes only the declaration snapshot (`Declared`) merged by the host (`host.rs:register_one`) | ✅ |
| PLG-05 | device = `#[pip::device]` + `@device` in the plugin's PHDL | `piperine-plugin-macros/src/lib.rs:27-71`; `registry.rs:18-29`; `tests/registration.rs`; the plugin package is an importable namespace (`piperine-project/src/source_map.rs::a_plugins_entry_is_an_importable_namespace` — **added by this audit**) | ✅ |
| PLG-06 | script/hook = one decorator, both hosts | `macros/src/lib.rs:78-169`; `python/piperine/__init__.py:1412-1477`; `tests/script_hook.rs`; `piperine-python` scripted bridge (`src/scripted.rs`) | ✅ |
| PLG-07 | `extern.phdl` stub loader deleted | grep `load_extern_stub`/`MissingExternStub`: zero; `tests/extern_stub.rs::shipped_extern_stub_is_inert_and_only_stdlib_schemas_seed` | ✅ |
| PLG-08 | no plugin-schema path | `contributions.rs` has no `schemas` map; module doc states the ban | ✅ |
| PLG-09 | only stdlib `@device`/`@port` seeded | `host.rs::seed_schemas` parses only `headers/device_port.phdl`; `tests/e2e.rs::device_attribute_requires_seeded_schema` | ✅ |
| PLG-10 | `@pip.script` dispatches like `#[pip::script]` | `scripted.rs::PythonScript::invoke`; `host.rs::run_script`; `tests/phase3.rs::script_runs_under_its_filesystem_capability` | ✅ |
| PLG-11 | five frozen phases, same `ctx` | `registry.rs:46-83` (`HookPhase::ALL`), `macros/src/lib.rs:173-230` per-phase payload; `host.rs::fire_declared` | ✅ |
| PLG-12 | decorator/hook/ctx name parity | `tests/plugin_parity.rs` — Rust side is a compile-time proof (every decorator/phase used for real, every `Ctx`/`DesignStaging` method called), Python side checked at runtime against the same canonical lists, incl. a synthetic-drift failure case | ✅ |
| PLG-13 | `staging.add_instance` injects | `view.rs:54-81`; `tests/inject.rs::injected_device_instance_solves_through_the_device_path` | ✅ |
| PLG-14 | injected `@device` solves | same test (solve reflects the injected resistor); `::injected_device_honors_staged_params` | ✅ |
| PLG-15 | bad injection fails loud | `tests/inject.rs::injection_to_an_unknown_parent_fails_loud`, `::injection_of_an_undeclared_module_fails_loud`, `::authored_structure_is_never_overwritten` (MD-25) | ✅ |
| PLG-16 | release → triple asset fetch + cache | `piperine-project/src/release.rs` (`ReleaseRef`, `PluginCache::fetch`); `tests/release.rs::fetch_downloads_hashes_and_caches_the_triple_asset` | ✅ |
| PLG-17 | TOFU pin `(release, triple, hash)` | `trust.rs::ensure_release_trusted` + `pin`; `tests/release_fetch.rs::an_approved_asset_is_pinned_as_release_triple_hash`, `::a_changed_asset_re_prompts_and_re_pins_on_accept` | ✅ |
| PLG-18 | `verify` checked up front | `trust.rs:91-100`; `tests/release_fetch.rs::a_verify_mismatch_hard_fails_without_a_prompt`, `::a_verify_match_loads_without_a_tofu_prompt` | ✅ |
| PLG-19 | unsupported triple loud | `release.rs::select_asset` → `NoAssetForTriple` (names triple + release); `release.rs::a_release_without_the_host_triple_fails_loud`, `tests/release_fetch.rs::a_wrong_triple_release_errors_naming_triple_and_release` | ✅ |
| PLG-20 | reproducible / offline-after-first | `PluginCache::fetch` pinned short-circuit; `tests/release_fetch.rs::a_second_machine_fetches_the_identical_asset_and_matches_the_pin_without_a_prompt`, `::a_cached_and_pinned_binary_loads_without_network`, `::an_offline_second_machine_without_cache_fails_loud` | ✅ |
| PLG-21 | shape inference, no `abi` | `manifest.rs::shape`; `tests/manifest.rs` (device / python / bare-section cases) | ✅ |
| PLG-22 | Go-style git source | `piperine-project/src/git.rs::GitSource::parse` (+ its unit tests); `piperine-cli/src/commands/add.rs:41-50` | ✅ |
| PLG-23 | permissions consent at `add` | `trust.rs::ensure_permissions_consented`; `add.rs:113-134` (deny reverts `Piperine.toml` **and** `Piperine.lock`); `piperine-cli/tests/add_cmd.rs` | ✅ |
| PLG-24 | device ABI-version check kept | `backend/native.rs:30` `piperine_plugin_abi_version` symbol check at dlopen | ✅ |
| PLG-25 | one worked example per shape | `docs/spec/part_vi_plugins.md` Appendix A.1–A.4 (pure-PHDL / scripted / device + the consolidated Rust≡Python table) | ✅ |
| PLG-26 | Part VI rewritten to v2 | `docs/spec/part_vi_plugins.md` — no stale WASM/process/Registrar/extern-stub; v2 shapes, trust gates, release distribution | ✅ |

---

## Findings and fixes applied by this audit

The 17 tasks were implemented as written; the gaps below were **under-specified
by tasks.md relative to `design.md` §1 / D9–D10**, not task regressions. All
are fixed in this pass, with tests.

1. **`piperine add` did not actually install a plugin.** `add` writes a
   `[dependencies]` entry, but the host loaded only `[plugins]` entries, so a
   plugin added through the documented command was never loaded (D9: "no
   separate install path"). Fix: `Resolver::resolve_plugins` also resolves
   every `[dependencies]` entry that declares contributions
   (`crates/piperine-project/src/resolver.rs`), and `PluginHost::load` no
   longer short-circuits on an empty `[plugins]` section.
   Tests: `resolver::tests::a_contributing_dependency_resolves_as_a_plugin`,
   `::an_explicit_plugins_entry_wins_over_the_dependency_path`,
   `piperine-plugin/tests/native_smoke.rs::a_contributing_dependency_loads_as_a_plugin`,
   `::a_plain_dependency_is_not_a_plugin`.
2. **A plugin's own PHDL was not importable.** D10 has the author write
   `@device pub mod …` in the plugin package and the user `use` it, but only
   `[dependencies]` became `SourceMap` namespaces — a `[plugins]` package's
   PHDL was unreachable. Fix: `project_source_map` registers plugin packages
   as namespaces too (project/dependency names still win).
   Test: `source_map::tests::a_plugins_entry_is_an_importable_namespace`.
3. **The manifest spelling did not match the design.** `design.md` §1 and
   `ideal.md` D9 declare contributions in the package's own `Piperine.toml`
   `[plugin]` section; only a dedicated `piperine-plugin.toml` was read. Fix:
   `Manifest::load` accepts both (dedicated file wins; inline `name` defaults
   to `[project].name`; `[plugin.permissions]` lifts to permissions).
   Tests: `manifest.rs::an_inline_plugin_section_is_a_manifest`,
   `::a_dedicated_manifest_wins_over_the_inline_section`,
   `::a_package_declaring_no_contributions_fails_loud`,
   `resolver::tests::an_inline_plugin_section_marks_a_dependency_as_a_plugin`.
4. **Stale docs.** `CLAUDE.md` and `AGENTS.md` still listed
   `piperine-plugin-wasm` and "native/WASM/process backends" (and the removed
   `wasm_smoke`/`process_smoke` tests); `view.rs`'s module doc still described
   an out-of-host WASM/process tier. All corrected, plus `part_vi_plugins.md`
   §4/§5.1/§5.2 updated for the two manifest spellings and the
   dependency-is-a-plugin resolution, and ROADMAP P5 marked delivered.

## Non-findings worth recording

- `host.rs` runs both `ensure_release_trusted` and `ensure_trusted` on a
  release-fetched artifact. Not a double prompt: the cache is
  content-addressed and `artifact_hash` produces the same `sha256:…` string
  the release pin recorded, so the second call hits the matching pin and
  returns without prompting.
- The Python `transform_design` bridge hands the hook a `design.clone()`.
  Staged writes are **not** lost: `Design`'s staging area is
  `Rc<RefCell<OverrideMap>>` and `derive(Clone)` shares it (`fork()` is the
  explicit "own, empty staging" path), so injections reach the real design.
  Covered by `phase3.rs::transform_design_injects_a_declared_instance`.

## Deferred (unchanged, per context.md)

- D12 — manifest `[plugin] piperine = ">=X.Y"` version-compat field.
- Committed-in-repo device binary as an offline fallback.
- Build-from-source for an unsupported triple (D6 keeps it a loud error).
- A full C-ABI `Element` vtable (D13) — the dlopen crossing stays Rust.
