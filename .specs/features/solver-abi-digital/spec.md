# Solver ABI Digital Specification

**Approved**: 2026-11-15 - user: "Ok, deixo tu ir tocando, pode ir disparando implementações em seguida ok?"

One of four features that make the solver adopt the device ABI (`.specs/features/device-abi/abi.md`, the authority): `solver-abi-core`, then `solver-abi-digital` (this one) and `solver-abi-analog` in parallel, then `solver-abi-integration`. This feature edits `crates/piperine-digital-solver` only, so it runs in its own worktree beside `solver-abi-analog`.

## Problem Statement

The digital sub-solver is a Verilog-shaped engine: four-state `LogicValue`, a strength table, a two-phase `seq`/`comb` contract and a net snapshot the analog side reads. MSL and the device ABI have none of these. The ABI gives the discrete side byte pins, pure functions, reactions and commits, and the user asked for propagation "estilo Verilator, onde a propagação é executada em DAG, até convergir ou falhar". That engine does not exist. This feature builds it inside `piperine-digital-solver`, standalone and unit-tested, beside the old engine; `solver-abi-integration` then switches the owner to it and deletes the old one.

## Goals

- [ ] `piperine-digital-solver` runs any set of bound device instances through the discrete rules of abi.md (§3.1 P1 to P7, §4 F1 to F4, §5 R1 to R6, §6 C1 to C6, §9 D1) with no analog type in its interface.
- [ ] Propagation is a levelized DAG over functions, repeated to a fixed point, with a bounded failure that names the nets.
- [ ] The engine can be checkpointed and restored with no device call, and advanced to the first instant that touches the analog side without running it.
- [ ] The crate's old engine and its 27 unit tests are untouched and green.
- [ ] No file outside `crates/piperine-digital-solver` changes, and the engine's facade fits the `DiscreteSide` trait of `solver-abi-core` through a thin adapter.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Deleting the strength table, `LogicValue`, the `seq`/`comb` contract and the shared snapshot | the owner still runs on them; `solver-abi-integration` deletes them in the commit that removes their last user |
| The owner protocol (landing, replay after a rejected step, the host sink) | `solver-abi-core` wrote it on stubs; this feature provides the real `checkpoint`, `restore`, `advance`, `preview_boundary` and `run_instant` |
| Implementing `DiscreteSide` for `Digital`, and any edit to `piperine-solver` | `solver-abi-integration`: the trait lives in the owner, which this crate cannot depend on |
| Analog functions, terms, inputs, watches | `solver-abi-analog` (parallel) |
| `force` and `release` (abi.md P8), host drives of ports, `E0806` | `solver-host-seam` (SCH-01, SCH-02, SCH-04) |
| The settle commit (a commit made on `SETTLE` is due at the next time point, §15 S1 to S6) | `solver-time-point` (SCT-05 AC 2, 3); here such a commit fails loud (SAD-12) |
| Exact reachability for `HORIZON` (abi.md R5) | a conservative horizon is always sound (SAD-17); the tight one is an optimization |
| Speed (a compiled or fused kernel) | this engine is the reference, as `solver-completeness/context.md` "Deferred Ideas" says |
| Threads | abi.md §12 allows them; nothing here needs them |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| The vision | the digital side is its own solver; the boundary is events; propagation is a levelized DAG to a fixed point or a failure | user, round 1: "Todo o path depois do evento de borda é feito estilo Verilator, onde a propagação é executada em DAG, até convergir ou falhar na conversão" | y |
| Values | opaque bytes; a change is a bytewise difference | user: "Prefiro bytes também" | y |
| What the crate depends on | `piperine-abi` only | MD-61 (proposed by `solver-abi-core`); MD-60: the sub-solvers never see each other | n - follows MD-61 |
| Where the store and the queue live | in this crate; the owner holds one `Digital` value and asks it to checkpoint, restore, advance and run an instant | abi.md §5: the checkpoint is "discrete store, queue, sink buffer"; user: "o dono dos eventos e dispatch para sub solvers" | y - agent's discretion (structure) |
| Resolved nets | no `resolve` call in the solver: the elaborator lowers a resolved carrier to a resolver instance whose drive pins are `RELAX` (abi.md D1). SCI-06 of the umbrella is met by the relaxation rule | abi.md §9 D1 | y - settled by the ABI |
| D1's log record | when a relaxed field with a known byte still moves, it is zeroed and one observation naming the net is emitted. abi.md says "one record goes to the resolver's log pin" and gives no way to name that pin in a shape | the shape has no flag for it; an observation loses nothing | y - agent's discretion; reported as an ABI gap |
| `HORIZON` | the tick before the earliest pending commit, capped by a limit the owner sets | a smaller horizon is always sound (R5: "may run its own clock through `HORIZON`") | y - agent's discretion |
| Defaults | tick 1 fs; 1000 microsteps per instant; relaxation passes = `RELAX` nets in the cycle + 1 | `docs/spec/appendix_c_implementation.md` C.2.1; abi.md D1 | y - settled by the spec |
| Error codes | `E0501` (an instant does not go quiet), `E0502` (tick range), `E0435` (NaN delay) | `docs/spec/part_v_execution.md` §15 "Errors"; umbrella SCI-02, SCE-02 | y - settled by the spec |
| Who models, who verifies | Sonnet implements and Opus verifies by default; the orchestrator may have Opus implement (a different Opus verifies). This feature: 13 tasks, protocol-delicate (ordering, transport, look-ahead), not numerically delicate | user, 2026-11-15, through the coordinator | y |
| Parallel work | the feature touches `crates/piperine-digital-solver` only; its boundary guard is a test of this crate, not an edit of the owner's guard target | user, 2026-11-15: digital and analog "runnable in parallel in separate worktrees (neither touches the other's crate)" | y |
| Proof vehicle | unit tests on hand-written `Device`s (flip-flop, gates, counter with a timer, a resolver, an emulator stub) | user: unit tests only | y |

