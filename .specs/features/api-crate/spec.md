# api-crate Specification

Extract the host API from the root `piperine` crate into a dedicated
`crates/piperine-api` (pure Rust), per **MD-20**. The root crate becomes a
thin re-export shell (`pub use piperine_api::*`) so Rust hosts keep
`use piperine::…`; the `piperine` binary stays in `piperine-cli`.

## Problem Statement

MD-19 put the host library (session/results/waveform/hooks) in the root
crate's `src/`. The user is dissatisfied with library code living in the
repository root: the external interface deserves a named crate, and the root
should be a shell. P2 (device ABI), P3 (python polish), and P5 (plugin
simplification) all build on this topology — it must land first.

## Goals

- [ ] `crates/piperine-api` exists and owns the host API: `SimSession`,
      `SolverConfig`, result objects, `Waveform` family, `SimHooks`, `Error`,
      `prelude`.
- [ ] Root `piperine` crate is a re-export shell only — zero own code beyond
      `pub use piperine_api::*` (+ crate doc).
- [ ] `piperine-python` and `piperine-cli` depend on `piperine-api` directly.
- [ ] Every existing guard stays green with identical assertions (session,
      ngspice live, spice smoke, compile-once, run_examples, python suites).

## Out of Scope

| Feature | Reason |
|---------|--------|
| Moving device/plugin ABI traits (`DeviceProvider`, plugin contracts) into the api crate | Deferred to P2/P5 features (MD-20 note) — avoids churning codegen/plugin now |
| Any behavior change in session/results/waveform | Pure relocation |
| Python facade changes | P3 feature |
| Workspace-wide crate renames beyond the new crate | Not requested |

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| Root package shape | Thin re-export lib (`pub use piperine_api::*`), no bin; bin stays in `piperine-cli` | User choice 2026-07-18 (option "re-export fino"); Rust hosts keep `use piperine::…` | y (user) |
| Where the host-API tests live | Stay in root `tests/`, importing through `piperine::…` | Proves the re-export shell exposes the full surface; zero test churn | n (agent) |
| Root dev-dependency on `piperine-python` | Kept (run_examples needs the embedded host); legal since python no longer depends on root | Dev-deps don't create package cycles | n (agent) |
| `piperine-python` dependency edge | `piperine-api = { path = "../piperine-api" }`, drop root dep | MD-20 flow `python → api` | y (MD-20) |
| Docs touched | `CLAUDE.md` crate table, `docs/spec/part_viii_host_api.md` Rust-face wording, `README` if it names the root lib | Keep the record consistent with MD-20 | n (agent) |

**Open questions:** none — all resolved or logged above.

**Implicit-dimension sweep (Large):** input validation N/A (no new inputs);
failure states N/A (pure move — compile errors are the failure mode);
idempotency/retry N/A; auth N/A; concurrency N/A (no runtime change); data
lifecycle N/A; observability N/A (no runtime change); external-dependency
failure N/A; state transitions N/A. The only real dimension is **dependency
topology integrity** → covered by API-02/API-03.

## User Stories

### P1: The host API lives in `piperine-api` ⭐ MVP

**User Story**: As a maintainer, I want the external Rust interface in a
named crate so the repository root is a shell and future pillars (P2/P3/P5)
have a home for contracts.

**Acceptance Criteria**:

1. WHEN the workspace builds THEN `crates/piperine-api` SHALL contain
   `session.rs`, `results.rs`, `waveform.rs`, `hooks.rs`, `error.rs`,
   `prelude.rs` (moved, not copied — root `src/` keeps none of them).
2. WHEN a Rust host writes `use piperine_api::{SimSession, SolverConfig}`
   THEN it SHALL compile and run analyses exactly as the root crate did
   (op/tran/ac/noise, staging, hooks).
3. WHEN `cargo tree -p piperine-api` runs THEN its dependencies SHALL be
   exactly {piperine-lang, piperine-codegen, piperine-solver} (+ external
   crates) — no python, no cli, no project.

**Independent Test**: a root test importing through `piperine_api::` runs an
op analysis; `cargo tree` output asserted.

### P1: Root is a thin re-export shell ⭐ MVP

**Acceptance Criteria**:

1. WHEN root `src/lib.rs` is inspected THEN it SHALL contain only
   `pub use piperine_api::*;` (+ crate-level doc comment) — no modules, no
   types of its own.
2. WHEN existing hosts write `use piperine::{SimSession, NetRef, prelude}`
   THEN they SHALL compile unchanged (the shell re-exports the full public
   surface, including the `prelude` module path).
3. WHEN the migrated guards run (root `tests/`: session, ngspice incl. live
   goldens, spice smoke, compile-once, run_examples; python suites; cli
   suites) THEN all SHALL pass with assertion content unchanged.

**Independent Test**: root `tests/session.rs` untouched and green through
`use piperine::…`.

### P1: Consumers retarget ⭐ MVP

**Acceptance Criteria**:

1. WHEN `piperine-python` builds THEN its Cargo dependency SHALL be
   `piperine-api` (root dep removed); result shapes unchanged.
2. WHEN `piperine-cli` builds THEN host-API imports SHALL come from
   `piperine-api` (directly or via the shell — direct preferred); CLI
   behavior unchanged.
3. WHEN the workspace builds THEN it SHALL emit zero warnings and the
   binary `piperine` SHALL still come from `piperine-cli`.

**Independent Test**: `cargo tree -p piperine-python -i piperine` (root
package) empty; full gate green.

## Edge Cases

- WHEN a downstream crate references a root-crate item that existed pre-move
  THEN the re-export SHALL cover it (no silently-dropped public item — the
  shell is `pub use *`, and the gate compiles every in-repo consumer).
- WHEN docs (`CLAUDE.md`, part VIII) name the root crate as the library face
  THEN they SHALL be updated to name `piperine-api` (+ shell note).

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
|---|---|---|---|
| API-01 | P1 api crate owns host API | - | Pending |
| API-02 | P1 api dependency set exact | - | Pending |
| API-03 | P1 root is pure re-export | - | Pending |
| API-04 | P1 guards green unchanged | - | Pending |
| API-05 | P1 python on api | - | Pending |
| API-06 | P1 cli on api; bin unchanged | - | Pending |
| API-07 | P1 docs updated (MD-20) | - | Pending |

**Coverage:** 7 total, 0 mapped to tasks (mapping happens in tasks.md).

## Success Criteria

- [ ] `cargo build --workspace` zero warnings; `cargo test --workspace`
      green at the closure baseline (449 passed / 5 ignored, ± new tests).
- [ ] Root `src/` = `lib.rs` shell only.
- [ ] `cargo tree -p piperine-api` shows the exact {lang, codegen, solver}
      dependency set.
