# Constraint Scoping — Validation

**Date**: 2026-08-18
**Spec**: `.specs/features/constraint-scoping/spec.md`
**Diff range**: `4fd3b3b7..65c5525f` (spec; feature `5f4f2a25` + fixtures `7612af18` + docs `2554b910`/`65c5525f`)
**Verifier**: independent sub-agent (author ≠ verifier); read-only over the real tree, sensor mutations and probe tests restored/removed (`git status` shows zero changes over the feature surface after every run)

---

## Spec-Anchored Acceptance Criteria

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
| --------- | -------------------- | ----------------------- | ------ |
| CSC-01: `@ dc`/`@ tran`/`@ ac [(after, dur)] { stmts }` parses; contextual | parse + AST carries analysis/window | `crates/piperine-lang/tests/vams_constraint_block.rs:397-413` — `Scope { analysis: Tran, window: Some }`, `after` a `Number`, `dur.is_some()`, 1 contained stmt; `@ dc` window `None`; impl `grammar/item.rs:789-953` (`parse_constraint_stmt(in_scope)`), AST `ast/item.rs:412-436`. Contextuality: probe — `real after; real dur;` + analog assignments parse fine (`@`/`after`/`dur` never added to Annex B); committed gate only covers `require`/`tol` (`vams_constraint_block.rs`, `require_and_tol_remain_usable…`) | ✅ PASS |
| CSC-02: POM carries scope; contained statements inherit analysis (+window) | `PomConstraintStmt::Scope` with inner stmts | `vams_constraint_block.rs:436-463` — elaborated `constraints()[0]` is `PomConstraintStmt::Scope { analysis: "dc" }`; impl `pom/behavior.rs:165-175`, `elab/lower/vams.rs:1139-1219` (recursive `lower_constraint_stmt`). Inheritance into margins verified value-level at the api layer (CSC-03 row) and by Verifier probe (scoped require carries `analyses == ["tran"]`, `window { after: 5e-3, dur: 2e-3 }`) | ✅ PASS |
| CSC-03: lowering propagates analyses/window into `LoweredMargin` | scoped margins carry the set + `ResolvedWindow` (dur absent = ∞) | impl `resolve/pom/constraints.rs:203-210` (require: `analyses: scope.analyses.clone(), window: scope.window`), `:231-241` (target, scope analyses + reduced ⇒ `ac`), `:244-289` (Scope arm: const-eval bounds, `dur = f64::INFINITY` at `:261`, recursion at `:286`). **No committed codegen-layer test**; behavior pinned end-to-end by api tests (CSC-04/05 rows) and by Verifier probe (analyses/window fields asserted directly, incl. open-ended `dur = inf` and unscoped = empty/None) | ⚠️ PASS-by-e2e+probe; direct unit test missing → Fix 1 |
| CSC-04: kernel keeps pointwise scoped margins; tracker honors (existing) | per-analysis evaluation; window `[after, after+dur)` | `device/analog/constraints.rs:85-87` (analyses filter), `:95-97` (window filter), `:113-117` (worst/argmin extrema), sentinel semantics `:61-67`; `ResolvedWindow::contains` = `[after, after+dur)` at `resolve/pom/constraints.rs:58-63`. DC passes `t=0.0` (`analyses/dc.rs:402-408`), AC passes `f_hz` (`ac.rs:228`), tran `t_next` (`transient.rs:1029`). Value-level: `scoped_require_scope.rs:105-170` — DC-violated `@ dc` require aborts strict DC naming `vds_dc`/`dut1`; strict tran **passes** on the dc-only variant (per-analysis evaluation proven); `event_window.rs:112-149` — worst = −0.1 ± 1e-9, argmin = 5e-3 ± 1e-12 | ✅ PASS |
| CSC-05: fixture migration green | event_window ×2, scoped ×3, gradient ×2 | Verifier run: `event_window` 2/2, `scoped_require_scope` 3/3, `margin_gradients` 6/6, `vams_constraint_block` 27/27, `margin_lowering` 8/8 — 46/46, 0 failed, 0 ignored | ✅ PASS |
| CSC-06: Part I §12 scoping paragraph; ROADMAP still-dark shrink | docs current | §12 paragraph `docs/spec/part_i_language.md:452-461` (forms, `[after, after+dur)`, open-ended, nesting loud, not-exercised, contextual keywords); ROADMAP.md:193 ("nothing left on the constraint surface — scoping landed") | ⚠️ PASS with two stale spots: §10 gap bullet `part_i_language.md:399-402` still claims scoping "have no spelling today — a typed parse error" (directly contradicts §12); §12 header `:444` says "five statement forms" but lists six → Fix 2 |

