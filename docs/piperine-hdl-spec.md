# Piperine Hardware Definition Language — Design Specification

PHDL is a standalone mixed-signal hardware definition language. It describes analog
(continuous, Newton–Raphson) and digital (discrete, event-driven) hardware in one model and is
paired one-to-one with the Piperine intermediate representation.

---

## 1. Goals

- **One model for mixed signal.** Continuous and discrete hardware share the same module,
  type, and function constructs, with an explicit, checked boundary between them.
- **Few orthogonal concepts.** A small set of constructs that compose. Where two concepts
  overlap they are unified; where they only look alike they are kept apart.
- **No implicit behavior.** Type conversion, domain crossing, and driver resolution are never
  inserted automatically; the source states them.
- **Well-formed by construction.** A program that type-checks elaborates to a structurally
  valid netlist: matched widths, single-driver where required, no implicit domain crossings.
- **Compile-time by default.** Anything resolvable before the run — constant folding, dead
  branches, generic monomorphization, analysis specialization — is resolved then, never paid
  for at runtime.
- **Machine-writable.** The grammar is LL(1) and unambiguous. A `todo!` expression is a legal
  placeholder that type-checks.

---

## 2. Core model

There are two layers, kept strictly separate:

- **Values** are pure data. A value's type is a **value type**, living in parameters,
  variables, expressions, and function results.
- **Nets** carry signals. A net's type is a **net type** — a **discipline** or a net-capable
  **bundle** — living in ports and wires. A net type is a **storage** (the value carried) plus
  a **resolution** (how multiple drivers combine).

A `mod` declares the **shape** of a module. Its **behavior** is written in `analog` and
`digital` blocks, which run on different engines (§8). A `bundle`'s methods live in an `impl`.
Pure value computation is a `fn`, and a `capability` is a contract a type may implement.

Two evaluation phases are distinguished by location: constructs in a `mod` body are
structural, resolved at **elaboration**; constructs in `analog`/`digital` blocks are
behavioral, evaluated during the **solve** (§9).

---

## 3. Naming conventions

- **PascalCase** for modules, bundles, value types, net types, disciplines, enums, and
  capabilities: `Resistor`, `UInt`, `Electrical`, `SarState`, `Add`.
- **snake_case** for functions and methods: `thermal_voltage()`, `get_vr()`.
- **lowercase / snake_case** for ports, parameters, variables, fields, and instances.

---

## 4. Top-level items and packages

| Item | Purpose |
|------|---------|
| `discipline` | A net type: storage plus resolution (§6.2). |
| `bundle` | An aggregate of values or nets (§6.5). |
| `enum` | An enumerated value over a digital representation (§6.4). |
| `capability` | A contract a type can implement, with operator sugar (§6.6). |
| `fn` | A pure value function (§7). |
| `mod` | Module shape (§5). |
| `analog` / `digital` | A module's hardware behavior (§8). |
| `impl` | A bundle's methods, or a capability implementation (§6.5, §6.6). |
| `const` | A global compile-time constant. |

**Packages** are file- and directory-based: a file or directory is a package, with no
namespace declaration, no index file, and no re-export. An item is private unless marked
`pub`, and `use pkg::item` imports a public item.

```phdl
// file devices/passives.phdl  →  package devices::passives
pub mod Resistor ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1.0e3; }

use devices::passives::Resistor;          // elsewhere
```

---

## 5. Modules

```phdl
mod Name [CONST] <TYPE> ( PORTS ) {
    // params, vars, wires and arrays, child instances, structural for / if
}
```

A module may take **compile-time const parameters** in `[]` after its name (each a `Natural`,
resolved at elaboration) and **type parameters** in `<>` (§6.6). A const parameter scales an
architecture without threading the width through ports, and is what makes user types uniform
with `UInt[N]`:

```phdl
mod Driver[N] ( input in_ : Bit[N], output out : DataLine[N] ) { ... }
```

Behavior is written separately in `analog` / `digital` blocks (§8). Braces are omitted when a
module has only ports.

