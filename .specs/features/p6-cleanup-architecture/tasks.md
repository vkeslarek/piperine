# P6 Cleanup — Architecture & Readability · Tasks

## Execution Protocol (MANDATORY — do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and
follow its Execute flow and Critical Rules.** Do not search for skill files by
filesystem path. The skill is the source of truth for the full flow (per-task
cycle, sub-agent delegation, adequacy review, Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed
without it.**

---

**Design**: `.specs/features/p6-cleanup-architecture/design.md`
**Spec**: `.specs/features/p6-cleanup-architecture/spec.md`
**Status**: In Progress — Phases 1–2 DONE (T1–T13), Phase 3 next

## Progress log

| Batch | Phase | Tasks | Commits | Result |
|---|---|---|---|---|
| 1 | 1 | T1–T7 ✅ | `e82e9ef`, `db2869c`, `213a9d6`, `2d25d26`, `6ca2fee`, `b9b60f3`, `c6d9cba`, `3a2a49c` | 1163 passed / 0 failed / 4 ignored (net 0: −1 deleted `Port` enum test, +1 guard). clippy clean. Guard MD-33 proven able to fail. |
| 2 | 2 | T8–T13 ✅ | `de4ff73`, `e19bf50`, `5611b71`, `cdef36b`, `f071364`, `2ad01f7` | 1164 passed / 0 failed / 4 ignored (+1 guard). Doc warnings 47→46 (the `IrProgram` unresolved link is gone; no new ones). Guard MD-35 proven able to fail. |

**Phase-2 findings:** `CLAUDE.md` carried **three false "Known gaps"** — `$limit`/
pnjlim, `idt`'s AC `1/jω`, and multiple `ac_stim` are all implemented and recorded
delivered in `ROADMAP.md`; `NewtonStrategy`/`StepperStrategy` likewise exist. A doc
claiming a capability is *missing* when it ships is worse than a stale count: it
sends the next reader to reimplement it. Also corrected: solver paths said `solver/`
where the tree has `analyses/`. `solver-simplification` is genuinely delivered
(Verifier PASS 18/18) — status corrected, no residue. `docs/manual/` removed with a
`ROADMAP.md` P6 backlog line; `mkdocs.yml` nav re-validated.

**Deferred (pre-existing, out of scope, surfaced by T13):** `mkdocs build` fails on
a missing `material` theme; `docs/spec/appendix_c_host_surface.md` and
`docs/spec/part_viii_host_api.md` are in neither mkdocs nav.

**Phase-1 findings that changed later tasks:** T46's scope grew by three
functions (the census said 4 >200-line functions, the truth is 7); the doc gate
excludes two crates (pre-existing `numpy` rustdoc ICE); `CARGO_PROFILE_DEV_DEBUG=0`
is mandatory (disk). T4's two done-when criteria were in conflict (grep-empty vs
keep-a-legibility-note) and were resolved in favour of the notes. T6 found an
8th item-scope allow the brief missed (`solver/src/digital/events.rs:86`).

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec — confirm before Execute.
> Guidelines found: `AGENTS.md` (Hard rules + MD-13 idiom rules + test-placement
> table), `CLAUDE.md` (build/test commands, tests of record), `.specs/STATE.md`
> (MD-13, MD-28 test placement, MD-31 policy-invariant-is-a-test).
>
> **Feature-specific note:** this is a refactor-only feature. For refactor tasks
> the *existing* suite is the required test — a task's tests are "the suites that
> cover the moved code, passing at an unchanged count". New tests are required
> only where the feature adds surface (guards, the api object model, the Rust
> object-model proof). MD-31 adds a hard rule on top: **every new guard must be
> proven able to fail** (inject violation → observe named failure → revert), and
> the proof is part of the task's Done-when.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
|---|---|---|---|---|
| Solver internals (`piperine-solver/src/**`) | integration (existing) | Every moved/deleted item's covering suite passes at unchanged count | `crates/piperine-solver/tests/*.rs` + inline `#[cfg(test)]` | `cargo test -p piperine-solver` |
| Codegen internals (`piperine-codegen/src/**`) | integration (existing) | Same, plus `analog_device_numerics.rs`/`silent_bugs.rs` for any expression-machinery change | `crates/piperine-codegen/tests/*.rs` | `cargo test -p piperine-codegen` |
| Lang frontend (`piperine-lang/src/**`, incl. `parse/`) | integration (existing, frozen corpora) | Parse/elab results identical for every frozen corpus input | `crates/piperine-lang/tests/*.rs` | `cargo test -p piperine-lang` |
| Lang-server (`piperine-lang-server/src/**`) | integration (existing) | Every handler suite passes at unchanged count | `crates/piperine-lang-server/tests/*.rs` | `cargo test -p piperine-lang-server` |
| Host api surface (`piperine-api/src/**`) | integration | Every AC of CLA-14..19 has a test; host parity enumerated both sides; ngspice numerics unchanged | root `tests/*.rs` (`host_*`, `session*`, `ngspice_validation`, new `host_object_model.rs`) | `cargo test --workspace` |
| Python bindings (`piperine-python/src/**`) | integration | Python-facing name/signature/return identical before/after (D6); extracted tests keep their assertions | `crates/piperine-python/tests/*.rs` + root `tests/host_parity.rs` | `cargo test -p piperine-python && cargo test --test host_parity` |
| Project-policy guards | integration + **failure proof** | Each guard names the offender in its message AND is demonstrated to fail on an injected violation (MD-31) | root `tests/suite_hygiene.rs` | `cargo test --test suite_hygiene` |
| Docs / MD entries (`*.md`, `.specs/**`) | none | Build gate only; claims that *can* be mechanized become guards instead of prose | — | build gate |

## Gate Check Commands

> Generated from codebase — confirm before Execute.

| Gate Level | When to Use | Command |
|---|---|---|
| Quick | Task touching exactly one crate's internals | `cargo test -p <crate>` |
| Full | Task touching the host surface, Python, or more than one crate | `cargo test --workspace` |
| Build | Phase completion, doc-only tasks, guard tasks | `cargo build --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo doc --workspace --no-deps --exclude piperine-python --exclude piperine-cli` |

**Two corrections from Phase 1's execution (binding on every later batch):**

1. **`CARGO_PROFILE_DEV_DEBUG=0` is mandatory for every gate.** The default
   dev profile's `debuginfo=2` makes the workspace test build ~63 GB across 166
   targets, which filled `/home` and failed T5's first gate with
   `No space left on device`. With the env var (no repo file changed) `target/`
   is ~6.8 GB. Run `df -h /home` before any full gate; stop and report if free
   space drops under ~10 GB.
2. **The doc gate excludes `piperine-python` and `piperine-cli`.** A rustdoc ICE
   inside `numpy 0.23.0` makes `cargo doc` exit 101 for those two crates at
   baseline — pre-existing, not this feature's. Baseline is **47 warnings**
   across the four documenting crates (see `baseline.md` §3.1). CLA-06's
   zero-broken-intra-doc-links criterion is scoped to those four crates.

**Baseline (captured by T1):** `cargo test --workspace` passed-count, ignored
count, clippy warning count (expected 0), `cargo doc` warning count. Every later
task states its count and it must be **≥ baseline** (increases come only from
tests this feature adds; any decrease must be itemized by name — MD-28.3
deletions are the only legal decrease).

