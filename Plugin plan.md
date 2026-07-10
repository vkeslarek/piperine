# Piperine — Plugin Implementation Plan

> The implementation refinement of the plugin extensibility model. The **normative
> contract** is `docs/spec/part_vi_plugins.md` (Part VI); this document is the
> engineering plan: design decisions with rationale, where each piece lives, the
> logic each piece implements, and the phased delivery with gates. It supersedes
> the earlier draft of this file (which predated Part VI; where the two diverged,
> Part VI won — the deltas are recorded in §2).
>
> Prerequisite status: the **POM project model landed 2026-07-09** —
> `Design::project()` carries name/version/dependencies and per-item provenance
> (`Project::origins`, stamped by the `use` resolver). Attribute schemas are
> validated and populated into the POM (`ElabContext.schemas`, `Attribute { schema,
> data }` on nodes). Those were the two hard prerequisites; nothing else blocks
> Tier 0.

Pipeline today, with the plugin touch points marked:

```
Piperine.toml ──[piperine-project]──► SourceMap ─┐
                                                 │  ① PluginHost::load  (before anything)
.phdl ──parse──► SourceFile                      │  ② register(): schemas → ElabContext,
        │                                        │     factories/tasks/scripts → host tables
        ▼            ③ after_parse               │
   elaborate ──► Design (POM) ── ④ after_elaborate / transform_design (staging)
        │
        ▼            ⑤ before_lower
   lower_bodies ──► LoweredBody map ── ⑥ after_lower (elevated capability)
        │
        ▼
   CircuitCompiler ── ⑦ @device instances → DeviceProvider::build → Box<dyn Device>
        │
        ▼            ⑧ before_solve
      solver
        │
        ▼            ⑨ after_solve
   bench results     ⑩ plugin SimTasks ($extract, …) via SimHost::syscall
                     ⑪ plugin scripts via the CLI catch-all
```

---

## 1. Goals, non-goals, ground rules

**Goals.** Devices, hooks, bench tasks, attribute schemas, and CLI scripts as
first-class extension points reachable from `Piperine.toml` (git or path), with one
SDK crate, one contract, three backends (native / WASM / process). "Install a
plugin from a git URL" must be safe by default.

**Non-goals.** A public registry (later; layers on the same manifest). Extending
the parser grammar, the codegen-private resolved form (`piperine-codegen/src/lower/`),
or the solver's math core — those stay closed. Reinventing resolution —
`piperine-project`'s `Resolver` is reused.

**Ground rules preserved** (checked at review of every phase):

- `piperine-solver` never depends on `piperine-codegen`. Plugins reach the solver
  only through `Device` / `AnalogDevice` / `DigitalDevice`
  (`solver/src/core/device.rs`, `solver/src/digital/interface.rs`) — the same
  boundary OSDI models use.
- The POM is the only reflection surface. `DesignView`/`DesignPatch` serialize POM
  shapes; nothing from `codegen/src/lower/` crosses the plugin boundary except
  behind the elevated `after_lower` capability.
- Fail loud: every unimplemented or denied path is a typed `PluginError` (P0xxx),
  never a silent no-op.
- No netlist magic: everything a plugin injects must reference a type declared in
  PHDL source (or `extern`) — validated against `Design.modules` before applying.
- No macro magic: the SDK's primary API is `Plugin::register(&mut Registrar)`;
  proc-macros live in an optional separate crate, never required.

---

## 2. Design decisions (resolved, with rationale)

Numbered so code comments and reviews can cite them (`PLUGIN-D3`).

**D1 — Manifest is minimal; contributions are declared in code.**
Part VI §4 simplified the manifest to identity + `abi` + `entry` + permissions.
The earlier draft duplicated device/schema/script declarations into TOML; that
duplication is gone. Rationale: one source of truth (`register()`), no drift
between manifest and code, and the TOFU prompt derives what it shows from the
*collected* contributions after a dry registration, not from self-reported TOML.

**D2 — Registration happens before elaboration; contributions are a passive
snapshot.** The elaborator must know plugin attribute schemas *before* it
validates `@device(...)` in source, but device factories are needed only at
circuit build. So `PluginHost::load_for_project()` runs first and `register()`
fills a `Contributions` struct (schema shapes, device type-ids → factories,
bench tasks, scripts). Schemas are handed to `ElabContext.schemas`; the rest
stays in the host and is consulted later. No plugin code runs *during*
elaboration — hooks fire at pipeline boundaries only. This keeps elaboration
pure and total (Part II §1).

