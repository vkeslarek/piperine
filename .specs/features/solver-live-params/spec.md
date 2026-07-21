# solver-live-params Specification

Live parameter mutation on a **compiled** circuit: once PHDL → POM → JIT has
produced a `CircuitInstance`, a host changes parameter values directly on the
solver — mid-simulation included — with no re-elaboration and no re-JIT
(MD-18). PHDL hierarchical names address the parameter, with the same
interface shape as the POM's `Design::set_param(path, param, value)`.

## Problem Statement

`CircuitInstance::set_element_param` (spice-stdlib T12) restamps params
between DC solves, but: it is untested mid-simulation; it addresses elements
by flat solver label rather than guaranteed-PHDL-path parity; nothing exposes
it to Python. The two driving use cases — future real-time interactive
simulation (Python → solver set) and optimization loops (fastest/safest way
to vary params) — need a first-class, named, live set path. Topology changes
legitimately re-elaborate; pure value changes must not.

## Goals

- [x] `set` on a live compiled circuit by PHDL instance path + param name,
      identical addressing to the POM interface.
- [x] Set during a running transient takes effect at the next accepted step
      and forces a breakpoint at the set time.
- [x] Python: live session object — compile once, `set(...)`, re-run
      analyses on the same compiled circuit (optimization loop ready).
- [x] Structural changes (`Invalidation::Rebuild`) re-elaborate
      **automatically at the host/session layer**; the solver itself stays
      fail-loud (it has no POM).
- [x] All caches consistent after set (device bypass, OP validity,
      temperature-derived constants).

## Out of Scope

| Feature | Reason |
|---------|--------|
| Interactive transient stepping from Python (`step()`/`run_until()`) | Real-time feature comes later; this feature lays the set mechanism it will use |
| New bench surface | Bench in-lang will be discontinued (user 2026-07-16) — host surface is Python |
| GUI/streaming result delivery | Real-time feature |
| Optimization algorithms themselves | Host code; we deliver the fast set+rerun primitive |
| Topology editing API (add/remove instances on POM) | Existing elaboration path already covers it |

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| Mid-transient set semantics | Effect at next accepted step + forced breakpoint at set time (TR-BDF2 discontinuity handling: skip LTE at edge, reset prev_h) | Deterministic, stable | y (user) |
| Structural set (`Rebuild`) | Auto re-elaborate at host/session layer, transparent; solver-level call still returns typed `Rebuild` outcome | User choice | y (user) |
| Python exposure | Live session: set + re-run on same compile; no interactive tran stepping yet | User choice | y (user) |
| Path grammar | Same hierarchical instance path as POM `Design::set_param` (`"x1.d1"`, param `"model_is"`); solver element labels are the flattened path — parity asserted by test | Single addressing scheme everywhere | n (agent) |
| Set during AC/noise | Set invalidates the operating point; the next analysis recomputes OP first | AC is a linearization of OP; stale OP would be silent wrongness | n (agent) |
| Unknown path/param | Loud typed error naming the element/param; error lists available params of the element when the element exists | Debuggability | n (agent) |
| State carry across auto re-elab | Node voltages carried by net name as nodeset/initial guess; nets that disappear are dropped, new nets start cold | Best-effort warm start; exact carry impossible across topology change | n (agent) |
| Mid-tran structural set | Auto re-elab + transient restarts from the set time with carried node state as ICs | Only coherent reading of "auto" mid-run | n (agent) |
| Bypass cache | Any successful set invalidates the device-bypass stamp cache for that element | Known trap (solver-convergence-perf: bypass invalidation) | n (agent) |

**Open questions:** none — all resolved or logged above.

## User Stories

### P1: Solver-level live set by PHDL name ⭐ MVP

**User Story**: As a host author, I want to set `"x1.d1"` / `"model_is"` on a
compiled `CircuitInstance` exactly as I would on the POM, so one addressing
scheme works before and after compilation.

**Acceptance Criteria**:

