# Part VIII. The host

A model describes hardware. Everything that drives, sequences and judges a model from outside
lives in the host. This part states what the language requires of a host and what a host may
rely on. It does not specify a host API: the names in the Python sketches belong to the host
library, not to the language.

**Terms defined here:** host, session, bench, testbench, host action, `set`, host drive, `force`,
`release`, objective, constraint, sensitivity, gradient, saltation correction, grazing
crossing.

**Error codes.** This part owns `E08xx`. Their phase is *host* (§0.3): they are reported to
the host program, not written to a stream, and the analysis continues.

---

## 23. Testbenches and optimization

The **host** is a program, in Rust or Python, that elaborates a design, sets its params,
drives its top ports, runs analyses and reads everything. A **session** is the host's handle
on one elaborated design, with its options. A **bench** is a module without ports, written in
MSL, that places the design with its sources, its stimulus and its monitors; the host picks it
as the top and asks the analysis. A **testbench** is host code that drives, sequences and
judges a design from outside it, doing what a bench cannot write. The host is outside the
language: no model construct refers to it.

Session options this specification refers to:

| Option | Read by | Default |
| --- | --- | --- |
| time resolution | §15 T1 | 1 fs |
| `reltol`, `trtol`, the default `ttol` | §17.1, §17.2, §17.3 | Appendix C.2 |
| bounds on microsteps per time and rounds per operating point | §15 I7, §16 step 4 | Appendix C.2 |
| nodesets, by path | §16 step 3, §18 | none |
| analysis temperature | `temperature()` (§14) | Appendix C.2 |
| which params are varied, their distributions, which are excluded from gradients | §23.1.5, §23.2 | none |

These are not model text (§8.1.3).

### 23.1 Testbenches

A bench writes in MSL what happens inside one run: its stimulus, as reactions and timers, and
its monitors and checks. The host keeps what acts on the design from outside or spans many
runs.

| In the model (a bench) | In the host |
| --- | --- |
| stimulus written as reactions and timers: `at`, `every`, a carrier that counts the steps of a sequence (§12.2) | `force` and `release` on any carrier, recorded as debug actions |
| checks at an instant: `require`, `cover`, `target` | constrained random stimulus, scoreboards, coverage models |
| monitors for temporal properties (§22.5) | interactive sequences that wait on the running design, `fork`/`join` |
| timing checks on history | regression, sweeps, Monte Carlo draws, optimization, time resolution, nodesets, which params are varied |

A **host action** is something the host does to a running or about-to-run design at a time it
names: a `set`, a host drive, a `force`, a `release`, a read or a probe. A **`set`** assigns a
param by path. A **host drive** writes one of the drives the host holds (rule 1).

**Semantics.**

1. The host drives the top module's `in` ports as their one driver, and holds one drive of
   each top-level resolved carrier. It drives each node port of the top with one **host
   source**: a potential source between the port and `gnd` that holds a DC value, an optional
   AC stimulus (magnitude and phase) and a piecewise-linear waveform whose corners are
   strictly increasing in time. A run needs a host source for every node port of the top
   (`E0810`). A value input of the top has no host source and is refused at elaboration
   (`E0811`).
2. The host **MAY** wait for any event of the model, read any carrier and probe any node
   (§21). Reading never changes the result, except that a probe at a scheduled time makes it
   a time point (§17.2.6).
3. A **`force`** overrides a carrier until its **`release`**. It is outside model semantics
   and recorded on `log` (§19 rule 2).
4. Made before the run, a host drive is an initial value like the model's own, and a `set`
   binds the param without a change (§16 step 2). Made during the run, either happens at a
   scheduled time: a drive is a commit that enters that instant at step 2 of §15; a `set`
   applies after the step to that time is accepted, at step 5 of §15 (§15.2).
5. Monte Carlo is a host loop. It assigns a distribution to any param by path, draws per
   instance or globally, and sets the values before elaboration.
6. A `set` is checked against the param's `where` (§8.1.1). A `set` that fails is
   refused (`E0802`) and changes nothing.
