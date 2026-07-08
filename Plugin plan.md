# Piperine — Plugin Plan

> A refinement spec for the plugin extensibility model. Resolves the `ROADMAP.md`
> "plugin model" blocker and unblocks `extract` / `.attach` / `.meta` (bench SPEC §11).
> This document is the extensibility contract, not a task list.

Piperine today is a closed pipeline: `.phdl` → POM `Design` → `lower_bodies` →
`CircuitCompiler` → solver. Every stage is internal. This plan opens **four** extension
surfaces without breaking "fail loud", without macro magic, and without coupling the
solver to codegen:

1. **Load devices** through the existing `DigitalDevice` / `AnalogDevice` traits (the
   "OSDI for plugins").
2. **Declarative binding** via `@device(...)` / `@port(...)` attributes in `.ppr`.
3. **Lifecycle hooks** that read/write the POM (e.g. inject parasitics).
4. **Custom scripts**, Cargo-style (`piperine spice foo.cir -o foo.ppr`).

Ground rule: **a user must never be able to pull a malicious git repo and have it
trash their machine.** Everything below derives from that.

---

## 1. Goals & non-goals

**Goals**

- Make devices, hooks, and scripts first-class extension points reachable from project
  imports (git or path), with one SDK and a stable contract.
- Treat the existing POM as the reflection ABI and the existing `Device` traits as the
  solver ABI. The IR (`piperine-codegen/src/lower/`) stays codegen-private.
- Define a threat model and a permission system that makes "install a plugin from a git
  URL" a safe operation by default.

**Non-goals**

- A public plugin registry / package index (out of scope; revisit later).
- Reinventing the existing resolver — `piperine-project` already does git/path dependency
  resolution. Plugins reuse it.
- Letting plugins extend the parser grammar, the IR types, or the solver's math core.
  Those stay closed; plugins reflect through the POM and contribute behavior.

---

## 2. Design principles

| Principle | Concrete meaning |
|---|---|
| **Security-first, capability-based** | A plugin declares permissions in its manifest; the host denies by default. Missing permission = no effect, not a crash. |
| **Fail loud** | Plugin that requests a nonexistent hook, references an unregistered device, or uses an attribute with no schema raises a typed `PluginError` — never `0.0`, never a no-op. Inherits the `AGENTS.md` rule. |
| **Two ABIs, one SDK** | WASM (safe by default, sandboxed) and native `.cdylib` (power, explicit opt-in) share the same `Plugin` trait and the same hooks. |
| **The POM is the reflection contract** | `piperine-lang/src/pom/` is already public and stable; the IR stays closed. Plugins reflect through the POM. |
| **No macro magic (in the SDK too)** | Registration via `Plugin::register(&mut Registrar)` — same "every helper has an owner" discipline as the rest of the codebase. Proc-macros are opt-in sugar, never required. |
| **Do not surprise the solver** | `piperine-solver` still does not depend on `piperine-codegen`. Plugins talk to the solver only through the existing `Device` / `AnalogDevice` / `DigitalDevice` traits. |

---

## 3. Security model

### 3.1 Threats and countermeasures

| Threat | Countermeasure |
|---|---|
| Malicious git repo carrying a payload in `build.rs` or in the `.cdylib` | (a) WASM sandbox by default; (b) `.cdylib` requires `abi = "native"` **+** TOFU approval **+** content hash in the lockfile; (c) pinned rev is mandatory for native (no `branch`/`latest`). |
| Plugin reads `~/.ssh` or writes outside the project | Capability-based FS: plugin may only access paths declared in the manifest, resolved relative to the project root, never absolute paths escaping it. |
| Plugin exfiltrates over the network | `network = false` by default. WASM has no sockets. Native with `network = true` requires interactive approval on every load (first time only) and is logged to `target/plugins/access.log`. |
| Plugin spawns a process (`sh -c …`) | `process_spawn = false` by default. WASM never. Native needs the capability and logs every spawn. |
| Silent binary swap via a git push | `Piperine.lock` stores the `content_hash` of the loaded artifact. A change forces re-approval. `piperine plugin update` is the only path that accepts a new hash. |
| Native plugin calls `exit()` or crashes the host | Loaded in-process (same as OSDI). A crash is treated as a process failure (same model as `OsdiLib`). For strong isolation, an out-of-process runner is offered via stdio JSON-RPC (see §6.4). |
| DoS via an infinite loop in a hook | WASM hooks have a per-invocation timeout (default 5 s, configurable). Native hooks have none — native is full trust. |

### 3.2 Trust on first use (TOFU)

On the first load of a plugin (or whenever the artifact hash changes) the CLI **blocks**
and presents:

```
$ piperine run sim.phdl

  Plugin 'avr-cosim' (native) loaded from:
    https://github.com/acme/piperine-avr @ abc1234
  Requested permissions:
    design        : read-write
    load_device   : Arduino::UnoR3 (digital)
    filesystem    : write *.ppr, read *.cir
    process_spawn : simavr, avr-objcopy
  Artifact hash: sha256:9f3a…b21c

  Trust and save to Piperine.lock? [y/N/details]
```

- `y` → writes `[plugins.avr-cosim] hash = "…"` to `Piperine.lock`. Never asks again while
  the hash is unchanged.
- `N` → aborts with `PluginError::Untrusted`.
- `details` → shows the manifest diff and the files that would be accessed.

CI modes: `--trust <file>` reads decisions from a checked-in trust file;
`--no-trust` silently rejects native plugins (read-only sandbox mode for suspicious CI).

### 3.3 Capability enforcement

The host exposes a **facade** (`HostCtx`) to the plugin. Every side-effecting call (FS,
spawn, network) goes through it and is checked against the manifest capabilities. In WASM
this is natural (host imports); in native, the SDK offers the same entry points and the
publisher is expected to call them rather than `std::fs` directly (documented contract;
best-effort). This is **not** a cryptographic sandbox for native — in-process sandboxing is
impossible. It is **audit + opt-in + reproducibility**. Real isolation comes from the
out-of-process backend (§6.4).

---

## 4. Plugin manifest (`piperine-plugin.toml`)

Lives at the root of the plugin repo. Full example (AVR co-sim):

```toml
[plugin]
name        = "avr-cosim"
version     = "0.1.0"
abi         = "wasm"              # "wasm" | "native" | "process"
entry       = "avr_cosm.wasm"     # .wasm | .cdylib/.dll/.so | runner binary path
description = "Co-simulation bridge for AVR microcontrollers"

[permissions]
design         = "read-write"     # "none" | "read" | "read-write"
lowered        = "read-write"     # IR access (rare; native or flagged WASM)
solve          = "read"           # inspect the assembled circuit post-build
bench_hooks    = true             # register SimTasks
filesystem     = ["read *.cir", "write *.ppr"]
network        = false
process_spawn  = ["simavr", "avr-objcopy"]   # whitelist of executables
timeout_ms     = 5000             # per WASM hook invocation

[attributes."device"]
# registers the @device(...) schema with the elaborator — closes the UnknownAttrSchema gap
fields = { plugin = "String", type = "String", params = "Map<String,Value>" }
applies_to = ["module", "instance"]

[attributes."port"]
fields   = { name = "String", kind = "String" }   # kind: "analog" | "digital" | "inout"
applies_to = ["port"]

[devices."Arduino::UnoR3"]
factory   = "uno_r3"              # symbolic name; mapped by the plugin in register()
kind      = "digital"             # "digital" | "analog" | "mixed"
boundary  = ["A0","A1","A2","A3","D0","D1","D2","D3"]   # logical port names

[scripts.spice]
entry    = "import_spice"
summary  = "Import a SPICE netlist (.cir) into a .ppr file"
args     = [
  { name = "INPUT",   required = true,  kind = "path" },
  { name = "--output", short = "-o", required = true, kind = "path" },
]
```

The manifest is parsed once at load time; capability fields become a `Permissions` struct
the host carries for the plugin's lifetime.

---

## 5. Discovery and resolution

### 5.1 `Piperine.toml`

A new optional section, **separate** from `[dependencies]` (plugins are not PHDL libraries):

```toml
[plugins.avr-cosim]
git = "https://github.com/acme/piperine-avr"
rev = "abc1234"          # mandatory for abi=native; optional but recommended for wasm

[plugins.spice-import]
path = "../piperine-spice"   # local path dep, same semantics as the existing resolver
```

This extends `piperine-project/src/lib.rs::PiperineToml` with
`pub plugins: HashMap<String, PluginSource>`, where `PluginSource` mirrors
`DependencySource` (`Git | Path`). The existing `Resolver`
(`crates/piperine-project/src/resolver.rs`) is reused — only a parallel walker for plugins
is added.

### 5.2 Lockfile

`Piperine.lock` gains:

```toml
[[plugins]]
name          = "avr-cosim"
source        = "git+https://github.com/acme/piperine-avr@abc1234"
manifest_hash = "sha256:…"   # hash of piperine-plugin.toml
content_hash  = "sha256:…"   # hash of the loaded artifact (wasm/cdylib)
abi           = "wasm"
trusted_at    = "2026-07-07T10:11:12Z"
```

Any hash change forces re-approval. `LockEntry`
(`piperine-project/src/lockfile.rs:6`) gains a `kind: EntryKind { Dependency, Plugin }`
discriminator so existing dependency entries are untouched.

