# Plugin Interface v2 Tasks

## Execution Protocol (MANDATORY — do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name
and follow its Execute flow and Critical Rules** (per-task cycle, sub-agent
delegation offer, adequacy review, Verifier, discrimination sensor). Do not
search for skill files by path. **If the skill cannot be activated, STOP and
tell the user.**

**Ideal:** `.specs/features/plugin-interface-v2/ideal.md`
**Spec:** `.specs/features/plugin-interface-v2/spec.md` (PLG-01..26)
**Context (locked decisions D1–D14):** `.specs/features/plugin-interface-v2/context.md`
**Design:** `.specs/features/plugin-interface-v2/design.md`
**Status:** In Progress (Execute started 2026-07-25, branch `feature/bench-removal`)

---

## Test Coverage Matrix

> Generated from codebase sampling + `CLAUDE.md` ("zero warnings is the bar",
> `cargo test --workspace`). Guidelines found: `CLAUDE.md` (build/test
> conventions, "fail loud" rule, tests-of-record list). Confirm before Execute.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
|---|---|---|---|---|
| `piperine-plugin` (manifest, host, backend, contributions, trust) | integration | All ACs; happy + fail-loud + edge | `crates/piperine-plugin/tests/*.rs` | `cargo test -p piperine-plugin` |
| `piperine-plugin-macros` (proc-macros) | unit + compile | macro expands to the right registration; a mis-use fails to compile | `crates/piperine-plugin-macros/tests/*.rs` (+ `trybuild`) | `cargo test -p piperine-plugin-macros` |
| `piperine-project` (git/release resolver, lockfile pin) | unit | resolver mapping, triple match, TOFU pin, verify-hash, reproducibility | `crates/piperine-project/tests/*.rs` or inline `#[cfg(test)]` | `cargo test -p piperine-project` |
| `piperine-cli` (`add`, script dispatch, permissions prompt) | integration | happy + deny + each shape | `crates/piperine-cli/tests/*.rs` | `cargo test -p piperine-cli` |
| `piperine-python` (facade decorators) | integration | each decorator registers + dispatches | `crates/piperine-python/tests/*.rs` + `*_tb.py` | `cargo build -p piperine-python --features extension-module && cargo test -p piperine-python` |
| Cross-host parity | integration | decorator/hook/ctx names identical | root `tests/plugin_parity.rs` | `cargo test -p piperine plugin_parity` |
| Docs (`part_vi_plugins.md`, worked examples) | none | completeness | `docs/spec/` | build/review gate |

## Gate Check Commands

| Gate | When | Command |
|---|---|---|
| Quick (plugin) | plugin-crate tasks | `cargo test -p piperine-plugin` |
| Quick (macros) | proc-macro tasks | `cargo test -p piperine-plugin-macros` |
| Quick (project) | resolver/lockfile tasks | `cargo test -p piperine-project` |
| Quick (cli) | CLI tasks | `cargo test -p piperine-cli` |
| Quick (python) | facade tasks | `cargo build -p piperine-python --features extension-module && cargo test -p piperine-python` |
| Full | workspace-membership / cross-crate / parity | `cargo test --workspace` |
| Build | docs-only | `cargo build --workspace` + review |

---

## Execution Plan

Phases are ordered and run sequentially.

### Phase 1 — Backend reduction + manifest + extern-stub removal (safe deletion)

```
T1 → T2 → T3 → T4
```

### Phase 2 — Macro surface + device/script/hook migration (the pivot)

```
T5 → T6 → T7 → T8 → T9
```

### Phase 3 — Device injection

```
T10
```

### Phase 4 — Install + distribution

```
T11 → T12 → T13 → T14 → T15
```

### Phase 5 — Docs

```
T16 → T17
```

