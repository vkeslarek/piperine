# Piperine device ABI — v1

Status: builder draft 1 of the adversarial duel (codegen context C8/O8). Complete and
self-contained; `changelog_v1.md` maps it to `critique_v0.md`. Home: crate `piperine-abi`
(no dependency), the only crate a device links against.

---

## 0. The idea

1. **A device is data plus two kinds of code.** The data is its *shape*. The code is
   *functions* (pure: bytes in, bytes out, with partial derivatives) and *reactions*
   (run at an instant; their only effect is a commit).
2. **The device has no memory.** Everything that exists over time lives in the solver:
   unknowns, discrete values, pending commits, integrator and delay histories, limiter
   iterates. State a device needs is a private discrete pin, exactly as state in MSL is a
   `signal`. So checkpoint, rollback and replay need no entry point.
3. **A pin is the only thing a device and the outside share.** Ports, internal nodes, branch
   flows, signals, state, params, temperature, time, timers, crossings, `settle`, record
   streams: all pins, told apart by *kind* (what they hold) and *role* (who binds them).
4. **The analog solver has four kinds of cell and nothing else:** a *term* (a value added to
   a row through one of three operators: 1, d/dt, delay), a *source* (a zero input that exists
   to be differentiated against: noise, AC stimulus), a *limit* (an iterate the solver holds
   between Newton iterations) and a *watch* (a value read at accepted solutions: an
   operating-point variable, a crossing, a step cap).
5. **One rule runs all code.** A function runs when the solver needs one of its outputs and
   one of its inputs changed. A reaction runs when an event pin it listens to fires.

---

## 1. Concepts (13, counted the critic's way: every table and every rule-bearing idea)

| # | Concept | One rule | A device that needs it |
| - | ------- | -------- | ---------------------- |
| 1 | **Module** | A named factory in a versioned library: `create(param values)` gives an instance and its shape. | any |
| 2 | **Shape** | The complete, immutable description of an instance as plain data. Instances with equal structure share one shape. The solver asks a device nothing by call. | any; 10 k transistors of one model |
| 3 | **Pin** | A place where the instance meets the outside. *Kind* says what it holds; *role* says who binds it (§3.1). | resistor, flip-flop, comparator |
| 4 | **Layout** | The only description of bytes: size, alignment, and fields with a closed set of scalar leaves. Two pins may share a net only if their layouts are equal. | an AVR pin wired to an MSL `Bit` |
| 5 | **Function** | Pure code with declared reads, writes and partials. Run when an output is needed and an input changed; may be run speculatively, in parallel, any number of times. | every analog block; a gate; a derived param |
| 6 | **Term** | A real value added to the row of pin `plus` and subtracted from the row of `minus`, through operator 1, d/dt or delay. | resistor (1), capacitor (d/dt), transmission line (delay) |
| 7 | **Source** | A real input that is zero in every solve and exists only to be differentiated against; it has a strength the sweep reads (a noise density, a stimulus phasor). | diode shot noise; an AC source |
| 8 | **Limit** | A real value the solver holds between Newton iterations: the device proposes `x`, answers `x̂`, and reads `x̂` back. The solve cannot converge while they differ. | any junction (`pnjlim`) |
| 9 | **Watch** | A value the solver reads at accepted solutions only: a variable for the host, a crossing to locate, a cap on the next step. | comparator (crossing); a PLL (step cap) |
| 10 | **Reaction** | Code run once at an instant when an event pin it listens to fires. It reads the settled pins and the accepted solution; its only effect is commits. | flip-flop; AVR emulator |
| 11 | **Commit** | One declared statement of a reaction writing bytes to a pin at a later instant: the next microstep, or the tick nearest `t + after`. Transport per statement. | flip-flop (`q <- d`); a timer; a record |
| 12 | **Session names** | The closed list of pins the solver itself binds by name (§3.2): time, frequency, analysis flags, temperature, continuation variables, the events `start` and `settle`, the streams `sim`, `checks`, `log`. Any other name is a host session parameter. | PWL source (`time`), flicker noise (`freq`), AVR (`tick`, `horizon`) |
| 13 | **Status** | Every call returns `OK`, `FAULT` (message and a device-chosen code in the note) or `RESHAPE` (the shape no longer fits the param values: re-create). | a model with a range check; OSDI node collapse |

Not concepts, and where they went:

| Usually in an ABI | Here |
| --- | --- |
| parameter get/set, structural arguments | a discrete pin with role `Param`; the host is its writer; `STRUCTURAL` is a flag; `create` takes initial values of the same pins |
| environment struct, temperature/simparam calls | pins with role `Session` |
| setup / temperature update / analysis hooks | a function that reads param and session pins and writes a private pin |
| internal node, branch-current unknown | a private continuous pin |
| private device memory | a private discrete pin, written by commit |
| timer, breakpoint, "wake me at" | a commit with a delay to a private event pin |
| record sites, `emit` | a pin of kind `Stream`; a record is a commit to it |
| `load_dc/ac/tran`, noise function, accept hook | functions; the per-solve meaning belongs to the cell's kind |
| checkpoint / restore | none |
| capability flags | none: an empty table is the capability |
| Jacobian mechanism (O1) | invisible (§8) |

## 2. Entry points (6)

| # | Entry | Side | Contract |
| - | ----- | ---- | -------- |
| 1 | `piperine_abi_1() -> *const Library` | device | the one exported symbol; `1` is the ABI major |
| 2 | `create(module, values, note) -> Created` | device | instance + shape for these param values |
| 3 | `destroy(instance)` | device | |
| 4 | `eval(instance, k, frame) -> Status` | device | run function `k`: read `input[]`, write `output[]` and `jac[]` |
| 5 | `react(instance, k, frame) -> Status` | device | run reaction `k`: read `input[]`, call `commit` |
| 6 | `commit(statement, offset, len, bytes, after, flags) -> Status` | solver | queue one commit of the calling reaction (or one trace record of the calling function) |

No function may unwind across the boundary.

---

## 3. The shape, exactly

`#[repr(C)]` Rust; these structs *are* the C layout (`cbindgen` prints the header). Every
enumeration is a `u32` with named constants, never a Rust `enum`, so an unknown value is
data that validation can reject.

