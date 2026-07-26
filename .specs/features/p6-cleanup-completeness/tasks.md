# P6 — Cleanup & Completeness Tasks

## Execution Protocol (MANDATORY — do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name
and follow its Execute flow and Critical Rules** (per-task cycle, adequacy
review, Verifier, discrimination sensor). Do not search for skill files by
path. **If the skill cannot be activated, STOP and tell the user.**

**Spec:** `.specs/features/p6-cleanup-completeness/spec.md` (CLN-01..21)
**Design:** `.specs/features/p6-cleanup-completeness/design.md`
**Status:** Approved — Execute (started 2026-07-25, branch `feature/bench-removal`)

**Sub-agents:** not used. This session's tooling policy forbids spawning
agents without an explicit user request, so all 25 tasks run inline,
sequentially, and the closing Verifier runs as the standalone fresh-eyes pass
from `validate.md`.

**Tools (all tasks):** MCP: NONE · Skill: NONE (the audit tool is a local
Python script created by T1).

---

## Test Coverage Matrix

> Generated from project guidelines + codebase sampling. Guidelines found:
> `CLAUDE.md` ("zero warnings is the bar", `cargo test --workspace`, tests-of-
> record list), `AGENTS.md` (Hard rules → Rust idiom rules, test map),
> `.specs/STATE.md` MD-28 (test placement) and MD-13 (idiom rules). No
> coverage-threshold tooling exists — the gate is the suite itself.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
|---|---|---|---|---|
| Policy guards (hygiene, §16 registry) | integration | Every invariant the guard claims + a proven-failing violation (a guard that cannot fail is not a guard) | `tests/suite_hygiene.rs`, `crates/piperine-solver/tests/failure_rules.rs` | `cargo test -p piperine suite_hygiene`, `cargo test -p piperine-solver failure_rules` |
| Solver ABI (`ElementCapabilities`, bypass gate) | unit + integration | All branches of the new gate; 1:1 with CLN-11..14; the negative case (a non-declaring device is never bypassed) | inline `#[cfg(test)]` in `analyses/dc.rs`; `crates/piperine-solver/tests/capabilities_contract.rs` | `cargo test -p piperine-solver` |
| Codegen device capability predicate | integration | Each disqualifier (operator / event / limiter / digital dep) proven to withhold the bit; one qualifying device proven to set it | `crates/piperine-codegen/tests/` | `cargo test -p piperine-codegen` |
| Spec-rule enforcement tests (§16 rows) | integration | One test per enforceable row, asserting the **typed** error, in the suite that owns the analysis | `crates/piperine-solver/tests/*` | `cargo test -p piperine-solver` |
| Test relocation (Phase 2) | none *(no new behavior)* | Not "no tests" — the **verification is the diff**: `cargo test -p <crate> -- --list` before/after must differ only by intended moves/deletions, each `delete` carrying a named survivor | `crates/*/tests/`, inline `#[cfg(test)]` | `cargo test -p <crate>` |
| Audit tool + reports | none | Build gate + report review; a report is evidence, not an invariant | `.specs/features/p6-cleanup-completeness/` | review |
| Docs (ROADMAP, STATE, Part VII) | none | Completeness review | `ROADMAP.md`, `.specs/STATE.md`, `docs/spec/` | review |

## Gate Check Commands

| Gate | When | Command |
|---|---|---|
| Quick (crate) | any single-crate task | `cargo test -p <crate>` |
| Quick (solver) | ABI / §16 tasks | `cargo test -p piperine-solver` |
| Quick (codegen) | codegen predicate tasks | `cargo test -p piperine-codegen` |
| Full | guards, cross-crate flag removal, phase ends | `cargo test --workspace` |
| Build | report/doc-only tasks | `cargo build --workspace` + review |

**Baseline (measured 2026-07-25, commit `30f5780`):** 1123 passed, 0 failed,
4 ignored (all doctests), 179 targets, 0 build warnings. Every task's gate is
measured against this baseline; a drop in passing tests is legal **only** for
deletions listed with a named survivor.

---

## Execution Plan

### Phase 0 — Measure (evidence before action)