### 5.1 Ports

A port has a direction and a **net type** — a discipline or a net-capable bundle, never a bare
value type. Vector ports use `NetType[N]`.

| Direction | Meaning |
|-----------|---------|
| `input`  | Directional in: a digital net read here, or a high-impedance analog sense. |
| `output` | Directional out, single-driver. |
| `inout`  | Bidirectional / conservative — the form for conservative analog terminals (KCL applies). |

### 5.2 Storage classes

| Keyword | Role |
|---------|------|
| `param` | Configuration constant, fixed at instantiation and settable by the parent. A value type. |
| `wire`  | An internal net, or net array `NetType[N]`. |
| `var`   | A mutable binding. In a `digital` block it is combinational unless it must hold a value, when it infers memory (§8.3). A value type. |

### 5.3 Instances and connectivity

A child binds **ports positionally in `()`** and **parameters by name in `{}`**. An instance
may be named with `name : Module`:

```phdl
Resistor ( a, b ) { .r = 50.0 };          // anonymous
r1 : Resistor ( a, b ) { .r = 50.0 };     // named
```

Connection is by shared net. A named instance exposes each of its ports as a net `name.port`,
which the parent may connect, probe, or **contribute to from its own `analog` block**. Because
contributions accumulate by KCL, the parent can add current at a child's terminal — a parasitic
load, coupling, or trim — without a separate component:

```phdl
load : Capacitor ( out, gnd ) { .c = 1.0e-12 };
analog Tile { I(load.p, gnd) <+ cpar * ddt(V(load.p, gnd)); }   // extra charge at the child node
```

Referencing a child's node is the reason an instance must be named; an anonymous instance can
be connected but never addressed afterward. An instance in a `for` is named as an array
`name[i]`, and `name[i].port` reaches the node of each replica.

### 5.4 Arrays and structural loops

A structural `for` over a constant range builds parametric structure; ranges are half-open
(`lo..hi`) or inclusive (`lo..=hi`):

```phdl
mod RcChain[N] ( inout in_ : Electrical, inout out : Electrical, inout gnd : Ground ) {
    param r : Real = 1.0e3;  param c : Real = 1.0e-9;
    wire  node : Electrical[N + 1];

    node[0] = in_;  node[N] = out;
    for i in 0..N {
        Resistor  ( node[i], node[i + 1] ) { .r = r };
        Capacitor ( node[i + 1], gnd )     { .c = c };
    }
}
```

### 5.5 Structural conditionals

An `if` in a `mod` body selects which instances exist; its condition is an elaboration
constant. Runtime topology change is a switch branch (§8.2), not here.

---

## 6. Types

### 6.1 Value types

| Type | Meaning |
|------|---------|
| `Real` | Real number; continuous quantities and real parameters. |
| `Natural` | Non-negative integer; indices, widths, counts. |
| `Integer` | Signed integer for computation. |
| `Complex` | Complex number; meaningful only in frequency-domain (AC) analysis. |
| `String` | Text, for diagnostics. |
| `Boolean` | Two-state logic value (0, 1). |
| `Quad` | Four-state logic value (0, 1, X, Z) with standard propagation. |

Primitive types carry built-in operators. `UInt[N]` and `SInt[N]` are *not* primitive — they
are standard-library bundles over `Bit[N]` (§6.6). `Boolean` widens to `Quad` implicitly;
casts are otherwise explicit (`real(x)`, `int(x)`, `bit(x)`).

Literals: `Real` as `1.0e3`; `Boolean` as `0`/`1`; `Quad` with a `0q` prefix (`0q0`, `0q1`,
`0qX`, `0qZ`). Arrays use an element list `[a, b, c]`, a repeat `[value; N]` (e.g. `[0qZ; 8]`),
or a comprehension `[ expr | i in 0..N ]`,
are indexed `a[i]` and sliced `a[lo..hi]`, and nest — `Bit[8][16]` is 16 eight-bit words.