---

## Execution Plan

Phases run sequentially; tasks within a phase run in order.

### Phase 1: Baseline & dead surface (7)
```
T1 → T2 → T3 → T4 → T5 → T6 → T7
```

### Phase 2: Comment & doc truth (6)
```
T8 → T9 → T10 → T11 → T12 → T13
```

### Phase 3: Homes — solver + codegen `mod.rs` (6)
```
T14 → T15 → T16 → T17 → T18 → T19
```

### Phase 4: Test placement (4)
```
T20 → T21 → T22 → T23
```

### Phase 5: Session collapse (7)
```
T24 → T25 → T26 → T27 → T28 → T29 → T30
```

### Phase 6: Object-model lift (6)
```
T31 → T32 → T33 → T34 → T35 → T36
```

### Phase 7: Codegen abstractions (5)
```
T37 → T38 → T39 → T40 → T41
```

### Phase 8: Long-function decomposition (5)
```
T42 → T43 → T44 → T45 → T46
```

### Phase 9: Free-fn debt + MD entries (4)
```
T47 → T48 → T49 → T50
```

---

## Task Breakdown

### T1: Capture the pre-feature baseline

**What**: Record the workspace's measurable state so every later task can prove it did not regress.
**Where**: `.specs/features/p6-cleanup-architecture/baseline.md` (new)
**Depends on**: None
**Reuses**: `cargo test --workspace`, `cargo clippy`, `cargo doc`
**Requirement**: D9 (spec Assumptions)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Test passed/failed/ignored counts recorded per crate and for the workspace
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` result recorded
- [ ] `cargo doc --workspace --no-deps` warning count recorded
- [ ] Census recorded: module-level `fn` count per crate, `mod.rs` sizes > 60, functions > 200 lines, file-scope `allow` list, dead-identifier hit list
- [ ] No source file changed

**Tests**: none (measurement task) · **Gate**: build

**Commit**: `docs(p6-arch): capture cleanup baseline`

---

### T2: Delete the two fully-dead solver files

**What**: Remove `math/constant.rs` (8 unused constants) and `core/port.rs` (unused `enum Port`), including their `mod` declarations.
**Where**: `crates/piperine-solver/src/math/constant.rs`, `crates/piperine-solver/src/core/port.rs`, `math/mod.rs`, `core/mod.rs`
**Depends on**: T1
**Reuses**: —
**Requirement**: CLA-03

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Both files absent; no `mod constant;`/`mod port;` remains
- [ ] `grep -rn "SPEED_OF_LIGHT\|PLANCK_CONSTANT\|BOLTZMANN_CONSTANT\|ELEMENTARY_CHARGE\|ABSOLUTE_ZERO_CELSIUS" crates src` → only unrelated live definitions (if any) remain
- [ ] `cargo test -p piperine-solver` passes at baseline count
- [ ] Zero warnings

**Tests**: integration (existing solver suite) · **Gate**: quick

**Commit**: `refactor(solver): delete dead constant/port modules`

---

### T3: Delete the four dead analysis traits

**What**: Remove `DcAnalysis`, `AcAnalysis`, `TransientAnalysis`, `NoiseSource` — the trait-based analysis abstraction nothing implements or calls.
**Where**: `crates/piperine-solver/src/analyses/{dc,ac,transient,noise}.rs`
**Depends on**: T2
**Reuses**: —
**Requirement**: CLA-05 (spec P1 story 1 AC5)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] The four trait definitions are gone; grep for each name returns nothing
- [ ] Any `//!` text promising a trait-based analysis contract is corrected in the same commit
- [ ] `cargo test -p piperine-solver` at baseline count, zero warnings

**Tests**: integration (existing) · **Gate**: quick

**Commit**: `refactor(solver): delete unimplemented analysis traits`

---

### T4: Delete MD-03's four dead per-analysis contexts

**What**: Remove `AcContext`, `TransientContext`, `NoiseContext`, `TfContext` (D2 — MD-03 is superseded, amendment lands in T50).
**Where**: `crates/piperine-solver/src/analyses/{ac,transient,noise,tf}.rs`
**Depends on**: T3
**Reuses**: —
**Requirement**: CLA-04

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] The four structs are gone; grep returns nothing
- [ ] A one-line note in each touched file's `//!` states that per-analysis context is not implemented (pointing at MD-03's superseded status), so the absence is legible
- [ ] `cargo test -p piperine-solver` at baseline count, zero warnings

**Tests**: integration (existing) · **Gate**: quick

**Commit**: `refactor(solver): delete unwired per-analysis contexts (MD-03)`

---

### T5: Delete the dead codegen resolve/pom surface

**What**: Remove the five never-used methods (`require_param_given`, `lookup_param`, `lookup_var`, `require_ident_as_param`, `require_var`), the two dead free functions (`has_ddt_marker`, `contrib_branch`), and the never-read fields (`structure.rs`'s `bundle_name`, `pz.rs`'s `options`).
**Where**: `crates/piperine-codegen/src/resolve/pom/{mod,stmt,structure}.rs`, `crates/piperine-solver/src/analyses/pz.rs`
**Depends on**: T4
**Reuses**: —
**Requirement**: CLA-01

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] All nine items gone; grep for each name returns nothing
- [ ] `cargo test -p piperine-codegen && cargo test -p piperine-solver` at baseline counts
- [ ] Zero warnings

**Tests**: integration (existing codegen + solver suites) · **Gate**: full

**Commit**: `refactor(codegen): delete dead resolve/pom helpers and fields`

---

### T6: Remove every file-scope lint suppression

**What**: Delete all 12 `#![allow(dead_code)]` lines and triage the 7 item-scope allows (delete the item, give it a consumer, or add a one-line justification comment).
**Where**: `crates/piperine-solver/src/analyses/{ac,dc,disto,events,noise,pz,sp,tf,transient}.rs`, `crates/piperine-solver/src/{math/constant.rs,core/port.rs}` (already gone via T2), `crates/piperine-codegen/src/resolve/pom/mod.rs`; item-scope: `codegen/src/emit/builder.rs:146,150,154`, `lang/src/resolve.rs:43`, `lang-server/src/state.rs:74`, `lang-server/src/handlers/diagnostics.rs:145`, `solver/src/analyses/pz.rs:77`
**Depends on**: T5
**Reuses**: —
**Requirement**: CLA-01, CLA-02

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `grep -rn "^#!\[allow(" crates/*/src src` → empty
- [ ] Every remaining `#[allow(dead_code)]` has a preceding one-line justification
- [ ] `cargo check --workspace --all-targets` → zero `dead_code` warnings
- [ ] `cargo test --workspace` at baseline count

**Tests**: integration (existing, whole workspace) · **Gate**: full

**Commit**: `refactor: remove file-scope dead_code suppression`

---

### T7: Guard — no file-scope lint suppression (MD-33)

**What**: Add `no_file_scope_lint_suppression` to the hygiene suite: scan every `crates/*/src` and `src` file, fail naming file+line on any `#![allow(`.
**Where**: `tests/suite_hygiene.rs`
**Depends on**: T6
**Reuses**: `tests/suite_hygiene.rs`'s existing tree walk; `capabilities_contract.rs`'s registry+exhaustiveness shape
**Requirement**: CLA-05 (AC6)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Guard passes on the clean tree
- [ ] **Failure proof**: a `#![allow(dead_code)]` injected into one file makes the guard fail naming that file; injection reverted; the proof is quoted in the task report
- [ ] `cargo test --test suite_hygiene` passes; workspace count = baseline + 1