```
T1 → T2 → T3 → T4
```

### Phase 1 — Dead test code + the guard that keeps it dead

```
T5 → T6 → T7
```

### Phase 2a — Allocation migration, small crates

```
T8 → T9 → T10 → T11
```

### Phase 2b — Allocation migration, large suites

```
T12 → T13 → T14 → T15 → T16 → T17
```

### Phase 3 — Capability flags: enforce or remove

```
T18 → T19 → T20
```

### Phase 4 — Part VII §16: every rule enforced

```
T21 → T22 → T23
```

### Phase 5 — Docs and memory

```
T24 → T25
```

---

## Task Breakdown

### T1: Audit tool — mechanical facts per test
**What:** A Python 3 script that lists every test in the workspace with the
evidence needed to classify it (pipeline entry points called, cross-crate
imports, global-state touches) and a `--check <crate>` regression mode.
**Where:** `.specs/features/p6-cleanup-completeness/tools/audit_tests.py` (new).
**Depends on:** None. **Requirement:** CLN-01.
**Reuses:** —
**Done when:**
- [ ] `python3 tools/audit_tests.py --root .` emits one TSV row per test:
      `crate, file, test_name, kind_hint, evidence`.
- [ ] `kind_hint` ∈ `{unit, integration, unclear}`; evidence names the entry
      points found (`parse_and_elaborate`, `CircuitCompiler`, `SimSession`,
      `PluginHost`, `Command::new`, …) and flags `env::set_var` /
      `set_current_dir` / `facade_lock` usage.
- [ ] It finds **all 1123** live tests plus the dead `ppr_ir.rs` block, and
      reports inline vs `tests/` placement per test.
- [ ] `--check <crate>` exits non-zero when a `unit`-hinted test sits in
      `tests/` (proven on a crate that currently has one).
- [ ] Script is stdlib-only and lives outside every crate (not in the build).
**Tests:** none (tool) · **Gate:** build

### T2: `audit.md` — the allocation verdicts
**What:** Run T1's tool over the workspace and turn its hints into per-test
**verdicts** (`keep` / `move-inline` / `regroup` / `delete`) using the spec's
unit-vs-integration definition; every `delete` names its surviving equivalent.
**Where:** `.specs/features/p6-cleanup-completeness/audit.md` (new).
**Depends on:** T1. **Requirement:** CLN-01, CLN-04, CLN-10.
**Reuses:** T1 output.
**Done when:**
- [ ] Every test in the workspace appears exactly once with a verdict.
- [ ] Each `delete` row carries `survivor: <file::test>`; a row with no
      survivor is instead `keep` or `move-inline`.
- [ ] Each row that touches process-global state is flagged so T8–T16 keep
      its serialization guard.
- [ ] The three shared file stems (`opvar_host.rs`, `run_examples.rs`,
      `session_analyses.rs`, root vs crate) each get an explicit
      same-layer-or-not verdict.
- [ ] Per-crate summary counts (`keep/move/regroup/delete`) head the file.
**Tests:** none (report) · **Gate:** build

### T3: §16 row classification
**What:** Classify all 16 Part VII §16 failure rows (measured; the draft said 18) as *enforced* (naming the
existing `file::test` that trips it), *enforceable-but-untested*, or
*unenforceable* (naming why no public surface reaches it).
**Where:** `.specs/features/p6-cleanup-completeness/audit.md` (new `## §16`
section).
**Depends on:** T2. **Requirement:** CLN-16.
**Reuses:** `docs/spec/part_vii_solver.md:1116-1140`; the solver suite.
**Done when:**
- [ ] All 16 rows classified, each with file:line evidence.
- [ ] Every *enforced* claim verified by actually running that test (name +
      pass recorded), not by reading its name.
- [ ] The untested and unenforceable sets are listed as the T21/T22 work lists.
**Tests:** none (report) · **Gate:** build