**Sequencing note (critical):** the imperative `Registrar::{device,script}`
is deleted in **Phase 2 (T7)**, coupled with the `#[pip::…]` macro
replacement — NOT in Phase 1 — because the native device path
(`e2e.rs` fixture) must keep working across the boundary. Phase 1 deletes
only what has no device-registration impact (backends, manifest `abi`,
extern stubs, the plugin-schema surface).

---

## Task Breakdown

### T1: Delete the WASM backend + `piperine-plugin-wasm` crate
**What:** Remove `crates/piperine-plugin-wasm`, `backend/wasm.rs`, and their
workspace/`backend/mod.rs` references.
**Where:** `crates/piperine-plugin-wasm/` (delete), `crates/piperine-plugin/src/backend/wasm.rs` (delete), `crates/piperine-plugin/src/backend/mod.rs`, root `Cargo.toml` (workspace members), `crates/piperine-plugin/Cargo.toml` (wasmtime deps).
**Depends on:** None. **Requirement:** PLG-01.
**Reuses:** —
**Tools:** MCP: NONE · Skill: NONE
**Done when:**
- [x] `crates/piperine-plugin-wasm` no longer exists; not in workspace members.
- [x] `backend/wasm.rs` gone; `backend/mod.rs` has no `wasm` module.
- [x] `wasm_smoke.rs` test deleted.
- [x] `cargo test --workspace` green (no dangling refs).
**Tests:** integration (existing suite stays green) · **Gate:** full

### T2: Delete the process backend
**What:** Remove `backend/process.rs` + `backend/wire_hosted.rs` + refs.
**Where:** `crates/piperine-plugin/src/backend/{process,wire_hosted}.rs` (delete), `backend/mod.rs`.
**Depends on:** T1. **Requirement:** PLG-01.
**Reuses:** —
**Tools:** MCP: NONE · Skill: NONE
**Done when:**
- [x] `process.rs`/`wire_hosted.rs` gone; `backend/mod.rs` keeps only `native`.
- [x] `process_smoke.rs` deleted.
- [x] `cargo test -p piperine-plugin` green.
**Tests:** integration · **Gate:** quick (plugin)

### T3: Manifest shape inference (drop `abi`/`entry`, add `python`/`device`)
**What:** Remove `Abi` enum + `abi`/`entry`/`default_timeout` fields; add
`python: Option<PathBuf>` + `device: Option<DeviceSource>`; infer shape from
which keys are present; a manifest with `abi = "wasm"|"process"` yields a
targeted `PluginError::RemovedBackend`.
**Where:** `crates/piperine-plugin/src/manifest.rs`, `error.rs`.
**Depends on:** T2. **Requirement:** PLG-02, PLG-21.
**Reuses:** existing `Manifest`/`Permissions` structs.
**Done when:**
- [x] No `abi`/`entry` field; `Abi` enum deleted.
- [x] `python` present → scripted; `device` present → device; neither → pure-PHDL — a `Manifest::shape()` returns the inferred shape.
- [x] `abi = "wasm"` manifest → `RemovedBackend` error (message names the removed backend), NOT a generic unknown-field error.
- [x] `manifest.rs` tests updated + a new shape-inference + RemovedBackend test.
- [x] `cargo test -p piperine-plugin` green.
**Tests:** integration · **Gate:** quick (plugin)

### T4: Delete the plugin-schema surface + extern-stub loader
**What:** Delete `Registrar::attr_schema`, the `Contributions.schemas` map,
`host.rs::load_extern_stub`, `PluginError::MissingExternStub`, and the
per-plugin `extern.phdl` auto-import; `seed_schemas` keeps seeding ONLY the
stdlib `@device`/`@port` from `headers/device_port.phdl`.
**Where:** `crates/piperine-plugin/src/{contributions.rs,host.rs,error.rs}`.
**Depends on:** T3. **Requirement:** PLG-07, PLG-08, PLG-09.
**Reuses:** `seed_schemas`, `device_port.phdl` (kept).
**Done when:**
- [x] No `attr_schema` / `Contributions.schemas` / `load_extern_stub` / `MissingExternStub`.
- [x] A plugin fixture carrying an `extern.phdl` is inert / rejected (no per-plugin stub loaded).
- [x] `seed_schemas` still seeds `@device`/`@port` (a device fixture still validates its `@device`).
- [x] `extern_stub.rs`/`schema_stub.rs` tests removed or rewritten to the new (no-plugin-schema) behavior.
- [x] `cargo test -p piperine-plugin` green.
**Tests:** integration · **Gate:** quick (plugin)

