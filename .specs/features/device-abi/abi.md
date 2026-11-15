# Piperine device ABI — final

Status: the outcome of the adversarial duel (codegen context C8/O8): v3, scored 8.1 with no
blocker, plus the fixes `critique_v3.md` asked for before closing. Complete and self-contained.
Nothing here has been compiled; §15 lists the language-spec amendments that await a decision. Home: crate `piperine-abi`
(no dependency), the only crate a device links against.

---

## 0. The idea

1. **A device is data plus two kinds of code.** The data is its *shape*. The code is
   *functions* (pure: bytes in, bytes out, with partial derivatives) and *reactions* (run at
   an instant; their only effect is a commit).
2. **The device has no memory.** Everything that exists over time lives in the solver:
   unknowns, discrete values, pending commits, integrator and delay histories, limiter
   iterates. State a device needs is a private discrete pin, as state in MSL is a `signal`.
   So checkpoint, rollback and replay need no entry point.
3. **A pin is a net seen from an instance, and obeys the net rules without exception.**
   Ports, internal nodes, branch flows, signals, state, params, temperature, timers,
   crossings, `settle`, records: all pins.
4. **What is not a net is an input or an output of a call.** An *input* is a value the solver
   supplies to a call (a limiter iterate, a noise or stimulus source; the time, the
   frequency, the horizon). A *term* is a value added to a row through one of three operators
   (1, d/dt, delay). A *watch* is an analog output that enters no row.
5. **One rule runs all code.** A function runs when the solver needs one of its outputs and
   one of its inputs changed. A reaction runs when an event pin it listens to fires.

---

## 1. Concepts (13, counted the critic's way)

| # | Concept | One rule | A device that needs it |
| - | ------- | -------- | ---------------------- |
| 1 | **Module** | A named factory in a versioned library. It carries a *prototype* shape (the shape at default param values); `create(values)` gives an instance and its shape. | any |
| 2 | **Shape** | The complete, immutable description of an instance as plain data. It depends on structure only, never on a non-structural value, so instances share it. | any; 10 k transistors of one model |
| 3 | **Pin** | A net seen from the instance. Its *kind* says what the net holds (a real unknown and its row; bytes; nothing but an occurrence); its *role* says who binds it. | resistor, flip-flop, comparator |
| 4 | **Layout** | The only description of bytes: size, alignment, fields over a closed set of scalar leaves. Pins share a net only if their layouts are equal. | an AVR pad wired to an MSL `Bit` |
| 5 | **Function** | Pure code with declared reads and writes. It writes analog cells or pins, never both. Run when an output is needed and an input changed; its outputs are held otherwise. | every analog block; a gate; a derived param |
| 6 | **Cell** | The address of one thing a function or reaction reads or writes: `(kind, index, part)` into the shape's tables. | — (it is how 5 and 11 name 3, 8, 9, 10) |
| 7 | **Partial** | A declared, exact derivative of one real output with respect to one real input of the same function; computed when the solver seeds it. | any analog block; a derived param under `sens` |
| 8 | **Term** | A real value added to the row of pin `plus` and subtracted from the row of `minus`, through operator 1, d/dt or delay. | resistor, capacitor, transmission line |
| 9 | **Input** | A value the solver supplies to a call and that is not a net. *Held* inputs are declared instances a function may differentiate against (the solver supplies the perturbation); *ambient* inputs are the call's coordinates, named by constant. | diode (`LIMIT`, `NOISE`), AC source (`STIM`), PWL source (`TIME`), AVR (`TICK`, `HORIZON`) |
| 10 | **Watch** | An analog output that enters no row; its kind says who reads it and when (§4.3): the host, the crossing locator, the step controller, the trace sink. | comparator; a PLL; a convergence trace |
| 11 | **Reaction** | Code run once at an instant when an event pin it listens to fires. It reads the settled pins and the accepted solution; its only effect is commits. | flip-flop; AVR emulator |
| 12 | **Commit** | One declared statement of a reaction writing bytes to a pin at a later instant: the next microstep, or the tick nearest `t + after`. Transport per statement. | flip-flop; a timer; a record |
| 13 | **Status** | A call returns `OK`, `FAULT` (message and a device-chosen code in the note), `RESHAPE` (this shape does not fit these param values) or `FINISH` (a reaction ends the analysis at its instant). | a range check; OSDI node collapse; `finish()` |

Not concepts, and where they went:

| Usually in an ABI | Here |
| --- | --- |
| parameters, structural arguments | discrete pins with role `PARAM`, written by the host; `STRUCTURAL` is a flag; `create` takes initial values of the same pins |
| temperature, `simparam`, analysis kind | discrete pins with role `GLOBAL`: real nets the host writes |
| time, frequency, continuation variables, horizon | inputs |
| setup / temperature / analysis hooks | a function that reads param and global pins and writes a private pin |
| internal node, branch-current unknown | a private continuous pin |
| device memory | a private discrete pin, written by commit |
| timer, breakpoint, "wake me at" | a commit with a delay to a private event pin |
| `emit`, record sites, streams | a commit to a discrete pin flagged `RECORD`; the sink observes every commit anyway |
| `load_dc/ac/tran`, noise function, accept hook | functions; the per-solve meaning belongs to the cell's kind |
| checkpoint / restore; capability flags | none |
| Jacobian mechanism (O1) | invisible (§8) |

## 2. Entry points (6), each with one contract

| # | Entry | Side | Contract |
| - | ----- | ---- | -------- |
| 1 | `piperine_abi_1() -> *const Library` | device | the one exported symbol; `1` is the ABI major |
| 2 | `create(module, values, note) -> Created` | device | instance + shape for these param values |
| 3 | `destroy(instance)` | device | frees the instance; releases its shape |
| 4 | `eval(instance, k, frame) -> Status` | device | run function `k`: read `input[]`, write `output[]` and seeded `jac[]` |
| 5 | `react(instance, k, frame) -> Status` | device | run reaction `k`: read `input[]`, call `commit` |
| 6 | `commit(statement, offset, len, bytes, after) -> Status` | solver | queue one write of the calling reaction (§6). Nothing else calls it, and it does nothing else. |

No function may unwind across the boundary.

---

## 3. The shape, exactly

`#[repr(C)]` Rust; these structs *are* the C layout (`cbindgen` prints the header). Every
enumeration is a `u32` with named constants, never a Rust `enum`.

