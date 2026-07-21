# api-crate Validation

**Date**: 2026-07-18
**Spec**: `.specs/features/api-crate/spec.md`
**Diff range**: `8e4fd77..bdd4e77` (445fd8c extraction, d231b8e retarget, bdd4e77 docs)
**Verifier**: independent sub-agent (author ≠ verifier), evidence-or-zero

---

## Task Completion

| Task | Status | Notes |
| ---- | ------ | ----- |
| T1 | ✅ Done | api crate + shell + smoke tests, verified below |
| T2 | ⚠️ Done, one inaccurate body claim | Python retarget verified; cli has no host-API imports (vacuous). Body claim "Root must no longer appear in `cargo tree -p piperine-cli`" is **false** — root appears via the pre-existing `piperine-plugin → piperine` edge (`crates/piperine-plugin/Cargo.toml:12`, `crates/piperine-plugin/src/host.rs:7` `use piperine::SimHooks`). Spec AC allows "via the shell" and plugin retarget is out of scope (deferred P5), so no AC violation — but the task-body sentence over-claims. |
| T3 | ⚠️ Done with residue | CLAUDE.md, part VIII, index updated. `README.md:27` ("the Rust root crate drive analyses") and `docs/spec/part_iii_interpreted_context.md:10` ("Rust (the root `piperine` crate)") still name the root crate as the Rust host. Still factually true (shell re-exports), but T3's "README if it names the root lib" was not fully executed. Minor. |

---

## Spec-Anchored Acceptance Criteria

### P1: The host API lives in `piperine-api`

| Criterion | Spec-defined outcome | Evidence | Result |
| --- | --- | --- | --- |
| AC1: api crate contains the six moved files; root keeps none | moved, not copied | `ls crates/piperine-api/src/` → `error.rs hooks.rs lib.rs prelude.rs results.rs session.rs waveform.rs`; `ls src/` → `lib.rs` only. Diffstat shows `{src => crates/piperine-api/src}/…` renames (0-line deltas for error/hooks/results/session/waveform; prelude.rs 2-line doc edit). | ✅ PASS |
| AC2: `use piperine_api::{SimSession, SolverConfig}` compiles and runs analyses | op solves as before | `crates/piperine-api/tests/smoke.rs:5` `use piperine_api::{NetRef, SimSession, SolverConfig};`; `:45` `assert!((mid - 5.0).abs() < 1e-9)` — test green in the full gate. | ✅ PASS |
| AC3: api deps exactly {lang, codegen, solver} | no python/cli/project | `cargo tree -p piperine-api --prefix none -e normal` (run by verifier): piperine entries are exactly `piperine-codegen v0.1.1`, `piperine-lang v0.1.1`, `piperine-solver v0.1.1`; no python/cli/project/plugin. Asserted in-repo at `crates/piperine-api/tests/smoke.rs:51-65` (`dependency_set_is_lang_codegen_solver_only`, forbidden list includes `piperine-plugin`). | ✅ PASS |

### P1: Root is a thin re-export shell

| Criterion | Spec-defined outcome | Evidence | Result |
| --- | --- | --- | --- |
| AC1: root `src/lib.rs` = `pub use piperine_api::*;` + doc only | zero own code | `src/lib.rs:9` `pub use piperine_api::*;` — the only item; lines 1–7 are crate doc. `src/` holds no other file. | ✅ PASS |
| AC2: `use piperine::{SimSession, NetRef, prelude}` compiles unchanged | full surface incl. `prelude` module path | Root suites: `tests/session.rs:7`, `tests/spice_smoke.rs:9`, `tests/ngspice_validation.rs:19`, `tests/compile_once_sweep.rs:8` all `use piperine::{…}` and are green. `prelude` module path proven by verifier scratch probe (`use piperine::prelude::{SimSession, …}` + `use piperine::{…, prelude}` → `test result: ok. 1 passed`); probe removed, tree clean. | ✅ PASS |
| AC3: migrated guards pass with assertion content unchanged | zero test churn | `git diff 8e4fd77..bdd4e77 -- tests/` → **0 lines**. Full gate green (below). Only non-root test touched: `crates/piperine-cli/tests/test_tb.rs` (removed now-unused `PathBuf` import — no assertion change). | ✅ PASS |

### P1: Consumers retarget

| Criterion | Spec-defined outcome | Evidence | Result |
| --- | --- | --- | --- |
| AC1: python dep is `piperine-api`; root dep removed | `cargo tree -p piperine-python -i piperine` empty | `crates/piperine-python/Cargo.toml:12` `piperine-api = { path = "../piperine-api" }` (no root dep); `cargo tree -p piperine-python -i piperine` → `error: package ID specification 'piperine' did not match any packages` (exit 101). All 5 python src files import `piperine_api` (grep: results/live/instance/module/lib). | ✅ PASS |
| AC2: cli host-API imports from `piperine-api` (directly or via shell) | CLI behavior unchanged | `grep -rn "piperine_api\|piperine::" crates/piperine-cli/src/` → no matches; cli has **no** host-API imports (`SimSession`/`SolverConfig` unused in cli src) — vacuously satisfied; it reaches the host through `piperine-python` (embedded CPython) and `piperine-plugin` (shell path, allowed by AC wording). `cargo test -p piperine-cli` green (11 tests in full gate). | ✅ PASS |
| AC3: zero warnings; binary from `piperine-cli` | workspace clean | `cargo build --workspace` exit 0; only "warning" lines are `piperine-cli` build-script notices about the python `.so` (`warning: piperine-cli@0.1.1: …`), zero rustc warnings. `crates/piperine-cli/Cargo.toml:9-10` `[[bin]] name = "piperine"`; root `Cargo.toml` has no `[[bin]]`. | ✅ PASS |

