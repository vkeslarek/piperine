# Plugin Interface v2 — Design

> Architecture for `spec.md` (PLG-01..25). Grounded in the current
> `crates/piperine-plugin` (manifest, trust, contributions, host, view,
> backend/*), `crates/piperine-project/src/lockfile.rs`, and the embedded
> CPython host (`crates/piperine-python/src/embed.rs`). **Most of v2 is
> reduction + two additive pieces** (the decorator surface and the
> release-fetch): the device ABI, TOFU trust, `DesignStaging::add_instance`,
> and the `EntryKind::Plugin` lockfile entry already exist.

## Component map

| Component | Location | v2 action |
|---|---|---|
| WASM backend + crate | `backend/wasm.rs`, `crates/piperine-plugin-wasm` | **delete** (PLG-01) |
| Process backend | `backend/process.rs`, `backend/wire_hosted.rs` | **delete** (PLG-01) |
| Native backend | `backend/native.rs` | keep (device binary path) |
| Imperative `Registrar` | `contributions.rs` | **delete** the `attr_schema/script/device` methods (PLG-04); repurpose the `Contributions` store as the *decorator-populated* table |
| `extern.phdl` stub loader | `host.rs::load_extern_stub`, `MissingExternStub` | **delete** (PLG-07) |
| `Manifest.abi` | `manifest.rs` | **delete** the `abi` field; add shape inference + `piperine` compat field (PLG-21/22) |
| Decorator surface | **new**: `pip::script`/`hook`/`device` (Rust proc-macro) + `@pip.script`/`hook`/`device` (Python) | **add** (PLG-06/10/11) |
| Device ABI export table | `backend/native.rs` + the Rust macro | **add** the `type`→ctor exported symbol table (PLG-05/23) |
| Release fetch | **new**: `crates/piperine-project` (git/release resolver) | **add** (PLG-16/19/20) |
| TOFU trust | `trust.rs` + `lockfile.rs` (`content_hash`, `EntryKind::Plugin`) | reuse; extend key to `(release-url, triple, hash)` (PLG-17/18) |
| `DesignStaging::add_instance` | `view.rs:51` | reuse (PLG-13/14/15) |
| Hooks | `Plugin` trait (5 methods) | keep the five; re-express as the decorator target (PLG-11) |

## §1 `add` = unified install; manifest v2 (shape inference, no `abi`)

**Install (PLG-22, D9):** `piperine add <git>` is the *dependency* command,
Go-style: a bare `owner/repo` resolves to `github.com/owner/repo`; a full
git URL (`https://…`, `git@…`) is used verbatim. A dependency whose root
`Piperine.toml` has a `[plugin]` section contributes (device/scripts/hooks);
importing it loads those contributions into the importing project. A plain
dependency (no `[plugin]`) just resolves via `use`. Same path, one concept.

```toml
[project]
name = "acme_bjt"
version = "1.2.0"

[plugin]                           # present ⇒ contributes; absent ⇒ pure code
python  = "plugin.py"              # present ⇒ scripted/hook entry (optional)
device  = { release = "github:acme/bjt-models@v1.2.0", verify = "sha256:ab…" }

[plugin.permissions]               # deny-by-default; USER must explicitly consent (PLG-23)
filesystem = ["read *.model"]
network = false
```

**Shape** = keys present: `device`→device (fetch+load a binary);
`python`→scripted; neither→pure code.

`Manifest` struct: drop `abi: Abi` and `entry: String`; add
`python: Option<PathBuf>`, `device: Option<DeviceSource>`. `Abi` enum +
`default_timeout` (WASM-only) removed. **No `piperine` compat field** (D12 —
deferred). A manifest with `abi = "wasm"|"process"` → a targeted error
(`PluginError::RemovedBackend`, PLG-02).

**Two trust gates at `add` (PLG-23, D11):**
1. **Permissions consent** — `add` prints `[plugin.permissions]` and requires
   an explicit accept/deny; deny aborts. (A new `add`-time prompt distinct
   from the artifact-hash TOFU.)
2. **Source/binary TOFU** — content-hash pin in `Piperine.lock` (§4),
   independent of the permissions consent.

## §2 Decorator surface + literal parity (PLG-06/10/11/12)

Two front-ends, one shared **contribution table** (`Contributions`, repurposed):

**Python** (`piperine` facade — `crates/piperine-python`): module-level
decorators register into a per-load table the embedded host reads back after
`exec`-ing the plugin's `python` entry:

```python
import piperine as pip

@pip.script("lint")
def lint(args, ctx): ...

@pip.hook.after_elaborate
def check(ctx): ...

@pip.hook.transform_design
def inject(ctx, staging): staging.add_instance(...)
```

**Rust** (`piperine-plugin` proc-macro crate — new
`crates/piperine-plugin-macros`): attribute macros that emit registration
into the same `Contributions` shape via a linker-section / inventory-style
collector (so a native plugin's `.so` self-describes its scripts/hooks/
devices without an imperative `register()` body):

```rust
#[pip::script("lint")]
fn lint(args: &[String], ctx: &Ctx) -> i32 { ... }

#[pip::hook(after_elaborate)]
fn check(ctx: &Ctx) -> Result<()> { ... }

#[pip::device("GummelPoon")]
pub struct GummelPoon { /* Element */ }
```

**Hook catalog (frozen, D8/PLG-11):** `after_parse`, `after_elaborate`,
`transform_design`, `before_lower`, `after_solve` — the exact five the
`Plugin` trait already defines. The decorators target these phases; the host
dispatches by phase name. `ctx` (read hooks) exposes the real `&Design`
(MD-25); `transform_design` additionally gets `&DesignStaging`.

**Parity mechanism (PLG-12):** a `plugin_parity.rs` test (mirroring
`tests/host_parity.rs`) enumerates the decorator names, the five hook phase
names, and the `ctx`/`staging` method names on both hosts and asserts they
are identical — a name added on one side without the other fails the test.

## §3 Device binary: the exported C ABI (PLG-05/23, language-agnostic)

A device binary exports (C ABI, `#[no_mangle]`/`extern "C"`):

