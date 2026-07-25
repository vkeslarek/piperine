# Part VI — Plugins

The plugin extensibility model (plugin-interface v2). Plugins extend Piperine
with custom devices, design-transform hooks, and custom scripts — without
breaking "fail loud", without coupling the solver to codegen, and without
compromising security.

A plugin is a dependency that declares contributions. It is installed with
`piperine add` like any other dependency, declared under `[plugins]` in
`Piperine.toml`, resolved through the existing dependency resolver, and
admitted through two explicit trust gates: a **permissions consent** at `add`
time and a **trust-on-first-use (TOFU)** artifact-hash approval at load time.
The POM is the reflection contract; the codegen-private IR is never exposed.

## Contents

- §1 Position
- §2 Design principles
- §3 Security model
- §4 Plugin manifest and shapes
- §5 Discovery and resolution
- §6 Declaration-coupled contributions
- §7 Device loading
- §8 Lifecycle hooks
- §9 Custom scripts
- §10 Device binary distribution
- §11 Error catalog
- §12 Validation rules (consolidated)

---

## §1 Position

Plugins are the layer-4 extension mechanism (Part I §14). They open three
extension surfaces without breaking the core pipeline's invariants:

1. **Devices** — custom analog or digital devices loaded through the
   existing device traits. The solver sees them as any other device.
2. **Hooks** — five frozen lifecycle points that read (and in one case,
   stage mutations to) the POM at specific stages of the pipeline.
3. **Scripts** — custom CLI subcommands (Cargo-style) for importers,
   exporters, and tooling.

**Three shapes, one umbrella.** A plugin contributes one or more of:

| Shape | Declared by | What loads |
|-------|-------------|------------|
| Pure-PHDL | `[plugin]` with no `python`/`device` key | A code library — its `pub` items resolve via `use`; nothing runs |
| Scripted | `python = "plugin.py"` | Python scripts/hooks in the embedded-CPython host |
| Device | `device = { … }` | A compiled binary through the native backend, bound to the plugin's `@device` mods |

A plugin may carry both `device` and `python` — the binary is pure device
ABI, the Python adds scripts/hooks.

**Two backends, no more.** Plugins are **native (dlopen) + embedded Python
only** (MD-21). The WASM and process backends are removed; a manifest still
declaring `abi = "wasm"` or `abi = "process"` fails with
`PluginError::RemovedBackend` (P0011), never a generic unknown-value error.

**What stays closed.** Plugins cannot extend the parser grammar, the IR
types, or the solver's math core. They reflect through the POM and
contribute behavior; they do not modify the compiler's internals. The
codegen-private IR is never exposed to plugins — the POM is the only
reflection surface.

**What stays independent.** `piperine-solver` still does not depend on
`piperine-codegen`. Plugins talk to the solver only through the existing
device traits.

**The ABI is Piperine's own.** The plugin device contract is the native
`AnalogDevice` / `DigitalDevice` trait pair — designed for mixed-signal
simulation and Piperine semantics. It is **not** OSDI and does not track any
external model ABI. OSDI compatibility is itself a plugin: the
`piperine-osdi` repository wraps compiled OSDI v0.4 models behind the native
traits; the solver core carries no OSDI or `libloading` dependency.

**One reflection surface, one model.** Every plugin reflects over the real
POM: native hooks receive `&Design` directly (Rust trait objects,
same-compiler); Python hooks run in the embedded-CPython host over the same
POM with name-identical decorators (MD-22 name parity — the mechanism
differs, the names do not). There is no second structural model, no
serialization tier, and no imperative registration API.

---

## §2 Design principles

| Principle | Meaning |
|-----------|---------|
| **Security-first, capability-based** | A plugin declares permissions in its manifest; the host denies by default. |
| **Fail loud** | A plugin that requests a nonexistent hook phase, references an unregistered device, or uses an undeclared capability raises a typed `PluginError` — never a silent `0.0` or no-op. |
| **No netlist magic** | Every element a plugin injects — instance, connection, param override — must reference a type that was declared in PHDL source. A plugin cannot invent a type that was never declared. |
| **Declaration = injection** | Every contribution is declared at its point of definition: a device is a `#[pip::device]` type plus a `@device pub mod` in the plugin's own PHDL; a script or hook is one decorator. There is no imperative `Registrar` and no per-plugin `extern.phdl` stub. |
| **No plugin schemas** | Plugins declare no `extern` and no attribute schemas. The only plugin-facing schemas are the stdlib `@device`/`@port`, seeded by the plugin *system* itself. |
| **The POM is the reflection contract** | The POM is public and stable; the codegen IR stays closed. Plugins reflect through the POM — the real `Design`, in both hosts. |
| **Literal Rust/Python parity** | `#[pip::script]`/`#[pip::hook(phase)]`/`#[pip::device]` (Rust) and `@pip.script`/`@pip.hook.<phase>`/`@pip.device` (Python) — same names, same hook catalog, same `ctx` surface (MD-22). A parity test locks the names. |

