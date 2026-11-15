# Part V. Execution

This part specifies what every conforming simulator guarantees, in terms a model can observe.
It is an algorithm specification: an implementation **MAY** use any algorithm whose observable
result equals the one stated here. Time is superdense (Lee and Zheng, HSCC 2005). How the
reference implementation meets these guarantees, and where it does not yet, is Appendix C.2.

**Terms defined here:** instant, microstep, time point, event time, tick, time resolution,
due, pending, solve, equilibrium solve, transient step, jump solve, small-signal sweep,
left value, right value, jump, state (of the jump rule), impulsive flow, settle phase,
settle commit, operating point, round, sweep point, held node, hold, continued transient,
converged, accepted step, rejected step, scheduled time, located crossing, active (of a
`limit` instance), nodeset.

**Error codes.** This part owns `E05xx`. A run error ends the analysis; the host receives the
code, the instant and the names stated in the table. Rule citations follow §0.2.

---

## 15. Time and the instant

An **instant** is a pair `(t, n)`: a real time `t` and a microstep index `n ≥ 0`. Instants are
ordered lexicographically. Analog values are continuous in `t`, except at a jump (§15.2),
where they have a left value and a right value.

A **microstep** is one iteration of the microstep loop (step 3 below). At each time `t` the
microsteps are numbered from 0.

A **time point** is a time at which the analog solver produces an accepted solution and runs
the time-point procedure. A simulator MAY accept solutions at other times; those are not time
points and `settle` does not fire there. The set of time points is fixed by §17.2.2. An **event time** is a time at which a commit, an
`at` event, a located crossing or a host action is due. An event time is a time point only
when §17.2.2 makes it one.

### Ticks

- **T1.** The **time resolution** is a session option of the host, 1 fs by default. It is
  constant for a session (mixing resolutions is open, G.10). A **tick** is an integer
  multiple of the time resolution.
- **T2.** Every scheduled time is a tick. `at(x)` is scheduled at the tick nearest `x`. A
  commit made at time `t` with `after d` is **due** at the tick nearest `t + d`. A host
  action and the stop time are scheduled at the tick nearest the time the host gave.
- **T3.** A commit with no `after`, with `d ≤ 0`, or whose due tick is not later than its own
  time `t`, is due at the next microstep, `(t, n + 1)`.
- **T4.** A located crossing (§17.3) is rounded **up** to a tick. The solution is evaluated at
  that tick and the crossing's reactions run there. The expression has crossed where they
  read it.
- **T5.** A time point chosen by step control alone (§17.2) need not be a tick. Only the
  settle phase, and the commits it made at the previous time point, act there (S1).
- **T6.** Two event times are equal when they are the same tick. Events and commits that
  should coincide therefore do.

> **Resolution.** T2 rounds a half-tick tie toward the later tick. T2 rounds the absolute due
> time `t + d`, not `d`; the two differ only when `t` is not a tick (T5). T3 extends the rule
> "`d ≤ 0` is the next microstep" (§12.1) to a positive delay that rounds to no advance
> (G.60).

> **Resolution.** Tick counts are signed integers of at least 64 bits. A scheduled time
> outside that range is error `E0502`.

### Solves

A **solve** determines the analog unknowns while every discrete carrier and param is held
constant. There are four kinds, and §16.2 states what each primitive is in each:

| Kind | Definition | Where it occurs |
| --- | --- | --- |
| **equilibrium solve** | Newton on the equations with every `ddt` equal to 0 | each round of an operating point; each `Dc` sweep point (§16, §16.1) |
| **transient step** | Newton on the equations discretized from the previous time point to a later time | advancing a `Tran` analysis (§17.2) |
| **jump solve** | Newton on the equations of the jump rule, at an unchanged `t` | step 3.4 below (§15.2) |
| **small-signal sweep** | one linear solve per frequency of the linearized equations | `Ac`, `Noise` and host analyses built on them (§11.1, §16.3) |

The first three converge under §17.1. A sweep has no instants: no reaction runs in it and no
carrier changes (§16.2).

### The time-point procedure

At each time point `t` of a transient, after the operating point (§16), a simulator **SHALL**
produce the result of the following procedure.

1. *Solve* at `t`. The result is the **left value** at `t`.
2. *Collect* what is due at `t`: `at` events, delayed commits, located crossings, host
   `set`s and host drives (§23.1.4). Collected commits are due at `(t, 0)`. Collected
   events fire at `(t, 0)`.
