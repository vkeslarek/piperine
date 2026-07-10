# Part VI — Plugins

The plugin extensibility model. Plugins extend Piperine with custom devices,
design-transform hooks, custom scripts, and attribute schemas — without
breaking "fail loud", without coupling the solver to codegen, and without
compromising security.

A plugin is a loadable artifact (WASM module, native shared library, or
external process) that registers contributions through a single contract.
It is declared in `Piperine.toml`, resolved through the existing dependency
resolver, and admitted through a trust-on-first-use (TOFU) approval flow.
The POM is the reflection contract; the codegen-private IR is never exposed.

## Contents

- §1 Position
- §2 Design principles
- §3 Security model
- §4 Plugin manifest
- §5 Discovery and resolution
- §6 ABI tiers
- §7 Device loading
- §8 Lifecycle hooks
- §9 Custom scripts
- §10 Attribute schema registration
- §11 Error catalog
- §12 Validation rules (consolidated)

---

## §1 Position

Plugins are the layer-4 extension mechanism (Part I §14). They open four
extension surfaces without breaking the core pipeline's invariants:

1. **Devices** — custom analog or digital devices loaded through the
   existing `Device` traits. The solver sees them as any other device.
2. **Hooks** — lifecycle points that read (and in one case, mutate) the
   POM at specific stages of the compilation pipeline.
3. **Scripts** — custom CLI subcommands (Cargo-style) for importers,
   exporters, and tooling.
4. **Attribute schemas** — plugin-registered schemas that validate
   `@schema_name(...)` attributes in source (Part I §8).

**What stays closed.** Plugins cannot extend the parser grammar, the IR
types, or the solver's math core. They reflect through the POM and
contribute behavior; they do not modify the compiler's internals. The
codegen-private IR is never exposed to plugins — the POM is the only
reflection surface.

**What stays independent.** `piperine-solver` still does not depend on
`piperine-codegen`. Plugins talk to the solver only through the existing
`Device` traits.

**The ABI is Piperine's own.** The plugin device contract is the native
`AnalogDevice` / `DigitalDevice` trait pair — designed for mixed-signal
simulation and Piperine semantics (two-phase digital evaluation, limited-Newton
analog loading, the event-sink boundary). It is **not** OSDI and does not track
any external model ABI. OSDI compatibility is itself a candidate *plugin*: the
current in-core OSDI loader is slated to move out of `piperine-solver` into an
`osdi-compat` plugin whose device factory wraps compiled OSDI models behind the
native traits.

---

## §2 Design principles

| Principle | Meaning |
|-----------|---------|
| **Security-first, capability-based** | A plugin declares permissions in its manifest; the host denies by default. Missing permission = no effect, not a crash. |
| **Fail loud** | A plugin that requests a nonexistent hook, references an unregistered device, or uses an attribute with no schema raises a typed `PluginError` — never a silent `0.0` or no-op. |
| **No netlist magic** | Every element a plugin injects — instance, connection, param override — must reference a type that was declared in PHDL source or marked `extern`. A plugin cannot invent a type that was never declared. |
| **Two ABIs, one SDK** | WASM (safe by default, sandboxed) and native shared library (power, explicit opt-in) share the same contract and hooks. |
| **The POM is the reflection contract** | The POM is public and stable; the codegen IR stays closed. Plugins reflect through the POM. |
| **No macro magic** | Registration is plain trait-method calls. Proc-macros are opt-in sugar, never required. |

---

## §3 Security model

### 3.1 Threats and countermeasures

| Threat | Countermeasure |
|--------|----------------|
| Malicious payload in a shared library or build script | WASM sandbox by default. Native requires explicit opt-in + TOFU approval + content hash in the lockfile. Pinned revision is mandatory for native. |
| Plugin reads sensitive files or writes outside the project | Capability-based filesystem: plugin may only access paths declared in the manifest, resolved relative to the project root. |
| Plugin exfiltrates over the network | `network = false` by default. WASM has no sockets. Native with `network = true` requires interactive approval on first load. |
| Plugin spawns a process | `process_spawn = false` by default. WASM never. Native needs the capability and logs every spawn. |
| Silent binary swap via a git push | `Piperine.lock` stores the content hash of the loaded artifact. A hash change forces re-approval. |
| Native plugin crashes the host | Loaded in-process (dlopen). For strong isolation, an out-of-process backend is offered (§6). |
| DoS via an infinite loop in a hook | WASM hooks have a per-invocation timeout (default 5 s). Native hooks have none — native is full trust. |

### 3.2 Trust on first use (TOFU)

