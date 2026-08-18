# Constraints on VAMS Validation

**Date**: 2026-08-17
**Spec**: `.specs/features/constraints-on-vams/spec.md`
**Diff range**: `ce3918e6^..8745bbbe` (10 commits, branch `feature/device-verification`)
**Verifier**: independent sub-agent (author ≠ verifier)

---

## Task Completion

| Task | Status | Notes |
| ---- | ------ | ----- |
| T1 AST nodes | ✅ Done | `ast/item.rs` `ConstraintBlockDecl`/`RequireDecl` present |
| T2 Grammar | ✅ Done | contextual keywords; Annex B gate untouched (`Keywords::is_reserved` test) |
| T3 POM + elab | ✅ Done | `Module.constraints`, E4008 duplicate (design.md said E4007 — trivial doc mismatch) |
| T4 Codegen producer | ✅ Done | `ConstraintLowering` + `lower_bodies` populates `constraints` |
| T5 api catalog | ✅ Done | `compute` walks flat instances; MD-45 doc comment removed from `margins.rs` |
| T6 Host e2e | ✅ Done | 5 tests, all 3 P-story Independent Tests |
| T7 Docs | ✅ Done | Part I §12, MD-49 (`STATE.md:930`), ROADMAP dv-* corrected |
| T8 Python + gate | ✅ Done | 2/3 python green; the red one is the named deferred `var` test |

---

## Spec-Anchored Acceptance Criteria

### P1: Declare a constraint and get a loud, named failure

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| AC1: block parses, elaborates, visible on POM | 2 requires on POM in authored order with spans + tol | `crates/piperine-lang/tests/vams_constraint_block.rs:179-188` — `assert_eq!(cs.len(), 2)`, `cs[0].name == "head"`, `cs[1].tol.is_some()`, `cs[0].span.is_some()`; parse half `:33-46` | ✅ PASS |
| AC2: ports/params/probes resolve unqualified | `V(p, n)` and param `hi` resolve with no qualification | `tests/constraints_e2e.rs:36-38` fixture + `:84-87` — `head.value == Pointwise { worst: 0.1, argmin: Some(0.0) }` (only possible if both resolved); params-only lowering `margin_lowering.rs:83-115` | ✅ PASS |
| AC3: strict violation fails loud naming constraint + instance path + signed margin | error names `head`, `dut1`, margin −0.1 | `crates/piperine-codegen/tests/margin_dc_points.rs:190-197` — `msg.contains("head")`, `contains("dut1")`, `contains("at t = 0")`, `contains("margin = -0.100000")`; e2e `constraints_e2e.rs:78-79` | ✅ PASS |
| AC4: all-satisfied run completes, worst margins reported | worst = 0.6−0.5 = 0.1, satisfied, requires_ok | `tests/constraints_e2e.rs:84-88` — `assert_eq!(head.value, MarginValue::Pointwise { worst: 0.6 - 0.5, argmin: Some(0.0) })`, `head.is_satisfied()`, `op.requires_ok()` | ✅ PASS |
| AC5: rejected-iteration violations not reported (LRM §9.7.3) | homotopy intermediates violated, final satisfied → strict passes, zero findings | `margin_dc_points.rs:96-104` — `homotopy_strategy == Some("gmin-stepping")` (fixture provably engages homotopy) + `stats.validation.is_empty()`; tran half `margin_tran_points.rs:107-118` — every finding's time ∈ accepted steps, `steps_rejected > 0` | ✅ PASS |

**Independent Test check**: `p1_violation_aborts_strict_and_raising_the_limit_passes` (`constraints_e2e.rs:72`) matches the spec definition exactly — divider past limit fails by name; `set("dut1","hi",0.6)` makes the same fixture pass. ✅

