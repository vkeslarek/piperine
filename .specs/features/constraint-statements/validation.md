# Constraint Statements — Validation

**Date**: 2026-08-18
**Spec**: `.specs/features/constraint-statements/spec.md`
**Diff range**: `2a345c11..a822ba09` (feature `65c904e6` + docs `a822ba09`)
**Verifier**: independent sub-agent (author ≠ verifier); read-only over the real tree, sensor mutations restored (`git status` clean over the feature surface after every run)

---

## Task Completion

| Task | Status | Notes |
| ---- | ------ | ----- |
| T1 AST + grammar | ✅ Done | `ast/item.rs:344-403` (`ConstraintStmt`/`TargetCmp`/`CoverBins`), `grammar/item.rs:726-848`; ≥6 new parse tests present |
| T2 POM list + E4008 | ✅ Done | `pom/behavior.rs:130-188`, `elab/lower/vams.rs:251-313`; cross-kind duplicate test present |
| T3 var/target lowering | ✅ Done | `resolve/pom/constraints.rs:162-226, 375-400`; margin_lowering ×2 migrated green |
| T4 cover lowering | ✅ Done | `constraints.rs:233-320`; cover_bin_mapping ×7 green |
| T5 api reduced math | ✅ Done | `api/src/reduced.rs` (469 lines), `margins.rs` catalog; reduced_metrics/violations_channel green |
| T6 coverage e2e + python | ✅ Done | coverage_merge ×5, python margins_surface ×3 green |
| T7 docs | ✅ Done (one stale spot) | Part I §12 + ROADMAP updated; §10 gap bullet left stale (see gaps) |

---

## Spec-Anchored Acceptance Criteria

