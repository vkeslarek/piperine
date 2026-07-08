# Part VI — Plugins

The plugin extensibility model. Plugins extend Piperine with custom devices,
design-transform hooks, custom scripts, and attribute schemas — without
breaking "fail loud", without coupling the solver to codegen, and without
compromising security.

A plugin is a loadable artifact (WASM module, native shared library, or
external process) that registers contributions through a single `Plugin`
trait. It is declared in `Piperine.toml`, resolved through the existing
dependency resolver, and admitted through a trust-on-first-use (TOFU)
approval flow. The POM is the reflection contract; the codegen-private IR
is never exposed.

## Contents

- §1 Goals and non-goals
- §2 Design principles
- §3 Security model
  - §3.1 Threats and countermeasures
  - §3.2 Trust on first use (TOFU)
  - §3.3 Capability enforcement
- §4 Plugin manifest
- §5 Discovery and resolution
  - §5.1 Project configuration
  - §5.2 Lockfile
- §6 ABI tiers
  - §6.1 The Plugin trait
  - §6.2 WASM backend
  - §6.3 Native backend
  - §6.4 Out-of-process backend
- §7 Device loading
  - §7.1 Binding flow
  - §7.2 Declarative binding (`@device` / `@port`)
  - §7.3 The DeviceFactory bridge
- §8 Lifecycle hooks
  - §8.1 Hook points
  - §8.2 Mutation through DesignStaging
  - §8.3 Parasitics reference case
  - §8.4 Ordering
- §9 Custom scripts
- §10 Attribute schema registration
- §11 Error catalog
- §12 Scope tiers

---

## §1 Goals and non-goals

**Goals**

- Make devices, hooks, and scripts first-class extension points reachable
  from project imports (git or path), with one SDK and a stable contract.
- Treat the POM as the reflection ABI and the existing `Device` traits as
  the solver ABI. The codegen-private IR stays closed.
- Define a threat model and a permission system that makes "install a plugin
  from a git URL" a safe operation by default.

**Non-goals**

- A public plugin registry / package index (revisit later).
- Reinventing the existing resolver — plugins reuse `piperine-project`'s
  git/path dependency resolution.
- Letting plugins extend the parser grammar, the IR types, or the solver's
  math core. Those stay closed; plugins reflect through the POM and
  contribute behavior.

---

## §2 Design principles

| Principle | Meaning |
|-----------|---------|
| **Security-first, capability-based** | A plugin declares permissions in its manifest; the host denies by default. Missing permission = no effect, not a crash. |
| **Fail loud** | A plugin that requests a nonexistent hook, references an unregistered device, or uses an attribute with no schema raises a typed `PluginError` — never a silent `0.0` or no-op. |
| **Two ABIs, one SDK** | WASM (safe by default, sandboxed) and native `.cdylib` (power, explicit opt-in) share the same `Plugin` trait and hooks. |
| **The POM is the reflection contract** | The POM is public and stable; the codegen IR stays closed. Plugins reflect through the POM. |
| **No macro magic** | Registration via `Plugin::register(&mut Registrar)` — plain trait methods. Proc-macros are opt-in sugar, never required. |
| **Do not surprise the solver** | `piperine-solver` still does not depend on `piperine-codegen`. Plugins talk to the solver only through the existing `Device` traits. |

---

## §3 Security model

### 3.1 Threats and countermeasures

| Threat | Countermeasure |
|--------|----------------|
| Malicious git repo carrying a payload in the build script or shared library | WASM sandbox by default. Native requires explicit opt-in + TOFU approval + content hash in the lockfile. Pinned revision is mandatory for native. |
| Plugin reads sensitive files or writes outside the project | Capability-based filesystem: plugin may only access paths declared in the manifest, resolved relative to the project root. |
| Plugin exfiltrates over the network | `network = false` by default. WASM has no sockets. Native with `network = true` requires interactive approval on first load. |
| Plugin spawns a process | `process_spawn = false` by default. WASM never. Native needs the capability and logs every spawn. |
| Silent binary swap via a git push | `Piperine.lock` stores the content hash of the loaded artifact. A hash change forces re-approval. |
| Native plugin crashes the host | Loaded in-process (same model as OSDI). For strong isolation, an out-of-process runner is offered via stdio JSON-RPC (§6.4). |
| DoS via an infinite loop in a hook | WASM hooks have a per-invocation timeout (default 5 s). Native hooks have none — native is full trust. |