3. *Microstep loop*, for `n = 0, 1, …`:
   1. *Apply* the commits due at `(t, n)`. Commits to one level of one drive are ordered by
      §12.1.2; the last one is the value.
   2. *Settle the structure.* Recompute every definition and every `resolve` whose inputs
      changed, in dependency order, loops by relaxation (§8.2.3). Every carrier or param
      whose value changed, here or in 3.1, fires its `changed()` at `(t, n)`.
   3. *React.* Run each reaction whose event fires at `(t, n)`, once, to completion. Each
      reads the state settled in 3.2 and the analog solution after microstep `n − 1` (for
      `n = 0`, the left value). Its commits are due at `(t, n + 1)`, or later under T2. An
      `extern "cosim"` module runs here as one reaction (§8.5).
   4. *Jump.* No solve runs here (§15.2). A carrier or param that `analog` reads and that
      changed in this microstep is read by the first transient step after `t`; a change due at
      `(t, 0)` was already read by the step that arrived at `t`.
   5. *Exit test.* Leave the loop when nothing is pending at `(t, n + 1)`.
4. *Settle.* Every reaction on `settle` runs once, reading the final discrete state and the
   final solution of `t`. This is the **settle phase**.
5. *Advance* to the next time point (§17.2).

Something is **pending** at `(t, n + 1)` when a commit is due there or an event fires there.
The table fixes the microstep at which each event fires; §13 defines the events themselves.

| Event | Fires at |
| --- | --- |
| `s.changed()` | `(t, n)`, when `s` changed in step 3.1 or 3.2 of microstep `n` |
| `at(x)`, scheduled at tick `t` before `t` was reached | `(t, 0)` |
| `at(x)`, armed or re-armed in microstep `n` with a time equal to `t` (§13 rule 4) | `(t, n + 1)` |
| `cross(e)`, located at `t` (§17.3) | `(t, 0)` |
| `settle` | the settle phase of `t`; no microstep |
| `a \|\| b`, `ev && c`, a function or module returning `Event` | whenever its definition (§13) fires from the rows above |

> **Resolution.** In the settle phase the only primitive event that has fired is `settle`.
> An event read as a `bool` there (§13 rule 2) is true only if `settle` makes it fire.

**Event times that are not time points.** At such a time `t`, steps 2, 3.1, 3.2, 3.3 and 3.5
run, in the same order. Steps 1, 3.4 and 4 do not: by §17.2.2 nothing at `t` probes and
nothing that `analog` reads can change. These instants run in increasing order of `(t, n)`,
interleaved with the time points.

> **Resolution.** The step list for an event time that is not a time point is stated here; the
> source design states only that such an instant "is not a time point" and that `on settle`
> does not run there.

### Invariants

A conforming execution satisfies all of the following at every instant.

- **I1. Determinism.** The result is a function of the source, the param values, the session
  options and the sequence of host actions. No rule depends on evaluation order within a
  microstep (§12.1.1–§12.1.3).
- **I2. Snapshot.** Every reaction of microstep `n` reads the same discrete state. No commit
  made at `(t, n)` is visible at `(t, n)`.
- **I3. Order.** Instants take effect in increasing order of `(t, n)`. A commit never applies
  at an instant earlier than or equal to the one that made it.
- **I4. Constancy.** Every discrete carrier and every param is constant during a solve.
- **I5. Agreement.** When the loop of a time point exits, the analog solution was solved with
  the final discrete state of that time point.
- **I6. Lag of one microstep.** The solution a reaction reads at `(t, n)` was solved with the
  discrete state `analog` read at `(t, n − 1)`.
- **I7. Termination.** The loop exits or the analysis fails with `E0501`. The bound on the
  number of microsteps per time is a session option (Appendix C.2 gives the default). Raising
  a bound never changes a result that did not hit it.
- **I8. No trace of rejection.** A rejected step (§17.2.3) leaves no observable effect,
  except the records `analog` wrote during it (§19 rule 3).

> **Resolution.** I3 and I8 make the ordering an as-if rule. An implementation **MAY** execute
> instants speculatively before the enclosing step is accepted, provided a rejected step
> undoes them. A commit made at an event time `t'` and due at a time that can touch `analog`
> makes that due time a time point, even when it precedes the tentative end of the step.

### The settle commit

A **settle commit** is a commit made by a reaction in the settle phase that is due at the
next microstep under T3. It is an ordinary commit with one fixed due instant.

- **S1.** A settle commit made at time point `t` is due at `(t_next, 0)`, where `t_next` is
  the next time point. There it fires `changed()`, and reactions to it run in that instant's
  loop like any others.
- **S2.** The solver knows the commit before it takes the step. The solve that arrives at
  `t_next` (step 1) already reads the value `(t_next, 0)` will apply to that carrier, and
  the definitions and `resolve`s that follow from it. When other commits to the same level
  are due at `(t_next, 0)`, the solve reads the winner under §12.1.2.
- **S3.** There is therefore no left and right value for that carrier at `t_next`: no second
  solve, no jump, no integrator restart. A settle commit is an explicit lag of one step.
- **S4.** Event times strictly between `t` and `t_next` read the old value.
- **S5.** If the step to `t_next` is rejected, the commit moves with `t_next`.
- **S6.** Where time does not advance, the next time point is the next sweep point (§16.1).
  A settle commit made at the stop time is dropped, unless the transient is continued
  (§16.4.1); then it is due at microstep 0 of the first time point after the stop time.