### T4: Capability-bit verdict evidence
**What:** Record the enforce-or-remove evidence for `SUPPORTS_QUERIES`,
`BYPASS_OK`, and `bound_step_hint` — every declarer and every consumer, by
file:line — so Phase 3 executes a decision, not a guess.
**Where:** `.specs/features/p6-cleanup-completeness/audit.md` (new `## flags`
section).
**Depends on:** T3. **Requirement:** CLN-11, CLN-12, CLN-15.
**Reuses:** `crates/piperine-solver/tests/capabilities_contract.rs` registry.
**Done when:**
- [ ] `SUPPORTS_QUERIES`: zero declarers / zero readers confirmed by grep →
      verdict **remove**.
- [ ] `BYPASS_OK`: the global bypass in `analyses/dc.rs` shown not to consult
      it, and the only declarer shown to be a test device → verdict **wire**,
      with the list of codegen disqualifiers the predicate must check.
- [ ] `bound_step_hint`: its three live call sites recorded → verdict
      **already enforced**, ROADMAP correction only.
- [ ] Any other registry entry claiming "no consumer" is caught here (expected:
      none besides the two).
**Tests:** none (report) · **Gate:** build

### T5: Triage the 38 switched-off tests (`ppr_ir.rs` + `analog_jit.rs`)
**What:** For each of the 38 tests in the two `#![cfg(any())]` files
(`ppr_ir.rs` 27, `analog_jit.rs` 11 — the second found during T3 and named in
`CLAUDE.md` as a test of record), restore it
against the current `piperine_codegen::resolve` API or delete it naming the
live test that already asserts the behavior; the file ends up either gone or
normally compiled and passing.
**Where:** `crates/piperine-codegen/tests/{ppr_ir,analog_jit}.rs` (delete or
rewrite),
possibly new/edited `crates/piperine-codegen/tests/resolve_lowering.rs`.
**Depends on:** T4. **Requirement:** CLN-05, CLN-06, CLN-07.
**Reuses:** `codegen_ir.rs`, `from_ir.rs`, `silent_bugs.rs`, `analog_jit.rs`
as the coverage baseline; `resolve::lower_bodies` as the entry point.
**Done when:**
- [ ] Zero `#![cfg(any())]` in the crate (both files); the triaged tests
      compile and run.
- [ ] Each restored test asserts the same behavior as its dead original
      (noise/flicker registration, `match` desugar, event guard, `simparam`,
      `bound_step`, string params, `transition`/`idtmod` state vars, both
      lowering-error cases, …), expressed through today's API.
- [ ] Each deleted test is listed in the commit body with `survivor:
      <file::test>`.
- [ ] Any behavior that cannot be expressed through today's `resolve` API is
      recorded in `audit.md` as a named gap (CLN-07) — never silently dropped.
- [ ] `cargo test -p piperine-codegen` green; passing count ≥ baseline for
      that crate plus the restored tests.
**Tests:** integration · **Gate:** quick (codegen)

### T6: Suite hygiene guard
**What:** Three `#[test]`s that walk the repository sources and fail on
disabled test code, on an `#[ignore]` without a reason (or any ignored
`#[test]`), and on an integration target with no `//!` scope header.
**Where:** `tests/suite_hygiene.rs` (new).
**Depends on:** T5. **Requirement:** CLN-05, CLN-08.
**Reuses:** `crates/piperine-lang/tests/extern_coverage_guard.rs` walk-and-
assert shape.
**Done when:**
- [ ] `no_disabled_test_code` fails on `#![cfg(any())]`/`#[cfg(FALSE)]`/a
      commented-out `#[test]`; passes on the current tree.
- [ ] `every_ignore_states_a_reason` passes for the 4 doctest `ignore =
      "reason"` items and fails for a bare `#[ignore]` or an ignored `#[test]`.
- [ ] `every_integration_target_declares_its_scope` passes only when every
      `crates/*/tests/*.rs` and `tests/*.rs` starts with `//!`.
- [ ] Each failure message names `file:line`.
- [ ] The walk reads sources (no hardcoded test-name list) and skips `target/`.
- [ ] `cargo test --workspace` green.
**Tests:** integration · **Gate:** full

