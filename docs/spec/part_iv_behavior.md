# Part IV. Behavior

Behavior with time is written in blocks attached to a module. An `analog` block is solved; a
`digital` block reacts to events. Events are values. What only the simulator can provide is the
core of §14.

**Terms defined here:** behavior block, `analog` block, `digital` block, contribution, branch kind,
selector, switch branch, operating-point variable, small-signal rule, noise source, reaction, commit,
transport delay, event, event position, guard, core primitive.

Error codes of this part: `E04xx`. Rule citations and error tables follow §0.2.

---

## 10. Behavior blocks

A **behavior block** attaches time-dependent behavior to a module: `analog<G> for M<G> { … }` or
`digital<G> for M<G> { … }`.

**Syntax**

```
behavior_block ::= 'analog'  names? 'for' path generic_args? '{' ( stmt | contribution )* '}'
                 | 'digital' names? 'for' path generic_args? '{' digital_item* '}'
names          ::= '<' IDENT ( ',' IDENT )* '>'
digital_item   ::= reaction | gen<digital_item>
gen<X>         ::= attrs 'for' pat 'in' expr '{' X* '}'
                 | attrs 'if' expr '{' X* '}' ( 'else' ( '{' X* '}' | gen<X> ) )?
reaction       ::= attrs 'on' expr '{' ( stmt | commit )* '}'                      // expr has type Event
contribution   ::= attrs expr '+>' expr ';'                                         // left side: a probe
commit         ::= attrs place '<-' expr ( 'after' expr )? ';'
place          ::= path ( '[' expr ']' | '.' IDENT )*                           // a carrier, an element or a field of one
```

| Block | Holds | Runs | Writes |
| --- | --- | --- | --- |
| `analog` | statements: `let`, `if`, `match`, calls, `+>` | at every Newton iteration of every analysis | branches, with `+>` |
| `digital` | reactions and generation | when their events fire | discrete carriers, with `<-` |

|  | contract | structure | without time | with time |
| --- | --- | --- | --- | --- |
| values | `trait` | `struct`, `enum` | `impl` | — |
| hardware | bundle (`struct`) | `module`, `nature` | `impl` | `analog for`, `digital for` |

**Static semantics**

- **§10 rule 1** Generic names follow the block keyword; bounds come from the module.
- **§10 rule 2** A block sees every port, param, signal and `let` of its module, and the `pub` items of
  its instances (§8.2.8).
- **§10 rule 3** `analog` and `digital` are keywords only before `for` or `<`.
- **§10 rule 4** A module **MAY** have any number of blocks of each kind. Blocks of one kind act as
  one, in source order. Source order decides the order of reactions (§12.1.2); for `analog` it is
  not observable, because contributions add.
- **§10 rule 5** A block **MUST** be in the file of its module (§3.4). The path names a module that is
  not `extern` (§8.5.7).
- **§10 rule 6** A block repeats the generic names of its module: the `names` list **MUST** hold
  the module's generic parameter names, the same names in the same order, and the path's generic
  arguments **MUST** be exactly those names. A block for one specialization is not expressible.
- **§10 rule 7** Each block is its own scope for locals. A local of one block is not visible in
  another. What blocks share is a named expression of the body (§8.2).
- **§10 rule 8** `contribution` is a statement of an `analog` block and of every nested block inside
  it. `commit` is a statement of a reaction body and of every nested block inside it. Neither
  appears anywhere else, nor inside a closure or a `fn`. `<-`, `+>`, `bound_step`, `on` and
  `signal` written in a block or body that is not theirs, a module body included, are `E0404`;
  inside a `fn` body, a closure in it included, the code is `E0121`.
- **§10 rule 9** A block places no item in the netlist. Signals, params and named instances are
  declared in the body. A placement inside a block is an anonymous instance of the module (§8.3.4).

> **Resolution.** §10 rule 6: the names are the module's own, repeated in order, so that a block
> reads as the module reads and covers every instantiation. A block never renames a generic.
> The source design shows only this form.

> **Resolution.** §10 rule 7 reads "act as one" as ordering, not as one lexical scope. The body's `let`
> is the stated way for blocks to share a computation.

> **Resolution.** §10 rule 8: the grammar lists `contribution` and `commit` at the top level of their
> blocks. Every example nests them under `if`, `match` and `for`, so they are statements at any
> depth (Appendix B.3.4).

**Dynamic semantics**

- **§10 rule 10** An `analog` block is evaluated in every solve of every analysis (first table of
  §16.2). A `digital` block acts only at instants (second table of §16.2).
