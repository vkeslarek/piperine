# Mixed-signal language: specification

Oct 26, 2026 · @Vinicius Keslarek

A language for mixed-signal models, similar to Rust and based on it, but not Rust. It models, simulates and optimizes digital, analog and mixed circuits, and it is free of races by construction. It reuses existing models: Verilog-A compiled to OSDI, and other simulators by cosimulation; importing Yosys netlists is planned (G.8). Testbenches live in the host (Part VIII).

## 0. Concepts

The language has ten concepts. Each has one rule.

| # | Concept | Rule | § |
| --- | --- | --- | --- |
| 1 | value and `fn` | data with no time; a function is pure, may read probes and may return an `Event` | 1–3 |
| 2 | nature | `Bit` is the one enumerated nature (levels, changed by events); every other nature is continuous (potential and flow, solved) | 4 |
| 3 | carrier | a `Bit<T>` signal or a node: something that exists over time and can be connected | 5–6 |
| 4 | module | ports, params, signals, instances, and `=`: a wire between nodes or a combinational definition | 8 |
| 5 | instance | a module placed; a module with a result can also be placed inside an expression | 8.3 |
| 6 | `analog for M` | equations, written with `+>`, evaluated at every Newton iteration | 11 |
| 7 | `digital for M` | reactions, `on ev { q <- x; }`, run when an event fires | 12 |
| 8 | event | a value of type `Event`: primitives, `\|\|`, guards | 13 |
| 9 | interface | a named bundle of carriers with directions | 7 |
| 10 | input and output | `emit!` writes records; `finish()` ends the run | 19 |

**State is structure.** What keeps state is a signal declared in a module, and nothing else: a function never keeps state, and anything with state is a module.

### Who writes what

| Form | Target | Written in | Takes effect |
| --- | --- | --- | --- |
| `a = b;` | two nodes: one node | module body | at elaboration |
| `y = expr;` | a `Bit` carrier: a combinational definition | module body | in the microstep its inputs change, before reactions run (8.2) |
| `branch +> expr;` | a branch: its potential or its flow | `analog` | in the current Newton iteration |
| `q <- expr [after dt];` | a `Bit` carrier: a commit | a reaction | at the next microstep, or `dt` later |

Params have no assignment form: models read them, the host sets them.

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

The module body is the netlist: ports, params, two signals, one definition. Behavior is in the `digital` block: three reactions, two on crossings of the input and one on the clock. Nothing in the model keeps state outside its signals.

## Part I. Values

Values are data with no time. Everything a model computes with is a value; what it connects to is a carrier (Part II). The value world follows Rust's, with the differences stated here.

### 1. Value types

| Kind | Types | Notes |
| --- | --- | --- |
| boolean | `bool` |  |
| integer | `UInt<N>`, `Int<N>` (`u1` … `u128`, `i1` … `i128`) | any width, a const expression (`UInt<{A + 1}>`); arithmetic wraps |
| float | `Float<N>`, `N` in {16, 32, 64, 128} (`f16` … `f128`) | IEEE 754; analog code computes in `f64` |
| string | `&'static str` | literals only; never carried |
| composite | `[T; N]`, tuples, `struct`, `enum` | as in Rust |

1. `as` truncates or extends; `checked_*` and `saturating_*` work as in Rust.
2. Left out, because a model is fixed at elaboration: references, heap types, `unsafe`, threads, `async`, `dyn`.
3. Constants are `const` items; the prelude has `Q`, `K_B`, `EPS0`, `PI`, `E`, `inf`.
4. Only a type with `Repr` can be carried.

### 2. `Repr`

`Repr` lays a value type out on the levels of `Bit`. It is the only bridge from values to carriers.

```rust
pub trait Repr: Copy {
    const WIDTH: u32;                                   // levels occupied
    fn encode(self) -> [bool; Self::WIDTH];
    fn decode(levels: [bool; Self::WIDTH]) -> Option<Self>;
}
```

1. `bool` is one level. Integers and floats are their bit pattern, least significant first; arrays concatenate.
2. `#[derive(Repr)]` on a struct concatenates the fields, first field lowest; on an enum, a tag then the widest payload. `decode` returns `None` for an unused tag, which reads as unknown (5.2). `#[repr(onehot)]`, `#[repr(gray)]`, `#[repr(uN)]` and explicit discriminants choose the encoding; behavior never depends on it.

```rust
#[derive(Repr, Clone, Copy, PartialEq)]
enum Tx { Idle, Start, Data { n: u3 }, Stop }      // 2-level tag + 3-level payload

#[derive(Repr, Clone, Copy, PartialEq)] #[repr(u2)]
enum Op { Add = 0b00, Sub = 0b01, And = 0b10, Or = 0b11 }

#[derive(Repr, Clone, Copy, PartialEq)]
enum Pam4 { L0, L1, L2, L3 }                        // a PAM4 symbol is a value; its voltage is analog
```

### 3. Functions

A function is computation without time. It is pure: it keeps no state and writes nothing. What it reads decides where it may be called.

| The body… | Callable in |
| --- | --- |
| uses values and reads `Bit` carriers | anywhere |
| probes (`v(a)`, `i(br)`) | `analog`, event expressions, reaction bodies, instance arguments |
| returns `Event` (13) | event positions; as `bool` inside a reaction |

1. Inside `analog`, calls are differentiated through.
2. `extern fn` is implemented outside (C ABI or a Rust crate); inside `analog` it needs `#[derivative(f)]`.
3. Closures `|a, b| expr` are values and may be passed to functions; they capture by value.
4. Something with state, such as an integrator, is a module with a result (8.3), never a function.

```rust
pub fn diode_current(vd: f64, is: f64, nvt: f64) -> f64 { is * ((vd / nvt).exp() - 1.0) }
pub fn handshake(req: Bit, ack: Bit) -> Event { req.rise() && ack }
#[derivative(erfcx_dx)] extern fn erfcx(x: f64) -> f64;
```

#### 3.1 `impl`

`impl X { … }` attaches functions to a type. It holds functions only, so it has no time.

| `impl` on | `self` is | Typical use |
| --- | --- | --- |
| value type | the value | methods, trait implementations |
| `Bit` or a continuous nature | one carrier, with its history (5.3) | events: `rise`, `fall`, `reaches(k)` |
| interface | the bundle | events and queries over several carriers |
| module | the instance, seen from inside | helpers its blocks share; private, never called by the parent |

1. On a hardware type, `self` is a handle that reads, never a copy.
2. Generic names follow `impl`; bounds come from the declaration (`impl<T> Stream<T>`).
3. A derived nature inherits its base's functions and may add new ones, never redefine them.
4. Names may not repeat a carrier method: `changed`, `prev`, `held`, `since`, `level`, `inflow`.

```rust
impl Thermal {
    pub fn reaches(self, k: f64) -> Event { above(temp(self) - k) }   // above: right from time 0 (16)
}
```

#### 3.2 Parameters and calls

One rule covers functions, methods, events, core primitives and instances; there is no overloading, no variadic call and no default value in a signature.

1. Arguments are positional.
2. Trailing parameters of type `Option<T>` may be left out; a left-out parameter is `None`.
3. An argument of type `T` passed for a parameter of type `Option<T>` is `Some` of it. This is the only implicit conversion in the language.
4. After the positional arguments, optional parameters may be passed by name, `name: value`, which lets a call skip earlier ones.
5. The callee decides what `None` means, usually with `unwrap_or`.

```rust
cross(e)                    // both directions, simulator tolerances
cross(e, Rising, 1p)        // with ttol
cross(e, etol: 1u)          // skips dir and ttol
idt(x)                      // an integrator from 0, no reset
idt(x, 1.0, reset: rst)
```

Module params follow the same rule at an instance, `Name(ports) { name: value }`. A module param may also have a default, `r: f64 = 1k`, because the host lists and sets params (21).

#### 3.3 Files and imports

One file is one module, `mod` declares submodules, `use` brings names into scope, `pub` controls visibility, and a package of models is a crate. The standard library prelude is imported implicitly.

## Part II. Carriers

A carrier exists over time and can be connected. Every port and internal signal is a carrier, and its type is a **nature**. There are two kinds of nature, and they never mix.

### 4. Natures

|  | `Bit`, the enumerated nature | a continuous nature |
| --- | --- | --- |
| declared | built in | `nature Name { potential: …, flow: … }` |
| carrier | `Bit<T>`: a `T: Repr` on `WIDTH` levels | a node, and branches between nodes |
| holds | levels `L` and `H`, each stable or unknown | a real quantity; complex in small-signal analyses |
| computed by | the event engine | the analog solver |
| past | the carrier's history (5.3) | located events and modules with state |
| drivers | one; a shared line is a module (5.4) | conservative: any number, by KCL; signal-flow: one |
| written by | `=` in the body, `<-` in reactions | `+>` in `analog` |

```rust
nature Electrical {
    potential: Voltage { access: v, units: "V", abstol: 1u },
    flow:      Current { access: i, units: "A", abstol: 1p, idt_abstol: 1f },
}
nature Thermal {
    potential: Temperature { access: temp, units: "K", abstol: 1m },
    flow:      Power       { access: pwr,  units: "W", abstol: 1n },
}
nature VSignal { potential: Voltage { access: v, units: "V", abstol: 1u } }   // signal-flow
nature HighVoltage: Electrical { potential: Voltage { abstol: 1m } }          // derived
```