### T7: Prove the guard can fail
**What:** Deliberately violate each of T6's three invariants in a scratch
copy, confirm the matching guard fails with the right file name, and revert —
recording the evidence (a guard that cannot fail is not a guard).
**Where:** `.specs/features/p6-cleanup-completeness/audit.md` (`## guard
proofs`), no production change.
**Depends on:** T6. **Requirement:** CLN-08.
**Reuses:** T6's guard.
**Done when:**
- [ ] Three violations injected one at a time; each makes exactly its guard
      fail (output quoted in `audit.md`).
- [ ] Tree restored; `cargo test --workspace` green afterwards.
**Tests:** integration (the guard itself) · **Gate:** full

### T8: Migrate `piperine-api` + `piperine-project`
**What:** Apply `audit.md`'s verdicts for the two smallest crates: unit tests
inline, integration regrouped, duplicates deleted with named survivors.
**Where:** `crates/piperine-api/{src,tests}`, `crates/piperine-project/{src,tests}`.
**Depends on:** T7. **Requirement:** CLN-02, CLN-03, CLN-04, CLN-09, CLN-10.
**Reuses:** the crates' existing inline `#[cfg(test)]` modules.
**Done when:**
- [ ] Every `move-inline` verdict applied; test names preserved.
- [ ] Every `regroup` verdict applied; each integration file's `//!` header
      names its functionality.
- [ ] Every `delete` applied with its survivor in the commit body.
- [ ] `cargo test -p piperine-api -p piperine-project` green; the
      before/after `-- --list` diff contains only intended changes (recorded).
- [ ] `python3 tools/audit_tests.py --check piperine-api` and
      `--check piperine-project` exit 0.
**Tests:** none *(relocation — verification is the list diff)* · **Gate:** quick (crate)

### T9: Migrate `piperine-cli`
**What:** Same for `piperine-cli` (5 test files, 582 lines), keeping every
cwd/env-dependent test's isolation.
**Where:** `crates/piperine-cli/{src,tests}`.
**Depends on:** T8. **Requirement:** CLN-02..CLN-04, CLN-09, CLN-10.
**Reuses:** existing `tests/*_cmd.rs` naming.
**Done when:**
- [ ] Verdicts applied; process-global-state tests keep their guards (CLN-10).
- [ ] `cargo test -p piperine-cli` green; list diff recorded.
- [ ] `--check piperine-cli` exits 0.
**Tests:** none *(relocation)* · **Gate:** quick (crate)

### T10: Migrate `piperine-plugin` + `piperine-plugin-macros`
**What:** Same for the plugin crates (9 + 3 files), which the just-delivered
plugin-interface-v2 left functionality-named — expect mostly `keep`.
**Where:** `crates/piperine-plugin/{src,tests}`, `crates/piperine-plugin-macros/{src,tests}`.
**Depends on:** T9. **Requirement:** CLN-02..CLN-04, CLN-09.
**Reuses:** the v2 suite layout.
**Done when:**
- [ ] Verdicts applied; `trybuild` UI tests untouched.
- [ ] `cargo test -p piperine-plugin -p piperine-plugin-macros` green; list
      diff recorded.
- [ ] `--check` exits 0 for both.
**Tests:** none *(relocation)* · **Gate:** quick (crate)

### T11: Migrate `piperine-python`
**What:** Same for `piperine-python` (24 files, 2569 lines), where the facade
lock makes global-state handling load-bearing.
**Where:** `crates/piperine-python/{src,tests}`.
**Depends on:** T10. **Requirement:** CLN-02..CLN-04, CLN-09, CLN-10.
**Reuses:** `embed::facade_lock`, `tests/facade_hygiene.rs`.
**Done when:**
- [ ] Verdicts applied; every relocated test that execs Python keeps the
      facade lock (CLN-10) — proven by running the crate suite twice.
- [ ] `cargo test -p piperine-python` green; list diff recorded.
- [ ] `--check piperine-python` exits 0.
**Tests:** none *(relocation)* · **Gate:** quick (crate)

### T12: Migrate `piperine-codegen`
**What:** Same for `piperine-codegen` (23 files, 5943 lines post-T5).
**Where:** `crates/piperine-codegen/{src,tests}`.
**Depends on:** T11. **Requirement:** CLN-02..CLN-04, CLN-09.
**Reuses:** T5's triaged layout.
**Done when:**
- [ ] Verdicts applied; JIT-compiling tests stay integration (spec edge case).
- [ ] `cargo test -p piperine-codegen` green; list diff recorded.
- [ ] `--check piperine-codegen` exits 0.
**Tests:** none *(relocation)* · **Gate:** quick (codegen)