**Tests**: integration + failure proof · **Gate**: full

**Commit**: `test(hygiene): guard against file-scope lint suppression`

---

### T8: Fix the broken intra-doc links

**What**: Rewrite `device/mod.rs`'s `CircuitCompiler` doc to name the POM types it actually walks, and `lang/src/math.rs`'s header to name the live const-evaluator entry point.
**Where**: `crates/piperine-codegen/src/device/mod.rs:8`, `crates/piperine-lang/src/math.rs:2`
**Depends on**: T7
**Reuses**: —
**Requirement**: CLA-06

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Neither file references `IrProgram`/`IrExpr`
- [ ] `cargo doc --workspace --no-deps` → zero broken intra-doc-link warnings
- [ ] `cargo test --workspace` at current count

**Tests**: none (doc layer) · **Gate**: build

**Commit**: `docs(codegen,lang): fix broken intra-doc links`

---

### T9: Rewrite the IR-archaeology comments

**What**: Convert the 14 remaining "formerly the IR crate / no `IrModule` twin" comments into positive statements about what the code does now; keep exactly one historical note, in `codegen/src/lib.rs`.
**Where**: `crates/piperine-codegen/src/{lib.rs,resolve/mod.rs,resolve/expr.rs,resolve/stmt.rs,resolve/pom/mod.rs,device/circuit.rs,device/builder.rs,flatten/analog.rs,emit/analog_expr.rs}`, `crates/piperine-lang/src/elab/lower/module.rs:212`
**Depends on**: T8
**Reuses**: —
**Requirement**: CLA-07

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `grep -rEn "IrProgram|IrModule|IrExpr|IrInstance|piperine[-_]ir" crates/*/src src` → exactly 1 hit, in `codegen/src/lib.rs`
- [ ] Every deliberate invariant that was expressed as a negation ("no structural twin") is now a positive statement of what codegen reads
- [ ] `cargo doc --workspace --no-deps` clean; `cargo test --workspace` at count

**Tests**: none (doc layer) · **Gate**: build

**Commit**: `docs(codegen): describe the present, not the deleted IR`

---

### T10: Guard — no dead-architecture identifiers (MD-35)

**What**: Add `no_dead_architecture_identifiers`: scan `src` trees for the five dead identifiers, allow exactly the one registered note, fail naming file+identifier.
**Where**: `tests/suite_hygiene.rs`
**Depends on**: T9
**Reuses**: the T7 scan helper
**Requirement**: CLA-08

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Guard passes; the single allowed note is registered in a table inside the test, not hardcoded in a regex exception
- [ ] **Failure proof**: adding `// see IrProgram` to any file fails the guard naming file+identifier; reverted; quoted in the report
- [ ] Workspace count = previous + 1

**Tests**: integration + failure proof · **Gate**: full

**Commit**: `test(hygiene): guard against dead-architecture identifiers`

---

### T11: `CLAUDE.md` truth pass

**What**: Remove the hand-maintained "51 green targets" count and the hand-maintained tests-of-record file list, replacing them with a pointer to the enumerating guard; correct the module names to today's tree.
**Where**: `CLAUDE.md`
**Depends on**: T10
**Reuses**: `tests/suite_hygiene.rs` as the enumerating authority
**Requirement**: CLA-07 (AC6)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] No numeric test-count claim remains
- [ ] Every file path named in `CLAUDE.md` exists (verified by grep/ls)
- [ ] `cargo test --workspace` at count

**Tests**: none (doc layer) · **Gate**: build

**Commit**: `docs: make CLAUDE.md claims verifiable`

---

### T12: `AGENTS.md` truth pass

**What**: Fix the stale briefing — MD-19 → MD-20 (`piperine-api` is the library face), the `lower/`+`jit/` module names → MD-23's `resolve/flatten/emit/kernel/device`, the "51 green targets" count, and the test-placement table's dead entries (`codegen_ir.rs`, `from_ir.rs`, `analog_jit.rs`, `phase3.rs`, `manifest.rs`).
**Where**: `AGENTS.md`
**Depends on**: T11
**Reuses**: `CLAUDE.md`'s corrected crate table
**Requirement**: CLA-07

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every crate/module/file path named in `AGENTS.md` exists
- [ ] The dependency-flow and library-face statements match MD-20
- [ ] No numeric test-count claim remains
- [ ] `cargo test --workspace` at count

**Tests**: none (doc layer) · **Gate**: build

**Commit**: `docs: align AGENTS.md with the current tree`

---

### T13: Reconcile `STATE.md` status and decide `docs/manual/`

**What**: Correct `solver-simplification`'s status (batch 6 delivered, or the residue named explicitly), and either give `docs/manual/` authored content with a ROADMAP backlog entry or remove it and its mkdocs wiring.
**Where**: `.specs/STATE.md`, `docs/manual/`, `mkdocs.yml`, `ROADMAP.md`
**Depends on**: T12
**Reuses**: `.specs/features/solver-simplification/`'s own task list as the source of truth
**Requirement**: CLA-07 (AC7, AC8)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `solver-simplification`'s status statement matches its task list's real state
- [ ] `docs/manual/` either has content + a tracked backlog line, or is gone with `mkdocs.yml` updated and the site config still valid
- [ ] `cargo test --workspace` at count

**Tests**: none (doc layer) · **Gate**: build

**Commit**: `docs: reconcile feature status and manual placeholder`

---

### T14: Give the solver's floating modules a home

**What**: Collapse the one-file `analog/` directory into `analog.rs` and move `result.rs` under `core/`.
**Where**: `crates/piperine-solver/src/{analog/,result.rs,lib.rs,core/mod.rs}`
**Depends on**: T13
**Reuses**: `core/net.rs` as the naming sibling for the netlist module
**Requirement**: CLA-10

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `crates/piperine-solver/src/analog/` no longer exists; `analog.rs` holds the netlist surface
- [ ] `result.rs` lives at `core/result.rs`
- [ ] `prelude.rs`/`abi.rs` re-exports unchanged in name (MD-17 surface intact)
- [ ] `cargo test --workspace` at count, zero warnings

**Tests**: integration (existing) · **Gate**: full

**Commit**: `refactor(solver): give netlist and result modules real homes`

---

### T15: Move the run config out of `analyses/mod.rs`

**What**: Relocate `Tolerances`, `Context`, and `Policy` into the config/context home and fold the `Once` init into `Solver::build` (MD-06), leaving `analyses/mod.rs` as declarations + the layer contract.
**Where**: `crates/piperine-solver/src/analyses/{mod.rs,config.rs,context.rs}`
**Depends on**: T14
**Reuses**: `analyses/config.rs` (the declared config home)
**Requirement**: CLA-10 (spec P1 story 3 AC3), MD-06

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `analyses/mod.rs` ≤ 60 lines, contains no type definition
- [ ] `Tolerances`/`Context`/`Policy` live in the config/context module and are re-exported unchanged
- [ ] One-time init happens in `Solver::build`, not on `Context::default` (MD-06)
- [ ] `cargo test --workspace` at count, zero warnings