1. A continuous nature declares `potential` and optionally `flow`; without `flow` it is signal-flow. A derived nature overrides attributes of its base.
2. `access` names the probe. `units` is metadata, never checked. `abstol` is the tolerance of that quantity's unknowns and residuals, and the default `etol` of crossings.
3. Choose each `abstol` below the smallest difference that matters: the potential's below the smallest meaningful change of the quantity, the flow's below the smallest meaningful flow or rate. A user state is best normalized to order one, so that round tolerances fit (11.2).
4. Quantity names are labels: two natures whose potentials share a name may connect.

### 5. `Bit` carriers

`Bit<T>` holds a value of `T` and changes only at events. Every level is `false` (L) or `true` (H), and **stable** or **unknown**; unknown is not a third level but the record that the value is not established. `Bit` alone means `Bit<bool>`.

#### 5.1 Declaring and reading

```rust
signal en:    Bit;                       // starts unknown
signal count: Bit<u8> = 0;
signal level: Bit<f64> = 0.0;            // real-number modeling: piecewise constant
signal st:    Bit<Tx> = Tx::Idle;
signal lines: [Bit; 8];                  // eight carriers, each with its own events
signal bus:   Bit<u4> = logic!("10xx");  // level by level; x is unknown
```

1. Without `= value`, every level starts unknown; `q <- Unknown;` commits an unknown value.
2. A `Bit<f64>` is constant during every Newton solve and has no derivative.
3. `q.level(k)` reads level `k` as `Option<bool>`, `None` when unknown; `q.prev().level(k)` reads it in the previous value.

#### 5.2 Unknown values

The rule is IEEE 1364's wherever Verilog defines one.

| Context | Rule |
| --- | --- |
| `&`, `\|`, `^`, `!` | per level: `unknown & false == false`, `unknown \| true == true`, otherwise unknown |
| arithmetic, shifts, comparisons | any unknown level makes the result unknown |
| an `if` that yields a value, with an unknown condition | both values merged level by level: equal levels kept, different levels unknown (Verilog `?:`) |
| an `if` that commits, with an unknown condition | the `else` branch, recorded on `stderr` (Verilog `if`) |
| `match` on an unknown value | the `_` arm, recorded (Verilog `case`) |
| an index that is unknown or out of range | reads unknown; a commit through it is dropped and recorded |
| copy, `y <- x` | levels and their stability are copied |
| an edge | IEEE 1364 `posedge`: `L → H`, `L → unknown`, `unknown → H` (22.1) |
| read inside `analog` | an error at that instant naming the carrier; `x.known_or(d)` reads explicitly |

A level becomes unknown with no initial value, a failed `decode`, arithmetic on an unknown, or a commit of `Unknown`: a bus in conflict, a converter that cannot decide, a failed timing check.

#### 5.3 History

Every `Bit` carrier, and every param, keeps one previous value and two times:

| Method | Gives |
| --- | --- |
| `s.changed()` | `Event`: `s` changed in this microstep |
| `s.prev()` | the value before the most recent change |
| `s.held()` | how long that previous value lasted |
| `s.since()` | the time since the most recent change |

A node has no `prev()`: a value one step ago would depend on the step size. Its past is reached through located events and modules with state. Longer memory is a signal written by a reaction.

#### 5.4 One driver per carrier

A `Bit` carrier has one owner, the instance that declares it or drives the `out` port, and inside it one source: either one `=` definition (8.2) or the reactions of that module, never both. Several reactions may commit to it, under the collision rule (12.1). A second driver is an elaboration error.

There is no resolution and no high impedance. A line shared by several devices is hardware: a module that takes one carrier per device and drives the line (`TriBus`, `OpenDrainBus`, `WiredOr`, `SumNet`, 22.3). A device that drives and reads a shared line has a `Pin`:

```rust
interface Pin<T: Repr> { out o: Bit<T>, out oe: Bit, in i: Bit<T> }    // value, drive enable, line
```

#### 5.5 Views

A definition `y = x;` between carriers of the same width but different value types is a view: `y` shows the levels of `x` under its own type, in the same microstep. An array of one-level carriers and an integer of the same width are views of each other. Writes go to the source; the view is read-only.

```rust
/// Shift register, LSB first: the register is the array, the byte a view of it.
module UartShift(in clk: Bit, in rx: Bit, out byte: Bit<u8>) {
    signal sh: [Bit; 8];
    byte = sh;
}
digital for UartShift { on clk.rise() { sh[7] <- rx; for k in 0..7 { sh[k] <- sh[k + 1]; } } }
```

### 6. Continuous carriers

A node of a continuous nature; the solver computes its value. A **branch** runs between two nodes; models contribute to it and probe it.

```rust
signal mid: Electrical;          // internal node
signal out: Electrical = 0.0;    // initial condition of its potential (SPICE ic)
```

| Probe | Reads |
| --- | --- |
| `v(a, b)`, `v(a)` | potential of the branch `(a, b)`; `b` is an `Option`, `gnd` when left out |
| `i(a, b)`, `i(a)` | its flow, from `a` to `b` through the module |
| `v(br)`, `i(br)` | a named branch, `let br = branch(a, b);` in the module body |
| `p.inflow()` | flow entering the module at its port `p` (VAMS `I(<p>)`) |

1. `gnd` is the reference node of every conservative nature. It is not a global net: a model reaches any other node only through its ports.
2. A signal-flow node has exactly one potential driver, checked at elaboration.
3. Two independent branches between the same nodes need names.
4. Every `access` has the signature `fn v<T: Terminals>(a: T, b: Option<Node>) -> f64`; nodes and named branches implement `Terminals`.
5. `i(p)` is the flow of this module's branch `(p, gnd)`; `p.inflow()` is the total flow of all this module's branches at `p`. They are equal only when that branch is the module's only one at `p`.

#### 6.1 Crossing between kinds

A `Bit` carrier is never connected to a node, and no converter is inserted implicitly: crossing is a module (stdlib `D2A`, `A2D`, `Bidir`, 22.4). Inside a module, behavior crosses freely: `analog` reads `Bit` carriers, and events and reactions probe.

### 7. Interfaces

A named bundle of carriers with directions, written from the producer's side; nodes have none. An interface has no behavior with time, and may have functions.

| Use | Written |
| --- | --- |
| producer view | `tx: Stream<T>` |
| consumer view, every direction reversed | `rx: flip Stream<T>` |
| observer view, every field an input | `in s: Stream<T>` |
| internal bundle | `signal a: Stream<T>;` |
| port list of a module | `module Dram1T1C: Cell { … }` (8.4) |

```rust
interface Stream<T: Repr> { out valid: Bit, in ready: Bit, out data: Bit<T> }
interface Diff { p: Electrical, n: Electrical }
interface Cell { in wl: Bit, bl: Electrical }

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
    /// Resistance.
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
| module | `module Name(ports) { items }` |
| module with a result | `module name(values) -> T { items; result }` (8.3) |
| interface implementer | `module Name: Interface { items }` |
| extern module | `#[osdi(…)] extern module Name(ports) { params }` |
| template | `Name { params }` |
| instance | `let x = Name(connections) { params };`, or a call inside an expression |

#### 8.1 Ports and params

A **port** is `[in | out | flip] name: carrier [= initial]`. `in` is read; `out` is driven by this module, may have an initial value, and is read inside the module like any of its carriers; a node has no direction. An `in` port with nothing connected is an error; an optional port is `Option<N>`, tested with `if let Some(t) = t { … }` at elaboration.

A **param** is `name: Type [= default] [in range] [where condition];`. Its type is a value type.

| Part | Example |
| --- | --- |
| no default: required | `r: f64;` |
| `Option`: may be left out (3.2) | `rs: Option<f64>;` |
| `in`: a range or a list of allowed values | `n: f64 = 1.0 in 0.1..=10.0;` `kind: &'static str = "npn" in ["npn", "pnp"];` |
| `where`: any boolean over this param and earlier ones | `vmax: f64 where vmax > vmin;` |
| `#[mc(…)]`: a Monte Carlo draw, per instance or `global` | `#[mc(gauss(rel: 0.05))] r: f64 = 1.5k;` |
| metadata | `#[units = "Ohm"]`, `#[alias("tc")]`, `#[nondiff]` |

`in` is a shorthand for the commonest `where`. Both are checked at elaboration and on every host `set`.

Const generics are constrained by a `where` after the port list, checked at elaboration: `module AsyncFifo<T: Repr, const A: u32>(…) where A >= 1 { … }`.

#### 8.2 The body

Items in any order; every name is visible in the whole body and in the module's blocks.

| Item | Example |
| --- | --- |
| param | `c: f64 = 1p where c > 0.0;` |
| signal | `signal mid: Electrical;` `signal q: Bit<u8> = 0;` |
| instance | `let r1 = Resistor(a, b) { r: 1k };` `let acc = idt(v(a));` |
| constant | `let half = r / 2.0;` |
| named branch | `let bq = branch(a, k);` |
| `=` between nodes | `n[0] = a;` |
| `=` defining a `Bit` carrier | `y = a & b;` `busy = st != Tx::Idle;` |
| generation | `for k in 0..N { … }`, `if let Some(t) = t { … }` |

**`=` between nodes** makes them one node.

**`=` defining a `Bit` carrier** makes it a combinational function of the carriers it reads:

1. When an input changes, the carrier is recomputed in the same microstep, before that microstep's reactions run (15). Definitions are evaluated in dependency order, so a chain settles within one microstep.
2. A change fires the carrier's `changed()` in that microstep.
3. A combinational loop, a definition that depends on itself with no reaction in between, is an elaboration error.
4. `y = expr after d;` is transport-delayed: `y` takes each value of `expr` `d` later. The delay may depend on the new value, `y = a & b after if a & b { tr } else { tf };`, and a delay of 0 is no delay.
5. A definition cannot probe: it would have to run at every Newton iteration. Sample an analog value in a reaction.
6. When `expr` has the same width but another type, the definition is a view (5.5).

The other rules:

1. A body `let` is fixed at elaboration: it names an instance, a branch, or a constant over params.
2. `for` and `if` run at elaboration over params and constants; a `let` in a `for` makes an array of instances.
3. `pub` on an item makes it visible to the parent as `inst.name`. A parent may read a public carrier, probe a public node or branch and contribute to it, but never commit to a child's carrier. Coupling between children is written this way: `v(l1.br) +> m * ddt(i(l2.br));` for a mutual inductance.

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
```

#### 8.3 Modules with a result

A module may have a result, and then it can be placed inside an expression. This is how anything with state that reads like an operator is written: integrators, wrapped phases, filters. Each place is an instance.

```rust
/// The stdlib integrator.
pub module idt(x: f64, ic: Option<f64>, reset: Option<bool>) -> f64 {
    signal y: Electrical = ic.unwrap_or(0.0);
    v(y)                                     // the result
}

analog for idt {
    if reset.unwrap_or(false) { v(y) +> ic.unwrap_or(0.0); } else { i(y) +> ddt(v(y)) - x; }
}

analog for Pi { v(outp) +> k1 * v(inp) + k2 * idt(v(inp)); }     // one instance of idt
```

1. Its ports are values (`x: f64`), not carriers. They are evaluated where the instance is used: at every Newton iteration when they probe, otherwise when their inputs change.
2. The last expression of the body is the result. It may read the module's carriers and ports and is evaluated where the instance is used.
3. Every call is an instance. A body may name one, `let acc = idt(v(a));`, and then `acc` reads its result in every block.
4. Being an instance, it is evaluated on every iteration whichever branch it sits in, and its state advances only on accepted steps. It cannot be created inside a reaction, nor in a loop whose trip count depends on a probe or a carrier; loops over params are unrolled, one instance each. `m`, `dtemp` and the metadata apply to it as to any instance.
5. A result that reads only the module's carriers, never its ports, passes through state, so an instance may read its own result in its arguments: `let y = idt(f(y));` is the differential equation `y' = f(y)`. A result that reads a port would close an algebraic loop, an elaboration error, as for definitions (8.2).

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

#### 8.4 Interfaces as port lists, templates, binning

`module Name: Interface` has exactly the interface's ports; a generic bound `C: Cell` means any module with those ports. A template is a module with params and no connections: a value, usable in params, constants and functions, with struct update syntax. Binning is a function returning a template.

```rust
/// One-transistor one-capacitor DRAM cell.
module Dram1T1C: Cell {
    cs: f64 = 30f;
    ron: f64 = 5k;
    rleak: f64 = 1T;
    pub signal sn: Electrical = 0.0;     // storage node
}

analog for Dram1T1C {
    let g = if wl { 1.0 / ron } else { 0.0 };
    i(bl, sn) +> g * v(bl, sn);
    i(sn) +> ddt(cs * v(sn)) + v(sn) / rleak;
}

module MemoryBank<C: Cell, const ROWS: u32, const COLS: u32>(in wl: [Bit; ROWS], bl: [Electrical; COLS]) {
    cell: C;
    for r in 0..ROWS { for c in 0..COLS { pub let cells = cell(wl[r], bl[c]); } }
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

An `extern module` is implemented outside the language: an OSDI model, or a simulator device (`#[device("name")]`, for a cosimulated Verilator or Icarus model). Its body lists params only, checked against the descriptor. It has no behavior blocks.

```rust
#[osdi("bsim4.osdi", model = "bsim4")]
extern module Bsim4(d: Electrical, g: Electrical, s: Electrical, b: Electrical) {
    l: f64 = 1u;
    w: f64 = 1u;
    vth0: f64 = 0.4;
}
```

### 9. Quantities

Every quantity a model uses is one of three:

| Quantity | Declared | Changes | In the matrix | Derivative |
| --- | --- | --- | --- | --- |
| param | `r: f64 = 1k;` | only by the host's `set` | no | `∂/∂p`, unless `#[nondiff]` |
| `Bit` signal | `signal r: Bit<f64> = 1k;` | at events | no: constant in every solve | none |
| node | `signal n: Electrical;` | continuously | yes | `ddt` |

An expression in `analog` is in the matrix exactly when it depends on a node. Models never write params. A change of a `Bit` carrier read by `analog` ends the step at that instant (17). Four ways to vary a resistance:

```rust
// A. Set from outside: a Bit input; each change ends the step.
module DigiPot(p: Electrical, n: Electrical, in r: Bit<f64>) {}
analog for DigiPot { i(p, n) +> v(p, n) / r; }

// B. A function of a node: in the matrix, Newton sees the dependence.
module VcResistor(p: Electrical, n: Electrical, ctl: Electrical) { r0: f64 = 1k; k: f64 = 0.1; }
analog for VcResistor { i(p, n) +> v(p, n) / (r0 * (1.0 + k * v(ctl))); }

// C. Updated after each time point: lagged by one step, no breakpoint.
module LaggedResistor(p: Electrical, n: Electrical, ctl: Electrical) {
    r0: f64 = 1k; k: f64 = 0.1;
    signal r: Bit<f64> = r0;
}
digital for LaggedResistor { on settle { r <- r0 * (1.0 + k * v(ctl)); } }
analog for LaggedResistor { i(p, n) +> v(p, n) / r; }

// D. A function of time: no Jacobian entry; the corner is a known time.
module SweptResistor(p: Electrical, n: Electrical) { r0: f64 = 1k; r1: f64 = 10k; tsweep: f64 = 1u; }
analog for SweptResistor {
    let x = (now() / tsweep).clamp(0.0, 1.0);
    i(p, n) +> v(p, n) / (r0 + (r1 - r0) * x);
    bound_step(tsweep / 50.0);
}
digital for SweptResistor { on at(tsweep) { discontinuity(); } }
```

A host may `set` a param during a run. The instant becomes a step boundary, values computed from params are recomputed, the integrator restarts, and signals and instance state are kept. A reaction can follow it with `on vref.changed()`.

## Part IV. Behavior

### 10. Behavior blocks

Behavior with time is written in blocks for a module: `analog<G> for M<G> { … }` and `digital<G> for M<G> { … }`. A module may have any number of each; blocks of one kind act as one.

| Block | Holds | Runs | Writes |
| --- | --- | --- | --- |
| `analog` | statements: `let`, `if`, `match`, calls, `+>` | at every Newton iteration of every analysis | branches, with `+>` |
| `digital` | reactions, named events, generation | when their events fire | `Bit` carriers, with `<-` |

1. Generic names follow the block keyword; bounds come from the module.
2. A block sees every port, param, signal and `pub` item of its module and its instances, and the module's `impl` helpers through `self`.
3. `analog` and `digital` are keywords only before `for` or `<`.

|  | contract | structure | without time | with time |
| --- | --- | --- | --- | --- |
| values | `trait` | `struct`, `enum` | `impl` | — |
| hardware | `interface` | `module`, `nature` | `impl` | `analog for`, `digital for` |

### 11. Analog

An `analog` block is straight-line code evaluated at every iteration, with no memory between iterations: the past comes from instances with state and from `Bit` carriers.

1. `i(a, b) +> x;` adds `x` to the branch's flow; `v(a, b) +> x;` adds `x` to its potential, which makes the branch a voltage source with its flow as an unknown. Contributions add, from anywhere.
2. A branch gets either potential or flow contributions in one iteration, never both. One that gets potential in some iterations and flow in others is a **switch branch**: `v(a, b) +> 0.0` is a short, `i(a, b) +> 0.0` is open.
3. A branch with no contribution in an iteration has zero flow.
4. Everything is differentiated automatically with respect to every probe, through function calls. An `if` on a probe triggers the `nonsmooth` lint unless `#[allow(nonsmooth)]`; the attribute only silences the lint.
5. `#[opvar] let gm = …;` exports a value for reading after an analysis.
6. An `analog` block has no reactions; events on analog quantities are written in `digital` (13).

```rust
analog for Capacitor { i(p, n) +> ddt(c * v(p, n)); }
analog for Inductor  { v(p, n) +> ddt(l * i(p, n)); }

module Switch(a: Electrical, b: Electrical, in ctl: Bit) {}
analog for Switch {
    if ctl { v(a, b) +> 0.0; } else { i(a, b) +> 0.0; }
}

/// Ideal op-amp (nullor): drives vout so that v(inp, inn) == 0.
module Nullor(inp: Electrical, inn: Electrical, vout: Electrical) { signal x: Electrical; }
analog for Nullor {
    i(x) +> v(inp, inn);    // KCL at x carries the equation
    v(vout) +> v(x);        // v(x) is the free unknown
}
```

There is no indirect contribution: an implicit equation is a KCL row of an internal node, as in `Nullor`, which names the extra unknown.

#### 11.1 One block, every analysis

`ddt` is 0 in equilibrium solves, `ac_stim` is 0 outside small-signal analyses, `noise` is 0 outside noise analysis. `analysis(a: Analysis)` is true when the current analysis is `a`.