---

## §3 Security model

### 3.1 Threats and countermeasures

| Threat | Countermeasure |
|--------|----------------|
| Malicious payload in a shared library | The host **never builds plugin sources** — it loads a prebuilt artifact whose bytes are hashed; native loading requires explicit TOFU approval + content hash in the lockfile. |
| Plugin reads sensitive files or writes outside the project | Capability-based filesystem: a plugin may only access paths matching its manifest globs, resolved relative to the project root; absolute paths and `..` segments are denied (P0002). |
| Plugin exfiltrates over the network | `network = false` by default. The host API exposes no network call at all — the manifest field exists so the consent gate can surface the request. |
| Plugin spawns a process | `process_spawn` is a whitelist, empty by default. (The spawn API itself is a follow-up; the field is declared and surfaced at consent/TOFU.) |
| Silent binary swap via a git push or a mutable release tag | `Piperine.lock` stores the sha256 of the approved artifact. A path-sourced artifact whose hash changes is `P0007 HashMismatch`; a release asset whose hash changes re-prompts (§10.3). A manifest `verify` hash mismatch is a hard fail (P0013) before any plugin code executes. |
| Native plugin crashes the host | Loaded in-process — documented full trust, gated by the two explicit consents of §3.2. |
| Unvetted permissions granted silently | `piperine add` prints the declared `[permissions]` and requires an explicit accept/deny (§3.2) — there is no silent-accept default. |

### 3.2 The two trust gates

Trust is **two explicit consents**, independent of each other (D11):

1. **Permissions consent — at `piperine add`.** Adding a dependency whose
   `piperine-plugin.toml` declares `[permissions]` prints them and requires
   an explicit accept/deny. A deny aborts the install: `Piperine.toml` and
   `Piperine.lock` are restored, nothing is installed
   (`PluginError::PermissionsDenied`, P0014). A manifest declaring no
   permissions needs no consent.
2. **Artifact TOFU — at load.** On the first load of a plugin (or whenever
   the artifact hash changes), the CLI blocks and presents the plugin's
   identity, source, requested permissions, and artifact hash. Approval is
   persisted to `Piperine.lock` keyed by the content hash. Rejection aborts
   with `PluginError::Untrusted` (P0001). A device manifest carrying
   `verify = "sha256:…"` skips this prompt entirely — the hash is checked
   up front and a mismatch is a hard fail (§10.3).

Both gates honor the `PIPERINE_PLUGIN_TRUST` environment variable:
`accept` grants/recording everything (CI with vetted plugins); `reject`
refuses anything not already consented/pinned; unset is interactive, and a
non-terminal stdin **denies** — CI must opt in explicitly, never by hanging
on a prompt.

### 3.3 Capability enforcement

The host exposes a facade (`HostCtx`) to every plugin. Side-effecting calls
go through it and are checked against the manifest capabilities:

| API | Capability required | Behavior |
|-----|---------------------|----------|
| `fs_read(path)` | a `"read <glob>"` filesystem entry | Path resolves under the project root; no match → P0002 |
| `fs_write(path, text)` | a `"write <glob>"` filesystem entry | Same confinement; no match → P0002 |
| `project_root()` | — (always available) | The directory holding `Piperine.toml` |
| `log(msg)` | — (always available) | Routes to the host logger, tagged with the plugin name |

For native plugins the SDK offers these entry points and the publisher is
expected to call them rather than the OS directly (documented contract;
best-effort). This is **not** a cryptographic sandbox — in-process
sandboxing is impossible. It is **audit + opt-in + reproducibility**.

---

## §4 Plugin manifest and shapes

The manifest (`piperine-plugin.toml`) lives at the root of the plugin
repository. It is intentionally minimal — identity, contribution shape, and
permissions. Contributions themselves are declared in code (§6), never
duplicated in the manifest.