```rust
#[repr(C)] pub struct Str     { ptr: *const u8, len: usize }             // UTF-8
#[repr(C)] pub struct Bytes   { ptr: *const u8, len: usize }
#[repr(C)] pub struct List<T> { ptr: *const T,  len: usize, stride: usize }
// stride = element size as the producer compiled it; a consumer reads the fields it knows.
// Appending a field to any element is a minor-version change.

#[repr(C)] pub struct Library { size: u32, minor: u32, name: Str, build: Str, modules: List<Module> }

#[repr(C)] pub struct Module {
    name: Str,
    data: *const c_void,                       // the module's own (an interpreter's IR, an OSDI descriptor)
    create:  unsafe extern "C" fn(*const Module, List<Value>, *mut Note) -> Created,
    destroy: unsafe extern "C" fn(*mut Instance),
    eval:    unsafe extern "C" fn(*mut Instance, u32, *const Eval)  -> u32,
    react:   unsafe extern "C" fn(*mut Instance, u32, *const React) -> u32,
}
#[repr(C)] pub struct Value    { name: Str, bytes: Bytes }                 // initial value of the Param pin of that name
#[repr(C)] pub struct Created  { status: u32, instance: *mut Instance, shape: *const Shape }
#[repr(C)] pub struct Instance { _opaque: [u8; 0] }
#[repr(C)] pub struct Note     { code: u32, len: u32, text: [u8; 248] }    // filled by the device on FAULT

#[repr(C)] pub struct Shape {
    size: u32,
    pins:      List<Pin>,
    terms:     List<Term>,
    sources:   List<Source>,
    limits:    List<Limit>,
    watches:   List<Watch>,
    functions: List<Function>,
    reactions: List<Reaction>,
}

#[repr(C)] pub struct Pin {
    name: Str, origin: Str,                    // origin = "file:line:col" of the source item, on every entry
    kind: u32,                                 // CONTINUOUS | DISCRETE | EVENT | STREAM
    role: u32,                                 // PORT | PARAM | SESSION | PRIVATE
    flags: u32,                                // STRUCTURAL | OPTIONAL | HAS_INIT | HAS_CHECK
    layout: Layout,                            // CONTINUOUS: one f64. EVENT: empty
    unit: Str,
    init: Bytes,                               // HAS_INIT. Discrete: its writer's first commit at (0,0); a Param's default.
                                               //           Continuous: Newton's start, and the hold of a transient's OP
    check: u32,                                // HAS_CHECK: a discrete pin of one bool that must be true for a host
                                               //            write of this Param to be accepted (its `where`)
    abstol: [f64; 2],                          // CONTINUOUS: tolerance of the unknown, of its row; 0 = session default
}

#[repr(C)] pub struct Layout { size: u32, align: u32, fields: List<Field> }
#[repr(C)] pub struct Field  { name: Str, leaf: u32, offset: u32, count: u32, flags: u32, known: u32 }
// leaf: BOOL U8 U16 U32 U64 I8 I16 I32 I64 F32 F64 (little-endian); TEXT (count bytes of UTF-8; Param pins only).
// count: array length (1 = scalar). Nested structs are flattened, names dotted.
// flags & HAS_KNOWN: `known` is the offset of one byte per element: 0 = this element holds no value.

#[repr(C)] pub struct Term   { name: Str, origin: Str, op: u32, plus: u32, minus: u32 }   // op: FLOW | DDT | DELAY
                                                                                            // plus/minus: a continuous pin, or GROUND
#[repr(C)] pub struct Source { name: Str, origin: Str, kind: u32 }                        // NOISE | STIM
#[repr(C)] pub struct Limit  { name: Str, origin: Str }
#[repr(C)] pub struct Watch  { name: Str, origin: Str, kind: u32, event: u32, layout: Layout } // VAR | CROSS | CAP
                                                                                            // event: CROSS fires this event pin

#[repr(C)] pub struct Cell    { kind: u32, index: u32, part: u32 }     // kind: PIN | TERM | SOURCE | LIMIT | WATCH
#[repr(C)] pub struct Partial { write: u32, to: u32, read: u32, at: u32 }
// d(writes[write], the f64 at byte `to`) / d(reads[read], the f64 at byte `at`)

#[repr(C)] pub struct Function { name: Str, origin: Str,
                                 reads: List<Cell>, writes: List<Cell>, partials: List<Partial> }
#[repr(C)] pub struct Reaction { name: Str, origin: Str, flags: u32,                    // IRREVERSIBLE
                                 on: List<u32>,                                         // event pins
                                 reads: List<Cell>, writes: List<Statement> }
#[repr(C)] pub struct Statement { pin: u32, offset: u32, len: u32, flags: u32 }         // LATER | REPLACE
```

The two frames. Every pointer in them is non-null and has the length the shape says.

```rust
#[repr(C)] pub struct Eval {
    size: u32,
    input:  *const *const u8,     // [reads.len]   the bytes of each read cell
    seed:   *const u8,            // [reads.len]   1 = partials against this read are wanted
    output: *const *mut u8,       // [writes.len]  a buffer per written cell, pre-filled with its current bytes
    jac:    *mut f64,             // [partials.len] pre-zeroed
    host:   *const Host, host_data: *mut c_void,     // only for trace records (§6)
    note:   *mut Note,
}
#[repr(C)] pub struct React {
    size: u32,
    input:  *const *const u8,     // [reads.len]
    host:   *const Host, host_data: *mut c_void,
    note:   *mut Note,
}
#[repr(C)] pub struct Host {
    commit: unsafe extern "C" fn(*mut c_void, statement: u32, offset: u32, len: u32,
                                 bytes: *const u8, after: f64, flags: u32) -> u32,      // flags: FINISH
}
```

### 3.1 Pins: kind and role

| Kind | Holds | Changes when | Writer |
| --- | --- | --- | --- |
| `CONTINUOUS` | one `f64` unknown and its equation row | the analog solver moves it | the solver; devices add terms to its row |
| `DISCRETE` | `layout.size` bytes | its bytes differ after a write | exactly one: a function, the statements of one instance's reactions, or the host |
| `EVENT` | nothing | it is written (it *fires*) | exactly one: a function (output byte nonzero), a statement, a `CROSS` watch, or the solver |
| `STREAM` | nothing; every write is a record delivered to the host's sink | never | any number; nobody reads it |

| Role | Bound by | If nothing binds it |
| --- | --- | --- |
| `PORT` | the netlist, by position among the `PORT` pins | private |
| `PARAM` | the netlist, by name | a net of its own written by the host, starting at `init` |
| `SESSION` | the solver, by name (§3.2) | error, unless `OPTIONAL`: then the cell reads as all zero |
| `PRIVATE` | nobody; the solver allocates it | — |