### 3.2 Trust on first use (TOFU)

On the first load of a plugin (or whenever the artifact hash changes) the
CLI blocks and presents:

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

- `y` writes the hash to `Piperine.lock`. Never asks again while the hash
  is unchanged.
- `N` aborts with `PluginError::Untrusted`.
- `details` shows the manifest diff and the files that would be accessed.

CI modes: `--trust <file>` reads decisions from a checked-in trust file;
`--no-trust` silently rejects native plugins (read-only sandbox mode).

### 3.3 Capability enforcement

The host exposes a facade (`HostCtx`) to the plugin. Every side-effecting
call (filesystem, process spawn, network) goes through it and is checked
against the manifest capabilities. In WASM this is natural (host imports);
in native, the SDK offers the same entry points and the publisher is
expected to call them rather than the OS directly (documented contract;
best-effort). This is **not** a cryptographic sandbox for native —
in-process sandboxing is impossible. It is **audit + opt-in +
reproducibility**. Real isolation comes from the out-of-process backend
(§6.4).

---

## §4 Plugin manifest

The manifest (`piperine-plugin.toml`) lives at the root of the plugin
repository. It declares the plugin's identity, ABI, permissions, attribute
schemas, device registrations, and custom scripts.

```toml
[plugin]
name        = "avr-cosim"
version     = "0.1.0"
abi         = "wasm"              # "wasm" | "native" | "process"
entry       = "avr_cosim.wasm"    # .wasm | .cdylib/.dll/.so | runner binary path
description = "Co-simulation bridge for AVR microcontrollers"

[permissions]
design         = "read-write"     # "none" | "read" | "read-write"
filesystem     = ["read *.cir", "write *.ppr"]
network        = false
process_spawn  = ["simavr", "avr-objcopy"]   # whitelist of executables
timeout_ms     = 5000             # per WASM hook invocation

[attributes."device"]
fields = { plugin = "String", type = "String", params = "Map<String,Value>" }
applies_to = ["module", "instance"]

[attributes."port"]
fields   = { name = "String", kind = "String" }
applies_to = ["port"]

[devices."Arduino::UnoR3"]
factory   = "uno_r3"
kind      = "digital"             # "digital" | "analog" | "mixed"
boundary  = ["A0","A1","A2","A3","D0","D1","D2","D3"]

[scripts.spice]
entry    = "import_spice"
summary  = "Import a SPICE netlist (.cir) into a .ppr file"
args     = [
  { name = "INPUT",   required = true,  kind = "path" },
  { name = "--output", short = "-o", required = true, kind = "path" },
]
```

The manifest is parsed once at load time; capability fields become a
`Permissions` struct the host carries for the plugin's lifetime.

**Validation.** An invalid manifest (missing required fields, unknown ABI,
malformed permissions) is `PluginError::BadManifest` at load time, before
any hook runs.

---

## §5 Discovery and resolution

### 5.1 Project configuration

A new optional `[plugins]` section in `Piperine.toml`, separate from
`[dependencies]` (plugins are not PHDL libraries):

```toml
[plugins.avr-cosim]
git = "https://github.com/acme/piperine-avr"
rev = "abc1234"          # mandatory for abi=native; recommended for wasm

[plugins.spice-import]
path = "../piperine-spice"   # local path dep
```

### 5.2 Lockfile

`Piperine.lock` gains plugin entries with content hashes:

```toml
[[plugins]]
name          = "avr-cosim"
source        = "git+https://github.com/acme/piperine-avr@abc1234"
manifest_hash = "sha256:…"
content_hash  = "sha256:…"
abi           = "wasm"
trusted_at    = "2026-07-07T10:11:12Z"
```

Any hash change forces re-approval. Plugins resolve into
`target/plugins/<name>/` — separated from PHDL library deps.

**Validation.** A plugin whose lockfile hash does not match the loaded
artifact is `PluginError::HashMismatch` — the run aborts before any code
executes.

---

