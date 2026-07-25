# Plugin Interface v2 — Ideal (north-star)

> P5 refinement. This is the **ideal-first** doc: the plugin interface as it
> *should* be, before diffing against what ships (`delta.md`, next) or
> writing requirements (`spec.md`, after). Governing decisions: **MD-21**
> (native + Python backends only; WASM/process removed), **MD-22** (Rust and
> Python are one API — same shape both sides), **MD-24** (declared language
> surface — every name resolves to a textual declaration; `extern` is a
> deliberately-special escape hatch).
>
> **Author's note on MD-21:** MD-21 as locked says the lifecycle registry is
> "exposed to Python so a plugin self-registers (attribute schemas, hooks,
> scripts, devices)." This doc **deliberately revises** that: no imperative
> self-registration of *attribute schemas*, and no plugin-authored `extern`
> at all (§4). The user is changing direction here — flagged for the
> Decisions log.

## Locked decisions (user, this session)

- **D1 — "plugin" is the umbrella for all 3 shapes** (pure-PHDL / scripted /
  device). A pure-PHDL package is still *a plugin* with a `[plugin]` marker,
  not silently folded into the plain dependency system — one uniform concept
  across doc/CLI.
- **D2 — total `extern`/schema ban for plugins.** No plugin declares
  `extern` or a new attribute schema. Only `@device`/`@port` (stdlib) +
  ordinary PHDL. Delete `load_extern_stub`/`MissingExternStub`/per-plugin
  `extern.phdl`.
- **D3 — literal Rust/Python decorator parity.** Real decorator API on both
  sides: `@pip.script`/`@pip.hook` (Python) *and* `#[pip::script]`/
  `#[pip::hook]` (Rust) — same names, same hook catalog, same `ctx`.
- **D4 — device binaries: GitHub-release + target-triple + TOFU.**
  `device = { release = "github:acme/x@v1", verify = "sha256" }`; loader
  picks the triple-matched asset, downloads, TOFU-hashes, pins in the
  lockfile.
- **D5 — a device is a *binary*, language-agnostic, no Python glue.** The
  contract is the **exported C ABI**: the shared library exports a `type`
  → `Element`-constructor table (plus the ABI-version symbol). `#[pip::device
  ("GummelPoon")]` is the *Rust ergonomic* that generates those exports — but
  any language that emits a `.so`/`.dll`/`.dylib` with the same exported
  symbols is a valid device binary (C, Zig, a hand-written ABI, OSDI-compat
  wrapper, …). A device-only plugin has no `.py`. Python enters only if the
  plugin *also* ships a script/hook.
- **D6 — unsupported target triple is a loud error.** No build-from-source
  in v1 — a device ships prebuilt binaries; a missing triple fails with a
  clear message naming the triple and release, never a silent skip.
- **D7 — `verify` hash is optional; TOFU is the floor.** No `verify`: first
  fetch prompts (TOFU) and pins the hash. With `verify`: the manifest pins
  the expected hash up front (no prompt, hard-fail on mismatch). Author
  chooses the rigor.
- **D8 — the five hooks are frozen.** `transform_design` (via staging) is
  the sole device-injection point. New hooks land only when a real consumer
  appears (same discipline that omitted `after_lower`).

---

## §0 — The driving scenarios (unchanged intent, simplified surface)

A plugin exists to do one or more of four things. Nothing else.

1. **Ship devices.** `piperine add github:acme/bjt-models` makes
   `@device`-annotated modules solvable — under the same trust-on-first-use
   (TOFU) policy already in place. A device carries a compiled binary
   (native `Element`).
2. **Ship referenceable PHDL code, zero Python.** A plugin can be *just
   `.phdl` files* — a pure code library (models, disciplines, bundles,
   fns). `use acme::opamps;` and it resolves, no Python, no binary, no
   manifest ceremony beyond naming the package.
3. **Ship cargo-style scripts.** `piperine <name> …` dispatches to a
   plugin-provided command (a `.py` entry), the way `cargo <subcommand>`
   works. Zero binary — pure Python.
4. **Hook the lifecycle.** Attach an action to a specific pipeline moment
   (after parse, after elaborate, before lower, after solve, …),
   **including injecting a device into the POM** at the right moment.

