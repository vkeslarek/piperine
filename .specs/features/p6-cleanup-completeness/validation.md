# P6 — Cleanup & Completeness Validation

**Date**: 2026-07-26
**Spec**: `.specs/features/p6-cleanup-completeness/spec.md` (CLN-01..21)
**Diff range**: `dde81c2..HEAD` (T1–T25 + the sensor fixes)
**Verifier**: standalone fresh-eyes pass (`validate.md`) — no sub-agents in this
session, so the author ran the checklist with the sensor as the empirical
counterweight to self-assessment. Recorded as a deviation from
"author ≠ verifier".

---

## Task Completion

| Task | Status | Notes |
|---|---|---|
| T1–T4 (measure) | ✅ Done | `audit.md` §1–§5, §7, §8; tool at `tools/audit_tests.py` |
| T5 (dead suites) | ✅ Done | 38 dead tests → 20 restored, 18 deleted with survivors |
| T6–T7 (hygiene guard) | ✅ Done | 5 tests incl. the fixture self-test added post-sensor |
| T8–T17 (allocation) | ✅ Done | 4 crates needed no change (recorded as findings), 6 changed |
| T18–T20 (flags) | ✅ Done | `SUPPORTS_QUERIES` removed, `BYPASS_OK` wired, guard added |
| T21–T23 (§16) | ✅ Done | 6 rule tests + enforcement column + 3-test guard |
| T24–T25 (docs) | ✅ Done | ROADMAP/CLAUDE.md corrected; MD-31, MD-32 recorded |

---

## Spec-Anchored Acceptance Criteria

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| CLN-01 audit classifies every test with evidence | one row per test, `kind_hint` + evidence | `tools/audit_tests.py` → `audit_verdicts.tsv` (1142 rows); `audit.md` §1 counts | ✅ |
| CLN-02 migrated crate re-audits clean | zero violations | `--check` exits 0 for **11/11** crates (`audit.md` §10) | ✅ |
| CLN-03 unit tests moved inline, names preserved | same names, same assertions | `crates/piperine-plugin/src/manifest.rs` `mod tests` (14 tests, names unchanged); lib target 15 passed | ✅ |
| CLN-04 regroup + delete only with a named survivor | each deletion names `file::test` | `audit.md` §6b/§6 tables: 18 + 3 deletions, each with a survivor | ✅ |
| CLN-05 no disabled test code | none anywhere | `tests/suite_hygiene.rs:~95` — `assert!(offences.is_empty(), …)` over `is_disabled_marker`; proven failing (T7) | ✅ |
| CLN-06 the 38 switched-off tests restored or deleted-with-survivor | either/or per test | `resolve_lowering.rs` (14), `analog_kernel.rs` (6); `audit.md` §6b delete table (18) | ✅ |
| CLN-07 un-restorable behaviour is a loud tracked gap | recorded, never silently dropped | `audit.md` §6b "Findings" 1–5 (label loss, `transition` stale gap, moved refusal boundary, silent `$simparam` default, 3 coverage gaps) | ✅ |
| CLN-08 ignore policy guarded, reading sources | guard fails on a bare `#[ignore]`/ignored test | `suite_hygiene.rs` `no_ignored_tests` + `the_detectors_recognise_what_they_forbid` (fixtures) | ✅ (tightened: zero `#[ignore]`, not "with a reason" — recorded in `audit.md` §9) |
| CLN-09 suite green per commit | per-crate gate in the same commit | every task commit ran its gate; final `cargo test --workspace` 1163 passed / 0 failed | ✅ |
| CLN-10 relocated tests keep serialization guards | guard survives the move | `audit.md` §3: all 16 global-state tests are `keep`; python facade suites run twice with identical results (T11) | ✅ |
| CLN-11 `SUPPORTS_QUERIES` enforced or removed | removed, purged | `core/element.rs:73` (vacant-bit note); `capabilities_contract.rs:91` `for gone in [… "SUPPORTS_QUERIES"]` | ✅ |
| CLN-12 `BYPASS_OK` enforced or removed | wired | `analyses/dc.rs` `bypass_allowed` + gate; `stamp_bypass.rs` — `assert_eq!(hits, 0, …)` / `assert!(hits > 0, …)` | ✅ |
| CLN-13 registry exhaustive, no "no consumer" entry | assertion, not prose | `capabilities_contract.rs::no_capability_flag_is_merely_reserved`; proven failing on a planted "reserved" entry | ✅ |
| CLN-14 removed bit purged; zero warnings | grep-clean + clean build | grep finds only the removal notes; `cargo build --workspace` has no code warnings | ✅ |
| CLN-15 `bound_step_hint` recorded as enforced | ROADMAP corrected, no code change | `audit.md` §8 (3 call sites); ROADMAP P6 row struck with "not a gap" | ✅ |
| CLN-16 §16 rows classified with evidence | all 16 | `audit.md` §7 (16 rows, verdict + file:line each) | ✅ |
| CLN-17 missing enforcement tests added, typed error | domain + fragment | `failure_rules.rs` — `assert_rule(&err, SolverDomain::Noise, "output node")`, `…::Pss, "period must be positive"`, `…::Newton, "converge"` | ✅ |
| CLN-18 unreachable rows removed from the spec | delete the row | **⚠️ Deviated** — the four rows were *reachable but unenforced*; each row now states its enforcement status and is guarded. Rationale in `audit.md` §7b | ⚠️ Documented deviation |
| CLN-19 §16 guard binds rows ↔ tests | row count ↔ enforcement | `spec_failure_rules_guard.rs` (3 tests); both failure modes proven | ✅ |
| CLN-20 ROADMAP restated | numbers match the tree | ROADMAP P6 rewritten (1123 not ~800; zero ignored; `bound_step_hint` not dead; 16 rows) | ✅ |
| CLN-21 STATE.md handoff + decisions | handoff + any new MD | `.specs/STATE.md` — MD-31, MD-32, feature entry, gate numbers | ✅ |

