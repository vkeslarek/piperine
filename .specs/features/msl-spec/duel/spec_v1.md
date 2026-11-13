# Mixed-signal language: specification (v1)

Nov 12, 2026 · round 1 of the spec duel, answering `critique_v0.md`

A language for mixed-signal models, similar to Rust and based on it, but not Rust. It models, simulates and optimizes digital, analog and mixed circuits, and every outcome is defined by the source: there are no races. It reuses existing models: Verilog-A compiled to OSDI, and other simulators by cosimulation (8.5); importing Yosys netlists is planned (G.8). Testbenches live in the host (Part VIII).

## 0. Concepts

The language has ten concepts. Each has one rule; the third column lists everything the rule covers, so nothing hides behind a name.

| # | Concept | Rule | Covers | § |
| --- | --- | --- | --- | --- |
| 1 | value and `fn` | data with no time, every level known or unknown; a function is pure and places no instance | `Repr`, the known flag, closures, `Event`-returning functions | 1–3 |
| 2 | nature | says what a carrier holds and how its drivers combine | `Bit` (one driver), resolved natures (`resolve`), continuous natures (KCL) | 4 |
| 3 | carrier | a signal or port of a nature: exists over time, can be connected | discrete carriers, nodes, branches, bundles (a `struct` of carriers) | 5–7 |
| 4 | param | a value fixed during a solve; the model reads it, the host sets it | defaults, `where`, structural params, templates as params | 8.1, 9 |
| 5 | module | ports, params, signals, instances, `=` and `let` | generation, `extern` modules, named expressions | 8 |
| 6 | instance | a module placed; value inputs and a result are expressions read in place | instances inside expressions, templates, `m`, `dtemp`, core modules (`ddt`, `cross`, …) | 8.3–8.5 |
| 7 | `analog for M` | equations, written with `+>`, evaluated at every Newton iteration | switch branches, small-signal and noise | 11 |
| 8 | `digital for M` | reactions, `on ev { q <- x; }`, run when an event fires | commits, `after` | 12 |
| 9 | event | a value of type `Event`: primitives, `\|\|`, guards | `changed`, `at`, `cross`, `init`, `settle`, `final` | 13 |
| 10 | host | drives, sets, observes; the model's only output is `emit!` | stimulus, checks, Monte Carlo, optimization | 19–23 |

**State is structure.** What keeps state is a signal declared in a module, and nothing else: a function never keeps state, and anything with state is a module.

**Two kinds of callable.** A `fn` is pure. A `module` is an instance. There is no third kind: every operator that keeps state (`ddt`, `delay`, `cross`, `idt`, `transition`, …) is a module with a result, core or stdlib (8.3).

### Who writes what

| Form | Target | Written in | Takes effect |
| --- | --- | --- | --- |
| `a = b;` | two nets (nodes, or carriers of a resolved nature): one net | module body | at elaboration |
| `y = expr;` | a discrete carrier: this module's drive of it, a combinational definition | module body | in the microstep its inputs change, before reactions run (8.2) |
| `branch +> expr;` | a branch: its potential or its flow | `analog` | in the current Newton iteration |
| `q <- expr [after dt];` | a discrete carrier: a commit to this module's drive | a reaction | at the next microstep, or `dt` later |

`let x = expr;` in a module body writes nothing: it names an expression (8.2). Params have no assignment form: models read them, the host sets them.

### How a model reads

```rust
/// A comparator with hysteresis that drives a flip-flop.
module Front(vin: Electrical, in clk: Bit, out q: Bit) {
    vth: f64 = 0.6;
    hyst: f64 = 50m;
    signal above_th: Bit = false;        // state: a signal
    signal d: Bit;
    d = above_th;                        // structure: a combinational definition
}

digital for Front {
    on above(v(vin) - vth - hyst / 2.0) { above_th <- true; }    // an analog event
    on above(vth - hyst / 2.0 - v(vin)) { above_th <- false; }
    on clk.rise() { q <- d; }                                    // a digital event
}
```

The module body is the netlist: ports, params, two signals, one definition. Behavior is in the `digital` block: three reactions, two on crossings of the input and one on the clock. `above` is a stdlib module with a result of type `Event` (22.2); each use is an instance. Nothing in the model keeps state outside its signals.

## Part I. Values

Values are data with no time. Everything a model computes with is a value; what it connects to is a carrier (Part II). The value world follows Rust's, with the differences stated here.

### 1. Value types

| Kind | Types | Notes |
| --- | --- | --- |
| boolean | `bool` |  |
| integer | `UInt<N>`, `Int<N>` (`u1` … `u128`, `i1` … `i128`) | any width, a const expression (`UInt<{A + 1}>`); arithmetic wraps |
| float | `Float<N>`, `N` in {16, 32, 64, 128} (`f16` … `f128`) | IEEE 754; analog code computes in `f64` |
| string | `&'static str` | literals only; never carried |
| composite | `[T; N]`, tuples, `struct`, `enum`, `Option<T>` | as in Rust |

1. `as` truncates or extends; `checked_*` and `saturating_*` work as in Rust.
2. Left out, because a model is fixed at elaboration: mutable references, heap types, `unsafe`, threads, `async`, `dyn`. `&` appears only as a shared, read-only borrow of an array, slice or string in a function parameter.
3. Constants are `const` items; the prelude has `Q`, `K_B`, `EPS0`, `PI`, `E`, `inf`.
4. Only a type with `Repr` can be carried.

### 2. `Repr`

`Repr` lays a value type out on levels. It is the only bridge from values to carriers.

```rust
pub trait Repr: Copy {
    const WIDTH: u32;                                   // levels occupied
    fn encode(self) -> [bool; Self::WIDTH];
    fn decode(levels: [bool; Self::WIDTH]) -> Option<Self>;
}
```

1. `bool` is one level. Integers and floats are their bit pattern, least significant first; arrays and tuples concatenate; `Option<T>` is a tag level then `T`.
2. `#[derive(Repr)]` on a struct concatenates the fields, first field lowest; on an enum, a tag then the widest payload. `decode` returns `None` for an unused tag, which reads as unknown (3.3). `#[repr(uN)]` and explicit discriminants choose the tag; any other encoding (one-hot, Gray) is a hand-written `impl Repr`. Behavior never depends on the encoding.

```rust
#[derive(Repr, Clone, Copy, PartialEq)]
enum Tx { Idle, Start, Data { n: u3 }, Stop }      // 2-level tag + 3-level payload

#[derive(Repr, Clone, Copy, PartialEq)] #[repr(u2)]
enum Op { Add = 0b00, Sub = 0b01, And = 0b10, Or = 0b11 }

#[derive(Repr, Clone, Copy, PartialEq)]
enum Pam4 { L0, L1, L2, L3 }                        // a PAM4 symbol is a value; its voltage is analog
```

### 3. Functions

A function is computation without time. It is pure: it keeps no state, writes no carrier and places no instance. What it reads decides where it may be called.

| The body… | Callable in |
| --- | --- |
| uses values and reads discrete carriers | anywhere |
| probes (`v(a)`, `i(br)`) or reads the environment (`now()`, …) | `analog`, event expressions, reaction bodies, body `let`s, value inputs of instances |
| returns `Event` (13) | event positions; as `bool` inside a reaction |

1. Inside `analog`, calls are differentiated through.
2. `extern "C" fn` is implemented outside; inside `analog` it needs `#[derivative(f)]`.
3. Closures `|a, b| expr` are values and may be passed to functions; they capture by value.
4. Something with state is a module with a result (8.3), never a function. A function cannot call one: `fn f(x: f64) -> f64 { ddt(x) }` is an error; write `module f(x: f64) -> f64 { ddt(x) }`.

```rust
pub fn diode_current(vd: f64, is: f64, nvt: f64) -> f64 { is * ((vd / nvt).exp() - 1.0) }
pub fn handshake(req: Bit, ack: Bit) -> Event { req.rise() && ack }
#[derivative(erfcx_dx)] extern "C" fn erfcx(x: f64) -> f64;
```

#### 3.1 `impl`

`impl X { … }` attaches functions to a type. It holds functions only, so it has no time.

| `impl` on | `self` is | Typical use |
| --- | --- | --- |
| value type | the value | methods, trait implementations |
| a nature | one carrier, read through a handle | events: `rise`, `fall` |
| a bundle (7) | the bundle | events and queries over several carriers |

1. On a nature or a bundle, `self` is a handle that reads, never a copy.
2. Generic names follow `impl`; bounds come from the declaration (`impl<T> Stream<T>`).
3. A derived nature inherits its base's functions and may add new ones, never redefine them.
4. Names may not repeat a carrier method: `changed`, `prev`, `since`, `others`, `inflow`.

There is no `impl` on a module: what a module's blocks share is a `let` in its body (8.2).

```rust
impl Thermal {
    pub fn celsius(self) -> f64 { temp(self) - 273.15 }
}
```

#### 3.2 Parameters and calls

One rule covers functions, methods, core primitives and instances; there is no overloading, no variadic call and no default value in a signature.

1. Arguments are positional.
2. Trailing parameters of type `Option<T>` may be left out; a left-out parameter is `None`.
3. An argument of type `T` passed for a parameter of type `Option<T>` is `Some` of it. This is the only implicit conversion in the language.
4. After the positional arguments, optional parameters may be passed by name, `name: value`, which lets a call skip earlier ones.
5. The callee decides what `None` means, usually with `unwrap_or`.

```rust
cross(e)                    // rising, simulator tolerances
cross(e, 1p)                // with ttol
cross(e, etol: 1u)          // skips ttol
idt(x)                      // an integrator, no initial condition, no reset
idt(x, 1.0, reset: rst)
```

Module params follow the same rule at an instance, `Name(ports) { name: value }`. A module param may also have a default, `r: f64 = 1k`, because the host lists and sets params (21).

#### 3.3 Unknown values

Every level of every value is **known** or **unknown**. Unknown is not a third level: it is the record that the level is not established. One rule covers carriers, locals, function arguments, closures and library code alike, because the flag travels with the value.

1. **Sources.** Literals, params, constants, probes and the environment are known. Unknown enters only through a carrier with no initial value, the literal `Unknown` (all levels unknown, of any `Repr` type), or a `decode` that returns `None`. A value of a type without `Repr` has one flag for the whole value.
2. **Operators.** `&`, `|`, `^`, `!` work per level: `unknown & false == false`, `unknown | true == true`, otherwise unknown. Field access, indexing by a known index, shifts by a known amount, `cast` and the built-in and derived `encode`/`decode` move levels with their flags. Every other operator (arithmetic, comparisons, `as`, a shift by an unknown amount, an `extern fn`) is all-or-nothing: one unknown level in any operand makes the whole result unknown.
3. **Control flow.** When what decides control is unknown (an `if` or `while` condition, a `match` scrutinee, a guard, a short-circuit operand, a loop bound, an index), every path the known levels do not exclude is taken, and **every effect is merged level by level**: equal known levels are kept, the rest become unknown. Effects are the value yielded, each local written, and each commit; a path that does not commit to a carrier counts as committing the value the carrier holds when the commit applies. A loop whose exit is unknown stops there, and everything its body could write becomes unknown. An `emit!` reached this way carries `maybe = true`.
4. **Reading the flag.** `x.level(k)` reads level `k` of any `Repr` value as a known `Option<bool>`: `None` when unknown. The stdlib builds `x.known() -> Option<T>` (all levels known, or `None`), `x.known_or(d)`, `x.is_known()`, `same(a, b)` and `merge(a, b)` on it (22.1).
5. **`analog` has no unknowns.** The solver computes with known reals. In an `analog` block, and in every `let` and value input an `analog` block reads, a discrete carrier is read only through `level`, `known` or `known_or`, after field access and indexing at most. Anything else is an elaboration error naming the carrier. The model says what an unknown means electrically; the language does not guess.

Rule 3 has no exceptions. `q <- if c { a } else { b };` and `if c { q <- a; } else { q <- b; }` are the same; `if c { s += 1; }` on a local is the merge of `s` and `s + 1`; an exhaustive `match` on an unknown enum merges all its arms; `iter().filter(|d| d.oe).count()` over an unknown `oe` is unknown, because `filter` is written with an `if`.

```rust
on fref.rise() || fvco.rise() {          // state has no initial value here
    let mut s = state;                   // s: i2, all levels unknown
    if fref.rise() && s < 1 { s += 1; }  // condition unknown: s = merge(s, s + 1) = unknown
    state <- s;                          // stays unknown until something resets it
}
```

This is stricter than Verilog's `if`, which takes the `else` branch on an unknown condition and hides the unknown. A model that wants that writes it: `if c.known_or(false) { … }`.

#### 3.4 Files and imports

One file is one module of the namespace, `mod` declares submodules, `use` brings names into scope, `pub` controls visibility, and a package of models is a crate. The standard library prelude is imported implicitly. A hardware module and all its behavior blocks are in one file.

## Part II. Carriers

A carrier exists over time and can be connected. Every port and internal signal is a carrier, and its type is a **nature**.

### 4. Natures

**A nature says what its carriers hold and how the drivers of one carrier combine.**

|  | discrete, no `resolve` | discrete with `resolve` | continuous, with `flow` | continuous, no `flow` |
| --- | --- | --- | --- | --- |
| declared | `nature N { value: T }` | `nature N { value: T, resolve: f }` | `nature N { potential: …, flow: … }` | `nature N { potential: … }` |
| holds | a `T: Repr`, level by level | the same | a real potential and flow; complex in small-signal analyses | a real potential |
| drivers combine | one driver | `f(&[T]) -> T` over all drives | flows sum to zero (KCL) | one driver |
| port direction | `in` or `out` | none | none | none |
| computed by | the event engine | the event engine | the analog solver | the analog solver |
| written by | `=`, `<-` | `=`, `<-` | `+>` | `+>` |
| past | `prev()`, `since()` (5.3) | the same | located events, modules with state | the same |

```rust
nature Bit<T: Repr = bool> { value: T }                              // prelude; Bit is Bit<bool>
nature Wire { value: Logic, resolve: Logic::wired }                  // stdlib (22.3): strengths, high impedance
nature Electrical {
    potential: Voltage { access: v, units: "V", abstol: 1u },
    flow:      Current { access: i, units: "A", abstol: 1p },
}
nature Thermal {
    potential: Temperature { access: temp, units: "K", abstol: 1m },
    flow:      Power       { access: pwr,  units: "W", abstol: 1n },
}
nature VSignal { potential: Voltage { access: v, units: "V", abstol: 1u } }   // signal-flow
nature HighVoltage: Electrical { potential: Voltage { abstol: 1m } }          // derived
```