## §6 ABI tiers

### 6.1 The Plugin trait

One trait, valid for all backends:

```rust
pub trait Plugin: Send + Sync {
    fn manifest(&self) -> &Manifest;
    fn register(&self, r: &mut Registrar) { let _ = r; }

    // Lifecycle hooks (§8). All default to no-op.
    fn after_parse(&self, _ctx: &mut HostCtx, _src: &SourceFileView) -> Result<()> { Ok(()) }
    fn after_elaborate(&self, _ctx: &mut HostCtx, _design: &DesignView) -> Result<()> { Ok(()) }
    fn transform_design(&self, _ctx: &mut HostCtx, _staging: &mut DesignStaging) -> Result<()> { Ok(()) }
    fn before_lower(&self, _ctx: &mut HostCtx, _design: &DesignView) -> Result<()> { Ok(()) }
    fn after_lower(&self, _ctx: &mut HostCtx, _bodies: &mut LoweredBodiesView) -> Result<()> { Ok(()) }
    fn before_solve(&self, _ctx: &mut HostCtx, _circuit: &CircuitHandle) -> Result<()> { Ok(()) }
    fn after_solve(&self, _ctx: &mut HostCtx, _result: &SolveResult) -> Result<()> { Ok(()) }
}
```

`Registrar` is the builder plugins use to contribute devices, attribute
schemas, bench tasks, and scripts — all optional:

```rust
impl Registrar<'_> {
    pub fn device(&mut self, type_id: &str, factory: Box<dyn DeviceFactory>);
    pub fn attr_schema(&mut self, schema: &str, shape: AttrShape);
    pub fn bench_task(&mut self, task: Box<dyn SimTaskPlugin>);
    pub fn script(&mut self, name: &str, handler: Box<dyn ScriptHandler>);
}
```

### 6.2 WASM backend (default, sandboxed)

- Runtime: `wasmtime` with WIT (WebAssembly Interface Types) for declarative
  typed interfaces.
- Types cross the boundary as serialized snapshots of POM views. A plugin
  never receives a raw `&Design` pointer; it receives a `DesignView` (a
  serializable struct) and returns a `DesignPatch` that the host validates
  and applies.
- Cost: a copy per hook invocation. Acceptable — hooks run a handful of
  times per simulation, not in the Newton inner loop.
- Hard caps via wasmtime fuel + per-invocation timeout.

### 6.3 Native backend (`abi = "native"`)

- Same traits, loaded as a shared library (`.cdylib` / `.dll` / `.so`).
  The library exports a single C entry point returning a `*mut dyn Plugin`.
- No real sandbox; trust derives from TOFU + content hash + declared
  capabilities (audit, not isolation). Useful for heavy external bridges
  (simavr, Verilator).
- Host symbols (math functions) are exported to the loaded library, same
  mechanism as OSDI device-model loading.

### 6.4 Out-of-process backend (`abi = "process"`)

The plugin is an executable speaking JSON-RPC 2.0 over stdio. The host
spawns it via `std::process`. Real isolation: a plugin crash cannot take
down the host; the process can be containerized. Trade-off: per-call
latency in the millisecond range. Ideal for heavyweight bridges (QEMU,
ModelSim).

---

## §7 Device loading

### 7.1 Binding flow

1. **Elaboration.** A module or instance carries
   `@device(plugin = "avr-cosim", type = "Arduino::UnoR3")`. The elaboration
   pass validates the attribute against the plugin-registered schema. An
   unknown schema or unregistered device type is an error.
2. **Pre-lowering.** The circuit compiler detects the `@device` attribute.
   Instead of compiling the module from PHDL source, it delegates to the
   plugin's `DeviceFactory`.
3. **Construction.** The host locates the `DeviceFactory` registered for
   the type ID, calls `instantiate(spec)`, receives a `Box<dyn Device>`,
   and injects it into the circuit.
4. **Port mapping.** The plugin reads the port bindings (derived from
   `@port(name = …)` attributes) to know which `NetRef` resolves to each
   logical port name.

The solver sees the plugin-provided device as **just another `Device`** —
the solver never learns it came from a plugin.

### 7.2 Declarative binding (`@device` / `@port`)

