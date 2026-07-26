# P6 Cleanup — Architecture & Readability Specification

**Source document:** `CLEANUP_PLAN.md` (root, approved by the user 2026-07-26).
Findings there are `CL-01..CL-13`; requirements here are `CLA-nn` and cite them.
**Scope tier:** Large/Complex. **ROADMAP:** P6's architecture/readability subset
(the hygiene subset closed 2026-07-26 as `p6-cleanup-completeness`).

## Problem Statement

The workspace passes 1161 tests with zero warnings, but the tree no longer
teaches a reader how the project works. Three concrete failures: the canonical
host crate ships **two** competing session surfaces in one 1329-line file;
twelve blanket `#![allow(dead_code)]` hide 22 dead items — including a
locked-but-unbuilt macro decision (MD-03); and 332 module-level free functions
in `codegen`/`lang`/`lang-server` mark abstractions that were never extracted
(MD-13 rule 2), while `piperine-solver` proves the house style works at the same
scale with 5. A newcomer cannot answer "where does this belong?" from the tree,
and neither can the author.

## Goals

- [ ] One host entry point in `piperine-api`, canonical for both hosts (MD-22/MD-27).
- [ ] The Rust host owns the navigable object model; Python becomes pure delegation.
- [ ] Zero dead items and zero file-scope lint suppression in `src`.
- [ ] Zero comments that define code by a deleted architecture.
- [ ] Module-level free functions reduced to owner-bearing methods/traits across
      `codegen`, `lang`, `lang-server` (MD-13 rule 2).
- [ ] No `mod.rs` carrying an implementation; no function over ~200 lines.
- [ ] Every rule above enforced by a test, not a paragraph (MD-31).
- [ ] `cargo test --workspace` stays at its pre-feature count with 0 failures
      throughout, and the ngspice cross-checks stay bit-comparable.

## Out of Scope

| Item | Reason |
|---|---|
| Any behavior change | Refactor-only feature; the suite + ngspice baselines are the oracle |
| New/removed crates | The 10-crate topology is sound; wrong seams are *inside* `piperine-api` |
| Grammar or parsed-language changes | `parse/` decomposition splits functions, never what they accept |
| `resolve/diff.rs` differentiation semantics | Correctness-critical core; only its callers' shape may move |
| Implementing MD-03's per-analysis contexts | User decision D2: kill + supersede instead |
| New analyses, new host capability | Except CL-02's object-model lift, which is a *relocation* of existing Python capability |
| `.specs/` feature-doc rewriting | Beyond status reconciliation (CLA-07) |
| `docs/manual/` authoring | CLA-07 only decides its fate (keep-with-backlog or remove) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| **D1 (CL-01)** Which host entry survives | `Session` survives; `SimSession` deleted | Ideal-first surface per MD-27; `SimSession` is the pre-`host-library` shape | **y** (user, 2026-07-26) |
| **D2 (CL-06.3)** MD-03's four `*Context` structs | Delete them + the four dead `*Analysis` traits; mark MD-03 superseded | Wiring per-analysis contexts is runtime architecture, outside a refactor-only feature | **y** (user) |
| **D3 (CL-02)** Host object model | Lift `Design`/`Module`/`Instance`/`Port`/`Net`/`Param` into `piperine-api`; Python delegates | MD-27 §1 api-canonical; kills the inversion at the root | **y** (user) |
| **D4** Feature scope | Phases A–D complete, including the `lang`/`lang-server` free-function grind and both >200-line parser functions | User chose full scope | **y** (user) |
| **D5** `SimSession`'s non-`Session` methods | Ported onto `Session` (`stage`, `snapshot_digital`, `snapshot_opvars`, `snapshot_introspect`, `set_device_provider`, `set_hooks`) before deletion, keeping names | Deleting capability is not in scope; only the entry object collapses | n (agent default) |
| **D6** Python-facing names | Unchanged (`_Design`, `_Module`, `.op()`, `.tran()`, …) — only their Rust backing moves | `tests/host_parity.rs` + the published Python API must not churn | n (agent default) |
| **D7** Parser decomposition target | "One dispatch level per function": each match arm one call; not a hard 60-line cap | A recursive-descent primary parser is a dispatch table by nature | n (agent default) |
| **D8** Free-function target | Every module-level `fn` in `codegen`/`lang`/`lang-server` gains an owner **or** is justified in a one-line comment naming why no owner exists (e.g. a `#[pymodule]`/CLI entry point) | MD-13 rule 2 is about missing abstractions, not about banning entry points | n (agent default) |
| **D9** Baseline test count | The pre-feature `cargo test --workspace` count is captured in task T1 and asserted unchanged (modulo tests *added* by this feature) at every gate | Refactor-only means the count never *drops* | n (agent default) |