1. A discrete nature declares `value` and optionally `resolve`. A continuous nature declares `potential` and optionally `flow`. A derived nature overrides attributes of its base. Discrete and continuous carriers never connect (6.1).
2. `resolve` is a pure function of the drives, in no particular order: it must be commutative and associative, and it is called with an empty slice for a carrier nobody drives. High impedance, strengths, wired logic and summing nets are value types and `resolve` functions in the stdlib (22.3), not language features.
3. `access` names the probe; it is an alias of the generic probes `potential` and `flow` (6). `units` is metadata, never checked. `abstol` is the tolerance of that quantity's unknowns and residuals, and the default `etol` of crossings.
4. Choose each `abstol` below the smallest difference that matters. A user state is best normalized to order one (stdlib nature `State`, 22.2).
5. Quantity names are labels: two natures whose potentials share a name may connect.

### 5. Discrete carriers

A discrete carrier holds a value of its nature's type and changes only at events. `Bit<T>` is the common one.

#### 5.1 Declaring and reading

```rust
signal en:    Bit;                       // starts unknown
signal count: Bit<u8> = 0;
signal level: Bit<f64> = 0.0;            // real-number modeling: piecewise constant
signal st:    Bit<Tx> = Tx::Idle;
signal lines: [Bit; 8];                  // eight carriers, each with its own events
signal bus:   Bit<u4> = logic!("10xx");  // level by level; x is unknown
signal sda:   Wire;                      // resolved: any number of drivers
```

1. Reading a carrier yields its value, with its known flags (3.3). Without `= value`, every level starts unknown; `q <- Unknown;` commits an unknown value.
2. A discrete carrier is constant during every Newton solve and has no derivative and no small-signal response (11.1).
3. An array of carriers reads as an array value; storage and history of an element exist from its first commit, so `[Bit<u8>; 1 << 32]` costs what is written.

#### 5.2 Drives

Each module that writes a carrier holds one **drive** of it: either one `=` definition (8.2) or the commits of its reactions, never both. The carrier's value is what its nature makes of the drives:

- **No `resolve`:** exactly one module holds a drive, the one that declares the carrier or has it as an `out` port. A second is an elaboration error. The value is the drive.
- **With `resolve`:** any module that sees the carrier may hold a drive. The value is `resolve` of all drives, recomputed in the microstep any drive changes. Readers, `changed()` and history see the resolved value. `s.others()` reads `resolve` of every drive but this module's own, which is how a bidirectional element looks through itself (22.3).

`= value` on a signal or port is the initial value of the declaring module's drive; a carrier's initial value is given once, by its driver. A drive with no initial value starts unknown; a module that never writes a carrier has no drive of it.

#### 5.3 History

Every discrete carrier, and every param, keeps one previous value and one time:

| Method | Gives |
| --- | --- |
| `s.changed()` | `Event`: `s` changed in this microstep |
| `s.prev()` | the value before the most recent change; the initial value before the first |
| `s.since()` | the time since the most recent change, or since the analysis began; seconds, an exact multiple of the tick (15) |

A node has no `prev()`: a value one step ago would depend on the step size. Its past is reached through located events and modules with state. Longer memory is a signal written by a reaction.

### 6. Continuous carriers

A node of a continuous nature; the solver computes its value. A **branch** runs between two nodes; models contribute to it and probe it.

```rust
signal mid: Electrical;          // internal node
signal out: Electrical = 0.0;    // initial condition of its potential (SPICE ic)
```

| Probe | Reads |
| --- | --- |
| `potential(a, b)`, `flow(a, b)` | the generic probes, for code generic over a nature |
| `v(a, b)`, `v(a)` | the nature's alias of `potential`; `b` is an `Option`, `gnd` when left out |
| `i(a, b)`, `i(a)` | the alias of `flow`, from `a` to `b` through the module |
| `v(br)`, `i(br)` | a named branch, `let br = branch(a, b);` in the module body |
| `p.inflow()` | flow entering the module at its port `p` (VAMS `I(<p>)`) |

1. `gnd` is the reference node of every continuous nature. It is not a global net: a model reaches any other node only through its ports.
2. A signal-flow node has exactly one potential driver, checked at elaboration.
3. Two independent branches between the same nodes need names.
4. Every probe has the signature `fn potential<T: Terminals>(a: T, b: Option<T>) -> f64`; nodes and named branches implement `Terminals`.
5. `i(p)` is the flow of this module's branch `(p, gnd)`; `p.inflow()` is the total flow of all this module's branches at `p`.

#### 6.1 Crossing between kinds

A discrete carrier is never connected to a node, and no converter is inserted implicitly: crossing is a module (stdlib `D2A`, `A2D`, `Bidir`, 22.4). Inside a module, behavior crosses freely: `analog` reads discrete carriers (3.3 rule 5), and events and reactions probe.

### 7. Bundles

A `struct` whose fields are carriers is a **bundle**: a carrier type, not a value. Fields of a one-driver nature have directions, written from the producer's side; other fields have none. A bundle has no behavior with time, and may have functions.

| Use | Written |
| --- | --- |
| producer view | `tx: Stream<T>` |
| consumer view, every direction reversed | `rx: flip Stream<T>` |
| observer view, every field an input | `in s: Stream<T>` |
| internal bundle | `signal a: Stream<T>;` |
| connection | a bundle carrier, or a literal `Cell { wl: wl[r], bl: bl[c] }` |

```rust
struct Stream<T: Repr> { out valid: Bit, in ready: Bit, out data: Bit<T> }
struct Diff { p: Electrical, n: Electrical }
struct Cell { in wl: Bit, bl: Electrical }

impl<T> Stream<T> {
    pub fn beat(self, clk: Bit) -> Event { clk.rise() && self.valid && self.ready }
}
impl Diff {
    pub fn vd(self) -> f64 { v(self.p, self.n) }
}
```

## Part III. Structure

Structure is what elaboration turns into a fixed netlist. It includes combinational logic: a definition `y = expr;` is a wire with a function on it, as an `assign` is in Verilog and a gate is in a Yosys netlist.

### 8. Modules

A **module** is ports, params and a netlist. Its time-dependent behavior is written in `analog for` and `digital for` blocks (Part IV).

```rust
/// Ideal resistor.
module Resistor(p: Electrical, n: Electrical) {
    /// Resistance, in ohms.
    r: f64 where r > 0.0;
    tc1: f64 = 0.0;
    tnom: f64 = 300.15 where tnom > 0.0;
}

analog for Resistor {
    i(p, n) +> v(p, n) / (r * (1.0 + tc1 * (temperature() - tnom)));
}
```

| Form | Written |
| --- | --- |
| module | `module Name(ports) [-> T] { items [result] }` |
| extern module | `#[link(…)] extern "abi" module Name(ports) { params }` |
| template | `Name { params }` |
| instance | `let x = Name(connections) { params };`, or a call inside an expression (8.3) |

#### 8.1 Ports and params

A **port** is one of:

| Port | Written | Connected to |
| --- | --- | --- |
| carrier of a one-driver discrete nature | `in a: Bit<T>`, `out q: Bit<T> [= initial]` | a carrier; an `in` port also takes an expression over carriers and params, which is an anonymous definition (8.2) |
| carrier of any other nature | `p: Electrical`, `sda: Wire [= initial drive]` | a carrier (a net) |
| bundle | `tx: Stream<T>`, `rx: flip Stream<T>`, `in s: Stream<T>` | a bundle |
| value input | `x: f64` | an expression (8.3) |

`in` is read; `out` is driven by this module and is read inside it like any of its carriers. An `in` port with nothing connected is an error; an optional port is `Option<N>`, tested with `if let Some(t) = t { … }` at elaboration.

A **param** is `name: Type [= default] [where condition];`. Its type is a value type, or a module (a template, 8.4).

| Part | Example |
| --- | --- |
| no default: required | `r: f64;` |
| `Option`: may be left out (3.2) | `rs: Option<f64>;` |
| `where`: any boolean over this param and earlier ones | `n: f64 = 1.0 where (0.1..=10.0).contains(&n);` `vmax: f64 where vmax > vmin;` |

1. `where` is checked at elaboration and on every host `set`. The host reads it as the param's bounds (23.2).
2. A param is **structural** when generation, a const generic, a carrier's initial value or a template reads it. A structural param is fixed at elaboration: a host `set` of it during a run is refused, and the host re-elaborates.
3. Units, aliases, distributions for Monte Carlo and "exclude from gradients" are not model text: `///` documents a param, and the rest belongs to the host session (23).

Const generics are constrained by a `where` after the port list, checked at elaboration: `module AsyncFifo<T: Repr, const A: u32>(…) where A >= 1 { … }`.

#### 8.2 The body

Items in any order; every name is visible in the whole body and in the module's blocks.

| Item | Example |
| --- | --- |
| param | `c: f64 = 1p where c > 0.0;` |
| signal | `signal mid: Electrical;` `signal q: Bit<u8> = 0;` |
| instance | `let r1 = Resistor(a, b) { r: 1k };` |
| named expression | `let half = r / 2.0;` `let drift = v(p, n) * 1e-3;` `let acc = idt(v(a));` |
| named branch | `let bq = branch(a, k);` |
| `=` between nets | `n[0] = a;` |
| `=` defining a discrete carrier | `y = a & b;` `busy = st != Tx::Idle;` |
| generation | `for k in 0..N { … }`, `if let Some(t) = t { … }` |

**`let` names an expression.** It writes nothing and has no events. Wherever the name is read, the expression is evaluated there: at the current iterate in `analog`, at the instant in a reaction, by the solver inside a located event. A `let` over params only is a constant. Instances placed in the expression are placed once, by the `let`, not once per read. This is how a module's blocks share a computation.

**`=` between two nets** (two nodes, or two carriers of one resolved nature) makes them one net. The right side is a bare carrier.

**`=` defining a discrete carrier** makes this module's drive of it a combinational function of the carriers it reads:

1. When an input changes, the drive is recomputed in the same microstep, before that microstep's reactions run (15). Definitions and `resolve` functions are evaluated in dependency order, so a chain settles within one microstep.
2. A change of the carrier's value fires its `changed()` in that microstep.
3. A combinational loop, a drive that depends on itself with no reaction or delay in between, is an elaboration error. Dependencies are tracked per drive: `s.others()` depends on the other drives of `s`, not on this module's.
4. `y = expr after d;` is transport-delayed: `y` takes each value of `expr` `d` later, under the transport rule (12.1 rule 4). The delay may depend on the new value, `y = a & b after if a & b { tr } else { tf };`. A delay of 0 is no delay. At an operating point `y` has the value of `expr`.
5. A definition cannot probe: it would have to run at every Newton iteration. Sample an analog value in a reaction.
6. A definition between carriers of the same width and different types is written with the stdlib `cast` (22.1): `byte = cast(sh);`.

The other rules:

1. `for` and `if` run at elaboration over params and constants; a `let` in a `for` makes an array.
2. `pub` on an item makes it visible to the parent as `inst.name`. A parent may read a public carrier, probe a public node or branch, and attach its own branches to a public node. It never writes a child's drive and never contributes to a child's branch.

```rust
/// Clock buffer and two flip-flops: the buffer settles before either reaction runs.
module Shift2(in clk: Bit, in d: Bit, out q2: Bit) {
    signal clk2: Bit;
    signal q1: Bit;
    clk2 = clk;
}
digital for Shift2 {
    on clk.rise()  { q1 <- d; }
    on clk2.rise() { q2 <- q1; }      // reads the old q1: two stages, as in hardware
}

/// Shift register, LSB first: the register is the array, the byte a cast of it.
module UartShift(in clk: Bit, in rx: Bit, out byte: Bit<u8>) {
    signal sh: [Bit; 8];
    byte = cast(sh);
}
digital for UartShift { on clk.rise() { sh[7] <- rx; for k in 0..7 { sh[k] <- sh[k + 1]; } } }
```

#### 8.3 Instances inside expressions

A module may take **value inputs** and have a **result**; then it can be placed inside an expression. This is how everything with state that reads like an operator is written: derivatives, integrators, filters, crossings.

```rust
/// The stdlib integrator, generic over the nature that gives its state a tolerance.
pub module idt<S: Nature = State>(x: f64, ic: Option<f64>, reset: Option<bool>) -> f64 {
    signal y: S;
    potential(y)                             // the result
}

analog<S> for idt<S> {
    let hold = reset.unwrap_or(false) || (ic.is_some() && analysis(Static));
    if hold { potential(y) +> ic.unwrap_or(0.0); } else { flow(y) +> ddt(potential(y)) - x; }
}

analog for Pi { v(outp) +> k1 * v(inp) + k2 * idt(v(inp), 0.0); }     // one instance of idt
```

1. **A value input is the caller's expression.** `x: f64` in a port list binds, at each instance, the expression the caller wrote. The module reads it as it would a `let` of its own (8.2): at the current iterate in `analog` (differentiated through), by the solver in a located event, at the instant in a reaction.
2. **The result is the module's expression.** The last expression of the body; the caller reads it the same way. A result may have any value type, including `Event`.
3. **Every placement is one instance.** A body may name one, `let acc = idt(v(a));`, and then `acc` reads its result in every block.
4. **Where.** In a body `let` or instance argument, in an `analog` block, in the event of an `on`. Never in a `fn`, in a reaction body, or in a loop whose trip count is not a constant; loops over params are unrolled, one instance each. An instance exists and its state advances whatever control flow surrounds its use; state advances only on accepted steps. `m` and `dtemp` apply to it as to any instance.
5. **Feedback.** A result that reads only the module's carriers passes through state, so an instance may read its own result in its arguments: `let y = idt(f(y));` is the differential equation `y' = f(y)`. A result that reads a value input would close an algebraic loop, an elaboration error.
6. **The core is modules too.** `ddt`, `delay`, `slew`, `limit`, `noise`, `cross` and `at` are modules with a result implemented by the simulator (14). They follow rules 1–5 and no others.

`idt` follows VAMS 4.5.4: in an equilibrium solve it holds `ic` when one is given; with no `ic` its row is `x = 0` and its output is free, which has a solution only inside a feedback loop (otherwise the solve fails naming the instance). Its state lives on a node of nature `S`, so its tolerances are those of what is integrated: `idt::<Heat>(pwr(t), 0.0)`.

```rust
fn windup(u: f64, lo: f64, hi: f64) -> f64 { u.clamp(lo, hi) - u }

/// PI with output limits and back-calculation anti-windup: the integral reads its own result.
module PiAw(e: VSignal, y: VSignal) {
    kp: f64 = 0.5; ki: f64 = 2k; kt: f64 = 10k;
    lo: f64 = 0.0; hi: f64 = 1.0 where hi > lo;
    let xi = idt(ki * v(e) + kt * windup(kp * v(e) + xi, lo, hi));
}
analog for PiAw { v(y) +> (kp * v(e) + xi).clamp(lo, hi); }
```

#### 8.4 Templates and swappable modules

A template is a module with params and no connections: a value, usable in params, constants and functions, with struct update syntax. A module's name is the type of its templates. The bound `C: Fn(P)` holds for every module whose port list is `P`. Binning is a function returning a template.

```rust
/// One-transistor one-capacitor DRAM cell; Cell is the bundle of section 7.
module Dram1T1C(c: Cell) {
    cs: f64 = 30f;
    ron: f64 = 5k;
    rleak: f64 = 1T;
    pub signal sn: Electrical = 0.0;     // storage node
}