- **P1. One writer.** Who writes a pin is derived from the shapes: the function whose
  `writes` names it, or the instance whose statements name it. A net with two writers, or
  with a function writer and a statement writer, is a bind error. A net no instance writes is
  the host's.
- **P2. Zero is "nothing".** Discrete storage starts all zero. A field with `HAS_KNOWN` whose
  known byte is 0 holds no value, and its value bytes are zero. This is the whole of what the
  solver knows about "unknown": it never interprets a leaf.
- **P3. Layout equality.** Two pins on one net must have equal layouts (size, alignment, and
  per field: leaf, offset, count, known). Names do not count. An MSL `Bit` is a layout the
  elaborator prints like any other; an external device declares the same fields to connect.
- **P4. Params.** `create` receives initial values for any subset of the `PARAM` pins by name;
  the returned shape lists every `PARAM` pin with its effective `init`. One pin may be a
  whole struct of parameters (a model card): a host `set` of one parameter is a commit of one
  field's bytes, and instances of one model bind that pin to one shared net. Binding at
  elaboration or before the run is not a change. A write to a `STRUCTURAL` pin during a run is
  refused by the host; between runs it is `destroy` + `create` + bind.
- **P5. `where`.** A param with a constraint names a `check` pin written by a function. Before
  it commits a `set`, the host runs that function on a shadow of the store and refuses the
  write if the pin is false. (The same shadow evaluation gives the look-ahead of a settle
  commit, §15 S2.)
- **P6. `force`** is the owner's: a forced net reads as the forced bytes, its writer keeps
  writing underneath, and the release shows the writer's value again. A forced node is a
  potential source the owner itself stamps. No device can tell, and none takes part.

### 3.2 Session names

A `SESSION` pin is read-only for devices. A discrete one is rewritten by the solver between
calls and **its change is never an event** (it is not a net); an event one fires at the
instants stated; a stream one is global.

| Name | Kind, leaf | Value |
| --- | --- | --- |
| `time` | discrete `F64` | `now()` by §16.2: `t`, or 0 where the table says 0 |
| `freq` | discrete `F64` | the sweep point's frequency, else 0 |
| `tick`, `resolution` | discrete `I64`, `F64` | the instant's tick; seconds per tick |
| `horizon` | discrete `I64` | read by a reaction: no pin it listens to or reads will change before this tick (§5 R5) |
| `dc`, `tran`, `ac`, `noise` | discrete `BOOL` | the analysis kind |
| `static` | discrete `BOOL` | this solve is an equilibrium solve, or this instant belongs to one |
| `temperature` | discrete `F64` | the analysis temperature, kelvin |
| `gmin`, `source_scale` | discrete `F64` | continuation variables; at rest (`gmin` option, 1.0) in the last solve of every sequence |
| `start` | event | fires at `(0, 0)` of an analysis that starts from initial values |
| `settle` | event | fires once per time point after the microstep loop goes quiet |
| `sim`, `checks`, `log` | stream | the reserved global streams |
| any other | discrete `F64` | the host's session parameter of that name (`simparam`) |

A device reads only the names it declares, so the solver knows which functions depend on
frequency, time or temperature and re-runs only those.

---

## 4. Functions and the four analog cells

`eval(instance, k, frame)` reads `input[i]` for each `reads[i]`, writes `output[j]` for each
`writes[j]`, and writes `jac[p]` for each `partials[p]` whose read is seeded.

**F1. Purity.** Outputs are a function of the inputs. The instance may keep scratch and
caches (it is `*mut`), but nothing observable. Consequently a function may be skipped when
no input changed, run on a shadow, run twice, recorded and replayed.

**F2. Two sorts, one rule to tell them.** A function that reads a continuous pin, a source or
a limit is *analog*: it writes terms, sources, limits and watches, and runs inside solves. Any
other function is *discrete*: it writes discrete and event pins, and runs at instants, in
dependency order, when a pin it reads changed (and once at the first instant). Both may read
discrete pins; the discrete pins that analog functions read are the D→A boundary.

**F3. Granularity is the library's.** One function may write one gate output or every
combinational pin of a module; a library may export a fused block as one function. The solver
levelizes whatever it is given.

**F4. Partials** are exact derivatives of real outputs with respect to real inputs, on the
declared pattern. `seed` selects inputs; an unseeded entry may be left zero. Discrete
functions declare them too, for the real fields they read and write: that is what carries a
gradient from a param through a derived value into a term (§23.3 4c).

What a `Cell` names, as a read and as a write:

| Cell kind | Part | Read gives | Write means |
| --- | --- | --- | --- |
| `PIN` continuous | `VALUE` | the iterate (or, in a reaction, the accepted solution) | — |
| `PIN` discrete | `VALUE` | its bytes | (discrete function) its new bytes |
| | `PREVIOUS` | its bytes before its most recent change | — |
| | `CHANGED` | one byte: 1 if it changed in this microstep | — |
| `PIN` event | `CHANGED` | one byte: 1 if it fired in this microstep | (discrete function) one byte: nonzero fires it |
| `PIN` stream | `VALUE` | — | no buffer: a record, written by calling `commit` (§6) |
| `TERM` | `VALUE` | — | the value `v` |
| | `ABSTOL` (`DDT`) | — | tolerance of this state in the truncation test |
| | `TIME`, `MAX` (`DELAY`) | — | the delay `td`; its bound |
| `SOURCE` | `VALUE` | `0.0`, to differentiate against | — |
| | `PSD` (`NOISE`) | — | one-sided density |
| | `RE`, `IM` (`STIM`) | — | the phasor |
| `LIMIT` | `LAST` | the value held | — |
| | `AGE` | one byte: 0 no value held, 1 from the last accepted solution, 2 from the previous iteration of this solve | — |
| | `PROPOSED` | — | `x`: what Newton proposes (declare its partials) |
| | `LIMITED` | `x̂`: what the solver holds for this iteration | `x̂`: the device's answer |
| `WATCH` | `VALUE` | — | `VAR`: the value. `CROSS`: `e`. `CAP`: `dt` |
| | `TTOL`, `ETOL` (`CROSS`) | — | location tolerances |

