# Language Server Follow-up Bugs — Validation Report

**Verdict: PASS ✅**

**Scope**: all 13 requirements (LSB-01..13), 10 tasks (T1–T10), 2 sequential
batches (B1=T1-T5, B2=T6-T9) plus this closing pass done by the
orchestrator (the B2 batch worker paused mid-`cargo test --workspace` on a
session limit before writing this report; T6-T9 were already committed and
clean, so the orchestrator completed T10 as a standalone-fallback Verifier
pass).

**Diff/commit range**: `96a0b1f` (T1) .. `a2324f7` (T9, last code commit)
on `feature/bench-removal`. Commits: `96a0b1f`, `06468c5`, `4b4a5ae`,
`bab93d5`, `623c7e5`, `5d51952`, `c29fcb6`, `38617ad`, `a2324f7`.

---

## 1. Gate check (deterministic)

`cargo test --workspace`: 1 failing test —
`piperine-plugin::process_smoke::dead_guest_is_a_loud_error` — confirmed
pre-existing and unrelated (spawn-timing flake under parallel test
scheduling in a process-backend smoke test that has nothing to do with
`piperine-lang`/`piperine-lang-server`; passes clean with
`--test-threads=1`, matches the exact same flake documented in this
session's earlier `p3b-blocking-fixes` and `language-server` validation
reports). `cargo test -p piperine-lang -p piperine-lang-server`: every
target green, 0 failed (30 test-result blocks, all `0 failed`).
`cargo build --workspace`: clean, only a pre-existing unrelated
`piperine-python` build-script warning (`.so not found`).

## 2. Spec-anchored coverage check (evidence-or-zero)

| Req | AC | Evidence | Spec-defined outcome | Match? |
|---|---|---|---|---|
| LSB-01 | goto on `ddt` lands on real declaring file | `crates/piperine-lang-server/tests/integration_test.rs:666` `goto_definition_on_ddt_lands_on_operators_header` — asserts `Location.uri` resolves to the real, existing `headers/operators.phdl` and range offset equals `header_text.find("extern operator ddt")` computed against the real file | real file + real offset, not fabricated | ✅ |
| LSB-02 | `use`-imported extern resolves to its real file | `crates/piperine-lang/tests/elab.rs:480` `test_use_loaded_item_maps_to_real_on_disk_path` | real on-disk path for a `use`-loaded item | ✅ |
| LSB-03 | same-file extern decl unregressed | `integration_test.rs:712` `goto_definition_on_same_file_extern_decl_still_works` | same-file goto still works | ✅ |
| LSB-04 | documented extern hover renders doc | `integration_test.rs:425` `hover_on_documented_extern_operator_renders_doc_as_markdown` | doc rendered as Markdown | ✅ |
| LSB-05 | undocumented extern hover unchanged | `integration_test.rs:449` `hover_on_undocumented_extern_operator_is_unchanged` | no doc paragraph, unchanged | ✅ |
| LSB-06 | `ddt` shows its authored doc | `headers/operators.phdl:13-18` (`///` block directly above `extern operator ddt`) + T6's status note (elaboration/registry tests confirm the doc threads through) | real authored doc appears for the exact reported case | ✅ — verified the `///` block is immediately adjacent to the decl (no blank-line break that would drop attachment per the lexer's rule) |
| LSB-07 | label click → tight label-token range | `crates/piperine-lang-server/tests/instance_highlight.rs:29` `highlighting_labeled_instance_label_targets_only_the_label_token` — asserts exactly 3 bytes (`"src"`), not 56 | exact 3-byte range | ✅ — spec-precision met, exact byte count asserted |
| LSB-08 | type-name click → tight type-token range | `instance_highlight.rs:50` — asserts exactly 10 bytes (`"RampSource"`) | exact 10-byte range | ✅ |
| LSB-09 | unlabeled instance unregressed | `instance_highlight.rs:70` `highlighting_unlabeled_instance_still_resolves_and_is_tight_to_type_token` — asserts 10-byte range, still resolves | no regression, now also tight | ✅ |
| LSB-10 | resolve_at/ResolutionIndex byte-for-byte agreement | `instance_highlight.rs:102` `resolved_decl_span_has_a_matching_binding_in_the_resolution_index` — bypasses the `occurrences_at` fallback masking and asserts the index lookup itself succeeds | the two sides agree on the same span convention | ✅ — this is the exact consistency check design.md calls out as the discrimination-sensitive one; independently re-confirmed by this Verifier's own sensor pass (§3) |
| LSB-11 | `for` suppressed right after `}` | `crates/piperine-lang-server/tests/completion_suppression.rs:25` `completion_right_after_module_close_brace_never_offers_for_and_offers_mod` | `"for"` absent | ✅ |
| LSB-12 | top-level keywords offered in that same case | same test — asserts `"mod"` present | `"mod"` present | ✅ |
| LSB-13 | mid-behavior-body `for` unregressed | `completion_suppression.rs:62` `completion_mid_behavior_body_still_offers_for` | `"for"` still offered mid-body | ✅ |