analog for Dram1T1C {
    let g = if c.wl.known_or(false) { 1.0 / ron } else { 0.0 };
    i(c.bl, sn) +> g * v(c.bl, sn);
    i(sn) +> ddt(cs * v(sn)) + v(sn) / rleak;
}

module MemoryBank<C: Fn(Cell), const ROWS: u32, const COLS: u32>(in wl: [Bit; ROWS], bl: [Electrical; COLS]) {
    cell: C;
    for r in 0..ROWS { for c in 0..COLS { pub let cells = cell(Cell { wl: wl[r], bl: bl[c] }); } }
}
let bank = MemoryBank::<_, 64, 32>(wl, bl) { cell: Dram1T1C { cs: 30f } };   // in a parent's body

pub const NCH: Bsim4 = Bsim4 { tox: 4n, u0: 0.04, vth0: 0.45, l: 1u, w: 1u };
pub fn nch(l: f64, w: f64) -> Bsim4 {
    let vth0 = if l < 0.18u { 0.42 } else if l < 1u { 0.45 } else { 0.47 };
    Bsim4 { l, w, vth0, ..NCH }
}
let m1 = nch(0.13u, 2u)(d, g, s, b);                                       // likewise
```

#### 8.5 Implicit params and extern modules

Every instance has two implicit params, inherited by its children:

- `m: f64 = 1.0 where m > 0.0`: the instance behaves as `m` identical instances in parallel (VAMS 6.3.6). Flow contributions and noise power are multiplied by `m`, flow probes divided by `m`.
- `dtemp: f64 = 0.0`: the instance runs `dtemp` above the analysis temperature; `temperature()` inside it returns the analysis temperature plus the sum of `dtemp` along its path.

An `extern "abi" module` is implemented outside the language. Its body lists params only, checked against the foreign descriptor. It has no behavior blocks.

```rust
#[link("bsim4.osdi", model = "bsim4")]
extern "osdi" module Bsim4(d: Electrical, g: Electrical, s: Electrical, b: Electrical) {
    l: f64 = 1u;
    w: f64 = 1u;
    vth0: f64 = 0.4;
}

#[link("libuart_rx.so")]
extern "cosim" module UartRxRtl(in clk: Bit, in rx: Bit, out byte: Bit<u8>, out dv: Bit) {}
```

| ABI | Ports | Contract |
| --- | --- | --- |
| `"osdi"` | nodes | The model is an analog block the language did not write: it is loaded at every Newton iteration, its limiting vetoes convergence (18), its state advances on accepted steps. Its params, operating-point variables, noise sources and internal nodes appear in the metadata (21) under the descriptor's names. |
| `"cosim"` | one-driver discrete carriers only | The foreign simulator is one module in the microstep loop (15). At a microstep where an input changed, or at a time it asked for, it is given the inputs and returns commits, which apply at the next microstep, and the next time it wants to run, which is a scheduled time (17.2). It never probes and is never advanced into a step that can be rejected, so it needs no rollback. A foreign model with analog pins sits behind `A2D`/`D2A` (22.4). |

### 9. Quantities

Every quantity a model uses is one of three:

| Quantity | Declared | Changes | In the matrix | Derivative |
| --- | --- | --- | --- | --- |
| param | `r: f64 = 1k;` | only by the host's `set` | no | `∂/∂p` when it reaches a contribution |
| discrete carrier | `signal r: Bit<f64> = 1k;` | at events | no: constant in every solve | none |
| node | `signal n: Electrical;` | continuously | yes | `ddt` |

An expression in `analog` is in the matrix exactly when it depends on a node. Models never write params. A change of a discrete carrier read by `analog` is a jump (15.2). Four ways to vary a resistance:

```rust
// A. Set from outside: a Bit input; each change is a jump.
module DigiPot(p: Electrical, n: Electrical, in r: Bit<f64>) { r0: f64 = 1k; }
analog for DigiPot { i(p, n) +> v(p, n) / r.known_or(r0); }

// B. A function of a node: in the matrix, Newton sees the dependence.
module VcResistor(p: Electrical, n: Electrical, ctl: Electrical) { r0: f64 = 1k; k: f64 = 0.1; }
analog for VcResistor { i(p, n) +> v(p, n) / (r0 * (1.0 + k * v(ctl))); }

// C. Updated after each time point: lagged by one step, no time point of its own.
module LaggedResistor(p: Electrical, n: Electrical, ctl: Electrical) {
    r0: f64 = 1k; k: f64 = 0.1;
    signal r: Bit<f64> = r0;
}
digital for LaggedResistor { on settle { r <- r0 * (1.0 + k * v(ctl)); } }
analog for LaggedResistor { i(p, n) +> v(p, n) / r.known_or(r0); }

// D. A function of time: no Jacobian entry; the corner is a carrier change, so the solver lands on it.
module SweptResistor(p: Electrical, n: Electrical) {
    r0: f64 = 1k; r1: f64 = 10k; tsweep: f64 = 1u;
    signal done: Bit = false;
}
analog for SweptResistor {
    let x = if done.known_or(false) { 1.0 } else { now() / tsweep };
    i(p, n) +> v(p, n) / (r0 + (r1 - r0) * x);
    bound_step(tsweep / 50.0);
}
digital for SweptResistor { on at(tsweep) { done <- true; } }
```

A host may `set` a non-structural param during a run. The instant is a jump (15.2): values computed from params are recomputed, and signals and instance state are kept. A reaction can follow it with `on vref.changed()`.

## Part IV. Behavior

### 10. Behavior blocks

Behavior with time is written in blocks for a module: `analog<G> for M<G> { … }` and `digital<G> for M<G> { … }`. A module may have any number of each; blocks of one kind act as one, in source order.

| Block | Holds | Runs | Writes |
| --- | --- | --- | --- |
| `analog` | statements: `let`, `if`, `match`, calls, `+>` | at every Newton iteration of every analysis | branches, with `+>` |
| `digital` | reactions and generation | when their events fire | discrete carriers, with `<-` |

1. Generic names follow the block keyword; bounds come from the module.
2. A block sees every port, param, signal and `let` of its module, and the `pub` items of its instances.
3. `analog` and `digital` are keywords only before `for` or `<`.

|  | contract | structure | without time | with time |
| --- | --- | --- | --- | --- |
| values | `trait` | `struct`, `enum` | `impl` | — |
| hardware | bundle (`struct`) | `module`, `nature` | `impl` | `analog for`, `digital for` |

### 11. Analog

An `analog` block is straight-line code evaluated at every iteration, with no memory between iterations: the past comes from instances and from discrete carriers.

1. `i(a, b) +> x;` adds `x` to the branch's flow; `v(a, b) +> x;` adds `x` to its potential, which makes the branch a potential source with its flow as an unknown. Contributions add, from anywhere in the module.
2. A branch gets either potential or flow contributions in one solve, never both. One that gets potential in some solves and flow in others is a **switch branch**: `v(a, b) +> 0.0` is a short, `i(a, b) +> 0.0` is open.
3. **A branch's kind is constant during a solve.** What selects between a potential and a flow contribution may depend on params, discrete carriers and `analysis`, never on a probe: a probe-dependent selector is an elaboration error. An ideal diode is a located crossing that flips a `Bit`, and the `Bit` selects (11.2 shows the pattern). When the selector depends on params only, the kind is fixed at elaboration, and a zero-potential branch merges its two nodes; otherwise the branch keeps its flow unknown.
4. A branch with no contribution in a solve has zero flow.
5. Everything is differentiated automatically with respect to every probe, through function calls. An `if` on a probe triggers the `nonsmooth` lint unless `#[allow(nonsmooth)]`; the attribute only silences the lint.
6. Every named `let` of an `analog` block is an operating-point variable: the host reads it by path after an analysis (21).
7. An `analog` block has no reactions; events on analog quantities are written in `digital` (13).

```rust
analog for Capacitor { i(p, n) +> ddt(c * v(p, n)); }
analog for Inductor  { v(p, n) +> ddt(l * i(p, n)); }

module Switch(a: Electrical, b: Electrical, in ctl: Bit) {}
analog for Switch {
    if ctl.known_or(false) { v(a, b) +> 0.0; } else { i(a, b) +> 0.0; }
}

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

There is no indirect contribution: an implicit equation is the row of an internal node, as in `Nullor`, which names the extra unknown. Coupling among any number of separate inductors goes through a port: each inductor has a `Magnetic` node (potential: magnetomotive force, flow: flux rate) and a core module connects them.

#### 11.1 One block, every analysis

`ddt` is 0 in equilibrium solves, `ac_stim` is 0 outside small-signal analyses, `noise` is 0 outside noise analysis. `analysis(a: Analysis)` is true when the current solve belongs to `a`.

| `Analysis` | True during |
| --- | --- |
| `Dc`, `Tran`, `Ac`, `Noise` | the whole of that analysis, its operating point included |
| `Ic` | the initial-condition solve before a transient |
| `Static` | any equilibrium solve, in any analysis |
| `Nodeset` | while nodesets are forced |
| `Tf`, `Sens`, `Pz`, `Sp`, `Disto`, `Pss` | further analyses; the enum is open to new ones |

**Small-signal analyses linearize the `analog` blocks at the operating point**, with every discrete carrier constant. A discrete carrier therefore has no small-signal response. A sampled system that has one states it: under `!analysis(Tran)` it writes the same transfer with `delay`, whose small-signal response is `exp(−jωT)` and whose equilibrium response is identity. `zi_nd` (22.2) is written this way, which gives `H(exp(jωT))` in `Ac` and `Noise` and `H(1)` in `Dc` (VAMS 4.5.12).

A noise source is an instance: one instance in two contributions is fully correlated; two instances are independent. The name only groups sources in reports.

```rust
let n = noise(4.0 * K_B * temperature() / r, "thermal");
i(a, mid) +> n;
i(mid, b) +> -n;          // fully correlated
```

#### 11.2 Bounded state

A state that must stay inside `[lo, hi]` is held at the bound by a switch branch, turned on by a crossing and off when the drift turns back. This is exact, unlike a window function. The same pattern, with `v` and `i` of a branch, is the ideal diode.

```rust
module Bounded(p: Electrical, n: Electrical) {
    hi: f64 = 1.0;
    signal xs: State = 0.0;                 // stdlib nature, order one: pos and rate (22.2)
    signal at_hi: Bit = false;
    let drift = v(p, n) * 1e-3;             // shared by both blocks
}
analog for Bounded {
    if at_hi.known_or(false) { pos(xs) +> hi; } else { rate(xs) +> ddt(pos(xs)) - drift; }
}
digital for Bounded {
    on above(pos(xs) - hi) { at_hi <- true; }
    on above(-drift) && at_hi { at_hi <- false; }
}
```

### 12. Digital

A `digital` block holds reactions and generation (`for`, `if` over params). It writes discrete carriers only, and cannot contribute.

#### 12.1 Reactions

`on ev { body }` runs `body` once, to completion, when the `Event` `ev` fires.

| Statement | Meaning |
| --- | --- |
| `q <- x;` | commit: applies at the next microstep |
| `q <- x after dt;` | applies `dt` later; `dt <= 0` is the next microstep |
| `bits[3] <- true;` `mem[a] <- d;` | commits one element |
| `let x = …;` | a local |
| `emit!(…)` | output (19) |

1. **Reads see the past.** Every read sees the values at the start of the microstep, after its definitions settled; the commits of a microstep apply together.
2. **The last write wins.** Commits to one level of a drive that apply in the same microstep are ordered by the instant they were made, then by source order (statement order in a reaction, reaction order in the module's blocks); the last one is the value. Every writer of a drive is in one module (5.2), so the order is always visible in one file. Two different reactions committing one level in one microstep raise the `collision` lint, not an error. Different levels never interact.
3. **Modules never race.** Between modules there is nothing to order: a one-driver carrier has one writer, and `resolve` is commutative.
4. **Delays are transport.** A commit made for time `T` removes that level's pending commits for times `≥ T`, then waits. So a shorter delay overtakes a longer one, and what a drive does is always the latest decision about each future time.
5. **Probes read the solution at the instant** the event fired; a reaction that probes makes its instant a time point of the solver (17.2).
6. **A reaction never suspends.** It runs to completion at its instant; a sequence over time is visible state (12.2).

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

#### 12.2 Sequences

There is no `wait` and no `sleep` in a model. A sequence over time is written with its state in the open: a carrier that counts the steps and an `at` for the next one. Sequential stimulus with waits belongs to the testbench, in the host (23).

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

### 13. Events

An event is a value of type `Event`: something that fires at an instant. `on` takes an `Event`.

| Primitive | Fires when |
| --- | --- |
| `s.changed()` | `s`, a discrete carrier or a param, changed in this microstep |
| `at(t0: f64, period: Option<f64>)` | time reaches `t0`, and every `period` after |
| `cross(e: f64, ttol: Option<f64>, etol: Option<f64>)` | `e` goes from `≤ 0` to `> 0` between two successive solutions; located by the solver when the two are time points |
| `init` | once per analysis, at microstep `(0, 0)` of its first operating point |
| `settle` | once per time point, after the instant has resolved; its commits apply at the next time point |
| `final` | at the end of each analysis |

1. `a || b` fires when either fires. `ev && c` with `c: bool` is a guard; an unknown guard runs the reaction under 3.3 rule 3. `!` applies to `bool` only; a `bool` alone is not an event.
2. Inside a reaction, an event used as a `bool` says whether it fired in this microstep.
3. An event is named in the module body, `let up = above(e);`, and every use of the name is the same event.
4. `at` fires once for each value of its arguments, and then every `period`. It is re-armed when the carriers and params in its arguments change; a re-armed time already past fires at the current instant; an unknown time does not fire. `now()` is not an input. `at` never fires in an analysis without time.
5. **`cross` compares successive solutions**, whatever separates them: two time points of a transient (then the solver locates the crossing, 17.3), two solves at one instant (the two sides of a jump, 15.2; the rounds of an operating point, 16), or two points of a sweep. It has no direction argument: a falling crossing is `cross(-e)`, either is `cross(e) || cross(-e)`. A guard made only of discrete carriers also tells the solver not to locate it while the guard is false.
6. **Initial values are not changes.** Nothing fires merely because an analysis starts, except `init`. A reaction that must hold from time 0, as level-sensitive logic must, includes `init` (E.2); a crossing that must be right at time 0 is the stdlib `above` (22.2), which is `cross(e) || (init && e > 0.0)`.
7. Restrict to an analysis with a guard: `init && analysis(Tran)`.
8. Every other event is a function returning `Event` (3), or a module with an `Event` result when it keeps state: `clk.rise()`, `s.beat(clk)`, `above(e)`, `absdelta(e, d)`.

```rust
/// Comparator, right from the operating point: the event says which way it went.
module Comparator(inp: Electrical, inn: Electrical, out q: Bit = false) {
    ttol: f64 = 1p;
    let up = above(v(inp, inn), ttol);
    let dn = above(-v(inp, inn), ttol);
}
digital for Comparator { on up || dn { q <- up; } }

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