**Tests**: integration (existing) · **Gate**: full

**Commit**: `refactor(solver): move run config into the config home`

---

### T16: `device/analog/mod.rs` → `instance.rs`

**What**: Move the 1237-line `AnalogInstance` implementation into a file named after it.
**Where**: `crates/piperine-codegen/src/device/analog/{mod.rs,instance.rs}`
**Depends on**: T15
**Reuses**: the sibling pattern already in that directory (`forces.rs`/`limits.rs`/`operators.rs`/`events.rs`)
**Requirement**: CLA-09

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `device/analog/mod.rs` ≤ 60 lines, declarations + `//!` contract only
- [ ] `AnalogInstance` lives in `device/analog/instance.rs`
- [ ] `cargo test -p piperine-codegen` at count, zero warnings

**Tests**: integration (existing codegen suite) · **Gate**: quick

**Commit**: `refactor(codegen): move AnalogInstance out of mod.rs`

---

### T17: `kernel/analog/mod.rs` → `kernel.rs`

**What**: Move the 864-line `AnalogKernel` implementation into a named file.
**Where**: `crates/piperine-codegen/src/kernel/analog/{mod.rs,kernel.rs}`
**Depends on**: T16
**Reuses**: the sibling pattern (`compile.rs`/`limits.rs`/`forces.rs`/`noise.rs`/`ac_stim.rs`/`reactive.rs`)
**Requirement**: CLA-09

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `kernel/analog/mod.rs` ≤ 60 lines, declarations only
- [ ] `AnalogKernel` in `kernel/analog/kernel.rs`; `lib.rs`'s re-export unchanged
- [ ] `cargo test -p piperine-codegen` at count, zero warnings

**Tests**: integration (existing) · **Gate**: quick

**Commit**: `refactor(codegen): move AnalogKernel out of mod.rs`

---

### T18: `device/mod.rs` → `compiled.rs` + `element.rs`

**What**: Split the 859-line `device/mod.rs` at its natural seam: `CompiledModule` (the per-module artifact) and `PiperineDevice` (the per-instance `Element`).
**Where**: `crates/piperine-codegen/src/device/{mod.rs,compiled.rs,element.rs}`
**Depends on**: T17
**Reuses**: existing `device/` siblings (`builder.rs`/`circuit.rs`/`fusion.rs`/`plugin.rs`)
**Requirement**: CLA-09

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `device/mod.rs` ≤ 60 lines, declarations + contract only
- [ ] `CompiledModule` and `PiperineDevice` each in the file named for it
- [ ] `lib.rs` façade re-exports unchanged (MD-23)
- [ ] `cargo test -p piperine-codegen` at count, zero warnings

**Tests**: integration (existing) · **Gate**: quick

**Commit**: `refactor(codegen): split device/mod.rs by artifact`

---

### T19: `resolve/pom/mod.rs` → `body.rs` + guard `mod.rs` size (MD-34)

**What**: Move `LoweredBody`/`lower_bodies` into `resolve/pom/body.rs`, then add `mod_rs_declares_only` (≤ 60 lines unless a recognized `// hygiene-exempt:` line is present).
**Where**: `crates/piperine-codegen/src/resolve/pom/{mod.rs,body.rs}`, `tests/suite_hygiene.rs`
**Depends on**: T18
**Reuses**: the T7/T10 scan helper
**Requirement**: CLA-09, CLA-13

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every `mod.rs` under `crates/*/src` ≤ 60 lines (or exempt with a reason)
- [ ] Guard passes; **failure proof**: 61+ lines injected into one `mod.rs` fails the guard naming file + line count; reverted; quoted
- [ ] `cargo test --workspace` = previous + 1, zero warnings

**Tests**: integration + failure proof · **Gate**: full

**Commit**: `refactor(codegen): move LoweredBody out of mod.rs; guard mod.rs size`

---

### T20: Move `tests/sens.rs` to the solver crate

**What**: Relocate the root `sens.rs` target (imports codegen+solver+lang, never `piperine`) into `crates/piperine-solver/tests/`, keeping its `//!` scope header (MD-28 §2).
**Where**: `tests/sens.rs` → `crates/piperine-solver/tests/sens.rs`
**Depends on**: T19
**Reuses**: the solver crate's existing test-target conventions
**Requirement**: CLA-12

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Target absent from root, present in the solver crate, `//!` header declares its scope
- [ ] Any test duplicated by an existing solver test is deleted with the survivor named in the commit body (MD-28.3)
- [ ] `cargo test --workspace` count = baseline-adjusted (any deletion itemized)

**Tests**: integration (the moved target itself) · **Gate**: full

**Commit**: `test(solver): move sens target to the crate it exercises`

---

### T21: Move `tests/pss.rs` and `tests/transient_reentry.rs` to the solver crate

**What**: Same relocation for the two remaining root targets that never touch the root crate.
**Where**: `tests/{pss,transient_reentry}.rs` → `crates/piperine-solver/tests/`
**Depends on**: T20
**Reuses**: T20's pattern
**Requirement**: CLA-12

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Both targets moved with `//!` scope headers; root `tests/pss_host.rs` (the host-level twin) stays put
- [ ] Duplicates deleted with survivors named
- [ ] `cargo test --workspace` count accounted for

**Tests**: integration (the moved targets) · **Gate**: full

**Commit**: `test(solver): move pss and transient-reentry targets`

---

### T22: Extract `piperine-python/src/lib.rs`'s test block

**What**: Move the inline `#[cfg(test)] mod tests` (~1070 lines, 3 ~100-line cases with inline PHDL) into `crates/piperine-python/tests/` split by feature, with fixtures in `tests/common/`.
**Where**: `crates/piperine-python/src/lib.rs` → `crates/piperine-python/tests/{design,waveform,ac,noise}.rs` + `tests/common/mod.rs`
**Depends on**: T21
**Reuses**: `crates/piperine-lang-server/tests/common/` as the fixture-sharing pattern
**Requirement**: CLA-11

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `lib.rs` ≤ 150 lines, façade only (declarations + `load`/`load_str` + `#[pymodule]`)
- [ ] Every extracted test keeps its assertions verbatim; PHDL fixtures shared via `tests/common/`
- [ ] `cargo test -p piperine-python` count unchanged from baseline
- [ ] `cargo test --workspace` count accounted for

**Tests**: integration (the extracted targets) · **Gate**: full

**Commit**: `test(python): extract binding tests out of the lib façade`

---

### T23: Root test-naming rule + guard

**What**: Adopt and enforce the root naming rule — `host_*.rs` = host-surface proof, `<feature>.rs` = shell/cross-crate proof — with the rule stated in `tests/suite_hygiene.rs`'s `//!` and enforced by a scan.
**Where**: `tests/suite_hygiene.rs` (+ renames of any non-conforming root target)
**Depends on**: T22
**Reuses**: the existing `//!`-header scope check in `suite_hygiene.rs` (MD-31)
**Requirement**: CLA-13 (spec P1 story 3 AC8)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every root target matches the rule (renames listed in the commit body)
- [ ] Guard passes; **failure proof**: a misnamed target added → guard fails naming it; reverted; quoted
- [ ] `cargo test --workspace` = previous + 1

**Tests**: integration + failure proof · **Gate**: full

**Commit**: `test(hygiene): enforce the root test-naming rule`

---