- **§10 rule 11** A module with no block has only the behavior of its definitions and instances.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0401` | a block whose path names no module of the same file | elaboration |
| `E0402` | a block for an `extern` module | elaboration |
| `E0403` | generic names that do not match the module's generic parameters (§10 rule 6) | elaboration |
| `E0404` | a well-formed statement in a block or body of the wrong kind: `<-` or `on` in `analog`; `+>` or `bound_step` in `digital` or a reaction; `signal` in `analog`, a reaction or a closure outside a `fn`; `<-`, `+>`, `on`, `bound_step` or `signal` in a module body (§10 rule 8). Inside a `fn` body `E0121` is the code reported (§3 rule 6). A `+>` or `<-` inside an expression is `E0907` | parse |

---

## 11. Analog

An **`analog` block** is straight-line code evaluated at every Newton iteration, with no memory
between iterations. The past comes from instances and from discrete carriers.

A **contribution** is the statement `probe +> x;`. It adds `x` to the flow or to the potential of
a branch. The **kind** of a branch in a solve is *potential* when it receives at least one potential
contribution in that solve, and *flow* otherwise. The **selector** of a contribution is the
conjunction of the conditions of the `if` and `match` arms that enclose it.

**Static semantics**

- **§11 rule 1** `i(a, b) +> x;` adds `x` to the branch's flow. `v(a, b) +> x;` adds `x` to its
  potential, which makes the branch a potential source with its flow as an unknown. Contributions
  add, from anywhere in the module.
- **§11 rule 3** **A branch's kind is constant during a solve.** For a branch that has a potential
  contribution anywhere in the module, the selector of every contribution to it, potential or flow,
  **MUST** depend only on what is constant during a solve: params, discrete carriers and the
  environment (§14.4), `now()` and `analysis(Static)` included. A selector that depends on a probe,
  or on a result that reads a node, is an elaboration error. The check is made per instance, after
  value inputs are substituted (§8.3.9).
  - When the selectors of a branch depend on params only, its kind is fixed at elaboration, and
    those params are structural (§8.1.2).
  - A fixed potential branch whose contributions are all the constant zero merges its two nodes.
    It keeps a flow unknown only if the module probes that flow, directly or through `inflow()`.
  - Every other potential-capable branch keeps its flow unknown.
- **§11 rule 5** Everything is differentiated automatically with respect to every probe, through
  function calls, value inputs and results. An `if` on a probe chooses between expressions, never
  between kinds (§11 rule 3). How smooth the model is there is the author's business.
- **§11 rule 6** Every named `let` of an `analog` block is an **operating-point variable**: the host
  reads it by path after an analysis (§21).
- **§11 rule 7** An `analog` block has no reactions. Events on analog quantities are written in
  `digital` (§13).
- **§11 rule 8** The left side of `+>` **MUST** be a call of `potential`, `flow` or a nature's
  `access` alias (§6), with one or two node arguments. `p.inflow()` is not a contribution target.
  The right side is an `f64` expression.
- **§11 rule 9** Each node of the left side **MUST** be `gnd`, a continuous port or signal of the
  module, a field or element of one, or a `pub` node of one of its instances (§8.2.8). The branch
  is this module's own, whoever declares the nodes.
- **§11 rule 10** The two nodes **MUST** differ. `(b, a)` is the branch `(a, b)` reversed (§6):
  `i(b, a) +> x` is `i(a, b) +> -x`.
- **§11 rule 11** A flow contribution needs a nature with a `flow`. A node of a nature without one
  takes potential contributions only, from exactly one module (§6).
- **§11 rule 12** An `analog` block holds value statements (`let`, `let mut`, assignments to locals,
  `if`, `match`, `for`, calls), contributions, `bound_step` (§14.10) and `emit!` (§19). A loop that
  contains a contribution or a placement **MUST** have a constant trip count (§8.3.4).
- **§11 rule 13** `analog` has no unknown values. Every read of a discrete carrier passes through
  `level` (§3.3.5). The same holds for the value inputs of `ddt`, `delay`, `limit` and `noise`,
  wherever they are placed.
- **§11 rule 14** There is no indirect contribution. An implicit equation is the row of an internal
  node (the `Nullor` example).

> **Resolution.** §11 rule 3 extends the selector rule to the flow contributions of a branch that can
> be a potential branch. Otherwise the kind would depend on a probe through the flow side (G.58).

> **Resolution.** §11 rule 3: a zero-potential branch is merged only when no flow probe of it exists.
> A probed short keeps its flow unknown, like any potential source.

> **Resolution.** §11 rule 10 rejects a branch from a node to itself. §11 rule 12 requires a constant trip
> count around a contribution, as §8.3.4 does around a placement.

> **Resolution.** §11 rule 13 applies §3.3.5 to the value inputs of the four solver-evaluated core
> modules even when the placement is in a body `let`. `cross` and `at` accept unknown arguments
> (§13 rule 4, §13 rule 5).

**Dynamic semantics**

- **§11 rule 2** *Static:* a branch that gets potential contributions in some solves and flow
  contributions in others is a **switch branch**: `v(a, b) +> 0.0` is a short, `i(a, b) +> 0.0` is
  open. *Dynamic:* a branch gets either potential or flow contributions in one solve, never both.
  The kind is decided at the start of the solve from the selectors. A solve in which both kinds
  are enabled fails naming the branch (`E0418`); when the selectors are params only, the failure
  is an elaboration error (`E0415`).
- **§11 rule 4** A branch with no contribution in a solve has zero flow.
- **§11 rule 15** The block is executed at every Newton iteration with probes reading the current
  iterate. Locals do not persist between iterations.
- **§11 rule 16** A branch's equation in a solve, with `Σ` over the contributions enabled in it and
  `M` the effective multiplicity (§8.5.4):

  | Kind | Equation | Unknown added |
  | --- | --- | --- |
  | flow | flow of the branch `= M · Σ x`, entered in the rows of both nodes | none |
  | potential | `potential(a, b) = Σ x` | the branch flow |

- **§11 rule 17** Per kind of solve, a contribution's right side is evaluated with every primitive in
  its column of §16.2:

  | | Equilibrium solve | Transient step | Jump solve (§15.2) | Small-signal sweep |
  | --- | --- | --- | --- | --- |
  | the block | executed each iteration | executed each iteration | executed each iteration | linearized, not executed |
  | a contribution `x` | `x` with `ddt` terms 0 | `x` with `ddt` as the derivative | `x` with `ddt` as the derivative of the step that reads the jump | `Σ ∂x/∂probe · response`, plus `ac_stim` and `noise` terms |
  | a branch's kind | from the selectors, `analysis(Static)` true | from the selectors | from the selectors after the change | from the selectors under the sweep's flags (§11.1.4) |

- **§11 rule 18** A change of kind happens only between solves. A discrete carrier or param that
  flips a selector is a jump (§15.2); the solve after it uses the new kind.
- **§11 rule 19** A non-finite contribution fails the solve (§17.1).
- **§11 rule 20** `emit!` in `analog` runs at every evaluation, rejected iterations and steps included
  (§19 rule 3). It does not run in a small-signal sweep.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0410` | the left side of `+>` is not a potential or flow probe of a branch (§11 rule 8) | elaboration |
| §6 | a contribution node the module may not use (`E0244`) or that is not continuous (`E0206`); a branch from a node to itself (`E0241`) | elaboration |
| `E0413` | a flow contribution on a nature without `flow` (§11 rule 11) | elaboration |
| `E0414` | a selector of a potential-capable branch depends on a probe (§11 rule 3) | elaboration |
| `E0415` | potential and flow contributions to one branch both enabled, the selectors being params only (§11 rule 2) | elaboration |
| `E0418` | potential and flow contributions to one branch enabled in one solve (§11 rule 2) | run |
| `E0416` | a contribution inside a loop whose trip count is not a constant (§11 rule 12) | elaboration |
| `E0417` | the right side of `+>` is not `f64` | elaboration |
| §3.3 | `analog` reads a discrete carrier without `level` (`E0150`) | elaboration |
| §6 | a second potential driver of a signal-flow node (`E0242`) | elaboration |
| §17.1 | a non-finite value in a solve (`E0507`) | run |

**Examples**

```rust
analog for Capacitor { i(p, n) +> ddt(c * v(p, n)); }
analog for Inductor  { v(p, n) +> ddt(l * i(p, n)); }

module Switch(a: Electrical, b: Electrical, in ctl: Bit) {}
analog for Switch {
    if ctl.high() { v(a, b) +> 0.0; } else { i(a, b) +> 0.0; }
}

/// Two nodes made one: with no selector the kind is fixed, and the nodes merge at elaboration.
pub module Short<N: Nature>(a: N, b: N) {}
analog<N> for Short<N> { potential(a, b) +> 0.0; }

/// An equation is a row: a nature whose "flow" is the residual gives it the right tolerance.
nature Servo {
    potential: Voltage { access: v,   abstol: 1u },
    flow:      Voltage { access: err, abstol: 1u },
}

/// Ideal op-amp (nullor): drives vout so that v(inp, inn) == 0.
module Nullor(inp: Electrical, inn: Electrical, vout: Electrical) { signal x: Servo; }
analog for Nullor {
    err(x) +> v(inp, inn);  // the row of x carries the equation, tested against 1 µV
    v(vout) +> v(x);        // v(x) is the free unknown
}

/// Two coupled inductors: coupling is inside one module, which writes only its own branches.
module Coupled(p1: Electrical, n1: Electrical, p2: Electrical, n2: Electrical) {
    l1: f64; l2: f64; k: f64 where k.abs() <= 1.0;
    let mu = k * (l1 * l2).sqrt();
}
analog for Coupled {
    v(p1, n1) +> ddt(l1 * i(p1, n1) + mu * i(p2, n2));
    v(p2, n2) +> ddt(l2 * i(p2, n2) + mu * i(p1, n1));
}
```