**D3 — Plugin schemas extend the existing `SchemaRegistry`, not a parallel one.**
`crates/piperine-lang/src/elab/registry/schemas.rs` today maps schema name →
backing *bundle* name. Plugin schemas have no PHDL bundle, so the registry value
becomes an enum:

```rust
pub enum SchemaShape {
    /// `@attribute(schema = "x")` on a PHDL bundle — fields come from the bundle.
    Bundle(String),
    /// Plugin-registered — fields carried directly.
    Declared(AttrShape),   // AttrShape = Vec<AttrField { name, ty: ValueType, required }>
}
```

The validation pass (the one that already produces E2022/E2023) matches on the
shape and reuses one code path for "field exists / type matches / required
present". Rationale: Part I §8 promises *one* metadata mechanism; two registries
would fork the diagnostics. Collision between a plugin schema and a bundle
schema (or two plugins) is P0003 `SchemaConflict` at load time.

**D4 — Dependency inversion at the codegen boundary.** `piperine-codegen` does
NOT depend on `piperine-plugin`. Instead codegen defines the minimal seam it
needs:

```rust
// crates/piperine-codegen/src/device/provider.rs (new)
pub trait DeviceProvider {
    /// Build the device for a `@device`-annotated instance, or error P0004.
    fn build(&self, spec: &PluginDeviceSpec) -> Result<Box<dyn piperine_solver::core::device::Device>, CodegenError>;
}
pub struct PluginDeviceSpec<'a> {
    pub type_id: &'a str,
    pub plugin: &'a str,
    pub attributes: &'a [piperine_lang::pom::Attribute],   // @device + @port data
    pub port_bindings: Vec<(String, piperine_solver::analog::NodeIdentifier)>, // logical name → net
    pub digital_ports: Vec<(String, piperine_solver::digital::DigitalNet)>,
    pub params: Vec<(String, piperine_lang::Value)>,
}
```

`piperine-plugin::PluginHost` implements `DeviceProvider`. The wiring happens in
the crates that already depend on both (bench, CLI). Rationale: keeps the crate
DAG flat (`plugin → {lang, solver, project}`; `codegen → lang`; `bench/cli →
everything`), and mirrors how `CircuitCompiler` already avoids knowing about the
bench.

**D5 — Mutation only through staging; staging gains two verbs.** `Design` is
immutable after elaboration; the only mutation surface is `OverrideMap`
(`pom/staging.rs` — today only `set(path, param, value)`). It gains
`add_instance(parent, InstanceSpec)` and `add_connection(parent, ConnectionSpec)`
consumed by `with_overrides_applied` (design.rs:296) during the pure
re-elaboration. The staging layer validates each injected instance's type name
against `Design.modules` (falling back to `Project::origin_of` for imported
names) — the **no-netlist-magic** check, P0005 with "type not declared".
Conflicts (two writers on one path) are P0008 `StagingConflict`, detected because
`OverrideMap` is keyed by path. Rationale: bench forks already prove the
staging → fork → applied model; plugins ride the same rails, and the POM never
hands out `&mut Design`.

**D6 — Bench-task gating moves to the host answer, not a bigger static list.**
`bench_task_implemented` (`piperine-lang/src/eval/tasks.rs:32`) is a static
allowlist consulted at bench validation. Plugin tasks can't be listed statically,
so the gate becomes two-stage: static list first, then a host callback —
`SimHost` owns the `SimTaskRegistry` (`piperine-bench/src/tasks.rs:275`) and the
registry gains `with_plugins(&PluginHost)`, which wraps each plugin
`bench_task` in an adapter implementing `SimTask`. The elaboration-time gate in
`piperine-lang` accepts an optional `extra: &dyn Fn(&str) -> bool` (threaded from
the host) so an unknown `$name` is still a *loud* elaboration error when no
plugin provides it. Rationale: fail-loud is preserved, the static builtin list
stays the fast path, and `piperine-lang` never learns about plugins.