**Status**: 20/21 ✅, 1 documented deviation (CLN-18).

---

## Discrimination Sensor

Lightweight tier, extended to 7 mutations because the first pass surfaced two
survivors. Each ran in a scratch copy (`cp` → mutate → run → restore).

| # | File:line | Mutation | Killed? |
|---|---|---|---|
| M1 | `analyses/dc.rs` bypass gate | `.all(BYPASS_OK)` → `.any(BYPASS_OK)` (one opted-in element re-enables the cache) | ✅ killed — `stamp_bypass.rs` |
| M2 | `kernel/analog/mod.rs` predicate | `runtime_states.is_empty()` → `true` (history-dependent devices opt in) | ✅ killed — `bypass_capability.rs` |
| M3 | `tests/suite_hygiene.rs` | disabled-code detector always false | ❌ **survived** → fixed, see below |
| M4 | `spec_failure_rules_guard.rs` | accounting predicate always "accounted" | ❌ **survived** → fixed, see below |
| M5 | `analyses/noise.rs` | reference-node message reworded | ✅ survived-by-design — a wording change is not a behaviour fault; re-run as M5b |
| M5b | `analyses/noise.rs` | unresolvable reference node **silently falls back to ground** | ✅ killed — `failure_rules.rs` |
| M6 | `kernel/analog/mod.rs` | `has_reactive` renamed away (charge capability lost) | ✅ killed — `analog_kernel.rs` |
| M3-retry | after the fix | same as M3 | ✅ killed |
| M4-retry | after the fix | same as M4 | ✅ killed |

**Root cause of M3/M4**: a guard whose input is already clean passes even when
its detector is broken — the guards were verified only by hand-injected
violations (T7), which no CI run repeats. Both detectors are now named functions
with fixture tests (`the_detectors_recognise_what_they_forbid`,
`the_accounting_predicate_recognises_an_unaccounted_row`), so the check itself is
under test. Commit `3476cc2`.

**Result**: 7/7 killed after the fix (2 initially survived, both fixed and
re-verified) — ✅ PASS.

---

## Code Quality

