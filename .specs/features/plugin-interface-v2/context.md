# Plugin Interface v2 — Context (discuss output)

**Gathered:** 2026-07-25
**North-star:** `.specs/features/plugin-interface-v2/ideal.md`
**Status:** Ideal locked — awaiting user sign-off before spec/strategic planning

---

## Feature Boundary

Rewrite the plugin story (P5, MD-21) around **native devices + Python**,
discarding WASM and process backends. A plugin contributes one or more of
three shapes — **pure-PHDL code**, **scripted (Python)**, **device
(binary)** — under one umbrella concept. Declaration and injection are
coupled (no imperative registration); plugins cannot declare `extern`.
Device binaries distribute via GitHub releases with TOFU. Rust and Python
express the same plugin API identically (MD-22).

---

## Locked Decisions

- **D1 — umbrella concept.** "Plugin" covers all 3 shapes; pure-PHDL stays a
  plugin (with a `[plugin]` marker), not folded into plain dependencies.
- **D2 — total `extern`/schema ban.** Plugins declare no `extern`, no new
  attribute schemas. `@device`/`@port` (stdlib) + ordinary PHDL only. The
  per-plugin `extern.phdl` stub mechanism (`load_extern_stub`,
  `MissingExternStub`) is deleted.
- **D3 — literal Rust/Python decorator parity.** `@pip.script`/`@pip.hook`
  (Python) and `#[pip::script]`/`#[pip::hook]` (Rust) — same names, same
  hook catalog, same `ctx`. (Requires a Rust proc-macro.)
- **D4 — device binaries via GitHub release + target triple + TOFU.**
  `device = { release = "github:owner/repo@tag", verify = "sha256" }`.
- **D5 — device is a language-agnostic binary; no Python glue.** The
  contract is the exported C ABI (`type`→constructor table + ABI-version
  symbol). `#[pip::device("Type")]` is the Rust ergonomic that generates
  those exports — but any language emitting a `.so`/`.dll`/`.dylib` with the
  same symbols is a valid device binary (C, Zig, OSDI-compat wrapper, …).
  No Python for a device-only plugin.
- **D6 — unsupported triple = loud error.** No build-from-source in v1.
- **D7 — `verify` optional.** TOFU is the floor; `verify` pins the hash up
  front for supply-chain rigor.
- **D8 — five hooks frozen.** `after_parse`/`after_elaborate`/
  `transform_design`/`before_lower`/`after_solve`; `transform_design` (via
  staging) is the sole device-injection point.

## Agent's Discretion (locked by "recommended", refine in Design)

- Release-asset naming convention (`lib<name>-<triple>.<ext>`) and the
  `github:owner/repo@tag` → asset-URL resolver.
- The exact `staging.add_instance(...)` device-injection API (parity with
  the existing `DesignStaging`).

## Still Open (must resolve in spec — Q6)

- How a plugin declares its target Piperine version / ABI compat now that
  `Manifest.abi` is gone. Leaning: a `piperine = ">=X"` field in `[plugin]`
  + keep the native `piperine_plugin_abi_version` check for the device
  binary only.
- Committed-in-repo device binary as an offline fallback (leaning v1 =
  release-only; note as follow-up).

---

## Supersedes / tensions with existing decisions

- **MD-21** (locked 2026-07-18) said the lifecycle registry is "exposed to
  Python so a plugin self-registers (attribute schemas, hooks, scripts,
  devices)." This feature **revises** it: no imperative *attribute-schema*
  self-registration, no plugin `extern`. Hooks/scripts/devices are still
  contributed, but via declaration-coupled decorators (`#[pip::…]`/`@pip.…`)
  and the `@device` attribute, never a hidden `Registrar` call. **A new
  Decisions-log entry (AD-NNN) should record this revision when the spec
  lands.**

---

## Deferred Ideas

- Build-from-source fallback for unsupported triples (D6 explicitly v1-out).
- Committed-binary offline fallback (leaning out of v1).
- Additional lifecycle hooks beyond the five (D8 — only with a real
  consumer).