`Water`'s branch `(t, gnd)` is a switch branch selected by a `Bit`, as rule 3 of section 11 requires; what happens to `temp(t)` at the instant the phase changes is the jump rule (15.2).

### 14. Core primitives

What only the simulator can provide; everything else is the standard library (22).

| Primitive | Kind | Meaning | Allowed in |
| --- | --- | --- | --- |
| `potential`, `flow`, every `access` alias; `p.inflow()` | `fn` | probes (6) | `analog`, events, reactions, body `let`s, value inputs |
| `ddx(e, probe)` | `fn` | partial derivative | `analog` |
| `ac_stim(mag, phase: Option<f64>)` | `fn` | small-signal stimulus | `analog` |
| `now()`, `freq()`, `temperature()`, `simparam(name, default: Option<f64>)`, `analysis(a)` | `fn` | environment | everywhere but definitions |
| `ddt(x, abstol: Option<f64>)` | module | time derivative; its argument is a state of the jump rule (15.2) | where instances go (8.3 rule 4) and `analog` reads them |
| `delay(x, td, max: Option<f64>)` | module | `x` as it was `td` ago; with `max`, `td` may vary up to it; `exp(−jω·td)` in small-signal | the same |
| `slew(x, rise: Option, fall: Option)` | module | follows `x` with a bounded rate | the same |
| `limit(x, f, seed: Option<f64>)` | module | Newton step limiting with `f: fn(new, old) -> f64` (18) | the same |
| `noise(psd, name: Option<&str>)` | module | noise source; `psd` may use `freq()` | the same |
| `cross`, `at` | module, result `Event` | located and scheduled events (13) | events |
| `s.changed()`, `init`, `settle`, `final` | `Event` values | (13) | events |
| `bound_step(dt)` | statement | the next step is at most `dt` | `analog` |
| `branch(a, b: Option)` | body item | a named branch | module body |
| `emit!` | statement | output (19) | reactions, `analog` |
| `s.prev()`, `s.since()`, `s.others()`, `x.level(k)` | methods | history, drives, the known flag | wherever the carrier is read |

Not core, and written in the language in 22: `idt`, `idtmod`, `above`, `absdelta`, `transition`, `last_crossing`, `laplace_*`, `zi_*`, `limexp`, `white_noise`, `flicker_noise`, `table_model`. There is no `discontinuity()`: a discontinuity is a discrete carrier change that `analog` reads (9, case D).

## Part V. Execution

What every simulator guarantees, in terms a model can observe. Time is superdense (Lee and Zheng, HSCC 2005). How the reference implementation meets these guarantees, with its numbers, and where it does not yet (G.6), is in Appendix C.2.

### 15. Time and the instant

An **instant** is `(t, n)`: real time `t` and a microstep `n`. Analog values are continuous in `t` except at a jump (15.2), where they have a left and a right value.

Event times are integer ticks of a global resolution, a session option of the host, 1 fs by default. `at` and `after` round to the nearest tick, so edges that should coincide do. A located crossing is rounded **up** to a tick, the solution is evaluated at that tick, and its reactions run there: the expression has crossed where they read it.

At each time point:

1. **Solve** at `t`: the left value.
2. **Collect** what is due at `t`: `at` events, delayed commits, commits held by `settle`, located crossings, host `set`s and drives.
3. **Microstep loop**, `n = 0, 1, …`:
   1. Apply the commits due at `(t, n)`, last write winning (12.1 rule 2).
   2. Settle the structure: recompute, in dependency order, every definition and every `resolve` whose inputs changed. Every carrier whose value changed fires its `changed()`.
   3. Run each reaction whose event fired, once, reading the settled state. Its commits go to `n + 1`, or later with `after`.
   4. If a carrier or param read by `analog` changed, solve the jump (15.2). `cross` expressions are compared across it.
   5. Leave the loop when nothing is pending at `n + 1`.
4. **Settle**: `on settle` runs once; its commits are held until the next time point.
5. **Advance** to the next time point (17).

The result is a function of the source alone: reactions read one snapshot, writers of one drive are ordered by 12.1 rule 2, and drives of one carrier combine commutatively. A microstep loop that never goes quiet is an error naming the carriers that kept changing.

| Code | Reads discrete carriers | Reads analog values |
| --- | --- | --- |
| `analog` | constant for the whole solve | the current iterate |
| a reaction at `(t, n)` | settled at microstep `n` | the solution after microstep `n − 1` |
| `on settle` | final values of the time point | the final solution |

#### 15.1 An instant, traced

`Front` (section 0) with `vin` crossing the upper threshold at `t = 10 ns`, exactly when `clk` rises:

| Microstep | Applied | Definitions | Events fired | Reactions commit |
| --- | --- | --- | --- | --- |
| (10 ns, 0) | `clk ← H` (from its clock source) | `d` unchanged | `clk.rise()`, the crossing | `q <- d` reads the old `d`; `above_th <- true` |
| (10 ns, 1) | `q`, `above_th` | `d ← H` | `d.changed()` | none listen to `d`; nothing pending, so `settle` runs and time advances |

The flip-flop took the value from before the edge, as hardware does: the comparator's new output reaches `q` at the next clock edge.

#### 15.2 The jump

When something `analog` reads changes at `(t, n)` (a discrete carrier, a param), the analog system is solved again at the same `t`. One rule defines that solve:

**The right value is the limit of a backward-Euler step of length `h → 0` taken from the left value.** Every `ddt(q)` reads `(q − q⁻) / h`, where `q⁻` is its argument at the previous solve of this instant.

What follows from it, with no further rule:

1. **The arguments of `ddt` are the states.** They are continuous across a jump wherever the equations allow: a capacitor keeps `c·v`, an inductor keeps `l·i`, an `idt` keeps its integral. Everything else (algebraic unknowns, flows) is free and is solved for.
2. **A constraint may force a state to jump.** Then the sum of the states over each cut the constraint closes is conserved, and the flow that moves it is impulsive: it is the `1/h` term, it exists only inside the solve, and no probe reads it. Shorting a 1 pF capacitor at 1 V to a 3 pF one at 0 V gives 0.25 V on both (F.5).
3. **A state multiplied by a carrier keeps the product.** In `ddt(c * v(p, n))` with `c` a `Bit<f64>` that jumps, `c·v` is continuous and `v` jumps: charge is conserved.
4. **The solve is repeated once** from its own result. The second pass moves nothing and gives every `ddt` its finite right-hand value, which is what probes and `cross` read at `(t, n + 1)`.
5. **Other instance state is untouched**: `delay` keeps its history, `slew` its output, `limit` its last value. `idt` and `transition` have no rule of their own: their state is a node under `ddt` and discrete carriers.
6. **Then the integrator restarts**: it keeps the right value and every `ddt` state, drops its multistep history and error estimate, and takes its first step from there.

A jump that leaves a node with no path to `gnd` except through `ddt` terms is still determined, by rule 1. A jump whose constraints contradict each other (two potential sources in a loop) fails as a singular solve naming the branches.

### 16. The operating point

The state an analysis starts from: an analog equilibrium and a digital state that agree.

1. **Elaborate**: check params, run generation.
2. **Initialize**: drives with `= value` take it; others are unknown. Newton starts from `#[nodeset]` values, else from zero.
3. **Solve** the equilibrium. When Newton alone fails, the simulator uses continuation methods that never change the answer: the last solve is the unmodified circuit.
4. **Fixed point** at `(0, n)`: definitions and `resolve`s settle, `init` fires at `(0, 0)` of the first round, and the microstep loop runs. If anything read by `analog` changed, the equilibrium is solved again, `cross` compares the two solutions (13 rule 5), and the loop runs again. A bounded number of rounds; more is an error naming the carriers still changing.
5. **Initial conditions.** Under `uic`, each node with `= value` holds that value through the time-0 solve and the first step.
6. **Settle** once and start.

`cross` needs two solutions, so it cannot fire on the first; `above` adds the `init` test and is right from time 0, as `Comparator` shows (13).

#### 16.1 Analyses without time

In `Dc` (each point of a sweep), and in the operating point of `Ac`, `Noise` and the other small-signal analyses, time does not advance. One paragraph covers them:

1. Each sweep point is an operating point: steps 3–4 above, then `settle` once.
2. **Discrete state carries from one sweep point to the next.** `init` fires at the first point only. A Schmitt trigger swept up and down traces its hysteresis loop, because `above` sees each threshold crossed between successive points (13 rule 5).
3. `at` never fires, and a commit with `after dt > 0` never comes due: it is dropped when the analysis ends. `since()` does not advance.
4. A small-signal analysis linearizes at the fixed point of step 4, with the discrete state found there. If the fixed point does not go quiet, the analysis fails; it never linearizes around an oscillating state.
5. Each analysis starts from the declared initial values unless the host asks it to continue from the previous analysis's state.

### 17. Transient

#### 17.1 Convergence

Every solve is a Newton iteration. It has converged when:

1. **Update**: every unknown moved by at most `reltol · max(|x|, |x_old|) + abstol`, where `abstol` is the nature's: for a node, the smallest potential `abstol` of the natures meeting there; for a branch unknown, its flow `abstol`.
2. **Residual**: every row's imbalance is at most `abstol + reltol · scale`, where `abstol` is, for a node's row, the flow `abstol` of its nature, and for the row of a branch with a potential contribution, the potential `abstol`.
3. **Limiting**: no `limit` instance is active (18).

A non-finite value fails the solve.

#### 17.2 Steps

1. The truncation error of every step is within `trtol`, measured with the natures' tolerances.
2. **The solver lands exactly on every instant that can touch it**: a scheduled time (`at`, `after`, a host action) one of whose reactions probes, or can change, directly or through definitions, `resolve`s and further reactions of the same instant, a carrier that `analog` or a probing reaction reads. This is decided from the static dependencies of the elaborated design. Every other digital instant runs in time order between time points, never ahead of the last accepted one, and is not a time point.
3. A rejected step leaves everything unchanged: carriers, pending events, instance state, the analog history.
4. Instance state (`ddt`, `delay`, `slew`, `limit`, `cross`, and the nodes and carriers of stdlib modules) advances only on accepted steps.
5. After a jump the integrator restarts (15.2 rule 6).

| Situation | Time point at `t` | Jump (15.2) |
| --- | --- | --- |
| discrete change that cannot reach `analog` or a probe | no | no |
| discrete change read by `analog` | yes | yes |
| host `set` of a param read by `analog` | yes | yes |
| reaction at `t` that probes | yes | no |
| `cross` located at `t` | yes | only if what `analog` reads changes |
| `on settle` | runs only at time points; adds none; its commits apply at the next | no |
| `bound_step(dt)` | no: caps the step | no |

A digital clock that nothing analog depends on costs the solver nothing.

#### 17.3 Locating crossings

A `cross` whose expression goes from `≤ 0` to `> 0` between two time points is located: the step is retaken until the crossing time is known within `ttol`, or `0 < e < etol`. Defaults: `ttol` from the simulator, `etol` the nature's `abstol`. The step is accepted at the tick at or after the crossing (15) and its reactions run there. An expression that is unknown does not fire.

A corner of a waveform computed from `now()` is invisible to the error estimate; it must be a carrier change (9, case D), which makes it a time point and a jump. `bound_step(dt)` is re-evaluated every step.

### 18. Limiting

`limit(x, f, seed)` bounds how far Newton moves the expression `x` per iteration, with `f: fn(new: f64, old: f64) -> f64`. It changes the path, never the answer.

1. At iteration `k` it returns `x̂_k = f(x_k, x̂_{k−1})`, where `x̂_{k−1}` is what this instance returned before. On the first iteration of an equilibrium solve, `x̂_{k−1}` is `seed` when given, else the last accepted value or the initial guess.
2. While the instance still holds its seed and the limited value would fall below it, the seed is kept: a junction opens near its turn-on point (ngspice `MODEINITJCT`).
3. `x̂_k` replaces `x` in the rest of the expression; for differentiation `limit` is transparent.
4. The right-hand side is corrected to linearize around `x̂_k` (OSDI `lim_rhs`).
5. An instance whose limited value still differs from its input beyond a tolerance vetoes convergence, so at convergence the model is exactly the unlimited one.

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
        None    => { v(a, ai) +> 0.0; }          // a param selects: ai and a are one node (11 rule 3)
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
    #[allow(nonsmooth)]
    let id = if vov <= 0.0 { 0.0 }
             else if vds < vov { beta * (vov - vds / 2.0) * vds * clm }
             else { beta / 2.0 * vov * vov * clm };
    i(d, s) +> id;
    let gm = ddx(id, v(g, s));          // an operating-point variable, like every named let (11 rule 6)
}
```

Starting points are hints, not equations: `#[nodeset = 0.6]` on a node sets where the first equilibrium iteration starts.

## Part VI. Input and output

A model writes records, reads files at elaboration, and exposes metadata. None of the three feeds back into the simulation.

### 19. Streams and checks

The core has one primitive: `emit!("name", field = value, …)` writes a typed record to the stream `name`. Every other output is a stdlib macro over `emit!` (22.6), including checks and `finish!()`: a record on the reserved stream `sim` with `finish = true` ends the analysis after the current instant.

1. Every record carries the instance path, analysis, instant `(t, n)`, inside `analog` the Newton iteration, and `maybe = true` when it was reached under unknown control (3.3).
2. Stream names are relative to the instance: `emit!("conv", …)` in `top.adc` writes `top.adc.conv`.
3. An output runs when the code around it runs: once per firing in a reaction; once per time point in `on settle`; at every evaluation in `analog`, including rejected steps, which is how non-convergence is debugged.

A check is an ordinary reaction, and a reduction over an analysis is an explicit signal:

```rust
module Dut(p: Electrical, n: Electrical, vo: Electrical, vi: Electrical, tj: Thermal) {
    hi: f64 = 1.2;
    lo: f64 = 0.0;
    signal ipk: Bit<f64> = 0.0;          // a reduction is state
}

digital for Dut {
    on settle {
        require!(head_hi, v(p, n) <= hi);         // a comparison also reports its margin
        require!(head_lo, v(p, n) >= lo);
        target!(gain_db, db20(v(vo) / v(vi)) >= 40.0);
        cover!(hot, temp(tj) > 400.0);
        ipk <- ipk.max(p.inflow().abs());
    }
    on final { require!(ipk_lim, ipk < 10m); }
}
```

### 20. Input data

Files are read at elaboration into `const`s; a run never reads a file. Paths are relative to the source file, and a row that does not fit the `const`'s type is an elaboration error.

