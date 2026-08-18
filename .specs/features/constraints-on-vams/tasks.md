# Constraints on VAMS — Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, adequacy review, Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed without it.**

---

**Design**: `.specs/features/constraints-on-vams/design.md`
**Status**: Done

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec. Guidelines found: `AGENTS.md` (MD-28 test placement table, "Build and verify" bar), `CLAUDE.md`, `tests/suite_hygiene.rs` (mechanical placement gate), `HANDOFF.md` §1 (A/B + discrimination-check disciplines — binding on every new test).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| `piperine-lang` grammar + elab (constraint block) | integration | Every parse/elab AC + edge case (CONVAMS-01, 14, 15-parse-half): parses, attrs/doc carried, `tol` clause, non-`require` statement typed error, keyword-as-label rejected, duplicate label E4007, multiple blocks compose, no-block module unchanged | `crates/piperine-lang/tests/vams_constraint_block.rs` (new, follows `vams_*.rs` pattern) | `CARGO_TARGET_DIR=/var/tmp/piperine-target cargo test -p piperine-lang --test vams_constraint_block` |
| `piperine-codegen` resolve (margin producer) | integration | Margin-form ACs (CONVAMS-06, 07): every comparison form's lowered shape + value-level sign check at satisfied/violated/boundary; loud errors for unsupported forms + unresolved names; existing restored `margin_*` suite migrated where require-only | `crates/piperine-codegen/tests/margin_*.rs` (restored this session) | `CARGO_TARGET_DIR=/var/tmp/piperine-target cargo test -p piperine-codegen` |
| `piperine-api` catalog + margins channel | integration | Catalog ACs (CONVAMS-09, 13): compute non-empty, digital classification, existing surface unchanged; un-blocked require-subset of the 66 | `crates/piperine-api/tests/` (existing dv-core targets, `//!` blocked notes updated) | `CARGO_TARGET_DIR=/var/tmp/piperine-target cargo test -p piperine-api` |
| host e2e (session, postures, analyses) | e2e | All P1/P3 ACs end to end (CONVAMS-02..05, 08, 10..14): strict abort naming constraint/instance/margin, collect records, off zero-cost, worst+argmin over sweep, rejected-iteration silence, per-instance paths, mid-hierarchy module | `tests/constraints_e2e.rs` (new, root — host surface per MD-28) | `CARGO_TARGET_DIR=/var/tmp/piperine-target cargo test --test constraints_e2e` |
| python parity | e2e | The blocked require-subset un-blocks; posture visible from Python | `crates/piperine-python/tests/` | `CARGO_TARGET_DIR=/var/tmp/piperine-target cargo test -p piperine-python` |
| docs (`docs/spec`, `.specs/STATE.md`, `ROADMAP.md`) | none | — (build + hygiene gate only) | — | `CARGO_TARGET_DIR=/var/tmp/piperine-target cargo test --test suite_hygiene` |

## Gate Check Commands

> From `AGENTS.md` ("Build and verify"): zero warnings is the bar; bare `cargo test` runs only the root package — always `-p` or `--workspace`.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | lang / codegen tasks | `CARGO_TARGET_DIR=/var/tmp/piperine-target cargo test -p piperine-lang -p piperine-codegen` |
| Full | api / e2e / python tasks | `CARGO_TARGET_DIR=/var/tmp/piperine-target cargo test -p piperine-api -p piperine-python --test constraints_e2e` |
| Build | every task commit (zero warnings) + final | `CARGO_TARGET_DIR=/var/tmp/piperine-target cargo build --workspace 2>&1 \| grep -c warning` (expect 0); final: `cargo test --workspace` |

---

## Execution Plan

Phases are ordered and run sequentially — each phase completes before the next begins, and tasks within a phase execute in order.

### Phase 1: Frontend (lang)

```
T1 → T2 → T3
```

### Phase 2: Producer (codegen + api)

```
T4 → T5
```

### Phase 3: End to end + docs

```
T6 → T7 → T8
```

8 tasks, 3 phases → packs into a single batch (≤ ~8) → **inline execution, no sub-agents**.

---

## Task Breakdown

### T1: Constraint AST nodes