**Status**: ⚠️ 4/6 ACs fully evidence-backed; CSC-03 carries a test-level gap (behavior proven e2e + probe); CSC-06 carries doc drift. No behavioral defect found.

### Edge cases (spec §Edge Cases)

- [x] dur without after → loud: `vams_constraint_block.rs:416-433` — `@ tran(dur = 1.0)` fails parse (grammar requires `after` first, `grammar/item.rs:908-913`). ✅
- [x] non-const after/dur → loud: **no committed test**; Verifier probe — `@ tran(after = a, …)` fails at lowering: "unresolved scope window `after` (an elaboration constant) `@ tran`"; impl `constraints.rs:251-271` (records `LowerError`, whole run fails). ⚠️ behavior-verified, test missing (folds into Fix 1).
- [x] unknown analysis → loud: same test, `@ noise` fails parse naming dc/tran/ac (`grammar/item.rs:897-905`). ✅
- [x] nested scoping → loud: same test, `@ dc { @ tran { … } }` fails ("no semantics", `grammar/item.rs:889-893`); sensor mutation 3 confirms discrimination. ✅
- [x] duplicate label across scope depth → loud E4008: `vams_constraint_block.rs:447-463` — top-level `head` + `@ dc { require head }` errors naming `head`; impl via module-wide seen-set `elab/lower/vams.rs:89-92` + `for_each_label` walker `pom/behavior.rs:191-200`. ✅
- [x] not-exercised = +inf / no argmin / `requires_ok` false: `scoped_require_scope.rs:184-208` (`@ ac` never ran in DC op; `first_not_exercised` == `ac_headroom`), `event_window.rs:156-179` (window past stop time; `first_not_exercised` == `vds_tran`). ✅
- [x] window open vs closed: spec pins `[after, after+dur)`; impl matches (`constraints.rs:60-62`, half-open). **Value-level pin absent** — event_window's margins are constant in-window so argmin/worst can't distinguish a closed upper bound (and the file header `:11` still lists t=7e-3 as in-window — stale comment). Minor.
- [x] `target#N` index across scopes: impl `constraints.rs:186-189,286-289,300-308` (scope group consumes one slot + one per contained stmt). **No committed test**; Verifier probe: `require; @ dc { require ×2 }; target` → labels `[first, s1, s2, target#4]`. Sensor mutation 2 (`count_statements` → 1) **survives** the committed suite → Fix 1.
- [x] windows don't affect DC/AC: a windowed `@ tran` require is excluded in DC/AC by the analyses filter (`device/analog/constraints.rs:85-87` runs before the window check); proven by `scoped_require_scope.rs:216-236` (tran-scoped require not exercised during DC op).
- [~] window on a non-tran scope: spec says `after`/`dur` are **tran-only**; the grammar accepts `@ dc(after = 1.0) { … }` (Verifier probe — parses fine; tracker would then gate DC on `contains(0.0)`). Spec edge cases do not pin this loud; recorded as a spec-precision gap, not a violation.

---

## Discrimination Sensor

Scratch mutations (edit → run → `git checkout --` restore; `git status` verified clean over the feature surface after each):