The three plugin *shapes* fall out of these: **pure-PHDL** (scenario 2),
**scripted** (scenarios 3–4, Python, no binary), **device** (scenario 1,
binary). A single plugin may combine them (a device plugin that also ships
PHDL model cards and a `bench`-style script).

---

## §1 — The three shapes, ideal form

### 1a. Pure-PHDL plugin (no Python, no binary)

The lowest-ceremony plugin: a git repo of `.phdl` files with a
`Piperine.toml` naming the package. `piperine add github:acme/opamps` and:

```phdl
use acme::opamps;                 // resolves to the added package's headers
x1 : opamps::TL072 ( .vp = a, .vn = b, .out = o ) { };
```

No manifest `[plugin]` section, no `entry`, no trust prompt (it is *source*,
reviewed like any dependency — same as a git `use`-dependency today). This
is the existing `piperine add <git>` dependency mechanism; a "pure-PHDL
plugin" is simply **a normal PHDL package** — the word "plugin" is almost a
misnomer here. **Ideal: this needs no plugin machinery at all.** It IS the
qualified-import / git-dependency path (just landed: `spice::passives::res`
full-path refs + `use` resolution).

### 1b. Scripted plugin (Python, no binary)

A plugin that contributes **scripts** and/or **lifecycle hooks**, written in
Python, no compiled artifact. Loaded through the embedded CPython host (same
isolation as `*_tb.py` testbenches). Example:

```python
# acme_lint/plugin.py
import piperine as pip

@pip.script("lint")                          # piperine lint <args>
def lint(args, ctx):
    design = ctx.design                      # the real POM (read-only)
    for m in design.modules:
        ...                                  # inspect, report
    return 0                                 # exit code

@pip.hook.after_elaborate                    # a lifecycle hook
def check(ctx):
    ...                                      # read-only inspection
```

The **same** capability in Rust is the **same shape**:

```rust
// acme_lint (native, but scriptless — Python parity)
#[pip::script("lint")]
fn lint(args: &[String], ctx: &Ctx) -> i32 { ... }

#[pip::hook(after_elaborate)]
fn check(ctx: &Ctx) -> Result<()> { ... }
```

(MD-22: identical names, identical hook catalog, identical `ctx` surface.)

### 1c. Device plugin (Python glue + a compiled binary)

Ships an `Element` implementation as a compiled native library, referenced
by `@device` in PHDL. **Declaration and injection are coupled** (§3): the
PHDL `mod` declares the shape, `@device(plugin, type)` binds it to the
plugin's factory — there is no imperative "register this device" call.

```phdl
@device(plugin = "acme_bjt", type = "GummelPoon")
mod BJT ( inout c: Electrical, inout b: Electrical, inout e: Electrical ) {
    param is  : Real = 1e-16;
    param bf  : Real = 100.0;
}
```

The binary is **fetched from a GitHub release** per target triple (§5), TOFU-
verified on first use, and cached. The device→constructor mapping lives
**inside the binary**, as an **exported C ABI table** (`type` → constructor)
— language-agnostic (D5). Rust authors get an ergonomic macro that generates
those exports:

```rust
// acme_bjt (the compiled device binary — Rust authoring)
#[pip::device("GummelPoon")]
pub struct GummelPoon { /* ... Element impl ... */ }
```

…but the *contract* is the exported symbol table, not Rust — a C, Zig, or
hand-written binary exporting the same symbols is equally valid. A device-
only plugin is **pure binary — no Python glue, no schema declaration, no
`extern`**. Python appears in a device plugin *only* if it also ships a
script or hook.

---

## §2 — What a plugin is (manifest)

`Piperine.toml`, one `[plugin]` section, deliberately tiny:

```toml
[project]
name = "acme_bjt"
version = "1.2.0"

[plugin]
# Which shapes this package contributes. Absent = pure-PHDL (shape 1a).
python = "plugin.py"          # scripted/hook entry (shape 1b/1c glue), optional
device = { release = "github:acme/bjt-models", verify = "sha256" }  # shape 1c, optional

[plugin.permissions]          # deny-by-default, unchanged intent
filesystem = ["read *.model"]
network = false
```