### T24: `SimSession` → `Session` equivalence matrix

**What**: Fill the design doc's appendix: every `SimSession` method mapped to its `Session` counterpart with verdict *identical* / *differs (how)* / *missing (port it)*. No code deleted in this task.
**Where**: `.specs/features/p6-cleanup-architecture/design.md` (appendix)
**Depends on**: T23
**Reuses**: `crates/piperine-api/src/session.rs` (both types), `tests/ngspice_validation.rs` (the staged-path oracle)
**Requirement**: CLA-14 (spec P1 story 4 AC2), design Risks row 1

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every appendix row has a verdict with evidence (`file:line` for each side)
- [ ] Every *differs* row names the exact behavioral difference (fork timing, hooks order, `info.instances` mirror, `compile_disto` gating, nodeset/ic handling)
- [ ] The task report lists which rows require porting work in T25/T26
- [ ] No source file changed

**Tests**: none (analysis task) · **Gate**: build

**Commit**: `docs(p6-arch): map SimSession onto Session`

---

### T25: `SessionBuilder` + ported `Session` capabilities

**What**: Add `SessionBuilder` (`provider`/`hooks`/`disto`/`stage` → `compile`) and port `design()`, `snapshot_digital`, `snapshot_opvars`, `snapshot_introspect` onto `Session`.
**Where**: `crates/piperine-api/src/session.rs`
**Depends on**: T24
**Reuses**: `SimSession::build_circuit` body (hooks order preserved), `SimSession`'s snapshot implementations
**Requirement**: CLA-14 (AC2)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `Session::builder(&design, module).provider(..).hooks(..).disto(true).compile()` works; `Session::compile` remains as the shorthand
- [ ] The four ported methods keep their names, signatures, and `Result`/`Error` shapes
- [ ] New tests cover the builder path (provider, hooks firing order, disto gating) — spec AC-anchored
- [ ] `cargo test --workspace` count = previous + new tests

**Tests**: integration (root `tests/session*.rs`) · **Gate**: full

**Commit**: `feat(api): add SessionBuilder and port SimSession capabilities`

---

### T26: Resolve every *differs* row from the matrix

**What**: Port the behavioral differences T24 found onto `Session` — including unifying the `info.instances` param mirror into one private helper used by both `set` and the sweep paths.
**Where**: `crates/piperine-api/src/session.rs`
**Depends on**: T25
**Reuses**: `SimSession::run_op_sweep`'s mirror logic
**Requirement**: CLA-14 (AC2, AC7)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every matrix row reads *identical*, with the appendix updated
- [ ] The param mirror exists once, not twice
- [ ] `tests/ngspice_validation.rs` passes with numerically unchanged values (compare against T1's recorded output)
- [ ] `cargo test --workspace` at count

**Tests**: integration (ngspice + session suites) · **Gate**: full

**Commit**: `refactor(api): align Session with the staged-path semantics`

---

### T27: Split `session.rs` by role

**What**: Break the file into `session/mod.rs` (declarations), `session/session.rs` (`Session` + builder), `session/sweep.rs` (`Sweep`/`SweepPoint`/`Grid`/`Nested`), `session/config.rs` (`SolverConfig`/`Scale`).
**Where**: `crates/piperine-api/src/session.rs` → `crates/piperine-api/src/session/`
**Depends on**: T26
**Reuses**: —
**Requirement**: CLA-15

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] No file in the new directory exceeds ~700 lines; `mod.rs` ≤ 60 (T19's guard covers it)
- [ ] `piperine-api`'s public re-exports and `prelude` unchanged in name
- [ ] `cargo test --workspace` at count, zero warnings

**Tests**: integration (existing) · **Gate**: full

**Commit**: `refactor(api): split the session surface by role`

---

### T28: Retarget `piperine-python` to `Session`

**What**: Point `_Module`'s per-analysis path, `live.rs`, and `results.rs` at `Session`/`SessionBuilder` instead of `SimSession`, with every Python-facing name unchanged (D6).
**Where**: `crates/piperine-python/src/{module.rs,live.rs,results.rs}`
**Depends on**: T27
**Reuses**: `_Module::session()`'s fork+replay logic (becomes builder configuration)
**Requirement**: CLA-16

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `grep -rn "SimSession" crates/piperine-python` → empty
- [ ] Every Python-facing name/signature/default/return type identical (checked against T1's recorded surface)
- [ ] `cargo test -p piperine-python` and `cargo test --test host_parity` pass
- [ ] `cargo test --workspace` at count

**Tests**: integration (python crate + host_parity) · **Gate**: full

**Commit**: `refactor(python): bind the collapsed Session`

---

### T29: Retarget the plugin host and the root suites

**What**: Point `piperine-plugin/src/host.rs` and the ~24 root targets that construct `SimSession` at `Session`/`SessionBuilder`.
**Where**: `crates/piperine-plugin/src/host.rs`, root `tests/*.rs` (per the grep list in `CLEANUP_PLAN.md` §CL-01)
**Depends on**: T28
**Reuses**: T25's builder
**Requirement**: CLA-16

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every listed target compiles against `Session`, asserting the same values as before
- [ ] `cargo test --workspace` at count, zero warnings
- [ ] `tests/urc_compile_count.rs` still proves compile-once (MD-18)

**Tests**: integration (the retargeted targets) · **Gate**: full

**Commit**: `test,plugin: retarget SimSession call sites to Session`

---

### T30: Delete `SimSession`

**What**: Remove the type and its re-exports; `Session` is the only session surface.
**Where**: `crates/piperine-api/src/session/`, `crates/piperine-api/src/{lib.rs,prelude.rs,hooks.rs}`, `crates/piperine-codegen/src/kernel/analog/kernel.rs` (doc mention)
**Depends on**: T29
**Reuses**: —
**Requirement**: CLA-14 (AC1)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `grep -rn "SimSession" crates src tests` → empty
- [ ] `cargo test --workspace` at count, zero warnings, `cargo doc` clean
- [ ] `tests/host_parity.rs` and `tests/ngspice_validation.rs` green with unchanged numerics

**Tests**: integration (whole workspace) · **Gate**: full

**Commit**: `refactor(api)!: delete SimSession — one host entry point`

---

### T31: `piperine-api::model` descriptors

**What**: Create the descriptor layer — `Port`, `Net`, `Instance`, `Param`, plus `ModelDescriptor`, `TerminalDescriptor`, `ObservableDescriptor`, `ParamDescriptor` — mirroring the Python types field for field.
**Where**: `crates/piperine-api/src/model/{mod.rs,descriptors.rs}`
**Depends on**: T30
**Reuses**: `crates/piperine-python/src/{module.rs,instance.rs}`'s existing descriptor bodies (moved, not rewritten)
**Requirement**: CLA-17

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] All eight types exist with the same fields/accessors the Python side exposes
- [ ] `model/mod.rs` ≤ 60 lines
- [ ] Tests cover each descriptor's accessors against a fixture design
- [ ] `cargo test --workspace` = previous + new tests

**Tests**: integration (new root target from T36 grows here) · **Gate**: full

**Commit**: `feat(api): add the model descriptor layer`

---

### T32: `piperine-api::model::Design`

**What**: `Design`/`Selection`/`Node` with `load`, `load_str`, `top`, `module`, `modules`, `const_`, `select`, mirroring `_Design`.
**Where**: `crates/piperine-api/src/model/design.rs`
**Depends on**: T31
**Reuses**: `crates/piperine-python/src/design.rs` bodies; `piperine_lang::parse_and_elaborate`; the POM selector
**Requirement**: CLA-17

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every `_Design` method has an api counterpart with the same semantics (including `infer_top`'s rule)
- [ ] `Design::modules` is the only hierarchy surface — `flat_modules` is never exposed (MD-25); an attempt fails loud
- [ ] Tests cover load/top/module/modules/const_/select + the MD-25 fail-loud case
- [ ] `cargo test --workspace` = previous + new tests

**Tests**: integration · **Gate**: full

**Commit**: `feat(api): lift the Design object model`

---

### T33: `piperine-api::model::Module`

**What**: `Module` with navigation (`name`/`ports`/`nets`/`instances`/`params`/`behaviors`), the analysis menu, `set` (staging), and `compile` → `Session`.
**Where**: `crates/piperine-api/src/model/module.rs`
**Depends on**: T32
**Reuses**: `crates/piperine-python/src/module.rs` bodies; T25's `SessionBuilder`
**Requirement**: CLA-17

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Navigation + every analysis + `set` + `compile` present, each delegating to `SessionBuilder`/`Session`
- [ ] Staged overrides stay isolated from the parent design (the `_Module` guarantee)
- [ ] Tests cover navigation, one analysis per family, staging isolation, and `compile()` returning a working `Session`
- [ ] `cargo test --workspace` = previous + new tests

**Tests**: integration · **Gate**: full

**Commit**: `feat(api): lift the Module object model`

---

### T34: `piperine-api::model::InstanceView`

**What**: Extend the existing `InstanceView` (today in `results.rs`) to the full Python surface — `label`, `terminal_connections`, `v`, `i`, `opvar`, `opvars`, `model`, `terminals`, `observables`, `param`, `params` — and move it into `model/instance.rs`.
**Where**: `crates/piperine-api/src/{results.rs,model/instance.rs}`
**Depends on**: T33
**Reuses**: existing `InstanceView` + `crates/piperine-python/src/instance.rs` bodies
**Requirement**: CLA-17

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] One `InstanceView` in the tree, in `model/instance.rs`, with the full surface
- [ ] `results.rs` re-exports it so existing host call sites keep compiling
- [ ] Tests cover `v`/`i`/`opvar`/`model`/`terminals`/`observables`/`param`
- [ ] `cargo test --workspace` = previous + new tests

**Tests**: integration · **Gate**: full

**Commit**: `feat(api): complete InstanceView and give it a home`

---

### T35: Python model files become pure delegation

**What**: Rewrite `design.rs`, `module.rs`, `instance.rs` so every method is a one-line forward to the api model — no POM traversal, no analysis logic, `PyErr` mapping only.
**Where**: `crates/piperine-python/src/{design.rs,module.rs,instance.rs}`
**Depends on**: T34
**Reuses**: `crates/piperine-api/src/results.rs`'s delegation style as the template
**Requirement**: CLA-18

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] No `piperine_lang::pom` traversal or `CircuitCompiler` use remains in the three files
- [ ] Every Python-facing name/signature/default/return identical (D6), checked against T1's recorded surface
- [ ] `cargo test -p piperine-python` and `tests/host_parity.rs` pass
- [ ] `cargo test --workspace` at count