### 5.3 Target directory

Plugins resolve into `target/plugins/<name>/` — not `target/deps/`, to keep them
separated from PHDL library deps. The manifest is mirrored to
`target/plugins/<name>/piperine-plugin.toml`.

---

## 6. ABI tiers

### 6.1 The `Plugin` trait (one contract, three backends)

Defined in a new crate `piperine-plugin` (see §7). Single trait, valid for all backends:

```rust
pub trait Plugin: Send + Sync {
    fn manifest(&self) -> &Manifest;

    fn register(&self, r: &mut Registrar) {
        let _ = r; // default empty — every contribution is optional
    }

    // Lifecycle hooks (§9). All default to no-op.
    fn after_parse(&self, _ctx: &mut HostCtx, _src: &SourceFileView) -> Result<()> { Ok(()) }
    fn after_elaborate(&self, _ctx: &mut HostCtx, _design: &DesignView) -> Result<()> { Ok(()) }
    fn transform_design(&self, _ctx: &mut HostCtx, _staging: &mut DesignStaging) -> Result<()> { Ok(()) }
    fn before_lower(&self, _ctx: &mut HostCtx, _design: &DesignView) -> Result<()> { Ok(()) }
    fn after_lower(&self, _ctx: &mut HostCtx, _bodies: &mut LoweredBodiesView) -> Result<()> { Ok(()) }
    fn before_solve(&self, _ctx: &mut HostCtx, _circuit: &CircuitHandle) -> Result<()> { Ok(()) }
    fn after_solve(&self, _ctx: &mut HostCtx, _result: &SolveResult) -> Result<()> { Ok(()) }
}
```

`Registrar` is the builder plugins use to contribute devices, attribute schemas, bench
tasks, and scripts (all optional):

```rust
pub struct Registrar<'a> { /* ... */ }
impl<'a> Registrar<'a> {
    pub fn device(&mut self, type_id: &str, factory: Box<dyn DeviceFactory>);
    pub fn attr_schema(&mut self, schema: &str, shape: AttrShape);
    pub fn bench_task(&mut self, task: Box<dyn SimTaskPlugin>);
    pub fn script(&mut self, name: &str, handler: Box<dyn ScriptHandler>);
}
```

### 6.2 WASM backend (default, sandboxed)

- Runtime: **`wasmtime`** (mature, supports WIT / WASI preview 2, component model).
- Protocol: **WIT (WebAssembly Interface Types)** — versioned declarative interface, no
  hand-rolled serialization heroics.
- Types cross the boundary as **serialized snapshots** (CBOR or postcard) of POM views.
  A plugin never receives a `&Design` pointer; it receives a `DesignView` (a plain
  serializable struct) and returns a `DesignPatch` that the host validates and applies.
- Cost: a copy per hook invocation. Acceptable: hooks run a handful of times per
  simulation, not in the Newton inner loop. Device loading under WASM is experimental
  (see §8.4).
- Hard caps via wasmtime fuel + per-invocation timeout.

### 6.3 Native backend (`abi = "native"`)

- Same traits, loaded via `libloading::Library::new` — a **direct precedent of `OsdiLib`**
  (`crates/piperine-solver/src/osdi/loader.rs:25-69`).
- The `.cdylib` exports a single C entry `piperine_plugin_entry() -> *mut dyn Plugin`.
- No real sandbox; trust derives from TOFU + content hash + declared capabilities
  (audit, not isolation). Useful for heavy external bridges (simavr, Verilator).
- `piperine-solver/build.rs:66` already exports host symbols so that math functions are
  visible to loaded OSDI plugins — the same mechanism ensures host functions are available
  to native Piperine plugins.

### 6.4 Out-of-process backend (`abi = "process"`)

The plugin is an executable speaking JSON-RPC 2.0 over stdio. The host spawns it via
`std::process`. Real isolation: a plugin crash cannot take down the host; the process can
be containerized / sandboxed. Trade-off: per-call latency in the millisecond range. Ideal
for heavyweight bridges (QEMU, ModelSim). This backend is the lowest priority tier.

---

## 7. SDK crate: `piperine-plugin`

A new crate at `crates/piperine-plugin/`:

```
crates/piperine-plugin/
├── src/
│   ├── lib.rs            # re-exports
│   ├── manifest.rs       # Manifest, Permissions, parsing
│   ├── sdk.rs            # Plugin trait, Registrar, DeviceFactory
│   ├── host.rs           # PluginHost: load, order, dispatch hooks
│   ├── error.rs          # PluginError (thiserror + miette::Diagnostic)
│   ├── trust.rs          # TOFU + lockfile integration
│   ├── capability.rs     # HostCtx, capability enforcement
│   ├── view/             # DesignView, DesignPatch, AttributeView (serializable)
│   │   ├── design.rs
│   │   ├── module.rs
│   │   └── patch.rs
│   ├── backend/
│   │   ├── mod.rs        # trait Backend
│   │   ├── wasm.rs       # wasmtime
│   │   ├── native.rs     # libloading
│   │   └── process.rs    # JSON-RPC over stdio (lowest tier)
│   └── wit/              # WIT interfaces (WASM tier)
└── tests/
    └── native_smoke.rs   # load a test .cdylib, fire hooks
```

### 7.1 `DeviceFactory` — the bridge into the solver

```rust
pub trait DeviceFactory: Send + Sync {
    fn kind(&self) -> DeviceKind;          // Digital | Analog | Mixed
    fn instantiate(&self, spec: &DeviceSpec) -> Result<Box<dyn Device>>;
}

pub struct DeviceSpec<'a> {
    pub type_id: String,                   // "Arduino::UnoR3"
    pub attributes: &'a [piperine_lang::pom::Attribute],
    pub port_bindings: Vec<PortBinding>,   // name -> resolved NetRef
    pub params: Vec<(String, Value)>,
}
```

The host injects the resulting `Box<dyn Device>` into `InstanceBuilder::devices` inside
`CircuitCompiler` (`crates/piperine-codegen/src/device/circuit.rs:291-431`) — see §8.

### 7.2 `PluginError`

Follows the codebase convention (thiserror + miette), occupying a fresh error-code range
to avoid colliding with parse (E1xxx) / elaboration (E2xxx) / reflection (E3xxx):

```rust
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum PluginError {
    #[error("plugin `{0}` is untrusted (TOFU pending)")]
    #[diagnostic(code(P0001))]
    Untrusted(String),

    #[error("plugin `{0}` used capability `{1}` not declared in its manifest")]
    #[diagnostic(code(P0002))]
    UndeclaredCapability(String, String),

    #[error("attribute schema `{0}` already registered by plugin `{1}`")]
    #[diagnostic(code(P0003))]
    SchemaConflict(String, String),

    #[error("device `{0}` is not registered by any loaded plugin")]
    #[diagnostic(code(P0004))]
    DeviceNotRegistered(String),

    #[error("hook `{0}` failed in plugin `{1}`: {2}")]
    #[diagnostic(code(P0005))]
    HookFailed(&'static str, String, String),

    #[error(transparent)] Native(#[from] libloading::Error),
    #[error(transparent)] Wasm(#[from] wasmtime::Error),
    #[error(transparent)] Project(#[from] piperine_project::ProjectError),
}
```

---

## 8. Device loading

### 8.1 Binding flow

1. **Elaboration.** A module or instance carries
   `@device(plugin = "avr-cosim", type = "Arduino::UnoR3")`. The elaboration pass
   (`crates/piperine-lang/src/elab/lower/passes.rs`) consults the new
   **AttributeSchemaRegistry** (populated by plugins during `register()`). An unknown
   schema raises `ElabError::UnknownAttrSchema` — closing the SPEC §313-314 gap.
2. **Pre-lowering.** `CircuitCompiler::add_instance` (`device/circuit.rs:291`) detects the
   `@device(plugin=…)` attribute. Instead of calling `CompiledModule::compile`, it
   delegates to `PluginHost::build_device(spec)`.
3. **Construction.** The host locates the `DeviceFactory` registered for
   `"Arduino::UnoR3"`, calls `instantiate(spec)`, receives a `Box<dyn Device>`, and adds
   it to the `InstanceBuilder`.