### P2: Signed margins, normalized and reduced

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| AC1: signed margin, m ≥ 0 ⟺ satisfied | sign asserted on values at satisfied/violated/boundary | `margin_lowering.rs:107-114` — `assert_eq!(m >= 0.0, satisfied)` over `[(2,3,true),(3,2,false),(2,2,true)]`, plus `m == b − a` | ✅ PASS |
| AC2: `tol <scale>` normalizes; absent → scale 1, no division | Div emitted iff tol authored; values 0.5 vs 1.0 | `margin_lowering.rs:166-182` — structural `BinOp::Div` with scale 2.0; `!matches!(plain.expr, Div)`; `eval(scaled) == 0.5`, `eval(plain) == 1.0` | ✅ PASS |
| AC3: worst + argmin + instance path over a sweep | worst −0.1 at swept coordinate 1.0 | `constraints_e2e.rs:102-108` — `(head.scalar() - (−0.1)).abs() < 1e-9`, `argmin() == Some(1.0)`; per-path `margin_dc_points.rs:129` | ✅ PASS |
| AC4: read through existing api surface, no parallel API | `margins().get().scalar()/argmin()/is_satisfied()` | `crates/piperine-api/tests/margins_channel.rs:105-109,172-173,198` + e2e; no new API in the diff surface | ✅ PASS |

**Independent Test check**: `p2_worst_margin_changes_sign_at_the_crossing_with_the_swept_argmin` (`constraints_e2e.rs:95`) — sweep `[0.6,0.8,1.0]` gives worst −0.1 with argmin 1.0; shrinking to `[0.6,0.7]` flips the sign positive with argmin 0.7. Matches spec. ✅

### P3: The three postures

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| AC1: strict = $error abort | run aborts naming the constraint | `constraints_e2e.rs:127-131` — `expect_err` + `contains("head")`; `margin_dc_points.rs:148` | ✅ PASS |
| AC2: collect = $warning, recorded, readable, run continues | violation recorded with label + instance path | `constraints_e2e.rs:134-139` — completes, `!requires_ok()`, `first_violation().label == "head"`, `instance_path == "dut1"` | ✅ PASS |
| AC3: off = nothing evaluated, no measurable cost | no margins in channel; evaluation short-circuits | `constraints_e2e.rs:144-149` — `margins().get("head").is_err()`; structural: `device/analog/constraints.rs:77-79` early-returns before any evaluation | ⚠️ PASS with spec-precision note — "no measurable cost" is not *measured* (no timing/eval-count assertion for off-with-constraints; `margin_compile_count` only covers the no-block case). The early-return makes non-evaluation structural, so the spirit is met |
| AC4: unavailable-quantity reference loud, naming it (every posture except off) | digital-instance require reads loud: names `stable`, says "no analog half" | `margins_channel.rs:215-241` — `expect_err`, `contains("stable")`, `contains("no analog half")` (collect posture; the read path is posture-independent) | ✅ PASS |

**Independent Test check**: `p3_the_three_postures_abort_record_and_stay_silent` (`constraints_e2e.rs:125`) — same fixture × 3 postures → abort / recorded-and-continue / silent. Matches spec. ✅

### Edge cases

| Edge case | Evidence | Result |
|---|---|---|
| Uninstantiated constrained module: silent | `vams_constraint_block.rs:238-242` — elaborates, 2 constraints, no diagnostic | ✅ |
| Multiple blocks compose, none dropped | `vams_constraint_block.rs:191-199` — `names == ["a1","a2"]`; shadow-hole check `:217-225` | ✅ |
| Duplicate require name fails loud naming it | `vams_constraint_block.rs:203-214` — `contains("head")`, `contains("duplicate")` (E4008) | ✅ |
| Unresolved name fails loud, never silent false | no constraint-specific test. Mechanism: `ConstraintLowering` uses the analog body's own `resolve_expr` (unresolved → typed `LowerError`, covered generically by `resolve_lowering.rs:258-276`), and error *propagation* from the constraint ctx is proven by `margin_lowering.rs:207-225` (unsupported-form error fails `lower_bodies`). CONVAMS-15's "elaboration fails loud" was deliberately re-read as "compile pipeline fails loud pre-simulation" (design.md Tech Decisions) — the outcome (loud, named, pre-simulation) is faithfully met by construction, but the require-predicate path has no direct test | ⚠️ Evidence gap (minor) |
| Digital/structural module: margin read loud (`digital` distinction) | `margins_channel.rs:215-241` | ✅ |
| Many instances: per-instance margins by path | `constraints_e2e.rs:193-196` — `dut1.head` (−0.1) and `dut2.head` distinct entries | ✅ |

### Docs requirements

| Req | Evidence | Result |
|---|---|---|
| CONVAMS-16: Part I extension section + MD- | `docs/spec/part_i_language.md:394` §12; `.specs/STATE.md:930` MD-49 (supersedes MD-45 tentative attribute path) | ✅ |
| CONVAMS-17: ROADMAP corrected | `ROADMAP.md:43,164-198` — "constraint core re-anchored", var/target/cover/distributions listed as dark | ✅ |