**Open questions:** none - all resolved or logged above.

**Implicit-requirement dimensions.** Input validation & bounds: SAD-02, SAD-03, SAD-05, SAD-10. Failure / partial failure: SAD-11, SAD-13, SAD-18. Idempotency / retry: SAD-14 (restore then replay gives the same result). Auth and rate limits: N/A because it is an in-process library. Concurrency / ordering: SAD-08, SAD-09, SAD-14. Data lifecycle: SAD-12 (start), SAD-14 (checkpoint). Observability: SAD-20. External-dependency failure: SAD-13 (a device that faults). State-transition integrity: SAD-15, SAD-16.

---

## User Stories

### P1: Byte nets, a queue and ticks ⭐ MVP

**User Story**: As a codegen author, I want every discrete value to be bytes on a one-writer net and every scheduled time to be an integer tick, so that discrete execution is exact and knows no language type.

**Why P1**: every other rule reads or writes the store and the queue.

**Acceptance Criteria**:

1. The crate SHALL depend on `piperine-abi` and on no other workspace crate, and its new modules SHALL name no analog type.
2. The engine SHALL keep every scheduled time as a signed 64-bit count of ticks, with the tick length a session option whose default is 1 fs.
3. WHEN a commit is made with a positive finite delay THEN it SHALL be due at the tick nearest `t + after`.
4. IF a scheduled time falls outside the tick range THEN the engine SHALL fail with `E0502`.
5. IF a commit is made with a NaN delay THEN the engine SHALL fail with `E0435`.
6. WHEN a commit is made with a delay of `+inf` THEN it SHALL never come due.
7. The engine SHALL store each discrete net as `layout.size` bytes that start all zero, and SHALL treat a net as changed exactly when its bytes differ after a write.
8. WHEN a net changes THEN its `PREVIOUS` part SHALL hold the bytes it had before, and its `CHANGED` part SHALL read 1 in that microstep and 0 in the next.
9. IF two instances write one net, or two pins on one net have unequal layouts THEN bind SHALL fail naming both pins.
10. IF the discrete functions form a cycle that passes through no `RELAX` net THEN bind SHALL fail naming the functions of the cycle.