**D7 — Native backend first, WASM second, process last.** Native is ~200 lines
on top of the exact `OsdiLib` precedent
(`solver/src/osdi/loader.rs:35` — `libloading::Library::new` + a single C entry
symbol; `solver/build.rs` already exports host symbols to loaded libraries).
WASM needs serialized POM views + wasmtime plumbing; process needs a JSON-RPC
loop. Security posture is unchanged from Part VI (WASM is the *default ABI* for
plugin authors; native demands TOFU + pinned rev + hash) — this decision is only
about implementation order, because native exercises the entire contract with
the least new machinery, and every contract test written for it re-runs against
the other backends later.

**D8 — Hook inputs are views, not references, on every backend.** Even the
native backend receives `DesignView` (a serializable snapshot) rather than
`&Design`, except for `transform_design` which receives the `DesignStaging`
handle. Rationale: one contract across backends (a native plugin recompiled as
WASM must not change semantics), and it prevents native plugins from growing
accidental dependencies on POM internals. Cost is a copy per hook — hooks run a
handful of times per run, never in the Newton loop.

**D9 — Devices are native/process-only in the first delivery.** A device sits in
the Newton inner loop or the delta-cycle loop; snapshot-per-call WASM is
unusable there. The `AnalogFn` ABI is already `unsafe extern "C" fn(...)`
(`codegen/src/jit/analog.rs:46`), so a future "WASM fast path" (plugin exports
C-ABI functions over shared linear memory via `wasmtime::Func`) is plausible —
recorded as research, not scheduled.

**D10 — TOFU state lives in `Piperine.lock`; hashing is sha256 of the artifact
bytes.** `LockEntry` (`piperine-project/src/lockfile.rs:12`) gains
`kind: EntryKind { Dependency, Plugin }` (serde-default `Dependency` so existing
lockfiles parse unchanged) plus `manifest_hash`, `content_hash`, `abi`,
`trusted_at`. Any hash change → re-prompt. `--trust <file>` / `--no-trust` for
CI per Part VI §3.2. New dep `sha2` in `piperine-project`.

**D11 — Scripts do not require a `.ppr` serializer.** The earlier draft made a
public binary `.ppr` writer a prerequisite. Dropped: scripts read/write text
`.phdl` through `HostCtx::fs()` and get the elaborated `Design` through
`project()`. A binary format, if it ever exists, is orthogonal.

**D12 — `after_lower` ships disabled.** The hook exists in the trait (so the
contract is complete) but the host rejects manifests requesting the `lowered`
capability with `PluginError::Other("after_lower not yet enabled")` until a
real consumer exists. Rationale: it is the only surface that exposes
codegen-private shapes; do not open it speculatively.

---

## 3. The SDK crate: `crates/piperine-plugin`

New workspace member. Depends on `piperine-lang` (POM, `Value`),
`piperine-solver` (device traits), `piperine-project` (resolver, lockfile).
Never on `piperine-codegen`.

```
crates/piperine-plugin/
├── Cargo.toml            # deps: piperine-lang, piperine-solver, piperine-project,
│                         #       thiserror, miette, serde, toml, sha2
│                         # feature "wasm" → wasmtime; feature "process" → serde_json
├── src/
│   ├── lib.rs            # re-exports; the Plugin trait
│   ├── manifest.rs       # Manifest + Permissions (D1, Part VI §4)
│   ├── error.rs          # PluginError, P0xxx codes (Part VI §11)
│   ├── contributions.rs  # Registrar + Contributions snapshot (D2)
│   ├── host.rs           # PluginHost: discover → verify → load → register → dispatch
│   ├── trust.rs          # TOFU prompt, lockfile round-trip (D10)
│   ├── capability.rs     # HostCtx: fs()/spawn()/log()/project(), glob checks
│   ├── view/
│   │   ├── design.rs     # DesignView, ModuleView, InstanceView, AttributeView (serde)
│   │   ├── staging.rs    # DesignStaging (wraps Design + OverrideMap; D5)
│   │   └── result.rs     # SolveResultView (op/tran/ac summaries)
│   ├── backend/
│   │   ├── mod.rs        # trait Backend { fn load(&Manifest, &Path) -> Box<dyn Plugin> }
│   │   ├── native.rs     # libloading, entry symbol `piperine_plugin_entry`
│   │   ├── wasm.rs       # wasmtime (feature-gated; Tier 3)
│   │   └── process.rs    # JSON-RPC over stdio (feature-gated; Tier 4)
│   └── device.rs         # DeviceFactory, DeviceKind, the DeviceProvider impl (D4)
└── tests/
    ├── manifest.rs       # parse/validate/permission round-trips
    ├── trust.rs          # TOFU state machine against a temp lockfile
    └── native_smoke.rs   # builds tests/fixtures/hello-plugin as cdylib, loads, fires hooks
```