**Open questions:** none — all resolved or logged above.

**Concern on record (user reaffirmed full scope, so this ships as specified):**
D3 (object-model lift) delivers capability relocation inside a refactor-only
feature, and D4 pulls `parse/` — a file CLAUDE.md marks "not to edit casually" —
into scope. Both are executed as specified; the mitigation is that each lands
as its own atomic commit with the frozen-corpus parser tests as oracle, and
`tests/host_parity.rs` guards the host surface.

**Implicit-requirement dimensions sweep (Large tier — every dimension resolved):**

| Dimension | Resolution |
|---|---|
| Input validation & bounds | N/A because no new input surface; the object-model lift preserves existing validation paths verbatim |
| Failure / partial-failure states | Covered by CLA-14: every `Session` method ported from `SimSession` keeps its `Result`/`Error` shape; no error type is widened or narrowed |
| Idempotency / retry / duplicate handling | Covered by CLA-13: `Session`'s compile-once invariant (MD-18) must survive the merge — `rebuilds()` stays observable and `urc_compile_count.rs` stays green |
| Auth boundaries & rate limits | N/A because no network/multi-user surface exists in the workspace |
| Concurrency / ordering | Covered by CLA-19: the `Once` init moved out of `analyses/mod.rs` must keep one-time semantics (MD-06) |
| Data lifecycle / expiry | N/A because nothing here persists state beyond a process |
| Observability | Covered by CLA-04: deleting dead traits/structs must not remove any live tracing/`SolverStats` surface |
| External-dependency failure | N/A because no external service is touched; the ngspice cross-check is a dev-time oracle, not a runtime dependency |
| State-transition integrity | Covered by CLA-13: the `Session` state machine (compile → set/schedule_set → analysis → sweep) keeps every currently reachable transition |

---

## User Stories

### P1: Dead surface deleted and kept dead ⭐ MVP

**User Story**: As the maintainer, I want every item in `src` to have a live
consumer, so that reading the tree tells me what the system actually does.

**Why P1**: 22 dead items hid behind 12 blanket allows, including a fictional
trait-based analysis abstraction that misleads anyone reading for the contract.

**Acceptance Criteria**:

1. WHEN `cargo check --workspace --all-targets` runs THEN the workspace SHALL
   emit zero `dead_code` warnings **with no file-scope `#![allow(dead_code)]`
   present anywhere in `crates/*/src` or `src`.
2. WHEN the tree is searched for `#![allow(` THEN the search SHALL return no
   hit under `crates/*/src` or `src`.
3. WHEN an item-scope `#[allow(dead_code)]` exists THEN it SHALL carry a
   one-line comment naming why the item has no consumer yet.
4. WHEN `crates/piperine-solver/src/math/constant.rs` and
   `crates/piperine-solver/src/core/port.rs` are looked up THEN they SHALL not
   exist (whole-file deletions, zero consumers).
5. WHEN `DcAnalysis`, `AcAnalysis`, `TransientAnalysis`, `NoiseSource`,
   `AcContext`, `TransientContext`, `NoiseContext`, `TfContext` are searched for
   THEN they SHALL not exist.
6. WHEN a contributor adds a new file-scope `allow` THEN the hygiene guard SHALL
   fail naming the file.

**Independent Test**: `tests/suite_hygiene.rs` gains a scan asserting no
file-scope `allow` in any crate `src`; `cargo check --workspace --all-targets`
is clean; the named symbols grep empty.

---

### P1: Comments and docs describe the present ⭐ MVP

**User Story**: As a reader, I want no comment to define code by an
architecture that was deleted, so that I can verify every statement against the
tree in front of me.

**Why P1**: 16 comments reference the removed IR crate; two are intra-doc links
to types that do not exist, which is actively wrong, not merely stale.