| Branch | Selector depends on | Kind | Flow unknown |
| --- | --- | --- | --- |
| `Switch (a, b)` | the carrier `ctl` | chosen per solve: a switch branch | kept |
| `Short (a, b)` | nothing | potential, fixed; nodes merged | none |
| `Diode (a, ai)` (§18), `match rs` | the param `rs`, structural | fixed at elaboration; merged when `rs` is `None` | none when merged |
| `idt (y, gnd)` (§8.3) | `reset`, `ic`, `analysis(Static)` | chosen per solve | kept |

Coupling among any number of separate inductors goes through a port: each inductor has a
`Magnetic` node (potential: magnetomotive force, flow: flux rate) and a core module connects them.

### 11.1 One block, every analysis

An **analysis** is one run the host asks of an elaborated design: `Dc`, `Transient`, `Ac`, `Noise`, or
a host procedure over them (§16.3). One `analog` block serves every analysis. What each primitive is in each kind of solve is one table
(§16.2). `analysis(a)` tells the block which analysis and which solve it is in.

**Static semantics**

- **§11.1.1** `Analysis` is a closed enum with five variants: `Dc`, `Transient`, `Ac`, `Noise`,
  `Static`. `fn analysis(a: Analysis) -> bool` is an environment function (§14.4).

  | `Analysis` | True during | Constant during an analysis |
  | --- | --- | --- |
  | `Dc`, `Transient`, `Ac`, `Noise` | the whole of that analysis, its operating point included | yes |
  | `Static` | an equilibrium solve: a `Dc` point and every operating point, all its rounds. False in a transient step, a jump and a small-signal sweep | no: it is the one variant that names a solve, not an analysis |

- **§11.1.2** Every other analysis a host offers is built by the host from these solves. The model
  sees the kind it is built from (§16.3).

**Dynamic semantics**

- **§11.1.3** In a reaction, the solve is the one whose solution the reaction reads. So
  `analysis(Static)` holds there exactly at the instants of an operating point or a `Dc` point.
- **§11.1.4** **The small-signal rule.** A small-signal sweep linearizes the equations the
  `analog` blocks execute *under the sweep's own flags* (`analysis(Static)` false, `ddt(x)` read as
  `jω·x`), at the solution and discrete state it is taken around. That is the operating point's
  solution and state or, in a periodic analysis (§16.3), those of each point of a periodic
  transient. Nothing else is said about the sweep.
- **§11.1.5** Consequences of §11.1.4, not further rules:

  | Construct | In the operating point | In the sweep |
  | --- | --- | --- |
  | a switch branch | the kind its selector gives with `analysis(Static)` true | the kind its selector gives with `analysis(Static)` false |
  | `idt` with an `ic` | held at `ic` | `1/jω` |
  | a capacitor held at an initial voltage (`CapIc`, §16) | a potential source | a capacitor |
  | a discrete carrier | its settled value | constant: no response |
  | `delay(x, td)` | `x` | `exp(−jω·td)·x` |

- **§11.1.6** A sampled system that has a small-signal response around an operating point states
  it: under `!analysis(Transient)` it writes the same transfer with `delay`. `zi_nd` (§22.2) is written
  this way, which gives `H(exp(jωT))` in `Ac` and `Noise` and `H(1)` in `Dc` (VAMS 4.5.12).
- **§11.1.7** Around a periodic trajectory a real-valued carrier carries the perturbation
  committed to it, and event times move (§23.3.4). Every sampled system then has a response with
  nothing written: `SampleHold` (§12.1) has its zero-order-hold response.
- **§11.1.8** **A noise source is an instance** of `noise` (§14.8). One instance read in two
  contributions is fully correlated. Two instances are independent. The name only groups sources in
  reports.
- **§11.1.9** In `Noise`, every operator applies its small-signal response to a noise result, as
  to any signal: `ddt(n)` is `jω·n`, `delay(n, td)` is `exp(−jω·td)·n`, a `laplace_*` filter is
  `H(jω)·n`. Partly correlated noise is one instance read along two paths.

Two points are open: whether "constant during an analysis" in §11.1.1 needs a row for a periodic
analysis (G.36), and that an analysis without time run at a transient's last instant (§16.4) has
equilibrium solves, so `analysis(Static)` is true there (G.33).

**Examples**

```rust,fragment
let n = noise(4.0 * K_B * temperature() / r, "thermal");
i(a, mid) +> n;
i(mid, b) +> -n;          // fully correlated
```

Induced gate noise is `n` in the drain and `ddt(c * n)` in the gate: one source, two paths.

### 11.2 Bounded state

A state that must stay inside `[lo, hi]` is held at the bound by a switch branch, turned on by a
crossing and off when the drift turns back. The pattern uses §11 rule 3 and §13 only; it adds no rule.
It is exact, unlike a window function. With `v` and `i` of a branch, it is the ideal diode.

**Examples**

```rust
module Bounded(p: Electrical, n: Electrical) {
    hi: f64 = 1.0;
    signal xs: State = 0.0;                 // stdlib nature, order one: pos and rate (§22.2)
    signal at_hi: Bit = false;
    let drift = v(p, n) * 1e-3;             // shared by both blocks
}
analog for Bounded {
    if at_hi.high() { pos(xs) +> hi; } else { rate(xs) +> ddt(pos(xs)) - drift; }
}
digital for Bounded {
    on above(pos(xs) - hi) { at_hi <- true; }
    on above(-drift) && at_hi { at_hi <- false; }
}
```

| Instant | What happens | Rule |
| --- | --- | --- |
| `pos(xs)` reaches `hi` | the crossing is located; `at_hi <- true` | §13 rule 5, §17.3 |
| next microstep | `at_hi` applies; `analog` reads it: a jump; the branch becomes a potential source at `hi` | §15.2, §11 rule 18 |
| `drift` goes negative | the second crossing fires under its guard; `at_hi <- false` | §13 rule 1 |
| next microstep | a jump; `pos(xs)` is a `ddt` state and stays at `hi`; integration resumes | §15.2.1 |

---

## 12. Digital

A **`digital` block** holds reactions and generation (`for`, `if` over params, §8.2.7). It writes
discrete carriers only. It cannot contribute.

**Static semantics**

- **§12 rule 1** Generation in a `digital` block follows §8.2.7. Each unrolled reaction is a reaction
  of its own, in unrolling order.
- **§12 rule 2** A `digital` block holds no statement outside a reaction.

### 12.1 Reactions

A **reaction** `on ev { body }` runs `body` once, to completion, when the `Event` `ev` fires. A
**commit** `q <- x;` is a write to this module's drive of a discrete carrier that applies at a
later microstep.

| Statement | Meaning |
| --- | --- |
| `q <- x;` | commit: applies at the next microstep |
| `q <- x after d;` | applies `d` later; `d <= 0` is the next microstep |
| `bits[3] <- true;` `mem[a] <- d;` `st.n <- 3;` | commits one element or field: those levels only |
| `let x = …;` | a local |
| `emit!(…)`, or a function that calls it | output (§19) |

**Static semantics**

- **§12.1.7** The target `place` of a commit **MUST** name a discrete carrier, or an element or
  field of one, that this module may drive. The table of §8.2.10 applies unchanged.