**Spec-precision gaps**: none. Every sampled AC specifies a checkable,
exact outcome (byte counts, presence/absence of specific labels, specific
file paths) and every test asserts that exact value, not a vague
"resolves to something."

## 3. Discrimination sensor (scratch mutations by this Verifier, independent of the batch workers' own inline checks)

Two mutations injected, gate re-run, mutation discarded via `git checkout --`:

1. **`crates/piperine-lang/src/pom/design.rs::Project::item_file`**:
   replaced the real lookup with `None` unconditionally (BUG-1's core
   query). `cargo test -p piperine-lang-server --test integration_test --
   goto_definition_on_ddt` → **FAILED** (`goto on ddt must land on
   headers/operators.phdl, got file:///goto_def_test.phdl`) — **mutant
   killed**.
2. **`crates/piperine-lang-server/src/handlers/completion.rs`**: changed
   `if has_rbrace && has_mod_body_only` to `if false && has_rbrace &&
   has_mod_body_only` (disables the BUG-4 suppression entirely). `cargo
   test -p piperine-lang-server --test completion_suppression` → **2/3
   FAILED** (both tests expecting `"for"` absent) — **mutant killed**.

A third mutation (T8's own inline sensor check, per its Status note:
reverting only the `index_design` half of the label/type-span consistency
fix while keeping `symbol_index.rs`'s half) was performed by the batch
worker during T8 itself, per its task instructions, and is not repeated
here — re-confirmed instead via LSB-10's dedicated non-masked test
(`resolved_decl_span_has_a_matching_binding_in_the_resolution_index`),
which is exactly the assertion that mutation would have broken.

Both of this Verifier's own mutations reverted; confirmed clean via `git
status --short` (no diff) and a green re-run of each affected test before
proceeding.

**Sensor result: 2/2 (this pass) + 1 (T8's own, reconfirmed via a
non-masked test) = 3/3 mutations killed, 0 survived.**

## 4. Process notes

- All 4 bugs were root-caused via direct code audit + empirical repro
  tests (documented in each Status note and in `spec.md`'s Root Cause
  sections) before any fix was written — no speculative fixes.
- BUG-4's fix is an explicitly-scoped heuristic (spec.md's Out of Scope +
  Assumptions table), not a general parser fix — documented as such, not
  overclaimed.
- BUG-1's embedded-header path resolution (`CARGO_MANIFEST_DIR`-relative)
  is valid for the project's current pre-V1 dev-tool deployment shape
  (source checkout running locally); flagged in spec.md as a future
  concern if the server ever ships without its source tree, not a live
  gap.
- The `examples/06_flash_adc.phdl`/`08_johnson_noise.phdl` uncommitted
  diffs visible in `git status` throughout this session are the user's own
  exploratory edits (converting `//` to `///` to test hover themselves) —
  untouched by this feature, left as-is per instruction not to manage
  files outside this feature's stated scope.

## 5. Ranked gaps

None. All 13 requirements matched their spec-defined outcome; 3
discrimination mutations across the feature's riskiest seams (BUG-1's file
lookup, BUG-3's span-consistency convention, BUG-4's heuristic condition)
were all caught; every crate this feature touched (`piperine-lang`,
`piperine-lang-server`) is fully green; the one workspace-level failure is
confirmed pre-existing, unrelated, and thread-scheduling-flaky.