```c
uint32_t piperine_plugin_abi_version(void);      // kept — host checks == ABI_VERSION
// A NUL-terminated table of (type_id, constructor) the host reads at load:
const PiperineDeviceEntry* piperine_plugin_devices(size_t* out_len);
// constructor: (const PluginDeviceSpec*) -> Box<dyn Element> (opaque handle)
```

- The **Rust `#[pip::device("Type")]` macro** collects each annotated
  `Element` into `piperine_plugin_devices`'s table via `inventory` — the
  author writes zero ABI boilerplate.
- **Any other language** (C, Zig, an OSDI-compat shim) emits the same three
  symbols by hand — the contract is the symbol table, not Rust (D5).
- The host's native backend (`backend/native.rs`) dlopen's the library,
  checks `piperine_plugin_abi_version`, reads `piperine_plugin_devices`, and
  builds a `type`→`DeviceFactory` map — replacing the old imperative
  `Registrar::device` calls.
- **The `@device`-annotated `mod` lives in the plugin's OWN `.phdl`** (D10),
  shipped in the plugin repo. `piperine add` loads the plugin's PHDL
  (injecting every `@device pub mod` it declares) and resolves each mod's
  `type` against the binary's map. The user just `use`s the plugin package
  and instantiates the mod — no `@device` at the user site.

## §4 Release fetch + triple + TOFU (PLG-16..20)

`DeviceSource = { release: "github:owner/repo@tag", verify: Option<Hash> }`.