### T13: Migrate `piperine-lang`
**What:** Same for `piperine-lang` (28 files, 6537 lines) — the crate with the
most unit-shaped assertions living in integration files.
**Where:** `crates/piperine-lang/{src,tests}`.
**Depends on:** T12. **Requirement:** CLN-02..CLN-04, CLN-09.
**Reuses:** the 11 existing inline modules; frozen corpora (`headers/`,
`tests/fixtures*`) stay untouched per CLAUDE.md.
**Done when:**
- [ ] Verdicts applied; no change to frozen fixtures.
- [ ] `cargo test -p piperine-lang` green; list diff recorded.
- [ ] `--check piperine-lang` exits 0.
**Tests:** none *(relocation)* · **Gate:** quick (crate)

### T14: Migrate `piperine-lang-server`
**What:** Same for the LSP crate, whose 1880-line `integration_test.rs` is
named after a construct, not a functionality (MD-13 rule 4 / MD-28 rule 2).
**Where:** `crates/piperine-lang-server/{src,tests}`.
**Depends on:** T13. **Requirement:** CLN-02..CLN-04, CLN-09.
**Reuses:** `tests/protocol.rs` as the naming model.
**Done when:**
- [ ] `integration_test.rs` split into functionality-named targets (hover,
      completion, goto/references, rename, diagnostics …) per `audit.md`.
- [ ] `cargo test -p piperine-lang-server` green; list diff recorded.
- [ ] `--check piperine-lang-server` exits 0.
**Tests:** none *(relocation)* · **Gate:** quick (crate)

### T15: Migrate `piperine-solver`
**What:** Same for `piperine-solver` (19 files, 6077 lines + 17 inline
modules).
**Where:** `crates/piperine-solver/{src,tests}`.
**Depends on:** T14. **Requirement:** CLN-02..CLN-04, CLN-09.
**Reuses:** the crate's already-functionality-named suites.
**Done when:**
- [ ] Verdicts applied; `capabilities_contract.rs` untouched (Phase 3 owns it).
- [ ] `cargo test -p piperine-solver` green; list diff recorded.
- [ ] `--check piperine-solver` exits 0.
**Tests:** none *(relocation)* · **Gate:** quick (solver)

### T16: Migrate the root `tests/` host suite
**What:** Same for the root package's 36 files, and resolve the three
root-vs-crate shared stems per their T2 verdicts.
**Where:** `tests/` (root).
**Depends on:** T15. **Requirement:** CLN-02..CLN-04, CLN-09.
**Reuses:** `host_parity.rs`/`plugin_parity.rs` as the cross-crate model.
**Done when:**
- [ ] Verdicts applied; layer-distinct duplicates kept (spec edge case),
      same-layer duplicates deleted with survivors named.
- [ ] `cargo test -p piperine` green; list diff recorded.
- [ ] `--check piperine` exits 0.
**Tests:** none *(relocation)* · **Gate:** quick (crate)

### T17: Workspace re-audit is clean
**What:** Re-run the audit over the whole workspace and record the zero-
violation result plus the final test accounting (baseline → final, every
delta explained).
**Where:** `.specs/features/p6-cleanup-completeness/audit.md` (`## final
accounting`).
**Depends on:** T16. **Requirement:** CLN-02, CLN-04, CLN-09.
**Reuses:** T1 tool.
**Done when:**
- [ ] `--check` exits 0 for **every** crate.
- [ ] Final passing-test count recorded; every test not present in the
      baseline list is either a rename (mapped) or a deletion with a survivor.
- [ ] `cargo test --workspace` green, zero warnings.
**Tests:** none (report) · **Gate:** full

