# Piperine — Plugin Implementation Plan

> The implementation refinement of the plugin extensibility model. The **normative
> contract** is `docs/spec/part_vi_plugins.md` (Part VI); this document is the
> engineering plan: design decisions with rationale, where each piece lives, the
> logic each piece implements, and the phased delivery with gates. It supersedes
> the earlier draft of this file (which predated Part VI; where the two diverged,
> Part VI won — the deltas are recorded in §2).
>
> **Delivery status (2026-07-10): Phases 0–4 and most of Phase 5 are
> implemented and green.**
> Phase 5: the **process backend** (line-delimited JSON-RPC over stdio;
> `piperine_plugin_wire::serve_stdio` is the whole guest main; the shared
> `WireHosted` adapter means WASM and process guests are indistinguishable
> above the transport; a dead/silent guest is a loud load error — no per-call
> timeout yet, the tier's story is the crash boundary). **Typed P0008**:
> staging carries writer provenance (`StagedInstance.staged_by`), a conflict
> names both plugins and the path. **OSDI extraction (D13)**: `solver/src/
> osdi/` moved to the external `~/Git/piperine-osdi` repo (34 tests green
> there); the solver core dropped the osdi module, build.rs, and libloading.
> Open from Phase 5: `extract`/`.attach`/`.meta` (needs its own design pass —
> selector→staging + overlay attributes), the DeviceProvider netlist seam
> for OSDI's internal nodes, WASM/process scripts (capability imports), and
> artifact distribution (deferred by decision).
>
> Phase 4 (WASM backend): the wire protocol (`piperine_lang::pom::wire` —
> see **D14** below, corrected from an earlier satellite-crate draft) —
> `Design`/`Module`/`Instance`/`Attribute` snapshots in, `Action` patches
> out, packed-i64 returns; `piperine-plugin-wasm` (guest SDK: implement
> `WasmPlugin`, export five thin symbols — no macros); `backend/wasm.rs` in
> the host (wasmtime, fuel cap = manifest `timeout_ms` × 1e6 per guest call,
> wire-ABI version handshake, the shared `WireHosted` adapter presenting a
> guest as an ordinary `Plugin`). Guest patches apply through the same
> staging surface as in-process plugins — same no-netlist-magic validation;
> read-only hooks returning actions fail loud. WASM scripts are a load-time
> error until capability-gated fs imports land; devices stay native/process
> (D9). Gate (`tests/wasm_smoke.rs`): the rc-parasitics guest compiled to
> wasm32-unknown-unknown passes the Phase-3 divider gate unmodified, guest
> bench tasks dispatch through `pp_task`, and an infinite-loop guest traps
> on the fuel cap. `piperine-lang` (miette + fancy included) compiles clean
> to `wasm32-unknown-unknown` — no feature-gating needed.
>
> Phases 0–2: `crates/piperine-plugin` (manifest/P0xxx errors/Registrar/host/
> TOFU/native dlopen backend), the fixture plugin (analog resistor + digital
> inverter through the native ABI — lives as `examples/fixture_plugin.rs`),
> the `DeviceProvider` seam + `@device` branch in `CircuitCompiler`,
> `SchemaShape` + seeded elaboration, and the bench/CLI wiring.
> Phase 3: lifecycle hooks (`after_parse`/`after_elaborate`/
> `transform_design`/`before_lower`/`after_solve`; `after_lower` stays off
> per D12) with view snapshots; staging mutation (`OverrideMap` gains
> idempotent `add_instance`/`add_connection`, applied by
> `with_overrides_applied` with the no-netlist-magic check); plugin bench
> tasks (allowlist gate consults `ElabContext.bench_tasks`, `SimHost::syscall`
> falls through to the host); scripts (`ScriptHandler` + capability-gated
> `HostCtx::fs_read/fs_write` under manifest globs + the CLI external-
> subcommand catch-all + `piperine plugin list`). The bench seam is
> `piperine_bench::plugins::BenchPlugins` — same inversion pattern as D4,
> implemented by `PluginHost`.
> Gates: `piperine-plugin/tests/{manifest,trust,e2e,native_smoke,phase3,
> process_smoke,wasm_smoke}.rs` (30 tests) + `piperine-plugin-wasm`.
> Implementation deltas vs. this plan: the SDK crate depends on
> `piperine-codegen` and `piperine-bench` (one public spec type + the bench
> seam impl; the D4 direction constraints — codegen/bench never depend on
> the plugin crate... bench *defines* the seam trait it consumes — hold);
> the host itself registers the builtin `@device`/`@port` schemas (they
> belong to the plugin *system*). Staging conflicts are now **typed**
> (§Phase-5 P0008 above), not the loud-string placeholder this line used to
> describe.
> `piperine-spice` carries the first real plugin face (`plugin/` +
> `piperine-plugin.toml`), including the registered-but-fail-loud
> `piperine spice` transcriber script.
> Next: Phase 5 (process backend, extract/.attach/.meta, OSDI extraction);
> artifact distribution (prebuilt binaries from git releases) is a
> deliberate open question — the host never builds plugin sources.
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
   bench results     ⑩ plugin BenchTasks ($extract, …) via SimHost::syscall
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
  (`solver/src/core/device.rs`, `solver/src/digital/interface.rs`) — Piperine's
  own mixed-signal ABI, never an external model ABI (D13).
- The POM is the only reflection surface (D8, D14). Native/in-process plugins reflect
  over the real `Design`; only WASM/process guests see the same `Design` serialized
  (`piperine_lang::pom::wire`, owned by the POM crate — not a second model).
  Nothing from `codegen/src/lower/` crosses the plugin boundary except behind the
  elevated `after_lower` capability.
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
`SimHost` owns the `BenchTaskRegistry` (`piperine-bench/src/tasks.rs:275`) and the
registry gains `with_plugins(&PluginHost)`, which wraps each plugin
`bench_task` in an adapter implementing `BenchTask`. The elaboration-time gate in
`piperine-lang` accepts an optional `extra: &dyn Fn(&str) -> bool` (threaded from
the host) so an unknown `$name` is still a *loud* elaboration error when no
plugin provides it. Rationale: fail-loud is preserved, the static builtin list
stays the fast path, and `piperine-lang` never learns about plugins.

**D7 — Native backend first, WASM second, process last.** Native is ~200 lines
of dlopen plumbing (`libloading::Library::new` + a single C entry symbol; the
in-core OSDI loader at `solver/src/osdi/loader.rs:35` is the *mechanical*
precedent for the loading step only — see D13; `solver/build.rs` already exports
host symbols to loaded libraries). WASM needs serialized POM views + wasmtime
plumbing; process needs a JSON-RPC loop. Security posture is unchanged from
Part VI (WASM is the *default backend* for plugin authors; native demands TOFU +
pinned rev + hash) — this decision is only about implementation order, because
native exercises the entire contract with the least new machinery, and every
contract test written for it re-runs against the other backends later.

**D8 — Native/in-process hooks receive the real POM; only out-of-host tiers
receive a serialized snapshot.** *(Revised 2026-07-10 — see D14; the original
version of this decision proposed a `DesignView` snapshot for every backend,
including native, and was rejected during implementation review as an
unnecessary parallel model.)* A native or in-process plugin's read-only hooks
(`after_parse`, `after_elaborate`, `before_lower`, `after_solve`) take
`&Design` — the real POM, the one reflection surface (SPEC Part IV). Only
WASM and process guests, which cannot hold a pointer into host memory,
receive the serialized `Design` itself. `transform_design` always goes through
`DesignStaging`, on every backend — that handle wraps `&Design` directly for
native plugins and is the thing a WASM/process guest's patch gets replayed
into.

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

**D13 — The ABI is Piperine's own; OSDI is a consumer, not the model.** The
plugin device contract is exactly the native `AnalogDevice` / `DigitalDevice`
trait pair — designed for mixed-signal simulation and Piperine semantics
(two-phase digital evaluation with NBA ordering, limited-Newton analog loading,
`EventSink` as the scheduler boundary, `accept_timestep` for cross-domain
coupling). It does **not** track OSDI or any external model ABI; OSDI's only
role in this plan is as the mechanical precedent for dlopen loading (D7).
Consequence: the in-core OSDI support (`solver/src/osdi/` — loader, descriptor
walk, `OsdiDevice`) is slated to move *out* of `piperine-solver` into an
`osdi-compat` plugin whose `DeviceFactory` wraps compiled OSDI v0.4 models
behind the native traits. The solver core then drops the `osdi` module and its
`libloading` dependency; Verilog-A models keep working through the plugin.
Scheduled as a Phase 5 deliverable — it is also the best possible validation of
the device ABI (if OSDI fits behind `DeviceFactory`, any vendor model does).
**Status: done** (2026-07-10) — see `~/Git/piperine-osdi`.

**D14 — The wire protocol lives *inside* `piperine-lang::pom::wire`, never
in a satellite crate.** *(Correction, 2026-07-10.)* The first Phase-4 pass
created a standalone `piperine-plugin-wire`/`piperine-pom-wire` crate to hold
the WASM/process serialization shapes — reasoned as "a leaf crate guest SDKs
can compile to `wasm32-unknown-unknown` without dragging in the parser." That
reasoning missed the actual complaint: **the POM is the one reflection
contract (SPEC Part IV); a second crate defining `Design`/`Module`/
`Instance`/`Attribute` types is a second structural model**, exactly the
thing Part IV §7 says never to build ("every host rebuilds the same typed
objects from this one ABI" — there is one ABI, not two). Splitting the crate
didn't change what it *was*, just where it lived.

The fix (completed 2026-07-11): there are **no wire model types at all**.
The real POM types carry serde derives — `Design`, `Module`, `Instance`,
`Attribute`, `Port`, `Param`, `NetRef`, and `Value` serialize as
themselves; runtime fields (spans, compiled ASTs, `behaviors`,
`Value::Closure`/`Object`) are `#[serde(skip)]`, and skipped runtime
handles fail loud if serialized (`serde` "rc" feature for the `Rc`
interiors). `crates/piperine-lang/src/pom/wire.rs` keeps only *protocol*:
`Registration`/`Hook*`/`Action`/`Task*`/`WirePlugin`/`serve_stdio`/RPC
framing/WASM glue, all carrying the real `Design`/`Value`. There is no
`wire_snapshot()`, no `to_wire`/`from_wire` — the host `clone()`s the
`Design` into the hook envelope and serde does the rest. A guest
deserializes the same type with the same accessors an in-process plugin
reflects over. Round-trip pinned by `piperine-lang/tests/pom_serde.rs`. The leaf-crate
concern turned out to be moot: `piperine-lang` (miette + "fancy" included)
compiles cleanly to `wasm32-unknown-unknown` as-is — verified by building
`piperine-plugin-wasm`'s example guest for that target. No feature-gating
needed.

Consequence for in-process plugins (D8): since the wire module is just
POM-adjacent code in `piperine-lang`, there was no longer any reason for
*native* plugins to go through a serialized view at all — they get `&Design`
directly. The only remaining serialization boundary is the one that's
structurally unavoidable: a WASM sandbox or a child process cannot hold a
pointer into host memory, so *those* tiers, and only those, receive the
`Design` serialized (as itself). `piperine-plugin-wasm` (the guest SDK) now depends on
`piperine-lang` directly and re-exports `pom::wire`; `piperine-plugin`
(the host) does the same. Zero satellite crates for this concern.

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
│   ├── error.rs          # PluginError, P0xxx codes (Part VI §12)
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
    /// Mirrors `piperine_bench::tasks::BenchTask::run`, but host-context-aware
    /// (capability-gated I/O) and without depending on piperine-bench.
    fn run(&self, args: Vec<Value>, cx: &mut HostCtx) -> Result<Value, String>;
}

pub trait ScriptHandler: Send + Sync {
    fn invoke(&self, args: ScriptArgs, cx: &mut HostCtx) -> PluginResult<std::process::ExitCode>;
}
```

`PluginError` carries the P0001–P0009 + P0099 catalog exactly as Part VI §12,
`thiserror` + `miette::Diagnostic` with `code(P000N)` like the rest of the
workspace.

> **As implemented (2026-07-10), corrected per D14:** there is no `view/`
> directory and no `DesignView`/`SourceView`/`LoweredView`/`CircuitView`
> types. `after_parse` takes `&str` (the raw source); `after_elaborate` and
> `before_lower` take `&piperine_lang::pom::Design` directly (native/
> in-process) — WASM/process guests get the same information via
> the serde-serialized `Design` inside the shared `WireHosted` adapter, invisible
> to plugin authors. `transform_design` takes `&DesignStaging` (an immutable
> reference wrapping `&Design`, not `&mut`, since staging itself is the
> mutation channel). `before_solve`/`after_lower`/`CircuitView`/`LoweredView`
> were never built — `after_lower` stays off per D12, `before_solve` wasn't
> needed by any Phase 0–5 consumer. `ScriptHandler::invoke` takes
> `&[String]` and returns `Result<i32, String>`, not `ScriptArgs`/`ExitCode`.
> The crate tree above is the *design* sketch; see `src/` for the real
> layout (`view.rs` holds only `DesignStaging` + `SolveResultView` now —
> singular file, no submodule, because there's no snapshot type left to
> shard out).

### 3.2 `PluginHost` — the one orchestrator

```rust
pub struct PluginHost {
    plugins: Vec<LoadedPlugin>,        // sorted by name (Part VI §8.1 ordering)
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
| `src/tasks.rs:275` | `BenchTaskRegistry::with_plugins(self, host: &Rc<PluginHost>) -> Self` — wraps each plugin task in an adapter `struct PluginTask(Rc<PluginHost>, &'static str)` implementing `BenchTask` (drops the `SimSession` arg; plugin tasks see the design through `HostCtx`, not the session). |
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

`backend/native.rs` (libloading + the `piperine_plugin_entry` C symbol — dlopen
mechanics only, D13), sha256 hashing, TOFU prompt + `--trust`/`--no-trust`,
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
validation; hook firing points in CLI + `SimSession`; `BenchTaskRegistry::
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
`add_connection`, `.meta` = staged overlay attributes per Part I §8).
**OSDI extraction (D13):** move `solver/src/osdi/` into an `osdi-compat`
plugin — its `DeviceFactory` loads a compiled OSDI `.so` and adapts it behind
`AnalogDevice`; the solver core drops the `osdi` module and the `libloading`
dependency; the existing OSDI test corpus becomes the plugin's test suite.
**Gate:** every current OSDI solver test passes through the plugin path with
the in-core module deleted. LSP surface (plugins contributing diagnostics) —
design note only until requested.

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
| Wire snapshot drifts from POM shapes | Closed structurally, not by test discipline (D14): the serialized form *is* the POM (serde on the real types) — there is no second definition anywhere that could drift. |
| wasmtime / WIT churn | The wire shapes are plain serde structs over JSON (`piperine_lang::pom::wire`) — no WIT dependency in the contract; WIT can be layered later without changing the patch language. |
| Allowlist bypass via plugin task names shadowing builtins | `Registrar::bench_task` rejects names already in the static list (P0003-class error). |

---

## 8. What Part VI changed vs. the earlier draft (audit trail)

- Manifest slimmed to identity + permissions (D1); `[attributes.*]`,
  `[devices.*]`, `[scripts.*]` TOML tables deleted — contributions in code.
- The **no-netlist-magic** principle added (Part VI §2) → the staging validation
  in D5 is now normative, not defensive.
- Error catalog frozen as P0001–P0009 + P0099 (Part VI §12); the draft's
  transparent `From<libloading::Error>`-style variants fold into P0099 `Other`
  with source attached.
- `@device` example corrected to real PHDL port syntax (Part VI §7.2).
- `.ppr` serializer prerequisite dropped (D11).
- POM project model — the draft's open prerequisite — landed (`Design::project()`,
  `Project::origins`); discovery/TOFU/no-netlist-magic all anchor on it.