An optional part nobody writes takes the session default. Parts that depend only on params
(`ABSTOL`, `MAX`, `TTOL`…) belong in a function that reads only params, so they are computed
when a param changes and not per Newton iteration.

**The solver's side, per cell kind** — this is §16.2 of the language, as the solver's own table:

| Cell | Equilibrium solve | Transient step | Small-signal sweep | At an accepted solution |
| --- | --- | --- | --- | --- |
| term `FLOW` | `v` | `v` | `∂v` | — |
| term `DDT` | 0 | the integrator's d/dt of `v`; `v` is a state of the truncation test | `jω·∂v` | history advances |
| term `DELAY` | `v` | `v(t − td)` from history | `exp(−jω·td)·∂v` | history advances |
| source `NOISE` | 0 | 0 | in `Noise`: density `PSD`, injected by the partials against it | — |
| source `STIM` | 0 | 0 | in `Ac`: phasor `RE + j·IM`, injected by the partials against it | — |
| limit | holds `x̂`; vetoes convergence while `x̂` and `x` differ beyond the limiting tolerance | same | `x̂ = x` | `LAST` advances |
| watch `VAR` | — | — | — | read; the host gets it by name |
| watch `CROSS` | compared between rounds and sweep points | located in the step (§17.3) | — | fires `event` when `e` went from `≤ 0` to `> 0` |
| watch `CAP` | — | — | — | the next step is at most the smallest `dt` |

- **A1. Row.** The row of a continuous pin is `Σ FLOW + d/dt Σ DDT + Σ DELAY = 0` over every
  term of every instance naming it. `i(a,b) +> x` is a `FLOW` term `{a, b}`. `v(a,b) +> x`
  uses a private continuous pin `f`: a `FLOW` term `{a, b}` of value `f`, and a `FLOW` term
  `{f, GROUND}` of value `v(a) − v(b) − x`. An operator whose result is used non-linearly is
  hoisted by elaboration onto a private pin `u` with the row `u − op(x) = 0`. So an operator is
  always linear at the top of a row, which is what lets the solver own it.
- **A2. Limit.** A function writes `PROPOSED` (with partials `p = ∂x/∂pins`) and `LIMITED =
  f(x, LAST)`; when `AGE` is 0, or the solve is `static` and `AGE` is 1, it starts from its
  seed (§18 rules 1, 2, 6). A limit it does not evaluate this iteration is answered with
  `LIMITED = PROPOSED = LAST`. Other functions read `LIMITED` and report plain values at
  `x̂` with partials `g` against it. The solver forms the Jacobian `g·p` and the
  right-hand-side correction `g·(x − x̂)` itself (§18 rule 4). No device does Newton
  bookkeeping, and a function that reads `x̂` is pure in `x̂`.
- **A3. Sources.** One noise source read on two paths is one source and two partials: fully
  correlated. Seen through an operator (`ddt(c·n)`) it is a partial of a `DDT` term, so the
  sweep applies `jω` with no further rule. Strengths are written by functions the solver
  needs only in the sweep that reads them; `k / freq` is never evaluated at `freq == 0`.
- **A4. Non-finite.** A non-finite value in a cell the current solve reads fails the solve; the
  solver names instance, cell and `origin`.

---

## 5. Reactions

`react(instance, k, frame)` reads `input[i]` for each `reads[i]` and calls `commit`.

- **R1. Trigger.** A reaction runs in a microstep in which an event pin in `on` fired, once.
  It listens to event pins only. `s.changed()`, `clk.rise()`, `a || b`, the non-probing part of
  a guard are discrete functions that fire a private event pin; `cross` is a `CROSS` watch;
  `at` is a delayed commit to an event pin; `settle` and `start` are session events. So
  "which reactions fire at `(t, n)`" is known before any reaction runs.
- **R2. Snapshot.** Every reaction of a microstep reads the same pins, settled before the
  first one ran; a commit is never visible in the microstep that made it. A reaction that
  reads a continuous pin reads the accepted solution of its instant (§15 I6).
- **R3. No memory.** A reaction is a function of its reads. State is a private discrete pin
  the reaction reads and commits to. The order of reactions in a microstep is therefore
  unobservable.
- **R4. Time-point test (§17.2.2).** A scheduled time is a time point when, after its commits
  and functions are applied: a discrete pin read by an analog function changed; or a reaction
  fires that reads a continuous pin; or a reaction fires that has a statement without
  `LATER` whose pin reaches, through reads and writes, an analog function or such a
  reaction. All of it is in the shapes.
- **R5. Horizon.** The session pin `horizon` is the earliest tick at which the owner will
  execute anything that can reach a pin this reaction listens to or reads, other than as a
  consequence of the commits this call makes (the stop time if nothing is queued; at most the
  end of the tentative analog step if a crossing can reach it). An emulator may run its own
  clock up to `horizon`, or to the due time of its first commit, whichever is earlier.
- **R6. Speculation and undo.** The owner may run reactions ahead of the accepted analog time
  and undo them; since reactions have no memory, undoing is restoring the owner's own
  discrete store, queue and record buffer. A reaction flagged `IRREVERSIBLE` (it keeps state
  the solver cannot copy: a foreign process) is never undone: the owner makes every instant
  at which it fires an accepted analog time before running it, and its `horizon` is exact,
  not tentative. It listens to `start` to learn that an analysis begins from initial values.

**The synchronization protocol** (what R6 means, normatively):

```
C  := checkpoint(discrete store, queue, record buffer)         at accepted analog time t
loop
  1. analog: solve tentatively to t_c  (Newton, truncation test, CROSS location; retake inside)
  2. run the discrete instants in (t, t_c) in order.
       stop at the first instant s that R4 makes a time point, or that fires an IRREVERSIBLE reaction
  3. if stopped at s < t_c:  restore(C);  t_c := s;  goto 1
  4. accept the analog solution at t_c; run the instant t_c (collect, microstep loop, settle)
  5. t := t_c;  C := checkpoint
```

Step 3 restores before every retake, so a crossing the retake discovers earlier than `s` is
applied before anything later than it (I3), and a rejected step leaves no trace (I8). The
replay in step 2 is deterministic. After step 4 nothing is undone: the solver's discontinuity
handling restarts the *next* step.

## 6. Commit

`commit(statement, offset, len, bytes, after, flags)`; `statement` indexes the calling
reaction's `writes`; `offset/len` lie inside the statement's declared range (an element of
an array, a field).