### 3.1 Core types (the contract)

```rust
pub trait Plugin: Send + Sync {
    fn manifest(&self) -> &Manifest;
    fn register(&self, r: &mut Registrar) { let _ = r; }

    // Seven hooks (Part VI §8), all default no-op:
    fn after_parse(&self, _cx: &mut HostCtx, _src: &SourceView) -> PluginResult<()> { Ok(()) }
    fn after_elaborate(&self, _cx: &mut HostCtx, _d: &DesignView) -> PluginResult<()> { Ok(()) }
    fn transform_design(&self, _cx: &mut HostCtx, _s: &mut DesignStaging) -> PluginResult<()> { Ok(()) }
    fn before_lower(&self, _cx: &mut HostCtx, _d: &DesignView) -> PluginResult<()> { Ok(()) }
    fn after_lower(&self, _cx: &mut HostCtx, _b: &mut LoweredView) -> PluginResult<()> { Ok(()) }  // D12: gated off
    fn before_solve(&self, _cx: &mut HostCtx, _c: &CircuitView) -> PluginResult<()> { Ok(()) }
    fn after_solve(&self, _cx: &mut HostCtx, _r: &SolveResultView) -> PluginResult<()> { Ok(()) }
}

pub struct Registrar<'a> { contributions: &'a mut Contributions }
impl Registrar<'_> {
    pub fn device(&mut self, type_id: &str, f: Box<dyn DeviceFactory>);   // P0003-style dup check
    pub fn attr_schema(&mut self, name: &str, shape: AttrShape);          // P0003 on collision
    pub fn bench_task(&mut self, name: &'static str, t: Box<dyn PluginBenchTask>);
    pub fn script(&mut self, name: &str, h: Box<dyn ScriptHandler>);
}

pub trait DeviceFactory: Send + Sync {
    fn kind(&self) -> DeviceKind;   // Analog | Digital | Mixed
    fn instantiate(&self, spec: &DeviceSpec) -> PluginResult<Box<dyn piperine_solver::core::device::Device>>;
}

pub trait PluginBenchTask: Send + Sync {
    /// Mirrors `piperine_bench::tasks::SimTask::run`, but host-context-aware
    /// (capability-gated I/O) and without depending on piperine-bench.
    fn run(&self, args: Vec<Value>, cx: &mut HostCtx) -> Result<Value, String>;
}

pub trait ScriptHandler: Send + Sync {
    fn invoke(&self, args: ScriptArgs, cx: &mut HostCtx) -> PluginResult<std::process::ExitCode>;
}
```

`PluginError` carries the P0001–P0009 + P0099 catalog exactly as Part VI §11,
`thiserror` + `miette::Diagnostic` with `code(P000N)` like the rest of the
workspace.

### 3.2 `PluginHost` — the one orchestrator

```rust
pub struct PluginHost {
    plugins: Vec<LoadedPlugin>,        // sorted by name (Part VI §8.4 ordering)
    contributions: Contributions,      // merged, collision-checked
    permissions: HashMap<String, Permissions>,
}
```

`PluginHost::load_for_project(root: &Path, trust: TrustMode)` logic, in order:

1. Parse `Piperine.toml` `[plugins]` (via `piperine-project`); empty → return an
   inert host (every downstream call is a cheap no-op — the zero-plugin path
   costs one `Vec::is_empty`).
2. Resolve each source into `target/plugins/<name>/` (reusing `Resolver`'s git
   walker; `rev` mandatory when the manifest later says `abi = "native"`).
3. Parse `piperine-plugin.toml` → `Manifest` (P0006 on malformed).
4. sha256 the entry artifact; compare with the lockfile (P0007 on mismatch);
   run TOFU if unknown hash (P0001 on rejection; `--no-trust` auto-rejects
   native).
5. Load through the backend selected by `abi` (D7: native first).
6. Call `register()` per plugin, alphabetical; merge into `contributions`
   (P0003 on schema/device/script/task collisions).