| `Analysis` | Analysis |
| --- | --- |
| `Dc`, `Tran`, `Ac`, `Noise` | operating point or sweep, transient, small-signal, noise |
| `Ic` | the initial-condition solve before a transient |
| `Static` | any equilibrium solve |
| `Nodeset` | while nodesets are forced |
| `Tf`, `Sens`, `Pz`, `Sp`, `Disto`, `Pss` | further analyses; the enum is open to new ones |

A noise source is a value: one source in two contributions is fully correlated; two calls are independent. The name only groups sources in reports.

```rust
let n = noise(4.0 * K_B * temperature() / r, "thermal");
i(a, mid) +> n;
i(mid, b) +> -n;          // fully correlated
```

#### 11.2 Bounded state

A state that must stay inside `[lo, hi]` is held at the bound by a switch branch, turned on by a crossing and off when the drift turns back. This is exact, unlike a window function.

```rust
/// A one-dimensional state, normalized to order one so that round tolerances fit (4).
nature StateVar {
    potential: State { access: pos,  abstol: 1u },
    flow:      Rate  { access: rate, abstol: 1n },
}

module Bounded(p: Electrical, n: Electrical) {
    hi: f64 = 1.0;
    signal xs: StateVar = 0.0;
    signal at_hi: Bit = false;
}
impl Bounded {
    fn drift(self) -> f64 { v(self.p, self.n) * 1e-3 }      // shared by both blocks
}
analog for Bounded {
    if at_hi { pos(xs) +> hi; } else { rate(xs) +> ddt(pos(xs)) - self.drift(); }
}
digital for Bounded {
    on above(pos(xs) - hi) { at_hi <- true; }
    on above(-self.drift()) && at_hi { at_hi <- false; }
}
```

### 12. Digital

A `digital` block holds reactions, named events and generation (`for`, `if` over params). It writes `Bit` carriers only, and cannot contribute.

#### 12.1 Reactions

`on ev { body }` runs `body` once, to completion, when the `Event` `ev` fires.

| Statement | Meaning |
| --- | --- |
| `q <- x;` | commit: applies at the next microstep |
| `q <- x after dt;` | applies `dt > 0` later; transport delay |
| `bits[3] <- true;` `mem[a] <- d;` | commits one element |
| `let x = …;` | a local |
| `emit!(…)`, `finish()` | output and control (19) |

1. **Reads see the past.** Every read sees the values at the start of the microstep, after its definitions settled; the commits of all reactions of a microstep apply together.
2. **Last write wins** within one reaction: that is how defaults work.
3. **Collision rule.** Two reactions committing the same level of a carrier in one microstep is an error naming both, found at elaboration when the two events can be shown to coincide and at run time otherwise. Different levels never collide.
4. **Delays are transport.** Every scheduled commit happens, in order.
5. **Probes read the solution at the instant** the event fired, and make it a step boundary (17).
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
analog for SampleHold { v(vout) +> held; }
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
| `s.changed()` | `s`, a `Bit` carrier or a param, changed in this microstep |
| `at(t0: f64, period: Option<f64>)` | time reaches `t0`, and every `period` after |
| `cross(e: f64, dir: Option<Dir>, ttol: Option<f64>, etol: Option<f64>)` | `e` crosses 0 in direction `dir` (`Rising`, `Falling`; both when `None`), located by the solver |
| `above(e: f64, ttol: Option<f64>, etol: Option<f64>)` | as `cross(e, Rising)`, and also after the initial solve if `e > 0` |
| `absdelta(e: f64, delta: f64, ttol: Option<f64>, etol: Option<f64>)` | at the initial solve, and each time `e` has moved by `delta` since it last fired |
| `init` | once per analysis, after its initial solve |
| `settle` | once per time point, after the instant has resolved; its commits apply at the next time point |
| `final` | at the end of each analysis, or at `finish()` |

1. `a || b` fires when either fires. `ev && c` with `c: bool` is a guard. `!` applies to `bool` only; a `bool` alone is not an event.
2. Inside a reaction, an event used as a `bool` says whether it fired in this microstep.
3. `let up = above(e);` in a `digital` block names an event; every use is the same event.
4. `at` is re-armed when the carriers and params in its arguments change, and after it fires. A re-armed time already past fires at the current instant. `now()` is not an input.
5. `cross`, `above` and `absdelta` may probe; the solver locates them (17.3). A guard made only of `Bit` carriers also tells the solver not to locate them while it is false.
6. A crossing expression that jumps across zero within one instant, between its left and right values, fires at that instant.
7. Restrict to an analysis with a guard: `init && analysis(Tran)`.
8. Every other event is a function returning `Event` (3), usually in an `impl`: `clk.rise()`, `t.reaches(k)`, `s.beat(clk)`.

```rust
/// Comparator, right from the operating point: the event says which way it went.
module Comparator(inp: Electrical, inn: Electrical, out q: Bit = false) { ttol: f64 = 1p; }
digital for Comparator {
    let up = above(v(inp, inn), ttol);
    let dn = above(-v(inp, inn), ttol);
    on up || dn { q <- up; }
}

/// Water heated through a thermal port boils at 100 °C: the event changes the state,
/// the state changes the contribution.
#[derive(Repr, Clone, Copy, PartialEq)]
enum Phase { Liquid, Boiling, Vapor }

module Water(t: Thermal) {
    mass: f64 = 1.0;     // kg
    tb:   f64 = 373.15;  // K
    cl:   f64 = 4.18k;   // J/(kg·K)
    cv:   f64 = 2.01k;
    hv:   f64 = 2.26M;   // J/kg
    pub signal phase: Bit<Phase> = Phase::Liquid;
    let absorbed = idt(pwr(t), 0.0, reset: phase != Phase::Boiling);   // energy since boiling began
}

analog for Water {
    match phase {
        Phase::Liquid  => { pwr(t) +> ddt(mass * cl * temp(t)); }
        Phase::Boiling => { temp(t) +> tb; }                 // held; the heat vaporizes
        Phase::Vapor   => { pwr(t) +> ddt(mass * cv * temp(t)); }
    }
}

digital for Water {
    on t.reaches(tb) && phase == Phase::Liquid { phase <- Phase::Boiling; }   // also at time 0 if hot
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

### 14. Core primitives

What only the simulator can provide; everything else is the standard library (22). `transition`, `slew` and `absdelta` are core because they need step control and breakpoints the simulator owns.

| Primitive | Meaning | Allowed in |
| --- | --- | --- |
| `v(a, b: Option)`, `i(…)`, any `access`; `p.inflow()` | probes (6) | `analog`, events, reactions, instance arguments |
| `ddt(x, abstol: Option<f64>)` | time derivative | `analog` |
| `delay(x, td, max: Option<f64>)` | `x` as it was `td` ago; with `max`, `td` may vary up to it | `analog` |
| `transition(x, td: Option, tr: Option, tf: Option)` | ramps a piecewise-constant `x`; a change mid-ramp starts from the current output; its corners are breakpoints | `analog` |
| `slew(x, rise: Option, fall: Option)` | follows `x` with a bounded rate | `analog` |
| `ddx(e, probe)` | partial derivative | `analog` |
| `limit(x, f, seed: Option<f64>)` | Newton step limiting with `f: fn(new, old) -> f64` (18) | `analog` |
| `noise(psd, name: Option<&str>)` | noise source; `psd` may use `freq()` | `analog` |
| `ac_stim(mag, phase: Option<f64>)` | small-signal stimulus | `analog` |
| `bound_step(dt)` | the next step is at most `dt` | `analog` |
| `discontinuity()` | restart the integrator here (17) | `analog`, reactions |
| `branch(a, b: Option<Node>)` | a named branch | module body |
| events of section 13 |  | `on`, named events |
| `now()`, `freq()`, `temperature()`, `simparam(name, default: Option<f64>)`, `analysis(a)` | environment | everywhere but definitions |
| `emit!`, `finish()` | output and control (19) | reactions; `emit!` also in `analog` |

`ddt`, `delay`, `transition`, `slew` and the located events keep state per call site, under the rules of 8.3 rule 4.

## Part V. Execution

What every simulator guarantees, in terms a model can observe. Time is superdense (Lee and Zheng, HSCC 2005). How the reference implementation meets these guarantees, with its numbers, and where it does not yet (G.6), is in Appendix C.2.

### 15. Time and the instant

An **instant** is `(t, n)`: real time `t` and a microstep `n`. Analog values are continuous in `t`; when a `Bit` carrier they read changes, the analog system is solved again at the same `t`, so a jump has a left and a right value.

Event times are integer ticks of a global resolution, 1 fs by default (`#![time_resolution = 1f]`). `at` and `after` round to a tick, so edges that should coincide do. A located crossing is rounded to a tick before its reactions run.

At each time point:

1. **Solve** at `t`: the left value.
2. **Collect** what is due at `t`: `at` events, delayed commits, commits held by `settle`, located crossings, host `set`s.
3. **Microstep loop**, `n = 0, 1, …`:
   1. Apply the commits due at `(t, n)`.
   2. Settle the definitions: recompute, in dependency order, every `=` whose inputs changed. Every changed carrier fires its `changed()`.
   3. Run each reaction whose event fired, once, reading the settled state. Its commits go to `n + 1`, or later with `after`.
   4. If a carrier or param read by `analog` changed, solve again at `t` and restart the integrator.
   5. Leave the loop when nothing is pending at `n + 1`.
4. **Settle**: `on settle` runs once; its commits are held until the next time point.
5. **Advance** to the next time point (17).

The result does not depend on the order reactions run in: they all read one snapshot, and the collision rule turns any disagreement into an error. A microstep loop that never goes quiet is an error naming the carriers that kept changing.

