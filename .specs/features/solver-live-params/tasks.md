# solver-live-params Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement with the `tlc-spec-driven` skill Execute flow (per-task cycle,
atomic commits, gates, adequacy checks, Verifier at the end). If the skill
cannot be activated, STOP.

---

**Design**: `.specs/features/solver-live-params/design.md`
**Status**: COMPLETE — all phases (T1–T11) delivered (2026-07-17)

## Batch 2 results (T6–T11)

- T6 `e74bb94` `_LiveSession` (compile once, set, re-run; result-shape parity)
- T7 `7d1a47b` Python facade + error parity + `schedule_set`
- T8 `c5f4b3f` `examples/live_optimize.py` (single compile, ≥10× vs rebuild)
- T9 `35ba4e8` auto re-elab on Rebuild (notice, net-name carry, LIVE-17)
- T10 `47d7dfa` mid-transient structural set restarts from `t`: segmented tran
  in `live.rs` (structural probe → split → rebuild → restart with carried ICs
  → stitched trace); `TransientAnalysisOptions::with_start` absolute clock;
  **solver fix en route:** TR stage now degrades to backward Euler when
  `prev_h = 0` (`TrBdf2::stage_coeffs`) — the old `2/(γh)` + assumed-zero
  previous current doubled the first-step derivative after any discontinuity;
  restarted segments begin at `1e-3·dt` (solver's own post-set convention)
- T11: docs Part VII §10.5 (live sets + host surface + restart convention),
  ROADMAP delivered-entry, traceability LIVE-01..17 Verified
- Workspace: **472 passed, 0 failed** (baseline 465 + 7); zero warnings;
  python examples **24/24** via `piperine run` (23 numbered + live_optimize)

## Batch 1 results (T1–T5)

- T1 `107bf40` loud live set: unknown param lists candidates, unknown path
  echoes, bypass stamp cache dropped on set (mutation-verified), idle set OK
- T2 `f15273a` POM path parity over the ACTUAL grammar: elaboration flattens
  the top module (nested hierarchy fail-louds in codegen today) → labels are
  flat instance names + `{param}_{field}` bundle flattening; bounds gate loud
- T3 `8d5c73d` MD-18 proof: 10 set+solve cycles (Restamp+Temperature),
  compile_count delta 0
- T4 `62486d8` `TransientSolver::schedule_set` + `SetQueue` → TRB-11 table;
  exact landing, LTE-exempt edge, prev_h reset; last-write-wins; ≥OP re-solve
  at t; Rebuild fail-loud at solver level; `Invalidation: Ord`. RC 2k→1k@5µs
  matches closed form reltol 1e-3
- T5 `d400973` C-jump/L-jump robust: zero rejections, no NaN
- **Two solver bugs fixed en route:** rejected TR-BDF2 attempts polluted the
  Newton history buffer (now snapshot/restored around candidates); inductor
  flux companion TR stage was missing the `−V_n` trapezoid term (RL τ ~35%
  off) — fixed in `force_flux_stamps` with prev_h=0 restart convention
- Reactive/transient live tests live in `piperine-codegen/tests/live_params.rs`
  (JIT devices needed; solver→codegen dev-dep would cycle)
- Workspace: 465 passed, 0 failed (baseline 445 + 20)

**Intel for batch 2 (T6–T11):** dotted `x1.d1` paths not compilable today
(flat elaboration) — LiveSession naming uses flat labels; LIVE-01 already
covered at actual grammar. Branch: `feature/solver-live-params`.

---

## Test Coverage Matrix

> Guidelines: `CLAUDE.md` (zero warnings, `cargo test --workspace`),
> `AGENTS.md` MD-13. Baseline: **445 workspace tests** green (post
> spice-stdlib). Python examples 21/21 via `piperine run`.

| Code Layer | Test Type | Coverage Expectation | Location | Run Command |
|---|---|---|---|---|
| solver set/queue (`core/circuit.rs`, `solver/transient.rs`) | unit + integration | 1:1 to LIVE-01..09 incl. edge cases (last-write-wins, bounds, bypass) | `piperine-solver/tests/live_params.rs` | `cargo test -p piperine-solver` |
| naming parity (POM vs solver) | integration | LIVE-01 over hierarchical + bundle design | `piperine-solver/tests/live_params.rs` (fixture via codegen) or `piperine-codegen/tests/` | `cargo test -p piperine-codegen -p piperine-solver` |
| LiveSession (piperine-python Rust) | integration | LIVE-10..17; compile-count proof isolated binary | `piperine-python/tests/live_session.rs` | `cargo test -p piperine-python` |
| Python facade + example | e2e | LIVE-12 optimization script passes via `piperine run` | `examples/*.py` + run_examples-style check | `cargo test -p piperine-bench run_examples` or python harness |
| Docs (spec Part refs, ROADMAP) | none | build gate only | — | build |

## Gate Check Commands

| Gate | Command |
|---|---|
| Quick | `cargo test -p <crate>` |
| Full | `cargo test --workspace` |
| Build | `cargo build --workspace` (zero warnings) + `cargo test --workspace` |

---

## Execution Plan

### Phase 1: Solver set core (T1 → T2 → T3)
### Phase 2: Mid-transient semantics (T4 → T5)
### Phase 3: Python LiveSession (T6 → T7 → T8)
### Phase 4: Auto re-elab + closure (T9 → T10 → T11)

Batching: batch 1 = Phases 1+2 (5 tasks), batch 2 = Phases 3+4 (6 tasks).

---

## Task Breakdown

### T1: Hardened solver set (errors, bypass, OP-dirty)
**What**: `set_element_param` unknown-param error lists `list_params()`;
successful set invalidates that element's bypass stamp cache and marks the
operating point dirty (next analysis re-solves OP).
**Where**: `piperine-solver/src/core/circuit.rs` + bypass hooks
**Depends on**: None · **Requirement**: LIVE-03, LIVE-04, LIVE-05, LIVE-08
**Done when**:
- [ ] Tests: unknown path loud; unknown param lists candidates; set → bypass cache miss on next iteration (CP-11 hooks); idle set applies next run
- [ ] Gate quick: `cargo test -p piperine-solver`
**Tests**: unit+integration · **Gate**: quick
**Commit**: `feat(solver): loud live set with bypass/OP invalidation`

### T2: PHDL path parity
**What**: Parity test (and label fixes if needed): element labels ==
POM `Design::set_param` paths over a hierarchical design with bundles;
out-of-bounds value rejected via `ParamDescriptor` bounds.
**Where**: `piperine-solver/tests/live_params.rs`, codegen labels if needed
**Depends on**: T1 · **Requirement**: LIVE-01 + bounds edge case
**Done when**:
- [ ] Parity test: same instance affected via POM path and solver label (2-level hierarchy + bundle param)
- [ ] Bounds test: out-of-range set fails loud, value unchanged
- [ ] Gate quick
**Tests**: integration · **Gate**: quick
**Commit**: `test(solver): live set addresses elements by pom path parity`

### T3: Zero-recompile proof at solver level
**What**: Restamp-only guarantee: set + re-solve N times with
`AnalogKernel::compile_count` unchanged after build.
**Where**: `piperine-solver`/`piperine-codegen` test (kernel counter exists)
**Depends on**: T1 · **Requirement**: LIVE-02
**Done when**:
- [ ] Isolated test binary proves compile count constant across ≥10 set+solve cycles (Restamp and Temperature invalidations)
- [ ] Gate quick
**Tests**: integration · **Gate**: quick
**Commit**: `test(solver): live set never recompiles (md-18 proof)`

### T4: Scheduled-set queue + breakpoint landing
**What**: `SetQueue` on the transient driver: `(t, label, param, value)`
entries feed the unified breakpoint table; on landing at t apply the set
(last-write-wins per param; one breakpoint per set call), map invalidation
(`Restamp`/`Temperature` → restamp; ≥`OperatingPoint` → consistent re-solve
at t).
**Where**: `piperine-solver/src/solver/transient.rs` (+ queue type per MD-13)
**Depends on**: T1 · **Requirement**: LIVE-06, LIVE-09 + last-write-wins edge
**Done when**:
- [ ] Unit: queue ordering, last-write-wins, breakpoint registration
- [ ] Integration: RC 2k→1k at t=5µs — step lands exactly on 5µs, LTE skipped at edge, new time constant after t (waveform vs fresh-run reference within reltol 1e-3)
- [ ] OP-strength invalidation re-solves at t (test with a source-value set)
- [ ] Gate full: `cargo test --workspace`
**Tests**: unit+integration · **Gate**: full
**Commit**: `feat(solver): scheduled live sets land on transient breakpoints`

### T5: Reactive discontinuity robustness
**What**: Set on reactive elements mid-tran (C value jump, L value jump):
charge/flux history handled by breakpoint edge rules — no NaN, no dt
collapse, waveform matches fresh simulation started from pre-set state.
**Where**: `transient.rs` (fixes if needed) + tests
**Depends on**: T4 · **Requirement**: LIVE-07
**Done when**:
- [ ] C-jump and L-jump tests within reltol 1e-3 vs reference; no LTE rejection storm (rejected-step count bounded)
- [ ] Gate full
**Tests**: integration · **Gate**: full
**Commit**: `fix(solver): reactive live sets ride discontinuity handling`

### T6: LiveSession (Rust core in piperine-python)
**What**: `LiveSession { design, circuit, info, rebuilds }`: build once via
codegen; `set(path, param, value)` routes to solver; analyses (`op/tran/ac/
noise`) run on the held circuit, reusing `module.rs` config/result mapping.
No bench surface added.
**Where**: `crates/piperine-python/src/live.rs` (+ `lib.rs` registration)
**Depends on**: T3 · **Requirement**: LIVE-10 (partial: single build), LIVE-13
**Done when**:
- [ ] Rust test: session builds once (compile-count), set+op loop works, results equal fresh builds
- [ ] Result shape identical to `_Module` results (same pyclass types)
- [ ] Gate quick: `cargo test -p piperine-python`
**Tests**: integration · **Gate**: quick
**Commit**: `feat(python): live session — compile once, set, re-run`

### T7: Python facade + parity
**What**: `design.compile()` → facade `LiveSession` with typed methods
(autocomplete parity), `set`/`schedule_set`, `rebuilds` property; error
parity with Rust path (same messages).
**Where**: `piperine-python/python/piperine/` facade + `src/live.rs`
**Depends on**: T6 · **Requirement**: LIVE-11
**Done when**:
- [ ] Python-side test: set errors match Rust (unknown path/param/bounds)
- [ ] `schedule_set` reaches the transient queue (mid-tran RC scenario from Python)
- [ ] Gate quick
**Tests**: e2e · **Gate**: quick
**Commit**: `feat(python): live session facade with phdl-name set`

### T8: Optimization-loop example + perf AC
**What**: Example script: bisection fitting a resistor to a target node
voltage on a live session; asserts single compilation; ≥100 set+op
iterations equal fresh builds within reltol 1e-3 and ≥10× faster than
re-elaborating per point.
**Where**: `examples/live_optimize.py` + test hook
**Depends on**: T7 · **Requirement**: LIVE-12
**Done when**:
- [ ] Example passes via `piperine run`; timing assertion (10×) in test
- [ ] Gate full
**Tests**: e2e · **Gate**: full
**Commit**: `feat(python): live optimization example (single-compile loop)`

### T9: Auto re-elab on Rebuild
**What**: LiveSession catches `Invalidation::Rebuild` (and solver set errors
classified structural): POM `Design::set_param` → re-elaborate → recompile →
carry node voltages by net name as next guess → `rebuilds += 1` notice.
Re-elab failure keeps old circuit usable.
**Where**: `piperine-python/src/live.rs`
**Depends on**: T6 · **Requirement**: LIVE-14, LIVE-15, LIVE-17
**Done when**:
- [ ] dio `ns none→1.2` test: rebuild reported, sidewall behavior appears, result correct
- [ ] Carried-state test: warm start uses previous node voltages (iteration count lower than cold, or guess inspected)
- [ ] Failing re-elab: error surfaced, previous circuit still solves
- [ ] Gate quick
**Tests**: integration · **Gate**: quick
**Commit**: `feat(python): auto re-elaboration on structural live set`

### T10: Mid-tran rebuild restart
**What**: Structural set scheduled mid-transient: auto re-elab at t,
transient restarts from t with carried node state as ICs.
**Where**: `live.rs` + transient plumbing as needed
**Depends on**: T9, T4 · **Requirement**: LIVE-16
**Done when**:
- [ ] Test: waveform continuous at t within tolerance; post-t behavior reflects new structure
- [ ] Gate full
**Tests**: integration · **Gate**: full
**Commit**: `feat(python): mid-transient structural set restarts from t`

### T11: Closure
**What**: docs (`docs/spec/` host-surface note, ROADMAP), traceability →
Verified, examples green, zero warnings.
**Where**: docs + `.specs/`
**Depends on**: T5, T8, T10 · **Requirement**: all
**Done when**:
- [ ] Gate build: zero warnings, `cargo test --workspace` ≥ 445 + new, 22/22 python examples (21 + live_optimize)
**Tests**: none (docs) · **Gate**: build
**Commit**: `chore(specs): solver-live-params complete`

---

## Phase Execution Map

```
Phase 1: T1 → T2 → T3
Phase 2: T4 → T5
Phase 3: T6 → T7 → T8
Phase 4: T9 → T10 → T11
```

## Diagram-Definition Cross-Check

| Task | Depends (body) | Diagram | Status |
|---|---|---|---|
| T1 none · T2 T1 · T3 T1 · T4 T1 · T5 T4 · T6 T3 · T7 T6 · T8 T7 · T9 T6 · T10 T9,T4 · T11 T5,T8,T10 | backward-only | phases sequential | ✅ all |

## Test Co-location Validation

| Task | Layer | Matrix | Task Says | Status |
|---|---|---|---|---|
| T1 solver | unit+integration | unit+integration | ✅ |
| T2 parity | integration | integration | ✅ |
| T3 proof | integration | integration | ✅ |
| T4 transient | unit+integration | unit+integration | ✅ |
| T5 transient | integration | integration | ✅ |
| T6 LiveSession | integration | integration | ✅ |
| T7 facade | e2e | e2e | ✅ |
| T8 example | e2e | e2e | ✅ |
| T9 rebuild | integration | integration | ✅ |
| T10 rebuild | integration | integration | ✅ |
| T11 docs | none | none | ✅ |