```toml
[plugin]
name        = "spice"
description = "ngspice-faithful device library"

# Shape keys (all optional; shape = which keys are present):
python = "plugin.py"                     # scripted shape
device = { path = "target/release/libspice_plugin.so" }        # device shape, local binary
# device = { release = "github:acme/spice@v1.2.0", verify = "sha256:ab…" }  # …or a fetched release (§10)

[permissions]
filesystem = ["read *.cir", "read *.sp", "write *.phdl"]
network    = false
```

| Field | Purpose |
|-------|---------|
| `name` | Plugin identity (used in `Piperine.toml`, lockfile, consent/TOFU prompts) |
| `description` | Free text |
| `python` | Python script/hook entry, relative to the plugin root — **scripted** shape |
| `device.path` | Prebuilt device binary, relative to the plugin root — **device** shape |
| `device.release` | `github:owner/repo@tag` release coordinate — **device** shape, fetched per §10 |
| `device.verify` | Optional `sha256:<hex>` — checked up front against the fetched asset (§10.3) |
| `permissions.filesystem` | `"read <glob>"` / `"write <glob>"` patterns, relative to the project root (`*` is the only wildcard) |
| `permissions.network` | `false` by default; surfaced at consent/TOFU (no host API) |
| `permissions.process_spawn` | Whitelist of executables; empty = none (API is a follow-up) |

**Shape inference.** The shape is *which keys are present*: `device` →
device, `python` → scripted, neither → pure-PHDL code library. There is **no
`abi` field** — the backend is inferred, never declared. Exactly one of
`device.path` / `device.release` may be set.

**Removed fields fail targeted.** A manifest carrying `abi = "wasm"` or
`abi = "process"` is `PluginError::RemovedBackend` (P0011) — backends are
native + Python only. Any other `abi` value, an `entry` key, or a
`bench_tasks` key is `PluginError::BadManifest` (P0006) with a message
naming the replacement. Unknown fields are rejected — a typo in a
permission name must never silently grant nothing.

**Validation.** An invalid manifest (missing `name`, both device sources,
malformed permissions) is `PluginError::BadManifest` (P0006) at load time,
before any plugin code runs.

---

## §5 Discovery and resolution

### 5.1 Install: `piperine add`

A plugin is installed like any other dependency (D9 — a plugin *is* a
contributing dependency). The source argument is resolved **Go-style**:

```
$ piperine add acme/bjt-models        # bare owner/repo → https://github.com/acme/bjt-models
$ piperine add https://github.com/acme/bjt-models   # full git URL, verbatim
$ piperine add git@github.com:acme/bjt-models.git   # scp-style, verbatim
$ piperine add spice --git https://github.com/acme/piperine-spice --rev abc1234
$ piperine add spice --path ../piperine-spice
```

With no `--git`/`--path` flag the positional argument is the source, and the
package name derives from the URL's last segment. A bare source that is not
exactly `owner/repo` fails loud. A dependency whose `piperine-plugin.toml`
declares `[permissions]` triggers the §3.2 permissions consent before
anything is written.

### 5.2 Project configuration

Loadable plugins are declared in a `[plugins]` section of `Piperine.toml`,
separate from `[dependencies]` (plugins are loadable artifacts, not PHDL
libraries, and have no transitive PHDL deps):

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
`subdir` points inside the checkout; it must be a relative path with no
`..`, and must exist in the checkout — anything else fails loud.

### 5.3 Artifacts are prebuilt

The host **never builds plugin sources** — running an arbitrary repo's build
script is exactly the payload §3.1 exists to block. A `device.path` artifact
must already exist when the plugin loads; a `device.release` artifact is
fetched prebuilt per §10. There is no build-from-source fallback: an
unsupported target triple is a loud error (P0012), never a compile.

### 5.4 Lockfile

`Piperine.lock` carries plugin entries with content hashes:

```toml
[[package]]
name         = "spice"
source       = "github:acme/spice@v1.2.0"
hash         = "sha256:9f3a…b21c"
kind         = "plugin"
content_hash = "sha256:9f3a…b21c"
abi          = "x86_64-unknown-linux-gnu"   # the device binary's target triple
trusted_at   = "2026-07-10T12:00:00Z"
```

Pre-plugin lockfiles parse unchanged — the plugin fields are optional and
`kind` defaults to `dependency`.