| Code | Reads `Bit` carriers | Reads analog values |
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

### 16. The operating point

The state an analysis starts from: an analog equilibrium and a digital state that agree.

1. **Elaborate**: check params, draw `#[mc]` values, run generation.
2. **Initialize**: carriers with `= value` take it; other `Bit` carriers are unknown. Newton starts from `#[nodeset]` values, else from zero.
3. **Solve** the equilibrium. When Newton alone fails, the simulator uses continuation methods that never change the answer: the last solve is the unmodified circuit.
4. **Fixed point** at `(0, n)`: every definition settles, `on init` runs (first round only), and `above` and `absdelta` fire for every expression already past its threshold. The microstep loop runs; if anything read by `analog` changed, solve again and check again. A bounded number of rounds; more is an error naming the carriers still changing.
5. **Initial conditions.** Under `uic`, each node with `= value` holds that value through the time-0 solve and the first step.
6. **Settle** once and start.

`cross` never fires during the operating point and `above` does: a model that must be right at time 0 uses `above`, as `Comparator` does (13).

### 17. Transient

#### 17.1 Convergence

Every solve is a Newton iteration. It has converged when:

1. **Update**: every unknown moved by at most `reltol · max(|x|, |x_old|) + abstol`, where `abstol` is the nature's: for a node, the smallest potential `abstol` of the natures meeting there; for a branch unknown, its flow `abstol`.
2. **Residual**: every row's imbalance is at most `abstol + reltol · scale`, where `abstol` is, for a node's KCL row, the flow `abstol` of its nature, and for the row of a branch with a potential contribution, the potential `abstol`.
3. **Limiting**: no `limit` call site is active (18).

A non-finite value fails the solve.

#### 17.2 Steps

1. The truncation error of every step is within `trtol`, measured with the natures' tolerances.
2. Every known time is landed on exactly: `at`, `after`, `transition` corners, host `set`s.
3. A rejected step leaves everything unchanged: carriers, pending events, instance state, the analog history.
4. Instance state (`ddt`, `delay`, `transition`, located events, modules with a result) advances only on accepted steps.
5. After a discontinuity the integrator restarts at that point.

| Situation | Step boundary at `t` | Integrator restart |
| --- | --- | --- |
| `Bit` change not read by `analog`, no probe | no | no |
| `Bit` change read by `analog` | yes | yes |
| host `set` of a param read by `analog` | yes | yes |
| reaction at `t` that probes | yes | no |
| reaction at `t` that calls `discontinuity()` | yes | yes |
| `cross`, `above`, `absdelta` located at `t` | yes | only if what `analog` reads changes |
| `on settle` commit | no: applies at the next time point | no |
| `bound_step(dt)` | no: caps the step | no |

A digital clock that nothing analog reads costs the solver nothing.

#### 17.3 Locating crossings

A crossing whose expression changes sign between two time points is located: the step is retaken until the crossing time is known within `ttol`, or `|e| < etol`. Defaults: `ttol` from the simulator, `etol` the nature's `abstol`. The step is accepted at the crossing and its reactions run there. A sign change between the left and right values of one instant fires at that instant (13, rule 6).

`discontinuity()` drops the integrator history at the current point; it is needed at corners of waveforms computed from `now()`, which the error estimate cannot see. `bound_step(dt)` is re-evaluated every step.

### 18. Limiting

`limit(x, f, seed)` bounds how far Newton moves the expression `x` per iteration, with `f: fn(new: f64, old: f64) -> f64`. It changes the path, never the answer.

1. At iteration `k` it returns `x̂_k = f(x_k, x̂_{k−1})`, where `x̂_{k−1}` is what this call site returned before. On the first iteration of an equilibrium solve, `x̂_{k−1}` is `seed` when given, else the last accepted value or the initial guess.
2. While the call site still holds its seed and the limited value would fall below it, the seed is kept: a junction opens near its turn-on point (ngspice `MODEINITJCT`).
3. `x̂_k` replaces `x` in the rest of the expression; for differentiation `limit` is transparent.
4. The right-hand side is corrected to linearize around `x̂_k` (OSDI `lim_rhs`).
5. A call site whose limited value still differs from its input beyond a tolerance vetoes convergence, so at convergence the model is exactly the unlimited one.

```rust
/// Junction diode with optional series resistance.
module Diode(a: Electrical, k: Electrical) {
    is: f64 = 1e-14 where is > 0.0;
    n: f64 = 1.0 in 0.1..=10.0;
    rs: Option<f64>;
    cj0: f64 = 0.0 in 0.0..;
    signal ai: Electrical;
}

analog for Diode {
    match rs {
        Some(r) => { i(a, ai) +> v(a, ai) / r; }
        None    => { v(a, ai) +> 0.0; }          // collapses ai into a
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
    #[opvar] let gm = ddx(id, v(g, s));
}
```

Starting points are hints, not equations: `#[nodeset = 0.6]` on a node sets where the first equilibrium iteration starts.

## Part VI. Input and output

A model writes records, reads files at elaboration, and exposes metadata. None of the three feeds back into the simulation.

### 19. Streams and checks

The core has two primitives: `emit!("name", field = value, …)` writes a typed record to the stream `name`, and `finish()` ends the analysis after the current instant. Every other output is a stdlib macro over `emit!` (22.6), including checks.

1. Every record carries the instance path, analysis, instant `(t, n)` and, inside `analog`, the Newton iteration.
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
| params | the body: type, default, `in`, `where`, attributes, `///` | `list`, `get`, `set` (checked) |
| ports | the port list | `list` |
| public items | `pub` | `list`, probe |
| every node, branch, carrier and instance | the elaborated netlist | probe and record by path, read-only |
| operating-point variables | `#[opvar]` | `get` after an analysis |
| streams and checks | every `emit!` site, with field names and types | `list`, subscribe before the run |
| gradients | every `f64` param without `#[nondiff]` | `∂(record)/∂p` after an analysis (23.2) |

`pub` decides what other models may use; the host can observe everything.

## Part VII. Standard library

### 22. Standard library

Everything here is written in the language over the core of section 14. It is imported by the prelude and can be replaced.

#### 22.1 `Bit`

```rust
impl Bit {
    /// IEEE 1364 posedge: L → H, L → unknown, unknown → H.
    pub fn rise(self) -> Event {
        self.changed() && match (self.prev().level(0), self.level(0)) {
            (Some(false), Some(true)) | (Some(false), None) | (None, Some(true)) => true,
            _ => false,
        }
    }
    /// IEEE 1364 negedge: H → L, H → unknown, unknown → L.
    pub fn fall(self) -> Event {
        self.changed() && match (self.prev().level(0), self.level(0)) {
            (Some(true), Some(false)) | (Some(true), None) | (None, Some(false)) => true,
            _ => false,
        }
    }
    /// A pulse shorter than w just ended.
    pub fn glitch(self, w: f64) -> Event { self.changed() && self.held() < w }
}

impl<T: Repr> Bit<T> {
    pub fn known(self) -> Option<T> {
        let mut l = [false; T::WIDTH];
        for k in 0..T::WIDTH { l[k] = self.level(k)?; }
        T::decode(l)
    }
    pub fn is_known(self) -> bool { self.known().is_some() }
    pub fn known_or(self, d: T) -> T { self.known().unwrap_or(d) }
}
```

#### 22.2 Analog functions and modules

```rust
pub fn vt() -> f64 { K_B * temperature() / Q }
pub fn white_noise(psd: f64, name: Option<&str>) -> f64 { noise(psd, name) }
pub fn flicker_noise(k: f64, exp: f64, name: Option<&str>) -> f64 { noise(k / freq().powf(exp), name) }
pub fn limexp(x: f64) -> f64 { limit(x, |new, old| deltalim(new, old, 1.0)).exp() }

/// Integral wrapped into offset..offset + modulus; each wrap is a located crossing.
pub module idtmod(x: f64, ic: Option<f64>, modulus: f64, offset: Option<f64>) -> f64 {
    signal k: Bit<i64> = 0;
    let acc = idt(x, ic);
    acc - k as f64 * modulus
}
digital for idtmod {
    on cross(acc - k as f64 * modulus - offset.unwrap_or(0.0) - modulus, Rising) { k <- k + 1; }
    on cross(acc - k as f64 * modulus - offset.unwrap_or(0.0), Falling) { k <- k - 1; }
}

/// Time of the last crossing; -1.0 before the first.
pub module last_crossing(e: f64, dir: Option<Dir>) -> f64 {
    signal t: Bit<f64> = -1.0;
    t
}
digital for last_crossing { on cross(e, dir) { t <- now(); } }
```

Also here, as modules with a result: `idt` (8.3), `laplace_nd`/`np`/`zd`/`zp` (one internal node per state), `zi_nd`/`np`/`zd`/`zp` (a sampled `Bit<f64>` state per delay), `table_model` (VAMS 9.21 control strings), `noise_table`. As functions: `db10`, `db20`, `pnjlim`, `fetlim`, `limvds`, `deltalim` (18), and `normal(rng, mean, sigma) -> (f64, u64)`, a pure PRNG step that returns `mean` when `sigma` is 0.

#### 22.3 Shared lines

Modules with one input per device and one driven line (5.4): `TriBus`, `OpenDrainBus`, `WiredOr`, and for real values `SumNet`, `MaxNet`, `MinNet`, `AvgNet`. They are pure structure.