### T5: Scaffold `piperine-plugin-macros` + `#[pip::device]`
**What:** New proc-macro crate; `#[pip::device("Type")]` on an `Element`
type registers it into a device-registration collector (`inventory` or a
generated accumulator — design §3/§7) keyed by the `type` string.
**Where:** `crates/piperine-plugin-macros/` (new), root `Cargo.toml` members.
**Depends on:** T4. **Requirement:** PLG-05, PLG-24.
**Reuses:** `PluginDeviceSpec`/`DeviceProvider` shapes (`piperine-codegen/src/device/plugin.rs`); native ABI (`backend/native.rs`, kept).
**Done when:**
- [ ] `#[pip::device("Foo")]` expands to a registration the host can read at load.
- [ ] A `trybuild`/unit test proves expansion + that a malformed use fails to compile.
- [ ] `cargo test -p piperine-plugin-macros` green.
**Tests:** unit + compile · **Gate:** quick (macros)

### T6: `#[pip::script]` + `#[pip::hook(phase)]` proc-macros
**What:** `#[pip::script("name")]` registers a CLI subcommand handler;
`#[pip::hook(after_parse|after_elaborate|transform_design|before_lower|after_solve)]`
registers a hook for one of the five frozen phases; hooks receive `&Ctx`
(with `ctx.design() -> &Design`; `transform_design` also `&DesignStaging`).
**Where:** `crates/piperine-plugin-macros/src/`.
**Depends on:** T5. **Requirement:** PLG-06, PLG-11.
**Reuses:** the five `Plugin`-trait hook signatures (kept as the internal target).
**Done when:**
- [ ] Both macros expand to registrations; an unknown hook phase name fails to compile.
- [ ] `cargo test -p piperine-plugin-macros` green.
**Tests:** unit + compile · **Gate:** quick (macros)

### T7: Wire macro contributions into the host + delete the imperative Registrar
**What:** The host reads the macro-collected device/script/hook tables at
load; DELETE `Registrar::{device,script}` (now replaced); migrate the
`e2e.rs` native-device fixture to `#[pip::device]`. **This is the pivot** —
native devices must still solve after it (PLG-03).
**Where:** `crates/piperine-plugin/src/{contributions.rs,host.rs,backend/native.rs}`, `crates/piperine-plugin/tests/e2e.rs` (+ its fixture crate).
**Depends on:** T6. **Requirement:** PLG-03, PLG-04, PLG-05, PLG-24.
**Reuses:** the kept native ABI (`piperine_plugin_entry`/`ABI_VERSION`).
**Done when:**
- [ ] No public `Registrar::{device,script,attr_schema}` remains (grep: zero).
- [ ] The `e2e.rs` fixture declares its device via `#[pip::device]` (no imperative register); `plugin_resistor_solves_dc` + `plugin_inverter_runs_through_scheduler` still pass.
- [ ] `cargo test -p piperine-plugin` green.
**Tests:** integration · **Gate:** quick (plugin)

