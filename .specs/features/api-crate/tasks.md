# api-crate Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name
and follow its Execute flow and Critical Rules.** If the skill cannot be
activated, STOP.

---

**Design**: `.specs/features/api-crate/design.md`
**Status**: Approved — ready to execute

---

## Test Coverage Matrix

> Guidelines found: `CLAUDE.md` (zero warnings, always `--workspace`; bare
> `cargo test` at root runs only the root package). Baseline at feature
> start: **449 passed / 5 ignored** (bench-removal closure + round-4 fixes).

| Code Layer | Test Type | Coverage Expectation | Location | Run Command |
|---|---|---|---|---|
| piperine-api (moved host API) | integration | existing root suites keep identical assertions; one new smoke importing `piperine_api::` directly + `cargo tree` dependency-set assertion | root `tests/` + `crates/piperine-api/tests/` | `cargo test -p piperine-api -p piperine` |
| root shell | integration | root `tests/` compile through `use piperine::…` unchanged (parity proof) | root `tests/` | `cargo test -p piperine` |
| piperine-python retarget | e2e | python suites + 22 examples green, shapes unchanged | existing python tests + run_examples | `cargo test -p piperine-python` + full |
| piperine-cli retarget | integration | cli suites green (test_tb 11 tests incl. run behaviors) | `crates/piperine-cli/tests/` | `cargo test -p piperine-cli` |
| docs | none | build gate only | — | build |

## Gate Check Commands

| Gate | Command |
|---|---|
| Quick | `cargo test -p <crate>` |
| Full | `cargo test --workspace` |
| Build | `cargo build --workspace` (zero warnings) + `cargo test --workspace` |

---

## Execution Plan

### Phase 1: Extraction (T1 → T2 → T3)

```
T1 → T2 → T3
```

---

## Task Breakdown

### T1: Create `piperine-api` and move the host API

**What**: New crate `crates/piperine-api` (workspace member). `git mv` root
`src/{session,results,waveform,hooks,error,prelude}.rs` into it; its
`lib.rs` = the old root `lib.rs` module tree + crate doc. Copy root's dep
list (lang/codegen/solver/thiserror/num-complex — exact set). Root `src/lib.rs`
becomes `pub use piperine_api::*;` + doc; root deps shrink to `piperine-api`
(+ dev-deps for `tests/`). Add `crates/piperine-api/tests/smoke.rs`: op
analysis through `use piperine_api::…` + assert `cargo tree -p piperine-api`
has no python/cli/project edge (run via `std::process::Command` or keep as a
plain compile-time import + manifest review in Done-when).
**Where**: `crates/piperine-api/`, root `src/lib.rs`, root `Cargo.toml`
**Depends on**: None · **Requirement**: API-01, API-02, API-03
**Done when**:
- [ ] Root `src/` contains only `lib.rs` (shell)
- [ ] `piperine-api/tests/smoke.rs` green (direct `piperine_api::` op run)
- [ ] `cargo tree -p piperine-api` deps = lang/codegen/solver only (no
      python/cli/project) — asserted or manually recorded in commit body
- [ ] Root `tests/` (session, ngspice, spice_smoke, compile_once,
      run_examples) green **unchanged** via `use piperine::…`
- [ ] Gate full
**Tests**: integration · **Gate**: full
**Commit**: `refactor(api)!: extract piperine-api; root becomes a re-export shell`

### T2: Retarget `piperine-python` and `piperine-cli`

**What**: python `Cargo.toml` → `piperine-api` path dep (drop root);
`use piperine::` → `use piperine_api::` across `crates/piperine-python/src/`
and any cli host-API import. Root must no longer appear in
`cargo tree -p piperine-python` / `-p piperine-cli` (except the cli's
dev/test use of the binary, which is its own package).
**Where**: `crates/piperine-python/`, `crates/piperine-cli/`
**Depends on**: T1 · **Requirement**: API-05, API-06
**Done when**:
- [ ] `cargo tree -p piperine-python -i piperine` (root package) → no match
- [ ] `cargo test -p piperine-python` green (facade/live/smoke + hygiene)
- [ ] `cargo test -p piperine-cli` green (11 tests)
- [ ] Gate full (449+ passed, zero warnings)
**Tests**: e2e + integration · **Gate**: full
**Commit**: `refactor(python,cli): host API from piperine-api`

### T3: Docs + closure

**What**: `CLAUDE.md` crate table (+pipeline hosts line) names
`piperine-api`; root described as re-export shell; part VIII Rust-face
section updated; README if it names the root lib. Traceability → Verified in
spec.md; STATE.md handoff snapshot updated.
**Where**: `CLAUDE.md`, `docs/spec/part_viii_host_api.md`, `README.md`,
`.specs/`
**Depends on**: T2 · **Requirement**: API-07, API-04 (final gate)
**Done when**:
- [ ] Grep: no doc claims the root crate *hosts* the library code
- [ ] Gate build (zero warnings, full workspace green)
**Tests**: none (docs) · **Gate**: build
**Commit**: `docs: piperine-api is the library face (MD-20)`

---

## Phase Execution Map

```
Phase 1: T1 → T2 → T3
```

Single batch (3 tasks) — inline execution, no sub-agents.

## Diagram-Definition Cross-Check

| Task | Depends (body) | Diagram | Status |
|---|---|---|---|
| T1 none · T2 T1 · T3 T2 | backward-only | T1→T2→T3 | ✅ all |

## Test Co-location Validation

| Task | Layer | Matrix | Task Says | Status |
|---|---|---|---|---|
| T1 api crate + shell | integration | integration | integration | ✅ |
| T2 python/cli | e2e+integration | e2e+integration | e2e+integration | ✅ |
| T3 docs | none | none | none | ✅ |