### Edge cases

- Re-export covers every pre-move public item: shell is glob `pub use` (`src/lib.rs:9`) and the gate compiles every in-repo consumer (451 green) — ✅.
- Docs naming root as library face updated: `CLAUDE.md` pipeline line + crate table rows (`piperine-api` = library face MD-20; root = re-export shell), `docs/spec/index.md:36` "Rust (`piperine-api`)", `docs/spec/part_viii_host_api.md:10-14` (`piperine-api` + shell note, `piperine_api::prelude` one-import face) — ✅ for the spec-named files. Residue: `README.md:27`, `docs/spec/part_iii_interpreted_context.md:10` (Minor, see Fix Plans).

**Status**: ✅ All 9 ACs covered (1 minor doc residue outside the spec-named file set)

---

## Discrimination Sensor

| Mutation | File:line | Description | Killed? |
| --- | --- | --- | --- |
| M1 | `src/lib.rs:9` | `pub use piperine_api::*;` → `pub use piperine_api::prelude;` (shell stops re-exporting the surface) | ✅ Killed — `cargo check -p piperine --tests` fails: E0432/E0425/E0282 in `compile_once_sweep`, `ngspice_validation` (compile error = killed) |
| M2 | `crates/piperine-api/src/session.rs:159` | `run_op`: `Self::snapshot_digital(&info, &circuit)` → `HashMap::new()` (drops digital snapshot) | ✅ Killed — `cargo test -p piperine --test session op_result_reads_digital_nets_directly` → `FAILED. 0 passed; 1 failed` |

Scratch discipline: each mutant applied, tested, restored via `git checkout --`; `git status --porcelain` empty after both.

**Sensor depth**: lightweight (2 mutations)
**Result**: 2/2 killed — PASS ✅

---

## Code Quality

| Principle | Status |
| --- | --- |
| Minimum code | ✅ pure `git mv` (0-line deltas on 5 of 6 files) + 27-line lib.rs + 9-line shell |
| Surgical changes | ⚠️ `ROADMAP_REFINEMENT.md` (1014 lines) deleted in 445fd8c — unrelated to the spec's scope; harmless cleanup but not in any task's Where |
| No scope creep in behavior | ✅ no behavior change (session.rs/results.rs/waveform.rs byte-identical) |
| Matches patterns | ✅ crate layout, doc style, workspace-version conventions match siblings |
| Spec-anchored outcome check | ✅ smoke asserts 5.0 V divider + exact dep set; root guards unchanged |
| Per-layer Coverage Expectation met | ✅ matrix rows all satisfied (api smoke + root parity + python e2e + cli integration; docs = build gate) |
| Every test maps to a requirement | ✅ 2 new tests map to API-01/API-02/API-03 (smoke.rs doc headers cite them); no other tests added |
| Documented guidelines followed | ✅ CLAUDE.md: `--workspace` used, zero-warnings bar met |

---

## Gate Check

- **Gate command**: `cargo build --workspace` + `cargo test --workspace` (Build gate, tasks.md)
- **Result**: **451 passed, 0 failed, 5 ignored** (aggregated over 76 test binaries)
- **Test count before feature**: 449 passed / 5 ignored (baseline, tasks.md)
- **Test count after**: 451 / 5 — **delta +2** (both in `crates/piperine-api/tests/smoke.rs`)
- **Warnings**: 0 rustc warnings (3 build-script `.so` notices from piperine-cli's build.rs, expected)
- **Skipped**: 5 ignored — pre-existing (live/network-gated), unchanged from baseline
- **Failures**: none

---

## Fix Plans (advisory — none blocking)

### Fix 1 (Minor, docs): README + Part III still name the root crate as the Rust host
- **Root cause**: T3 sweep covered CLAUDE.md/part VIII/index but not `README.md:27` ("the Rust root crate drive analyses") or `docs/spec/part_iii_interpreted_context.md:10` ("or Rust (the root `piperine` crate)").
- **Fix task**: reword both to `piperine-api` (+ shell note), mirroring part VIII's phrasing.
- **Priority**: Minor (statements remain technically true through the shell).

### Note (no fix): T2 body over-claim on cli tree
`cargo tree -p piperine-cli -i piperine` is non-empty (root via `piperine-plugin`, pre-existing edge, `crates/piperine-plugin/src/host.rs:7`). Spec AC-06 permits the shell path and plugin retarget is deferred to P5 — correct the expectation when P5 lands, not the code now.

---

## Requirement Traceability Update

| Requirement | Previous | New |
| --- | --- | --- |
| API-01 | Pending | ✅ Verified |
| API-02 | Pending | ✅ Verified |
| API-03 | Pending | ✅ Verified |
| API-04 | Pending | ✅ Verified |
| API-05 | Pending | ✅ Verified |
| API-06 | Pending | ✅ Verified |
| API-07 | Pending | ✅ Verified (minor residue: README.md:27, part_iii:10 — advisory fix) |

---

## Summary

**Overall**: ✅ Ready

**Spec-anchored check**: 9/9 ACs matched spec outcome
**Sensor**: 2/2 mutations killed
**Gate**: 451 passed / 0 failed / 5 ignored, zero rustc warnings

**What works**: clean extraction (byte-identical moves), shell proven by parity tests + prelude-path probe, exact dependency topology asserted in-repo, python fully retargeted, binary stays in piperine-cli.

**Issues found**: two stale doc mentions of the root crate as Rust host (Minor, advisory Fix 1); T2 body over-claims the cli tree (informational).