| Principle | Status |
|---|---|
| Minimum code | ✅ — the only production code added is one kernel predicate and one solver gate field |
| Surgical changes | ✅ — test relocation touched no behaviour; the two production changes are the flag dispositions the spec required |
| No scope creep | ⚠️ two justified extensions, both recorded: T6 added `//!` headers to 13 targets (the guard's precondition) and T16 fixed a `--check` scope bug in the audit tool |
| Matches patterns | ✅ — registry+exhaustiveness mirrors `capabilities_contract.rs`; source-walking mirrors `extern_coverage_guard.rs`; source-driven kernel tests mirror `limiters.rs` |
| Spec-anchored outcomes | ✅ — see the AC table; the one loose assertion (§9's domain) was corrected to what the code does, with the reason in the test |
| No unclaimed tests | ✅ — every new test maps to CLN-05..19; the fixture self-tests map to CLN-08/CLN-19 via the sensor finding |
| Guidelines followed | ✅ `CLAUDE.md` (zero warnings, `cargo test --workspace`), `AGENTS.md` (MD-13 idiom rules), `.specs/STATE.md` MD-28 |

---

## Edge Cases (from spec.md)

- [x] Private-item unit test moves inline — the manifest suite is the case.
- [x] Heavy-fixture tests stay integration — solver/codegen JIT suites kept.
- [x] Layer duplication kept, same-layer duplication deleted — `opvar_host`/`session_analyses` pairs kept; `run_examples` triplicate reduced to one.
- [x] Helpers move with their tests — `lang-server` harness extracted to `tests/common/`.
- [x] Process-global-state tests keep their guards — `audit.md` §3, verified by a double run.
- [x] Sibling `mod tests` allowed — not needed; the one inline move fit its file.
- [x] Assertion-free tests reported — none found; the two Debug-dump "printer smoke" tests were deleted as the weaker duplicates.

---

## Gate Check

- **Command**: `cargo test --workspace`
- **Result**: **1163 passed, 0 failed, 4 ignored** (illustrative doctests, each registered with its reason), 0 code warnings.
- **Before the feature**: 1123 passed.
- **Delta**: +40 = +20 restored from the dead suites, +5 hygiene guard (incl. the fixture self-test), +2 bypass gate, +6 bypass capability, +6 §16 rules, +3 §16 guard, +1 `no_capability_flag_is_merely_reserved`, −3 deletions with named survivors.
- **Skips**: none.

---

## Residual gaps (tracked, not silently dropped)

1. **Eight §16 rules remain unenforced** — marked in the table, listed as
   ROADMAP P6 residue. §2, §4 ×2, §6 are reachable-but-unchecked or
   unconstructible; §8, §10, §11, §14 have sites but no test.
2. **Three behaviours lost coverage in T5** with no survivor available: `I(p)`
   single-argument branch access, `$bound_step` at codegen level, and
   `LoweredBody::validate`'s mismatched-contribution-kind diagnostic
   (`audit.md` §6b finding 5).
3. **`NoiseSource.label` is unreachable from PHDL** — the operators no longer
   take a label (`audit.md` §6b finding 1).
4. **`$simparam` with an unknown key silently returns its default** — ngspice
   semantics, but a silent fallback in a fail-loud codebase (finding 4).
5. **The unresolved-identifier error no longer names its module** (finding 3).
6. **295 heuristic hint/placement conflicts remain by design** — `--check`
   enforces the recorded verdicts, and `audit.md` §2 states why the hint is
   wrong where they cluster.

---

## Summary

**Overall**: ✅ Ready.

**Spec-anchored**: 20/21 ACs matched their spec-defined outcome; CLN-18 landed as
a documented deviation (the rows were reachable-but-unenforced, a bucket the
spec did not anticipate).
**Sensor**: 7/7 killed after fixing two real survivors.
**Gate**: 1163 passed, 0 failed, 0 warnings.

**What works**: the suite no longer contains dead or ignored test code and cannot
regain it; allocation decisions are recorded per test and enforced per crate;
every capability bit names a live consumer; §16 is a contract rather than a wish;
and the roadmap's numbers match the tree.

**Verifier caveat**: this pass was run by the implementer (no sub-agents this
session). The sensor is the part that does not depend on self-assessment, and it
did find two genuine weaknesses — which is the reason it is mandatory.