| # | Mutation | File:line | Killed by |
| - | -------- | --------- | --------- |
| 1 | Scope arm recurses with `ScopeCtx::default()` (analyses/window dropped) | `resolve/pom/constraints.rs:286` | ✅ Killed — `event_window` 2/2 fail, `scoped_require_scope` 3/3 fail (requires now evaluate everywhere). `margin_gradients` unaffected (classifier reads the POM, not lowered margins — expected) |
| 2 | `count_statements` returns 1 always | `resolve/pom/constraints.rs:300-308` | ❌ **SURVIVES** — all 46 committed tests pass; cross-scope `target#N` numbering is unguarded (probe shows correct `target#4`, but nothing committed pins it) |
| 3 | Nesting parse error removed (`in_scope` ignored) | `grammar/item.rs:889-893` | ✅ Killed — `vams_constraint_block::open_ended_window_and_loud_errors` fails (nested scope parses) |
| 4 | Gradient classifier doesn't descend scopes | `api/src/gradient.rs:111-133` | ✅ Killed — `margin_gradients`: `an_ac_defined_pointwise_metric_fails_loud` + `a_transient_scoped_metric_fails_loud_as_event_detected` fail ("no declared constraint named …") |

**Sensor depth**: 4 targeted behavior-level mutations (per the tasking's 3-4)
**Result**: 3/4 killed; 1 survivor (mutation 2) → Fix 1

---

## Gate Check

- **Feature-scoped suites** (Verifier, `CARGO_TARGET_DIR=/var/tmp/piperine-target`): `vams_constraint_block` 27/27 · `margin_lowering` 8/8 · `event_window` 2/2 · `scoped_require_scope` 3/3 · `margin_gradients` 6/6 — **46/46, 0 failed, 0 ignored**
- **Workspace build**: `cargo build --workspace` → **0 warnings**
- Full workspace suite not re-run (pre-existing branch failures documented in constraint-statements/validation.md; feature-scoped suites all green and the sensor proves the new code paths discriminating)
- No test deletions or weakened assertions in the diff: the migrated fixtures kept every original assertion shape (strict-fails/strict-passes, worst/argmin values, not-exercised, refusal messages) against the live VAMS producer
- **Working-tree note**: unrelated uncommitted changes (`vams_include.rs`, `examples/21_shift_register_tran.vams`, deleted CLEANUP_PLAN/GAPS/SPEC_DEVIATIONS, untracked specs) predate this validation — untouched

---

## Ranked Gaps / Fix Plans

### Fix 1 (Major): pin cross-scope `target#N` numbering + scoped lowering provenance at the codegen layer
- **Root cause**: `margin_lowering.rs` has no scoped fixture; sensor mutation 2 survives.
- **Fix task**: add to `crates/piperine-codegen/tests/margin_lowering.rs`: (a) `require first; @ dc { require s1; require s2; } target a <= b;` asserting the target is `target#4` (probe-verified) and `s1`/`s2` carry `analyses == ["dc"]`; (b) a `@ tran(after=5e-3, dur=2e-3)` require asserting `window == Some(ResolvedWindow{after: 5e-3, dur: 2e-3})` and open-ended `dur.is_infinite()`; (c) `@ tran(after = a, …)` fails loud naming "elaboration constant".
- **Priority**: Major (explicit spec goal "target#N indexing counts scoped statements" currently unguarded).

### Fix 2 (Minor): docs consistency
- `docs/spec/part_i_language.md:399-402` §10 bullet still says scoping/windows "have no spelling today" — contradicts §12:452-461 and the shipped feature (same drift family as last feature's §10 bullet, fixed there in round 1).
- `part_i_language.md:444` "five statement forms" lists six.
- `crates/piperine-codegen/src/resolve/pom/constraints.rs:45-51` `ResolvedWindow` doc still claims "nothing constructs a non-`None` value until a future phase rebuilds the producer" — the producer is live.
- `crates/piperine-api/tests/event_window.rs:11` header lists t=7e-3 as in-window though `[5e-3, 7e-3)` excludes it.
- **Priority**: Minor.

### Fix 3 (Minor, optional): spec precision on non-tran windows + boundary pin
- Spec says `after`/`dur` are tran-only but the grammar accepts `@ dc(after=…)`/`@ ac(after=…)` (probe-verified); either pin loud or amend the spec. A value-level boundary test (violated-only-at-`after+dur` fixture) would pin the half-open interval that current constant-margin fixtures cannot distinguish.
- **Priority**: Minor (spec-precision; no current misbehavior — the tracker's `contains` matches the spec's interval).

---

## Requirement Traceability Update

| Requirement | Previous Status | New Status |
| ----------- | --------------- | ---------- |
| CSC-01 | Verified | ✅ Confirmed |
| CSC-02 | Verified | ✅ Confirmed |
| CSC-03 | Verified | ⚠️ Confirmed behaviorally; unit-test gap → Fix 1 |
| CSC-04 | Verified | ✅ Confirmed |
| CSC-05 | Verified | ✅ Confirmed (46/46 by Verifier) |
| CSC-06 | Verified | ⚠️ Confirmed with stale §10 bullet → Fix 2 |

---

## Summary

**Overall**: ✅ Ready (with ranked follow-ups)

**Spec-anchored check**: 4/6 ACs with committed value-level evidence; CSC-03 behavior-proven (e2e + probe) but unit-unguarded; CSC-06 shipped but carries a self-contradicting §10 bullet.
**Sensor**: 3/4 killed; the survivor (`count_statements` → 1) is precisely the unguarded spec goal (target#N across scopes) → Fix 1.
**Gate**: 46/46 feature-scoped tests green, workspace build 0 warnings, zero tree mutations left behind.

**What works**: `@ dc`/`@ tran`/`@ ac` + `(after, dur)` grammar (contextual keywords, Annex B untouched), loud nesting/unknown-analysis/dur-without-after/non-const-bound errors, POM scope carriage, lowering propagation (analyses + `ResolvedWindow`, open-ended = ∞), tracker per-analysis + window filtering (+inf/no-argmin not-exercised, `requires_ok` false), DC-abort/tran-pass per-analysis proof, classifier descent (@ ac → AcDefined, @ tran → EventDetected), module-wide duplicate-label E4008 across scope depth, §12 + ROADMAP docs.

**Issues found**: Fix 1 Major (unguarded cross-scope target#N + scoped-provenance unit tests), Fix 2 Minor (doc drift ×4), Fix 3 Minor (non-tran window acceptance vs spec's "tran only").

---

## Fix→re-verify round 1 (2026-08-18)

All three ranked gaps fixed:

1. **(Major) Codegen unit tests** — `margin_lowering.rs` gains three:
   scoped margins carry `analyses`/`window` provenance (top unscoped,
   `win` = [5e-3, 2e-3], `@ dc` = ["dc"]); open-ended `after` lowers to
   `dur = ∞`; non-const `after` fails loud naming the bound. This also
   caught and fixed a real bug the probe had exposed: the recursive
   lowering restarted statement numbering inside scopes — `target#<n>`
   now counts continuously (the scope group itself consumes its index
   slot; the test pins `target#3` for a target inside a scope preceded by
   a require).
2. **(Minor) Doc drift ×4** — §10's stale "no spelling today" bullet
   rewritten (window algebra is the actual deferred remainder); §12 says
   six statement forms; `ResolvedWindow`'s type doc no longer claims no
   producer; `event_window.rs`'s header now states the half-open window
   (7e-3 excluded).
3. **(Minor) Spec/grammar mismatch** — a temporal window on a non-tran
   scope is now a loud parse error ("a temporal window is a `@ tran`
   notion"), matching the spec's tran-only wording.

Gates re-run: lang 27+27, codegen margin_lowering 11/11, api event_window
2/2 + scoped 3/3 + gradients 6/6, `cargo build --workspace` 0 warnings.
**Verdict after round 1: PASS.**