On the first load of a plugin (or whenever the artifact hash changes), the
CLI blocks and presents the plugin's identity, source, requested
permissions, and artifact hash. The user approves or rejects. Approval is
persisted to `Piperine.lock` keyed by the content hash — the user is never
asked again while the hash is unchanged. Rejection aborts with
`PluginError::Untrusted`.

CI modes: `--trust <file>` reads decisions from a checked-in trust file;
`--no-trust` silently rejects native plugins (read-only sandbox mode for
suspicious CI).

### 3.3 Capability enforcement

The host exposes a facade to every plugin. All side-effecting calls
(filesystem, process spawn, network) go through this facade and are checked
against the manifest capabilities. In WASM this is enforced by the sandbox
(host imports); in native, the SDK offers the same entry points and the
publisher is expected to call them rather than the OS directly (documented
contract; best-effort). This is **not** a cryptographic sandbox for native
— in-process sandboxing is impossible. It is **audit + opt-in +
reproducibility**. Real isolation comes from the out-of-process backend
(§6).

---

## §4 Plugin manifest

The manifest (`piperine-plugin.toml`) lives at the root of the plugin
repository. It is intentionally minimal — just identity, artifact location,
and permissions. Device registrations, attribute schemas, and script
handlers are all declared in code at registration time, not duplicated in
the manifest.

```toml
[plugin]
name        = "avr-cosim"
abi         = "wasm"              # "wasm" | "native" | "process"
entry       = "avr_cosm.wasm"     # .wasm | .cdylib/.dll/.so | runner binary path

[permissions]
filesystem     = ["read *.cir", "write *.ppr"]
network        = false
process_spawn  = ["simavr", "avr-objcopy"]   # whitelist of executables
timeout_ms     = 5000              # per WASM hook invocation
```

| Field | Purpose |
|-------|---------|
| `name` | Plugin identity (used in `Piperine.toml`, lockfile, TOFU prompt) |
| `abi` | Backend: `wasm` (default, sandboxed), `native` (shared lib, TOFU required), `process` (out-of-process JSON-RPC) |
| `entry` | Path to the artifact relative to the plugin root |
| `permissions.filesystem` | Glob patterns the plugin may read/write, relative to project root |
| `permissions.network` | `false` by default; `true` requires TOFU approval |
| `permissions.process_spawn` | Whitelist of executables the plugin may spawn; empty/absent = none |
| `permissions.timeout_ms` | Per-hook-invocation timeout for WASM |

The manifest is parsed once at load time. Capability fields become a
permissions struct the host carries for the plugin's lifetime.

**Validation.** An invalid manifest (missing required fields, unknown ABI,
malformed permissions) is `PluginError::BadManifest` (P0006) at load time,
before any hook runs.

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
artifact is `PluginError::HashMismatch` (P0007) — the run aborts before
any code executes.

---

## §6 ABI tiers

Three backends share one contract. The contract defines a `Plugin` with a
`manifest()` accessor, a `register()` method for contributing devices,
schemas, tasks, and scripts, and seven lifecycle hooks (§8) that default
to no-op. Every contribution is optional — a plugin that registers nothing
but hooks is valid.

The registration surface offers four contribution types:

| Contribution | Method | What it provides |
|--------------|--------|------------------|
| Device | `device(type_id, factory)` | A `DeviceFactory` that constructs a solver `Device` for a given type ID |
| Attribute schema | `attr_schema(name, shape)` | Registers a schema name, validated against source `@name(...)` attributes |
| Bench task | `bench_task(task)` | A custom `SimTask` callable from bench fns via `$name(...)` |
| Script | `script(name, handler)` | A custom CLI subcommand |

### 6.1 WASM backend (default, sandboxed)

The default ABI. The plugin is a WASM module loaded via `wasmtime`. Types
cross the boundary as serialized snapshots of POM views — a plugin never
receives a raw pointer into the host's `Design`; it receives a serializable
view and returns a patch that the host validates and applies. This copy
per hook invocation is acceptable because hooks run a handful of times per
simulation, not in the Newton inner loop. Hard caps via wasmtime fuel +
per-invocation timeout.

### 6.2 Native backend (`abi = "native"`)

The plugin is a shared library (`.cdylib` / `.dll` / `.so`) loaded in-process.
The library exports a single C entry point returning a `Plugin` — the contract
is Piperine's own (§1); dlopen is only the loading mechanism. No real sandbox;
trust derives from TOFU + content hash + declared capabilities (audit, not
isolation). Useful for heavy external bridges (simavr, Verilator). The host
exports its math-function symbols to the loaded library.

### 6.3 Out-of-process backend (`abi = "process"`)

