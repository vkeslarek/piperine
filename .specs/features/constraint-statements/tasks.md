# Constraint Statements — Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path.

---

**Design**: `.specs/features/constraint-statements/design.md`
**Status**: Done

---

## Test Coverage Matrix

> Guidelines: `AGENTS.md` (MD-28), `tests/suite_hygiene.rs`, HANDOFF §1 (A/B + discrimination on every new test).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| lang grammar+elab (new stmts) | integration | CONSTMT-01/09 parse/elab halves + edge cases | `crates/piperine-lang/tests/vams_constraint_block.rs` (extend) | `cargo test -p piperine-lang --test vams_constraint_block` |
| codegen lowering (var/target/cover) | integration | CONSTMT-01..03, 06, 07 lowering+kernel value-level | `crates/piperine-codegen/tests/{margin_lowering,cover_bin_mapping}.rs` (migrate the red) | `cargo test -p piperine-codegen` |
| api reduced + coverage channels | integration | CONSTMT-04, 05, 08 | `crates/piperine-api/tests/{reduced_metrics,coverage_merge,margins_channel,violations_channel}.rs` (migrate) | `cargo test -p piperine-api` |
| python parity | e2e | the reduced-var python test unblocks | `crates/piperine-python/tests/margins_surface.rs` | `cargo test -p piperine-python --test margins_surface` |
| docs | none | build gate | — | build gate |

## Gate Check Commands

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | lang/codegen tasks | `CARGO_TARGET_DIR=/var/tmp/piperine-target cargo test -p piperine-lang -p piperine-codegen` |
| Full | api/python tasks | `CARGO_TARGET_DIR=/var/tmp/piperine-target cargo test -p piperine-api -p piperine-python` |
| Build | every commit + final | `cargo build --workspace` zero warnings; final `cargo test --workspace` |

---

## Execution Plan

### Phase 1: Frontend (lang)
```
T1 → T2
```
### Phase 2: Producer (codegen)
```
T3 → T4
```
### Phase 3: Host (api + python + docs)
```
T5 → T6 → T7
```

7 tasks — single inline batch.

---

## Task Breakdown

### T1: AST + grammar for var/target/cover/cross

**What**: `ConstraintStmt` enum (replaces `requires` field) + parser arms.
**Where**: `crates/piperine-lang/src/ast/item.rs`, `grammar/item.rs`
**Depends on**: None
**Requirement**: CONSTMT-01/02 grammar halves, CONSTMT-06/07 grammar
**Done when**:
- [ ] All five statement forms parse; attrs/doc carried; keyword labels rejected; unknown statement typed error updated
- [ ] vams_constraint_block parse tests extended (≥6 new); annex_b untouched
- [ ] Quick gate
**Tests**: integration **Gate**: quick
**Commit**: `feat(lang): parse var/target/cover constraint statements`

### T2: POM statement list + E4008 across kinds

**What**: `PomConstraintStmt`, `Module.constraints` retype, duplicate-name check across kinds.
**Where**: `pom/behavior.rs`, `pom/module.rs`, `elab/lower/vams.rs`, codegen/api call-site adapts (compile-only)
**Depends on**: T1
**Requirement**: CONSTMT-09
**Done when**:
- [ ] Ordered stmt list on the POM; consumers compile; duplicate across kinds fails loud
- [ ] ≥3 elab tests; discrimination check
- [ ] Quick gate
**Tests**: integration **Gate**: quick
**Commit**: `feat(lang): constraint statements onto the POM as one ordered list`

### T3: Lowering — vars + targets

**What**: bindings substitution + target margins + classification; `fa223679~1` restoration.
**Where**: `resolve/pom/constraints.rs`
**Depends on**: T2
**Requirement**: CONSTMT-01, 02
**Done when**:
- [ ] margin_lowering's two red tests green (fixture migrated); target#N index semantics pinned
- [ ] Reduced-class targets stay out of the kernel (D7) — assert via num_margins
- [ ] Discrimination check
- [ ] Quick gate
**Tests**: integration **Gate**: quick
**Commit**: `feat(codegen): lower constraint vars and targets`

### T4: Lowering — cover points + crosses

**What**: `LoweredCoverPoint/Cross` production (bins const-eval, joint count).
**Where**: `resolve/pom/constraints.rs`, `body.rs`
**Depends on**: T3
**Requirement**: CONSTMT-06, 07
**Done when**:
- [ ] cover_bin_mapping ×7 green (fixtures migrated)
- [ ] Bins const-eval + cross point existence loud
- [ ] Quick gate
**Tests**: integration **Gate**: quick
**Commit**: `feat(codegen): lower cover points and crosses`

### T5: api reduced math restored

**What**: `reduced.rs` engine restored from `34c0c632`, walking the new stmt list; catalog not_defined.
**Where**: `api/src/reduced.rs`, `margins.rs`
**Depends on**: T4
**Requirement**: CONSTMT-04, 05
**Done when**:
- [ ] reduced_metrics ×5 green; margins_channel/violations_channel reduced tests green
- [ ] "MD-45 regression" docs gone
- [ ] Full gate
**Tests**: integration **Gate**: full
**Commit**: `feat(api): restore reduced metric reductions for constraint vars`

### T6: Coverage channel e2e + python parity

**What**: coverage_merge ×4 + python margins_surface reduced test migrated green.
**Where**: `crates/piperine-api/tests/coverage_merge.rs`, `crates/piperine-python/tests/margins_surface.rs`
**Depends on**: T5
**Requirement**: CONSTMT-08, 10 (python half)
**Done when**:
- [ ] coverage_merge ×4 green; python 3/3 green
- [ ] Full gate
**Tests**: e2e **Gate**: full
**Commit**: `test(host): cover merge and python reduced parity on the live producer`

### T7: Docs + final sweep

**What**: Part I §12 statement surface; ROADMAP still-dark shrink; workspace sweep table.
**Where**: `docs/spec/part_i_language.md`, `ROADMAP.md`, spec statuses
**Depends on**: T6
**Requirement**: CONSTMT-11
**Done when**:
- [ ] Docs state current surface; ROADMAP correct
- [ ] `cargo build --workspace` 0 warnings; `cargo test --workspace` — every remaining red named
- [ ] Build gate
**Tests**: none **Gate**: build
**Commit**: `docs: constraint statement surface current, ROADMAP shrunk`

---

## Phase Execution Map

```
Phase 1:  T1 ──→ T2
Phase 2:  T3 ──→ T4
Phase 3:  T5 ──→ T6 ──→ T7
```

## Task Granularity Check

T1 grammar (1 file+AST) ✅ · T2 POM+elab (3 files, one surface) ✅ · T3 var/target lowering (1 file) ✅ · T4 cover lowering (2 files) ✅ · T5 api reduced (2 files) ✅ · T6 test migration (2 test files) ✅ · T7 docs ✅

## Diagram-Definition Cross-Check

T1←none · T2←T1 · T3←T2 · T4←T3 · T5←T4 · T6←T5 · T7←T6 — matches map. ✅

## Test Co-location Validation

T1/T2 integration ✅ · T3/T4 integration ✅ · T5 integration ✅ · T6 e2e ✅ · T7 none (docs) ✅