**Acceptance Criteria**:

1. WHEN `crates/piperine-codegen/src/device/mod.rs` is read THEN its
   `CircuitCompiler` doc SHALL name the POM types it actually walks, with no
   link to `crate::resolve::IrProgram`.
2. WHEN `crates/piperine-lang/src/math.rs` is read THEN its header SHALL name
   the live const-evaluator entry point, not `IrExpr::eval_const`.
3. WHEN `src` is searched for `IrProgram|IrModule|IrExpr|IrInstance|piperine-ir|piperine_ir`
   THEN at most **one** hit SHALL remain — a single historical note in
   `crates/piperine-codegen/src/lib.rs` where the pipeline is introduced.
4. WHEN `cargo doc --workspace --no-deps` runs THEN it SHALL emit zero broken
   intra-doc-link warnings.
5. WHEN a contributor reintroduces any of the dead identifiers in a comment THEN
   the hygiene guard SHALL fail naming the file and identifier.
6. WHEN `CLAUDE.md` is read THEN it SHALL contain no hand-maintained test count
   and no hand-maintained "tests of record" file list that a guard does not
   enforce.
7. WHEN `.specs/STATE.md` is read THEN `solver-simplification`'s status SHALL
   reflect reality (batch 6 delivered, or explicitly recorded as the remaining
   residue with what is left).
8. WHEN `docs/manual/` is inspected THEN it SHALL either contain authored
   content with a tracked backlog entry, or not exist (with `mkdocs.yml`
   updated accordingly).

**Independent Test**: guard test greps the dead identifiers and file-scope
allows; `cargo doc` clean; manual read of the three docs.

---

### P1: Everything sits where its name says ⭐ MVP

**User Story**: As a reader, I want the file tree to name what each file holds,
so that I can find any struct without grepping.

**Why P1**: MD-13 rule 4's golden rule fails on four `mod.rs` files holding
1237/864/859/615 lines of implementation.

**Acceptance Criteria**:

1. WHEN any `mod.rs` under `crates/*/src` is measured THEN it SHALL be at most
   60 lines and SHALL contain only module declarations, re-exports, and the
   layer's `//!` contract.
2. WHEN `AnalogInstance`, `AnalogKernel`, `CompiledModule`, `PiperineDevice`,
   and `LoweredBody` are located THEN each SHALL live in a file named after it
   (not `mod.rs`).
3. WHEN `Tolerances`, `Context`, and `Policy` are located THEN they SHALL live
   in the solver's config/context home, not in `analyses/mod.rs`.
4. WHEN `crates/piperine-solver/src/analog/` is inspected THEN the one-file
   directory SHALL be collapsed to a single module file, and `result.rs` SHALL
   live under `core/`.
5. WHEN a contributor grows a `mod.rs` past the limit THEN the hygiene guard
   SHALL fail naming the file and its line count.
6. WHEN `crates/piperine-python/src/lib.rs` is measured THEN it SHALL be a
   façade of at most ~150 lines, with its former inline test block living in
   `crates/piperine-python/tests/` split by feature with fixtures in
   `tests/common/`.
7. WHEN root `tests/` is inspected THEN no target SHALL exercise only
   non-root crates: `sens.rs`, `pss.rs`, and `transient_reentry.rs` SHALL live
   in `crates/piperine-solver/tests/`.
8. WHEN root `tests/` naming is checked THEN every target SHALL match the
   declared rule (`host_*.rs` = host-surface proof; `<feature>.rs` = shell or
   cross-crate proof) and the rule SHALL be enforced by `suite_hygiene.rs`.

**Independent Test**: guard asserts `mod.rs` size + root-test naming; the moved
targets run green in their new crates; `cargo test --workspace` count unchanged.

---

### P1: One host entry point ⭐ MVP

**User Story**: As a host author (Rust or Python), I want exactly one session
type to learn, so that "how do I run an analysis?" has one answer.

**Why P1**: `SimSession` and `Session` coexist with overlapping analysis menus;
Python binds the older one; MD-27 exists to prevent exactly this.

**Acceptance Criteria**:

1. WHEN `piperine-api`'s public surface is enumerated THEN `SimSession` SHALL
   not appear, and `Session` SHALL be the only session type.