**Validation.** A path-sourced plugin whose artifact hash does not match
the trusted hash is `PluginError::HashMismatch` (P0007) — the run aborts
before any code executes. (A release-sourced hash change re-prompts instead
— §10.3.)

### 5.5 Inspection

`piperine plugin list` shows every loaded plugin with its inferred shape and
contribution counts (devices, scripts).

---

## §6 Declaration-coupled contributions

**Declaration and injection are one act.** A contribution exists because its
declaration exists — there is no imperative `register()` body, no
`Registrar`, and no per-plugin `extern.phdl` stub. Two front-ends populate
one contribution table with **name-identical** decorators (MD-22):

| Contribution | Rust | Python |
|--------------|------|--------|
| Device | `#[pip::device("Type")]` on an `Element` type | `@pip.device("Type")` (Python-glue binding marker) |
| Script | `#[pip::script("name")]` | `@pip.script("name")` |
| Hook | `#[pip::hook(phase)]` | `@pip.hook.<phase>` |

```rust
#[pip::script("lint")]
fn lint(args: &[String], ctx: &Ctx) -> Result<i32, String> { /* … */ }

#[pip::hook(after_elaborate)]
fn check(ctx: &Ctx) -> Result<(), String> { /* ctx.design() -> &Design */ }
```

```python
import piperine as pip

@pip.script("lint")
def lint(args, ctx): ...

@pip.hook.after_elaborate
def check(ctx): ...
```

The Rust attributes register into an inventory-style collector the host
drains at load; the Python decorators register into a per-load table the
embedded host reads back after exec-ing the plugin's `python` entry. The
hook catalog is exactly the five phases of §8.1 — an unknown phase name
fails to compile (Rust) or fails at decoration time (Python). A cross-host
parity test locks the decorator, phase, and `ctx`/`staging` method names: a
name added on one side without the other fails the suite.

**No plugin schemas.** Plugins declare no `extern` and no attribute schemas
(D2). The `@device`/`@port` schemas are registered by the plugin *system*
itself whenever at least one plugin is loaded — they belong to no single
plugin, so two device plugins never collide on them. A plugin shipping an
`extern.phdl` gets no special treatment: there is no stub loader.

**Collisions.** Two plugins declaring the same device type ID or script
name fail at load with `PluginError::SchemaConflict` (P0003), naming both
plugins.

---

## §7 Device loading

Plugin devices bind to the solver through the same device ABI as compiled
PHDL devices and external model wrappers. The normative solver-side
device-loading contract, including factory inputs, terminal bindings, and
solver validation rules, is Part VII §5.

**The ABI is Rust, same-compiler (D13).** A device binary is a prebuilt
shared library (`.so`/`.dll`/`.dylib`) loaded via dlopen, exporting two C
symbols the SDK provides:

```rust
#[unsafe(no_mangle)]
pub extern "C" fn piperine_plugin_abi_version() -> u32 { piperine_plugin::ABI_VERSION }

#[unsafe(no_mangle)]
pub extern "C" fn piperine_plugin_entry() -> *mut core::ffi::c_void {
    piperine_plugin::entry(MyPlugin::new())
}
```

An exported `piperine_plugin_abi_version` differing from the host's
`ABI_VERSION` is a loud load error — this is the **only** version guard in
v2 (a manifest-level compatibility field is a deferred follow-up). Loaded
libraries stay mapped for the process lifetime.

**The `@device` mod lives in the plugin's own PHDL (D10).** The plugin
ships `@device pub mod …` declarations in its `.phdl`; importing the plugin
injects every declared device, each mod's `type` resolved against the
binary's `#[pip::device]` table. The user `use`s the plugin package and
instantiates the mod — a user design **never** writes `@device`:

```phdl
// in the plugin's own source:
@device(plugin = "spice", type = "Spice::Diode")
pub mod Diode(inout p: Electrical, inout n: Electrical) {
    param is: Real = 1e-14;
}
```

```phdl
// in the user's design — no @device here:
use spice::Diode;
mod Top() { /* … */ d1 : Diode (.p = out, .n = gnd); }
```

A `@device` type ID no loaded plugin provides is
`PluginError::DeviceNotRegistered` (P0004). Without any plugin loaded,
`@device(...)` in source is an ordinary unknown-schema error (E2022).

This Part owns plugin discovery, trust, declaration coupling, and
contribution collisions. Once a plugin contributes a device factory, the
resulting device is a solver object and follows Part VII.

---

