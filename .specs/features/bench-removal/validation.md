# bench-removal Validation

**Date**: 2026-07-17
**Spec**: `.specs/features/bench-removal/spec.md`
**Diff range**: `f408761..7e67ed1` (branch `feature/bench-removal`, 10 commits)
**Verifier**: independent sub-agent (author ≠ verifier)

---

## Task Completion

| Task | Status | Notes |
| ---- | ------ | ----- |
| T1 root lib face | ✅ Done | root is lib-only; `[[bin]] name = "piperine"` in piperine-cli |
| T2 tests migrated | ✅ Done | root `tests/` holds ngspice/spice/compile-once/run_examples/session |
| T3 python retarget | ✅ Done | `piperine = { path = "../.." }`; zero `piperine-bench` refs repo-wide |
| T4 cli `*_tb.py` runner | ✅ Done | test_tb.rs 7 tests green |
| T5 plugin bench-task removal | ✅ Done | hooks preserved via root `SimHooks` (user amendment honored) |
| T6 language bench removal | ✅ Done | grammar gone; residuals in comments + 1 error string (see gaps) |
| T7 crate deletion + examples | ✅ Done | `cargo tree -i piperine-bench` → "did not match any packages" |
| T8 python sanitation | ⚠️ Issue | facade green, but hygiene gate has a blind spot (surviving mutant M6a) |
| T9 docs + closure | ✅ Done | `docs/spec/part_viii_host_api.md` covers all required topics |

---

## Spec-Anchored Acceptance Criteria