**Independent Test**: bind two flip-flops and an inverter by hand, read every net as zero bytes, write one, and see `PREVIOUS` and `CHANGED` follow.

---

### P1: Functions over a levelized DAG, reactions and commits ⭐ MVP

**User Story**: As a codegen author, I want functions run in dependency order until nothing changes and reactions run on one snapshot with ordered commits, so that every `digital` block of MSL has one deterministic execution.

**Why P1**: it is the logic change the user asked for.

**Acceptance Criteria**:

1. WHEN a pin that a discrete function reads changed in a microstep THEN the engine SHALL run that function in that microstep, after every function that writes a pin it reads.
2. WHILE no pin a discrete function reads has changed, the engine SHALL NOT call it, except once at the first instant.
3. WHEN functions form a cycle through `RELAX` nets THEN the engine SHALL start those nets at zero bytes and run the cycle's functions in order until no byte changes, at most (`RELAX` nets in the cycle + 1) passes.
4. IF a field with a known byte still moves after the last relaxation pass THEN the engine SHALL zero it and emit one observation naming the net.
5. IF a field without a known byte still moves after the last relaxation pass THEN the engine SHALL fail naming the net.
6. WHEN an event pin a reaction listens to fired in a microstep THEN the engine SHALL run that reaction once in that microstep.
7. WHILE the reactions of one microstep run, each SHALL read the pin values settled before the first of them ran.
8. WHEN a reaction commits with `after ≤ 0`, or with a delay that rounds to no advance THEN the commit SHALL apply at the next microstep.
9. WHEN several commits to overlapping bytes apply in one microstep THEN they SHALL apply in the order (instant made, statement index, call order) and the last SHALL be the value.
10. WHEN a statement commits THEN the pending commits of the same statement and byte range due at or after the new one SHALL be removed, and no commit of another statement.
11. WHEN a `REPLACE` statement commits THEN every pending commit of that statement SHALL be removed.
12. IF a commit's offset and length fall outside its statement's declared range THEN the engine SHALL fail with a fault naming the instance, the reaction and the statement.
13. IF a `LATER` statement commits with a delay below its `min_after` THEN the engine SHALL fail with a fault naming the statement.
14. WHEN a `LATER` statement's `min_after` is below one tick THEN bind SHALL drop the flag and report no error.
15. IF an instant does not go quiet within the session's bound (default 1000 microsteps) THEN the engine SHALL fail with `E0501` naming the nets still changing.
16. WHEN a commit is made to a `QUIET` net and no further microstep would run THEN it SHALL apply as the instant exits and SHALL NOT start a microstep.
17. WHEN an analysis starts from initial values THEN the engine SHALL zero the store, drop the queue, commit every `init` at `(0, 0)` and fire `START`.
18. IF a reaction that ran because `SETTLE` fired commits with no advancing delay THEN the engine SHALL fail with an error that says the settle commit is not supported yet.
19. IF a function or reaction returns `FAULT` THEN the engine SHALL fail with the device's code and message, the tick and the instance path, as typed fields.
20. IF a function outside the parameter cone returns `RESHAPE` THEN the engine SHALL fail as for a `FAULT`.
21. WHEN a reaction returns `FINISH` THEN the instant's outcome SHALL be marked final.

**Independent Test**: the flip-flop of abi.md §11.4 clocked by a timer device reproduces the trace of `docs/spec/part_v_execution.md` §15.1; a ring of three inverters written as functions is refused at bind; the same ring through a resolver with `RELAX` pins relaxes.

---

### P1: Checkpoint, advance and the time-point test ⭐ MVP

**User Story**: As the owner of events, I want to run discrete instants ahead of analog time, stop before the first one that touches the analog side and undo everything by restoring a checkpoint, so that mixed-signal coupling needs one solve per landing.

**Why P1**: it is the digital half of the synchronization protocol of abi.md §5.

**Acceptance Criteria**:

1. WHEN the engine is restored to a checkpoint THEN its store, queue and observation buffer SHALL equal, byte for byte, their state when the checkpoint was taken, and no device SHALL be called.
2. WHEN the same plan, initial values and host commits are run twice THEN the sequence of observations SHALL be identical.
3. WHEN the engine is asked to advance to a tick THEN it SHALL run the instants up to that tick in order and stop before the first instant that is a time point or that fires an `IRREVERSIBLE` reaction, leaving that instant untouched and reporting whether it is a time point.
4. The engine SHALL treat an instant as a time point when, after its commits and functions apply, a net flagged as read by the analog side changed, or a reaction fires that reads a continuous pin, or a reaction fires that has a statement without `LATER` whose pin reaches one of those through reads and writes.
5. WHEN an instant changes nothing that reaches the analog side THEN `advance` SHALL run it and SHALL NOT stop there.
6. WHEN the owner runs an instant with a vector of samples THEN every reaction that reads a continuous pin SHALL read its sample, and the outcome SHALL list the analog-read nets that changed with their bytes, whether a host write to a `PARAM` or `GLOBAL` net applied, and whether the instant is final.
7. WHEN a reaction reads `TICK` THEN it SHALL read the instant's tick, and `HORIZON` SHALL be no later than the tick before the earliest pending commit and no later than the limit the owner set.
8. IF a commit is due later than the tick the engine is advanced to THEN it SHALL stay pending.
9. WHEN two commits are due at times that round to the same tick THEN they SHALL apply in one instant.
10. ~~WHEN the owner asks for the boundary values of a landing instant THEN the engine SHALL return ... computed on a copy~~ SUPERSEDED (user decision, 2026-11-15, solver-completeness context.md "Orchestrator note": "ABI order: solve, then react (Recommended)"): at a scheduled instant the owner solves at t on the left value first, then runs the instant; there is no microstep-0 preview and `preview_boundary` is removed from the seam and from this engine.

**Independent Test**: a divider clock that no analog-read net depends on runs 1000 edges inside one `advance`; the same clock driving a net flagged analog-read stops `advance` before its first edge.

---

### P2: Host writes, the parameter cone and observations

**User Story**: As a host author, I want a write to a param net checked on a shadow before it is committed, and every change, commit and record delivered in order, so that a bad `set` changes nothing and a design can be debugged from one stream.

**Why P2**: the owner needs both, but a design runs without them.

**Acceptance Criteria**:

1. WHEN the host writes a net at a tick THEN the write SHALL be a commit by the host due at microstep 0 of that tick.
2. IF the host writes a `STRUCTURAL` param net while a run is in progress THEN the engine SHALL refuse the write and change nothing.
3. WHEN the host writes a `PARAM` or `GLOBAL` net THEN the engine SHALL first run the parameter cone downstream of that net on a copy of the store.
4. IF a `check` pin of the written param is false on the copy THEN the engine SHALL refuse the write, name the pin and change nothing.
5. IF a cone function returns `RESHAPE` on the copy THEN the engine SHALL report the instance and change nothing.
6. The engine SHALL emit one observation for every applied change, every commit made (instance, reaction, statement, instant made, instant due) and every commit to a `RECORD` net, in (instant, instance path, statement index) order.
7. WHEN the engine is restored THEN the observations emitted after the checkpoint SHALL be gone.

**Independent Test**: a param with `where r > 0` refuses `set r = -1` and the next read still returns the old bytes; a loop that emits eight records through one statement delivers eight observations.

---

## Edge Cases