- **S7.** A commit made in the settle phase with a delay that advances time (T2) is an
  ordinary delayed commit. It is not a settle commit, and S2–S3 do not apply to it.

> **Resolution.** S7 and the second sentence of S6 are stated here. A settle commit made in an
> analysis without time that is run at a transient's last instant is open (G.34).

### What each kind of code reads

| Code | Discrete carriers and params | Analog values |
| --- | --- | --- |
| `analog` | constant for the whole solve | the current iterate |
| a reaction at `(t, n)` | settled at microstep `n` (step 3.2) | the solution after microstep `n − 1` |
| a reaction on `settle` | the final values of the time point | the final solution |
| a definition | settled in dependency order within the microstep | none (§8.2.5) |
| a located `cross` expression | constant between event times | the solver's solution along the step |

The operating point and the instant an analysis ends are always time points (§17.2.2).
`settle` therefore fires at least there, also in a design with nothing analog.

### Errors

| Code | Condition | Phase |
| --- | --- | --- |
| `E0501` | The microstep loop at one time does not go quiet within the session's bound. The report names the carriers that kept changing. | run |
| `E0502` | A scheduled time lies outside the tick range. The report names the `at`, commit or host action. | run |

### 15.1 An instant, traced

`Front` (§0) with `vin` crossing the upper threshold at `t = 10 ns`, exactly when `clk` rises.
10 ns is a time point: a crossing is located there (§17.2.2).

| Step | Instant | Applied (3.1) | Definitions (3.2) | Events fired | Reactions commit (3.3) |
| --- | --- | --- | --- | --- | --- |
| 1 | 10 ns | — | — | — | the left value is solved |
| 2 | 10 ns | — | — | — | collected: the commit of `clk`, the located crossing |
| 3 | (10 ns, 0) | `clk ← H`, from its clock source | `d` unchanged | `clk.rise()`, the crossing | `q <- d` reads the old `d`; `above_th <- true` |
| 3 | (10 ns, 1) | `q`, `above_th` | `d ← H` | `d.changed()` | none listens to `d` |
| 4 | 10 ns | — | — | `settle` | no reaction on `settle` |
| 5 | — | — | — | — | time advances |

No jump is read: `Front` has no `analog` block, so step 3.4 never holds. At `(10 ns, 1)`
nothing is pending at microstep 2, so the loop exits.

The flip-flop took the value from before the edge, as hardware does. The comparator's new
output reaches `q` at the next clock edge.

### 15.2 The jump

A **jump** is the change, at one time `t`, of something `analog` reads at `(t, n)`: a discrete
carrier or a param. The solution accepted before the change is its **left value**; the
**right value** is the solution of the first transient step that reads the change. One rule
defines the jump:

**A jump has no solve of its own.** The change is read by a transient step (§17.2) and by
nothing else. The solver lands exactly on the time `t` of the change (§17.2.2). What is due at
microstep 0 of `t` is applied before the step that arrives at `t` is solved, so that step reads
it. A change made in a later microstep of `t` is read by the first step after `t`. The step
that lands on a jump is exempt from the truncation-error test (§17.2.1), and the step after it
restarts the integrator (rule 6).

When several changes occur at one `t`, in successive microsteps, the first step after `t`
reads them all.

**Dynamic semantics.** The following hold with no further rule.

1. **The arguments of `ddt` are the states.** A **state** is the argument of one `ddt`
   instance. The integrator's history carries each state across a jump, to the accuracy of the
   step and not as an exact limit at one time. A capacitor keeps `c·v`, an inductor keeps
   `l·i`, an `idt` keeps its integral. Everything else (algebraic unknowns, flows) is free and
   is solved for.
2. **A constraint may force a state to move.** The step that reads the jump takes the new
   value of the constraint, and the sum of the states over each cut the constraint closes is
   conserved by that step, to the accuracy of the step. Shorting a 1 pF capacitor at 1 V to a
   3 pF one at 0 V gives 0.25 V on both at the first step that reads the jump (Appendix F.5).
3. **A state multiplied by a carrier keeps the product.** In `ddt(c * v(p, n))` with `c` a
   `Bit<f64>` that jumps, `c·v` is carried and `v` moves. Charge is conserved.
4. **Probes and `cross` read solutions that exist.** There is no second pass. A reaction at
   `(t, n)` reads the solution after microstep `n − 1` (§15), which is the left value until a
   step has read the change. A crossing that a change makes is detected between the solution
   of the step before it and that of the step that reads it (§17.3).
5. **Other instance state is untouched.** `delay` keeps its history and `limit` its last
   value. `idt`, `slew` and `transition` have no rule of their own: their state is a node
   under `ddt` and discrete carriers.
6. **The integrator restarts after the jump.** The step after the step that lands on a jump
   restarts the integrator: its trapezoidal stage drops the previous derivative and runs as
   backward Euler. After a host `set` the step size also restarts (Appendix C.2); after any
   other jump the controller's proposal is kept.