```rust
const IV:  [(f64, f64); 64] = include_table!("diode_iv.tbl");   // whitespace columns
const ROM: [u32; 1024]      = include_mem!("boot.hex");        // $readmemh format

module MeasuredDiode(a: Electrical, k: Electrical) {}
analog for MeasuredDiode { i(a, k) +> table_model(v(a, k), &IV, "3E"); }   // cubic, error outside

module BootRom(in addr: Bit<u10>, out data: Bit<u32>) {
    signal mem: [Bit<u32>; 1024] = ROM;
    data = mem[addr];
}
```

### 21. Metadata

What a compiled model tells its host without running. It is fixed at elaboration; names are hierarchical paths (`top.adc.dac.r[3].r`).

| Item | From | Host operations |
| --- | --- | --- |
| params | the body: type, default, `where`, structural or not, `///` | `list`, `get`, `set` (checked) |
| ports | the port list | `list` |
| public items | `pub` | `list`, probe |
| every node, branch, carrier, drive and instance | the elaborated netlist | probe and record by path, read-only |
| operating-point variables | every named `let` of `analog`; an OSDI model's opvars | `get` after an analysis |
| streams and checks | every `emit!` site, with field names and types | `list`, subscribe before the run |
| gradients | every `f64` param that reaches a contribution, a probe-reading expression or an event time; found by dependency analysis | `∂(record)/∂p` after an analysis (23.3) |

`pub` decides what other models may use; the host can observe everything.

## Part VII. Standard library

### 22. Standard library

Everything here is written in the language over the core of section 14. It is imported by the prelude and can be replaced.

#### 22.1 Values and `Bit`

```rust
pub trait Known: Repr {
    /// The value when every level is known.
    fn known(self) -> Option<Self> { for k in 0..Self::WIDTH { self.level(k)?; } Some(self) }
    fn known_or(self, d: Self) -> Self { self.known().unwrap_or(d) }
    fn is_known(self) -> bool { self.known().is_some() }
}
impl<T: Repr> Known for T {}

/// Level by level: equal known levels kept, the rest unknown. This is rule 3 of 3.3, used as a function.
pub fn merge<T: Repr>(a: T, b: T) -> T { let c: bool = Unknown; if c { a } else { b } }

/// The same levels under another type of the same width.
pub fn cast<A: Repr, B: Repr>(a: A) -> B where A::WIDTH == B::WIDTH {
    B::decode(a.encode()).unwrap_or(Unknown)
}

impl Bit {
    /// IEEE 1364 posedge: L → H, L → unknown, unknown → H.
    pub fn rise(self) -> Event {
        self.changed() && matches!((self.prev().level(0), self.level(0)),
            (Some(false), Some(true)) | (Some(false), None) | (None, Some(true)))
    }
    /// IEEE 1364 negedge: H → L, H → unknown, unknown → L.
    pub fn fall(self) -> Event {
        self.changed() && matches!((self.prev().level(0), self.level(0)),
            (Some(true), Some(false)) | (Some(true), None) | (None, Some(false)))
    }
}
```

`same(a, b)` compares levels and flags; `matches_bits!(s, "1x0")` matches with don't-cares; `logic!("10xx")` writes a value with unknown levels.

#### 22.2 Analog functions and modules

```rust
nature State {                                      // a unitless state of order one
    potential: Value { access: pos,  abstol: 1u },
    flow:      Rate  { access: rate, abstol: 1n },
}

pub fn vt() -> f64 { K_B * temperature() / Q }
pub module white_noise(psd: f64, name: Option<&str>) -> f64 { noise(psd, name) }
pub module flicker_noise(k: f64, exp: f64, name: Option<&str>) -> f64 { noise(k / freq().powf(exp), name) }
pub module limexp(x: f64) -> f64 { limit(x, |new, old| deltalim(new, old, 1.0)).exp() }

/// Rising crossing that is also right at the operating point.
pub module above(e: f64, ttol: Option<f64>, etol: Option<f64>) -> Event {
    cross(e, ttol, etol) || (init && e > 0.0)
}

/// Fires at the start, and each time e has moved by delta since it last fired.
pub module absdelta(e: f64, delta: f64) -> Event {
    signal last: Bit<f64> = 0.0;
    let moved = init || cross(e - last - delta) || cross(last - delta - e);
    moved
}
digital for absdelta { on moved { last <- e; } }

/// Time of the last rising crossing; -1.0 before the first.
pub module last_crossing(e: f64) -> f64 {
    signal t: Bit<f64> = -1.0;
    t
}
digital for last_crossing { on cross(e) { t <- now(); } }

/// Integral wrapped into lo..lo + modulus. At each wrap the integrator itself is re-seated
/// (its ic is an expression, 8.3 rule 1), so its state never grows and its tolerances stay meaningful.
pub module idtmod<S: Nature = State>(x: f64, ic: Option<f64>, modulus: f64, offset: Option<f64>) -> f64 {
    signal base: Bit<f64>;                   // where the integral was last seated
    signal seat: Bit = false;                // true for the one microstep that seats it
    let lo = offset.unwrap_or(0.0);
    let start = lo + (ic.unwrap_or(0.0) - lo).rem_euclid(modulus);
    let acc = idt::<S>(x, base.known_or(start), reset: seat.known_or(false));
    acc
}
digital<S> for idtmod<S> {
    on cross(acc - lo - modulus) { base <- acc - modulus; seat <- true; }    // over the top
    on cross(lo - acc)           { base <- acc + modulus; seat <- true; }    // under the bottom
    on seat.rise()               { seat <- false; }
}
```

The wrap is three microsteps of one instant: the crossing commits `base` and `seat`; with `seat` high `idt` forces its node to `base` (a jump forced by a potential constraint, 15.2 rule 2); with `seat` low again the integral continues from there (15.2 rule 1). The seated value lies inside the range, so neither `cross` fires again at that instant.

```rust
/// Ramps a piecewise-constant input: delay td, then a straight edge of tr (up) or tf (down).
/// A change mid-ramp starts from the current output. Unknown inputs are ignored. Both corners
/// of every edge are carrier changes that analog reads, so the solver lands on them (17.2).
pub module transition(in x: Bit<f64>, td: Option<f64>, tr: Option<f64>, tf: Option<f64>) -> f64 {
    signal xd: Bit<f64>;                     // x after the delay
    signal y0: Bit<f64> = 0.0;               // output where the current edge began
    signal y1: Bit<f64> = 0.0;               // where it ends
    signal t0: Bit<f64> = 0.0;               // when it began
    signal dt: Bit<f64> = 0.0;               // its duration; 0.0 at rest
    xd = x after td.unwrap_or(0.0);
    let (a, b, s, d) = (y0.known_or(0.0), y1.known_or(0.0), t0.known_or(0.0), dt.known_or(0.0));
    let y = if d > 0.0 && analysis(Tran) { a + (b - a) * ((now() - s) / d).clamp(0.0, 1.0) } else { b };
    y
}
digital for transition {
    on at(t0 + dt) && dt > 0.0 { dt <- 0.0; }                 // the far corner
    on init || xd.changed() {                                 // written second: wins a tie (12.1 rule 2)
        if let Some(to) = xd.known() {
            let from = if init { to } else { y };
            y0 <- from; y1 <- to; t0 <- now();
            dt <- if to > from { tr.unwrap_or(0.0) } else if to < from { tf.unwrap_or(0.0) } else { 0.0 };
        }
    }
}

/// Discrete-time filter y[n] = Σ num[k]·x[n−k] − Σ den[k]·y[n−k], sampled every period.
/// In a transient it is sampled state; elsewhere it is the same transfer written with delay (11.1).
pub module zi_nd<const N: usize, const D: usize>(x: f64, period: f64) -> f64 {
    num: [f64; N]; den: [f64; D];            // den[0] is the coefficient of y[n−1]
    signal xs: [Bit<f64>; N];                // xs[k] = x[n−k]
    signal ys: [Bit<f64>; D];                // ys[k] = y[n−1−k]
    signal out: Bit<f64> = 0.0;
    signal y: State;
    pos(y)
}
digital<N, D> for zi_nd<N, D> {
    on at(0.0, period) {
        let mut acc = num[0] * x;
        for k in 1..N { acc += num[k] * xs[k - 1].known_or(0.0); }
        for k in 0..D { acc -= den[k] * ys[k].known_or(0.0); }
        out <- acc;
        xs[0] <- x;   for k in 1..N { xs[k] <- xs[k - 1]; }
        ys[0] <- acc; for k in 1..D { ys[k] <- ys[k - 1]; }
    }
}
analog<N, D> for zi_nd<N, D> {
    if analysis(Tran) { pos(y) +> out.known_or(0.0); } else {
        let mut acc = 0.0;
        for k in 0..N { acc += num[k] * delay(x, k as f64 * period); }
        for k in 0..D { acc -= den[k] * delay(pos(y), (k + 1) as f64 * period); }
        rate(y) +> pos(y) - acc;             // the row of y: y = Σ num·x(t−kT) − Σ den·y(t−kT)
    }
}
```

Also here, as modules with a result: `idt` (8.3), `laplace_nd`/`np`/`zd`/`zp` (one internal `State` node per pole), `zi_np`/`zd`/`zp` (over `zi_nd`), `noise_table`. As functions: `table_model` (VAMS 9.21 control strings), `db10`, `db20`, `pnjlim`, `fetlim`, `limvds`, `deltalim` (18), and `normal(rng, mean, sigma) -> (f64, u64)`, a pure PRNG step that returns `mean` when `sigma` is 0.

#### 22.3 Resolved nets

High impedance, strengths and wired logic are a value type and a `resolve` function (4, 5.2).

```rust
#[derive(Repr, Clone, Copy, PartialEq, PartialOrd)]
pub enum Strength { HighZ, Weak, Pull, Strong, Supply }

#[derive(Repr, Clone, Copy, PartialEq)]
pub struct Logic { pub v: bool, pub s: Strength }

impl Logic {
    pub const Z: Logic = Logic { v: Unknown, s: Strength::HighZ };     // not driving
    pub fn strong(v: bool) -> Logic { Logic { v, s: Strength::Strong } }
    pub fn pull(v: bool)   -> Logic { Logic { v, s: Strength::Pull } }
    pub fn weak(v: bool)   -> Logic { Logic { v, s: Strength::Weak } }
    pub fn supply(v: bool) -> Logic { Logic { v, s: Strength::Supply } }

    /// IEEE 1364 net resolution: the strongest drive wins; equal strengths that disagree are unknown.
    pub fn wired(d: &[Logic]) -> Logic {
        d.iter().fold(Logic::Z, |a, x|
            if x.s > a.s { *x } else if x.s < a.s { a } else { Logic { v: merge(a.v, x.v), s: a.s } })
    }
}

nature Wire { value: Logic, resolve: Logic::wired }

/// A typed bus: one driver at a time; two at once are unknown; none is None.
pub fn tri<T: Repr>(d: &[Option<T>]) -> Option<T> {
    d.iter().fold(None, |a, x| match (a, *x) { (None, y) | (y, None) => y, _ => Some(Unknown) })
}
nature Tri<T: Repr> { value: Option<T>, resolve: tri }

/// A real-valued net whose drivers add (currents into a summing node, RNM).
pub fn sum(d: &[f64]) -> f64 { d.iter().sum() }
nature SumNet { value: f64, resolve: sum }

module Tribuf(in d: Bit, in oe: Bit, y: Wire) { y = if oe { Logic::strong(d) } else { Logic::Z }; }
module Pullup(w: Wire)   { w = Logic::pull(true); }
module Pulldown(w: Wire) { w = Logic::pull(false); }

/// Bus keeper (Verilog trireg): holds the last driven level, weakly.
module Keeper(w: Wire) { signal last: Bit; w = Logic::weak(last); }
digital for Keeper { on (init || w.changed()) && w.s > Strength::Weak { last <- w.v; } }

/// Bidirectional pass gate (Verilog tranif1): each side is driven with what the rest of the
/// other side drives. others() excludes this gate's own drive, so the gate does not latch itself.
module Tran(a: Wire, b: Wire, in en: Bit) {
    a = if en { b.others() } else { Logic::Z };
    b = if en { a.others() } else { Logic::Z };
}
```

An unknown `oe` or `en` merges the two arms (3.3): the level stays known only where both agree. `Logic` reads as a `bool` through `.v`, which is unknown on an undriven wire. A driver is added from anywhere in the hierarchy by passing the `Wire` down as a port; no module needs to know how many there are. A ring of `Tran`s is a combinational loop (8.2 rule 3) and needs a delay or an analog model (G.27).

#### 22.4 Converters

```rust
/// Bit to node: a level through a resistance, with finite edges; unknown is mid-rail.
module D2A(in d: Bit, a: Electrical, vdd: Electrical) {
    rout: f64 = 100; tr: f64 = 20p;
    let ramp = transition(match d.known() { Some(true) => 1.0, Some(false) => 0.0, None => 0.5 }, 0.0, tr, tr);
}
analog for D2A { i(a) +> (v(a) - ramp * v(vdd)) / rout; }

/// Node to Bit, with hysteresis; right from the operating point.
module A2D(a: Electrical, out d: Bit = false) { vth: f64 = 0.6; hyst: f64 = 50m; }
digital for A2D {
    on above(v(a) - vth - hyst / 2.0) { d <- true; }
    on above(vth - hyst / 2.0 - v(a)) { d <- false; }
}

/// A pad: one Wire, one node. It drives the node while the digital side drives the wire,
/// and drives the wire from the node while the digital side has released it.
module Bidir(d: Wire, a: Electrical, vdd: Electrical) {
    rout: f64 = 100; tr: f64 = 20p; vth: f64 = 0.6; hyst: f64 = 50m;
    signal seen: Bit = false;                                  // the node, thresholded
    signal driving: Bit;
    driving = d.others().s.known_or(Strength::Strong) != Strength::HighZ;
    d = if driving { Logic::Z } else { Logic::strong(seen) };
    let ramp = transition(match d.others().v.known() { Some(true) => 1.0, Some(false) => 0.0, None => 0.5 }, 0.0, tr, tr);
}
analog for Bidir {
    if driving.known_or(true) { i(a) +> (v(a) - ramp * v(vdd)) / rout; }     // released: no contribution, zero flow
}
digital for Bidir {
    on above(v(a) - vth - hyst / 2.0) { seen <- true; }
    on above(vth - hyst / 2.0 - v(a)) { seen <- false; }
}
```

#### 22.5 Monitors

A temporal property is a module that watches carriers and checks with `require!`. No syntax is needed beyond reactions.