7. A `set` of a structural param during a run is refused (`E0803`); the host re-elaborates
   (§8.1.2). A `set` that would change the structure of an extern module (§8.5.10) is
   refused the same way, with `E0803`; in a `Dc` sweep that point re-elaborates the instance.
8. A `set` of a non-structural param during a run follows §9 rule 4.
9. A top-level `in` port starts as §3.3.4 says: its `= value`, else the value the host gave it
   before the analysis, else `Default::default()` of its type. When none of the three exists
   the analysis is refused with `E0187` naming the port.
10. At time 0, a reset held low by the host and one held low by `= false` inside the design
    are the same thing (rule 4; §13 rule 6).
11. Between two analyses, a host `set` or drive follows §16.4: at a transient's last instant
    it is a commit collected there; otherwise it is an initial value of the next analysis.
12. The result of a session is a function of the source, the param values, the session
    options and the ordered host actions (§15 I1).

> **Resolution.** A host action taken in response to an event the host observed at `(t, n)`,
> with no wait, is due at `(t, n + 1)`, like a commit with no delay (§15 T3). A host action
> scheduled for a time already past is refused (`E0806`). Both this note and the next add host
> semantics (G.63).

> **Resolution.** A `force` or `release` enters an instant as a host drive does (rule 4).
> While forced, a discrete carrier's value is the forced value, whatever its drives; a change
> fires `changed()`. The drives go on updating underneath and show again at the `release`.
> On a node, a `force` is a potential source to `gnd` applied by the host: its application
> and its release are each a time point and a jump, and a loop of potential contributions
> through it is singular (`E0503`).

> **Resolution.** A host source (rule 1) is a potential source to `gnd`, never a current source:
> a current source is an MSL module placed in a design. A transient lands on every corner of
> a piecewise-linear host source, each a time point (§15.2), and a host change of its value
> at a time is a jump there. An `ac` analysis takes the source's magnitude and phase as the
> port's stimulus. A port tied to `gnd` inside the design is still driven by the host or the
> run is refused (`E0810`).

> **Resolution.** Rule 9 and the refusal semantics of rules 6–7 (nothing changes) are stated
> here. Which module is the top is the host's choice (G.17).

> **Resolution.** A host reads a carried tuple, array, struct, enum or `Option`, and a record
> field of such a type, as a tree of its parts: the language's literal syntax in a text table,
> a record line and JSON Lines (a struct an object, a tuple and an array an array, a payload
> variant `{"Variant": {..}}`), and one column per dotted leaf in CSV and a plot (`st.n`,
> `bits[3]`, `t.0`; an enum a variant column and its payload leaves `st.Data.n`). An enum with no
> payload reads as the name of its variant, not its tag; the names live in the module's metadata,
> and the bytes of a net do not change. A printed path that names a leaf (`Top.st.n`,
> `Top.v[2]`) prints that leaf alone. A composite param is set by the path of one leaf
> (`Top.f.num[2]`, `Top.p.x`, `Top.t.0`), before the run; the `where` of the param is checked on
> the whole value after the write and a false one is `E0317`, a path that names no leaf `E0801`.

> **Resolution.** A cell of a table is a value of its type, and a host that cannot read one fails
> with the cause: a row with no cell for a column, a plain value in a column of composite values,
> a real column that is not real at the next step. It never fills the cell in. Two cells have no
> value of their own, and they are **absent**, which is not an unknown value (no level of any type
> is missing): a leaf the type can hold and the value of this row does not (the payload of a
> `None`, a field of another variant), and a param no placement has given a value. An absent cell
> is an empty field in a text table and in CSV, `null` in JSON Lines, and nothing in a plot.

A sketch in Python, not normative:

```python
import piperine as pp

sim = pp.Session("uart.pp", top="UartRx", params={"clks_per_bit": 87}, time_resolution=1e-15)
sim.clock("clk", period=100e-9)
bit = 87 * 100e-9

async def send(byte):
    for level in [0] + [(byte >> k) & 1 for k in range(8)] + [1]:
        sim.drive("rx", level)
        await sim.wait(bit)

async def test_receives_a5():
    got = sim.rise("dv")          # armed before the frame: dv pulses inside the stop bit
    await send(0xA5)
    await got
    assert sim.read("byte") == 0xA5
```

