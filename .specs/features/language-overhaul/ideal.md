# language-overhaul IDEAL — one IR, five concepts, a visible graph

**Status:** surface design sketch, rev 5 (2026-08-09). **Discussion document** —
written to be argued with, not implemented from. `spec.md` comes after this
settles.

**Revision history — the IR got smaller every time, which is the point.**

| rev | what changed |
|---|---|
| 1 | One IR, **eight** concepts — enumerated capabilities and gave each its own mechanism, the exact failure mode this document exists to fix |
| 2 | Cut to **five**. Deleted `struct`, the "crossing op" category, three of four cell kinds, hierarchy, and a third regime for signal-flow |
| 3 | Checked against the tree and against Yosys RTLIL. Corrected three factual errors (§1.7); took RTLIL's chunk model, deleting four more ops |
| 4 | **User answers D1, D2, D4–D7.** The type system collapses to two value classes (§1.1); `V`/`I` are removed from the IR entirely (§1.2); events are shown to cost no new concept (§1.5) |
| 5 | **D3, D8–D12 answered.** Arrays `[T; D]` added, absorbing `cell`'s `depth` modifier (§1.1); `flatten/` disappears rather than becoming a pass (§3); `piperine-signal` *becomes* `piperine-ir` (§1.6); internal-unknown accounting made explicit (§1.4); a solver analog/digital split proposed as step 5 and flagged as a scope amendment (§6.1) |

**Scope, fixed by the user (2026-08-09):**

- **Solver and host stay.** `Element` + `ElementCapabilities` is the frontier.
  The ngspice parity matrix is a paid-for asset and is not re-earned.
- **Language and codegen are redone / adapted.** Everything between the POM and
  the `Element` ABI is in play.
- **PHDL is modified, not replaced.** `headers/spice/` must survive with
  bounded edits.
- **Processor bar = (a): expressible and simulates correctly.** Not synthesis.

---

## 0. The thesis

Five defects surfaced in two days of building one small mixed-signal circuit
(`GAPS.md`: T0, A0, A0b, A0c, A0d). Every one lives **between** two stages,
never inside one:

| defect | seam |
|---|---|
| T0 | step controller ↔ accept gate — two targets for one quantity |
| A0 | constraint expression ↔ analog emitter — the two natures collapsed |
| A0b | storage-`Real` discipline ↔ MNA stamping — a node with no row |
| A0c | analog force ↔ digital net — destination fell to a catch-all |
| A0d | analog var bank ↔ digital var bank — ownership undefined |

1841 tests missed them because each test exercises **one stage against a
hand-built fixture** (`digital_jit.rs` constructs `LoweredBody` by hand). A test
shaped like that can never disagree with itself.

The structural cause: **there is no IR.** Four partial, overlapping forms:

| representation | covers |
|---|---|
| POM (`Design`/`Module`) | authored structure |
| `resolve` | analog *expressions* only, interned ids, symbolic diff |
| `flatten` (`FlatAnalog`) | analog bodies — temp tape + contributions |
| *(none)* | **digital bodies** — raw AST straight to Cranelift |
| `piperine-signal` | runtime semantics, a fourth parallel model |

`LoweredBody` carries two of these in one struct. A0d is exactly that: the
digital side held the module's `VarDecl` in its statement list while the analog
side held a flattened tape, and the two shared a var namespace with no declared
owner.

**The thesis: one IR, owned end to end, printable at every pass.** Seams stop
being where invented rules disagree and become where one rule is checked.

Three of the user's complaints are one complaint:

- *"codegen is a black box"* → the IR **is** the intermediate language
- *"I lost traceability of my own code"* → `--emit=ir-after-<pass>`, diffable
- *"the device DAG is built obscurely"* → the DAG is IR, so it prints

---

## 1. The IR — five concepts

**Design rule, from the user (rev 4):** *as simple as a C IR or Yosys RTLIL.*
The minimum here is larger than C's — analog needs the whole computation that
produces a stamp value, digital needs two cycle recipes — but the *shape* is
the same: **values, ops, blocks, and explicit state.** Nothing more.

