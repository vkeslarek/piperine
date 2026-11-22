# Digital Performance Specification

**Status**: POSTPONED (user, 2026-11-22). Not in the automatic queue. It is scheduled after item 13
(`spec-closure`) and after the `code-clarity` refactor; no design, no tasks until the user reopens it.

**Approved**: 2026-11-22 - user: "Q1 Sim Q2 Isso, o ABI suporta já Q3 Sim Q4 Ok Q5 Perfeito" (every open question takes its recommended option; the feature stays POSTPONED until after spec-closure and code-clarity).

## Decision (user, 2026-11-22, verbatim)

"Escreve a spec do digital-performance e deixa postponed por favor. Coloca no final a ordem topologica
como requisito também, assim matamos a maior parte do problema de performance. Vai ficar para depois do
spec closure pois precisamos primeiro de um bom refactor e de todas as features funcionando antes de
pensar em performance focada no digital"

Earlier, on the topological order: "ordena em topo order e reavalia até convergir ou limite de passos,
assim ele aproveita o cache".

What they settle:

| Answer | Effect |
| ------ | ------ |
| "deixa postponed" | Status POSTPONED; the feature leaves the automatic queue. |
| "Coloca no final a ordem topologica como requisito" | The fused, topologically ordered evaluation (lever L5 of the diagnosis) is a requirement of this feature, its last story (DP-P5), not a later feature as the diagnosis recommended. |
| "reavalia até convergir ou limite de passos" | Fused evaluation re-runs its static order until nothing changes or a bounded number of passes (DP-22, DP-23). |
| "depois do spec closure [...] um bom refactor" | Dependencies: `spec-closure` (item 13) and `code-clarity` close first. |

## Problem Statement

The measured diagnosis (`diagnosis.md`, 2026-11-22) finds the digital engine 175x (a 64-bit counter) to
560 000x (10 000 flops) slower per clock cycle than Verilator on the same logic. The generated code is
1 to 8 % of the time; the rest is per-call marshalling, quadratic scheduler paths, per-commit
allocation and observation, and per-microstep scans of the whole design. Worse, a digital-only run holds
every journal entry and every observation until its stop time: 3 M cycles of a counter abort on an
8 GB cap, and a host receives no record until the run ends. The product's target is large, mostly
digital mixed-signal circuits, so this is where it breaks.

## Goals

- [ ] A digital-only run of any length runs in bounded memory and streams its records (defect fix).
- [ ] The cost per flop update does not grow with the number of flops.
- [ ] The event engine's cost per device call falls from 0.9-2.7 us to at most 300 ns (500 ns for
      the DPWM), with no heap allocation on the call path.
- [ ] Per-microstep work is proportional to what moved, not to the size of the design.
- [ ] Synchronous digital logic of one compiled unit runs as straight-line code in a static topological
      order over a flat state, within 50x of Verilator per cycle on register-heavy designs.
- [ ] Nothing observable changes: every record, observation and error of every existing design is
      byte-identical.

## Non-negotiables

These bind every story; each is also an acceptance criterion (DP-01 to DP-05).

1. **No semantics change.** Records, observations, `HORIZON`, transport (C3), `MERGE` under unknown
   control, `after` delays, rewindable instants and every error code behave exactly as today.
2. **The explicit-unknowns model.** The feature is written against the value model of
   `explicit-unknowns` (no hidden known flag; `Bit` two-state, `Logic` four-state, `Option` explicit)
   and does not reintroduce an implicit unknown.
3. **Parity and twins untouched.** The ngspice parity targets, the transpile goldens, the interpreter
   twins and the buck run are byte-identical; no tolerance moves.
4. **The ABI stays the external boundary.** Fusion happens inside one MSL-compiled unit, which appears
   to the solver as one device honoring the ABI. A foreign device (a native plugin, an OSDI model) is
   never fused and keeps its per-device ABI calls.
5. **The bench harness is the measurement of record.** `crates/piperine-api/tests/digital_bench.rs`
   prints every figure and every gate; a gate is passed only by its printed line.

## Dependencies