```
Attribute ::= "@" Ident "(" [ AttrArg { "," AttrArg } ] ")"
```

A module annotated with `@device(plugin = "name", type = "TypeId")` declares
that its behavior is provided by a plugin device, not by PHDL `analog`/
`digital` blocks. Ports annotated with `@port(name = "A0", kind = "analog")`
map logical port names to the plugin's boundary.

```phdl
@device(plugin = "avr-cosim", type = "Arduino::UnoR3")
module ArduinoUno {
    @port(name = "A0", kind = "analog")  port a0;
    @port(name = "A1", kind = "analog")  port a1;
    @port(name = "D0", kind = "digital") port d0;
    @port(name = "D1", kind = "digital") port d1;
}
```

### 7.3 The DeviceFactory bridge

```rust
pub trait DeviceFactory: Send + Sync {
    fn kind(&self) -> DeviceKind;          // Digital | Analog | Mixed
    fn instantiate(&self, spec: &DeviceSpec) -> Result<Box<dyn Device>>;
}

pub struct DeviceSpec<'a> {
    pub type_id: String,                   // "Arduino::UnoR3"
    pub attributes: &'a [Attribute],
    pub port_bindings: Vec<PortBinding>,   // name -> resolved NetRef
    pub params: Vec<(String, Value)>,
}
```

The resulting `Box<dyn Device>` implements the solver's `AnalogDevice` or
`DigitalDevice` trait. The solver schedules it like any other device.

**Validation.** A `@device` attribute referencing a type not registered by
any loaded plugin is `PluginError::DeviceNotRegistered`. A port listed in
`@port` that does not match the device's declared boundary is an error at
circuit-build time.

---

## §8 Lifecycle hooks

### 8.1 Hook points

Seven hook points aligned with the compilation pipeline:

| # | Hook | When | Input | Mutable? | Use case |
|---|------|------|-------|----------|----------|
| 1 | `after_parse` | after parser, before elaboration | `SourceFileView` | no | custom lint, metrics |
| 2 | `after_elaborate` | once `Design` is ready | `DesignView` | no | reporting, external validation |
| 3 | `transform_design` | before lowering | `DesignStaging` | yes (via staging overrides) | set params, inject parasitic instances |
| 4 | `before_lower` | just before body lowering | `DesignView` | no | final POM audit |
| 5 | `after_lower` | after body lowering | `LoweredBodiesView` | yes (rare; requires `lowered = "read-write"` capability) | inject stamps / parasitics directly |
| 6 | `before_solve` | after circuit compilation | `CircuitHandle` | no | instrument, log |
| 7 | `after_solve` | after an analysis | `SolveResult` | no | extract metrics, custom reports |

Bench hooks are delivered through plugin-registered `SimTask` entries,
dispatched by the bench interpreter's syscall path. This is the path for
`extract` / `.attach` / `.meta` (Part III §9).

### 8.2 Mutation through DesignStaging

`Design` is immutable after elaboration by design — the only mutation
surface is the staging overrides (the same mechanism bench fns use to stage
param changes). Plugins respect that:

```rust
pub struct DesignStaging<'a> { /* borrows Design + OverrideMap */ }

impl DesignStaging<'_> {
    pub fn set_param(&self, path: &str, value: Value);
    pub fn add_instance(&self, parent: &str, inst: InstanceSpec);
    pub fn add_connection(&self, parent: &str, conn: ConnectionSpec);
    pub fn attributes_on(&self, path: &str) -> Result<&[Attribute]>;
}
```

Mutation through staging preserves the "staging → fork → applied" model.
A plugin never receives `&mut Design`.

### 8.3 Parasitics reference case

A plugin `rc-parasitics`:

1. `register()` declares the `@extract_rc` attribute schema.
2. `after_elaborate` reads `@extract_rc` on instances, computes R/C from
   geometric attributes — read-only; records an internal to-do list.
3. `transform_design` walks the to-do list and calls
   `staging.add_instance(parent, Resistor{…})` plus
   `staging.add_connection(…)`. The result is an applied `Design` carrying
   the parasitics before lowering runs.

### 8.4 Ordering