## §8 Lifecycle hooks

### 8.1 Hook points

Five hook points — **frozen** (D8; new hooks only with a real consumer) —
aligned with the compilation pipeline. Every plugin sees the **real
`Design`** (Rust: `&Design` directly; Python: the same POM in the embedded
host).

| # | Hook | When | Input | Mutable? |
|---|------|------|-------|----------|
| 1 | `after_parse` | after parsing, before elaboration | raw source text | no |
| 2 | `after_elaborate` | once the `Design` is ready | `&Design` | no |
| 3 | `transform_design` | before each analysis consumes staged overrides | staging handle | yes (via staging) |
| 4 | `before_lower` | the applied design, just before body lowering | `&Design` | no |
| 5 | `after_solve` | after an analysis | analysis kind + (for `$op`) solved node voltages | no |

Hooks fire in alphabetical plugin order (deterministic). The first hook
failure aborts the run as `P0005 HookFailed` naming the hook, the plugin,
and the message — a failed hook is never skipped.

### 8.2 Mutation through design staging

`Design` is immutable after elaboration — the only mutation surface is the
staging layer, the same mechanism a host's `set` uses to stage param writes
(Part VIII §3). The `transform_design` hook receives a staging handle,
never `&mut Design`:

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

- `set_param(instance, param, value)` — same as a host `set` write.
- `add_instance(parent, label, module, ports, params)` — inject an instance
  of a **declared** type — including a `@device` mod, which then solves
  through the §7 device path like any authored instance.
- `add_connection(parent, lhs, rhs)` — inject a net connection.

Staging is validated at write time: an undeclared module type is P0005 with
"type not declared" (no-netlist-magic, §2); a non-existent parent module
fails the same way; the port count must match the declared module. Because
`transform_design` fires once per analysis, re-staging an **identical**
spec is idempotent; a *different* spec under the same `(parent, label)` is
a typed `P0008 StagingConflict` naming both writers and the path
(`Top.r_par`). Applied specs become ordinary POM nodes and pass the same
structural validation as source-declared instances — an injected label that
collides with an **authored** instance is a loud staging conflict, never an
overwrite: authored structure is never rewritten by a plugin (MD-25).

### 8.3 Parasitics reference case

The canonical gate: a design whose `r1` dangles from `vin` to `out` with
nothing after it. The plugin's `transform_design` stages a declared
`Resistor` from `out` to `gnd` — and the host observes a 2.5 V divider that
only exists if the injection happened:

```phdl
mod Top() {
    wire gnd : Electrical;
    wire vin : Electrical;
    wire out : Electrical;
    src : VoltageSource (.p = vin, .n = gnd) { .voltage = 5.0 };
    r1  : Resistor (.p = vin, .n = out);
}
```

```python
op = session.run_op(&SolverConfig::default(), None)?;   # host API (Part VIII §7)
assert 2.49 < op.v("out") < 2.51                        # divider at 2.5 V
```

---

## §9 Custom scripts

A plugin may declare custom CLI subcommands (Cargo-style) with
`#[pip::script("name")]` / `@pip.script("name")` (§6). The CLI dispatcher
checks registered scripts before treating an unknown subcommand as an
error:

```
$ piperine spice rectifier.cir -o rectifier.phdl
```

```rust
#[pip::script("spice")]
fn spice(args: &[String], ctx: &Ctx) -> Result<i32, String> {
    let netlist = ctx.fs_read(&args[0]).map_err(|e| e.to_string())?;   // capability-gated
    let phdl = transcribe(&netlist)?;                                  // plugin logic
    ctx.fs_write("rectifier.phdl", &phdl).map_err(|e| e.to_string())?;
    Ok(0)
}
```

Scripts receive the capability facade of §3.3 — filesystem access is
confined to the manifest globs under the project root; there is no
`system()` and no network API. `piperine plugin list` shows loaded plugins
and their scripts.

**Validation.** A CLI subcommand not registered by any loaded plugin is
`PluginError::UnknownScript` (P0009).

---

## §10 Device binary distribution

### 10.1 Release coordinates

A device plugin may ship its binary as a GitHub release asset instead of a
committed path (D4):

```toml
[plugin]
name   = "bjt"
device = { release = "github:acme/bjt-models@v1.2.0", verify = "sha256:ab…" }
```

