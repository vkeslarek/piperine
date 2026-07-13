# Part VI — Plugins

The plugin extensibility model. Plugins extend Piperine with custom devices,
design-transform hooks, bench tasks, custom scripts, and attribute schemas —
without breaking "fail loud", without coupling the solver to codegen, and
without compromising security.

A plugin is a loadable artifact (native shared library, WASM module, or
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
- §6 ABI tiers and the wire protocol
- §7 Device loading
- §8 Lifecycle hooks
- §9 Bench tasks
- §10 Custom scripts
- §11 Attribute schema registration
- §12 Error catalog
- §13 Validation rules (consolidated)

---

## §1 Position

Plugins are the layer-4 extension mechanism (Part I §14). They open five
extension surfaces without breaking the core pipeline's invariants:

1. **Devices** — custom analog or digital devices loaded through the
   existing `Device` traits. The solver sees them as any other device.
2. **Hooks** — lifecycle points that read (and in one case, mutate) the
   POM at specific stages of the compilation pipeline.
3. **Bench tasks** — custom `$name(...)` system tasks callable from bench
   fns, extending the allowlist at load time.
4. **Scripts** — custom CLI subcommands (Cargo-style) for importers,
   exporters, and tooling.
5. **Attribute schemas** — plugin-registered schemas that validate
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
any external model ABI. OSDI compatibility is itself a plugin: the
`piperine-osdi` repository wraps compiled OSDI v0.4 models behind the native
traits; the solver core carries no OSDI or `libloading` dependency.

**One reflection surface, one model.** An in-process (native) plugin reflects
over the real POM — its hooks receive `&Design` directly. A WASM or process
guest cannot hold a pointer into host memory, so it receives the same
`Design` *serialized as itself*: the POM types carry serde derives (runtime
fields — spans, compiled ASTs, closures — are `#[serde(skip)]`, and skipped
runtime handles fail loud if serialized). There is no second structural
model and no conversion layer; `piperine_lang::pom::wire` adds only the
protocol envelopes (registration, hook input/output, actions, RPC framing)
around the POM.

---

## §2 Design principles

| Principle | Meaning |
|-----------|---------|
| **Security-first, capability-based** | A plugin declares permissions in its manifest; the host denies by default. Missing permission = no effect, not a crash. |
| **Fail loud** | A plugin that requests a nonexistent hook, references an unregistered device, or uses an attribute with no schema raises a typed `PluginError` — never a silent `0.0` or no-op. |
| **No netlist magic** | Every element a plugin injects — instance, connection, param override — must reference a type that was declared in PHDL source or marked `extern`. A plugin cannot invent a type that was never declared. |
| **One contract, three backends** | Native (in-process, full trust), WASM (sandboxed), and process (isolated) share the same contract and hooks. A plugin recompiled from native to WASM keeps its semantics. |
| **The POM is the reflection contract** | The POM is public and stable; the codegen IR stays closed. Plugins reflect through the POM — the real `Design` in-process, the same `Design` serialized (serde on the POM itself) out-of-host. |
| **No macro magic** | Registration is plain trait-method calls. A native plugin exports two C symbols; a WASM guest exports five thin functions. No proc-macros required anywhere. |

---

## §3 Security model

### 3.1 Threats and countermeasures

| Threat | Countermeasure |
|--------|----------------|
| Malicious payload in a shared library or build script | The host **never builds plugin sources** — it loads a prebuilt artifact whose bytes are hashed. WASM is sandboxed; native requires explicit opt-in + TOFU approval + content hash in the lockfile. |
| Plugin reads sensitive files or writes outside the project | Capability-based filesystem: a plugin may only access paths matching its manifest globs, resolved relative to the project root; absolute paths and `..` segments are denied (P0002). |
| Plugin exfiltrates over the network | `network = false` by default. WASM has no sockets. The host API exposes no network call at all — the manifest field exists only so the TOFU prompt can surface the request. |
| Plugin spawns a process | `process_spawn` is a whitelist, empty by default. (The spawn API itself is a follow-up; the field is declared and surfaced at TOFU.) |
| Silent binary swap via a git push | `Piperine.lock` stores the sha256 of the loaded artifact. A hash change is `P0007 HashMismatch` — the run aborts before any plugin code executes. |
| Native plugin crashes the host | Loaded in-process — documented full trust. For real isolation use the `process` backend: a guest crash is a loud load/call error, never a host crash. |
| DoS via an infinite loop in a hook | WASM guests run under a **fuel cap** derived from the manifest's `timeout_ms` (1e6 fuel per millisecond); a runaway guest traps loudly. Native and process tiers have no cap — their trust stories are opt-in and isolation respectively. |

### 3.2 Trust on first use (TOFU)

On the first load of a plugin (or whenever the artifact hash changes), the
CLI blocks and presents the plugin's identity, source, requested
permissions, and artifact hash. The user approves or rejects. Approval is
persisted to `Piperine.lock` keyed by the content hash — the user is never
asked again while the hash is unchanged. Rejection aborts with
`PluginError::Untrusted` (P0001).

Non-interactive runs (stdin is not a terminal) reject unknown plugins —
CI must opt in explicitly, never by hanging on a prompt. The
`PIPERINE_PLUGIN_TRUST` environment variable overrides the mode:
`accept` trusts and records everything (CI with vetted plugins);
`reject` refuses anything not already in the lockfile.

### 3.3 Capability enforcement

The host exposes a facade (`HostCtx`) to every plugin. Side-effecting calls
go through it and are checked against the manifest capabilities:

| API | Capability required | Behavior |
|-----|---------------------|----------|
| `fs_read(path)` | a `"read <glob>"` filesystem entry | Path resolves under the project root; no match → P0002 |
| `fs_write(path, text)` | a `"write <glob>"` filesystem entry | Same confinement; no match → P0002 |
| `project_root()` | — (always available) | The directory holding `Piperine.toml` |
| `log(msg)` | — (always available) | Routes to the host logger, tagged with the plugin name |

In WASM this is enforced by the sandbox; in native, the SDK offers the same
entry points and the publisher is expected to call them rather than the OS
directly (documented contract; best-effort). This is **not** a cryptographic
sandbox for native — in-process sandboxing is impossible. It is **audit +
opt-in + reproducibility**. Real isolation comes from the process backend.

---

## §4 Plugin manifest

The manifest (`piperine-plugin.toml`) lives at the root of the plugin
repository. It is intentionally minimal — just identity, artifact location,
and permissions. Device registrations, attribute schemas, bench tasks, and
script handlers are all declared in code at registration time, never
duplicated in the manifest.

```toml
[plugin]
name        = "spice"
abi         = "native"            # "native" | "wasm" | "process"
entry       = "plugin/target/debug/libpiperine_spice_plugin.so"
description = "ngspice-faithful device library"

[permissions]
filesystem = ["read *.cir", "read *.sp", "write *.phdl"]
```

| Field | Purpose |
|-------|---------|
| `name` | Plugin identity (used in `Piperine.toml`, lockfile, TOFU prompt) |
| `abi` | Backend: `native` (shared lib, TOFU required), `wasm` (sandboxed), `process` (out-of-process JSON-RPC) |
| `entry` | Path to the **prebuilt** artifact, relative to the plugin root |
| `permissions.filesystem` | `"read <glob>"` / `"write <glob>"` patterns, relative to the project root (`*` is the only wildcard) |
| `permissions.network` | `false` by default; surfaced at TOFU (no host API yet) |
| `permissions.process_spawn` | Whitelist of executables; empty = none (API is a follow-up) |
| `permissions.timeout_ms` | Per-call fuel budget for WASM guests (default 5000 → 5·10⁹ fuel) |

The manifest is parsed once at load time. Unknown fields are rejected —
a typo in a permission name must never silently grant nothing.

**Validation.** An invalid manifest (missing required fields, unknown ABI,
malformed or unknown permissions) is `PluginError::BadManifest` (P0006) at
load time, before any plugin code runs.

---

## §5 Discovery and resolution

### 5.1 Project configuration

A `[plugins]` section in `Piperine.toml`, separate from `[dependencies]`
(plugins are not PHDL libraries and have no transitive PHDL deps):

```toml
[plugins.spice]
path = "../piperine-spice"       # local path, relative to the project root

[plugins.osdi]
git = "https://github.com/acme/piperine-osdi"
rev = "abc1234"                  # pinned revision

[plugins.yosys]                  # a plugin inside a monorepo —
git    = "https://github.com/acme/plugins"
subdir = "piperine-yosys"        # its directory within the repository
```

Path sources are used in place; git sources sync into
`target/plugins/<name>/` through the same resolver PHDL dependencies use.
`subdir` points inside the checkout (the official-plugins monorepo case:
one repository, one directory per plugin, released together). It must be a
relative path with no `..`, and must exist in the checkout — anything else
fails loud. The same key works on `[dependencies]` git sources.

### 5.2 Artifacts are prebuilt

The host **never builds plugin sources** — running an arbitrary repo's build
script is exactly the payload §3.1 exists to block. The `entry` artifact
must already exist when the plugin loads; for a path dependency you build it
yourself (`cargo build` in the plugin repo), and the hash in the lockfile
pins what you approved. Distribution of prebuilt binaries (e.g. attached to
git releases, per target triple) is a planned follow-up.

### 5.3 Lockfile

`Piperine.lock` gains plugin entries with content hashes:

```toml
[[package]]
name         = "spice"
source       = "Path(PathDependency { path: \"../piperine-spice\" })"
hash         = "sha256:9f3a…b21c"
kind         = "plugin"
content_hash = "sha256:9f3a…b21c"
abi          = "native"
trusted_at   = "2026-07-10T12:00:00Z"
```

Any hash change forces re-approval (§3.2). Pre-plugin lockfiles parse
unchanged — the plugin fields are optional and the `kind` defaults to
`dependency`.

**Validation.** A plugin whose artifact hash does not match the trusted
hash is `PluginError::HashMismatch` (P0007) — the run aborts before any
code executes.

### 5.4 Inspection

`piperine plugin list` shows every loaded plugin with its ABI and
contribution counts (devices, schemas, bench tasks, scripts).

---

## §6 ABI tiers and the wire protocol

Three backends share one contract: a `Plugin` with a `manifest()` accessor,
a `register()` method for contributing devices, schemas, tasks, and scripts,
and the lifecycle hooks of §8. Every contribution is optional — a plugin
that registers nothing but hooks is valid. Whatever the backend, the host
presents the guest as an ordinary `Plugin`; nothing downstream can tell them
apart.

The registration surface offers five contribution types:

| Contribution | Method | What it provides |
|--------------|--------|------------------|
| Device | `device(type_id, factory)` | A `DeviceFactory` that constructs a solver `Device` for a given type ID (§7) |
| Attribute schema | `attr_schema(name, fields)` | A schema name validated against source `@name(...)` attributes (§11) |
| Bench task | `bench_task(name, task)` | A custom task callable from bench fns via `$name(...)` (§9) |
| Script | `script(name, handler)` | A custom CLI subcommand (§10) |
| Hooks | (trait methods, no registration) | Lifecycle observation and design transformation (§8) |

### 6.1 Native backend (`abi = "native"`)

The plugin is a shared library (`.so`/`.dll`/`.dylib`) loaded in-process via
dlopen — dlopen is only the loading mechanism; the contract is Piperine's
own (§1). The library exports two C symbols; the SDK provides both bodies:

```rust
use piperine_plugin::{entry, Manifest, Plugin, Registrar, ABI_VERSION};

pub struct MyPlugin { manifest: Manifest }

impl Plugin for MyPlugin {
    fn manifest(&self) -> &Manifest { &self.manifest }
    fn register(&self, r: &mut Registrar) {
        // devices, schemas, bench tasks, scripts — all optional
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn piperine_plugin_abi_version() -> u32 { ABI_VERSION }

#[unsafe(no_mangle)]
pub extern "C" fn piperine_plugin_entry() -> *mut core::ffi::c_void {
    entry(MyPlugin::new())
}
```

An ABI-version mismatch is a loud load error. Loaded libraries stay mapped
for the process lifetime (unloading a Rust cdylib is unsound). No sandbox;
trust derives from TOFU + content hash + declared capabilities (§3.3).
Native is the only tier that can contribute **devices** and **scripts**
today.

### 6.2 The wire protocol (WASM and process tiers)

Out-of-host guests speak the **POM itself**: the real `Design` and the real
`Value`, serialized through their own serde derives (runtime fields —
spans, compiled ASTs, closures — are skipped; skipped runtime handles fail
loud if serialized). `piperine_lang::pom::wire` adds only the protocol
around them — registration, hook envelopes, actions, RPC framing — and
re-exports `Design`/`Value` so a guest imports one module. A guest receives
the deserialized `Design` (same type, same accessors as in-process) and
returns `wire::Action`s — a patch the host validates and applies through
the ordinary staging surface (§8.2), under the same no-netlist-magic and
conflict rules as in-process plugins.

Both tiers implement one guest trait:

```rust
use piperine_lang::pom::wire::{Action, Design, Registration, Value, WirePlugin};

struct Parasitics;

impl WirePlugin for Parasitics {
    fn register(&self) -> Registration {
        Registration { bench_tasks: vec!["wgain".into()], ..Default::default() }
    }

    fn transform_design(&self, design: &Design) -> Result<Vec<Action>, String> {
        if design.module("Top").is_none() {
            return Err("expected a `Top` module".into());
        }
        Ok(vec![Action::AddInstance {
            parent: "Top".into(),
            label:  "r_par".into(),
            module: "Resistor".into(),          // must be declared in PHDL
            ports:  vec!["out".into(), "gnd".into()],
            params: vec![("r".into(), Value::Real(1e3))],
        }])
    }

    fn bench_task(&self, name: &str, _args: Vec<Value>) -> Result<Value, String> {
        match name {
            "wgain" => Ok(Value::Real(42.0)),
            other => Err(format!("unknown task `{other}`")),
        }
    }
}
```

Wire-tier guests may contribute schemas and bench tasks and observe every
hook; they may **not** contribute devices (a device sits in the Newton
inner loop — snapshot-per-call is unusable there) or scripts (scripts need
capability-gated fs the out-of-host tiers don't have yet). Declaring either
is a loud load error.

### 6.3 WASM backend (`abi = "wasm"`)

The guest is a `wasm32-unknown-unknown` cdylib exporting five thin
functions, each one line over the SDK (`piperine-plugin-wasm`, a re-export
of `pom::wire`):

```rust
use piperine_plugin_wasm as sdk;   // impl sdk::WirePlugin for MyGuest…

#[unsafe(no_mangle)]
pub extern "C" fn pp_abi_version() -> i32 { sdk::wasm_abi_version() }
#[unsafe(no_mangle)]
pub extern "C" fn pp_alloc(len: i32) -> i32 { sdk::wasm_alloc(len) }
#[unsafe(no_mangle)]
pub extern "C" fn pp_register() -> i64 { sdk::wasm_register(&MyGuest) }
#[unsafe(no_mangle)]
pub extern "C" fn pp_hook(ptr: i32, len: i32) -> i64 { sdk::wasm_hook(&MyGuest, ptr, len) }
#[unsafe(no_mangle)]
pub extern "C" fn pp_task(ptr: i32, len: i32) -> i64 { sdk::wasm_task(&MyGuest, ptr, len) }
```

Payloads cross guest linear memory as JSON; an `i64` return packs
`(ptr << 32) | len`. The guest has no host imports — it is pure: snapshots
in, patches out. Every call runs under the fuel cap (§3.1); an
infinite-loop guest traps with a message naming the cap.

### 6.4 Process backend (`abi = "process"`)

The guest is an executable speaking line-delimited JSON-RPC over stdio —
the same shapes as §6.2, framed as `{"id", "method", "params"}` requests
with methods `abi_version` / `register` / `hook` / `task`. The whole guest
main is one call:

```rust
fn main() {
    piperine_lang::pom::wire::serve_stdio(&Parasitics);
}
```

Real isolation: a guest crash or a guest that exits without speaking the
protocol is a loud host-side error, never a host crash; the process can be
containerized. Trade-off: per-call latency in the millisecond range, and no
per-call timeout (the tier's story is the crash boundary, not DoS
protection). Ideal for heavyweight bridges (QEMU, ModelSim).

---

## §7 Device loading

Plugin devices bind to the solver through the same device ABI as compiled PHDL
devices and external model wrappers. The normative solver-side device-loading
contract, including factory inputs, terminal bindings, and solver validation
rules, is Part VII §5.

The source-level syntax that requests such a device, including any attributes on
modules or ports, belongs to the language and elaboration specifications rather
than to the solver.

This Part owns plugin discovery, trust, registration, and contribution
collisions. Once a plugin contributes a device factory, the resulting device is a
solver object and follows Part VII.

---

## §8 Lifecycle hooks

### 8.1 Hook points

Five hook points aligned with the compilation pipeline. In-process plugins
receive the **real `Design`** (the full POM reflection surface); wire-tier
guests receive the same `Design` deserialized on their side — same type,
same accessors, no pointer.

| # | Hook | When | Input | Mutable? |
|---|------|------|-------|----------|
| 1 | `after_parse` | after parsing, before elaboration | raw source text | no |
| 2 | `after_elaborate` | once the `Design` is ready | `&Design` | no |
| 3 | `transform_design` | before each analysis consumes staged overrides | staging handle | yes (via staging) |
| 4 | `before_lower` | the applied design, just before body lowering | `&Design` | no |
| 5 | `after_solve` | after an analysis | analysis kind + (for `$op`) solved node voltages | no |

Two hook points from earlier drafts are deliberately not part of the
contract yet: `after_lower` (it would expose codegen-private shapes; it
stays closed until a real consumer exists) and `before_solve` (no consumer
has needed it).

Hooks fire in alphabetical plugin order (deterministic). The first hook
failure aborts the run as `P0005 HookFailed` naming the hook, the plugin,
and the message — a failed hook is never skipped. A read-only hook that
tries to return staging actions (wire tiers) fails loud.

### 8.2 Mutation through design staging

`Design` is immutable after elaboration — the only mutation surface is the
staging layer, the same mechanism bench fns use to stage param writes
(Part III §9). The `transform_design` hook receives a staging handle, never
`&mut Design`:

```rust
fn transform_design(&self, _cx: &mut HostCtx, staging: &DesignStaging) -> PluginResult<()> {
    // read: the full POM
    let design = staging.design();
    // write: staged, applied by the next pure re-elaboration
    staging.add_instance(
        "Top", "r_par", "Resistor",
        vec!["out".into(), "gnd".into()],
        vec![("r".into(), Value::Real(1e3))],
    )
}
```

The staging handle offers three verbs:

- `set_param(instance, param, value)` — same as a bench `inst.r = …` write.
- `add_instance(parent, label, module, ports, params)` — inject an instance
  of a **declared** type.
- `add_connection(parent, lhs, rhs)` — inject a net connection.

Staging is validated at write time: an undeclared module type is P0005 with
"type not declared" (no-netlist-magic, §2); the port count must match the
declared module. Because `transform_design` fires once per analysis,
re-staging an **identical** spec is idempotent; a *different* spec under the
same `(parent, label)` is a typed `P0008 StagingConflict` naming both
writers and the path (`Top.r_par`). Applied specs become ordinary POM nodes
and pass the same structural validation as source-declared instances
(E2013/E2014/E2020 fire on bad injections).

### 8.3 Parasitics reference case

The canonical gate (it runs, verbatim, as a test on all three backends):
a design whose `r1` dangles from `vin` to `out` with nothing after it. The
plugin's `transform_design` stages a declared `Resistor` from `out` to
`gnd` — and the bench observes a 2.5 V divider that only exists if the
injection happened:

```phdl
mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire out : Electrical;
    src : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
    r1  : Resistor (.p = vin, .n = out);
}
bench Top {
    fn divider() {
        var r = $op();
        $assert(r.v(out, gnd) > 2.49, "divider low");
        $assert(r.v(out, gnd) < 2.51, "divider high");
    }
}
```

---

## §9 Bench tasks

A plugin may contribute `$name(...)` system tasks callable from bench fns.
Registered names join the bench allowlist at load time — the elaboration
gate (Part III §5) accepts them exactly like builtins, and an unknown
`$name` is still a loud elaboration error when no plugin provides it.
Builtin task names (`op`, `tran`, `write`, …) cannot be shadowed — the
attempt is a P0003 collision at load.

```rust
struct GainTask;
impl PluginBenchTask for GainTask {
    fn run(&self, _args: Vec<Value>, _cx: &mut HostCtx) -> Result<Value, String> {
        Ok(Value::Real(42.0))
    }
}
// in register():  r.bench_task("gain", Box::new(GainTask));
```

```phdl
bench Top {
    fn uses_plugin_task() {
        $assert($gain() == 42.0, "plugin task value");
    }
}
```

This is the landing path for `extract` / `.attach` / `.meta` (Part III §9)
once their design pass happens.

---

## §10 Custom scripts

A plugin may register custom CLI subcommands (Cargo-style). The CLI
dispatcher checks registered scripts before treating an unknown subcommand
as an error:

```
$ piperine spice rectifier.cir -o rectifier.phdl
```

```rust
struct SpiceTranscribe;
impl ScriptHandler for SpiceTranscribe {
    fn invoke(&self, args: &[String], cx: &mut HostCtx) -> Result<i32, String> {
        let netlist = cx.fs_read(&args[0]).map_err(|e| e.to_string())?;   // capability-gated
        let phdl = transcribe(&netlist)?;                                  // plugin logic
        cx.fs_write(&output_of(args)?, &phdl).map_err(|e| e.to_string())?;
        Ok(0)
    }
}
// in register():  r.script("spice", Box::new(SpiceTranscribe));
```

Scripts receive the capability facade of §3.3 — filesystem access is
confined to the manifest globs under the project root; there is no
`system()` and no network API. Scripts are native-tier only for now
(§6.2); a wire-tier guest declaring one is a load-time error.

`piperine plugin list` shows loaded plugins and their scripts.

**Validation.** A CLI subcommand not registered by any loaded plugin is
`PluginError::UnknownScript` (P0009).

---

## §11 Attribute schema registration

Plugin-declared schemas join the **same registry** that backs
`@attribute(schema = "...")` bundles (Part I §8) — one metadata mechanism,
one validation path (E2022/E2023). A schema is a name plus typed fields:

```rust
// in register():
r.attr_schema("spice_model", vec![
    AttrField { name: "card".into(),   ty: "String".into(), required: true,  default: None },
    AttrField { name: "corner".into(), ty: "String".into(), required: false, default: None },
]);
```

```phdl
@spice_model(card = "1N4148", corner = "tt")
d1 : dio (.p = out, .n = gnd);
```

The `@device` and `@port` schemas are registered by the plugin *system*
itself whenever at least one plugin is loaded — they belong to no single
plugin, so two device plugins never collide on them. If two plugins
register the same schema name (or the same device type ID, bench task
name, or script name), the host raises `PluginError::SchemaConflict`
(P0003) at load time.

Schema seeding happens before elaboration: the host contributes its
schemas and task names to the elaboration registries, then the design
elaborates. Without the plugin loaded, `@device(...)` in source is an
ordinary unknown-schema error (E2022) — attributes never validate against
a plugin that isn't there.

---

## §12 Error catalog

Plugin errors use the `P0xxx` code range, distinct from parse (`E1xxx`),
elaboration (`E2xxx`), and reflection (`E3xxx`).

| Code | Variant | Trigger |
|------|---------|---------|
| P0001 | `Untrusted` | TOFU pending — plugin not approved |
| P0002 | `UndeclaredCapability` | plugin used a capability not in its manifest |
| P0003 | `SchemaConflict` | two plugins registered the same schema / device / task / script name (or shadowed a builtin task) |
| P0004 | `DeviceNotRegistered` | `@device` references a type no plugin provides |
| P0005 | `HookFailed` | a hook returned an error (hook name, plugin, message) — includes staging "type not declared" |
| P0006 | `BadManifest` | manifest is missing required fields or malformed |
| P0007 | `HashMismatch` | lockfile content hash does not match the loaded artifact |
| P0008 | `StagingConflict` | two writers staged different specs at one path — names both plugins and the path |
| P0009 | `UnknownScript` | CLI subcommand not registered by any plugin |
| P0099 | `Other` | catch-all (load failures, ABI mismatches, guest protocol errors) |

---

## §13 Validation rules (consolidated)

| Section | Rule | Error |
|---------|------|-------|
| §4 | manifest missing required fields, unknown ABI, or unknown permission | P0006 `BadManifest` |
| §5.3 | lockfile hash does not match loaded artifact | P0007 `HashMismatch` |
| §3.2 | plugin not approved (TOFU pending / non-interactive reject) | P0001 `Untrusted` |
| §3.3 | filesystem access outside manifest globs or project root | P0002 `UndeclaredCapability` |
| §6.1 | native ABI version mismatch, missing entry symbols | P0099 `Other` (load aborts) |
| §6.2 | wire-tier guest declares a device or script | P0099 `Other` (load aborts) |
| §6.3 | WASM guest exceeds its fuel cap | P0099 `Other` (call aborts, message names the cap) |
| §7 | registered plugin device type collides with an existing plugin contribution | P0003 `SchemaConflict` |
| §8.1 | a hook returns an error / a read-only hook returns actions | P0005 `HookFailed` |
| §8.2 | staged instance of an undeclared type, or port-count mismatch | P0005 `HookFailed` ("type not declared") |
| §8.2 | two writers stage different specs at one `(parent, label)` | P0008 `StagingConflict` |
| §9 | plugin bench task shadows a builtin name | P0003 `SchemaConflict` |
| §10 | CLI subcommand not registered by any plugin | P0009 `UnknownScript` |
| §11 | two plugins register the same schema name | P0003 `SchemaConflict` |
