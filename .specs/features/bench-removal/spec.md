# bench-removal Specification

Remove the in-language PHDL bench entirely — grammar, interpreter, crate —
and consolidate the external Rust host interface in the **root `piperine`
crate** (lib+bin). Python becomes the sole scripting host; the Python library
is sanitized and documented.

**Sequencing:** executes AFTER `solver-live-params` lands (both touch
`piperine-python`; LiveSession must exist before bench-based flows die).

## Problem Statement

Two host surfaces (PHDL bench + Python) duplicate effort; Python covers
everything and more (user decision 2026-07-16/17). The bench keyword,
interpreter (`eval/`), task allowlist, `piperine-bench` crate, and plugin
bench-task extension point are dead weight. Meanwhile the project has no
single Rust library face — host plumbing hides inside `piperine-bench`.

## Goals

- [ ] `bench` gone from the language (total removal — plain syntax error).
- [ ] `piperine-bench` crate deleted; session/results/waveform plumbing lives
      in the root `piperine` crate as the public Rust library (lib+bin), the
      complete external view of the project.
- [ ] `piperine test` runs project Python testbenches: `*_tb.py`.
- [ ] Examples: `.phdl` are circuit modules only; `.py` twins are the
      runnable part.
- [ ] Python lib sanitized (consistent naming, no bench-era leakage) and
      documented (docstrings + user-facing docs).
- [ ] Everything that guarded correctness keeps guarding: ngspice harness,
      spice smoke, compile-once proof, run_examples — migrated, green.

## Out of Scope

