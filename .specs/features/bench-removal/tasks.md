# bench-removal Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement with the `tlc-spec-driven` skill Execute flow. If the skill cannot
be activated, STOP. **Do not start until `solver-live-params` is DONE** (its
Verifier PASSed) — branch from that feature's final HEAD.

---

**Design**: `.specs/features/bench-removal/design.md`
**Status**: Approved — execution gated on solver-live-params

---

## Test Coverage Matrix

> Guidelines: `CLAUDE.md` (zero warnings, `--workspace`), MD-13. Baseline:
> whatever `solver-live-params` closes at (≥445; check its tasks.md closure
> figure). Python examples baseline: 22 (21 + live_optimize).

| Code Layer | Test Type | Coverage Expectation | Location | Run Command |
|---|---|---|---|---|
| root lib (session/results/waveform) | integration | migrated suites keep identical assertions; BRM-04/05 | root `tests/` | `cargo test -p piperine` |
| piperine-lang cleanup | unit/integration | bench block = parse error; all existing elab/parse suites green (const-eval intact) | `piperine-lang/tests/` | `cargo test -p piperine-lang` |
| CLI `test` runner | integration | BRM-08/09 incl. timeout + exit codes | `piperine-cli/tests/` or root `tests/` | `cargo test -p piperine-cli` |
| plugin manifest | unit | bench-task manifest → loud error | `piperine-plugin/tests/manifest.rs` | `cargo test -p piperine-plugin` |
| python facade | e2e | docstring-walk test + stub parity + 22 examples | python tests + run_examples | full |
| docs | none | build gate | — | build |

## Gate Check Commands

| Gate | Command |
|---|---|
| Quick | `cargo test -p <crate>` |
| Full | `cargo test --workspace` |
| Build | `cargo build --workspace` (zero warnings) + `cargo test --workspace` |

---

## Execution Plan

### Phase 1: Root library face (T1 → T2)
### Phase 2: Consumers retarget (T3 → T4 → T5)
### Phase 3: Language + crate removal (T6 → T7)
### Phase 4: Python sanitation + docs (T8 → T9)

Batching: batch 1 = Phases 1+2 (5 tasks), batch 2 = Phases 3+4 (4 tasks).

---

## Task Breakdown

### T1: ✅ DONE — Root lib scaffold + plumbing move (lib-only; bin → cli)
**What**: Root crate becomes **lib-only** (topology B, amended 2026-07-17):
`src/main.rs` moves to `piperine-cli` as `[[bin]] name = "piperine"`; root
deps become lang/codegen/solver (drop cli/project). Move `session/objects(→
results)/waveform/error(BenchError→Error)` from piperine-bench into root
`src/`; strip bench-task hooks from SimSession (keep param staging + all
run_*); `prelude` re-exports lang/codegen/solver public faces.
**piperine-python and piperine-cli retarget their `piperine-bench` imports
to the root lib in this same task** (a bench→root temporary re-export is
itself a cycle — bench→root→…; instead bench keeps its own code until T7
deletes it, consumers just stop using it). Record MD-19 (root-as-library-
face, lib-only + bin-in-cli) in `.specs/STATE.md` Decisions.
**Where**: `src/lib.rs`, `src/{session,results,waveform,error,prelude}.rs`,
`Cargo.toml`, `crates/piperine-cli/` (bin target + main), `.specs/STATE.md`
**Depends on**: None · **Requirement**: BRM-04
**Done when**:
- [ ] Root lib compiles; `piperine::session::SimSession` usable from a root test
- [ ] `cargo build --workspace` still emits the `piperine` binary (from piperine-cli); CLI behavior unchanged
- [ ] Workspace green
- [ ] Gate full
**Tests**: integration · **Gate**: full
**Commit**: `feat(piperine)!: root crate is the library face; binary moves to cli`

### T2: ✅ DONE — Migrate tests of record to root
**What**: Move ngspice_validation(+ngspice/), spice_smoke(+spice/ fixtures
ported off bench blocks onto session-API calls, same assertions),
compile_once_sweep, run_examples into root `tests/`; delete `bench.rs`.
**Where**: root `tests/`
**Depends on**: T1 · **Requirement**: BRM-05 + smoke-fixture edge case
**Done when**:
- [ ] All migrated suites green under `cargo test -p piperine` (ngspice live: harness circuit count unchanged, 19+ tests)
- [ ] No test fixture contains a bench block
- [ ] Gate full
**Tests**: integration · **Gate**: full
**Commit**: `test(piperine): host-api test suites live on the root crate`

### T3: ✅ DONE — Python retarget verification + cleanup
**What**: T1 already flips `piperine-python` onto the root lib (cycle-free
under topology B). This task verifies and finishes: drop every residual
piperine-bench reference from python (Cargo + imports), result shapes
untouched (PY-17), lib build lean.
**Where**: `crates/piperine-python/Cargo.toml`, `src/*.rs`
**Depends on**: T1 · **Requirement**: BRM-06
**Done when**:
- [ ] `cargo test -p piperine-python` green; 22/22 python examples pass
- [ ] `cargo tree -p piperine-python -i piperine-bench` empty
- [ ] Gate quick
**Tests**: e2e (examples) · **Gate**: quick
**Commit**: `refactor(python): host plumbing from the root piperine lib`