**Tests**: integration (python crate + host_parity) · **Gate**: full

**Commit**: `refactor(python): delegate the object model to piperine-api`

---

### T36: Rust-side object-model proof + parity extension

**What**: New root target driving the whole lifted path in Rust (load → module → analysis → instance view → opvar → compile → live `set`), and extend `host_parity.rs` to enumerate the lifted model on both sides.
**Where**: `tests/host_object_model.rs` (new), `tests/host_parity.rs`
**Depends on**: T35
**Reuses**: `tests/host_parity.rs`'s enumeration mechanism
**Requirement**: CLA-19 (spec P1 story 5 AC4, AC5)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] The new target proves each lifted capability without Python, with a `//!` scope header
- [ ] `host_parity.rs` fails if either side gains a model method the other lacks (proven by injecting a Rust-only method; reverted; quoted)
- [ ] `cargo test --workspace` = previous + new tests

**Tests**: integration + failure proof · **Gate**: full

**Commit**: `test(api): prove the lifted object model on both hosts`

---

### T37: `ExprBuilder` — expression construction gets an owner

**What**: Absorb `select`, `binary`, `lit`, `not_expr`, `and_guards`, `subst_expr`, `subst_block`, `subst_scope`, `substitute_marker` into one owning builder (or inherent constructors on the resolved `Expr`).
**Where**: `crates/piperine-codegen/src/flatten/analog.rs`, `crates/piperine-codegen/src/emit/builder.rs`, `crates/piperine-codegen/src/resolve/`
**Depends on**: T36
**Reuses**: the existing free-function bodies verbatim
**Requirement**: CLA-20

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] None of the nine names is a module-level `fn` any more
- [ ] `cargo test -p piperine-codegen` at count, zero warnings
- [ ] `tests/ngspice_validation.rs` numerics unchanged

**Tests**: integration (codegen + ngspice) · **Gate**: full

**Commit**: `refactor(codegen): own expression construction in a builder`

---

### T38: Expression query surface — one structural-equality algorithm

**What**: Turn `has_branch_current`, `has_branch_access`, `has_marker`, `expr_eq`, `expr_structural_eq`, `blocks_eq`, `stmts_eq`, `isolate_branch_coeff`, `collect_branch_current_pairs`, `zero_branch_currents` into inherent methods (or one query trait), unifying the four equality functions into one algorithm.
**Where**: `crates/piperine-codegen/src/resolve/`, `crates/piperine-codegen/src/emit/cse.rs`, `crates/piperine-codegen/src/flatten/analog.rs`
**Depends on**: T37
**Reuses**: existing bodies; `emit/cse.rs` is the only consumer needing the structural variant
**Requirement**: CLA-21

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] One equality implementation remains, reachable as a method
- [ ] None of the ten names is a module-level `fn`
- [ ] `cargo test -p piperine-codegen` at count; CSE-sensitive tests (`analog_device_numerics.rs`, `silent_bugs.rs`) green
- [ ] ngspice numerics unchanged

**Tests**: integration (codegen + ngspice) · **Gate**: full

**Commit**: `refactor(codegen): give expression queries an owner`

---

### T39: `LimitCollector`

**What**: Fold `collect_limits`, `limit_branch`, `limit_branches_into`, `ident_of` into one owning collector type in the existing limits module.
**Where**: `crates/piperine-codegen/src/kernel/analog/{compile.rs,limits.rs}`
**Depends on**: T38
**Reuses**: `kernel/analog/limits.rs` (58 lines — the natural home)
**Requirement**: CLA-22

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] The four names are gone as free functions; one collector owns the walk
- [ ] `$limit` limiter naming still resolves per slot (the PIA-15..18 catalog behavior)
- [ ] `cargo test -p piperine-codegen` at count; `tests/host_limiting.rs` green

**Tests**: integration (codegen + host_limiting) · **Gate**: full

**Commit**: `refactor(codegen): collect $limit slots in one owner`

---

### T40: Triage codegen's remaining module-level functions

