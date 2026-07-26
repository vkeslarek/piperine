# Language Server Follow-up Bugs Validation

**Date**: 2026-07-25
**Spec**: `.specs/features/language-server-followup-bugs/spec.md`
**Diff range**: `96a0b1f..a2324f7` (T1-T9 code commits) + this report's own
commit (T10 close-out)
**Verifier**: standalone-fallback pass performed by the same batch worker
that implemented T6-T10, per this task's explicit instruction ("run the
standalone-fallback Verifier pass yourself ... you are a batch worker and
never spawn further sub-agents"). Independence from implementation is
approximated by re-deriving every AC's evidence directly against
`spec.md` (not against the implementer's own claims) and by running fresh
discrimination mutations beyond the one mandatory check already performed
inline during T8.

*Note: a validation.md existed at this path before this pass, dated to a
different (stale) session narrative — its gate-check section reported an
unrelated `piperine-plugin::process_smoke` flake that this session's own
full `cargo test --workspace` run did not reproduce. This report replaces
it with results freshly re-verified in this session.*

---

## Task Completion

| Task | Status  | Notes |
|------|---------|-------|
| T1   | ✅ Done | `96a0b1f` — `Resolver.item_files` |
| T2   | ✅ Done | `06468c5` — `Project.item_files`/`item_file()` |
| T3   | ✅ Done | `4b4a5ae` — `goto_def.rs` cross-file extern branch |
| T4   | ✅ Done | `bab93d5` — `doc` field on `ExternSig`/`ExternDecl` |
| T5   | ✅ Done | `623c7e5` — doc threaded into registries + `Resolution` |
| T6   | ✅ Done | `5d51952` — `///` docs authored on `ddt`/`Real` headers |
| T7   | ✅ Done | `c29fcb6` — `label_span`/`type_span` on AST/POM `Instance` |
| T8   | ✅ Done | `38617ad` — `symbol_index.rs` + `index_design` consistency |
| T9   | ✅ Done | `a2324f7` — completion suppression heuristic |
| T10  | ✅ Done | This report + `cargo test --workspace` green |

---

## Spec-Anchored Acceptance Criteria

### P1: BUG-1 — goto-definition works for `extern` names

| Criterion (WHEN X THEN Y) | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| LSB-01: goto on `ddt`/any `extern` name → real declaring file + correct range | `Location.uri` ends `headers/operators.phdl`; range offset == `header_text.find("extern operator ddt")` | `crates/piperine-lang-server/tests/integration_test.rs:684-704` (`goto_definition_on_ddt_lands_on_operators_header`) — `assert!(uri_str.ends_with("headers/operators.phdl"))`, `assert_eq!(expected_offset_byte, expected_offset, ...)` | ✅ PASS |
| LSB-02: `use`-imported extern (e.g. `use spice::diode`) → still resolves to real on-disk file | goto lands on the real file the item was loaded from, not just the 5 embedded headers | `crates/piperine-lang/tests/elab.rs:499-508` (`test_use_loaded_item_maps_to_real_on_disk_path`) — `assert_eq!(std::fs::canonicalize(mynet_path).unwrap(), std::fs::canonicalize(&lib_path).unwrap(), ...)` proves `item_files` threading for `use`-loaded items; `goto_def.rs`'s `cross_file_location` (`crates/piperine-lang-server/src/handlers/goto_def.rs:63-82`) is generic over any `resolution.file`, not prelude-specific | ⚠️ Spec-precision gap — unit-level proof of the underlying mechanism exists and is solid; no dedicated LSP-level integration test drives goto specifically on a `use spice::...`-loaded extern name end-to-end. The mechanism is the exact same code path as LSB-01's fully-covered case, so risk is low, but this is a real coverage gap, not fabricated coverage. |
| LSB-03: extern declared in the *current* document still resolves (no regression) | goto stays same-file, unaffected by the new cross-file branch | `crates/piperine-lang-server/tests/integration_test.rs:712-737` (`goto_definition_on_same_file_extern_decl_still_works`) | ✅ PASS |

### P2: BUG-2 — hover shows `///` docs for `extern` declarations

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| LSB-04: `///`-preceded extern renders doc as Markdown on hover | hover contents contain the authored doc text | `crates/piperine-lang-server/tests/integration_test.rs:425-443` (`hover_on_documented_extern_operator_renders_doc_as_markdown`) — `assert!(contents.contains("Time derivative of its argument."), ...)` | ✅ PASS |
| LSB-05: extern with no `///` renders unchanged (no doc paragraph) | hover contents have no doc text | `crates/piperine-lang-server/tests/integration_test.rs:449-467` (`hover_on_undocumented_extern_operator_is_unchanged`) — `assert!(!contents.to_lowercase().contains("derivative"), ...)` | ✅ PASS |
| LSB-06: hovering the *real* `ddt` shows the doc authored in this batch (T6) | `ctx.operators.lookup("ddt").doc()` returns the real header prose, not a synthetic fixture | `crates/piperine-lang/tests/elab.rs:529-553` (`test_ddt_doc_comes_from_the_real_header_content`) — `assert!(doc.contains("ddt(qtotal)"), ...)`, cross-checked against `std::fs::read_to_string(headers/operators.phdl)` | ✅ PASS |

### P3: BUG-3 — document-highlight/goto targets the clicked token on a labeled instance

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| LSB-07: click label → highlight covers only the label token | 3-byte range (`"src"`), not 56-byte whole statement | `crates/piperine-lang-server/tests/instance_highlight.rs:29-48` (`highlighting_labeled_instance_label_targets_only_the_label_token`) — `assert_eq!(end - start, 3, ...)`, `assert_eq!(&SRC[start..end], "src")` | ✅ PASS |
| LSB-08: click type name → highlight covers only the type-name token | 10-byte range (`"RampSource"`) | `crates/piperine-lang-server/tests/instance_highlight.rs:50-67` (`highlighting_labeled_instance_type_name_targets_only_the_type_token`) — `assert_eq!(end - start, 10, ...)`, `assert_eq!(&SRC[start..end], "RampSource")` | ✅ PASS |
| LSB-09: unlabeled instance click → unchanged/improved (tight to type token, no regression) | still resolves; range now tight to type-name token | `crates/piperine-lang-server/tests/instance_highlight.rs:70-89` (`highlighting_unlabeled_instance_still_resolves_and_is_tight_to_type_token`) — `assert_eq!(end - start, 10, ...)` | ✅ PASS |
| LSB-10: goto-definition from label or type click resolves the same *target* as before (range fix must not break resolution) | cross-file goto on an instance's type name still lands on the declaring module | `crates/piperine-lang-server/tests/integration_test.rs:1359-1386` (`cross_file_goto_opens_the_declaring_file`) — `assert_eq!(loc.uri, a_uri, "goto on \`A\` must open a.phdl, not b.phdl")`. Note: this pre-existing test caught a real regression introduced by T7/T8's span tightening (`instance_module_type_at` matched only against the whole-statement `span`); fixed within T8 by also matching `label_span`/`type_span` — `crates/piperine-lang-server/src/symbol_index.rs:156-172` | ✅ PASS |

### P4: BUG-4 — completion doesn't suggest behavior-only keywords at true top level

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| LSB-11: cursor at module's `}` + `RBrace`+mod-body-only-keyword signature → behavior-only keywords absent | `"for"`/`"match"`/`"return"`/`"when"` all absent from labels | `crates/piperine-lang-server/tests/completion_suppression.rs:25-38` (`completion_right_after_module_close_brace_never_offers_for_and_offers_mod`), `:42-59` (`..._suppresses_all_behavior_only_keywords_but_keeps_if_and_var`) — `assert!(!labels.contains(&behavior_only), ...)` for each of the 4 | ✅ PASS |
| LSB-12: same condition → legitimate top-level keywords ARE offered | `"mod"` present in labels | `crates/piperine-lang-server/tests/completion_suppression.rs:33-36` — `assert!(labels.contains(&"mod"), ...)` | ✅ PASS |
| LSB-13: genuine mid-behavior-body cursor → `for` still offered (no regression) | `"for"` present | `crates/piperine-lang-server/tests/completion_suppression.rs:62-77` (`completion_mid_behavior_body_still_offers_for`) — `assert!(labels.contains(&"for"), ...)` | ✅ PASS |

**Status**: ✅ 12/13 ACs fully PASS with direct evidence. LSB-02 has a
⚠️ spec-precision gap (mechanism proven at unit level, no dedicated
LSP-level integration test) — noted as a gap, not silently passed.

---

## Discrimination Sensor

Four mutations run during this standalone Verifier pass, plus the
mandatory T8 mutation already performed and recorded during
implementation (reproduced here for completeness — not re-run, since the
T8 task explicitly performed and documented it already).

| # | File:line | Description | Killed? |
|---|-----------|-------------|---------|
| 1 (T8, during implementation) | `crates/piperine-lang/src/elab/resolution.rs` (`index_design`'s Instance loop) | Reverted `index_design`'s decl-span convention back to `i.span` (whole statement) while leaving `symbol_index.rs`'s half of the fix in place | ✅ Killed — `resolved_decl_span_has_a_matching_binding_in_the_resolution_index` failed; other 3 `instance_highlight.rs` tests still passed (confirming `occurrences_at`'s fallback would have masked a weaker test) |
| 2 (Verifier, this pass) | `crates/piperine-lang-server/src/symbol_index.rs:308` | `let extern_file = design.project().item_file(&word)...` → forced to always `None` (BUG-1's file-path lookup disabled) | ✅ Killed — `goto_definition_on_ddt_lands_on_operators_header` failed: `goto on \`ddt\` must land on headers/operators.phdl, got file:///goto_def_test.phdl` |
| 3 (Verifier, this pass) | `crates/piperine-lang-server/src/handlers/completion.rs` (the `if has_rbrace && has_mod_body_only` guard) | Changed the guard to `if has_rbrace && false` (never suppress) | ✅ Killed — both `completion_right_after_module_close_brace_never_offers_for_and_offers_mod` and `..._suppresses_all_behavior_only_keywords_but_keeps_if_and_var` failed, listing `"for"` in the returned labels |
| 4 (Verifier, this pass) | `crates/piperine-lang/src/elab/registry/operators.rs` (`ExternOperatorDecl::doc()`) | Reverted `fn doc(&self) -> Option<&str> { self.sig.doc.as_deref() }` → hardcoded `None` | ✅ Killed — `hover_on_documented_extern_operator_renders_doc_as_markdown` failed: hover contents lacked the expected doc text |

**Sensor depth**: lightweight (default tier) — mutations target BUG-1's
file-path lookup, BUG-2's doc-field wiring, BUG-3's dual-side span
convention, and BUG-4's suppression condition — the four highest-risk
seams this batch touched.
**Result**: 4/4 killed, 0 survived — PASS ✅

All mutations were applied directly to the real working tree (backed up
via `cp` before each edit, since a git worktree/stash round trip is
disruptive mid-incremental-build), restored from the backup immediately
after each mutant was confirmed killed, and verified via `git status
--short` (clean, only the pre-existing unrelated example-file diffs and
this task's own staged files) plus a green re-run of the affected test
before moving to the next mutation.

---

## Code Quality

| Principle | Status |
|---|---|
| Minimum code | ✅ — each task's diff is the smallest change satisfying its Done-when criteria (T9 is a single post-pass block; T6 is a two-comment-block edit) |
| Surgical changes | ✅ — only files listed per task touched; synthetic `Instance` construction sites (flatten.rs splice, staged instances, existing unit-test fixtures) got the minimum `None, None` needed to compile, not a redesign |
| No scope creep | ✅ — the `instance_module_type_at` fix in T8 was in-scope (spec.md AC4/LSB-10 explicitly requires goto resolution not regressing from the span-tightening change) |
| Matches patterns | ✅ — new tests follow the existing `DocumentState`-direct pattern (design.md's Test strategy section) and existing `Connection::memory()` integration-test conventions |
| Spec-anchored outcome check (asserted values match spec) | ✅ — byte-length assertions (3/10-byte ranges) match spec.md's exact reported offsets, not vague "some smaller range" assertions |
| Per-layer Coverage Expectation met | ✅ domain (piperine-lang) tests map 1:1 to LSB-01/03/04-06/07-10 unit-level ACs; integration (piperine-lang-server) tests cover happy path + edge cases (unlabeled instance, undocumented extern, mid-behavior-body) for every route in scope |
| Every test maps to a spec requirement | ✅ — no speculative tests found; all new tests trace to a specific LSB-NN |
| Documented guidelines followed | `.claude/skills/tlc-spec-driven/references/{implement,coding-principles}.md` — Conventional Commits, one-task-one-commit, Test Adequacy Review performed inline per task |

---

## Edge Cases (from spec.md)

- [x] extern name with no textual declaration (native-only, e.g. `rfport`) → goto declines rather than fabricating: unaffected by this batch (pre-existing T20 behavior; `resolution.file` stays `None` for it since it's not in `item_files`).
- [x] two files declaring the same extern name (shadowing) → resolves to whichever the compiler actually used: `item_file()` mirrors `origin_of`'s existing precedent (per T2's design), unchanged by T6-T10.
- [x] `///` before an `extern impl` block, not per-method → only block-level doc attaches: covered by T4's `extern_grammar.rs` tests (pre-existing, out of this batch's scope but not broken — `cargo test -p piperine-lang` still green).

---

## Gate Check

- **Gate command**: `cargo test --workspace`
- **Result**: 1050 passed, 0 failed, 0 skipped (unjustified) — 5 doctests `ignored` (pre-existing, unrelated to this feature: async-runtime-dependent doc examples in `piperine-plugin`/`piperine-plugin-wasm`/`piperine-solver`)
- **piperine-python::simulation_error**: did NOT flake this run (green). Documented in the task briefing as a known pre-existing cross-thread PyO3 flake unrelated to this work — not observed as a failure in this gate run, so no special handling was needed.
- **Failures**: none. (A stale prior `validation.md` at this path reported an unrelated `piperine-plugin::process_smoke` flake from a different session; this session's own full run did not reproduce it and is the authoritative result for this report.)
- **Test count before this batch** (T1-T5 baseline, from `e373b85`): workspace green at that commit (per T1-T5's own status notes)
- **Delta**: T6 added 1 test, T7 added 2 tests, T8 added 4 tests (3 highlight + 1 index-consistency), T9 added 3 tests — 10 new tests total across T6-T9, all passing

---

## Requirement Traceability Update

| Requirement | Previous Status | New Status |
|---|---|---|
| LSB-01 | Done (T3) | ✅ Verified |
| LSB-02 | Done (T1) | ⚠️ Verified with spec-precision gap (unit-level mechanism proof only, no dedicated LSP-level integration test) |
| LSB-03 | Done (T3) | ✅ Verified |
| LSB-04 | Done (T5) | ✅ Verified |
| LSB-05 | Done (T5) | ✅ Verified |
| LSB-06 | Pending → T6 | ✅ Verified |
| LSB-07 | Pending → T7/T8 | ✅ Verified |
| LSB-08 | Pending → T7/T8 | ✅ Verified |
| LSB-09 | Pending → T7/T8 | ✅ Verified |
| LSB-10 | Pending → T7/T8 | ✅ Verified |
| LSB-11 | Pending → T9 | ✅ Verified |
| LSB-12 | Pending → T9 | ✅ Verified |
| LSB-13 | Pending → T9 | ✅ Verified |

---

## Summary

**Overall**: ✅ Ready (with one documented, low-risk spec-precision gap)

**Spec-anchored check**: 12/13 ACs matched spec outcome with direct
evidence; 1 spec-precision gap flagged (LSB-02 — mechanism proven at unit
level, no dedicated LSP-level integration test for `use`-loaded extern
goto)
**Sensor**: 4/4 mutations killed
**Gate**: 1050 passed, 0 failed

**What works**: All four bugs (BUG-1 through BUG-4) are fixed and covered
by tests that assert spec-defined exact outcomes (byte offsets, byte
lengths, exact label presence/absence) rather than vague "something
happened" assertions. The T8 mandatory discrimination check plus this
Verifier pass's 3 additional mutations confirm the tests actually detect
regressions, not just that they pass once.

**Issues found**:
1. LSB-02 spec-precision gap — the underlying mechanism (`item_files`
   tagging for `use`-loaded items) is solidly unit-tested
   (`test_use_loaded_item_maps_to_real_on_disk_path`), and shares code
   with LSB-01's fully-covered goto path (`cross_file_location`'s
   `resolution.file` branch is generic, not prelude-specific), so actual
   risk is low. But no test drives an end-to-end LSP goto request on a
   `use spice::...`-loaded extern name specifically. **Not treated as a
   blocking gap** given the shared-code argument and that a fix would
   only add test-surface, not change behavior — recommend a low-cost
   follow-up test (not blocking this feature's close-out).

**Next steps**: None required to close out this feature. If desired as a
low-cost follow-up: add one `Connection::memory()` integration test
driving goto-definition on a `use`-imported extern declaration (mirrors
`goto_definition_on_ddt_lands_on_operators_header`'s shape, substituting
a scratch-project `use`-loaded header for the embedded prelude one).
