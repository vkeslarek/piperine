# GAPS.md — spec ↔ implementation audit

**Date:** 2026-08-08, **amended 2026-08-09** (Tier O) ·
**Branch:** `feature/device-verification` ·
**Scope:** `docs/spec/` (Parts I–X + A/B/C, ~10.9k lines) read against
`crates/*/src`, `src/`, `tests/`, `examples/`, `headers/`; plus, as of the
amendment, the `piperine new` onboarding path exercised for real.

This is **not** a rehash of `SPEC_DEVIATIONS.md` or `ROADMAP.md`. Those two
already record what the project knows it owes. This document records what the
audit found that **neither of them names**, plus the small set of already-known
items whose *severity* the audit re-rates.

Ranking rule used throughout: a gap where the toolchain **silently does the
wrong thing or nothing** outranks a gap where it **fails loud**, which outranks
a gap that is only **unvalidated**, which outranks a **documentation** gap.
That is the project's own fail-loud convention (CLAUDE.md, ROADMAP.md) applied
as a triage key.

**Bottom line (revised 2026-08-09).** The original reading — "the engine is
fine, the language frontend is where the gaps are" — survived a spec-vs-source
read but not first contact with an actual design. Building one small mixed-signal
circuit surfaced **A0** (a current probe in a `constraint` silently returns the
branch voltage, affecting three shipped SOA rules) and **A0b** (a driven
signal-flow net makes the MNA matrix singular). Both are wrong-answer or
dead-on-arrival defects in delivered, spec'd, test-covered features.

So the honest bottom line is: gaps cluster wherever **nothing executes the
feature end to end**. The frontend items (§A1–A4) and the onboarding path
(§O1–O4) are still real. A0/A0b say the pattern is not confined to the frontend.

---

## Legend

| Tier | Meaning |
|---|---|
| **A** | Silent — accepted by the compiler, then dropped or computed wrong. Violates fail-loud. |
| **O** | Onboarding — the `piperine new` path a first-time user walks. |
| **B** | Fail-loud hole — honest error, but a spec-declared construct is unusable. |
| **C** | Spec hole — the spec is silent, ambiguous, or self-contradictory. |
| **D** | Unvalidated — implemented, but nothing falsifies it. |
| **E** | Docs / infrastructure. |

`[tracked]` = already named in `ROADMAP.md` or `SPEC_DEVIATIONS.md`.
Everything else is new in this audit.

---

# Tier A — Silent gaps (fail-loud violations)

These are the ones that justify mexida. In each case a user writes a construct
the spec declares normative, the compiler accepts it without a word, and the
construct does nothing.

## A0 — `I(a, b)` inside a `constraint` returns the branch **voltage**

**Added 2026-08-09. The most serious defect in this document.** A current probe
in a `constraint` block silently evaluates to the branch voltage. Wrong number,
no error, in the feature whose entire job is catching wrong numbers.

### The mechanism

`crates/piperine-codegen/src/emit/analog_expr.rs:50` dispatches both names to
one function:

```rust
"V" | "I" => return self.emit_analog_branch(args),
```

and `emit_analog_branch` (`:121-132`) never looks at which one it was called
for:

```rust
fn emit_analog_branch(&mut self, args: &[Expr]) -> Result<Value, CodegenError> {
    let plus_name  = ident_from_expr(args.first()).unwrap_or_else(|| "?".into());
    let minus_name = ident_from_expr(args.get(1)).unwrap_or_else(|| "0".into());
    let plus  = self.resolve_node(&plus_name)?;
    let minus = self.resolve_node(&minus_name)?;
    self.branch_voltages.as_ref().expect("analog context")
        .get(&(plus, minus))
        .copied()
        …
}
```

`branch_voltages`, unconditionally. There is no flow lookup anywhere in this
emitter. Every `I(...)` reaching `emit_analog` is a voltage.

**Why ordinary device models are unaffected:** the contribution path is a
different pipeline. `flatten/analog.rs` carries real `NatureKind::Flow`
handling (`:689`) and current-specific cases at `:1045`, `:1064`, `:1088`,
`:1106`, `:1123`. `res`, `dio`, `mos1` and friends compute correctly. It is the
**expression** emitter — the one the constraint/margin kernel reads through —
that collapses the two natures.

### The proof

Ideal 12 V source into 6 Ω. True branch current 2.000000 A, branch voltage
12.000000 V. Two requires against the same generous limit:

```phdl
constraint Vs {
    require probe_i : abs(I(p, n)) <= ilim;   // ilim = 100
    require probe_v : abs(V(p, n)) <= ilim;
}
```

```
v(a) = 12.000000
    probe_i  margin=   88.000000   =>  |probe| = 12.000000     ← should be 2.0
    probe_v  margin=   88.000000   =>  |probe| = 12.000000     ← correct
```

Identical margins. The current probe reported the voltage.

### Blast radius — the shipped SOA blocks

Three `require`s in `headers/spice/` are checking the wrong quantity right now:

| File | Rule | Actually evaluates |
|---|---|---|
| `diode.phdl:503` | `require ifwd : I(p, n) <= model_if_max;` | `V(p,n) <= model_if_max` |
| `bjt.phdl:607` | `require ic : abs(I(c, e)) <= model_ic_max;` | `abs(V(c,e)) <= model_ic_max` |
| `bjt.phdl:608` | `require pdiss : abs(V(c, e)) * abs(I(c, e)) <= model_pd_max;` | `V(c,e)²` — a power limit computing volts squared |

`pdiss` is the one to sit with: a **power** constraint that never multiplies by
a current.

### Why nobody caught it

The SOA limits ship as absent-by-default sentinels, so every one of these
requires is *vacuously satisfied* until a foundry card overrides the limit. The
dv-core tests prove the channel plumbing — that a margin is produced, signed,
carries an argmin, and respects the three postures — and prove it with
**voltage** predicates. No test binds a real current limit and asserts the
margin's numeric value against a hand-computed current. The margin was always
present, always signed, always the wrong number.