**Status**: ⚠️ All ACs covered; 2 spec-precision/evidence notes (P3-AC3 cost unmeasured; CONVAMS-15 unresolved-half untested directly)

---

## Discrimination Sensor

Mutations applied in scratch (in-place edit → test → `git checkout --` restore; the four target files were verified clean beforehand, and the pre-existing unrelated dirty files were never touched). Tree confirmed restored (`git status` identical to pre-verification state).

| # | Mutation | File:line | Killed by | Killed? |
|---|----------|-----------|-----------|---------|
| 1 | Flipped margin sign: `Le` arm `difference(rhs,lhs)` → `difference(lhs,rhs)` | `codegen/src/resolve/pom/constraints.rs:170` | `margin_lowering::le_and_ge_lower_to_signed_differences` + all 5 `constraints_e2e` | ✅ Killed |
| 2 | Deleted the duplicate-label check | `lang/src/elab/lower/vams.rs:251-261` | `a_duplicate_require_label_fails_loud_naming_both` + `a_duplicate_label_across_blocks_fails_loud_too` | ✅ Killed |
| 3 | `ConstraintCatalog::compute` returns empty again | `api/src/margins.rs:154-156` | `margins_channel::a_require_on_a_digital_instance_reads_loud_as_digital` | ✅ Killed |
| 4 | `lower_bodies` drops the margins (`let _ = margins;`) | `codegen/src/resolve/pom/body.rs:444` | all 5 `constraints_e2e` + `margin_lowering` | ✅ Killed |
| 5 | `&&` arm drops the second part (`lm` instead of `min(lm,rm)`) | `constraints.rs:172-176` | `margin_lowering::the_interval_form_lowers_to_min_of_parts` | ✅ Killed |

**Sensor depth**: lightweight (5 mutations, highest-risk new code: sign convention, composition guard, catalog, producer wiring, interval combinator)
**Result**: 5/5 killed — PASS ✅

---

## Code Quality

| Principle | Status |
|---|---|
| Minimum code | ✅ producer is ~90 lines of new lowering; backend untouched per design |
| Surgical changes | ✅ solver/kernel/device files untouched as designed |
| No scope creep | ✅ var/target/cover producers absent; their tests present-and-red with named mechanisms |
| Matches patterns | ✅ contextual-keyword parse mirrors `analog` dispatch; E4xxx error family; flat `Vec<PomRequire>` per design F1 |
| Spec-anchored outcome check | ✅ asserted values match spec (margins −0.1/0.1, argmins, sign convention on values) |
| Every test maps to a spec requirement | ✅ except the 15 present-and-red tests, each carrying a named deferred mechanism (verified below) |
| Project guidelines | ✅ `AGENTS.md` MD-28 placement (root e2e, crate-local unit/integration), zero-warnings build confirmed (`cargo build --workspace` → 0 warnings), no unwrap on user paths, no macros in new code |

---

## Gate Check

