# P3b Blocking-Fixes Tasks

**Spec**: `.specs/features/p3b-blocking-fixes/spec.md`. Design skipped (Medium
scope, no architecture decisions — each task follows an existing pattern in
the same file). 8 tasks, single batch (≤8), executed inline.

## Gate Check Commands

| Gate | Command |
|---|---|
| cli | `cargo test -p piperine-cli` |
| codegen | `cargo test -p piperine-codegen` |
| solver | `cargo test -p piperine-solver` |
| full | `cargo test --workspace` |

## Tasks

- [x] **T1 (PB-01..04)**: `piperine build` actually elaborates + compiles.
  DONE, commit `2fa0e69`. `crates/piperine-cli/src/commands/build.rs`
  rewritten: elaborates via `check`-style discovery, runs
  `lower_bodies`+`CircuitCompiler::build_circuit` on every zero-port module.
  Tests: `crates/piperine-cli/tests/build_cmd.rs` (5 tests — valid build,
  elaboration failure, codegen failure attributed, library-only note,
  explicit-file override). Gate: cli — green.
- [x] **T2 (PB-05)**: digital `fn` inlining in `emit/builder.rs::call_expr`.
  DONE, commit `dae7e71`. Tree-substitution mirroring the analog inliner,
  kept local to `emit/builder.rs` (not cross-module) — see
  `inline_user_fn`/`subst_expr`. SPEC_DEVIATION: found and documented (not
  fixed — out of scope) a pre-existing name-collision bug shared with the
  analog inliner: a fn param name colliding with the calling module's own
  node name resolves to that node, not the local param.
- [x] **T3 (PB-06)**: digital enum-pattern `match`. DONE, commit `dae7e71`.
  SPEC_DEVIATION: fix landed in `resolve/pom/mod.rs` (a lowering-time
  pre-resolution pass rewriting `Pattern::Path`→`Pattern::Literal` via the
  same `enum_values` map analog already uses), not `emit/stmt.rs` as this
  tasks.md originally assumed — `emit/stmt.rs`'s existing `Pattern::Literal`
  handling needed no change once the pattern is pre-resolved.
- [x] **T4 (PB-07)**: digital real↔Quad coercion. DONE, commit `dae7e71`.
  SPEC_DEVIATION: `Real→Quad` compares the real value's truthiness directly
  (`fcmp`) rather than the literally-specified `Real→Int→Quad` route, which
  would truncate a fractional nonzero value (e.g. `0.5`) to `0` before the
  truthiness check — contradicting the AC's own stated semantics.
  `Quad→Real` keeps the specified `Quad→Int→Real` route unchanged.
- [x] **T5 (PB-08)**: `.tf` dead-code removal/guard. DONE, commit `b600700`.
  `#![allow(dead_code)]` kept (verified still needed for the pre-existing,
  unrelated unused `TfContext` struct).
- [x] **T6**: full workspace gate + tasks.md status update. Gate: full —
  see `validation.md` for the Verifier's independent confirmation.

Sequential, one atomic commit per task.