7. The step that reads a jump converges under §17.1. Each primitive has in it the meaning of
   the "Jump solve" column of §16.2, which is the meaning in a transient step.
   `analysis(Static)` is false and `now()` is the time of the step.
8. A jump that leaves a node with no path to `gnd` except through `ddt` terms is still
   determined, because the step discretizes those terms.
9. A jump whose constraints contradict each other, such as two potential sources in a loop,
   fails as a singular solve (`E0503`).
10. Sensitivities across a jump are those of the states carried and of the step that reads it
    (§23.3.2).

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0503` | A solve of any kind is singular. The report names the branches, or the node, that make it so. | run |
| `E0504` | A transient step that reads a jump does not converge (§17.1). The report names the instant and the carriers or params whose change caused it. | run |

**Example.** Appendix F.5 traces the charge-sharing jump row by row.

---

## 16. The operating point

The **operating point** is the state an analysis starts from: an analog equilibrium and a
digital state that agree. It is the instant `(0, ·)` of §15 with one difference. There is no
left value, so the first solve comes after the commits of microstep 0 instead of before them.

**Procedure.**

1. *Elaborate*: check params, run generation. Every level of every discrete carrier is
   unknown.
2. *Commit the initial values* at `(0, 0)`: every `= value` of the design, and every drive
   the host made before the run (§23.1.4). This is step 3.1 of §15. Definitions and
   `resolve`s settle and `changed()` fires (step 3.2; §13 rule 6). A `set` made before the
   run binds the param; it is not a change (§5.3.11).
3. *Solve* the equilibrium with that state. Newton starts from the host's nodesets (§18),
   else from the nodes' `= value`, else from zero. When Newton alone fails, the simulator
   uses continuation methods. A continuation method **MUST NOT** change the answer: the last
   solve is of the unmodified circuit.
4. *Run the microstep loop*, starting with the reactions of `(0, 0)` (step 3.3 of §15),
   which read that solution. In place of step 3.4: when something `analog` reads has
   changed, the equilibrium is solved again, and `cross` compares the two solutions (§13
   rule 5). This is a new equilibrium, not a jump: there are no states to carry. The number
   of rounds is bounded; exceeding the bound is error `E0506`.
5. *Settle* once (the settle phase of §15) and start.

A **round** is one equilibrium solve of an operating point: the solve of step 3 is the first,
and each re-solve in step 4 is another. `analysis(Static)` is true through the whole instant,
every round of it (§11.1).

**Dynamic semantics.**

- **O1.** `cross` needs two solutions, so it cannot fire on the first round. A crossing that
  must be right at time 0 is the stdlib `above` (§13, §22.2).
- **O2.** A crossing found between round `r` and round `r + 1` fires at the microstep after
  the one that caused round `r + 1`.
- **O3.** In a `Tran` analysis, time advances from this instant. A delayed commit made at
  `(0, n)` is due at its time (§15 T2). `at(x)` with `x` at tick 0 is armed with a time equal
  to the current one and fires at `(0, 1)` (§13 rule 4); a negative time never fires.
- **O4.** A settle commit made at the operating point follows §15 S1 and S6.
- **O5.** `transition`, `zi_nd` and `inertial` seed their state under `analysis(Static)`
  (§22).

> **Resolution.** O2 and the `at` clause of O3 apply §15's event table to the operating point.

**Held nodes.** A node's `= value` is where Newton starts. In a transient it is also held.

- **H1.** A **held node** is a node declared with `= value`, during the operating point of a
  `Tran` analysis. Through step 3 and every round of step 4 it is tied to that potential by a
  potential source to `gnd`, the **hold**. The hold is removed as the first step begins
  (SPICE `.ic`).
- **H2.** The first step starts from the held solution, with the arguments of `ddt` as its
  states (§15.2.1). The release is not a jump. A held node with no `ddt` on it moves
  freely in that step, which the integrator takes as a restart (§15.2.6).
- **H3.** In every other analysis nothing is held. For a held node that has a DC path, a
  transient's operating point and a `Dc` of the same circuit differ on purpose. A model that
  wants only a hint leaves it to the host's nodeset (§18).
- **H4.** A hold is a potential source like any other. Where it closes a loop of potential
  contributions the solve is singular (`E0503`) and names both, whether or not their values
  agree.
- **H5.** A node with no DC path has no equilibrium of its own. A transient starts from its
  held value. A `Dc` of that circuit fails as singular (`E0503`), naming the node.
- **H6.** There is no `uic`. A held start is always a solved, consistent one.
- **H7.** An initial condition on a branch is model text: the switch-branch pattern below.
  By the small-signal rule (§11.1) it is gone in a sweep.

```rust
/// A capacitor that starts charged.
module CapIc(p: Electrical, n: Electrical) { c: f64; ic: f64; }
analog for CapIc {
    if analysis(Tran) && analysis(Static) { v(p, n) +> ic; }      // the transient's operating point
    else { i(p, n) +> ddt(c * v(p, n)); }
}
```

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| §15.2 | A singular equilibrium solve (`E0503`): a hold closing a loop of potential contributions (H4); a node with no DC path in an analysis that does not hold it (H5); an `idt` with no `ic` outside a feedback loop (§8.3), naming the instance. | run |
| `E0505` | An equilibrium solve does not converge, after Newton and every continuation method. The report names the unknowns or `limit` instances that failed §17.1. | run |
| `E0506` | The rounds of an operating point exceed the session's bound. The report names the carriers still changing. | run |

### 16.1 Analyses without time

In `Dc` (each point of a sweep), and in the operating point of `Ac`, `Noise` and the other
small-signal analyses, time does not advance. A **sweep point** is one setting of what a `Dc`
analysis sweeps; an unswept `Dc` has one.

1. Each sweep point is an operating point. What the host changed for the point is committed
   at its microstep 0. Then steps 3–4 of §16 run, then the settle phase once.
2. **Discrete state carries from one sweep point to the next.** Initial values are committed,
   and `init()` fires, at the first point only. For `settle` a sweep point is a time point
   (§15 S6). `cross` compares the last solution of one point with the first of the next
   (§13 rule 5), and such a crossing fires at microstep 0 of the later point.
3. **What waits for time does not happen.** `at` never fires. A commit with `after d` whose
   due time is later than the current time never comes due: it is dropped when the analysis
   ends. A delayed definition is such a commit (§8.2.4), so a delayed drive holds its
   value through the analysis. A gate that should follow its input here says so (G.32).
4. A small-signal analysis linearizes at the fixed point of step 4 of §16, with the discrete
   state found there (§11.1). If the fixed point does not go quiet the analysis fails
   (`E0501` or `E0506`). It never linearizes around an oscillating state.

A Schmitt trigger swept up and down traces its hysteresis loop, because `above` sees each
threshold crossed between successive points (rule 2). `SrLatch` (Appendix E.2) keeps the
state it was given (rule 3).

> **Resolution.** The microstep at which a crossing between sweep points fires (rule 2) is
> stated here. It follows from §16: the solve of a point precedes the reactions of its
> microstep 0.

### 16.2 Every primitive in every solve

Two tables close the semantics. Everything in the standard library inherits its behavior from
their rows.

The first has one column per kind of solve (§15). `analog` is what runs in a solve. A
small-signal analysis is its operating point (the first column) followed by a sweep over
frequency (the last), in which `analog` is linearized under the small-signal rule (§11.1),
not executed.

| In `analog` | Equilibrium solve (a `Dc` point, every operating point) | Transient step | Jump solve (§15.2) | Small-signal sweep |
| --- | --- | --- | --- | --- |
| probes, `p.inflow()` | the iterate | the iterate | the iterate; the impulsive part of a flow is not read | the complex response |
| `ddt(x)` | 0 | the derivative | `(x − x⁻)/h`, `h → 0` | `jω·x` |
| `delay(x, td)` | `x` | `x(t − td)`; the operating point's `x` while `t < td` | as in the step | `exp(−jω·td)·x` |
| `limit(x, f)` | bounds the Newton path; equals `x` at convergence | the same | the same | `x` |
| `ddx(e, probe)` | the partial derivative at the iterate | the same | the same | its value at the operating point |
| `noise(psd)` | 0; `psd` is not read | 0; not read | 0; not read | 0 in `Ac`; in `Noise` a source of density `psd`, carried through every operator by its response |
| `ac_stim(mag, phase)` | 0 | 0 | 0 | the phasor in `Ac`; 0 in `Noise` |
| `bound_step(dt)` | nothing | caps the next step | nothing | nothing |
| `now()` | 0 | `t` | `t` | 0 |
| `freq()` | 0 | 0 | 0 | the frequency of the point |
| `gmin()` | the session `gmin`; the shunt of the step in a gmin-stepping solve | the session `gmin` | the session `gmin` | the session `gmin` |
| `source_scale()` | 1.0; the scale of the step in a source-stepping solve | 1.0 | 1.0 | 1.0 |
| `analysis(Static)` | true | false | false | false |
| `temperature()`, `simparam`, `analysis` of a kind | constant | constant | constant | constant |
| a discrete carrier, a param | constant | constant | constant: its change is why the solve runs | constant: no response |
| `emit!` | at every evaluation | the same | the same | not run |

1. `noise` reads its `psd` only where it is a source, because a value input is evaluated where
   it is read (§8.3.1). `k / freq().powf(exp)` is never evaluated at `freq() == 0`.
2. In a periodic analysis (§16.3) the last column is taken at each point of the trajectory.
   There `now()` is the point's time and a real carrier is as in §23.3.4.
3. In an analysis run at a transient's last instant (§16.4), `now()` is that instant in every
   column.
4. `analog` computes with known reals in every column (§3.3.5).

The second table is what happens at instants. A sweep over frequency has none.

| At instants | Where time does not advance (`Dc` points; the operating point of `Ac`, `Noise`) | Transient | Small-signal sweep |
| --- | --- | --- | --- |
| commits with no delay or `after 0`, definitions, `resolve`, `changed()`, reactions | at the microsteps of `(0, ·)`, and of each sweep point | at every instant | none |
| `cross(e)` | compares successive solves and sweep points; nothing to locate | located in time; also compared across a jump and across the rounds of the operating point | none |
| `at`, `after d` with `d > 0` | never due | due at their times | none |
| `settle` | once per operating point or sweep point | once per time point | none |

### 16.3 Analyses the host builds

`Analysis` has five variants (§11.1): four kinds of run and one kind of solve a model can
tell apart. Every other analysis is a host procedure over them and adds no column to §16.2.
A host **MUST NOT** expose to a model an analysis kind outside the table's last column.

| Host analysis | Built from | The model sees |
| --- | --- | --- |
| transfer function, poles and zeros, S-parameters, AC sensitivity | an operating point and the sweep's linearization, with the host's own stimulus, ports or eigenproblem in place of `ac_stim` | `Ac` |
| DC sensitivity | `Dc` and the gradients of §21 | `Dc` |
| distortion, harmonic measures | a transient, and the host's Fourier analysis of its records | `Tran` |
| periodic steady state | transients, each continued from the last (§16.4), until the state at the end of a period equals the state at its start | `Tran` |
| periodic small-signal and periodic noise | a periodic steady state, then the small-signal rule (§11.1) at each point of it | `Tran` for the trajectory, `Ac` or `Noise` for the linearization |

In a periodic small-signal or periodic noise analysis:

1. The discrete state is the trajectory's and `now()` is the point's time.
2. Commits and event times carry the perturbation as in §23.3.4.
3. The linearization runs under the sweep's own flags. `zi_nd` (§22.2) is therefore in its
   `delay` form there, on purpose.

How a shooting method updates the state between continued transients is the simulator's
procedure. No model can observe it.

### 16.4 From one analysis to the next

An analysis starts from the declared initial values (§16) unless the host runs it at the last
instant of a transient.

**An analysis run at a transient's last instant keeps everything**: time, the event queue
(pending commits, armed `at`s), discrete state, instance state and the analog history. It has
no operating point of its own. No initial value is committed again, nothing is held, no
`changed()` fires and `init()` does not.

1. **A `Tran` run there is the same transient with a later stop time**: a **continued
   transient**. `analysis(Static)` is never true in it. The old stop time remains a time
   point. Whatever the host `set` or drove between the two runs is a commit collected at
   that instant (§15 step 2), not an initial value.
2. **An analysis without time (`Dc`, `Ac`, `Noise`) run there is one more instant of that
   transient, at which time does not advance** (§16.1). `now()` stays. The queue stays and
   nothing in it comes due. Newton starts from the transient's final solution. The transient
   can be continued after it.

`ClockGen` (Appendix D.2) keeps ticking across a continuation, and a `transition` mid-ramp
finishes its edge. A periodic steady state is continued transients only (§16.3).

Three points are open: that `analysis(Static)` is true in an analysis of rule 2 (G.33), which
commits made during it survive (G.34), and continuing after a `finish` record (G.35).

---

## 17. Transient

A transient advances from the operating point to the stop time through a sequence of time
points. Between them it runs the event times that are not time points (§15).

### 17.1 Convergence

Every solve but a small-signal sweep is a Newton iteration. It has **converged** when all
three hold:

1. **Update.** Every unknown moved by at most `reltol · max(|x|, |x_old|) + abstol`. `abstol`
   is the nature's: for a node, the smallest potential `abstol` of the natures meeting there;
   for a branch unknown, its flow `abstol`.
2. **Residual.** Every row's imbalance is at most `abstol + reltol · scale`. For a node's row
   `abstol` is the flow `abstol` of its nature. For the row of a branch with a potential
   contribution it is the potential `abstol`.
3. **Limiting.** No `limit` instance is active (§18 rule 5).
4. A non-finite value in an unknown, a residual or a Jacobian entry fails the solve
   (`E0507`).
5. `reltol` is a session option. Appendix C.2 gives its default and the reference
   implementation's damping and bypass, which **MUST NOT** change what counts as converged.

> **Resolution.** In rule 2, `scale` is the largest magnitude among the terms summed in the
> row. Where several natures meet at a node (§4 rule 5), the row uses the smallest of their
> flow `abstol`s, by symmetry with rule 1.

> **Resolution.** A node of a nature without `flow` has no row of its own. Its potential is
> set by the row of its one potential driver (§6 rule 2).

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0507` | A non-finite value appears in a solve. The report names the instance and the contribution or unknown. | run |