### 1.1 Value classes — two, and effectively untyped

The IR runs **after** type checking. What survives is not a type system:

```
real        -- every analog value; also the digital `var acc = 0.0` idiom
bits<N>     -- every digital logic value; N is a width, not a type
[T; D]      -- an array of D elements of T
```

That is the whole universe. Everything rev 1–3 proposed collapses into it:

| rev 3 had | rev 4 |
|---|---|
| `int<N>` | `bits<N>`; signed/unsigned is a **property of the op**, not the value (RTLIL does exactly this) |
| `logic4` | not a type — the *encoding* of `bits` (§1.6) |
| `vec<T,N>` | gone. A digital value **is** a vector of width N |
| `bool` | `bits<1>` |
| `struct` | gone at elaboration/lowering (§1.7 M3) |

The completeness win survives untouched, because it never depended on a type
system — it depended on **width**. `Bit[8]` stops being eight independent
scalar nets (`DigitalNet` indexing an array of `LogicValue`, digital universe
literally `enum DigTy { Int, Real, Quad }`, `emit/resolver.rs:71`) and becomes
one value of width 8. That is what removes 17 hand-declared wires from a 4-bit
adder. A number does it; a type lattice was never needed.

**Signal references are chunk lists, not ops** (RTLIL `SigSpec`). A reference is
a list of chunks, each a slice of a value or a constant. `x[31:25]`,
`concat(a,b)`, `{4'b0, y}` are **representation, not computation** — no
`slice`/`concat`/`extract`/`insert` ops at all, and bit rewiring is free.

**Arrays and width are orthogonal — they compose, they do not compete** (D11,
user). Collapsing a digital bus into "an array of bits" would make a 32-bit
`add` into 32 ops, which is the exact cost this design exists to remove. So:

| form | is | buys |
|---|---|---|
| `bits<N>` | **one** machine value, N bits; arithmetic is one op | the 4-bit adder stops costing 17 wires |
| `[T; D]` | D elements of T, indexed | memories and analog buses |

`[bits<32>; 1024]` is a register file. `[real; 8]` is an analog bus. One
concept covers both, which is why D11 closed by *adding* the aggregate rather
than by adding a width to `real`.

**Consequence: `cell` loses its `depth` modifier** — depth lives in the type
(§1.4). One special form fewer.

**Not in the IR: arrays of _nets_.** `wire x : Electrical[8]` is expanded into
individual nets by elaboration (the `gap-3` array-net expansion, already
landed). Nets stay scalar below the POM.

### 1.2 Net — across and through, never `V` and `I`

```
node    -- conservative. Two natures: one ACROSS, one THROUGH.
           THROUGH sums to zero at the node — that is KCL.
signal  -- discrete. Carries a value. No conservation law, no KCL row.
```

**The IR does not know what a volt is.** Rev 3 wrote "carries a potential and a
flow" and anchored on electrical names; that is wrong and the user caught it.
`Thermal` is `temp`/`pwr`, `Magnetic` is `mmf`/`phi`, `Rotational` is
`theta`/`tau`. An IR that knows `V` and `I` breaks on every non-electrical
discipline — and `headers/spice/`, which must survive, already uses thermal
nodes.

What is universal is the **structure**, not the names: a conservative discipline
declares exactly one across nature (measured between two nodes) and one through
nature (summed to zero at a node). `V`/`I` are POM-level metadata that never
descend. The IR ops are `read_across`, `read_through`, `contribute_through`,
`force_across` — nature-generic.

This is also a better reason why **A0** cannot recur than rev 3 gave. Rev 3
said "two ops with different result types". Truer: across and through are
**different natures of a discipline**. Collapsing them is not a dispatch bug, it
is a category error — and `emit_analog_branch` serving `"V" | "I"` from one
`branch_voltages` lookup is exactly that category error made executable.

`signal` has no conservation law **by construction**, which is what makes
**A0b** unrepresentable: the analog backend is never handed one to stamp, so a
driven signal-flow net cannot produce `SymbolicSingular`.

### 1.3 Value graph