**What**: Give every remaining module-level `fn` in `piperine-codegen` an owner or a `// hygiene-exempt: <reason>` line.
**Where**: `crates/piperine-codegen/src/**`
**Depends on**: T39
**Reuses**: T37–T39's owners
**Requirement**: CLA-20 (AC4)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every module-level `fn` in the crate is owned or exempt-with-reason
- [ ] The exemption count is reported in the task summary (visible debt, not hidden)
- [ ] `cargo test -p piperine-codegen` at count, zero warnings

**Tests**: integration (existing) · **Gate**: quick

**Commit**: `refactor(codegen): give every helper an owner`

---

### T41: Guard — module-level functions have owners (codegen scope)

**What**: Add `module_level_fns_have_owners`, scoped to `piperine-codegen` first, failing with file:line + fn name for any unowned, unexempted module-level `fn`.
**Where**: `tests/suite_hygiene.rs`
**Depends on**: T40
**Reuses**: the T7/T10/T19 scan helper
**Requirement**: CLA-28

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Guard passes for `piperine-codegen`; its crate scope list is a table in the test
- [ ] **Failure proof**: an unexempted module-level `fn` added → guard fails naming it; reverted; quoted
- [ ] `cargo test --workspace` = previous + 1

**Tests**: integration + failure proof · **Gate**: full

**Commit**: `test(hygiene): guard module-level function ownership`

---

### T42: Decompose `lower_bodies`

**What**: Break the 253-line `lower_bodies` into named phase methods on the lowering type.
**Where**: `crates/piperine-codegen/src/resolve/pom/body.rs`
**Depends on**: T41
**Reuses**: `analyses/transient.rs`'s phase-method pattern
**Requirement**: CLA-23

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] No method in the file exceeds 60 lines
- [ ] Every extracted piece is a method, not a new free function
- [ ] `cargo test -p piperine-codegen` at count; ngspice numerics unchanged

**Tests**: integration (codegen + ngspice) · **Gate**: full

**Commit**: `refactor(codegen): decompose lower_bodies into phases`

---

### T43: Decompose `extract_symbols`

**What**: Break the 207-line `extract_symbols` into one method per symbol kind, owned by the handler or `SymbolIndex`.
**Where**: `crates/piperine-lang-server/src/handlers/symbols.rs`
**Depends on**: T42
**Reuses**: `SymbolIndex`, `DocumentState`
**Requirement**: CLA-23

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] No function in the file exceeds 60 lines; no new free function
- [ ] `cargo test -p piperine-lang-server` at count (the nine feature suites)

**Tests**: integration (lang-server suites) · **Gate**: quick

**Commit**: `refactor(lang-server): decompose symbol extraction`

---

### T44: Decompose `parse_primary` (dispatch-only)

**What**: Turn the 316-line `parse_primary` into a dispatch whose every match arm is a single call to a named `Parser` method (D7). Grammar unchanged.
**Where**: `crates/piperine-lang/src/parse/parser/expr.rs`
**Depends on**: T43
**Reuses**: existing `Parser` methods
**Requirement**: CLA-24

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every arm is one call; each extracted piece is a `Parser` method
- [ ] Frozen corpora parse identically: `cargo test -p piperine-lang` at count, `headers/`/`tests/fixtures*` untouched
- [ ] `cargo test --workspace` at count
- [ ] If any parse result changes, the task is reverted (spec Edge Case)

**Tests**: integration (frozen-corpus parse/elab suites) · **Gate**: full

**Commit**: `refactor(lang): make parse_primary a dispatch`

---

### T45: Decompose `parse_mod_stmt` (dispatch-only)

**What**: Same treatment for the 212-line `parse_mod_stmt`.
**Where**: `crates/piperine-lang/src/parse/parser/stmt.rs`
**Depends on**: T44
**Reuses**: T44's pattern
**Requirement**: CLA-24

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every arm is one call to a named method
- [ ] Frozen corpora identical; `cargo test --workspace` at count
- [ ] Revert-on-difference rule honored

**Tests**: integration (frozen-corpus suites) · **Gate**: full

**Commit**: `refactor(lang): make parse_mod_stmt a dispatch`

---

### T46: Bring the 100–200 band under the ceiling + guard 200 lines

**What**: Trim the remaining >100-line functions where it costs nothing, then add `no_function_over_200_lines` (brace-balance scan over `crates/*/src`).
**Where**: the twelve functions listed in `design.md` §C5; **plus the three >200-line functions the original census missed** (`baseline.md` §5, found in Phase 1): `crates/piperine-codegen/src/kernel/analog/compile.rs::compile` (581 lines — the largest function in the workspace), `crates/piperine-lang/src/elab/lower/module.rs::lower_mod_stmt` (223), `crates/piperine-lang-server/src/symbol_index.rs::resolve_at` (215); `tests/suite_hygiene.rs`

**Scope note (added by the orchestrator after Phase 1):** the feature brief said
four functions exceed 200 lines; the measured count is **seven**. The three above
were in no task's `Where`, so T46 could not have passed as written. They are now
in scope. `compile.rs::compile` at 581 lines is the heaviest single item in this
phase — decompose it into named phase methods (the `analyses/transient.rs`
pattern) and keep `analog_device_numerics.rs`/`silent_bugs.rs` plus the ngspice
numerics as the oracle.
**Depends on**: T45
**Reuses**: T7/T10/T19/T41's scan helper
**Requirement**: CLA-25

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] No function in `crates/*/src` exceeds 200 lines
- [ ] Guard passes; **failure proof**: a 201-line function injected → guard fails naming file:line + fn + length; reverted; quoted
- [ ] `cargo test --workspace` = previous + 1, zero warnings

**Tests**: integration + failure proof · **Gate**: full

**Commit**: `refactor,test: cap function length at 200 lines`

---

### T47: `piperine-lang` free-function ownership pass

**What**: Give each of `piperine-lang`'s ~75 module-level functions an owner (parser/elaborator/interpreter method) or a `// hygiene-exempt:` reason.
**Where**: `crates/piperine-lang/src/**`
**Depends on**: T46
**Reuses**: the crate's existing types as owners (`Parser`, `Elaborator`, `Interpreter`, `Lexer`)
**Requirement**: CLA-26

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every module-level `fn` owned or exempt-with-reason; exemption count reported
- [ ] Frozen corpora identical; `cargo test -p piperine-lang` at count
- [ ] `cargo test --workspace` at count, zero warnings

**Tests**: integration (lang suites) · **Gate**: full

**Commit**: `refactor(lang): give every helper an owner`

---

### T48: `piperine-lang-server` free-function ownership pass

**What**: Same for the ~66 module-level functions, owned by `DocumentState`, `SymbolIndex`, `ProjectContext`, or the handler type they serve.
**Where**: `crates/piperine-lang-server/src/**`
**Depends on**: T47
**Reuses**: `DocumentState::{analyze,resolve_at,word_occurrences}`, `SymbolIndex`, `ProjectContext::discover`
**Requirement**: CLA-27

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every module-level `fn` owned or exempt-with-reason; exemption count reported
- [ ] `cargo test -p piperine-lang-server` at count (all nine suites)
- [ ] Every LSP request id still receives a response (the existing protocol tests prove it)

**Tests**: integration (lang-server suites) · **Gate**: quick

**Commit**: `refactor(lang-server): give every helper an owner`