### 6.2 Disciplines — net types

A `discipline` declares a net type as a **storage** (its representation) plus a resolution
(§6.3). A conservative analog discipline declares a potential and a flow, with optional named
attributes, and resolves by Kirchhoff's current law:

```phdl
discipline Electrical {
    potential v : Real (unit = "V", abstol = 1e-6);
    flow      i : Real (unit = "A", abstol = 1e-12);
}
discipline Thermal {
    potential temp : Real (unit = "K", abstol = 1e-4);
    flow      pwr  : Real (unit = "W", abstol = 1e-9);
}
```

A signal-flow discipline declares only a potential. `Ground` is predefined, fixed at zero. A
digital discipline declares a single storage value type:

```phdl
discipline Bit   { storage Boolean; }     // two-state
discipline Logic { storage Quad; }        // four-state
```

A digital net is read by name, yielding its storage value; an analog net is read through its
discipline's accessors (`V`, `I`, `Temp`, `Pwr`), since it carries both a potential and a flow.

### 6.3 Resolution

- **Conservative** disciplines resolve by KCL, always and implicitly.
- **Single-driver** is the default for signal-flow and digital nets; a second driver is an
  error. `Bit` is single-driver only — with no high-impedance state, two drivers cannot combine.
- **Resolved** nets permit multiple drivers via a `resolve` clause, available only where the
  storage is `Quad`. A vector resolves per line.

```phdl
discipline DataLine { storage Quad; resolve tri; }
```

Built-in resolutions are `tri`, `or`, and `and`.

### 6.4 Enums

```phdl
enum SwState { Open, Closed }                                  // sequential → Bit[1]
enum Phase   { Idle = 0b00, P1 = 0b01, P2 = 0b10, P3 = 0b11 }  // explicit values
enum OpCode : Bit[32] { Mov = 0, Add, Sub, Jmp = 16 }          // explicit width
```

An optional `: Repr` fixes the underlying digital net type; otherwise it is
`Bit[ceil(log2(count))]`. Values default to sequential from zero.

### 6.5 Bundles

A `bundle` aggregates named fields, each a value or net type, with optional defaults:

```phdl
bundle FilterSpec { cutoff : Real = 1.0e3, order : Natural = 2 }
bundle DiffPair   { p : Electrical, n : Electrical }          // all nets → net-capable
bundle Stream     { data : Bit[8], valid : Bit }
```

A bundle is **net-capable** when every field is a net type (recursively); such a bundle types
a port or wire, otherwise a `param`/`var`. A bundle is **direction-agnostic**: a port applies
one direction to the whole bundle; an interface with mixed internal direction is two bundles.

**Connection and field access.** Two nets of the same bundle type connect field-by-field by
name. A field is read or driven individually as `b.field`.

**Methods and constructors.** A bundle's methods are `fn`s over `self` in an `impl`; an
operator is sugar for a method (§6.6). A named constructor is an associated `fn` returning
`Self`. A value is built with a literal `Name { .field = value }`, and an omitted field takes
its default:

```phdl
bundle Complex { re : Real = 0.0, im : Real = 0.0 }

impl Complex {
    fn polar(mag: Real, ang: Real) -> Self {
        return Complex { .re = mag * cos(ang), .im = mag * sin(ang) };
    }
}

var c : Complex = Complex::polar(1.0, 0.5);
```

### 6.6 Capabilities and generics

A `capability` is a named contract — function signatures a type can implement, like a trait. A
type satisfies it through a separate `impl ... for`, where `Self` is the implementing type:

```phdl
capability Add { fn add(self, o: Self) -> Self; }
```

**Operators are sugar over standard capabilities** — `a + b` is `a.add(b)` — so implementing
`Add` grants `+`. The standard set: `Add`/`Sub`/`Mul`/`Div`, `Eq`, `Ord`, and bitwise
`BitAnd`/`BitOr`/`BitXor`/`Not`. Primitive types satisfy the relevant ones intrinsically. A
capability may require others and supply default bodies:

```phdl
capability Number : Add, Sub, Mul {
    fn double(self) -> Self { return self.add(self); }
}
```

**Generics.** A `mod`, `bundle`, or `capability` is parameterized by type in `<>` and by const
in `[]`. A bound is a set of capabilities combined with `+`; `Type` (any value type) and `Net`
(any net type) are the root capabilities:

```phdl
mod Adder <T: Add + Net> ( input a : T, input b : T, output y : T );
digital Adder { y <- a + b; }

bundle Pair <T: Type> { fst : T, snd : T }
```

**Fixed-width integers are library, not magic.** `UInt[N]` and `SInt[N]` are bundles over
`Bit[N]` implementing the arithmetic capabilities in PHDL — letting vectors, buses, and numeric
types be defined rather than built in:

```phdl
bundle UInt[N] { bits : Bit[N] }

impl Add for UInt[N] {
    fn add(self, o: Self) -> Self {
        var r : Bit[N];
        var carry : Boolean = 0;
        for i in 0..N {
            r[i]  = self.bits[i] ^ o.bits[i] ^ carry;
            carry = (self.bits[i] & o.bits[i]) | (carry & (self.bits[i] ^ o.bits[i]));
        }
        return UInt[N] { .bits = r };
    }
}
```

### 6.7 Global constants

A `const` declares a global compile-time constant that is evaluated during elaboration. It can be used anywhere a parameter or literal is valid, and is visible to any module in scope.

```phdl
const PI : Real = 3.141592653589793;
const THRESHOLD : Natural = 10;
```

---

## 7. Functions

A `fn` is a pure value function — value arguments to a value result, with no contributions,
forces, state, or events. Because it is pure it inlines at the call site, which is what lets it
serve every context uniformly:

- at **elaboration**, computing a parameter or width;
- in a **digital** block, as combinational logic (the adder above);
- in an **analog** block, as an *analog function* — a `Real`-valued `fn` inlines into the
  contribution and is differentiated with it for the Jacobian, exactly as a Verilog-A analog
  function. The split removes the old ambiguity: a `fn` over `Real` belongs in analog, a `fn`
  over discrete types in digital, and the types decide, with no separate function kind.

```phdl
fn thermal_voltage(t: Real) -> Real { return 8.617e-5 * t; }
```

A `mod` is reusable structure; a `fn` is reusable value computation. Arguments pass by value
for basic types and by reference (read-only, since pure) for bundles. Functions live at package
scope and as bundle methods (§6.5).

### 7.1 Higher-order functions and generation

Generation in PHDL is the elaboration phase being a real language — pure values and types
evaluated early to *emit* hardware — not a macro stage over syntax. The distinction is the
whole guard against magic: a generator is understood by **running** it, not expanding it, and
what it produces is type-checked afterward like any other code.

A function is a value. Its type is written `fn(T, U) -> R`, a `fn` may take and return
functions, and a lambda `|a, b| a + b` is an anonymous one. A lambda is pure and may capture
only elaboration constants, never mutable state — the restriction that keeps higher-order code
free of hidden behavior. With these, the collection operators are ordinary library functions:

```phdl
fn map<T, U>(xs: T[N], f: fn(T) -> U) -> U[N] { return [ f(xs[i]) | i in 0..N ]; }

fn reduce<T>(xs: T[N], op: fn(T, T) -> T) -> T {
    if (N == 1) { return xs[0]; }
    return op( reduce(xs[0..N/2], op), reduce(xs[N/2..N], op) );
}
```

When `T` is a net type and `op` is combinational, `reduce(parts, |a, b| a + b)` emits a
balanced adder tree — a mux tree, priority encoder, or parallel-prefix network is the same
pattern. Generation by evaluation, with nothing expanded.

**Bounded recursion.** A `fn` may recurse, but recursion is resolved entirely at elaboration
and must terminate: each call reduces a const parameter (above, `N` halves toward the `N == 1`
base case), with a hard depth limit as a backstop. The elaboration phase stays a total, pure
evaluator — never a Turing-complete macro stage — so generation cannot loop forever or escape
the type system.