| Dependency | Why | State on 2026-11-22 |
| ---------- | --- | ------------------- |
| `explicit-unknowns` merged to `main` | It changes the generated digital code's shape; fusion written before it would be written twice | Verified on its branch, not merged |
| `code-clarity` | The engine's modules are refactored there; performance work on code about to move is wasted | Spec approved, inventory pending |
| `spec-closure` (item 13) | "todas as features funcionando antes de pensar em performance" | Spec approved |

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| L7: parallel partitions of one instant (threads per level or per microstep) | Diagnosis §5: negative today - a call costs about 2 us and an instant holds a handful of calls, so a thread handoff (1 to 10 us) costs more than the work. Worth reconsidering only after DP-P5, on domains of thousands of flops with coarse partitions (Verilator's own `--threads` needs large designs to gain). A future feature. |
| Fusing foreign devices (native plugins, OSDI) | Non-negotiable 4: the ABI is their boundary. |
| Changing the value model to two-state for speed | Non-negotiable 2: `Logic` stays four-state where the source says `Logic`. |
| A cycle-based clock (toggling the clock outside simulated time, as Verilator's testbench does) | The clock is an `Osc` in simulated time; replacing it changes semantics. Its cost is part of the fixed per-cycle cost the gates already include. |
| Analog-side or mixed-signal transient step cost | Owned by `transient-performance`; this feature only helps it through the shared digital machinery. |
| Compile-time cost (6 to 13 s for 10 000 flops, one unexplained 419 s cold compile) | Not a run cost; recorded in the diagnosis, not investigated. A separate item if it recurs. |
| Removing rewindability of instants in mixed runs | A semantic of the owner protocol (non-negotiable 1); only a digital-only run, which is never rewound, may release its journal early. |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Reference machine for absolute gates | AMD Ryzen 5 4600H (the diagnosis machine), release build, best of 3 runs; the load is printed beside the figure | Absolute ns depend on the machine; the diagnosis numbers are from it | n |
| Verilator ratios | Measured in the same session with `verilator/run.sh` (Verilator 5.043, flags of the diagnosis §4), so they are robust to the machine | A ratio cancels the machine; the twins already exist | n |
| Streaming granularity of a digital-only run | Records of an instant reach the sink when that instant finishes; the journal is released at the same point | A digital-only run is never rewound, so nothing can retake a finished instant | n |
| Per-call target | 300 ns per device call (500 ns for the DPWM, 7 to 9 commits per edge) | Diagnosis §5 L3: "from 1 to 2.7 us per call to a few hundred ns" | n |
| Default pass bound of fused evaluation | 100 passes, configurable in `SolverConfig` | Verilator's default `--converge-limit` is 100; a levelized domain settles in 1 pass, so the bound only matters for combinational loops | n |
| Non-convergence of a fused domain | The run fails with the error the event engine gives for the same design (`E0501`, naming the carriers still changing) | Non-negotiable 1: same outcome as the oracle, no new error | n |
| Constructs fusion does not cover | A reaction or definition that reads `HORIZON`, commits with a nonzero `after`, drives a resolved net, or is under unknown control that needs `MERGE` stays on the event engine; it is never approximated | Exactness first; the design measures how much this leaves out | n |

**Open questions:** none - answered 2026-11-22, every recommendation accepted (user: "Q1 Sim Q2 Isso, o ABI suporta já Q3 Sim Q4 Ok Q5 Perfeito"). The table records the questions and the chosen option:

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | What sizes do we design for? | (a) the bench as is, up to 10 000 flops; (b) add a 100 000-flop case as a scaling gate (flat per-flop cost only, no absolute target); (c) also 1 M flops | **(b).** 10 000 is where today's engine breaks; 100 000 proves the flat cost holds an order further without making the bench minutes long |
| Q2 | Does DP-P5 need a new ABI kernel contract? | (a) no: the fused unit is an ordinary device, its domain function exposed through the existing reaction/function entry points; (b) a new entry kind ("domain kernel") in the ABI, MINOR bump | **(a)**, and (b) only if the design proves (a) cannot carry it, coming back to the user as a question. Non-negotiable 4 already holds either way |
| Q3 | DP-P5's Verilator gates: are they right? | (a) register-heavy cases (`flops*`, `pipeline*`) within 50x of Verilator per cycle, small cases (`counter64`, `fsm`, `dpwm`, `chain*`) within 100x; (b) tighter, 10x everywhere; (c) no ratio gate, only the speed-up over the event engine | **(a).** The diagnosis estimates L5 at 20x to 100x over the event engine, which lands near (a); 10x is the aim, printed but not gated |
| Q4 | How is DP-P6's per-domain choice (full re-evaluation vs change tracking) made? | (a) a static rule at compile time (e.g. the share of the domain's state written every edge) with an override in `SolverConfig`; (b) measured at run time and switched; (c) always full re-evaluation | **(a).** Deterministic, testable, and the override lets the bench force both modes to check the rule's choice |
| Q5 | Order of delivery inside the feature | (a) DP-P1 to DP-P4 first, measured, then DP-P5/DP-P6; (b) DP-P5 directly, skipping the event-engine work | **(a).** DP-P1 is a defect; the event engine is the oracle of DP-P5 and must be fast enough to verify it on the large cases |

---

## User Stories

### P1: Bounded memory and streamed records for a digital-only run ⭐ MVP (DP-P1, lever L1)

**User Story**: As a designer simulating a large, mostly digital circuit, I want a digital-only run of
any length to complete in constant memory and to hand me its records while it runs, so that long runs
are possible at all.

**Why P1**: It is a defect, not an optimization: 3 M cycles of a counter abort, 20 M reach 20 GB.
Every other lever is moot while memory grows with time.

**Acceptance Criteria**:

1. WHEN `counter64` runs 20 000 000 cycles under `ulimit -v` of 2 GiB THEN the bench SHALL complete the run and print the probe's final value equal to the cycle count.
2. WHEN `counter64` runs 2 000 000 cycles and then 20 000 000 cycles THEN the bench SHALL print a peak-memory difference between the two runs of at most 16 MiB (today: 3.7 KiB per cycle).
3. WHEN `flops1000` runs 1 000 and then 10 000 cycles THEN the bench SHALL print a peak-memory difference between the two runs of at most 16 MiB (today: about 1.2 KiB per flop update).
4. WHILE a digital-only run is in progress the engine SHALL deliver the records of each instant to the `RecordSink` when that instant finishes, before the next instant starts.
5. WHEN a digital-only instant finishes THEN the engine SHALL release that instant's store and queue journal entries and its observations.
6. WHILE an analog unknown exists in the design the engine SHALL keep today's checkpoint and rewind behavior of the owner protocol unchanged.

**Independent Test**: run the long `counter64` case under the memory cap and a sink that records the
simulated time of each record together with the engine's current time; the run completes, the memory
line passes, and no record arrives more than one instant late.

---

### P1: No quadratic path in the reaction and commit path (DP-P2, lever L2)

**User Story**: As a designer of register-heavy logic, I want the cost per flop update to stay the same
whether the design has 100 or 10 000 flops, so that big designs are not disproportionately slow.

**Why P1**: The per-flop cost grows 3x from 100 to 1 000 flops and 14x from 1 000 to 10 000; one cycle
of 10 000 flops costs 1.9 s. "Big" is exactly where it breaks.

**Acceptance Criteria**:

1. WHEN the bench runs `flops100`, `flops1000` and `flops10000` THEN it SHALL print a ns-per-flop-update for `flops10000` at most 1.5x that of `flops100`.
2. WHEN the bench runs `pipeline100` and `pipeline1000` THEN it SHALL print a ns-per-stage for `pipeline1000` at most 1.5x that of `pipeline100`.
3. WHERE the Q1 100 000-flop case exists, WHEN the bench runs it THEN it SHALL print a ns-per-flop-update at most 1.5x that of `flops100`.
4. The engine SHALL compute the `HORIZON` caps of one microstep's reactions in time O(J log J) for J reactions, and SHALL re-call with a capped horizon only the reactions that read `HORIZON`.
5. The queue SHALL apply transport (C3) to a new commit in time independent of the number of pending commits.
6. The engine SHALL look up a net's layout in constant time at run time.
7. The engine SHALL name an observation by its net id at run time, with no scan of net names.

**Independent Test**: the three `flops` cases and the two large `pipeline` cases in one bench run; the
ratio lines pass and every probe matches its expected value.

---

### P1: An allocation-free device call path (DP-P3, lever L3)

**User Story**: As a designer, I want each device call to cost what the logic costs plus a small
constant, so that the event engine is three to ten times faster on every design.

**Why P1**: The call protocol is 47 to 57 % of the time on most cases and allocation 36 to 47 %.

**Acceptance Criteria**:

1. WHEN the bench runs every case THEN it SHALL print a ns-per-call of at most 300 ns for every case except `dpwm`.
2. WHEN the bench runs `dpwm` THEN it SHALL print a ns-per-call of at most 500 ns.
3. WHEN `counter64` and `dpwm` run 10 000 cycles after 1 000 warm-up cycles with a sink that drops records THEN the counting-allocator line SHALL print fewer than 0.01 heap allocations per device call over those 10 000 cycles.
4. The engine SHALL compute the cell lengths and offsets of every device's shape once at bind, and SHALL NOT walk or validate a shape on a call.
5. The engine SHALL take the device-call lock and the unwind guard at most once per instant, not once per call.
6. WHERE no probe, record, analog boundary or host watches a net, the engine SHALL NOT build an observation for a change of that net.

**Derived ns/cycle gates** (from the calls per cycle of the diagnosis §2.1; the bench prints both):

| case | today ns/cycle | gate ns/cycle |
| ---- | -------------- | ------------- |
| counter64 | 10 504 | 2 100 |
| fsm | 12 807 | 2 100 |
| pipeline10 | 62 786 | 14 100 |
| pipeline100 | 550 003 | 122 100 |
| pipeline1000 | 13 730 296 | 1 202 100 |
| flops100 | 459 466 | 92 100 |
| flops1000 | 13 962 779 | 902 100 |
| flops10000 | 1 946 752 146 | 9 002 100 |
| chain10 | 21 000 | 5 100 |
| chain100 | 103 031 | 32 100 |
| chain1000 | 896 327 | 302 100 |
| dpwm | 35 361 | 6 500 |

**Independent Test**: one full bench run plus the allocation-count run; every ns-per-call and
allocation line passes.

---

### P2: Per-microstep work proportional to activity (DP-P4, lever L4)

**User Story**: As a designer of a large design where little moves per edge, I want a microstep to cost
what moved, so that idle logic costs nothing.

**Why P2**: Small today, but after DP-P2 and DP-P3 the O(design) scans per microstep dominate large
sparse designs.

**Acceptance Criteria**:

1. WHEN the bench runs `sparse10000` (the `counter64` design plus 10 000 flops whose clock never moves) THEN it SHALL print a ns-per-cycle at most 1.2x that of `counter64` in the same run.
2. The engine SHALL find the functions to evaluate in a microstep from a worklist of dirty functions in level order, not from a walk of the whole order.
3. The engine SHALL find the reactions to call in a microstep from the readers of the event nets that fired, not from a scan of every instance.
4. The engine SHALL find the fired and boundary changes of a microstep from a list of changed nets, not from a scan of every net.

**Independent Test**: `sparse10000` against `counter64` in one bench run.

---

### P2: Topological-order fused evaluation of a compiled unit's digital circuit (DP-P5, lever L5)

**User Story**: As a designer of a large synchronous digital block, I want the digital circuit of one
compiled unit evaluated as straight-line code in a static topological order over a flat contiguous
state, re-evaluated until it converges or a bounded number of passes, so that the CPU cache is used
the way Verilator uses it and most of the remaining gap closes.

**Why P2**: The user's requirement: "ordena em topo order e reavalia até convergir ou limite de passos,
assim ele aproveita o cache"; "assim matamos a maior parte do problema de performance". It comes last
because the event engine of DP-P1 to DP-P4 is its exactness oracle.

**Acceptance Criteria**:

1. WHEN a unit is compiled THEN the compiler SHALL compute a static topological order of the unit's fusable definitions and reactions and SHALL store it in the compiled unit.
2. WHEN a fused domain is triggered THEN the unit SHALL evaluate the domain's reactions two-phase (every next value computed from the registered state, then every commit applied) followed by its definitions in the static order, over one flat contiguous state.
3. WHEN one pass of a fused domain changes any value its own order reads THEN the unit SHALL run the order again, until a pass changes nothing or the pass bound (default 100, set in `SolverConfig`) is reached.
4. IF a fused domain still changes after the pass bound THEN the run SHALL fail with the error the event engine gives for the same design (`E0501`, naming the carriers still changing).
5. IF a definition or reaction reads `HORIZON`, commits with a nonzero `after`, drives a resolved net or needs a `MERGE` under unknown control THEN the compiler SHALL leave it on the event engine and SHALL print it in the unit's fusion report.
6. The fused unit SHALL appear to the solver as one device honoring the ABI, and a foreign device SHALL NOT be fused.
7. WHEN the same design runs fused and with fusion off THEN every record, observation and error SHALL be byte-identical between the two runs, for every design of the digital test corpus, every bench case and the buck.
8. WHERE fusion is turned off in `SolverConfig` the engine SHALL run every device on the event engine, as the exactness oracle.
9. WHILE a fused unit runs in a mixed-signal transient the unit SHALL honor the owner protocol's checkpoint and rewind of its whole flat state.
10. WHERE no probe, record, analog boundary or host watches a net of a fused domain, the unit SHALL NOT emit an observation for it.
11. WHEN the bench runs `flops1000`, `flops10000`, `pipeline100` and `pipeline1000` with the Verilator twins in the same session THEN it SHALL print a ratio to Verilator per cycle of at most 50x for each (Q3).
12. WHEN the bench runs `counter64`, `fsm`, `dpwm` and the `chain*` cases with the Verilator twins in the same session THEN it SHALL print a ratio to Verilator per cycle of at most 100x for each (Q3).

**Independent Test**: the differential run (fusion on against fusion off) over the digital corpus is
byte-identical; the bench prints the ratio lines and they pass.

---

### P3: Per-domain choice of full re-evaluation or change tracking (DP-P6, lever L6)

**User Story**: As a designer, I want each fused domain to use whichever of full re-evaluation or change
tracking is cheaper for it, so that datapaths and sparse control logic are both fast.

**Why P3**: 1x to 3x per domain, only after DP-P5 exists (diagnosis §5).

**Acceptance Criteria**:

1. WHEN a fused domain is compiled THEN the compiler SHALL choose full re-evaluation or change tracking for it by the rule of Q4 and SHALL print the choice and its reason in the fusion report.
2. WHERE `SolverConfig` forces a mode for every domain the unit SHALL use that mode.
3. WHEN a design runs with each forced mode and with the chosen one THEN every record, observation and error SHALL be byte-identical across the three runs.
4. WHEN the bench runs `fsm`, `pipeline100` and `flops1000` in both forced modes and in the chosen one THEN it SHALL print a chosen-mode ns-per-cycle at most 1.1x the faster forced mode for each.

**Independent Test**: the three-mode differential run and the bench mode lines.

---

### Cross-cutting: the non-negotiables and the bench of record (DP-NN)

**Acceptance Criteria**:

1. The feature SHALL leave every record, observation and error code of every existing design byte-identical.
2. The feature SHALL leave the ngspice parity targets, the transpile goldens, the interpreter twins and the buck run byte-identical, with no tolerance changed.
3. The engine SHALL NOT reintroduce a hidden known flag or an implicit unknown in the generated digital code.
4. WHEN the bench runs THEN it SHALL print, for every case, its figures, the gate each figure is held to and `PASS` or `FAIL` beside it.
5. WHERE `DIGITAL_BENCH_GATE=1` is set, IF any printed gate is `FAIL` THEN the bench SHALL exit non-zero.
6. WHERE `verilator` is not on `PATH` the bench SHALL print the ratio gates as `SKIPPED (no verilator)` and SHALL NOT count them as passed.

---

## Edge Cases

- IF a digital-only run is stopped by the host before its stop time THEN the engine SHALL have delivered the records of every finished instant.
- IF a fused domain's combinational part is a loop that settles in more than one pass THEN the unit SHALL give the values the event engine gives (DP-P5 AC 7).
- WHEN a design has no fusable logic THEN the unit SHALL run entirely on the event engine and the fusion report SHALL say so.
- WHEN a unit holds two clock domains THEN each SHALL be a separate fused domain with its own trigger.

## Implicit-requirement sweep

| Dimension | Resolution |
| --------- | ---------- |
| Input validation & bounds | Pass bound (DP-P5 AC 3, 4), memory bound (DP-P1). |
| Failure / partial-failure | Non-convergence fails as the oracle does (DP-P5 AC 4); a stopped run keeps finished records (Edge Cases). |
| Idempotency / retry | N/A because a run is deterministic and the feature adds no retried operation. |
| Auth / rate limits | N/A because the engine has no caller identity. |
| Concurrency / ordering | Single-threaded; ordering is the static order (DP-P5 AC 1) and the oracle's (AC 7); parallelism is out of scope (L7). |
| Data lifecycle | Journals and observations released per instant in digital-only runs (DP-P1 AC 5); kept in mixed runs (AC 6). |
| Observability | The bench lines (DP-NN AC 4); the fusion report (DP-P5 AC 5, DP-P6 AC 1). |
| External-dependency failure | Verilator missing (DP-NN AC 6). |
| State-transition integrity | Checkpoint and rewind of the fused state (DP-P5 AC 9). |

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| DP-01 | DP-NN: records/observations/errors unchanged | - | Postponed |
| DP-02 | DP-NN: parity, goldens, twins, buck unchanged | - | Postponed |
| DP-03 | DP-NN: explicit-unknowns model kept | - | Postponed |
| DP-04 | DP-NN: bench prints gates, exit on fail | - | Postponed |
| DP-05 | DP-NN: Verilator missing is skipped, not passed | - | Postponed |
| DP-06 | P1: DP-P1 long run completes under cap | - | Postponed |
| DP-07 | P1: DP-P1 constant memory (counter, flops) | - | Postponed |
| DP-08 | P1: DP-P1 records streamed per instant | - | Postponed |
| DP-09 | P1: DP-P1 journal released; mixed runs unchanged | - | Postponed |
| DP-10 | P1: DP-P2 flat per-flop and per-stage cost | - | Postponed |
| DP-11 | P1: DP-P2 `HORIZON` caps O(J log J), re-call only readers | - | Postponed |
| DP-12 | P1: DP-P2 transport per push independent of pending | - | Postponed |
| DP-13 | P1: DP-P2 net layout and observation by id | - | Postponed |
| DP-14 | P1: DP-P3 ns-per-call gates | - | Postponed |
| DP-15 | P1: DP-P3 allocation-free call path | - | Postponed |
| DP-16 | P1: DP-P3 shapes bound once, lock once per instant | - | Postponed |
| DP-17 | P1: DP-P3 observations only for watched nets | - | Postponed |
| DP-18 | P2: DP-P4 `sparse10000` within 1.2x of `counter64` | - | Postponed |
| DP-19 | P2: DP-P4 worklists for functions, reactions, changes | - | Postponed |
| DP-20 | P2: DP-P5 static topological order in the unit | - | Postponed |
| DP-21 | P2: DP-P5 two-phase flat-state evaluation | - | Postponed |
| DP-22 | P2: DP-P5 re-evaluate to convergence or pass bound | - | Postponed |
| DP-23 | P2: DP-P5 non-convergence fails as the oracle | - | Postponed |
| DP-24 | P2: DP-P5 unfusable constructs stay on the event engine | - | Postponed |
| DP-25 | P2: DP-P5 one device under the ABI, no foreign fusion | - | Postponed |
| DP-26 | P2: DP-P5 byte-identical to the event engine oracle | - | Postponed |
| DP-27 | P2: DP-P5 checkpoint/rewind of the flat state | - | Postponed |
| DP-28 | P2: DP-P5 Verilator ratio gates (50x / 100x) | - | Postponed |
| DP-29 | P3: DP-P6 per-domain mode, forced modes, identical output | - | Postponed |
| DP-30 | P3: DP-P6 chosen mode within 1.1x of the faster | - | Postponed |

**Coverage:** 30 total, 0 mapped to tasks (postponed).

## Sources

- `.specs/features/digital-performance/diagnosis.md` - the measured diagnosis: findings 1-4, the
  numbers of §2.1/§2.2 every gate derives from, the levers L1-L7 and the reason L7 is deferred.
- `crates/piperine-api/tests/digital_bench.rs` - the bench of record (cases, cycles, calls per cycle).
- `.specs/features/digital-performance/verilator/` - the Verilator twins (`gen.py`, `run.sh`).
- `.specs/features/digital-performance/profile/` - the gdb-sampling profile scripts.
- `docs/spec/part_v_execution.md` - `E0501` (the microstep loop bound) and the instant loop.
- User decisions of 2026-11-22 quoted above.

## Success Criteria

- [ ] A 20 M-cycle digital-only run completes under a 2 GiB cap.
- [ ] Every bench gate prints `PASS` on the reference machine.
- [ ] The fused and event-engine runs of every digital design are byte-identical.
- [ ] Register-heavy designs run within 50x of Verilator per cycle (aim: 10x, printed, not gated).