- WHEN a plan has no function and no reaction THEN `advance` SHALL return at once and `run_instant` SHALL apply due host commits only.
- WHEN a net has no reader THEN a commit to it SHALL still apply and be observed.
- IF a pin with role `GLOBAL` and without `OPTIONAL` is bound to nothing THEN bind SHALL fail naming the pin.
- WHEN an `OPTIONAL` pin is bound to nothing THEN it SHALL read as zero bytes.
- IF a function reads `CHANGED` of a net in the microstep after the change THEN it SHALL read 0.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| SAD-01 | P1: Nets, AC 1 (crate boundary) | Tasks | In Tasks |
| SAD-02 | P1: Nets, AC 2, 3, 4 (ticks, `E0502`) | Tasks | In Tasks |
| SAD-03 | P1: Nets, AC 5, 6 (`E0435`, `+inf`) | Tasks | In Tasks |
| SAD-04 | P1: Nets, AC 7, 8 (byte store, `PREVIOUS`, `CHANGED`) | Tasks | In Tasks |
| SAD-05 | P1: Nets, AC 9, 10 (bind errors) | Tasks | In Tasks |
| SAD-06 | P1: Functions, AC 1, 2 (levelized, run on change) | Tasks | In Tasks |
| SAD-07 | P1: Functions, AC 3, 4, 5 (relaxation) | Tasks | In Tasks |
| SAD-08 | P1: Functions, AC 6, 7 (reactions, snapshot) | Tasks | In Tasks |
| SAD-09 | P1: Functions, AC 8 to 11 (commit time, order, transport) | Tasks | In Tasks |
| SAD-10 | P1: Functions, AC 12, 13, 14 (statement range, `LATER`) | Tasks | In Tasks |
| SAD-11 | P1: Functions, AC 15, 16 (`E0501`, `QUIET`) | Tasks | In Tasks |
| SAD-12 | P1: Functions, AC 17, 18 (start, settle commit refused) | Tasks | In Tasks |
| SAD-13 | P1: Functions, AC 19, 20, 21 (faults, `FINISH`) | Tasks | In Tasks |
| SAD-14 | P1: Checkpoint, AC 1, 2 (restore, determinism) | Tasks | In Tasks |
| SAD-15 | P1: Checkpoint, AC 3, 4, 5, 8, 9 (advance, time-point test) | Tasks | In Tasks |
| SAD-16 | P1: Checkpoint, AC 6 (run an instant with samples) | Tasks | In Tasks |
| SAD-17 | P1: Checkpoint, AC 7 (`TICK`, `HORIZON`) | Tasks | In Tasks |
| SAD-18 | P2: Host, AC 3, 4, 5 (cone, shadow) | Tasks | In Tasks |
| SAD-19 | P2: Host, AC 1, 2 (host commit, structural) | Tasks | In Tasks |
| SAD-20 | P2: Host, AC 6, 7 (observations) | Tasks | In Tasks |
| SAD-21 | Goals: the old engine and its tests are untouched | Tasks | In Tasks |
| SAD-22 | P1: Checkpoint, AC 10 (boundary preview of a landing) | superseded (see AC 10) | Superseded |

**Coverage:** 22 total, 22 mapped to tasks, 0 unmapped. Umbrella requirements met here: SCI-01 to SCI-05, SCI-07 to SCI-09, SCE-02, and the digital half of SCE-01, SCE-03, SCE-04; SCI-06 is met by D1 (see Assumptions).

---

## Sources

- `.specs/features/device-abi/abi.md`: §3.1 (pins, P1 to P7), §4 (F1 to F5, cells), §5 (R1 to R7 and the synchronization protocol), §6 (C1 to C6), §9 (lifecycle, D1), §10 (call sequences).
- `.specs/features/solver-completeness/spec.md`: "P1: The event boundary and the digital DAG" (AC 3, 5, 7 to 19); `context.md` rounds 1 and 2.
- `docs/spec/part_v_execution.md` §15 (ticks, the instant, errors), §15.1 (the traced instant); `appendix_c_implementation.md` C.2.1 (defaults).
- `.specs/STATE.md` MD-60, MD-61 (proposed).
- `.specs/features/solver-abi-core/design.md`, component `protocol`: the `DiscreteSide` trait this engine's facade must fit.
- The coordinator's message of 2026-11-15 (user): parallel worktrees, model roles.
- `crates/piperine-digital-solver/src/` as it stands: what must stay green until `solver-abi-integration`.

---

## Success Criteria

- [ ] `cargo test -p piperine-digital-solver` is green with zero warnings: the 27 old tests and the new ones.
- [ ] The trace of `docs/spec/part_v_execution.md` §15.1 is reproduced by hand-written devices.
- [ ] No new source file of the crate names `Netlist`, `Stamp`, `AnalogReference` or any item of `piperine-analog-solver`.