1. WHEN `set` is called with a PHDL instance path that the POM
   `Design::set_param` accepts for the same design THEN the solver SHALL
   resolve the same instance (parity test over a hierarchical design).
2. WHEN the param exists and is numeric-only THEN the solver SHALL return
   `Invalidation::Restamp` (or `Temperature`/`OperatingPoint` as declared)
   and the next load SHALL use the new value with **zero** recompilation
   (compile-count unchanged).
3. WHEN the element exists but the param does not THEN the call SHALL fail
   loud, naming the element and listing its available params.
4. WHEN the path resolves to no element THEN the call SHALL fail loud with
   the path.
5. WHEN a set succeeds on an element with an active bypass stamp cache THEN
   the cache SHALL be invalidated (next iteration re-evaluates the device).

**Independent Test**: Rust test — compiled two-level hierarchy, set via both
POM path and solver path, same element affected; wrong names loud.

### P1: Mid-transient set semantics ⭐ MVP

**User Story**: As a future real-time host, I want a set issued while a
transient runs to land deterministically, so interactive simulation is stable.

**Acceptance Criteria**:

1. WHEN set is issued between accepted steps at time t THEN the new value
   SHALL apply from the next accepted step and a breakpoint SHALL be forced
   at t (step lands exactly on t; LTE skipped at the edge per the TR-BDF2
   discontinuity rules).
2. WHEN the set changes a reactive element (e.g. C) THEN charge history
   SHALL be handled by the existing discontinuity machinery (no dt collapse,
   no NaN, no LTE rejection storm) — waveform after t matches a fresh
   simulation started from the pre-set state within reltol 1e-3.
3. WHEN set is issued with no transient running THEN it simply applies to
   the next analysis run.
4. WHEN the set's `Invalidation` is `OperatingPoint` (or stronger) during a
   transient THEN the driver SHALL re-solve consistently at the breakpoint
   before continuing.

**Independent Test**: RC step-response: mid-tran set of R (2k→1k) at t=5µs;
waveform shows the new time constant from t=5µs; breakpoint exactly at 5µs.

### P1: Python live session ⭐ MVP

**User Story**: As a Python user doing optimization, I want
`sim = design.compile()` … `sim.set("d1", "model_is", v)` … `sim.op()` in a
loop with one JIT compilation total, so parameter studies are fast and safe.

**Acceptance Criteria**:

1. WHEN a live session is created THEN elaboration+JIT SHALL happen once;
   subsequent `set` + analysis re-runs SHALL not recompile
   (compile-count-style proof, as in spice-stdlib `compile_once_sweep.rs`).
2. WHEN `set` is called from Python with PHDL names THEN behavior SHALL be
   identical to the Rust solver path (same errors, same invalidation
   semantics).
3. WHEN an optimization-style loop runs (≥100 set+op iterations on a
   nonlinear circuit) THEN results SHALL equal per-point fresh builds within
   reltol 1e-3 and complete ≥10× faster than re-elaborating per point.
4. WHEN analyses re-run after set THEN results objects SHALL behave exactly
   as the existing `op/tran/ac/noise` results (same shape — PY-17 rule).

**Independent Test**: Python script fitting a resistor to hit a target node
voltage via bisection on a live session; asserts single compilation + result.

### P2: Auto re-elaboration on structural change

**User Story**: As a designer iterating, I want a structural param change
(e.g. `Real?` none→given that adds a sidewall diode) to just work, so I don't
manage the elaborate/compile boundary manually.

**Acceptance Criteria**:

1. WHEN a set returns `Invalidation::Rebuild` on a live session THEN the
   session SHALL re-elaborate + recompile automatically and report it
   (visible flag/notice on the session, not silent).
2. WHEN auto re-elab happens THEN node voltages SHALL carry by net name as
   the next solve's initial guess; dropped nets discarded, new nets cold.
3. WHEN auto re-elab happens mid-transient THEN the transient SHALL restart
   from the set time with carried node state as ICs.
4. WHEN re-elaboration itself fails THEN the session SHALL surface the
   elaboration error and keep the previous compiled circuit usable.