```rust
#[repr(C)] pub struct Str     { ptr: *const u8, len: usize }             // UTF-8
#[repr(C)] pub struct Bytes   { ptr: *const u8, len: usize }
#[repr(C)] pub struct List<T> { ptr: *const T,  len: usize, stride: usize }
// stride = element size as the producer compiled it; a consumer reads the fields it knows.

#[repr(C)] pub struct Library { size: u32, minor: u32, name: Str, build: Str, modules: List<Module> }

#[repr(C)] pub struct Module {
    name: Str,
    prototype: *const Shape,                   // the shape at default values: lists every PARAM pin, its layout, its default
    data: *const c_void,                       // the module's own (an interpreter's IR, an OSDI descriptor)
    create:  unsafe extern "C" fn(*const Module, List<Value>, *mut Note) -> Created,
    destroy: unsafe extern "C" fn(*mut Instance),
    eval:    unsafe extern "C" fn(*mut Instance, u32, *const Eval)  -> u32,
    react:   unsafe extern "C" fn(*mut Instance, u32, *const React) -> u32,
}
#[repr(C)] pub struct Value    { pin: u32, bytes: Bytes }                  // a PARAM pin of the prototype, in its layout
#[repr(C)] pub struct Created  { status: u32, instance: *mut Instance, shape: *const Shape }
#[repr(C)] pub struct Instance { _opaque: [u8; 0] }
#[repr(C)] pub struct Note     { code: u32, len: u32, text: [u8; 248] }

#[repr(C)] pub struct Shape {
    size: u32,
    pins:      List<Pin>,
    terms:     List<Term>,
    inputs:    List<Input>,
    watches:   List<Watch>,
    functions: List<Function>,
    reactions: List<Reaction>,
}

#[repr(C)] pub struct Pin {
    name: Str, origin: Str,                    // origin = "file:line:col" of the source item, on every entry
    kind: u32,                                 // CONTINUOUS | DISCRETE | EVENT
    role: u32,                                 // PORT | PARAM | GLOBAL | PRIVATE
    global: u32,                               // role GLOBAL: TEMPERATURE | RESOLUTION | DC | TRAN | AC | NOISE | START | SETTLE
                                               //              | NAMED (the host's session parameter called `name`)
    flags: u32,                                // STRUCTURAL | OPTIONAL | HAS_INIT | HAS_CHECK | QUIET | RECORD | RELAX
    layout: Layout,                            // CONTINUOUS: one F64. EVENT: empty
    unit: Str,
    init: Bytes,                               // HAS_INIT. Discrete: its writer's first commit at (0,0); a PARAM's default.
                                               //           Continuous: Newton's start, and the hold of a transient's OP
    check: u32,                                // HAS_CHECK: a discrete BOOL pin that must be true for a host write
                                               //            of this PARAM to be accepted (its `where`)
    abstol: [f64; 2],                          // CONTINUOUS: tolerance of the unknown, of its row; 0 = session default
}

#[repr(C)] pub struct Layout { size: u32, align: u32, fields: List<Field> }
#[repr(C)] pub struct Field  { name: Str, leaf: u32, offset: u32, count: u32, flags: u32, known: u32 }
// leaf: BOOL U8 U16 U32 U64 I8 I16 I32 I64 F32 F64, little-endian. count: array length. Nested structs are
// flattened, names dotted. flags & HAS_KNOWN: `known` is the offset of one byte per element; 0 = no value.

#[repr(C)] pub struct Term  { name: Str, origin: Str, op: u32,        // FLOW | DDT | DELAY
                              plus: u32, minus: u32,                  // a continuous pin, or GROUND
                              abstol: u32, max: u32 }                 // DDT state tolerance, DELAY bound:
                                                                      //   a discrete F64 pin of this instance, or DEFAULT
#[repr(C)] pub struct Input { name: Str, origin: Str, kind: u32 }     // held inputs only: LIMIT | NOISE | STIM (§4.2)
#[repr(C)] pub struct Watch { name: Str, origin: Str, kind: u32,      // VAR | CROSS | CAP | TRACE
                              layout: Layout,                         // VAR, TRACE (CROSS, CAP: one F64)
                              event: u32, ttol: u32, etol: u32 }      // CROSS: the event pin it fires; tolerance pins or DEFAULT

#[repr(C)] pub struct Cell    { kind: u32, index: u32, part: u32 }    // kind: PIN | TERM | INPUT | WATCH | AMBIENT
                                                                      // AMBIENT: index is the constant (TIME, FREQ, …)
#[repr(C)] pub struct Partial { write: u32, to: u32, read: u32, at: u32 }
// d(writes[write], the F64 at byte `to`) / d(reads[read], the F64 at byte `at`)

#[repr(C)] pub struct Function { name: Str, origin: Str, flags: u32,                    // OWNS_CORRECTION (A3)
                                 reads: List<Cell>, writes: List<Cell>, partials: List<Partial> }
#[repr(C)] pub struct Reaction { name: Str, origin: Str, flags: u32,                    // IRREVERSIBLE
                                 on: List<u32>,                                         // event pins
                                 reads: List<Cell>, writes: List<Statement> }
#[repr(C)] pub struct Statement { pin: u32, offset: u32, len: u32, flags: u32,          // LATER | REPLACE
                                  min_after: f64 }                                      // LATER: every delay is at least this
```

**Constant values** (pinned by `piperine-abi`, `raw.rs`). Every enumeration group starts at 1
and every flag group at bit 0, so 0 is never a valid constant and an all-zero entry fails
validation:

| Group | Values |
| --- | --- |
| `kind` | `CONTINUOUS` 1, `DISCRETE` 2, `EVENT` 3 |
| `role` | `PORT` 1, `PARAM` 2, `GLOBAL` 3, `PRIVATE` 4 |
| `global` | `TEMPERATURE` 1, `RESOLUTION` 2, `DC` 3, `TRAN` 4, `AC` 5, `NOISE` 6, `START` 7, `SETTLE` 8, `NAMED` 9 |
| `Pin.flags` | `STRUCTURAL` 1, `OPTIONAL` 2, `HAS_INIT` 4, `HAS_CHECK` 8, `QUIET` 16, `RECORD` 32, `RELAX` 64 |
| `leaf` | `BOOL` 1, `U8` 2, `U16` 3, `U32` 4, `U64` 5, `I8` 6, `I16` 7, `I32` 8, `I64` 9, `F32` 10, `F64` 11 |
| `Field.flags` | `HAS_KNOWN` 1 |
| `op` | `FLOW` 1, `DDT` 2, `DELAY` 3 |
| `Input.kind` | `LIMIT` 1, `NOISE` 2, `STIM` 3 |
| `Watch.kind` | `VAR` 1, `CROSS` 2, `CAP` 3, `TRACE` 4 |
| `Cell.kind` | `PIN` 1, `TERM` 2, `INPUT` 3, `WATCH` 4, `AMBIENT` 5 |
| `Cell.part` | `VALUE` 1, `PREVIOUS` 2, `CHANGED` 3, `TIME` 4, `LAST` 5, `AGE` 6, `PROPOSED` 7, `LIMITED` 8, `PSD` 9, `RE` 10, `IM` 11 |
| ambient | `TIME` 1, `FREQ` 2, `STATIC` 3, `GMIN` 4, `SOURCE_SCALE` 5, `TICK` 6, `HORIZON` 7 |
| `Function.flags` | `OWNS_CORRECTION` 1 |
| `Reaction.flags` | `IRREVERSIBLE` 1 |
| `Statement.flags` | `LATER` 1, `REPLACE` 2 |
| status | `OK` 1, `FAULT` 2, `RESHAPE` 3, `FINISH` 4 |
| indices | `GROUND` = `u32::MAX` (a term's `plus`/`minus`), `DEFAULT` = `u32::MAX − 1` (a tolerance or bound pin) |
| version | `MAJOR` 1, `MINOR` 0, symbol `piperine_abi_1`; `Note.text` holds 248 bytes |

`piperine-abi` also ships two layouts beside those of P3: `F64` (`{ v: F64 @0 }`, size 8,
align 8, no known byte: the layout of every continuous pin) and `NONE` (size 0, align 1, no
field: the layout of every event pin).

The two frames. Every pointer is non-null and has the length the shape says.

```rust
#[repr(C)] pub struct Eval {
    size: u32,
    input:  *const *const u8,     // [reads.len]    the bytes of each read cell
    output: *const *mut u8,       // [writes.len]   a buffer per written cell, pre-filled with its current bytes
    seed:   *const u8,            // [partials.len] 1 = this partial is wanted
    jac:    *mut f64,             // [partials.len] pre-zeroed
    note:   *mut Note,
}
#[repr(C)] pub struct React {
    size: u32,
    input:  *const *const u8,     // [reads.len]
    host:   *const Host, host_data: *mut c_void,
    note:   *mut Note,
}
#[repr(C)] pub struct Host {
    size: u32,                    // a later minor may append (a commit that also carries tangent bytes, §14.4)
    commit: unsafe extern "C" fn(*mut c_void, statement: u32, offset: u32, len: u32,
                                 bytes: *const u8, after: f64) -> u32,
}
```

### 3.1 Pins

| Kind | The net holds | It changes when | Roles allowed |
| --- | --- | --- | --- |
| `CONTINUOUS` | one `F64` unknown and its equation row | the analog solver moves it | `PORT`, `PRIVATE` |
| `DISCRETE` | `layout.size` bytes | its bytes differ after a write | all four |
| `EVENT` | nothing | it is written (it *fires*) | `PORT`, `GLOBAL`, `PRIVATE` |

| Role | Bound by | If nothing binds it |
| --- | --- | --- |
| `PORT` | the netlist, by position among the `PORT` pins | private |
| `PARAM` | the netlist, by name | a net of its own, written by the host |
| `GLOBAL` | the session, by its `global` constant (or by `name` when `NAMED`) to one design-wide net | error, unless `OPTIONAL`: then it reads as all zero |
| `PRIVATE` | nobody; the solver allocates the net | — |

Rules that hold for every pin, whatever its role:

- **P1. One writer.** A discrete or event net has exactly one writer, derived from the shapes:
  the function whose `writes` names it, or the instance whose statements name it; else the
  host. (`START` and `SETTLE` are written by the owner; a `CROSS` watch writes its event pin.)
- **P2. A change is an event at an instant.** Whoever writes, a change of a net happens at a
  microstep and is seen by every reader there. A `GLOBAL` or `PARAM` net the host rewrites
  between analyses or sweep points changes at microstep 0 of the next point, like any host
  commit: a function that reads `TEMPERATURE` re-runs there and what it writes changes there.
- **P3. Zero is "nothing".** Discrete storage starts all zero. A field with `HAS_KNOWN` whose
  known byte is 0 holds no value and its value bytes are zero; debug sessions check this
  canonical form on every write. `piperine-abi` ships the common layouts as constants
  (`BIT`, `REAL`, `BYTE`), so an external author does not re-derive them. Each has one field `v` with a known byte:
  `BIT` is `{ v: BOOL @0, known @1 }` (size 2, align 1), `REAL` is `{ v: F64 @0, known @8 }`
  (size 16, align 8), `BYTE` is `{ v: U8 @0, known @1 }` (size 2, align 1).
- **P4. Layout equality.** Pins on one net have equal layouts (size, alignment, and per field
  leaf, offset, count, known). Names do not count.
- **P5. Params.** `Module.prototype` lists the `PARAM` pins with layouts and defaults; that is
  how the host encodes `Value`s and how the elaborator checks an `extern` declaration without
  creating anything. `create` gets initial values for any subset; the solver, not the shape,
  seeds each param net with what it passed. One pin may be a whole struct of parameters (a
  model card): a `set` of one parameter is a commit of one field, and instances of one model
  bind the pin to one shared net. `PARAM` pins come first in every shape of a module, in
  the prototype's order, so `Value.pin` means the same pin everywhere. A parameter that may
  be "not given" is a field with a known byte: 0 means not given, and the device's own
  setup function resolves its default (which may depend on other parameters). Binding a
  `PARAM` or `GLOBAL` net before the run is not a change, and neither is the flip of
  `DC`/`TRAN`/`AC`/`NOISE` by an analysis run at a transient's last instant. A host write to a
  `STRUCTURAL` pin during a run is refused; between runs it is `destroy` + `create` + bind.
- **P6. The parameter cone.** The *cone* is the set of discrete functions all of whose inputs
  derive, through functions only, from `PARAM` and `GLOBAL` nets; it is computed from the
  shapes at bind. A function with an input reachable from a reaction's statement, or from a
  `PORT` bound to a net something writes, is outside it. Only inside the cone may a `check`
  pin be written and may a function return `RESHAPE`; from anywhere else `RESHAPE` is a
  `FAULT`. Before the owner commits a host write to a `PARAM` or `GLOBAL` net (a `set`, a new
  `TEMPERATURE`, a sweep point's value) it runs the cone downstream of that net on a shadow of
  the store, across instances. If a `check` pin of the written param comes out false (its
  `where`), the write is refused. If a function returns `RESHAPE`: inside a transient the
  write is refused; at a sweep point or between analyses the owner re-creates and re-binds
  that instance and then commits — legal because the device has no memory and every net keeps
  its value. A host write to any other net (driving a port, a clock) is an ordinary commit
  with no shadow. So no `eval` of a committed store ever returns `RESHAPE`.
- **P7. Flags that remove a capability.** `QUIET`: the net has no change — nothing may read
  its `PREVIOUS`/`CHANGED`, no function may read it, and a commit to it never keeps the
  microstep loop alive (one that no further microstep picks up applies as the instant
  exits, so the next call reads it); it is for state only its own reactions read. `RECORD`: implies
  `QUIET`; the host's sink delivers every commit made to it as a record of the stream
  `<instance path>.<pin name>` (§6 C5). `RELAX`: a
  dependency cycle among functions may pass through this net (§9 D1).
- **P8. `force`** is the owner's overlay on a net: readers see the forced bytes, the writer
  keeps writing underneath, the release shows its value again. A forced node is a potential
  source the owner stamps itself. No device takes part.

---

## 4. Functions, cells, and the analog side

`eval(instance, k, frame)` reads `input[i]` for each `reads[i]`, writes `output[j]` for each
`writes[j]`, and writes `jac[p]` for each `partials[p]` with `seed[p] = 1`.

- **F1. Purity.** Outputs are a function of the inputs. The instance may keep scratch and
  caches, but nothing observable. A function may therefore be skipped when no input changed,
  run on a shadow, run twice, recorded and replayed.
- **F2. Two sorts, told apart by what they write.** An *analog* function writes terms, inputs
  and watches. A *discrete* function writes discrete and event pins. Mixing is a bind error.
  Either sort may read discrete pins; only analog functions read continuous pins and inputs.
  The discrete pins analog functions read are the D→A boundary.
- **F3. When.** A discrete function runs at an instant, in dependency order, when a pin it
  reads changed (once at the first instant). An analog function runs inside a solve when an
  output is needed and an input changed since its outputs were computed: a current source
  that reads only a param runs when the param changes and its term is held between.
- **F4. Granularity is the library's.** One function may write one gate or every
  combinational pin of a module; the solver levelizes what it is given.
- **F5. Partials** are exact, on the declared pattern; an unseeded entry may be left zero.
  Discrete functions declare them too, for the real fields they read and write: that carries
  a gradient from a param through a derived value into a term.

### 4.1 Cells

| Cell | Part | As a read | As a write |
| --- | --- | --- | --- |
| `PIN` continuous | `VALUE` | the iterate; in a reaction, the accepted solution | — |
| `PIN` discrete | `VALUE` | its bytes | (discrete function) its new bytes |
| | `PREVIOUS` | its bytes before its most recent change. It changes exactly when `VALUE` does, so any function or reaction may read it. | — |
| | `CHANGED` | one byte: 1 if it changed in this microstep. Only for reactions and for functions that write event pins only (anything else would go stale one microstep later). | — |
| `PIN` event | `CHANGED` | one byte: 1 if it fired in this microstep | (discrete function) one byte: nonzero fires it |
| `TERM` | `VALUE` | — | the value |
| | `TIME` (`DELAY`) | — | the delay `td` |
| `INPUT` (held) | `VALUE` | the value the solver supplies (§4.2) | — |
| | `LAST`, `AGE` (`LIMIT`) | the value held; one byte: 0 none held, 1 from the last accepted solution, 2 from the previous iteration | — |
| | `PROPOSED`, `LIMITED` (`LIMIT`) | — | `x` (declare its partials); `x̂` |
| | `PSD` (`NOISE`); `RE`, `IM` (`STIM`) | — | the strength |
| `WATCH` | `VALUE` | — | the value |
| `AMBIENT` | `VALUE` | the value the solver supplies (§4.2) | — |

A reaction reads `PIN` cells and the ambient inputs `TIME`, `STATIC`, `TICK`, `HORIZON`;
nothing else.

### 4.2 Inputs

*Held* inputs are entries of `Shape.inputs`: one per `limit`, noise source or stimulus of the
instance. A function reads `VALUE`, may declare partials against it, and writes its other
parts.

| Kind | `VALUE` as supplied | The solver's perturbation against it |
| --- | --- | --- |
| `LIMIT` | `x̂`, the iterate held for this Newton iteration | Newton's `x − x̂` |
| `NOISE` | 0 | in a `Noise` sweep, a random source of density `PSD` |
| `STIM` | 0 | in an `Ac` sweep, the phasor `RE + j·IM` |

*Ambient* inputs are the coordinates of the call. They have no shape entry: a cell names
one by constant, and that declaration is how the solver knows who depends on it.

| Constant | Leaf | Value | Read by |
| --- | --- | --- | --- |
| `TIME` | `F64` | the time of the solve or instant; 0 where time does not exist | analog functions, reactions |
| `FREQ` | `F64` | the sweep point's frequency, else 0 | analog |
| `STATIC` | `BOOL` | this is an equilibrium solve, or an instant of one | analog, reactions |
| `GMIN`, `SOURCE_SCALE` | `F64` | continuation variables; at rest in the last solve of every sequence | analog |
| `TICK` | `I64` | the last tick not after the instant | reactions |
| `HORIZON` | `I64` | the last tick through which nothing this reaction listens to or reads can change (R5) | reactions |

One rule for both families: an input belongs to a call, so it is never an event and no
discrete function reads one. A value that must exist at instants (temperature, the analysis
kind, a session parameter) is a `GLOBAL` pin instead, and obeys P1–P4.

### 4.3 What the solver does with each cell

Terms and held inputs:

| Cell | Equilibrium solve | Transient step | Small-signal sweep | At an accepted solution |
| --- | --- | --- | --- | --- |
| term `FLOW` | `v` | `v` | `∂v` | — |
| term `DDT` | 0 | the integrator's d/dt of `v`; `v` is a state of the truncation test (tolerance `abstol`) | `jω·∂v` | history advances |
| term `DELAY` | `v` | `v(t − td)` from history, `td ≤ max` | `exp(−jω·td)·∂v` | history advances |
| input `LIMIT` | holds `x̂`; no convergence while `x̂` and `x` differ beyond the limiting tolerance | same | `x̂ = x` | `LAST` advances |
| inputs `NOISE`, `STIM` | 0 | 0 | injected through the partials against them | — |

Watches, by who reads them and when:

| Watch | Equilibrium solve | Transient step | Small-signal sweep | At an accepted solution |
| --- | --- | --- | --- | --- |
| watch `VAR` | — | — | — | read; the host gets it by name |
| watch `CROSS` | compared between rounds and sweep points | located in the step | — | fires `event` when the value went from `≤ 0` to `> 0` |
| watch `CAP` | — | — | — | the next step is at most the smallest value |
| watch `TRACE` | read at every evaluation of its function | same | not read | — |

- **A1. Row.** The row of a continuous pin is `Σ FLOW + d/dt Σ DDT + Σ DELAY = 0` over every
  term naming it. A flow contribution into branch `(a, b)` is a `FLOW` term `{a, b}`. A
  potential contribution uses a private continuous pin `f`: a `FLOW` term `{a, b}` of value
  `f` and a `FLOW` term `{f, GROUND}` of value `x(a) − x(b) − value`. An operator whose result
  is used non-linearly is hoisted onto a private pin `u` with the row `u − op(x) = 0`. So an
  operator is always linear at the top of a row, which is what lets the solver own it.
- **A2. Inputs with a perturbation.** A function reads the input's `VALUE` and declares
  partials `g` against it; the solver multiplies by its perturbation. For `LIMIT`: a function
  writes `PROPOSED` (partials `p = ∂x/∂pins`) and `LIMITED = f(x, LAST)`, starting from its
  seed when `AGE` is 0 or when `STATIC` and `AGE` is 1; readers of `VALUE` report plain values
  at `x̂`; the solver forms the Jacobian `g·p` and the right-hand side `g·(x − x̂)`. For `NOISE`
  and `STIM`: the sweep injects `g` times the strength; one source on two paths is fully
  correlated; through a `DDT` term it picks up `jω` with no further rule.
- **A3. A limit not evaluated** in an iteration is answered `LIMITED = PROPOSED = LAST`.
  A function that writes a limit *and* the terms that depend on it in one call (a compiled
  foreign model) sets `OWNS_CORRECTION`: it declares no partial against that limit, reports
  values linearized at `x̂` and evaluated at `x`, and total partials against the pins; the
  solver applies no correction and its veto still works. The flag covers exactly the limits
  the function writes; it may still read another limit's `VALUE` in the ordinary form.
  Without the flag, a function that
  writes a limit and a term in one call is a bind error, so a forgotten partial cannot turn
  one form silently into the other.
- **A4. Non-finite.** A non-finite value in a cell the current solve reads fails the solve,
  naming instance, cell and `origin`. Strengths are needed only by the sweep that reads them,
  so `k / freq` is never evaluated at `freq = 0`.
- **A5. Trace.** A `TRACE` watch has a record layout. "Not written in this evaluation" is the
  canonical empty value of P3: every known byte 0. A site inside a loop is an array field
  with a declared maximum `count` and a count field; exceeding the maximum is a `FAULT`. The
  sink delivers each written record, stamped with the Newton iteration, whatever happens to
  the step.

---

## 5. Reactions

`react(instance, k, frame)` reads `input[i]` for each `reads[i]` and calls `commit`.

- **R1. Trigger.** A reaction runs in a microstep in which an event pin in `on` fired, once.
  It listens to event pins only. A change of a discrete pin, an edge, an `or` of events, the
  non-probing part of a guard are discrete functions that fire a private event pin; a
  crossing is a `CROSS` watch; a timer is a delayed commit to an event pin; `SETTLE` and
  `START` are global event pins. So which reactions fire at `(t, n)` is known before any runs.
- **R2. Snapshot.** Every reaction of a microstep reads the same pins, settled before the
  first one ran. A reaction that reads a continuous pin reads the accepted solution of its
  instant.
- **R3. No memory.** A reaction is a function of its reads; state is a private pin it reads
  and commits to. The order of reactions in a microstep is unobservable.
- **R4. Time-point test.** A scheduled time is a time point when, after its commits and
  functions are applied: a discrete pin read by an analog function changed; or a reaction
  fires that reads a continuous pin; or a reaction fires that has a statement without
  `LATER` whose pin reaches, through reads and writes, an analog function or such a reaction.
- **R5. Horizon.** `HORIZON` is the last tick before the earliest instant at which the owner
  can execute anything that reaches a pin this reaction listens to or reads, other than as a
  consequence of the commits this call makes (the stop time if nothing is queued; at most the
  end of the tentative analog step if a crossing can reach it). Nets the host may write in
  answer to a waiter (below) count as reachable from that waiter's event. An emulator may
  run its own clock through `HORIZON`, or to the due time of its first commit, whichever is
  earlier.
- **R6. Undo.** The owner may run reactions ahead of the accepted analog time and undo them by
  restoring its own discrete store, queue and sink buffer. A reaction flagged `IRREVERSIBLE`
  (state the solver cannot copy: a foreign process) is never undone: the owner has the analog
  solution accepted up to each instant at which it fires before running it, and its `HORIZON`
  is exact. Such a forced landing is **not a time point**: no settle phase runs there unless
  R4 makes it one. A session with an irreversible reaction cannot be rewound by the host, and
  every one of its instants costs an analog landing whenever the design has a `CROSS` watch.
- **R7. The host is a reaction.** Host code cannot be undone, so the owner treats it exactly
  as R6 says: a host waiter on an event (or on a pin's change, through an event function) is
  an `IRREVERSIBLE` reaction on that event pin whose statements are the nets the host
  writes; a host probe or read at a scheduled time is one that reads a continuous pin, which
  R4 makes a time point. The host is therefore called once, at an instant that is final, and
  what it does in answer is a commit due at the next microstep. A host that only schedules
  actions ahead of time and subscribes to records is not a reaction: its commits are already
  in the queue. The price is R6's: in a design with a `CROSS` watch, every firing of an event
  the host waits on is an analog landing, so a testbench that awaits every clock edge pays a
  solve per edge; scheduling the stimulus ahead and reading records does not.

**The synchronization protocol** (normative):

```
C := checkpoint(discrete store, queue, sink buffer)             at accepted analog time t
1. propose t_c  (step control, CAP watches, stop time)
2. run the discrete instants after t in order, up to t_c, stopping BEFORE the first instant s
   that R4 makes a time point or that fires an IRREVERSIBLE reaction (a host waiter included);
   if stopped, t_c := s.   If a reaction at an instant s returns FINISH:  t_c := s;  mark s final.
   The owner takes a sub-checkpoint c_s at the start of every instant it enters here: whether s
   is a landing may only be known after some of its microsteps ran (R4 is tested after commits
   and functions apply; a firing at microstep n > 0 is seen after microsteps 0..n-1). When s
   turns out to be a landing, restore(c_s): s is then untouched and step 5 runs it whole.
   (the instants completed here cannot read or move anything analog, so they need no solve)
3. analog: solve to t_c. A failed step or a located crossing gives an earlier landing t' < t_c.
4. if t' < t_c:  restore(C);  replay the discrete instants in (t, t');  t_c := t';  drop any `final` mark
   (the instant that returned FINISH is no longer reached in this step)
5. accept the analog solution at t_c;  run the instant t_c (collect, microstep loop; settle if a time point);
   if t_c is marked final, or a reaction returns FINISH here, the analysis ends after this instant
6. t := t_c;  C := checkpoint
```

One Newton solve per landing; what a restore replays is discrete work only. The instant of
a `FINISH` is thereby an accepted analog time and a time point, as the language requires; the
reaction that returned it is reversible (an irreversible one only ever runs in step 5), so
running it again there is sound. A crossing found
at `t'` is applied before anything later (I3); a rejected step leaves no trace (I8). After
step 5 nothing is undone: the discontinuity handling restarts the *next* step.

## 6. Commit

`commit(statement, offset, len, bytes, after)`; `statement` indexes the calling reaction's
`writes`; `offset/len` lie inside the statement's declared range.

- **C1. When.** `after ≤ 0`, or a delay that rounds to no advance: the next microstep.
  Otherwise the tick nearest `t + after` seconds. `+inf` never comes due; NaN is refused.
  Where time does not advance, a positive delay never comes due. A `LATER` statement promises
  a delay that advances by at least one tick. Whether a delay does depends on the session's
  resolution, which no library knows when it is compiled; so a generator sets `LATER` only on
  a statement whose delay has a lower bound it can state, the shape gives that bound
  (`Statement.min_after`, seconds), and bind checks it against `RESOLUTION` — a statement whose
  bound is under one tick simply loses the flag, with no error. A commit that then breaks its
  own stated bound is a `FAULT` naming the statement.
- **C2. Order.** Commits to overlapping bytes that apply in one microstep are ordered by
  (instant made, statement index, call order); the last is the value.
- **C3. Transport.** A commit removes the pending commits of the same statement and byte range
  due at or after its own due instant; a `REPLACE` statement removes all of them.
- **C4. Events.** A statement on an event pin has `len = 0`; the pin fires when it applies.
- **C5. Records.** The owner's sink sees every commit, with the instance, reaction, statement,
  the instant it was made and the instant it is due. A record is nothing more than a commit
  to a `RECORD` pin (one per stream and instance; a field absent from a site is a field with
  its known byte 0): the host receives it stamped with the instant it was *made*, in
  (instant, instance path, statement index) order. The stream is the pin's
  name under the instance path; which names are global (`sim`, `checks`) is the host's
  convention. Delivery is at the time the commit is *made*, so the transport removal of C3
  (a loop emitting eight records through one statement) loses no record: it only decides
  which one the pin ends up holding. Being `QUIET` it never prolongs an instant; being a
  commit it disappears if the owner undoes the instant that made it.
- **C6. The solver writes the same way.** A located crossing is a commit to the watch's event
  pin due at `(t, 0)`; a crossing between rounds is due at the next microstep; a host `set`
  or drive is a commit by the host.

---

## 7. The Rust side

```rust
pub trait Device: Sized + Send + 'static {
    fn prototype() -> &'static Shape;
    fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault>;   // ShapeRef = Arc<ShapeBuf>
    fn eval (&self, k: u32, cx: &mut Eval<'_>)  -> Result<(), Fault>;
    fn react(&self, k: u32, cx: &mut React<'_>) -> Result<Flow, Fault>;      // Flow::Continue | Flow::Finish
}
pub fn module<D: Device>(name: &'static str) -> Module;             // generic extern "C" thunks; no macro
```

`create` returns a `ShapeRef` (`Arc<ShapeBuf>`), not an `Arc<Shape>`: a `Shape` only points to
its lists, and `ShapeBuf` is what owns them; `module` is a plain `fn`, not `const fn`, because it
calls `D::prototype()`, a trait method. A `Fault` is `FAULT` with its code, or `RESHAPE` when
built with `Fault::reshape`; `cx.set(j, v)` takes any `Pod` (an `F64`, or the byte of an event
pin).

`Eval` and `React` are borrow-checked lenses over the frames: `cx.f64(i)`, `cx.bytes(i)`,
`cx.get::<T>(i) -> Option<&T>` and `cx.out::<T>(j)` with `T: Pod`, `cx.set(j, v)`,
`cx.wanted(p)`, `cx.jac()`, `cx.commit(statement, bytes, after)`. The thunks wrap each method
in `catch_unwind` (a panic is `FAULT` with the message in the note). A library is one
hand-written function returning a `static Library`.

**One path.** The solver calls every device through `Module`'s pointers, whether the
`Library` came from `dlopen` (generated Rust, C, an OSDI adapter, an emulator) or from a
value in the same process (the IR interpreter, a test element, a host stimulus).

## 8. O1: the Jacobian

The contract is F5. How a device computes partials cannot be observed. Recommended: generated
Rust and the interpreter evaluate a function over dual numbers with one tangent per seeded
partial's read; the pattern is the IR's static dependency set. A foreign model fills `jac`
from its own tables.

Purity gives the verifier: in a debug session the solver perturbs each real input of a
function and compares finite differences with `jac`, at points where every limit the function
writes has `LIMITED = PROPOSED` (for a function that only reads a limit, anywhere).

---

## 9. Lifecycle

```
dlopen ─► piperine_abi_1()        major = symbol; check Library.size / minor
        ─► read Module.prototype   params, layouts, defaults: enough to check a declaration and encode values
        ─► create(module, values)  per instance ─► (instance, shape)
        ─► bind (solver only)      validate; map pins to nets; seed param nets; allocate private nets and buffers;
                                   levelize functions; build the sparsity pattern from terms × partials
        ─► analyses                eval / react
        ─► destroy(instance)
```

- The prototype lives with the library. Any other shape lives until the last instance created
  with it is destroyed; a library returns the same shape to every instance of equal structure.
- An analysis from initial values: the solver zeroes discrete storage, drops queue and
  histories, commits every `init`, fires `START`. An analysis at a transient's last instant
  keeps everything. Neither is a device call.
- `RESHAPE` from `create` never happens (it returns the fitting shape). `RESHAPE` from `eval`
  means these values need another shape. It is legal only from a function of the parameter cone (P6),
  and there it is only ever seen on the shadow, because every host write to a `PARAM` or
  `GLOBAL` net passes that check first; the owner then refuses the write (inside a transient)
  or re-creates and re-binds (at a sweep point, between analyses). Outside the cone it is a
  `FAULT`. Nothing is recompiled.
- Bind validation fails loud: an unknown constant; an index out of range; a kind × role pair
  outside §3.1; a partial whose `to`/`at` is not an `F64`; a term target that is not
  continuous; a function writing both sorts (F2); a discrete function reading an input or a
  continuous pin; `CHANGED` read by a function that writes a discrete pin; a function writing a limit and a term without `OWNS_CORRECTION`, or with it and a partial against that limit; a read
  of a `QUIET` pin by a function; two writers (P1); unequal layouts on a net (P4); a reaction
  listening to a non-event pin; a statement on a continuous pin; a `CROSS` watch whose `event`
  is not an event pin; a term whose `abstol` or `max` is neither `DEFAULT` nor a discrete pin
  holding an `F64` at byte 0; a cycle among discrete functions that passes through no
  `RELAX` net.

**D1. Cycles** (resolved nets). The elaborator lowers a resolved carrier to one-writer nets
plus a resolver *instance* generated from the nature: one drive pin per driver in
instance-path order, flagged `RELAX`; one value pin; one "others" pin per reader that needs
it. An external device joins by ordinary binding. A cycle of functions through `RELAX` nets is
relaxed: those nets start the microstep at zero bytes (P3: not driving); the cycle's functions
run in order until no byte changes, at most (`RELAX` nets in the cycle + 1) passes; a field with a
known byte that still moves is zeroed (P3: no value) and one record goes to the resolver's
log pin; a field without one that still moves is a fault. A cycle through no `RELAX` net (two
cross-coupled gates written as functions) is a bind error.

---

## 10. Call sequences

`E(set)` = run the functions of `set` whose outputs are needed and whose inputs changed.
`D` = run discrete functions to a fixed point. `R` = run the reactions whose event pins
fired, then apply what is due at the next microstep.

**Operating point / one `Dc` point**

1. First point: zero, commit `init`s and the host's initial writes, fire `START`; `D`. Later
   points: commit what the host changed (a swept param, `TEMPERATURE`); `D`.
2. Equilibrium solve, `STATIC = 1`: per Newton iteration `E(functions writing terms and
   limits)`, seeded against continuous pins and limits. Continuation changes `GMIN` /
   `SOURCE_SCALE` between solves. Then `E(functions writing VAR, CROSS, CAP)` at the solution.
3. Microstep loop: `R`, `D`, … When a discrete pin read by an analog function changed: back to
   2 (a round; bounded); `CROSS` watches are compared between rounds. Bounded microsteps.
4. Fire `SETTLE`; `R`. A commit with no advancing delay made by a reaction that ran because
   `SETTLE` fired is due at microstep 0 of the next time point, and the solve that arrives
   there already reads it (`D` on a shadow).

**Transient**: the operating point with every continuous `init` held, then the protocol of §5.
The analog sub-solver reads its own copy of the discrete pins its functions read, updated only
by boundary events.

**`Ac`**: operating point; per frequency `E` of the analog functions (only those reading
`FREQ` actually re-run), seeded against continuous pins, limits and `STIM` inputs, `STATIC =
0`. Matrix `Σ_terms resp(op, ω, td)·∂v`; right side `−Σ g_k·(RE_k + j·IM_k)`. No reaction
runs, no pin changes.

**`Noise`**: the same, seeded against `NOISE` inputs; source `k` injects its column of partials
through each term's response with density `PSD_k`; output density `Σ_k |H·b_k|² PSD_k`.

**Host procedures**: `tf`, `pz`, `sp` use the `Ac` linearization with their own right side.
`sens` and `adjoint` seed exactly the partials on the path from the chosen param fields, and
chain through discrete functions' partials; where a path has none (a foreign model's opaque
cache) the host uses a finite difference, which purity makes a plain re-evaluation. `pss` is
continued transients.

**Host actions**: `set`, drive, `force`, `release` are owner operations on nets (P5–P8), each
those on `PARAM` and `GLOBAL` nets checked on a shadow first (P6). Scheduled ahead, they are commits in the queue; made in
answer to an event, they come from the host's reaction (R7).

---

## 11. Worked examples

Rust lens; partials by hand so the contract shows. Shapes are comments: `reads`/`writes` in
order, partials as `(write ← read)`. `BIT` is the shipped layout `{ v: BOOL @0, known @1 }`;
`LOW = [0,1]`, `HIGH = [1,1]`.

### 11.1 Resistor

```rust
// pins:   r (PARAM F64, default 1e3);  p, n (PORT continuous)        (PARAM pins first, P5)
// terms:  T0 FLOW {p, n} "i"
// fn 0 "i":  reads [p, n, r]   writes [T0]   partials (0←0) (0←1) (0←2)
fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
    let (v, r) = (cx.f64(0) - cx.f64(1), cx.f64(2));
    cx.set(0, v / r);
    cx.jac().copy_from_slice(&[1.0 / r, -1.0 / r, -v / (r * r)]);    // the last matters only when seeded
    Ok(())
}
```

Two resistors of 1 kΩ and 2 kΩ share this shape; each param net is seeded by the solver. A
current source is `fn 0: reads [idc] writes [T0]`: analog by what it writes, run when `idc`
changes.

### 11.2 Diode with limiting and noise

```rust
// pins:    is, n, cj0 (PARAM F64);  a, k (PORT);  temperature (GLOBAL TEMPERATURE)
// inputs:  I0 LIMIT "vd", I1 NOISE "shot", I2 NOISE "flicker"        (ambient: FREQ, STATIC)
// terms:   T0 FLOW{a,k} "id", T1 DDT{a,k} "qj"          watches: W0 VAR "gd"
// fn 0 "limit":   reads [a, k, is, n, temperature, I0.LAST, I0.AGE, STATIC]
//                 writes [I0.PROPOSED, I0.LIMITED]            partials (0←0)=1 (0←1)=-1
// fn 1 "current": reads [I0, is, n, cj0, temperature, I1, I2]
//                 writes [T0, T1]                             partials (0←0) (1←0) (0←5) (0←6)
// fn 2 "shot":    reads [I0, is, n, temperature]        writes [I1.PSD]
// fn 3 "flicker": reads [I0, is, n, temperature, FREQ]  writes [I2.PSD]
// fn 4 "op":      reads [I0, is, n, temperature]        writes [W0]
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
            cx.set(0, is * (e - 1.0) + cx.f64(5) + cx.f64(6)); // plain values at vd; the solver corrects
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

`fn 1` adds the `VALUE`s of `I1` and `I2` (0 outside a `Noise` sweep) to the current, so its
declared partials against them, 1 each, are exact (A2, F5).

### 11.3 Capacitor

```rust
// pins: c (PARAM F64);  p, n (PORT)      terms: T0 DDT{p,n} "q"
// fn 0: reads [p, n, c]  writes [T0]  partials (0←0) (0←1) (0←2)
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
// pins: clk, d (PORT BIT);  q (PORT BIT, init LOW);  rise (PRIVATE event)
// fn 0 "rise":  reads [clk.VALUE, clk.PREVIOUS, clk.CHANGED]   writes [rise]
// reaction 0:   on [rise]   reads [d]   writes [S0 {q}]
fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
    cx.set(0, (cx.bytes(2) == [1] && cx.bytes(0) == HIGH && cx.bytes(1) == LOW) as u8);
    Ok(())
}
fn react(&self, _k: u32, cx: &mut React) -> Result<Flow, Fault> {
    cx.commit(0, cx.bytes(0), 0.0)?;                  // q <- d : applies at (t, n+1)
    Ok(Flow::Continue)
}
```

### 11.5 Comparator: analog → event

```rust
// pins: inp, inn (PORT continuous);  out (PORT BIT, init LOW);  up, down (PRIVATE event)
// watches: W0 CROSS → up, W1 CROSS → down
// fn 0: reads [inp, inn]  writes [W0, W1]  partials (0←0) (0←1) (1←0) (1←1)
// reaction 0: on [up] writes [S0 {out}]        reaction 1: on [down] writes [S0 {out}]
fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
    let e = cx.f64(0) - cx.f64(1);
    cx.set(0, e);  cx.set(1, -e);
    cx.jac().copy_from_slice(&[1.0, -1.0, -1.0, 1.0]);
    Ok(())
}
fn react(&self, k: u32, cx: &mut React) -> Result<Flow, Fault> {
    cx.commit(0, if k == 0 { &HIGH } else { &LOW }, 0.0)?;
    Ok(Flow::Continue)
}
```

The analog sub-solver sees `W0` go positive inside a step, lands on the located tick, and
commits a firing of `up` at `(t, 0)`. The digital sub-solver sees an event pin fire.

### 11.6 DAC: event → analog

```rust
// pins: vref (PARAM F64);  code (PORT BYTE);  out (PORT continuous);  f (PRIVATE continuous: the branch flow)
// terms: T0 FLOW{out, GROUND} "i", T1 FLOW{f, GROUND} "eq"
// fn 0: reads [code, out, f, vref]  writes [T0, T1]  partials (0←2) (1←1) (1←3)
fn eval(&self, _k: u32, cx: &mut Eval) -> Result<(), Fault> {
    let k = cx.bytes(0)[0] as f64 / 255.0;
    cx.set(0, cx.f64(2));                             // KCL at out
    cx.set(1, cx.f64(1) - k * cx.f64(3));             // v(out) = k·vref
    cx.jac().copy_from_slice(&[1.0, 1.0, -k]);
    Ok(())
}
```

`code` is read by an analog function: when a commit changes it, R4 makes that instant a time
point, the owner delivers one boundary event and the integrator restarts.

### 11.7 An AVR emulator: an external device with its own time base

```rust
// pins:  pad_in[8], pad_out[8] (PORT BIT)     adc0 (PORT continuous)     reset (PORT BIT)
//        firmware (PARAM U8[256], STRUCTURAL)  f_cpu (PARAM F64)
//        cpu (PRIVATE discrete, QUIET; layout: registers, SRAM, timers, its own tick — plain fields)
//        adc_val (PRIVATE REAL)                log (PRIVATE discrete, RECORD)
//        irq, wake, sample (PRIVATE event)
//        resolution (GLOBAL RESOLUTION), start (GLOBAL START)
// fn 0 "pad change": reads [pad_in[0..8].CHANGED, reset.CHANGED, adc_val.CHANGED]   writes [irq]
// reaction 0 "run":    on [start, irq, wake]
//                      reads  [cpu, pad_in[0..8], reset, adc_val, resolution, start.CHANGED, TICK, HORIZON]
//                      writes [S0 {cpu}, S1..S8 {pad_out[i], LATER}, S9 {wake, LATER|REPLACE},
//                              S10 {sample, LATER}, S11 {log}]
// reaction 1 "sample": on [sample]   reads [adc0]   writes [S0 {adc_val}]
fn react(&self, k: u32, cx: &mut React) -> Result<Flow, Fault> {
    if k == 1 { cx.commit(0, &real(cx.f64(0)), 0.0)?; return Ok(Flow::Continue); }

    let mut cpu: Cpu = *cx.get::<Cpu>(CPU).unwrap();           // a copy of the state pin; Cpu: Pod
    let (now, horizon, res) = (cx.i64(TICK), cx.i64(HORIZON), cx.f64(RES));
    if cx.bytes(START) == [1] { cpu = Cpu::reset(now); }
    cpu.set_pads(cx.bits(PAD_IN), cx.bytes(RESET));            // pin-change interrupts are raised here
    cpu.adc_result(cx.get::<Real>(ADC_VAL));

    // run my own clock through the horizon, or to my first externally visible action
    let stop = cpu.run(&self.flash, self.ticks_per_cycle, horizon);
    let dt = (cpu.tick - now) as f64 * res;
    match stop {
        Stop::PadWrite { pad, level } => cx.commit(1 + pad, &level, dt)?,   // the edge keeps its exact time
        Stop::AdcStart                => cx.commit(10, &[], dt)?,           // fire `sample` then
        Stop::Break { pc }            => cx.commit(11, bytes_of(&Log { pc, known: 1 }), 0.0)?,   // a record
        Stop::Horizon | Stop::Sleep   => {}
    }
    cx.commit(9, &[], cpu.next_wake(now, res))?;               // "advance me to": REPLACE re-arms
    cx.commit(0, bytes_of(&cpu), 0.0)?;                        // the state, committed like any signal
    Ok(Flow::Continue)
}
```

| The emulator needs | It is |
| --- | --- |
| its own time base | `cpu.tick` in its state pin, in session ticks |
| to know how far it may run | `HORIZON` (R5); it stops earlier at its own first output, because that output may come back |
| "advance to a time" | the `wake` event pin, re-armed by a `REPLACE` commit |
| interrupts | function 0 fires `irq` when a pad changed |
| to start with nothing driving it | the global event `START` |
| pad writes at exact times | `LATER` commits; their due instants, not the wakes, are what R4 tests |
| an ADC | a second reaction that reads a continuous pin, so only samples are time points |
| rollback | none of its own: its state is a pin in the owner's checkpoint |
| cheap state | `QUIET`: no previous value, no change detection, no extra microstep |
| inspection | `cpu` has a field layout: the host reads registers by path |

An emulator that cannot expose its state sets `IRREVERSIBLE` on `run` and drops `cpu`; R6
states the price.

### 11.8 A compiled OSDI-like model through an adapter

```rust
// module "OsdiModel" (one instance per .model):
//   pins: card (PARAM, one field per model parameter);  temperature (GLOBAL);  mdata (PORT discrete: the model data)
//   fn 0 "setup_model": reads [card, temperature]  writes [mdata]
// module "Osdi" (one instance per device), shape fixed by the collapse `create` computes:
//   pins: d, g, s, b (PORT);  di, si, … (PRIVATE continuous: the uncollapsed nodes)
//         mdata (PORT discrete);  inst (PARAM: w, l, m, …);  temperature (GLOBAL)
//         idata (PRIVATE discrete: the instance data + a hash of it);  ok (PRIVATE BOOL: the `check` of inst)
//   inputs: I* LIMIT (one per limited quantity), I* NOISE
//   fn 0 "setup":  reads [mdata, inst, temperature]   writes [idata, ok]
//                  → RESHAPE if the collapse differs from this shape's
//   fn 1 "eval" (OWNS_CORRECTION):  reads [nodes, idata, I*.LAST, I*.AGE, STATIC, I* NOISE]
//                  writes [T*  (resistive → FLOW, reactive → DDT),  I*.PROPOSED, I*.LIMITED]
//                  partials = the descriptor's jacobian entries, total, against the nodes; none against a limit (A3)
//   fn 2 "noise":  reads [nodes, idata, FREQ]   writes [I*.PSD]
//   fn 3 "opvars": reads [nodes, idata]         writes [W*]
```

The compiled `eval` limits inside itself through the simulator callbacks; the adapter serves
those callbacks from `LAST`/`AGE`, records `PROPOSED`/`LIMITED`, and passes on residuals with
the model's own correction (A3). It mutates its instance data, so the adapter keeps a scratch
copy in the instance and refreshes it when the hash in `idata` differs: F1 holds, a refused
shadow `set` (P6) corrupts nothing, and nothing is copied per iteration. A `set` of `vth0` is
a commit of eight bytes to the shared `card` net; `setup_model` runs once, each `setup` once.
Both setups are in the parameter cone (their inputs are `PARAM` and `GLOBAL` nets and each
other), so before that commit P6 has run `setup_model` and every downstream `setup` on a shadow: if one
of them reports another collapse, the `set` is refused in a transient, and re-creates that
instance at a sweep point. A parameter the netlist did not give has known byte 0 in `card`
or `inst`, and `setup` applies the model's own default.

---

## 12. Safety, versioning, debugging

**Ownership.** Every pointer in a frame is valid for that call only; a device retains none.
Buffers have the shape's lengths and the layouts' alignment; every copy the solver makes of a
pin keeps its alignment. The library and its prototypes live until the process ends; another
shape until its last instance is destroyed.

**Threads.** No two calls on one instance overlap. Calls on different instances may run on
different threads; functions of different instances may run in parallel. A library has no
mutable global state. `commit` is called only inside the `react` that received the `Host`.

**Undefined behavior closed.** No Rust `enum`, reference or trait object crosses. `Instance`
is an opaque zero-sized struct. No unwinding. Typed access in the lens requires `Pod`. An
absent `OPTIONAL` pin reads as zeros through a valid pointer and the lens returns `Option`.
The shape is validated at bind before any constant is interpreted. Debug sessions poison and
guard output buffers. A device leaves the floating-point environment and signals alone.

**Versioning.** Major is the symbol name. A minor may add fields at the end of any struct
(`size`, `stride`), a constant, a `global` constant. Reserved names are constants, so a host
session parameter can never collide with a later one. An unknown constant from a newer device
is a bind error naming it; an older device never receives one, because every value it is
given is one its own shape declared.

**Debugging a misbehaving device.**

1. *Read it.* Every shape entry has a name and an `origin`; `piperine inspect lib.so Module`
   prints pins, cells and functions. Generated Rust is ordinary source with debuginfo.
2. *Tap it.* A call is bytes in, bytes out, plus commits. A session option records the calls
   of chosen instances.
3. *Replay it.* Nothing is hidden (F1, R3), so a recorded call is a unit test: the same bytes
   through the compiled device and through the interpreter must agree, for functions and
   reactions alike.
4. *Check it.* Finite-difference verification of `jac` (§8); the canonical-form check of P3;
   a non-finite value reported with instance, cell and source position; a convergence failure
   naming the limits still active and the rows out of tolerance.
5. *Watch it.* Every pin change, commit, crossing and record passes through the owner's one
   sink in `(t, n)` order.

---

## 13. Coverage of the brief

| Must couple | Mechanism |
| --- | --- |
| contributions, charges/`ddt`, `idt`, delays | terms `FLOW`, `DDT`, `DELAY`; `idt` is stdlib over `DDT` and a private pin |
| noise incl. flicker and correlation; AC stimulus | inputs `NOISE`, `STIM`, `FREQ`; A2 |
| limiting | input `LIMIT`; A2, A3 |
| internal unknowns | private continuous pins |
| `bound_step`, crossings, operating-point variables | watches |
| operating point, continuation | `STATIC`, `GMIN`, `SOURCE_SCALE`, continuous `init` |
| definitions, drives, resolve | discrete functions; one-writer nets; resolver instance; D1 |
| reactions, commits with transport, microsteps | §5, §6 |
| the mixed boundary | discrete pins read by analog functions; `CROSS` → event pin; reactions reading continuous pins |
| DC/OP, AC, noise, transient, sweeps, tf/pz/sp/sens/adjoint/pss | §10 |
| names, params, live changes | prototype and shape; `PARAM` pins; host commits; `check` |
| records | commits to `RECORD` pins; `TRACE` watches; `FINISH` |
| checkpoint/rollback | the device has no memory; §5 protocol |
| external devices with their own time base | §11.7; §11.8 |
| versioning, `cdylib` safety, debuggability | §12 |

**Counts.** Concepts: 13, with Cell and Partial counted as the critic counts them.
Entry points: 6, each with one contract.

## 14. Known weak points

1. Input is one name over two families (three held kinds with a perturbation, seven ambient
   constants). One sentence is true of both — "the solver supplies it to a call; it is not a
   net" — and the tables are now printed apart.
2. The ABI leans on amendments to the language specification (§15). The one with observable
   weight is `LATER` (the time-point rule); it is the user's decision.
3. An `IRREVERSIBLE` reaction pays an analog landing per instant in any design with a
   crossing, and makes the session non-rewindable.
4. Gradients across events are not carried; reactions declare no partials. The extension is a
   `partials` list on `Reaction` plus a commit that carries tangent bytes: `Host.size` exists
   so that entry can be appended.
5. A `TRACE` site in a loop is bounded by a maximum the elaborator declares; past it the
   evaluation faults.
6. D1's "a moving field with a known byte is zeroed" is the one place the solver acts on a
   layout's fields.
7. `Pin` carries fields that apply to a subset of kind × role (`global`, `check`, `init`,
   `abstol`); it is one struct on purpose, and it is where a future field would land.
8. P6's shadow costs one pass of the cone downstream of a written param (10 000 `setup`
   calls for a model-card `set`); the committed pass can reuse its results. Host drives of
   ports cost nothing extra.
9. Two limit forms exist (A2, A3) because one foreign format limits inside its own call.

---

## 15. Spec amendments awaiting a decision

The ABI is implementable against the language specification (`docs/spec/` of the codegen-plan
repo) as written, except for the sentences below. None has been edited. The verdict column is
the critic's, from `critique_v3.md`; the last column is what this document assumes meanwhile.

| # | Where | Proposed change | Critic's verdict | This document assumes |
| - | ----- | --------------- | ---------------- | --------------------- |
| 1 | `part_v_execution.md` §17.2 rule 2, second bullet | "… or can change, **at that instant**, a carrier that `analog` or a probing reaction reads. A commit whose delay always advances time **by at least one tick** changes nothing at the instant that makes it; its due time is a scheduled time tested on its own." | **Approve with the tick wording — user decision.** The only amendment with observable weight: the set of time points `settle` sees shrinks. | the amended reading (`LATER`, R4, C1). If refused: delete the `LATER` flag; every wake of a delayed driver becomes a time point; nothing else changes. |
| 2 | `part_v_execution.md` §15, definition of *time point* | "A time point is a time at which the analog solver produces an accepted solution **and runs the time-point procedure**. A simulator MAY accept solutions at other times; those are not time points and `settle` does not fire there." Instance state advances on every accepted solution (§17.2.4 already says so). | **Approve.** Needed by host waiters independently of co-simulation. | amended (R6, R7). |
| 3a | `part_iii_structure.md` §8.5.8 and contract table; `appendix_g_scope.md` G.65 | The C interface of an extern module is the device ABI; `"cosim"` is a module whose reaction is irreversible; `"osdi"` names the adapter of §11.8. | **Approve.** | amended. |
| 3b | `part_iii_structure.md` §8.5.11, §8.5.15, §8.5.16 | Let a `"cosim"` module have continuous ports and read them; move the port restriction to the adapter. | **Defer to the user**: a language scope change the ABI does not need; §8.5.15 and §8.5.16 would have to change with it. | *not* amended. The ABI allows a reaction to read a continuous pin (the AVR of §11.7 does); whether the language's `extern "cosim"` may declare one is separate. |
| 4 | `part_iii_structure.md` §8.5.10; `part_viii_host.md` §23.1 rule 7 (`E0803`) | A `set` that would change the structure of an extern module is refused like a structural param; in a `Dc` sweep that point re-elaborates the instance. | **Approve.** | amended (`RESHAPE`, P6). |
| 5 | `part_vi_io.md` §19 rule 3; `part_iv_behavior.md` §11 rule 12 | Require a constant trip count for `emit!` in an `analog` loop. | **Reject as proposed**: it bends the language to fit a cell. Use an array field with a declared maximum. | rejected; A5 carries a bounded array instead, no language change. |
| 6 | `part_i_values.md` §2 (`Repr`); `part_ii_carriers.md` §5.3.4–§5.3.6 | State the canonical byte form of a value. | **Trim.** Approve two sentences: *all-zero bytes are the all-unknown value*; *an unknown level's value bytes are zero*. Reject "one known byte per level": where the flags live is what `Layout` prints, not a language guarantee. | the two sentences (P3). The byte-per-element choice stays in `Field.known`, an ABI matter. |
| 7 | `part_iii_structure.md` §8.2.3 step 4 | A level of a type with no unknown that still moves is a run error. | **Reject**: the language cannot produce that case; it is D1's own rule. | rejected; the rule lives in D1 only. |
| 8 | `part_iv_behavior.md` §14.4.6 | Give models access to the continuation variables. | **Rework — user decision.** As `simparam` names it contradicts §14.4.1 (they change between solves). Options: (a) two core environment functions `gmin()`, `source_scale()` in the class of `now()`/`freq()`; (b) no language change: independent sources are scaled by the solver, gmin is the solver's shunt only. ngspice parity for junction gmin argues for (a). | (a): the ambient inputs `GMIN` and `SOURCE_SCALE` exist. Under (b) they are deleted and `Term` gains nothing: the solver scales terms it is told are sources — that would need one flag on `Term`. |
| 9 | `part_vi_io.md` §19 rule 1 | No text change: a note that records travel as commits to quiet pins and add no microstep. | **Fine**; belongs in Appendix C, not here. | a note for Appendix C. |

Assumed from earlier decisions and not re-opened: Appendix C.2 and §15.2 (the jump) describe
the solver's present discontinuity algorithm (solver-completeness Q7); §17.1.2 keeps the
present tolerance mapping (Q9).

For the user, in order of weight: **1** (changes what `on settle` counts), **8** (two new core
names or none), **3b** (may co-simulated modules have analog ports), then **6** (two sentences
in Part I). Items 2, 3a, 4, 9 are approved by the critic; 5 and 7 are withdrawn.
