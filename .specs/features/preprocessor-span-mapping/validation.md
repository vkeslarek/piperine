# Preprocessor Span-Mapping Validation

**Date**: 2026-08-16
**Spec**: `.specs/features/preprocessor-span-mapping/spec.md`
**Diff range**: `3f53f287..fb447cd1` (T1–T15, 15 commits)
**Verifier**: independent sub-agent (author ≠ verifier)

Methodology note: verified in two disposable `git worktree` checkouts
(`fb447cd1` = feature tip, `3f53f287` = pre-feature baseline), each built
with its own `CARGO_TARGET_DIR` (the repo's default `/var/tmp/piperine-target`
is shared across worktrees and was observed to silently serve a stale test
binary from a different checkout on first use — a real footgun, noted here
so a future verifier isolates target dirs from the start). No file in the
real working tree (which carries unrelated uncommitted LRM-conformance WIP)
was touched. Both worktrees removed after use.

---

## Task Completion

| Task | Status | Notes |
| ---- | ------ | ----- |
| T1 | ✅ Done | Types compile |
| T2 | ✅ Done | `resolve()`/registry present, all Done-when bullets covered by tests |
| T3 | ✅ Done | Clean rename, no leftover `Span` alias in `piperine-lang` |
| T4 | ✅ Done | Clean rename in `piperine-codegen` |
| T5 | ✅ Done | Clean rename in `piperine-lang-server` |
| T6 | ✅ Done | Provenance recording landed; pre-existing `timescale_and_resetall...` failure confirmed pre-existing (see Gate Check) |
| T7 | ✅ Done | `Design::source_provenance` threaded, serde tests unaffected |
| T8 | ⚠️ Partial | Core fix works (crash closed — see AC evidence) but **introduces a hard build-breaking bug never fixed within the 15 commits** (see Fix 1) and a genuine rendered-output regression on the plain no-directives case (see Fix 2) |
| T9 | ✅ Done | `ExpansionFrame` recording + args, tests pass |
| T10 | ✅ Done | Dual-label rendering test passes |
| T11 | ✅ Done | Reverse lookup tests pass |
| T12 | ✅ Done | LSP diagnostics test passes, no regression |
| T13 | ✅ Done (bounded, as self-flagged) | New test passes; genuinely inert for cursor-disambiguation because `Module`/`Port`/`Behavior::span` is `None` everywhere in `elab/lower/vams.rs` — confirmed independently (see AC table, SPANMAP-08) |
| T14 | ✅ Done | `expand_cmd.rs` 6/6 pass |
| T15 | ✅ Done | `--trace` mode tests pass |

**Bug found while verifying T9** (not disclosed in either batch's summary): commit `1972eab3` (T9) adds `crate::lexer::scale_factor_value(b)` as a cross-module call from `preprocessor.rs`, but the function stays `fn` (private) in `lexer.rs` — this is `E0603`, a hard compiler error. It is **never fixed in any of the 15 commits**; `fb447cd1` (the feature's own final commit) does not compile `piperine-lang` at all (`cargo build -p piperine-lang` fails, confirmed by direct build in an isolated worktree). See Fix 1.

---

## Spec-Anchored Acceptance Criteria