### P1: The language has no bench

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| bench block → parse failure | plain syntax error (total removal, user choice) | `crates/piperine-lang/tests/bench_removed.rs:19-20` — `parse_str(src).expect_err("a bench block must not parse")` + non-empty diagnostic | ✅ PASS |
| no bench AST/interpreter/allowlist/`Design::benches()` | grep-clean public API | verifier grep: no `Bench` AST node, no `benches` member, `eval/tasks.rs` pure-only (`is_pure`: assert/diagnostics/display, "never an effectful task"). **Residual**: `crates/piperine-lang/src/pom/design.rs:385` error string `"bench root module `{root_module}` not found"` on pub `with_overrides_applied`, reachable via `SimSession::new` + analysis with unknown module | ⚠️ Minor leak (gap #2) |
| const-eval unchanged | elaboration folds params as before | `bench_removed.rs:26-41` — design with `{ .r = 2.0e3 }` override elaborates; full lang suites green in gate | ✅ PASS |

### P1: Root crate is the Rust library face

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| Rust host uses root lib | session/results/waveform from `piperine` | `tests/session.rs:7` — `use piperine::{NetRef, SimSession, SolverConfig}`; op/tran/staging/digital flows; `src/prelude.rs` re-exports lang/codegen/solver faces; `cargo tree -i piperine-bench` → no match | ✅ PASS |
| migrated suites pass unchanged | same assertion content | `tests/ngspice_validation.rs` (19 tests — live: 12 golden/sweep cases `PASS`, 0 `SKIP`), `tests/spice_smoke.rs:83` (`(mid - 7.5).abs() < 1e-6`), `:99-100` (RC corner), `:108` (VCVS 2 V), `tests/compile_once_sweep.rs:67-71` (`sweep_compiles == per_build`) | ✅ PASS |
| python on root lib, PY-17 shapes | dep + shapes | `crates/piperine-python/Cargo.toml` — `piperine = { path = "../.." }`; python smoke/live/facade suites + 22 examples green | ✅ PASS |
| CLI unchanged except `test` | binary from cli | `crates/piperine-cli/Cargo.toml` `[[bin]] name = "piperine"`; `cli_check.rs` green; logged deviation: `run <file>.phdl` now prints migration notice (`run.rs:43-49`, notice itself untested) | ✅ PASS (deviation logged) |

### P1: `piperine test` runs Python testbenches

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| discover root+`tests/`, per-file report, nonzero exit | exact semantics | `test_tb.rs:53-57` — discovery = `[tests/nested_tb.py, top_tb.py]`, skips `.venv`/`target`; `:65-68` — exit 0 + `1 run, 1 passed, 0 failed`; `:77-81` — exit `Some(1)` + FAIL + `1 run, 0 passed, 1 failed` | ✅ PASS |
| none found → notice + exit 0 | exact | `test_tb.rs:88-90` — success + `"No Python testbenches"` | ✅ PASS |
| raise → traceback shown, failed | exact | `test_tb.rs:77-80` — `Some(1)` + `"boom-marker"` in output | ✅ PASS |

### P1: Examples are modules + Python twins

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| no bench block in `.phdl` | zero blocks | verifier grep: no `^\s*bench\s` in `examples/*.phdl`; gallery non-empty guard `tests/run_examples.rs:24` | ✅ PASS (stale "the bench" header comments in 10 files — gap #3) |
| dual contract green | elaborate + run | `run_examples.rs:31-40` (every `.phdl` elaborates), `:47-56` (every `.py` runs via embedded host) — green in gate | ✅ PASS |

### P2: Python lib sanitized + documented

| Criterion | Spec-defined outcome | `file:line` + assertion | Result |
|---|---|---|---|
| consistent naming, no bench-era leak, docstrings everywhere | walk test | `facade_hygiene.rs:27-37` (docstring walk), `:40-46` (`"bench"`/`"stage"` name ban + `$op`/`$tran`/`$ac(` doc-leak ban) — **surviving mutant**: walk uses `inspect.getdoc`, which inherits docstrings from documented non-object bases (Python 3.12+), so `Scale` (only Enum) class-doc removal is undetectable | ❌ Sensor gap (#1) |
| host-API doc covers required topics | doc exists | `docs/spec/part_viii_host_api.md` §1–7: load/Design/Module, analyses + results tables, LiveSession (`set`/`schedule_set`/`rebuilds`), CLI `run`/`-i`/`test`, runnable snippets | ✅ PASS |
| stub/facade parity | parity test | `facade_hygiene.rs:49-58` (every native public surfaced), `:61-73` (every facade method has a native counterpart). Note: no `.pyi` files exist — the annotated facade *is* the typed surface | ✅ PASS |

**Status**: ❌ One sensor-killed gap (BRM-12 blind spot) + one minor vocabulary leak (BRM-02)

---

## Discrimination Sensor

| Mutation | File:line | Description | Killed? |
|---|---|---|---|
| M1 | `crates/piperine-lang/src/parse/mod.rs:51` | `parse_str` swallows parse errors → `bench` parses again | ✅ Killed (`bench_removed::bench_block_is_a_syntax_error`) |
| M2 | `crates/piperine-cli/src/commands/test.rs:42` | discovery filter `_tb.py` → `.py` | ✅ Killed (`test_tb::discovery_finds_tb_files_and_skips_venv_and_target`) |
| M3 | `crates/piperine-cli/src/commands/test.rs:155` | drop `exit(1)` on failure | ✅ Killed (`failing_testbench…` + `hanging_testbench…`) |
| M4 | `crates/piperine-plugin/src/manifest.rs:112` | `bench_tasks` rejection disabled | ✅ Killed (`manifest::bench_tasks_manifest_is_a_clear_removal_error`) |
| M5 | `src/session.rs:100` | `fire_after_solve` no-op (hooks never fire) | ✅ Killed (`phase3::read_only_hooks_observe_the_pipeline`) |
| M6a | `crates/piperine-python/python/piperine/__init__.py:77` | remove `Scale` class docstring | ❌ **Survived** — `inspect.getdoc` inherits `Enum`'s docstring (verified: `getdoc(Scale)` returns Enum docs on Python 3.13) → fix task |
| M6b | `__init__.py:212` | remove `Design.module` method docstring | ✅ Killed (`facade_hygiene`) — walk discriminates methods and plain classes |

**Sensor depth**: lightweight+ (6 faults, feature-critical paths)
**Result**: 6/7 killed, 1 survived — **FAIL** (surviving mutant = weak test)

All mutations reverted; `git status` clean.

---

## Code Quality

| Principle | Status |
|---|---|
| Minimum code | ✅ (small `--list`/explicit-file extras on `piperine test`, documented in-code) |
| Surgical changes | ✅ |
| No scope creep | ✅ |
| Matches patterns | ✅ (cli commands keep the existing `pub fn execute` module pattern; MD-13 governs solver/codegen) |
| Spec-anchored outcome check | ⚠️ (gap #1/#2) |
| Per-layer coverage met | ✅ |
| Every test maps to a spec requirement | ✅ |
| Guidelines followed: `CLAUDE.md`, MD-13 | ✅ |

---

## Edge Cases

- [x] `bench_tasks` manifest → loud removal error naming `*_tb.py` (`manifest.rs:86-102`; mutant M4 killed)
- [x] hanging `_tb.py` → timeout kill + FAIL + exit 1 (`test_tb.rs:95-102`, `PIPERINE_TEST_TIMEOUT_SECS=2`)
- [x] headers/spice fixtures ported off bench blocks (`tests/spice/*.phdl` bench-free; `spice_smoke.rs` assertions preserved; `headers/` frozen-untouched)

---

## Gate Check

- **Gate command**: `cargo build --workspace` + `cargo test --workspace`
- **Build**: zero rustc warnings (exit 0, no warning lines)
- **Result**: 445 passed, 0 failed, 5 ignored
- **Test count at feature close (baseline)**: 445 passed / 5 ignored
- **Delta**: 0 (bench.rs/lang bench tests deleted, new suites added — net matches closure baseline)
- **Skipped**: 5 ignored — pre-existing ignored doctests (plugin/lib, plugin-wasm, solver×3), none introduced by this feature
- **ngspice**: on PATH (`/usr/bin/ngspice`) — harness ran live: 12 golden/sweep cases `PASS`, 0 `SKIP`
- **Failures**: none

---

## Fix Plans

### Fix 1: facade_hygiene docstring walk blind to inherited docstrings (BRM-12)

- **Root cause**: probe uses `inspect.getdoc(obj)`; on Python 3.12+ `getdoc` on a class falls back to the nearest documented non-`object` base (`Enum` for `Scale`), so removing a class's own docstring is undetectable for Enum subclasses.
- **Fix task**: in `crates/piperine-python/tests/facade_hygiene.rs` probe, assert on the *own* docstring — `getattr(obj, "__doc__", None)` for classes (or `type(obj).__dict__.get("__doc__")`) — keeping `getdoc` only for routines. Re-run sensor mutant M6a to confirm kill.
- **Priority**: Major (test cannot detect the regression it guards)

### Fix 2: "bench" in a reachable error message (BRM-02)

- **Root cause**: `pom/design.rs:385` — `"bench root module `{root_module}` not found"` predates removal; reachable via `SimSession::new(design, bad_name)` + any analysis.
- **Fix task**: reword to `"root module `{root_module}` not found"`; sweep the stale doc comments at `pom/design.rs:361/372/376/493`, `parse/ast.rs:71/431/741`, `piperine-plugin/src/{view.rs:21,42, capability.rs:11}` and the 10 `examples/*.phdl` header comments referencing "the bench".
- **Priority**: Minor (gap #2) / Cosmetic (gap #3)

---

## Observations (not gaps for this feature)

- **Embedded-CPython stdout buffering (pre-existing)**: `print()` in scripts run via `piperine run`/`piperine test` is lost unless flushed — the embedded interpreter is never finalized, so buffered stdout dies with the process (`embed.rs`, untouched by this feature; tracebacks survive because stderr is unbuffered). Affects the usefulness of `piperine test` output for chatty testbenches. Belongs to the python-host feature's backlog.
- **Logged deviation untested**: `piperine run <file>.phdl` migration notice (`run.rs:43-49`) has no assertion; deviation was user-logged, so accepted, but a notice-text test would close it.

---

## Requirement Traceability Update

| Requirement | Previous | New |
|---|---|---|
| BRM-01 | Verified | ✅ Verified (sensor M1) |
| BRM-02 | Verified | ⚠️ Minor leak (design.rs:385 + stale comments) |
| BRM-03 | Verified | ✅ Verified |
| BRM-04 | Verified | ✅ Verified |
| BRM-05 | Verified | ✅ Verified (ngspice live) |
| BRM-06 | Verified | ✅ Verified |
| BRM-07 | Verified | ✅ Verified (deviation logged) |
| BRM-08 | Verified | ✅ Verified (sensor M2) |
| BRM-09 | Verified | ✅ Verified (sensor M3) |
| BRM-10 | Verified | ✅ Verified |
| BRM-11 | Verified | ✅ Verified |
| BRM-12 | Verified | ❌ Needs Fix (surviving mutant M6a) |
| BRM-13 | Verified | ✅ Verified |
| BRM-14 | Verified | ✅ Verified (sensor M6b) |
| BRM-15 | Verified | ✅ Verified (sensors M4, M5; hooks amendment honored) |

---

## Summary

**Overall**: ❌ Not Ready (one surviving mutant — weak test on BRM-12)

**Spec-anchored check**: 17/19 ACs matched spec outcome; 1 sensor gap (BRM-12), 1 minor vocabulary leak (BRM-02)
**Sensor**: 7 injected, 6 killed, 1 survived
**Gate**: 445 passed, 0 failed, 5 ignored; build zero warnings; ngspice live

**What works**: grammar removal + parse error; root lib face (lib-only, bin in cli); all migrated suites green with identical assertions incl. live ngspice; `piperine test` discovery/report/exit-codes/timeout; plugin bench-task loud error; SimHooks lifecycle preserved and fired; examples dual contract; docs complete.

**Issues found**:
1. Major — facade_hygiene `getdoc` blind spot for Enum-subclass class docstrings (fix: assert `__doc__` directly)
2. Minor — `"bench root module … not found"` reachable error string (`pom/design.rs:385`)
3. Cosmetic — stale bench-era comments in lang/plugin/examples

**Next steps**: route Fix 1 (+2/3 optionally) to an implementer; re-verify sensor mutant M6a after the fix.

---

# Round 2 — Re-verification

**Date**: 2026-07-17
**Diff range**: `f408761..HEAD` (`5498d3a` fix commit on top of round-1 range)
**Verifier**: independent sub-agent (author ≠ verifier)

## Per-Gap Re-verification

| Gap | Claimed fix | Fresh evidence | Result |
|---|---|---|---|
| #1 Major (BRM-12) `getdoc` inherits base docstrings | walk asserts own `__doc__` | `facade_hygiene.rs:26` — `doc = getattr(obj, "__doc__", None) or ""` (+ rationale comment :19-21). Independent M6a re-run: deleted `Scale` docstring (`python/piperine/__init__.py:77`) → `cargo test -p piperine-python --test facade_hygiene` FAILED `piperine.Scale: missing docstring`; `git checkout` restore → PASS | ✅ FIXED — mutant killed |
| #2 Minor (BRM-02) `"bench root module … not found"` | renamed | `pom/design.rs:387` — now `"root module `{root_module}` not found"`; repo-wide grep: zero `"bench root"` hits | ✅ FIXED |
| #3 Cosmetic stale comments | swept design.rs/ast.rs/view.rs/capability.rs + 14 example headers | grep: those 4 files → zero `bench` hits; `examples/*.phdl` → only "testbench" (`12_opamp_follower.phdl:20`, new-world vocabulary). **However** the full-surface sweep (this round's mandate) found residuals the fix commit did not claim — see New Gaps below | ⚠️ claimed scope clean; new residuals found |
| #4 L-004 logged deviation untested | test added | `test_tb.rs:131-148` `run_phdl_elaborates_and_points_at_testbenches` asserts "elaborates" + "bench"+"removed" + "_tb.py"; green in gate | ✅ FIXED |

## Discrimination Sensor (round 2)

| Mutation | File:line | Description | Killed? |
|---|---|---|---|
| M6a (re-run) | `python/piperine/__init__.py:77` | remove `Scale` class docstring | ✅ Killed (`piperine.Scale: missing docstring`) |
| M7 (new) | `crates/piperine-cli/src/commands/run.rs:43-49` | neuter migration notice → `"{} elaborates."` | ✅ Killed (`run_phdl_elaborates_and_points_at_testbenches`: "removal named") |

**Result**: 2/2 killed. All mutations reverted; `git status` clean.

## Gate Check (round 2)

- **Build**: `cargo build --workspace` — zero rustc warnings
- **Test**: `cargo test --workspace` — **446 passed, 0 failed, 5 ignored** (matches post-fix baseline; +1 vs round 1 = the new run-phdl test)
- **Failures**: none

## New Gaps (found by the mandated full-surface sweep)

1. **Minor (BRM-02/BRM-10)** — `crates/piperine-cli/src/lib.rs:33`: clap doc on `Run::entry` — `piperine run --help` prints "The entry point to run: `module::fn` (bench), …", advertising a removed entry-point form to users (verified live: `./target/debug/piperine run --help`). Same leak class as round-1 gap #2, one surface over. Fix: drop "`module::fn` (bench), " from the help text.
2. **Cosmetic (BRM-02)** — stale present-tense bench comments in files outside the fix commit's claimed sweep: `piperine-codegen/src/device/circuit.rs:41,122,646`; `piperine-lang/src/pom/staging.rs:36` (documents `"bench"` as a live `staged_by` value — no code writes it anymore), `eval/const_host.rs:3`, `eval/interp.rs:102,131,151,330,415` (incl. a "bench spec §1" citation), `eval/error.rs:2`; `piperine-python/src/lib.rs:3` (crate doc "exposes the Piperine bench") + test comments `lib.rs:351,427-450,797-1053`, `module.rs:33,76`, `instance.rs:162`.

**Intentional, not flagged**: `run.rs:18,44` migration notice (names the removal); `manifest.rs:74-114` `bench_tasks` rejection field + loud removal error; test assertions naming the removed surface; "testbench"/"_tb.py" new-world vocabulary; historical docs/spec tombstones.

## Requirement Traceability Update (round 2)

| Requirement | Round 1 | Round 2 |
|---|---|---|
| BRM-02 | ⚠️ Minor leak | ⚠️ Minor leak (new: cli lib.rs:33 help text; cosmetic comments) |
| BRM-12 | ❌ Needs Fix | ✅ Verified (sensor M6a killed) |
| BRM-07 | ✅ (deviation logged) | ✅ Verified (deviation now tested; sensor M7) |
| all others | as round 1 | unchanged |

## Round-2 Summary

**Overall**: ❌ Not Ready — one new Minor user-facing vocabulary leak (+ cosmetic residuals)

**Spec-anchored check**: all 4 round-1 gaps verifiably fixed with fresh evidence; BRM-12 mutant now killed; but the full-surface sweep (round-2 mandate) exposed a `run --help` text advertising the removed bench entry point — same BRM-02 class as the just-fixed error string.
**Sensor**: 2 injected (M6a re-run, M7 new), 2 killed
**Gate**: 446 passed, 0 failed, 5 ignored; build zero warnings

**Next steps**: one-line help-text fix at `crates/piperine-cli/src/lib.rs:33` (+ optional cosmetic comment sweep listed above); no re-sensor needed beyond gates.
