# ROADMAP.md — Open work items

Distilled from the closed-out `SPEC_BENCH_GAPS.md` / `IDE_GAPS.md` handoff drafts
(2026-07-04). Everything listed in those documents that got implemented is gone; this file
keeps only what is still open. Conventions: fail-loud until closed — an unimplemented bench
task is an elaboration error (`bench_task_implemented` allowlist in
`piperine-lang/src/eval/tasks.rs`), never a silent no-op. Closing an item updates the bench
spec §11 row (`crates/piperine-bench/docs/SPEC.md`) in the same change.

---

## Bench

### `$plot(waveform, title)` (was G1)

**Spec:** bench spec §8 table row, §11 — "emit artifacts".
**Today:** elaboration-rejected (not in `bench_task_implemented`). `$write` (CSV) is the
reference `SimTask` to copy.

Sketch:
1. Artifact format: hand-rolled SVG line chart (~100 lines, zero deps, viewable anywhere).
   Axis autoscale from `Waveform.points`, polyline, title text.
2. New `Plot` struct in `piperine-bench/src/tasks.rs` implementing `SimTask`; accepts
   `(Value::Object(Waveform | ComplexWaveform), Value::Str(title))`; downcast via
   `Object::as_any` exactly like `$noise` does for `NetRef`.
3. Output path: `<title>.svg` in the CWD (same convention as `$write`); sanitize the title
   into a filename.
4. Add `"plot"` to `bench_task_implemented`; flip the spec §11 row; e2e test in
   `piperine-bench/tests/bench.rs` asserting the file exists and starts with `<svg`.

### The uniform API (was G12) — milestone

Bench spec §8 in full: public `load()` + `Design::op/tran/ac/noise` Rust surface first
(`SimSession` renamed/made public with typed config structs), Python via `pyo3` only after
the Rust surface settles. The §8 identical-shape rule is the review gate for every signature.

### `extract` / `.attach` / `.meta` (was G13)

Blocked on writing the extensibility spec (plugin model). Do not implement ahead of it; the
only prep is keeping `Attribute` surfaces public on POM nodes (they are).

---

## Codegen / solver

- `transition`, `laplace_*`, `zi_*` analog operators — recognized in the IR, fail loud at
  codegen. Each is its own companion-model follow-up.
- `ac_stim` in *potential* contributions, and multiple `ac_stim` per contribution — fail loud.
- `idt` AC small-signal `1/jω` admittance not stamped (contributes 0 in AC).
- `Trace.i` over time on devices with runtime state/vars — fails loud (per-step var/state
  banks are not recorded in `TransientAnalysisResult`).

## Language server

- True scope-aware name resolution: `symbol_index::resolve_at` is still a global first-match
  lookup; hovering `p` in module `B` can show module `A`'s port. Needs the elaborator's
  name→id maps exposed as a query.
- References/rename/highlight are word-occurrence scans gated by `resolve_at`, not
  resolver-driven use-site lists; comments/strings containing the word match.
- Project-unit elaboration: documents elaborate per-file with a project `SourceMap`;
  cross-file goto/rename and per-file diagnostic fan-out need
  `ServerState.projects: HashMap<Root, ProjectState>`.
- Protocol-level tests: drive the server over real JSON-RPC via
  `lsp_server::Connection::memory()` (init → didOpen → hover/completion round-trips);
  today's tests exercise helpers only.
- Error-accumulating elaboration (first `ElabError` stops analysis) — the editor shows one
  elaboration error at a time.

## Extension / packaging (user-owned, deliberately out of agent scope)

VS Code extension productization, marketplace packaging, grammar/registry sync tests,
release/versioning story — see `editors/vscode/`.
