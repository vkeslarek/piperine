# P3b Blocking-Fixes — Validation Report

**Verdict: PASS ✅**

**Scope**: all 8 requirements (PB-01..08), 5 tasks (T1–T5), executed inline
(single batch, ≤8 tasks — no sub-agent delegation per the skill's threshold).
**Diff/commit range**: `2fa0e69` (T1, `piperine build`) .. `7fcfc95` (docs:
mark all requirements delivered) on `feature/bench-removal`. Commits:
`2fa0e69`, `dae7e71`, `b600700`, `7fcfc95`.

**Verifier note**: standalone fallback (implementer ran this pass itself,
per `validate.md`'s standalone-fallback mode for non-delegated Execute) —
re-checked evidence against source rather than trusting memory of having
written it, and ran fresh discrimination mutations rather than reasoning
about coverage abstractly.

---

## 1. Gate check (deterministic)

`cargo test --workspace`: 1 failing test out of the full suite —
`piperine-python::simulation_error::non_converging_run_raises_convergence_error`
— traced to a pre-existing PyO3 "unsendable, but sent to another thread"
panic triggered only under `cargo test`'s default parallel test-thread
scheduling; confirmed passing with `--test-threads=1`. This test file and
crate were not touched by any P3b commit (P3b only touched `piperine-cli`,
`piperine-codegen`, `piperine-solver`); not a regression from this feature.
Every crate this feature actually changed is fully green:
`cargo test -p piperine-cli` (16/16), `cargo test -p piperine-codegen`
(all green, digital_codegen_gaps.rs 4/4 + build_cmd.rs* 5/5 +
pre-existing suites), `cargo test -p piperine-solver --lib` (100/100).

## 2. Spec-anchored coverage check (evidence-or-zero)

| Req | AC | Evidence | Spec-defined outcome | Match? |
|-----|----|----------|----------------------|--------|
| PB-01 | `build` elaborates+compiles a valid zero-port design, exit 0 | `crates/piperine-cli/tests/build_cmd.rs:44` `valid_zero_port_design_builds_and_exits_zero` — `assert!(out.status.success())` + `text.contains("built \`Board\`")` | exit 0, per-module success line | ✅ |
| PB-02 | elaboration failure → exit non-zero, error printed | `build_cmd.rs:60` `elaboration_failure_exits_nonzero_with_error` — `assert!(!out.status.success())` + `text.contains("Elaboration failed")` | non-zero exit, error shown | ✅ |
| PB-03 | codegen failure → exit non-zero, attributed to module | `build_cmd.rs:83` `codegen_failure_exits_nonzero_attributed_to_module` — `text.contains("\`Board\` failed to build")` | attributed error | ✅ |
| PB-04 | no zero-port modules → note, exit 0 | `build_cmd.rs:98` `library_only_project_prints_note_and_exits_zero` — `assert!(out.status.success())` + `text.contains("nothing to build")` | exit 0, note printed | ✅ |
| PB-05 | digital `fn` call inlines, computes correctly | `crates/piperine-codegen/tests/digital_codegen_gaps.rs` `digital_fn_call_inlines_and_computes_nand` — full 4-row NAND truth table asserted, not just "compiles" | exact truth-table values | ✅ — spec-precision met (AC required a computed value, test asserts the full table) |
| PB-06 | enum-pattern `match` selects the matching const-evaluator variant | `digital_codegen_gaps.rs` `digital_enum_pattern_match_selects_the_right_arm` — 3 variants (Idle/Run/Done), each asserted against its literal PHDL-declared arm | correct arm per variant | ✅ |
| PB-07 | Real↔Quad coercion defined, no truncation-before-truthiness bug | `digital_codegen_gaps.rs` `digital_quad_real_coercion_both_directions` (round-trip via `coerce_pair`/`store_net`) + `digital_real_to_quad_coercion_does_not_truncate_fractional_nonzero` (`0.5`→`One`, the exact SPEC_DEVIATION case) | nonzero→1, zero→0, no truncation | ✅ — the fractional-value test specifically targets the spec's own stated precision requirement |
| PB-08 | `.tf` dead branch removed/guarded, existing suite stays green | `crates/piperine-solver/src/analyses/tf.rs` diff (branch removed, `debug_assert_eq!` added) + `cargo test -p piperine --test session_tf` `tf_current_source_input_fails_loud`/`tf_matches_the_closed_form_divider_transfer_characteristics` both green | dead code gone, no live-path regression | ✅ |

**Spec-precision gaps**: none. Every AC specified a checkable outcome and
every test asserts that exact outcome (truth tables, specific variant
mappings, specific numeric truthiness cases at the fractional boundary the
spec called out), not vague "no error thrown" assertions.

## 3. Discrimination sensor (scratch mutations, real tree untouched)

Two mutations injected, gate re-run, mutation discarded via `git checkout --`:

1. **`crates/piperine-cli/src/commands/build.rs`**: `filter(|m|
   m.ports().is_empty())` → `filter(|m| false && m.ports().is_empty())`
   (never selects a zero-port module). `cargo test -p piperine-cli --test
   build_cmd` → 3/5 tests **FAILED** (the three that expect a module to
   actually build) — **mutant killed**.
2. **`crates/piperine-codegen/src/emit/builder.rs:759`**: `FloatCC::NotEqual`
   → `FloatCC::Equal` in the `Real -> Quad` truthiness check (inverts
   nonzero/zero). `cargo test -p piperine-codegen --test
   digital_codegen_gaps` → 2/4 tests **FAILED** (both PB-07 tests) —
   **mutant killed**.

Both reverted; confirmed clean via `git status --short` (no diff) and a
green re-run of each affected test file before proceeding.

**Sensor result: 2/2 mutations killed, 0 survived.**

## 4. Process notes

- PB-05/PB-06 each carry a documented `SPEC_DEVIATION`: PB-05's fix landed
  entirely in `emit/builder.rs` (self-contained tree substitution) rather
  than reusing `resolve/pom/expr.rs`'s inliner directly, to avoid a
  cross-module dependency on the analog-lowering-only `pom` module — same
  algorithm, kept local. PB-06's fix landed in `resolve/pom/mod.rs` (a
  lowering-time pre-resolution pass) rather than `emit/stmt.rs` as
  originally assumed in `tasks.md` — discovered during implementation that
  `emit/stmt.rs`'s existing `Pattern::Literal` handling already covers the
  post-resolution case, so no emit-side change was needed. Both are
  documented in `tasks.md`'s T2/T3 status notes and in code comments.
- PB-05 also surfaced (but did not fix, correctly out of scope) a
  pre-existing name-collision bug shared with the analog fn inliner: a
  digital/analog fn's param name colliding with the calling module's own
  node name resolves to that node instead of the local param binding. Not
  a P3b target — flagged for a future task.
- PB-08's re-verification finding (the `1e20` placeholder was provably
  unreachable, not a live wrong-number bug as ROADMAP.md's gap catalog
  described) is the single most consequential finding of this feature: it
  changed the fix from "correct a wrong number" to "remove dead code and
  guard the invariant" — a materially different, smaller, and more
  accurate fix. Recorded in `spec.md`'s Verified Findings table.
- Repeated environment issue (not a code defect): the `/home` partition
  filled to 100% mid-session during `cargo test --workspace`/`cargo build`
  runs, requiring `cargo clean` (freed 53GiB) to proceed. Recorded here in
  case it needs infra attention — this is the third time this exact issue
  has recurred across recent sessions on this project.

## 5. Ranked gaps

None. All 8 requirements matched their spec-defined outcome; both
discrimination mutations were caught; every crate this feature touched is
green; the one workspace-level test failure is confirmed pre-existing,
unrelated, and thread-scheduling-flaky, not a regression.