---

## 8. Behavior

Behavior is written in `analog` and `digital` blocks named after the module. The two run on
different engines and obey different rules:

- An **`analog`** block builds the continuous system. Its statements are contributions (`<+`)
  and forces (`<-`) that the Newton–Raphson solver stamps and resolves every iteration. It may
  read analog quantities (`V`, `I`) and digital values.
- A **`digital`** block computes next state. Its statements are drives (`<-`), assignments
  (`=`), and events (`@`), evaluated by the event-driven kernel. It may read digital values and
  sample analog quantities.

A leaf device has one block; a boundary device takes the block of the domain it *drives* — a
comparator is `digital` (samples `V`, drives a `Bit`), a 1-bit DAC is `analog` (reads a `Bit`,
forces `V`). A `for` in either block is unrolled into hardware, so its bound must be an
elaboration constant; an unbounded loop is a compile error. Behavior may branch on the current
analysis via `$analysis`, which returns an `Analysis` enum (`Dc`, `Ac`, `Tran`, `Noise`); the
compiler specializes each analysis, so the branch costs nothing at runtime.

### 8.1 Access functions

Continuous quantities are read through discipline accessors; the node pair is the branch:
`V(a, b)` and `I(a, b)` for the branch between `a` and `b`, `V(n)` for a node potential.
Built-ins: `ddt(x)`, `idt(x)`, the math functions (`exp`, `ln`, `sqrt`, `pow`, `tanh`, …), and
the casts of §6.1.

### 8.2 Analog behavior

| Operator | Meaning |
|----------|---------|
| `<+` | **Contribution.** Adds a term to a branch's equation; contributions to one branch accumulate. |
| `<-` | **Force.** Imposes a single-driver value — an ideal source or short, value or controlled expression. One force per quantity per branch. |

Each contribution or force is a stamp: a flow is an injected current, a potential a voltage
source (an internal branch-current unknown). The solver resolves all stamps together. An ideal
element defined by a pure constraint is approximated with finite parameters (a large but finite
gain), keeping every statement a direct stamp.

A **switch branch** toggles which quantity it forces, giving runtime topology change over a
static set of nodes; an open ideal switch is stabilized by a small conductance. Analog state
takes an initial condition through `@ initial`; `$bound_step(dt)` caps the next solver step.

```phdl
analog Switch {
    if (ctrl == Closed) { V(a, b) <- 0.0; } else { I(a, b) <- 0.0; }
}
```

### 8.3 Digital behavior

A `<-` drives a net; a `=` assigns a `var`. A digital body is **combinational by default**: an
assignment is dataflow and a later statement reads the value just assigned. A `var` read on a
path where it was not assigned must retain its value, inferring a **latch** — the latch
inference of Verilog and VHDL. A `var` updated inside a clocked `@` block instead becomes an
edge-triggered **register**: its reads within the block see the value held before the edge, so
a chain of register writes is a pipeline, not a collapse. Where two writes overlap, the last in
source order wins.

Control flow is `if`/`else` and `match`; a `match` over an enum is checked for exhaustiveness.

```phdl
digital SarAdc {
    result <- code;
    @ posedge(clk) {
        match state {
            Idle    => { if (start == 1) { state = Convert; idx = n - 1; code = 0; code[n-1] = 1; } }
            Convert => {
                if (cmp == 0) { code[idx] = 0; }
                if (idx == 0) { state = Done; } else { idx = idx - 1; code[idx-1] = 1; }
            }
            Done    => { if (start == 0) { state = Idle; } }
        }
    }
}
```

### 8.4 Events

A nested `@ EVENT { ... }` block runs when its event fires — the place a `var` becomes state.

