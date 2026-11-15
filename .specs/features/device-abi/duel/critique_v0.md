# Critique of `abi_v0.md`

Round 0: no previous fixes to verify. Items are ranked; each names the section of v0 it attacks
and the MSL rule it breaks. 3 BLOCKER, 14 MAJOR, 7 MINOR.

---

## BLOCKER

### B1. R2 ("reactions run only at instants that will not be undone") is unsound against the transient sequence of §11

R2 is the load-bearing wall of the whole design (it is why `mem` and foreign engines are safe, why
there is no checkpoint entry point). The transient sequence contradicts it.

```
t = 0, tentative t_c = 10 ns.         Comparator: cross(v(x)) -> en <- false
step 2-3: analog solves [0,10ns]; e(10ns) <= 0, no crossing seen (e rose and fell, or stays low)
step 4:   owner runs the digital instants in (0,10ns), IRREVOCABLY (R2):
            6 ns: Counter reacts on clk, reads en == true, commits count+1     <- ran
            8 ns: a reaction commits `code` (a DAC input, in analog_reads) -> 8 ns is a time point
          analog "retakes to it (pure work discarded)": new solve [0,8ns], different solution,
          A(0) at 8 ns now shows e > 0  -> crossing located at 5 ns
          5 ns: `en <- false` must apply BEFORE the 6 ns instant (I3). The 6 ns reaction already
          ran with en == true and cannot be undone. Same if the retake to 8 ns fails LTE and
          a crossing appears in the shorter step.
```

Any retake after step 4 can discover an analog→digital event earlier than digital instants
already executed. Either reactions must be undoable (spec §15 I8 resolution: speculation is
allowed "provided a rejected step undoes them"), or the owner may never run a digital instant
later than the last *accepted* analog time — and then it cannot know, before running the
reactions of (t, t_c), whether one of them creates a time point (the 8 ns commit is dynamic).
v0 picks neither. §15 "Known weak point 1" names the tension but the spec text pretends it is
solved ("a rejected analog step discards only pure work").

Required: state the synchronization protocol exactly. The cheap honest answer is already in the
design: **`mem` is solver-owned, so every MSL/pure device is rollbackable for free**; the owner
snapshots (pins, queue, mem) per analog step and may speculate. The only devices that need "never
undone" are foreign engines — and for them the guarantee must be a *declared* property
(`Reaction` flag `IRREVERSIBLE`) that makes the owner conservative only around those reactions
(analog accepted up to `t` before the reaction at `t` runs; any commit it makes that can reach
`analog_reads` must have `after > 0`, checked). That is one flag and one rule, not a hidden
global assumption.

### B2. The AVR emulator (§12.7) has no time base of its own; the example is wrong three ways

The brief's flagship external device: "advances to a time, raises events/interrupts".

```
AVR, pin-change interrupt enabled on pb3 (driven by the comparator of 12.5).
react "run" at tick T: cpu.run(now) "runs ahead to the next instruction that reads a pin or sleeps"
   -> cpu state in `mem` is now at T + 4000 cycles; outputs committed with after = dt.
tick T + 100 cycles: pb3_in changes (interrupt).
   react "run" is called. `mem` holds the cpu 3900 cycles in the FUTURE. There is no older mem.
   The interrupt is taken 3900 cycles late, or the emulator must never run ahead.
```

1. **No horizon.** With asynchronous inputs every instruction "reads a pin", so the only correct
   emulator is one `wake` per instruction: 16 M events/s through the owner's queue. A device with
   its own time base needs the solver to tell it how far it may safely run: the earliest instant
   at which any pin in its `on` list *can* change (the solver knows: it owns the queue and the
   analog accepted time). v0's `Env` has nothing of the kind. This is the single datum that makes
   "advance to a time" exist; without it the device is a callback on a timer.