### 17.2 Steps

1. The truncation error of every accepted step is within `trtol`, measured with the natures'
   tolerances. `trtol` is a session option.
2. **The solver lands exactly on every instant that can touch it.** A **scheduled time** is
   the time of an `at`, of a commit with `after`, or of a host action. A reaction *counts*
   at a scheduled time `t` when its event fires at `(t, 0)` on the discrete state settled
   there, a guard that probes being taken as true; or when a reaction that counts can make
   it fire in a later microstep of `t`, through commits, definitions and `resolve`s. The
   first test is dynamic and needs no solve. The second is made on the static dependencies
   of the elaborated design. A scheduled time is a time point exactly when one of these
   holds:
   - what is due at `(t, 0)` changes, directly or through definitions and `resolve`s, a
     carrier or param that `analog` reads;
   - a reaction that counts there probes, in its guard or body, or can change, at that
     instant, a carrier that `analog` or a probing reaction reads. A commit whose delay
     always advances time by at least one tick changes nothing at the instant that makes
     it; its due time is a scheduled time tested on its own;
   - a host action there probes (rule 6).

   The operating point, the stop time, every located crossing (§17.3) and the instant at
   which a `finish` record ends the analysis (§19 rule 5) are time points too. Every other
   event time is not a time point (§15).