- **C1. When.** `after ≤ 0`, or a delay that rounds to no advance: the next microstep.
  Otherwise the tick nearest `t + after` seconds. `+inf` never comes due; NaN is refused.
  Where time does not advance, a positive delay never comes due. A statement declared `LATER`
  promises a delay that advances; the solver refuses a commit that breaks the promise.
- **C2. Order.** Commits to overlapping bytes that apply in one microstep are ordered by
  (instant made, statement index, call order); the last is the value. Statement index is
  source order.
- **C3. Transport.** A commit removes the pending commits of the same statement and byte range
  due at or after its own due instant. A statement declared `REPLACE` removes all of them (an
  armed time that moved).
- **C4. Events.** A statement on an event pin has `len = 0`; the pin fires when the commit
  applies.
- **C5. Records.** A statement on a stream pin is a record: it is not queued; it is stamped
  with instance, analysis and `(t, n)` and delivered when the instant is kept (dropped if the
  owner undoes the instant). `FINISH` ends the analysis at that instant. An analog function
  may also name stream cells in its `writes` and call `commit` on them: those are **traces**,
  stamped with the Newton iteration, delivered whatever happens to the step, ignored in a
  sweep. Records and traces are buffered per call and merged in (instant, instance path,
  statement index) order, so parallel evaluation cannot reorder them.
- **C6. The solver writes the same way.** A located crossing is a commit to the watch's event
  pin due at `(t, 0)`; a crossing between rounds is due at the next microstep; a host `set`
  or drive is a commit by the host.

---

## 7. The Rust side

```rust
pub trait Device: Sized + Send + 'static {
    fn create(values: &Values<'_>) -> Result<(Self, &'static Shape), Fault>;
    fn eval (&self, k: u32, cx: &mut Eval<'_>)  -> Result<(), Fault>;
    fn react(&self, k: u32, cx: &mut React<'_>) -> Result<(), Fault>;
}
pub const fn module<D: Device>(name: &'static str) -> Module;       // generic extern "C" thunks; no macro
```

`Eval` and `React` are borrow-checked lenses over the frames: `cx.f64(i)`, `cx.bytes(i)`,
`cx.get::<T>(i) -> Option<&T>` and `cx.out::<T>(j)` with `T: Pod` (any bit pattern valid,
zero included), `cx.set(j, v)`, `cx.jac()`, `cx.commit(statement, bytes, after)`. The thunks
wrap each method in `catch_unwind` (a panic is `FAULT` with the message in the note).
A library is one hand-written function returning a `static Library`.

**One path.** The solver calls every device through `Module`'s pointers, whether the
`Library` came from `dlopen` (generated Rust, C, an OSDI adapter, an emulator) or from a
value in the same process (the IR interpreter, a test element, a host stimulus). The
interpreter is a `Device` whose `Module.data` is one module's IR.

## 8. O1: the Jacobian

The contract is F4: exact partials on the declared pattern, for the seeded inputs. How a
device computes them cannot be observed. Recommended and assumed by the examples' comments:
generated Rust and the interpreter evaluate a function over dual numbers with one tangent
per seeded read; the pattern is the IR's static dependency set; `ddx` reads a tangent. A
foreign model fills `jac` from its own tables.

Purity gives the verifier for free: in a debug session the solver perturbs each real input of
a function and compares finite differences with `jac`. With A2 this is valid whether or not
a limiter is active, because `x̂` is an input.

---

## 9. Lifecycle

```
dlopen ─► piperine_abi_1()        major = symbol; check Library.size / minor
        ─► create(module, values)  per instance ─► (instance, shape); shapes are interned by the library
        ─► bind (solver only)      validate; map pins to nets; allocate private pins and buffers;
                                   levelize functions; build the sparsity pattern from terms × partials
        ─► analyses                eval / react
        ─► destroy(instance)
```

- A shape and everything it points to live until the process ends; instances share shapes
  (one per module and structural values), so per-instance cost is the binding.
- An analysis from initial values: the solver zeroes discrete storage, drops queue and
  histories, commits every `init`, fires `start`. An analysis at a transient's last instant
  (§16.4) keeps everything. Neither is a device call.
- `RESHAPE` from any call, or a host write to a `STRUCTURAL` pin between runs: `destroy`,
  `create` with the current param values, bind. Nothing is recompiled (C7). During a run
  `RESHAPE` ends the analysis.
- Bind validation fails loud: an unknown enumeration value; an index out of range; a partial
  whose `to`/`at` is not an `F64` leaf; a term target that is not continuous; an analog
  function writing a pin or a discrete function writing an analog cell (F2); two writers
  (P1); unequal layouts on a net (P3); a `CROSS` watch whose `event` is not an event pin; a
  reaction listening to a non-event pin; a dependency cycle through a one-writer function
  chain that is not closed by a relaxable net (D1 below).

**D1. Cycles among discrete functions** (resolved nets, §8.2.3). The elaborator lowers a
resolved carrier to one-writer pins plus a resolver *instance*: one drive pin per driver in
instance-path order, one value pin, one "others" pin per reader that needs it. An external
device joins such a net by having the netlist bind its output to a drive pin; it declares
nothing special. A cycle is then a cycle of functions across instances, and the solver
relaxes it with one generic rule: every pin of the cycle starts the microstep at zero bytes
(P2: nothing driven); the cycle's functions run in order until no byte changes, at most
(pins in the cycle + 1) passes; if bytes still move, the fields that moved are zeroed (P2:
they hold no value) and one record goes to `log`. The solver applies it to bytes and fields;
that zero means "not driving / unknown" is the layout's business.

---

## 10. Call sequences

`E(set)` = run the functions the solver needs from `set` whose inputs changed. `D` = run
discrete functions to a fixed point. `R` = run the reactions whose event pins fired, then
apply what is due at the next microstep.

**Operating point / one `Dc` point**

1. First point: zero, commit `init`s and the host's initial writes, fire `start`; `D`. Later
   sweep points: commit what the host changed; `D`.
2. Equilibrium solve, `static` true: per Newton iteration `E(functions writing terms and
   limits)` seeded on continuous pins and limits. Continuation rewrites `gmin` /
   `source_scale` between solves. Then `E(functions writing watches)` at the solution.
3. Microstep loop: `R`, `D`, … When a discrete pin read by an analog function changed: back to
   2 (a round; bounded); `CROSS` watches are compared between rounds. Bounded microsteps.