2. **Every wake is an analog time point.** R4: a time is a time point when "a reaction is about to
   run … whose `writes` can reach (statically) an `analog_reads` pin". `run` writes `pb*_out`;
   any MCU output that drives a D2A/LED/gate reaches analog. So every wake lands the analog
   solver. The table's claim "only ADC samples are analog time points, not every wake" is false
   for every real board. The shape cannot say "this reaction writes only with `after > 0`", which
   is exactly what would make the *due* instant, not the wake, the time point.
3. **It never starts.** `on:[pb*_in, reset, wake, adc_val]`. At (0,0) only pins that receive a
   first commit change. If `reset` is undriven or the host binds it before the run, nothing fires,
   `wake` is never armed, the CPU never executes. The fix (a private discrete pin with `init`)
   is an idiom the ABI does not state; the same idiom is needed for `at(param)` (a param does not
   change at (0,0), §13 rule 16). A design whose worked example depends on an unstated trick to
   run at all is underspecified.

Also unstated: the tick resolution. The emulator uses `env.tick` and commits `after: f64`
seconds; converting cycles to ticks needs the session's resolution, which is nowhere in the ABI.

### B3. A compiled OSDI-like model cannot be coupled without breaking purity or losing live params

Brief: "a compiled OSDI-like analog model". OSDI 0.4 (normative for `extern "osdi"`, §8.5.8):

```
bsim4.osdi:  setup_model(params) ; setup_instance(params, temperature) -> writes instance data,
             decides node collapse (rs == 0 -> d/di merged) ; eval() reads+writes instance data ;
             ~900 params, none marked "structural".
```

- Node collapse depends on param *values* → the `Shape` depends on them → every OSDI param must
  be a `create` arg (v0: "`args` are what fixes structure") → none is a `Param` pin → a live
  `set` is destroy/create/rebind, `p.changed()` never fires, and `W_REAL` sensitivities
  (`sens`, `adjoint` over a BSIM `vth0`) do not exist. Or they are `Param` pins and the shape
  silently lies about the collapse.