### T8: Python decorators `@pip.script`/`@pip.hook`/`@pip.device`
**What:** Add the decorators to the Python facade; the embedded host reads
the per-load registration table after exec-ing the plugin's `python` entry
and wires scripts/hooks (device decorator marks the `@device` binding for a
Python-glue plugin).
**Where:** `crates/piperine-python/python/piperine/__init__.py`, `crates/piperine-python/src/*` (host readback), `.pyi` stub.
**Depends on:** T7. **Requirement:** PLG-06, PLG-10, PLG-11.
**Reuses:** embedded CPython host (`piperine-python/src/embed.rs`), the frozen hook catalog.
**Done when:**
- [ ] A `.py` plugin with `@pip.script("lint")` makes `piperine lint …` dispatch; a `@pip.hook.after_elaborate` fires.
- [ ] `cargo test -p piperine-python` green (+ a `*_tb.py` exercising a decorator).
**Tests:** integration · **Gate:** quick (python)

### T9: Cross-host decorator/hook/ctx parity test
**What:** `tests/plugin_parity.rs` enumerates the decorator names, the five
hook phase names, and the `ctx`/`staging` method names on both hosts and
asserts identical — a name added on one side without the other fails.
**Where:** root `tests/plugin_parity.rs` (mirror `tests/host_parity.rs`).
**Depends on:** T8. **Requirement:** PLG-12.
**Reuses:** `host_parity.rs` technique.
**Done when:**
- [ ] The parity test asserts name-identical surfaces; a synthetic drift (a name on one host only) fails it.
- [ ] `cargo test -p piperine plugin_parity` green.
**Tests:** integration · **Gate:** full

### T10: Device injection via `transform_design` staging
**What:** A `transform_design` hook calls `staging.add_instance(parent,
label, module, ports…)` to inject a `@device` module; the injected device
appears in the analysed design and solves; a bad parent/module fails loud;
authored structure is never overwritten (MD-25).
**Where:** `crates/piperine-plugin/src/view.rs` (reuse `add_instance`), a new `crates/piperine-plugin/tests/inject.rs`.
**Depends on:** T9. **Requirement:** PLG-13, PLG-14, PLG-15.
**Reuses:** `DesignStaging::add_instance` (`view.rs:51`), the device provider path.
**Done when:**
- [ ] A hook injects a parasitic `@device` resistor into `Top`; the solve reflects it.
- [ ] Injection to a non-existent parent/module errors loud.
- [ ] `cargo test -p piperine-plugin` green.
**Tests:** integration · **Gate:** quick (plugin)

### T11: Git-source resolver (Go-style)
**What:** `piperine add <git>` resolution: a bare `owner/repo` →
`https://github.com/owner/repo`; a full git URL (`https://…`, `git@…`) is
used verbatim.
**Where:** `crates/piperine-project/src/` (the dependency resolver — extend the existing git dep resolution), `crates/piperine-cli/src/commands/add.rs`.
**Depends on:** T10. **Requirement:** PLG-22.
**Reuses:** the existing `piperine add` git-dependency resolver.
**Done when:**
- [ ] `add acme/bjt` resolves to `github.com/acme/bjt`; `add https://…`/`git@…` verbatim.
- [ ] Unit tests for each form.
- [ ] `cargo test -p piperine-project` green.
**Tests:** unit · **Gate:** quick (project)

### T12: Release-asset resolver by target triple
**What:** Resolve `device = { release = "github:owner/repo@tag" }` to the
release asset whose name matches `lib<pkg>-<host-triple>.<ext>`; no match →
`PluginError::NoAssetForTriple { triple, release }` (loud).
**Where:** `crates/piperine-project/src/release.rs` (new), `crates/piperine-plugin/src/error.rs`.
**Depends on:** T11. **Requirement:** PLG-16, PLG-19.
**Reuses:** target-triple detection (`env!`/`std`), the manifest `DeviceSource` (T3).
**Done when:**
- [ ] A stubbed release listing → the correct triple asset is selected.
- [ ] A release with no matching triple → `NoAssetForTriple` naming triple + release.
- [ ] `cargo test -p piperine-project` green.
**Tests:** unit · **Gate:** quick (project)