4. Fire `settle`; `R`. A commit with no advancing delay made by a reaction that ran because
   `settle` fired is a settle commit (§15 S1–S7).

**Transient**: the operating point with every continuous `init` held, then the protocol of §5.
The analog sub-solver reads its own copy of the discrete pins its functions read, updated
only by boundary events (and by the look-ahead of settle commits, via `D` on a shadow).

**`Ac`**: operating point; per frequency `E` of the functions that read `freq` (the others'
partials are reused), seeded on continuous pins, limits (`x̂ = x`) and sources; `static`
false. Matrix `Σ_terms resp(op, ω, td)·∂v`; right side `−Σ ∂v/∂stim_k · (RE_k + j·IM_k)`.
No reaction runs, no pin changes.

**`Noise`**: the same; for source `k` the injection vector is its column of partials through
each term's response, its density is `PSD_k`; output density `Σ_k |H·b_k|² PSD_k`.

**Host procedures**: `tf`, `pz`, `sp` use the `Ac` linearization with their own right side
(`STIM` strengths ignored). `sens` and `adjoint` seed the real fields of chosen `PARAM` pins
only, and chain through discrete functions' partials; where a path has no declared partial
(a foreign model's opaque cache) the host falls back to a finite difference, which purity
makes a plain re-evaluation. `pss` is continued transients.

**Host actions**: `set`, drive, `force`, `release` are owner operations on nets (P4–P6); a
probe is a scheduled time R4 counts.

---

## 11. Worked examples

Rust lens; partials written by hand so the contract shows (generated code gets them from
duals). Shapes are given as comments: `reads`/`writes` in order, partials as
`(write ← read)`. `bit` is the layout `{ v: BOOL @0, HAS_KNOWN @1 }`, 2 bytes;
`LOW = [0,1]`, `HIGH = [1,1]`.

### 11.1 Resistor

```rust
// pins:   p, n (PORT continuous);  r (PARAM F64, init 1e3)
// terms:  T0 FLOW {p, n} "i"
// fn 0 "i":  reads [p, n, r]   writes [T0.VALUE]   partials (0←0) (0←1) (0←2)
fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
    let (v, r) = (cx.f64(0) - cx.f64(1), cx.f64(2));
    cx.set(0, v / r);
    cx.jac().copy_from_slice(&[1.0 / r, -1.0 / r, -v / (r * r)]);    // the last only when r is seeded
    Ok(())
}
```

A live `set` of `r` is a host commit to its net; an analog function reads it, so the solver
lands there and restarts.

### 11.2 Diode with limiting and noise

```rust
// pins:    a, k (PORT);  is, n, cj0 (PARAM F64);  temperature, freq (SESSION)
// limits:  L0 "vd"       terms: T0 FLOW{a,k} "id", T1 DDT{a,k} "qj"
// sources: S0 NOISE "shot", S1 NOISE "flicker"        watches: W0 VAR "gd"
// fn 0 "limit":   reads [a, k, is, n, temperature, L0.LAST, L0.AGE, static]
//                 writes [L0.PROPOSED, L0.LIMITED]            partials (0←0)=1 (0←1)=-1
// fn 1 "current": reads [L0.LIMITED, is, n, cj0, temperature, S0, S1]
//                 writes [T0.VALUE, T1.VALUE]                 partials (0←0) (1←0) (0←5) (0←6)
// fn 2 "shot":    reads [L0.LIMITED, is, n, temperature]      writes [S0.PSD]
// fn 3 "flicker": reads [L0.LIMITED, is, n, temperature, freq] writes [S1.PSD]
// fn 4 "op":      reads [L0.LIMITED, is, n, temperature]      writes [W0.VALUE]
fn eval(&self, k: u32, cx: &mut Eval) -> Result<(), Fault> {
    match k {
        0 => {
            let nvt   = cx.f64(3) * K_B * cx.f64(4) / Q;
            let vcrit = nvt * (nvt / (SQRT_2 * cx.f64(2))).ln();
            let v     = cx.f64(0) - cx.f64(1);
            let (age, is_static) = (cx.u8(6), cx.bool(7));
            let old   = if age == 0 || (is_static && age == 1) { vcrit } else { cx.f64(5) };   // the seed
            cx.set(0, v);
            cx.set(1, pnjlim(v, old, nvt, vcrit));
            cx.jac().copy_from_slice(&[1.0, -1.0]);
        }
        1 => {
            let (vd, is, cj) = (cx.f64(0), cx.f64(1), cx.f64(3));
            let nvt = cx.f64(2) * K_B * cx.f64(4) / Q;
            let e   = (vd / nvt).exp();
            cx.set(0, is * (e - 1.0));                       // plain values at vd: no correction here
            cx.set(1, cj * vd);
            cx.jac().copy_from_slice(&[is * e / nvt, cj, 1.0, 1.0]);
        }
        2 => cx.set(0, 2.0 * Q * id(cx).abs()),
        3 => cx.set(0, KF * id(cx).abs() / cx.f64(4)),       // needed, hence run, only in a Noise sweep
        _ => cx.set(0, gd(cx)),
    }
    Ok(())
}
```

The solver holds `vd`, forms `∂id/∂vd · ∂v/∂(a,k)` and the correction `∂id/∂vd·(v − vd)`,
vetoes convergence while they differ, advances `LAST` on accepted steps. Function 3 is never
run where `freq` is 0. Function 4 runs once per accepted solution.

### 11.3 Capacitor

```rust
// pins: p, n (PORT);  c (PARAM F64)      terms: T0 DDT{p,n} "q"
// fn 0: reads [p, n, c]  writes [T0.VALUE]  partials (0←0) (0←1) (0←2)
fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
    let (v, c) = (cx.f64(0) - cx.f64(1), cx.f64(2));
    cx.set(0, c * v);
    cx.jac().copy_from_slice(&[c, -c, v]);
    Ok(())
}
```

The device says "this is a charge". Equilibrium, TR-BDF2, the truncation test, `jω·c`, the
history across an undone step are the solver's.

### 11.4 D flip-flop

```rust
// pins: clk, d (PORT bit);  q (PORT bit, init LOW);  rise (PRIVATE event)
// fn 0 "clk.rise()":  reads [clk.VALUE, clk.PREVIOUS, clk.CHANGED]   writes [rise]
// reaction 0 "on clk.rise()":  on [rise]   reads [d.VALUE]   writes [S0 {q}]
fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
    cx.set(0, (cx.bytes(2) == [1] && cx.bytes(0) == HIGH && cx.bytes(1) == LOW) as u8);
    Ok(())
}
fn react(&self, _k: u32, cx: &mut React) -> Result<(), Fault> {
    cx.commit(0, cx.bytes(0), 0.0)                    // q <- d : applies at (t, n+1)
}
```

A falling edge fires nothing, so it is never a candidate time point. `q` has one writer (the
statement); if an analog function somewhere reads `q`, R4 finds it from the shapes.

### 11.5 Comparator: analog → event

```rust
// pins: inp, inn (PORT continuous);  out (PORT bit, init LOW);  up, down (PRIVATE event)
// watches: W0 CROSS → up, W1 CROSS → down
// fn 0: reads [inp, inn]  writes [W0.VALUE, W1.VALUE]  partials (0←0) (0←1) (1←0) (1←1)
// reaction 0: on [up]   writes [S0 {out}]        reaction 1: on [down] writes [S0 {out}]
fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
    let e = cx.f64(0) - cx.f64(1);
    cx.set(0, e);  cx.set(1, -e);
    cx.jac().copy_from_slice(&[1.0, -1.0, -1.0, 1.0]);
    Ok(())
}
fn react(&self, k: u32, cx: &mut React) -> Result<(), Fault> {
    cx.commit(0, if k == 0 { &HIGH } else { &LOW }, 0.0)
}
```

The analog sub-solver runs function 0 at tentative solutions, sees `W0` go positive inside
a step, retakes to the located tick and commits a firing of `up` at `(t, 0)`. The digital
sub-solver sees an event pin fire. Neither names a type of the other.

### 11.6 DAC: event → analog

```rust
// pins: code (PORT U8);  out (PORT continuous);  vref (PARAM F64);  f (PRIVATE continuous: the branch flow)
// terms: T0 FLOW{out, GROUND} "i", T1 FLOW{f, GROUND} "eq"
// fn 0: reads [code, out, f, vref]  writes [T0.VALUE, T1.VALUE]  partials (0←2) (1←1) (1←3)
fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
    let k = cx.bytes(0)[0] as f64 / 255.0;
    cx.set(0, cx.f64(2));                             // KCL at out
    cx.set(1, cx.f64(1) - k * cx.f64(3));             // v(out) = k·vref
    cx.jac().copy_from_slice(&[1.0, 1.0, -k]);
    Ok(())
}
```

`code` is read by an analog function: when a commit changes it, R4 makes that instant a time
point, the owner delivers one boundary event (pin, bytes) and the integrator restarts.

### 11.7 An AVR emulator: an external device with its own time base

```rust
// pins:  pin_in[8]  (PORT bit)                what the pads read
//        pin_out[8] (PORT bit)                what the MCU drives
//        adc0 (PORT continuous)               reset (PORT bit)
//        firmware (PARAM TEXT, STRUCTURAL)    f_cpu (PARAM F64)
//        cpu (PRIVATE discrete, layout: the emulator's registers, SRAM, timers, its own tick — plain fields)
//        adc_val (PRIVATE F64)
//        irq, wake, sample (PRIVATE event)
//        tick, resolution, horizon, start (SESSION)
// fn 0 "pin change": reads [pin_in[0..8].CHANGED, reset.CHANGED, adc_val.CHANGED]   writes [irq]
// reaction 0 "run":    on [start, irq, wake]
//                      reads  [cpu, pin_in[0..8], reset, adc_val, tick, resolution, horizon, start.CHANGED]
//                      writes [S0 {cpu}, S1..S8 {pin_out[i], LATER}, S9 {wake, LATER|REPLACE}, S10 {sample, LATER}]
// reaction 1 "sample": on [sample]   reads [adc0]   writes [S0 {adc_val}]
fn react(&self, k: u32, cx: &mut React) -> Result<(), Fault> {
    if k == 1 { return cx.commit(0, &cx.f64(0).to_le_bytes(), 0.0); }

    let mut cpu: Cpu = *cx.get::<Cpu>(CPU).unwrap();           // a copy of the state pin; Cpu: Pod
    let (now, res, horizon) = (cx.i64(TICK), cx.f64(RES), cx.i64(HORIZON));
    if cx.bytes(START) == [1] { cpu = Cpu::reset(now); }
    cpu.set_pads(cx.bits(PIN_IN), cx.bytes(RESET));            // pin-change interrupts are raised here
    cpu.adc_result(cx.f64(ADC_VAL));

    // run my own clock until the horizon, or until my first externally visible action
    let stop = cpu.run(&self.flash, self.ticks_per_cycle, horizon);
    let dt = (cpu.tick - now) as f64 * res;
    match stop {
        Stop::PinWrite { pin, level } => cx.commit(1 + pin, &level, dt)?,   // the edge keeps its exact time
        Stop::AdcStart                => cx.commit(10, &[], dt)?,           // fire `sample` then
        Stop::Horizon | Stop::Sleep   => {}
    }
    cx.commit(9, &[], cpu.next_wake(now, res))?;               // "advance me to": REPLACE re-arms
    cx.commit(0, bytes_of(&cpu), 0.0)                          // the state, committed like any signal
}
```

| The emulator needs | It is |
| --- | --- |
| its own time base | `cpu.tick` inside its state pin, in session ticks (`resolution`) |
| to know how far it may run | `horizon` (R5): inputs cannot change before it; it stops earlier at its own first output, because that output may come back |
| "advance to a time" | the `wake` event pin, re-armed by a `REPLACE` commit |
| interrupts | function 0 fires `irq` when any pad changed; the reaction listens to it |
| to start with nothing driving it | the session event `start` |
| pin writes at exact times | `LATER` commits; their *due* instants, not the wakes, are what R4 tests |
| an ADC | a second reaction that reads a continuous pin, so only samples are analog time points |
| rollback | none of its own: its state is a pin, so the owner's checkpoint covers it and it may be run speculatively |
| inspection | `cpu` is a pin with a field layout: the host reads registers by path; pads are nets |

An emulator that cannot expose its state (a foreign process, a vendor model) sets
`IRREVERSIBLE` on `run`, keeps the state behind the instance, and drops the `cpu` pin and
`S0`. The owner then accepts the analog solution up to each of its instants before calling it
(§5 protocol) and gives it an exact `horizon`; the price is that a crossing that can reach its
pads keeps the horizon at "now".

### 11.8 A compiled OSDI-like model through an adapter

```rust
// create(values): runs setup_model + setup_instance on the given params; the collapse it reports fixes the shape
// pins:  d, g, s, b (PORT);  di, si, … (PRIVATE continuous, the uncollapsed internal nodes)
//        model (PARAM, layout = one field per model parameter; shared net per .model)
//        inst  (PARAM, layout = one field per instance parameter: w, l, m, …)
//        temperature (SESSION);  cache (PRIVATE discrete, layout = the model's instance data as U8[count])
//        ok (PRIVATE BOOL)  — `check` of `model` and `inst`
// fn 0 "setup":  reads [model, inst, temperature]   writes [cache, ok]
//                → RESHAPE if the collapse pattern differs from the shape's
// fn 1 "eval":   reads [d, g, s, b, di, si, cache, L*.LIMITED, S*]   writes [T*.VALUE]
//                partials = the descriptor's jacobian entries (resistive → FLOW terms, reactive → DDT terms)
// fn 2 "limit":  reads [nodes, cache, L*.LAST, L*.AGE]   writes [L*.PROPOSED, L*.LIMITED]
// fn 3 "noise":  reads [nodes, cache, freq]              writes [S*.PSD]
// fn 4 "opvars": reads [nodes, cache]                    writes [W*.VALUE]
```

`setup` runs when a parameter or the temperature changes and at no other time; 10 000
instances share one shape and one `model` net; a host `set` of `vth0` is a commit of eight
bytes that fires `changed` and re-runs `setup`. The cache is opaque, so `sens` over a foreign
parameter is a finite difference (§10); a generated MSL model declares partials on its own
setup function and gets exact gradients.

---

## 12. Safety, versioning, debugging

**Ownership.** Every pointer in a frame is valid for that call only; a device retains none.
Buffers have the shape's lengths and the layouts' alignment, and every copy the solver makes
of a pin keeps its alignment. `Library` and shapes live until the process ends; libraries
are never unloaded. An instance lives from `create` to `destroy` and holds only what `create`
derived plus scratch.

**Threads.** No two calls on one instance overlap. Calls on different instances may run on
different threads; functions of different instances may run in parallel. A library has no
mutable global state. `commit` is called only inside the call that received the `Host`.

**Undefined behavior closed.** No Rust `enum`, reference or trait object crosses: `u32`,
`f64`, raw pointers, `repr(C)` structs. `Instance` is an opaque zero-sized struct. No
unwinding. Typed access to bytes in the lens requires `Pod`. An absent `OPTIONAL` pin reads
as zeros through a valid pointer, and the lens returns `Option`. The shape is validated at
bind before any enumeration value is interpreted. In debug sessions outputs are poisoned and
guarded to catch an unwritten cell or a write past a buffer. A device leaves the
floating-point environment and signal handlers alone.

**Versioning.** Major is the symbol name. `Library.minor` may add: fields at the end of any
struct (`size`, `stride`), a new constant of an enumeration, a session name. An unknown
constant from a newer device is a bind error naming it; an older device never receives an
unknown one, because every value it is given is one its own shape declared. `Library.build`
identifies the artifact for the cache (C7).

**Debugging a misbehaving device.**

1. *Read it.* Every shape entry has a name and an `origin`; `piperine inspect lib.so Module`
   prints pins, cells, functions with their reads and writes. Generated Rust is ordinary
   source with debuginfo.
2. *Tap it.* A call is bytes in, bytes out, plus commits. A session option records the calls
   of chosen instances.
3. *Replay it.* Because nothing is hidden (F1, R3), a recorded call is a unit test: the same
   bytes through the compiled device and through the interpreter must agree. This is the
   differential oracle of C6 per call, for functions *and* reactions.
4. *Check it.* Finite-difference verification of `jac` (§8). A non-finite value is reported
   with instance, cell name and source position. A convergence failure names the limits still
   active and the rows out of tolerance.
5. *Watch it.* Every pin change, commit (instance, reaction, statement, made and due
   instants), crossing and record passes through the owner's one sink in `(t, n)` order.

---

## 13. Coverage of the brief

| Must couple | Mechanism |
| --- | --- |
| contributions, charges/`ddt`, `idt`, delays | terms `FLOW`, `DDT`, `DELAY`; `idt` is stdlib over `DDT` and a private pin |
| noise incl. flicker and correlation; AC stimulus | sources and partials against them; `freq` |
| limiting | limit cells, A2 |
| internal unknowns | private continuous pins |
| `bound_step`, crossings, operating-point variables | watches |
| operating point, continuation | `static`, `gmin`, `source_scale`, continuous `init` |
| definitions, drives, resolve | discrete functions; one-writer pins; resolver instance; D1 |
| reactions, commits with transport, microsteps | §5, §6 |
| the mixed boundary | discrete pins read by analog functions; `CROSS` → event pin; reactions reading continuous pins |
| DC/OP, AC, noise, transient, sweeps, tf/pz/sp/sens/adjoint/pss | §10 |
| names, params, live changes | shape; `PARAM` pins; host commits; `check` |
| checkpoint/rollback | the device has no memory; §5 protocol |
| external devices with their own time base | §11.7 (`horizon`, `start`, `LATER`, `IRREVERSIBLE`); §11.8 |
| versioning, `cdylib` safety, debuggability | §12 |

**Counts.** Concepts: 13 (counted as the critic counts). Entry points: 6 (5 device, 1 solver).

## 14. Known weak points

1. The analog side still has four cell kinds with seventeen named parts. They are typed and
   each has one row in the solver's table, but it is the densest part of the ABI.
2. `LATER` narrows §17.2.2 ("a reaction that can change a carrier `analog` reads") to "can
   change it at this instant". That is the reading that makes a delayed driver cheap; it
   should be confirmed in the spec text.
3. An `IRREVERSIBLE` reaction reachable from a crossing has `horizon = now`; such a device
   must expose its state as a pin or pay one call per own event.
4. State as a private pin costs one copy in and one committed range out per call.
5. Gradients across events (§23.3 rules 2, 3, 4a: saltation, `dt*/dp`, a commit carrying
   a sensitivity) are not carried: reactions declare no partials. The solver-completeness
   spec defers them; the extension is a `partials` list on `Reaction` (a minor version).
6. D1 lets the solver zero "fields that moved": the one place it acts on a layout's fields.