2. WHEN `Session` is used THEN it SHALL expose every capability `SimSession`
   had — `stage`, `snapshot_digital`, `snapshot_opvars`, `snapshot_introspect`,
   `set_device_provider`, `set_hooks` — under the same names and with the same
   `Result`/`Error` shapes.
3. WHEN `crates/piperine-api/src/` is inspected THEN the session surface SHALL
   be split by role: the entry object, `sweep.rs` (`Sweep`/`SweepPoint`/
   `Grid`/`Nested`), and the run-config types, with no file over ~700 lines.
4. WHEN `piperine-python` is built THEN it SHALL reference `Session` only, and
   every Python-facing name SHALL be unchanged (D6).
5. WHEN `tests/host_parity.rs` runs THEN it SHALL pass with no divergence
   between the Rust and Python surfaces.
6. WHEN a live parameter is set and an analysis re-run THEN the compile-once
   invariant SHALL hold — `rebuilds()` observable and `tests/urc_compile_count.rs`
   green (MD-18).
7. WHEN every reachable `Session` transition is exercised (compile → set /
   schedule_set → analysis → sweep) THEN each SHALL behave as it did before the
   merge, proven by the existing session test targets retargeted to `Session`.

**Independent Test**: the ~24 root targets that used `SimSession` compile and
pass against `Session`; `host_parity.rs` green; grep for `SimSession` empty.

---

### P1: The object model is api-canonical ⭐ MVP

**User Story**: As a Rust host author, I want to navigate a design the way
Python does, so that the two hosts are one API in two languages (MD-22).

**Why P1**: MD-27 §1 makes `piperine-api` the single source of truth; today the
navigable model exists only in `piperine-python` (1436 LOC), inverting the rule.

**Acceptance Criteria**:

1. WHEN `piperine-api` is used from Rust THEN it SHALL expose the navigable
   model Python has: design load + `top`/`module`/`modules`/`const_`/`select`,
   module `name`/`ports`/`nets`/`instances`/`params`/`behaviors` + the analysis
   menu + `compile`, and the `Port`/`Net`/`Instance`/`Param` descriptors.
2. WHEN `piperine-python`'s `design.rs`, `module.rs`, and `instance.rs` are read
   THEN every method SHALL be a delegation to the api type — the shape
   `results.rs` already uses — with no POM traversal or analysis logic of its
   own.
3. WHEN a Python-facing name, signature, default, or returned type is compared
   before and after THEN it SHALL be identical (D6).
4. WHEN `tests/host_parity.rs` runs THEN it SHALL enumerate the lifted model on
   both sides and pass.
5. WHEN the api-side model is exercised from Rust THEN a new root integration
   target SHALL prove each lifted capability (load → module → analysis →
   instance view → opvar) without going through Python.
6. WHEN elaboration output is navigated THEN the authored hierarchy SHALL be
   what the model exposes (MD-25) — the lift SHALL NOT surface `flat_modules`.

**Independent Test**: new `tests/host_object_model.rs` drives the whole path in
Rust; the Python suite (`crates/piperine-python/tests/`) passes unchanged.

---

### P2: Missing abstractions extracted in codegen

**User Story**: As a codegen maintainer, I want each helper owned by the type it
serves, so that a change has one obvious home.

**Why P2**: 112 module-level free functions cluster into three named missing
abstractions; the code works today, so this is debt, not breakage.

**Acceptance Criteria**:

1. WHEN expression construction helpers (`select`, `binary`, `lit`, `not_expr`,
   `and_guards`, `subst_expr`, `subst_block`, `subst_scope`,
   `substitute_marker`) are located THEN they SHALL be methods on one owning
   builder type or inherent constructors on the resolved expression type — no
   module-level `fn`.
2. WHEN expression query helpers (`has_branch_current`, `has_branch_access`,
   `has_marker`, `expr_eq`, `expr_structural_eq`, `blocks_eq`, `stmts_eq`,
   `isolate_branch_coeff`) are located THEN they SHALL be inherent methods or
   one query trait on the expression/statement types, with the three
   structural-equality functions unified into one algorithm.
3. WHEN the limits collection walk (`collect_limits`, `limit_branch`,
   `limit_branches_into`, `ident_of`) is located THEN it SHALL be one owning
   collector type.