| Criterion (WHEN X THEN Y) | Spec-defined outcome | `file:line` + assertion | Result |
| ------------------------- | -------------------- | ----------------------- | ------ |
| CONSTMT-01 AC1: `var ap1 = a + 1.0;` + `require via_var : ap1 <= b` | margin `b − (a + 1)` on values | `crates/piperine-codegen/tests/margin_lowering.rs:283-288` — `assert_eq!(m, b - (a + 1.0))` at 3 points incl. violated | ✅ PASS |
| CONSTMT-01 AC2: var→var substitutes transitively | chained bindings compose | **no committed test**; behavior verified by Verifier probe (via_var = 0.400000 = b − (1+1+1)); impl `resolve/pom/constraints.rs:171-178` (bindings store substituted+resolved exprs) | ⚠️ GAP: correct behavior, missing test |
| CONSTMT-01 AC3: unknown name in var fails loud naming it | loud error naming the name | **no committed test**; probe: `unresolved analog identifier \`nope\`` at compile; impl via `resolve_expr` → `ctx.errors` (`constraints.rs:177`, fail-loud path tested for require at `margin_lowering.rs:231-252`) | ⚠️ GAP: correct behavior, missing test |
| CONSTMT-02 AC1: target lowers to signed scale-normalized `target#<index>` kind Target | index = stmt index across ALL stmts, 0-based | `margin_lowering.rs:162-175` — finds `target#7`/`target#8` in a 9-stmt fixture (var@0, requires@1..6); `assert_eq!(scaled.kind, MarginKind::Target)`; `(2−1)/2 = 0.5` value-level at :179 | ✅ PASS |
| CONSTMT-02 (class): reduced target carries class + analyses, stays out of kernel | Reduced iff helper appears after substitution | `margin_lowering.rs:312-317` — `target#0` class Reduced, `analyses == ["ac"]`, require Pointwise; impl `constraints.rs:199-217` (substitute-then-classify) | ✅ PASS (sensor 1 proves discrimination) |
| CONSTMT-03: violated target NEVER a finding; `requires_ok` ignores targets | soft goal | impl `codegen/src/device/analog/constraints.rs:120` (`margin < 0.0 && info.kind == MarginKind::Require`), `api/src/margins.rs:375-380` (requires_ok filters Require); **no committed test**; probe: violated target#1 (−0.9) → `requires_ok=True`, `violations=0` | ⚠️ GAP: correct behavior, missing test (explicit AC, highest-ranked gap) |
| CONSTMT-04 AC1: `var ugbw = ac_unity_gain_freq(V(out, gnd)); target ugbw >= 1e4 tol 1e3` on AC | kind var, shape reduced, value ≈ analytic; `target#N` = (ugbw−1e4)/1e3 | `crates/piperine-api/tests/reduced_metrics.rs:115-124` (kind `MarginKind::Var`, `MarginShape::Reduced`, rel err < 1e-2 vs `FC·√(A0²−1)`), `:149-158` (`target#3` — 3 vars precede, `(analytic_ugbw()−1e4)/1e3` ± 0.1, `argmin() == None`) | ✅ PASS |
| CONSTMT-04 AC2/AC3: pm/gm reduce over the sweep | pm ≈ 180°−atan, gm = +inf single-pole | `reduced_metrics.rs:126-139` (±0.5°; `is_infinite() && is_sign_positive()`) | ✅ PASS |
| CONSTMT-05: reduced read outside AC fails loud "not defined in this analysis" | not_defined catalog → loud get() | `crates/piperine-api/tests/violations_channel.rs:196-210` — op result `get("ugbw")`/`get("x1.ugbw")` err contains "not defined in this analysis", defined on AC; python: `piperine-python/tests/margins_surface.rs:211-215` (RuntimeError, same message) | ✅ PASS |
| CONSTMT-06 AC1: `bins [0.0:1.0:3.0]` nearest-edge; out-of-bin counts nothing | 0.4→bin0, 2.6→bin3; 99.0→no hit | `crates/piperine-codegen/tests/cover_bin_mapping.rs:74-80` (`&[vec![2,0,0,1]]`), `:87-91` (all-zero + `eval_calls==1`); impl `device/analog/coverage.rs:23-39` (`.round()` nearest-edge) | ✅ PASS |
| CONSTMT-06 AC2: `bins in {…}` set membership | membership hit | parse-level only: `vams_constraint_block.rs:294-306`; impl `coverage.rs:37` (ε-position). No value-level set-bin hit test | ⚠️ Spec-precision gap (minor) |
| CONSTMT-07: cross joint = same-instant bins, joint count = product | product of point bin counts | `cover_bin_mapping.rs:118-133` — 2×2 pts → 4 joint bins (`&[vec![0,0,0,0]]`), same-instant hit at joint 0; product computed at `constraints.rs:250-274` (`joint *= …`) | ✅ PASS |
| CONSTMT-08: merge_run sums; empty bins name themselves | sum, never replace | `crates/piperine-api/tests/coverage_merge.rs:72-81` (`vec![2,0,0,0]`), `:87-109` (`d1.c1[2]`,`d1.c1[3]` named; second run merges → `[1,1,1,0]`), `:119-135` (zero-point run → every bin named), `:143-157` (persists across sessions) | ✅ PASS |
| CONSTMT-09: duplicate labels across kinds → E4008 | module-wide across require/var/cover/cross | `vams_constraint_block.rs:333-343` (var `head` + require `head` → "duplicate"); same-kind :216-227, cross-block :230-238; impl `elab/lower/vams.rs:295-311` over `PomConstraintStmt::label()` (`pom/behavior.rs:167-175`); code `E4008` at `pom/error.rs:343` | ✅ PASS |
| CONSTMT-10: migrated suites green | all named suites | lang 22/22, margin_lowering 8/8, cover_bin_mapping 7/7, api reduced_metrics 3/3 + coverage_merge 5/5 + violations_channel 5/5, python margins_surface 3/3 (run by Verifier) | ✅ PASS (reduced_metrics file holds 3 tests vs the spec inventory's ×5 — the other two blocked cases live in violations_channel/python; no assertion weakened, total migrated ≥ inventory) |
| CONSTMT-11: Part I §12 + ROADMAP updated | current surface documented | `docs/spec/part_i_language.md:436-497` (five forms, target#N, soft-goal, bins/cross semantics), ROADMAP still-dark list drops var/target/cover (`a822ba09`) | ⚠️ PASS with one stale contradiction: `part_i_language.md:393-397` (§10 gap bullet) still says var/target/cover "are not part of the language today" |

**Status**: ⚠️ 8/11 ACs fully evidence-backed; 3 carry gaps — none behavioral (all verified correct by probe or inspection), all test/doc-level.

### Edge cases (spec §Edge Cases)

- [x] Var read before declaration → loud: **behavior verified** (probe: `unresolved analog identifier \`ap1\``); no committed test — same gap family as CONSTMT-01 AC2/AC3.
- [x] Target over unresolved name → loud at lowering: **behavior verified** (probe: `unresolved analog identifier \`nope_lvl\``); no dedicated committed test (shared path partially covered by `margin_lowering.rs:231-252` for requires).
- [x] `requires_ok` with only violated targets → still true: probe-verified (see CONSTMT-03); no committed test.
- [x] Non-finite cover value → no bin hit: impl `device/analog/coverage.rs:102-104` (`if !value.is_finite() { continue; }`); the analogous loud rule for margins is tested (`margin_evaluation.rs:143-165`) but the cover non-finite path has no direct test — minor.
- [x] Duplicate labels across kinds E4008: tested (`vams_constraint_block.rs:333-343`).

---

## Discrimination Sensor

Scratch mutations (file → mutate → run → `git checkout --` restore; feature surface verified clean after each):

| # | Mutation | File:line | Killed by |
| - | -------- | --------- | --------- |
| 1 | Classify target from the **un-substituted** expr (drop the substitution-before-classify fix) | `resolve/pom/constraints.rs:199-209` | ✅ Killed — `reduced_metrics` 3/3 fail (`target ugbw >= …` misclassifies Pointwise → compile fails); margin_lowering unaffected (helper inline, as expected) |
| 2 | `target#N` enumerates **only targets** (per-target counter) | `resolve/pom/constraints.rs:169-224` | ✅ Killed — `margin_lowering::tol_normalizes…` panics (`target#7` missing → `target#0`). Note: reduced_metrics survives (host walk labels independently) — cross-layer label agreement pinned only at the codegen layer |
| 3 | `lower_cover` skips crosses (no `LoweredCoverCross` emitted) | `resolve/pom/constraints.rs:250-275` | ✅ Killed — `cover_bin_mapping::cover_cross_combines…` panics (cross catalog empty) |
| 4 | Flip `TargetCmp` margin direction (Le/Ge arms swapped) | `resolve/pom/constraints.rs:384-387` | ✅ Killed — `margin_lowering::tol_normalizes…` value 0.5 → −0.5. Note: reduced_metrics survives — the host's `reduced.rs:164-167` implements the direction independently (both layers separately value-pinned) |
| 5 | Catalog classifies pointwise vars as `digital` (Var arm pushes digital unconditionally) | `api/src/margins.rs:171-179` | ✅ Killed — `violations_channel::a_reduced_var_read_outside_ac…` (message becomes "margins are an analog notion") |

**Sensor depth**: lightweight-plus (5 targeted behavior-level mutations, per the tasking's 3-5)
**Result**: 5/5 killed — PASS ✅

---

## Gate Check

- **Gate command** (tasks.md Build): `cargo build --workspace` → **0 warnings**; `cargo test --workspace --no-fail-fast`
- **Feature-scoped suites** (run by Verifier, `CARGO_TARGET_DIR=/var/tmp/piperine-target`): `vams_constraint_block` 22/22 · `margin_lowering` 8/8 · `cover_bin_mapping` 7/7 · `reduced_metrics` 3/3 · `coverage_merge` 5/5 · `violations_channel` 5/5 · `margins_channel` 6/6 · python `margins_surface` 3/3 — **57/57, 0 failed, 0 skipped**
- **Full workspace**: 318 targets ok, **24 targets failed — all 24 re-run at parent commit `2a345c11` and fail identically there** (11 re-run directly incl. `ngspice_uic_*`; remaining 13 verified in a clean parent worktree). The feature commits introduced **zero** new failures. The 24 map to the branch's documented deferred mechanisms (distributions → monte_carlo/high_sigma/center_policy/tol_inert family, gradients classifier → margin_gradients/sensitivity_surface, event blocks/windows → event_window/scoped_require_scope, T44 UIC → ngspice_uic_*, model_* debt, LSP/plugin/sp_parity WIP) — consistent with `65c904e6`'s commit message. suite_hygiene fails on oversized `mod.rs` in piperine-ir/ast, unrelated.
- **Test count vs before**: migrated totals ≥ the spec's 22-test blocked inventory (25 green across the migrated files); no deletions, no weakened assertions found.
- **Working-tree note**: the tree carries unrelated uncommitted changes (`vams_include.rs`, `examples/21_shift_register_tran.vams`, deleted CLEANUP_PLAN/GAPS/SPEC_DEVIATIONS, untracked specs) that predate this validation; the 24 failures were proven independent of them (clean-worktree re-runs).

---

## Code Quality

| Principle | Status |
| --------- | ------ |
| Minimum code / no scope creep | ✅ producer + restored `reduced.rs` only; consumers unchanged as designed |
| Surgical changes | ✅ diff confined to the named files + migrated tests + docs |
| Matches patterns | ✅ tracker/catalog/loud-error shapes follow the crate's precedents |
| Spec-anchored outcome check | ⚠️ 8/11 ACs value-anchored; 3 gaps above |
| Every test maps to a spec requirement | ✅ (spot-checked margin_lowering, cover_bin_mapping, reduced_metrics) |
| Documented guidelines | AGENTS.md/MD-28 co-location respected; zero `unwrap()` added on user paths | ✅ |
| Stale comments | ⚠️ three doc-drift spots (below) — cosmetic |

---

## Fix Plans

### Fix 1: Pin CONSTMT-03 with a host test (Major)
- **Root cause**: no committed test asserts the soft-goal contract end-to-end.
- **Fix task**: add to `crates/piperine-api/tests/margins_channel.rs`: a divider fixture with a violated pointwise `target V(p,n) <= 0.1;` — assert `requires_ok() == true`, `violations().is_empty()`, and the target entry present with `kind == Target`, `scalar < 0`, argmin carried. (Verifier probe output confirms the exact expected values: requires_ok=True, violations=0, target#1 −0.9.)
- **Priority**: Major (explicit AC, behavior currently unguarded against regression).

### Fix 2: Pin transitive var substitution + var-unknown-name (Minor)
- **Root cause**: CONSTMT-01 AC2/AC3 have no test.
- **Fix task**: extend `margin_lowering.rs` — `var ap1 = 1.0; var ap2 = ap1 + 1.0; var ap3 = ap2 + 1.0; require via_var : ap3 <= b;` asserting the margin evaluates to `b − 3` (probe-verified 0.4 = 3.4 − 3); plus a `var ap1 = nope + 1.0;` loud-failure test asserting the error names `nope`.
- **Priority**: Minor.

### Fix 3: Docs consistency (Minor)
- `docs/spec/part_i_language.md:393-397` §10 bullet still claims var/target/cover are "not part of the language today" — contradicts §12 and ROADMAP.
- `crates/piperine-codegen/tests/margin_lowering.rs:254-261` and `cover_bin_mapping.rs:12-18` headers still say the fixtures are "present-and-red / blocked / use the retired PHDL grammar" — they are live and green now. `resolve/pom/body.rs:25-32,42-47` still documents `constraints`/`cover` as "permanently `None` post-MD-45". `pom/error.rs:341-351` E4008 message says "require label" though it now covers var/cover/cross labels.
- **Priority**: Minor (doc drift, no behavior).

### Fix 4 (optional, Minor): set-bins value-level test
- `bins in {…}` membership is only parse-tested; one value-level hit/miss assertion in `cover_bin_mapping.rs` would close CONSTMT-06 AC2's precision gap. Also noted: catalog's `is_reduced_stmt` (api) is syntactic over the authored expr, so a var reading another reduced *var* (`var half = ugbw/2`) catalogues as unknown (not not_defined) outside AC — cross-analysis message for that shape is "unknown constraint"; host evaluation still handles it correctly. Spec does not pin this shape; record as a spec-precision note only.

---

## Requirement Traceability Update

| Requirement | Previous Status | New Status |
| ----------- | --------------- | ---------- |
| CONSTMT-01 | Pending | ✅ Verified (AC2/AC3 test gaps → Fix 2) |
| CONSTMT-02 | Pending | ✅ Verified |
| CONSTMT-03 | Pending | ⚠️ Verified-by-inspection+probe; test gap → Fix 1 |
| CONSTMT-04 | Pending | ✅ Verified |
| CONSTMT-05 | Pending | ✅ Verified |
| CONSTMT-06 | Pending | ✅ Verified (set-bins precision gap → Fix 4) |
| CONSTMT-07 | Pending | ✅ Verified |
| CONSTMT-08 | Pending | ✅ Verified |
| CONSTMT-09 | Pending | ✅ Verified |
| CONSTMT-10 | Pending | ✅ Verified |
| CONSTMT-11 | Pending | ✅ Verified (stale §10 bullet → Fix 3) |

---

## Summary

**Overall**: ✅ Ready (with ranked follow-ups)

**Spec-anchored check**: 8/11 ACs matched spec outcome with value-level test evidence; 3 flagged (all behavior-verified correct, test/doc gaps only)
**Sensor**: 5/5 mutations killed
**Gate**: 57/57 feature-scoped tests green; workspace build 0 warnings; 24 workspace failures all proven pre-existing at the parent commit

**What works**: full var/target/cover/cross producer (parse → POM → lowering → kernel → host channels), substitution-before-classify, `target#N` across-kind indexing, soft-goal tracking, reduced ugbw/pm/gm against analytic oracles, not-defined cross-analysis reads, cover nearest-edge/set/out-of-bin/cross-product counting, merge-summing coverage database, E4008 across kinds, docs + ROADMAP.

**Issues found**: ranked above (Fix 1 Major test gap; Fixes 2-4 Minor).

**Next steps**: route Fix 1-3 as fix tasks; re-verify with one iteration (gap closure is mechanical).

---

## Fix→re-verify round 1 (2026-08-18)

All four ranked gaps from the PASS verdict above were fixed:

1. **CONSTMT-03 host test** — `margins_channel.rs::a_violated_target_is_
   tracked_but_never_a_violation`: both require and target negative, only
   the require reports, `requires_ok` gated by the require alone, the
   target's worst margin readable and negative. Green.
2. **Transitive var + unknown-name tests** — `vams_constraint_block.rs`:
   transitive substitution lowers (vars ride inside the require, one
   margin); an unresolved var name fails loud at kernel compilation
   (`unresolved … nope`). Also probed the analog-less corner: a
   constrained module with no analog body fails loud ("no analog body") —
   no silent path. Green (24/24).
3. **Docs consistency** — §10's "beyond require" bullet rewritten (event
   blocks/windows/distributions are the actual deferred set);
   `resolve/pom/body.rs`'s LoweredBody/field docs no longer claim
   "permanently None post-MD-45".
4. **Set-bins value-level test** — `margins_channel.rs::
   set_bins_count_by_membership_value_level`: one operating point, one
   member bin counted. Green (margins_channel 8/8).

Gates re-run: feature suites green (lang 24, margin_lowering 8,
cover_bin_mapping 7, api margins_channel 8 + reduced_metrics 3 +
coverage_merge 5 + violations_channel 5, python 3), `cargo build
--workspace` 0 warnings. **Verdict after round 1: PASS.**