| Source | Fires on |
|--------|----------|
| `posedge(sig)` / `negedge(sig)` | a digital edge |
| `change(sig)` | any change in value |
| `cross(expr)` / `above(expr)` | an analog expression crossing / exceeding zero |
| `initial` / `final` | once, at t=0 / end |

Events combine with **OR** (`|`). A `when` guard restricts a block to fire only while a level
holds. An analog crossing (`cross`/`above`) may drive digital state, which is how a comparator
or level detector couples the domains.

### 8.5 Diagnostics

`$error(msg)`, `$warn(msg)`, `$info(msg)` report during evaluation; `$assert(cond, msg)` reports
when `cond` is false. In `@ initial` it validates setup: `@ initial { $assert(n > 0, "n>0"); }`.

---

## 9. Phase model

Two phases by location: **elaboration** (params, structural `for`/`if`, instance selection in a
`mod` body — resolved once into a fixed netlist) and **solve** (`analog`/`digital` behavior,
evaluated during simulation). A solve-phase value never controls elaboration structure;
hardware is neither created nor destroyed during the solve. Runtime topology is a switch branch
over a static netlist.

---

## 10. The No-Magic rule

Connecting incompatible disciplines is a compile error; crossing a discipline or domain
boundary requires an explicit converter `mod`. The rule governs net connections only — reading
a net into a value or driving a value onto a net is ordinary. A device that couples two
disciplines internally (§B.2) needs no converter, because no single net crosses a boundary.

---

## 11. Future layers

- **In-language verification.** Stimulus, assertions, sweeps, and scoreboards as a language layer.
- **Higher-order modules.** Passing a module generator as a parameter — a `Pipeline<Stage>[K]`
  that instantiates any stage K times, an `Arbiter<T>[N]` over any interface — for a real
  structural standard library. It needs modules to expose their interface as a type, and is
  deferred until the value-level generation of §7.1 has settled. Generation is deliberately not
  built on macros: a macro operates on syntax outside the type system, which is the source of
  the "magic" PHDL avoids.

---

## 12. Open design questions

- **Composite storage.** Storage is a scalar value type. Whether a net may carry an aggregate
  (a bundle of values, a structured nettype) is left open.
- **Bidirectional bundles.** Mixed-direction interfaces are two bundles today; a per-field
  direction and flip may be worth adding if handshake-heavy digital appears.

---

## Appendix A — Core library

```phdl
discipline Electrical {
    potential v : Real (unit = "V", abstol = 1e-6);
    flow      i : Real (unit = "A", abstol = 1e-12);
}

mod Resistor  ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1.0e3; }
analog Resistor  { I(p, n) <+ V(p, n) / r; }

mod Capacitor ( inout p : Electrical, inout n : Electrical ) { param c : Real = 1.0e-9; }
analog Capacitor { I(p, n) <+ c * ddt(V(p, n)); }

mod VSource ( inout p : Electrical, inout n : Electrical ) { param dc : Real = 0.0; }
analog VSource { V(p, n) <- dc; }

fn thermal_voltage(t: Real) -> Real { return 8.617e-5 * t; }

mod Diode ( inout a : Electrical, inout c : Electrical ) {
    param is_sat : Real = 1.0e-14;  param temp : Real = 300.0;
}
analog Diode { I(a, c) <+ is_sat * (exp(V(a, c) / thermal_voltage(temp)) - 1.0); }

mod Comparator ( input vp : Electrical, input vn : Electrical, output out : Bit );
digital Comparator { out <- (V(vp) > V(vn)); }

mod BitToVoltage ( input d : Bit, inout a : Electrical ) {
    param vlow : Real = 0.0;  param vhigh : Real = 1.8;
}
analog BitToVoltage { if (d == 1) { V(a) <- vhigh; } else { V(a) <- vlow; } }
```

## Appendix B — Worked architectures

### B.1 Parametric N-bit SAR ADC — analog + digital in one module set