Dispatch API consumed by the pipeline:

```rust
impl PluginHost {
    pub fn schemas(&self) -> impl Iterator<Item = (&str, &AttrShape)>;    // → ElabContext
    pub fn has_bench_task(&self, name: &str) -> bool;                     // → allowlist gate (D6)
    pub fn run_bench_task(&self, name: &str, args: Vec<Value>) -> ...;
    pub fn script(&self, name: &str) -> Option<&dyn ScriptHandler>;
    pub fn fire<H: HookSelector>(&self, input: H::Input<'_>) -> PluginResult<()>;  // alphabetical, P0005 wraps errors
}
impl piperine_codegen_seam::DeviceProvider for PluginHost { ... }  // via the bench/CLI wiring, D4
```

Hook dispatch wraps every plugin error as `P0005 HookFailed { hook, plugin, msg }`
and **aborts the run** (fail loud — a failed hook is not skipped).

### 3.3 `HostCtx` (capability facade)

Holds the plugin's `Permissions` + project root. Every side-effecting call
checks first, `P0002 UndeclaredCapability` on violation:

- `fs()` — path canonicalized, must stay under project root, must match one
  of the manifest globs (`read`/`write` verbs separate).
- `spawn(exe, args)` — `exe` must be in the `process_spawn` whitelist; child
  stdout/stderr captured and routed to the logger.
- `project()` — always available, read-only: manifest values, target dir,
  the current `DesignView`.
- `log(level, msg)` — always available.

There is no `network()` in the API at all until a real consumer appears — the
manifest field exists (Part VI §4) and is surfaced in the TOFU prompt, but the
facade offers nothing to call, which is the strongest possible enforcement.

---

## 4. Integration surface (per crate, with today's anchors)

### `piperine-project`

| Where | Logic |
|---|---|
| `src/lib.rs` (`PiperineToml`) | `pub plugins: HashMap<String, DependencySource>` (`#[serde(default)]`) — reuses the existing `Git \| Path` source enum; plugins are *not* merged into `dependencies`. |
| `src/resolver.rs` | `Resolver::resolve_plugins(&toml) -> ResolvedMap` — same walker, different target subdir (`target/plugins/`). |
| `src/lockfile.rs:12` | `LockEntry` gains `#[serde(default)] kind: EntryKind`, `manifest_hash`, `content_hash`, `abi`, `trusted_at` (all `Option`/defaulted → old lockfiles parse). Accessors `plugin_entry(name)` / `record_trust(...)`. |

### `piperine-lang`

| Where | Logic |
|---|---|
| `src/elab/registry/schemas.rs` | `SchemaRegistry` value type becomes `SchemaShape` (D3). `register_declared(name, AttrShape)` for plugins; existing `register(name, bundle)` wraps `SchemaShape::Bundle`. The attribute-validation pass matches on the shape; error paths stay E2022/E2023. |
| `src/pom/staging.rs` | `OverrideMap` gains `add_instance(parent, InstanceSpec)` / `add_connection(parent, ConnectionSpec)` + iteration for the apply step. `InstanceSpec { label, module, params, port_map }`, `ConnectionSpec { lhs, rhs }` — plain data, serde-able (the WASM patch reuses them). |
| `src/pom/design.rs:296` (`with_overrides_applied`) | After param overrides, apply staged instances/connections: validate `spec.module` against `self.modules` (**no-netlist-magic**, D5), synthesize the `Instance`/`Connection` POM nodes on the parent module, then re-run the module validation checks so E2013/E2014/E2020 fire on bad injections. |
| `src/eval/tasks.rs:32` | `bench_task_implemented(name)` keeps the static list; the bench-gate call site accepts an optional host predicate (D6) so `SimHost` can vouch for plugin tasks. |

### `piperine-codegen`

| Where | Logic |
|---|---|
| `src/device/provider.rs` (new) | `trait DeviceProvider` + `PluginDeviceSpec` (D4). Owned here so codegen needs no new dependency. |
| `src/device/circuit.rs:77` | `CircuitCompiler::new(...)` unchanged; new `with_device_provider(self, p: &dyn DeviceProvider) -> Self`. |
| `src/device/circuit.rs:288` (`add_instance`) | First check: does the instance's module (or the instance itself) carry a `@device` attribute (`Module::attributes()` / `Instance::attributes()`)? If yes and a provider is present → build `PluginDeviceSpec` (resolve each `@port(name=…)`-annotated port to the already-computed net mapping — analog terminals as `NodeIdentifier`, digital ones as `DigitalNet`), call `provider.build(spec)`, push the returned `Box<dyn Device>` and **skip** `CompiledModule::compile` for that instance. If `@device` is present and no provider → fail loud (`CodegenError::Unsupported("@device without a plugin host")`); unregistered type inside the provider → P0004. |

