# FOLLOW_UP_IDEAL — verification, optimization, and centering as one declaration

**Status:** vision document, not a spec. Written 2026-07-26 to feed `ROADMAP.md`
P7 (Optimizer) and a future P8 (Design Verification). Nothing here is
implemented; nothing here is committed to. Syntax is illustrative and uses real
PHDL grammar (`mod`/`analog`/`digital`/`bundle`/`discipline`/`param` blocks,
`I(a, b) <+ expr` contributions and `V(a, b) <- expr` forces — two distinct
operators, per `part_i_language.md:242`) so the sketches read against the tree.

**Scope decision (2026-07-26, revised 2026-07-27):** of everything explored in
this document, these constructs pass the No-Bloat burden of proof and enter the
grammar. The first two carry the feature:

1. **`tol` on `param`** — statistical variation declared on the parameter it
   perturbs, visible in the POM.
2. **The `constraint` block** — a third body kind whose statements evaluate
   inside the kernel at every accepted solver point.

Two smaller additions follow them, in later waves of the same release:
event-block **window arguments** (`after =`/`dur =`, reusing the existing
`EventBlock` production) and the **`cover`** statement kind inside a constraint
body. **Four grammar additions in total, and the list is closed.**

The third mechanism is not a language addition at all: the **programmatic
tester** (§4), an element that advances time and reacts imperatively in Python
or Rust. It carries functional verification, and adopting it *removed* a
grammar addition — RNM and its `behavioral` body kind left with it (D16).

Everything else — the optimizer, Monte Carlo, centering, coverage closure,
high-sigma methods, monitors, testers, aging — is host code or stdlib over those
primitives. **All of it is V1** (D5): the three waves in §9 are a delivery order,
not a staging plan with two halves postponed. The rest of this document argues
why these few constructs, and only these, deserve layer 0.

**Expertise pass (2026-07-27).** Every checkable claim below was verified
against the tree and is cited by `file:line`. Five inconsistencies were resolved
into design text — the pointwise/reduced split (§2.2), evaluation points
(§2.5), contextual keywords (§2.6), absent-by-default SOA limits (§2.8), and
the validation channel's shape and capability bit (§2.9) — and two new forks
were added to §10 (adjoint DC-vs-AC ordering, where reduced metrics are
computed). Three claims turned out **stronger** than written: the RNM resolve
kinds are already in the formal grammar, `AnalogDevice` already exists as the
home for `param_stamps`, and hierarchical instance-port refs already work.

---

## 0. The thesis

Every commercial AMS flow treats verification, optimization, and centering as
**three separate tools** bolted onto a simulator: a checker deck, an optimizer
cockpit, and a Monte Carlo wrapper. Each re-declares the same knowledge — what
"working" means for this circuit — in its own dialect. The designer writes the
SOA limit in the checker, the objective in the optimizer, and the yield target
in the MC setup, and nothing enforces that the three agree.

**The claim of this document: those three things are one declaration.**

A constraint says "this quantity must stay on this side of this level". Read it
three ways and you get all three tools:

| Read the same constraint as… | You get |
|---|---|
| a predicate checked over the trajectory | **verification** (SOA, ERC, spec compliance) |
| a feasibility boundary with a signed distance | **optimization** (constrained sizing) |
| a distance measured in units of statistical spread | **design centering** (yield) |

**Calibrating that claim.** Verification and centering really do fall out:
checking `m ≥ 0` *is* verification, and maximizing `min_i (m_i / σ_i)` *is*
worst-case-distance centering — the normalized margin is the exact quantity each
one needs. Optimization is the weaker of the three legs. The declaration gives
an optimizer the **feasible set and the gradients**, which is the hard half; it
does not give the objective (host-side by decision), the corner list, or the
search policy. So the precise claim is not that `constraint` replaces the
optimizer cockpit — it is that the cockpit stops holding its own private copy of
what "correct" means and starts reading the model's.

Piperine can make this the *native* form because of three assets no
SPICE-derived flow has:

1. **Symbolic differentiation already in the compiler.** `resolve/diff.rs`
   exists to emit the Jacobian. Pointed at parameters instead of unknowns, the
   same machinery yields ∂(anything)/∂(any parameter) — gradient-based sizing
   instead of black-box search. This is the single biggest lever in the document.
2. **Compile once, restamp (MD-18).** Already proven by `urc_compile_count.rs`
   and `compile_once_sweep.rs`. An optimization loop and a Monte Carlo run *are*
   parameter sweeps over an already-JITed circuit. The per-iteration cost is
   already near the floor.
3. **Analog and digital in one process, no tool boundary.** `06_flash_adc.phdl`
   already reads `V(vin, gnd)` inside a `digital` body. The A/D boundary is an
   internal detail, not an integration between two simulators.

Three mechanisms spend those assets, and only two of them touch the language:

1. **The `constraint` block** (§2) — invariants and spec, evaluated at the
   solver's rhythm, compiled into the kernel.
2. **Parameter gradients** (§3) — the adjoint path that makes margins actionable
   instead of merely observable.
3. **The programmatic tester** (§4) — imperative time in the host language,
   evaluated at the *test's* rhythm. No grammar at all.

The third one is what keeps the first two small: because functional testing lives
in the host, the language only has to carry what the host cannot do at the
solver's rhythm.

Everything below is an attempt to spend those three assets well — and to spend
*nothing else*.

---

## 1. Language addition one: `tol` on `param`

Statistical variation is part of a parameter's declaration, not a separate
deck. It attaches to the parameter it perturbs, survives monomorphization, and
is readable from the POM.

```phdl
mod Mos1(...) {
    param vto : Real = 0.7  tol gauss(sigma = 0.005 / sqrt(w * l));  // mismatch: per-instance
    param u0  : Real = 600.0 tol gauss(sigma_rel = 0.02) global;     // process: shared
    param tox : Real = 4e-9   tol uniform(rel = 0.02);               // ±2% uniform
}
```

- `tol <dist>(args)` follows the default. Distributions are **declared names**,
  not grammar: `gauss(sigma | sigma_rel)`, `uniform(half | rel)`, and friends
  live as `extern` declarations in a `headers/statistics.phdl`, per MD-24.
  Adding a distribution never touches the parser.
- Without `global`, the variation is **mismatch** — independent per instance.
  With `global`, it is **process** — one draw shared by every instance. That
  distinction is the whole game in analog yield, and putting it next to the
  parameter beats a separate mismatch deck.
- The distribution is **inert by default**. A nominal `op()`/`tran()`/`ac()`
  uses the declared value and is fully deterministic; a sim that secretly
  resamples parameters would be a trap. Sampling happens only when the host
  asks for it (`monte_carlo`, `center`), under an explicit seed. The POM
  carries the distribution unconditionally — `Param.distribution()` — so the
  host can enumerate the statistical knobs without simulating anything.
- A tolerance on a parameter does not change its `Invalidation` class: a
  sampled draw is an ordinary restamp write through the MD-18 path.
- Distribution arguments are elaboration-constant expressions and may
  reference sibling params (`sigma = 0.005 / sqrt(w * l)`) — they evaluate in
  the module's param scope, like any parameter default.