3. A **rejected step** is a transient step whose result is discarded. It leaves everything
   unchanged: carriers, pending events, instance state, the analog history.
4. An **accepted step** is a transient step whose solution becomes a time point. Instance
   state (`ddt`, `delay`, `limit`, `cross`, and the nodes and carriers of stdlib modules)
   advances only on accepted steps.
5. `bound_step(dt)` caps the step that follows the time point at which it was evaluated. The
   cap is the smallest `dt` among the calls executed in that time point's final evaluation of
   `analog`. It is re-evaluated at every time point. A call not executed imposes no cap.
6. A host `set`, drive, read or probe at a scheduled time is a host action in the sense of
   rule 2. A host action that probes makes its time a time point.

**Advance procedure** (step 5 of §15). From the accepted time point `t`:

- **A1.** Choose a candidate `t_c > t`, no later than the next scheduled time that is a time
  point, the stop time, and `t` plus the cap of rule 5.
- **A2.** Perform the transient step to `t_c`, reading the settle commits of `t` (§15 S2).
- **A3.** If Newton does not converge, or rule 1 fails, reject the step and go to A1 with a
  smaller candidate.
- **A4.** If a `cross` expression went from `≤ 0` to `> 0` in the step, locate it (§17.3).
  This may replace `t_c` by an earlier time and repeat from A2.