### T13: Fetch + cache + TOFU-pin the device binary (+ `verify`)
**What:** Download the resolved asset to a per-user cache; hash it; if
`verify = "sha256:…"` compare up front (mismatch = hard fail, no prompt);
else TOFU (prompt/accept/reject by `TrustMode`); pin `(release-url, triple,
content-hash)` in `Piperine.lock` (`EntryKind::Plugin`).
**Where:** `crates/piperine-project/src/release.rs`, `crates/piperine-plugin/src/trust.rs` (reuse), `crates/piperine-project/src/lockfile.rs` (reuse `EntryKind::Plugin`).
**Depends on:** T12. **Requirement:** PLG-16, PLG-17, PLG-18.
**Reuses:** `trust.rs::{artifact_hash,ensure_trusted}`, `lockfile` `EntryKind::Plugin`/`content_hash`/`abi`.
**Done when:**
- [ ] A fetched asset is hashed + pinned; a changed asset re-prompts.
- [ ] `verify` mismatch hard-fails (no prompt); match loads without a prompt.
- [ ] `cargo test -p piperine-project` (or `-p piperine-plugin`) green.
**Tests:** integration · **Gate:** full

### T14: `piperine add` permissions-consent gate
**What:** On adding a dependency that declares `[plugin.permissions]`, print
the declared permissions and require an explicit accept/deny; deny aborts
the install. Distinct from the artifact-hash TOFU (T13).
**Where:** `crates/piperine-cli/src/commands/add.rs`.
**Depends on:** T13. **Requirement:** PLG-23.
**Reuses:** `Permissions` (manifest), the interactive-prompt pattern in `trust.rs`.
**Done when:**
- [ ] Adding a plugin prints its permissions; an explicit deny aborts (nothing installed); accept proceeds.
- [ ] `RejectUntrusted`/`AcceptAll` modes bypass the prompt deterministically.
- [ ] `cargo test -p piperine-cli` green.
**Tests:** integration · **Gate:** quick (cli)

### T15: Reproducible / offline-after-first fetch
**What:** A locked-and-approved plugin entry short-circuits the fetch — a
second machine fetches the identical asset and matches the pinned hash (no
prompt); a cached+pinned binary loads without network.
**Where:** `crates/piperine-project/src/release.rs`, `lockfile.rs`.
**Depends on:** T14. **Requirement:** PLG-20.
**Reuses:** the pin from T13.
**Done when:**
- [ ] A pre-populated lockfile + cache loads without a prompt or re-download.
- [ ] A missing-network + cached path still loads.
- [ ] `cargo test -p piperine-project` green.
**Tests:** integration · **Gate:** quick (project)

### T16: Rewrite `docs/spec/part_vi_plugins.md` to v2
**What:** Rewrite the normative plugin spec to the v2 surface (native +
Python, three shapes, declaration coupling, no Registrar/extern-stub/WASM,
release distribution, TOFU + permissions).
**Where:** `docs/spec/part_vi_plugins.md`.
**Depends on:** T15. **Requirement:** PLG-26.
**Reuses:** the delivered surface (T1–T15).
**Done when:**
- [ ] No stale WASM/process/Registrar/extern-stub; describes the v2 model.
- [ ] Build/review gate.
**Tests:** none · **Gate:** build

### T17: One "write a plugin" worked example per shape
**What:** A worked example for each shape (pure-PHDL, scripted, device),
showing the Rust/Python decorator equivalence side by side.
**Where:** `docs/spec/part_vi_plugins.md` (or an appendix) + `examples/`-style snippets.
**Depends on:** T16. **Requirement:** PLG-25.
**Reuses:** the fixtures from T7/T8/T10.
**Done when:**
- [ ] One example per shape; Rust ≡ Python decorator equivalence shown.
- [ ] Build/review gate.
**Tests:** none · **Gate:** build

---

## Phase Execution Map

```
Phase 1: T1 → T2 → T3 → T4
Phase 2: T5 → T6 → T7 → T8 → T9
Phase 3: T10
Phase 4: T11 → T12 → T13 → T14 → T15
Phase 5: T16 → T17
```