**What**: `ConstraintBlockDecl`/`RequireDecl` structs + `ModuleItem::Constraint` arm in the AST.
**Where**: `crates/piperine-lang/src/ast/item.rs`
**Depends on**: None
**Reuses**: neighboring decl shapes (`BehaviorConstruct`, `ParamDecl` for `doc`/`attrs` conventions)
**Requirement**: CONVAMS-01 (AST half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `ModuleItem::Constraint(ConstraintBlockDecl)` exists with `span`/`attrs`/`doc`/`requires`
- [ ] `RequireDecl { span, doc, name, pred: Expr, tol: Option<Expr> }`
- [ ] `cargo build -p piperine-lang` clean (zero warnings)

**Tests**: none (data nodes; exercised by T2/T3) — matrix row "grammar+elab" lands its tests in T2/T3, same phase
**Gate**: build

---

### T2: Grammar — `constraint begin … end`

**What**: `parse_module_item` dispatch on contextual `constraint` + `parse_constraint_block`/`parse_require_stmt` (contextual `require`/`tol`).
**Where**: `crates/piperine-lang/src/grammar/item.rs`
**Depends on**: T1
**Reuses**: `parse_behavior_construct`'s dispatch shape, `parse_attr_instances`, `parse_expr`, doc-comment capture, `Keywords::expect_identifier`
**Requirement**: CONVAMS-01

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Block + `require name : pred [tol expr];` parse; attrs/doc carried
- [ ] Non-`require` statement inside the block → typed parse error naming the token
- [ ] LRM keyword as require label rejected (`expect_identifier`)
- [ ] `constraint`/`require`/`tol` NOT added to `KEYWORDS` (annex_b gate untouched — verify green)
- [ ] Quick gate passes; new tests: block parses, tol clause, error paths (≥6 tests)
- [ ] Discrimination check on at least one new parse test (HANDOFF §1)

**Tests**: integration (vams_constraint_block.rs — parse half)
**Gate**: quick

**Commit**: `feat(lang): parse constraint begin blocks as module items`

---

### T3: POM + elaboration

**What**: `PomRequire` + `Module.constraints` + accessor; `ModuleItem::Constraint` elab arm (compose blocks, E4007 duplicate labels).
**Where**: `crates/piperine-lang/src/pom/{module,behavior}.rs`, `crates/piperine-lang/src/elab/lower/vams.rs`, `elab/error.rs` (new `ConstraintLabelDuplicate`)
**Depends on**: T2
**Reuses**: `Behavior` lowering arm shape; E4xxx error family conventions
**Requirement**: CONVAMS-01, 14, 15 (duplicate half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `Module::constraints()` exposes authored requires with spans (AC: "visible on the POM")
- [ ] Multiple blocks compose in authored order (test)
- [ ] Duplicate label fails loud E4007 naming both requires (test; discrimination-checked)
- [ ] Module without a block: `constraints()` empty, everything else unchanged
- [ ] Uninstantiated constrained module: no diagnostic (edge case)
- [ ] Quick gate passes

**Tests**: integration (vams_constraint_block.rs — elab half)
**Gate**: quick

**Commit**: `feat(lang): elaborate constraint blocks onto the POM`

---

### T4: Codegen margin producer

**What**: `ConstraintLowering` (restored from `fa223679`, require-only) lowering `PomRequire` → `LoweredMargin`; `lower_bodies` populates `constraints`.
**Where**: `crates/piperine-codegen/src/resolve/pom/{constraints,body}.rs`
**Depends on**: T3
**Reuses**: `fa223679:resolve/pom/constraints.rs` rules (`a<=b → b−a`, `a>=b → a−b`, `&& → min(parts)`, tol divides); `resolve_expr`/`subst_expr`
**Requirement**: CONVAMS-06, 07 (+ producer half of 01)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every comparison form lowers; sign convention asserted **on values** at satisfied/violated/boundary (migrated `margin_lowering.rs`)
- [ ] `tol` divides; absent `tol` emits no division (structural assertion)
- [ ] Unsupported predicate form → loud `LowerError` naming the require (discrimination-checked)
- [ ] Unresolved name → typed error naming name + require
- [ ] Migrated require-only fixtures of `margin_{kernel,dc,ac,tran}_points`, `margin_postures`, `margin_compile_count` go **green** (present-and-red → green; no test weakened)
- [ ] Fixtures exercising deferred `var`/`target`/`cover` stay red with `//!` notes naming the deferred mechanism (`cover_bin_mapping` wholly; the deferred tests inside mixed files)
- [ ] `tol_inert` migrates if require-only; else stays red with updated note
- [ ] Quick gate passes; test counts recorded (44 restored → how many green vs red-with-reason)

**Tests**: integration (restored codegen suite)
**Gate**: quick

**Commit**: `feat(codegen): lower require constraints to signed margins`

---

### T5: api catalog restore

**What**: `ConstraintCatalog::compute` restored (digital classification; `not_defined` list stays live-but-empty for deferred reduced statements); MD-45 doc comment removed.
**Where**: `crates/piperine-api/src/margins.rs`
**Depends on**: T4
**Reuses**: `9bf4a5a9~1:api/margins.rs` walk minus deferred arms
**Requirement**: CONVAMS-09, 13

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `compute` non-empty for a constrained design (digital case catalogued)
- [ ] "MD-45 regression" doc comment gone (success criterion)
- [ ] `MarginsResult::get` on a digital-instance require fails loud naming it
- [ ] Un-blocked require-subset of the 66 api tests green; still-blocked ones carry named deferred mechanisms
- [ ] Full gate passes

**Tests**: integration (api dv-core targets)
**Gate**: full

**Commit**: `feat(api): restore the constraint catalog from the VAMS producer`

---

### T6: Host end-to-end

**What**: `tests/constraints_e2e.rs` — the spec's three Independent Tests + edge cases through `Session`.
**Where**: `tests/constraints_e2e.rs` (new)
**Depends on**: T5
**Reuses**: divider fixture shape from `margin_postures.rs`; `SolverConfig.checks`
**Requirement**: CONVAMS-02..05, 08, 10..14

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] P1 Independent Test: divider past limit fails under `strict` naming constraint+instance+margin; raising the limit passes with worst margins reported
- [ ] P2 Independent Test: swept parameter — worst margin changes sign at the expected point, `argmin()` names it
- [ ] P3 Independent Test: same fixture × three postures → abort / recorded-continue / silent
- [ ] Rejected-iteration: violated-in-homotopy DC (the `margin_dc_points` Shockley shape) reports nothing; satisfied-at-final passes (CONVAMS-05)
- [ ] Multiple instances report per-instance margins by path; multiple blocks compose e2e; mid-hierarchy constrained module still lowers (flatten risk)
- [ ] ≥2 discrimination checks across the file; Full gate passes

**Tests**: e2e
**Gate**: full

**Commit**: `test(host): constraints end-to-end over the VAMS producer`

---

### T7: Docs

**What**: Part I extension section, MD-49 in STATE.md, ROADMAP.md dv-* correction.
**Where**: `docs/spec/part_i_language.md`, `.specs/STATE.md`, `ROADMAP.md`
**Depends on**: T6
**Reuses**: existing extension documentation precedents (§1.1 `///`, §8 attributes)
**Requirement**: CONVAMS-16, 17

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Part I documents syntax, severity anchor, soft-keyword rule, escaped-identifier escape hatch
- [ ] MD-49 recorded (supersedes MD-45's tentative attribute path for constraints; cites precedents)
- [ ] ROADMAP dv-core/dv-tester/dv-gradients/host-library state corrected (require core live; var/target/cover/distributions dark with follow-ups)
- [ ] Build gate (zero warnings) + `suite_hygiene` green

**Tests**: none (matrix says none for docs)
**Gate**: build

**Commit**: `docs: record the constraint begin extension (MD-49), correct ROADMAP dv-*`

---

### T8: Python parity + final gate

**What**: Un-block the require-subset of the 3 python tests; workspace sweep.
**Where**: `crates/piperine-python/tests/`
**Depends on**: T7
**Reuses**: existing parity test shapes
**Requirement**: CONVAMS-09 (python half), MD-22 uniform-host confirmation

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Blocked-note triage in python targets: require-subset green, rest named
- [ ] `cargo build --workspace` zero warnings; `cargo test --workspace` — every failure is one of: named deferred mechanism, named pre-existing debt, or another feature's present-and-red
- [ ] Test-count table vs HANDOFF §2 recorded for the Verifier

**Tests**: e2e (python)
**Gate**: build

**Commit**: `test(python): unblock the require-subset constraint parity tests`

---

## Phase Execution Map

```
Phase 1:  T1 ──→ T2 ──→ T3
Phase 2:  T4 ──→ T5
Phase 3:  T6 ──→ T7 ──→ T8
```

Execution is strictly sequential. Single batch (8 tasks) → inline, no sub-agents.

---

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: AST nodes | 1 file, 2 structs + 1 enum arm | ✅ Granular |
| T2: Grammar | 1 file, dispatch + 2 parse fns | ✅ Granular |
| T3: POM + elab | 3 files, 1 node + 1 arm + 1 error kind | ✅ Granular (cohesive: one POM surface) |
| T4: Codegen producer | 2 files, 1 lowering + 1 populate | ✅ Granular |
| T5: api catalog | 1 function | ✅ Granular |
| T6: Host e2e | 1 new test file | ✅ Granular |
| T7: Docs | 3 doc files, 1 section each | ✅ Granular |
| T8: Python + final gate | fixture triage + sweep | ✅ Granular |

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| ---- | ---------------------- | ------------- | ------ |
| T1 | None | phase-1 head | ✅ Match |
| T2 | T1 | T1 → T2 | ✅ Match |
| T3 | T2 | T2 → T3 | ✅ Match |
| T4 | T3 | T3 → T4 (phase boundary) | ✅ Match |
| T5 | T4 | T4 → T5 | ✅ Match |
| T6 | T5 | T5 → T6 (phase boundary) | ✅ Match |
| T7 | T6 | T6 → T7 | ✅ Match |
| T8 | T7 | T7 → T8 | ✅ Match |

No task depends on a later phase. ✅

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1 | AST data nodes | (none — exercised by T2/T3, same phase) | none | ✅ OK (data-only, no behavior) |
| T2 | grammar | integration | integration | ✅ OK |
| T3 | elab + POM | integration | integration | ✅ OK |
| T4 | codegen resolve | integration | integration | ✅ OK |
| T5 | api catalog | integration | integration | ✅ OK |
| T6 | host e2e | e2e | e2e | ✅ OK |
| T7 | docs | none | none | ✅ OK |
| T8 | python | e2e | e2e | ✅ OK |

No test deferral. ✅