Nodesets and Monte Carlo distributions are given by path in the same way:
`sim.nodeset({"x1.q": 0.6})`, `sim.mc({"r1.r": gauss(rel=0.05)})`.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0801` | A host operation names a path that is not in the metadata (§21). | host |
| `E0802` | A `set` gives a value of the wrong type, or one that fails the param's `where`. Refused. | host |
| `E0803` | A `set` of a structural param during a run, or one that would change the structure of an extern module. Refused. | host |
| `E0804` | A host drive of a carrier of which the host holds no drive (rule 1). | host |
| `E0805` | A subscription to a stream after the run has started (§21). | host |
| `E0806` | A host action scheduled for a time already past. | host |
| `E0810` | A run starts with a node port of the top that has no host source (rule 1). | host |
| `E0811` | The top module has a value input, which the host cannot drive as a source (rule 1). | elaboration |

### 23.2 Optimization

The language already states an optimization problem; the host solves it.

| Piece | Where |
| --- | --- |
| variables and bounds | params with `where` (§8.1) |
| objectives | `target` margins (§19), maximized |
| constraints | `target` margins kept at or above zero; `require` as pass or fail |
| gradients | `∂F/∂p` for every gradient param (§21), and the solver's adjoint sensitivity, across events as in §23.3 |
| yield | Monte Carlo draws (§23.1.5) |

1. An **objective** is a `target` the host maximizes. A **constraint** is a `target` the host
   keeps at or above zero, or a `require`. Which role a `target` plays is the host's choice;
   the model writes only the margin.
2. The host keeps each target's last record of a run (§22.6). That record is the value of the
   objective or constraint for the run.
3. A `require` record is a pass or a fail (§19): its `pass` field is a `bool`, and there is
   no third verdict.
4. A `fatal` record ends the run (§19).
5. The host reads a param's `where` as its bounds. A candidate that fails a `where` is never
   simulated (§23.1.6).
6. Varying a structural param re-elaborates (§8.1.2); it has no gradient (§23.3.6).

> **Resolution.** Rules 3–4 state how a host counts records: each `require` is counted as a
> pass or a fail, and a `fatal` ends the run.

```python
opt = pp.Optimize(sim, vary=["w", "l"], objectives=["gain_db"], constraints=["ipk_lim"], mc=200)
best = opt.run()          # gradient-based, from the model's own derivatives
```

### 23.3 Gradients across events

A mixed-signal trajectory is piecewise smooth in its params. A **sensitivity** is the
derivative of a quantity of the trajectory with respect to one gradient param `p` (§21). A
**gradient** is the sensitivity of a record's real field, `∂(record)/∂p`. Both are defined
piece by piece:

1. **Between events**, sensitivities are those of the solver's equations, forward or adjoint.
2. **A located event moves with the params.** For a crossing of `e` at `t*`,
   `dt*/dp = −(∂e/∂p) / (de/dt)`, where `∂e/∂p` includes the state's own sensitivity. At `t*`
   the state's sensitivity jumps by the change of the right-hand side across the event, times
   `dt*/dp`: the **saltation correction**. Across a jump (§15.2) the sensitivities of the
   states are carried by the step that reads it, as the states are (§15.2.10).
3. **A scheduled time moves by its own derivative.** An `at` or `after` time computed from
   params has `dt/dp` equal to the derivative of the expression that gives it.
4. **A commit of a real value carries the sensitivity of the expression committed, taken at
   the commit's instant.** Its slope times `dt*/dp` is added when that instant itself moves
   (rule 2). A value of any other type has none: it acts on a record only through the times
   of the events that change it. The same rule gives the periodic small-signal response of
   §16.3, with the perturbation in place of the param.
5. **Where no gradient exists**, the host gets a diagnostic naming the event instead of a
   number: a **grazing crossing** (`de/dt = 0` at `t*`), or an event that appears or
   disappears as a param moves.
6. **A param that only selects** (`match`, `if`, generation), or only feeds discrete values
   that are not real, has no gradient. The dependency analysis of §21 finds these, and the
   host is told which.

**Rule 4 in detail.** Let a reaction at instant `(t*, n)` execute `q <- x`, with `x` of type
`f64` and `q` a discrete carrier of a real type.

| Symbol | Meaning |
| --- | --- |
| `∂x/∂p` | the derivative of `x` at the fixed instant, through every probe, param and real carrier `x` reads |
| `ẋ` | the time derivative of `x` along the solution the reaction reads (§15 step 3.3: the previous time point's solution before step 3.4, the solution at `t` after it) |
| `dt*/dp` | how the instant moves: rule 2 for a located crossing, rule 3 for a scheduled time, 0 for a fixed time |
| `s_q` | the sensitivity the carrier `q` holds |

- **4a.** The commit sets `s_q = ∂x/∂p + ẋ · dt*/dp`.
- **4b.** `q` holds `s_q` with its value, from the instant the commit applies until the next
  commit to it applies. A discrete carrier is still constant during every solve and has no
  small-signal response around an operating point (§5.1.2).
- **4c.** Every reader differentiates through `q` with `s_q`. `analog` adds
  `(∂/∂q) · s_q` to the sensitivity of each contribution that reads `q`. A reaction's
  expression over `q` carries it by the chain rule into its own commits. A real-valued
  definition and a real-valued `impl Resolve` of a summing nature the model declares (`sum`, §22.3) carry it the same way.
- **4d.** A delayed commit `q <- x after d` carries the sensitivity taken at the instant the
  commit was made. Its due time moves by `dt*/dp + ∂d/∂p` (rule 3).
- **4e.** A settle commit (§15) carries `∂x/∂p` at its time point. The time point does not
  move with `p` unless it is an event time, so `dt*/dp` is 0 for a step chosen by step
  control.
- **4f.** A commit of a non-real value (`bool`, an integer, an `enum`) sets no sensitivity.
  Its effect on a record is the motion of the event times it causes, by rules 2 and 3.
  `bit <- v(x) > 0.0` carries none; the edges that `bit` later causes move.
- **4g.** A `Logic` or an `Option` carrier has no sensitivity: a `Logic` reaches a real
  expression through a function that returns an `f64` (§3.3.6), and that expression carries
  the sensitivity of what it reads.
- **4h.** In a periodic small-signal or periodic noise analysis (§16.3) the same rules hold
  with the perturbation phasor in place of `dp`. A real carrier then carries the perturbation
  committed to it and event times move with it. `SampleHold` (§12.1) has its zero-order-hold
  response this way, with nothing written in the model.

> **Resolution.** "Real type" in rule 4 is a float type (`Float<N>`). In a composite value,
> each float-typed leaf carries its own sensitivity and every other leaf carries none.

> **Resolution.** The side of `ẋ` in 4a is the one the reaction reads: for `n = 0` the left
> derivative at `t*`; after a jump, the derivative of the right value. Rules 4b–4g are stated
> here as consequences of rule 4 and of differentiation through the readers.

**Dynamic semantics.** Computing sensitivities **MUST NOT** change the trajectory: values,
event times and records are those of the run without gradients.

**Errors**

| Code | Condition | Phase |
| --- | --- | --- |
| `E0807` | No gradient: a grazing crossing. The diagnostic names the `cross` instance and `t*`. | host |
| `E0808` | No gradient: an event appears or disappears as the param moves. The diagnostic names the event. | host |
| `E0809` | A gradient is requested with respect to a param that has none (rule 6), or that the session excludes. | host |

`E0807` and `E0808` replace the number for the records that depend on the named event.

**Example.** For `Comparator` (§13) with `inn` at a threshold `vth`, the edge time solves
`v(inp)(t*) = vth`. So `dt*/dvth = 1 / v'(inp)(t*)`: the edge moves by the threshold change
over the slew rate (rule 2).

| Reader of the edge | Receives | Rule |
| --- | --- | --- |
| a `Bit<f64>` that samples `v(x)` at that edge | `∂v(x)/∂vth + v'(x)(t*) · dt*/dvth` | 4a |
| a node that the edge switches, through a `D2A` | the saltation correction at `t*` | 2 |
| a `Bit` set by the edge | nothing; later events it causes move by `dt*/dvth` | 4f |