- **§12.1.8** Within one module, a carrier driven by commits has no definition (§5.2, §8.2.20).
- **§12.1.9** The committed expression's type **MUST** be the value type of the levels the `place`
  names. `Unknown` is of every `Repr` type.
- **§12.1.10** The head of `on` **MUST** have type `Event` (§13). A `bool` alone is not an event.
- **§12.1.11** A reaction body holds value statements (`let`, `let mut`, assignments to locals,
  `if`, `match`, loops, calls), commits and `emit!`. It holds no placement (§8.3.4), no
  contribution and no definition. It reads named results of the body.
- **§12.1.12** A reaction body **MAY** probe and read the environment (§14.1, §14.4). A named
  expression it reads is evaluated at its instant (§8.2.25).
- **§12.1.13** The delay of `after` is an `f64` expression in seconds, evaluated when the commit is
  made.
- **§12.1.14** A statement, for §12.1.4, is one `commit` of one elaborated reaction of one
  instance. Copies made by generation are different statements. A loop in a reaction body runs one
  statement several times.

> **Resolution.** §12.1.14 fixes the identity of "the same statement" under generation and loops.

**Dynamic semantics**

- **§12.1.1** **Reads see the past.** Every read sees the values at the start of the microstep,
  after its definitions settled. The commits of a microstep apply together.