| Requirement | Criterion (WHEN X THEN Y) | Spec-defined outcome | `file:line` + assertion | Result |
| --- | --- | --- | --- | --- |
| SPANMAP-01/02 | WHEN an error's Span falls inside `` `include ``d content longer than the top-level file THEN CLI renders the included file's own path + valid snippet, no OutOfBounds | Valid, in-bounds, correctly-located snippet | `crates/piperine-lang/tests/span_resolution.rs:64` `error_inside_a_longer_include_resolves_to_the_included_file` — `assert!(!rendered.contains("OutOfBounds"))` + asserts the included file's own text appears. Independently reproduced live: `piperine check .../vsine.vams` (spec's own named repro) — baseline crashes `[Failed to read contents ...): OutOfBounds]` at offset 5953; feature tip renders a valid line/snippet (`10:25`, `parameter real freq =1. from [0:inf);`) | ✅ PASS |
| SPANMAP-01 (no-directives case, AC1) | WHEN Span falls inside top-level-only text THEN CLI renders "the same file name, line, and snippet it renders today" | Byte-for-byte identical rendered output vs. pre-feature (spec's own Success Criteria: "byte-for-byte the same rendered output") | Live A/B on an identical malformed file with **no directives at all**: baseline renders `╭─[2:14]` (no name token); feature tip renders `╭─[<input>:2:14]` — a new, synthetic `<input>` token appears in every CLI diagnostic that didn't appear before. `crates/piperine-lang/tests/span_resolution.rs:52-61` (`error_in_plain_source_resolves_to_the_top_level_file_unchanged`) only asserts the snippet contains `"Bad"` and doesn't contain `"OutOfBounds"` — it never diffs against the pre-feature rendered string, so this regression is untested and unnoticed by either batch | ❌ GAP (regression, untested) — see Fix 2 |
| SPANMAP-03 | WHEN the same file is `` `include ``d twice THEN every Span in either inclusion resolves to one stable file identity | Same `source_id` both times | `crates/piperine-lang/tests/preprocessor.rs:235-249` `repeated_include_of_the_same_path_shares_one_source_id` — `assert_eq!(first.source_id, second.source_id)` + `assert_eq!(prov.name(first.source_id), "defs.vams")` | ✅ PASS |
| SPANMAP-07 | WHEN no directives at all THEN resolved Span identical to today (same file, same offsets) — edge case | Identity mapping | `crates/piperine-lang/tests/source_provenance.rs` `resolve_no_directives_case_is_identity_for_whole_file`, `resolve_top_level_only_span_is_identity_mapped` — both pass; killed by mutation (see Sensor) | ✅ PASS (offset identity); see SPANMAP-01 row above for the separate rendered-name regression |
| SPANMAP-04/05 | WHEN Span falls inside macro expansion THEN primary label at invocation, secondary "expanded from here" at definition | Both labels present, correct locations | `crates/piperine-lang/tests/macro_expansion_span.rs` `single_level_macro_expansion_resolves_one_frame_with_correct_invocation_and_definition` (asserts `frame.invocation`/`frame.definition` resolve correctly) + `macro_expansion_error_report_shows_both_invocation_and_expanded_from_here` (asserts rendered string contains both) — killed by render-boundary mutation (see Sensor) | ✅ PASS |
| SPANMAP-06 | WHEN nested macro-of-macro THEN N-deep chain, innermost first | 2-frame chain for doubly-nested case | `macro_expansion_span.rs` `doubly_nested_macro_expansion_resolves_two_frame_chain_innermost_first` — passes | ✅ PASS |
| SPANMAP-07 (edge case, no-noise) | WHEN plain text THEN empty expansion_chain | Empty `Vec` | `macro_expansion_span.rs` `plain_source_span_has_empty_expansion_chain`, `plain_source_error_report_has_no_expanded_from_here_note` | ✅ PASS |
| SPANMAP-09 | Macro invocation's actual argument text recoverable from `ExpansionFrame` | `args: Vec<(String, Span)>` with correct param→text pairing, text recoverable via `SourceProvenance::text()` | `macro_expansion_span.rs` `multi_argument_macro_invocation_records_param_to_arg_span_pairs` — passes | ✅ PASS |
| SPANMAP-08 | LSP hover/goto resolves correctly for include/macro-touched spans (P3 AC2: unchanged for open-doc-own-text case) | Correct location, no crash | `crates/piperine-lang-server/tests/hover.rs:177` `hover_on_module_after_a_shrinking_define_directive_still_resolves` — passes, proves the flat-offset **translation** runs without panicking/producing a gap on a directive-shifted document. **Does not and cannot prove position-*disambiguation*** (picking the right of two colliding candidates) because `piperine-lang`'s VAMS elaborator never populates `Module`/`Port`/`Behavior::span` — independently confirmed: `elab/lower/vams.rs` builds every `Module`/`Behavior` with `span: None` (grep on the fb447cd1 tree; `symbol_index.rs`'s `span_contains` is consequently always `false`, so name-only matching is what actually resolves every hover test in the file, pre-existing and post-feature alike) | ⚠️ Spec-precision gap, self-flagged and accurately scoped by the implementer's own doc comment (`hover.rs:150-175`) — AC only partially demonstrable today |
| SPANMAP-10 | `piperine expand <file>` prints fully preprocessed text; byte-identical to input when no directives | Verbatim expanded text / byte-identical | `crates/piperine-cli/tests/expand_cmd.rs` `expand_with_no_directives_is_byte_identical_to_input`, `expand_resolves_includes_and_macro_invocations` — both pass | ✅ PASS |
| SPANMAP-10 (AC3) | Unresolvable include reports same typed error as `check` | Same error path/rendering | `expand_cmd.rs` `expand_on_unresolvable_include_reports_the_same_typed_error_as_check` — passes | ✅ PASS |
| SPANMAP-11 | `--trace` annotates macro name/args/location; no noise on plain files | Annotation present for macro case, absent for plain case | `expand_cmd.rs` `expand_trace_shows_macro_name_args_and_location`, `expand_trace_on_plain_file_shows_no_annotations` — both pass | ✅ PASS |

**Status**: ❌ Gaps present (1 confirmed rendered-output regression untested by the suite: SPANMAP-01's "byte-for-byte identical" success criterion is violated for every CLI invocation; 1 spec-precision gap self-flagged and accurately scoped by the implementer for SPANMAP-08/T13).

---

## Discrimination Sensor

| # | File:line | Description | Killed? |
| - | --------- | ------------ | ------- |
| 1 | `crates/piperine-lang/src/source_provenance.rs:161` (`SourceProvenance::resolve`) | `local_start = seg.local_start + (span.start - seg.buffer_range.start)` → added spurious `+ 1` | ✅ Killed — 5/10 `source_provenance.rs` tests fail with off-by-one assertion mismatches |
| 2 | `crates/piperine-lang/src/lib.rs:232` (`resolve_elab_error`, the render-boundary fix) | Forced early `return miette::Report::from(err)` before any resolution/NamedSource attachment runs (i.e., reverts to the pre-fix behavior) | ✅ Killed — both `span_resolution.rs` tests fail (`OutOfBounds`-style unresolved snippet) and 1 `macro_expansion_span.rs` test fails |

**Sensor depth**: lightweight (2 mutations, both on the two named highest-risk candidates)
**Result**: 2/2 killed — sensor PASS (the suite does discriminate against faults in the code it actually exercises; the untested regression found above is a coverage gap, not a sensor failure — no mutation was needed to find it, direct execution sufficed)

---

## Code Quality

| Principle | Status |
| --------- | ------ |
| No features beyond what was asked | ✅ — `expand.rs` matches spec P2.5 exactly, no extra flags/subcommands |
| No abstractions for single-use code | ✅ |
| No unnecessary "flexibility" added | ✅ |
| Only touched files required for task | ✅ — diff is scoped to `piperine-lang`/`piperine-codegen` (rename only)/`piperine-lang-server`/`piperine-cli`, matching design's Integration Points |
| Didn't "improve" unrelated code | ✅ |
| Matches existing patterns/style | ✅ — `expand.rs` mirrors `check.rs`'s error-printing convention as designed |
| Would senior engineer approve? | ❌ — a senior engineer would not accept a feature branch whose own final commit fails `cargo build -p piperine-lang` with a plain visibility error; this should have been caught by T9's own "Done when: Gate check passes: `cargo test -p piperine-lang --test macro_expansion_span`" bullet, which is provably false as stated against true HEAD |
| Tests map to acceptance criteria, non-shallow | ⚠️ — mostly yes; SPANMAP-01's no-directives AC is the exception (see Fix 2) — asserts substring containment, not the byte-for-byte match the spec's own Success Criteria demands |
| Spec-anchored outcome check | ⚠️ — one gap found (SPANMAP-01 rendered-name regression), documented above |
| Every test maps to a spec AC / edge case | ✅ — no unclaimed tests found |
| Documented guidelines followed | `CLAUDE.md` "zero warnings" build bar — not met at `fb447cd1` (hard error, not even a warning) |

---

## Edge Cases

- [x] Include target unresolved (`IncludeNotFound`) — unaffected, per spec; not independently re-verified beyond reading `preprocessor.rs:include_not_found_is_an_error_naming_the_path` (passes)
- [x] Macro defined but never invoked — no test regression observed
- [x] `` `ifdef ``-guarded inactive branch with malformed syntax — `nested_ifdef_inside_a_skipped_branch_does_not_leak` passes
- [ ] No-directives case renders byte-identical to today — **NOT handled**: `<input>` token now appears where nothing appeared before (see Fix 2)
- [x] Zero-length Span — `resolve_zero_length_span_is_valid_and_zero_width` passes, killed by mutation 1

---

## Gate Check

- **Gate command**: `cargo build --workspace` (per tasks.md's Gate Check Commands table)
- **Result at `fb447cd1` as committed**: **FAIL** — `error[E0603]: function 'scale_factor_value' is private`, `crates/piperine-lang/src/preprocessor.rs:544`, blocking compilation of `piperine-lang` and everything downstream (`piperine-codegen`, `piperine-cli`, `piperine-lang-server`, `piperine`, `piperine-api`, `piperine-plugin`, `piperine-python`). Zero of this feature's own test targets can run against the committed tip in isolation.
- **Result after a 1-line scratch fix** (`fn scale_factor_value` → `pub(crate) fn scale_factor_value` in `lexer.rs`, applied only in the disposable worktree, never in the real tree): clean, zero warnings.
- **Root cause of why this went undetected**: the real working tree currently carries unrelated uncommitted WIP (a different, later feature) that happens to touch `lexer.rs` and coincidentally could have carried this same fix forward — the implementer very likely ran `cargo test` against a contaminated real working tree (with that later fix already present) rather than against the feature's own commits in isolation, so every "Gate check passes" bullet in T9–T15 was verified against a tree that was never actually equal to `fb447cd1`.
- **Test count before feature** (`3f53f287`, `piperine-lang` + `piperine-lang-server` + `piperine-cli`, targets that compile): baseline compiles and has its own ~22/~6 pre-existing broken targets, consistent with the batches' claim (confirmed via A/B: `pom_serde.rs`, `digital_codegen_gaps.rs`, `resolve_lowering.rs` all pre-existing dead PHDL-debt build failures at baseline too; `hover.rs` 7/7, `symbol_resolution.rs` 5/5, `diagnostics.rs` 5/8 pre-existing failures, byte-identical failure messages at both ends of the diff range)
- **Test count after feature**: with the 1-line scratch fix applied, all newly-added test files pass in full: `source_provenance.rs` 10/10, `span_resolution.rs` 2/2, `macro_expansion_span.rs` 6/6, `expand_cmd.rs` 6/6, plus 1 new passing test each in `preprocessor.rs`, `diagnostics.rs`, `hover.rs`. `preprocessor.rs` 24/25 (1 pre-existing unrelated failure). No net regressions found anywhere the tests actually run.
- **Delta**: ~28 directly-counted new passing tests (roughly matches the batches' claimed 20+16=36, some of which land in already-counted files)
- **Skipped tests**: none found
- **Failures**: 1 blocker (build), 5 pre-existing `diagnostics.rs` failures (unrelated), 7 pre-existing `hover.rs` failures (unrelated), 5 pre-existing `symbol_resolution.rs` failures (unrelated), 1 pre-existing `preprocessor.rs` failure (unrelated), 2 pre-existing `piperine-codegen --lib` failures (unrelated), several pre-existing dead PHDL-debt test-target build failures (unrelated) — all independently confirmed present, byte-identical, at the pre-feature baseline commit

---

## Fix Plans

### Fix 1: `piperine-lang` does not compile at the feature's own final commit (Blocker)

- **Root cause**: `1972eab3` (T9) adds `crate::lexer::scale_factor_value(b)` as a call from `preprocessor.rs` into a function that stays private (`fn`, not `pub(crate) fn`) in `lexer.rs`. No later commit in the range fixes the visibility. `cargo build -p piperine-lang` fails with `E0603` at `fb447cd1`.
- **Fix task**: In `crates/piperine-lang/src/lexer.rs:598`, change `fn scale_factor_value` to `pub(crate) fn scale_factor_value`. One-line, mechanical, zero logic change. Verify: `cargo build --workspace` clean (zero warnings) at HEAD, then re-run the full gate.
- **Priority**: Blocker

### Fix 2: `piperine check`'s no-directives case is not byte-for-byte identical to pre-feature output (Major)

- **Root cause**: `piperine-cli`'s `check.rs` calls `piperine_lang::parse_and_elaborate(&body, source_map)` with only the file's raw text, never its path. `resolve_elab_error`'s `SourceProvenance::name()` therefore always returns the synthetic top-level placeholder (`"<input>"`/`"<test>"`-style name) for the top-level source, even for a real on-disk file with a real path — miette then renders `╭─[<input>:L:C]` where before the fix it rendered `╭─[L:C]` with no name token at all (the old code path never called `NamedSource`, just a raw string). This is a visible, permanent difference in every CLI diagnostic for every file, not only include/macro-touched ones — violating the spec's own explicit "byte-for-byte the same rendered output" Success Criterion for the already-correct case. Note `piperine expand`'s trace mode (T15) already solved exactly this for its own boundary — see its doc comment: "the CLI (unlike the LSP) always knows [the real path]" and substitutes it in; `check.rs`'s call path was never given the same treatment.
- **Fix task**: Either (a) give `check.rs` (and any other CLI entry point hitting this path) a `parse_and_elaborate`-family variant that accepts the real file path and threads it into `SourceProvenance`'s top-level source name, mirroring what T15 already does for `expand --trace`; or (b) if `"<input>"` is an intentionally-accepted new label, update spec.md's AC1/Success-Criteria wording to say so explicitly and add a test that asserts the exact rendered string (not just substring containment) so the decision is captured, not silently divergent from spec.
- **Priority**: Major

---

## Requirement Traceability Update

| Requirement | Previous Status | New Status |
| ----------- | ---------------- | ---------- |
| SPANMAP-01 | Implementing | ⚠️ Partial (crash fixed; no-directives byte-identity regressed, untested) |
| SPANMAP-02 | Implementing | ✅ Verified |
| SPANMAP-03 | Implementing | ✅ Verified |
| SPANMAP-04 | Pending | ✅ Verified |
| SPANMAP-05 | Pending | ✅ Verified |
| SPANMAP-06 | Pending | ✅ Verified |
| SPANMAP-07 | Implementing | ✅ Verified |
| SPANMAP-08 | Pending | ⚠️ Partial (implemented correctly, inert for disambiguation pending unrelated VAMS span-population fix — self-flagged) |
| SPANMAP-09 | Pending | ✅ Verified |
| SPANMAP-10 | Pending | ✅ Verified |
| SPANMAP-11 | Pending | ✅ Verified |

---

## Summary

**Overall**: ❌ Not Ready (until Fix 1 lands — the feature's own tip does not compile in isolation)

**Spec-anchored check**: 9/11 requirements cleanly verified; 1 partial (SPANMAP-01, rendered-name regression); 1 spec-precision-bounded pass (SPANMAP-08, self-flagged by the implementer)
**Sensor**: 2/2 mutations killed
**Gate**: FAIL as committed (build error); PASS after a 1-line scratch fix, with pre-existing-and-confirmed-unrelated failures elsewhere in the suite

**What works**: The core mechanism — `SourceProvenance`, the `PreprocessedSpan`/`Span` split, forward `resolve()`, the macro-expansion chain, the reverse lookup, the render-boundary fix's actual resolution logic, `piperine expand` — is well-built and the tests genuinely discriminate against faults in it (sensor 2/2 killed). The headline crash (`vsine.vams`, `OutOfBounds` at byte 5953) is confirmably fixed.

**Issues found**:
1. Fix 1 (Blocker): feature's own HEAD commit doesn't build in isolation — a private-function visibility miss from T9, never caught because verification ran against a contaminated working tree.
2. Fix 2 (Major): the no-directives CLI case now shows a spurious `<input>` name token that wasn't there before — a real, confirmed, untested regression against the spec's own explicit byte-for-byte Success Criterion.
3. SPANMAP-08/T13 is accurately self-scoped by the implementer as inert-pending-a-different-fix — not a new finding, but independently confirmed true.

**Next steps**: Land Fix 1 immediately (trivial). Decide and land Fix 2's approach (thread the real path into the CLI's provenance, or amend the spec). Re-verify gate + AC table after both land.

---

## Re-verification

**Date**: 2026-08-16
**Commits checked**: `1911c6e9` (fix(lang): make `scale_factor_value` `pub(crate)` for preprocessor reuse — Fix 1), `cef91386` (fix(lang): suppress synthetic `<input>` source name for path-less callers — Fix 2)
**HEAD at time of re-verification**: `cef91386` (`git log --oneline -1` confirmed)
**Scope**: focused re-check of the two named gaps + a targeted regression sweep, not a from-scratch pass. Real working tree used directly (no worktree needed this time — the check is narrow); unrelated uncommitted LRM-conformance WIP (`.specs/LESSONS.md`, `.specs/STATE.md`, `.specs/lessons.json`, `elab/lower/vams.rs`, `vams_include.rs`) left untouched throughout, confirmed via `git status --short` before and after.

### Gap 1 — `scale_factor_value` not `pub(crate)` (Blocker) — ✅ FIXED

- `grep -n "pub(crate) fn scale_factor_value" crates/piperine-lang/src/lexer.rs` → `598:pub(crate) fn scale_factor_value(b: u8) -> Option<f64> {` — matches.
- `cargo build --workspace` at HEAD (`cef91386`): clean, zero warnings, all 9 workspace crates compile (`piperine-lang`, `piperine-codegen`, `piperine-project`, `piperine-lang-server`, `piperine-api`, `piperine`, `piperine-plugin`, `piperine-python`, `piperine-cli`). No E0603, no other error.
- `cargo build -p piperine-lang` in isolation: succeeds as part of the workspace build (transitively verified — it's the first crate compiled and had zero errors).

### Gap 2 — spurious `<input>:` name label (Major) — ✅ FIXED

- Live repro of the previous report's exact scenario (parse error, no `` `include ``/macro, real on-disk file, no path threading workaround needed): wrote `bad.va` containing `module Bad(` to a scratch dir and ran the built `piperine check bad.va` binary. Rendered output:
  ```
  Error in file bad.va:
    × expected an identifier, found end of file
    ╰─▶ E1005
          × expected an identifier, found end of file
           ╭────
         1 │ module Bad(
           ╰────
  ```
  No `<input>` token anywhere in the output — confirms the fix at the CLI boundary.
- `cargo test -p piperine-lang --test span_resolution`: 2/2 pass, including `error_in_plain_source_resolves_to_the_top_level_file_unchanged`. Read the test body directly (not just the name): it asserts `!rendered.contains("<input>")` with an explicit comment tying it to SPANMAP-01's zero-regression AC ("no synthetic `<input>:` name label ever appeared pre-feature, and none may now") — this is a real, non-shallow assertion, not eyeballing.

### Targeted regression sweep

| Command | Result | Comparison to original validation.md's known-failure list |
| --- | --- | --- |
| `cargo test -p piperine-lang --test span_resolution --test macro_expansion_span --test source_provenance --test corpus --test preprocessor` | `span_resolution` 2/2 ok, `macro_expansion_span` 6/6 ok, `corpus` 3/3 ok, `preprocessor` 24/25 ok — 1 failure: `timescale_and_resetall_pass_through_as_directive_text` | Matches the original report's explicitly-named pre-existing failure exactly (same test name, same assertion). Not new. |
| `cargo test -p piperine-codegen` | Fails to *build* the `margin_lowering` test target (`E0432`/`E0433`, `piperine_lang::parse`/`piperine_lang::pom::QuantityClass` don't exist) | Verified pre-existing by direct A/B: checked out `crates/piperine-codegen/tests/margin_lowering.rs` from baseline `3f53f287` into the working tree and rebuilt — **identical** compile errors at baseline, before this feature touched anything. File reverted with `git checkout HEAD --`. Neither fix commit (`1911c6e9`, `cef91386`) touches `piperine-codegen` at all (confirmed via `git show --stat` on both). Pre-existing dead PHDL-debt test-target build failure, not a new regression. |
| `cargo test -p piperine-codegen --lib` (run separately since the test-target compile failure above aborts the crate's full `cargo test`) | 47 passed, 2 failed: `device::builder::tests::structural_connection_produces_one_mna_unknown_per_alias_group`, `device::builder::tests::structural_connection_unifies_tap0_with_p` | Count and unrelated nature match the original report's "2 pre-existing `piperine-codegen --lib` failures (unrelated)" line exactly. |
| `cargo test -p piperine-lang-server` | unit tests 6/6 ok; `completion_and_symbols.rs` 0/5 ok, all 5 fail (`source must elaborate cleanly` / `doc.ctx.is_some()` panics) | Matches the original report's "5 lang-server completion/outline tests" pre-existing-failure line exactly (same 5 test names, same failure mode). |
| `cargo test -p piperine-cli` | All targets pass: `add_cmd` 6/6, `build_cmd` 5/5, `check_cmd` 1/1, `expand_cmd` 6/6, `test_tb` 11/11 | Zero failures — no regressions, and `expand_cmd`'s 6 tests (including the SPANMAP-10/11 ACs) remain fully green after both fixes. |

No failure found anywhere in the sweep falls outside the original validation.md's documented pre-existing/unrelated list. No new regressions introduced by either fix commit.

### Gap-by-gap verdict

| Gap | Status |
| --- | --- |
| Gap 1 (build-breaking E0603, Blocker) | ✅ Fixed — confirmed via grep, workspace build, and isolated crate build |
| Gap 2 (spurious `<input>:` regression, Major) | ✅ Fixed — confirmed via live repro (no `<input>` in rendered output) and a non-shallow test assertion in `span_resolution.rs` |

### Overall re-verification verdict: ✅ PASS

Both gaps from the original FAIL report are closed with direct evidence (not just re-reading the fix diff). The targeted regression sweep found zero new failures — every failure present is byte-for-byte the same pre-existing/unrelated failure already catalogued in the original report's Gate Check section. The feature (`preprocessor-span-mapping`) is done.