The coordinate is exactly `github:owner/repo@tag` — v1 targets the
`github:` scheme only; anything else is a loud `BadManifest`-class error.
`verify` is optional (D7): TOFU is the floor, `verify` pins the hash up
front for supply-chain rigor.

### 10.2 Triple-matched assets

Release assets follow one naming convention, matched **case-sensitively**:

```
lib<pkg>-<target-triple>.<ext>     # .so (linux), .dll (windows), .dylib (macOS)
```

`<pkg>` is the repository name; `<target-triple>` is the host's build triple
(`x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, …). The loader lists
the release's assets and selects the one matching the host triple. **No
match is a loud error** naming the triple and the release —
`PluginError::NoAssetForTriple` (P0012). There is no silent skip and no
build-from-source (D6).

The selected asset downloads to a per-user, **content-addressed** cache:
`$PIPERINE_PLUGIN_CACHE` if set, else
`$XDG_CACHE_HOME/piperine/plugins` or `~/.cache/piperine/plugins`, as
`<content-hash>.<ext>`.

### 10.3 Verify, TOFU, and the pin

After the fetch, the asset's `sha256:<hex>` content hash is computed and
one of three paths runs:

1. **`verify` set, hash matches** — the manifest's explicit consent: the
   hash is pinned with **no TOFU prompt**.
2. **`verify` set, hash differs** — a hard fail
   (`PluginError::VerifyMismatch`, P0013). No prompt, no pin, no load.
3. **No `verify`** — TOFU (§3.2): prompt (interactive) / accept
   (`PIPERINE_PLUGIN_TRUST=accept`) / reject (`reject`), then pin.

The approved pin records **`(release-url, triple, content-hash)`** in
`Piperine.lock` as a `kind = "plugin"` entry (the triple in `abi`, the
coordinate in `source` — §5.4). A **changed asset** under a mutable tag —
the fetched hash differs from the pinned one — re-prompts per the trust
mode and re-pins on accept; a reject aborts as P0001 and leaves the old
pin. This is the content-hash tripwire for mutable release tags.

### 10.4 Reproducibility and offline use

A locked-and-approved entry **short-circuits**: the pin names the exact
cached file, so a cached + pinned binary loads **without any network**
(offline-after-first-fetch) and without a prompt. A second machine with the
same `Piperine.lock` but an empty cache fetches the release again; identical
bytes produce the identical hash, which matches the pin — the load proceeds
with no prompt, reproducible from the lockfile alone. A cache file whose
bytes no longer match its name is corruption and fails loud.

---

## §11 Error catalog

Plugin errors use the `P0xxx` code range, distinct from parse (`E1xxx`),
elaboration (`E2xxx`), and reflection (`E3xxx`).

| Code | Variant | Trigger |
|------|---------|---------|
| P0001 | `Untrusted` | TOFU pending — plugin not approved (or a changed release asset rejected at re-prompt) |
| P0002 | `UndeclaredCapability` | plugin used a capability not in its manifest |
| P0003 | `SchemaConflict` | two plugins declared the same device type ID or script name |
| P0004 | `DeviceNotRegistered` | `@device` references a type no plugin provides |
| P0005 | `HookFailed` | a hook returned an error (hook name, plugin, message) — includes staging "type not declared" and bad-parent injections |
| P0006 | `BadManifest` | manifest is missing required fields, malformed, or carries removed fields (`entry`, unknown `abi`, `bench_tasks`) |
| P0007 | `HashMismatch` | lockfile content hash does not match a path-sourced artifact |
| P0008 | `StagingConflict` | two writers staged different specs at one path — names both plugins and the path |
| P0009 | `UnknownScript` | CLI subcommand not registered by any plugin |
| P0011 | `RemovedBackend` | manifest declares `abi = "wasm"` or `abi = "process"` — backends are native + Python only |
| P0012 | `NoAssetForTriple` | no release asset matches the host target triple — names triple + release |
| P0013 | `VerifyMismatch` | fetched asset hash does not match the manifest's `verify` hash — hard fail, no prompt |
| P0014 | `PermissionsDenied` | declared permissions denied at `piperine add` — install aborted |
| P0099 | `Other` | catch-all (load failures, ABI mismatches, fetch/IO failures) |

---

## §12 Validation rules (consolidated)

| Section | Rule | Error |
|---------|------|-------|
| §4 | manifest missing required fields / unknown fields / both device sources | P0006 `BadManifest` |
| §4 | manifest declares `abi = "wasm"` / `abi = "process"` | P0011 `RemovedBackend` |
| §3.2 | permissions denied at `add` | P0014 `PermissionsDenied` |
| §3.2 | plugin not approved (TOFU pending / non-interactive reject) | P0001 `Untrusted` |
| §3.3 | filesystem access outside manifest globs or project root | P0002 `UndeclaredCapability` |
| §5.1 | bare source not exactly `owner/repo` | loud `add` error naming the expected form |
| §5.4 | lockfile hash does not match a path-sourced artifact | P0007 `HashMismatch` |
| §6 | two plugins declare the same device type / script name | P0003 `SchemaConflict` |
| §7 | device binary ABI version mismatch / missing entry symbols | P0099 `Other` (load aborts) |
| §7 | `@device` type ID no plugin provides | P0004 `DeviceNotRegistered` |
| §8.1 | a hook returns an error | P0005 `HookFailed` |
| §8.2 | staged instance of an undeclared type, bad parent, or port-count mismatch | P0005 `HookFailed` |
| §8.2 | two writers stage different specs at one `(parent, label)` | P0008 `StagingConflict` |
| §9 | CLI subcommand not registered by any plugin | P0009 `UnknownScript` |
| §10.2 | no release asset for the host triple | P0012 `NoAssetForTriple` |
| §10.3 | fetched asset fails the `verify` hash | P0013 `VerifyMismatch` |
| §10.3 | changed release asset rejected at re-prompt | P0001 `Untrusted` |

---

## Appendix A — Writing a plugin: one worked example per shape

Three complete plugins, smallest possible. Each is a repository with a
`Piperine.toml`, a `piperine-plugin.toml`, and its contribution sources;
each installs with `piperine add <git>` (§5.1) and is declared under
`[plugins]` in the user's `Piperine.toml` (§5.2).

### A.1 Pure-PHDL: a model library

No code runs — the plugin is a code library whose `pub` items resolve via
`use`.

```
bjt-models/
├── Piperine.toml
├── piperine-plugin.toml
└── src/
    └── lib.phdl
```

```toml
# piperine-plugin.toml — [plugin] alone ⇒ pure-PHDL shape (§4)
[plugin]
name        = "bjt-models"
description = "Gummel–Poon model library"
```

```phdl
// src/lib.phdl
pub mod GummelPoon(inout c: Electrical, inout b: Electrical, inout e: Electrical) {
    param is:  Real = 1e-15;
    param bf:  Real = 100.0;
    // …
}
```

```phdl
// the user's design
use bjt_models::GummelPoon;

mod Amp() {
    // …
    q1 : GummelPoon (.c = vcc, .b = inp, .e = gnd);
}
```

Nothing is registered, hashed, or prompted — importing resolves the `pub`
items and that is all.

### A.2 Scripted: a lint script + an elaboration hook

The `python` key makes it scripted; the decorators declare AND bind each
contribution in one place (§6). The Rust form is shown side by side — same
names, same phases, same `ctx` (MD-22).

```toml
# piperine-plugin.toml
[plugin]
name   = "lintpack"
python = "plugin.py"

[permissions]
filesystem = ["write *.phdl"]
```

<table>
<tr><th>Python (<code>plugin.py</code>)</th><th>Rust (native equivalent)</th></tr>
<tr><td>

```python
import piperine as pip

@pip.script("lint")
def lint(args, ctx):
    issues = run_lint(args)
    ctx.fs_write("lint-report.phdl", issues)
    return 0

@pip.hook.after_elaborate
def check(ctx):
    design = ctx.design()
    assert design.module("Top") is not None

@pip.hook.transform_design
def inject(ctx, staging):
    staging.add_instance(
        "Top", "r_par", "Resistor",
        ["out", "gnd"], [("r", 1e3)],
    )
```

</td><td>

```rust
#[pip::script("lint")]
fn lint(args: &[String], ctx: &Ctx)
    -> Result<i32, String>
{
    let issues = run_lint(args);
    ctx.fs_write("lint-report.phdl", &issues)
        .map_err(|e| e.to_string())?;
    Ok(0)
}

#[pip::hook(after_elaborate)]
fn check(ctx: &Ctx) -> Result<(), String> {
    let design = ctx.design();
    assert!(design.module("Top").is_some());
    Ok(())
}

#[pip::hook(transform_design)]
fn inject(ctx: &Ctx, staging: &DesignStaging)
    -> Result<(), String>
{
    staging.add_instance(
        "Top", "r_par", "Resistor",
        vec!["out".into(), "gnd".into()],
        vec![("r".into(), Value::Real(1e3))],
    )
    .map_err(|e| e.to_string())
}
```

</td></tr>
</table>

```
$ piperine add acme/lintpack
  Plugin 'lintpack' declares permissions:
    filesystem    : write *.phdl
  Grant these permissions? [y/N] y
$ piperine lint src/main.phdl      # dispatches to @pip.script("lint")
```

### A.3 Device: a compiled binary + `@device` in the plugin's own PHDL

Three pieces: the `Element` implementation with `#[pip::device]`, the
`@device pub mod` in the plugin's **own** PHDL (D10), and the manifest
pointing at a release (§10).

```rust
// src/lib.rs — the device binary (crate-type = ["cdylib"])
use piperine_plugin::{entry, Plugin, PluginDevice, PluginDeviceSpec, /* … */};

pub struct DiodePlugin { manifest: Manifest }
impl Plugin for DiodePlugin {
    fn manifest(&self) -> &Manifest { &self.manifest }
    // contributions come from the #[pip::device] declaration below
}

#[pip::device("Spice::Diode")]
struct Diode { label: String, a: AnalogReference, b: AnalogReference, is: f64 }

impl PluginDevice for Diode {
    const KIND: DeviceKind = DeviceKind::Analog;
    fn from_spec(spec: &PluginDeviceSpec) -> Result<Self, String> { /* ports + params */ }
}
impl AnalogDevice for Diode { /* load_dc / load_transient stamps */ }
impl DigitalDevice for Diode {}
impl Introspect for Diode {}
impl Element for Diode {
    fn name(&self) -> &str { &self.label }
    fn capabilities(&self) -> ElementCapabilities { ElementCapabilities::ANALOG }
}

#[unsafe(no_mangle)]
pub extern "C" fn piperine_plugin_abi_version() -> u32 { piperine_plugin::ABI_VERSION }
#[unsafe(no_mangle)]
pub extern "C" fn piperine_plugin_entry() -> *mut core::ffi::c_void {
    entry(DiodePlugin::new())
}
```

```phdl
// src/lib.phdl — the @device mod lives in the PLUGIN's PHDL; the user
// never writes @device (D10).
@device(plugin = "spice", type = "Spice::Diode")
pub mod Diode(inout p: Electrical, inout n: Electrical) {
    param is: Real = 1e-14;
}
```

```toml
# piperine-plugin.toml
[plugin]
name   = "spice"
device = { release = "github:acme/spice@v1.2.0", verify = "sha256:9f3a…" }
```

The release carries one asset per supported host, named by the §10.2
convention — CI builds and attaches them:

```
libspice-x86_64-unknown-linux-gnu.so
libspice-aarch64-apple-darwin.dylib
libspice-x86_64-pc-windows-msvc.dll
```

The user adds the plugin and instantiates the mod — no `@device` at the
user site:

```phdl
use spice::Diode;

mod Rectifier() {
    // …
    d1 : Diode (.p = out, .n = gnd) { .is = 1e-13 };
}
```

On first load the asset for the user's triple is fetched, hashed, and
TOFU-approved (or matched against `verify` with no prompt); the pin lands
in `Piperine.lock`, and every later run — including offline ones — loads
from the content-addressed cache (§10.4). A host triple with no matching
asset is `P0012 NoAssetForTriple`, loud at load.

### A.4 The decorator equivalence, consolidated

| Contribution | Rust | Python |
|--------------|------|--------|
| Script `lint` | `#[pip::script("lint")]` | `@pip.script("lint")` |
| Hook `after_parse` | `#[pip::hook(after_parse)]` | `@pip.hook.after_parse` |
| Hook `after_elaborate` | `#[pip::hook(after_elaborate)]` | `@pip.hook.after_elaborate` |
| Hook `transform_design` | `#[pip::hook(transform_design)]` | `@pip.hook.transform_design` |
| Hook `before_lower` | `#[pip::hook(before_lower)]` | `@pip.hook.before_lower` |
| Hook `after_solve` | `#[pip::hook(after_solve)]` | `@pip.hook.after_solve` |
| Device `Type` | `#[pip::device("Type")]` | `@pip.device("Type")` (glue marker) |

A name on one side without the other fails the cross-host parity test —
this table is the contract the test locks (§6).