**Resolver** (`crates/piperine-project`, new `release.rs`):
1. Parse `github:owner/repo@tag` → the GitHub release API URL.
2. Host **target triple** = `env!("TARGET")`-style current triple.
3. Pick the asset named `lib<pkg>-<triple>.<ext>` (`.so`/`.dll`/`.dylib`);
   **no match → `PluginError::NoAssetForTriple { triple, release }`** (loud,
   PLG-19).
4. Download to a per-user cache (`~/.cache/piperine/plugins/<hash>.<ext>`).
5. **TOFU** (`trust.rs`): hash the bytes; if `verify` is set, compare up
   front (mismatch = hard fail, PLG-18); else prompt/accept/reject by
   `TrustMode`. On accept, pin a `LockEntry { kind: Plugin, source:
   release-url, content_hash, abi: Some(triple) }` in `Piperine.lock`
   (PLG-17).
6. **Reproducibility (PLG-20):** a locked entry short-circuits — a second
   machine fetches the identical asset and matches the pinned hash (no
   prompt). Offline-after-first-fetch: a cached+pinned binary loads without
   network (edge case).

`Piperine.lock` already has `EntryKind::Plugin` + `content_hash` + `abi` +
`trusted_at` — the triple goes in `abi`, the url in `source`, the hash in
`content_hash`. No lockfile schema change needed.

## §5 Removals in detail

- `contributions.rs`: delete `Registrar` + its three methods; `Contributions`
  becomes the decorator-populated store (schemas map deleted entirely — no
  plugin schemas, PLG-08).
- `host.rs`: delete `load_extern_stub`, `MissingExternStub`, and the
  per-plugin stub auto-import (PLG-07). `seed_schemas` seeds ONLY the stdlib
  `@device`/`@port` (`headers/device_port.phdl`) — unchanged gate on
  `!is_empty()` (PLG-09).
- `manifest.rs`: delete `Abi`, `entry`, `default_timeout`, `bench_tasks`
  remnants; add shape fields + `piperine` compat.
- `backend/`: delete `wasm.rs`, `process.rs`, `wire_hosted.rs`; `mod.rs`
  keeps only `native`.
- Delete `crates/piperine-plugin-wasm` from the workspace.

## §6 Phasing (feeds Tasks)

```
Phase 1 — Reduction (PLG-01,02,03,04,07,08,09,21): delete backends/wasm crate,
          kill Registrar + extern-stub, manifest shape inference. The surface
          shrinks to native+Python before anything is added on top.
Phase 2 — Decorator surface + parity (PLG-05,06,10,11,12,24): the pip::/@pip
          decorators (both hosts), the device ABI export table (with the
          @device mod living in the plugin's PHDL, injected on import), the
          parity test.
Phase 3 — Injection (PLG-13,14,15): transform_design staging.add_instance
          device injection.
Phase 4 — Install + distribution (PLG-16,17,18,19,20,22,23): the git-source
          resolver (Go-style), the github-release + triple match + TOFU pin,
          the permissions-consent gate at add.
Phase 5 — Docs (PLG-25,26): part_vi rewrite + one worked example per shape.
```

Phase 1 is pure deletion (safe, big diff, no new concepts). Phases 2–4 are
the additive core. Phase 5 closes.

## §7 Risks / notes

- **Rust proc-macro + `inventory`** is the one genuinely new mechanism
  (self-describing native contributions without a `register()` body). If
  `inventory` proves unworkable across the dlopen boundary, fall back to a
  single generated `piperine_plugin_devices`/`_scripts`/`_hooks` export the
  macro accumulates — same ABI, no inventory dependency. Design keeps the
  ABI (§3) as the source of truth so the collection mechanism is swappable.
- **MD-21 revision:** this design drops MD-21's "self-registers attribute
  schemas" clause (D2). A new `AD-NNN` Decisions-log entry records it when
  the spec is confirmed.
- **Parity is a hard gate** (PLG-12) — the decorator/hook/ctx surface is the
  MD-22 contract; the parity test is non-negotiable, like `host_parity.rs`.
