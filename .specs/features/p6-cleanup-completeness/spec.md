# P6 — Cleanup & Completeness Specification

> **Refines ROADMAP P6** ("Cleanup & completeness"). Governing decisions:
> **MD-28** (test placement standard — unit inline, integration grouped by
> functionality, redundant tests deleted), **MD-13** (Rust idiom rules),
> **MD-01/MD-12** (one `Element` ABI; ABI-vs-policy classification).
> Scope is the ROADMAP's own triage: *"The clear-V1 subset here is test
> sanitization + ignored-test + dead-flag cleanup (hygiene). The completeness
> items are post-V1 unless a user hits them."*

## Problem Statement

The suite has grown to **1123 tests across 179 targets** by accretion, so its
shape reflects authoring order rather than the code: unit-level assertions sit
in distant integration files, integration files are grouped by the session
that wrote them, one 602-line test file is silently switched off with
`#![cfg(any())]`, and two capability bits plus a table of spec failure rules
have no enforcement anywhere. That makes regressions in every later refactor
harder to see, which is exactly what P6 exists to fix (a stated P3b/P6
prerequisite). Three ROADMAP P6 claims are also stale and must be corrected
in place while the work happens.

## Goals

- [ ] Every test lives where **MD-28** says it belongs: unit tests inline
      (`#[cfg(test)] mod tests` in the implementation file), integration tests
      grouped by the functionality they exercise, redundant tests deleted —
      applied crate by crate with the suite green after **every** commit.
- [ ] Zero dead test code: no `#![cfg(any())]`/commented-out test module
      anywhere; each of `ppr_ir.rs`'s 27 switched-off tests is either restored
      against the current `resolve` API or deleted with its duplicate named.
- [ ] Every remaining `ignore` (all 4 are illustrative doctests) carries a
      reason string, and the count is asserted by a guard so a new silent
      `#[ignore]` fails the suite.
- [ ] Every `ElementCapabilities` bit and every Part VII §16 failure row is
      **enforced or removed** — no reserved-forever bits, no spec rule without
      a test.
- [ ] ROADMAP P6 restated with verified numbers, and the delivered rows
      checked off.
- [ ] `cargo test --workspace` green with **no loss of assertions**: the final
      count of *distinct asserted behaviors* never drops without a named
      equivalent, and coverage of every touched file is argued in the commit.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Non-blocking language/interpreter completeness (slice exprs outside analog/digital bodies, `for` in a digital body, selector complex-exprs / field-less match) | ROADMAP P6 explicitly triages these as post-V1 "schedule on demand"; they are behavior additions, not hygiene, and the selector half overlaps the language-backlog selector-axes item, to be decided as one. |