- **A5.** Accept the step. `t_c` is the next time point. Event times in `(t, t_c)` that are
  not time points take effect in order, before `t_c` (§15 I3).

How the candidate is chosen is the simulator's (Appendix C.2). The set of times at which the
solver **MUST** land is rule 2.

| Situation | Time point at `t` | Jump (§15.2) |
| --- | --- | --- |
| discrete change that cannot reach `analog` or a probe | no | no |
| scheduled commit whose only listeners cannot fire there: a falling `clk` heard by `rise()` | no | no |
| discrete change at a microstep, read by `analog` | yes | yes |
| host `set` of a param read by `analog` | yes | yes |
| reaction at `t` that probes | yes | no |
| `cross` located at `t` | yes | only if what `analog` reads changes |
| `on settle` | runs only at time points; adds none | no |
| a commit from `on settle`, read by `analog` | no: it is due at the next one | no (§15 S3) |
| `bound_step(dt)` | no: caps the step | no |

A digital clock that nothing analog depends on costs the solver nothing.

> **Resolution.** Rule 5's "smallest among the calls executed" and rule 6 are stated here.
> A `dt` that is not finite and positive is error `E0509`.

> **Resolution.** Rule 2 tests the reactions of microstep 0 on the settled discrete state, and
> those of later microsteps statically. The set of scheduled times that are time points is
> exactly the one rule 2 gives: `settle` observes it. The `finish` clause is dynamic too: it
> is the instant at which the record is written.

> **Resolution.** Where rule 1 is measured: on node potentials and branch-flow unknowns
> against their natures' `abstol`, and on a `ddt` state against that instance's own `abstol`
> argument (§14) when one is given.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0508` | A transient step cannot be completed: Newton does not converge at the smallest step the simulator takes. The report names the time and the failing unknowns or `limit` instances. | run |
| `E0509` | `bound_step(dt)` evaluated with `dt` not finite or `dt ≤ 0`. | run |

### 17.3 Locating crossings

A **located crossing** is a `cross` whose expression `e` goes from `≤ 0` to `> 0` between two
time points, and whose time the solver finds.

1. The step is retaken until the crossing time is known within `ttol`, or `0 < e < etol`.
2. Defaults: `ttol` is a session option; `etol` is the `abstol` of the nature of the quantity
   crossed (§4 rule 3).
3. The landing time is rounded up to a tick (§15 T4). At the landing time point `e > 0`
   holds, and `e ≤ 0` held at the previous time point.
4. When several crossings fall in one step the solver lands on the earliest. Crossings that
   round to the same tick fire together at `(t, 0)`.
5. An expression that is unknown does not fire and is not located.
6. A `cross` under a guard made only of discrete carriers is not located while the guard is
   false (§13 rule 5).
7. A corner of a waveform computed from `now()` is invisible to the error estimate. It
   **MUST** be written as a carrier change (§9, case D), which makes it a time point and a
   jump.

> **Resolution.** Rule 3's guarantee is stated as: the landing time exceeds the true crossing
> time by at most `ttol` plus one tick, or `e < etol` holds there. Rule 4 is stated here.

> **Resolution.** A `ttol` or `etol` argument that is known and not finite and positive is
> error `E0510`. An unknown one falls under rule 5.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0510` | `cross` evaluated with a `ttol` or `etol` that is not finite and positive. | run |