### T18: Remove `SUPPORTS_QUERIES`
**What:** Delete the bit from `ElementCapabilities` and every mention, keeping
all other bit positions unchanged.
**Where:** `crates/piperine-solver/src/core/element.rs`,
`crates/piperine-solver/tests/capabilities_contract.rs`, any doc mention
(`docs/spec/part_vii_solver.md`, `ROADMAP.md`).
**Depends on:** T17. **Requirement:** CLN-11, CLN-13, CLN-14.
**Reuses:** T4's evidence; the contract test's `removed_write_only_flags_stay_gone`
list (extend it).
**Done when:**
- [ ] `SUPPORTS_QUERIES` gone from the bitflags; `1 << 10` left unused;
      `HAS_DISTO2/3`/`NUMERIC_JACOBIAN` still assert `1 << 12/13/14`.
- [ ] `removed_write_only_flags_stay_gone` includes `SUPPORTS_QUERIES` so it
      cannot return.
- [ ] `documented_consumer` has no "no consumer today" entry left.
- [ ] Grep for the name over `crates/`, `tests/`, `docs/` returns nothing but
      the removal notes.
- [ ] `cargo test --workspace` green, zero warnings.
**Tests:** integration · **Gate:** full

### T19: Wire `BYPASS_OK` (gate + codegen predicate)
**What:** Gate the DC stamp-bypass cache on per-element consent, and have
codegen declare `BYPASS_OK` only for devices whose DC stamps are a pure
function of terminal voltages.
**Where:** `crates/piperine-solver/src/analyses/dc.rs` (gate + inline tests),
`crates/piperine-codegen/src/device/mod.rs` (predicate),
`crates/piperine-codegen/tests/` (predicate tests).
**Depends on:** T18. **Requirement:** CLN-12, CLN-14.
**Reuses:** existing `cache_valid`/`any_limiting_report`/`invalidate_bypass`
seams; `AnalogKernel`'s capability `Option`s.
**Done when:**
- [ ] The bypass applies only when **every** element in the circuit declares
      `BYPASS_OK`; limiter suppression and `invalidate_bypass` unchanged.
- [ ] Inline test: a circuit with one non-declaring element records
      `bypass_hits == 0` while an all-declaring circuit still records hits.
- [ ] Codegen predicate withholds the bit for each disqualifier (runtime
      operator, analog event, `$limit` limiter, digital dependency) — one
      test per disqualifier — and sets it for a plain resistor-class device.
- [ ] `piperine-solver/tests/live_params.rs`'s declaring test device still
      passes unchanged.
- [ ] ngspice validation + solver suites still green (`cargo test --workspace`).
**Tests:** unit + integration · **Gate:** full

### T20: Capability registry has no reserved-forever bits
**What:** Assert the invariant the two verdicts create: every flag's registry
entry names a live consumer, and no entry may say "reserved"/"no consumer".
**Where:** `crates/piperine-solver/tests/capabilities_contract.rs`.
**Depends on:** T19. **Requirement:** CLN-13.
**Reuses:** the registry test's exhaustiveness assert.
**Done when:**
- [ ] A new `#[test]` fails if any entry's text contains "no consumer" or
      starts with "reserved" (proven by temporarily reintroducing such text).
- [ ] `BYPASS_OK`'s entry now names its consumer (`analyses/dc.rs` gate).
- [ ] `cargo test -p piperine-solver` green.
**Tests:** integration · **Gate:** quick (solver)

### T21: Add the missing §16 enforcement tests
**What:** For each row T3 marked *enforceable-but-untested*, add a test that
trips exactly that failure and asserts the typed error.
**Where:** `crates/piperine-solver/tests/` (per-analysis suites).
**Depends on:** T20. **Requirement:** CLN-17.
**Reuses:** existing per-analysis error tests as the shape; `SolverDomain`
typed errors.
**Done when:**
- [ ] One test per untested row, named for the rule it enforces.
- [ ] Each asserts the typed error (not a string fragment alone) and fails if
      the guard clause is removed.
- [ ] `cargo test -p piperine-solver` green.
**Tests:** integration · **Gate:** quick (solver)

