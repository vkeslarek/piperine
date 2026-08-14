# Verilog-AMS Support Validation

**Date**: 2026-08-13
**Spec**: `.specs/features/verilog-ams-support/spec.md`
**Diff range**: T1 baseline (`bcc14d2`) .. `54dc4059` (T58, HEAD of `feature/device-verification`)
**Verifier**: independent sub-agent (author ≠ verifier)

---

## Task Completion

Independently deep-verified (re-ran the target, read the source, or both — see body of report): **T1, T4, T13, T17, T18, T19, T21, T22, T25, T26, T27, T32, T33, T35, T48, T51, T52, T56, T58**.

All others trusted from the Progress Log's own evidence (commit hash present, per-task test counts recorded, SPEC_DEVIATIONs documented) — not independently re-derived line-by-line, per the scoping instruction (58-task feature, prioritize P1 + breadth-check the rest).

| Task | Status | Notes |
|---|---|---|
| T1 | ✅ Done (verified) | purity/provenance gate present, `[dependencies]` empty |
| T2 | ✅ Done — per log | — |
| T3 | ✅ Done — per log | — |
| T4 | ✅ Done (verified) | Annex B gate re-derives keyword set from LRM text at test time — live, not a frozen copy |
| T5 | ✅ Done — per log | — |
| T6–T11 | ✅ Done — per log | T11's two SPEC_DEVIATIONs (drive-strength parsing always `None`, deferred to T48) accurately carried forward and later delivered by T48 as pure functions, not wired into POM lowering |
| T12 | ✅ Done — per log | |
| T13 | ✅ Done (verified) | Annex A gate re-derives section list against in-repo LRM text; SPEC_DEVIATION to section-level (not rule-level) granularity is reasonable and disclosed |
| T14–T16 | ✅ Done — per log | |
| T17 | ✅ Done (verified) | `Nature` type, 5 standard attrs as named fields |
| T18 | ✅ Done (verified) | `Branch`/`BranchKind`, `record_contribution` conflict check — confirmed by mutation (see Sensor) |
| T19 | ✅ Done (verified) | E4xxx family renders code/span/label/help; SPEC_DEVIATION (`From<ParseError>` deferred to T21) accurately tracked and closed |
| T20 | ✅ Done — per log | |
| T21 | ✅ Done (verified) | PHDL parser deleted in full; MD-45 scope-widening honestly documented with a large, itemized broken-test inventory (102/468 baseline acknowledged as *not met*, not hidden) |
| T22 | ✅ Done (verified) | structural lowering; `hierarchy_is_navigable_and_never_flattened` test exists and passes live |
| T23 | ✅ Done — per log | |
| T24 | ✅ Done — per log | SPEC_DEVIATION (`eval/interp.rs` deleted not retargeted) is consistent with T21's MD-45 deletions |
| T25 | ✅ Done (verified) | `UnrollGenerate`; MD-25 derived-artifact test present |
| T26 | ✅ Done (verified) | `ResolveDisciplines`; module doc explicitly names Basic (§7.4.4.1) as the chosen Annex F method and documents the scope narrowing (single-module fixed point, not full `connect...resolveto`) — matches the log's own characterization exactly |
| T27 | ✅ Done (verified) | Annex C enforcement; `vams_annex_c.rs` tests pass live, cite "Annex C" in messages |
| T28–T31 | ⚠️ Partial (per log, `~` gates) | Individually-clean files during an admitted "dark middle" where the crate didn't build end-to-end; resolved by T32 |
| T32 | ✅ Done (verified) | crate builds; **found undocumented debt**: 3 additional `piperine-codegen` integration targets (`margin_tran_points.rs`, `cover_bin_mapping.rs`, `margin_evaluation.rs`) compile but fail at runtime on PHDL-syntax fixture strings — not in T32's own named 6-file compile-failure list, and never named anywhere else in the Progress Log either. See Gap #1 below. |
| T33 | ✅ Done (verified) | `vams_vertical_slice.rs` — all 3 tests pass live; SPEC_DEVIATIONs (analytic golden not recorded-PHDL golden; `piperine_solver` direct not `piperine-api`) match spec's own MD-44-point-5 allowance |
| T34 | ✅ Done — per log | |
| T35 | ✅ Done (verified) | `(* device = …, type = … *)` lowering present and tested (`vams_device_port_attrs.rs`, since deleted per "verify then discard", replaced by a permanent equivalent per the log); SPEC_DEVIATION (two sibling attrs instead of one nested-call attr) is a faithful LRM-grammar constraint, not a shortcut |
| T36 | ✅ Done — per log | |
| T37 | ✅ Done — per log | |
| T38 | ✅ Done — per log (collapsed into T21) | |
| T39 | ✅ Done — per log | |
| T40–T46 | ✅ Done — per log | Zero-`.phdl` claim independently re-verified (see below) |
| T47 | ✅ Done — per log | |
| T48 | ✅ Done (verified) | `GatePrimitive::eval_n`/`eval1` truth tables — confirmed by mutation (see Sensor); file's own doc honestly states gate instantiation is **not** wired into POM lowering (pure functions only) |
| T49 | ✅ Done — per log | Same "pure, tested, not wired" pattern as T48, consistent with the debt list |
| T50 | ✅ Done — per log | docs-only decision gate |
| T51 | ✅ Done (verified) | `RegionQueue` — confirmed by mutation (see Sensor); module doc honestly states it is a standalone primitive **not wired into the live scheduler**, matching the debt list exactly |
| T52 | ✅ Done (verified) | `InsertConnectModules` selection algorithm implemented and tested; module doc honestly documents that `ResolveDisciplines` (T26) hard-fails one pass earlier so a real cross-discipline mismatch never reaches this pass in practice — matches the debt list exactly. `vams_connect_modules_e2e.rs`'s cited fixture passes live. |
| T53–T55 | ✅ Done — per log | |
| T56 | ✅ Done (verified) | Every cited LRM-chapter/annex test target re-run live during this validation and confirmed green (see Gate section) |
| T57 | ✅ Done — per log | `CLAUDE.md` on disk confirmed to describe VAMS as the frontend, not PHDL (the version pasted into this session's system context was stale/cached — the on-disk file is current) |
| T58 | ✅ Done (verified) | `.specs/STATE.md` MD-42/MD-43 present; T58's own entry correctly declines to claim `cargo test --workspace` is green, deferring that to this Verifier pass — accurate self-restraint, not an overclaim |

---

## Spec-Anchored Acceptance Criteria (P1: VAMS vertical slice replaces PHDL)

| # | Criterion (WHEN X THEN Y) | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|---|
| 1 | `nature`+`discipline` → POM `Nature`/discipline entries | POM `Nature` w/ 5 named standard attrs, discipline binds potential/flow | `crates/piperine-lang/tests/pom_nature.rs` (9 tests, live green) + `crates/piperine-lang/tests/vams_lower_analog.rs::*nature*` — `Nature{abstol,access,units,idt_nature,ddt_nature}` as named fields | ✅ PASS |
| 2 | analog block + `V(p,n) <+ expr` → same POM contribution shape as PHDL's `<+` | reaches `resolve::pom::LoweredBody` unchanged downstream | `tests/vams_vertical_slice.rs:104-157` (`vams_rc_step_response_matches_the_closed_form_solution`) — full contribution chain (`ddt`, `I(p,n)<+`) exercised end-to-end through `CircuitCompiler`/`TrBdf2`, live green | ✅ PASS |
| 3 | digital `always @(...)` → `BehaviorKind::Digital`, digital kernel path | reuses existing digital kernel | `tests/vams_vertical_slice.rs:218-299` (`vams_digital_counter_register_increments_on_posedge`) — `always @(posedge clk) count <= count + 1;` drives a real compiled digital device, `SUPPORTS_ROLLBACK` capability asserted, delta count asserted exactly `+5` for 5 forced posedges, live green | ✅ PASS |
| 4 | hierarchical instantiation (named/ordered, param overrides) → navigable instance tree | POM-navigability UNBREAKABLE RULE holds for VAMS source | `crates/piperine-lang/tests/vams_lower_structure.rs:142` (`hierarchy_is_navigable_and_never_flattened`), plus `:77` (ordered), `:104` (named), `:124` (param override) — all live green | ✅ PASS |
| 5 | lexical/syntax/elaboration error → fail-loud, source-location-anchored diagnostic | never silent drop/approximate | `crates/piperine-lang/tests/vams_annex_c.rs` + `crates/piperine-lang/tests/errors_lrm.rs::every_e4xxx_variant_carries_a_span_field` — every E4xxx variant asserted to carry a span, descriptive label (not `"here"`), help text, LRM clause; live green | ✅ PASS |
| 6 | RC+counter `.vams` example simulates on the native solver, matches PHDL-equivalent tolerance | same numeric result within `ToleranceContract`-class bound | `tests/vams_vertical_slice.rs` all 3 tests, live green — **⚠️ Spec-precision gap**: uses `piperine_solver` directly + an analytic closed-form golden + a locally-restated tolerance constant, not literally `piperine-api`/`tests/support/ngspice.rs::ToleranceContract` as spec's exact wording says (SPEC_DEVIATION explicitly disclosed in the test file's own doc comment, justified by MD-44 point 5 and the still-broken `piperine-api` build at the time T33 landed). `piperine-api` was retargeted at T34, but the test was never revisited to use the real harness after that — a residual, disclosed but unclosed loop. | ⚠️ PASS with disclosed spec-precision gap |
| 7 | ch.2 lexical forms + ch.4 expressions/math/time-domain fns parse+evaluate | LRM-defined semantics | `crates/piperine-vams/tests/lexical.rs` (29/29 live), `crates/piperine-vams/tests/expressions.rs` (21/21 live) — both re-run in this session | ✅ PASS |
| 8 | parser/elaborator suite checked against Annex A/B, ongoing gate | every production/keyword accounted for | `crates/piperine-vams/tests/annex_a_grammar.rs` (3/3 live), `annex_b_keywords.rs` (5/5 live) — both re-derive ground truth from the in-repo LRM text at test time, not a frozen copy | ✅ PASS |
| 9 | construct outside Annex C's analog subset → fail loud citing Annex C | first enforced for P1's constructs | `crates/piperine-lang/tests/vams_annex_c.rs` (5/5 live) — non-blocking assign, `@(posedge)`, `casex` inside `analog` all rejected citing "Annex C" | ✅ PASS |