```rust
/// Open-drain line: low while any device pulls, else high.
module OpenDrainBus<const N: usize>(in pull: [Bit; N], out line: Bit) {
    line = !pull.iter().fold(false, |a, p| a | p);     // unknown | true == true
}

/// Tristate line: exactly one enabled driver, otherwise unknown.
module TriBus<T: Repr, const N: usize>(dev: [flip Pin<T>; N]) {
    signal line: Bit<T>;
    line = match dev.iter().filter(|d| d.oe).count() {
        1 => dev.iter().find(|d| d.oe).unwrap().o,
        _ => Unknown,
    };
    for k in 0..N { dev[k].i = line; }
}
```

#### 22.4 Converters

```rust
/// Bit to node: a level through a resistance, with finite edges; unknown is mid-rail.
module D2A(in d: Bit, a: Electrical, vdd: Electrical) { rout: f64 = 100; tr: f64 = 20p; }
analog for D2A {
    let x = match d.level(0) { Some(true) => 1.0, Some(false) => 0.0, None => 0.5 };
    i(a) +> (v(a) - transition(x, 0.0, tr, tr) * v(vdd)) / rout;
}

/// Node to Bit, with hysteresis; right from the operating point.
module A2D(a: Electrical, out d: Bit = false) { vth: f64 = 0.6; hyst: f64 = 50m; }
digital for A2D {
    on above(v(a) - vth - hyst / 2.0) { d <- true; }
    on above(vth - hyst / 2.0 - v(a)) { d <- false; }
}
```

`Bidir` combines both on one node, for a pad with a `Pin`.

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
```

The other common monitors:

```rust
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
```

A monitor only reports: it never writes what it watches (5.4). A model that should turn a violation into unknowns reads the monitor's output, `q <- if viol { Unknown } else { d };`, so the injection is opt-in and visible in the netlist.

#### 22.6 Unknowns, output and files

`same(a, b)` compares levels and stability; `matches_bits!(s, "1x0")` matches with don't-cares; `logic!("10xx")` writes a value with unknown levels.

| Macro | Expands to `emit!` on |
| --- | --- |
| `println!`, `eprintln!` | `stdout`, `stderr` |
| `info!`, `warning!`, `error!`, `assert!` | `stderr`, with a severity |
| `fatal!` | `stderr`, then `finish()` with an error |
| `require!`, `target!`, `cover!` | `checks`, with pass/fail, a signed margin when the condition is a comparison, or a count |

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
|  | regression, sweeps, Monte Carlo runs |

1. The host drives the top module's `in` ports; it is their one driver.
2. The host may wait for any event of the model, read any carrier and probe any node (21).
3. `force` overrides a carrier until `release`; it is outside model semantics and recorded on `stderr`.

A sketch in Python (the API belongs to the host library, not to the language):

```python
import piperine as pp

sim = pp.Session("uart.pp", top="UartRx", params={"clks_per_bit": 87})
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
| variables and bounds | params with `in` and `where` (8.1) |
| objectives | `target!` margins (19) |
| constraints | `require!` margins (19) |
| gradients | `∂F/∂p` for every `f64` param without `#[nondiff]` (9), and the solver's adjoint sensitivity, across events as in 23.3 |
| yield | `#[mc]` draws (8.1) |

```python
opt = pp.Optimize(sim, vary=["w", "l"], objectives=["gain_db"], constraints=["ipk_lim"], mc=200)
best = opt.run()          # gradient-based, from the model's own derivatives
```

#### 23.3 Gradients across events

A mixed-signal trajectory is piecewise smooth in its params. Its gradient is defined piece by piece:

1. Between events, sensitivities are those of the solver's equations, forward or adjoint.
2. A located event moves with the params. For a crossing of `e` at `t*`, `dt*/dp = −(∂e/∂p) / (de/dt)`, where `∂e/∂p` includes the state's own sensitivity. At `t*` the state's sensitivity jumps by the change of the right-hand side across the event times `dt*/dp` (the saltation correction).
3. An `at` or `after` time computed from params moves by its own derivative.
4. `Bit` values are piecewise constant in the params: they have no derivative, and they act on a record only through the times of the events that change them.
5. Where no gradient exists, the host gets a diagnostic naming the event instead of a number: a grazing crossing (`de/dt = 0` at `t*`), or an event that appears or disappears as a param moves.
6. `#[nondiff]` takes a param out of gradients. It belongs on params that only select (`match`, `if` on a param) or only feed `Bit` logic.

For `Comparator` (13) with `inn` at a threshold `vth`, the edge time solves `v(inp)(t*) = vth`, so `dt*/dvth = 1 / v'(inp)(t*)`: the edge moves by the threshold change over the slew rate. A `Bit<f64>` sampled at that edge has derivative zero; a node that the edge switches, through a `D2A`, receives the saltation correction at `t*`.

## Appendix A. Lexical structure

1. **Close to Rust, not Rust.** Identifiers, comments, attributes, literals and the value syntax are Rust's. The lexer and the grammar are the language's own: they add the tokens and productions below and B.3, and drop what Part I leaves out.
2. **SI-suffixed reals.** A number followed by `T G M k m u n p f a` is an `f64` times `1e12 … 1e-18`: `1k`, `2.2u`, `10f`; `1m` is milli, `1M` mega. The suffix is a multiplier, not a unit.
3. **Unknown values.** `Unknown` is the unknown value of any `Bit` carrier; `logic!("10xx")` writes one level by level.
4. **Doc comments** `///` are metadata (21).
5. **Two operators.** `+>` (contribution, in `analog`) and `<-` (commit, in reactions) are single tokens, read by longest match: `a<-b` is a commit, and a comparison with a negative operand is written `a < -b`.

## Appendix B. Reference

### B.1 Added to Rust

| Kind | Items |
| --- | --- |
| keywords | `module` `interface` `nature` `signal` `on` `out` `flip` `after`; new uses of `in` and `extern` |
| contextual keywords | `analog`, `digital` before `for` or `<`; `potential`, `flow` in a nature |
| operators | `+>`, `<-` |
| calls | optional trailing `Option` parameters, named arguments (3.2) |
| types | `Bit<T>` (`Bit` is `Bit<bool>`); continuous natures; interfaces; `Event`, `Dir`, `Analysis`; `uN`, `iN`, `fN` |
| traits | `Repr`, `Terminals` |
| carrier methods | `changed` `prev` `held` `since` `level` `inflow` |
| literals | SI suffixes, `Unknown` |
| events | `at` `cross` `above` `absdelta` `init` `settle` `final` |
| analog | `ddt` `delay` `transition` `slew` `ddx` `limit` `noise` `ac_stim` `bound_step` `discontinuity` `branch` |
| environment | `now` `freq` `temperature` `simparam` `analysis` |
| output | `emit!` `finish` |
| attributes | `#[opvar]` `#[nodeset]` `#[allow(nonsmooth)]` `#[derive(Repr)]` `#[repr(onehot \| gray \| uN)]` `#[mc]` `#[units]` `#[alias]` `#[nondiff]` `#[osdi]` `#[device]` `#[derivative]` `#![time_resolution]` |

Everything else is the standard library (22).

### B.2 Where each thing is allowed

|  | module body | `digital` block | reaction | `analog` | `fn` |
| --- | --- | --- | --- | --- | --- |
| param, `signal`, instance, constant `let`, `branch` | yes |  |  |  |  |
| `=` (nodes, definitions, views) | yes |  |  |  |  |
| generation `for`/`if` over params | yes | yes |  |  |  |
| `on`, named events |  | yes |  |  |  |
| `+>` |  |  |  | yes |  |
| `<-` |  |  | yes |  |  |
| probes, environment | in instance arguments | in event expressions | yes | yes | yes |
| `ddt`, `delay`, `transition`, `slew`, `ddx`, `limit`, `noise`, `ac_stim`, `bound_step` |  |  |  | yes |  |
| an instance inside an expression | yes |  |  | yes |  |
| `discontinuity()` |  |  | yes | yes |  |
| `emit!` |  |  | yes | yes |  |
| `finish()` |  |  | yes |  |  |

A `fn` that probes or reads the environment is callable only where those are allowed.

### B.3 Grammar of the additions

