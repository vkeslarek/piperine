<div align="center">

<img src="assets/logo.svg" alt="Piperine Logo" width="50%"/>

**One language for analog, digital and mixed-signal circuits, and a native solver that runs it.**

</div>

> **Work in progress.** The language, the command line and the library still
> change. Use Piperine to explore and to contribute, not yet for work you depend on.

---

Piperine is a circuit simulator built around **MSL**, the Piperine mixed-signal
language. A device, a logic block, a converter with its control loop and the
bench that tests it are all written in MSL, and the `piperine` command runs them
on Piperine's own solver. There is no SPICE underneath.

This page is a tour of the language. Every snippet is MSL as the
[specification](docs/spec/index.md) defines it; most come from the specification's
galleries, the Book or [`examples/`](examples). The tour shows the whole language.
The compiler does not run all of it yet: [Status](#status) says exactly which parts
run from source today and which are specified and being implemented.

## Contents

- [One language, three kinds of circuit](#one-language-three-kinds-of-circuit)
- [Natures and carriers](#natures-and-carriers)
- [Modules](#modules)
- [Analog behavior](#analog-behavior)
- [Digital behavior](#digital-behavior)
- [Events](#events)
- [Unknown values](#unknown-values)
- [Values and functions](#values-and-functions)
- [Output: streams and checks](#output-streams-and-checks)
- [Running it: the host](#running-it-the-host)
- [Correctness](#correctness)
- [A first run](#a-first-run)
- [Status](#status)
- [Documentation](#documentation)
- [Building](#building)

## One language, three kinds of circuit

A module declares ports, params and signals. An `analog` block gives it
continuous-time equations, a `digital` block gives it reactions to events, and one
module may have both. This sample-and-hold is the whole model: the reaction reads
a voltage, and the analog block reads what the reaction stored.

```rust
/// Samples `vin` on each rising clock edge and holds it on `vout`.
module SampleHold(vin: Electrical, vout: Electrical, in clk: Bit) {
    signal held: Bit<f64> = 0.0;
}

digital for SampleHold { on clk.rise() { held <- v(vin); } }
analog  for SampleHold { v(vout) +> held.known_or(0.0); }
```

No interface element sits between the two halves and no rule about who runs first
has to be learned: every outcome of a simulation is a function of the source and
of what the host does, so two reactions never race. Benches, sources and monitors
are MSL modules as well.

## Natures and carriers

A **nature** says what a signal holds and how several drives of it combine. A
continuous nature has a potential and a flow, and its nodes obey a conservation
law. A discrete nature holds a value. These are the declarations the library
itself uses; a project declares its own the same way.

```rust
pub nature Electrical {
    potential: Voltage { access: v, units: "V", abstol: 1u },
    flow:      Current { access: i, units: "A", abstol: 1p },
}
pub nature Thermal {
    potential: Temperature { access: temp, units: "K", abstol: 1m },
    flow:      Power       { access: pwr,  units: "W", abstol: 1n },
}
pub nature Bit<T: Repr = bool> { value: T }                 // one driver; `Bit` is `Bit<bool>`
pub nature Wire { value: Logic, resolve: Logic::wired }     // any number of drivers, with strengths
```

A **carrier** is a signal or a port of a nature. It exists over time and is
connected by being passed to a port.

```rust
module Carriers(a: Electrical, in clk: Bit) {
    signal mid:   Electrical;                // an internal node
    signal vout:  Electrical = 0.0;          // where the solver starts; held there through the first solve
    signal en:    Bit;                       // no initial value: starts unknown
    signal count: Bit<u8> = 0;               // any value type that has a bit representation
    signal level: Bit<f64> = 0.0;            // a real number that changes at instants
    signal lines: [Bit; 8];                  // eight carriers, each with its own events
    signal bus:   Bit<u4> = logic("10xx");   // written level by level; x is unknown
    signal sda:   Wire;                      // resolved: open drain, tristate, pull-ups
}
```

A **bundle** is a `struct` of carriers: one port for a whole interface. `flip`
mirrors the directions for the other end, and a bundle has methods like any type.

```rust
struct Stream<T: Repr> { out valid: Bit, in ready: Bit, out data: Bit<T> }
struct Diff { p: Electrical, n: Electrical }

impl<T> Stream<T> {
    pub fn beat(self, clk: Bit) -> Event { clk.rise() && self.valid && self.ready }
}
impl Diff {
    pub fn vd(self) -> f64 { v(self.p, self.n) }
}

/// A register slice between two streams.
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

Resolved nets carry strengths, so an open-drain line with a pull-up is three
definitions:

```rust
module IrqSource(in req: Bit, irq_n: Wire) {
    irq_n = if req { Logic::strong(false) } else { Logic::Z };
}

module Board(in req0: Bit, in req1: Bit, out pending: Bit) {
    signal irq_n: Wire;
    let pu = Pullup(irq_n);                  // wins only while every source is released
    let s0 = IrqSource(req0, irq_n);
    let s1 = IrqSource(req1, irq_n);
    pending = !irq_n.v;
}
```

## Modules

### Params and their conditions

A param is a value the model reads and the host sets. It may have a default and a
`where` condition, checked when the design is built and again whenever the host
sets it. A param of type `Option<T>` may be left out.

```rust
/// Ideal resistor with a linear temperature coefficient.
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

### Hierarchy

A `let` in a module body places an instance: ports in parentheses, params in
braces. A parent reaches a child's `pub` items by path, and may add its own
equation to a child's public node.

```rust
module SarAdc(vin: Electrical, in clk: Bit, in start: Bit, out code: Bit<u8>, out done: Bit) {
    cpar: f64 = 50f;
    signal vh: Electrical;
    signal vdac: Electrical;
    signal cmp: Bit;
    let sh    = SampleHold(vin, vh, start);
    let dac   = Dac8(code, vdac) { vref: 1.2 };
    let comp  = Comparator(vh, vdac, cmp);
    let logic = SarLogic(clk, start, cmp, code, done);
}
analog for SarAdc { i(dac.core) +> ddt(cpar * v(dac.core)); }   // a parasitic on the DAC's public node
```

### Instances inside expressions

Anything that keeps state is a module, and a module may yield a result. Writing
one inside an expression places an instance of it there: an integrator, a delay,
a ramp generator or a crossing detector is used where its value is needed. An
integral may even read its own result.

```rust
fn windup(u: f64, lo: f64, hi: f64) -> f64 { u.clamp(lo, hi) - u }

/// PI controller with output limits and back-calculation anti-windup.
module PiAw(e: Electrical, y: Electrical) {
    kp: f64 = 0.5; ki: f64 = 2k; kt: f64 = 10k;
    lo: f64 = 0.0; hi: f64 = 1.0 where hi > lo;
    let xi = idt(ki * v(e) + kt * windup(kp * v(e) + xi, lo, hi));
}
analog for PiAw { v(y) +> (kp * v(e) + xi).clamp(lo, hi); }
```

You write such modules yourself. This one is the library's periodic timer:

```rust
pub module every(t0: f64, period: f64) -> Event {
    signal k: Bit<u64> = 0;
    let tick = at(t0 + k as f64 * period);
    tick
}
digital for every { on tick { k <- k + 1; } }
```

### Generics and generation

A module takes type and const parameters, with conditions on them. `for` and `if`
in a body or in a `digital` block generate structure when the design is built.

```rust
/// A delay line of N registers of any value type.
module Delay<T: Repr, const N: usize>(in clk: Bit, in d: Bit<T>, out q: Bit<T>) where N >= 1 {
    signal r: [Bit<T>; N];
    q = r[N - 1];
}
digital<T, N> for Delay<T, N> {
    on clk.rise() { r[0] <- d; for k in 1..N { r[k] <- r[k - 1]; } }
}

module Divider<const N: u32>(in clk: Bit, out q: Bit = false) where N >= 2 && N % 2 == 0 {
    signal cnt: Bit<u32> = 0;
}
digital<N> for Divider<N> {
    on clk.rise() { if cnt == N / 2 - 1 { cnt <- 0; q <- !q; } else { cnt <- cnt + 1; } }
}

module Tap(in clk: Bit, in d: Bit<u8>, out q: Bit<u8>, out slow: Bit) {
    let line = Delay::<u8, 4>(clk, d, q);
    let div  = Divider::<8>(clk, slow);
}
```

### Templates and swappable modules

A module with its params bound is a value. It can be stored in a param, returned
by a function, and placed later. An array is built from whichever cell the parent
hands in, and a function picks the params of a transistor from its geometry.

```rust
struct Cell { in wl: Bit, bl: Electrical }

/// One-transistor one-capacitor DRAM cell.
module Dram1T1C(c: Cell) {
    cs: f64 = 30f;
    ron: f64 = 5k;
    pub signal sn: Electrical = 0.0;     // storage node
}

module MemoryBank<C: Fn(Cell), const ROWS: u32, const COLS: u32>(in wl: [Bit; ROWS], bl: [Electrical; COLS]) {
    cell: C;
    for r in 0..ROWS { for c in 0..COLS { pub let cells = cell(Cell { wl: wl[r], bl: bl[c] }); } }
}

module Array(in wl: [Bit; 64], bl: [Electrical; 32]) {
    let bank = MemoryBank::<_, 64, 32>(wl, bl) { cell: Dram1T1C { cs: 30f } };
}
```

### Existing models

An `extern` module reuses a model written elsewhere: Verilog-A compiled to OSDI,
or another simulator by cosimulation.

```rust
#[link("bsim4.osdi", model = "bsim4")]
extern "osdi" module Bsim4(d: Electrical, g: Electrical, s: Electrical, b: Electrical) {
    l: f64 = 1u;
    w: f64 = 1u;
    vth0: f64 = 0.4;
}

/// Binning: the function picks the params, the caller places the result.
pub fn nch(l: f64, w: f64) -> Bsim4 {
    let vth0 = if l < 0.18u { 0.42 } else if l < 1u { 0.45 } else { 0.47 };
    Bsim4 { l: l, w: w, vth0: vth0 }
}

module Inverter(a: Electrical, y: Electrical, vss: Electrical) {
    let m1 = nch(0.13u, 2u)(y, a, vss, vss);
}

#[link("libuart_rx.so")]
extern "cosim" module UartRxRtl(in clk: Bit, in rx: Bit, out byte: Bit<u8>, out dv: Bit) {}
```

## Analog behavior

### Contributions

`probe +> x` adds `x` to the flow or to the potential of a branch, a pair of
nodes. `ddt` is the time derivative. A capacitor, an inductor and two coupled
inductors:

```rust
analog for Capacitor { i(p, n) +> ddt(c * v(p, n)); }
analog for Inductor  { v(p, n) +> ddt(l * i(p, n)); }

module Coupled(p1: Electrical, n1: Electrical, p2: Electrical, n2: Electrical) {
    l1: f64; l2: f64; k: f64 where k.abs() <= 1.0;
    let mu = k * (l1 * l2).sqrt();
}
analog for Coupled {
    v(p1, n1) +> ddt(l1 * i(p1, n1) + mu * i(p2, n2));
    v(p2, n2) +> ddt(l2 * i(p2, n2) + mu * i(p1, n1));
}
```

### Switch branches

A branch may be a flow branch on one path and a potential branch on another. An
ideal switch is a short when closed and an open circuit when open, with no
`ron`/`roff` to tune:

```rust
module Switch(a: Electrical, b: Electrical, in ctl: Bit) {}
analog for Switch {
    if ctl.high() { v(a, b) +> 0.0; } else { i(a, b) +> 0.0; }
}
```

### Nonlinear devices: limiting, charge and noise

A junction diode with an optional series resistance. `limit` bounds how far Newton
moves the junction voltage per iteration; it changes the path to the answer, never
the answer. The same block serves every analysis: the charge term is a current in
a transient and an admittance in AC, and the noise source appears only in a noise
analysis.

```rust
module Diode(a: Electrical, k: Electrical) {
    is: f64 = 1e-14 where is > 0.0;
    n: f64 = 1.0 where (0.1..=10.0).contains(n);
    rs: Option<f64>;
    cj0: f64 = 0.0 where cj0 >= 0.0;
    signal ai: Electrical;
}

analog for Diode {
    match rs {
        Some(r) => { i(a, ai) +> v(a, ai) / r; }
        None    => { v(a, ai) +> 0.0; }          // no resistor: `ai` and `a` are one node
    }
    let nvt   = n * vt();
    let vcrit = nvt * (nvt / (2.0_f64.sqrt() * is)).ln();
    let vd    = limit(v(ai, k), |new, old| pnjlim(new, old, nvt, vcrit), vcrit);
    let id    = is * ((vd / nvt).exp() - 1.0);
    i(ai, k) +> id + ddt(cj0 * vd) + white_noise(2.0 * Q * id, "shot");
}
```

The library has `pnjlim`, `fetlim`, `limvds`, `deltalim` and `limexp`,
`white_noise`, `flicker_noise` and `noise_table`, and `ddx` for a partial
derivative such as `gm`.

### Sources and small-signal analysis

A source says what it is in each analysis. `ac_stim` is the phasor of an AC sweep
and zero everywhere else.

```rust
module Vsource(p: Electrical, n: Electrical) {
    dc: f64 = 0.0;
    ac: f64 = 0.0;
}
analog for Vsource { v(p, n) +> dc + ac_stim(ac); }
```

### Integrators, filters and edges

The library's stateful operators are modules used inside expressions: `idt` and
`idtmod`, `delay`, `slew`, `transition`, the Laplace filters (`laplace_nd` and its
zero-pole forms), the sampled filters (`zi_nd` and its forms) and `table_model`. A
voltage-controlled oscillator integrates a phase and turns its crossings into a
clock:

```rust
module Vco(vctl: Electrical, out clk: Bit = false) {
    f0: f64 = 1G;
    kvco: f64 = 100M;
    let ph = idtmod(f0 + kvco * v(vctl), 0.0, 1.0, -0.5);    // phase in [-0.5, 0.5)
}

digital for Vco {
    on above(ph + 0.25) { clk <- true; }
    on cross(ph - 0.25) { clk <- false; }
}

analog for Vco { bound_step(0.05 / f0); }
```

### Equations and other physics

A nature whose flow is a residual turns any equation into a row of the system, with
its own tolerance. Nothing in the language is specific to electricity: the same
forms describe heat.

```rust
/// An equation is a row: `err` is the residual, tested against 1 µV.
nature Servo {
    potential: Voltage { access: v,   abstol: 1u },
    flow:      Voltage { access: err, abstol: 1u },
}

/// Ideal op-amp: drives `vout` so that v(inp, inn) == 0.
module Nullor(inp: Electrical, inn: Electrical, vout: Electrical) { signal x: Servo; }
analog for Nullor {
    err(x) +> v(inp, inn);
    v(vout) +> v(x);
}

/// A thermal mass on a thermal port.
module HeatSink(t: Thermal) { cth: f64 = 2.0; rth: f64 = 4.0; tamb: f64 = 300.0; }
analog for HeatSink { pwr(t) +> ddt(cth * temp(t)) + (temp(t) - tamb) / rth; }
```

## Digital behavior

### Reactions and commits

A reaction `on ev { … }` runs once when its event fires. A commit `q <- x` takes
effect after every reaction of that instant has run, so each reaction reads the
values from before the edge, as registers do.

```rust
module Counter(in clk: Bit, out count: Bit<u8> = 0, out full: Bit = false) {}
digital for Counter {
    on clk.rise() {
        count <- count + 1;
        full  <- count == 254;      // reads the old count
    }
}
```

### Definitions and delays

`y = expr;` defines a carrier as a function of others, like a gate. `after` gives
it a delay, on a definition or on a commit. A latch from two cross-coupled NOR
gates and a free-running clock:

```rust
module SrLatch(in s: Bit, in r: Bit, out q: Bit = false, out qn: Bit = true) {
    tpd: f64 = 10p where tpd > 0.0;
    q  = !(r | qn) after tpd;
    qn = !(s | q)  after tpd;
}

module Osc(out clk: Bit = false) {
    f: f64 = 200M where f > 0.0;
}
digital for Osc { on clk.changed() { clk <- !clk after 0.5 / f; } }
```

### Enums and state machines

A carrier holds any value that has a bit representation, an enum with fields
included, and `match` reads it.

```rust
#[derive(Repr, PartialEq)]
enum Tx { Idle, Start, Data { n: u3 }, Stop }      // a 2-level tag and a 3-level payload

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

### Memories

An array of carriers is a memory. A word exists once it is written.

```rust
module Ram(in clk: Bit, in we: Bit, in addr: Bit<u8>, in wdata: Bit<u32>, out rdata: Bit<u32>) {
    signal mem: [Bit<u32>; 256];
}
digital for Ram {
    on clk.rise() {
        if we { mem[addr] <- wdata; }
        rdata <- mem[addr];              // read-before-write
    }
}
```

## Events

An event is a value of type `Event`: it fires at instants. Events combine with
`||`, take a guard with `&&`, can be named by a `let` and returned by a function.

| Event | Fires |
| --- | --- |
| `c.changed()`, `c.rise()`, `c.fall()` | when a discrete carrier changes |
| `cross(e)` | when the real expression `e` rises through zero, located inside the step |
| `above(e)` | like `cross`, and also at the start if `e` is already positive |
| `at(t)` | at the time `t`; re-armed when `t` changes |
| `every(t0, period)` | periodically |
| `init()` | once, at the start |
| `settle` | once per time point, after everything has settled |
| `a \|\| b`, `ev && cond` | either event; an event under a guard |

```rust
/// A comparator that is right from the operating point on.
module Comparator(inp: Electrical, inn: Electrical, out q: Bit = false) {
    ttol: f64 = 1p;
    let up = above(v(inp, inn), ttol);
    let dn = above(-v(inp, inn), ttol);
}
digital for Comparator { on up || dn { q <- up; } }

/// A phase-frequency detector: two edges set, a guarded event clears after a delay.
module Pfd(in fref: Bit, in fdiv: Bit, out up: Bit = false, out dn: Bit = false) { treset: f64 = 50p; }
digital for Pfd {
    on fref.rise() { up <- true; }
    on fdiv.rise() { dn <- true; }
    on (up.changed() || dn.changed()) && up && dn { up <- false after treset; dn <- false after treset; }
}

pub fn handshake(req: Bit, ack: Bit) -> Event { req.rise() && ack }
```

An analog crossing is a located instant: the solver lands on it, the reaction
runs there, and what the reaction commits is seen by the analog block from that
instant on.

## Unknown values

Every level of every value is **known** or **unknown**. Unknown is not a third
logic level: it records that a level has not been established, as for a register
nobody reset. One rule covers carriers, locals and function arguments.

| Written | Result |
| --- | --- |
| `u & false`, `u \| true` | `false`, `true`: the bitwise operators work level by level |
| `u + 1`, `u == 3`, `u as f64` | unknown: every other operator is all or nothing |
| `if u { a } else { b }` | the **merge** of both paths: levels where `a` and `b` agree stay known |
| `x.is_known()`, `x.known()`, `x.known_or(d)`, `same(a, b)` | always known: how a model reads the flag |

When the condition that decides control is unknown, every path it does not exclude
is taken and every effect is merged, commits included:

```rust
module Mux(in sel: Bit, in a: Bit<u8>, in b: Bit<u8>, out y: Bit<u8>) {
    y = if sel { a } else { b };       // `sel` unknown: the levels where a and b agree stay known
}
```

An `analog` block computes with known reals only, so a model must say what an
unknown means electrically. The compiler refuses a discrete read that does not
decide:

```rust
module Dac8(in code: Bit<u8>, vout: Electrical) {
    vref: f64 = 1.0; rout: f64 = 1k;
    pub signal core: Electrical;
}
analog for Dac8 {
    v(core) +> vref * (code.known_or(0x80) as f64) / 256.0;     // unknown code: mid-scale
    i(core, vout) +> v(core, vout) / rout;
}
```

## Values and functions

Models compute with an ordinary typed value language.

**Scalars.** `bool`; integers of any width (`u1` … `u128`, `i1` … `i128`,
`UInt<N>`, `Int<N>`), which wrap at their width in every build; floats, with `f64`
the type of every physical quantity. Real literals take SI suffixes.

```rust
const R_LOAD: f64 = 6.6;
const C_OUT: f64 = 2.2u;              // T G M k m u n p f a
const F_SW: f64 = 500k;
const COUNTS: u9 = 400;               // nine bits, no more
const MASK: u4 = !0u4;                // 15

fn dithered_width(duty: u12, frame: u3) -> u9 {
    let coarse = (duty >> 3) as u9;
    let fine = (duty & 7) as u3;
    if frame < fine { coarse + 1 } else { coarse }
}
```

**Composite types.** Tuples, arrays with a const length, structs, enums with
fields, `Option`, and patterns over all of them.

```rust
#[derive(PartialEq)]
struct Point { x: f64, y: f64 }

#[derive(PartialEq)]
enum Shape { Dot, Circle(f64), Rect { w: f64, h: f64 } }

fn area(s: Shape) -> f64 {
    match s {
        Shape::Dot => 0.0,
        Shape::Circle(r) => PI * r * r,
        Shape::Rect { w, h } => w * h,
    }
}

fn first_set(bits: [bool; 8]) -> Option<u8> {
    let k = 0u8;
    while k < 8 {
        if bits[k] { return Some(k); }
        k += 1;
    }
    None
}
```

**Traits, generics and closures.** A trait names what a type provides; functions,
structs and modules are generic over types and constants; a closure is a function
written where it is used.

```rust
trait Area {
    const SIDES: u32;
    fn area(self) -> f64;
    fn is_empty(self) -> bool { self.area() == 0.0 }
}

struct Square { side: f64 }

impl Area for Square {
    const SIDES: u32 = 4;
    fn area(self) -> f64 { self.side * self.side }
}

fn total<T: Area, const N: usize>(shapes: [T; N]) -> f64 {
    shapes.iter().map(|s| s.area()).sum()
}

fn apply<F: Fn(f64) -> f64>(f: F, x: f64) -> f64 { f(x) }
```

**Value semantics.** Data are values. A value is copied wherever it is used, every
`let` may be assigned again, and a method that changes a value returns the new one.
There is nothing to borrow and nothing to move; hardware, which does have identity,
is named through ports instead.

```rust
struct Filter { acc: f64 }

impl Filter {
    fn step(self, x: f64) -> Filter { Filter { acc: 0.9 * self.acc + 0.1 * x } }
}

fn smooth(xs: [f64; 4]) -> f64 {
    let f = Filter { acc: 0.0 };
    for x in xs { f = f.step(x); }
    f.acc
}
```

**Functions** keep no state and write no carrier, so they are safe to call from
any block. Trailing `Option` parameters may be left out or named
(`cross(e, etol: 1u)`), a `const fn` runs when the design is built, and a function
may take a carrier and return an `Event`. Files are modules (`mod logic;`,
`use crate::logic::Sar;`), and a package is a `Piperine.toml` with `path` and
`git` dependencies.

## Output: streams and checks

There is one output statement. `emit!` writes a typed record to a named stream;
checks are library functions built on it. Nothing in the language reads a stream
back, so printing never changes a result, and a record written during a step the
solver rejects never appears.

```rust
/// Writes a record when `a` rises through `level`.
module Monitor(a: Electrical) {
    level: f64 = 0.5;
}

digital for Monitor {
    on cross(v(a) - level) {
        emit!("trip", v = v(a));
        require("crossed_upward", v(a) >= level);
    }
}
```

`require` must hold, `cover` records that a case was reached, and `target` reports
a margin that an optimizer can maximize. `on settle` runs them at every time point:

```rust
module Limits(p: Electrical, n: Electrical, vo: Electrical, vi: Electrical, tj: Thermal) {
    hi: f64 = 1.2;
}

digital for Limits {
    on settle {
        require("head_hi", v(p, n) <= hi);
        target("gain_db", db20(v(vo) / v(vi)) - 40.0);       // a margin: met at or above zero
        cover("hot", temp(tj) > 400.0);
    }
}
```

The library also has monitors for temporal properties, placed like any instance:
`Never`, `Stable`, `Setup`, `Hold`, `MinPulse`, `AckWithin`.

```rust
module Guarded(in clk: Bit, in hs: Bit, in ls: Bit) {
    let no_shoot_through = Never(clk, hs & ls);
}
```

## Running it: the host

The language describes one design and what happens inside one run. Everything
around a run belongs to the **host**: the `piperine` command, or a program that
uses the host library. The host picks the top module, sets params, drives the top
ports, asks for an analysis and reads every node, carrier and stream by path.

### Analyses

A run is a table of `Piperine.toml`, or flags on a lone file:

```toml
[run.step]
bench = "benches/step.msl"
analysis = "transient"
stop = 5e-6
print_step = 1e-6
```

| Analysis | Gives |
| --- | --- |
| `op` | the operating point |
| `dc` | an operating point per value of a swept param |
| `ac` | magnitude and phase over frequency |
| `transient` | the waveforms over time, analog and digital together |
| `tf` | small-signal gain, input and output impedance |
| `pz` | poles and zeros |
| `sens` | the derivative of a signal with respect to params |
| `pss` | the periodic steady state |

```console
$ piperine run lowpass.msl --analysis tf --input v1 --output vout
run lowpass: tf, top Lowpass, backend rust

quantity          value
gain              7.500000e-1
input_impedance   4.000000e3
output_impedance  7.500000e2
```

### The command line

```sh
piperine new rc                      # a project that runs
piperine check                       # parse, resolve, type-check, elaborate
piperine run step                    # a run of Piperine.toml
piperine run f.msl --analysis op     # a lone file
piperine test                        # judge every run by its checks
piperine info                        # params, ports and streams of a design
piperine build                       # compile ahead
piperine fmt [--check]               # format, comments kept
```

A run prints its records as they are written and then its table. Flags change one
run without touching the manifest: `--param r1.r=2200` sets a param by path,
`--stream trip` filters records, `--records r.jsonl`, `--csv t.csv` and
`--plot p.svg` write files, and `--reltol`, `--temp`, `--nodeset vout=5` and the
other solver options carry the names ngspice gives them.

### Params at run time, sweeps and optimization

A param is fixed during a solve and the host may set it between solves, so a DC
sweep or a change of supply in the middle of a transient needs no recompilation:
the `where` condition is checked on every set, and a value that fails it is
refused. A host program in Rust opens a `Session` on a project
(`piperine-api`), drives the top module's `in` ports, sets params at chosen times
and reads the result as a typed table and record streams.

Sweeps over many runs, Monte Carlo, regression and optimization are host loops by
design: the language has no construct for them, and a model stays the same file
whether it is run once or ten thousand times. The specification sketches that
host in Python ([Part VIII](docs/spec/part_viii_host.md)):

```python
sim = pp.Session("uart.msl", top="UartRx", params={"clks_per_bit": 87})
opt = pp.Optimize(sim, vary=["w", "l"], objectives=["gain_db"], constraints=["ipk_lim"], mc=200)
best = opt.run()          # gradient-based, from the model's own derivatives
```

## Correctness

- **Checked before it runs.** Names, types, natures, directions, param conditions
  and the rules of each block are checked when the design is built. A flow probe on
  a discrete carrier, two drivers on a `Bit`, a commit from an `analog` block or an
  unknown that reaches an equation is an error, not a surprise at 3 µs.
- **Every error has a code and a place.** Codes are grouped by the part of the
  specification that owns the rule (`E03xx` structure, `E04xx` behavior, `E05xx`
  the run, `E08xx` the host), and a diagnostic points at the source span or the
  instance path. A form the compiler does not support yet is refused by name with
  `P0001`; nothing is ever computed by a rule other than the specified one.
- **Deterministic.** The result of a session is a function of the source, the
  param values, the options and the host's actions. There are no races to find.
- **Cross-checked against ngspice.** The analog engine is compared with ngspice on
  twin circuits (the same circuit as a `.cir` and as `.msl`) under fixed tolerances,
  across operating point, DC sweep, AC, transient, transfer function, poles and
  zeros, sensitivity and periodic steady state.
- **A design held to its own specification.** [`examples/buck`](examples/buck) is a
  digitally controlled buck converter: 12 V in, a 200 MHz digital PWM with dither
  and dead time, a 10-bit ADC, a fixed-point PID, soft start, current limit,
  over-voltage protection and power-good, about 700 lines of MSL in one package. It
  was written to [`SPEC.md`](examples/buck/SPEC.md), starts with its loop closed
  and regulates at 0.8 V within 3 mV; the tests run the SPEC's scenarios against
  the SPEC's numbers.

## A first run

```sh
piperine new rc        # an RC low-pass filter with a step-response bench
cd rc
piperine run op        # the operating point
piperine run step      # a 5 µs transient
```

The generated library is three devices; the resistor is the whole model of a
resistor:

```rust
/// Ideal resistor.
pub module Resistor(p: Electrical, n: Electrical) {
    r: f64 where r > 0.0;
}

analog for Resistor {
    i(p, n) +> v(p, n) / r;
}
```

The bench places the parts and a monitor (the `Monitor` of
[Output](#output-streams-and-checks)):

```rust
module Step() {
    signal vin: Electrical;
    signal vout: Electrical = 0.0;
    let v1 = Vsource(vin, gnd) { dc: 1.0 };
    let r1 = Resistor(vin, vout) { r: 1k };
    let c1 = Capacitor(vout, gnd) { c: 1n };
    let mon = Monitor(vout);
}
```

`piperine run step` prints the monitor's record at the located crossing, RC·ln 2,
and then the table:

```console
run step: transient, top Step, backend rust
@6.931071e-7 Step.mon trip v=5.000000e-1

time         Step.vin    Step.vout
0.000000e0   1.000000e0  0.000000e0
1.000000e-6  1.000000e0  6.320928e-1
2.000000e-6  1.000000e0  8.646682e-1
3.000000e-6  1.000000e0  9.502193e-1
4.000000e-6  1.000000e0  9.816886e-1
5.000000e-6  1.000000e0  9.932653e-1
```

[`examples/rc`](examples/rc) is this project, and [`examples/buck`](examples/buck)
is the converter:

```sh
cd examples/buck
piperine run startup    # the closed-loop start, 600 µs
piperine test           # every run, judged by its checks
```

## Status

The specification is complete, and the front end parses and resolves all of it:
the standard library, the galleries and every snippet above. Type checking,
elaboration and the backends cover a subset that grows feature by feature.
[`GAPS.md`](GAPS.md) is the full list, kept against the binary.

**Runs from source today**

- Analog: continuous natures, your own included; flow and potential contributions;
  `ddt`; contributions under `if` and `match` (switch branches); `limit` with
  `pnjlim`, `fetlim`, `deltalim` and `limexp`; `ac_stim`; `transition`; held nodes.
- Digital: reactions, commits and definitions with `after`; `bool`, integers up to
  64 bits, `f64`, enums without fields, `Option`; `if`, `match`, `if let`; unknown
  values.
- Events: `changed`, `rise`, `fall`, `cross`, `above`, `at`, `every`, `init`,
  `settle`, `||` and guards.
- Mixed-signal: reactions that probe, analog blocks that read carriers, modules
  with a result, `A2D` and `D2A`. `examples/buck` runs whole, loop closed.
- Output: `emit!` from reactions, `require`, `cover`, `target`, `log`, `Never`.
- Modules: `f64` and `bool` params with `where`, hierarchy, `pub` items read by
  path, files, crates and packages.
- Host: the eight analyses above, the `piperine` commands above, params set before
  a run and at a time during one, drives of top ports from a Rust host.
- Parity with ngspice on a corpus of small twin circuits: a divider, a current source, diode
  circuits, RC in AC and transient, a resistor over temperature, and one twin each
  for `tf`, `pz`, `sens` and `pss`. Transistor models have no twin yet.

**Specified, parsed, and refused by name (`P0001`) until implemented**

- Values: structs, tuples and arrays as values; enums with fields; assignment and
  loops; generic functions, your own traits and methods with `self`; closures kept
  in a local; integers wider than 64 bits.
- Carriers: bundles and `flip`; resolved natures (`Wire`, `Tri`, `SumNet`) and the
  library built on them; arrays of carriers and memories; a discrete nature of your
  own.
- Modules: generic modules and const parameters; generation `for`/`if`; templates
  and module-typed params; `extern` modules; params of an integer, enum or
  `Option` type.
- Analog: `noise`, `idt`, `idtmod`, `delay`, `ddx`, the Laplace and sampled
  filters, `table_model` and `include_str!`; `emit!` in an `analog` block.
- Host: a Python host; Monte Carlo, optimization and gradients; noise,
  S-parameter and distortion analyses (the solver has them, the host does not
  offer them yet); `force` and `release`.

**Known differences from the specification**

- The specification removed references, `mut`, `Clone` and `Copy` on 2026-11-20
  (data are values). The parser still accepts the earlier spellings, and
  `examples/buck` and the standard library's sources still use some of them.
- `piperine check` types only what a run's top module reaches, so a library with
  no run is parsed and resolved but not type-checked.
- An unknown integer carries one flag for the whole value, where the specification
  keeps one per level; `&` and `|` therefore lose known bits in some cases.
- The language server does not build at present; `piperine check` gives the same
  diagnostics.

## Documentation

- **The MSL Book** ([`docs/book/`](docs/book)): the user manual, 21 chapters.
  Part 1 teaches the language, mixed-signal modelling, running simulations and the
  command line, with the output of every command; Part 2 goes into the object
  model, the device ABI, the solver and how Piperine is organized.
- **The specification** ([`docs/spec/`](docs/spec/index.md)): the authoritative
  definition of MSL. Parts I to VIII define values, carriers, structure, behavior,
  execution, output, the standard library and the host; the appendices hold the
  grammar, three galleries of worked models (analog, digital, mixed) and the
  library catalogue.
- **The documentation site** builds both with `mkdocs build`; the `Docs` workflow
  also produces the specification as a PDF.

## Building

Piperine is a Rust workspace (edition 2024). The compiled backend turns a design
into Rust, so running a model needs a Rust toolchain on the machine.

```sh
cargo build --release      # builds the piperine command
cargo test                 # the test suite
```

| Crate | Role |
| --- | --- |
| `piperine-lang` | MSL front end: parser, object model, name resolution, type checker, elaboration to the IR |
| `piperine-ir` | The elaborated design as an object model, with its printer and checks |
| `piperine-plan` | What the two backends share: the plan of each body |
| `piperine-transpile` | The compiled backend: a design printed as Rust and built with `rustc` |
| `piperine-interp` | The reference interpreter, a second backend for checking the first |
| `piperine-abi` | The device ABI between compiled models and the solver |
| `piperine-analog-solver` | The analog engine: assembly, Newton, TR-BDF2, step control |
| `piperine-digital-solver` | The digital engine: nets, events, microsteps, commits |
| `piperine-solver` | The analyses and the protocol that lets the two engines take turns |
| `piperine-api` | The host library: sessions, runs, results and streams |
| `piperine-cli` | The `piperine` command |
| `piperine-project` | Packages: `Piperine.toml`, dependencies and the lockfile |
| `piperine-parity` | The ngspice parity harness and the end-to-end scenarios |

## License

See [LICENSE](LICENSE).