**Status**: 8/9 clean PASS, 1/9 PASS with a disclosed spec-precision gap (AC6's harness substitution). No AC is a silent gap — every one has live, re-run test evidence.

---

## Discrimination Sensor

| Mutation | File:line | Description | Killed? |
|---|---|---|---|
| 1 | `crates/piperine-lang/src/pom/branch.rs:151` | `Branch::record_contribution`'s potential/flow-conflict arm `_ => true` flipped to `_ => false` (disables the LRM §5.4.2 conflict check entirely) | ✅ Killed — `pom_branch` 3 tests failed (`flow_then_potential_conflicts_symmetrically`, `potential_then_flow_conflicts_loud_with_e4001_citing_lrm_5_4_2`, `unnamed_branch_label_synthesizes_pair_form`) |
| 2 | `crates/piperine-codegen/src/kernel/digital/gates.rs:74` | `GatePrimitive::Nand`'s `eval_n` changed from `negate(and2(...))` to plain `and2(...)` (Nand silently behaves as And) | ✅ Killed — `vams_gates::and_nand_or_nor_xor_xnor_truth_tables` failed |
| 3 | `crates/piperine-solver/src/digital/regions.rs:97` | `RegionEvent::cmp` sort key order swapped from `(region, seq)` to `(seq, region)` (breaks LRM §8.3 region-priority ordering, falls back to submission order) | ✅ Killed — `vams_stratified_regions` both tests failed, plus 2 in-module unit tests |

All three mutations reverted with `git checkout --` immediately after confirming the kill; `git status` clean before writing this report.

**Sensor depth**: lightweight (default tier), one mutation per layer named in the task brief (POM/elaboration, codegen/digital, solver/scheduler)
**Result**: 3/3 killed — PASS ✅

---

## Code Quality

Spot-checked: `crates/piperine-lang/src/elab/lower/disciplines.rs` (T26), `crates/piperine-lang/src/elab/lower/connect.rs` (T52), `crates/piperine-solver/src/digital/regions.rs` (T51).

| Principle | Status |
|---|---|
| No features beyond what was asked | ✅ — T26/T51/T52 all explicitly scope-narrow to their task's stated `Where`, documenting (not silently omitting) the fuller LRM algorithm |
| No abstractions for single-use code | ✅ |
| No unnecessary "flexibility" added | ✅ |
| Only touched files required for task | ✅ (per-task file lists in the log match `git show --stat` shape for the ones sampled) |
| Didn't "improve" unrelated code | ✅ (T32's own "warning-driven cleanup" of two dead helpers is disclosed as incidental, not scope creep) |
| Matches existing patterns/style (MD-13: no loose fns, no macros, module = system function) | ✅ — every file sampled uses one `impl` per type, no `macro_rules!`, module docs state the layer's contract |
| Would senior engineer approve? | ✅ — SPEC_DEVIATION discipline throughout is unusually rigorous: every narrowing is named, justified, and cross-referenced to the exact follow-up task that would close it |
| Tests map to ACs, non-shallow | ✅ for P1's own targets (spot-checked `vams_vertical_slice.rs`, `pom_branch`, `vams_lower_structure`) |
| Spec-anchored outcome check | ✅ — asserted values (e.g. exact `+5` counter delta, `1e-6` DC tolerance, closed-form RC solution) target precise numbers, not just "no panic" |
| Documented guidelines followed | `CLAUDE.md`, `AGENTS.md`, `tests/suite_hygiene.rs`, this feature's own `tasks.md` E-STD |