```
nature_decl    ::= 'nature' IDENT ( ':' path )? '{' quantity ( ',' quantity )? ','? '}'
quantity       ::= ( 'potential' | 'flow' ) ':' IDENT ( '{' ( IDENT ':' expr ',' )* '}' )?
interface_decl ::= 'interface' IDENT generics? '{' ( port ',' )* '}'
impl_block     ::= 'impl' generics? path generic_args? ( 'for' path generic_args? )? '{' ( attrs 'pub'? fn_decl )* '}'
fn_decl        ::= 'extern'? 'fn' IDENT generics? '(' ( IDENT ':' type ',' )* ')' ( '->' type )? ( block | ';' )
call           ::= path '(' ( expr ',' )* ( IDENT ':' expr ',' )* ')'           // 3.2

module_decl    ::= attrs 'extern'? 'module' IDENT generics? module_head ( 'where' expr )? '{' module_item* expr? '}'
module_head    ::= '(' ( port ',' )* ')'                                       // a module
                 | '(' ( IDENT ':' type ',' )* ')' '->' type                    // a module with a result
                 | ':' path                                                     // an interface implementer
port           ::= attrs dir? IDENT ':' sigtype ( '=' expr )?
dir            ::= 'in' | 'out' | 'flip'
sigtype        ::= path generic_args? | 'Option' '<' sigtype '>' | '[' sigtype ';' expr ']'
module_item    ::= attrs 'pub'? ( param | signal_decl | 'let' pat '=' expr ';' )
                 | place '=' expr ( 'after' expr )? ';'                         // nodes, definitions, views
                 | gen<module_item>
param          ::= IDENT ':' type ( '=' expr )? ( 'in' expr )? ( 'where' expr )? ';'
signal_decl    ::= 'signal' IDENT ':' sigtype ( '=' expr )? ';'

behavior_block ::= 'analog'  names? 'for' path generic_args? '{' ( stmt | contribution )* '}'
                 | 'digital' names? 'for' path generic_args? '{' digital_item* '}'
names          ::= '<' IDENT ( ',' IDENT )* '>'
digital_item   ::= reaction | 'let' IDENT '=' expr ';' | gen<digital_item>
gen<X>         ::= 'for' pat 'in' expr '{' X* '}' | 'if' expr '{' X* '}' ( 'else' '{' X* '}' )?
reaction       ::= 'on' expr '{' ( stmt | commit )* '}'                      // expr has type Event
contribution   ::= expr '+>' expr ';'                                         // left side: a probe
commit         ::= place '<-' expr ( 'after' expr )? ';'
instance       ::= path generic_args? '(' ( port_conn ',' )* ')' ( '{' ( IDENT ':' expr ',' )* '}' )?
template       ::= path generic_args? '{' ( IDENT ':' expr ',' )* ( '..' expr )? '}'
```

`stmt`, `expr`, `type`, `pat`, `block` and `generics` are the value grammar, Rust's apart from Part I's changes. `in`, `where` and `;` end a param's default without lookahead.

## Appendix C. Implementation

### C.1 Lowering to AIR

Structure (interfaces, templates, binning, generics, generation, `m`, modules with a result) is resolved at elaboration, and every function is inlined, so AIR v2 sees flat instances and core primitives only.

| Language | AIR v2 |
| --- | --- |
| param with `in`/`where`, `#[mc]` | param metadata, checked in `setup`; each draw applied as a `set` |
| live `set`, `on p.changed()` | `live` param, `on change` region |
| `signal x: Bit<T> = v;` | digital state, initialized in `init` |
| `prev()`, `held()`, `since()` | one previous value and two times per carrier |
| node, `= v`, `#[nodeset]` | internal node, initial condition, nodeset hint |
| `=` defining a `Bit` carrier | a combinational cell, evaluated in topological order |
| `=` with `after` | an `on change` region with a delayed commit |
| `analog for M` | `iter` region |
| `+>` | `+=` contribution, split into resistive and reactive parts |
| switch branch | a branch whose kind changes per iteration |
| `limit(x, f, seed)` | limited probe with previous-iteration state, seed and RHS correction |
| reaction, `<-`, `after` | sequential `on` region, commit, delayed commit |
| `init`, `settle`, `final` | the regions of the same names |
| `at`, `cross`, `above`, `absdelta` | scheduled and located events; `above` also checked in `init` |
| `transition`, `slew` | runtime operators that publish their breakpoints |
| `bound_step`, `discontinuity` | step-control operations |
| `emit!` | stream emit |
| `#[osdi] extern module` | foreign (OSDI) instance |

### C.2 Reference implementation

How the Piperine solver meets Part V. Every number is a default the host may change.

| Part V guarantee | Piperine algorithm |
| --- | --- |
| microstep loop (15) | two-phase delta cycle: combinational cells in topological order, then sequential evaluation over one snapshot; at most 1000 microsteps per instant |
| operating point (16) | Newton; then gmin stepping (a shunt on every node from 0.1 S down, a decade per converged solve, adaptive factor); then source stepping (scale from 0 to 1, first step 0.1 growing ×1.5 up to 0.25, halved on failure, a 1 µS knee shunt ramped out at the end); at most 20 mixed rounds |
| initial conditions (16) | a 10¹² S clamp across each seeded branch through the time-0 solve and the first accepted step |
| convergence (17.1) | `reltol` 1e-3, default potential `abstol` 1 µV, flow `abstol` 1 pA, `gmin` 1e-12 S; an update much larger than the last is damped to the midpoint; stamps may be reused while the inputs move less than the tolerance (SPICE bypass), never while limiting |
| truncation error (17.2) | TR-BDF2 (a trapezoidal stage over `γh`, then BDF2 over `(1 − γ)h`, `γ = 2 − √2`); error by Milne's device over node potentials against `trtol` 7; a node whose differences straddle a jump is skipped |
| step control (17.2) | PI controller, `h · (1/lte)^p`, `p = kp + ki · (lte − lte_prev)/lte`, `kp` 0.7, `ki` 0.4, growth clamped to \[0.2, 1.5\]; a rejected step is divided by 8; at `dt_min` an error rejection is accepted with a warning and a Newton failure fails |
| restart (17.2) | after a discontinuity the trapezoidal stage degrades to backward Euler; after a live `set` the step restarts at 10⁻³ of the last one |
| rollback (17.2) | one event queue (scheduled commits restored, `at` times and breakpoints re-polled, step bounds and crossings discarded and re-emitted), with the digital state, device state and analog history checkpointed per step |
| crossings (17.3) | secant prediction from the bracketing pair, bisection when it stalls, retake to the predicted time. Today Piperine fires at the first accepted point after the sign change; landing on the prediction is the work left (G.6) |
| limiting (18) | ngspice `pnjlim`, `fetlim`, `limvds`; a call site is active while `abs(x̂ − x) > 1e-6 + 1e-4 · abs(x)`; when a limited branch potential becomes node potentials, the minus node moves and the plus node stays, unless the minus node is `gnd` |
| temperature | `temperature` and `tnom` 300.15 K |

## Appendix D. Gallery: sources, clocks and continuous time

No new semantics; the point is where the step boundaries land.

### D.1 Piecewise-linear source

Each corner changes a `Bit` signal read by `analog`: a boundary and a restart (17.2).

```rust
module Pwl<const N: usize>(p: Electrical, n: Electrical) {
    points: [(f64, f64); N];          // (time, value), times increasing
    signal seg: Bit<u32> = 0;
}

digital<N> for Pwl<N> {
    for k in 1..N { on at(points[k].0) { seg <- k as u32; } }
}

analog<N> for Pwl<N> {
    let k = seg as usize;
    let y = if k + 1 >= N { points[N - 1].1 } else {
        let ((t0, y0), (t1, y1)) = (points[k], points[k + 1]);
        y0 + (y1 - y0) * ((now() - t0) / (t1 - t0)).clamp(0.0, 1.0)
    };
    v(p, n) +> y;
}
```

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

The edges are scheduled; if nothing analog reads `clk`, they cost the solver nothing.

### D.3 Voltage-controlled oscillator

The phase is an integral wrapped to one cycle. The edges are two rising crossings a half cycle apart, never the wrap itself, as in Kundert's models.

```rust
module Vco(vctl: Electrical, out clk: Bit = false) {
    f0: f64 = 1G;
    kvco: f64 = 100M;
    let ph = idtmod(f0 + kvco * v(vctl), 0.0, 1.0, -0.5);    // phase in [-0.5, 0.5)
}

digital for Vco {
    on cross(ph + 0.25, Rising) { clk <- true; }
    on cross(ph - 0.25, Rising) { clk <- false; }
}

analog for Vco { bound_step(0.05 / f0); }
```

### D.4 DAC with finite edges

`transition` ramps a piecewise-constant value and declares its corners as breakpoints; a code change mid-ramp starts from the current output.

```rust
module DacRamp(in code: Bit<u8>, vout: Electrical) { vref: f64 = 1.0; tedge: f64 = 100p; }
analog for DacRamp {
    v(vout) +> transition(vref * (code as f64) / 256.0, 0.0, tedge, tedge);
}
```

### D.5 Sampling at a clock edge: delta-sigma modulator

```rust
module DeltaSigma1(vin: Electrical, in clk: Bit, out bit: Bit = false) {
    r: f64 = 100k; c: f64 = 1p; vref: f64 = 1.0;
    signal x: Electrical = 0.0;
}

analog for DeltaSigma1 {
    let fb = if bit { vref } else { -vref };
    i(x) +> ddt(c * v(x)) - (v(vin) - fb) / r;
}

digital for DeltaSigma1 {
    on clk.rise() { bit <- v(x) > 0.0; }
}
```

At a rising edge, microstep 0 probes `v(x)` and commits `bit`; microstep 1 applies it, `analog` reads it, so the system is solved again at `t` and the integrator restarts. Falling edges cost nothing.

### D.6 Switched-capacitor integrator

```rust
module RSwitch(a: Electrical, b: Electrical, in ctl: Bit) { ron: f64 = 100; roff: f64 = 1G; }
analog for RSwitch { i(a, b) +> v(a, b) / if ctl { ron } else { roff }; }

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

Four boundaries per period, one at each phase edge; `clk` itself costs nothing. `TwoPhase` is two definitions with value-dependent delays: a non-overlapping clock generator is combinational logic with delays.

### D.7 Guidance

- Read a `Bit` carrier in `analog` only when the analog behavior depends on it.
- Prefer known times (`at`, `after`) to crossings.
- Sample at a clock edge with a reaction, not with `cross` on the clock's analog image.
- Make the edges of a phase with offset crossings, not with the wrap of `idtmod`.
- Give edges a duration (`transition`) when the edge rate matters to the analog side.

## Appendix E. Gallery: digital

`Bit` carriers only: none of these touches the solver.

### E.1 Flip-flop with asynchronous reset

```rust
module Dff<T: Repr>(in clk: Bit, in rst_n: Bit, in d: Bit<T>, out q: Bit<T>) { reset: T; }
digital<T> for Dff<T> {
    on clk.rise() || rst_n.fall() { if !rst_n { q <- reset; } else { q <- d; } }
}
```

`q` is unknown until the first edge or reset. With `rst_n` unknown, the `if` that commits takes the `else` branch, as Verilog's `if (!rst_n)` does.

### E.2 Latches

```rust
module DLatch<T: Repr>(in en: Bit, in d: Bit<T>, out q: Bit<T>) {}
digital<T> for DLatch<T> {
    on en.changed() || d.changed() { if en { q <- d; } }
}