```phdl
enum SarState : Bit[2] { Idle, Convert, Done }

mod Dac[N] ( input code : Bit[N], inout out : Electrical, inout gnd : Electrical ) {
    param vref : Real = 1.8;
}
analog Dac {
    var acc : Real = 0.0;
    for i in 0..N {
        if (code[i] == 1) { acc = acc + vref * pow(2.0, real(i)) / pow(2.0, real(N)); }
    }
    V(out, gnd) <- acc;
}

mod SarAdc[N] (
    input clk : Bit, input start : Bit, input vin : Electrical, inout gnd : Electrical,
    output result : Bit[N], output done : Bit,
) {
    wire dout : Electrical;  wire cmp : Bit;
    var  state : SarState = Idle;  var code : Bit[N] = 0;  var idx : Natural = 0;
    param cload : Real = 50.0e-15;

    dac  : Dac[N]     ( code, dout, gnd );
    comp : Comparator ( vin, dout, cmp );
}
analog SarAdc {
    I(dac.out, gnd) <+ cload * ddt(V(dac.out, gnd));   // parasitic load on the DAC node
}
digital SarAdc {
    result <- code;
    done   <- (state == Done);
    @ posedge(clk) {
        match state {
            Idle    => { if (start == 1) { state = Convert; idx = N - 1; code = 0; code[N-1] = 1; } }
            Convert => {
                if (cmp == 0) { code[idx] = 0; }
                if (idx == 0) { state = Done; } else { idx = idx - 1; code[idx-1] = 1; }
            }
            Done    => { if (start == 0) { state = Idle; } }
        }
    }
}
```

### B.2 Electrothermal coupling — two disciplines, one device, no converter

```phdl
discipline Thermal {
    potential temp : Real (unit = "K", abstol = 1e-4);
    flow      pwr  : Real (unit = "W", abstol = 1e-9);
}

mod HeatedResistor ( inout p : Electrical, inout n : Electrical, inout th : Thermal ) {
    param r0 : Real = 1.0e3;  param t0 : Real = 300.0;  param tc : Real = 0.004;
}
analog HeatedResistor {
    var rt : Real = r0 * (1.0 + tc * (Temp(th) - t0));
    I(p, n) <+ V(p, n) / rt;
    Pwr(th) <+ V(p, n) * V(p, n) / rt;
}
```

### B.3 LC oscillator — analog initial condition

```phdl
mod LcTank ( inout p : Electrical, inout n : Electrical ) {
    param l : Real = 1.0e-6;  param c : Real = 1.0e-9;
}
analog LcTank {
    I(p, n) <+ c * ddt(V(p, n)) + idt(V(p, n)) / l;
    @ initial { V(p, n) = 1.0; }
}
```

### B.4 SR latch — bistability as event-held state

```phdl
mod SrLatch ( input s : Bit, input r : Bit, output q : Bit ) { var st : Bit = 0; }
digital SrLatch {
    q <- st;
    @ (posedge(s) | posedge(r)) { if (s == 1) { st = 1; } else { st = 0; } }
}
```

### B.5 Ideal op-amp — finite-gain VCVS

```phdl
mod OpAmp ( input inp : Electrical, input inn : Electrical, inout out : Electrical ) {
    param gain : Real = 1.0e6;
}
analog OpAmp { V(out) <- gain * V(inp, inn); }
```

### B.6 Tri-state data bus — resolved multi-driver

```phdl
discipline DataLine { storage Quad; resolve tri; }

mod Driver[N] ( input en : Bit, input val : Logic[N], inout bus : DataLine[N] );
digital Driver { if (en == 1) { bus <- val; } else { bus <- [0qZ; N]; } }
```

### B.7 Two clock domains — synchronizer

```phdl
mod Synchronizer ( input d : Bit, input clk_b : Bit, output q : Bit ) {
    var m : Bit = 0;  var n : Bit = 0;
}
digital Synchronizer {
    q <- n;
    @ posedge(clk_b) { m = d; n = m; }
}
```

### B.8 First-order delta-sigma modulator — a closed mixed-signal loop