### T4: ✅ DONE — CLI `test` = `*_tb.py` runner
**What**: Rewrite `commands/test.rs`: discover `*_tb.py` (root + `tests/`,
skip `.venv`/`target`), run via the `piperine run` embedded-CPython path,
per-file PASS/FAIL + traceback, per-file timeout, exit 1 on failure, exit 0
with notice when none found.
**Where**: `crates/piperine-cli/src/commands/test.rs`
**Depends on**: T3 · **Requirement**: BRM-07, BRM-08, BRM-09 + timeout edge
**Done when**:
- [ ] Scratch-project test: passing/failing/hanging `_tb.py` → correct report + exit codes
- [ ] No-testbench case exits 0 with notice
- [ ] Gate quick: `cargo test -p piperine-cli`
**Tests**: integration · **Gate**: quick
**Commit**: `feat(cli): piperine test runs *_tb.py python testbenches`

### T5: ✅ DONE — Plugin bench-task surface removal
**What**: Remove `BenchTask` extension point from plugin SDK + host; manifest
declaring bench tasks → loud "bench removed; use python testbenches" error.
**Where**: `crates/piperine-plugin/`, `piperine-plugin-wasm` if touched
**Depends on**: T3 · **Requirement**: BRM-15 + manifest edge case
**Done when**:
- [ ] manifest.rs test: bench-task manifest rejected with the message
- [ ] Plugin suites green (e2e, smoke, trust, manifest)
- [ ] Gate quick: `cargo test -p piperine-plugin`
**Tests**: unit+integration · **Gate**: quick
**Commit**: `feat(plugin)!: remove bench-task extension point`

### T6: ✅ DONE — Language bench removal
**What**: Remove `bench` keyword/AST/elab/`Design::benches()`/`eval/`
interpreter+Host+`tasks.rs` from piperine-lang, preserving const/param
folding (elab + codegen fn-inliner defaults). Port/delete lang tests that
used bench blocks (`bench.rs`, `spec_simulation.rs` — port assertions that
still make sense to session-API root tests).
**Where**: `crates/piperine-lang/`
**Depends on**: T2, T4, T5 · **Requirement**: BRM-01, BRM-02, BRM-03
**Done when**:
- [ ] Fixture with bench block → parse error test
- [ ] Grep-clean: no bench surface in piperine-lang public API
- [ ] All parse/elab suites green (const folding proven intact)
- [ ] Gate full
**Tests**: unit+integration · **Gate**: full
**Commit**: `feat(lang)!: remove the in-language bench`

### T7: ✅ DONE — Delete piperine-bench + strip examples
**Completed inside T6's commit** (physical coupling: the crate cannot
compile once `eval/` dies, and stripped examples cannot parse before the
grammar changes). Verified at closure: `cargo tree -i piperine-bench` →
no match; zero `bench` blocks in `examples/*.phdl`; run_examples dual
contract green; build gate zero warnings.
**What**: Delete the crate (workspace member, temporary re-exports, docs);
strip bench blocks from all `examples/*.phdl` (modules stay); run_examples
asserts every `.phdl` elaborates + every `.py` runs.
**Where**: `crates/piperine-bench/` (rm), `examples/`, root `tests/run_examples.rs`, `Cargo.toml`
**Depends on**: T6 · **Requirement**: BRM-10, BRM-11
**Done when**:
- [ ] `cargo tree -i piperine-bench` → gone; no `.phdl` example has bench
- [ ] run_examples dual contract green
- [ ] Gate build (zero warnings)
**Tests**: integration · **Gate**: build
**Commit**: `feat(piperine)!: delete piperine-bench — python is the host`

### T8: ✅ DONE — Python sanitation
**What**: Facade/pyclass sweep: consistent naming, no bench-era vocabulary,
dead paths removed; docstrings on every public class/method; python test
walks the facade asserting non-empty `__doc__` + stub/impl parity.
**Where**: `crates/piperine-python/{src,python}/`
**Depends on**: T7 · **Requirement**: BRM-12, BRM-14
**Done when**:
- [ ] Docstring-walk + parity test green; 22/22 examples green
- [ ] Gate quick
**Tests**: e2e · **Gate**: quick
**Commit**: `refactor(python): sanitized, fully docstringed public surface`

### T9: ✅ DONE — Host-API docs + closure
**What**: `docs/spec/` Python host-API part/appendix (load→Design→analyses→
results→LiveSession→CLI run/-i/test) with runnable snippets; CLAUDE.md/
README/ROADMAP updated (bench gone, root lib face); traceability → Verified.
**Where**: `docs/spec/`, `CLAUDE.md`, `README.md`, `.specs/`
**Depends on**: T8 · **Requirement**: BRM-13
**Done when**:
- [ ] Gate build: zero warnings, full workspace green, examples green
**Tests**: none (docs) · **Gate**: build
**Commit**: `docs: python host api; bench removal complete`

---

## Phase Execution Map

```
Phase 1: T1 → T2
Phase 2: T3 → T4 → T5
Phase 3: T6 → T7
Phase 4: T8 → T9
```

## Diagram-Definition Cross-Check

| Task | Depends (body) | Diagram | Status |
|---|---|---|---|
| T1 none · T2 T1 · T3 T1 · T4 T3 · T5 T3 · T6 T2,T4,T5 · T7 T6 · T8 T7 · T9 T8 | backward-only | sequential phases | ✅ all |

## Test Co-location Validation

| Task | Layer | Matrix | Task Says | Status |
|---|---|---|---|---|
| T1 root lib | integration | integration | ✅ |
| T2 suites | integration | integration | ✅ |
| T3 python | e2e | e2e | ✅ |
| T4 cli | integration | integration | ✅ |
| T5 plugin | unit+integration | unit+integration | ✅ |
| T6 lang | unit+integration | unit+integration | ✅ |
| T7 removal | integration | integration | ✅ |
| T8 python | e2e | e2e | ✅ |
| T9 docs | none | none | ✅ |