/// Cross-coupled NOR gates with gate delay: pure structure.
module SrLatch(in s: Bit, in r: Bit, out q: Bit = false, out qn: Bit = true) {
    tpd: f64 = 10p where tpd > 0.0;
    q  = !(r | qn) after tpd;
    qn = !(s | q)  after tpd;
}
```

Releasing `s` and `r` together oscillates with period `2·tpd`, as the ideal circuit does. Without `after`, the two definitions would form a combinational loop, an elaboration error.

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

### E.5 Memory

```rust
module Ram(in clk: Bit, in we: Bit, in addr: Bit<u8>, in wdata: Bit<u32>, out rdata: Bit<u32>) {
    signal mem: [Bit<u32>; 256];         // one carrier per word: one event per write
}
digital for Ram {
    on clk.rise() {
        if we { mem[addr] <- wdata; }
        rdata <- mem[addr];              // read-before-write
    }
}
```

An asynchronous read port is a definition, `rdata = mem[addr];`. An unknown address reads unknown and drops a write (5.2).

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
module IrqController(in clk: Bit, in irq_n: Bit, out pending: Bit = false) {}
digital for IrqController { on clk.rise() { pending <- !irq_n; } }

module Board(in clk: Bit, in req0: Bit, in req1: Bit, out pending: Bit) {
    signal irq_n: Bit;
    let bus = OpenDrainBus::<2>([req0, req1], irq_n);   // the wired-AND is hardware (22.3)
    let ic = IrqController(clk, irq_n, pending);
}
```

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
    v(src) +> vref * (code as f64) / 256.0;
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
analog for SarAdc { i(dac.core) +> ddt(cpar * v(dac.core)); }
```

Boundaries per conversion: `start` (the sample probes), eight code changes (the DAC reads `code`), and each located comparator crossing.

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
    i(vc) +> if dn { icp } else { 0.0 } - if up { icp } else { 0.0 };
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

Boundaries: each edge of `up` and `dn`, and each VCO edge. `fref`, `fdiv` and the counter are purely digital.

### F.3 DRAM array with a swappable cell

```rust
/// Behavioral cell with the ports of Cell (7), for fast runs.
module BehavioralCell: Cell { vdd: f64 = 1.0; ron: f64 = 2k; signal bit: Bit = false; }
analog for BehavioralCell {
    if wl { i(bl) +> (v(bl) - if bit { vdd } else { 0.0 }) / ron; }
}
digital for BehavioralCell {
    on wl.fall() { bit <- v(bl) > vdd / 2.0; }
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

`BehavioralCell {}` in place of `Dram1T1C { cs: 30f }` swaps every cell without touching `MemoryBank`. A storage node is probed as `bank.cells[3][5].sn`.

## Appendix G. Scope and open questions

**Out of scope:** high impedance and drive strengths (a shared line is a module); formal-verification constructs; the SystemVerilog verification language (the testbench lives in the host, 23); blocking writes and `#0`; indirect contributions; heap types; reading files during a run; switch-level primitives; implicit converters; global nets other than `gnd`; physical units in the type system (units are labels).

**Highest impact:** G.6 (crossings landed on time), G.8 (import), G.21 (lossy lines), G.22 (load-dependent delay).

**G.1 Name.** Distinct from Piperine (the toolchain) and AIR (the IR).

**G.2 Collisions at elaboration.** How far the static check of 12.1 goes before falling back to the run-time error.

**G.3 The `nonsmooth` lint.** Too coarse for region models that are smooth by construction.

**G.4 Interfaces with extra ports.** A real DRAM cell also needs a plate voltage: put it in `Cell`, allow named extra ports, or route a parent `pub` node.

**G.5 Result syntax.** A module with a result ends its body with the result expression (8.3). Alternatives: a named result port, or `-> name: T`.

**G.6 AIR and Piperine gaps.** Switch branches, user `limit` functions, `above`, integer time, modules with a result, and landing on predicted crossings (Piperine fires at the first accepted point after the sign change today).

**G.7 Inertial delay.** Delays are transport. Stdlib module, or `after tpd inertial`?

**G.8 Import.** Running Yosys netlists needs its own part: the netlist form consumed (RTLIL or cell netlists), a cell-to-language table (`$mux` to the `if` that yields a value, `$tribuf` to `TriBus`, `$dff` to a reaction), a policy for several writers in behavioral netlists, and the contract of `#[device]` cosimulation.

**G.9 Real-valued change.** `Bit<f64>` changes when its levels change: `0.0 → -0.0` is a change, the same `NaN` is not.

**G.10 Time resolution.** 1 fs by default; may crates mix resolutions?

**G.11 Custom floats.** `Float<E, M>` for bfloat16 and 8-bit formats.

**G.12 Threshold view.** VHDL-AMS `'Above` takes two `above`s and a signal (`A2D`). Is the stdlib module enough?

**G.13 Metastability.** A stdlib synchronizer whose first stage commits `Unknown` when `d.since() < tw` at the edge.

**G.14 Field-init shorthand.** `{ r }` for `{ r: r }` in instances and templates.

**G.15 Keywords as names.** Accept raw identifiers (`r#out`), listed without `r#` in metadata.

**G.16 Shadowed probes.** Warn when a local takes the name of a probe in scope.

**G.17 The top module.** Left to the host: any module without ports, or one it names.

**G.18 Contracts over functions.** `C: Cell` promises ports only; an interface could also declare `fn` signatures, as a trait does.

**G.19 Depth of history.** One previous value and two times (5.3). Longer patterns are state; confirm it is enough.

**G.20 Reductions.** An explicit signal per reduction (19) is plain but verbose; stdlib modules with a result, `running_max(x)`, would shorten it.

**G.21 Lossy lines and measured responses.** Lossy and coupled lines (SPICE `O`, `Y`, `P`) and S-parameter blocks need a convolution over the input's history. Rational fitting into `laplace_zp` in the stdlib, or a core `convolve(x, h, tmax)`?

**G.22 Load-dependent delay.** A gate's delay often depends on the load it drives (XSPICE input loads, Liberty NLDM tables), and a module cannot see what its output connects to. Proposal: an `#[load]` param on input ports and an elaboration query `y.load()`.

**G.23 Bit fields on values.** Slices and concatenations of integers (Yosys `$slice`, `$concat`) are written with shifts, masks or views. A stdlib `bits::<H, L>(x)` and `concat!(a, b)` would read better.

**G.24 Frequency-domain post-processing.** `.measure` over an AC sweep has no reaction to run in: reactions are transient. Measures over complex results belong to the host; confirm, or add a hook.

**G.25 More enumerated natures.** Only `Bit` today (4). A user enumerated nature (`Pam4` as levels) returns if a real case needs it.

**G.26 `above` and the operating point.** `above` is core because it fires after the initial solve. If `init && e > 0.0` were enough, it could move to the stdlib.

## Appendix H. Change log

| Date | Change |
| --- | --- |
| 2026-10-26 | Second review and an internal audit. An instance may read its own result through state (8.3); gradients across events (23.3); tolerance rules (4, 17.1); the monitors shown, reporting only (22.5); i(p) and inflow() told apart (6); import moved from the header to G.8. Removed: Bit::L and Bit::H, the duplicate rules of 5.4 and 6, sparse storage as a rule, two unused attributes, emit! in fn. Fixed: reaches fires at time 0, probes in instance arguments (B.2), a duplicate Tx and a duplicate Pfd, the UART test race (23.1). |
| 2026-10-26 | Ten concepts. `=` in the module body covers node identity, combinational definitions and views; drives, combinational `let`s and aliases of `Bit` carriers left `digital`. Functions are pure; anything with state is a module, and a module with a result can be placed in an expression. `analog` has only equations and `digital` only reactions. `accept` merged into `settle`; `timer` became `at` with a period; `transition`, `slew` and `absdelta` are core. `Bit` is the only enumerated nature. Unknowns follow IEEE 1364, including edges and `?:` merging; an unknown read by `analog` is an error. Crossings across a jump within an instant fire. Part V states guarantees; Piperine's algorithms moved to C.2. The testbench moved to the host (Part VIII). |
| 2026-10-26 | `wait` and `sleep` removed; optional parameters as trailing `Option`s for every callable; public branches, `dtemp`, unknown indices, value-dependent delays; a capability check against Yosys and SPICE ([analysis](https://claude.ai/artifact/7AzUnmdfmFCRYs7cx8fsBx)). |
| 2026-10-25 | Two kinds of nature made explicit; resolution left the language for bus modules; `check`, `process`, `->`, `event fn`, `analog fn`, `SparseMem`, `History`, `~` and `%` removed. |
| 2026-10-24 | The 38 gaps and 5 inconsistencies of the first draft's audit against VAMS 2.4, IEEE 1800 and Piperine accepted; the record stays in the first draft. |