The reference implementation does not yet land on the predicted crossing (Appendix C.2,
G.6).

---

## 18. Limiting

**Limiting** bounds how far Newton moves an expression per iteration: `limit(x, f, seed)`
does it for `x`, with
`f: fn(new: f64, old: f64) -> f64`. It changes the path, never the answer. `limit` is a core
module with a result (§14); each use is an instance.

**Dynamic semantics.**

1. At iteration `k` the instance returns `x̂_k = f(x_k, x̂_{k−1})`, where `x̂_{k−1}` is what
   this instance returned before. On the first iteration of an equilibrium solve, `x̂_{k−1}`
   is `seed` when given, else the last accepted value or the initial guess.
2. While the instance still holds its seed and the limited value would fall below it, the
   seed is kept. A junction opens near its turn-on point (ngspice `MODEINITJCT`).
3. `x̂_k` replaces `x` in the rest of the expression. For differentiation `limit` is
   transparent: `∂x̂/∂x = 1`.
4. The right-hand side is corrected to linearize around `x̂_k` (OSDI `lim_rhs`).
5. An instance is **active** while its limited value differs from its input beyond a
   tolerance. An active instance vetoes convergence (§17.1.3). At convergence the model
   is exactly the unlimited one.
6. On the first iteration of a transient step or a jump solve, `x̂_{k−1}` is the instance's
   value at the last accepted solution. `seed` is not read there.
7. In a small-signal sweep `limit(x, f)` is `x` (§16.2). The instance's last value advances
   only on accepted steps (§17.2.4) and is untouched by a jump (§15.2.5).
8. `f` **SHOULD** satisfy `f(x, x) == x`. With an `f` that does not, the instance stays
   active and the solve fails to converge (`E0504`, `E0505` or `E0508`).
9. An `extern "osdi"` module's own limiting vetoes convergence under rule 5 (§8.5).

A **nodeset** is a starting potential the host gives for a node, by path. It is a hint, not
an equation, so it is not model text. It is read only as Newton's starting point in step 3 of
§16 and never changes a converged answer.

> **Resolution.** Rules 6–8 are stated here. The tolerance of rule 5 is the simulator's
> (Appendix C.2 gives the reference value).

**Errors.** `limit` raises no error of its own. A non-finite result of `f` or a non-finite
`seed` is `E0507`.

**Examples**

```rust
/// Junction diode with optional series resistance.
module Diode(a: Electrical, k: Electrical) {
    is: f64 = 1e-14 where is > 0.0;
    n: f64 = 1.0 where (0.1..=10.0).contains(&n);
    rs: Option<f64>;
    cj0: f64 = 0.0 where cj0 >= 0.0;
    signal ai: Electrical;
}

analog for Diode {
    match rs {
        Some(r) => { i(a, ai) +> v(a, ai) / r; }
        None    => { v(a, ai) +> 0.0; }          // a param selects: ai and a are one node (§11 rule 3)
    }
    let nvt   = n * vt();
    let vcrit = nvt * (nvt / (2.0_f64.sqrt() * is)).ln();
    let vd    = limit(v(ai, k), |new, old| pnjlim(new, old, nvt, vcrit), vcrit);
    let id    = is * ((vd / nvt).exp() - 1.0);
    i(ai, k) +> id + ddt(cj0 * vd) + white_noise(2.0 * Q * id, "shot");
}

/// Square-law NMOS. Assumes vds >= 0.
module Nmos1(d: Electrical, g: Electrical, s: Electrical) {
    kp: f64 = 100u; vth: f64 = 0.5; lambda: f64 = 0.0; w: f64 = 1u; l: f64 = 1u;
}

analog for Nmos1 {
    let vgs = limit(v(g, s), |new, old| fetlim(new, old, vth));
    let vds = limit(v(d, s), limvds);
    let beta = kp * w / l;
    let vov = vgs - vth;
    let clm = 1.0 + lambda * vds;
    let id = if vov <= 0.0 { 0.0 }
             else if vds < vov { beta * (vov - vds / 2.0) * vds * clm }
             else { beta / 2.0 * vov * vov * clm };
    i(d, s) +> id;
    let gm = ddx(id, v(g, s));          // an operating-point variable, like every named let (§11 rule 6)
}
```