---

### T49: Extend the ownership guard to `lang` and `lang-server`

**What**: Widen `module_level_fns_have_owners`' crate table to include `piperine-lang` and `piperine-lang-server`.
**Where**: `tests/suite_hygiene.rs`
**Depends on**: T48
**Reuses**: T41's guard
**Requirement**: CLA-28 (AC3)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] The guard covers three crates, each an explicit table row (exhaustiveness, not a wildcard)
- [ ] **Failure proof**: an unexempted module-level `fn` added in each newly covered crate fails the guard; reverted; quoted
- [ ] `cargo test --workspace` at count

**Tests**: integration + failure proof · **Gate**: full

**Commit**: `test(hygiene): extend ownership guard to lang and lang-server`

---

### T50: Record the decisions (MD-33/34/35 + amendments)

**What**: Append MD-33 (no file-scope lint suppression), MD-34 (`mod.rs` declares, never implements), MD-35 (comments describe the present) to `.specs/STATE.md`, each naming its guard; amend MD-03 (superseded, D2), MD-20 (`Session` is the single host entry, D1), MD-22/MD-27 (object model api-canonical, D3); update the Handoff Snapshot; mark `CLEANUP_PLAN.md` delivered with per-finding status.
**Where**: `.specs/STATE.md`, `CLEANUP_PLAN.md`, `ROADMAP.md` (P6 line)
**Depends on**: T49
**Reuses**: the guards added in T7/T10/T19/T23/T41/T46/T49 as the enforcement citations
**Requirement**: CLA-29, CLA-30

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Three new MD entries present, each citing its enforcing test by file + test name
- [ ] MD-03/MD-20/MD-22/MD-27 amended with dates and the D1/D2/D3 rationale
- [ ] `CLEANUP_PLAN.md` lists every `CL-nn` with its final status; `ROADMAP.md` P6 reflects the closed subset
- [ ] Handoff Snapshot updated with the final test count
- [ ] `cargo test --workspace` at count

**Tests**: none (docs layer) · **Gate**: build

**Commit**: `docs(p6-arch): lock MD-33/34/35 and amend MD-03/20/22/27`

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8 → Phase 9

Phase 1:  T1 → T2 → T3 → T4 → T5 → T6 → T7
Phase 2:  T8 → T9 → T10 → T11 → T12 → T13
Phase 3:  T14 → T15 → T16 → T17 → T18 → T19
Phase 4:  T20 → T21 → T22 → T23
Phase 5:  T24 → T25 → T26 → T27 → T28 → T29 → T30
Phase 6:  T31 → T32 → T33 → T34 → T35 → T36
Phase 7:  T37 → T38 → T39 → T40 → T41
Phase 8:  T42 → T43 → T44 → T45 → T46
Phase 9:  T47 → T48 → T49 → T50
```

Strictly sequential — 50 tasks, 9 phases. Batch packing at ~7 tasks per worker
lands on phase boundaries: **9 batches** (P1, P2, P3, P4, P5, P6, P7, P8, P9).

---

## Task Granularity Check

| Task | Scope | Status |
|---|---|---|
| T1 | measurement only | ✅ |
| T2 | 2 file deletions, same concern | ✅ |
| T3, T4 | 4 same-shaped deletions each | ✅ |
| T5 | 9 dead items in one module family | ✅ (cohesive) |
| T6 | 12+7 suppression sites, one rule | ✅ (cohesive) |
| T7, T10, T19, T23, T41, T46, T49 | 1 guard each | ✅ |
| T8, T9 | doc-comment rewrites, one concern each | ✅ |
| T11, T12, T13 | 1 doc each (T13: status + manual decision) | ✅ |
| T14, T15 | 1 module relocation each | ✅ |
| T16, T17, T18, T19 | 1 file rename/split each | ✅ |
| T20, T21, T22 | 1 test relocation each | ✅ |
| T24 | 1 analysis artifact | ✅ |
| T25, T26, T27, T28, T29, T30 | 1 step of the collapse each | ✅ |
| T31–T36 | 1 model file / 1 proof target each | ✅ |
| T37, T38, T39 | 1 abstraction each | ✅ |
| T40, T47, T48 | 1 crate's triage each | ⚠️ large but single-rule, cohesive |
| T42–T45 | 1 function each | ✅ |
| T50 | 1 decisions-record change set | ✅ |

No ❌ — T40/T47/T48 are wide but apply one mechanical rule per crate, which is
the only cut that keeps the guard-widening honest.

---

## Diagram-Definition Cross-Check

| Task | Depends on (body) | Diagram shows | Status |
|---|---|---|---|
| T1 | None | start of Phase 1 | ✅ |
| T2..T7 | T1..T6 respectively | T1→T2→T3→T4→T5→T6→T7 | ✅ |
| T8..T13 | T7..T12 respectively | T8→T9→T10→T11→T12→T13 | ✅ |
| T14..T19 | T13..T18 respectively | T14→…→T19 | ✅ |
| T20..T23 | T19..T22 respectively | T20→T21→T22→T23 | ✅ |
| T24..T30 | T23..T29 respectively | T24→…→T30 | ✅ |
| T31..T36 | T30..T35 respectively | T31→…→T36 | ✅ |
| T37..T41 | T36..T40 respectively | T37→…→T41 | ✅ |
| T42..T46 | T41..T45 respectively | T42→…→T46 | ✅ |
| T47..T50 | T46..T49 respectively | T47→T48→T49→T50 | ✅ |

Every dependency points backward; no task depends on a later phase.

---

## Test Co-location Validation

| Task | Layer modified | Matrix requires | Task says | Status |
|---|---|---|---|---|
| T1 | none (measurement) | none | none | ✅ |
| T2–T5 | solver/codegen internals | integration (existing) | integration | ✅ |
| T6 | whole workspace internals | integration (existing) | integration | ✅ |
| T7, T10, T19, T23, T41, T46, T49 | policy guards | integration + failure proof | integration + failure proof | ✅ |
| T8, T9, T11, T12, T13 | docs | none (build gate) | none | ✅ |
| T14–T18 | solver/codegen internals | integration (existing) | integration | ✅ |
| T20–T22 | test placement | integration (moved targets) | integration | ✅ |
| T24 | analysis artifact (docs) | none | none | ✅ |
| T25 | host api surface | integration + new AC tests | integration | ✅ |
| T26, T27, T29, T30 | host api surface | integration | integration | ✅ |
| T28, T35 | python bindings | integration (crate + parity) | integration | ✅ |
| T31–T34 | host api surface (new) | integration, AC-anchored | integration | ✅ |
| T36 | host api + parity guard | integration + failure proof | integration + failure proof | ✅ |
| T37–T40 | codegen internals | integration (existing + ngspice) | integration | ✅ |
| T42, T43 | codegen / lang-server internals | integration (existing) | integration | ✅ |
| T44, T45 | lang `parse/` (frozen corpora) | integration (frozen corpus) | integration | ✅ |
| T47, T48 | lang / lang-server internals | integration (existing) | integration | ✅ |
| T50 | docs | none | none | ✅ |

No ❌ VIOLATION. Doc-layer `Tests: none` is matrix-sanctioned; every task that
touches code carries the covering suite as its gate, and the four tasks that add
surface (T25, T31–T34, T36) carry new AC-anchored tests.