This is the exact failure mode `tests/ngspice_parity_matrix.rs` was built to
prevent for device models — a claim with no oracle that can falsify it — and
the constraint family shipped without the equivalent.

### The fix

Thread the nature through the emitter: `emit_analog` must pass which of
`V`/`I` it matched into `emit_analog_branch`, and the `I` case must resolve the
branch *current* unknown (the same one `flatten/analog.rs` already knows how to
find) rather than index `branch_voltages`. If a branch current is not available
at a given evaluation point, that is a `CodegenError::Unsupported` — a loud
refusal is correct here and silence is not.

**Then add the oracle**: a constraint test with a hand-computable current
(ideal source into a resistor is enough — the repro above) asserting the margin
equals the arithmetic answer, so a regression cannot hide again. Without that
test the fix is unfalsifiable in exactly the way the bug was.

## A0b — a driven signal-flow (`Voltage`/`Current`) net makes the matrix singular

**Added 2026-08-09.** The storage-`Real` disciplines shipped in
`headers/disciplines.phdl` — `Voltage` and `Current`, the signal-flow pair —
are non-functional. A circuit containing one that is merely *driven* fails to
solve:

```phdl
mod Src(output s : Voltage) { param k : Real = 2.0; }
analog Src { s <- k; }
// …plus any ordinary Electrical sub-circuit, `s` read by nobody
```

```
solver error: SpaceMatrix: The library threw an error while trying to create
the RHS of the sparse matrix
Caused by: SymbolicSingular { index: 1 }
```

Removing the `Voltage` net (control run) solves fine. The design **builds**
without complaint — this surfaces only at solve time, as an opaque
linear-algebra error naming a matrix index, with nothing pointing at the net or
the discipline responsible.

