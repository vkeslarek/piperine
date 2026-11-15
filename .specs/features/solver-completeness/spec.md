# Solver Completeness Specification

Umbrella. Each feature has its own directory with spec, design and tasks; this document holds the shared goal, the user's decisions and the requirement map. `solver-restructure` (feature A) is done. Feature B, `solver-event-boundary`, was never planned under that name: the device ABI (`.specs/features/device-abi/abi.md`, 2026-11-15) replaced it with five features, `spec-abi-amendments`, `solver-abi-core`, `solver-abi-digital`, `solver-abi-analog` and `solver-abi-integration`. The section "After the device ABI" says what they take and what is left for `solver-time-point`, `solver-crossings`, `solver-host-seam` and `solver-numerics`.

## Problem Statement

The MSL frontend (spec, parser, POM) is done and the solver is the only other crate that builds. The elaborator and codegen that will join them do not exist yet. Before they are written, the solver must offer an engine for every dynamic rule of the MSL specification, and nothing else. Today it lacks the MSL discrete model (integer ticks, typed carriers with `resolve`, a clean analog/digital boundary, settle commits, located crossings), it carries about 3.4k lines that serve only Verilog-AMS, and none of its tests can run because they depend on the dead frontend.

Evidence, row by row: `analysis.md` (68 rules traced: 15 present, 31 partial, 22 missing; 14 surplus items).

The user's vision (2026-11-14): the solver is the owner of events and dispatches to two sub-solvers, analog and digital, "praticamente como dois solvers distintos que rodam no mesmo solver". The boundary between them is events, always. Past a boundary event the digital side propagates Verilator-style over a levelized DAG until it converges or fails.

## Goals