The plugin is an executable speaking JSON-RPC 2.0 over stdio. The host
spawns it as a child process. Real isolation: a plugin crash cannot take
down the host; the process can be containerized. Trade-off: per-call latency
in the millisecond range. Ideal for heavyweight bridges (QEMU, ModelSim).
Lowest priority tier.

---

## §7 Device loading

### 7.1 Binding flow

1. **Elaboration.** A module or instance carries
   `@device(plugin = "avr-cosim", type = "Arduino::UnoR3")`. The elaboration
   pass validates the attribute against the plugin-registered schema. An
   unknown schema or unregistered device type is an error.
2. **Pre-lowering.** The circuit compiler detects the `@device` attribute.
   Instead of compiling the module from PHDL source, it delegates to the
   plugin's device factory.
3. **Construction.** The host locates the factory registered for the type
   ID, calls it with the device spec (attributes, port bindings, params),
   and receives a `Device` that it injects into the circuit.
4. **Port mapping.** The plugin reads the port bindings (derived from
   `@port(name = …)` attributes) to know which net resolves to each logical
   port name.

The solver sees the plugin-provided device as **just another `Device`** —
the solver never learns it came from a plugin.

### 7.2 Declarative binding (`@device` / `@port`)

A module annotated with `@device(plugin = "name", type = "TypeId")` declares
that its behavior is provided by a plugin device, not by PHDL `analog`/
`digital` blocks. Ports annotated with `@port(name = "A0", kind = "analog")`
map logical port names to the plugin's boundary.

```phdl
@device(plugin = "avr-cosim", type = "Arduino::UnoR3")
mod ArduinoUno (
    @port(name = "A0", kind = "analog")  inout a0 : Electrical,
    @port(name = "A1", kind = "analog")  inout a1 : Electrical,
    @port(name = "D0", kind = "digital") inout d0 : Logic,
    @port(name = "D1", kind = "digital") inout d1 : Logic,
);
```

The module declares no `analog`/`digital` block and no body — its behavior is the
plugin device. The port declarations are ordinary PHDL ports (Part I §7.1); the
`@port` attribute only maps each port to the plugin's logical boundary name.

### 7.3 The device factory contract

A device factory is a callable that receives a device spec (type ID,
attributes, resolved port bindings, params) and returns a `Device`
implementing the solver's `AnalogDevice` or `DigitalDevice` trait. The
solver schedules it like any other device — the factory is the bridge
between the plugin world and the solver world.

The device spec carries:

| Field | Content |
|-------|---------|
| `type_id` | The type name from `@device(type = …)` |
| `attributes` | The validated `@device` and `@port` attribute data |
| `port_bindings` | Each port name → resolved `NetRef` (the net it connects to) |
| `params` | Instance params from `{ .name = value }` |

**Validation.** A `@device` attribute referencing a type not registered by
any loaded plugin is `PluginError::DeviceNotRegistered` (P0004). A port
listed in `@port` that does not match the device's declared boundary is an
error at circuit-build time.

---

## §8 Lifecycle hooks

### 8.1 Hook points

Seven hook points aligned with the compilation pipeline:

| # | Hook | When | Input | Mutable? | Use case |
|---|------|------|-------|----------|----------|
| 1 | `after_parse` | after parser, before elaboration | source file view | no | custom lint, metrics |
| 2 | `after_elaborate` | once `Design` is ready | design view | no | reporting, external validation |
| 3 | `transform_design` | before lowering | design staging | yes (via staging) | set params, inject parasitic instances |
| 4 | `before_lower` | just before body lowering | design view | no | final POM audit |
| 5 | `after_lower` | after body lowering | lowered bodies view | yes (rare; requires elevated capability) | inject stamps / parasitics directly |
| 6 | `before_solve` | after circuit compilation | circuit handle | no | instrument, log |
| 7 | `after_solve` | after an analysis | solve result | no | extract metrics, custom reports |

Bench hooks are delivered through plugin-registered `SimTask` entries,
dispatched by the bench interpreter's syscall path. This is the path for
`extract` / `.attach` / `.meta` (Part III §9).

### 8.2 Mutation through design staging

`Design` is immutable after elaboration by design — the only mutation
surface is the staging overrides (the same mechanism bench fns use to stage
param changes, Part III §9). Plugins respect that: the `transform_design`
hook receives a staging handle, not a mutable design pointer. The staging
handle offers:

- `set_param(path, value)` — stage a param override (same as bench `inst.r = …`).
- `add_instance(parent, type_name, spec)` — inject an instance of a declared
  type into a parent module.
- `add_connection(parent, connection)` — inject a net connection.
- `attributes_on(path)` — read attributes on a node.

Mutation through staging preserves the "staging → fork → applied" model.
A plugin never receives a mutable design.