### `piperine-bench`

| Where | Logic |
|---|---|
| `src/tasks.rs:275` | `SimTaskRegistry::with_plugins(self, host: &Rc<PluginHost>) -> Self` — wraps each plugin task in an adapter `struct PluginTask(Rc<PluginHost>, &'static str)` implementing `SimTask` (drops the `SimSession` arg; plugin tasks see the design through `HostCtx`, not the session). |
| `src/host.rs:206` (`SimHost::syscall`) | Registry lookup already dispatches by name; nothing changes here beyond the registry containing plugin entries. The allowlist predicate passed to bench validation becomes `|n| bench_task_implemented(n) || host.has_bench_task(n)` (D6). |
| `src/session.rs` / `src/runner.rs` | `BenchRunner`/`SimSession` gain an optional `Rc<PluginHost>`. `SimSession::build_circuit` (session.rs:88) chains `.with_device_provider(host)` and fires `before_lower` / `after_lower`(gated) / `before_solve`; each `run_*` fires `after_solve` with a result view. `transform_design` fires once per analysis, right before `with_overrides_applied` — its staged mutations ride the same apply. |

### `piperine-cli`

| Where | Logic |
|---|---|
| `src/lib.rs:15` (`Commands`) | Clap `allow_external_subcommands = true`; unknown subcommand + trailing args → `commands::plugin_script::execute(name, rest)`. |
| `src/commands/plugin_script.rs` (new) | `PluginHost::load_for_project` → `host.script(&name)` → `invoke(args, ctx)`; unknown → P0009. |
| `src/commands/plugin.rs` (new) | `piperine plugin list` (name, abi, source, trust state, contributions), `plugin trust <name>` (re-run TOFU), `plugin update <name>` (accept new hash — the *only* path that does). |
| `src/commands/{check,run,test}.rs` | Load the host once next to `build_source_map()`; feed schemas into elaboration (an `ElabContext` seeding hook on `parse_and_elaborate` — new optional-arg variant `parse_and_elaborate_with(input, source_map, seed: impl FnOnce(&mut ElabContext))`), fire `after_parse`/`after_elaborate`, and hand the host to `BenchRunner`. |

---

## 5. Delivery phases and gates

Each phase is independently shippable, keeps `cargo test --workspace` green, and
ends with its gate test checked in.

### Phase 0 — Skeleton (no backends)

Crate `piperine-plugin` with `Manifest`/`Permissions` parsing, `PluginError`,
`Contributions`/`Registrar`, `HostCtx` with capability checks, inert
`PluginHost`. `piperine-project` changes ([plugins] section, lockfile fields,
`resolve_plugins`). **Gate:** manifest/permission/lockfile round-trip tests;
a project without `[plugins]` behaves byte-identically (inert-host fast path).

### Phase 1 — Native backend + TOFU

`backend/native.rs` (libloading, `piperine_plugin_entry` C symbol — same shape
as `OsdiLib::load`), sha256 hashing, TOFU prompt + `--trust`/`--no-trust`,
`trust.rs` lockfile round-trip. A `tests/fixtures/hello-plugin` cdylib crate
compiled by the test harness. **Gate:** `native_smoke.rs` — load, register, fire
every hook, deny an undeclared `fs()` call (P0002), reject a tampered artifact
(P0007), reject unapproved (P0001).

### Phase 2 — Attribute schemas + device loading

`SchemaShape` in `piperine-lang` (D3); `parse_and_elaborate_with` seeding;
`DeviceProvider` seam in codegen (D4) + the `add_instance` branch; CLI/bench
wiring. Sample plugin `tests/fixtures/relay-device` providing a trivial
`DigitalDevice` (input follows output with one delta — enough to prove the
scheduler path). **Gate:** a `.phdl` with `@device(plugin=…, type=…)` +
`@port(...)` elaborates, builds, and a bench `$op`/`$tran` observes the plugin
device's behavior; the same source without the plugin loaded fails loud with
P0004/`Unsupported`.