4. WHEN `crates/piperine-codegen/src` is scanned THEN every remaining
   module-level `fn` SHALL carry a one-line justification comment (D8).
5. WHEN the codegen suite runs THEN it SHALL pass unchanged — the extractions
   are behavior-preserving.

**Independent Test**: the free-function scan for `piperine-codegen` reports only
justified entries; `cargo test -p piperine-codegen` green at its prior count.

---

### P2: Long functions decomposed

**User Story**: As a reader, I want to understand one operation without
scrolling, so that a function's name is a promise about its size.

**Why P2**: four functions exceed 200 lines; the pattern that fixes them
(`analyses/transient.rs`'s named phase methods) is already proven in-tree.

**Acceptance Criteria**:

1. WHEN `lower_bodies`, `extract_symbols`, `parse_primary`, and `parse_mod_stmt`
   are measured THEN each SHALL be at most 60 lines **or**, for the two parser
   functions, SHALL be a dispatch whose every arm is a single call to a named
   method (D7).
2. WHEN any function in `crates/*/src` is measured THEN none SHALL exceed 200
   lines.
3. WHEN the parsers are exercised THEN the frozen corpora (`headers/`,
   `tests/fixtures*`) SHALL parse to identical results — proven by the existing
   `parse_elab.rs`/`elab.rs` suites passing unchanged.
4. WHEN the decomposition lands THEN each extracted piece SHALL be a method on
   the parser/lowering type, not a new free function (MD-13 rule 2).

**Independent Test**: a function-length scan in `suite_hygiene.rs` asserting the
200-line ceiling; parser and codegen suites green.

---

### P2: Free-function debt cleared in lang and lang-server

**User Story**: As a maintainer, I want the same ownership rule applied
everywhere, so that `piperine-solver` stops being the exception.

**Why P2**: 75 + 66 free functions; large but mechanical, and behind the P1 work
in value.

**Acceptance Criteria**:

1. WHEN `crates/piperine-lang/src` is scanned THEN every module-level `fn` SHALL
   either belong to a trait/struct or carry a one-line justification (D8).
2. WHEN `crates/piperine-lang-server/src` is scanned THEN the same SHALL hold,
   with handler helpers owned by `DocumentState`, `SymbolIndex`, or the handler
   type they serve.
3. WHEN the scan runs as a test THEN it SHALL fail on any new unjustified
   module-level `fn` in the crates it covers.
4. WHEN both suites run THEN they SHALL pass at their prior counts.

**Independent Test**: the guard scan is the test; both crate suites green.

---

### P3: The rules become MD entries

**User Story**: As a future contributor, I want these invariants locked in
`STATE.md`, so that the cleanup is not redone in six months.

**Acceptance Criteria**:

1. WHEN `.specs/STATE.md` is read THEN it SHALL contain **MD-33** (no
   file-scope lint suppression), **MD-34** (`mod.rs` declares, never
   implements), and **MD-35** (comments describe the present), each naming its
   enforcing guard.
2. WHEN MD-03 is read THEN it SHALL be marked superseded by this feature, citing
   D2 and the deleted structs.
3. WHEN MD-22/MD-27 are read THEN they SHALL record that the object model is now
   api-canonical, citing D3.
4. WHEN MD-20 is read THEN it SHALL record `Session` as the single host entry,
   citing D1.
5. WHEN each new guard is added THEN it SHALL be proven able to fail —
   violation injected, failure observed, reverted — and the proof noted in
   `validation.md` (MD-31).

---

## Edge Cases

- WHEN a `mod.rs` legitimately needs more than declarations (e.g. a `#[cfg]`
  shim) THEN the guard SHALL accept it only with an inline justification comment
  the guard recognizes.
- WHEN a lifted api model type would need to expose `flat_modules` to answer a
  Python query THEN the lift SHALL fail loud rather than surface the flat
  artifact (MD-25).
- WHEN deleting `SimSession` reveals a capability with no `Session` equivalent
  THEN the capability SHALL be ported first and the deletion deferred to a later
  task — never dropped silently.
- WHEN a parser decomposition would change a parse result for any frozen corpus
  input THEN the task SHALL be reverted and re-approached; the corpora are the
  contract.
- WHEN an extracted abstraction would need a macro to avoid repetition THEN the
  repetition SHALL stay and a data table + plain helper SHALL be used instead
  (MD-13 rule 5).
- WHEN a moved test target reveals it duplicates an existing test in its new
  home THEN the weaker one SHALL be deleted (MD-28 rule 3) and the deletion
  named in the commit.
- WHEN the workspace test count would drop THEN the task SHALL stop and account
  for every missing test before proceeding (D9).

---

## Requirement Traceability

| ID | Story | Finding | Phase | Status |
|---|---|---|---|---|
| CLA-01 ✅ | P1 Dead surface | CL-06 | A | Verified |
| CLA-02 ✅ | P1 Dead surface | CL-06, CL-07 | A | Verified |
| CLA-03 ✅ | P1 Dead surface | CL-06 (whole files) | A | Verified |
| CLA-04 ✅ | P1 Dead surface | CL-06 (traits/contexts, D2) | A | Verified |
| CLA-05 ✅ | P1 Dead surface | CL-06.4 (guard, MD-33) | A | Verified |
| CLA-06 ✅ | P1 Comments/docs | CL-08 (broken links) | A | Verified |
| CLA-07 ✅ | P1 Comments/docs | CL-08, CL-09, CL-10 | A | Verified |
| CLA-08 ✅ | P1 Comments/docs | CL-08 (guard, MD-35) | A | Verified |
| CLA-09 ✅ | P1 Placement | CL-11 (`mod.rs`) | B | Verified |
| CLA-10 ✅ | P1 Placement | CL-05, CL-04 (solver homes) | A/B | Verified |
| CLA-11 | P1 Placement | CL-13 (python façade) | B | Pending |
| CLA-12 | P1 Placement | CL-03 (root tests) | B | Pending |
| CLA-13 | P1 Placement | CL-11/CL-03 (guards, MD-34) | B | Pending |
| CLA-14 | P1 One host entry | CL-01 (D1, D5) | C | Pending |
| CLA-15 | P1 One host entry | CL-01 (file split) | C | Pending |
| CLA-16 | P1 One host entry | CL-01 (python retarget, D6) | C | Pending |
| CLA-17 | P1 Object model | CL-02 (D3, lift) | C | Pending |
| CLA-18 | P1 Object model | CL-02 (python delegation) | C | Pending |
| CLA-19 | P1 Object model | CL-02 (Rust proof target) | C | Pending |
| CLA-20 | P2 Codegen abstractions | CL-10.1 (builder) | D | Pending |
| CLA-21 | P2 Codegen abstractions | CL-10.2 (query surface) | D | Pending |
| CLA-22 | P2 Codegen abstractions | CL-10.3 (limits collector) | D | Pending |
| CLA-23 | P2 Long functions | CL-12 (`lower_bodies`, `extract_symbols`) | D | Pending |
| CLA-24 | P2 Long functions | CL-12 (parsers, D7) | D | Pending |
| CLA-25 | P2 Long functions | CL-12 (200-line guard) | D | Pending |
| CLA-26 | P2 Free-fn debt | CL-10 (`lang`) | D | Pending |
| CLA-27 | P2 Free-fn debt | CL-10 (`lang-server`) | D | Pending |
| CLA-28 | P2 Free-fn debt | CL-10 (guard, D8) | D | Pending |
| CLA-29 | P3 MD entries | plan §8 | D | Pending |
| CLA-30 | P3 MD entries | D1/D2/D3 amendments | D | Pending |

**Coverage:** 30 requirements. Mapping to tasks happens in `tasks.md`.

---

## Success Criteria

- [ ] `cargo test --workspace`: 0 failures, count ≥ the T1-captured baseline, at
      every commit.
- [ ] `cargo check --workspace --all-targets` and `cargo clippy --workspace`:
      zero warnings, with no file-scope suppression in `src`.
- [ ] `cargo doc --workspace --no-deps`: zero broken intra-doc links.
- [ ] `grep -rn "SimSession" crates src tests` → empty.
- [ ] Module-level free functions in `codegen`/`lang`/`lang-server`: every
      remaining one justified, enforced by a guard.
- [ ] Largest `mod.rs` ≤ 60 lines; largest function ≤ 200 lines; both guarded.
- [ ] ngspice cross-check suite green and numerically unchanged.
- [ ] Five guards added, each proven able to fail (MD-31), noted in
      `validation.md`.