- `setup_instance` is thousands of flops. v0 has no setup tier ("`analog` may memoize on its
  inputs") but the instance is `*const`, immutable, `Sync`, and A5 forbids remembered state.
  Memoization means interior mutability plus comparing 900 param byte-strings on every Newton
  iteration. "Known weak point 2" admits it.
- The in-design answer — precompute in a **definition** writing a private blob pin that `analog`
  reads — works for cost but kills gradients ("weak point 5": nothing is differentiated through
  a definition). So v0 offers two mechanisms for derived params, each broken for one requirement.

Required: one mechanism for "values derived from params/temperature, recomputed when they
change, differentiable" — the definition is the right one; give it the same partial table as
`analog` (see M9, M10). And a rule for value-dependent structure: a `Param` pin flag
`STRUCTURAL` ("a change of this pin invalidates the shape: re-create") instead of the Arg/Pin
split (M7).

---

## MAJOR

### M1. "Events are definitions" (D3) is a convention; §12.4 violates it; the set of time points becomes author-dependent

D3: `clk.rise()` is a definition firing a private event pin, "which is what makes §17.2.2 a
question the solver answers without running the reaction". The flip-flop of §12.4 does the
opposite: `on:[clk]` and tests the edge inside `react`. Both encodings are legal. Now take
`SampleHold` (`on clk.rise() { held <- v(vin) }`): encoded as 12.4 with `probes:[vin]`, the
*falling* edge is a time point; encoded per D3 it is not. The spec says that set is exact and
observable ("`settle` observes it", §17.2 resolution). The ABI lets two correct-looking devices
differ in observable behavior and gives the solver no way to tell. Either reactions listen to
event pins **only** (then `on` of a discrete pin is illegal, one rule), or drop D3.

### M2. Resolved carriers across instances: "one definition after elaboration" cannot exist

§1 "not a concept" and D1 say MSL's legal loops among resolved drives become one definition.

```
module A { Tran(w1, w2, en) }   module B { Tran(w2, w3, en) }   module C { Tran(w3, w1, en) }
three instances, three compiled crates (O4), a loop of resolved drives through w1,w2,w3.
```

A definition belongs to one instance (`define(instance, k)`). A cross-instance SCC is one
definition only if elaboration fuses instances — impossible with per-module `cdylib`s, and
impossible when one participant is external (an I2C slave emulator on a `Wire`). Left as a
solver-visible cycle, D1 relaxes "to a bytewise fixed point or fails 501", which is **not**
§8.2.3 (relax from empty, drives+1 passes, *unknown* on failure, no error). So either the solver
knows the MSL relaxation rule (a language rule in the solver) or the result is wrong. Also
unspecified: which device contains the resolver, who orders drives "in instance-path order"
(§4 rule 2), and how an external device declares that its output is a *drive* of a resolved net
rather than a one-writer pin.

### M3. Typed bytes: the ABI imports the language's layout by reference, in five different spellings

§3.2: "Every other tag is the MSL type name and its layout is MSL Part I §2". That is the
language leaking into the ABI by citation. Concretely:

```
AVR (C, external): pb0_out  tag "bit"  (0 unknown, 1 low, 2 high)
MSL:  module Led(in a: Bit)            tag "Bit" or "Bit<bool>", layout = Repr (mask + value?)
bind: "Two pins on one net must agree in size and tag" -> bind error, or UB if the strings match
      and the layouts do not.
```

Whether `"bit"` is MSL's `Bit` is not said. An external author must read the MSL `Repr` chapter
to connect one wire. Type identity by string equality is unsound for user types
(`Bit<Phase>` from two libraries). And "typed bytes" is spelled five ways: `Arg{tag,default}`,
`ArgValue{tag,value}`, `Pin{size,align,tag,init}`, `Site{size,fields}` + `Field{tag,offset,size}`,
`mem_size/mem_align`. Records are self-describing (fields); pins — the thing the host must
display (§21) — are not. One `Layout { size, align, fields: List<Field> }` with a closed set of
scalar leaf tags and a stated "known" encoding per leaf replaces all five and removes the
reference to Part I.

### M4. Two channels for the ambient, with a rule the document itself breaks

Concept 10: Env is "what the solver itself produces … nothing the host provides is in it".
Then: `analysis` (host's choice) is in Env; `gmin`/`source_scale` (produced by the analog
sub-solver, changed *between solves of one instant*) are `Global` pins. A `Global` pin is a
discrete pin, and a change of a discrete pin that `analog` reads is a boundary event, a time
point and an integrator restart (R4, step 5) — except these, "in the analog sub-solver's copy".
That is a special case inside the one-mechanism claim. Device authors do not care who produces
`temperature` versus `time`; they see two APIs (`cx.env().time`, `cx.f64(TEMP)`).

Cost of the split: the solver cannot know which Env fields a device reads, so in `Ac`/`Noise` it
must call **every** instance at **every** frequency although only flicker sources read `freq`;
and purity (A5) is unverifiable for Env. Pick one: (a) everything ambient is a pin the solver
writes (time, freq, analysis, static included; declared in `analog_reads` → the solver skips
devices whose inputs did not change; concept 10 disappears), with one stated rule for pins that
vary inside an instant; or (b) everything ambient is Env and role `Global` disappears.

### M5. "Slot" is four concepts under one name

| Role | Kinds | What `v`/`a`/`b`/`plus`/`minus` mean |
| --- | --- | --- |
| a term of an equation row | `Add`, `Ddt`, `Delay` | value / abstol or td / max; rows |
| an *input* to differentiate against | `Noise`, `Stim` | psd or mag / phase; no target; appears as `Partial.wrt` |
| solver-kept iteration state | `Limit` | limited / proposed; read back through `Call.last` |
| a monitor read at accepted solutions | `Cross`, `Cap`, `Var` | e, ttol, etol / dt / value; `plus` is an *event pin* |

Nine kinds, three untyped scalars whose meaning is a table lookup, `plus` that is a row or an
event pin, and slots that are outputs except when they are inputs. Sentence 3 of §0 says the
solver "knows three linear operators (1, d/dt, delay)" — then the concept is *term* with three
operators, and the other six are something else. Static arguments (`abstol`, `max`, defaults of
`ttol`/`etol`) travel through `a`/`b` on every Newton iteration and are validated each time.
A2 (switch branches, `418`) is not an ABI rule at all: the device chooses and the device faults;
delete it. MSL's concepts each have one rule; this one has nine rows.

### M6. Limiting: the Newton right-hand-side correction is pushed into every device, and it breaks the "free" verifier

A4 makes each device report `v(x̂) + ∂v/∂x·(x − x̂)` for *every* slot downstream of a limited
value (§12.2 does it by hand for `ID` and `QJ`; forgetting one is a silent convergence bug).
The solver's own Newton bookkeeping (§18 rule 4, OSDI `lim_rhs`) is now model code. Consequences:

- The FD check of §9/§13 ("perturb each input, compare with `jac`") gives false alarms whenever
  a limiter is active: `∂(reported v)/∂x` is not `jac` because `x̂ = f(x, last)` moves with `x`.
  The verifier is only valid where no `Limit` is active — unstated.
- `Env.iter` exists only so the device can pick its seed. MSL has no `iter()`; a Newton counter
  is solver-internal state exposed to "pure" code. A flag "no previous value: use your seed"
  (or `last = NaN` under a stated rule) removes it.
- §14.7.4 "an instance not read in an iteration keeps its state" — v0 says `a = NaN` means not
  read, but the pre-set is `v = 0`; whether `last` then becomes 0 or is kept is not stated.
- Consistency: `Noise`/`Stim` are handled as "differentiate against the slot" (`Wrt::Source`).
  The same device gives limiting with no device arithmetic: `Partial{of: Limit}` =
  `∂slot/∂x̂`, and the solver applies `(x − x̂)`. One pattern instead of two.

### M7. One language concept (`param`) became three ABI things in two namespaces

`Arg` (structural, bytes at `create`), `Pin{role: Param}` (live), `Pin{role: Global}` (session).
The host's `set("x1.r", …)` must know which kind it is to choose between "commit" and
"destroy/create/rebind"; introspection lists params from two tables (`Module.args`, `Shape.pins`);
a structural param that is also read as a value (`rs: Option<f64>`) appears in which? A
`STRUCTURAL` flag on a `Param` pin and `create(initial values of all Param pins)` gives one
namespace, one introspection path, and solves B3's collapse case.

### M8. `Shape` per instance, with strings

`create` returns a `Shape` per instance; `Pin` is ~130 bytes (3 `Str`, `Bytes`, 2 `f64`, …).
10 k BSIM instances × 900 params = 9 M pins ≈ 1.2 GB of shape, plus 9 M host-written private
param nets each with `now`/`prev`/`changed` storage, plus three per-instance pointer arrays
(`now`, `prev`, `out`). There is no model-card concept (one `.model` shared by 10 k instances)
except "bind param nets by name", which nothing in §3.1 makes the default. Shapes must be
shareable by contract (one shape per (module, structural values), instances reference it), and
the per-instance part reduced to the binding.

### M9. Gradients: the coverage table claims what the text disclaims

§14 lists `sens/adjoint` as covered; §15 item 5 says gradients through definitions and across
events "are not carried". §23.3 is spec: 4a (`held <- v(vin)` carries `∂x/∂p + ẋ·dt*/dp`), 4c
(definitions and real `resolve` carry it). In v0: a reaction has no derivative output; a
definition has none; `Wrt::Real` is restricted to tag `"f64"`, which excludes every MSL real
carrier (`Bit<f64>`), so 4c cannot even be declared. And `want = W_REAL` is all-or-nothing: the
host asks `∂/∂w` of one transistor and every instance computes partials against all its real
pins (27 k `Partial` entries and a 216 KB `jac` per BSIM instance; a dual number with 900
tangents in generated code). Needed: a partial table on every pure function (analog *and*
definition), and a `want` that selects inputs, not classes.

### M10. `analog` and `define` are the same thing described twice, asymmetrically

Both are pure functions of declared inputs, callable speculatively any number of times. One
declares reads implicitly (all continuous pins + `analog_reads`) and has a partial table; the
other declares `reads`/`writes` and has none. One is a single function per instance; the other
is indexed by `k`. One writes "slots", the other "pins". "Symmetric analog/digital treatment" is
a scoring criterion and this is the place it fails: a `Var` slot is a definition of a real
value from continuous inputs; a derived param is a definition `analog` reads. A single concept
— *a pure function with declared reads, writes and partials* — covers both, gives definitions
their gradients (M9, B3), and takes an entry point away.

### M11. UB at the boundary that the "closed" list does not close

- `SlotKind`, `PinKind`, `PinRole`, `Wrt`, `Analysis` are Rust `enum`s inside `#[repr(C)]`
  structs. "An unknown `SlotKind` from a newer device is a bind error" — no: reading an
  out-of-range discriminant into a Rust enum is UB *before* validation runs. Same for an old
  Rust device receiving a newer `Analysis` in `Env`. They must be `u32` newtypes.
- `pub enum Instance {}`: an uninhabited type behind a pointer; any `&Instance` in a thunk is
  UB. Use an opaque zero-sized struct.
- `cx.mem::<T>()` over zeroed, solver-owned bytes: UB for any `T` for which all-zero is not
  valid (`bool` is fine, an `enum` without a 0 variant, `NonZero*`, references, `Box` are not).
  The "safe lens" needs a stated bound (plain-old-data, zero-valid), and `mem_align` must be
  honored on every copy the solver makes.
- `cx.f64(pin)` on an absent `OPTIONAL` pin is a null read in the safe lens; it must return
  `Option`.

### M12. A6 makes the document's own flicker source fault at DC

A6: "a non-finite `v` or `jac` is `507`". §12.2: flicker is "a `Noise` slot whose `v` divides
by `cx.env().freq`". In every `Dc`/`Tran` call `freq == 0`, `v = inf`, fault 507. §14.8.3 says
`psd` is *not evaluated* outside `Noise`. Either the device must guard on `env.analysis`
(an unstated obligation on every noise source) or the non-finite rule applies only to slots the
solve reads. As written, the rule and the example contradict each other.

### M13. Host actions do not fit "the host is the writer of every pin no module writes"

- `force`/`release` (§23.1.3) overrides a carrier that *has* a writer: "a `force` is a commit by
  the host on nets" is a second writer (bind error `220` by §3.1), and "the drives go on
  updating underneath and show again at the release" needs an overlay the pin model lacks.
- `force` on a node is a host potential source (a time point and a jump): which device?
- A `where` is "a definition of a `bool` pin" validated on a shadow (D2), but nothing in `Shape`
  marks a pin as the constraint of a given param; the solver cannot find it.

### M14. Determinism holes around `mem` and `emit`

- All reactions of an instance share one `mem`. Two of them firing in one microstep observe each
  other through it, so §12.1.16 ("order not observable") is false and I1 needs a stated order
  (index order?). Not stated.
- `analog` "over instances may be parallel" and `emit` is "delivered at once": record order
  across instances is then a race. The sink must be per-instance buffered and merged in a
  stated order.
- Concept 9 says the solver "drops what a rejected step wrote from a reaction"; §7 says records
  from reactions "are never undone, by R2". One of the two is wrong (and B1 decides which).

---

## MINOR

- **m1. In-band sentinels everywhere**: `iter == u32::MAX` = accepted solution, `tick == i64::MIN`
  = not a tick, `a = NaN` = not read, `Cap.v = +inf` = no call, `init` empty = none, `NONE =
  u32::MAX`, zeroed `mem` = "please reset". Flags and options exist; use them.
- **m2. Redundant sources of truth**: `Shape.ports` *and* `PinRole::Port`; `Pin.flags & WRITES`
  *and* `Definition.writes`/`Reaction.writes`; two ways to fire an event pin (a definition's
  1-byte buffer, a commit with `len = 0`).
- **m3. `Call` is a union of three frames** ("unused members are null"): eleven nullable
  pointers, of which each function may touch a different subset. In C that is a null dereference
  waiting; it is "one struct" only on paper.
- **m4. MSL error numbers in the ABI** (`418`, `435`, `453`, `454`, `456`, `501`, `507`, `509`,
  `510`): the language's code space is the ABI's fault space. And `lane` values 100/101/102 in
  §12.7 are hand-picked magic; nothing checks uniqueness.
- **m5. No provenance**: a slot/definition/reaction/pin has a `name` and nothing else. "NaN in
  slot 17 of x1.m3" for an anonymous `+>`. The user left Cranelift over debuggability: add an
  `origin` (file:line:col) on every shape entry; `Var` should not be `f64`-only.
- **m6. `want` cannot select slots**: the post-solve `A(0)` pass (Cross/Cap/Var) and the
  crossing search re-evaluate every contribution of every instance to read three numbers.
- **m7. Digital cost model**: one indirect call per definition per delta through pointer
  arrays indexed by pin, events doubling it (definition → event pin → reaction). "Verilator-
  style levelized DAG" with no fusion story across the ABI; state whether a library may export
  a fused block.

---

## What to delete or merge (downward pressure)

| Now | Proposal | Effect |
| --- | --- | --- |
| `analog`, `define` + Slot-as-output, Definition | one *pure function* concept with reads/writes/partials (M10) | −1 concept, −1 entry point, fixes B3/M9 |
| Env + role `Global` | one of them (M4) | −1 concept or −1 role |
| `Arg` + `Param` | `Param` with `STRUCTURAL` flag (M7) | −1 struct pair, one namespace |
| five typed-bytes spellings | `Layout` (M3) | −4 structs, no reference to Part I |
| 9 `SlotKind` | term (3 operators) / source / watch; `Limit` as a differentiation input (M5, M6) | 9 → ~5, `a`/`b` disappear |
| A2, `Env.iter`, `Shape.ports`, `WRITES` | delete (M5, M6, m2) | — |
| R2 as a global promise | `IRREVERSIBLE` reaction flag + horizon (B1, B2) | makes the AVR real |

---

## Score

**Honest concept count: 16** (v0 claims 10). Library/Module, Arg, Shape, Pin (3 kinds × 4 roles),
Tag/layout, Slot (9 kinds), Partial (3 classes), Definition, Reaction, Commit, Lane, Mem,
Record/Site, Env, Global-name registry, Fault. Plus 19 lettered rules (A1–A7, D1–D3, R1–R4,
C1–C5) and 9 slot rows; MSL has 11 concepts with one rule each.

**Entry-point count: 8** (as claimed: 1 symbol, `create`, `destroy`, `analog`, `define`, `react`,
`commit`, `emit`). Reachable target: 6–7.

**Elegance: 5.5 / 10.**

- For: the core move — device = pure functions, all state solver-owned, zero checkpoint entry
  points, one call path for generated/interpreted/external — is the right spine and worth
  keeping. Noise/stimulus as differentiation inputs is a real unification.
- Against: the spine rests on R2, which is unsound as sequenced (B1); the two external devices
  the brief names (AVR, OSDI-like) do not actually couple (B2, B3); "everything is a pin" is
  claimed and then contradicted by Env, slots, mem, args; "Slot" hides nine special cases with
  overloaded fields; analog and digital are described by two unrelated vocabularies (M10);
  the language leaks in by reference (Repr layouts, error numbers, A2, A7); enum-typed FFI
  fields are UB. The count is 10 only because six concepts were left out of the table.