### Phase 3 — Hooks + staging mutation + bench tasks + scripts

`DesignStaging` over the extended `OverrideMap` (D5) with the no-netlist-magic
validation; hook firing points in CLI + `SimSession`; `SimTaskRegistry::
with_plugins` + the two-stage allowlist (D6); CLI script catch-all + `piperine
plugin list`. Sample plugin `rc-parasitics` (native): `@extract_rc` schema,
`after_elaborate` collects, `transform_design` injects `Resistor` instances.
**Gate:** an AC bench differs with/without the plugin by exactly the injected
parasitics; injecting an undeclared type errors P0005; two plugins staging the
same path errors P0008; `piperine <script>` round-trips a file through a sample
importer script under capability enforcement.

### Phase 4 — WASM backend

`backend/wasm.rs` (wasmtime + fuel + timeout), serde views crossing as
postcard/CBOR, `DesignPatch` = the same `InstanceSpec`/`ConnectionSpec`/param
triples the staging already speaks (one patch language, D5). Hooks, schemas,
bench tasks, and scripts work under WASM; devices stay native/process (D9).
**Gate:** `rc-parasitics` recompiled to WASM passes the exact Phase-3 gate
unmodified; an infinite-loop hook is killed by the timeout.

### Phase 5 — Maturation

Out-of-process backend (JSON-RPC over stdio; same view/patch language).
`extract` / `.attach` / `.meta` shipped as plugin bench tasks (closes ROADMAP
G13; `.attach` = a bench-side wrapper that stages `add_instance` +
`add_connection`, `.meta` = staged overlay attributes per Part I §8). LSP
surface (plugins contributing diagnostics) — design note only until requested.

---

## 6. Reference use cases (validation matrix)

Become integration tests as their phase lands:

1. **`relay-device`** (native, Phase 2) — `@device`/`@port` binding, digital
   scheduler path.
2. **`rc-parasitics`** (native Phase 3, WASM Phase 4) — schema + read hook +
   staged injection; the canonical layer-4 closure-loop citizen.
3. **`spice-import`** (script, Phase 3) — CLI catch-all + capability-gated fs.
4. **proprietary analog device** (native, Phase 2+) — `AnalogDevice` with only
   `load_dc` overridden; proves the default-method surface.
5. **`power-audit`** (WASM, Phase 4) — `after_solve` read-only metrics; the
   minimum-capability path (no fs, no spawn, no mutation).

---

## 7. Risks

| Risk | Mitigation |
|---|---|
| Native plugin crashes the host | Documented full-trust tier; `process` backend is the isolation answer (Phase 5). |
| Staged `add_instance` breaks POM invariants | Single mutation point + re-running module validation inside `with_overrides_applied` (E2013/E2014/E2020 fire on bad injections). |
| Schema collision plugin×bundle | One registry (D3) makes the collision *detectable*; P0003 at load; `@plugin-name:schema` namespacing as escape hatch. |
| View snapshots drift from POM shapes | Views live in one module (`view/`), built from POM accessors only; a golden serialization test pins the wire shape per contract version. |
| wasmtime / WIT churn | Views are plain serde structs over postcard — no WIT dependency in the contract; WIT can be layered later without changing the patch language. |
| Allowlist bypass via plugin task names shadowing builtins | `Registrar::bench_task` rejects names already in the static list (P0003-class error). |

---

## 8. What Part VI changed vs. the earlier draft (audit trail)

- Manifest slimmed to identity + permissions (D1); `[attributes.*]`,
  `[devices.*]`, `[scripts.*]` TOML tables deleted — contributions in code.
- The **no-netlist-magic** principle added (Part VI §2) → the staging validation
  in D5 is now normative, not defensive.
- Error catalog frozen as P0001–P0009 + P0099 (Part VI §11); the draft's
  transparent `From<libloading::Error>`-style variants fold into P0099 `Other`
  with source attached.
- `@device` example corrected to real PHDL port syntax (Part VI §7.2).
- `.ppr` serializer prerequisite dropped (D11).
- POM project model — the draft's open prerequisite — landed (`Design::project()`,
  `Project::origins`); discovery/TOFU/no-netlist-magic all anchor on it.