SSA over pure ops: arithmetic, comparison, `select`, explicit conversions, and
calls into the **intrinsic catalog** (§1.6). Signedness, width behaviour, and
saturation are op properties.

Every `real` subgraph is **differentiable by construction**. Symbolic
differentiation — `resolve/diff.rs` today, and the asset with no equivalent in
competing tools at this position in the stack — becomes an **IR → IR pass**:
given a value and a set of independents, emit the derivative graph. One pass
serves the analog Jacobian, `.disto`'s higher derivatives, and dv-gradients'
`∂F/∂p`, which are three separate code paths today.

*Optimization is explicitly out of scope for v1* (user, rev 4): the IR must
**permit** passes — dead-op elimination, CSE, cone extraction — and ship none
beyond what the backends already need.

### 1.4 Cell — the only state

```
cell <name> : <type>                      -- one declared writer, one init
                                          -- <type> is real | bits<N> | [T; D]
```

A scalar-typed cell is a register; an array-typed cell (`[T; D]`) is a memory,
and passes reason about it as a memory — port count, read/write conflict, address aliasing — never scalarizing
by default. Direct answer to *"our array is not represented as an array nor
optimized as one"*.

Memory **access is separate ops, not ports baked into the declaration** —
RTLIL splits `$memrd`/`$memwr`/`$meminit` from the `Memory` object precisely so
port sharing can be optimized before anything merges. Same reasoning.

**Params are cells** whose single writer is the host. `ParamId`/`ParamInfo` and
the per-instance param bank are first-class today, and MD-18 (restamp a compiled
circuit, never re-JIT) depends on a param being a runtime bank entry rather than
a folded constant. Not a sixth concept.

**Not cells:** the `ddt` companion history and the state behind
`idt`/`transition`/`delay`/`slew`. Those are private to their intrinsic,
declared with it, and are backend bookkeeping.

**Internal MNA unknowns are counted, not improvised.** The `Element` ABI already
has the seam — `allocate_unknowns(&mut self, alloc: &mut UnknownAllocator)`,
called once per element in insertion order *before the matrix shape freezes*,
with `HAS_INTERNAL_UNKNOWNS` declared by whoever takes it. What is folklore
today is the distinction between two different things:

| | origin | when |
|---|---|---|
| **module-internal wire** (the buck's `sw`) | hierarchy flattening — becomes a real circuit node | POM |
| **device-internal unknown** (a branch current for a force, hidden state) | `allocate_unknowns` | circuit build |

In the IR every `force_across` **requires** a branch unknown, so a block's
unknown budget is *countable* — derived by counting forces, not hand-written per
device. `allocate_unknowns` becomes mechanical, and a class of error nobody
checks today disappears (a device that allocates without declaring the flag, or
declares it without allocating).

**One declared writer** is load-bearing. **A0d** happened because a module-level
var had two claimed owners — the analog var bank and the digital var bank — and
the D2A bridge blitted one over the other. Here that does not typecheck. The
three sub-traps hit while fixing it are the same missing concept said three ways:
slot ≠ ownership, inferred ≠ written, power-on init ≠ runtime write.

### 1.5 Block — and why events cost nothing

```
block { ops… }   domain: analog | digital   trigger: <kind>
```

A block is ops plus a domain plus a trigger.

**Events are triggers, not a mechanism.** The user was unsure these were
representable. They are, and they need no new concept:

| trigger | used by |
|---|---|
| `always` | digital combinational; the analog residual |
| `edge(sig)` | the digital **synchronous** recipe |
| `cross(expr)`, `timer(period)`, `initial` | analog |

The user's *"receita de bolo, uma pro ciclo síncrono e outra pro assíncrono"* is
exactly `trigger: edge(clk)` versus `trigger: always`. The *detection* of a
crossing is an intrinsic returning a fire decision (§1.6), and the breakpoints it
declares are its output.

**The two domains produce different things, and the IR keeps them apart** (user,
rev 4 — *"deixar bem separado esses dois pontos"*):

| | reads | writes |
|---|---|---|
| **analog** block | `read_across(a,b)`, `read_through(a,b)`, `read_signal(s)` | `contribute_through(a,b,x)`, `force_across(a,b,x)` |
| **digital** block | `read_signal(s)`, `read_across(a,b)` *(cross-sampling)* | `drive(s, v)` |

An analog block computes **contributions**; a digital block **drives outputs**.
Cross-sampling is allowed and explicit. What kills **A0c** is not machinery but
the absence of a fall-through: today `flatten/analog.rs::parse_dest` ends in
`Ok((NatureKind::Flow, NodeId::GROUND, NodeId::GROUND))`, so a bare-identifier
destination became a contribution from ground to ground and the authored value
vanished. With named ops and no default arm, the front-end either emits a write
the backend implements or lowering fails loud.

### 1.6 The intrinsic catalog

**D5, resolved (user):** `extern` declarations become **calls into the IR's own
catalog**, which needs expanding. Not `extern impl` capabilities — a catalog.

One entry per intrinsic, declaring: signature, private state shape, derivative
rule (if `real`), declared breakpoints (if any), and whether it is legal in
`analog`, `digital`, or both.

| family | entries |
|---|---|
| math | `sin`, `cos`, `exp`, `ln`, `sqrt`, `pow`, `abs`, `min`, `max`, `floor`, … |
| analog operators | `ddt`, `idt`, `idtmod`, `transition`, `delay`, `slew`, `absdelay`, `table`, `ac_stim` |
| detectors | `cross`, `above`, `timer` |
| convergence | `$limit` / pnjlim / fetlim |
| noise | `white_noise`, `flicker_noise` |
| logic encoding | 4-state ops over `bits<N>` — encoded as a (value, mask) word pair, §11 D3 |
| system | `$temperature`, `$abstime`, `$analysis`, … |

This is where MD-24 finally lands cleanly: the catalog *is* the declared
surface, one place, and both backends plus the interpreter implement the same
entry. It also absorbs today's four undeclared operators (`idtmod`, `absdelay`,
`table`, `ac_stim` — `GAPS.md` B6) and gives the vestigial `laplace_*`/`zi_*`
(B3) a place to be either implemented or formally absent.

**D8, resolved (user): `piperine-signal` *becomes* `piperine-ir`.** The crate
identity transfers rather than the code being copied into a new home. Its
existing families — runtime operators, event detectors, the junction limiter,
`table`, `Behavior`/`Measure`/`Wave`, digital edge logic — are the first catalog
entries and their implementations. Its purity guard (`tests/purity.rs`) stops
being a test and becomes a property of the thing: **the IR depends on nothing
in-tree by construction**, which is exactly the invariant an IR crate wants
anyway.

### 1.7 Where this sketch and the current codegen disagree

Checked against the tree, not remembered. Several rows contradicted earlier revs.

| # | reality | consequence |
|---|---|---|
| M1 | **Both bodies carry raw AST.** `AnalogBody.stmts` and `DigitalBody.stmts` are `Vec<ast::Stmt>`. Only analog *expressions* were resolved. | Less IR exists today than rev 1 claimed. The analog migration is bigger than "re-target". |
| M2 | `resolve::Type` = `{Real, Integer, Bool, Quad}` — no width, no vector. | Width is genuinely new. `Quad` becomes the 4-state encoding of `bits`. |
| M3 | Bundle-typed **values** are expanded in codegen lowering, not elaboration; `resolve/pom/structure.rs:151` is `ValueType::Bundle(_) => Type::Real` — a silent lossy exit. | Expansion moves ahead of the IR; the silent coercion becomes a named error. |
| M4 | Params are first-class with a runtime bank; MD-18 depends on it. | §1.4: host-written cells. |
| M5 | Natures are named per discipline (`NatureId`/`NatureInfo`/`NatureKind`). | §1.2: across/through, never `V`/`I`. **This is the one that would have broken thermal models.** |
| M6 | Symbol table also carries `StateKind`/`StateVar`, `TableRef`, `NoiseSource`, `InterpMode`, `LaplaceKind`/`ZKind`. | Most fold into §1.6's catalog. `LaplaceKind`/`ZKind` are vestigial — confirm before carrying across. |
| M7 | Digital `regs` come from **latch inference**, not declaration. | Conflicts with "one declared writer". See D10. |