**Independent Test**: dio model with `ns = none` → set `ns = 1.2` on live
session → auto rebuild reported, sidewall branch appears, results correct.

## Edge Cases

- WHEN set targets a digital element param THEN the same `Element::set_param`
  surface applies (or fails loud if the element declares none).
- WHEN two sets hit the same param before the next step THEN last-write-wins,
  single breakpoint at the last set time (or both times — must be one rule,
  asserted by test: **last-write-wins, breakpoint per set call**).
- WHEN set value is out of the param's declared bounds (`ParamDescriptor`
  bounds) THEN fail loud, no partial apply.
- WHEN `Temperature` invalidation is returned THEN temperature-derived
  constants recompute before the next load (no stale tnom-scaled values).

## Requirement Traceability

| Requirement ID | Story | Phase | Status | Evidence |
|---|---|---|---|---|
| LIVE-01 | P1 naming parity | Done | Verified | T2 `piperine-solver/tests/live_params.rs` (POM-path parity over the actual flat grammar + bundles) |
| LIVE-02 | P1 restamp, zero recompile | Done | Verified | T3 compile-count proof: 10 set+solve cycles, delta 0 |
| LIVE-03 | P1 loud unknown param | Done | Verified | T1 unknown param lists candidates; python parity in `live.rs` tests |
| LIVE-04 | P1 loud unknown path | Done | Verified | T1 unknown path echoed; python `KeyError` parity |
| LIVE-05 | P1 bypass invalidation | Done | Verified | T1 mutation-verified bypass stamp-cache drop |
| LIVE-06 | P1 next-step + breakpoint | Done | Verified | T4 `SetQueue` → TRB-11 table, exact landing, RC 2k→1k@5µs closed form reltol 1e-3 |
| LIVE-07 | P1 reactive discontinuity | Done | Verified | T5 C-jump/L-jump: zero rejections, no NaN, reltol 1e-3 |
| LIVE-08 | P1 idle set applies next run | Done | Verified | T1/T4: sets due at or before start drain pre-OP, no breakpoint |
| LIVE-09 | P1 OP-invalidation mid-tran | Done | Verified | T4 ≥OperatingPoint re-solves the landing point |
| LIVE-10 | P1 python single compile | Done | Verified | T6 `piperine-python/tests/live_session.rs` isolated compile-count binary |
| LIVE-11 | P1 python parity | Done | Verified | T7 same messages/exceptions as the Rust path (`live.rs` tests) |
| LIVE-12 | P1 optimization loop perf | Done | Verified | T8 `examples/live_optimize.py`: 1 compile, ≥100 iters, ≥10× vs rebuild-per-point |
| LIVE-13 | P1 result-shape uniformity | Done | Verified | T6 same pyclass types as `_Module` (`_OpResult`/`_Trace`/`_AcTrace`/`_NoiseTrace`) |
| LIVE-14 | P2 auto re-elab + notice | Done | Verified | T9 dio `ns none→1.2`: rebuild reported, sidewall appears, oracle match |
| LIVE-15 | P2 state carry by net name | Done | Verified | T9 warm start beats cold build's Newton count |
| LIVE-16 | P2 mid-tran rebuild restart | Done | Verified | T10 `live.rs::mid_transient_structural_set_restarts_from_t_with_carried_state`: continuous stitched trace, two-phase closed form reltol 1e-3, tail at new structure |
| LIVE-17 | P2 re-elab failure keeps old circuit | Done | Verified | T9 failing set surfaces error, previous circuit still solves |

**Coverage:** 17 total, 17 mapped to tasks (T1–T10), 0 unmapped ✅

## Success Criteria

- [x] RC mid-tran set demo: exact breakpoint, correct time constant after t.
- [x] Python optimization loop: 1 compile, ≥10× faster than rebuild-per-point.
- [x] Structural set auto-rebuilds with notice; solver core never re-elaborates.
- [x] `cargo test --workspace` green (472 ≥ 445), zero warnings; examples green
      (24/24 python via `piperine run`, incl. `live_optimize.py`).