- No `abi = wasm|native|process` field — the *shape* is inferred from what's
  present (`python` key → scripted; `device` key → device binary; neither →
  pure-PHDL). WASM/process are gone (MD-21).
- No `entry` symbol-name plumbing exposed to the author (the host knows the
  native ABI symbol / the Python entry convention).

---

## §3 — Declaration + injection coupling (the big change)

**Old model (removed):** a plugin imperatively called
`Registrar::attr_schema(name, fields)` / `Registrar::device(type, factory)`
/ `Registrar::script(name, handler)` at load — a hidden registration the
PHDL author couldn't see, plus a published `extern.phdl` stub so the schema
name resolved.

**New model:** every *referenceable* thing a plugin contributes is
**declared in PHDL by the user** and merely *bound* to the plugin's
implementation:

| Contribution | Declared where | Bound how | Imperative registration? |
|---|---|---|---|
| Device | user's `mod` + `@device(plugin, type)` | plugin's factory keyed by `type` | **No** — the `@device` attribute IS the coupling |
| Script (`piperine <name>`) | plugin's `@pip.script("name")` decorator | that decorator | the decorator declares AND binds in one place — no separate stub |
| Lifecycle hook | plugin's `@pip.hook.<phase>` decorator | that decorator | same — one place |
| Attribute schema | **not contributable** — see §4 | — | — |

The unifying rule: **there is no place where a plugin registers a name that
has no visible declaration at the point of contribution.** A device's name
is a `mod` the user wrote. A script's name is right there in the decorator.
A hook has no name. Nothing is injected "behind" a stub.

`@device` and `@port` remain the *only* plugin-facing attribute schemas, and
they are **stdlib-declared** (`headers/device_port.phdl`), not plugin-
declared.

---

## §4 — No plugin `extern`, ever

A plugin **cannot** declare `extern` (MD-24's escape hatch). Consequences,
by design:

- **No plugin-authored attribute schemas.** The old `extern.phdl` stub
  mechanism (auto-imported per plugin) is deleted. `@device`/`@port` are the
  fixed plugin surface; a plugin that wants "extra metadata" uses ordinary
  PHDL (`param`, a `bundle`), not a bespoke `@schema`.
- **No plugin-authored `extern fn`/`operator`/`type`.** A plugin cannot add
  a new intrinsic/operator/primitive — those are stdlib-only, deliberately.
  A plugin ships *devices* (compiled `Element`s referenced by `@device`) and
  *PHDL code* (ordinary `pub mod`/`fn`/`bundle`), never new language
  primitives.
- **Rationale (user):** `extern` was built for the stdlib's own native
  surface, where every intrinsic is coverage-guarded
  (`extern_coverage_guard.rs`). Letting arbitrary plugins mint `extern`
  turns a rare, auditable escape hatch into a common tool — and destroys the
  "you can always see what's happening" property MD-24 exists to protect.

**Resolved (D2):** total ban. `@device`/`@port` are stdlib externs seeded
when a plugin loads (unchanged — stdlib-declared, not plugin-declared); every
other "extra metadata" need is served by ordinary PHDL (`param`, a `bundle`).
The per-plugin `extern.phdl` stub mechanism is deleted outright.

---

## §5 — Artifact distribution (the important open problem)

Two artifact realities:

- **Pure-PHDL / scripted plugins → NO binary.** Everything is source (`.phdl`
  + `.py`), fetched by `piperine add` exactly like a git dependency. No
  release-asset machinery, no target triples. Trust = source review (same as
  any dependency).
- **Device plugins → a compiled binary per target triple.** This is where
  the release-reference scheme is needed.

**Ideal for device binaries:**

```toml
[plugin]
device = { release = "github:acme/bjt-models@v1.2.0", verify = "sha256" }
```

- The loader resolves `release` → a GitHub release, picks the asset matching
  the host target triple (`libacme_bjt-x86_64-unknown-linux-gnu.so`, etc.),
  downloads it to a per-user cache.
- **TOFU on the fetched binary's content hash** (the existing `trust.rs`
  mechanism, keyed in `Piperine.lock`) — first fetch prompts, the approved
  hash is pinned; a changed asset re-prompts. `verify = "sha256"` optionally
  pins the expected hash up front (supply-chain: the manifest author commits
  to the exact bytes, so a swapped release asset fails without a prompt).