### T22: Remove unreachable §16 rows from the spec
**What:** Delete any §16 row T3 proved unreachable through every public
surface, with a one-line note saying so.
**Where:** `docs/spec/part_vii_solver.md` §16.
**Depends on:** T21. **Requirement:** CLN-18.
**Reuses:** T3's classification.
**Done when:**
- [ ] Each removed row is justified in the section's note (why no surface
      reaches it).
- [ ] No row remains that neither a test nor a note accounts for.
**Tests:** none (docs) · **Gate:** build

### T23: §16 coverage guard
**What:** The registry test binding every §16 row to its enforcement test,
parsed from the spec file.
**Where:** `crates/piperine-solver/tests/failure_rules.rs` (new).
**Depends on:** T22. **Requirement:** CLN-19.
**Reuses:** `capabilities_contract.rs` registry pattern; `include_str!` on the
spec file.
**Done when:**
- [ ] `rows()` parses the §16 table (assert non-empty — a reflow fails loud).
- [ ] `every_failure_rule_is_accounted_for` is exhaustive; an unaccounted row
      fails naming it.
- [ ] `every_named_enforcement_test_exists` verifies each named
      `file::test` exists in the tree (proven by pointing one entry at a
      missing name).
- [ ] `cargo test --workspace` green.
**Tests:** integration · **Gate:** full

### T24: Restate ROADMAP P6
**What:** Correct P6's stale claims (test count, "28 ignored", `bound_step_hint`),
check off the delivered rows, and leave the completeness items explicitly
post-V1.
**Where:** `ROADMAP.md`.
**Depends on:** T23. **Requirement:** CLN-15, CLN-20.
**Reuses:** T17's final accounting; T4's evidence.
**Done when:**
- [ ] Numbers match the tree (final count, 4 reasoned doctest ignores).
- [ ] `bound_step_hint` recorded as enforced with its call sites, not as dead.
- [ ] Delivered rows `[x]`; the language/interpreter completeness row stays
      `[ ]` and post-V1 with its reason.
**Tests:** none (docs) · **Gate:** build

### T25: `.specs/STATE.md` — handoff + decisions
**What:** Add the feature's handoff entry and the two macro decisions this
work locks (policy-invariants-live-in-the-gate; `BYPASS_OK` per-circuit
consent).
**Where:** `.specs/STATE.md`.
**Depends on:** T24. **Requirement:** CLN-21.
**Reuses:** MD-28 (extended, not superseded); the MD entry format.
**Done when:**
- [ ] Handoff snapshot updated with the final gate numbers.
- [ ] Two new MD entries, each with status + rationale, referencing MD-28/MD-01.
- [ ] `cargo test --workspace` green (final gate re-run recorded).
**Tests:** none (docs) · **Gate:** full

---

## Phase Execution Map

```
Phase 0 → Phase 1 → Phase 2a → Phase 2b → Phase 3 → Phase 4 → Phase 5

Phase 0:  T1 → T2 → T3 → T4
Phase 1:  T5 → T6 → T7
Phase 2a: T8 → T9 → T10 → T11
Phase 2b: T12 → T13 → T14 → T15 → T16 → T17
Phase 3:  T18 → T19 → T20
Phase 4:  T21 → T22 → T23
Phase 5:  T24 → T25
```

25 tasks, executed inline and sequentially (no sub-agents — see Execution
Protocol). Phase 2 is split at the small-crate/large-suite seam so no phase
exceeds ~1.5× the task budget.

---

## Task Granularity Check

| Task | Scope | Status |
|---|---|---|
| T1 | one script | ✅ |
| T2 | one report section (verdicts) | ✅ |
| T3 | one report section (§16) | ✅ |
| T4 | one report section (flags) | ✅ |
| T5 | one test file's triage | ✅ |
| T6 | one test file (3 cohesive guards) | ✅ |
| T7 | one verification pass, no production change | ✅ |
| T8 | two smallest crates (3 test files total) | ⚠️ OK — cohesive, both are trivially small |
| T9–T16 | one crate each | ✅ |
| T17 | one report section + full gate | ✅ |
| T18 | one flag removal | ✅ |
| T19 | one gate + one predicate (same invariant, cannot be split without landing a half-wired bit) | ⚠️ OK — one tight dependency |
| T20 | one test | ✅ |
| T21 | one test suite addition per T3's list | ✅ |
| T22 | one doc section | ✅ |
| T23 | one test file | ✅ |
| T24 | one doc section | ✅ |
| T25 | one doc file | ✅ |

