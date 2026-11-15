# Appendix C. Implementation

This appendix is informative. It describes how the language is lowered to AIR v2 (C.1) and how the Piperine solver meets Part V (C.2).
A conforming implementation meets Parts I–VIII. It need not use AIR, nor the algorithms or numbers given here.

> **Resolution.** The design does not state the status of this appendix. It is informative, because Part V is defined as "what every simulator guarantees" and this appendix as how one implementation meets it.

Where the reference implementation does not yet meet a guarantee, the row says so and cites Appendix G.6.

## C.1 Lowering to AIR

Structure is resolved at elaboration, and every function is inlined. AIR v2 therefore sees flat instances and core primitives only.

### C.1.1 What elaboration removes

| Language construct | Resolved by | Section |
| --- | --- | --- |
| bundles | expansion into their carriers | §7 |
| templates, binning functions | evaluation: a template is a value | §8.4 |
| generics, const generics | monomorphization | §8.1 |
| generation `for` / `if` | unrolling over params and constants | §8.2 |
| `m`, `dtemp` | scaling of contributions and noise; an offset on `temperature()` | §8.5 |
| value inputs, results, named expressions | inlining at each read; their instances are hoisted to the module | §8.2, §8.3 |
| functions and closures | inlining | §3 |
| the standard library | it is ordinary source: lowered like user code | §22 |

### C.1.2 The lowering table

| Language | AIR v2 | Status |
| --- | --- | --- |
| param with `where` | param metadata, checked in `setup` | |
| live `set`, `on p.changed()` | `live` param, `on change` region | |
| known flags | a mask word beside each discrete value, elided where the elaborator proves it all-known | gap (G.6) |
| `signal x: Bit<T> = v;` | digital state, unknown at start, with `v` committed at `(0, 0)` | |
| a carrier with `resolve` | one state slot per drive and a combinational cell calling `resolve` | gap (G.6) |
| `prev()` | one previous value per carrier, allocated on first commit | |
| node, `= v` | internal node; Newton's starting value, and a hold during a transient's operating point | |
| `=` defining a discrete carrier | a combinational cell, evaluated in topological order; a loop among resolved drives is one strongly connected component, relaxed from empty | |
| `=` with `after`, `<-` with `after` | a delayed commit in a transport queue per statement and level | |
| `let` and value inputs | inlined at each read; their instances hoisted to the module | gap (G.6) |
| `analog for M` | `iter` region | |
| `+>` | `+=` contribution, split into resistive and reactive parts | |
| switch branch | a branch whose kind is chosen per solve | gap (G.6) |
| the jump (§15.2) | two backward-Euler solves at the same `t` | gap (G.6) |
| `limit(x, f, seed)` | limited probe with previous-iteration state, seed and RHS correction | gap for user `f` (G.6) |
| reaction, `<-` | sequential `on` region, commit | |
| `settle` | the region of that name; its commits are queued for microstep 0 of the next accepted time point (§15) | |
| `at`, `cross` | scheduled and located events | landing is a gap (G.6) |
| event times as integer ticks (§15) | integer time in the event queue | gap (G.6) |
| `bound_step` | step-control operation | |
| `emit!` | stream emit | |
| `extern "osdi" module` | foreign (OSDI) instance | |
| `extern "cosim" module` | a digital region that calls the foreign step function | |

Notes on single rows:

1. **Known flags.** The mask travels with the value through every operator (§3.3.2). Elision is an optimization: it never changes a result.
2. **Resolved carriers.** The drives are passed to `resolve` in instance-path order (§4 rule 2). The cell is re-evaluated in the microstep any drive changes (§5.2).
3. **Transport queue.** The queue is per statement and per level, because a commit removes only pending commits of the same statement to that level (§12.1.4).
4. **Contributions.** The reactive part of a contribution is what lies under `ddt`. Its argument is a state of the jump rule (§15.2.1).
5. **Switch branches.** When the selector depends on params only, the kind is fixed at elaboration and a zero-potential branch merges its nodes (§11 rule 3). Otherwise the branch keeps its flow unknown and the kind is chosen per solve.
6. **`settle` commits.** The solver reads them before it takes the step, so the solve that arrives at the next time point already uses them (§15).
7. **Records.** An `emit!` record travels as a commit to a quiet pin: it is delivered stamped with the instant it was made and adds no microstep (§19 rule 1).

## C.2 Reference implementation

How the Piperine solver meets Part V. Every number is a default the host may change.