```rust
/// Every req is acknowledged within n clock cycles.
pub module AckWithin(in clk: Bit, in req: Bit, in ack: Bit) {
    n: u32 = 4;
    signal waiting: Bit<u32> = 0;      // cycles since an unanswered req; 0 is idle
}
digital for AckWithin {
    on clk.rise() {
        if ack { waiting <- 0; }
        else if waiting > 0 || req {
            require!(ack_within_n, waiting < n);
            waiting <- waiting + 1;
        }
    }
}

/// bad is never true at a clock edge.
pub module Never(in clk: Bit, in bad: Bit) {}
digital for Never { on clk.rise() { require!(never, !bad); } }

/// d does not change while en is high.
pub module Stable<T: Repr>(in en: Bit, in d: Bit<T>) {}
digital<T> for Stable<T> { on d.changed() && en { require!(stable, false); } }

/// d was stable for ts before each rising edge.
pub module Setup<T: Repr>(in clk: Bit, in d: Bit<T>, out viol: Bit = false) { ts: f64 where ts > 0.0; }
digital<T> for Setup<T> {
    on clk.rise() { require!(setup, d.since() >= ts); viol <- d.since() < ts; }
}

/// d holds for th after each rising edge; assumes th is shorter than the high phase.
pub module Hold<T: Repr>(in clk: Bit, in d: Bit<T>) { th: f64 where th > 0.0; }
digital<T> for Hold<T> {
    on d.changed() && clk { require!(hold, clk.since() >= th); }    // clk high: its last change was a rise
}

/// No pulse on d is shorter than w.
pub module MinPulse(in d: Bit) { w: f64 where w > 0.0; signal t_prev: Bit<f64> = -inf; }
digital for MinPulse {
    on d.changed() { require!(min_pulse, now() - t_prev >= w); t_prev <- now(); }
}

/// Inertial delay: y follows x only after x has been stable for tpd (G.7 closed: a module, not syntax).
pub module Inertial<T: Repr>(in x: Bit<T>, out y: Bit<T>) { tpd: f64 where tpd > 0.0; signal seen: Bit<f64> = 0.0; }
digital<T> for Inertial<T> {
    on init || x.changed() { seen <- now(); }
    on at(seen + tpd) { y <- x; }           // re-armed by every change: only the last one survives
}
```

A monitor only reports: it never writes what it watches (5.2). A model that should turn a violation into unknowns reads the monitor's output, `q <- if viol { Unknown } else { d };`, so the injection is opt-in and visible in the netlist. Initial values are not changes (13 rule 6), so no monitor fires merely because the run started.

#### 22.6 Output and files

| Macro | Expands to `emit!` on |
| --- | --- |
| `println!`, `eprintln!` | `stdout`, `stderr` |
| `info!`, `warning!`, `error!`, `assert!` | `stderr`, with a severity |
| `finish!()` | `sim`, with `finish = true` |
| `fatal!` | `stderr`, then `sim` with `finish = true` and an error |
| `require!`, `target!`, `cover!` | `checks`, with pass/fail, a signed margin when the condition is a comparison, or a count; an unknown condition reports `unknown` |

`include_table!` and `include_mem!` are `const` parsers over `include_bytes!`.

## Part VIII. The host

### 23. Testbenches and optimization

A model describes hardware. Everything that drives, sequences and judges a model from outside lives in the host: a Rust or Python program that holds a session. This keeps the language free of the verification surface that makes SystemVerilog large.

#### 23.1 Testbenches

| In the model | In the host |
| --- | --- |
| checks at an instant: `require!`, `assert!`, `cover!`, `target!` | stimulus sequences with waits, `fork`/`join` |
| monitors for temporal properties (22.5) | `force` and `release` on any carrier, recorded as debug actions |
| timing checks on history | constrained random stimulus, scoreboards, coverage models |
|  | regression, sweeps, Monte Carlo draws, time resolution, which params are varied |

1. The host drives the top module's `in` ports as their one driver, and holds one drive of each top-level resolved carrier.
2. The host may wait for any event of the model, read any carrier and probe any node (21).
3. `force` overrides a carrier until `release`; it is outside model semantics and recorded on `stderr`.
4. A host `set` or drive happens at a scheduled time and enters the instant at step 2 of section 15, like any other commit.
5. Monte Carlo is a host loop: it assigns a distribution to any param by path (`sim.mc({"r1.r": gauss(rel=0.05)})`), draws per instance or globally, and sets the values before elaboration.

A sketch in Python (the API belongs to the host library, not to the language):

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

#### 23.2 Optimization

The language already states an optimization problem; the host solves it.

| Piece | Where |
| --- | --- |
| variables and bounds | params with `where` (8.1) |
| objectives | `target!` margins (19) |
| constraints | `require!` margins (19) |
| gradients | `∂F/∂p` for every differentiable param (21), and the solver's adjoint sensitivity, across events as in 23.3 |
| yield | Monte Carlo draws (23.1) |

```python
opt = pp.Optimize(sim, vary=["w", "l"], objectives=["gain_db"], constraints=["ipk_lim"], mc=200)
best = opt.run()          # gradient-based, from the model's own derivatives
```

#### 23.3 Gradients across events

A mixed-signal trajectory is piecewise smooth in its params. Its gradient is defined piece by piece:

1. Between events, sensitivities are those of the solver's equations, forward or adjoint.
2. A located event moves with the params. For a crossing of `e` at `t*`, `dt*/dp = −(∂e/∂p) / (de/dt)`, where `∂e/∂p` includes the state's own sensitivity. At `t*` the state's sensitivity jumps by the change of the right-hand side across the event times `dt*/dp` (the saltation correction). Across a jump (15.2) the sensitivities of the states obey the same backward-Euler limit as the states.
3. An `at` or `after` time computed from params moves by its own derivative.
4. Discrete values are piecewise constant in the params: they have no derivative, and they act on a record only through the times of the events that change them.
5. Where no gradient exists, the host gets a diagnostic naming the event instead of a number: a grazing crossing (`de/dt = 0` at `t*`), or an event that appears or disappears as a param moves.
6. A param that only selects (`match`, `if`, generation) or only feeds discrete logic has no gradient; the dependency analysis of 21 finds these, and the host is told which.

For `Comparator` (13) with `inn` at a threshold `vth`, the edge time solves `v(inp)(t*) = vth`, so `dt*/dvth = 1 / v'(inp)(t*)`: the edge moves by the threshold change over the slew rate. A `Bit<f64>` sampled at that edge has derivative zero; a node that the edge switches, through a `D2A`, receives the saltation correction at `t*`.

## Appendix A. Lexical structure

1. **Close to Rust, not Rust.** Identifiers, comments, attributes, literals and the value syntax are Rust's. The lexer and the grammar are the language's own: they add the tokens and productions below and B.3, and drop what Part I leaves out.
2. **SI-suffixed reals.** A number followed by `T G M k m u n p f a` is an `f64` times `1e12 … 1e-18`: `1k`, `2.2u`, `10f`; `1m` is milli, `1M` mega. The suffix is a multiplier, not a unit.
3. **Unknown values.** `Unknown` is the value of any `Repr` type with every level unknown (3.3); `logic!("10xx")` writes one level by level.
4. **Doc comments** `///` are metadata (21).
5. **Two operators.** `+>` (contribution, in `analog`) and `<-` (commit, in reactions) are single tokens, read by longest match: `a<-b` is a commit, and a comparison with a negative operand is written `a < -b`.

## Appendix B. Reference

### B.1 Added to Rust

| Kind | Items | Count |
| --- | --- | --- |
| keywords | `module` `nature` `signal` `on` `out` `flip` `after`; new uses of `in` and `extern` | 9 |
| contextual keywords | `analog`, `digital` before `for` or `<`; `potential`, `flow`, `value`, `resolve` in a nature (`potential` and `flow` are also the generic probes) | 6 |
| operators | `+>`, `<-` | 2 |
| calls | optional trailing `Option` parameters, named arguments (3.2) | 2 |
| literals | SI suffixes, `Unknown` | 2 |
| types and traits | `Event`, `Analysis`, `uN`/`iN`/`fN`, `Repr`, `Terminals`, `Nature` | 6 |
| carrier and value methods | `changed` `prev` `since` `others` `inflow` `level` | 6 |
| events | `at` `cross` `init` `settle` `final` | 5 |
| analog | `ddt` `delay` `slew` `ddx` `limit` `noise` `ac_stim` `bound_step` `branch` | 9 |
| environment | `now` `freq` `temperature` `simparam` `analysis` | 5 |
| output | `emit!` | 1 |
| attributes | `#[nodeset]` `#[derivative]` `#[derive(Repr)]` `#[allow(nonsmooth)]`; Rust's `#[link]` on an extern module | 5 |
| **subtotal, the categories of v0's B.1** |  | **58** |
| names and forms v0 used but did not list | `gnd`; implicit params `m`, `dtemp`; carrier fields in a `struct` (bundle); struct-update on templates; the `Fn(P)` bound on modules; ABI strings `"osdi"`, `"cosim"` | 7 |
| **total** |  | **65** |

Everything else is the standard library (22), including `Bit`, `Wire`, `above`, `absdelta`, `transition`, `idt`, `cast` and `finish!`.

### B.2 Where each thing is allowed

|  | module body | `digital` block | reaction | `analog` | `fn` |
| --- | --- | --- | --- | --- | --- |
| param, `signal`, `let`, `branch` | yes |  |  |  |  |
| `=` (nets, definitions) | yes |  |  |  |  |
| generation `for`/`if` over params | yes | yes |  |  |  |
| `on` |  | yes |  |  |  |
| `+>`, `bound_step` |  |  |  | yes |  |
| `<-` |  |  | yes |  |  |
| `emit!` |  |  | yes | yes |  |
| probes, environment, `ddx`, `ac_stim` (`fn`s) | in `let`s and value inputs | in event expressions | yes | yes | yes, and then the `fn` is callable only where these are |
| an instance inside an expression (every module with a result, core or not) | in `let`s and value inputs | in the event of an `on` |  | yes |  |

Two rows replace v0's three: a callable is a `fn` or an instance.

### B.3 Grammar of the additions

```
nature_decl    ::= 'nature' IDENT generics? ( ':' path )? '{' nature_field ( ',' nature_field )* ','? '}'
nature_field   ::= ( 'potential' | 'flow' ) ':' IDENT ( '{' ( IDENT ':' expr ',' )* '}' )?
                 | 'value' ':' type
                 | 'resolve' ':' expr                                           // a fn(&[T]) -> T
struct_field   ::= attrs 'pub'? dir? IDENT ':' ( type | sigtype )               // carrier fields make a bundle
impl_block     ::= 'impl' generics? path generic_args? ( 'for' path generic_args? )? '{' ( attrs 'pub'? fn_decl )* '}'
fn_decl        ::= ( 'extern' STRING )? 'fn' IDENT generics? '(' ( IDENT ':' type ',' )* ')' ( '->' type )? ( block | ';' )
call           ::= path '(' ( expr ',' )* ( IDENT ':' expr ',' )* ')'           // 3.2

module_decl    ::= attrs ( 'extern' STRING )? 'module' IDENT generics?
                   '(' ( port ',' )* ')' ( '->' type )? ( 'where' expr )? '{' module_item* expr? '}'
port           ::= attrs dir? IDENT ':' ( sigtype | type ) ( '=' expr )?         // a value type: a value input
dir            ::= 'in' | 'out'
sigtype        ::= 'flip'? path generic_args? | 'Option' '<' sigtype '>' | '[' sigtype ';' expr ']'
module_item    ::= attrs 'pub'? ( param | signal_decl | 'let' pat '=' expr ';' )
                 | place '=' expr ( 'after' expr )? ';'                         // nets, definitions
                 | gen<module_item>
param          ::= IDENT ':' type ( '=' expr )? ( 'where' expr )? ';'
signal_decl    ::= 'signal' IDENT ':' sigtype ( '=' expr )? ';'

behavior_block ::= 'analog'  names? 'for' path generic_args? '{' ( stmt | contribution )* '}'
                 | 'digital' names? 'for' path generic_args? '{' digital_item* '}'
names          ::= '<' IDENT ( ',' IDENT )* '>'
digital_item   ::= reaction | gen<digital_item>
gen<X>         ::= 'for' pat 'in' expr '{' X* '}' | 'if' expr '{' X* '}' ( 'else' '{' X* '}' )?
reaction       ::= 'on' expr '{' ( stmt | commit )* '}'                      // expr has type Event
contribution   ::= expr '+>' expr ';'                                         // left side: a probe
commit         ::= place '<-' expr ( 'after' expr )? ';'
instance       ::= path generic_args? '(' ( expr ',' )* ( IDENT ':' expr ',' )* ')' ( '{' ( IDENT ':' expr ',' )* '}' )?
template       ::= path generic_args? '{' ( IDENT ':' expr ',' )* ( '..' expr )? '}'
```

`stmt`, `expr`, `type`, `pat`, `block` and `generics` are the value grammar, Rust's apart from Part I's changes. `where` and `;` end a param's default without lookahead. `flip` belongs to the type, so `rx: flip Stream<T>` and `[flip Stream<T>; N]` both parse.

## Appendix C. Implementation

### C.1 Lowering to AIR

Structure (bundles, templates, binning, generics, generation, `m`, value inputs, results, named expressions) is resolved at elaboration, and every function is inlined, so AIR v2 sees flat instances and core primitives only.

| Language | AIR v2 |
| --- | --- |
| param with `where` | param metadata, checked in `setup` |
| live `set`, `on p.changed()` | `live` param, `on change` region |
| known flags | a mask word beside each discrete value, elided where the elaborator proves it all-known |
| `signal x: Bit<T> = v;` | digital state, initialized in `init` |
| a carrier with `resolve` | one state slot per drive and a combinational cell calling `resolve` |
| `prev()`, `since()` | one previous value and one time per carrier, allocated on first commit |
| node, `= v`, `#[nodeset]` | internal node, initial condition, nodeset hint |
| `=` defining a discrete carrier | a combinational cell, evaluated in topological order |
| `=` with `after`, `<-` with `after` | a delayed commit in a per-level transport queue |
| `let` and value inputs | inlined at each read; their instances hoisted to the module |
| `analog for M` | `iter` region |
| `+>` | `+=` contribution, split into resistive and reactive parts |
| switch branch | a branch whose kind is chosen per solve |
| the jump (15.2) | two backward-Euler solves at the same `t` |
| `limit(x, f, seed)` | limited probe with previous-iteration state, seed and RHS correction |
| reaction, `<-` | sequential `on` region, commit |
| `init`, `settle`, `final` | the regions of the same names |
| `at`, `cross` | scheduled and located events |
| `bound_step` | step-control operation |
| `emit!` | stream emit |
| `extern "osdi" module` | foreign (OSDI) instance |
| `extern "cosim" module` | a digital region that calls the foreign step function |

### C.2 Reference implementation

How the Piperine solver meets Part V. Every number is a default the host may change.