Plugins run in **alphabetical order by name** within each hook
(deterministic, easy to reason about). Conflicts (two plugins mutating the
same path) are detected by the staging layer and surface as
`PluginError::StagingConflict`.

---

## §9 Custom scripts

A plugin may register custom CLI commands (Cargo-style subcommands). The
manifest declares the script name, arguments, and summary; the plugin
provides the handler.

```
$ piperine spice rectifier.cir -o rectifier.ppr
```

The CLI dispatcher checks registered scripts before treating an unknown
subcommand as an error. Scripts receive a `HostCtx` with explicit APIs:

- `ctx.fs()` — filesystem restricted to the manifest globs, relative to
  the project root.
- `ctx.project()` — access to `Piperine.toml`, the loaded `Design`, the
  target directory.
- `ctx.spawn(exe, args)` — only if `process_spawn` is granted and `exe`
  is whitelisted; stdout/stderr captured and logged.
- `ctx.log(level, msg)` — routes to the host logger.

There is no `ctx.system()` and no `ctx.network()` unless the capability is
granted.

`piperine plugin list` shows loaded plugins and available scripts;
`piperine help` merges builtin help with script help.

---

## §10 Attribute schema registration

Plugins register attribute schemas through `Registrar::attr_schema`. This
closes the loop with Part I §8: a plugin that declares
`[attributes."device"]` in its manifest causes the schema name `"device"`
to be registered during the elaborator's registration pass. Any
`@device(...)` attribute in source is then validated against the plugin's
declared shape.

If two plugins register the same schema name, the host raises
`PluginError::SchemaConflict` at load time. Plugins may namespace their
schemas to avoid collisions (e.g., `@avr-cosim:device`).

---

## §11 Error catalog

Plugin errors use the `P0xxx` code range, distinct from parse (`E1xxx`),
elaboration (`E2xxx`), and reflection (`E3xxx`).

| Code | Variant | Trigger |
|------|---------|---------|
| P0001 | `Untrusted(String)` | TOFU pending — plugin not approved |
| P0002 | `UndeclaredCapability(String, String)` | plugin used a capability not in its manifest |
| P0003 | `SchemaConflict(String, String)` | two plugins registered the same schema name |
| P0004 | `DeviceNotRegistered(String)` | `@device` references a type no plugin provides |
| P0005 | `HookFailed(&'static str, String, String)` | a hook returned an error (hook name, plugin, message) |
| P0006 | `BadManifest(String)` | manifest is missing required fields or malformed |
| P0007 | `HashMismatch(String)` | lockfile content hash does not match the loaded artifact |
| P0008 | `StagingConflict(String)` | two plugins mutated the same staging path |
| P0009 | `UnknownScript(String)` | CLI subcommand not registered by any plugin |
| P0099 | `Other(String)` | catch-all |

---

## §12 Scope tiers

The contract is specified as a whole; delivery is gated by tier so each
tier is independently useful and testable.

| Tier | Scope | Gate |
|------|-------|------|
| **0** | Skeleton: `Plugin` / `Registrar` / `Manifest` / `PluginError` / `HostCtx` stubs. `[plugins]` in `Piperine.toml`. Lockfile entries. | Manifest parsing, capability validation, lockfile round-trip. |
| **1** | Native backend + devices. `DeviceFactory` injected into the circuit compiler. Attribute schema registry on `ElabContext`. TOFU + content hash. | An `avr-cosim` sample plugin loads, `Arduino::UnoR3` shows up as a `DigitalDevice`, co-sim bench runs end-to-end. |
| **2** | Hooks + scripts. `DesignStaging` with `add_instance` / `add_connection`. Hooks `after_elaborate` / `transform_design` / `before_lower` / `after_solve`. CLI script dispatch. | An `rc-parasitics` plugin injects resistors in a bench; `piperine spice foo.cir -o foo.ppr` works. |
| **3** | WASM backend. Serializable `DesignView` / `DesignPatch`. Hooks and scripts via WASM. WASM devices experimental. | The `rc-parasitics` plugin re-implemented in WASM-Rust runs the same bench without recompiling the host. |
| **4** | Maturation. `extract` / `.attach` / `.meta` via bench-task plugins. Out-of-process backend. LSP integration. Optional public registry. | — |
