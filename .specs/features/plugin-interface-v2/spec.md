# Plugin Interface v2 Specification

> **Refines ROADMAP P5** ("Plugin interface simplified"). Ideal-first:
> `ideal.md` is the north-star, `context.md` the locked discuss decisions
> (D1–D8). Governing: **MD-21** (native + Python backends only), **MD-22**
> (Rust ≡ Python API), **MD-24** (declared language surface; `extern` is a
> stdlib-only escape hatch), **MD-25** (POM navigability — a plugin-injected
> device is a side artifact, never a rewrite of authored structure).

## Problem Statement

The current plugin system carries three backends (native, WASM, process),
an imperative `Registrar` (a plugin injects attribute schemas / devices /
scripts behind the PHDL author's back), and per-plugin `extern.phdl` stubs
that let any plugin mint new schema names. This is more surface than V1
needs and it erodes the "you can always see what's happening" property. V1
collapses to **native devices + Python**, couples every contribution to a
visible declaration, bans plugin `extern`, and solves device-binary
distribution via GitHub releases + TOFU.

## Goals

- [ ] Delete the WASM and process backends and `piperine-plugin-wasm`
      (MD-21); native dlopen + embedded-Python remain.
- [ ] A plugin contributes one or more of **three shapes** under one
      umbrella: **pure-PHDL** (code library), **scripted** (Python scripts /
      hooks, no binary), **device** (a compiled binary referenced by
      `@device`).
- [ ] **Declaration + injection are coupled** — no imperative `Registrar`.
      A device is a user `mod` + `@device`; a script/hook is a decorator
      that declares AND binds in one place.
- [ ] **No plugin `extern`, no plugin attribute schemas** — the
      `extern.phdl` stub mechanism is deleted; `@device`/`@port` (stdlib)
      are the only plugin-facing schemas.
- [ ] **Literal Rust/Python decorator parity** — `@pip.script`/`@pip.hook`/
      `@pip.device` (Python) and `#[pip::script]`/`#[pip::hook]`/
      `#[pip::device]` (Rust) — same names, same hook catalog, same `ctx`.
- [ ] **Device binary distribution** — `device = { release =
      "github:owner/repo@tag", verify = "sha256" }`; the loader fetches the
      triple-matched release asset, TOFU-verifies its content hash, and pins
      it in `Piperine.lock`; an unsupported triple is a loud error.
- [ ] The device binary is **language-agnostic** — an exported C ABI
      (`type`→`Element`-constructor table + ABI-version symbol); the Rust
      macro is one way to emit it.
- [ ] The **five lifecycle hooks are frozen**; `transform_design` (staging)
      is the sole device-injection point.
- [ ] `cargo test --workspace` green; one "write a plugin" doc per shape.

## Out of Scope

| Feature | Reason |
|---------|--------|
| WASM / process backends | Deleted (MD-21). |
| Build-from-source for an unsupported triple | D6 — v1 is prebuilt-binary only; a missing triple fails loud. |
| Committed-in-repo device binary (offline fallback) | Leaning v1 = release-only; note as follow-up (context.md Deferred). |
| Plugin-authored attribute schemas / `extern` | D2 — banned by design. |
| New lifecycle hooks beyond the five | D8 — only with a real consumer. |
| Non-GitHub release hosts (GitLab, raw URL, S3) | v1 targets `github:` scheme; the resolver is pluggable later. |
| A plugin *registry*/index (crates.io-style discovery) | v1 installs by explicit `piperine add <git>`; discovery is post-V1. |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| Backends | native dlopen + embedded Python only | MD-21 | y (user) |
| Plugin umbrella | all 3 shapes are "plugins" | D1 | y (user) |
| Contribution coupling | declaration + injection coupled; no `Registrar` | D2/D3 | y (user) |
| `extern`/schema ban | total; stub mechanism deleted | D2 | y (user) |
| Rust/Python parity | literal decorators both sides | D3 | y (user) |
| Device distribution | GitHub release + triple + TOFU | D4 | y (user) |
| Device binary language | language-agnostic C ABI; Rust macro is one emitter | D5 (user clarified: "device is a binary, not necessarily Rust") | y (user) |
| Unsupported triple | loud error | D6 | y (user) |
| `verify` hash | optional; TOFU is the floor | D7 | y (user) |
| Hook catalog | frozen at five | D8 | y (user) |
| **Version/ABI compat (Q6)** | `[plugin] piperine = ">=X.Y"` semver-compat field, checked at load; the device binary additionally exports `piperine_plugin_abi_version` (kept), checked against the host's `ABI_VERSION` | The manifest field guards the *source/script* surface (Python/PHDL) where there's no compiled ABI symbol; the exported symbol guards the *binary* surface. Two surfaces, two guards. | n (Design) |
| Release-asset naming | `lib<pkg>-<target-triple>.<ext>` (`.so`/`.dll`/`.dylib`), matched case-sensitively against the release's assets | Predictable, script-generatable in CI; the resolver fails loud if no asset matches the host triple | n (Design) |
| Manifest shape inference | shape = which keys present (`python`→scripted, `device`→device, neither→pure-PHDL); no `abi` field | D1 + MD-21 (WASM/process gone) | n (Design) |

**Open questions:** the three `n (Design)` rows (compat-field shape, asset
naming, shape inference) are HOW-decisions for Design; they do not change
WHAT.

---

## User Stories

### P1: Kill WASM/process, collapse to native + Python ⭐ MVP

**User Story:** As the maintainer, I want the WASM and process backends and
`piperine-plugin-wasm` gone so the plugin surface is native + Python only.

**Why P1:** Every later story is defined against the reduced surface;
removing the dead backends first prevents building v2 on top of v1 cruft.

**Acceptance Criteria:**
1. WHEN the workspace builds THEN `crates/piperine-plugin-wasm` SHALL NOT
   exist and `backend/wasm.rs`/`backend/process.rs`/`backend/wire_hosted.rs`
   SHALL be removed.
2. WHEN a manifest declares `abi = "wasm"` or `abi = "process"` THEN loading
   SHALL fail with a clear "backend removed (native + Python only)" error,
   not an unknown-value error.
3. WHEN the existing native device path (`e2e.rs`'s `@device` fixture) runs
   THEN it SHALL still solve — no regression to native devices.

**Independent Test:** the WASM smoke test is deleted; `native_smoke`/`e2e`
stay green; a `abi="wasm"` manifest errors with the removal message.

---

### P1: Declaration-coupled contributions — delete the imperative Registrar ⭐ MVP

**User Story:** As a PHDL author, I want every plugin contribution to have a
visible declaration at the point of contribution — a device is a `mod` I
wrote, a script/hook is a decorator naming itself — never a hidden
`Registrar::device/attr_schema/script` call.

**Why P1:** The core philosophy change; the decorator surface (next story)
and the extern ban depend on it.

**Acceptance Criteria:**
1. WHEN the plugin API is used THEN there SHALL be no public
   `Registrar::attr_schema` / `Registrar::script` / `Registrar::device`
   imperative-registration surface.
2. WHEN a device is contributed THEN its ports/params SHALL be declared by a
   user `mod` and coupled via `@device(plugin, type)` — the binary supplies
   only the `type`→constructor factory, no schema.
3. WHEN a script or hook is contributed THEN it SHALL be declared AND bound
   by a single decorator (`@pip.script("name")` / `@pip.hook.<phase>`), with
   no separate registration or stub.

**Independent Test:** grep proves `Registrar::{attr_schema,script,device}`
are gone; a device plugin fixture works through `@device` + a
`#[pip::device]`/`@pip.device` factory only.

---

### P1: Ban plugin `extern` + delete `extern.phdl` stubs ⭐ MVP

**User Story:** As the maintainer, I want plugins unable to declare `extern`
or new attribute schemas, so MD-24's escape hatch stays rare and auditable.

**Acceptance Criteria:**
1. WHEN a plugin ships an `extern.phdl` (or any `extern` declaration) THEN
   it SHALL be rejected/ignored — the per-plugin stub loader
   (`load_extern_stub`, `MissingExternStub`) SHALL be deleted.
2. WHEN a plugin needs extra device metadata THEN it SHALL use ordinary PHDL
   (`param`, a `bundle`) — there is no plugin-schema path.
3. WHEN a plugin loads THEN only the stdlib `@device`/`@port` schemas
   (`headers/device_port.phdl`) SHALL be seeded — no plugin-contributed
   schema names.

**Independent Test:** a plugin fixture carrying an `extern.phdl` fails loud
(or the file is inert); the extern-coverage guard still passes for the
stdlib surface only.

---

### P2: Decorator API with literal Rust/Python parity

**User Story:** As a plugin author, I want the same decorators in Rust and
Python — `@pip.script`/`#[pip::script]`, `@pip.hook.<phase>`/
`#[pip::hook(<phase>)]`, `@pip.device`/`#[pip::device]` — same names, same
hook catalog, same `ctx`.

**Acceptance Criteria:**
1. WHEN a script is declared `@pip.script("lint")` (Python) or
   `#[pip::script("lint")]` (Rust) THEN `piperine lint …` SHALL dispatch to
   it identically.
2. WHEN a hook is declared for one of the five phases in either language
   THEN it SHALL fire at that phase with the same `ctx` surface (read-only
   POM for read hooks; staging for `transform_design`).
3. WHEN the two hosts are compared THEN the decorator names, the hook phase
   names, and the `ctx`/`staging` method names SHALL be identical (a parity
   test locks them, mirroring `host_parity.rs`).

**Independent Test:** a parity test enumerates the decorator + hook + ctx
surface on both hosts and asserts identical names; a Python `@pip.script`
and a Rust `#[pip::script]` fixture both dispatch `piperine <name>`.

---

### P2: Device injection via `transform_design`

**User Story:** As a plugin author, I want to inject a device into the POM at
`transform_design` — the scenario-4 case — through the staging surface, not
free mutation.

**Acceptance Criteria:**
1. WHEN a `transform_design` hook calls `staging.add_instance(parent, label,
   module, ports…)` THEN the instance SHALL appear in the design consumed by
   the subsequent analysis, and NOT overwrite authored structure (MD-25).
2. WHEN the injected module is itself a `@device` THEN it SHALL solve through
   the same native-device path.
3. WHEN injection targets a non-existent parent/module THEN it SHALL fail
   loud at staging time, never silently drop.

**Independent Test:** a hook injects a parasitic `@device` resistor into a
top module; the solve reflects it; an injection to a bad parent errors.

---

### P2: Device-binary distribution — GitHub release + triple + TOFU

**User Story:** As a plugin user, I want `piperine add github:acme/bjt@v1`
to fetch the right prebuilt device binary for my platform, verify it, and
pin it, with no hand-placed artifact.

**Acceptance Criteria:**
1. WHEN a manifest declares `device = { release = "github:owner/repo@tag" }`
   THEN the loader SHALL resolve the release, pick the asset whose name
   matches the host target triple, download it to a per-user cache, and load
   it as the device binary.
2. WHEN the fetched binary's content hash is first seen THEN TOFU SHALL
   prompt (Interactive) / accept (AcceptAll) / reject (RejectUntrusted), and
   the approved hash SHALL be pinned in `Piperine.lock` (`(release-url,
   triple, content-hash)`); a changed asset re-prompts.
3. WHEN `verify = "sha256:<hex>"` is set THEN the fetched asset's hash SHALL
   be checked against it up front — mismatch is a hard fail with no prompt;
   match loads without a TOFU prompt.
4. WHEN no release asset matches the host triple THEN loading SHALL fail with
   a clear message naming the triple and the release — no silent skip, no
   build-from-source.
5. WHEN a second machine resolves the same locked entry THEN it SHALL fetch
   the identical, already-approved binary (reproducible from the lockfile).

**Independent Test:** a fixture "release" (local file server or a stubbed
resolver) serves a triple-named asset; the loader fetches + hashes + pins
it; a wrong-triple release errors; a `verify` mismatch hard-fails.

---

### P2: Manifest shape inference + version compat

**User Story:** As a plugin author, I want the manifest to infer my plugin's
shape from what I declare (no `abi` field) and to state which Piperine
version I target.

**Acceptance Criteria:**
1. WHEN `[plugin]` has a `python` key THEN the plugin is scripted; a `device`
   key THEN it is a device; neither THEN it is pure-PHDL — with no `abi`
   field anywhere.
2. WHEN `[plugin] piperine = ">=X.Y"` is set and the host version does not
   satisfy it THEN loading SHALL fail with a version-mismatch error.
3. WHEN a device binary's exported `piperine_plugin_abi_version` differs from
   the host `ABI_VERSION` THEN loading that binary SHALL fail loud (kept
   check).

**Independent Test:** manifests exercising each shape load with the inferred
shape; an incompatible `piperine = ">=99"` errors; an ABI-mismatched binary
errors.

---

### P3: One "write a plugin" doc per shape

**User Story:** As a new plugin author, I want a worked example for each
shape (pure-PHDL, scripted, device) so writing a plugin is a documented
afternoon.

**Acceptance Criteria:**
1. WHEN the docs are read THEN there SHALL be a worked example per shape, and
   the Rust/Python decorator equivalence SHALL be shown side by side.
2. WHEN `docs/spec/part_vi_plugins.md` is read THEN it SHALL describe the v2
   surface only (no WASM/process/Registrar/extern-stub).

**Independent Test:** doc review gate.

---

## Edge Cases

- WHEN a plugin ships BOTH a `device` binary AND a `python` script THEN both
  load; the device is pure-ABI, the Python adds the script/hook.
- WHEN two plugins contribute the same `@device` `type` id THEN it is a
  loud conflict (unchanged from today's `SchemaConflict`-class behavior).
- WHEN a pure-PHDL plugin has a `[plugin]` section but no `python`/`device`
  THEN it loads as a code library — its `pub` items resolve via `use`,
  nothing else runs.
- WHEN the network is unavailable during a device fetch AND the binary is
  already cached+pinned THEN it SHALL load from cache (offline-after-first-
  fetch), never re-download.
- WHEN a `github:` release tag is mutable and its asset changes THEN TOFU's
  content-hash mismatch SHALL catch it (re-prompt / hard-fail with `verify`).

---

## Requirement Traceability

| ID | Story | Phase | Status |
|----|-------|-------|--------|
| PLG-01 | P1 kill WASM/process | — | Pending |
| PLG-02 | P1 abi=wasm/process errors loud | — | Pending |
| PLG-03 | P1 native device no-regression | — | Pending |
| PLG-04 | P1 no imperative Registrar | — | Pending |
| PLG-05 | P1 device = mod + @device coupling | — | Pending |
| PLG-06 | P1 script/hook = one decorator | — | Pending |
| PLG-07 | P1 delete extern.phdl stub loader | — | Pending |
| PLG-08 | P1 no plugin schema path | — | Pending |
| PLG-09 | P1 only stdlib @device/@port seeded | — | Pending |
| PLG-10 | P2 script parity dispatch | — | Pending |
| PLG-11 | P2 hook parity (5 phases, same ctx) | — | Pending |
| PLG-12 | P2 decorator/hook/ctx name parity test | — | Pending |
| PLG-13 | P2 staging.add_instance injects | — | Pending |
| PLG-14 | P2 injected @device solves | — | Pending |
| PLG-15 | P2 bad-injection fails loud | — | Pending |
| PLG-16 | P2 release→triple asset fetch | — | Pending |
| PLG-17 | P2 TOFU hash pin in lockfile | — | Pending |
| PLG-18 | P2 verify hash up-front | — | Pending |
| PLG-19 | P2 unsupported triple loud error | — | Pending |
| PLG-20 | P2 reproducible from lockfile | — | Pending |
| PLG-21 | P2 shape inference (no abi field) | — | Pending |
| PLG-22 | P2 piperine version compat | — | Pending |
| PLG-23 | P2 device ABI-version check kept | — | Pending |
| PLG-24 | P3 docs per shape + parity | — | Pending |
| PLG-25 | P3 part_vi rewrite to v2 | — | Pending |

25 requirements.

---

## Success Criteria

- [ ] Native device + Python scripted + pure-PHDL all work; WASM/process
      gone.
- [ ] No imperative `Registrar`; no plugin `extern`/schema.
- [ ] Rust and Python decorator surfaces are name-identical (parity test).
- [ ] `piperine add github:owner/repo@tag` fetches + verifies + pins a
      device binary; wrong triple / bad hash fail loud.
- [ ] `cargo test --workspace` green; docs rewritten to v2.