4. **Port mapping.** The plugin reads `spec.port_bindings` (derived from the module
   ports' `@port(name = …)` attributes) to know which `NetRef` resolves to "A1", "D0",
   etc.

### 8.2 AVR reference example

`.ppr`:

```phdl
@device(plugin = "avr-cosim", type = "Arduino::UnoR3")
module ArduinoUno {
    @port(name = "A0", kind = "analog")  port a0;
    @port(name = "A1", kind = "analog")  port a1;
    @port(name = "D0", kind = "digital") port d0;
    @port(name = "D1", kind = "digital") port d1;
}
```

Plugin:

```rust
struct AvrCosim;
impl Plugin for AvrCosim {
    fn register(&self, r: &mut Registrar) {
        r.attr_schema("device", DEVICE_SHAPE);
        r.attr_schema("port",   PORT_SHAPE);
        r.device("Arduino::UnoR3", Box::new(UnoR3Factory));
    }
}

struct UnoR3Factory;
impl DeviceFactory for UnoR3Factory {
    fn kind(&self) -> DeviceKind { DeviceKind::Digital }
    fn instantiate(&self, spec: &DeviceSpec) -> Result<Box<dyn Device>> {
        // spec.port_bindings[{A0,A1,D0,D1}] -> NetRefs resolved by CircuitCompiler
        let avr = AvrSim::load_firmware(spec.attributes, &spec.port_bindings)?;
        Ok(Box::new(AvrDigitalDevice::new(avr)))
    }
}

// AvrDigitalDevice implements DigitalDevice (trait at solver/src/digital/interface.rs:81).
// init()       -> ask simavr for the initial port state.
// comb_phase() -> read inputs from EvalCtx, advance one cycle, emit events via EventSink.
// boundary()   -> return the DigitalPorts corresponding to the @port(name=...) entries.
```

The solver sees `AvrDigitalDevice` as **just another `DigitalDevice`** — the scheduler
never learns it is a co-simulator. That is exactly the use case anticipated by the
`interface.rs:8-16` doc comment.

### 8.3 Pure analog devices

Same mechanism, implementing `AnalogDevice` (`solver/src/core/device.rs:18-53`). All of
`load_dc` / `load_ac` / `load_transient` / `noise_current_psd` have default no-op
implementations, so a plugin overrides only the loads it needs. Useful for proprietary
models that have no OSDI form.

### 8.4 WASM devices (experimental)

Devices run inside the Newton inner loop (many calls per step). Pure snapshot-based WASM
is too expensive there. Strategy:

- **Tier 1 (initial):** devices supported only via `native` or `process`. WASM reserved
  for hooks / scripts / POM reflection.
- **Tier 2 (research):** a "WASM fast path" — the plugin exports `load_dc`-style functions
  with a C ABI through `wasmtime::Func` (no per-call snapshot; the host passes arrays via
  shared linear memory). Feasible because `AnalogFn` is already
  `unsafe extern "C" fn(...)` (`crates/piperine-codegen/src/jit/analog.rs:46`). The same
  idea applies to digital `comb_phase`.

---

## 9. Lifecycle hooks

### 9.1 The seven hook points (aligned with the real pipeline)

| # | Hook | When | Input | Mutable? | Use case |
|---|---|---|---|---|---|
| 1 | `after_parse` | after parser, before elaboration | `&SourceFileView` | no | custom lint, metrics |
| 2 | `after_elaborate` | once `Design` is ready | `&DesignView` | no | reporting, external validation |
| 3 | `transform_design` | before `fork` / lower | `&mut DesignStaging` | yes (via staging `overrides`) | set params, inject parasitic instances |
| 4 | `before_lower` | just before `lower_bodies` | `&DesignView` | no | final POM audit |
| 5 | `after_lower` | after `lower_bodies` | `&mut LoweredBodiesView` | yes (rare; capability `lowered = "read-write"`) | inject stamps / parasitics directly into the IR |
| 6 | `before_solve` | after `CircuitCompiler` | `&CircuitHandle` | no | instrument, log |
| 7 | `after_solve` | after an analysis | `&SolveResult` | no | extract metrics, custom reports |

Plus: bench hooks are delivered through `on_bench_event` registered as a plugin
`SimTask` (`bench_hooks = true`), dispatched by `SimHost::syscall`
(`crates/piperine-bench/src/host.rs:207`). This is the natural path to close
`extract` / `.attach` / `.meta` (bench SPEC §11).

### 9.2 Why mutation happens through `DesignStaging`, not `&mut Design`

`Design` is **immutable after elaboration by design** — the only mutation surface is the
staging `overrides` (`crates/piperine-lang/src/pom/design.rs:196`,
`with_overrides_applied` at design.rs:236-271, `fork()` at design.rs:219). Plugins
**respect that**:

```rust
pub struct DesignStaging<'a> {
    design: &'a Design,
    overrides: &'a Rc<RefCell<OverrideMap>>,
}

impl<'a> DesignStaging<'a> {
    pub fn set_param(&self, path: &str, value: Value);          // wraps Design::set_param
    pub fn add_instance(&self, parent: &str, inst: InstanceSpec);
    pub fn add_connection(&self, parent: &str, conn: ConnectionSpec);
    pub fn attributes_on(&self, path: &str) -> Result<&[Attribute]>;
}
```

`add_instance` / `add_connection` are **new** on `OverrideMap` — a surgical extension to
`piperine-lang/src/pom/staging.rs` that preserves the "staging → fork → applied" model.
This prevents a plugin from breaking POM invariants directly.

### 9.3 Reference case: parasitics

A plugin `rc-parasitics`:

1. `register()` declares `attr_schema("extract_rc", …)`.
2. `after_elaborate` reads `@extract_rc` on instances, computes R/C from geometric
   attributes (length, width) — read-only; records an internal to-do list.
3. `transform_design` walks the to-do list and calls `staging.add_instance(parent,
   Resistor{…})` plus `staging.add_connection(…)`. The result is an applied `Design`
   carrying the parasitics before `lower_bodies` runs. **The codegen-private IR is never
   touched.**

An advanced variant (capability `lowered = "read-write"`) adds stamps directly to the
`LoweredBodiesView` in the `after_lower` hook. More efficient, more coupled — opt-in.

### 9.4 Ordering

Plugins run in **alphabetical order by name** within each hook (deterministic, easy to
reason about). Conflicts (two plugins mutating the same path) are detected by the staging
layer (since `OverrideMap` is keyed by path) and surface as
`PluginError::StagingConflict`.

---

## 10. Custom scripts (Cargo-style)

### 10.1 Registration

Manifest declares `[scripts.spice]`; the plugin contributes the handler:

```rust
impl Plugin for SpiceImport {
    fn register(&self, r: &mut Registrar) {
        r.script("spice", Box::new(SpiceImportScript));
    }
}

struct SpiceImportScript;
impl ScriptHandler for SpiceImportScript {
    fn invoke(&self, args: ScriptArgs, ctx: &mut HostCtx) -> Result<ExitCode> {
        let input  = args.path("INPUT")?;
        let output = args.path("--output")?;
        let netlist = ctx.fs().read_to_string(&input)?;     // capability-gated
        let design  = parse_spice(&netlist)?;               // plugin logic
        ctx.fs().write(&output, design.to_ppr()?)?;         // capability-gated
        Ok(ExitCode::SUCCESS)
    }
}
```

### 10.2 CLI integration

`Commands` in `crates/piperine-cli/src/lib.rs:14` gains a plugin-script catch-all. Before
treating an unknown subcommand as an error, the dispatcher consults `PluginHost` for a
registered script:

```rust
// commands/plugin_script.rs (new)
pub fn execute(name: String, args: Vec<String>) -> Result<()> {
    let host = PluginHost::load_for_project()?;       // reads Piperine.toml + lockfile
    let script = host.script(&name)
        .ok_or_else(|| PluginError::UnknownScript(name.clone()))?;
    script.invoke(args.into(), &mut host.ctx())?;
    Ok(())
}
```

The `match cli.command` at `lib.rs:83-125` gains a fallback: if no builtin subcommand
matches, dispatch to `commands::plugin_script::execute(name, rest)`. Clap is configured
with `allow_external_subcommands = true` so that `piperine spice foo.cir -o foo.ppr` is
captured cleanly.

`piperine plugin list` shows loaded plugins and available scripts; `piperine help` merges
builtin help with script help (declared in each manifest).

### 10.3 `HostCtx` surface for scripts

Scripts receive a `HostCtx` with explicit APIs:

- `ctx.fs()` — filesystem restricted to the manifest globs, resolved relative to the
  project root.
- `ctx.project()` — access to `Piperine.toml`, the loaded `Design`, the target directory.
- `ctx.ppr()` — reader/writer for the `.ppr` format (parse → POM `Design`, serialize).
- `ctx.spawn(exe, args)` — only if `process_spawn` is granted and `exe` is whitelisted;
  stdout/stderr captured and logged.
- `ctx.log(level, msg)` — routes to the host logger.

There is no `ctx.system()` and no `ctx.network()` unless the capability is granted.

### 10.4 Dependency: a public `.ppr` (de)serializer

Scripts read and write `.ppr` files. This plan assumes a public
`Design::to_ppr() / from_ppr()` surface on `piperine-lang`. If a binary `.ppr` format is
not yet exposed, exposing it becomes a prerequisite of the script tier (not a plugin
concern). The text `.phdl` form is always available as a fallback serialization.

---

## 11. Integration surface

All changes are surgical, reversible, and each carries its own test. The dependency
direction (`piperine-solver` never depends on `piperine-codegen`) is preserved.

### `piperine-lang`

| File | Change |
|---|---|
| `src/elab/registry/mod.rs:12` | New `AttributeSchemaRegistry` member on `ElabContext`. Populated by plugins at boot. |
| `src/elab/lower/passes.rs:25` | `Register` pass validates attributes against the registry; implements `UnknownAttrSchema` (SPEC §313). |
| `src/pom/staging.rs` | Extend `OverrideMap` with `add_instance` / `add_connection` (consumed by `DesignStaging`). |
| `src/pom/error.rs:10` | New variant `UnknownAttrSchema(String)`. |
| `src/pom/design.rs` (public surface) | Public `to_ppr()` / `from_ppr()` if not already present (script tier depends on it). |

### `piperine-codegen`

| File | Change |
|---|---|
| `src/device/circuit.rs:291-431` | `InstanceBuilder::add_instance`: if the attributes contain `@device(plugin=…)`, delegate to `PluginHost::build_device`; otherwise the existing path. |
| `src/device/circuit.rs:73` | `CircuitCompiler::new` accepts `Option<&PluginHost>` (backward-compatible: `None` keeps current behavior). |

### `piperine-bench`

| File | Change |
|---|---|
| `src/host.rs:207` | `SimHost::syscall` first consults the builtin `SimTaskRegistry`, then plugin-registered tasks. |
| `src/tasks.rs:284` | `SimTaskRegistry::with_builtins` plus a new `with_plugins(host)` extension. |
| `src/session.rs:87` | Each `run_*` fires the `before_lower` / `after_lower` / `before_solve` / `after_solve` hooks when a `PluginHost` is present. |

### `piperine-cli`

| File | Change |
|---|---|
| `src/lib.rs:14` | `Commands` with `allow_external_subcommands`; fallback to `plugin_script::execute`. |
| `src/commands/plugin_script.rs` (new) | Script dispatcher. |
| `src/commands/plugin.rs` (new) | `piperine plugin {list, trust, update, add, remove}`. |

### `piperine-project`

| File | Change |
|---|---|
| `src/lib.rs:12` | `PiperineToml` gains `[plugins]`. |
| `src/resolver.rs:39` | `Resolver::resolve_plugins()` reusing the existing git walker. |
| `src/lockfile.rs:6` | `LockEntry` gains `kind: EntryKind { Dependency, Plugin }` plus the plugin hash fields. |

### New: `piperine-plugin`

The whole crate (§7). It depends on `piperine-lang` (POM types), `piperine-solver` (device
traits), and `piperine-project` (resolver / lockfile). It does **not** depend on
`piperine-codegen` internals — it reaches codegen only through the public POM plus the
injection points listed above.

---

## 12. Scope tiers

The contract is specified as a whole; delivery is gated by tier so each tier is
independently useful and testable.

- **Tier 0 — Skeleton.** `piperine-plugin` crate with `Plugin` / `Registrar` / `Manifest`
  / `PluginError` / `HostCtx` stubs (no real backends). `[plugins]` section in
  `Piperine.toml` and lockfile entries. Tests: manifest parsing, capability validation,
  lockfile round-trip.
- **Tier 1 — Native backend + devices.** `backend/native.rs` via `libloading` (mirrors
  `OsdiLib`). `DeviceFactory` injected into `CircuitCompiler`. `AttributeSchemaRegistry`
  on `ElabContext`; the `Register` pass validates schemas. TOFU + content hash in the
  lockfile. **Gate:** an `avr-cosim` sample plugin loads, `Arduino::UnoR3` shows up as a
  `DigitalDevice` in the solver, and a co-sim bench runs end-to-end.
- **Tier 2 — Hooks + scripts.** `DesignStaging` with `add_instance` / `add_connection`.
  Hooks `after_elaborate` / `transform_design` / `before_lower` / `after_solve`.
  `Registrar::script` and the CLI catch-all dispatch. **Gate:** an `rc-parasitics` plugin
  injects resistors in a bench; `piperine spice foo.cir -o foo.ppr` works with a sample
  plugin.
- **Tier 3 — WASM backend.** `backend/wasm.rs` with wasmtime + WIT. Serializable
  `DesignView` / `DesignPatch` (CBOR). Hooks and scripts via WASM; WASM devices remain
  experimental. **Gate:** the `rc-parasitics` plugin re-implemented in WASM-Rust, without
  recompiling the host, runs the same bench.
- **Tier 4 — Maturation.** `extract` / `.attach` / `.meta` delivered via bench-task
  plugins (closes G13). Out-of-process backend (`process`) for strong isolation. LSP
  integration so plugins can contribute diagnostics / completion through
  `piperine-lang-server`. Optional public registry.

---

## 13. Risks and trade-offs

| Risk | Mitigation |
|---|---|
| Native plugin crashes the host | Document that native = full trust; offer the `process` backend for real isolation. |
| POM snapshot per WASM hook is expensive | Hooks run outside the inner loop; measure before optimizing; add incremental caching if needed. |
| Schema collisions between plugins | `PluginError::SchemaConflict` at `register()` time; namespace by plugin name (`@avr-cosim:device`). |
| `add_instance` via staging breaks invariants | Staging remains the single mutation point; `with_overrides_applied` validates the coherence of the resulting `Design`. |
| WIT / WASI preview 2 is still moving | Version the WIT interface (`piperine.plugin@1.0`); semantic-version the contract. |
| Proc-macros conflict with "no macro magic" | The SDK's primary API is trait-based; macros are opt-in and live in a separate `piperine-plugin-macros` crate that can be ignored. |
| Co-sim performance (AVR) | The `DigitalDevice` proxy is the extension point designed for exactly this (interface.rs:8-16); latency is bounded by simavr, not Piperine. |

---

## 14. Reference use cases (design validation matrix)

These five cases exercise every pillar and become integration tests in
`crates/piperine-plugin/tests/`:

1. **AVR co-sim** (`avr-cosim`, native). A PHDL module annotated
   `@device(type="Arduino::UnoR1")`, ports annotated `@port(name="A1")`, firmware `.hex`
   on the project path. A bench asserts a digital pin toggles over time. Exercises §8
   (device loading + attribute binding).
2. **Parasitics** (`rc-parasitics`, wasm). Geometric attributes on instances; the plugin
   injects series R/C before lowering; an AC run compares with and without parasitics.
   Exercises §9 hooks via `transform_design`.
3. **SPICE import** (`spice-import`, wasm / script). `piperine spice rectifier.cir -o
   rectifier.ppr` produces an equivalent `.ppr`. Exercises §10 scripts.
4. **Proprietary analog device** (`bsim4-proprietary`, native). A vendor ships a `.so`
   implementing `AnalogDevice` directly, without OSDI. Exercises §8.3.
5. **Custom metrics** (`power-audit`, wasm, `after_solve` hook). Reads the DC result,
   computes dissipation, writes a report. No mutation permission granted. Exercises the
   minimum-capability read-only hook path.

---

## 15. Resolved decisions

Each ambiguity in the original four requirements is resolved here, with rationale.

1. **Default ABI.** WASM is the default; native requires `--allow-native` and TOFU.
   Rationale: the security requirement is non-negotiable, and WASM is the only tier that
   honors it without trusting the publisher.
2. **`.ppr` serialization.** The script tier depends on a public
   `Design::to_ppr() / from_ppr()` surface on `piperine-lang`. If absent today, exposing
   it is a prerequisite of the script tier (Tier 2), not a plugin responsibility. The
   text `.phdl` form is the fallback.
3. **Proc-macros in the SDK.** Allowed, in a separate `piperine-plugin-macros` crate, as
   opt-in sugar. The trait-based `Plugin::register` API is primary and always sufficient.
   This keeps macro magic out of the core while not penalizing plugin authors.
4. **Network policy.** Blocked by default in every tier. `--allow-net` opts in for a single
   invocation; persistent network access requires the manifest capability plus TOFU. CI
   runs with `--no-trust` reject native plugins and ignore `--allow-net`.
5. **Public registry.** Out of scope for this spec. The contract is local (git/path
   sources). A future registry would layer on top of the same manifest and resolver.
6. **Mutation surface.** Plugins never receive `&mut Design`. They mutate through
   `DesignStaging`, preserving the existing "Design is immutable after elaboration"
   invariant and reusing the staging machinery already used by bench forks.
7. **Backend isolation boundary.** Native = trust (in-process, like OSDI). Process = real
   isolation (separate OS process, JSON-RPC). WASM = sandbox by construction. The three
   tiers give a clean ladder from "I trust this vendor" to "I just downloaded this from
   an unverified repo".
8. **Attribute schema registry.** Lives on `ElabContext` as a new registry alongside
   `TypeRegistry` / `ComponentRegistry` / `CallableRegistry` / `EventRegistry`, populated
   at boot from loaded plugins. This closes the SPEC §313-314 "unknown schema" gap as a
   direct side effect of the plugin model.

---

## 16. Summary

- **Security.** WASM sandbox by default; native requires TOFU + content hash + pinned rev;
  capabilities deny-by-default; out-of-process backend for real isolation.
- **Devices.** `DeviceFactory` produces a `Box<dyn Device>` injected into
  `CircuitCompiler`; the solver sees it as any other `AnalogDevice` / `DigitalDevice`.
  Binding flows through `@device` / `@port` (and closes the `UnknownAttrSchema` gap).
- **Hooks.** Seven surgical hook points; mutation happens only through the existing
  staging surface (`OverrideMap`); parasitics are the canonical case.
- **Scripts.** CLI catch-all dispatch; `HostCtx` is capability-gated; `piperine spice
  foo.cir -o foo.ppr` is the canonical example.
- **Rules preserved.** The solver still does not depend on codegen; the POM remains the
  public reflection contract; the codegen-private IR is untouched; "fail loud" and "no
  macro magic" are respected.