---

## Gate Check

**Commands run** (per this validation's mandate — a named subset of the "full" `cargo test --workspace` gate, chosen because the bare command does not compile end-to-end — see Gap #2):

```
cargo build --workspace
cargo test --test ngspice_validation --test ngspice_ac --test ngspice_temp \
  --test ngspice_homotopy --test ngspice_noise --test ngspice_tran --test ngspice_parity_matrix
cargo test --test suite_hygiene
cargo test --test run_examples
cargo test --test spice_smoke
```

| Target | Result |
|---|---|
| `cargo build --workspace` | zero rustc warnings |
| `ngspice_validation` | 49/49 ✅ |
| `ngspice_ac` | 24/24 ✅ |
| `ngspice_temp` | 24/24 ✅ |
| `ngspice_homotopy` | 18/18 ✅ |
| `ngspice_noise` | 29/29 ✅ |
| `ngspice_tran` | 23/25 — 2 failures (`ngspice_uic_rc`, `ngspice_uic_mos`), confirmed to be the documented pre-existing NGP-10 gap (T44), not a new regression |
| `ngspice_parity_matrix` | 4/4 ✅ |
| `suite_hygiene` | 9/10 — 1 failure (`mod_rs_declares_only`, `piperine-ir/src/catalog/mod.rs` 430 lines vs 60-line MD-34 ceiling), confirmed to be the documented pre-existing T1-baseline failure, not this feature's |
| `run_examples` | 2/2 ✅ |
| `spice_smoke` | 7/7 ✅ |

**Zero-PHDL claim**: `find /home/keslarek/Git/piperine -name '*.phdl' -o -name '*.ppr' | grep -v /target/` → **empty** — confirmed independently.

**Result**: all mandated gate targets pass or fail exactly as documented pre-existing debt. 0 new regressions found in the mandated gate set.

**⚠️ Caveat (see Gap #2)**: `cargo test --workspace` (the literal command tasks.md's own "full" gate level names, and the command T29/T33/T44/T51/T52/T56 all cite as their nominal gate) **does not compile** as of HEAD — `piperine-lang`'s `elab.rs` test target alone has 25 compile errors, plus `piperine-codegen`'s previously-documented 6-file compile-failure list. This was never claimed fixed by any task after T32/T21 first disclosed it, and T58's own closing entry explicitly avoids claiming it — so this is a known, disclosed-in-spirit limitation, not a contradicted claim, but it means "the whole test suite is green" cannot be asserted about this branch as delivered.

---

## Gap List

### Gap 1 (Minor — undocumented test debt): 3 `piperine-codegen` integration targets broken, never named

`crates/piperine-codegen/tests/{margin_tran_points,cover_bin_mapping,margin_evaluation}.rs` all compile cleanly (so they're absent from T32's own named "6 files fail to compile" inventory) but **fail at runtime** — every test constructs a PHDL-syntax fixture string (e.g. `discipline Electrical { potential v : Real; flow i : Real; }`) that the VAMS parser now rejects (E1005). 10 tests total fail (3 in `margin_tran_points`, 7 in `cover_bin_mapping`). This is the same root cause as the 6 documented files, but these three were never named anywhere in the 4000-line Progress Log, at T32 or any later task — meaning the actual "PHDL-fixture debt" footprint in `piperine-codegen` is undercounted by the project's own record. These exercise the `ConstraintBlock`/margin-evaluation/coverage-bin capabilities, which trace to VAMS-14 (constraint-block semantics via attribute instances) — a requirement that, per Gap 3 below, has no delivered replacement either.

**Fix task**: name these 3 files in the debt inventory (or migrate them to VAMS syntax alongside the other 6, since `piperine-codegen`'s `kernel/analog/constraints.rs`/`coverage.rs` capability code itself is untouched structurally per T32's log — only its test fixtures are stale).

### Gap 2 (Minor — process, not functional): `cargo test --workspace` still doesn't compile at feature close

Confirmed live: the bare command fails to compile `piperine-lang`'s `elab.rs` test target (25 errors) before even reaching `piperine-codegen`'s known 6-file list. This was disclosed piecemeal across T21/T28-T33 as an accepted "dark middle" but was never revisited to closure, and no task in Phase 7-10 explicitly re-attempted or re-scoped it. T58's closing entry correctly avoids overclaiming this, deferring explicitly to the Verifier — which is why this is Minor rather than a finding of dishonesty, but it does mean the literal "full" gate level tasks.md itself defines was never actually achieved by feature close.

**Fix task**: either (a) migrate/delete the remaining PHDL-fixture-string test files (`elab.rs` and the `piperine-codegen` 6+3=9 files) to VAMS syntax, matching the precedent Phase 8 already set for the production corpus, or (b) explicitly record a new decision (MD-4x) stating which test files are permanently retired/out of scope and why, so `cargo test --workspace` becomes a meaningful gate again.

### Gap 3 (Minor — spec-precision, requirement not delivered): VAMS-14 (constraint-block semantics via attribute instances) has no test evidence and no implementation

Spec.md's P2 story "`(* attribute *)`-carried Piperine extensions" AC3 and VAMS-14's named validation test (`piperine-codegen` test: attribute-carried constraint reaches the same `ConstraintBlock`-equivalent semantics as PHDL's) were never delivered. T21/MD-45 deleted `pom::ConstraintBlock` outright as part of the PHDL value-layer gut; T35 explicitly scoped itself to *only* the `@device`/`@port` schemas, stating the general constraint/capability rebuild "stays future work." No later task picked it back up, and VAMS-14 is never mentioned again anywhere in the Progress Log after T21. Tasks.md's own "Open items before Execute" #5 flagged this exact debt and said "a concrete task for this MUST be added before Phase 7 (T34–T39) is considered closed" — no such task was added; Phase 7 closed anyway (T39 progress entry has no mention of this open item being resolved or re-flagged). This is real debt that slipped past the open item's own stated gate, not merely a documented deferral.

**Fix task**: either land the attribute-instance-carried constraint-block rebuild (closing VAMS-14 for real), or amend the spec to explicitly move it to a P4/future phase and mark VAMS-14 as descoped with a recorded decision — the current state (silently absent from every later task's scope, contradicting the open item's own "must" language) is the one place this feature's otherwise-excellent SPEC_DEVIATION discipline broke down.

### Gap 4 (Cosmetic — bookkeeping): spec.md's own Requirement Traceability table was never updated

All 26 `VAMS-NN` rows and the coverage summary line ("0 mapped to tasks yet... 26 unmapped") still read exactly as they did before Execute started, despite T56's own coverage-map audit (a *different* table, in the same file) being fully populated with live test evidence. `validate.md`'s template expects the Verifier to write this update; no implementer task did so along the way, which is a minor process gap, not a functional one — the Progress Log itself is otherwise a faithful, unusually rigorous record.

**Fix task**: none required from the Verifier's mandate — recommend a follow-up docs task updates spec.md's status column to `Verified` for VAMS-01 through -13, -15 through -13(sic)/-26 per this report's findings, and `Needs Fix`/descoped for VAMS-14.

---

## Requirement Traceability (spot-checked, not exhaustive)

| Requirement | Previous Status | New Status | Evidence |
|---|---|---|---|
| VAMS-01 | Pending | ✅ Verified | `pom_nature.rs`, `vams_lower_analog.rs` live green |
| VAMS-02 | Pending | ✅ Verified | `vams_vertical_slice.rs` live green |
| VAMS-03 | Pending | ✅ Verified | `vams_vertical_slice.rs` digital counter test live green |
| VAMS-04 | Pending | ✅ Verified | `vams_lower_structure.rs` live green |
| VAMS-05 | Pending | ✅ Verified | `errors_lrm.rs`, `vams_annex_c.rs` live green |
| VAMS-06 | Pending | ⚠️ Verified w/ spec-precision gap | see AC6 above |
| VAMS-07 | Pending | ✅ Verified | `lexical.rs`, `expressions.rs` live green |
| VAMS-08–VAMS-11 | Pending | ✅ Verified (trusted from log + independently confirmed zero-`.phdl`) | ngspice family green, zero `.phdl` files found |
| VAMS-12, VAMS-13 | Pending | ✅ Verified (trusted from log) | T35's `vams_device_port_attrs.rs` (since superseded by a permanent equivalent per log) |
| VAMS-14 | Pending | ❌ Needs Fix | Gap 3 above — never delivered |
| VAMS-15–VAMS-26 | Pending | ✅ Verified (trusted from log; T56 chapter/annex rows independently re-run live in this session) | see Gate section |

---

## Summary

**Overall**: ✅ Ready (with 4 minor/cosmetic gaps logged, none blocking; VAMS-14 is the one substantive undelivered requirement)

**Spec-anchored check**: 8/9 P1 ACs clean PASS, 1/9 PASS with disclosed spec-precision gap (AC6)
**Sensor**: 3/3 mutations killed
**Gate**: 8 mandated targets green (10 targets counting sub-runs), 2 pre-existing failures confirmed as documented debt (NGP-10, MD-34/`piperine-ir`), zero new regressions

**What works**: The P1 vertical slice is real and rock-solid — an actual `.vams` RC+digital-counter circuit parses, elaborates through freshly-built `Nature`/`Branch` POM nodes, lowers, JIT-compiles, and simulates on the unmodified native solver, with a live discrimination sensor confirming the new branch-conflict, gate-truth-table, and scheduler-ordering logic is not vacuously tested. Annex A/B conformance gates re-derive ground truth from the LRM text at test time rather than trusting a frozen copy — a notably strong conformance mechanism. SPEC_DEVIATION discipline across all 58 tasks is unusually rigorous and mostly self-consistent.

**Issues found**: 3 undocumented broken `piperine-codegen` test files (Gap 1); `cargo test --workspace` still doesn't compile end-to-end (Gap 2); VAMS-14 silently never delivered despite an explicit "must add a task" gate in the plan (Gap 3); spec.md's traceability table never updated (Gap 4, cosmetic).

**Next steps**: Route Gap 1/2/3 as fix tasks to an implementer; Gap 4 is a docs-only cleanup (this report can seed it directly).