| Part V guarantee | Piperine algorithm |
| --- | --- |
| microstep loop (15) | two-phase delta cycle: combinational cells in topological order, then sequential evaluation over one snapshot; commits sorted by (instant made, source index); at most 1000 microsteps per instant |
| the jump (15.2) | a backward-Euler solve with `h` equal to one tick and time not advanced, started from the left value; repeated once from its own result; the 1/h rows are scaled by `h` so the matrix stays conditioned |
| operating point (16) | Newton; then gmin stepping (a shunt on every node from 0.1 S down, a decade per converged solve, adaptive factor); then source stepping (scale from 0 to 1, first step 0.1 growing ×1.5 up to 0.25, halved on failure, a 1 µS knee shunt ramped out at the end); at most 20 mixed rounds |
| initial conditions (16) | a 10¹² S clamp across each seeded branch through the time-0 solve and the first accepted step |
| convergence (17.1) | `reltol` 1e-3, default potential `abstol` 1 µV, flow `abstol` 1 pA, `gmin` 1e-12 S; an update much larger than the last is damped to the midpoint; stamps may be reused while the inputs move less than the tolerance (SPICE bypass), never while limiting |
| truncation error (17.2) | TR-BDF2 (a trapezoidal stage over `γh`, then BDF2 over `(1 − γ)h`, `γ = 2 − √2`); error by Milne's device over node potentials against `trtol` 7; a node whose differences straddle a jump is skipped |
| step control (17.2) | PI controller, `h · (1/lte)^p`, `p = kp + ki · (lte − lte_prev)/lte`, `kp` 0.7, `ki` 0.4, growth clamped to \[0.2, 1.5\]; a rejected step is divided by 8; at `dt_min` an error rejection is accepted with a warning and a Newton failure fails |
| time points (17.2) | a reachability set computed once at elaboration marks the reactions that can touch the analog side; only their scheduled times are breakpoints |
| restart (15.2) | after a jump the trapezoidal stage degrades to backward Euler for one step, and the step restarts at 10⁻³ of the last one |
| rollback (17.2) | one event queue (scheduled commits restored, `at` times and breakpoints re-polled, step bounds and crossings discarded and re-emitted), with the digital state, device state and analog history checkpointed per step |
| crossings (17.3) | secant prediction from the bracketing pair, bisection when it stalls, retake to the predicted time. Today Piperine fires at the first accepted point after the sign change; landing on the prediction is the work left (G.6) |
| limiting (18) | ngspice `pnjlim`, `fetlim`, `limvds`; an instance is active while `abs(x̂ − x) > 1e-6 + 1e-4 · abs(x)`; when a limited branch potential becomes node potentials, the minus node moves and the plus node stays, unless the minus node is `gnd` |
| temperature | `temperature` and `tnom` 300.15 K |

## Appendix D. Gallery: sources, clocks and continuous time

No new semantics; the point is where the time points land.

### D.1 Piecewise-linear source

Each corner changes a `Bit` signal read by `analog`: a time point and a jump (17.2).

```rust
module Pwl<const N: usize>(p: Electrical, n: Electrical) {
    points: [(f64, f64); N];          // (time, value), times increasing
    signal seg: Bit<u32> = 0;
}

digital<N> for Pwl<N> {
    for k in 1..N { on at(points[k].0) { seg <- k as u32; } }
}

analog<N> for Pwl<N> {
    let k = seg.known_or(0) as usize;
    let y = if k + 1 >= N { points[N - 1].1 } else {
        let ((t0, y0), (t1, y1)) = (points[k], points[k + 1]);
        y0 + (y1 - y0) * ((now() - t0) / (t1 - t0)).clamp(0.0, 1.0)
    };
    v(p, n) +> y;
}
```

`points` is not structural (8.1): it feeds no generation, only values, so the host may `set` it during a run. `N` is.

### D.2 Clock with jitter

```rust
module ClockGen(out clk: Bit = false) {
    period: f64 where period > 0.0;
    jitter: f64 = 0.0 where jitter < period / 10.0;    // keeps every half period positive in practice
    seed: u64 = 1;
    signal rng: Bit<u64> = seed;
}

digital for ClockGen {
    on init || clk.changed() {
        let (j, next) = normal(rng, 0.0, jitter);
        rng <- next;
        clk <- !clk after period / 2.0 + j;
    }
}
```

The edges are scheduled; if nothing analog depends on `clk`, they cost the solver nothing. In an analysis without time the first commit never comes due (16.1).

### D.3 Voltage-controlled oscillator

The phase is an integral wrapped to one cycle. The edges are two rising crossings a half cycle apart, never the wrap itself, as in Kundert's models.

```rust
module Vco(vctl: Electrical, out clk: Bit = false) {
    f0: f64 = 1G;
    kvco: f64 = 100M;
    let ph = idtmod(f0 + kvco * v(vctl), 0.0, 1.0, -0.5);    // phase in [-0.5, 0.5)
}

digital for Vco {
    on cross(ph + 0.25) { clk <- true; }
    on cross(ph - 0.25) { clk <- false; }
}

analog for Vco { bound_step(0.05 / f0); }
```

At the wrap `ph` jumps from 0.5 to −0.5; both expressions fall, and `cross` fires on rises only.

### D.4 DAC with finite edges

`transition` (22.2) ramps a piecewise-constant value; a code change mid-ramp starts from the current output. Its argument is an expression over carriers bound to an `in` port: an anonymous definition (8.1).

```rust
module DacRamp(in code: Bit<u8>, vout: Electrical) { vref: f64 = 1.0; tedge: f64 = 100p; }
analog for DacRamp {
    v(vout) +> transition(vref * (code as f64) / 256.0, 0.0, tedge, tedge);
}
```

An unknown `code` makes the argument unknown, which `transition` ignores: the output holds.

### D.5 Sampling at a clock edge: delta-sigma modulator

```rust
module DeltaSigma1(vin: Electrical, in clk: Bit, out bit: Bit = false) {
    r: f64 = 100k; c: f64 = 1p; vref: f64 = 1.0;
    signal x: Electrical = 0.0;
}

analog for DeltaSigma1 {
    let fb = if bit.known_or(false) { vref } else { -vref };
    i(x) +> ddt(c * v(x)) - (v(vin) - fb) / r;
}

digital for DeltaSigma1 {
    on clk.rise() { bit <- v(x) > 0.0; }
}
```

At a rising edge, microstep 0 probes `v(x)` and commits `bit`; microstep 1 applies it, `analog` reads it, so there is a jump at `t`: `c·v(x)` is a state and stays, the current into it changes. Falling edges cost nothing.

### D.6 Switched-capacitor integrator

```rust
module RSwitch(a: Electrical, b: Electrical, in ctl: Bit) { ron: f64 = 100; roff: f64 = 1G; }
analog for RSwitch { i(a, b) +> v(a, b) / if ctl.known_or(false) { ron } else { roff }; }

module OpAmpIdeal(inp: Electrical, inn: Electrical, vout: Electrical) { a0: f64 = 1e5; }
analog for OpAmpIdeal { v(vout) +> a0 * v(inp, inn); }

/// Non-overlapping two-phase clock: pure structure.
module TwoPhase(in clk: Bit, out phi1: Bit = false, out phi2: Bit = false) {
    gap: f64 = 50p;
    phi1 = clk after if clk { gap } else { 0.0 };      // rises gap after clk, falls with it
    phi2 = !clk after if clk { 0.0 } else { gap };     // the mirror
}

/// Stray-insensitive SC integrator: structure only.
module ScIntegrator(vin: Electrical, vout: Electrical, in clk: Bit) {
    cs: f64 = 1p; cf: f64 = 4p;
    signal phi1: Bit;
    signal phi2: Bit;
    signal x: Electrical;
    signal y: Electrical;
    signal vg: Electrical;
    let ck = TwoPhase(clk, phi1, phi2);
    let s1 = RSwitch(vin, x, phi1);
    let s2 = RSwitch(x, gnd, phi2);
    let s3 = RSwitch(y, gnd, phi1);
    let s4 = RSwitch(y, vg, phi2);
    let csmp = Capacitor(x, y) { c: cs };
    let cfb = Capacitor(vg, vout) { c: cf };
    let amp = OpAmpIdeal(gnd, vg, vout);
}
```

Four time points per period, one at each phase edge. `TwoPhase` is two definitions with value-dependent delays. If `clk` falls before `gap` has elapsed, the fall is scheduled for now and removes the pending rise (12.1 rule 4): `phi1` never sticks high. With ideal `Switch`es in place of `RSwitch`, each phase edge is a charge-conserving jump (15.2, F.5).

### D.7 Guidance

- Read a discrete carrier in `analog` only when the analog behavior depends on it, and say what unknown means there (`known_or`).
- Prefer known times (`at`, `after`) to crossings.
- Sample at a clock edge with a reaction, not with `cross` on the clock's analog image.
- Make the edges of a phase with offset crossings, not with the wrap of `idtmod`.
- Give edges a duration (`transition`) when the edge rate matters to the analog side.
- Make every corner of a `now()` waveform a carrier change.

## Appendix E. Gallery: digital

Discrete carriers only: none of these touches the solver.

### E.1 Flip-flop with asynchronous reset

```rust
module Dff<T: Repr>(in clk: Bit, in rst_n: Bit, in d: Bit<T>, out q: Bit<T>) { reset: T; }
digital<T> for Dff<T> {
    on clk.rise() || rst_n.fall() { if !rst_n { q <- reset; } else { q <- d; } }
}
```

`q` is unknown until the first edge or reset. With `rst_n` unknown, both arms run and merge (3.3): `q` keeps the levels on which `reset` and `d` agree. Verilog's behavior, `else` on unknown, is `if !rst_n.known_or(true)`.

### E.2 Latches

```rust
module DLatch<T: Repr>(in en: Bit, in d: Bit<T>, out q: Bit<T>) {}
digital<T> for DLatch<T> {
    on init || en.changed() || d.changed() { if en { q <- d; } }    // init: transparent from time 0
}

/// Cross-coupled NOR gates with gate delay: pure structure.
module SrLatch(in s: Bit, in r: Bit, out q: Bit = false, out qn: Bit = true) {
    tpd: f64 = 10p where tpd > 0.0;
    q  = !(r | qn) after tpd;
    qn = !(s | q)  after tpd;
}
```

Level-sensitive logic includes `init`, because initial values are not changes (13 rule 6). With `en` unknown the latch output is the merge of `d` and what it held. Releasing `s` and `r` together oscillates with period `2·tpd`, as the ideal circuit does. Without `after`, the two definitions would form a combinational loop, an elaboration error.

### E.3 Pipelines

Reads see the past, so one reaction describes a whole pipeline, in any statement order:

```rust
module MacPipe(in clk: Bit, in a: Bit<i16>, in b: Bit<i16>, in valid_in: Bit,
               out acc: Bit<i40> = 0, out valid_out: Bit = false) {
    signal a1: Bit<i16>;
    signal b1: Bit<i16>;
    signal v1: Bit = false;
    signal p2: Bit<i32>;
    signal v2: Bit = false;
}

digital for MacPipe {
    on clk.rise() {
        a1 <- a; b1 <- b; v1 <- valid_in;                    // stage 1
        p2 <- (a1 as i32) * (b1 as i32); v2 <- v1;           // stage 2
        if v2 { acc <- acc + p2 as i40; } valid_out <- v2;   // stage 3
    }
}

module Delay<T: Repr, const N: usize>(in clk: Bit, in d: Bit<T>, out q: Bit<T>) {
    signal r: [Bit<T>; N];
    q = r[N - 1];
}
digital<T, N> for Delay<T, N> {
    on clk.rise() { r[0] <- d; for k in 1..N { r[k] <- r[k - 1]; } }
}
```

### E.4 State machine with an enum

```rust
/// Tx is the enum of section 2.
module UartTx(in clk: Bit, in baud_tick: Bit, in start: Bit, in byte: Bit<u8>,
              out txd: Bit = true, out busy: Bit) {
    signal st: Bit<Tx> = Tx::Idle;
    signal sh: Bit<u8> = 0;
    busy = st != Tx::Idle;
}

digital for UartTx {
    on clk.rise() {
        match st {
            Tx::Idle => if start { sh <- byte; st <- Tx::Start; },
            _ if !baud_tick => {}
            Tx::Start => { txd <- false; st <- Tx::Data { n: 0 }; }
            Tx::Data { n } => {
                txd <- (sh & 1) == 1;
                sh <- sh >> 1;
                st <- if n == 7 { Tx::Stop } else { Tx::Data { n: n + 1 } };
            }
            Tx::Stop => { txd <- true; st <- Tx::Idle; }
        }
    }
}
```

An unknown `st` runs every arm its known levels allow and merges their commits: the machine stays unknown until something resets it, instead of silently falling into one state.

### E.5 Memory

```rust
module Ram(in clk: Bit, in we: Bit, in addr: Bit<u8>, in wdata: Bit<u32>, out rdata: Bit<u32>) {
    signal mem: [Bit<u32>; 256];         // one carrier per word, materialized on first write (5.1)
}
digital for Ram {
    on clk.rise() {
        if we { mem[addr] <- wdata; }
        rdata <- mem[addr];              // read-before-write
    }
}
```

An asynchronous read port is a definition, `rdata = mem[addr];`. An index is control (3.3 rule 3): an unknown address reads the merge of every word it could select, and a write through it merges `wdata` into each of them.

### E.6 Stream register slice

```rust
module Slice<T: Repr>(in clk: Bit, rx: flip Stream<T>, tx: Stream<T>) {
    signal full: Bit = false;
    signal buf: Bit<T>;
    tx.data = buf;
    tx.valid = full;
    rx.ready = !full || tx.ready;
}
digital<T> for Slice<T> {
    on rx.beat(clk) { buf <- rx.data; full <- true; }
    on clk.rise() && !(rx.valid && rx.ready) && tx.ready { full <- false; }
}
```

### E.7 Open-drain interrupt line

```rust
module IrqSource(in req: Bit, irq_n: Wire) { irq_n = if req { Logic::strong(false) } else { Logic::Z }; }

module IrqController(in clk: Bit, irq_n: Wire, out pending: Bit = false) {}
digital for IrqController { on clk.rise() { pending <- !irq_n.v; } }

module Board(in clk: Bit, in req0: Bit, in req1: Bit, out pending: Bit) {
    signal irq_n: Wire;
    let pu = Pullup(irq_n);                  // wins only while every source is released
    let s0 = IrqSource(req0, irq_n);
    let s1 = IrqSource(req1, irq_n);
    let ic = IrqController(clk, irq_n, pending);
}
```

A third source is one more line anywhere `irq_n` is visible; nothing counts drivers.

## Appendix F. Gallery: mixed-signal systems

### F.1 SAR ADC