- **Gate command**: the feature's test commands (below) + `cargo build --workspace` (0 warnings)
- `cargo test --test constraints_e2e` — **5 passed, 0 failed**
- `cargo test -p piperine-lang --test vams_constraint_block` — **16 passed, 0 failed**
- `cargo test -p piperine-codegen --test margin_lowering` — 5 passed, **2 failed** (named deferred `var`/`target`, `//!` note lines 9-14, 227-233)
- `margin_kernel` — 3 passed; `margin_dc_points` — 3 passed; `margin_ac_points` — 3 passed; `margin_postures` — 5 passed; `margin_evaluation` — 7 passed; `margin_compile_count` — 1 passed
- `margin_tran_points` — 1 passed, **2 failed** (UIC fixtures; `//!` note lines 9-15 names the **T44 UIC-producer gap** — independently verified real: `tests/ngspice/uic_rc.vams` header documents `ic` no longer wired into seeds; failure is at the PHDL fixture's discipline line, unrelated to the constraint surface)
- `cover_bin_mapping` — 0 passed, 7 failed (named deferred `cover`; `//!` note present)
- `tests/tol_inert.rs` — 0 passed, 5 failed (named deferred parameter-distribution surface; `//!` note lines 8-13)
- `cargo test -p piperine-api --test margins_channel` — **6 passed, 0 failed**
- `cargo test -p piperine-python --test margins_surface` — 2 passed, **1 failed** (`reduced_vars_are_defined_on_ac_only_through_python`, BLOCKED note at `margins_surface.rs:157` names the deferred `var`/reduced statement)
- **Test-count integrity**: every failure in scope carries a named deferred mechanism or named pre-existing debt, per T8's own bar. No test was weakened — the restored margin tests assert on values (sign convention, worst/argmin numerics), which the sensor confirmed discriminating.

---

## Fix Plans

### Fix 1 (Minor): no direct test for an unresolved name inside a `require` predicate

- **Root cause**: CONVAMS-15's unresolved half rides the analog body's `resolve_expr` path; the unsupported-*form* error is tested (`margin_lowering.rs:207`) but an unresolved *name* in a predicate is not.
- **Fix task**: add one test to `margin_lowering.rs` — `require bad : V(p, n) <= nope;` → `lower_bodies` errs naming `nope` (and ideally `bad`).
- **Priority**: Minor

### Fix 2 (Cosmetic): stale "MD-45 regression" comment at `crates/piperine-api/src/session/entry.rs:524`

- **Root cause**: comment claims `Module::constraints()` "were gutted with the PHDL AST at T21 … nothing left to walk for candidates" — now factually false (the gradient metric path legitimately doesn't walk margins, but the stated reason is stale).
- **Fix task**: reword the comment to describe the gradient-path limitation without the gutted-frontend claim.
- **Priority**: Cosmetic

### Fix 3 (Cosmetic): design.md says E4007; the code is E4008

- **Fix task**: update `design.md` Component 4 to E4008 (code + AGENTS.md briefing already agree on E4008).
- **Priority**: Cosmetic

---

## Requirement Traceability Update

| Requirement | New Status |
|---|---|
| CONVAMS-01..11, 13, 14, 16, 17 | ✅ Verified |
| CONVAMS-12 | ✅ Verified (spec-precision note: cost unmeasured, non-evaluation structural) |
| CONVAMS-15 | ⚠️ Verified for the duplicate half; unresolved half met-by-construction, direct test missing (Fix 1) |

---

## Summary

**Overall**: ✅ Ready (with 1 minor + 2 cosmetic follow-ups)

**Spec-anchored check**: 17/17 ACs traced to file:line assertions matching the spec-defined outcome; 2 spec-precision notes
**Sensor**: 5/5 mutations killed
**Gate**: all green except named-deferred/named-debt present-and-red tests (verified individually)

**What works**: the full producer chain — grammar → AST → POM → lowering → kernel → solver evaluation points → margins channel — proven end to end on values; all three P-story Independent Tests match their spec definitions and pass; rejected-iteration rule proven against a real homotopy run and a real LTE-rejection run.

**Issues found**: unresolved-name-in-predicate has no direct test (Fix 1); two stale docs (Fix 2, 3).

**Next steps**: land Fix 1 (one test); optionally Fixes 2-3. No re-verification cycle needed beyond Fix 1's single assertion.

---

## Fix→re-verify round 1 (2026-08-17)

The three gaps from the PASS verdict above were fixed in `da63a0dc`:

1. **CONVAMS-15 direct test** — `margin_lowering.rs::an_unresolved_name_in_a_require_fails_loud_naming_it`
   added: `require bad : V(p, n) <= nope;` fails loud naming `nope` at
   kernel compilation (the design's recorded stage), green.
2. **Stale MD-45 comment in `session/entry.rs`** — replaced by the real
   behavior: the unknown-metric error now lists the declared margins from
   the built instances' kernel catalogs;
   `sensitivity_surface::unknown_metric_fails_loud_listing_candidates` now
   green. The margin-name *resolution* half (`a_metric_name_resolves…`)
   stays red on the deferred dv-gradients `classify_margin_gradient`,
   its in-place note already accurate.
3. **design.md E4007 → E4008** — corrected (E4007 was already taken by
   `UndeclaredNetDefaultNettypeNone`).

Gates re-run after the fixes: `margin_lowering` 8 tests (6 green +
2 named deferred), `sensitivity_surface` 3/5 green (2 named deferred),
`cargo build --workspace` 0 warnings. **Verdict after round 1: PASS.**