### 8.3 Parasitics reference case

A plugin `rc-parasitics`:

1. At registration, declares the `@extract_rc` attribute schema.
2. In `after_elaborate`, reads `@extract_rc` on instances and computes R/C
   from geometric attributes — read-only; records an internal to-do list.
3. In `transform_design`, walks the to-do list and calls
   `add_instance(parent, "Resistor", …)` plus `add_connection(…)`. The
   `Resistor` type must be declared in the project's PHDL source (or marked
   `extern`) — a plugin cannot inject an instance of a type that was never
   declared (§2: no netlist magic). The result is an applied `Design`
   carrying the parasitics before lowering runs.

**Validation.** A plugin that calls `add_instance` with a type name not
present in the elaborated design (and not declared `extern`) is
`PluginError::HookFailed` (P0005) with a "type not declared" message. The
staging layer validates every injected instance against the design's module
table before applying it.

### 8.4 Ordering

Plugins run in **alphabetical order by name** within each hook
(deterministic, easy to reason about). Conflicts (two plugins mutating the
same path) are detected by the staging layer and surface as
`PluginError::StagingConflict` (P0008).

---

## §9 Custom scripts

A plugin may register custom CLI subcommands (Cargo-style). The CLI
dispatcher checks registered scripts before treating an unknown subcommand
as an error:

```
$ piperine spice rectifier.cir -o rectifier.ppr
```

Scripts receive a host context with explicit, capability-gated APIs:

| API | Capability required | Purpose |
|-----|---------------------|---------|
| `fs()` | `filesystem` | Read/write files restricted to manifest globs, relative to project root |
| `project()` | — (always available) | Access to `Piperine.toml`, the loaded `Design`, the target directory |
| `spawn(exe, args)` | `process_spawn` | Run a whitelisted executable; stdout/stderr captured and logged |
| `log(level, msg)` | — (always available) | Route to the host logger |

There is no `system()` and no `network()` unless the capability is granted.

`piperine plugin list` shows loaded plugins and available scripts;
`piperine help` merges builtin help with script help.

**Validation.** A CLI subcommand not registered by any loaded plugin is
`PluginError::UnknownScript` (P0009).

---

## §10 Attribute schema registration

Plugins register attribute schemas through the registration surface. This
closes the loop with Part I §8: a plugin that registers the schema name
`"device"` causes any `@device(...)` attribute in source to be validated
against the plugin's declared shape during the elaboration pass.

If two plugins register the same schema name, the host raises
`PluginError::SchemaConflict` (P0003) at load time. Plugins may namespace
their schemas to avoid collisions (e.g., `@avr-cosim:device`).

---

## §11 Error catalog

Plugin errors use the `P0xxx` code range, distinct from parse (`E1xxx`),
elaboration (`E2xxx`), and reflection (`E3xxx`).

| Code | Variant | Trigger |
|------|---------|---------|
| P0001 | `Untrusted` | TOFU pending — plugin not approved |
| P0002 | `UndeclaredCapability` | plugin used a capability not in its manifest |
| P0003 | `SchemaConflict` | two plugins registered the same schema name |
| P0004 | `DeviceNotRegistered` | `@device` references a type no plugin provides |
| P0005 | `HookFailed` | a hook returned an error (hook name, plugin, message) |
| P0006 | `BadManifest` | manifest is missing required fields or malformed |
| P0007 | `HashMismatch` | lockfile content hash does not match the loaded artifact |
| P0008 | `StagingConflict` | two plugins mutated the same staging path |
| P0009 | `UnknownScript` | CLI subcommand not registered by any plugin |
| P0099 | `Other` | catch-all |

---

## §12 Validation rules (consolidated)

| Section | Rule | Error |
|---------|------|-------|
| §4 | manifest missing required fields or malformed | P0006 `BadManifest` |
| §5.2 | lockfile hash does not match loaded artifact | P0007 `HashMismatch` |
| §3.2 | plugin not approved (TOFU pending) | P0001 `Untrusted` |
| §3.3 | plugin used a capability not declared in manifest | P0002 `UndeclaredCapability` |
| §7.3 | `@device` references unregistered type | P0004 `DeviceNotRegistered` |
| §7.3 | `@port` name does not match device boundary | P0005 `HookFailed` |
| §8.3 | `add_instance` with undeclared type | P0005 `HookFailed` ("type not declared") |
| §8.4 | two plugins mutate same staging path | P0008 `StagingConflict` |
| §9 | CLI subcommand not registered by any plugin | P0009 `UnknownScript` |
| §10 | two plugins register same schema name | P0003 `SchemaConflict` |