- **§12.1.2** **The last write wins.** Commits to one level of a drive that apply in the same
  microstep are ordered by the instant they were made, then by source order (statement order in a
  reaction, reaction order in the module's blocks), then by execution order of one statement. The
  last one is the value. Every writer of a drive is in one module (§5.2), so the order is visible in
  one file. Different levels never interact: two reactions that write different fields of one
  carrier compose.
- **§12.1.3** **Modules never race.** Between modules there is nothing to order: a one-driver
  carrier has one writer, and `resolve` sees the drives in a fixed order (§4).
- **§12.1.4** **Delays are transport, per statement.** A commit made for time `T` removes the
  pending commits *of the same statement* to that level due at or after the new commit's due
  instant, then waits. In one
  statement a shorter delay overtakes a longer one, and the statement's schedule is always its
  latest decision about each future time. Different statements never cancel each other: their
  commits meet only when they apply, under §12.1.2. `p <- true; p <- false after w;` is a pulse in
  either order. This behavior is the **transport delay**.
- **§12.1.5** **Probes read the latest solution at the instant** the event fired: at `(t, n)`,
  the solution of the previous time point before §15 step 3.4 solves `t`, the solution at `t`
  after it (§15 step 3.3). A reaction that probes makes its instant a time point of
  the solver (§17.2).
- **§12.1.6** **A reaction never suspends.** It runs to completion at its instant. A sequence over
  time is visible state (§12.2).
- **§12.1.15** A reaction runs at most once per microstep, however many of the events in its head
  fired.
- **§12.1.16** The order in which the reactions of one microstep run is not observable through
  carriers, by §12.1.1. The order of their records is §19.
- **§12.1.17** A commit applies at microstep `n + 1` of its instant, or at microstep 0 of the
  instant `d` later, with `d` rounded to the tick (§15). A commit made from the `settle` phase is
  due at microstep 0 of the next time point (§15).
- **§12.1.18** A commit that changes a level of the carrier's value fires `changed()` in the
  microstep where it applies (§13). A commit that leaves the value as it was fires nothing.
- **§12.1.19** Unknown control in a reaction follows §3.3.3: every path the known levels allow is
  taken and every commit is merged level by level. An unknown delay commits `Unknown` at the next
  microstep. An unknown index is control.
- **§12.1.20** A delay of `+inf` never comes due. A known `NaN` delay is a run error.
- **§12.1.21** Per analysis, reactions and commits follow the second table of §16.2. Where time
  does not advance, a commit with a positive delay is never due and is dropped when the analysis
  ends (§16.1.3).

- **§12.1.22** A discrete carrier that `analog` reads and a commit changes makes a jump (§15.2).
  Along a trajectory a commit of a real value carries the sensitivity of the expression committed
  (§23.3.4).

> **Resolution.** §12.1.2 adds "then by execution order" for one statement run several times in one
> reaction. §12.1.15, §12.1.18 and §12.1.20 state what the source design leaves implicit.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0430` | the head of `on` is not of type `Event` (§12.1.10) | elaboration |
| `E0432` | a commit whose type differs from its target's (§12.1.9) | elaboration |
| `E0433` | a delay that is not `f64` (§12.1.13) | elaboration |
| `E0434` | a statement outside a reaction in a `digital` block (§12 rule 2) | parse |
| `E0435` | a known `NaN` delay (§12.1.20) | run |
| §5.2 | a commit target this module may not drive (§12.1.7; `E0220`, `E0222`); a carrier with both a definition and commits in one module (`E0221`) | elaboration |
| §8.3 | a placement in a reaction body (`E0350`) | elaboration |
| §15 | a microstep loop that never goes quiet (`E0501`) | run |

**Examples**

```rust
module Counter(in clk: Bit, out count: Bit<u8> = 0, out full: Bit = false) {}
digital for Counter {
    on clk.rise() {
        count <- count + 1;
        full  <- count == 254;      // reads the old count
    }
}
module SampleHold(vin: Electrical, vout: Electrical, in clk: Bit) { signal held: Bit<f64> = 0.0; }
digital for SampleHold { on clk.rise() { held <- v(vin); } }
analog for SampleHold { v(vout) +> held.known_or(0.0); }
```

A doc comment or an attribute goes before a reaction, as before any other item of a `digital`
block:

```rust
module Latch(in d: Bit, in en: Bit, out q: Bit = false) {}
digital for Latch {
    /// Copies `d` to `q` when `en` rises.
    on en.rise() { q <- d; }
}
```

A `clk` that starts high has risen at `(0, 0)` (§13 rule 6), so `Counter` counts that edge. A model that
must ignore it says so: `on clk.rise() && clk.prev().is_known()`.

Transport per statement, for `phi1 = clk after if clk { gap } else { 0.0 };` (D.6), a delayed
definition and therefore one statement (§8.2.4):

| Instant | `clk` | Commit made | Pending after it |
| --- | --- | --- | --- |
| `t0` | rises | `true` for `t0 + gap` | `true @ t0 + gap` |
| `t1 < t0 + gap` | falls | `false` for the next microstep of `t1` | `false @ t1`; the pending `true` is removed (time `≥ t1`) |

### 12.2 Sequences

There is no `wait` and no `sleep` in a model. A sequence over time is written with its state in the
open: a carrier that counts the steps and an `at` for the next one. A bench writes its stimulus
this way (§23.1); a sequence that waits on the running design from outside is the host's (§23).
This section adds no rule.

**Examples**

```rust
/// Sends one byte, MSB first, SPI mode 0.
module SpiMaster(out sclk: Bit = false, out mosi: Bit = false, out cs_n: Bit = true, in miso: Bit) {
    half: f64 = 50n;
    byte: u8 = 0xA5;
    signal k: Bit<u5> = 0;                         // half periods done
}

digital for SpiMaster {
    on at(1u + k as f64 * half) && k <= 16 {        // re-armed when k changes
        match k {
            0  => { cs_n <- false; mosi <- (byte >> 7) & 1 == 1; }
            16 => { sclk <- false; cs_n <- true; }
            _ if k % 2 == 1 => { sclk <- true; }
            _  => { sclk <- false; mosi <- (byte >> (7 - k / 2)) & 1 == 1; }
        }
        k <- k + 1;
    }
}
```

---

## 13. Events

An **event** is a value of type `Event`: something that fires at instants. `on` takes an `Event`.

| Primitive | Fires when |
| --- | --- |
| `s.changed()` | `s`, a discrete carrier or a param, changed in this microstep |
| `at(t: f64)` | time reaches `t` |
| `cross(e: f64, ttol: Option<f64>, etol: Option<f64>)` | `e` goes from `≤ 0` to `> 0` between two successive solutions; located by the solver when the two are time points |
| `settle` | once per time point, after the instant has gone quiet (§15) |

An **event position** is the head of an `on`, a body `let`, the result of a module, the return
value of a function, or an operand of `||` or `&&` in one of these. A **guard** is the right operand
of `&&` whose left operand is an `Event`.

**Static semantics**

- **§13 rule 3** An event is named in the module body, `let up = above(e);`, and every use of the name
  is the same event. Two placements with equal arguments are two instances (§8.3.3); they fire
  together.
- **§13 rule 7** Every other event is a function returning `Event` (§3), or a module with an `Event`
  result when it keeps state: `clk.rise()`, `s.beat(clk)`, `init()`, `every(t0, period)`,
  `above(e)`, `absdelta(e, d)`.
- **§13 rule 8** `Event` has no `Repr`. It is never carried, committed or stored. An `Event` value
  appears only in event positions and, as a `bool`, under §13 rule 2.
- **§13 rule 9** Inside a guard, a subexpression of type `Event` reads as a `bool`, as in a reaction
  body (§13 rule 2): `on d.changed() && en && !en.changed()`.
- **§13 rule 10** `cross` and `at` are core modules with an `Event` result (§14.9). They are placed in
  event positions only, under §8.3.4. `settle` is a core `Event` value; `s.changed()` is a core
  method.
- **§13 rule 11** The argument of `at` **MUST** be a discrete expression (§8.2): no probe, no `now()`,
  no `freq()`, no `gmin()`, no `source_scale()`, no `analysis(Static)`. The check is made per instance, after value inputs are
  substituted.
- **§13 rule 12** The argument `e` of `cross` is any `f64` expression, continuous or not. It **MAY** be
  unknown (§13 rule 5).
- **§13 rule 13** `changed()` applies to a discrete carrier, an element or field of one, a bundle
  field, and a param. A node has none (§5.3).
- **§13 rule 14** `settle` **MAY** be combined with `&&` and `||` like any event. The phase in which a
  reaction ran decides when its commits are due (§12.1.17).

> **Resolution.** §13 rule 9 extends "an event used as a `bool`" from reaction bodies to guards. The
> stdlib monitor `Stable` (§22.5) is written this way.

> **Resolution.** §13 rule 11 reads "`now()` is not an input" as a prohibition, together with probes:
> an `at` time is recomputed only when a carrier or param changes, exactly as a definition is
> (G.50).

> **Resolution.** §13 rule 14 gives `settle || ev` a meaning instead of rejecting it.

**Dynamic semantics**

- **§13 rule 1** *Static:* `a || b` has type `Event` when both operands do. `ev && c` has type
  `Event` when `ev` does; `c` is a `bool`. `!` applies to `bool` only. A `bool` alone is not an
  event. *Dynamic:* `a || b` fires in a microstep when either fires. `ev && c` fires when
  `ev` fires and `c` is true. `c` is evaluated in that microstep, reading what the reaction would
  read (§12.1.1, §12.1.5). When `c` is unknown the reaction runs under §3.3.3: its effects are
  merged with those of not running.
- **§13 rule 2** Inside a reaction, an event used as a `bool` says whether it fired in this microstep.
- **§13 rule 4** **`at` fires once for each value of its argument.**
  1. It is armed at `(0, 0)`, after the initial values settle, with its argument's value, rounded
     to the tick (§15).
  2. It is re-armed when the carriers and params in the argument change its value.
  3. A time already past does not fire. An unknown time does not fire. A time equal to the
     current one fires at the next microstep. A later time is a scheduled time (§17.2).
  4. `at` never fires in an analysis without time (§16.1.3).

  A periodic event is an `at` re-armed by a count: the stdlib `every(t0, period)` (§22.1).
- **§13 rule 5** **`cross` compares successive solutions**, whatever separates them:

  | The two solutions are | `cross` |
  | --- | --- |
  | two time points of a transient | located by the solver (§17.3); fires at the located instant, rounded up to a tick (§15) |
  | two time points a jump separates (§15.2) | not located: fires at the time point of the step that reads the change, at the microstep after that step's solve (§15 step 3.4), never before the change (§15.2.4) |
  | two rounds of an operating point (§16 step 4) | fires in the round that follows |
  | two points of a sweep (§16.1.2) | fires at the later point |

  It fires when `e` was `≤ 0` in the earlier solution and is `> 0` in the later. It has no
  direction argument: a falling crossing is `cross(-e)`, either is `cross(e) || cross(-e)`. It
  cannot fire on the first solution of an analysis. An `e` that is unknown in either solution does
  not fire. A guard made only of discrete carriers also tells the solver not to locate the crossing
  while the guard is false.
- **§13 rule 6** **The first commit is a change.** At `(0, 0)` every initial value is committed onto an
  unknown carrier (§16 step 2), whoever supplied it. So `changed()` fires there for every carrier
  that starts known, and `rise()` for a `Bit` that starts high, as in IEEE 1364. Level-sensitive
  logic and asynchronous resets are right from time 0 (E.1, E.2).
- **§13 rule 15** `s.changed()` fires in the microstep in which the value of `s` changed: by a commit
  that applied (§15 step 3.1) or by a definition or `resolve` that settled (§15 step 3.2). On a
  resolved carrier it follows the resolved value. On an element or field it follows those levels.
- **§13 rule 16** `p.changed()` on a param fires at the instant a host `set` during a run changes it
  (§9 rule 4). It does not fire at `(0, 0)`.
- **§13 rule 17** `settle` fires once per time point, after the microstep loop has gone quiet (§15 step
  4). Its reactions read the final state. Their commits do not reopen the loop: they are due at
  microstep 0 of the next time point.
- **§13 rule 18** An event fires in a microstep or does not; firing is always known. Two events that
  fire in one microstep are simultaneous, and a reaction tells them apart under §13 rule 2.
- **§13 rule 19** Per analysis, every event follows the second table of §16.2.

- **§13 rule 20** Along a trajectory, a located event moves with the params, and an `at` time moves by
  its own derivative (§23.3).

> **Resolution.** §13 rule 4 step 1 states the initial arming. With step 3, `at(0.0)` in a transient
> fires at `(0, 1)`, inside the operating point's instant, as §15 also states.

> **Resolution.** §13 rule 16: a param is never unknown, so it has no first commit and no change at
> `(0, 0)`, also when a `set` made before the run bound it (§5.3.11, §16 step 2; G.52).

**The start is said three ways, none of them a rule of its own.**

| Need | Written | Mechanism |
| --- | --- | --- |
| once per analysis | `init()` (§22.1) | a carrier whose first commit is its only one (§13 rule 6) |
| through the whole operating point, every round | the guard `analysis(Static)` | §11.1.1 |
| a crossing that is right at time 0 | `above(e)` (§22.2) | `cross(e) \|\| (init() && e > 0.0)` |

Any of them is restricted to an analysis with a guard: `init() && analysis(Transient)`.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0440` | `!` applied to an `Event` outside a guard or reaction body; `&&` whose left operand is a `bool` and right an `Event` in an event position (§13 rule 1) | elaboration |
| `E0441` | an `Event` value outside an event position, a guard or a reaction body; an `Event` committed or carried (§13 rule 8) | elaboration |
| `E0442` | the argument of `at` is a continuous expression (§13 rule 11) | elaboration |
| `E0443` | `cross` or `at` placed outside an event position (§13 rule 10) | elaboration |
| §12.1 | a `bool` as the head of `on` (`E0430`) | elaboration |
| §5.3 | `changed()` or `prev()` on a node (`E0230`) | elaboration |

**Examples**

```rust
/// Comparator, right from the operating point: the event says which way it went.
module Comparator(inp: Electrical, inn: Electrical, out q: Bit = false) {
    ttol: f64 = 1p;
    let up = above(v(inp, inn), ttol);
    let dn = above(-v(inp, inn), ttol);
}
digital for Comparator { on up || dn { q <- up; } }

/// Three-state phase detector: two events in one reaction, each tested as a bool.
module Pfd3(in fref: Bit, in fvco: Bit, out up: Bit, out dn: Bit) {
    signal state: Bit<i2> = 0;
    up = state == 1;
    dn = state == -1;
}
digital for Pfd3 {
    on fref.rise() || fvco.rise() {
        let mut s = state;
        if fref.rise() && s < 1 { s += 1; }    // which one fired?
        if fvco.rise() && s > -1 { s -= 1; }   // both at once: up first, then down
        state <- s;
    }
}
```

```rust
/// Water heated through a thermal port boils at 100 °C: the event changes the state,
/// the state changes the contribution.
#[derive(Repr, Clone, Copy, PartialEq)]
enum Phase { Liquid, Boiling, Vapor }

nature Heat {
    potential: Energy { access: joules, units: "J", abstol: 1m },
    flow:      Power  { access: pwr,    units: "W", abstol: 1n },
}

module Water(t: Thermal) {
    mass: f64 = 1.0;     // kg
    tb:   f64 = 373.15;  // K
    cl:   f64 = 4.18k;   // J/(kg·K)
    cv:   f64 = 2.01k;
    hv:   f64 = 2.26M;   // J/kg
    pub signal phase: Bit<Phase> = Phase::Liquid;
    let ph = phase.known_or(Phase::Liquid);
    let absorbed = idt::<Heat>(pwr(t), 0.0, reset: ph != Phase::Boiling);   // energy since boiling began
}

analog for Water {
    match ph {
        Phase::Liquid  => { pwr(t) +> ddt(mass * cl * temp(t)); }
        Phase::Boiling => { temp(t) +> tb; }                 // held; the heat vaporizes
        Phase::Vapor   => { pwr(t) +> ddt(mass * cv * temp(t)); }
    }
}

digital for Water {
    on above(temp(t) - tb) && phase == Phase::Liquid { phase <- Phase::Boiling; }   // also at time 0 if hot
    on above(absorbed - mass * hv) { phase <- Phase::Vapor; }
}
```

`Water`'s branch `(t, gnd)` is a switch branch selected by a `Bit`, as §11 rule 3 requires. What
happens to `temp(t)` at the instant the phase changes is the jump rule (§15.2).

---

## 14. Core primitives

A **core primitive** is what only the simulator can provide. Everything else is the standard
library (§22), written in the language over this section.

| Primitive | Kind | Meaning | Allowed in | § |
| --- | --- | --- | --- | --- |
| `potential`, `flow`, every `access` alias; `p.inflow()` | `fn` | probes (§6) | `analog`, events, reactions, body `let`s, value inputs | 14.1 |
| `ddx(e, probe)` | `fn` | partial derivative | `analog` | 14.2 |
| `ac_stim(mag, phase: Option<f64>)` | `fn` | small-signal stimulus | `analog` | 14.3 |
| `temperature()`, `simparam(name, default: Option<f64>)`, `analysis(a)` for an analysis kind | `fn` | environment, constant during an analysis | everywhere | 14.4 |
| `now()`, `freq()`, `gmin()`, `source_scale()`, `analysis(Static)` | `fn` | environment of the solve (§16.2) | everywhere but definitions (§8.2.5) | 14.4 |
| `ddt(x, abstol: Option<f64>)` | module | time derivative; its argument is a state of the jump rule (§15.2) | where instances go (§8.3.4) and `analog` reads them | 14.5 |
| `delay(x, td, max: Option<f64>)` | module | `x` as it was `td` ago | the same | 14.6 |
| `limit(x, f, seed: Option<f64>)` | module | Newton step limiting (§18) | the same | 14.7 |
| `noise(psd, name: Option<&str>)` | module | noise source | the same | 14.8 |
| `cross`, `at` | module, result `Event` | located and scheduled events (§13) | event positions | 14.9 |
| `s.changed()`, `settle` | `Event` values | (§13) | event positions | 14.9 |
| `bound_step(dt)` | statement | the next step is at most `dt` | `analog` | 14.10 |
| `emit!` | statement | output (§19) | reactions, `analog` | 14.11 |
| `include_str!` | `const` expression | the text of a file, read at elaboration (§20) | `const` items | 14.11 |
| `s.prev()`, `s.others()`, `x.level(k)` | methods | history, drives, the known flag | wherever the carrier is read | 14.12 |

**Static semantics**

- **§14 rule 1** The names in the table, and `gnd`, `Nature`, `m`, `dtemp` and `str`, are core
  names. A model **MUST NOT** redefine them at item
  level. An `access` alias is declared by its nature (§4).
- **§14 rule 2** A core `fn` follows §3: a function that calls one is callable only where that
  primitive is allowed.
- **§14 rule 3** A core module follows §8.3.1–§8.3.5 and no other rule (§8.3.6). It takes `m`,
  `dtemp` and no param block of its own. It has no `pub` items.
- **§14 rule 4** "Everywhere" in the table means every position evaluated during a run. No
  environment function is allowed in a const context (§8.1.21).
- **§14 rule 5** The value inputs of `ddt`, `delay`, `limit` and `noise` are read by the solver and
  **MUST** be known (§11 rule 13).

**Dynamic semantics**

- **§14 rule 6** The first table of §16.2 is the consolidated per-solve definition of every
  primitive. §14.1–§14.10 cite its rows, restate one only to add detail, and add what a
  reaction reads. Where they differ, §16.2 governs.
- **§14 rule 7** A continuous core result read in a reaction, a guard or an event argument has the
  value it has in the solution that context reads (§15 step 3.3): the solution of the previous
  time point before step 3.4 solves `t`, the solution at `t` after it and in `on settle`.
- **§14 rule 8** State of a core module advances only on accepted steps (§17.2.4) and is restored by
  a rejected one (§17.2.3). It is kept by an analysis run at a transient's last instant (§16.4).
- **§14 rule 9** A core module's state advances whatever control flow surrounds its placement
  (§8.3.14). Its value inputs are evaluated at every accepted solution for that purpose.

> **Resolution.** §14 rule 3 and §14 rule 4 bound what "follows rules 1–5" and "everywhere" mean.
> §14 rule 9 makes §8.3.14 concrete for modules the simulator implements.

**Errors (all of §14)**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0450` | a core primitive written where the table does not allow it. Reached through a function, it is that call's `E0122`; `emit!` is `E0601` | elaboration |
| `E0451` | the second argument of `ddx` is not a potential or flow probe of a branch (§14.2.2) | elaboration |
| `E0452` | `delay` without `max` whose `td` is not constant during an analysis (§14.6.2) | elaboration |
| `E0453` | `delay` with `td < 0`, or `max` not positive | run |
| `E0454` | `noise` with a negative or non-finite `psd` in a `Noise` sweep | run |
| §17 | an invalid `bound_step` argument (`E0509`); an invalid `ttol` or `etol` of `cross` (`E0510`) | run |
| `E0456` | `simparam(name)` with no default and no such session parameter | run, at the start of the analysis |
| `E0457` | a core name redefined by a model (§14 rule 1) | elaboration |
| §8.1 | an environment function in a const context (`E0323`) | elaboration |
| §8.2 | `now()`, `freq()`, `gmin()`, `source_scale()`, `analysis(Static)` or a probe in a definition (`E0336`) | elaboration |
| §8.3 | a core module placed in a `fn`, a closure or a reaction body (`E0350`) | elaboration |
| §3.3 | an unknown value reaching `ddt`, `delay`, `limit` or `noise` (`E0150`) | elaboration |

### 14.1 Probes

`fn potential<N: Nature>(a: N, b: Option<N>) -> f64` and `fn flow<N: Nature>(a: N, b: Option<N>) -> f64`
read a branch (§6). A nature's `access` names are aliases. `p.inflow()` reads the total flow
entering the module at its port `p`.

**Static semantics**

- **§14.1.1** A probe is a continuous expression (§8.2). It is allowed in `analog`, event
  expressions, reaction bodies, body `let`s and value inputs. It is an error in a definition, an
  anonymous definition, the argument of `at` and a const context.
- **§14.1.2** Its nodes follow §11 rule 9. `b` left out is `gnd`. A probe of `(a, a)` reads 0 (§6 rule 9).
- **§14.1.3** A probe call is also the left side of a contribution (§11 rule 8) and the second
  argument of `ddx` (§14.2).

**Dynamic semantics**

- **§14.1.4** A probe is always known (§3.3).
- **§14.1.5** In `analog` a probe is the first row of §16.2. In a reaction or a guard it reads
  the solution §12.1.5 states, and makes the instant a time point. In `on settle` it reads the
  final solution of the time point; as the argument of `cross`, each successive solution
  (§13 rule 5).

- **§14.1.6** A flow probe and `p.inflow()` are divided by the effective multiplicity (§8.5.4).

### 14.2 `ddx`

`fn ddx(e: f64, probe) -> f64` is the partial derivative of `e` with respect to one branch quantity.

**Static semantics**

- **§14.2.1** `ddx` is allowed in `analog`, and in a body `let` or value input that only `analog`
  reads (Appendix B.2.2, note ²).
- **§14.2.2** The second argument **MUST** be a probe call: a potential or a flow of a branch of
  this module. `p.inflow()` is not accepted.
- **§14.2.3** The derivative is symbolic. A probe of the same branch contributes 1, the reversed
  branch −1. A probe of any other branch is held constant. `limit` is transparent (§18 rule 3). The
  result of any other instance is held constant.

> **Resolution.** §14.2.3 defines the derivative over probe leaves. `ddx(v(g) - v(s), v(g, s))` is
> therefore 0: the two single-node probes are other branches.

**Dynamic semantics.** Per solve: the `ddx` row of §16.2.

### 14.3 `ac_stim`

`fn ac_stim(mag: f64, phase: Option<f64>) -> f64` is a small-signal stimulus.

**Static semantics**

- **§14.3.1** `ac_stim` is allowed where `ddx` is (§14.2.1). `phase` is in radians; left out, it is 0.
- **§14.3.2** Several calls in one contribution add as phasors.

**Dynamic semantics.** Per solve: the `ac_stim` row of §16.2. The phasor is `mag·exp(j·phase)`,
with `mag` and `phase` evaluated at the operating point.

- **§14.3.3** In a host-built analysis that supplies its own stimulus (§16.3), `ac_stim` is 0.

> **Resolution.** Radians, as in VAMS 4.5.13. §14.3.3 reads "in place of `ac_stim`" in §16.3.

### 14.4 Environment

| Function | Returns | Class |
| --- | --- | --- |
| `temperature() -> f64` | the analysis temperature plus the instance's effective `dtemp` (§8.5.5), in kelvin | constant during an analysis |
| `simparam(name: &str, default: Option<f64>) -> f64` | the session parameter `name`, else `default` | constant during an analysis |
| `analysis(a: Analysis) -> bool`, `a` in `Dc`, `Transient`, `Ac`, `Noise` | whether the analysis is of that kind (§11.1.1) | constant during an analysis |
| `now() -> f64` | the time of the solve | environment of the solve |
| `freq() -> f64` | the frequency of the sweep point | environment of the solve |
| `gmin() -> f64` | the conductance of the shunt the solver adds on every node, in siemens | environment of the solve |
| `source_scale() -> f64` | the factor, in `[0, 1]`, by which the solver scales independent sources | environment of the solve |
| `analysis(Static) -> bool` | whether the solve is an equilibrium solve (§11.1.1) | environment of the solve |

**Static semantics**

- **§14.4.1** The functions constant during an analysis form discrete expressions. They are
  allowed in every run-time position, definitions included.
- **§14.4.2** The functions of the environment of the solve form continuous expressions (§8.2).
  They are allowed everywhere but definitions, anonymous definitions and the argument of `at`.
- **§14.4.3** All are known. All are constant during a solve, so any of them **MAY** appear in a
  selector (§11 rule 3).
- **§14.4.6** This specification names no session parameter of `simparam`. The names are the
  host's (G.66).
- **§14.4.7** `gmin()` and `source_scale()` are the continuation variables (§16 step 3). They
  change between solves, as `now()` and `freq()` do, so they are not constant during an
  analysis and §14.4.1 does not apply to them. At rest `gmin()` is the session `gmin` and
  `source_scale()` is 1.0: every solve reads these values except the solves of a continuation
  method of an equilibrium, where `gmin()` is the shunt of the step and `source_scale()` the
  scale of the step. The last solve of every sequence is at rest (§16 step 3).

**Dynamic semantics.** Per solve: the environment rows of §16.2.

- **§14.4.4** In a reaction, `now()` is the time `t` of its instant, `freq()` is 0,
  `gmin()` is the session `gmin` and `source_scale()` is 1.0 (their values at rest, §14.4.7),
  and `analysis(Static)` follows §11.1.3.
- **§14.4.5** In a periodic analysis the last column is taken at each point of the trajectory,
  with `now()` its time. In an analysis run at a transient's last instant, `now()` is that instant
  (§16.2, §16.4).

### 14.5 `ddt`

`module ddt(x: f64, abstol: Option<f64>) -> f64` is the time derivative of `x`. Its argument is a
state of the jump rule (§15.2).

**Static semantics**

- **§14.5.1** `x` is a value input: any known `f64` expression (§14 rule 5). The result is a
  continuous expression with a direct dependence on `x` (§8.3.6).
- **§14.5.2** `abstol` is the absolute tolerance of the state `x` in the truncation-error test
  (§17.2.1). Left out, the simulator derives
  it from the tolerances of the natures `x` reads.

> **Resolution.** §14.5.2 assigns `abstol` its role. The source design gives the parameter and no
> text for it.

**Dynamic semantics.** Per solve: the `ddt` row of §16.2. In a transient step the derivative is
the integration method's (§17.2); in a jump solve, which is a transient step (§15.2), it is the
same.

- **§14.5.3** State: the accepted history of `x`. Across a jump `x` is continuous wherever the
  equations allow (§15.2.1). The history is kept across a jump; the previous derivative is
  dropped only in the steps §15.2.6 restarts.
- **§14.5.4** Read in a reaction or an event argument, the result is the derivative in the
  solution read: 0 at an equilibrium solution, the derivative of the step that read the jump, after one (§15.2.4).
- **§14.5.5** In `Noise`, `ddt(n)` of a noise result is `jω·n` (§11.1.9).

### 14.6 `delay`

`module delay(x: f64, td: f64, max: Option<f64>) -> f64` is `x` as it was `td` ago.

**Static semantics**

- **§14.6.1** `x` and `td` are value inputs and **MUST** be known (§14 rule 5).
- **§14.6.2** Without `max`, `td` **MUST** be constant during an analysis: an expression over
  params, constants and the environment constant during an analysis. The check is made per
  instance, after value inputs are substituted (§8.3.9). With `max`, `td` **MAY** be any
  expression, and its value is limited to `max`.

**Dynamic semantics.** Per solve: the `delay` row of §16.2. In a sweep, `td` is its value at the
operating point.

- **§14.6.3** State: the accepted history of `x` over the last `td`, or `max`. A jump leaves it
  untouched (§15.2.5). Between time points the history is interpolated by the simulator.
- **§14.6.4** `td == 0` returns `x`. With `max`, a `td` above `max` reads as `max`. A negative
  `td` is a run error.
- **§14.6.5** `delay` adds no time point. A corner of `x` reappears `td` later without one.

> **Resolution.** §14.6.2 and §14.6.4 complete "with `max`, `td` may vary up to it" in the way
> VAMS 4.5.7 does for `absdelay`.

### 14.7 `limit`

`module limit(x: f64, f, seed: Option<f64>) -> f64`, with `f: fn(new: f64, old: f64) -> f64`, bounds
how far Newton moves `x` per iteration. §18 defines it; it changes the path, never the answer.

**Static semantics**

- **§14.7.1** `f` is a function or a closure. It keeps no state (§3).
- **§14.7.2** For differentiation `limit` is transparent (§18 rule 3).

**Dynamic semantics.** Per solve: the `limit` row of §16.2.

- **§14.7.3** State: the last value returned. A jump leaves it untouched (§15.2.5).
- **§14.7.4** An active instance vetoes convergence (§17.1.3, §18 rule 5). An instance whose result is
  not read in an iteration is inactive in it and keeps its state.
- **§14.7.5** Read in a reaction or an event argument, the result is `x`.

> **Resolution.** §14.7.4, second sentence: a `limit` under an untaken branch cannot hold a solve
> open.

### 14.8 `noise`

`module noise(psd: f64, name: Option<&str>) -> f64` is a noise source of power spectral density
`psd`.

**Static semantics**

- **§14.8.1** `psd` is a value input and **MAY** read `freq()`. `name` is a string literal; it
  only groups sources in reports.
- **§14.8.2** Correlation is instance identity (§11.1.8).

**Dynamic semantics.** Per solve: the `noise` row of §16.2. In `Noise`, `psd` is evaluated at
the operating point and the point's `freq()` (§11.1.9).

- **§14.8.3** `psd` is read only where the instance is a source, so `k / freq().powf(exp)` is
  never evaluated at `freq() == 0`.
- **§14.8.4** `psd` is a one-sided density in the squared unit of the contribution per hertz. It
  scales with the effective multiplicity as §8.5.4 states.
- **§14.8.5** Read in a reaction or an event argument, the result is 0.

### 14.9 `cross`, `at`, `changed`, `settle`

`module cross(e: f64, ttol: Option<f64>, etol: Option<f64>) -> Event` and
`module at(t: f64) -> Event` are the located and the scheduled event. `s.changed()` and `settle`
are `Event` values. §13 defines all four; this section adds their state.

| Primitive | State | Arguments |
| --- | --- | --- |
| `cross` | `e` at the last accepted solution | `e` may be continuous and unknown; `ttol` defaults to the simulator's, `etol` to the nature's `abstol` (§17.3) |
| `at` | the armed time, and whether it fired | a discrete expression (§13 rule 11) |
| `s.changed()` | none: the carrier's previous value is §5.3 | — |
| `settle` | none | — |

Per analysis, all four follow §13 rule 19.

- **§14.9.1** A located `cross` makes its instant a time point. It makes a jump only if what
  `analog` reads changes (§17.2).
- **§14.9.2** An `at` time is a scheduled time. It is a time point only under §17.2.2.

### 14.10 `bound_step`

`bound_step(dt);` caps the next transient step at `dt`.

**Static semantics**

- **§14.10.1** `bound_step` is a statement of `analog` (§11 rule 12). `dt` is a known `f64`.

**Dynamic semantics.** Per solve: the `bound_step` row of §16.2.

- **§14.10.2** It is re-evaluated every step (§17.2.5). The cap is the smallest `dt` among the
  calls executed at the accepted solution of the step. A call under an untaken branch caps nothing.
- **§14.10.3** It adds no time point and no jump (§17.2).

> **Resolution.** §14.10.2 takes the cap from the accepted solution, not from intermediate
> iterates.

### 14.11 `emit!` and `include_str!`

`emit!("name", field = value, …)` writes a record (§19). `include_str!(path)` is the text of a
file, read at elaboration (§20). Both are defined in Part VI. For Part IV: `emit!` is a statement of
reactions and of `analog`; in `analog` it is a trace (§19 rule 4) and is not run in a small-signal sweep
(§11 rule 20).

### 14.12 Carrier and value methods

`s.changed()` (§13 rules 15 and 16), `s.prev()` (§5.3), `s.others()` (§5.2), `x.level(k)`
(§3.3.4) and `p.inflow()` (§6, §14.1) are defined where cited. They are allowed wherever the
carrier is read. `impl` names **MUST NOT** repeat them (§3.1).

### 14.13 Not core

Written in the language in §22: `init`, `every`, `since`, `idt`, `idtmod`, `slew`, `above`,
`absdelta`, `transition`, `inertial`, `last_crossing`, `laplace_*`, `zi_*`, `limexp`,
`white_noise`, `flicker_noise`, `table_model`, and every output function. Each inherits its
behavior in every analysis from the rows of §16.2.

There is no `discontinuity()`: a discontinuity is a discrete carrier change that `analog` reads
(§9, case D).