| Feature | Reason |
|---------|--------|
| New Python capabilities (interactive tran stepping, plotting) | Future real-time feature |
| Removing `piperine run foo.py` / REPL | Stays — it is the host |
| Wave A/B/C device work | Separate program |
| Publishing to PyPI / packaging polish | Later |

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| Plumbing destination | Root `piperine` crate becomes **lib-only**; `src/lib.rs` = public host API (session, results, waveform + re-exports of lang/codegen/solver public faces). The `piperine` **binary target moves to `piperine-cli`** (`[[bin]] name = "piperine"`) — root(bin)→cli→python→root(lib) was a cargo package cycle, empirically confirmed | User choice ("interface externa no root"), topology amended 2026-07-17 (user chose option B over host-crate/merge alternatives) | y (user) |
| `piperine test` | Discovers and runs `*_tb.py` via embedded CPython | User choice ("_tb no final") | y (user) |
| Examples | Strip bench blocks; keep module-only `.phdl` + `.py` twin | User choice | y (user) |
| Grammar | Total removal (generic syntax error, no friendly migration error) | User choice | y (user) |
| `piperine-python` dependency | Depends on root `piperine` lib (no cycle: root→cli never returns); keep the lib target lean (feature-gate cli-adjacent weight if needed) | Single library face | n (agent) |
| Plugin bench-task extension point | Removed with the bench (plugin SDK loses `BenchTask` contribution surface); CLI-script tier unaffected. **Lifecycle hooks (`transform_design`/`before_lower`/`after_solve`) are preserved**: they move to a root `SimHooks` trait fired by the root `SimSession`; PluginHost implements it (user decision 2026-07-17, supersedes the design's "plugins.rs deleted" row) | Bench-task dies with its only consumer; hooks are Part VI §8 capability | y (user, hooks amendment) |
| Test relocation | `piperine-bench/tests/{ngspice_validation, ngspice/, spice_smoke, spice/, compile_once_sweep, run_examples}` move to root-crate `tests/`; `bench.rs` (PHDL-bench e2e) deleted | Tests of the host API belong to the host crate | n (agent) |
| run_examples contract | Every `.phdl` elaborates + every `.py` runs green | Keeps both artifact kinds guarded | n (agent) |
| Python doc home | Docstrings on every public class/method + `docs/spec/` host-API part (or appendix) + README section; examples referenced | "documentá-la" | n (agent) |
| eval/ interpreter | `eval/` bench interpreter + `tasks.rs` allowlist removed from piperine-lang; **const/param-fold evaluation kept** (elaboration needs it) — split verified during design | Elaboration still folds constants | n (agent) |

**Open questions:** none — all resolved or logged above.

## User Stories

### P1: The language has no bench ⭐ MVP

**Acceptance Criteria**:

1. WHEN a `.phdl` contains a `bench` block THEN parsing SHALL fail with a
   syntax error (keyword no longer exists).
2. WHEN the workspace builds THEN no bench AST node, interpreter path, task
   allowlist, or `Design::benches()` surface SHALL remain (grep-clean:
   `bench` absent from piperine-lang public API).
3. WHEN elaboration folds parameters/constants THEN behavior SHALL be
   unchanged (const-eval split from bench-eval; existing elab tests green).

**Independent Test**: fixture with bench block fails to parse; full elab
suite green.

### P1: Root crate is the Rust library face ⭐ MVP

**Acceptance Criteria**:

1. WHEN a Rust host (test, python crate) needs load→elaborate→compile→
   simulate THEN it SHALL use the root `piperine` lib (session, results,
   waveform, prelude re-exports) — `piperine-bench` no longer exists.
2. WHEN the migrated tests run (ngspice harness incl. sweeps, spice smoke,
   compile-once proof, run_examples) THEN all SHALL pass unchanged in
   assertion content.
3. WHEN `piperine-python` builds THEN it SHALL depend on the root lib, not on
   a bench crate; result shapes unchanged (PY-17).
4. WHEN the binary builds THEN CLI behavior (`check/build/run/...`) SHALL be
   unchanged except `test`.

**Independent Test**: workspace green with `crates/piperine-bench` deleted;
`cargo tree -i piperine-bench` empty.

### P1: `piperine test` runs Python testbenches ⭐ MVP

**Acceptance Criteria**:

1. WHEN `piperine test` runs in a project THEN it SHALL discover `*_tb.py`
   (project root + `tests/`), run each via embedded CPython, and report
   pass/fail per file with nonzero exit on any failure.
2. WHEN no `*_tb.py` exists THEN it SHALL say so and exit 0.
3. WHEN a testbench raises THEN the traceback SHALL be shown and the run
   marked failed.

**Independent Test**: scratch project with passing + failing `_tb.py`; exit
codes and report verified.

### P1: Examples are modules + Python twins ⭐ MVP

**Acceptance Criteria**:

1. WHEN examples are inspected THEN no `.phdl` SHALL contain a bench block;
   each keeps its circuit modules and its runnable `.py` twin.
2. WHEN run_examples runs THEN every `.phdl` SHALL elaborate and every `.py`
   SHALL run green.

### P2: Python lib sanitized + documented

**Acceptance Criteria**:

1. WHEN the public Python surface is reviewed THEN naming SHALL be consistent
   (no bench-era terms like "bench"/"stage" leaking), dead paths removed,
   and every public class/method SHALL carry a docstring (checked by a test
   walking the facade).
2. WHEN a user reads the docs THEN a Python host-API document SHALL cover:
   load/Design/Module, analyses + results, LiveSession (set/schedule_set/
   rebuilds), `piperine run`/`-i`/`test`, with runnable snippets.
3. WHEN typing stubs / facade types are checked THEN they SHALL match the
   implemented surface (autocomplete parity preserved).

## Edge Cases

- WHEN a project's `Piperine.toml` or plugin manifest references bench tasks
  THEN loading SHALL fail loud with a clear "bench removed" error (manifest
  schema updated).
- WHEN `piperine test` finds a `_tb.py` that hangs THEN a timeout SHALL kill
  it and mark it failed (bounded).
- WHEN headers/spice smoke fixtures used bench blocks THEN they SHALL be
  ported to the Rust host API / Python without losing coverage.

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
|---|---|---|---|
| BRM-01 | P1 grammar removal | T6 | Verified |
| BRM-02 | P1 no bench surface left | T6 | Verified |
| BRM-03 | P1 const-eval preserved | T6 | Verified |
| BRM-04 | P1 root lib face | T1 | Verified |
| BRM-05 | P1 tests migrated green | T2 | Verified |
| BRM-06 | P1 python on root lib | T3 | Verified |
| BRM-07 | P1 CLI unchanged but test | T4+T5(deviation) | Verified |
| BRM-08 | P1 `*_tb.py` discovery/run | T4 | Verified |
| BRM-09 | P1 empty/failing tb semantics | T4 | Verified |
| BRM-10 | P1 examples stripped | T6 | Verified |
| BRM-11 | P1 run_examples dual contract | T2+T7 | Verified |
| BRM-12 | P2 sanitized naming + docstrings | Design | Pending |
| BRM-13 | P2 host-API docs | Design | Pending |
| BRM-14 | P2 stub/facade parity | Design | Pending |
| BRM-15 | P1 plugin bench-task removal | T5 | Verified |

**Coverage:** 15 total, 12 verified (T1–T7), 3 pending (BRM-12/13/14 → T8/T9)

## Success Criteria

- [ ] `cargo tree -i piperine-bench` → crate gone; workspace green, zero warnings.
- [ ] ngspice harness + smoke + compile-once + examples all green post-move.
- [ ] `piperine test` runs `*_tb.py` in a scratch project.
- [ ] Python API fully docstringed + host-API doc published in-repo.