- `tol` applies to `Real` params. A statistical clause on a discrete param
  (`Integer`/`Natural`) is rejected loud — discrete variation is a different
  mechanism (§3.4's discrete-knob discussion), not a silent rounding.

**Rejected sugar.** A percent literal (`tol 5%`) was considered and rejected:
`%` is already the modulo operator in `MulExpr`, and a second lexer meaning
for it is exactly the kind of surface growth No-Bloat exists to stop. The
distribution form covers the intent: `tol uniform(rel = 0.05)`.

**Why this passes No-Bloat.** The alternative is declaring variation in the
host — which re-declares every parameter name in Python, breaks the moment a
model refactors, and cannot ship with a foundry PDK written in PHDL. Variation
attached to the parameter composes with hierarchy for free. This is the same
argument as §2.8 for constraints: knowledge that belongs to the *model* must
live where the model lives.

---

## 2. Language addition two: the `constraint` block

A third body kind alongside `analog` and `digital`, attached to a module by
name, exactly as they are.

```phdl
mod Amplifier(input vin : Electrical, output vout : Electrical,
              inout vdd : Electrical, inout gnd : Electrical) {
    param w1 : Real = 2e-6;
    param w2 : Real = 4e-6;
    param ibias : Real = 10e-6;
}

constraint Amplifier {
    // ── hard limits: violation means the circuit is wrong, not off-spec ──
    require vds_m1     : abs(V(m1.d, m1.s)) <= 1.98;
    require headroom   : V(vout, gnd) >= 0.2 && V(vdd, vout) >= 0.2;
    require no_fwd_bulk: V(m1.b, m1.s) <= 0.3;

    // ── named scalars the host can read and the optimizer can use ──
    // pointwise (§2.2) — one value per accepted solver point
    var power    : Real = abs(I(vdd)) * V(vdd, gnd);
    var gain_db  : Real = 20.0 * log10(abs(ac_gain(vout, vin)));
    // reduced (§2.2) — properties of the whole AC sweep
    var ugbw     : Real = ac_unity_gain_freq(vout, vin);
    var phase_m  : Real = ac_phase_margin(vout, vin);

    // ── spec: soft constraints with a scale, giving normalized margins ──
    target gain_db >= 60.0  tol 3.0;
    target ugbw    >= 10e6  tol 1e6;
    target phase_m >= 60.0  tol 5.0;
}
```

There is no `minimize` in the block. The objective is **host-side**: it is the
intent of one optimization run, not a truth about the model, and optimization
scripts typically run once. The block declares the *vocabulary* (`var`s,
`target`s); Python picks the objective per run — `ota.optimize(objective=
"power", …)`. An objective naming an unknown `var` fails loud at the host
boundary.

### 2.1 The three statement kinds

| Statement | Meaning | Margin | Violation |
|---|---|---|---|
| `require <name> : <pred>;` | invariant that must hold everywhere it is evaluated | unnormalized signed distance | **loud failure** in strict posture — analysis errors, reports instance + time + value |
| `target <expr> <cmp> <level> [tol <scale>];` | specification the design should meet | signed distance ÷ `tol` | recorded, not fatal; drives optimization and centering |
| `var <name> : <Type> = <expr>;` | named scalar extracted from the analysis | — | — |

`require` is for physics and reliability: SOA, oxide field, junction polarity,
supply headroom, current density. `target` is for specification, where "worse"
is a gradient, not an error.

A `var` in a constraint block is the same keyword with its ordinary meaning —
bind a computed value to a name — under two block-specific rules: single
assignment, and exported by name to the host (it is how `r.margins`, the
objective reference, and coverage expressions address computed scalars). No
new vocabulary: `var` already exists in every `fn`-body grammar in PHDL, and
it keeps its **mandatory type annotation** there (`var code : Real = 0.0;` in
`06_flash_adc.phdl`, one of 930 `var` sites in the tree, every one annotated).
A constraint block does not get inference the rest of the language lacks —
that would be a parser special case bought for four saved characters.

### 2.2 Two classes of measured quantity — the distinction that decides lowering

`var` and `target` expressions split into two kinds, and **conflating them is
the single easiest way to make this feature unimplementable**:

| Class | Example | Exists at | Lowers to |
|---|---|---|---|
| **Pointwise** | `abs(I(vdd))`, `V(vout, gnd)`, `m1.region` | every accepted solver point | a margin kernel, evaluated per point (§7) |
| **Reduced** | `ac_unity_gain_freq(vout, vip)`, `ac_phase_margin(…)`, settling time, overshoot | only after a whole sweep is known | a post-analysis reduction over the collected points |

Unity-gain frequency is not a property of a frequency point; it is a property
of the *sweep*. No per-point kernel can compute it, because at any single
frequency the answer does not yet exist. The same is true of phase margin,
settling time, overshoot instant, and every "where did the curve do X" metric.

Three consequences that must hold in the design:

1. **Each declared helper carries its class.** `headers/constraints.phdl`
   declares `ac_gain` as pointwise and `ac_unity_gain_freq` as reduced (§7's
   MD-24 note). The class is part of the declaration, not folklore.
2. **A `require` cannot read a reduced quantity inside a pointwise scope.**
   `@ tran { require settle : ac_phase_margin(…) >= 60.0; }` is a loud
   elaboration error, not a kernel that returns garbage per step. A reduced
   quantity is checkable only *once*, against the completed analysis.
3. **Margins therefore come in two shapes too.** A pointwise margin has an
   argmin (time or frequency + instance); a reduced margin has one value per
   analysis and no argmin. The host surface must say which it is rather than
   reporting a fake `t = 0` — which is one of the two reasons margins get their
   own result channel rather than being folded into waveform rows (D3).

**Reduced metrics are computed host-side (D7).** The solver stays
pointwise-pure: it evaluates and emits per-point values, and the host applies the
declared reduction to produce `ugbw` and friends. This keeps sweep-shaped state
out of the kernel and keeps `piperine-solver` from growing a second notion of
what a result is. The declaration still lives in PHDL — the constraint block is
where `ugbw` is *named* and specified — but its computation is one pass over
points the solver already returns.

Gradients inherit the split: `∂m/∂p` for a pointwise margin is the adjoint at
the argmin point (§3.4's non-smoothness caveat); for a reduced margin it is
the derivative of the reduction, which exists only if the reduction itself is
differentiable — a unity-gain *frequency* differentiates through the implicit
function theorem on `|gain(f)| = 1`, while an event-detected settling time
mostly does not. Fail loud on the ones that do not (§3.4).

A `var` or `target` whose expression calls an analysis-specific helper —
`ac_gain` exists only in AC — is defined only in that analysis; reading it
from another analysis is a loud "not defined in this analysis", never a stale
or zero value.

**Why `require` is not `$assert`.** `$assert` is a diagnostic: it prints. A
`require` produces a *value* — the signed margin — with provenance (instance,
time, worst point), readable by the host, differentiable by the compiler, and
consumable by three different tools. An assert that a margin read as "−0.03"
is a print statement; the same declaration as a `require` is the objective
function of a centering run. Same surface idea, different type.

### 2.3 Margins — the one mechanism behind everything

Every comparison lowers to a **signed margin function**:

```
a <= b          →   m = (b − a) / scale
a >= b          →   m = (a − b) / scale
a in [l,u]      →   m = min(u − a, a − l) / scale
```

with `scale = 1` when no `tol` is given. (The range form spells `in`, keeping
the window vocabulary of §2.7 free of double duty.) Two roles share the
spelling `tol`: on a `param` it declares a distribution (§1); on a `target` it
declares the normalization scale. The rhyme is deliberate — both express "how
much slack exists" — but a spec may rename the target's to `scale` if the
collision reads badly. The invariant that makes the whole
design work:

> **`m ≥ 0` ⟺ satisfied.** `m` is a real number the compiler can differentiate.

That single convention is what collapses three tools into one declaration:

- **Verification** evaluates `m` at every accepted solver point and fails
  `require`s where `m < 0` (strict posture).
- **Optimization** treats `{m_i ≥ 0}` as the feasible set and has
  `∂m_i/∂p` from the compiler (§3).
- **Centering** maximizes `min_i (m_i / σ_i)` where `σ_i` is the spread of
  `m_i` under the declared `tol` statistics — worst-case-distance centering,
  and the normalized margin is exactly the quantity it needs.

Nothing else in this document is as important as that table. Get margins right
and the rest is engineering.

### 2.4 Check posture: strict, collect, off

A `constraint` block is **evaluated at runtime, per accepted solver point** —
and it is **skippable**, because the right posture depends on what the run is
for:

| Posture | Behavior | Who uses it |
|---|---|---|
| `strict` (default) | `require` margin < 0 → loud analysis failure naming instance + time + value | verification runs, `piperine test` |
| `collect` | margins recorded, never fatal | **the optimizer's inner loop** — an infeasible iterate is a margin to climb out of, not an exception to catch; Monte Carlo — a failing sample is a yield event, not an abort |
| `off` | constraint kernels not called; zero cost | performance-critical sweeps by a host that already knows the design is legal |

The posture is a solver/host config knob (`pip.Solver(checks=…)`), not a
language construct; it joins the canonical `Solver`/`SolverConfig` field set
on both hosts (HOST-20 parity). The default stays strict: silent pass is
never the surprise a user gets by default.

### 2.5 Which points count — the definition that keeps strict posture honest

"Per accepted solver point" is not precise enough to implement, and the
imprecise reading produces false violations on circuits that are fine. A
margin is evaluated at, and only at:

| Analysis | Evaluated at | **Not** at |
|---|---|---|
| DC / OP | the **final converged solution** | any homotopy stage — gmin stepping and source stepping each converge at intermediate stages that are *non-physical by construction* (gmin adds shunt conductance; source stepping scales every source down). A headroom or SOA `require` checked at a gmin stage fails on a solve that is converging correctly. |
| Transient | each **accepted** timestep | rejected steps (the LTE/rollback path — `SUPPORTS_ROLLBACK`, `accept_timestep`); a rejected step's state is discarded and must never emit a finding |
| Transient at `t = 0` | the operating point, if one was computed | the UIC / `@initial` state — a user-forced initial condition may legitimately sit outside SOA before the circuit has settled. Skip `t = 0` under UIC, or the first honest transient a new user runs reports a violation they did not cause. |
| AC / noise | each frequency point (pointwise margins); once per sweep (reduced, §2.2) | the DC operating point solve underneath it, which the DC row already covers |
| Sweep / `sweep_grid` | each swept point, reduced to a worst-across-sweep margin with the **swept coordinate** as part of the argmin | — |

The sweep row is not a detail: `sweep`/`sweep_grid` over a compiled session is
the loop that optimization, corner runs, and Monte Carlo are all built from
(§5). "Worst margin over the sweep, and at which point" is the quantity those
three consume, so it is a first-class result, not something the host
reconstructs by scraping per-point results.

This is also why the postures matter more than they first appear: an optimizer
walking through infeasible iterates would otherwise abort on its own search
path. `collect` is not a convenience — it is the only posture in which
gradient descent through an infeasible region is expressible.

### 2.6 Keywords: only `constraint` can be reserved

Measured against the corpus, not assumed. Occurrences as identifiers across
`headers/`, `examples/`, and `crates/piperine-lang/tests/`:

| Word | Hits | Verdict |
|---|---|---|
| `constraint` | 0 | safe to reserve at the parser level |
| `tol`, `require`, `cover`, `global` | 0 | free today, but see below |
| `target` | **7** | **cannot be reserved** |
| `in` | 55 | already reserved (`part_i_language.md:233`), used by comprehensions — `a in [l, u]` reuses it in a new position |

`target` is used as an ordinary variable in
`crates/piperine-lang/tests/examples/ring_oscillator.phdl:5` and
`oscillator.phdl:16` — `var target : Real = -gain * V(a, gnd);` — and CLAUDE.md
marks `tests/fixtures*` a **frozen corpus**. Reserving it at the parser level
breaks two frozen fixtures for a keyword.

So all five new words are **contextual keywords**, reserved only where the
grammar expects them: `require`/`target`/`var`/`cover` at statement position
inside a `constraint` body, `tol` after a `param` default or a `target`
comparison, `global` after a `tol` clause. PHDL already has this mechanism and
already documents it — the resolve kinds `tri`/`or`/`and`/`sum`/`avg`/`max`/`min`
are contextual inside a `discipline` body (`part_i_language.md:238`). Nothing
new is invented; the precedent is followed.

`constraint` is the one exception worth reserving properly, because it opens a
top-level item and a contextual read there buys nothing.

### 2.7 Scoping: the existing event block selects *when* a constraint holds

A `require`'s applicability uses **the event-block syntax the language already
has** — `@ EventSpec [when (cond)] { … }`, the same production as in `analog`
and `digital` bodies — with the block holding constraint statements instead of
behavior statements:

```phdl
constraint PowerStage {
    @ dc {
        require bias_sane  : V(mid, gnd) > 0.4 && V(mid, gnd) < 1.4;
    }
    @ tran {
        require soa_switch : abs(V(sw.d, sw.s)) <= 40.0;
    }
    @ ac {
        require no_peaking : abs(ac_gain(vout, vin)) <= 4.0;
    }

    // a 5 µs window after the enable crossing
    @ tran(after = cross(V(en, gnd), rise), dur = 5e-6) {
        require settle : abs(V(vout, gnd) - 1.8) <= 0.018;
    }

    // ignore the startup transient
    @ tran(after = 1e-6) {
        require soa_steady : abs(V(sw.d, sw.s)) <= 40.0;
    }

    // composition: holds in both analyses
    @ (dc | tran) {
        require supply_sane : V(vdd, gnd) >= 1.62;
    }
}
```

The design:

- **One event syntax everywhere.** The grammar addition is one production:
  `ConstraintStmt` grows `ScopeBlock`, which *is* the existing `EventBlock`
  with a constraint-statement body. `dc`/`tran`/`ac`/… register as event
  terms in the existing `EventRegistry` — the same registry that already
  resolves `cross`/`above`/`timer` through `headers/operators.phdl`. Blocks
  nest; `when (cond)` level-gates exactly as it does in behavior bodies.
- **`after =` and `dur =` are the whole window language.** `EventTerm` grows
  named arguments, mirroring how `timer(period, phase)` already carries a
  second argument. `after` takes a time (`1e-6`) or an event term
  (`cross(V(en, gnd), rise)`) — the window starts at that time or at the
  first fire; `dur` closes it. A bare event term (`@ cross(x)`) denotes its
  fire instants; an analysis term denotes every accepted point of that
  analysis.
- **Scopes are sets of evaluation points and compose as sets.** `|` is union
  (already the event-OR in the grammar), `&` intersection, `not` complement —
  boolean algebra over windows, in the same `EventSpec` production. A
  statement reduces over its enclosing set: `m_worst = min_t m(t)`.
- **An empty scope is reported, never silently vacuous.** A window that
  selects no points (the crossing never fired, the analysis never ran) is a
  *result* — "not exercised" — surfaced to the host, so a misspelled trigger
  can't masquerade as a pass. This is the coverage instinct applied to
  constraints.
- **A top-level `require` (no enclosing block) holds in every analysis** — and
  that default is a `Context` field, not a hard-coded rule (D2). Holding
  everywhere is the safe reading: a rule that silently applied nowhere would let
  a real violation hide. §2.5 already removed the sources of false positives
  that made this uncomfortable, and the knob covers whatever residue a specific
  design has, without making the safe behavior something a user must opt into.

The host gets `m_worst` plus the argmin — time and instance — because "gain
is 3 dB low" and "gain is 3 dB low *at 1.2 µs on instance u3.m1*" differ in
usefulness by an order of magnitude.

### 2.8 Reuse: constraints belong to the model, not to the testbench

The payoff appears when a constraint ships with a *device* rather than with a
circuit. `headers/spice/mos.phdl` grows:

```phdl
constraint Mos1 {
    require vgs_ox : abs(V(g, s)) <= vgs_max;
    require vds_ox : abs(V(d, s)) <= vds_max;
    require bulk_bd: V(b, d) <= 0.3;
    require bulk_bs: V(b, s) <= 0.3;
}
```

with `vgs_max`/`vds_max` as ordinary model-card parameters in `bundle
Mos1Model`. Now **every** MOSFET in **every** design carries its own SOA
rules, and a foundry PDK expressed in PHDL ships enforceable reliability
limits instead of a PDF appendix. A designer who never writes a `constraint`
block still gets SOA checking on every device they instantiate.

**The limits must default to absent, and the reason is not caution.** The
built-in `headers/spice/` models are **ngspice-faithful** — that is their stated
contract, and ngspice has no SOA limits at all. Inventing a `vds_max` for
`Mos1` would (a) fabricate a number no source supports, (b) break the
faithfulness claim the ngspice cross-check suite exists to defend, and (c) with
`strict` as the default posture, trip the entire example gallery on limits its
author never wrote.

So `vgs_max`/`vds_max` default to unset (`Real?`, or `+inf`), and a `require`
reading an unset limit is **inert** — not skipped by posture, but vacuously
satisfied because no limit was declared. A foundry deck that sets them gets
enforcement on every instance; the built-in models stay faithful and the
gallery stays green *by construction*, not by choosing a lenient default well.

The distinction matters for the default posture argument: what should be quiet
by default is an **undeclared limit**, never the check itself. Keep `strict` as
the default; make absence of data mean absence of a claim.

This is the strongest single argument for putting constraints *in the
language* rather than in the host: a constraint attached to a model composes
with hierarchy automatically, and a constraint written in a Python testbench
does not.

**Hierarchical reach: instance ports yes, instance internals no.** Reading a
child instance's *port* net is existing capability, not a new one —
`crates/piperine-lang/tests/examples/sar_adc.phdl:29` already writes
`I(dac.out, gnd) <+ cload * ddt(V(dac.out, gnd));`. So `V(m1.d, m1.s)` in a
parent's constraint block resolves the nets the parent itself connected, and
`constraint Mos1` reading its own `V(d, s)` is plainer still.

An instance's **internal** node is a different matter: `Mos1` creates an
internal drain node when `rd > 0`, and a parent reaching it (`m1.di`) would
cross the encapsulation boundary, depend on whether an optional resistance was
given, and break under monomorphization. That should be a loud error, not a
quiet resolution — and the two cases must be distinguished explicitly, because
"resolve through the authored instance tree" reads as though it covered both.

### 2.9 The validation channel: one ABI hook under everything

A `require` compiled from PHDL is not the only thing that can have something
to say about correctness. Three sources exist, and they must converge:

1. **Constraint kernels** (PHDL `require`s) — margins crossing zero.
2. **Monitor modules** — ordinary `digital` (or mixed) modules evaluating
   sequential properties (the SVA story, §9 item 11).
3. **Plugin/OSDI devices** — model-embedded checks, which is exactly how
   industry foundry decks emit SOA warnings today. A Verilog-A model wrapped
   behind the `Element` ABI has no PHDL constraint block; without an ABI
   channel, its checks are second-class.

The answer is one hook on the `Element` ABI, shaped after the existing
`limiting_report()` precedent (a structured per-step report the solver polls
and acts on):

```
validation_reports() -> Option<ValidationReport {
    findings: [ValidationFinding { severity: Warning | Error,
                                   label, message, value, time, instance_path }]
}>
```

Cross-cutting on `Element` (like `accept_timestep`), not `AnalogDevice`-only —
a digital monitor element reports through the same channel. Polled at the points
§2.5 defines, and in `off` posture not polled at all.

Two shape decisions, both taken from what the tree already does:

- **`Option<Report>`, not `Vec<Finding>`.** The precedent is
  `limiting_report() -> Option<LimitingReport>` (`core/element.rs:173`) — the
  same "structured per-step report the solver polls and acts on" role. `None`
  is the common case and costs nothing to return.
- **Polling is gated by a capability bit, `EMITS_VALIDATION`.** A defaulted
  method that returns `None` still costs a virtual call *per element, per
  accepted point* — on a 10 k-element circuit over 10⁶ timesteps that is 10¹⁰
  calls to learn nothing. `EMITS_NOISE` already establishes the pattern: the
  solver asks only the elements that declare they have something to say.
  `ElementCapabilities` is a `u32` whose highest used bit is
  `NUMERIC_JACOBIAN = 1 << 14`, so there is room.

"Costs nothing when unused" has to mean *not called*, not *returns empty*.

The layering is clean:

> **Margins are values; findings are events.** A `require`'s margin crossing
> zero *generates* a finding (severity `Error`). The margin channel is
> continuous and differentiable — it feeds the optimizer. The finding channel
> is discrete and postural — in `strict`, an `Error` finding fails the
> analysis loud, naming instance, time, and label; in `collect`, it lands on
> the result object (`r.violations`); in `off`, nobody listens.

Consequences that fall out for free:

- **Device self-testing covers every element origin.** A `Mos1` constraint
  block, a monitor FSM, and an OSDI foundry model all report into the same
  channel with the same provenance — SOA checking no longer depends on the
  device being PHDL-compiled.
- **Digital acceptance testing is the same channel.** A monitor module's
  sequential failure is a finding with time and instance, emitted from the
  digital scheduler's accepted points — no `ok`-net indirection, no special
  assertion engine.
- **Warnings become representable.** The `Warning` severity exists in the
  channel whether or not Wave-1 PHDL grows a `warn name : pred;` statement
  kind (a cheap later addition — the channel already carries it, and OSDI
  models will use it from day one).
- **Testers are devices, not a framework.** An ATE-style test program (Wave 3,
  item 15) is an ordinary element that drives nets, wakes on its own
  breakpoints, and reports through this channel — acceptance suites compose
  with hierarchy instead of living in a separate testbench dialect.

### 2.10 Where it does *not* go

**PHDL describes the design and its invariants. The host gives the commands.**
That is the whole division, and it is not a matter of taste — it follows from
who chooses the evaluation instants (§4.1):

| In PHDL | In the host (Python, or Rust) |
|---|---|
| what "correct" and "in-spec" mean for this circuit | stimulus of every kind — including analog drive (§4.2) |
| SOA, ERC, spec limits, `target`s | test sequencing: drive, wait, sample, compare (§4.3) |
| anything the **solver's** rhythm evaluates — every accepted point | anything the **test's** rhythm evaluates — its own breakpoints |
| — | the objective, corner lists, sample counts, regression, coverage closure, reporting |

A `constraint` is evaluated at every accepted solver point, so a host crossing
per point would dominate the run — it compiles into the kernel. A tester wakes
at its own breakpoints, 10⁴–10⁵ times per test, so it stays imperative in the
host where it is easy to reason about.

**What this division buys: the in-language verification surface stays small.**
Functional testing — drive vectors, expect responses — is host work and stays
host work; `piperine test` and `*_tb.py` are the right shape for it. The
language adds only what the host provably cannot do at the solver's rhythm.

**One consequence worth stating plainly: margins are an analog notion.** The
signed distance of §2.3 is real-valued and differentiable, and no such quantity
exists for a `Bit` or a `Quad` — a digital obligation is pass/fail, not "3 dB of
room". So:

- **Analog** verification rides **both** channels — margins (continuous,
  differentiable, feeds the optimizer) and findings (discrete, postural).
- **Digital** verification rides **findings only** — no margin, no gradient, no
  centering.

That makes the validation channel (§2.9) the *primary* mechanism for digital
verification rather than an accessory, and it is why a monitor or a tester needs
no margin to be first-class.

---

## 3. Gradients: differentiated before the solver, delivered through one ABI bit

The asset that makes Piperine's optimization story different from everyone
else's. Two questions were open here; both now have a proposed answer.

### 3.1 Where derivatives are born: a pre-solver differentiation layer

`resolve/diff.rs` differentiates residual contributions with respect to
*unknowns* to build the Jacobian. Sizing needs derivatives with respect to
*parameters*. Same symbolic pass, different differentiation variable — so the
answer to "do we need a layer before the solver?" is **yes, and it already
half-exists**: at compile time, alongside the residual and Jacobian kernels,
codegen emits a third kernel computing `∂F/∂p` stamps for every differentiable
parameter — where *differentiable* means `Real`-typed and restamp-class, the
same admissibility rule `.sens` already enforces (Part VII §17). A gradient
requested through a rebuild-class or discrete parameter fails loud, exactly as
a `.sens` request does today. This is MD-18-safe by construction — the kernel
is emitted once; an optimizer iteration restamps values, never re-elaborates.

**One kernel with a parameter index, never one kernel per parameter.** This is
not a style preference — it is the lesson of a bug found in this codebase on
2026-07-26 and now open in `ROADMAP.md` P1: the `.disto` 2nd/3rd-derivative
kernels emit one JIT function per *ordered* controlling-branch combination, and
on MOS2/MOS3 that count overruns Cranelift with `TryFromIntError`, making those
devices uncompilable. A `Mos1` model card carries ~60 parameters. A `∂F/∂p`
design that emits a function per parameter walks into the identical wall, with
the identical symptom, one crate over. Emit one kernel taking a parameter index.

The classical consumption method is **adjoint sensitivity** (Director &
Rohrer, 1969): for a scalar output, one extra linear solve yields the gradient
with respect to *all* parameters at once:

```
∂f/∂p  =  −λᵀ (∂F/∂p),      where  Jᵀ λ = ∂f/∂x
```

`J` is the Jacobian the solver already factors. The adjoint solve shape —
transpose, apply an excitation, solve once — is **already in the tree**:
`analyses/noise.rs:240` `solve_adjoint_system` transposes the system and solves
with a unit excitation at the output, per frequency (Part VII §12).

**But that existing adjoint is complex and per-frequency — it is the AC shape,
not the DC one.** This inverts the sequencing the rest of this document assumed.
A DC adjoint needs a *real* transpose solve, which is a sibling code path, not
the one noise already walks. Two consequences:

1. "Reuse the noise solve shape" is accurate for **AC** sensitivity and
   optimistic for **DC**. Say which one is being reused.
2. The metrics a designer actually optimizes — gain, bandwidth, phase margin —
   are AC metrics anyway (§8). If AC adjoint is the closer of the two, a
   DC-only first delivery buys the harder half and postpones the useful half.
   Whether Wave 1 lands DC-first (simpler algebra, `.sens` available as a
   cross-check oracle) or AC-first (closer to existing code, immediately useful
   metrics) was the sharpest remaining fork. **Decided: DC first (D6)** — an
   unverifiable gradient is worth less than a slower verifiable one, and `.sens`
   only backs DC. The cost is explicit: AC-metric gradients wait for the AC
   adjoint, and until then an optimizer finite-differences them or treats those
   targets as feasibility filters.

The existing `.sens` driver (`analyses/sens.rs:2`: central finite difference
over the restamp path, already refusing `Invalidation::Rebuild`) is the
cross-check oracle either way: a DC adjoint gradient must match it within the
finite difference's own error, which makes DC the easier thing to *verify* even
if it is the further thing to *build*.

### 3.2 The delivery contract: one method, one capability bit

The solver never sees PHDL. It sees elements. So the one thing the ABI must
answer is: *how does an element hand over ∂F/∂p?*

- `AnalogDevice` grows one method: `param_stamps(param_handle, &mut sink)` —
  the element's residual differentiated w.r.t. one of its declared parameters,
  written as ordinary stamps at the current operating point. (`AnalogDevice`
  does exist and is the right home: `abi.rs`, implemented for `PiperineDevice`
  at `device/element.rs:139`. CLAUDE.md's "one ABI: `Element`" summary reads as
  though it were gone.)
- **Stamps go into a caller-provided sink, not a returned `Vec`.** `Stamp`
  already exists (`math/linear.rs:12`, generic over `AsIndex`/`Scalar`), and
  this call sits in the adjoint's inner loop over parameters — returning a fresh
  allocation per parameter per solve is the wrong shape for a hot path when
  `load_dc` already demonstrates the sink pattern.
- `ElementCapabilities` grows `HAS_SENSITIVITY`, alongside the existing
  `HAS_DISTO2 = 1 << 12`/`HAS_DISTO3 = 1 << 13` precedent: disto already proved
  that "this element carries extra derivative kernels" is a
  capability-bit-shaped fact.

For a JIT-compiled PHDL device, the bit is set and the method calls the §3.1
kernel. For a plugin or OSDI device, the bit is clear — and the driver falls
back to **stamp perturbation**: call `load_dc` twice at the *same* operating
point with the parameter perturbed through the ordinary restamp path, and
difference the stamps. That is two load calls per parameter — **no re-solve**
— so the adjoint economy (one Jᵀ solve for all parameters) survives intact;
only the ∂F/∂p term becomes approximate. Absence of the bit degrades accuracy
and speed, never correctness, and never silently: the driver reports which
elements used the fallback.

That is the whole ABI change. One method, one bit, one documented fallback.

### 3.3 Host surface

```python
m = piperine.load("amp.phdl").module("Amplifier")

# DC-defined quantities come off a DC result
r = m.op()
r.margins["headroom"]              # 0.34  — signed, normalized
r.sensitivity("idd", "w1")         # A per metre, one adjoint solve
r.gradient("idd")                  # {"w1": ..., "w2": ..., "ibias": ...}

# AC-defined quantities come off an AC result — reading `gain_db` from `op()`
# is a loud "not defined in this analysis", per §2.1
a = m.ac(piperine.AcConfig(start=1e3, stop=1e9, points=201))
a.margins["gain_db"]               # pointwise metric, per-frequency argmin
a.margins["ugbw"]                  # reduced metric, one value, no argmin (§2.2)
a.sensitivity("gain_db", "w1")     # needs the AC adjoint — not in Wave 1 (D6)
```

Each result object serves the analysis that produced it. A metric is readable
where it is defined and loud everywhere else — which is why the two blocks
above cannot be collapsed into one, however much shorter that would read.

The existing `.sens` (central finite difference, two full DC re-solves per
parameter) stays as the always-available baseline; the adjoint path is the
fast exact route for differentiable elements. Same host surface, two engines
underneath — and the difference is reported, not hidden.

### 3.4 Honest difficulties

- **Transient adjoint runs backwards in time** and needs the forward
  trajectory stored (or checkpointed). Memory cost is real and must be a
  design input, not a surprise.
- **`min` over time and over corners is non-smooth.** The worst-case margin has
  a kink where the argmin switches. **Decided (D1): differentiate at the located
  argmin** — correct almost everywhere, standard practice, and reported as such
  in the result object. The rejected alternative was a declared-sharpness
  softmin, which would have silently changed what the optimizer believes about
  feasibility. The kink remains real: an optimizer stepping across an argmin
  switch sees a gradient discontinuity, so the step control has to tolerate one
  rather than assume smoothness.
- **Discrete parameters are not differentiable.** Device multipliers, topology
  choices, and segment counts need a hybrid: gradient inside a discrete shell.
- **Not every metric is differentiable through the solver.** A metric defined
  by event detection (settling time, overshoot instant) has derivative
  information only through the event condition. Fail loud on the ones that
  cannot be differentiated rather than returning a plausible wrong number.

---

## 4. The programmatic tester — imperative time, in the host language

The third mechanism, and the one that decides how much verification needs to be
in the language at all. PHDL describes the design and its invariants; the host
gives the commands. A tester is an **ordinary element that advances time and
reacts programmatically**, written in Python (or Rust — same contract, and
Python is simply better at it).

### 4.1 The rule that separates it from `constraint`

Not "inside the time loop versus outside" — that reading is too coarse and would
have put testers in the kernel. The dividing line is **who chooses the
evaluation instants**:

| Instants chosen by | Must live | Why |
|---|---|---|
| **the solver** — every accepted point | compiled, in-kernel (`constraint`) | 10⁶–10⁹ evaluations; a host crossing per point would dominate the run |
| **the test** — its own breakpoints | the host, imperative (tester) | 10⁴–10⁵ wake-ups per test; at ~1 µs a crossing, that is noise |

A `constraint` is in the kernel because the *solver* dictates its rhythm. A
tester can be Python because **it** dictates its own. Same time loop, different
rate, and the rate is what decides the language.

**The mechanism already exists in the ABI.** `Element::next_breakpoints(&self,
from, horizon) -> Vec<f64>` (`core/element.rs:187`) is how an element declares
its own wake-up times, and the transient driver already lands on them. A tester
is that method plus a host callback. `SimHooks` is *not* the right hook — it is
coarse-grained (`transform_design`, `before_lower`, `after_solve`), with nothing
per-step; a tester is an `Element`.

### 4.2 Analog drive without a host crossing inside Newton

The interesting engineering problem: a stamp value must exist during every
Newton iteration, and calling Python from inside the iteration loop is exactly
what §4.1 says not to do.

Resolution: **the host writes segments; the element stamps them.**
`t.ramp("vin", 0.0, 1.2, 50e-9)` installs a segment at the current breakpoint,
and the element stamps from that description until the next one — no host
crossing in between. This is how a PWL source already behaves. Python decides
*what* to drive; the element decides *how* to stamp.

Drive carries a **declared impedance**, which is where this meets §2.8's bridge
argument: `t.drive_voltage("vin", 1.2, rout=50.0)` stamps a Norton source, not
an ideal one. Without that, the tester reintroduces exactly the silent ideality
this document criticizes `wreal` for. An ideal force stays available — it is a
legitimate bring-up tool — but it is spelled explicitly, never defaulted.

Discontinuities are the solver's ordinary business (the digital path already
lives on them), so an ideal step is allowed; the requirement is that its
breakpoint be registered so the integrator sees the edge instead of
interpolating across it.

### 4.3 The programming model: a generator

Imperative state lives in the generator frame, so nothing hand-rolls an FSM:

```python
def uart_echo(t):
    t.drive("rst", 1);                yield t.advance(100e-9)
    t.drive("rst", 0)
    t.ramp("vdd", 0.0, 1.8, 1e-6);    yield t.advance(2e-6)
    for byte in (0x55, 0xAA):
        yield from t.uart_send("tx", byte, baud=1e6)
        got = yield from t.uart_recv("rx", baud=1e6)
        t.expect(got == byte, f"echo {got:#x} != {byte:#x}")
```

`advance(dt)` returns control to the solver, which runs to the breakpoint and
resumes the function there. `t.expect(...)` reports through the same
`validation_reports()` channel as everything else (§2.9), so a tester failure is
a typed finding with time and instance — not a print.

This is the "easy to reason about" property that makes the whole approach worth
it: a designer reads a test as a sequence of actions in time, which is how they
already think about bring-up.

### 4.4 Rollback, and why the simple answer is the right one for now

The hard problem: the solver rejects timesteps, and a Python generator cannot
un-advance.

The answer (user decision, D11): **explicit time advance is itself the
guarantee.** `advance(dt)` inserts a breakpoint at `t + dt` and asserts that
nothing the tester cares about changes inside that window; the tester is
resumed only at that breakpoint, which is an accepted point by construction. No
rollback protocol, no speculative state, no un-advancing.

The cost is honest and accepted: forcing the solver to land on every tester
breakpoint constrains its timestep, so a fine-grained tester makes the
transient slower than it would otherwise be. That is a known inefficiency, not
an unknown one, and it buys a programming model with no failure mode.

### 4.5 Side effects are allowed

A tester may read a vector file, consult a golden model, log to disk, or import
anything — it is a testbench, not a device model, and forbidding that would kill
the main use case (D12).

The consequence must be stated rather than policed: **reproducibility becomes
the tester author's responsibility.** A tester that reads a file whose contents
change does not reproduce, and the framework will not detect it. This matters
most where reproducibility is otherwise guaranteed by construction: Monte Carlo
draws are reproducible from `(seed, index)` (§5), and a side-effecting tester in
the same run is the one link in that chain the seed does not cover.

### 4.6 Reach: read anywhere, drive with a declaration

- **Reading internal nets is allowed** — `t.read_voltage("u3.core.bus")` is what
  the existing probe/trace surface already does, and refusing it would make a
  tester weaker than the debugger.
- **Driving an internal net is allowed but never quiet** (agent default, D14):
  forcing a node changes the circuit, so it is reported in the run's findings.
  A test that "passes" while forcing an internal node must be visibly doing so;
  real ATE only reaches package pins, and the gap between that and a simulation
  force is worth surfacing rather than hiding.

### 4.7 Known gap: the tester is transient-only

A tester sequences time, so it has no meaning in AC (D13, deliberate gap for
now). DC could reasonably treat installed drive segments as a fixed bias, but
that is unspecified too. Until it is: instantiating a tester and requesting an
AC analysis **fails loud** rather than silently ignoring the tester or silently
freezing it at `t = 0`. This is recorded as a known gap, not solved here.

---

## 5. Optimization, centering, Monte Carlo — host policies over the primitives

Nothing in this section touches the grammar. All of it is Python (and Rust
parity) driving the compiled session. Reached via `Module` or `Session`, these
methods compile once and hold the session internally: a 10³-sample Monte Carlo
is 10³ restamps on one JIT, never 10³ elaborations (MD-18).

This section fixes the **shape** of that surface and the one definition
(centering) the rest of the document leans on. The per-item delivery detail —
how the optimizer phases its search, how sampling walks the instance tree, what
each item needs from earlier waves — lives once, in §9's waves, and is not
repeated here.

```python
mc = m.monte_carlo(n=500, seed=7)    # samples every `tol` in the design
mc.yield_()                          # fraction with all margins >= 0
mc.worst("headroom")                 # the failing sample, reproducible by seed
mc.sigma("gain_db")                  # spread of the metric

c = m.center(over={"w1": (1e-6, 1e-4), "w2": (1e-6, 1e-4)},
             corners=["tt", "ss", "ff"], temp=[-40, 27, 125])
c.wcd                                # worst-case distance, in sigmas
c.params                             # the centered sizing
```

**Design centering, stated precisely:** maximize `min_i (m_i / σ_i)` over the
design parameters, where `m_i` is constraint *i*'s margin and `σ_i` its spread
under the declared `tol` statistics. Equivalently: push the nominal design as
far from every feasibility boundary as the variation-weighted geometry allows.
**This is the single most valuable thing in this document for a real analog
designer**, and the margin convention plus the gradients is most of what it
needs.

**One engine, three policies — not three features.** Optimization, centering,
and high-sigma sampling look like three items in §9 and are one driver:

| Policy | Objective | Over | Sampling |
|---|---|---|---|
| optimize | a host-named `var` | design params | none (nominal or corners) |
| center | `min_i (m_i / σ_i)` | design params | inner sampling to estimate `σ_i` |
| high-sigma | failure probability | statistical params | importance-weighted, tail-focused |

All three walk the same restamp loop, consume the same margin channel, and use
the same gradients where they exist. Building them as one driver with a policy
parameter is the difference between one tested engine and three that drift.
§9 items 6, 8, and 13 are that driver's three policies, delivered in order.

The honest cost note, since "host-side" is doing a lot of load-bearing work in
this document: *no language surface* is not *no cost*. This driver, the margin
and finding channels on the result objects, the reductions D7 moved host-side,
and MD-22 parity across Rust and Python are a substantial amount of
`piperine-api` code — very likely more total code than the two grammar
additions it stands on. The argument for putting it in the host is that it is
*policy*, which changes per run and per user, not that it is small.

---

## 6. What industry does today — and whether this covers it

An honest read of the production AMS verification landscape, so the two
additions are judged against what designers actually use, not against a
caricature.

| Capability | Industry answer | Where this document lands |
|---|---|---|
| Unified A/D simulation | Spectre X / AMS Designer (Cadence), PrimeSim (Synopsys), Symphony Pro (Siemens) — unified kernels or tight co-simulation | Already Piperine's base: one process, one `Element` ABI, A2D/D2A native (§0, asset 3) |
| SOA / reliability checks | Model-embedded checks in foundry Verilog-A decks (`$strobe`-style **warnings**), simulator assert cards; RelXpert / MOSRA (aging: HCI/BTI), Legato | `constraint` on the model (§2.8) — same placement as the foundry deck, but margins are **first-class values**, not print warnings. Aging is Wave 3 item 14 |
| Assertions on analog behavior | SVA + Verilog-AMS event checks (`cross`/`above`), PSL; sequential properties live in the digital domain; host-side scoreboards | Scoped `require` windows (§2.7) cover the analog subset; sequential protocol properties are Wave 3 as **monitor modules on the validation channel** (§2.9) — no assertion language |
| Coverage | SV covergroups over real/RNM signals, vManager / MDV closure dashboards | `cover` bins (Wave 3 item 10); closure reporting is host work, matching where vManager lives |
| Real-number modeling | `wreal` / SV real vars, DMS ports; the industry's main AMS speedup — with **silent ideality** at every real→electrical boundary | **Not in V1** (D16). The programmatic tester covers RNM's testbench role; simple ordinary modules cover most of its abstraction role. What is left is throughput |
| Functional / acceptance testing | UVM(-AMS) testbenches, SV classes and sequences; a separate verification language and methodology from the design language | **The programmatic tester** (§4) — imperative Python or Rust advancing time on its own breakpoints. No verification language, no class library: the host language already has loops, files, and golden models |
| Corners / Monte Carlo orchestration | ADE Assembler / Maestro, PrimeWave | Host sweeps on the compiled session (already shipped: `sweep`/`sweep_grid`) + `monte_carlo` reading `tol` declarations |
| High-sigma yield (3–6σ) | Solido (ML-guided importance sampling — the production reference), scaled-sigma, statistical blockade | Host methods over the restamp loop (§5); no language surface needed |
| Optimization | ADE Optimizer, ASO.ai (AI/black-box), **MunEDA WiCkeD** — the production reference for gradient-based sizing and worst-case-distance centering | §5's `center` *is* WCD centering; the difference is gradients are analytic and free, not finite-differenced |
| Differentiable simulation | **Absent commercially.** Academic differentiable SPICE (JAX-based inverse design) and photonic adjoint design (Lumerical-style) prove the value; no production SPICE exposes it | §3 — the defensible differentiator |

**Verdict.** The two additions cover the industry's core needs — SOA on every
device, spec margins, corner/statistical flow, sizing, centering — with the
same *placement* the industry converged on (checks ship with models;
orchestration lives in the cockpit), but with three upgrades no incumbent can
match without reopening their numerical kernels: (1) one declaration feeds all
three tools, (2) analytic gradients, (3) margins as typed values with
provenance instead of warning text.

Four capabilities the incumbents have and this document does not deliver in
Wave 1 — SVA-sequential assertions, coverage closure tooling, aging, RNM — are
real gaps, and per D5 all four are **in V1**, as Waves 2–3. What stays
permanently out is narrow and deliberate: analog formal, RL/GNN sizing, and the
foundry-qualified *model data* (aging coefficients, SOA limits) that no
open-source project can fabricate — which is why §2.8 makes absent data mean an
inert check rather than an invented number.

---

## 7. How the additions map onto the architecture that exists

```
param tol  ──────────────────────────────────────────────►
   parse/ParamDecl grammar  →  POM Param.distribution()   (authored, walkable)
   →  inert at solve; host MC samples → restamp writes (MD-18, nothing new)

constraint block
   │  parse/            new body kind, sibling of `analog`/`digital`
   ▼
POM  Module::constraints           ← authored structure, per the UNBREAKABLE RULE
   │  elab/
   ▼
resolve/  margin expressions, interned ids
   │  resolve/diff.rs differentiates them w.r.t. params (§3.1)
   ▼
flatten/  → emit/  one kernel function per margin, CSE'd with the residual
   │                + the ∂F/∂p param-Jacobian kernel (§3.1)
   ▼
kernel/analog/constraints.rs       new capability sub-struct behind Option
   ▼
device/analog/constraints.rs       evaluated per accepted step, like limits.rs
   ▼
solver  reports margins + argmin (time, instance); HAS_SENSITIVITY gates
        param_stamps (§3.2); validation_reports() (§2.9) is the universal
        findings channel (constraint kernels, monitors, OSDI all emit here)
   ▼
piperine-api  margins/sensitivities on results; optimize/center/monte_carlo

tester (no grammar at all) ────────────────────────────────────────────────►
   host generator (§4.3)  →  sequencer `Element`: `next_breakpoints`
   (`core/element.rs:187`, already exists) + a resume callback
   →  drive installs stamp *segments*; the element stamps them between
      breakpoints, so no host crossing enters a Newton iteration (§4.2)
   →  `expect(...)` reports into the same `validation_reports()` channel
```

Every arrow lands on a file or pattern that already exists — the tester most of
all, since `next_breakpoints` was in the ABI before anyone designed a tester for
it. The new capability
sub-struct behind `Option` is exactly how `forces.rs`/`limits.rs`/
`operators.rs`/`events.rs` are already organized — a circuit with no
constraints pays nothing.

**Five existing rules constrain the design, and all five are satisfiable:**

- **POM navigability (UNBREAKABLE).** Constraints and `tol` are authored
  structure and attach to the module/param as written. A hierarchical
  `require` referencing `u3.m1.d` resolves through the authored instance tree,
  never through `flat_modules`.
- **MD-18 (compile once).** Constraints, margin kernels, and ∂F/∂p kernels are
  emitted at compile time. An optimizer iteration changes parameter *values*
  and restamps — it must never re-elaborate. `compile_once_sweep.rs` is the
  guard.
- **MD-24 (declared surface).** New referenceable names get textual
  declarations: `headers/statistics.phdl` for distributions (`gauss`,
  `uniform`), `headers/constraints.phdl` for the margin helpers and scope
  terms (`ac_gain`, `ac_phase_margin`, `dc`/`tran`/`ac` as event terms, …) as
  `extern fn`/`extern operator`. No Rust-side registry of magic names;
  `extern_coverage_guard.rs` extends. Each helper's declaration also carries
  its **class** — pointwise or reduced (§2.2) — because that decides whether it
  can be lowered into a per-point kernel at all. `abs`/`log10`/`sqrt` are already
  declared (`headers/math.phdl:23-27`), so the sketches type-check today.
  `m1.region` wants a declared `enum Region { Cutoff, Triode, Saturation }` —
  PHDL already has enums in the prelude (`prelude.phdl:13`
  `enum Scale { Lin, Dec, Oct }`), so the region opvar is an ordinary enum-valued
  observable, not integer codes.
- **Fail loud.** A constraint referencing an unknown signal, a
  non-differentiable metric asked for a gradient, a `tol` distribution with
  unknown name: compile or analysis errors, never a silent `0.0`.
- **No-Bloat.** The entire language delta is one clause on `ParamDecl` and one
  body kind. Both were argued in §1 and §2 against the host-side alternative
  and survived.

---

## 8. A complete worked example

One file, one Python driver, all three tools reading one declaration.

```phdl
use piperine::disciplines;
use spice::mos;

mod Ota(input vip : Electrical, input vin : Electrical,
        output vout : Electrical, inout vdd : Electrical, inout gnd : Electrical) {
    param w_in   : Real = 4e-6  tol gauss(sigma_rel = 0.01);
    param w_load : Real = 8e-6  tol gauss(sigma_rel = 0.01);
    param ibias  : Real = 20e-6;
    param l      : Real = 180e-9;

    m1 : Mos1(.d = n1, .g = vip, .s = tail, .b = gnd) { .w = w_in, .l = l };
    m2 : Mos1(.d = vout, .g = vin, .s = tail, .b = gnd) { .w = w_in, .l = l };
    // … load, bias, compensation …
}

constraint Ota {
    // physics — inherited SOA from Mos1's own constraint block also applies
    @ dc {
        require all_saturated : m1.region == saturation && m2.region == saturation;
        require headroom      : V(vout, gnd) >= 0.25 && V(vdd, vout) >= 0.25;
    }
    @ tran {
        require soa_out       : abs(V(vout, gnd)) <= 1.98;
    }

    // pointwise, DC — a per-point kernel (§2.2)
    var idd     : Real = abs(I(vdd));

    // pointwise, AC — evaluated at each frequency point
    var gain_db : Real = 20.0 * log10(abs(ac_gain(vout, vip)));

    // reduced, AC — properties of the whole sweep, not of any one point
    var ugbw    : Real = ac_unity_gain_freq(vout, vip);
    var pm      : Real = ac_phase_margin(vout, vip);

    target gain_db >= 65.0 tol 3.0;
    target ugbw    >= 20e6 tol 2e6;
    target pm      >= 60.0 tol 5.0;
}
```

```python
import piperine

ota = piperine.load("ota.phdl").module("Ota")

# 1 ─ verification: strict posture. DC requires and DC vars off the DC result.
r = ota.op()
assert r.requires_ok, r.first_violation      # instance, value, margin
print(r.margins["headroom"], r.margins["idd"])

# AC targets are read where they are defined — `r.margins["gain_db"]` would be
# a loud "not defined in this analysis" (§2.1).
a = ota.ac(piperine.AcConfig(start=1e3, stop=1e9, points=201))
print(a.margins["gain_db"], a.margins["ugbw"], a.margins["pm"])

# 2 ─ optimization: gradients, not guesses (constraints run in collect posture)
opt = ota.optimize(
    objective="idd",            # a DC var — differentiable through the DC adjoint
    over={"w_in": (1e-6, 40e-6), "w_load": (1e-6, 40e-6), "ibias": (5e-6, 200e-6)},
    method="gradient",          # adjoint sensitivities from the compiler
    corners=["tt", "ss", "ff"],
)
print(opt.params, opt.iterations)            # ~10^1-10^2 sims, not 10^3-10^5

# The AC targets constrain that search. Gradients through them need the AC
# adjoint (§3.1) — with DC-only sensitivity they are constraints the optimizer
# can *evaluate* but not differentiate, so it finite-differences those or
# treats them as feasibility filters — the accepted cost of D6's DC-first call.

# 3 ─ centering: push away from every boundary, in sigmas
c = ota.center(over=opt.bounds, corners=["tt", "ss", "ff"], temp=[-40, 27, 125])
print(f"worst-case distance: {c.wcd:.2f} sigma")

# 4 ─ confirm with statistics; the failing sample is reproducible
mc = ota.monte_carlo(n=1000, seed=7, params=c.params)
print(f"yield {mc.yield_():.4f}   worst: {mc.worst('headroom')}")
```

One `constraint` block, two `tol` clauses. Verified, optimized, centered, and
yield-confirmed, with no restatement of intent anywhere in the flow.

The example is deliberately honest about the seam it sits on: an OTA's
*specification* is mostly AC, its cheapest *gradient* is DC, and the two
classes of `var` (§2.2) are not interchangeable. Any version of this snippet
that reads `gain_db` off `op()` is describing a language that cannot be built.

---

## 9. Sequencing

**All three waves are V1** (user decision, 2026-07-27). This is not a staging
plan with two halves deferred to a later release — it is the delivery order of
one release, and the reason is competitive: the whole advantage argued in §11 is
that verification, optimization, and centering read one declaration. Ship only
the declaration and the advantage is a claim; ship only the optimizer and it is
another sizing tool. The waves are ordered by dependency, not by commitment.

"Wave" replaces the earlier "Tier" for exactly that reason — a tier reads like a
priority, a wave reads like a sequence.

**Wave 1 — the foundation (V1)**

1. `tol` on `param`: grammar clause, POM `Param.distribution()`,
   `headers/statistics.phdl`. Inert at solve; readable by the host.
2. `constraint` block: grammar, POM, `require`/`var`/`target`,
   margin lowering, the three postures. No temporal windows yet.
3. Margin evaluation in the kernel (`kernel/analog/constraints.rs` +
   `device/analog/constraints.rs`) + the `validation_reports()` ABI hook,
   with instance and time provenance.
4. `constraint` blocks on the `headers/spice/` models → **SOA on every device
   in every design, for free.** Highest industrial value per line of code here.
   (This pulls the operating-region opvar — `m1.region` — forward into the same
   headers work; an introspection addition, not a language change.)
5. The ∂F/∂p kernel (**one kernel, parameter-indexed** — §3.1's `.disto`
   lesson) + `param_stamps` into a caller sink + `HAS_SENSITIVITY`; one adjoint
   sensitivity driver; stamp-perturbation fallback for elements without the bit.
   The driver is **DC** (D6); note that the noise adjoint it is often said to
   reuse is the AC shape, so the transpose solve here is a sibling path, not a
   reuse.

**Wave 2 — optimization, statistics, centering (V1)**

Nothing here touches the grammar except item 9. All of it is host code over the
Wave-1 primitives, and all of it is the reason Wave 1 exists.

6. **Host optimizer over the restamp loop.** Gradient-based by default, using
   the DC adjoint (D6) for differentiable objectives and constraints. Structure:
   a feasibility phase (climb until every `require` margin ≥ 0) followed by an
   objective phase (descend the objective while projecting onto the feasible
   set), constraints supplied as `{m_i ≥ 0}` from the margin channel. Runs in
   `collect` posture by definition (§2.5) — an optimizer that aborted on its own
   infeasible iterates could not search. Black-box fallback (CMA-ES, or Bayesian
   optimization with a GP surrogate) for the parts the compiler cannot
   differentiate: discrete knobs, event-detected metrics, reduced metrics whose
   reduction is non-differentiable. The two are composable — gradient inside a
   discrete shell — and *which* engine produced a given result is reported, never
   inferred. `piperine-api` owns the driver so Rust and Python hosts get one
   implementation (MD-22); scipy/BoTorch stay optional accelerators on the Python
   side, never the source of truth.
7. **Monte Carlo over `tol` declarations.** Sample every declared distribution
   under one explicit seed, restamp, re-solve, collect margins. The mismatch/
   process distinction from §1 is the whole engine: a `global` draw is shared by
   every instance in the sample, a plain draw is independent per instance — so
   sampling walks the *authored instance tree* to decide how many draws a
   parameter needs. Every sample is reproducible from `(seed, index)`, which is
   what makes `mc.worst("headroom")` a debuggable artifact instead of an anecdote:
   the failing sample can be replayed as an ordinary session. Yield is the
   fraction of samples with all margins ≥ 0; per-metric spread `σ_i` is the
   by-product that Wave-2 item 8 needs. Cost is 10³ restamps on one JIT (MD-18),
   never 10³ elaborations.
8. **Design centering** — the `center` policy of item 6's driver (§5), not a
   separate engine. Maximize `min_i (m_i / σ_i)`: the margin convention (§2.3)
   already normalizes, item 7 supplies `σ_i`, item 5's gradients make it a
   smooth-ish program rather than Monte-Carlo-in-the-loop. Two honest wrinkles: `min` over
   constraints is non-smooth, resolved the same way as `min` over time (D1 —
   differentiate at the active constraint, documented); and `σ_i` itself depends
   on the design point, so a rigorous formulation re-estimates spread as the
   design moves. Cheap version: hold `σ_i` fixed within an outer iteration and
   re-sample between them. This is the item a working analog designer would name
   as the reason to adopt the tool.
9. **Scoped `require` windows** — the one grammar addition in this wave: event
   blocks inside constraint bodies (§2.7), `after =`/`dur =` event-term
   arguments, and the `|`/`&`/`not` window algebra, all reusing the existing
   `EventBlock` production and `EventRegistry`. Wave 1 ships analysis-scoped
   blocks (`@ dc`/`@ tran`/`@ ac`); this adds the temporal windows that make
   `require settle : …` expressible. The empty-scope rule (§2.7) is what keeps a
   misspelled trigger from masquerading as a pass, and it is worth landing with
   the windows rather than after them.

**Wave 3 — verification at scale (V1)**

The audit of what each item below *actually* costs in language surface is the
surprising part: across all three waves the total grammar delta is the `tol`
clause, the `constraint` block, one `cover` statement kind, and one body kind.
Everything else is host code, stdlib, or implementing declarations that already
exist in the formal spec.

10. **Operating-region coverage.** One new statement kind inside the
    `constraint` block, not a new construct:
    `cover input_cm : V(vip, gnd) bins [0.4:0.1:1.4];` and
    `cover region_m1 : m1.region in {cutoff, triode, saturation};`

    Kernel side it is *the same per-point expression evaluation as a margin*
    plus a bin-mapper — the pointwise path from §2.2 with a different reducer,
    so no new machinery. A bin hit is a counter increment, which makes coverage
    strictly cheaper per point than a margin (no signed distance, no argmin
    tracking). Cross coverage (`cover cross temp_x_region`) is a pair of
    expressions binned jointly; the bin space is the product, so the language
    should refuse a cross whose product exceeds a declared cap rather than
    quietly allocating a sparse 10⁶-bin table (D10).

    The state, though, is unlike anything else in this document: coverage
    **accumulates across runs**, which margins never do. A margin is a property
    of one analysis; coverage is a property of a whole regression. So the
    counters live on the host, merge across runs and seeds, and persist between
    sessions — the vManager role, living where vManager lives, outside the
    language. Closure reporting ("which bins are empty, and which stimulus would
    fill them") is the actual deliverable, and it is host analysis over merged
    counters. The `m1.region` opvar it samples already landed with Wave-1 item 4.

Two design consequences, both now decided. Coverage is the only construct here
    whose value is *accumulation*, so it gets its own posture — `cover=on|off` on
    `Context` (D9), not a ride on `checks=`, or an optimizer's 10³ inner-loop
    iterates pollute the database with runs nobody meant as verification. And the
    joint bin count of a cross is checked **at elaboration** against a
    `Context`-raisable cap (D10): bin edges are literal ranges, so the product is
    known statically and a 10⁶-bin cross is refused before anything allocates.

11. **Sequential properties — what is left after the tester.** Most protocol
    checking happens at clock edges, which is the *tester's* rhythm (§4.1), so
    the tester checks it imperatively in Python and no language construct is
    needed. What remains for an in-language monitor is the narrow case: an
    obligation that must be watched **continuously at the solver's rhythm**, or
    one buried deep enough in the hierarchy that it should ship with the block
    rather than with the test.

    For that residue: a monitor is a small ordinary `digital` module — registers,
    `match` on state — reporting through `validation_reports()` (§2.9). No SVA
    import, no assertion engine, no `ok`-net plumbing: PHDL's `digital` grammar
    already expresses FSMs and hierarchy already composes them. A monitor is
    parameterizable, instantiable in an array, and shippable in a library, which
    an assertion one-liner is not; the cost is that `req |-> ##[1:3] ack` becomes
    a handful of states.

    A dedicated sequence syntax stays inadmissible until usage proves it: with
    the tester covering the common case, the evidence bar for growing the grammar
    here is now higher, not lower.

    The one genuinely new piece is the **digital poll site** — §2.5's table
    defines evaluation points for analog analyses; a monitor fires on the digital
    scheduler's accepted events, where "accepted" means after event settling, not
    mid-delta-cycle.

12. **Implement the declared `resolve` kinds for `Real` storage nets.** Not RNM
    (dropped — see below); this is closing a gap in *declared* language.
    `docs/spec/part_i_language.md:477` carries the production
    `ResolveDecl ::= "resolve" ("tri"|"or"|"and"|"sum"|"avg"|"max"|"min") ";"`
    and `:484` assigns `sum`/`avg`/`max`/`min` to `Real` storage nets; `:238`
    already makes those kinds contextual keywords. The grammar promises them and
    the implementation does not deliver, which is precisely the kind of debt MD-24
    exists to prevent.

    `sum` is the one that earns its place: it is how a real-valued summing node
    works. All four declared kinds are order-independent reductions by
    construction, which is why `last_write` is *not* in the set — multi-driver
    resolution must not depend on evaluation order.

    No new keyword, no new body kind, no methodology. This is finishing what the
    spec already says.

13. **High-sigma importance sampling** — the third policy of item 6's driver
    (§5). Plain Monte Carlo needs ~10⁸ samples to
    observe a 6σ failure, which is why memory bitcells are verified with
    importance sampling instead: **statistical blockade** (train a classifier on
    cheap samples, then simulate only the ones predicted near the tail),
    **scaled-sigma sampling** (sample at inflated σ, extrapolate the failure
    probability back), and subset simulation. All three are host methods over the
    Wave-2 item 7 restamp loop — sampling policy, not language.

    Two hooks they need from earlier waves, both already present: the margin is a
    *continuous* signed distance (§2.3), which is what makes a classifier or an
    extrapolation possible at all — a pass/fail bit carries far less information;
    and `(seed, index)` reproducibility (item 7), because a tail sample that
    cannot be replayed cannot be debugged. A yield number reported without saying
    which estimator produced it is not a yield number, so the estimator and its
    confidence interval travel with the result.

14. **Aging / reliability (HCI, NBTI, TDDB).** Host-side parameter drift over a
    declared lifetime: the model ships aging coefficients as ordinary params, the
    host computes stress from an operating-point or transient run, then restamps
    the drifted parameters and re-verifies. Fresh-versus-aged is two runs of the
    same margins, and the deliverable is the margin *delta* — "which constraint
    goes negative first, and after how long" — which the margin channel already
    expresses without any new type.

    No language surface: `tol` already proved that variation metadata belongs on
    the parameter, so a future `drift` clause has an obvious shape if one is ever
    wanted, and until then aging coefficients are just params. The honest gap
    versus MOSRA/RelXpert is the *model data*, not the mechanism — a stress
    equation per device is model-authoring work, and the built-in ngspice-faithful
    models have none of it (same reasoning as §2.8's absent SOA limits).

15. **The tester library.** §4 is the design; this is the delivery. A sequencer
    element (`next_breakpoints` + a host resume callback) plus the host-side API:
    `advance(dt)`, `drive_voltage`/`drive_current`/`ramp` with declared impedance,
    `drive` for digital nets (through the `EventSink`, never the MNA),
    `read_voltage`/`read_port`, and `expect(...)`/`warn(...)` into
    `validation_reports()` (§2.9). Python and Rust implement one contract (D15),
    with the generator form of §4.3 as Python's ergonomic face.

    **This item moved earlier in the wave, and it is why RNM left.** A tester with
    analog drive covers every use of RNM-as-testbench-component: any stimulus at
    all, expressed imperatively where a designer can reason about it. Note the
    scope shift from the original sketch — this is not merely "acceptance suites
    become instantiable parts". It is the primary functional-verification path,
    and the reason items 11 and 12 shrank.

    It remains the item most likely to expose gaps in the `Element` ABI's
    imperative surface: a device that wants to *wait* is not how the ABI reads
    today, even though `next_breakpoints` provides the scheduling half.

**Dropped from V1: real-number modeling** (user decision, D16). RNM is a
throughput play, and two mechanisms already cover what it was here to do:

- **RNM as a testbench component** — abstract neighbor, stimulus generator,
  boundary checker — is fully replaced by the programmatic tester (§4), which
  does the same job imperatively, in the host, with declared drive impedance.
- **RNM as internal block abstraction** is mostly covered by writing the abstract
  model as an ordinary simple `analog` or `digital` module. The large speedup
  comes from *not simulating transistors*, which a simple module already gets.

What leaving the MNA additionally buys — no Newton iterations, no LTE-limited
timesteps, a smaller matrix — is real but the smaller factor, and unquantified
for this codebase. Against that: the `behavioral` body kind (D8, now reverted),
and a model-versus-SPICE **correlation methodology** that RNM requires and this
document never had. A real-valued model can be wrong while the simulation passes;
industry answers that with correlation decks and acceptance criteria as
first-class activity. Inventing that in V1 is a whole feature hidden inside a
wave item.

Also decisive: a whole design is already verifiable today, in one process,
without RNM. RNM would make that faster; it does not make it possible. Piperine's
defensible ground is differentiability, not throughput — competing on speed
against Spectre X and PrimeSim is choosing their terrain.

Recorded as **V2, with reasons** — a good idea at the wrong time, which is worth
writing down rather than quietly omitting. The declared `resolve` kinds (item 12)
stay in V1 because they are declared-language debt, independent of RNM; the
declared-impedance bridge stays as an ordinary module pattern (§2.8), which it
always was.

**Explicitly not planned:** analog formal/reachability (research-scale, does
not survive contact with a real netlist), RL/GNN sizing (dominated by the
gradient path for continuous sizing; revisit only for topology choices).

**Where the grammar delta actually lands**, now that all three waves are V1:

| Wave | Grammar addition | Everything else |
|---|---|---|
| 1 | `tol` clause on `param`; `constraint` block with `require`/`var`/`target`; analysis-scoped `@ dc`/`@ tran`/`@ ac` blocks | kernel margin evaluation, validation channel + capability bit, ∂F/∂p kernel, DC adjoint, SOA blocks on the spice models |
| 2 | event-term `after =`/`dur =` args + window algebra (reusing `EventBlock`) | optimizer, Monte Carlo, centering — all host |
| 3 | `cover` statement kind | declared `resolve` kinds (grammar already exists), monitors (ordinary `digital` modules), the tester library, high-sigma, aging — all host or stdlib |

**Four grammar additions**, for a feature set the incumbents spread across five
separate tools. It was five before the tester displaced RNM (D16) and took the
`behavioral` body kind with it — the strongest possible No-Bloat outcome, since a
*capability* decision made the language *smaller*.

The dependency spine is short: **`tol` (1) and margins (2–3) unlock SOA (4);
margins plus gradients (5) unlock optimization (6); those plus sampling (7)
unlock centering (8).** Item 4 is the cheapest real win and item 8 is the
biggest one, and both stand on the same convention from §2.3.

---

## 10. Decisions and open questions

An expertise pass on 2026-07-27 closed five earlier inconsistencies in place
(they are now design text, not questions): the two classes of measured quantity
(§2.2), which points a margin is evaluated at (§2.5), contextual keywords
against the frozen corpus (§2.6), SOA limits defaulting to absent (§2.8), and
the validation channel's capability bit and `Option` shape (§2.9). What follows
is what genuinely still needs a decision.

### 10.1 Decided (2026-07-27, user)

| # | Question | Decision |
|---|---|---|
| D1 | Non-smooth worst case | **Differentiate at the argmin**, documented in the result object. A gradient that silently smoothed the constraint would be a plausible wrong number — the failure mode the project's fail-loud rule exists to prevent. |
| D2 | Unscoped `require` default | **Holds in every analysis**, but the default is a **`Context` field**, not a hard-coded rule. §2.5 already removed the false-positive sources (homotopy stages, rejected steps, UIC at `t = 0`); the knob covers the residue without making the safe behavior conditional on remembering to ask for it. |
| D3 | Margins: rows or own channel | **Own channel.** Margins are per-point scalars with provenance, and §2.2's pointwise/reduced split gives them two shapes; hammering both onto the waveform rows would produce exactly the frankenstein the nine-type taxonomy rule exists to prevent. |
| D4 | Constraints under monomorphization | **Variants carry them, like any body.** Monomorphization already clones a module's `analog`/`digital` behavior into `urc__5`; a `constraint` block rides the same path. The POM holds one authored block per authored module and the variant carries its copy — no special representation, no new rule. |
| D5 | Wave staging | **All three waves are V1** (§9). The advantage is the whole loop; a partial delivery is a claim. |
| D6 | DC adjoint or AC first | **DC.** `.sens` (`analyses/sens.rs:2`) is a finite-difference oracle to verify against, and AC has none — an unverifiable gradient is worth less than a slower verifiable one. Consequence: AC-metric gradients (gain, UGBW, phase margin) are finite-differenced or treated as feasibility filters until the AC adjoint lands. |
| D7 | Where reduced metrics are computed | **Host.** The solver stays pointwise-pure: it emits points, the host applies the declared reduction. Keeps the kernel free of sweep-shaped state and keeps `piperine-solver` free of a second notion of "result". |

D2 and D3 both landed on the same instinct: make the safe thing the default,
and give it a knob rather than a special case.

| D8 | RNM bodies: explicit or inferred | **Superseded by D16.** Was: explicit `behavioral` body kind, so that leaving the MNA is declared rather than inferred. The reasoning held; the premise did not survive — with RNM dropped there is no event-scheduled body to declare. Kept in the log because the *rule* it established still applies to any future construct that changes what numerical question a module answers: declare it, do not infer it. |
| D9 | Does `cover` get its own posture | **Yes — `cover=on\|off` on `Context`,** beside D2's scoping knob. Coverage is the one construct whose value is accumulation across runs, so riding `checks=` would let an optimizer's 10³ inner-loop iterates pollute the coverage database with iterates nobody meant as verification runs. Same instinct as D2/D3: separate concern, separate knob. |
| D10 | Cross-coverage bin cap | **Loud at elaboration against a default cap, cap raisable on `Context`** (agent's call). Bin edges are literal ranges, so the joint bin count is known statically — elaboration can refuse a 10⁶-bin cross before anything allocates. A runtime warning was the alternative and loses: this project's rule is fail loud, and an unguarded product silently allocating a sparse table surfaces much later as memory, far from its cause. Raisable rather than fixed so a legitimately large cross is one explicit line, not a fork of the compiler. |

| D11 | Tester rollback | **Explicit time advance is the guarantee** (§4.4). `advance(dt)` inserts a breakpoint and asserts nothing the tester cares about changes inside the window; the tester resumes only at that breakpoint, accepted by construction. No rollback protocol. Accepted cost: landing on every tester breakpoint constrains the solver's timestep, so a fine-grained tester runs slower — a known inefficiency rather than an unknown failure mode. |
| D12 | Tester side effects | **Allowed** (§4.5). Reading a vector file, consulting a golden model, logging — a testbench is not a device model, and forbidding this kills the use case. Consequence stated, not policed: reproducibility becomes the tester author's responsibility, and it is the one link a Monte Carlo seed does not cover. |
| D13 | Tester in AC | **Known gap, fails loud** (§4.7). A tester sequences time and has no AC meaning; DC treatment of installed drive segments is also unspecified. Refuse the combination rather than silently freezing or ignoring the tester. |
| D14 | Tester reach into hierarchy | **Read internal nets freely; drive them loudly** (agent default). Reading is what the probe/trace surface already does. Forcing an internal node changes the circuit, so it lands in the run's findings — a test that passes while forcing a node must be visibly doing so. |
| D15 | Tester language | **One contract, two implementations.** A sequencer `Element` driven by a host resume callback; Python and Rust both implement it (MD-22 parity falls out). Python gets the generator ergonomics (§4.3) because it is better at this. |
| D16 | RNM | **Dropped from V1, recorded as V2 with reasons** (§9 item 15). The tester replaces RNM-as-testbench-component outright; ordinary simple modules cover most of RNM-as-internal-abstraction. What remains is throughput, and throughput is the incumbents' terrain. Takes D8's `behavioral` body kind with it and drops the grammar delta from five to four. The declared `resolve` kinds stay in V1 as declared-language debt. |

D2, D3, D9, and D10 all landed on the same instinct: make the safe thing the
default, and give it a knob rather than a special case. D11–D16 landed on a
different one, and it is the more surprising result of this pass: **the strongest
capability decision in the document made the language smaller.**

### 10.2 Still open

**Nothing blocking.** Two things to revisit once there is usage rather than
argument:

1. **An imperative sequencing syntax, if the generator form proves awkward.**
   §4.3's generator carries imperative state in a frame, which is the cheapest
   possible answer and needs no grammar. If real testers show it reads badly —
   deeply nested protocol phases, or Rust's version being much clumsier than
   Python's — that is evidence for sugar. Not before.
2. **An in-language sequence syntax for monitors** (`##`-style) stays
   inadmissible for now, and the bar went *up*: with the tester covering
   clock-edge checking (§9 item 11), the residue that needs an in-language
   monitor is narrow, so growing the grammar for it would need a strong case.

The AC/DC semantics of a tester (D13) is a recorded known gap rather than an open
question — it is deliberately unspecified, and refusing the combination loudly is
the interim behavior.---

## 11. Why this is worth doing

The honest competitive read: commercial AMS tools have better device models,
better layout integration, and decades of foundry qualification. Piperine will
not win on model breadth.

What they cannot easily do is make the simulator *differentiable* and make
"what correct means" a **single first-class declaration** shared by
verification, optimization, and centering. Their simulators are closed
numerical kernels with optimizer cockpits bolted on; the gradient information
is not there to expose. Piperine's compiler already computes symbolic
derivatives as a matter of course.

That is a narrow but real advantage, and §2.3's margin convention is what
turns it from a numerical curiosity into a design methodology. Two grammar
additions buy it. Everything else is engineering we already know how to do.