- **Reproducibility:** the lockfile records `(release-url, target-triple,
  content-hash)` so a second machine fetches the identical, already-approved
  binary.

**Resolved (D6/D7):** an unsupported target triple is a **loud error** naming
the triple + release (no build-from-source in v1). `verify` is **optional** —
TOFU is the floor; `verify` pins the expected hash up front for supply-chain
rigor. **Still to pin down in the spec:** the exact release-asset naming
convention (`lib<name>-<triple>.<ext>`?) and whether a committed-in-repo
binary is an accepted offline fallback (leaning: v1 = release-only, note the
fallback as a follow-up).

---

## §6 — Lifecycle hooks (Rust/Python parity)

One catalog, both sides, same names. Read-only hooks receive the real POM;
the one mutating hook goes through a staging surface (never free mutation).

| Hook | When | POM access | Can inject a device? |
|---|---|---|---|
| `after_parse` | raw source parsed | read-only source | no |
| `after_elaborate` | `Design` built | read-only `Design` | no |
| `transform_design` | before analysis consumes overrides | **staging (mutable)** | **yes** — the device-injection path |
| `before_lower` | applied design, pre-codegen | read-only `Design` | no |
| `after_solve` | analysis done | result view (voltages for `op`) | no |

Python:

```python
@pip.hook.transform_design
def inject(ctx, staging):
    staging.add_instance(parent="Top", label="rp", module="acme::Rparasitic", ...)
```

Rust — identical catalog, identical `staging` methods (MD-22).

**Resolved (D8):** the catalog is **frozen at these five**. `transform_design`
is the sole device-injection point (via staging). More hooks land only when a
real consumer appears.

---

## §7 — What is deleted

- `crates/piperine-plugin-wasm` (whole crate) and the WASM backend.
- The process JSON-RPC backend (`backend/process.rs`, `wire_hosted.rs`).
- `Registrar::attr_schema` / `Registrar::script` / `Registrar::device`
  imperative surface (replaced by decorators / `@device` coupling).
- Per-plugin `extern.phdl` stub loading (`load_extern_stub`,
  `MissingExternStub`) — plugins no longer declare schemas.
- `Manifest.abi` field and `entry` symbol plumbing (shape is inferred).

## §8 — What is kept / reused

- TOFU trust (`trust.rs`) + `Piperine.lock` content-hash pinning — extended
  to the release-fetched binary.
- Native dlopen device loading (the `Element` ABI) — the device binary path.
- The embedded CPython host (bench isolation) — the Python plugin runtime.
- `@device`/`@port` stdlib schemas (`headers/device_port.phdl`).
- `piperine add <git>` — becomes the universal install path (source for
  pure-PHDL/scripted, release-fetch for device binaries).

---

## Open questions rollup — status

| Q | Topic | Resolution |
|---|---|---|
| Q1 | plugin attribute schemas | **D2** — total ban, stub mechanism deleted |
| Q2 | device-binary distribution | **D4/D6/D7** — release+triple+TOFU, loud on missing triple, `verify` optional. *Spec must still pin the asset-naming convention.* |
| Q3 | hook catalog | **D8** — freeze the five; `transform_design` sole injection point |
| Q4 | "plugin" scope | **D1** — umbrella for all 3 shapes (pure-PHDL kept as a plugin) |
| Q5 | Rust/Python parity | **D3** — literal decorator parity both sides |
| Q6 | versioning/ABI compat | **still open** — how a plugin declares its target Piperine version now that `abi` is gone. Leaning: a `piperine = ">=X"` compat field in `[plugin]` + the native ABI version check (`piperine_plugin_abi_version`) kept for the device binary only. Pin in spec. |

**Remaining for the spec phase (not blocking the ideal):**
- Release-asset naming convention + the `github:owner/repo@tag` → asset URL
  resolution (Q2 tail).
- The `[plugin]` compat/version field shape (Q6).
- Whether a committed-in-repo device binary is an accepted offline fallback
  (leaning v1 = release-only).
- The exact `staging.add_instance(...)` device-injection API surface (parity
  with the existing `DesignStaging`).