```rust
/// 8-bit DAC: ideal level, ladder resistance, output switch.
module Dac8(in code: Bit<u8>, vout: Electrical) {
    vref: f64 = 1.0; rout: f64 = 1k; rsw: f64 = 200;
    signal src: Electrical;
    pub signal core: Electrical;
}
analog for Dac8 {
    v(src) +> vref * (code.known_or(0x80) as f64) / 256.0;     // unknown code: mid-scale
    i(src, core) +> v(src, core) / rout;
    i(core, vout) +> v(core, vout) / rsw;
}

/// Binary search; cmp is high when the input is above the DAC.
module SarLogic(in clk: Bit, in start: Bit, in cmp: Bit, out code: Bit<u8> = 0x80, out done: Bit = false) {
    signal bit: Bit<u3> = 7;
    signal busy: Bit = false;
}
digital for SarLogic {
    on clk.rise() && start && !busy { busy <- true; bit <- 7; code <- 0x80; done <- false; }
    on clk.rise() && busy {
        let kept = if cmp { code } else { code & !(1u8 << bit) };
        if bit == 0 { code <- kept; done <- true; busy <- false; }
        else { code <- kept | (1u8 << (bit - 1)); bit <- bit - 1; }
    }
}

module SarAdc(vin: Electrical, in clk: Bit, in start: Bit, out code: Bit<u8>, out done: Bit) {
    cpar: f64 = 50f;
    signal vh: Electrical;
    signal vdac: Electrical;
    signal cmp: Bit;
    let sh = SampleHold(vin, vh, start);      // 12.1
    let dac = Dac8(code, vdac);
    let comp = Comparator(vh, vdac, cmp);    // 13
    let logic = SarLogic(clk, start, cmp, code, done);
}
analog for SarAdc { i(dac.core) +> ddt(cpar * v(dac.core)); }    // the parent's own branch on a public node
```

Time points per conversion: `start` (the sample probes), eight code changes (the DAC reads `code`), and each located comparator crossing. The two `SarLogic` reactions never fire together (`busy` separates them), so the `collision` lint is silent.

### F.2 Charge-pump PLL

```rust
module Pfd(in fref: Bit, in fdiv: Bit, out up: Bit = false, out dn: Bit = false) { treset: f64 = 50p; }
digital for Pfd {
    on fref.rise() { up <- true; }
    on fdiv.rise() { dn <- true; }
    on (up.changed() || dn.changed()) && up && dn { up <- false after treset; dn <- false after treset; }
}

module ChargePump(in up: Bit, in dn: Bit, vc: Electrical) { icp: f64 = 100u; }
analog for ChargePump {
    i(vc) +> if dn.known_or(false) { icp } else { 0.0 } - if up.known_or(false) { icp } else { 0.0 };
}

module Divider<const N: u32>(in clk: Bit, out q: Bit = false) { signal cnt: Bit<u32> = 0; }
digital<N> for Divider<N> {
    on clk.rise() { if cnt == N / 2 - 1 { cnt <- 0; q <- !q; } else { cnt <- cnt + 1; } }
}

/// Pure structure.
module Pll(in fref: Bit, out clk: Bit) {
    r: f64 = 10k; c1: f64 = 10p; c2: f64 = 1p;
    signal up: Bit;
    signal dn: Bit;
    signal fdiv: Bit;
    signal vc: Electrical = 0.0;
    signal mid: Electrical = 0.0;
    let pfd = Pfd(fref, fdiv, up, dn);
    let cp = ChargePump(up, dn, vc);
    let rz = Resistor(vc, mid) { r: r };
    let cz = Capacitor(mid, gnd) { c: c1 };
    let cp2 = Capacitor(vc, gnd) { c: c2 };
    let vco = Vco(vc, clk);                  // D.3
    let div = Divider::<8>(clk, fdiv);
}
```

An `fref` edge landing on the tick where the delayed reset applies is ordinary: the reset applies at `(t, 0)`, the edge's reaction runs at `(t, 0)` and its `up <- true` applies at `(t, 1)`. No two commits meet, and nothing is an error (12.1). Time points: `fref` edges (their reaction can change `up`, which `analog` reads), reset times, and each VCO edge. A VCO edge that only advances the divider's counter is a located crossing anyway.

### F.3 DRAM array with a swappable cell

```rust
/// Behavioral cell with the ports of Dram1T1C (8.4), for fast runs.
module BehavioralCell(c: Cell) { vdd: f64 = 1.0; ron: f64 = 2k; signal bit: Bit = false; }
analog for BehavioralCell {
    if c.wl.known_or(false) { i(c.bl) +> (v(c.bl) - if bit.known_or(false) { vdd } else { 0.0 }) / ron; }
}
digital for BehavioralCell {
    on c.wl.fall() { bit <- v(c.bl) > vdd / 2.0; }
}

module SenseAmp(bl: Electrical, vref: Electrical, in en: Bit, out q: Bit) {}
digital for SenseAmp { on en.rise() { q <- v(bl, vref) > 0.0; } }

module Dram<const ROWS: u32, const COLS: u32>(in wl: [Bit; ROWS], in pre: Bit, in sense: Bit, out q: [Bit; COLS]) {
    cbl: f64 = 100f; vpre: f64 = 0.5;
    signal bl: [Electrical; COLS];
    signal vref: Electrical;
    pub let bank = MemoryBank::<_, ROWS, COLS>(wl, bl) { cell: Dram1T1C { cs: 30f } };   // 8.4
    for c in 0..COLS {
        let cb = Capacitor(bl[c], gnd) { c: cbl };
        let pc = RSwitch(bl[c], vref, pre);  // D.6
        let sa = SenseAmp(bl[c], vref, sense, q[c]);
    }
}
analog<ROWS, COLS> for Dram<ROWS, COLS> { v(vref) +> vpre; }
```

`BehavioralCell {}` in place of `Dram1T1C { cs: 30f }` swaps every cell without touching `MemoryBank`: both satisfy `Fn(Cell)`. A storage node is probed as `bank.cells[3][5].sn`.

### F.4 Two tristate buses joined by a pass gate

```rust
/// A device on a shared line: drives when enabled, always listens.
module Dev(in d: Bit, in oe: Bit, out seen: Bit, pad: Wire) {
    pad = if oe { Logic::strong(d) } else { Logic::Z };
    seen = pad.v;
}

module Board(in en: Bit, in d: [Bit; 4], in oe: [Bit; 4], out seen: [Bit; 4]) {
    signal la: Wire;
    signal lb: Wire;
    let a0 = Dev(d[0], oe[0], seen[0], la);
    let a1 = Dev(d[1], oe[1], seen[1], la);
    let b0 = Dev(d[2], oe[2], seen[2], lb);
    let b1 = Dev(d[3], oe[3], seen[3], lb);
    let gate = Tran(la, lb, en);             // 22.3
    let keep = Keeper(lb);
}
```

| Situation | `la` | `lb` |
| --- | --- | --- |
| `oe[0]`, gate on | strong `d[0]` | strong `d[0]`, through the gate |
| `oe[0]` and `oe[2]`, gate on, levels differ | strong, level unknown | strong, level unknown |
| nobody enabled, gate on | weak, the keeper's level, through the gate | weak, the keeper's level |
| nobody enabled, gate off | high impedance (`s == HighZ`, level unknown) | weak, the keeper's level |

Undriven, contended and kept are three different values, told apart by the strength. There is no loop: the gate's drive of `la` reads `lb.others()`, which is `b0`, `b1` and the keeper, not the gate's own drive of `lb`.

### F.5 Charge sharing through an ideal switch

```rust
module ShareCharge(a: Electrical, b: Electrical, in close: Bit) {
    let c1 = Capacitor(a, gnd) { c: 1p };      // a at 1 V
    let c2 = Capacitor(b, gnd) { c: 3p };      // b at 0 V
    let sw = Switch(a, b, close);              // 11: a short while close is high
}
```

At `close.rise()` the jump rule (15.2) writes, with `i` the switch flow and `h → 0`:

| Row | Equation | |
| --- | --- | --- |
| node `a` | `1p · (v(a) − 1) / h + i = 0` | the capacitor's `ddt` reads `(q − q⁻)/h` |
| node `b` | `3p · (v(b) − 0) / h − i = 0` |  |
| switch | `v(a) − v(b) = 0` | the constraint that forces the jump |

Adding the first two rows removes `i`: `1p·(v − 1) + 3p·v = 0`, so `v(a) = v(b) = 0.25 V`. The charge `1 pC` is conserved; `i = 0.75 pC / h` is the impulse and is never observed. The second pass starts from 0.25 V on both, moves nothing, and gives `i = 0`, which is what a probe reads at the next microstep.

## Appendix G. Scope and open questions

**Out of scope:** formal-verification constructs; the SystemVerilog verification language (the testbench lives in the host, 23); blocking writes and `#0`; indirect contributions; heap types; reading files during a run; implicit converters; global nets other than `gnd`; physical units in the type system (units are labels); charge-storage strengths on nets (`trireg` sizes), which are analog.

**Highest impact:** G.6 (crossings landed on time, the jump solve), G.8 (import), G.21 (lossy lines), G.22 (load-dependent delay).

**G.1 Name.** Distinct from Piperine (the toolchain) and AIR (the IR).

**G.2 The `collision` lint.** How far the static check of 12.1 rule 2 goes before staying silent.

**G.3 The `nonsmooth` lint.** Too coarse for region models that are smooth by construction.

**G.4 Bundles with extra ports.** A real DRAM cell also needs a plate voltage: put it in `Cell`, or give `Fn` a second argument.

**G.5 Result syntax.** A module with a result ends its body with the result expression (8.3). Alternatives: a named result port, or `-> name: T`.

**G.6 AIR and Piperine gaps.** Switch branches, the jump as a zero-length backward-Euler solve, known flags, resolved natures, user `limit` functions, integer time, value inputs and results, and landing on predicted crossings (Piperine fires at the first accepted point after the sign change today).

**G.8 Import.** Running Yosys netlists needs its own part: the netlist form consumed (RTLIL or cell netlists) and a cell-to-language table (`$mux` to `if`, `$tribuf` to `Tribuf` on a `Wire`, `$dff` to a reaction, `inout` to a `Wire` port). Several writers in a behavioral netlist are several drives of a resolved carrier.

**G.9 Real-valued change.** `Bit<f64>` changes when its levels change: `0.0 → -0.0` is a change, the same `NaN` is not.

**G.10 Time resolution.** A session option (15); may one design mix resolutions?

**G.11 Custom floats.** `Float<E, M>` for bfloat16 and 8-bit formats.

**G.13 Metastability.** A stdlib synchronizer whose first stage commits `Unknown` when `d.since() < tw` at the edge.

**G.14 Field-init shorthand.** `{ r }` for `{ r: r }` in instances and templates.

**G.15 Keywords as names.** Accept raw identifiers (`r#out`), listed without `r#` in metadata.

**G.16 Shadowed probes.** Warn when a local takes the name of a probe in scope.

**G.17 The top module.** Left to the host: any module without ports, or one it names.

**G.19 Depth of history.** One previous value and one time (5.3). Longer patterns are state; confirm it is enough.

**G.20 Reductions.** An explicit signal per reduction (19) is plain but verbose; stdlib modules with a result, `running_max(x)`, would shorten it.

**G.21 Lossy lines and measured responses.** Lossy and coupled lines (SPICE `O`, `Y`, `P`) and S-parameter blocks need a convolution over the input's history. Rational fitting into `laplace_zp` in the stdlib, or a core `convolve(x, h, tmax)`?

**G.22 Load-dependent delay.** A gate's delay often depends on the load it drives (XSPICE input loads, Liberty NLDM tables), and a module cannot see what its output connects to. On a resolved carrier the load can be a field of the drive type that `resolve` sums; confirm that this is enough before adding an elaboration query.

**G.23 Bit fields on values.** Slices and concatenations of integers (Yosys `$slice`, `$concat`) are written with shifts, masks or `cast`. A stdlib `bits::<H, L>(x)` and `concat!(a, b)` would read better.

**G.24 Frequency-domain post-processing.** `.measure` over an AC sweep has no reaction to run in. Measures over complex results belong to the host; confirm, or add a hook.

**G.27 Rings of pass gates.** A cycle of `Tran`s is a combinational loop (22.3). Verilog resolves it with a switch-level relaxation. Options: a delay in `Tran`, an analog model, or evaluating `resolve` over the whole connected component when every element of the cycle is a `Tran`.

**G.28 Cost of unknown control.** An unknown address merges into every word it could select (E.5), which is exact and can be expensive on a large memory. The elaborator may warn; the semantics stays.

**G.29 `known_or` on a module's own state.** 3.3 rule 5 asks for it even on a signal the module always commits known (`seg` in D.1). A static proof of known-ness could waive it; the rule without exceptions was preferred.

**G.30 Small-signal response of other sampled blocks.** `zi_nd` states its own (11.1, 22.2). A general sampled system (a digital loop filter between an ADC and a DAC) has none unless written the same way; periodic small-signal analysis (`Pss`) is the general answer.

Closed since v0: G.7 (inertial delay: stdlib `Inertial`), G.12 (threshold view: `A2D`), G.18 (contracts: bundles and `Fn(P)`), G.25 (more enumerated natures: any `nature { value: T }`), G.26 (`above` is stdlib).

## Appendix H. Change log

| Date | Change |
| --- | --- |
| 2026-11-12 | Round 1 of the spec duel (`changelog_v1.md`). A nature says how drivers combine: `Bit` left the core for `nature Bit<T> { value: T }`, resolved natures (`Wire`, `Tri`, `SumNet`) replaced the bus modules and `Pin`. Unknown is a flag on every value, with one control-flow rule (merge). The jump is a zero-length backward-Euler step. Two kinds of callable: `fn` and module; `ddt`, `cross` and the rest are core modules with a result; value inputs and `let` are expressions read in place. Last write wins by source order; delays are transport with removal. Initial values are not changes. Analyses without time defined. A branch's kind is constant during a solve. Removed: `interface`, views, `impl` on modules, `above`/`absdelta`/`transition` from the core, `discontinuity()`, `finish()`, `Dir`, `held()`, the `in` range on params, eight attributes, parent contributions into a child's branch. |
| 2026-10-26 | Second review and an internal audit. An instance may read its own result through state (8.3); gradients across events (23.3); tolerance rules (4, 17.1); the monitors shown, reporting only (22.5); i(p) and inflow() told apart (6); import moved from the header to G.8. |
| 2026-10-26 | Ten concepts. `=` in the module body covers net identity and combinational definitions. Functions are pure; anything with state is a module. `analog` has only equations and `digital` only reactions. `accept` merged into `settle`; `timer` became `at` with a period. Part V states guarantees; Piperine's algorithms moved to C.2. The testbench moved to the host (Part VIII). |
| 2026-10-26 | `wait` and `sleep` removed; optional parameters as trailing `Option`s for every callable; `dtemp`, value-dependent delays; a capability check against Yosys and SPICE. |
| 2026-10-25 | Two kinds of nature made explicit; `check`, `process`, `->`, `event fn`, `analog fn`, `SparseMem`, `History`, `~` and `%` removed. |
| 2026-10-24 | The 38 gaps and 5 inconsistencies of the first draft's audit against VAMS 2.4, IEEE 1800 and Piperine accepted; the record stays in the first draft. |