| Rewriting test *content* / strengthening weak assertions beyond the touched files | This feature relocates, deletes duplicates, and enforces; a general assertion-quality pass is unbounded. A weak assertion found in a file being moved is fixed in that task (see CLN-04). |
| New test frameworks, harnesses, or a coverage tool in CI | The gate stays `cargo test --workspace`; adding tooling is a separate decision. |
| Renaming public API to satisfy MD-13 | Idiom cleanups that change the public surface belong to their own feature (breaking-change budget). |
| Performance work on the suite (parallelism, sharding, fixture caching) | Runtime is not the complaint; allocation is. Recorded as a follow-up. |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| P6 scope | The hygiene subset only (test sanitization, dead/ignored test cleanup, dead-flag + §16 enforce-or-remove) | ROADMAP P6's own closing triage sentence | y (ROADMAP) |
| "~800 tests, possibly mis-allocated" | The real number is **1123 passed / 179 targets / 4 ignored** (measured 2026-07-25); ROADMAP's figure is stale and gets corrected | Measured from `cargo test --workspace` | y (measured) |
| "28 `#[ignore]`d tests" | Stale. The 28 attributes all sit in `crates/piperine-codegen/tests/ppr_ir.rs`, whose first line is `#![cfg(any())]` — the file is **never compiled**, so they are dead code, not ignored tests. The only truly ignored items are **4 doctests**, each already annotated with a reason | Read of the file + the ignored list in the test log | y (measured) |
| `bound_step_hint` "same disposition as a dead flag" | Stale. It **is** wired: `analyses/events.rs:326` folds it into `EventEntry::step_hint`, `piperine-codegen/src/device/mod.rs:238` implements it, `piperine-solver/tests/event_adapters.rs:119` covers it. It is recorded as enforced, not removed | Grep of all call sites | y (measured) |
| Which capability bits are actually unowned | `SUPPORTS_QUERIES` ("no solver consumer today") and `BYPASS_OK` ("reserved: solver-performance owns stamp bypass") are the only two registry entries that name no live consumer | `piperine-solver/tests/capabilities_contract.rs` registry | y (measured) |
| Deleting a "redundant" test | Allowed **only** when an equivalent assertion exists elsewhere, and the commit message names the surviving `file::test` that covers it. Otherwise the test moves, never disappears | Coverage is the metric, not count (MD-28) — but a deletion without a named survivor is a coverage loss disguised as hygiene | y (agent default, from MD-28) |
| Definition of "unit test" for the inline/integration split | A test is *unit* when it exercises one module's behavior through that module's own API and needs no cross-crate wiring; it is *integration* when it drives a pipeline boundary (parse→elab, POM→device, solve, CLI, host API) or spans crates. A test that only reaches its subject through `pub` re-exports because the item is private stays where the subject is | Makes MD-28 mechanically decidable per file instead of per taste | y (agent default) |
| Test-name churn | Relocation preserves test names wherever possible so `git log -S` and the lessons store stay searchable; a rename is only for a name that no longer describes its new home | Traceability | y (agent default) |
| `#[ignore]` policy | Illustrative doctests keep `ignore = "<reason>"`; a plain `#[ignore]` with no reason is forbidden and guarded | Ignored tests rot silently (the ROADMAP's own complaint) | y (agent default) |
| Enforce-or-remove verdicts | Decided per item during Design from live evidence, then executed; a "keep as reserved" verdict is **not** available for a bit with no consumer (that is the status quo P6 exists to end) | The row is explicitly "drop it or wire it" | y (ROADMAP) |
| §16 rows whose failure is unreachable today | If a rule cannot be triggered through any public surface, the row is removed from the spec table with a note, rather than left unenforced | An untestable normative rule is documentation drift | y (agent default) |

**Open questions:** none — all resolved or logged above.

### Implicit-requirement dimensions sweep (Large ⇒ full)

| Dimension | Resolution |
|---|---|
| Input validation & bounds | **N/A because** this feature adds no input surface; it moves tests and removes flags. |
| Failure / partial-failure states | Covered: CLN-09 — every commit leaves the suite green, so a partially-migrated crate is never committed; a migration that cannot keep green is reverted, not landed red. |
| Idempotency / retry / duplicate handling | Covered: CLN-04 — duplicate detection is the core mechanic (a deletion requires a named surviving equivalent); re-running the allocation audit on a migrated crate must report zero violations (CLN-02). |
| Auth boundaries & rate limits | **N/A because** no auth surface exists in this project. |
| Concurrency / ordering | Covered: CLN-10 — relocated tests must not introduce shared-state coupling; a test moved into an inline module runs in the same process as its neighbors, so any test relying on process-global state (env vars, the Python facade lock, cwd) keeps its serialization guard. |
| Data lifecycle / expiry | **N/A because** no persisted data is involved; scratch dirs in touched tests keep their existing cleanup. |
| Observability | Covered: CLN-08 — the guard test makes ignored/dead-test drift observable in the suite itself instead of a doc. |
| External-dependency failure | **N/A because** the suite's external-tool tests (ngspice cross-checks) are untouched by this feature and already skip when the tool is absent. |
| State-transition integrity | **N/A because** no state machine is introduced or modified. |

---

## User Stories

### P1: Tests live where MD-28 says ⭐ MVP

**User Story:** As the maintainer, I want every test allocated by rule — unit
inline, integration grouped by functionality — so a refactor's regressions
show up in the file I am already reading instead of a distant suite.

**Why P1:** This is the row the user added by hand and the stated prerequisite
for later refactors; the rest of P6 is small by comparison.

**Acceptance Criteria:**
1. WHEN the allocation audit runs over a crate THEN it SHALL classify every
   test as *unit* or *integration* by the Assumptions-table definition and
   report each misallocated test as `file::test → target location`, with the
   whole-workspace report committed as the feature's working record.
2. WHEN a unit test is found in an integration file THEN it SHALL be moved
   into a `#[cfg(test)] mod tests` in the implementation file that owns its
   subject, keeping its name and its assertions unchanged.
3. WHEN an integration file mixes unrelated functionality THEN its tests SHALL
   be regrouped so each file's name names the functionality it covers, and no
   test is left in a file whose name does not describe it.
4. WHEN a crate's migration is committed THEN `cargo test -p <crate>` SHALL be
   green in that same commit, and the workspace suite SHALL be green at the
   end of every phase.
5. WHEN the migration of a crate is complete THEN re-running the audit on that
   crate SHALL report **zero** violations.

**Independent Test:** the audit report for a migrated crate is empty, and that
crate's tests pass with the same test names present before and after
(`cargo test -p <crate> -- --list` diff shows only intended moves/deletions).

---

### P1: No dead or silently-skipped test code ⭐ MVP

**User Story:** As the maintainer, I want zero switched-off test code, so what
the suite claims to cover is what it actually runs.

**Why P1:** 602 lines of `ppr_ir.rs` plus 412 of `analog_jit.rs` look like coverage and
are not compiled at all — the most misleading artifact in the tree.

**Acceptance Criteria:**
1. WHEN the tree is searched for disabled test code THEN there SHALL be no
   `#![cfg(any())]`, no `#[cfg(FALSE)]`-style switch, and no commented-out
   test module in any test target or `#[cfg(test)]` module.
2. WHEN each of the 38 switched-off tests (`ppr_ir.rs` 27, `analog_jit.rs` 11
   — the latter named in `CLAUDE.md` as a test of record) is triaged THEN it SHALL
   be either (a) restored against the current `piperine_codegen::resolve` API
   and passing, or (b) deleted with the surviving `file::test` that already
   asserts the same behavior named in the commit message — never dropped
   without one of the two.
3. WHEN the triage is complete THEN each of the two files SHALL either not
   exist or exist as a normally-compiled, fully-passing target with no
   `ignore` attributes.
4. WHEN a behavior in the restored set has no current equivalent AND cannot be
   expressed through today's `resolve` API THEN the task SHALL fail loud in
   the report (a named, tracked gap), never silently delete the test.

**Independent Test:** `grep -rn 'cfg(any())' crates tests` is empty and the
codegen suite runs the triaged tests (visible in the test list).

---

### P1: Ignored tests cannot rot silently ⭐ MVP

**User Story:** As the maintainer, I want every `ignore` to justify itself and
the count to be guarded, so ignored tests stop being a place where work hides.

**Acceptance Criteria:**
1. WHEN the suite runs THEN every ignored item SHALL be a documentation
   example annotated `ignore = "<reason>"` (the 4 known: `piperine-plugin`
   `lib.rs` entry example, `piperine-solver` `ac.rs`, `builder.rs`,
   `prelude.rs`), and no `#[test] #[ignore]` SHALL exist.
2. WHEN a plain `#[ignore]` (no reason) or a new `#[ignore]`d `#[test]` is
   introduced THEN a guard test SHALL fail, naming the offending file.
3. WHEN the guard runs THEN it SHALL read the repository sources (not a
   hardcoded list of names) so it cannot pass stale.

**Independent Test:** adding `#[ignore]` to any test makes the guard fail;
removing it makes the guard pass.

---

### P2: Every capability bit is enforced or removed

**User Story:** As a solver author, I want no `ElementCapabilities` bit that
nothing consumes, so the ABI stops carrying promises it does not keep.

**Acceptance Criteria:**
1. WHEN `SUPPORTS_QUERIES` is dispositioned THEN it SHALL either gain a real
   solver/host consumer with a test that fails when the bit is unset, or be
   deleted from the bitflags together with every declaration site.
2. WHEN `BYPASS_OK` is dispositioned THEN the same enforce-or-remove rule
   SHALL apply, and the choice SHALL be recorded with its evidence.
3. WHEN a bit is removed THEN `capabilities_contract.rs` SHALL still be
   exhaustive over `ElementCapabilities::all()`, and no remaining registry
   entry SHALL read "no consumer today".
4. WHEN a bit is removed THEN every device that set it (codegen devices,
   plugin fixtures, tests) SHALL stop setting it, and the workspace SHALL
   build with zero warnings.
5. WHEN `bound_step_hint` is reviewed THEN it SHALL be recorded as **enforced**
   (with its call sites) and left in place — the ROADMAP's dead-code note is
   corrected, not acted on.

**Independent Test:** `capabilities_contract.rs` passes with no
"no consumer" entry, and a grep for the removed bit's name returns nothing.

---

### P2: Every Part VII §16 failure row has a test

**User Story:** As a spec reader, I want each normative failure row to be
enforced by a test, so the spec table is a contract instead of a wish.

**Acceptance Criteria:**
1. WHEN the §16 table is audited THEN each of its 16 rows (measured count; an
   earlier draft of this spec said 18) SHALL be marked
   *enforced* (naming the `file::test` that trips it) or *unenforceable*
   (naming why no public surface can trigger it).
2. WHEN a row is enforced-but-untested THEN a test SHALL be added that trips
   exactly that failure and asserts the typed error, in the suite that owns
   the analysis.
3. WHEN a row is unenforceable through any public surface THEN the row SHALL
   be removed from `docs/spec/part_vii_solver.md` §16 with a one-line note
   saying so — the table SHALL NOT retain rules nothing can reach.
4. WHEN the audit is complete THEN a guard SHALL assert the §16 row count
   matches the number of enforcement tests registered, so a new normative row
   without a test fails the suite.

**Independent Test:** the §16 guard passes; deleting one enforcement test
makes it fail naming the orphaned row.

---

### P3: ROADMAP P6 restated with verified numbers

**User Story:** As the next reader of the roadmap, I want P6 to state facts
that match the tree, so I stop planning against stale counts.

**Acceptance Criteria:**
1. WHEN P6 is read after this feature THEN the test count, the ignored-test
   claim, and the `bound_step_hint` claim SHALL match the measured reality,
   and the delivered rows SHALL be checked off with the residue (the
   completeness items) explicitly left as post-V1.
2. WHEN the feature closes THEN `.specs/STATE.md` SHALL carry its handoff
   entry and any new macro decision it forces.

**Independent Test:** doc review gate.

---

## Edge Cases

- WHEN a unit test's subject is a **private** item reachable only from an
  integration test through a `pub` façade THEN it SHALL move inline next to
  the private item (the inline module can see it) rather than stay integration.
- WHEN moving a test inline would pull heavy fixtures (a JIT compile, a full
  elaboration, an ngspice run) into the crate's unit build THEN the test is
  **integration by definition** and stays in `tests/`, regrouped by
  functionality — the split is by what the test needs, never by file size.
- WHEN two tests assert the same behavior at different layers (e.g. a lang
  parse assertion and a host-level end-to-end) THEN both are kept: layer
  duplication is coverage, not redundancy. Only same-layer, same-assertion
  duplicates are deleted.
- WHEN a relocated test depends on a test-local helper THEN the helper moves
  with it (or is duplicated as a private helper) — no new shared test-utility
  crate is introduced by this feature.
- WHEN a moved test needs process-global state (env var, cwd, the Python
  facade lock) THEN it SHALL keep or gain the serialization guard its old home
  provided, so inline co-location does not introduce flakiness.
- WHEN a `#[cfg(test)]` inline module would exceed the implementation file's
  readable size THEN the tests may live in a sibling `mod tests` file included
  by the implementation module (`#[cfg(test)] mod tests;` + `tests.rs`) —
  still inline by MD-28's meaning (same module tree), not a distant target.
- WHEN the audit finds a test that asserts nothing (no `assert!`/`expect` on a
  meaningful value) THEN it SHALL be reported and either strengthened or
  deleted with rationale — a test that cannot fail is not coverage.

---

## Requirement Traceability

| ID | Story | Phase | Status |
|----|-------|-------|--------|
| CLN-01 | P1 allocation audit report (whole workspace) | — | Pending |
| CLN-02 | P1 migrated crate re-audits clean | — | Pending |
| CLN-03 | P1 unit tests moved inline (name + assertions preserved) | — | Pending |
| CLN-04 | P1 integration files regrouped by functionality; same-layer duplicates deleted only with a named survivor | — | Pending |
| CLN-05 | P1 no `cfg(any())` / commented-out test code anywhere | — | Pending |
| CLN-06 | P1 `ppr_ir.rs`'s 27 tests restored or deleted-with-survivor | — | Pending |
| CLN-07 | P1 un-restorable behavior is a loud tracked gap, never a silent delete | — | Pending |
| CLN-08 | P1 ignore guard: every `ignore` has a reason, no ignored `#[test]`, guard reads sources | — | Pending |
| CLN-09 | P1 suite green in every commit; per-crate gate per commit | — | Pending |
| CLN-10 | P1 relocated tests keep their global-state serialization guards | — | Pending |
| CLN-11 | P2 `SUPPORTS_QUERIES` enforced or removed | — | Pending |
| CLN-12 | P2 `BYPASS_OK` enforced or removed | — | Pending |
| CLN-13 | P2 `capabilities_contract.rs` exhaustive, zero "no consumer" entries | — | Pending |
| CLN-14 | P2 removed bit purged from every declaration site; zero warnings | — | Pending |
| CLN-15 | P2 `bound_step_hint` recorded as enforced (ROADMAP correction) | — | Pending |
| CLN-16 | P2 §16 rows classified enforced / unenforceable with evidence | — | Pending |
| CLN-17 | P2 missing §16 enforcement tests added (typed error asserted) | — | Pending |
| CLN-18 | P2 unreachable §16 rows removed from the spec table | — | Pending |
| CLN-19 | P2 §16 guard: row count ↔ enforcement tests | — | Pending |
| CLN-20 | P3 ROADMAP P6 restated with verified numbers + rows checked off | — | Pending |
| CLN-21 | P3 `.specs/STATE.md` handoff + decisions updated | — | Pending |

21 requirements.

---

## Success Criteria

- [ ] Allocation audit reports zero violations workspace-wide; no
      `cfg(any())` test code; no `#[test] #[ignore]`.
- [ ] `cargo test --workspace` green with **≥ 1123** passing tests (the count
      may only drop by tests deleted with a named surviving equivalent, each
      listed in the feature's validation record), zero build warnings.
- [ ] Every `ElementCapabilities` bit and every §16 row is enforced or gone,
      each with named evidence.
- [ ] Two new guards (ignore-policy, §16 coverage) fail when their invariant
      is violated — proven by a deliberate violation during validation.
- [ ] ROADMAP P6 + `.specs/STATE.md` match the tree.
