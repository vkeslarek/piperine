# Piperine device ABI — v0

Status: builder draft 0 of the adversarial duel (codegen context C8/O8). Self-contained.
Crate that would hold it: `piperine-abi` (no dependency, `#![no_std]`-able, the only crate a
device links against).

---

## 0. The idea in five sentences

1. **A device is three pure functions over data the solver owns.** `analog` is the `analog`
   block, `define` is `=`, `react` is `on ev { … }`. The device keeps no memory: everything
   that exists over time (unknowns, carriers, pending commits, integrator history, limiter
   history, a reaction's private bytes) lives in the solver.
2. **A pin is the only thing a device and the solver share.** A pin is continuous (one `f64`
   unknown and its equation row), discrete (opaque bytes, one writer) or an event (no value;
   it fires). Ports, internal nodes, branch flows, signals, params, session parameters,
   temperature, timers, located crossings and `settle` are all pins.
3. **`analog` is the analog block with the core primitives of MSL §14 taken out.** Each
   application of a primitive (`+>`, `ddt`, `delay`, `noise`, `ac_stim`, `limit`, `cross`,
   `bound_step`, a named `let`) is one output *slot*; the solver gives every slot its column
   of the per-solve table (§16.2). The solver therefore knows three linear operators
   (1, d/dt, delay) and nothing of the language.
4. **Everything that happens is a pin changing.** A boundary event, a `changed()`, a timer, a
   crossing, a host `set`, `settle`: one mechanism, one sink, one trace.
5. **The host is the writer of every pin no module writes.** There is no parameter call, no
   temperature call, no setup call.

Because the three functions are pure, checkpoint and rollback need **zero** entry points, any
call can be captured and replayed outside the simulator, and the Jacobian can be checked by
finite differences at any time.

---

## 1. Concepts (10)

| # | Concept | One rule | A device that cannot be coupled without it |
| - | ------- | -------- | ------------------------------------------ |
| 1 | **Module** | A named factory inside a versioned library: `create(args)` gives an instance. `args` are what fixes structure. | any (`Diode` with/without `rs`; an AVR with a firmware path) |
| 2 | **Shape** | The instance's complete, immutable description as plain data: its pins, slots, definitions, reactions, record sites, `mem` size. The solver learns everything about a device by reading it; nothing is asked by call. | any; an OSDI model whose nodes collapse per instance |
| 3 | **Pin** | A place where the instance meets a net. *Continuous*: an `f64` unknown plus its equation row. *Discrete*: fixed-size opaque bytes with exactly one writer; it *changes* when its bytes differ. *Event*: no bytes; it changes when fired. A pin the netlist does not connect is private to the instance. | resistor (continuous), flip-flop (discrete), comparator (event) |
| 4 | **Slot** | One output of `analog`: a value `v`, two optional arguments `a`, `b`, and its partial derivatives on a declared pattern. Its *kind* is the core primitive it stands for; the solver applies that primitive's column of §16.2. | capacitor (`Ddt`), diode (`Limit`, `Noise`), comparator (`Cross`) |
| 5 | **Definition** | A pure function from the discrete pins it reads to the discrete/event pins it writes, visible in the same microstep. Run when a pin it reads changes, in dependency order. May be run speculatively and any number of times. | a combinational gate; a bus resolver; a derived param |
| 6 | **Reaction** | A function run once at an instant when a pin it listens to changes. It reads the settled pins (and, if it declares so, the accepted analog solution), and its only effects are commits, records and its `mem`. Run only at committed instants, in increasing `(t, n)`. | flip-flop; AVR emulator |
| 7 | **Commit** | `(pin, byte range, bytes, delay, lane)`: a write that applies at a later instant — the next microstep, or the tick nearest `t + delay`. Transport per lane. | flip-flop (`q <- d`), clock generator, AVR pin write |
| 8 | **Mem** | Private bytes of the instance, owned and copied by the solver, zero when an analysis starts from initial values, read and written only inside `react`. | AVR emulator (registers, SRAM) |
| 9 | **Record** | A typed blob written to a declared site (`emit!`). The solver adds instance, analysis, instant, iteration; drops what a rejected step wrote from a reaction; keeps what `analog` wrote. | any `require`/`finish`; a convergence trace |
| 10 | **Env** | What the solver itself produces and a device may read: time, tick, microstep, Newton iteration, frequency, analysis kind, the equilibrium flag. Nothing the host provides is in it. | a PWL source (`now()`), flicker noise (`freq()`), `CapIc` (`analysis(Static)`) |

What is deliberately **not** a concept, and where it went:

| Usually an ABI concept | Here |
| --- | --- |
| parameter, `set_param`, `param_given` | a discrete pin with role `Param`; the host is its writer; a live `set` is a commit; `p.changed()` and `p.prev()` come free; structural params are `create` args |
| temperature, `simparam`, gmin, source-stepping factor | discrete pins with role `Global`, bound by name to nets the host (or the analog sub-solver, for continuation variables) writes |
| setup / temperature update / start-finish analysis hooks | none; `analog` is a function of its inputs and may memoize on them |
| internal node, branch-current unknown | a private continuous pin |
| drive, `resolve`, `others()` | resolved by elaboration into one-writer pins plus a resolver *device* (C.1.2 "one state slot per drive and a combinational cell"); the ABI has one-writer pins only |
| timer / breakpoint / "next time I want to run" | a commit with a delay to a private event pin |
| crossing callback, `settle` hook, trigger kinds | event pins; the solver fires them |
| `load_dc` / `load_ac` / `load_tran` / noise function / `accept` | one `analog`; the per-solve meaning is the slot kind's |
| checkpoint / restore | none; the device has no memory |
| capability flags | none; an empty table is the capability |
| Jacobian mechanism (O1) | invisible: the contract is "values and exact partials on the declared pattern" (§9) |

---

## 2. Entry points (8)

Device side (6): one exported symbol and five functions in a module's table.

| # | Entry | Called | Contract |
| - | ----- | ------ | -------- |
| 1 | `piperine_abi_1() -> *const Library` | once per `dlopen` | returns a static table; the `1` is the ABI major |
| 2 | `create(module, args, note) -> Created` | elaboration; once per instance | returns the instance and its `Shape`, or a fault |
| 3 | `destroy(instance)` | once | frees instance and shape |
| 4 | `analog(instance, call) -> u32` | every Newton iteration, every accepted solution, every sweep point | pure; reads pins and env, writes slots and partials |
| 5 | `define(instance, k, call) -> u32` | when a pin definition `k` reads changed | pure; writes its output buffers |
| 6 | `react(instance, k, call) -> u32` | when a pin reaction `k` listens to changed, at a committed instant | reads pins, `mem`, optionally the solution; effects only through `commit`, `emit`, `mem` |

Solver side (2), a table handed in with each call:

| # | Callback | Used by | Contract |
| - | -------- | ------- | -------- |
| 7 | `commit(pin, offset, len, bytes, after, lane, flags) -> u32` | `react` | queues a commit (§6) |
| 8 | `emit(site, bytes, flags) -> u32` | `react`, `analog` | writes a record (§7) |

Every function returns `0` or a fault code (an MSL run-error number such as `418`, or a
device-defined code ≥ 0x1_0000); the device may put a message in the call's `note` buffer.
No function may unwind across the boundary.

---

## 3. The shape, exactly

The types below are the ABI. They are `#[repr(C)]` Rust and *are* the C layout
(`cbindgen` prints the header; a C or Zig device uses that). The Rust trait of §8 is a safe
lens over the same structs, not a second interface.

```rust
#[repr(C)] pub struct Str     { pub ptr: *const u8, pub len: usize }            // UTF-8, not NUL-terminated
#[repr(C)] pub struct Bytes   { pub ptr: *const u8, pub len: usize }
#[repr(C)] pub struct List<T> { pub ptr: *const T,  pub len: usize, pub stride: usize }
// `stride` is the element size as the producer compiled it. A consumer reads only the
// fields it knows. Appending a field to an element is therefore a minor-version change.

#[repr(C)] pub struct Library {
    pub size: u32, pub minor: u32,            // sizeof(Library) as built; ABI minor
    pub name: Str, pub build: Str,            // library name; free-form build id (hash of the IR)
    pub modules: List<Module>,
}

#[repr(C)] pub struct Module {
    pub name: Str,
    pub args: List<Arg>,                      // structural arguments `create` accepts (introspection, E0373)
    pub data: *const c_void,                  // the module's own (an interpreter's IR handle, an OSDI descriptor)
    pub create:  unsafe extern "C" fn(*const Module, List<ArgValue>, *mut Note) -> Created,
    pub destroy: unsafe extern "C" fn(*mut Instance),
    pub analog:  Option<unsafe extern "C" fn(*const Instance, *mut Call) -> u32>,
    pub define:  Option<unsafe extern "C" fn(*const Instance, u32, *mut Call) -> u32>,
    pub react:   Option<unsafe extern "C" fn(*const Instance, u32, *mut Call) -> u32>,
}
#[repr(C)] pub struct Arg      { pub name: Str, pub tag: Str, pub default: Bytes }
#[repr(C)] pub struct ArgValue { pub name: Str, pub tag: Str, pub value: Bytes }
#[repr(C)] pub struct Created  { pub instance: *mut Instance, pub shape: *const Shape, pub fault: u32 }
pub enum Instance {}                           // opaque
#[repr(C)] pub struct Note     { pub len: u32, pub text: [u8; 252] }

#[repr(C)] pub struct Shape {
    pub size: u32,
    pub ports: u32,                            // the first `ports` pins are the module's ports, in order
    pub pins:        List<Pin>,
    pub slots:       List<Slot>,               // outputs of `analog`
    pub partials:    List<Partial>,            // the Jacobian pattern of `analog`
    pub analog_reads: List<u32>,               // discrete pins `analog` reads: the D→A boundary
    pub definitions: List<Definition>,
    pub reactions:   List<Reaction>,
    pub sites:       List<Site>,
    pub mem_size: usize, pub mem_align: usize,
}

#[repr(C)] pub struct Pin {
    pub name: Str,
    pub kind: PinKind,                         // Continuous | Discrete | Event
    pub role: PinRole,                         // Port | Param | Global | Private
    pub flags: u32,                            // WRITES (this instance is the writer) | OPTIONAL (a Global that may be absent)
    pub size: u32, pub align: u32,             // Discrete: byte size (Continuous: 8, Event: 0)
    pub tag: Str,                              // layout name: "f64", "bool", "Bit<u8>", …  (§3.2)
    pub unit: Str,
    pub init: Bytes,                           // empty = none. Discrete: first commit at (0,0), or a Param's default.
                                               //               Continuous: Newton's start, and the hold of a transient's OP
    pub abstol: [f64; 2],                      // Continuous: tolerance of the unknown, of its row; NaN = session default
}

#[repr(C)] pub struct Slot {
    pub name: Str,
    pub kind: SlotKind,                        // §4
    pub plus: u32, pub minus: u32,             // target pins; NONE = u32::MAX
}
#[repr(C)] pub struct Partial { pub slot: u32, pub wrt: u32, pub of: Wrt }   // Wrt = Continuous | Real | Source
// of = Continuous: wrt is a continuous pin.   of = Real: wrt is a discrete pin with tag "f64".
// of = Source: wrt is the index of a Noise or Stim slot.

#[repr(C)] pub struct Definition { pub name: Str, pub reads: List<u32>, pub writes: List<u32> }
#[repr(C)] pub struct Reaction   { pub name: Str, pub on: List<u32>, pub reads: List<u32>,
                                   pub writes: List<u32>, pub probes: List<u32> }
#[repr(C)] pub struct Site  { pub stream: Str, pub size: u32, pub fields: List<Field> }
#[repr(C)] pub struct Field { pub name: Str, pub tag: Str, pub offset: u32, pub size: u32 }
```

The one call frame, shared by the three functions (unused members are null):

```rust
#[repr(C)] pub struct Env {
    pub time: f64,            // now(): 0 in an equilibrium solve of a fresh analysis and in a sweep, else t (§16.2)
    pub freq: f64,            // freq(): the sweep point's, else 0
    pub tick: i64,            // the instant's tick; i64::MIN where the time is not a tick (T5)
    pub micro: u32,           // microstep n (react, define)
    pub iter: u32,            // Newton iteration (analog); u32::MAX at an accepted solution
    pub analysis: Analysis,   // Dc | Tran | Ac | Noise
    pub flags: u32,           // STATIC: this is an equilibrium solve / an instant of one (analysis(Static))
}

#[repr(C)] pub struct Call {
    pub size: u32,
    pub want: u32,                    // analog: which partial classes are needed (W_CONTINUOUS | W_SOURCE | W_REAL)
    pub env: Env,
    // ---- inputs, all indexed by pin ----
    pub x:       *const f64,          // value of each continuous pin (analog; react when it has `probes`)
    pub now:     *const *const u8,    // bytes of each discrete pin; null for an absent OPTIONAL pin
    pub prev:    *const *const u8,    // the value before its most recent change (react, define)
    pub changed: *const u8,           // 1 = changed / fired in this microstep (react, define)
    pub last:    *const f64,          // analog, indexed by slot: a Limit slot's `v` at the previous iteration of this
                                      //   solve, or on iteration 0 at the last accepted solution; NaN if there is none
    // ---- outputs ----
    pub slot: *mut SlotOut,           // analog: [slots.len]
    pub jac:  *mut f64,               // analog: [partials.len], pre-zeroed
    pub out:  *const *mut u8,         // define: one buffer per entry of its `writes` (an event pin: 1 byte, nonzero fires)
    pub mem:  *mut u8,                // react
    // ---- effects ----
    pub host: *const Host, pub host_data: *mut c_void,
    pub note: *mut Note,
}
#[repr(C)] pub struct SlotOut { pub v: f64, pub a: f64, pub b: f64 }   // pre-set to (0, NaN, NaN)

#[repr(C)] pub struct Host {
    pub commit: unsafe extern "C" fn(*mut c_void, pin: u32, offset: u32, len: u32, bytes: *const u8,
                                     after: f64, lane: u32, flags: u32) -> u32,      // flags: REPLACE
    pub emit:   unsafe extern "C" fn(*mut c_void, site: u32, bytes: *const u8, flags: u32) -> u32, // flags: FINISH
}
```

### 3.1 Pin roles and binding

| Role | Bound by | If nothing binds it |
| --- | --- | --- |
| `Port` | the netlist, by position (the module's port list) | private |
| `Param` | the netlist, by name (an instance's param net) | a net of its own, written by the host, starting at `init` |
| `Global` | by name to a session net (`temperature`, `settle`, `gmin`, `source_scale`, any `simparam` name) | fault `456` unless `OPTIONAL`; then `now[pin]` is null |
| `Private` | nobody | the solver allocates it: an internal node, a branch flow, a signal, a timer |

The writer of a discrete pin is the one instance that sets `WRITES` on it; if none does, it
is the host. Two writers on one net is a bind error (`220`). Binding a `Param`'s net at
elaboration or before the run is not a change (§5.3.11); a host write during a run is a
commit like any other and fires `changed`.

A continuous pin bound to `gnd` reads 0 and its row is dropped.

### 3.2 Bytes

The solver never interprets a discrete pin: it stores, copies and compares bytes. Three
facts are fixed by the ABI so that this is enough:

1. Storage starts **all zero**. A language with "unknown" encodes all-unknown as all zero
   (MSL: the known mask is the zero-is-unknown side; unknown levels are canonical zeros,
   which also makes §5.3.4–§5.3.6 bitwise).
2. `tag` names the layout. Two pins on one net must agree in `size` and `tag` (bind error
   otherwise). The ABI defines four tags for devices not written in MSL — `"f64"`, `"i64"`,
   `"u8"` (little-endian, always known) and `"bit"` (one byte: 0 unknown, 1 low, 2 high).
   Every other tag is the MSL type name and its layout is MSL Part I §2 (`Repr`).
3. A *level* is a byte range. A commit of some levels is a commit of a byte range.

---

## 4. Slots: the analog contract

`analog` reads `x[pin]` for continuous pins, `now[pin]` for the discrete pins listed in
`analog_reads`, `env`, and `last[]`. It writes every `slot[k]` and every `jac[j]` whose class
is in `want`. `want` is **permission to skip, never a change of meaning**: a device that
ignores it and always writes everything is correct.

`jac[j]` is `∂ slot[partials[j].slot].v / ∂ (the input partials[j] names)`. Inputs are of three
classes: continuous pins (always wanted in Newton), real discrete pins (sensitivities,
adjoint), and *sources* — the `Noise` and `Stim` slots, which as inputs are identically zero
in every solve and exist only to be differentiated against.

| Kind | Is MSL's | `plus`, `minus` | `v` | `a` | `b` | What the solver does with it |
| --- | --- | --- | --- | --- | --- | --- |
| `Add` | `+>` (the part outside any operator) | rows | value | – | – | adds `v` to row `plus`, subtracts from row `minus` |
| `Ddt` | `ddt(x, abstol)` | rows | `x` (a charge, a flux) | `abstol` | – | adds `d/dt v`: 0 in equilibrium, the integrator in a step, `jω` in a sweep; `v` is a state of the jump rule and of the truncation test |
| `Delay` | `delay(x, td, max)` | rows | `x` | `td` | `max` | adds `v(t − a)`: `v` in equilibrium, history in a step, `exp(−jω·a)` in a sweep; keeps the history |
| `Noise` | `noise(psd, name)` | – | `psd` | – | – | a source of density `v` in `Noise`; nothing elsewhere |
| `Stim` | `ac_stim(mag, phase)` | – | `mag` | `phase` | – | a source of phasor `v·exp(j·a)` in `Ac`; nothing elsewhere |
| `Limit` | `limit(x, f, seed)` | – | `x̂` (what the device used) | `x` (what Newton proposed) | – | keeps `v` as `last`; the solve cannot converge while `v` and `a` differ beyond the limiting tolerance; `a = NaN` means "not read this iteration" |
| `Cross` | `cross(e, ttol, etol)` | event pin, – | `e` | `ttol` | `etol` | locates `v` going from `≤ 0` to `> 0` (§17.3) and fires `plus` at the located tick; NaN never crosses |
| `Cap` | `bound_step(dt)` | – | `dt` (+inf: no call executed) | – | – | caps the step after an accepted solution |
| `Var` | a named `let` | – | value | – | – | an operating-point variable, read by the host by name |

Rules, each stated once:

- **A1. Equation.** Row `r` of a continuous pin is
  `Σ Add + d/dt Σ Ddt + Σ Delay = 0` over every slot of every instance targeting `r`. A
  flow contribution `i(a,b) +> x` is `Add{plus:a, minus:b}`. A potential contribution
  `v(a,b) +> x` uses a private continuous pin `f` (the branch flow): `Add{a,b} = x[f]` and
  `Add{f} = x[a] − x[b] − x`. A primitive whose result is used non-linearly is hoisted by
  elaboration onto a private pin `u` with the row `u − op(x) = 0`. So every operator appears
  linearly at the top of a row, which is what lets the solver own it.
- **A2. Switch branches.** Slots and the pattern are the union over kinds; the device writes
  zeros to what the solve's selector disables. Both kinds enabled is fault `418`.
- **A3. Sources.** A noise source read on two paths is one `Noise` slot and two partials
  against it: full correlation is the default, independence is two slots. A source seen
  through an operator (`ddt(c·n)`) is a partial of a `Ddt` slot against it, so the sweep
  applies `jω` with no further rule (§11.1.9). Injection onto a potential branch is a
  partial of that branch's row.
- **A4. Limiting.** The device applies its own limiter: `x̂ = f(x, last)`, using its seed when
  `env.iter == 0 && STATIC` and `last` is NaN or the seed (§18 rules 1, 2, 6). It reports
  values linearized around `x̂` *at* `x` — `v(x̂) + ∂v/∂x·(x − x̂)` — so the solver needs no
  right-hand-side correction of its own (§18 rule 4). At convergence `x̂ = x` and nothing was
  changed.
- **A5. Purity.** Equal inputs give equal outputs. A device may memoize on its inputs; it may
  not remember an iterate. The solver may call `analog` at any point for any reason
  (Newton, an accepted solution, a crossing search along a step, a finite-difference check,
  a replay).
- **A6. Faults.** A non-finite `v` or `jac` is `507`; the solver names the instance and the
  slot. The solver also raises `453` (negative `Delay.a`), `454` (negative `Noise.v` in
  `Noise`), `509` (`Cap`), `510` (`Cross.a/b`) from the values, so devices do not.
- **A7. Multiplicity and `dtemp`** are elaboration's (C.1.1): they are already inside the
  values. An adapter for a foreign model receives them as `Param` pins.

---

## 5. Definitions and reactions: the discrete contract

**Definition `k`** — `define(inst, k, call)`. Reads `now`, `prev`, `changed` for the pins in
its `reads`; writes one buffer per pin in its `writes` (`call.out[j]`, pre-filled with the
pin's current bytes). For an event pin the buffer is one byte; nonzero fires it in this
microstep.

- **D1.** The digital sub-solver levelizes definitions by `reads → writes` and runs a
  definition when a pin it reads changed (and every definition once at the first instant).
  A cycle is relaxed to a bytewise fixed point or fails `501`. (MSL's legal loops among
  resolved drives — relax from empty, `drives + 1` passes, unknown on failure — are one
  definition after elaboration; the solver never sees them as a cycle.)
- **D2.** A definition is pure and writes nothing else. The solver runs definitions on a
  shadow of the store whenever it needs "what would this pin be": the look-ahead of a settle
  commit (§15 S2), the validation of a host `set` against a `where` (a `where` is a
  definition of a `bool` pin).
- **D3. Events are definitions.** `clk.rise()`, `a || b`, the non-probing part of a guard:
  a definition that reads `clk` and fires a private event pin. A reaction listens to that
  pin. This is what makes "does a reaction fire at `(t, 0)`" (§17.2.2) a question the solver
  answers without running the reaction.

**Reaction `k`** — `react(inst, k, call)`. Runs when a pin in its `on` list changed in this
microstep, at most once per microstep (§12.1.15). Reads `now/prev/changed` for `on ∪ reads`,
`x[pin]` for the pins in `probes` (the solution after microstep `n − 1`; in the settle phase
the final one), `env`, `mem`.

- **R1. Snapshot.** Every reaction of a microstep sees the same pins; commits are not visible
  in the microstep that made them (I2).
- **R2. Committed only.** `react` is called only at instants that will not be undone, in
  increasing `(t, n)`, exactly once per firing. The owner runs the discrete side strictly
  behind the accepted analog solution it may read; a rejected analog step discards only
  pure work. This is why `mem` — or a foreign engine hidden behind the instance — is safe.
- **R3. `settle`** is the `Global` event pin of that name, fired once per time point after the
  loop goes quiet. A commit with no advancing delay made by a reaction that ran because of
  it is a settle commit (§15 S1–S7); the solver knows because it fired the pin.
- **R4. Time-point test.** A scheduled time is a time point when, after its commits and
  definitions are applied, a pin in some `analog_reads` changed, or a reaction is about to
  run whose `probes` is non-empty or whose `writes` can reach (through `reads/writes/on`,
  statically) an `analog_reads` pin or a probing reaction. All of it is in the shapes.

### 6. Commit

`commit(pin, offset, len, bytes, after, lane, flags)` from a reaction, on a pin it `WRITES`.

- **C1. When.** `after ≤ 0`, or rounding to no advance: the next microstep. Otherwise the tick
  nearest `t + after` (seconds; T2, T3). `+inf` never comes due. NaN is fault `435`. Where
  time does not advance a positive delay never comes due (§16.1.3).
- **C2. Order.** Commits to overlapping bytes that apply in one microstep are ordered by
  (instant made, `lane`, call order); the last is the value (§12.1.2). `lane` is the
  statement's index in source order, unique in the instance.
- **C3. Transport.** A commit removes the pending commits of the same `(pin, lane, offset,
  len)` due at or after its own due instant (§12.1.4). With `REPLACE` it removes all of them
  (an armed time that moved: `at`, `inertial`, "wake me at").
- **C4. Event pins.** `len = 0` on an event pin: the pin fires when the commit applies. A
  timer is that and nothing else.
- **C5.** The solver writes with the same mechanism: a located crossing is a commit to the
  `Cross` slot's event pin due at `(t, 0)`; a crossing across two rounds or a jump is due at
  the next microstep; a host action is a commit by the host.

### 7. Record

`emit(site, bytes, flags)`; `bytes` has the site's `size` and field layout. The solver adds
the instance path, the analysis, `(t, n)`, and inside `analog` the Newton iteration. `FINISH`
on a record from a reaction ends the analysis at its instant (the device sets it only when
`finish` is known true and not `maybe`). Records from `analog` are traces: delivered at once,
never undone, ignored in a sweep. Records from reactions are never undone either, by R2.
Fields are bytes plus tags, like pins; formatting is the host's.

---

## 8. The Rust side

```rust
pub trait Device: Sized + Send + Sync + 'static {
    fn create(args: &Args<'_>) -> Result<Self, Fault>;
    fn shape(&self) -> &Shape;                                   // owned by self, stable until drop
    fn analog(&self, _cx: &mut Analog<'_>) -> Result<(), Fault> { Ok(()) }
    fn define(&self, _k: u32, _cx: &mut Define<'_>) -> Result<(), Fault> { Ok(()) }
    fn react (&self, _k: u32, _cx: &mut React<'_>)  -> Result<(), Fault> { Ok(()) }
}
pub const fn module<D: Device>(name: &'static str, args: &'static [Arg]) -> Module;  // generic thunks, no macro
```

`Analog`, `Define`, `React` are borrow-checked views of the same `Call` (slices with the
shape's lengths; `x(pin)`, `f64(pin)`, `bytes(pin)`, `slot(k)`, `jac()`, `commit(…)`,
`emit(…)`, `mem::<T>()`). `module::<D>()` instantiates five `extern "C"` thunks that wrap each
method in `catch_unwind` (a panic becomes fault `0xFFFF_FFFF` with the message in `note`).
A library is one hand-written function:

```rust
static LIB: Library = library("stdlib", &[module::<Resistor>("Resistor", &[]), module::<Diode>("Diode", DIODE_ARGS)]);
#[no_mangle] pub extern "C" fn piperine_abi_1() -> *const Library { &LIB }
```

**One path.** The solver calls every device through `Module`'s function pointers, whether the
`Library` came from `dlopen` (generated Rust, a C model, an OSDI adapter, an AVR emulator) or
from a Rust value in the same process (the IR interpreter, a hand-written test element, a
host stimulus). The interpreter is a `Device` whose `Module.data` is the IR of one module.
Generated and external devices are the same thing because there is no other thing.

---

## 9. O1: the Jacobian

The ABI's contract is: *`jac[j]` is the exact partial on the declared pattern.* How a device
gets it is its own business and cannot be observed. Recommendation recorded here because the
duel was asked: generated Rust and the interpreter evaluate `analog` over a dual number with
one tangent per input of the instance's pattern (forward AD; `ddx` is a read of a tangent);
the pattern is the IR's static dependency set. A hand-written or foreign device writes
partials by hand or from its own tables (OSDI's `jacobian_entries` map one to one).

Because `analog` is pure, the solver carries a verifier at no ABI cost: in a debug session
option it perturbs each input, compares finite differences with `jac`, and reports
instance, slot and input by name.

---

## 10. Lifecycle

```
dlopen ─► piperine_abi_1()         check major (symbol) and Library.size/minor
        ─► create(module, args)     per instance, at elaboration  ─► Shape
        ─► bind (solver only)       validate the shape; map pins to nets; allocate private pins, mem,
                                    slot/jac buffers; levelize definitions; build the sparsity pattern
        ─► analyses                 analog / define / react, any number, any order of analyses
        ─► destroy(instance)
```

- An analysis that starts from initial values: the solver zeroes discrete storage and `mem`,
  drops the queue and the analog histories, commits every `init` at `(0, 0)`. No device call.
- An analysis at a transient's last instant (§16.4): the solver keeps all of it. No device call.
- A structural change (a `set` of a structural param): `destroy`, `create` with new args,
  bind again. No recompilation (C7).
- Bind validation fails loud: index out of range, a partial naming a non-existent input, a
  slot target of the wrong pin kind, a `Real` partial on a pin whose tag is not `"f64"`, a
  reaction writing a pin without `WRITES`, a definition and a reaction both writing one pin,
  tag or size mismatch on a net.

---

## 11. Call sequences per analysis

`A(want)` = `analog` on every instance that has slots. `D` = run definitions to a fixed point.
`R` = run the reactions whose `on` pins changed, then apply what is due at the next microstep.

**Operating point / one `Dc` point**

1. First point only: zero, commit `init`s and the host's initial writes at `(0,0)`; `D`.
   Later sweep points: commit what the host changed; `D`.
2. Equilibrium solve: per Newton iteration `A(W_CONTINUOUS)` with `STATIC`, `iter = k`.
   Continuation (gmin, source stepping) varies the `Global` pins `gmin` and `source_scale`
   in the analog sub-solver's copy between solves; the last solve has them at rest.
   Then `A(0)` once at the solution with `iter = u32::MAX`: `Var`, `Cross`, `Limit`.
3. Microstep loop: `R` (reactions with `probes` read that solution), `D`, … If a pin in any
   `analog_reads` changed: back to 2 (a round; bound → `506`); `Cross` slots are compared
   between the two rounds and fire at the next microstep. Bound on microsteps → `501`.
4. Fire `settle`; `R`. Settle commits are due at the next point.

**Transient**: the operating point with every continuous `init` held; then per step

1. Owner picks `t_c` = min(step proposal, smallest `Cap`, stop time, next scheduled time that
   R4 can make a time point).
2. Analog: Newton to `t_c`, `A(W_CONTINUOUS)` per iteration, reading its own copy of the
   `analog_reads` pins with pending settle commits already applied (via `D` on a shadow).
   Not converged or LTE fails on the `Ddt` states → retake smaller. Nothing discrete ran.
3. `A(0)` at the tentative solution: a `Cross` slot that went `≤ 0 → > 0` → locate, shrink
   `t_c` to the tick, retake.
4. Owner runs, in order, the event times in `(t, t_c)`: commits, `D`, `R` — none of them can
   read or move analog. If one creates a time point before `t_c`, analog retakes to it
   (pure work discarded).
5. Accept analog at `t_c`. Histories of `Ddt`, `Delay`, `Limit`, `Cross` advance. Collect what
   is due at `(t_c, 0)` including located crossings; microstep loop `D`/`R`; a changed
   `analog_reads` pin is delivered to the analog sub-solver as a boundary event and the
   integrator restarts on the next step (the solver's present discontinuity algorithm; a
   jump solve, if ever built, is two more solves with `Ddt` read as `(v − v⁻)/h` and no ABI
   change).
6. Fire `settle`; `R`. A `FINISH` record ends the analysis here.

**`Ac`**: operating point; then per frequency `A(W_CONTINUOUS | W_SOURCE)` at the operating
solution with `analysis = Ac`, `STATIC` clear, `freq = f`. The matrix is
`Σ_slots resp(kind, ω, a) · jac`; the right side is `−Σ ∂slot/∂stim_k · v_k·exp(j·a_k)`.
No reaction runs, no pin changes.

**`Noise`**: the same call with `analysis = Noise`; for each `Noise` slot `k` the injection
vector is its column of partials (through each target slot's response), its density is
`slot[k].v`; the output density is `Σ_k |H·b_k|² v_k`.

**Host procedures** use the same calls: `tf`, `pz`, `sp` take the `Ac` linearization and
ignore `Stim` strengths (§14.3.3); `sens` and `adjoint` add `W_REAL` to get `∂F/∂p` for
every real pin; `pss` is continued transients (nothing is reset between them).

**Host actions**: a `set`, a drive, a `force`/`release` are commits by the host on nets; a
probe is a scheduled time that R4 counts. None is a device call.

---

## 12. Worked examples

The examples use the Rust lens and write partials by hand so the contract is visible;
generated code produces the same calls from duals. `node`, `param`, `global`, `input`,
`output`, `event`, `add`, `ddt`, … are plain `const fn` constructors of `Pin` and `Slot`.
`LOW`/`HIGH` are the `"bit"` bytes 1 and 2.

### 12.1 Resistor

```rust
// pins: 0 p, 1 n, 2 r (Param "f64" = 1e3)        slots: 0 Add{p,n} "i"
// partials: (0, p), (0, n), (0, Real r)
fn analog(&self, cx: &mut Analog) -> Result<(), Fault> {
    let (v, r) = (cx.x(P) - cx.x(N), cx.f64(R));
    cx.slot(I).v = v / r;
    cx.jac().copy_from_slice(&[1.0 / r, -1.0 / r, -v / (r * r)]);   // the third is read only under W_REAL
    Ok(())
}
```

No `create` argument, no state, no other function. A live `set` of `r` is a host commit to
the net of pin 2; `r` is in `analog_reads`, so the solver lands there and restarts.

### 12.2 Diode with limiting and noise

```rust
// pins: 0 a, 1 k, 2 is, 3 n, 4 cj0 (Params), 5 temperature (Global "f64")
// slots: 0 Limit "vd" | 1 Add{a,k} "id" | 2 Ddt{a,k} "qj" | 3 Noise "shot" | 4 Var "gd"
// partials: (1,a) (1,k) (2,a) (2,k) (1, Source 3)
fn analog(&self, cx: &mut Analog) -> Result<(), Fault> {
    let nvt   = cx.f64(NF) * K_B * cx.f64(TEMP) / Q;
    let is    = cx.f64(IS);
    let vcrit = nvt * (nvt / (SQRT_2 * is)).ln();
    let v     = cx.x(A) - cx.x(K);
    let old   = cx.last(VD);
    let old   = if cx.env().iter == 0 && cx.is_static() || old.is_nan() { vcrit } else { old };   // the seed
    let vd    = pnjlim(v, old, nvt, vcrit);
    *cx.slot(VD) = SlotOut { v: vd, a: v, b: f64::NAN };

    let e  = (vd / nvt).exp();
    let id = is * (e - 1.0);
    let gd = is * e / nvt;
    let cj = cx.f64(CJ0);
    cx.slot(ID).v   = id + gd * (v - vd);          // A4: linearized around vd, reported at v
    cx.slot(QJ).v   = cj * vd + cj * (v - vd);
    cx.slot(SHOT).v = 2.0 * Q * id.abs();          // read by the solver only in Noise
    cx.slot(GD).v   = gd;
    cx.jac().copy_from_slice(&[gd, -gd, cj, -cj, 1.0]);
    Ok(())
}
```

The solver holds `vd` between iterations, vetoes convergence while `|vd − v|` exceeds the
limiting tolerance, advances it on accepted steps and restores it on rejected ones. Flicker
noise is a second `Noise` slot whose `v` divides by `cx.env().freq`.

### 12.3 Capacitor

```rust
// pins: 0 p, 1 n, 2 c (Param)      slots: 0 Ddt{p,n} "q"      partials: (0,p) (0,n)
fn analog(&self, cx: &mut Analog) -> Result<(), Fault> {
    let c = cx.f64(C);
    cx.slot(0).v = c * (cx.x(P) - cx.x(N));
    cx.jac().copy_from_slice(&[c, -c]);
    Ok(())
}
```

The device says "this is a charge". Equilibrium, TR-BDF2, the truncation test on that state,
`jω·c` in a sweep, the history across a rejected step: all the solver's.

### 12.4 D flip-flop

```rust
// pins: 0 clk (Port "bit"), 1 d (Port "bit"), 2 q (Port "bit", WRITES, init = [LOW])
// reactions: 0 "on clk.rise()"  on:[clk] reads:[d] writes:[q]
fn react(&self, _k: u32, cx: &mut React) -> Result<(), Fault> {
    if cx.bytes(CLK) == [HIGH] && cx.prev(CLK) == [LOW] {
        cx.commit(Q, cx.bytes(D), 0.0, /*lane*/ 0)?;      // q <- d : applies at (t, n+1)
    }
    Ok(())
}
```

`d` is read as settled at the start of the microstep; the commit is invisible until the next
one. Nothing analog reads `q`, so no clock edge is ever a time point.

### 12.5 Comparator: analog → event

```rust
// pins: 0 inp, 1 inn (continuous), 2 out (Port "bit", WRITES, init=[LOW]), 3 up, 4 down (Private Event)
// slots: 0 Cross{plus: up} | 1 Cross{plus: down}        partials: (0,inp) (0,inn) (1,inp) (1,inn)
// reactions: 0 on:[up] writes:[out] | 1 on:[down] writes:[out]
fn analog(&self, cx: &mut Analog) -> Result<(), Fault> {
    let e = cx.x(INP) - cx.x(INN);
    cx.slot(0).v =  e;
    cx.slot(1).v = -e;
    cx.jac().copy_from_slice(&[1.0, -1.0, -1.0, 1.0]);
    Ok(())
}
fn react(&self, k: u32, cx: &mut React) -> Result<(), Fault> {
    cx.commit(OUT, if k == 0 { &[HIGH] } else { &[LOW] }, 0.0, k)
}
```

The analog sub-solver sees slot 0 go positive inside a step, retakes the step to the located
tick, and commits a firing of `up` at `(t, 0)`. The digital sub-solver sees an event pin
fire. Neither knows the other exists. The partials of a `Cross` slot are what the secant
locator and, later, `dt*/dp` (§23.3.2) use.

### 12.6 DAC: event → analog

```rust
// pins: 0 code (Port "u8"), 1 out (continuous), 2 vref (Param), 3 f (Private continuous: the branch flow)
// analog_reads: [code, vref]
// slots: 0 Add{out} "i" | 1 Add{f} "eq"            partials: (0,f) (1,out) (1, Real vref)
fn analog(&self, cx: &mut Analog) -> Result<(), Fault> {
    let k = cx.bytes(CODE)[0] as f64 / 255.0;
    cx.slot(0).v = cx.x(F);                               // KCL at out
    cx.slot(1).v = cx.x(OUT) - k * cx.f64(VREF);          // v(out) = k·vref
    cx.jac().copy_from_slice(&[1.0, 1.0, -k]);
    Ok(())
}
```

When a commit changes `code`, the owner delivers one boundary event (pin, bytes) to the
analog sub-solver, which lands on that time, updates its copy and restarts the integrator.

### 12.7 An AVR emulator (external, own time base)

```rust
// create args: firmware: "str" (path), f_cpu: "f64"
// pins:  0..8   pb0..pb7_in  (Port "bit")            what the pins read
//        8..16  pb0..pb7_out (Port "bit", WRITES)    what the MCU drives (to a resolver or a D2A)
//        16 adc0 (continuous)        17 reset (Port "bit")
//        18 wake (Private Event, WRITES)   19 sample (Private Event, WRITES)
//        20 adc_val (Private "f64", WRITES)
// mem:   size_of::<Cpu>()   — registers, SRAM, timers, last tick; plain data, no pointers
// reactions: 0 "run"    on:[pb*_in, reset, wake, adc_val] writes:[pb*_out, wake, sample]
//            1 "sample" on:[sample] probes:[adc0] writes:[adc_val]
fn react(&self, k: u32, cx: &mut React) -> Result<(), Fault> {
    if k == 1 { return cx.commit(ADC_VAL, &cx.x(ADC0).to_le_bytes(), 0.0, 100); }
    let now = cx.env().tick;
    let (cpu, io) = cx.mem_and_io::<Cpu>();                 // &mut Cpu, and the rest of cx
    if cpu.fresh() { cpu.reset(&self.flash); }              // mem is zero at the start of an analysis
    cpu.set_inputs(io.bits(0..8), io.bytes(RESET));         // pin-change interrupts are raised in here
    if io.changed(ADC_VAL) { cpu.adc_done(io.f64(ADC_VAL)); }
    // run until the present, then ahead to the next instruction that reads a pin or sleeps
    for ev in cpu.run(now, &self.flash) {
        match ev {
            Out { pin, level, dt } => io.commit(8 + pin, &[level], dt, pin as u32)?,   // transport: edges keep their times
            AdcStart { dt }        => io.fire(SAMPLE, dt, 101)?,                         // commit, len 0
        }
    }
    io.fire_replace(WAKE, cpu.next_wake_dt(), 102)                                       // "wake me at": REPLACE
}
```

What each requirement became:

| The emulator needs | It is |
| --- | --- |
| its own time base | `mem.last_tick` and `env.tick`; it is called only at instants it asked for or at an input change |
| "advance to a time" | the `wake` event pin fired by its own delayed commit |
| interrupts on pins | `on: [pb*_in]` |
| write pins at exact times | commits with `after = dt` |
| read an analog pin | a second reaction with `probes`, so that only ADC samples are analog time points, not every wake |
| never be rolled back | R2; and its state is `mem`, so a session can still be snapshotted and a trace replayed |
| firmware, clock | `create` args, held in the immutable instance |
| inspection | `pb*_out`, `adc_val` are nets the host probes by path; registers can be `emit`ted |

A co-simulated RTL block (`extern "cosim"`, §8.5) is the same shape with no `probes`; an
engine whose state cannot live in `mem` (a foreign process) keeps it behind the instance and
re-initializes when it sees zeroed `mem` — R2 is the only guarantee it needs.

---

## 13. Safety, versioning, debugging

**Ownership and lifetime**

- Every pointer in a `Call` is valid for the duration of that call only. A device retains
  none. Buffers have the lengths of the instance's shape; the solver allocated them.
- `Library` and everything it points to live until the process ends; libraries are never
  unloaded. An instance's `Shape` and strings live until `destroy`.
- The instance is immutable after `create` (`*const Instance`). `mem` is the only writable
  private memory and belongs to the solver.

**Threads**

- No two calls on one instance overlap. Calls on different instances may run on different
  threads (`Device: Send + Sync`); `analog` over instances may be parallel. A library has no
  mutable global state. `commit`/`emit` are called only from inside the call that received
  the `Host`, on its thread.

**Undefined behavior the ABI closes**

- No unwinding across the boundary: thunks `catch_unwind`; a C device returns a code.
- No Rust type crosses: only `repr(C)` structs, integers, `f64`, raw pointers.
- The solver validates the shape at bind (§10) and, in debug sessions, poisons output
  buffers with signaling NaN and guard words to catch a device writing past them or leaving
  a wanted entry unwritten.
- A device does not change the floating-point environment and installs no signal handler.

**Versioning**

- Major = the symbol name (`piperine_abi_1`). A solver looks for the majors it speaks.
- Minor = `Library.minor`. A minor version may append fields to any struct (`size` on
  `Library`/`Shape`/`Call`, `stride` on every `List`), add a `SlotKind`, a flag or a `Global`
  name. An unknown `SlotKind` from a newer device is a bind error naming it; unknown trailing
  fields are ignored.
- `Library.build` identifies the artifact (hash of the IR and the transpiler) for the cache
  of C7 and for bug reports.

**Debugging a misbehaving device**

1. *Read it.* The shape names every pin, slot, definition, reaction and site; `piperine
   inspect lib.so Module` prints it. The generated Rust is ordinary source with debuginfo:
   a breakpoint in `analog` works.
2. *Tap it.* Every call is plain data in, plain data out. A session option records calls of
   chosen instances (`Env`, inputs, outputs, commits, records) to a file.
3. *Replay it.* Purity (A5, D2) and solver-owned `mem` make a recorded call a unit test: the
   same inputs through the compiled device and through the interpreter must agree. That is
   the differential oracle of C6, available per call and per instance, not only per run.
4. *Check it.* Finite-difference verification of `jac` (§9). NaN → `507` with the slot's name.
   A convergence failure names the `Limit` slots still active and the rows out of tolerance.
5. *Watch it.* Every pin change, commit (with instance, reaction, lane, due instant),
   crossing and record goes through the owner's one sink, in `(t, n)` order.

---

## 14. Coverage of the brief

| Must couple | Mechanism |
| --- | --- |
| contributions, charges/`ddt`, `idt`, delays | `Add`, `Ddt`, `Delay` slots; `idt` is stdlib over `Ddt` and a private pin |
| noise incl. flicker and correlation | `Noise` slot as a differentiable source; `env.freq` |
| AC stimulus | `Stim` slot |
| limiting | `Limit` slot + `last` + A4 |
| internal unknowns | private continuous pins |
| `bound_step`, crossings | `Cap`, `Cross` slots |
| operating point, continuation | `STATIC`, `Global` pins `gmin`/`source_scale`, `init` holds |
| definitions, reactions, drives, resolve | definitions, reactions, one-writer pins, resolver device |
| commits with transport, microsteps | §6, R1 |
| the mixed boundary | `analog_reads` (D→A), `Cross` → event pin (A→D), `probes` |
| DC/OP, AC, noise, transient, sweeps, tf/pz/sp/sens/adjoint/pss | §11 |
| names, params, live changes | shape; `Param` pins; host commits |
| checkpoint/rollback | no device memory; R2 |
| external devices with their own time base | §12.7 |
| versioning, `cdylib` safety, debuggability | §13 |

**Counts.** Concepts: 10. Entry points: 8 (6 device, 2 solver).

## 15. Known weak points (for the critic)

1. R2 spends the solver's licence to speculate on reactions (§15 I8 allows speculation) to
   make reactions effectful-but-safe. If the owner must speculate, `mem` already makes pure
   devices rollbackable, but a foreign-engine device then needs a declared exception.
2. Params-as-pins means a device that precomputes heavily from params (BSIM) must memoize
   inside `analog` keyed on the bytes of its `Param`/`Global` pins; there is no "params
   changed" call and no cheap change stamp in `Env` yet.
3. `SlotKind` has nine members. They are exactly MSL's core primitives, but nine kinds under
   one concept is the place a critic will count.
4. `REPLACE` on commit and event pins of size 0 are two small extensions of "bytes with
   transport" introduced for timers.
5. Gradients through definitions (a derived param) and across events (§23.3) are not carried;
   `W_REAL` gives `∂F/∂pin` only.