Mechanism: `resolve/pom/structure.rs:30` deliberately classifies a storage-`Real`
discipline as **analog** domain (*"a storage discipline with `Real` storage is
analog (signal-flow)"*), so the net becomes an MNA node — but the drive `s <- k`
never stamps a row that determines it. Empty row and column ⇒ structurally
singular.

Two secondary findings on the same family, both real but subordinate:

- **Read-by-name does not work in an analog body.** `emit/analog_expr.rs:32-43`
  resolves a bare `Expr::Ident` against `params` and `vars` only, never
  `nodes` — even though the resolver holds a `nodes` map (`emit/resolver.rs:15`).
  `V(s)` works as a read; bare `s` is `unresolved analog identifier`. Part I
  §6.2 promises both halves: *"Storage — … **read/driven by name**"*.
- ~~**Digital storage nets are fine.**~~ **Retracted — see A0c.** The original
  note claimed a `Bit` net driven from an analog body and read from one "works
  and solves". It builds and solves; it does not work. Corrected below.

**Consequence for users:** signal-flow modelling — the natural way to write a
sensor output, a control voltage, a behavioral block diagram — is unavailable.
Every such signal must be carried as an `Electrical` net against ground. That
works, but it is a conservative node with a KCL row where an abstract value was
wanted, and it forces a `gnd` port onto blocks that have no physical need for
one.

**Fix ordering:** make it fail loud at build time first (a driven storage-`Real`
net that cannot be stamped is a named error, not a singular matrix at solve
time), then decide whether signal-flow gets a real stamping story or the
`Voltage`/`Current` disciplines are withdrawn from `headers/disciplines.phdl`
and Part I §6.2 is amended. Shipping declared, documented disciplines that
cannot appear in a solvable circuit is the worst of the three options.

## A1 — Solve-time diagnostics are collected and never executed

**Severity: highest.** `$display` / `$write` / `$info` / `$warn` / `$error` /
`$fatal` / `$finish` / `$assert` inside an `analog` or `digital` body are
**parsed, lowered, pushed into a vector, and never read**.

Evidence:

- `crates/piperine-codegen/src/flatten/analog.rs:500,514` — the analog path
  pushes a `FlatDiagnostic { severity, format }` into `out.diagnostics`.
- `crates/piperine-codegen/src/emit/stmt.rs:95` —
  `S::Diagnostic { .. } => Ok(())`, commented *"collected, not executed"*, on
  the **digital** path.
- `crates/piperine-codegen/src/kernel/analog/kernel.rs:693` —
  `AnalogKernel::diagnostics()` exists. Repo-wide grep for a call site returns
  **zero hits** outside its own definition. Nothing in `piperine-solver`,
  `piperine-api`, `piperine-python`, or the CLI ever reads it.
- Worse than "not printed": `flatten/analog.rs:497-499` keeps **only the format
  string literal** and drops every argument expression on the floor. The
  `PomStmt::Diagnostic` arm (`:514`) stores `format: String::new()` — the
  message text is discarded too.

Spec says otherwise, explicitly and in a table:

- Part V §7 (system-task availability matrix) marks
  `$assert`/`$info`/`$warn`/`$error`/`$fatal`, `$display`/`$write`, and
  `$finish` as **✓ in both the `analog` and `digital` columns**.
- Part I §10.5 (Diagnostics) presents them as behavior-block constructs.

Contrast: the **elaboration-time** path is fully implemented and correct —
`crates/piperine-lang/src/eval/tasks.rs:59` genuinely `println!`s, with proper
argument rendering (`display()` at `:73`). So the language has a working
diagnostic facility that stops existing the moment the code is compiled into a
device.

**Why it matters.** This is the single most-used debugging affordance in every
Verilog-family language. A device author instrumenting a convergence problem
with `$display` gets silence and concludes the branch never executes. `$fatal`
and `$finish` are worse than silent — they are *safety* constructs that a model
uses to refuse an out-of-range parameter, and today they let the simulation run
on. `$assert` in a body claims to check an invariant and checks nothing.

**Shape of the fix.** A `SimCtx` callback (§5.3 of Part X already owns the
runtime ABI) that the emitted code calls with a format id + evaluated args; the
host owns the sink (stdout, a `SimHooks` method, or the findings channel that
dv-core already built for validation reports). `$finish` maps to an
early-terminate on the analysis driver; `$fatal` to a `SolverDomain` error.
Argument evaluation is the real work — the args are ordinary analog expressions
and already emittable.

**Interim honesty option** (cheap, hours not days): make the four
non-implementable-today spellings a `CodegenError::Unsupported` at flatten
time, so silence becomes a loud error while the real feature is scheduled.

---

## A2 — `resolve` clauses on disciplines are parsed, then dropped

A `discipline` may declare `resolve tri|or|and|sum|avg|max|min` (Part I §6.2,
§6.3). The parser builds `ResolveKind` (`parse/parser/discipline_decl.rs:45-57`,
`parse/ast.rs:442`). Downstream, **the only consumer is a boolean**:

```rust
// crates/piperine-lang/src/elab/typecheck.rs:230
DisciplineItem::Resolve(_) => resolves = true,
```

That flag exists solely to **suppress** the `E2020 MultipleDrivers` error. The
*kind* is never read again — grep for `ResolveKind` across `elab/`, `pom/`, and
all of `piperine-codegen` returns nothing.

What actually happens at runtime: `piperine-solver` has exactly one hardcoded
resolution function, `LogicValue::resolve` (`src/digital/events.rs:20`),
implementing tri-state semantics only.

Consequences, in order of badness:

1. `resolve or` / `resolve and` on a `Quad` storage net silently behave as
   `tri`. A wired-AND bus reads as `X` where it should read `0`.
2. `resolve sum` / `avg` / `max` / `min` on a `Real` storage net — the entire
   numeric-bus-resolution feature of §6.3 — has **no implementation at all**.
   The clause's only observable effect is to disable the multiple-driver
   guard, so a user writing a summing bus gets a net with an arbitrary
   last-writer value and no error.
3. §6.3's "`Boolean` storage is single-driver only — it has no `resolve`
   clause" is unenforced: the parser accepts `resolve sum;` on a `Boolean`
   discipline and elaboration lets it through.

**Why it matters.** This is the only place in the audit where a user can get a
**numerically wrong waveform** with no diagnostic anywhere in the pipeline.
Everything else in Tier A is a missing effect; this one is a wrong answer.

**Shape of the fix.** Two independent halves, and they should be scheduled
separately:

- *Half 1 (small, do first):* reject at elaboration what is not implemented —
  `resolve sum|avg|max|min` and `resolve or|and` become named elaboration
  errors, `resolve tri` stays. Turns a wrong answer into a loud one. Also
  enforce the §6.3 kind↔storage-type table while in there.
- *Half 2 (real feature):* carry `ResolveKind` into the POM discipline record,
  into the digital net descriptor, and make the solver's resolution function a
  per-net choice instead of a hardcoded `tri`. Numeric (`sum`/`avg`/`max`/
  `min`) resolution additionally needs a `Real`-valued digital net, which the
  digital kernel's `LogicValue` net model does not have today — that is the
  expensive part and is arguably a separate feature.

---

## A3 — Bus-indexed assignment is unsupported, and the spec's flagship example uses it

```rust
// crates/piperine-codegen/src/emit/stmt.rs:120
_ => Err(CodegenError::unsupported("complex assignment target (bus indexing)")),
```

This one *does* fail loud, so it is Tier-A-by-association rather than by
silence: what makes it an A is that **Part I §10.3's normative worked example
does not compile**. The `SarAdc` block printed in the spec assigns
`code[N-1] = 1`, `code[idx] = 0`, `code[idx-1] = 1`. Every one of those lines
hits the error above.

The same example also uses a **multi-bit pattern** in its `match`, and:

```rust
// crates/piperine-codegen/src/emit/stmt.rs:246
_ => Err(CodegenError::unsupported(
    "multi-bit patterns in a digital `match` (bus signals)")),
```

Only the single-character forms `"0"`, `"1"`, `"?"` are handled
(`stmt.rs:238-245`). Part I §10.3 declares `BitPattern ::= "0b" {"0"|"1"|"?"}`
— i.e. `0b1??0` — and Appendix B §B.8 repeats it.

Corroborating coverage evidence: **no `examples/*.phdl` contains a `match` at
all**, and none contains an enum-state FSM. The 18 shipped examples are
combinational or analog. The language's sequential-digital story has no runnable
proof.

**Why it matters.** A reader who works through Part I §10 in order hits a spec
example that the compiler rejects. That is the worst possible first impression
of the digital half of the language, and it means the digital path's most
important idiom (a bit-sliced state machine) is unreachable.

**Shape of the fix.** Bus-indexed store is the substantive item: it needs a
read-modify-write on the vector net/var in the digital emit path (`store_var`/
`store_net` taking a bit index). Multi-bit patterns are mechanical once vectors
are addressable — a mask + compare against the non-`?` positions. Then add the
SarAdc (or a smaller FSM) to `examples/` so `tests/run_examples.rs` keeps it
honest.

---

## A4 — `enum … : Repr` is parsed, stored, and never consulted

Part I §6.4 declares an optional representation clause:
`enum OpCode : Bit[32] { … }`, with the rule *"the default is
`Bit[ceil(log2(count))]`"*.

- Parsed: `crates/piperine-lang/src/parse/parser/enum_decl.rs:16`.
- Stored: `parse/ast.rs:513` (`pub repr: Option<Type>`).
- Read: **nowhere.** The only repo-wide hit for `.repr` is the field
  declaration itself.

So the declared width is never checked against the variant count, never
propagated to the net type an enum-typed signal gets, and never reaches
codegen. A user writing `enum Phase : Bit[1] { A, B, C, D }` — four variants
into one bit — gets no error.

**Why it matters.** Lower blast radius than A1–A3 (enum values do work; only
the *repr* annotation is inert), but it is the same class of defect and the fix
is small: validate `repr` width ≥ `ceil(log2(count))` at elaboration, or reject
the clause as unimplemented. Doing nothing is the one option the project's own
rules forbid.

---

## A5 — Implicit widening is a compiler-internal table (MD-24 residue)

`crates/piperine-lang/src/elab/typecheck.rs:365-381` hardcodes the four
implicit widenings of Part I §6.1. The spec itself flags this
(*"Note (roadmap): the widening table is intended to become explicit
`impl From<T>` capability declarations in the prelude, replacing the
compiler-internal table"*), and §6.6 carries the twin note for intrinsic
operator capabilities.

Recorded here because it is the **last live MD-24 violation of the "declared
language surface" rule** and the spec's `extern` machinery (§5.4,
`extern impl Capability for TypeName`) was built precisely to close it — it
exists and is unused for this purpose. CLAUDE.md concedes the point: *"binary
operators are pure grammar, not dispatched through capabilities today"*.

`crates/piperine-lang/tests/extern_coverage_guard.rs` covers `MATH_FNS`, the
task registry, the operator contract, the 7 primitive types, and the
`@device`/`@port` schemas — **it does not cover the widening table or the
intrinsic operator impls**, which is why this has stayed invisible.

Low urgency, but it is the difference between MD-24 being a rule and being a
mostly-rule.

## A0c — an analog-driven `Bit` net reads back as 0 in another analog body

**Added 2026-08-09.** The analog↔digital coupling that Part I §10.4 calls out
by name (*"An analog crossing may drive digital state (domain coupling — the
ngspice switch idiom)"*) silently produces zero.

Minimal repro — `s = 1.0`, `th = 0.5`, so `d` must be 1 and both gates must sit
at 10 V:

```phdl
mod Gen(output d : Bit) { param th : Real = 0.5; var s : Real = 1.0; }
analog Gen { d <- s > th; }

mod Drv (input d : Bit, inout g : Electrical, inout gnd : Electrical) { param vdrv : Real = 10.0; }
analog Drv  { V(g, gnd) <- vdrv * d; }                                    // direct

mod DrvT(input d : Bit, inout g : Electrical, inout gnd : Electrical) { param vdrv : Real = 10.0; }
analog DrvT { V(g, gnd) <- vdrv * transition(d, 0.0, 1.0e-9, 1.0e-9); }   // via transition
```

```
direct  V(g)  = 0.0000   (expect 10.0)
transit V(gt) = 0.0000   (expect 10.0)
tran end V(g)=0.0000  V(gt)=0.0000
```

Both paths, DC and transient. The design **builds without a diagnostic** and
the solver converges — to the wrong answer.

**How it was found, and why that matters.** A behavioral buck converter whose
controller emits PWM on a `Bit` net and whose driver turns that into a gate
voltage. Every block elaborates, the circuit builds, the transient runs to
completion and reports plausible-looking statistics — and the converter simply
never switches. `v(gate)` is 0.0000 for the entire run. There is no error, no
warning, and nothing in the output that says "the thing under test did
nothing". Only probing an internal node reveals it.

Note the earlier misreading in **A0b**: an initial check concluded this path
"works and solves" on the evidence that it *built and produced a solution*.
That is exactly the trap this defect sets — and the same trap A0 sets, and the
same one the SOA constraints sat in for a whole release. A green run is not a
correct run.

### Root cause — found, and made loud 2026-08-09

Split the two directions and only one is broken:

| path | result |
|---|---|
| `Bit` driven by a **digital** body → read from analog | **10.0000 ✅** |
| `Bit` driven by an **analog** body → read from analog | **0.0000 ❌** |

The D2A read bridge is sound (`LowerCtx::shadow_var_for` allocates a shadow
`var`; a synthetic digital body assigns it every eval; `sync_vars` carries it
into the analog bank). The broken direction is the **write**.

`flatten/analog.rs::parse_dest` had a silent catch-all:

```rust
fn parse_dest(&self, dest: &PomExpr) -> Result<(NatureKind, NodeId, NodeId), CodegenError> {
    if let PomExpr::Call(func, args) = dest && let PomExpr::Ident(name) = func.as_ref() {
        …  // the V(a,b) / I(a,b) forms
    }
    Ok((NatureKind::Flow, NodeId::GROUND, NodeId::GROUND))   // ← everything else
}
```

`pwm <- duty > ramp` has `dest = Ident("pwm")` — not a `Call` — so it fell to
the catch-all and `add_force` emitted `add_flow(value, GROUND, GROUND)`: a
current contribution from ground to ground. The authored expression was
evaluated and discarded. Exactly the "silently emit a no-op" that CLAUDE.md
forbids by name.

**Fixed**: `parse_dest` now returns `CodegenError::Unsupported` naming the
authored net and pointing at the supported direction (a `digital` body reading
`V(...)` through the A2D bridge, `emit/builder.rs::analog_branch_voltage`).
Workspace suite green at 1841 after the change.

**Still open**: the *feature*. Driving a digital net from an analog body is
what Part I §10.4 advertises ("an analog crossing may drive digital state — the
ngspice switch idiom") and it is now a loud refusal rather than a working
path. Either build the A2D write bridge (mirror of the shadow mechanism: the
analog body publishes to a shadow, the module's digital body drives the net
from it) or amend §10.4 to say the comparison belongs in the `digital` body.
The refusal is honest; it is not the documented language.

## A0d — a `digital` block silently disables the module's analog `@timer`

**Added 2026-08-09.** Adding a `digital` body to a module stops the `@timer`
events in that module's `analog` body from firing. No error, no warning.

Minimal repro — one module, one line of difference. A sawtooth that must reset
every 10 µs:

```phdl
mod Ramp(inout p : Electrical, inout gnd : Electrical) {
    param tsw : Real = 1.0e-5;  param fsw : Real = 1.0e5;  var t0 : Real = 0.0;
}
analog Ramp {
    var r : Real = ($abstime() - t0) * fsw;
    V(p, gnd) <- r;
    @timer(tsw) { t0 = $abstime(); }
}
```

| module | t = 8.85 µs | t = 10.90 µs |
|---|---|---|
| as written above | 0.885 | **0.090** ✅ sawtooth reset |
| + `digital Ramp { q <- V(p, gnd) > 0.5; }` | 0.885 | **1.090** ❌ ramps forever |

It is per-module: a second module in the same simulation with an analog
`@timer` and no digital body keeps working in the very same run.

**Consequence.** Any mixed-signal block that needs both halves — a PWM
modulator, a sampled comparator, a clocked converter, i.e. the exact shape of
every design that motivates a mixed-signal simulator — loses its analog
timebase the moment it grows a digital body. And it loses it quietly: the
transient completes, the statistics look ordinary, the waveform is simply
wrong.

### Root cause — found and fixed 2026-08-09

The timer was never the problem. Instrumenting the event dispatch showed it
firing at exactly the right instants and its action writing exactly the right
value:

```
EV t=5.0000e-6 fired=[true] vars_before=[0.0]  ->  vars_after=[1.0]    (Toggle: s)
EV t=1.0000e-5 fired=[true] vars_before=[0.0]  ->  vars_after=[1e-5]   (Ramp: t0)
```

…and `vars_before` back at `0.0` on the *next* fire. The write was being
reverted between accepted steps, by the D2A bridge:

1. `DigitalLayout::build` allocates a bank slot for **every var in the
   module**, not just the ones the digital body uses. A module with both
   bodies shares one var namespace, so the analog side's `s`/`t0` got a digital
   slot the digital side never updates.
2. `export_vars` returned a bare `Vec<f64>` **sized by the digital var count
   but indexed by global `VarId`** — two different index spaces — leaving `0.0`
   filler in every position the digital side did not own.
3. `AnalogInstance::sync_vars` then blitted that whole vector **positionally**
   over the analog var bank after every `comb_phase`
   (`element.rs::accept_timestep` runs it whenever the digital kernel reads
   analog voltages, i.e. for exactly the mixed modules in question).

Net effect: an analog-owned module var was overwritten with a zero on every
combinational evaluation, microseconds after the `@timer` correctly wrote it.

**Three changes, all in the ownership contract:**

- `export_vars` returns `Vec<(VarId, f64)>` — identity, not position.
- `sync_vars` writes by `VarId` and only the entries handed to it — never a
  positional blit of the bank.
- `DigitalLayout` gains a `written` set: the vars the digital body actually
  writes at run time. Only those cross the bridge. Owning a *slot* is not
  owning a *value*.

Two traps inside that third change, both hit and both instructive:

- Seeding `written` from `body.regs` re-admits everything. **Latch inference
  marks a var read-before-assignment as a register**, so `regs` contains vars
  the digital body never writes — including the analog side's. A real register
  is assigned inside its clocked block, so the statement walk already covers it.
- Counting `Stmt::VarDecl` as a write re-admits it too. A declaration is a
  **power-on init** (`compile.rs` routes it to `reg_inits` when the var is a
  register), not a per-evaluation write. A genuine comb-local — declared,
  recomputed each pass, not a register — *is* digital-owned and still counts.

**Result**: the repro saws again with a digital body present; `cargo test
--workspace` 1841 passed, 0 failed, 0 warnings. The buck converter that
motivated all of this now switches and regulates — `v(load)` rises to ~12 V,
overshoots, and settles with the PI pulling toward its 5 V setpoint at 0.43 A.

**Follow-up worth having**: nothing in the suite covered a module with both an
`analog` and a `digital` body sharing a module-level var. That is the whole
mixed-signal premise, and its absence is why three separate ownership bugs
stacked up unseen in one code path.

---

# Tier O — Onboarding path (`piperine new`)

**Added 2026-08-09**, found by running the CLI rather than by reading it. The
original audit compared `docs/spec/` against `crates/*/src` and never walked the
path a new user takes. That path is broken end to end: **the very first command
a new user runs prints an error.**

Reproduction, verbatim, on a clean directory:

```
$ piperine new lab-bench-ps
Created piperine project `lab-bench-ps`
Building design for: .../lab-bench-ps/src/main.phdl
Elaboration failed in .../lab-bench-ps/src/main.phdl:
  × undefined type: `Electrical`
  ╰─▶ E2002
```

## O1 — The `new` template does not elaborate

`crates/piperine-cli/templates/main.phdl` (the mixed-signal "Hello World" that
`new.rs:39` embeds via `include_bytes!`) references `Electrical` and `Bit`
without `use piperine::disciplines;`. Elaboration fails with **E2002 undefined
type: `Electrical`**.

Confirmed by inserting the single missing `use` line into a generated project
and re-running `piperine build` — it then elaborates cleanly. **One-line fix in
the template.**

Severity is out of proportion to the fix: this is the first output of the first
command, and it tells a new user the toolchain is broken before they have
written a character.

## O2 — `new` aborts before the Python venv setup, and the code says it shouldn't

`new.rs:46` calls `build::execute`, which calls `std::process::exit(1)` on an
elaboration failure (`crates/piperine-cli/src/commands/build.rs:45`). The
process dies there, so `new.rs:49`'s `python_setup::setup(&abs_path)` **never
runs**.

The intent is documented as the exact opposite —
`crates/piperine-cli/src/commands/python_setup.rs:29`:

> *"Called by `piperine new` after the project skeleton is created. Prints
> progress + instructions; **never aborts `new` on a Python failure** (the PHDL
> project is still valid without Python — the setup is a bonus)."*

The comment guards against Python failing the project. Nobody guarded the
reverse: the **project** failing Python. So while O1 is live, **no new project
gets a `.venv`** — no `import piperine`, no IDE autocomplete, none of what
`python_setup.rs` exists to deliver.

O2 is independent of O1 and outlives it: any future template regression, or any
user error in a hand-edited `src/main.phdl` before the first `new`, silently
costs the venv. `build::execute` should return a `Result` and let the caller
decide, or `new` should sequence the Python setup before the build.

Verified on the generated project: `.venv/` absent, `target/` contains only an
empty `deps/`.

## O3 — CLI help advertises a frontend that was deleted

```
$ piperine --help
The Piperine Verilog-AMS toolchain

Commands:
  check   Parse and validate a Verilog-AMS file or project
```

Verilog-AMS was dropped entirely; PHDL is the only frontend (CLAUDE.md, and
Part III records the parallel `bench` removal). Cosmetic, but it is on the
`--help` screen and in the `check` subcommand description — the two strings a
new user reads first, immediately after the O1 error.

## O4 — No end-to-end test of the onboarding path

The root cause behind O1–O3 staying invisible. `crates/piperine-cli/tests/`
carries `add_cmd.rs`, `build_cmd.rs`, `check_cmd.rs`, `test_tb.rs` — **nothing
runs `new`**. A test that generates a project into a temp dir and asserts it
elaborates would have caught O1 on the commit that introduced it, and asserting
`.venv/` exists would have caught O2.

Note the asymmetry with the rest of the repo: `tests/run_examples.rs` gates
every `examples/*.phdl`, `tests/suite_hygiene.rs` walks the sources for
switched-off tests, `tests/delegation_guard.rs` forbids reintroduced copies.
The project guards its *contents* rigorously and its *front door* not at all.

**Fix order:** O4 first (the test), then O1 (one line, and the new test proves
it), then O2 (`build::execute` returns `Result`), then O3 (two strings).
Together they are well under a day.

---

# Fixed — T0: the transient stepper ratcheted `dt` to the floor

**Found and fixed 2026-08-09**, while running the buck converter of A0c. Kept
here because the *shape* of the bug is the most instructive thing in this
document.

## Symptom

A transient that never ends. 14 minutes of 100% CPU and **2.5 GB RSS** for a
3 ms run on an 8-node circuit; a 20 µs run — two switching periods — still
unfinished after 9 minutes. Adding **one 50 mΩ resistor in series with the
load** flipped a healthy 0.26 s run into an unbounded one. The cliff was sharp,
not gradual (0.1 Ω fine at 0.14 s; 0.05 Ω never returns) and non-monotonic in
`stop` (3 µs hung, 4 µs fine, 4.6 µs hung), which is what made it look like
chaos rather than a defect.

## Root cause

`PiController::propose_dt` (`analyses/convergence.rs`) is **not monotone in the
error**:

```rust
let de = (lte - prev) / lte.max(1e-30);      // normalized by the CURRENT lte
let p  = self.gains.kp + self.gains.ki * de;
let factor = lte.powf(-p).clamp(0.2, 1.5);
```

`de` is normalized by the *current* `lte`, so a step whose error **improves**
sharply — `lte` falling 20×, routine right after a switching edge — gives
`de ≈ −19` and `p = 0.7 + 0.4·(−19) = −6.9`. For the `lte < 1` of a
well-resolved step, `lte^(6.9)` underflows to the 0.2 shrink clamp. **The
controller cut `dt` hardest exactly when accuracy was best.**

Paired with the 1.5 growth cap, that asymmetry is a ratchet. The captured trace
shows it plainly — grow ×1.5 three or four times, take one ÷5, envelope walking
down — until `dt` reaches `dt_min` and the run advances 1e-15 s per iteration,
recording a snapshot each time. Hence the memory growth.

**Zero rejections, zero `dt_min` floor hits** throughout: the accept gate was
perfectly happy (`milne ≈ 1e-6`, threshold `trtol = 7`). Nothing in the
existing statistics could have flagged this — which is why it needed a stack
sample and hand-rolled instrumentation to find.

## Fix

Two changes, both in `analyses/convergence.rs`:

- bound the relative change: `de.clamp(-1.0, 1.0)` — the integral term stays a
  correction instead of a sign flip;
- floor the exponent: `p.max(0.0)` — "better error" can never mean "smaller
  step". With default gains `p` now lands in `[0.3, 1.1]`.

Plus a termination guarantee in `analyses/transient.rs`:
`TransientAnalysisOptions::max_steps` (default 2_000_000) turns a
non-terminating run into a loud, diagnostic error naming the time reached, the
collapsed `dt`, and the accept/reject/floor-hit counts. `solve()`'s
`while current_time < stop_time` had **no** termination bound — a step accepted
at the `dt_min` floor still advances time, so the loop was formally productive
and practically infinite. That error message is what turned a mystery hang into
a five-minute diagnosis, and it is worth keeping for the next one.

## One thing tried and reverted

Normalizing `milne` by `trtol` before handing it to the controller, so its
unity point matches the gate's threshold. It looks like an inconsistency
(controller targets 1, gate rejects at 7) and it is **not**: the gap is a
deliberate safety band — the controller aims well inside the accept region so
rejections stay rare. Closing it coarsened the grid enough to break
`tran_rc`'s ngspice parity at the steep part of the charging curve. Reverted,
with a note at the site so the next reader does not re-try it.

## Result

- The circuit that never finished: **500 µs in 0.06 s.**
- `cargo test --workspace`: **1841 passed, 0 failed, 0 warnings.**
- One pinned baseline moved and was **re-pinned on evidence, not convenience**:
  `parity_rc_transient`'s captured `v_last` shifted by 4.8e-7. Driving that
  same circuit onto a converged grid (`dt_max` swept to 1e-8, ~100k steps)
  gives `−0.450_456_791_8`, stable to nine digits. Against it the old baseline
  was off by 2.84e-5 and the new one by 2.89e-5 — both dominated by the coarse
  ~386-step adaptive grid, their difference 60× smaller than the error they
  share. Accuracy-neutral. The old number was the captured one, never the
  accurate one.
- Recorded steps for that fixture: **386 → 161**. The old law paid 2.4× the
  steps for the same accuracy, so this is a general speedup, not just a hang fix.

## Open follow-up

`dt_min_seen` now reports values *below* the configured `dt_min` (5e-21 against
a 1e-15 floor) on the buck fixture. A breakpoint landing can legitimately
shorten a step, but 5e-21 is a breakpoint essentially at the current time.
Harmless where observed (the runs complete) and not chased.

---

# Tier B — Fail-loud holes

Honest errors; a spec-declared construct is unusable. Listed for scheduling
weight, not for alarm.

| # | Gap | Evidence | Status |
|---|---|---|---|
| B1 | `for` in a `digital` body | `emit/stmt.rs:98` — *"must be unrolled at elaboration"*; digital bodies are not unrolled | `[tracked]` ROADMAP P6 residue |
| B2 | Selector axes `driver::`/`load::`/`parent::`/`ancestor::` | Parsed (`pom/selector/parse.rs:58-61`), `AxisNotImplemented` at eval (`selector/eval.rs:283`) | `[tracked]` ROADMAP language backlog |
| B3 | `laplace_*` / `zi_*` | No `extern operator`; MD-24 stops them at elaboration | `[tracked]` |
| B4 | `.disto` on MOS2/MOS3 | Cranelift function-count explosion; opt-in, off by default | `[tracked]` P1 backlog |
| B5 | `Complex` has no literal syntax | Part I §6.1 lists it as one of the seven primitives; `value.rs:42` concedes *"POM attribute surface; no literal syntax yet"* — no lexer or interpreter path | **new** |
| B6 | Four analog operators undeclared | `idtmod`, `absdelay`, `table`, `ac_stim` resolve through codegen's registry, no `extern operator` | `[tracked]` Part V §2 + CLAUDE.md |

**B5 is the new one.** The spec's primitive-type table promises seven types and
the `extern_coverage_guard` asserts seven `extern type` declarations exist — but
one of the seven has no way to produce a value in source. Either give `Complex`
a literal form, or demote it in §6.1 to what it actually is: a POM/host-surface
type, not a source-level primitive. The second option is a one-paragraph spec
edit and is probably the right call.

---

# Tier C — Spec holes

## C1 — `Quad` is specified as 4-state, implemented as 8-level

Part I §6.1: *"`Quad` | `0q0` / `0q1` / `0qX` / `0qZ` | 4-state logic, standard
propagation"*. Implementation: `crates/piperine-lang/src/value.rs:44-45` —
*"8-level logic value (backed by a `u8`)"*, and `Display` renders `0q{v}` for
arbitrary `v` (`value.rs:431`).

Either the extra levels (strength modelling?) are real and undocumented, or the
`u8` is over-wide and the spec is right. Nobody reading either artifact can tell
which. Needs a decision recorded, then one of the two edited.

## C2 — Latch inference is specified with a diagnostic that does not exist

Part I §10.3 declares latch inference *"raises a **warning by default**
(deny-able per project or via attribute)"*, and §16's validation table names the
diagnostic `InferredLatch(var)`.

Repo-wide grep for `InferredLatch`: **zero hits.** The behavior itself is
implemented — the digital kernel's module headers state the rule
(`kernel/digital/{abi,layout,compile}.rs:18`, *"Unassigned-before-read variables
hold their previous value (a latch)"*) — so accidental storage is created
silently, which is precisely the failure mode the warning exists to catch. The
"deny-able per project or via attribute" mechanism does not exist either.

This is a spec hole *and* an implementation hole; filed under C because the spec
also never says *how* the deny is spelled.

## C3 — Part VII §1 disclaims three shipped analyses

Part VII §1's analysis-entry row: pole-zero, distortion, and S-parameters are
*"host-facing analyses whose numerical contracts this Part does not yet specify
section-by-section"*. All three ship (`analyses/{pz,disto,sp}.rs`), all three
are on `Session` (`session/entry.rs:666,693,727`), all three have root test
targets (`tests/{pz,disto,sp}.rs`).

Three delivered analyses with no normative contract. Everything else in the
family (§9 DC, §10 tran, §11 AC, §12 noise, §13 TF, §17 sens, §18 PSS) has one.

## C4 — `mkdocs-spec.yml` and `mkdocs.yml` disagree, and both miss Parts IX/X

- `mkdocs.yml` nav: Parts I, II, III, IV, V, VI, VII, VIII + App A, B, C.
- `mkdocs-spec.yml` nav: Parts I–VII + App A, B — **no VIII, no C**.
- **Neither** carries **Part IX (Runtime Semantics)** or **Part X (Codegen
  Pipeline)** — two of the largest and newest documents in the set
  (897 + 656 lines).

Note this also **supersedes `SPEC_DEVIATIONS.md` §4.1**, which still records
Part VIII + Appendix C as unpublished; `mkdocs.yml` has since been wired. That
audit is one revision stale on this point.

---

# Tier D — Unvalidated

## D1 — The ngspice parity matrix is mostly `pending` off the DC diagonal

From `ROADMAP.md`'s own measured table (which is the honest source of truth, and
worth reading as a *risk register*, not a status board):

- **AC:** validated for `res`, `cap`, `urc` only. `ind`, `sw`, `csw`, `dio`,
  `jfet`, `bjt`, `mos1`, `mos2`, `mos3`, `tline` — all `pending`.
- **Transient:** `res`, `cap`, `dio`, `mos1` only. Nine models `pending`.
- **Temperature:** `dio`, `bjt` only.
- **`ind` and `tline`: `pending` in every single column.** Two shipped device
  models with zero cross-validation of any kind. `tline` has only an in-process
  test (`tests/tline.rs`), never checked against ngspice.

`[tracked]` in the sense that the table exists and is honest. Re-rated here
because *the shape of the hole is a diagonal*: DC op-point is well covered,
everything else is thin, and the two fully-uncovered models are not flagged as
such anywhere a reader would notice.

## D2 — Seven Part VII §16 failure rules unenforced

`[tracked]` — Part VII §16, `SPEC_DEVIATIONS.md` §2, ROADMAP P6 residue, and
`crates/piperine-solver/tests/spec_failure_rules_guard.rs` all agree.

Audit's only addition: of the seven, **§2 (an element declaring no capability
is silently inert)** is the one that belongs in Tier A on merit, not here — it
is a silent no-op, and the stated reason for not enforcing it is that two test
fixtures depend on the silence (`live_params.rs`, `solver_entry.rs`). Fixing the
fixtures is a smaller job than the rule's risk warrants leaving open.

## D3 — No sequential-digital example

Covered under A3. Restated as a coverage fact: `tests/run_examples.rs` is the
gate over `examples/`, and the corpus it gates contains no `match`, no enum
state variable, and no FSM. `examples/21_shift_register_tran.phdl` is the only
`posedge` user.

---

# Tier E — Docs / infrastructure

| # | Item | Detail |
|---|---|---|
| E1 | `CLAUDE.md` stale on `piperine-signal` | Says *"codegen keeps its own copies until `codegen-simplification` delegates to this crate"*. That feature **shipped** — `tests/delegation_guard.rs` mechanically forbids the second copies across five families. Sentence should be past tense. |
| E2 | `SPEC_DEVIATIONS.md` §4.1 stale | Part VIII + Appendix C are in `mkdocs.yml` now. See C4 for what is actually missing. |
| E3 | `docs/manual/` absent | `docs/` holds `index.md` + `spec/` only. `[tracked]` ROADMAP P6 residue. |
| E4 | Python surface asymmetry | `Waveform` measurements / transforms / `ComplexWaveform` margins are Rust-only — `piperine-python/src` binds none of `fft`/`rise_time`/`slew_rate`/`bandwidth_3db`. `[tracked]` HOST-14/15/16. |
| E5 | `Freq::from(&str)` panics | `From` is infallible; a malformed SI string panics rather than erroring. `[tracked]` SPEC_DEVIATIONS §1.5. One-line fix (`TryFrom`) that keeps getting deferred. |

---

# What the audit did **not** find

Stated explicitly, because a gap report with no negative space is not
trustworthy:

- **Zero** `TODO` / `FIXME` / `XXX` / `HACK` comments in `crates/`, `src/`,
  `tests/` (the single grep hit is a string literal inside a guard test).
- **Zero** `todo!()` / `unimplemented!()` in the entire workspace.
- No undocumented `SPEC_DEVIATION` markers — every one of the 19 in-code markers
  carries its rationale at the site.
- The solver's analysis family, the Element ABI, the plugin security model
  (TOFU + permissions + capability facade, `piperine-plugin/src/{trust,
  capability,manifest}.rs`), and the dv-core/dv-gradients/dv-tester surfaces are
  all present, tested, and consistent with their spec parts.
- `CodegenError` is genuinely one named type with `Unsupported` at every
  unlowered construct — Part X §11's coverage contract holds, with the
  documented exception of the Tier A silent paths above.

The codebase is disciplined. The gaps are concentrated, not diffuse.

**One methodological caveat, added with Tier O.** Every bullet above comes from
*reading* the tree. Tier O came from *running* it, and found four defects in the
first command a user types — none of which any amount of reading had surfaced,
because the audit's own frame was spec-vs-source. The lesson generalizes past
the CLI: this project's guards are excellent at proving internal consistency
(`suite_hygiene`, `delegation_guard`, `extern_coverage_guard`,
`ngspice_parity_matrix`) and have almost nothing to say about whether the
assembled product works from the outside. Worth keeping in mind when reading the
"not found" list above as reassurance.

---

# Suggested order of work

Ranked by (silent-wrongness × reachability) ÷ effort.

| Rank | Item | Why here | Rough size |
|---|---|---|---|
| ✅ | **A0d** — a `digital` block killed the module's analog `@timer` | **FIXED 2026-08-09.** D2A bridge overwrote analog-owned vars; see the entry | done |
| −2 | **A0c** — the A2D *write* bridge (analog body driving a digital net) | Silence fixed (now a loud refusal); the feature Part I §10.4 documents still does not exist | M |
| −1 | **A0** — `I()` in a constraint returns the branch voltage, **plus** the numeric oracle test | A wrong number, silently, in the verification feature. Three shipped SOA requires are affected today | M |
| ✅ | **T0** — transient stepper ratchet (`PiController::propose_dt` not monotone in the error) | **FIXED 2026-08-09.** Was an unbounded hang; see below | done |
| 0 | **O1–O4** — fix the `piperine new` path, guarded by an end-to-end test | The first command a user runs prints an error, and no new project gets a `.venv`. Cheapest item on the list and the most visible | S |
| 0b | **A0b** — make a driven signal-flow net fail loud at build, then decide its fate | A declared, documented discipline pair that cannot appear in a solvable circuit; today it dies as `SymbolicSingular` | S then M |
| 1 | **A2 half 1** — reject unimplemented `resolve` kinds at elaboration | Turning a wrong answer loud is hours | S |
| 2 | **A1** — wire solve-time diagnostics to a host sink | Highest user-visible pain; unblocks every future model-debugging session | M |
| 3 | **A3** — bus-indexed assignment + multi-bit patterns, then add the FSM example | Makes the spec's own §10.3 example compile; unblocks the digital idiom | M |
| 4 | **A4** — validate or reject `enum : Repr` | Small, closes a silent path | S |
| 5 | **D2/§2** — enforce "element declares no capability", fix the two fixtures | A silent no-op misfiled as a coverage gap | S |
| 6 | **C1/C2/C3/B5** — spec reconciliation pass (`Quad` levels, `InferredLatch`, pz/disto/sp contracts, `Complex`'s status) | Cheap; each is a decision + an edit, and they are what a careful reader trips on | S each |
| 7 | **C4/E1/E2** — nav wiring for Parts IX/X, stale-line fixes | Trivial, high signal-to-noise for anyone onboarding | S |
| 8 | **A2 half 2** — real per-net resolution functions | Needs a `Real`-valued digital net; genuinely its own feature | L |
| 9 | **D1** — AC/transient parity fixtures, `ind` and `tline` first | The two zero-coverage models are the exposure | L |

Item 0 is under a day and should go first regardless of what else is scheduled —
it is the difference between a new user's first minute working and not.
Items 1–7 are, in total, roughly one focused cycle, and they retire every
fail-loud violation the audit found. Items 8–9 are feature-sized and deserve
their own `.specs/features/` entries.