17 tasks → ~3 batches (Phase 1+2 = 9; Phase 3+4 = 6; Phase 5 = 2 → fold into
prior). Sub-agent offer applies at Execute.

---

## Requirement → Task Coverage

| Req | Task(s) | Req | Task(s) |
|---|---|---|---|
| PLG-01 | T1,T2 | PLG-14 | T10 |
| PLG-02 | T3 | PLG-15 | T10 |
| PLG-03 | T7 | PLG-16 | T12,T13 |
| PLG-04 | T7 | PLG-17 | T13 |
| PLG-05 | T5,T7 | PLG-18 | T13 |
| PLG-06 | T6,T8 | PLG-19 | T12 |
| PLG-07 | T4 | PLG-20 | T15 |
| PLG-08 | T4 | PLG-21 | T3 |
| PLG-09 | T4 | PLG-22 | T11 |
| PLG-10 | T8 | PLG-23 | T14 |
| PLG-11 | T6,T8 | PLG-24 | T5,T7 |
| PLG-12 | T9 | PLG-25 | T17 |
| PLG-13 | T10 | PLG-26 | T16 |

All 26 requirements mapped.

---

## Diagram-Definition Cross-Check

| Task | Depends On (body) | Diagram | Status |
|---|---|---|---|
| T1 | None | (start) | ✅ |
| T2 | T1 | T1→T2 | ✅ |
| T3 | T2 | T2→T3 | ✅ |
| T4 | T3 | T3→T4 | ✅ |
| T5 | T4 | T4→T5 | ✅ |
| T6 | T5 | T5→T6 | ✅ |
| T7 | T6 | T6→T7 | ✅ |
| T8 | T7 | T7→T8 | ✅ |
| T9 | T8 | T8→T9 | ✅ |
| T10 | T9 | T9→T10 | ✅ |
| T11 | T10 | T10→T11 | ✅ |
| T12 | T11 | T11→T12 | ✅ |
| T13 | T12 | T12→T13 | ✅ |
| T14 | T13 | T13→T14 | ✅ |
| T15 | T14 | T14→T15 | ✅ |
| T16 | T15 | T15→T16 | ✅ |
| T17 | T16 | T16→T17 | ✅ |

All backward/same-phase; no forward deps.

## Test Co-location Validation

| Task | Layer created/modified | Matrix requires | Task says | Status |
|---|---|---|---|---|
| T1 | plugin (workspace) | integration | integration | ✅ |
| T2 | plugin backend | integration | integration | ✅ |
| T3 | plugin manifest | integration | integration | ✅ |
| T4 | plugin host/contributions | integration | integration | ✅ |
| T5 | plugin-macros | unit+compile | unit+compile | ✅ |
| T6 | plugin-macros | unit+compile | unit+compile | ✅ |
| T7 | plugin host+native | integration | integration | ✅ |
| T8 | python facade | integration | integration | ✅ |
| T9 | parity | integration | integration | ✅ |
| T10 | plugin view/inject | integration | integration | ✅ |
| T11 | project resolver | unit | unit | ✅ |
| T12 | project release | unit | unit | ✅ |
| T13 | project+trust+lockfile | integration | integration | ✅ |
| T14 | cli add | integration | integration | ✅ |
| T15 | project release/lockfile | integration | integration | ✅ |
| T16 | docs | none | none | ✅ |
| T17 | docs | none | none | ✅ |

No violations.

---

## Open before Execute

- **AD-NNN Decisions-log entry** should record the MD-21 revision (no
  imperative attribute-schema self-registration; no plugin `extern`) when
  this feature starts — see `context.md` "Supersedes / tensions".
- **Spike T5 first** (the `piperine-plugin-macros` collection mechanism —
  `inventory` vs. generated impl across dlopen) — design §7 flags it as the
  highest-risk new piece.