| Part V guarantee | Piperine algorithm | Status |
| --- | --- | --- |
| microstep loop (§15) | two-phase delta cycle: combinational cells in topological order, then sequential evaluation over one snapshot; commits sorted by (instant made, source index); at most 1000 microsteps per instant; relaxation passes per loop of resolved drives bounded by its drive count plus one | |
| the jump (§15.2) | a backward-Euler solve with `h` equal to one tick and time not advanced, started from the left value; repeated once from its own result; the `1/h` rows are scaled by `h` so the matrix stays conditioned | gap (G.6) |
| operating point (§16) | Newton; then gmin stepping (a shunt on every node from 0.1 S down, a decade per converged solve, adaptive factor); then source stepping (scale from 0 to 1, first step 0.1 growing ×1.5 up to 0.25, halved on failure, a 1 µS knee shunt ramped out at the end); at most 20 mixed rounds | |
| held nodes (§16) | a 10¹² S clamp from each node with `= value` to that value through every solve of a transient's operating point, removed before the first step | |
| convergence (§17.1) | `reltol` 1e-3, default potential `abstol` 1 µV, flow `abstol` 1 pA, `gmin` 1e-12 S; an update much larger than the last is damped to the midpoint; stamps may be reused while the inputs move less than the tolerance (SPICE bypass), never while limiting | |
| truncation error (§17.2) | TR-BDF2 (a trapezoidal stage over `γh`, then BDF2 over `(1 − γ)h`, `γ = 2 − √2`); error by Milne's device over node potentials against `trtol` 7; a node whose differences straddle a jump is skipped | |
| step control (§17.2) | PI controller, `h · (1/lte)^p`, `p = kp + ki · (lte − lte_prev)/lte`, `kp` 0.7, `ki` 0.4, growth clamped to \[0.2, 1.5\]; a rejected step is divided by 8; at `dt_min` an error rejection is accepted with a warning and a Newton failure fails | |
| time points (§17.2) | a reachability set computed once at elaboration marks the reactions that can touch the analog side; only their scheduled times are breakpoints | |
| restart (§15.2) | a state counts as moved when its value or its `ddt` differs between the left value and the second pass by more than the convergence tolerance; if any did, the trapezoidal stage degrades to backward Euler for one step and the step restarts at 10⁻³ of the last one, with the error estimate taken over the moved states only; if none did, the step continues as if no jump had happened | |
| rollback (§17.2) | one event queue (scheduled commits restored, `at` times and breakpoints re-polled, step bounds and crossings discarded and re-emitted), with the digital state, device state and analog history checkpointed per step | |
| crossings (§17.3) | secant prediction from the bracketing pair, bisection when it stalls, retake to the predicted time | gap (G.6): today Piperine fires at the first accepted point after the sign change |
| limiting (§18) | ngspice `pnjlim`, `fetlim`, `limvds`; an instance is active while `abs(x̂ − x) > 1e-6 + 1e-4 · abs(x)`; when a limited branch potential becomes node potentials, the minus node moves and the plus node stays, unless the minus node is `gnd` | |
| analyses the host builds (§16.3, §16.4) | continuation keeps the checkpoint of the last accepted step; periodic steady state by shooting over continued transients | the periodic small-signal and noise linearization is not implemented (G.6, G.37) |
| temperature | `temperature` and `tnom` 300.15 K | |

### C.2.1 Defaults in one place

| Quantity | Default | Guarantee it serves |
| --- | --- | --- |
| time resolution (tick) | 1 fs | §15 |
| microsteps per instant | 1000 | §15: a loop that never goes quiet is an error |
| relaxation passes per loop | drive count + 1 | §8.2.3 |
| mixed rounds of an operating point | 20 | §16 step 4 |
| `reltol` | 1e-3 | §17.1 |
| potential `abstol`, flow `abstol` | 1 µV, 1 pA; a nature's own values take precedence | §17.1, §4 rule 3 |
| `gmin` | 1e-12 S | §16 step 3 |
| hold conductance | 10¹² S | §16 |
| `trtol` | 7 | §17.2.1 |
| step controller | `kp` 0.7, `ki` 0.4, growth in \[0.2, 1.5\], rejection ÷ 8 | §17.2 |
| restart step after a jump | 10⁻³ of the last step | §15.2.6 |
| limiting activity | `abs(x̂ − x) > 1e-6 + 1e-4 · abs(x)` | §18 rule 5 |
| `temperature`, `tnom` | 300.15 K | §14 |

### C.2.2 What the defaults may not change

1. Continuation methods never change the answer: the last solve of an operating point is the unmodified circuit (§16 step 3).
2. Bypass never applies while a `limit` instance is active (§17.1.3).
3. A rejected step leaves carriers, pending events, instance state and the analog history unchanged (§17.2.3).
4. Accepting an error rejection at `dt_min` is reported as a warning. It is the one place where the reference implementation relaxes §17.2.1.

### C.2.3 Conformance set

No part of this specification has been compiled or executed; every standard-library and gallery module is hand-traced (Appendix G.38).
The first compiler pass **SHOULD** run the following as its conformance set:

| Item | Exercises |
| --- | --- |
| `transition` (§22.2.11) | `after` in a definition, `at` re-arming, `analysis(Static)` seeding, tie order (§12.1.2) |
| `zi_nd` (§22.2.12) | `settle` commits, a switch branch selected by the environment, `delay` in small-signal |
| `inertial` (§22.5.8) | `at` re-armed by a carrier, generic results |
| `Tran` on `Wire`s (§22.3.7) | relaxation of loops among resolved drives (§8.2.3) |
| the trace of §15.1 | microstep order: commits, definitions, reactions |