The hardest case for the analog/digital boundary: a loop that crosses it twice. The clocked
quantizer samples the analog integrator (analog → digital), and the feedback level is read back
into the analog block from the digital register (digital → analog). The register `q` is what
makes the loop well-posed — it is a unit delay, so there is no zero-delay algebraic loop across
the boundary.

```phdl
mod DeltaSigma ( input vin : Electrical, inout gnd : Ground, input clk : Bit, output dout : Bit ) {
    param c : Real = 1.0e-12;  param r : Real = 1.0e3;  param vref : Real = 1.0;
    wire intg : Electrical;            // integrator output
    var  q : Bit = 0;                  // quantizer register (held across clocks)
}
analog DeltaSigma {
    var vfb : Real = if (q == 1) { vref } else { -vref };   // digital state read in analog
    I(intg, gnd) <+ c * ddt(V(intg, gnd));                  // integrating capacitor
    I(intg, gnd) <+ (vfb - V(vin)) / r;                     // (feedback − input) drives the node
}
digital DeltaSigma {
    dout <- q;
    @ posedge(clk) { q = (V(intg) > 0.0); }                 // clocked 1-bit quantizer
}
```

### B.9 Ring oscillator — feedback that is illegal in digital and essential in analog

A combinational cycle is a compile error in a `digital` block, because a zero-delay loop has no
fixed point. The same topology is the entire mechanism of a ring oscillator, and it is
well-posed in `analog`: each stage's finite bandwidth is a differential equation, and an odd
ring has no stable DC point, so it oscillates. The two engines treat a cycle oppositely, which
is exactly correct — delay is physical in analog and absent in ideal digital.

```phdl
mod Inverter ( input a : Electrical, inout y : Electrical, inout gnd : Ground ) {
    param gain : Real = 10.0;  param c : Real = 1.0e-15;  param r : Real = 1.0e3;
}
analog Inverter {
    var target : Real = -gain * V(a, gnd);                  // inverting gain
    I(y, gnd) <+ c * ddt(V(y, gnd)) + (V(y, gnd) - target) / r;   // single-pole settling
}

mod RingOsc[N] ( inout gnd : Ground ) {                     // N odd
    wire node : Electrical[N];
    for i in 0..N {
        Inverter ( node[i], node[(i + 1) % N], gnd );       // last stage closes the ring
    }
}
```

### B.10 RC ladder with per-tap parasitics — named-instance arrays and parent contributions

Each leg is named in the `for`, so the parent's `analog` block can reach every internal tap as
`name[i].port` and add a parasitic capacitor there — modeling layout without inserting a
component per node.

```phdl
mod Ladder[N] ( inout bus : Electrical, inout gnd : Ground ) {
    param r : Real = 1.0e3;  param cpar : Real = 5.0e-15;
    wire tap : Electrical[N];
    for i in 0..N {
        rseg[i] : Resistor ( bus, tap[i] )    { .r = r };
        rgnd[i] : Resistor ( tap[i], gnd )    { .r = r };
    }
}
analog Ladder {
    for i in 0..N {
        I(rseg[i].n, gnd) <+ cpar * ddt(V(rseg[i].n, gnd));   // parasitic at each tap, via the named leg
    }
}
```

### B.11 Generic pipelined accumulator — generics, registers, and width

Parameterized over width and depth. It stresses two corners: the register inference of a
clocked block applied to a generic type, and the width of an accumulating sum — `acc` must be
wide enough to hold the running total, which the type, not the compiler, must state. Whether
`UInt[W] + UInt[W]` yields `UInt[W]` (wrapping) or `UInt[W+1]` (carry-out) is the open width
question §6.6 leaves to the standard library's capability definitions.

```phdl
mod Accumulator[W] ( input clk : Bit, input en : Bit, input x : UInt[W], output sum : UInt[W] ) {
    var acc : UInt[W] = 0;
}
digital Accumulator {
    sum <- acc;
    @ posedge(clk) when (en) { acc = acc + x; }     // '+' is UInt's Add capability (§6.6)
}
```