No ❌.

---

## Diagram-Definition Cross-Check

| Task | Depends On (body) | Diagram | Status |
|---|---|---|---|
| T1 | None | (start) | ✅ |
| T2 | T1 | T1→T2 | ✅ |
| T3 | T2 | T2→T3 | ✅ |
| T4 | T3 | T3→T4 | ✅ |
| T5 | T4 | T4→T5 (phase boundary) | ✅ |
| T6 | T5 | T5→T6 | ✅ |
| T7 | T6 | T6→T7 | ✅ |
| T8 | T7 | T7→T8 (phase boundary) | ✅ |
| T9 | T8 | T8→T9 | ✅ |
| T10 | T9 | T9→T10 | ✅ |
| T11 | T10 | T10→T11 | ✅ |
| T12 | T11 | T11→T12 (phase boundary) | ✅ |
| T13 | T12 | T12→T13 | ✅ |
| T14 | T13 | T13→T14 | ✅ |
| T15 | T14 | T14→T15 | ✅ |
| T16 | T15 | T15→T16 | ✅ |
| T17 | T16 | T16→T17 | ✅ |
| T18 | T17 | T17→T18 (phase boundary) | ✅ |
| T19 | T18 | T18→T19 | ✅ |
| T20 | T19 | T19→T20 | ✅ |
| T21 | T20 | T20→T21 (phase boundary) | ✅ |
| T22 | T21 | T21→T22 | ✅ |
| T23 | T22 | T22→T23 | ✅ |
| T24 | T23 | T23→T24 (phase boundary) | ✅ |
| T25 | T24 | T24→T25 | ✅ |

All dependencies point backward or within the phase; no forward deps.

---

## Test Co-location Validation

| Task | Layer created/modified | Matrix requires | Task says | Status |
|---|---|---|---|---|
| T1 | audit tool | none | none | ✅ |
| T2 | report | none | none | ✅ |
| T3 | report | none | none | ✅ |
| T4 | report | none | none | ✅ |
| T5 | codegen tests | integration | integration | ✅ |
| T6 | policy guard | integration | integration | ✅ |
| T7 | policy guard (proof) | integration | integration | ✅ |
| T8–T16 | test relocation | none *(verification is the list diff)* | none *(list diff recorded)* | ✅ |
| T17 | report + full gate | none | none | ✅ |
| T18 | solver ABI | integration | integration | ✅ |
| T19 | solver ABI + codegen predicate | unit + integration | unit + integration | ✅ |
| T20 | policy guard | integration | integration | ✅ |
| T21 | spec-rule enforcement | integration | integration | ✅ |
| T22 | docs | none | none | ✅ |
| T23 | policy guard | integration | integration | ✅ |
| T24 | docs | none | none | ✅ |
| T25 | docs | none | none | ✅ |

No violations. The `none` on T8–T16 is **not** test deferral: those tasks
create no behavior, and their verification (`-- --list` diff + per-crate gate +
`--check` exit 0) is stated in each task's Done-when, per the matrix's
relocation row.

---

## Requirement → Task Coverage

| Req | Task(s) | Req | Task(s) |
|---|---|---|---|
| CLN-01 | T1, T2 | CLN-12 | T4, T19 |
| CLN-02 | T8–T17 | CLN-13 | T18, T20 |
| CLN-03 | T8–T16 | CLN-14 | T18, T19 |
| CLN-04 | T2, T8–T17 | CLN-15 | T4, T24 |
| CLN-05 | T5, T6 | CLN-16 | T3 |
| CLN-06 | T5 | CLN-17 | T21 |
| CLN-07 | T5 | CLN-18 | T22 |
| CLN-08 | T6, T7 | CLN-19 | T23 |
| CLN-09 | T8–T17 | CLN-20 | T24 |
| CLN-10 | T2, T8–T11 | CLN-21 | T25 |
| CLN-11 | T4, T18 | | |

All 21 requirements mapped.