### 1.8 Honest limits

**T0 would not have been caught by this.** It was a control-law defect inside
the solver's step controller (`PiController::propose_dt` non-monotone in the
error), beyond the frozen frontier. The IR addresses the *seam* class — four of
five.

---

## 2. The concept budget

Five concepts, one per §1 subsection: **value class** (§1.1), **net** (§1.2),
**value graph** (§1.3), **cell** (§1.4), **block** (§1.5) — plus the catalog
(§1.6), which is a table, not a mechanism. Rev 1 had eight. Rev 4 removed a
whole type system and rev 5 removed a pipeline stage; neither added anything.

| removed across revs | why |
|---|---|
| `struct` type | elaboration/lowering expands it |
| `region` as a structural layer | a tag on a block |
| 3 of 4 cell kinds | `charge`/`op` state is intrinsic-private |
| "crossing ops" as a category | six ordinary net accesses; crossing is emergent |
| hierarchy in the IR | lives in the POM |
| third regime for signal-flow | rescued a non-functional feature |
| `slice`/`concat`/`extract`/`insert` | representation (chunk lists), not ops |
| `int<N>`, `logic4`, `vec<T,N>`, `bool` as types (rev 3's set) | width + op properties |
| the `flatten` stage itself (rev 5) | control stays structured; nothing to flatten |

**The rule: five concepts. Any addition deletes something or carries a written
justification here.** A sixth appearing during migration is a signal to stop,
not to add.

Stated because it will be tested: rev 1 violated *"one mechanism per concept"*
two sections below where it wrote the rule, with the problem fresh. Attention is
not the antidote. A budget is.

---

## 3. What RTLIL teaches, and where we part ways

Studied 2026-08-09
([yosyshq.readthedocs.io](https://yosyshq.readthedocs.io/projects/yosys/en/latest/yosys_internals/formats/rtlil_rep.html)).

**Correction to a premise:** RTLIL *does* have memory — `RTLIL::Memory` (name,
word width, word count) plus `$memrd`/`$memwr`/`$meminit` cells linked by
`MEMID`. What it lacks for us is the **entire analog half**: no continuous
values, no conservation law, no differentiability, no residual, no Jacobian.
That is the real reason it cannot be adopted wholesale.

**Taken:**

| idea | use |
|---|---|
| `SigSpec`/`SigChunk`/`SigBit` — a signal is a concatenation of chunks | slice/concat become representation; four ops deleted |
| a wire has a **width**; a bus is one wire | §1.1 — and the untyped-after-typecheck insight follows from it |
| signedness is a **cell parameter**, not a wire property | §1.1 — which is why width alone suffices and no integer *type* is needed |
| memory kept separate from its access cells | §1.4 |
| `\user` vs `$generated` names | provenance for free; direct answer to the traceability complaint. Adopt verbatim |

**Taken, and further than RTLIL goes — structured control (D9, resolved).**
RTLIL keeps behavioral control flow as a distinct object
(`CaseRule`/`SwitchRule`/`SyncRule`) and lowers it with the `proc` pass.
Piperine does the opposite today: `flatten/analog.rs::walk` carries a guard and
flattens immediately into `select(g, new, old)`.

**The IR keeps control structured, and there is no flattening pass at all** —
not even for analog (user, rev 4). The premise that analog *needs* a flat form
does not survive inspection: symbolic differentiation is perfectly happy over
structured control, because `d/dx (if c then f(x) else g(x))` is
`if c then f'(x) else g'(x)` — you differentiate each arm. The discontinuity
when `c` itself depends on `x` already exists in today's `select` form, so this
is not a regression.

Cranelift emits branches instead of selects. That costs more calls and buys
back the entire `flatten/` stage. Optimization is out of scope for v1 anyway
(§1.3), and a flattening pass can be *added later as an optimization* if
measurement ever asks for one — which is the right order.

Consequence for §7: `flatten/` does not become a pass. It **disappears**.

---

## 4. Completeness — the three-design bar

Complete when these lower cleanly and simulate correctly. Different axes; none a
superset.

| design | axis | needs |
|---|---|---|
| **Opamp** | differentiable expressions, nonlinear convergence | `real` graph + diff pass, `ddt`, limiters, contributions |
| **Buck** | cross-domain access, time, events | node reads + signal drives in one design, `timer`/`cross` triggers, breakpoints |
| **RV32I core** | width, memories, control flow | `bits<32>`, `cell : [bits<32>; 32]`, chunk refs, indexed write, `match`, `edge` triggers |

The CPU is what today's language fails, and it fails on **representation**:

- no width → `pc + 4` on a `Bit[32]` does not compile
- no memory → no register file, no instruction memory
- `code[idx] = 0` → `Unsupported` (`emit/stmt.rs:120`) — the spec's own §10.3
  `SarAdc` example therefore does not compile
- `0b1??0` → unsupported (`emit/stmt.rs:246`)
- `for` in digital → unsupported (`emit/stmt.rs:98`)
- `UInt`/`SInt`/`concat`, promised by Part I §6.1/§6.6 → **do not exist** in
  `headers/`

Designing for the CPU is what makes the *buck's* controller pleasant. An FSM, a
bus, a counter, a lookup table are the same primitives. Not RTL ambition —
refusing to ship a mixed-signal simulator whose digital half cannot express a
state machine.

---

## 5. Three consumers, one representation

### 5.1 Analog backend

| IR | emitted |
|---|---|
| `contribute_through` / `force_across` | residual rows; a force adds a branch unknown |
| pure `real` subgraph | residual function |
| diff pass over it | Jacobian — derived once, not per consumer |
| `ddt` | `Q(V)` + companion via `IntegrationMethod::coeffs` |
| operator intrinsics | state slot + advance on accept |
| `ac_stim`, noise | AC row, `noise_current_psd` |

The win is the **input**: one shared IR, so the constraint/margin kernel reads
the same ops as the residual and A0 cannot recur.

### 5.2 Digital backend — a real backend for the first time

Today: AST → Cranelift, with cone and fusion logic the user cannot inspect. With
an IR it gets what a digital backend normally has — **cone extraction** as a
printable dataflow pass, **fusion** as IR → IR so "why did these fuse" is a
diff, **memories as memories**, and **width-native ops** until a lowering pass
says otherwise.

### 5.3 Reference interpreter — the strongest argument here

A direct IR walker. Slow, obvious, no Cranelift. It buys two things nothing else
does:

1. **Differential testing.** Interpreter and JIT must agree on every input. That
   alone would have caught A0, A0c, A0d, and likely A0b. Today there is exactly
   one implementation of each semantic, so nothing can disagree with it —
   precisely why 1841 tests missed a whole class.
2. **Traceability.** Step the IR, watch cells change. The direct answer to *"I
   lost traceability of what is happening in my own code."*

---

## 6. Migration order (D6, user-specified)

**Build the IR and its backends first, prove them, then move the language onto
it.** Not analog-first-inside-the-live-pipeline as rev 3 proposed.

1. **`piperine-ir` crate** — concepts, catalog, printer, verifier. Standalone.
2. **Backends + interpreter over the IR** — hand-written IR fixtures, differential
   tests, no PHDL involved.
3. **Test hard** — the three §4 designs expressed directly in IR before any
   front-end exists.
4. **Move the language onto it** — POM → IR lowering replaces
   `resolve`/`flatten`; codegen consumes IR.

The parity matrix runs unchanged throughout and gates step 4 continuously, not
just at the end.

### 6.1 Step 5 — splitting analog and digital devices in the solver

**Proposed by the user (rev 4), and it is a scope amendment — flagged, not
smuggled.**

The observation is empirically right. `SAMPLES_ANALOG` and `DEPENDS_ON_DIGITAL`
*are* the coupling, and it is sampling in both directions — a digital device
never loads a matrix, an analog device never drives a digital net except through
an explicit sample. So every `load_dc`/`load_ac`/`load_transient` on a digital
element is dead weight, and a dedicated digital branch could flatten the network
into one fused kernel, schedule by levelized cones, and drop per-element virtual
dispatch entirely. That is where the digital performance is.

Three things must be said out loud before this is agreed:

1. **It amends MD-01**, which is locked: *"One Element ABI, no downcast."* Its
   recorded justification is not aesthetic — *"a downcast-based facet split
   would block the future C-style FFI ABI; supertraits keep the object flat
   across the boundary."* If the FFI ABI is still a goal, that reason still
   stands and needs re-examination, not omission.
2. **It crosses the frontier this feature fixed.** "Solver and host stay" was
   the agreed scope. This moves the solver. Recording it as a new decision, not
   letting it drift in.
3. **The real risk: the split creates a new seam.** A mixed module — the buck's
   `Controller` has both bodies — becomes **two** solver objects that must stay
   coherent. That is precisely the shape of A0d.

**Why it is nonetheless safe, and only in this order.** The IR already separates
the two per *block*, and blocks share state only through single-writer cells
(§1.4). The solver split then becomes the **image** of a separation the IR
already enforces, rather than a parallel invention — and the shared-state
failure that produced A0d is governed by the single-writer rule, not by keeping
both halves inside one object.

So: **step 5, after the IR, never before or beside it.** Splitting first would
recreate A0d with a bigger blast radius.

---

## 7. What this does to the tree

| today | after |
|---|---|
| `piperine-lang` POM + elaboration | **stays**, plus §8's surface additions |
| `piperine-signal` → | **`piperine-ir`** (D4 + D8): concepts + catalog + printer + verifier + interpreter |
| `resolve/` | **becomes** POM → IR lowering; `diff.rs` moves to an IR pass |
| `flatten/` | **disappears** — control stays structured (D9), so there is nothing to flatten |
| digital AST→Cranelift path | **replaced** by IR → digital backend — the largest single win |
| `emit/` | **stays**, re-targeted onto IR |
| `piperine-signal` | **becomes `piperine-ir`** (D8) — crate identity transfers; its families are the first catalog entries |
| `device/` | **stays**, fed by IR |
| `piperine-solver` | **untouched in steps 1-4**; §6.1 proposes an analog/digital split as step 5 — a scope amendment, flagged |
| `piperine-api`, hosts, plugins | **untouched** — frontier |

---

## 8. PHDL surface changes (modified, not replaced)

The good ideas stay and get *sharper*, because the IR turns conventions into
structure:

- **`type` vs `net type`** → value class vs `node`/`signal`. Kept.
- **Elaboration as a phase** → unchanged; it produces the POM the IR is lowered
  from. Kept.
- **Symbolic differentiation** → promoted to an IR pass.
- **Plugin architecture** → kept, gains a new contribution kind (§9).

Additions §4 forces:

| need | surface |
|---|---|
| width-carrying values | `Bit[32]` becomes one value of width 32, not N nets |
| slice / concat | `x[31:25]`, `concat(a,b)` — Part I §6.6 promises `concat`; it does not exist |
| indexed write | `code[idx] = 0` — the spec's own §10.3 example must compile |
| multi-bit patterns | `0b1??0` — in the grammar, unsupported below |
| memories | a declaration form for an addressed cell |
| `for` in digital | unrolled at elaboration, or an IR loop |
| `UInt`/`SInt` | actually ship them in `headers/` |

**Hard constraint:** `headers/spice/` keeps working with bounded edits. Those
models are the ngspice-validated asset; a migration that rewrites them loses the
oracle. §1.2's nature-generic nodes are what make this safe — the thermal parts
of those models do not care that the IR forgot the word "volt".

---

## 9. The extension argument

Today a plugin contributes a **device** or a **script**. With an IR it can
contribute a **pass** — categorically larger, and where the parked ambitions
live: synthesis (goal (b), the idle `piperine-yosys`), schematic generation,
parasitic annotation, custom optimization. None need core changes if the IR is
the contract. **The IR is the product surface for extensibility**, not only an
internal cleanup.

---

## 10. Risks

| risk | mitigation |
|---|---|
| Rewrites kill projects | Frontier fixed at `Element`. Solver, host, plugins, parity matrix do not move. |
| Losing the ngspice parity asset | `headers/spice/` edits bounded; parity gates every migration step, not just the last. |
| "Complete" fighting "simpler" | The complexity that hurts is incidental (four representations), not essential. |
| Big-bang cutover | §6's order: IR proven standalone before the language moves. |
| Concept creep | §2's budget. |
| The interpreter rots | It is the differential oracle in CI — disagreement is a red build. |

---

## 11. Decisions and open questions

**Answered by the user, rev 4:**

| # | question | answer |
|---|---|---|
| D1 | `vec` element-typed or bit-flat | **Moot.** The type system collapsed (§1.1); what remains is width. |
| D2 | signal-flow (`Voltage`/`Current`) | **Stays declared-but-non-functional**, for reflection/query only. *Rider:* it must fail **loud**, not as `SymbolicSingular` (`GAPS.md` A0b). §1.2 gives that for free. |
| D4 | IR crate | **Own crate**, `piperine-ir`. Codegen consumes it. |
| D5 | `extern` → what | **Calls into the IR's intrinsic catalog** (§1.6), which needs expanding. |
| D6 | migration order | **IR → its backends → test hard → migrate the language** (§6). |
| D7 | how minimal | **As simple as a C or RTLIL IR.** The minimum here is larger (analog needs the full stamp computation; digital needs two cycle recipes) but the shape is the same: values, ops, blocks, explicit state. |
| — | types after typecheck | **Two value classes, effectively untyped** (§1.1). The user's provocation; it removed an entire type system. |
| — | `V`/`I` in the IR | **Removed.** Across/through, nature-generic (§1.2). Would have broken every thermal model. |
| — | events | **Representable at zero cost** — a block trigger (§1.5). |
| — | optimization | **Out of scope for v1.** The IR must permit passes and ship none. |
| D3 | 4-state encoding | **(value, mask) word pair** (agent's call). Classic Verilog-simulator form; `and`/`or`/`xor` are ordinary integer ops on the value word plus two mask ops, where packed 2-bit lanes need shift+mask per lane on *every* op. Same memory. Permits a future pass that spots an always-zero mask and drops to one word — most designs are 2-state in practice. |
| D8 | `piperine-signal`'s fate | **It becomes `piperine-ir`** (§1.6). |
| D9 | structured or flat control | **Structured, and no flattening pass at all** — not even for analog (§3). `flatten/` disappears. |
| D10 | inferred state | **Inference is a front-end pass that emits an explicit `cell`**, so the IR only ever sees declarations and §1.4's single-writer rule stays checkable. Also gives `InferredLatch` — specified in Part I §10.3/§16 with zero implementation (`GAPS.md` C2) — a place to be raised. |
| — | solver analog/digital split | **Endorsed in principle, deferred in practice** (§6.1). Amends MD-01, whose justification D12 retired. Out of scope here — its own feature when scheduled. |

| D11 | analog buses | **Add arrays** — `[T; D]` — covering the analog bus *and* the memory case in one concept (§1.1). Width (`bits<N>`) stays orthogonal: collapsing a digital bus into an array of bits would turn a 32-bit add into 32 ops. `cell` loses its `depth` modifier as a side effect. |
| D12 | is the C-style FFI ABI still a goal (MD-01's justification) | **No.** The downcast objection mattered before and does not now; two device types is judged *better* than one. §6.1's split is endorsed in principle — **and still deferred**: the solver is out of scope for this feature. |

**Still open:** nothing blocking. `spec.md` covers §6 steps 1–4; the solver
split (§6.1) is a separate feature, to be specified when scheduled.

---

## 12. What this document is not

Not a spec. Not a task list. No estimates.

**`spec.md` covers §6 steps 1–4.** Every open question is answered (rev 5).
The solver analog/digital split (§6.1) is endorsed in principle, deferred in
practice, and belongs to a separate feature.