- [ ] The solver is three crates: the event owner, the analog sub-solver and the digital sub-solver, and the two sub-solvers exchange nothing but events.
- [ ] Each solver crate builds and passes its own unit tests with no other crate of the tree.
- [ ] Every row of `analysis.md` section 2 except rows 67 and 68 is `present`, as amended by the decisions below (rows 11, 12 and 42 keep the solver's present behavior and the spec is amended to it).
- [ ] The surplus of `analysis.md` section 3 is gone.
- [ ] The solver stays an engine: it evaluates no model expression, knows no MSL type, formats no output and reads no file. Its complexity is physical, not linguistic.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| The elaborator, codegen and the connection of the solver to `piperine-lang` | planned separately, after a discussion about codegen (user, round 2) |
| Integration tests and ngspice parity runs | user: "por enquanto vamos ter que nos contentar com unitários pois integrado só vai existir depois que conectarmos o solver ao lang" |
| Rebuilding `piperine-api` as a solver-backed surface (`Session` over Rust-built circuits, results, waveform, units) | accepted by the user as the target; its own feature after `solver-restructure` |
| `piperine-cli`, `piperine-lang-server` | stay in the tree; user: "ambos vão ter sua própria feature" |
| Everything in `analysis.md` section 4 | belongs to the language, the elaborator, codegen or the host |
| Sensitivities along a trajectory and across events (§23.3; row 67) | a later feature once a design runs end to end |
| Periodic small-signal and periodic noise (§16.3; row 68) | the spec lists it as unimplemented and untraced (G.37) |
| The C interface of `extern "cosim"` (G.65) | open in the spec |
| The jump solve of Appendix C.2 as a new algorithm, and a change of the residual test's tolerance mapping | user: the present algorithm and the ngspice-parity mapping stay; the spec is amended instead |
| Moving `tf`, `pz`, `sp`, `sens`, `adjoint`, `pss` to a host crate | user: "Sim, por enquanto beleza" (they stay in the solver) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Where the discrete engine lives (Q1) | the solver owns events and dispatches to an analog and a digital sub-solver; three crates, each with its exclusive code | user: "pode ser modelado como "solver" o dono dos eventos e dispatch para sub solvers analogicos e digitais. Se quiser dividir em 3 crates, o dono dos eventos, o digital-solver e o analog-solver cada um com seu código exclusivo" | y |
| The analog/digital boundary | events only, always; no shared state, no snapshot of the other side | user: "A fronteira é via eventos, sempre... Separar em uma fronteira BEM delimitada o que ocorre no analog e o que ocorre no digital" | y |
| Digital propagation | Verilator-style: levelized DAG evaluation after a boundary event, repeated until it converges or fails | user: "Todo o path depois do evento de borda é feito estilo Verilator, onde a propagação é executada em DAG, até convergir ou falhar" | y |
| How the solver stores a carrier value (Q2) | opaque fixed-size bytes, laid out by codegen; a change is a bytewise difference | user: "Prefiro bytes também. Solver é o core do negócio e tem que ser mais simples, não ter a complexidade da linguagem e sim a complexidade física" | y |
| Removal of the Verilog-AMS surface and its tests (Q3) | removed | user: "Ok, o que precisar apagar, pode apagar, aproveitando que estamos fazendo um makeover dele" | y |
| Constraint, margin and coverage channels (Q4) | removed; one event/record sink replaces them and is a first-class debug surface | user: "Concordo e na verdade o sink de eventos é bem importante para debug" | y |
| Host-built analyses in the solver (Q5) | `disto` deleted; `tf`, `pz`, `sp`, `sens`, `adjoint`, `pss` stay | user: "Sim, por enquanto beleza" | y |
| Tests that depend on `piperine-codegen` (Q6) | rewritten on hand-written `Element`s, same assertions | user: "Fio, verilog vai embora" | y |
| The jump solve (Q7) | not built. The solver keeps its present discontinuity handling (land on the event time, apply, solve the step with the new values, restart the integrator); Appendix C.2 is amended to describe it | user: "eu manteria o algoritmo atual do solver que funcionou bem nos testes, inclusive cross validation e convergence com descontinuidades" | y |
| Which digital events cut an analog step (Q8) | only an event that crosses the boundary, that is, changes something analog reads. Purely digital activity never forces an analog solve. Where analog code exists it is solved at every analog time step | user: "o solver analógico roda em todo timestep... se existir qualquer código analog, tem que rodar ele"; round 2: "Q8. Ok" | y |
| The residual test's tolerances (Q9) | the present mapping stays (ngspice parity); §17.1.2 is amended to match | user: "a paridade com ngspice é um strenght bem alto e um ponto positivissimo"; round 2: "mantém Q7 e Q9" | y |
| Crates removed (Q10) | `piperine-ir`, `piperine-codegen`, `piperine-python`, `piperine-plugin`, `piperine-plugin-macros`; the root package's VAMS end-to-end tests; `examples/*.vams` | user: "Vamos remover o IR, cranelift e Python por agora. Vamos deixar apenas a API testável em RUST, solver, linguagem, apenas o core"; round 2: "plugin pro enquanto vai embora" | y |
| Crates kept | `piperine-lang`, `piperine-project`, the solver crates, `piperine-api` (to be reduced), `piperine-cli`, `piperine-lang-server` | user: "lang-server é bom ficar... cli também... api aceito" | y |
| The `Element` ABI | may change freely in this refactor | user: "fica a vontade para mudar o ABI também ok?" | y |
| Feature split and who builds what | (A) `solver-restructure`, structure only, done. (B) replaced on 2026-11-15 by `spec-abi-amendments`, then `solver-abi-core`, then `solver-abi-digital` and `solver-abi-analog` in parallel worktrees, then `solver-abi-integration`. Sonnet implements and Opus verifies by default; a feature that is delicate and not large is implemented by Opus and verified by a different Opus, as the orchestrator decides (user, 2026-11-15, superseding the round-2 roles). Then `solver-time-point`, `solver-crossings`, `solver-host-seam`, `solver-numerics`, each reduced as listed in "After the device ABI" | user: "Core cleanup + solver-split poderia ser uma feature só pois é refactor de estrtura. Digital dag + analog split uma feature separada pois quero que mexer no core core (mudança de logica) seja implementada por opus e verificada com fable" | y |
| Proof vehicle | unit tests on hand-written Rust `Element`s, per crate | user: unit tests only for now | y |
| Spec amendments | §15.2 (the jump), Appendix C.2 (the rows "the jump" and "restart", and G.6's jump row) and §17.1.2 are rewritten to describe the solver's present behavior. They land in `spec-abi-amendments`, with the seven accepted amendments of the device ABI, each with its `spec_text` guard (MD-57) | follows from Q7 and Q9; user, round 3: "Amend §15.2 to the solver" | y |
| Numeric defaults | Appendix C.2.1 | the spec's reference values; most are the code's already | y - settled by the spec |

**Open questions:** none. The one that was open is answered (user, round 3: "Amend §15.2 to the solver"): `spec-abi-amendments` amends §15.2 itself, together with Appendix C.2 and §17.1.2, to describe the kept algorithm. The value a jump produces is then defined one analog step after the event, not at the same `t`.

**Implicit-requirement dimensions.** Input validation & bounds: SCI-02, SCX-03, SCH-04. Failure / partial failure: SCI-07, SCT-07, SCN-02. Idempotency / retry: SCT-09. Auth and rate limits: N/A because the solver is an in-process library. Concurrency / ordering: SCI-04, SCI-05, SCI-09. Data lifecycle: SCT-11, SCT-12. Observability: SCE-04, SCH-05. External-dependency failure: SCH-07, otherwise N/A because the solver calls no service. State-transition integrity: SCE-01 to SCE-03, SCT-05.

---

## User Stories

Roles: the **codegen author** writes the elaborator and device code that will target the solver. The **host author** writes `piperine-api`.

### P1: Restructure: a clean core in three crates ⭐ MVP

**User Story**: As a codegen author, I want the solver cleaned of everything MSL lacks and split into the event owner, the analog sub-solver and the digital sub-solver, each testable alone, so that the logic work that follows lands on a small, proven structure.

**Why P1**: Nothing can be verified until the suite runs, and the split is the frame of the user's vision. Feature (A) `solver-restructure`; structure only, no logic change. Full criteria: `.specs/features/solver-restructure/spec.md`.

**Acceptance Criteria**:

1. WHEN the unit tests of each solver crate run on a tree where no other crate compiles THEN each crate SHALL build and pass.
2. The analog and the digital sub-solver crates SHALL depend on no crate of this workspace.
3. The workspace SHALL contain none of `piperine-ir`, `piperine-codegen`, `piperine-python`, `piperine-plugin`, `piperine-plugin-macros`.
4. WHEN a test that passed before the restructure, and whose subject was not removed, runs afterwards THEN it SHALL pass with its assertions unchanged.

**Independent Test**: `cargo test` on each of the three solver crates is green; the numeric baseline target (`parity_baseline`) passes untouched.

---

### P1: The event boundary and the digital DAG ⭐ MVP

**User Story**: As a codegen author, I want the digital sub-solver to hold typed carriers and propagate over a levelized DAG, and the analog and digital sides to meet only through events owned by the solver, so that every `digital` block of MSL has deterministic execution and mixed-signal coupling has one seam.

**Why P1**: It is the core logic change. It is delivered by the `solver-abi-*` features on the device ABI, where a carrier is a discrete pin, a drive and a `resolve` are a resolver instance with `RELAX` pins (abi.md D1; criteria 10 and 11 are met by relaxation, with no `resolve` call in the solver), and an element is a `Device`.

**Acceptance Criteria**:

1. The analog sub-solver's interface SHALL name no digital type, and the digital sub-solver's interface SHALL name no analog type.
2. WHEN the analog side must tell the digital side something (a crossing, an accepted point) or the reverse (a carrier that analog reads changed) THEN it SHALL be one event delivered through the owner, and nothing else SHALL cross.
3. WHEN a boundary event reaches the digital sub-solver THEN it SHALL evaluate its elements in levelized DAG order and repeat until no value changes, or fail.
4. The owner SHALL make every boundary event observable through one sink, with its time, source and payload.
5. WHEN a digital event changes nothing that an analog element reads THEN the analog sub-solver SHALL NOT end a step at its time.
6. WHEN a digital event changes something an analog element reads THEN the analog sub-solver SHALL land on its time, solve the step with the new value and restart its integrator, as the solver does today for a breakpoint.
7. The digital sub-solver SHALL keep every scheduled time as a signed 64-bit count of ticks, with the tick length a session option whose default is 1 fs.
8. IF a scheduled time falls outside the tick range THEN the solver SHALL fail the analysis with `E0502`.
9. The digital sub-solver SHALL store each carrier and each drive as a fixed number of bytes given at build time, and SHALL treat a carrier as changed exactly when its bytes differ.
10. WHEN any drive of a resolved carrier changes THEN the digital sub-solver SHALL call that carrier's resolve function with all its drives in build order and store the result.
11. WHEN an element asks for the value of a resolved carrier without its own drive THEN the digital sub-solver SHALL return the resolve of every other drive.
12. WHEN several commits to one level of one drive apply at one instant THEN they SHALL apply in the order (instant made, source index, execution index) and the last SHALL be the value.
13. WHEN a statement commits to a level THEN the pending commits of the same statement to that level due at or after the new one SHALL be removed, and no commit of another statement.
14. WHILE one propagation pass runs, every reaction SHALL read the values settled at its start.
15. IF propagation at one time does not converge within the session's bound (default 1000 passes) THEN the solver SHALL fail the analysis with `E0501` naming the carriers still changing.
16. IF a commit is made with a NaN delay THEN the solver SHALL fail the analysis with `E0435`; a delay of `+inf` SHALL never come due.
17. WHEN the same elements, initial values and host actions are run twice THEN the sequence of (time, carrier, value) changes SHALL be identical.
18. WHEN a scheduled event is armed with a tick THEN it SHALL fire once at that tick, or at the next pass when the tick equals the current one, and never when the tick is in the past.
19. IF an element reports a run error while it evaluates THEN the solver SHALL fail the analysis with that error's code, the time and the element's instance name, as typed fields.

**Independent Test**: a purely discrete design of hand-written elements reproduces the trace of §15.1; a divider clock that no analog element reads adds no analog time point; a `Switch` driven by a carrier makes the analog side land on the change.

---

### P2: The time point

**User Story**: As a codegen author, I want settle, operating-point rounds, held nodes, sweep points and continuation as §15 and §16 state them, on the kept discontinuity algorithm, so that stdlib modules such as `transition` and `zi_nd` run.

**Why P2**: Built on (B). Feature `solver-time-point`.

**Acceptance Criteria**:

1. WHEN propagation at an analog time point has converged THEN the solver SHALL run every settle reaction once, with the final discrete state and the final solution.
2. WHEN a settle reaction commits with no advancing delay THEN the commit SHALL be due at the next analog time point, and the solve that arrives there SHALL already read its value.
3. IF the step to the next time point is rejected THEN a pending settle commit SHALL move to the new next time point.
4. IF the rounds of an operating point exceed the session's bound (default 20) THEN the solver SHALL fail the analysis with `E0506` naming the carriers still changing.
5. WHEN a transient step is rejected THEN carriers, pending commits, armed events, element state and the analog history SHALL equal their values before the step.
6. WHILE the operating point of a transient is solved, every node given a held value at build time SHALL be tied to it by a potential source to ground, removed before the first step.
7. WHEN a `Dc` analysis runs several sweep points THEN the discrete state SHALL carry from one point to the next, initial values SHALL be committed at the first point only, and settle reactions SHALL run once per point.
8. WHEN an analysis without time ends THEN every commit and scheduled event that was waiting for a later time SHALL be dropped.
9. WHEN a transient is continued to a later stop time THEN time, the event queue, carriers, element state and the analog history SHALL be those of its last instant.
10. WHEN a `Dc`, `Ac` or `Noise` analysis runs at a transient's last instant THEN elements SHALL read that instant as the current time and the queue SHALL be unchanged afterwards.
11. WHILE any solve runs, an element SHALL be able to read the analysis kind (`Dc`, `Tran`, `Ac`, `Noise`) and whether the solve is an equilibrium solve.

**Independent Test**: hand-written `SampleHold` and settle-commit elements reproduce the tables of §15; a `Dc` sweep of a Schmitt trigger traces its hysteresis.

---

### P2: Located crossings and `bound_step`

**User Story**: As a codegen author, I want the solver to land on a crossing and to honor a step cap, so that `cross`, `above` and every converter built on them fire at the right time.

**Why P2**: `cross` is the only analog-to-digital path of MSL and fires one step late today. Feature `solver-crossings`.

**Acceptance Criteria**:

1. WHEN a crossing expression goes from `<= 0` to `> 0` inside a step THEN the solver SHALL retake the step until the landing time exceeds the true crossing by at most `ttol` plus one tick, or `0 < e < etol` holds there.
2. WHEN a crossing is located THEN its landing time SHALL be a tick and its event SHALL cross the boundary at that time.
3. IF a crossing is registered with a `ttol` or `etol` that is not finite and positive THEN the solver SHALL fail the analysis with `E0510`.
4. WHEN several crossings fall in one step THEN the solver SHALL land on the earliest, and those that round to the same tick SHALL fire together.
5. WHEN a new round of an operating point or the next sweep point moves a crossing expression from `<= 0` to `> 0` THEN its event SHALL fire.
6. WHEN an element reports a step cap at an accepted solution THEN the next step SHALL be at most the smallest cap reported there.
7. IF an element reports a step cap that is not finite and positive THEN the solver SHALL fail the analysis with `E0509`.

**Independent Test**: a ramp through a threshold lands within `ttol` plus one tick for several step sizes; a sine with a 1 ns cap never takes a longer step.

---

### P2: The host seam

**User Story**: As a host author, I want to drive carriers, force and release, act at scheduled times, receive records and stop on `finish`, so that a testbench of §23.1 can be written over the solver.

**Why P2**: Without it a design runs but cannot be driven or observed as the spec states. Feature `solver-host-seam`.

**Acceptance Criteria**:

1. WHEN the host writes a drive it holds before the run THEN the value SHALL be committed at time 0 as an initial value.
2. WHEN the host schedules a drive, a `set`, a `force` or a `release` at a time THEN it SHALL enter that time as an event through the owner.
3. WHILE a carrier is forced, its value SHALL be the forced value whatever its drives, and its drives SHALL go on updating underneath.
4. IF the host schedules an action at a time already past THEN the solver SHALL refuse it with `E0806` and change nothing.
5. WHEN the host schedules a probe at a time THEN that time SHALL be an analog time point and the host SHALL receive the solution there.
6. WHEN an element writes a record from a reaction in a step that is later rejected THEN the record SHALL not reach the sink; a record written from an analog evaluation SHALL reach it.
7. WHEN a record flagged `finish` is accepted THEN the analysis SHALL end at its time.
8. The record sink SHALL deliver, with each record, the instance name, the analysis kind, the time, and inside an analog evaluation the Newton iteration.
9. WHEN an element asks for a session parameter by name THEN the solver SHALL return the value the host set, or report that there is none.
10. WHEN a discrete element asks to be run again at a time THEN that time SHALL be scheduled, and an element flagged as never rolled back SHALL run only at times of accepted steps.

**Independent Test**: a hand-written UART receiver is driven bit by bit from a test acting as the host, with a forced reset, and the host receives the `byte` record and a `finish`.

---

### P3: Numeric conformance

**User Story**: As a codegen author, I want tolerances per nature, named singular solves, a per-state truncation test, switch branches and correlated noise, so that the remaining rows of §11, §14 and §17 hold.

**Why P3**: The engine runs without them. Feature `solver-numerics`.

**Acceptance Criteria**:

1. The update and residual tests SHALL use, for each unknown and each row, the absolute tolerance given for it at build time, under the solver's present mapping of tolerances to rows.
2. IF a linear solve is singular THEN the solver SHALL fail with `E0503` naming the unknowns of the rows that make it so.
3. WHERE an element declares a state with its own absolute tolerance, the truncation-error test SHALL measure that state against it.
4. WHEN an element stamps a different set of matrix positions in one solve than in another, both sets having been declared at build time, THEN each solve SHALL factor correctly.
5. WHEN one noise source is injected at several rows with complex weights THEN the output density SHALL be that of one fully correlated source.
6. WHEN a noise source is injected on the row of a potential branch THEN the output density SHALL include it.

**Independent Test**: a floating node reports its name; a resistor split in two with one shared noise instance gives zero noise across the pair.

---

## Edge Cases

- WHEN a design has no analog unknown THEN the solver SHALL run every scheduled digital time and the stop time with no analog solve.
- WHEN a design has no discrete carrier THEN the transient SHALL take the same steps as before these features within tolerance.
- IF a commit is due later than the stop time THEN it SHALL stay pending and SHALL apply only if the transient is continued.
- WHEN two event times round to the same tick THEN they SHALL be one time.
- IF a hold closes a loop of potential sources THEN the solve SHALL fail with `E0503` naming both.

---

## After the device ABI

Decided 2026-11-15 (`.specs/features/codegen/context.md`, "Device ABI decisions"). The device ABI is the element contract of this umbrella: `Element` is replaced, not extended.

**What replaces feature B, in order.**

| Feature | What it delivers | Tasks | Delicate |
| --- | --- | --- | --- |
| `spec-abi-amendments` | `docs/spec` edits: ABI amendments 1, 2, 3a, 4, 6, 8, 9; §15.2, Appendix C.2 and §17.1.2 describe the kept algorithm and tolerances | 11 | no |
| `solver-abi-core` | the crate `piperine-abi` (contract types, `Device`, thunks, shape checks, partials verifier) and the owner's protocol over two traits, proven on stubs | 16 | protocol |
| `solver-abi-digital` (parallel) | the digital sub-solver on the ABI: byte nets, levelized DAG to a fixed point, reactions, commits with transport, checkpoint, look-ahead | 13 | protocol |
| `solver-abi-analog` (parallel) | the analog sub-solver on the ABI: rows from terms and partials, solver-owned companion, limiting, delay, noise, watches | 10 | numeric |
| `solver-abi-integration` | bind, drivers on the assembly, adapters, differential oracle, every surviving test on `Device`, `Element` and the old digital engine removed | 25 | numeric and protocol |

**What remains of the four later features.** Each keeps its name and shrinks to the rows below; its criteria in this document that are not listed are met by the `solver-abi-*` features.

| Feature | Still to build | Requirements |
| --- | --- | --- |
| `solver-time-point` | the settle commit (due at the next time point, moved when a step is rejected; until then a commit made on `SETTLE` with no delay fails loud); discrete state carried across `Dc` sweep points with inits at the first point only; dropping what waits for time when an analysis without time ends; continuing a transient and running `Dc`, `Ac`, `Noise` at its last instant (the re-entry state exists; the rules do not) | SCT-05 AC 2 and 3, SCT-10, SCT-11, SCT-12 |
| `solver-crossings` | locating a crossing inside a step (retake until within `ttol` plus one tick or `etol`), earliest of several, crossings across operating-point rounds and sweep points, `E0510`, `E0509`. The `CROSS` and `CAP` watches, the event pin and the protocol's "landed earlier" path exist; only the locator and the two validations are missing | SCX-01, SCX-02, SCX-03 AC 3 and 7 |
| `solver-host-seam` | `force` and `release` (abi.md P8), host drives of ports before and during a run, scheduled probes as an API, `E0806`. Host commits, the parameter cone, waiters, the sink, records and `FINISH` exist | SCH-01, SCH-02, SCH-03, SCH-04 |
| `solver-numerics` | residual tolerance per row from `Pin.abstol[1]` (the per-unknown half is wired), singular solves that name their unknowns (`E0503`), the truncation test per `ddt` state from `Term.abstol` | SCN-01, SCN-02, SCN-03 |

No jump solve is planned anywhere: Q7 stands, and `spec-abi-amendments` writes the kept algorithm into §15.2.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| SCR-01 | P1: Restructure, AC 1 to 4 (detailed as SR-01 to SR-06 in `solver-restructure`) | solver-restructure | Verified |
| SCE-01 | P1: Event boundary, AC 1, 2 (event-only boundary) | solver-abi-core, -digital, -integration | In Tasks |
| SCE-02 | P1: Event boundary, AC 3, 15 (DAG propagation to convergence or `E0501`) | solver-abi-digital | In Tasks |
| SCE-03 | P1: Event boundary, AC 5, 6 (which events cut an analog step) | solver-abi-core, -digital, -integration | In Tasks |
| SCE-04 | P1: Event boundary, AC 4 (the event sink) | solver-abi-core, -digital, -integration | In Tasks |
| SCI-01 | P1: Event boundary, AC 7 (ticks) | solver-abi-digital | In Tasks |
| SCI-02 | P1: Event boundary, AC 8, 16 (`E0502`, `E0435`) | solver-abi-digital | In Tasks |
| SCI-03 | P1: Event boundary, AC 9 (byte carriers) | solver-abi-digital | In Tasks |
| SCI-04 | P1: Event boundary, AC 12, 13 (commit order, transport) | solver-abi-digital | In Tasks |
| SCI-05 | P1: Event boundary, AC 14 (snapshot) | solver-abi-digital | In Tasks |
| SCI-06 | P1: Event boundary, AC 10, 11 (resolve, others) | solver-abi-digital | In Tasks |
| SCI-07 | P1: Event boundary, AC 19 (typed run errors) | solver-abi-digital | In Tasks |
| SCI-08 | P1: Event boundary, AC 18 (scheduled events) | solver-abi-digital | In Tasks |
| SCI-09 | P1: Event boundary, AC 17 (determinism) | solver-abi-digital | In Tasks |
| SCT-05 | P2: Time point, AC 1, 2, 3 (settle, settle commit) | AC 1: solver-abi-core, -digital; AC 2, 3: solver-time-point | Pending |
| SCT-07 | P2: Time point, AC 4 (`E0506`) | solver-abi-core, -integration | In Tasks |
| SCT-08 | P2: Time point, AC 6 (held nodes) | solver-abi-integration | In Tasks |
| SCT-09 | P2: Time point, AC 5 (rejection leaves no trace) | solver-abi-core, -integration | In Tasks |
| SCT-10 | P2: Time point, AC 7 (sweep points) | solver-time-point | Pending |
| SCT-11 | P2: Time point, AC 8 (what waits for time is dropped) | solver-time-point | Pending |
| SCT-12 | P2: Time point, AC 9, 10 (continuation) | solver-time-point | Pending |
| SCT-13 | P2: Time point, AC 11 (analysis and solve kind) | solver-abi-integration | In Tasks |
| SCX-01 | P2: Crossings, AC 1, 2, 4 (location) | solver-crossings | Pending |
| SCX-02 | P2: Crossings, AC 5 (across rounds and sweep points) | solver-crossings | Pending |
| SCX-03 | P2: Crossings, AC 3, 6, 7 (`E0510`, step cap, `E0509`) | AC 6: solver-abi-analog, -integration; AC 3, 7: solver-crossings | Pending |
| SCH-01 | P2: Host seam, AC 1, 2 (drives and scheduled actions) | solver-host-seam | Pending |
| SCH-02 | P2: Host seam, AC 3 (force, release) | solver-host-seam | Pending |
| SCH-03 | P2: Host seam, AC 5 (scheduled probe) | solver-host-seam | Pending |
| SCH-04 | P2: Host seam, AC 4 (`E0806`) | solver-host-seam | Pending |
| SCH-05 | P2: Host seam, AC 6, 7, 8 (records, `finish`) | solver-abi-core, -digital, -integration | In Tasks |
| SCH-06 | P2: Host seam, AC 9 (session parameters) | solver-abi-integration | In Tasks |
| SCH-07 | P2: Host seam, AC 10 (wake request, no rollback) | solver-abi-core, -digital, -integration | In Tasks |
| SCN-01 | P3: Numerics, AC 1 (tolerances per nature, present mapping) | solver-numerics | Pending |
| SCN-02 | P3: Numerics, AC 2 (`E0503` named) | solver-numerics | Pending |
| SCN-03 | P3: Numerics, AC 3 (truncation per state) | solver-numerics | Pending |
| SCN-04 | P3: Numerics, AC 4 (switch-branch pattern) | solver-abi-analog | In Tasks |
| SCN-05 | P3: Numerics, AC 5, 6 (correlated and potential noise) | solver-abi-analog | In Tasks |

**Coverage:** 37 total. 1 verified (SCR-01). 20 mapped to the tasks of the `solver-abi-*` features (SCE-01 to SCE-04, SCI-01 to SCI-09, SCT-07, SCT-08, SCT-09, SCT-13, SCH-05, SCH-06, SCH-07, SCN-04, SCN-05), plus SCT-05 AC 1 and SCX-03 AC 6. 14 still to plan, in whole or in part: see "After the device ABI".

---

## Sources

- `context.md`, sections "User answers" round 1 and round 2 and the ABI row: every decision above.
- `docs/spec/part_v_execution.md` §15 to §18; `part_iv_behavior.md` §11 to §14; `part_ii_carriers.md` §4, §5; `part_iii_structure.md` §8.2.3, §8.5; `part_vi_io.md` §19, §21; `part_viii_host.md` §23.
- `docs/spec/appendix_c_implementation.md` C.2 (to be amended, see Assumptions) and `appendix_g_scope.md` G.0, G.6.
- `analysis.md`: the traceability matrix, the surplus list, the split.
- `.specs/STATE.md` MD-01, MD-17, MD-18, MD-31, MD-57, MD-60.
- `.specs/features/device-abi/abi.md` and `.specs/features/codegen/context.md` ("Device ABI decisions", 2026-11-15): the contract that replaces `Element` and feature B.
- `.specs/features/{spec-abi-amendments,solver-abi-core,solver-abi-digital,solver-abi-analog,solver-abi-integration}/`: the five features that replace feature B.

---

## Success Criteria

- [ ] `cargo test` on each solver crate is green on a tree where no other crate compiles.
- [ ] The spec's conformance set (C.2.3) can be written as hand-built elements and passes.
- [ ] A design with a free-running digital clock that no analog element reads takes the same number of analog steps with and without the clock.
- [ ] No source file of the analog sub-solver names a digital type, and none of the digital sub-solver names an analog type.
