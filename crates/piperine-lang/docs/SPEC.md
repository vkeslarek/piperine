# Piperine Hardware Definition Language — Complete Specification

The single authoritative reference. Part I is the normative language specification; Parts II–VI
are the elaboration model, formal grammar, reflection API, selector, and builtins reference. All
parts are consistent with the current design: `UInt`/`SInt`/`Complex` are library bundles;
disciplines are conservative or storage; metadata rides on `@` attributes; the core grammar is
closed and grows through extension layers.

## Contents

- **I — Language Specification.** Goals and governing rules (No-Magic, tier independence,
  No-Bloat), the value/net model, lexical form, modules, types, attributes, functions and
  generation, `analog`/`digital` behavior, phases, the extension model, rejected features, and
  worked architectures.
- **II — Elaboration.** Source → `ElabProgram`: const evaluation, type/discipline resolution,
  structural elaboration, monomorphization, bundle expansion, events, injected stdlib, validation.
- **III — Grammar.** The LL(1) EBNF, including SI literals, attributes, inferred `var` types,
  named ports, and match patterns.
- **IV — Reflection (POM).** The typed object graph for `bench`/Python/Rust and the plugin ABI.
- **V — Selector.** The query language whose axes are POM relations and predicates POM attributes.
- **VI — Builtins.** Normative catalog of math, analog operators, `$`-syscalls, tasks, events,
  and the prelude/stdlib, with fidelity gaps and the alias policy.

---

# Part I — Language Specification

*Piperine Hardware Definition Language — Specification*

PHDL is a mixed-signal HDL: continuous (Newton–Raphson) and discrete (event-driven) hardware in
one model, with an explicit checked boundary. It lowers to the Piperine IR, which is
authoritative; the surface is a projection of it.

---

### 1. Goals and governing rules

- **One mixed-signal model.** Continuous and discrete hardware share module, type, and function
  constructs; the boundary between them is explicit and checked.
- **No-Magic.** Type conversion, domain crossing, and driver resolution are never inserted
  implicitly. The source states them.
- **Well-formed by construction.** A program that type-checks elaborates to a structurally valid
  netlist: matched widths, single-driver where required, no implicit domain crossings.
- **Compile-time by default.** Anything resolvable before the run (const folding, dead branches,
  monomorphization, analysis specialization) is resolved then.
- **Tier independence.** Tier-1 code (leaf devices, RTL) is readable and writable with no
  knowledge of the tier-3 machinery (capabilities, generics, higher-order functions). A construct
  that forces generic or capability syntax into a leaf device model is a defect.
- **No-Bloat (burden of proof).** A feature must demonstrate it cannot live in an extension layer
  (§13) before it may touch the core grammar.
- **Machine-writable.** Grammar is LL(1). `todo!` is a legal placeholder that type-checks.

---

### 2. Core model

Two layers, strictly separate:

- **Value** — pure data. A value type lives in `param`, `var`, expressions, `fn` results.
- **Net** — a signal carrier. A net type is a **discipline** or a net-capable **bundle**, and is
  defined as **storage** (the value carried) plus **resolution** (how drivers combine). It lives
  in ports and wires.

Constructs: `mod` (module shape, has identity/instances/ports/behavior), `bundle` (value or net
aggregate), `fn` (pure value computation), `capability` (a type contract), `impl` (a bundle's
methods or a capability implementation), `analog`/`digital` (behavior). Metadata attaches via
`@` attributes (§8).

Two phases by location: `mod`-body constructs are **elaboration** (resolved once into a fixed
netlist); `analog`/`digital` constructs are **solve** (evaluated by the engines). A solve value
never controls elaboration structure; topology is static (switch branches, §10.2, give runtime
topology over a fixed node set).

---

### 3. Naming conventions

PascalCase: modules, bundles, value types, net types, disciplines, enums, capabilities.
snake_case: functions, methods. lowercase/snake_case: ports, params, vars, fields, instances.
Instance-type vs instance-name matching in the selector relies on this convention.

---

### 4. Lexical

Identifiers: `[A-Za-z_][A-Za-z0-9_]*`. Comments: `//` line, `/* */` block.

Literals:
- `Real`: `1.0e3`; `Boolean`: `0`/`1`; `Quad`: `0q0` `0q1` `0qX` `0qZ`.
- **SI suffixes** on numeric literals (case-sensitive): `T G M k` (1e12…1e3), `m u n p f a`
  (1e-3…1e-18). `2u` = `2e-6`, `M` = mega, `m` = milli.
- **Digit separators**: `_` anywhere between digits (`1_000_000`).
- Arrays: element list `[a, b, c]`, repeat `[v; N]`, comprehension `[expr | i in 0..N]`; index
  `a[i]`, slice `a[lo..hi]`; nesting `Bit[8][16]` = 16 words of 8 bits.

System names are a distinct token class `$name` (§11). `@` prefixes both attributes (§8) and
events (§10.3); position disambiguates.

---

### 5. Top-level items and packages

| Item | Purpose |
|------|---------|
| `discipline` | net type: storage + resolution (§6.2) |
| `bundle` | value/net aggregate (§6.5) |
| `enum` | enumerated value over a digital repr (§6.4) |
| `capability` | type contract, operator sugar (§6.6) |
| `fn` | pure value function (§9) |
| `const` | global compile-time constant |
| `mod` | module shape (§7) |
| `analog`/`digital` | module behavior (§10) |
| `impl` | bundle methods / capability impl (§6.5–§6.6) |

Packages are file/directory-based: a file or directory is a package; no namespace declaration,
no index file, no re-export. Items are private unless `pub`; `use pkg::item` imports.

```phdl
// devices/passives.phdl → package devices::passives
pub mod Resistor ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1k; }
use devices::passives::Resistor;
```

`const NAME : T = expr;` — a global constant evaluated at elaboration, usable wherever a param
or literal is.

---

### 6. Types

#### 6.1 Value types

`Real`, `Natural` (indices/widths/counts), `Integer`, `Boolean` (2-state), `Quad` (4-state
0/1/X/Z, standard propagation), `String` (diagnostics). Primitives carry built-in operators.

`UInt[N]`, `SInt[N]`, and `Complex` are **standard-library bundles**, not primitives (§6.6).
`Boolean` widens to `Quad` implicitly; other casts are explicit (`real`, `int`, `bit`).

#### 6.2 Disciplines — net types

A discipline is one of exactly two kinds:

- **Conservative** — declares a `potential` and a `flow` (with optional named attributes);
  resolves by KCL, always and implicitly. Read via accessors (`V`, `I`, and the declared names).
- **Storage** — declares one `storage` value type; single-driver by default; read/driven by
  name. Optional `resolve` clause (§6.3) permits multiple drivers.

```phdl
discipline Electrical { potential v : Real (unit = "V", abstol = 1e-6);
                        flow      i : Real (unit = "A", abstol = 1e-12); }
discipline Voltage  { storage Real; }        // (former signal-flow: storage Real)
discipline Bit      { storage Boolean; }
discipline Logic    { storage Quad; resolve tri; }
```

`Ground` is the predefined conservative reference node. There is no separate "signal-flow" kind;
a potential-only net is `storage Real`.

#### 6.3 Resolution

- Conservative → KCL, implicit.
- Storage default → single-driver; a second driver is an error.
- `resolve` clause: on `Quad` storage, `tri | or | and` (needs the high-impedance state); on
  `Real` storage, `sum | avg | max | min` (numeric bus resolution). `Boolean` storage is
  single-driver only. A vector resolves per line.

#### 6.4 Enums

```phdl
enum SwState { Open, Closed }                          // sequential → Bit[1]
enum Phase   { Idle = 0b00, P1, P2, P3 }               // explicit / continuing
enum OpCode : Bit[32] { Mov = 0, Add, Sub, Jmp = 16 }  // explicit repr
```

`: Repr` fixes the underlying digital net type; default `Bit[ceil(log2(count))]`. Values default
sequential from zero, continuing from the last explicit.

#### 6.5 Bundles

Named fields of value or net type with optional defaults. **Net-capable** iff every field is a
net type (recursive): types a port/wire. Otherwise value-only: types a `param`/`var`.
**Direction-agnostic**: a port applies one direction to the whole bundle; mixed-direction
interfaces are two bundles. Same-type net bundles connect field-by-field by name; a field is
read/driven as `b.field`.

Methods and constructors live in `impl`; an operator is sugar for a method (§6.6). A named
constructor is an associated `fn -> Self`. Literal `Name { .field = v }`; omitted fields take
defaults.

```phdl
bundle Complex { re : Real = 0.0, im : Real = 0.0 }
impl Complex { fn polar(mag: Real, ang: Real) -> Self { return Complex { .re = mag*cos(ang), .im = mag*sin(ang) }; } }
```

#### 6.6 Capabilities and generics

A `capability` is a named contract of `fn` signatures; a type satisfies it via `impl Cap for T`.
`Self` is the implementing type. Operators desugar to standard capabilities (`a + b` → `a.add(b)`):
`Add Sub Mul Div`, `Eq`, `Ord : Eq`, `BitAnd BitOr BitXor Not`, `Number : Add+Sub+Mul` (with a
default `double`). Primitives satisfy the relevant ones intrinsically. A capability may require
supertraits and supply default bodies.

Generics: type params in `<>`, const params (Natural) in `[]`. A bound is a `+`-set of
capabilities; `Type` (any value type) and `Net` (any net type) are root capabilities.

```phdl
capability Add { fn add(self, o: Self) -> Self; }
mod Adder <T: Add + Net> ( input a : T, input b : T, output y : T );
digital Adder { y <- a + b; }
```

`UInt`/`SInt`/`Complex` are library bundles built this way (fixed-width arithmetic in PHDL, not
compiler magic). `Bit`-vector concatenation is the library `fn concat`, not an operator.

---

### 7. Modules

```phdl
mod Name [CONST] <TYPE> ( PORTS ) { params, vars, wires, instances, structural for/if }
```

Const params `[N]` scale an architecture without threading widths; type params `<T>` per §6.6.
Braces omitted when a module has only ports. Behavior is separate (§10).

#### 7.1 Ports

Direction + net type. `input` (directional in / high-impedance analog sense), `output`
(single-driver out), `inout` (bidirectional / conservative terminal, KCL). Vectors `NetType[N]`.

#### 7.2 Storage classes

`param` (elaboration constant, settable by parent, value type), `wire` (internal net / net
array), `var` (mutable binding; in `digital` combinational unless it must hold a value, then it
infers memory, §10.3; value type). An initialized `var` infers its type (`var acc = 0.0;` →
`Real`); `param`, ports, and fields require explicit types.

#### 7.3 Instances and connectivity

Ports positionally in `()` or by name `.p = net`; params by name in `{}`. An instance may be
named `name : Module`. A named instance exposes ports as nets `name.port`, which the parent may
connect, probe, or contribute to from its own `analog` block (KCL accumulation — parasitic load,
coupling, trim, with no extra component). Anonymous instances cannot be addressed afterward. A
`for` instance is an array `name[i]`; `name[i].port` reaches each replica. After behavioral `for`
unrolling (§10), `name[i].port` becomes `name_0.port`, `name_1.port`, etc. — the loop variable
is substituted by its concrete value, same as `if` const-folding.

```phdl
r1 : Resistor ( .p = a, .n = b ) { .r = 50 };
load : Capacitor ( out, gnd ) { .c = 1p };
analog Tile { I(load.p, gnd) <+ cpar * ddt(V(load.p, gnd)); }
```

`name.port` resolves to the parent-scope node that the instance's port is connected to. It is
not a separate terminal — it IS the parent node. Contributing `I(load.p, gnd) <+ expr` adds
current to that node (KCL accumulation); probing `V(load.p, gnd)` reads its voltage.

#### 7.4 Structural control

`for i in lo..hi` / `lo..=hi` over a constant range builds parametric structure; `if (const) {}
else {}` selects which instances exist. `$assert(cond, msg)` in a `mod` body is an
elaboration-time check.

---

### 8. Attributes (metadata)

`@schema(name = value, ...)` prefixes any declaration (`wire`, port, `param`, `mod`, `bundle`,
instance). It attaches typed metadata for tools (layout, routing, floorplan, matching). The
attribute set is stackable:

```phdl
@layout(min_width = 2u, layer = "m3") @route(priority = high) wire clk : Electrical;
@floorplan(x = 0, y = 0) mod Cpu ( ... ) { ... }
```

Governing rules (violation of any is a defect):
1. **Inert.** The core compiler never reads attribute content; it validates against the schema
   and attaches. Attributes do not affect elaboration or simulation.
2. **Removable.** Deleting every `@` yields an identical elaborated netlist and identical
   simulation. (Contrast Verilog `full_case`/`parallel_case` — attributes that changed synthesis
   semantics and caused sim/synth mismatch; forbidden here by rule 1.)
3. **Schema-typed.** Each `schema` is registered by a plugin as a value-only bundle shape;
   `@schema(...)` is type-checked against it. Unknown schema (plugin not loaded) is an error.

Attributes have two entry paths over one metadata store: **inline** (design intent, lives in
source) and **overlay** via the selector in a `bench`/host (flow intent — bulk annotation without
touching source, e.g. `select("//pll//net::*").meta(layout, spacing = 2u)`; versioned separately,
like SDC/UPF). Overlay wins over inline on conflict, with a diagnostic. Reflection exposes both
uniformly (POM `aspect::` axis). A module-level attribute replaces the former "aspect block";
there is one metadata mechanism.

---

### 9. Functions

`fn name(args) -> T` — pure (no contributions, forces, state, events). Inlines at the call site,
so it serves every context uniformly:
- elaboration (compute a param/width);
- `digital` (combinational logic);
- `analog` (a `Real`-valued `fn` inlines into a contribution and is differentiated for the
  Jacobian — the Verilog-A analog function, gated by type: `Real` → analog, discrete → digital).

Arguments pass by value (basic types) or read-only reference (bundles). `mod` = reusable
structure; `fn` = reusable value computation.

#### 9.1 Higher-order functions and generation

Generation is the elaboration phase evaluating pure values/types to emit hardware — not macros
over syntax. A function is a value: type `fn(T, U) -> R`; lambdas `|a, b| a + b` are pure and
capture only elaboration constants. Collection operators are library functions:

```phdl
fn map<T, U>(xs: T[N], f: fn(T) -> U) -> U[N] { return [ f(xs[i]) | i in 0..N ]; }
fn reduce<T>(xs: T[N], op: fn(T, T) -> T) -> T {
    if (N == 1) { return xs[0]; }
    return op( reduce(xs[0..N/2], op), reduce(xs[N/2..N], op) );
}
```

With net `T` and combinational `op`, `reduce(parts, |a,b| a+b)` emits a balanced adder tree
(mux tree / priority encoder / prefix network are the same pattern). Recursion is
elaboration-only and must terminate (each call reduces a const param; a hard depth limit is the
backstop) — the elaboration phase stays a total pure evaluator, never a Turing-complete macro
stage.

---

### 10. Behavior

`analog` and `digital` blocks, named after the module, run on different engines under one
statement grammar:

- **`analog`** builds the continuous system: contributions `<+` and forces `<-`, stamped and
  resolved by Newton–Raphson each iteration (blocking instruction list). Reads analog quantities
  and digital values.
- **`digital`** computes next state: drives `<-`, assignments `=`, events `@`, on the
  event-driven kernel (combinational dataflow; inferred memory). Reads digital values and samples
  analog quantities.

A leaf device has one block; a boundary device takes the block of the domain it *drives*
(Comparator: `digital`, samples `V`, drives `Bit`; 1-bit DAC: `analog`, reads `Bit`, forces `V`).
A `for` is unrolled (bound must be an elaboration constant; unbounded is an error). The loop
variable is substituted by its concrete value in every iteration — `for` is syntactic sugar,
fully resolved at elaboration, same as `if` const-folding. After unrolling, `rseg[i].n` becomes
`rseg_0.n`, `rseg_1.n`, etc. Behavior may branch on `$analysis` (§11), specialized per analysis
at compile time.

#### 10.1 Access functions

Conservative quantities read through accessors, node-pair as branch: `V(a,b)`, `I(a,b)`, `V(n)`.
Built-ins: `ddt`, `idt`, math (`exp ln sqrt pow tanh …`), casts (§6.1). The analog-operator set
is open (registry, §13); the builtins reference is normative and includes `idtmod`, `ddx`,
`transition`, `slew`, `delay`/`absdelay`, `laplace_*`, `zi_*`, `ac_stim`, `table` (1-D/N-D
measured-data lookup), and the noise sources `white_noise`/`flicker_noise`.

#### 10.2 Analog behavior

`<+` (contribution: accumulates on a branch) and `<-` (force: single-driver value/controlled
expression; one force per quantity per branch). Each is a stamp (flow = injected current,
potential = voltage source / internal branch-current unknown); the solver resolves all together.
Ideal constraint elements use a finite-parameter approximation (large-but-finite gain), keeping
every statement a direct stamp.

A **switch branch** toggles which quantity it forces (runtime topology over a static node set; an
open ideal switch is stabilized by a small conductance). `@ initial` sets an analog initial
condition; `$bound_step(dt)` caps the next step.

```phdl
analog Switch { if (ctrl == Closed) { V(a,b) <- 0.0; } else { I(a,b) <- 0.0; } }
```

#### 10.3 Digital behavior

`<-` drives a net; `=` assigns a `var`. Combinational by default: an assignment is dataflow, a
later statement reads the value just assigned. A `var` read on a path where it was not assigned
retains its previous value → **latch inference** (as Verilog/VHDL). A `var` updated in a clocked
`@` block is an edge-triggered **register**: within the block reads see the pre-edge value (a
chain of register writes is a pipeline). Overlapping writes: last in source order wins.

Inferred latches raise a **warning by default** (deny-able per project/attribute); register
inference is silent. *Fidelity note:* scalar digital state is fully device-compiled; `Bit[N]`
bus variables and indexed/sliced assignment (`code[idx] = 1`) parse and elaborate but are not
yet lowered by the digital JIT — they fail loud at device-compile. Control flow is `if`/`else` and `match`; `match` over an enum is
exhaustiveness-checked. Patterns: enum variants, `_`, and bit-pattern wildcards (`0b1??0`; `?` is
pattern-only, distinct from the `Quad` value `X`).

```phdl
digital SarAdc {
    result <- code;
    @ posedge(clk) {
        match state {
            Idle    => { if (start == 1) { state = Convert; idx = N-1; code = 0; code[N-1] = 1; } }
            Convert => { if (cmp == 0) { code[idx] = 0; }
                         if (idx == 0) { state = Done; } else { idx = idx - 1; code[idx-1] = 1; } }
            Done    => { if (start == 0) { state = Idle; } }
        }
    }
}
```

#### 10.4 Events

`@ EVENT [ when (cond) ] { ... }` — the only place a `var` becomes state. Sources:
`posedge`/`negedge`/`change` (digital), `cross`/`above` (analog crossing), `initial`/`final`
(once), `timer(period)` (periodic). Combine with OR via `|`. `when` gates on a level. An analog
crossing may drive digital state (domain coupling). An unrecognized event name is a compile
error. The event set is extensible (registry, §13).

#### 10.5 Diagnostics

`$info` / `$warn` / `$error` / `$fatal` report at increasing severity; `$assert(cond, msg)`
reports when false. Format strings interpolate arguments: `$info("vout = {}", V(out))`.
`$finish` ends the run. The `$`-syscall set (`$temperature`, `$vt`, `$abstime`, `$analysis`,
`$limit`, `$random`/`$dist_*`, `$simparam`, `$port_connected`, …) is open (registry, §13) and the
builtins reference is normative.

---

### 11. Phase model

**Elaboration**: params, structural `for`/`if`, instance selection, generic monomorphization,
const evaluation, analysis specialization — resolved once into a fixed netlist. **Solve**:
`analog`/`digital` behavior. Hardware is neither created nor destroyed during solve; runtime
topology is a switch branch over a static netlist.

---

### 12. No-Magic

Connecting incompatible disciplines is a compile error; crossing a discipline/domain boundary
needs an explicit converter `mod`. The rule governs net connections only — reading a net into a
value or driving a value onto a net is ordinary. A device coupling two disciplines internally
(Appendix B.2) needs no converter (no single net crosses a boundary).

---

### 13. Extension model

The core grammar (layer 0) is closed. Growth happens above it:

| Layer | Mechanism | Adds |
|-------|-----------|------|
| 1 | standard library (capabilities, generics, HOF) | value types (`UInt`, `Complex`), `map`/`reduce`/`concat` |
| 2 | compiler registries (trait + registry) | analog operators, `$`-syscalls, `@`-event kinds |
| 3 | attribute schemas (§8) | typed per-declaration metadata (layout, routing, …) |
| 4 | POM + selector + plugins (Rust/Python) | reflection, overrides, annotations — the design-closure loop |
| 5 | `bench` | orchestration, sweeps, verification, closure loops (effectful, interpreted) |

New value types (layer 1) and new operators/syscalls/events (layer 2) never touch the grammar.
Layers 3–4 carry the physical-design loop: `aspect`/attribute metadata + a plugin that reflects
over the POM, runs placement/extraction, and returns **annotations** (parasitics keyed by
`NetId`, fused by KCL) and **overrides** (staged, consumed by pure re-elaboration). The loop is
`reflect → emit → re-elaborate → simulate`, deterministic at every step. Verification and timing
live in layer 5, outside the hardware language. See the companion reflection, selector, and
extensibility specifications; layer 0 stays closed by the No-Bloat rule (§1).

---

### 14. Rejected features (decisions, not omissions)

- **Digital `#` delays.** Rejected: source of race semantics and delta cycles; RTL is zero-delay
  combinational + registers. Timing belongs to analog (`transition`, `absdelay`) or verification.
- **Connect modules / connectrules (auto-insertion).** Rejected by No-Magic; use explicit
  converter modules.
- **Indirect branch assignment (`V(x): f(x)==0`).** Rejected: singular systems; use the
  finite-parameter idiom.
- **`mod`/`bundle` unification.** Rejected: reproduces the SystemVerilog `interface`. They are
  distinct (identity+behavior vs. valued aggregate); their field syntax rhymes and that suffices.

---

### 15. Open questions

- **Composite storage** — whether a net may carry an aggregate value (structured nettype).
- **Bidirectional bundles** — per-field direction/flip if handshake-heavy digital appears.
- **Fixed-width arithmetic result width** — `UInt[W] + UInt[W]` as `UInt[W]` (wrap) vs `UInt[W+1]`
  (carry): a library capability decision.

---

### Appendix A — Core library (excerpt)

```phdl
discipline Electrical { potential v : Real (unit = "V", abstol = 1e-6);
                        flow      i : Real (unit = "A", abstol = 1e-12); }

mod Resistor  ( inout p : Electrical, inout n : Electrical ) { param r : Real = 1k; }
analog Resistor  { I(p, n) <+ V(p, n) / r; }

mod Capacitor ( inout p : Electrical, inout n : Electrical ) { param c : Real = 1n; }
analog Capacitor { I(p, n) <+ c * ddt(V(p, n)); }

mod VSource ( inout p : Electrical, inout n : Electrical ) { param dc : Real = 0.0; }
analog VSource { V(p, n) <- dc; }

fn thermal_voltage(t: Real) -> Real { return 8.617e-5 * t; }

mod Diode ( inout a : Electrical, inout c : Electrical ) { param is_sat : Real = 1e-14; param temp : Real = 300.0; }
analog Diode { I(a, c) <+ is_sat * (exp(V(a, c) / thermal_voltage(temp)) - 1.0); }

mod Comparator ( input vp : Electrical, input vn : Electrical, output out : Bit );
digital Comparator { out <- (V(vp) > V(vn)); }

mod BitToVoltage ( input d : Bit, inout a : Electrical ) { param vlow : Real = 0.0; param vhigh : Real = 1.8; }
analog BitToVoltage { if (d == 1) { V(a) <- vhigh; } else { V(a) <- vlow; } }
```

### Appendix B — Worked architectures

Each stresses a corner of the model.

**B.1 Parametric N-bit SAR ADC** — analog + digital in one module set; named children the parent
loads through its own `analog` block.

```phdl
enum SarState : Bit[2] { Idle, Convert, Done }

mod Dac[N] ( input code : Bit[N], inout out : Electrical, inout gnd : Electrical ) { param vref : Real = 1.8; }
analog Dac {
    var acc = 0.0;
    for i in 0..N { if (code[i] == 1) { acc = acc + vref * pow(2.0, real(i)) / pow(2.0, real(N)); } }
    V(out, gnd) <- acc;
}

mod SarAdc[N] ( input clk : Bit, input start : Bit, input vin : Electrical, inout gnd : Electrical,
                output result : Bit[N], output done : Bit ) {
    wire dout : Electrical;  wire cmp : Bit;
    var state : SarState = Idle;  var code : Bit[N] = 0;  var idx : Natural = 0;
    param cload : Real = 50f;
    dac  : Dac[N]     ( code, dout, gnd );
    comp : Comparator ( vin, dout, cmp );
}
analog SarAdc { I(dac.out, gnd) <+ cload * ddt(V(dac.out, gnd)); }   // parasitic load
digital SarAdc {
    result <- code;  done <- (state == Done);
    @ posedge(clk) {
        match state {
            Idle    => { if (start == 1) { state = Convert; idx = N-1; code = 0; code[N-1] = 1; } }
            Convert => { if (cmp == 0) { code[idx] = 0; }
                         if (idx == 0) { state = Done; } else { idx = idx - 1; code[idx-1] = 1; } }
            Done    => { if (start == 0) { state = Idle; } }
        }
    }
}
```

**B.2 Electrothermal** — two disciplines coupled inside one device; no converter needed.

```phdl
discipline Thermal { potential temp : Real (unit = "K", abstol = 1e-4); flow pwr : Real (unit = "W", abstol = 1e-9); }
mod HeatedResistor ( inout p : Electrical, inout n : Electrical, inout th : Thermal ) {
    param r0 : Real = 1k; param t0 : Real = 300.0; param tc : Real = 0.004;
}
analog HeatedResistor {
    var rt = r0 * (1.0 + tc * (Temp(th) - t0));
    I(p, n) <+ V(p, n) / rt;
    Pwr(th) <+ V(p, n) * V(p, n) / rt;
}
```

**B.3 LC oscillator** — analog initial condition, no DC operating point.

```phdl
mod LcTank ( inout p : Electrical, inout n : Electrical ) { param l : Real = 1u; param c : Real = 1n; }
analog LcTank { I(p, n) <+ c * ddt(V(p, n)) + idt(V(p, n)) / l;  @ initial { V(p, n) = 1.0; } }
```

**B.4 SR latch** — bistability as event-held state.

```phdl
mod SrLatch ( input s : Bit, input r : Bit, output q : Bit ) { var st : Bit = 0; }
digital SrLatch { q <- st;  @ (posedge(s) | posedge(r)) { if (s == 1) { st = 1; } else { st = 0; } } }
```

**B.5 Ideal op-amp** — finite-gain VCVS.

```phdl
mod OpAmp ( input inp : Electrical, input inn : Electrical, inout out : Electrical ) { param gain : Real = 1M; }
analog OpAmp { V(out) <- gain * V(inp, inn); }
```

**B.6 Tri-state bus** — resolved multi-driver (`Quad` + `resolve tri`).

```phdl
discipline DataLine { storage Quad; resolve tri; }
mod Driver[N] ( input en : Bit, input val : Logic[N], inout bus : DataLine[N] );
digital Driver { if (en == 1) { bus <- val; } else { bus <- [0qZ; N]; } }
```

**B.7 Synchronizer** — two clock domains; the register chain is a pipeline (pre-edge reads).

```phdl
mod Synchronizer ( input d : Bit, input clk_b : Bit, output q : Bit ) { var m : Bit = 0; var n : Bit = 0; }
digital Synchronizer { q <- n;  @ posedge(clk_b) { m = d; n = m; } }
```

**B.8 First-order delta-sigma** — a closed loop crossing the analog/digital boundary twice; the
register `q` is the unit delay that makes it well-posed (no zero-delay algebraic loop).

```phdl
mod DeltaSigma ( input vin : Electrical, inout gnd : Ground, input clk : Bit, output dout : Bit ) {
    param c : Real = 1p; param r : Real = 1k; param vref : Real = 1.0;
    wire intg : Electrical;  var q : Bit = 0;
}
analog DeltaSigma {
    var vfb = if (q == 1) { vref } else { -vref };
    I(intg, gnd) <+ c * ddt(V(intg, gnd));
    I(intg, gnd) <+ (vfb - V(vin)) / r;
}
digital DeltaSigma { dout <- q;  @ posedge(clk) { q = (V(intg) > 0.0); } }
```

**B.9 Ring oscillator** — feedback is a `digital` error (zero-delay loop, no fixed point) and the
`analog` mechanism itself (finite-bandwidth ODE, no stable DC point on an odd ring).

```phdl
mod Inverter ( input a : Electrical, inout y : Electrical, inout gnd : Ground ) {
    param gain : Real = 10.0; param c : Real = 1f; param r : Real = 1k;
}
analog Inverter { var target = -gain * V(a, gnd);  I(y, gnd) <+ c * ddt(V(y, gnd)) + (V(y, gnd) - target) / r; }

mod RingOsc[N] ( inout gnd : Ground ) {                     // N odd
    wire node : Electrical[N];
    for i in 0..N { Inverter ( node[i], node[(i + 1) % N], gnd ); }
}
```

**B.10 RC ladder with per-tap parasitics** — named-instance arrays; the parent reaches each tap
via `name[i].port`. Layout intent rides on an attribute.

```phdl
mod Ladder[N] ( inout bus : Electrical, inout gnd : Ground ) {
    param r : Real = 1k; param cpar : Real = 5f;
    wire tap : Electrical[N];
    for i in 0..N {
        rseg[i] : Resistor ( bus, tap[i] ) { .r = r };
        @route(shield = true) rgnd[i] : Resistor ( tap[i], gnd ) { .r = r };
    }
}
analog Ladder { for i in 0..N { I(rseg[i].n, gnd) <+ cpar * ddt(V(rseg[i].n, gnd)); } }
```

**B.11 Generic pipelined accumulator** — generics + register inference; sum width is a library
decision (§15).

```phdl
mod Accumulator[W] ( input clk : Bit, input en : Bit, input x : UInt[W], output sum : UInt[W] ) { var acc : UInt[W] = 0; }
digital Accumulator { sum <- acc;  @ posedge(clk) when (en) { acc = acc + x; } }
```

---

# Part II — Elaboration Phase

*Piperine HDL — Elaboration Phase*

Elaboration transforms a parsed `SourceFile` into a resolved `ElabProgram`: no generic
parameters, no unresolved constants, no structural `for`/`if`, no bundle references in port
lists. Consistency with the language spec (Complex/UInt/SInt as library bundles, two discipline
kinds, `@` attributes, unknown-event-is-error, latch warning) is assumed throughout.

### 1. Two phases

| Phase | Where | What |
|-------|-------|------|
| Elaboration | `mod` body, type annotations, structural control | resolved once into a fixed netlist |
| Solve | `analog`/`digital` | evaluated by the NR / event-driven engine |

A solve value never controls elaboration structure; runtime topology is a switch branch over a
static netlist.

### 2. Entry point

```rust
let source = parse_str(input)?;
let program = elaborate(source)?;   // inject stdlib → register items → validate → elaborate
```

Steps: prepend stdlib; register items (disciplines, bundles, enums, modules, behaviors, fns,
capabilities, impls, consts) into symbol tables; validate (§11); elaborate in dependency order.

### 3. Constant evaluation

Elaboration-position expressions (array dims, structural `for`/`if`, param defaults, const args,
`const`) must be compile-time constant.

`ConstVal = Int | Nat | Real | Bool | Str`. `ConstEnv` is a scoped name→ConstVal stack; each
`for` iteration pushes/pops the loop var. Evaluatable: literals (incl. SI-suffixed and
`_`-separated numerics), named bindings, unary `-`/`!`, binary arithmetic/comparison/bitwise,
`if/else`, block-with-trailing-value. Anything else (general calls, field access, comprehension)
is `NotConst`.

### 4. Type resolution

Resolves to `ElabNetType` or `ElabValueType`.

Primitive value types: `Real`, `Natural`, `Integer`, `Boolean`, `Quad`, `Str`. (`Complex`,
`UInt[N]`, `SInt[N]` are library bundles, resolved as bundles — not primitives.)

Disciplines resolve to `ElabNetType::Discipline(name)` of one of two kinds: **conservative**
(potential+flow, KCL) or **storage** (`storage T` + optional `resolve`). Enums →
`ElabValueType::Enum`. Arrays `Type[N]` evaluate `N` via `ConstEnv::eval_nat`; a free dimension
is an error.

A bundle is **net-capable** iff every field (recursively) is a net type; it may type a port and
is expanded (§7). A value-field bundle used as a net type is `NotNetCapable`. `Type`/`Net` are
root capabilities, satisfied implicitly.

### 5. Structural elaboration (mod body)

`for` unrolls (both bounds `Nat`); `if (const)` folds, emitting only the taken branch. `$assert`
in a `mod` body is an elaboration-time check. Errors: `<+`/`<-` in a `mod` body
(`ContribInModBody`/`ForceInModBody`).

V1: loop termination is not proven — the elaborator recurses with the loop var bound; a hard
depth limit is the intended backstop (else stack overflow on runaway const recursion).

### 6. Monomorphization

Const-param modules (`mod Dac[N]`) elaborate on instantiation: substitute `N`, elaborate, cache
under a mangled name (`Dac[8]`→`Dac__8`, `Grid[4,4]`→`Grid__4_4`). Type params (`<T: Add+Net>`)
substitute via a `type_subst` map; uninstantiated generics keep generic bodies, monomorphized on
demand. Generic `fn`s keep generic bodies; full inlining is the type checker's job.

### 7. Bundle expansion

A net-capable bundle port expands to one `ElabPort` per field, `{port}_{field}`:

```
input inp : DiffPair  →  input inp_p : Electrical,  input inp_n : Electrical
```

Field-access (`inp.p`) is preserved in behavior; the type checker rewrites to expanded names.

### 8. Behavior elaboration

Same const machinery, per-block rules. Behavioral `for` unrolls (elaboration-constant bound;
runtime loop is an error). Const `if` folds; runtime `if` is kept. `match` arms are kept
(type-checker does exhaustiveness). Latch inference (a `var` read unassigned on some path) raises
a warning by default; register inference (clocked `@`) is silent.

### 9. Event system

The parser emits `EventSpec::Named { name, arg }`; the elaborator resolves each against the
`EventRegistry` — events are extensible. Built-ins: `posedge`/`negedge`/`change` (digital),
`cross`/`above` (analog), `initial`/`final` (both), `timer` (both). Combine via `|`.

Domain validation: digital-edge in `analog` → `DigitalEventInAnalog`; analog-crossing in
`digital` → `AnalogEventInDigital`. **An unrecognized event name is `UnknownEvent` (a hard
error)** — no silent fallback.

Custom event (registry):

```rust
impl EventKind for SampleEvent { fn name(&self) -> &str { "sample" } fn is_digital_edge(&self) -> bool { true } }
elaborator.events_mut().register(SampleEvent);
```

### 10. Standard library (injected)

Capabilities: `Add Sub Mul Div` (one op method each), `Eq`, `Ord : Eq` (`lt`), `BitAnd BitOr
BitXor Not`, `Number : Add+Sub+Mul` (default `double`). Operator sugar: `+ - * / == < & | ^ !` →
`add sub mul div eq lt bitand bitor bitxor not`.

Collections: `map<T,U>(xs: T[N], f) -> U[N]`, `reduce<T>(xs: T[N], op) -> T` (divide-and-conquer,
base `N==1`), `concat`. Numeric bundles `UInt[N]`/`SInt[N]`/`Complex`. All are elaboration-time
generators: called with concrete `N`, they unroll to fixed structure.

### 11. Validation catalog

| Error | Trigger |
|-------|---------|
| `ContribInDigital` / `ContribInModBody` | `<+` in `digital` / `mod` body |
| `ForceInModBody` | `<-` in `mod` body |
| `AnalogEventInDigital` / `DigitalEventInAnalog` | event/block domain mismatch |
| `UnknownEvent(name)` | event name not registered |
| `UnknownAttrSchema(name)` | `@schema` not registered by any plugin |
| `NotNetCapable(name)` | value-field bundle used as net type |
| `UndefinedType` / `UndefinedModule` | name not found |
| `MissingConstParam { param, module }` | wrong const-arg count |
| `ConstEval { context, source }` | const expression failed |

Diagnostics (not errors): `InferredLatch(var)` — warning by default.

### 12. Future work

Import resolution (`use` stored, not resolved); full type-param monomorphization; bundle
field-access rewrite (`inp.p`→`inp_p`); operator-desugar pass; elaboration depth counter;
attribute-overlay merge from `bench`.

---

# Part III — Grammar (EBNF)

*Piperine HDL — Grammar (EBNF)*

LL(1) after the inline left-factoring. Semantic distinctions (value vs net type, access vs call,
`[Expr]` as const-arg vs array-dim) are left to the type checker.

Notation: `::=` def, `|` alt, `{X}` zero+, `[X]` opt, `(X)` group, `"x"` terminal.

### 1. Lexical

```
Ident      ::= (letter|"_") { letter|digit|"_" }
RealLit    ::= Digits "." Digits [ ("e"|"E") ["+"|"-"] Digits ] [ SiSuffix ]
             | Digits ("e"|"E") ["+"|"-"] Digits [ SiSuffix ]
NatLit     ::= Digits [ SiSuffix ] | "0b" BinDigits | "0x" HexDigits
Digits     ::= digit { digit | "_" }                     -- '_' separators
SiSuffix   ::= "T"|"G"|"M"|"k" | "m"|"u"|"n"|"p"|"f"|"a" -- case-sensitive; M=mega, m=milli
QuadLit    ::= "0q" ("0"|"1"|"X"|"Z")
StringLit  ::= '"' {char} '"'
SysCall    ::= "$" Ident
```

Comments `//` and `/* */`. PascalCase vs snake_case is convention, not lexical. `@` prefixes
attributes (§9) and events (§7); position disambiguates.

Keywords: `mod analog digital impl fn bundle enum discipline capability const use pub param wire
var input output inout potential flow storage resolve tri or and for in if else match return
when self Self initial final posedge negedge change cross above`.

### 2. Compilation unit

```
CompilationUnit ::= { UseDecl | Item }
UseDecl   ::= "use" Path ";"
Path      ::= Ident { "::" Ident }
Item      ::= { Attribute } [ "pub" ] ItemKind
ItemKind  ::= ModDecl | BehaviorDecl | DisciplineDecl | BundleDecl | EnumDecl
            | CapabilityDecl | ImplDecl | FnDecl | ConstDecl
ConstDecl ::= "const" Ident ":" Type "=" Expr ";"
```

### 3. Attributes

```
Attribute ::= "@" Ident "(" [ AttrArg { "," AttrArg } ] ")"
AttrArg   ::= Ident "=" Expr
```

An attribute prefixes any declaration (item, port, `param`, `wire`, `var`, instance); stackable.
`Ident` is a plugin-registered schema; args are checked against it. Attributes are inert (§8 of
the language spec).

### 4. Modules

```
ModDecl    ::= "mod" Ident [ConstParams] [TypeParams] PortList [ModBody]
ConstParams::= "[" Ident {"," Ident} "]"
TypeParams ::= "<" TypeParam {"," TypeParam} ">"
TypeParam  ::= Ident [ ":" Bound ]
Bound      ::= Ident { "+" Ident }
PortList   ::= "(" [ Port {"," Port} [","] ] ")"
Port       ::= { Attribute } Direction Ident ":" Type
Direction  ::= "input" | "output" | "inout"
ModBody    ::= "{" { ModStmt } "}"
ModStmt    ::= { Attribute } ( ParamDecl | WireDecl | VarDecl | StructuralFor | StructuralIf
                             | AssertStmt | InstanceOrConnect )
ParamDecl  ::= "param" Ident ":" Type [ "=" Expr ] ";"
WireDecl   ::= "wire" Ident ":" Type ";"
VarDecl    ::= "var" Ident [ ":" Type ] [ "=" Expr ] ";"        -- type inferred if initialized
AssertStmt ::= "$assert" "(" Expr "," Expr ")" ";"
```

Instances / connections, left-factored on `Ident { Indexer | Field }`:

```
InstanceOrConnect ::= Ident { Indexer | Field } InstTail
InstTail  ::= ":" ModuleRef PortArgs [ParamArgs] ";"    -- named instance
            | ConstArgs PortArgs [ParamArgs] ";"         -- anon w/ const args
            | PortArgs [ParamArgs] ";"                   -- anon
            | "=" Expr ";"                               -- net connection
ModuleRef ::= Ident [ConstArgs] [TypeArgs]
ConstArgs ::= "[" Expr {"," Expr} "]"
TypeArgs  ::= "<" Type {"," Type} ">"
PortArgs  ::= "(" [ PortArg {"," PortArg} ] ")"
PortArg   ::= Expr | "." Ident "=" Expr                  -- positional or named
ParamArgs ::= "{" [ ParamArg {"," ParamArg} ] "}"
ParamArg  ::= "." Ident "=" Expr
Indexer   ::= "[" Expr "]"   ;   Field ::= "." Ident
StructuralFor ::= "for" Ident "in" Range ModBody
StructuralIf  ::= "if" "(" Expr ")" ModBody [ "else" (ModBody|StructuralIf) ]
Range     ::= Expr ("..'|"..=") Expr
```

### 5. Types

```
Type ::= Ident [TypeArgs] { Indexer }     -- Indexer = const-arg or array-dim (semantic)
```

Disciplines / enums / bundles:

```
DisciplineDecl ::= "discipline" Ident "{" { DisciplineItem } "}"
DisciplineItem ::= NatureDecl | StorageDecl | ResolveDecl
NatureDecl     ::= ("potential"|"flow") Ident ":" Type [ AttrList ] ";"
AttrList       ::= "(" NamedAttr {"," NamedAttr} ")"     -- unit="V", abstol=1e-6
NamedAttr      ::= Ident "=" Expr
StorageDecl    ::= "storage" Type ";"
ResolveDecl    ::= "resolve" ("tri"|"or"|"and"|"sum"|"avg"|"max"|"min") ";"
EnumDecl       ::= "enum" Ident [ ":" Type ] "{" EnumVariant {"," EnumVariant} [","] "}"
EnumVariant    ::= Ident [ "=" Expr ]
BundleDecl     ::= "bundle" Ident [ConstParams] [TypeParams] "{" [ Field {"," Field} [","] ] "}"
Field          ::= { Attribute } Ident ":" Type [ "=" Expr ]
```

### 6. Capabilities, generics, functions

```
CapabilityDecl ::= "capability" Ident [ ":" Ident {"," Ident} ] "{" { FnSig | FnDecl } "}"
FnSig          ::= "fn" Ident [TypeParams] ParamList "->" Type ";"
ImplDecl       ::= "impl" [ Ident "for" ] TypeRef "{" { FnDecl } "}"
TypeRef        ::= Ident [ConstArgs] [TypeArgs]
FnDecl         ::= "fn" Ident [TypeParams] ParamList "->" Type Block
ParamList      ::= "(" [ Param {"," Param} ] ")"
Param          ::= "self" | Ident ":" Type
Block          ::= "{" { Stmt } [ Expr ] "}"             -- trailing Expr = value
```

`impl Cap for T` vs `impl T`: peek for `for` after the first `Ident`.

### 7. Behavior

```
BehaviorDecl  ::= ("analog"|"digital") Ident "{" { BehaviorStmt } "}"
BehaviorStmt  ::= VarDecl | BindStmt | IfStmt | MatchStmt | ForStmt | EventBlock | Diagnostic | ExprStmt
BindStmt      ::= Expr BindOp Expr ";"     ;   BindOp ::= "<+" | "<-" | "="
EventBlock    ::= "@" EventSpec [ "when" "(" Expr ")" ] Block
EventSpec     ::= EventTerm | "(" EventTerm { ("|"|"or") EventTerm } ")"
EventTerm     ::= Ident "(" [Expr] ")" | "initial" | "final"     -- name resolved by registry
Diagnostic    ::= SysCall "(" [ Expr {"," Expr} ] ")" ";"
```

LHS of `<+`/`<-` must be an access (checker); of `=`, an lvalue. After the LHS `Expr`, the
operator (or `";"`) gives the single-token branch.

### 8. Statements and expressions

```
Stmt      ::= VarDecl | ReturnStmt | IfStmt | MatchStmt | ForStmt | BindStmt | ExprStmt
ReturnStmt::= "return" Expr ";"   ;   ExprStmt ::= Expr ";"
IfStmt    ::= "if" "(" Expr ")" Block [ "else" (Block|IfStmt) ]
ForStmt   ::= "for" Ident "in" Range Block
MatchStmt ::= "match" Expr "{" { MatchArm } "}"
MatchArm  ::= Pattern "=>" Block [","]
Pattern   ::= Path | "_" | BitPattern            -- BitPattern: "0b" {"0"|"1"|"?"}

Expr      ::= OrExpr
OrExpr    ::= AndExpr  { "|"  AndExpr }
AndExpr   ::= EqExpr   { "&"  EqExpr }
EqExpr    ::= RelExpr  { ("=="|"!=") RelExpr }
RelExpr   ::= XorExpr  { ("<"|"<="|">"|">=") XorExpr }
XorExpr   ::= AddExpr  { "^"  AddExpr }
AddExpr   ::= MulExpr  { ("+"|"-") MulExpr }
MulExpr   ::= UnaryExpr{ ("*"|"/"|"%") UnaryExpr }
UnaryExpr ::= ("!"|"-") UnaryExpr | PostfixExpr
PostfixExpr::= Primary { Call | Indexer | Slice | Field | PathSeg }
Call      ::= "(" [ Expr {"," Expr} ] ")"   ;   Slice ::= "[" Expr ("..'|"..=") Expr "]"
PathSeg   ::= "::" Ident

Primary   ::= Literal | SysCall | Ident | "(" Expr ")" | Block | IfExpr | ArrayExpr | BundleLit | Lambda
IfExpr    ::= "if" "(" Expr ")" Block "else" Block
ArrayExpr ::= "[" ( Expr ";" Expr | Expr "|" Ident "in" Range | Expr {"," Expr} ) "]"
BundleLit ::= TypeRef "{" [ FieldInit {"," FieldInit} [","] ] "}"
FieldInit ::= "." Ident "=" Expr
Lambda    ::= "|" [ Ident {"," Ident} ] "|" Expr
```

`BundleLit` (needs `{` after `TypeRef`) vs a block collides in statement position; a
statement-leading bundle literal is parenthesized or appears in value position (after
`=`/`<-`/`return`). Operator precedence is a starting point, tunable.

Native canonical spellings only: `|` (not `or`) for event OR, one print/diagnostic per severity.
Verilog-AMS aliases (`or`, `log`, `$warning`, `$stop`, `$strobe`/`$monitor`) are accepted only in
the AMS ingestion front end.

---

# Part IV — Reflection API (Object Model)

*Piperine Reflection API — The Piperine Object Model (POM)*

POM exposes an elaborated design as a graph of typed runtime nodes — the single surface for
Piperine `bench`, Python, Rust, and the plugin ABI. The `ElabProgram`/`IrProgram` behind it is
never exposed. POM is **read-first**: querying has no effect; assigning a parameter **stages an
override** consumed by a later pure elaboration (§3). Companion docs (selector, extensibility)
build on it.

### 1. Core concepts

**Distinct typed nodes.** Each construct is its own node type with its own interface — `Module`,
`Instance`, `Net`, `Port`, `Param`, `Attribute`, `Behavior`; definition nodes `Bundle`, `Enum`,
`Discipline`, `Capability`; leaf nodes `Field`, `Variant`, `Nature`, `Signature`. Distinct types
give precise, type-safe interfaces per host; one wire protocol underneath (§7) gives one ABI and
one selector.

**Two-axis model.** *Attributes* are scalar properties returning a `Value`; a few are settable,
the rest read-only. *Relations* are named axes to other nodes, always returning a `Selection`.
(This is what makes the selector's axes = relations, predicates = attributes.)

**Identity.** `Node.id() -> Id`, stable across re-elaboration while the source is unchanged.
`Net` also exposes `NetId` — the anchor for extraction/LVS across the closure loop.

**Value** = the value layer: primitives `Real Natural Integer Boolean Quad String`; a node
reference; or a value-layer collection. (`Complex` is a `Bundle`, not a primitive.)

**Selection\<T\>** — ordered, duplicate-free set; the universal navigation result:

```
len, is_empty ; get(i)->Option<T> ; first/last->Option<T> ; iter->Vec<T>
filter(pred)->Selection<T> ; map(f)->Vec<U> ; where(path)->Selection<Node> ; one()->Result<T,_>
```

`sel[i]` = `get(i)` unwrapped. Settable attributes write across a whole selection (§3); `attach`
(annotation) and `meta` (attribute overlay) are in the extensibility spec.

**Value-layer collections** (value layer only, never net-capable hardware): `Vec Map Set Option
Result`. A value-only bundle may hold them; a net-capable bundle may not.

### 2. Design root

```
Design
  top()->Module ; module(name)->Option<Module> ; modules()->Selection<Module>
  select(path)->Selection<Node>
  const_(name)->Option<Value> ; consts()->Map<String,Value>
  bundles()/enums()/disciplines()/capabilities()->Selection<...>
```

In `bench Module { ... }` the root is the bench's module; `select` is rooted there.

### 3. Staging and determinism

`Param.set(v)->Result<Unit,ReflectError>`, or sugar `sel.r = 0.2e-2` / `sel.set("w", 2.0)`.
Writing a parameter **stages an override**; it does not edit the design. The next
`simulate`/`elaborate` re-elaborates purely from staged overrides (structural param →
re-elaborate; non-structural → netlist patch; the engine decides). No in-place mutation; writing
a read-only attribute is a `ReflectError`.

### 4. Elaborated-structure nodes

**Module** — `name ; is_generic ; ports()/params()/nets()/instances()/behaviors()->Selection ;
attributes()->Selection<Attribute> ; port(n)/net(n)/instance(n)/param(n)->Option ;
attribute(schema)->Option<Attribute> ; select(path)`. `instances()` is direct children; `nets()`
= port nets + internal wires.

**Instance** — `name ; path->Path ; of()->Module ; ports()/params()/children()->Selection ;
port(n)/param(n)->Option ; net(port)->Option<Net>` (the `dac.out` access). `param.set` /
`inst.r = ...` stages an override scoped to the instance path.

**Net** — `id->NetId ; name ; discipline()->Discipline ; resolution()->Resolution ;
width()->Natural ; line(i)->Option<Net> ; is_port ; drivers()/loads()/connected()->Selection<Port>`.
A bus is one Net of `width N`; `line(i)` is a line.

**Port** — `name ; direction()->Direction ; net_type()->Type ; net()->Net ; owner()->Node`.

**Param** — `name ; type()->Type ; value()->Value ; is_overridden ; set(v) ; owner()->Node`.

**Attribute** — plugin metadata from `@schema(...)` on any declaration (§8 of the language spec):
`schema()->String ; data()->Value ; field(name)->Option<Value> ; owner()->Node`. Inline and
selector-overlay attributes appear identically here (overlay wins on conflict).

**Behavior** — `domain()->Domain (Analog|Digital) ; owner()->Module`. (Body reflection is a later
refinement.)

### 5. Definition nodes

**Bundle** — `name ; is_net_capable ; type_params()/const_params()->Vec<String> ;
fields()->Selection<Field> ; capabilities()->Selection<Capability>`.
**Field** — `name ; type()->Type ; default()->Option<Value>`.

**Enum** — `name ; repr()->Type ; variants()->Selection<Variant>`. **Variant** — `name ;
value()->Value`.

**Discipline** — `name ; kind()->DisciplineKind (Conservative|Storage) ; storage()->Option<Type>
; resolution()->Resolution ; natures()->Selection<Nature>` (conservative).
**Nature** — `name ; kind()->NatureKind (Potential|Flow) ; value_type()->Type ; unit()->Option ;
abstol()->Option<Real>`.

**Capability** — `name ; supers()->Selection<Capability> ; signatures()->Selection<Signature> ;
implementors()->Selection<Node>`. **Signature** — `name ; params()->Vec<Type> ; returns()->Type ;
has_default->Boolean`.

### 6. Leaf enums

```
Direction     ::= Input | Output | Inout
Domain        ::= Analog | Digital
DisciplineKind::= Conservative | Storage
Resolution    ::= Single | Tri | Or | And | Sum | Avg | Max | Min | Kcl
NatureKind    ::= Potential | Flow
ReflectError  ::= NotFound | NotSettable | TypeMismatch | OutOfRange | UnknownSchema
```

`Node` is the supertype; `Node.kind()` discriminates it (how an untyped host recovers the type).

### 7. One model, three languages, one ABI

Uniform underneath, idiomatic on top. Wire protocol = a serialized node carrying `kind`, `id`,
scalar attributes, relation axes; every host rebuilds the same typed objects from one ABI.
Piperine (`bench`) built-in nodes + assignment sugar; Python classes with `__getattr__`/
`__setattr__`; Rust structs/traits with explicit `inst.param("r")?.set(...)?`. The ABI *is* this
API (serialized-node protocol + per-type method tables). Compiled plugins (Rust/Python) load like
OSDI compact models — a shared library exposing a descriptor.

### 8. Out of scope

Selector spec (the query language; POM relations are its axes, attributes its predicates).
Extensibility spec (`@` schema registration, plugin verbs, annotations `Selection.attach`,
attribute overlay `Selection.meta`, the `bench` toolchain). These may still reshape POM.

---

# Part V — Selector

*Piperine Selector — Query Language for the Object Model*

The selector ("XPath of the circuit") evaluates against a design and returns `Selection<Node>`.
It is the one addressing mechanism for reflection, overrides, and annotation. It adds no model:
**axes are POM relations, predicates are POM attributes.**

### 1. Model

Evaluated against a **context node** (`design.select`, `module.select`, or `selection.where`),
producing an ordered, duplicate-free `Selection<Node>`. A path is **steps**; each step moves
along an **axis**, keeps nodes matching a **node test**, filters through **predicates**. Results
union across context nodes, dedup by identity, first-seen order.

### 2. Grammar (EBNF)

```
Selector  ::= [ "/" | "//" ] Step { ( "/" | "//" ) Step }
Step      ::= [ Axis "::" ] NodeTest { Predicate }
Axis      ::= "inst"|"net"|"port"|"param"|"attr"|"behavior"|"driver"|"load"|"parent"|"ancestor"
NodeTest  ::= Name | "*"
Predicate ::= "[" ( Index | PredExpr ) "]"
Index     ::= NatLit | "last" "(" ")"
PredExpr  ::= OrExpr
OrExpr    ::= AndExpr { "or" AndExpr }
AndExpr   ::= NotExpr { "and" NotExpr }
NotExpr   ::= "not" "(" PredExpr ")" | Compare
Compare   ::= Operand [ CmpOp Operand ]
CmpOp     ::= "=="|"!="|"<"|"<="|">"|">="|"~"          -- '~' = glob
Operand   ::= AttrRef | AxisRef | Func | Literal
AttrRef   ::= "@" Name [ "." Name ]                     -- @direction ; @layout.min_width
AxisRef   ::= Axis "::" NodeTest
Func      ::= "of" "(" StringLit ")" | "count" "(" AxisRef ")"
Literal   ::= NumberLit | StringLit | BoolLit | Ident
```

`/` absolute from context; `//` descendant closure over `inst::`.

### 3. Axes

| Axis | POM relation |
|------|--------------|
| `inst::` *(default)* | `instances()`/`children()` |
| `net:: port:: param:: attr:: behavior::` | `nets/ports/params/attributes/behaviors()` |
| `driver:: load::` | net `drivers()/loads()` |
| `parent:: ancestor::` | reverse / transitive |

`//X` = `X` at any instance depth; a step after `//` may switch axis (`//*/net::clk`).

### 4. Node tests

`*` = any. A name matches by node name; on `inst::`, **PascalCase** matches by module type
(`of()`, source name — `Dac` matches `Dac__8`), **snake_case** by instance name. An instance
array `leg[N]` shares base name `leg` (matches all replicas; index predicate picks one).

### 5. Predicates

Positional `[i]` (0-based) / `[last()]`. Attribute `@name`/`@schema.field` compared
(`[@direction == Input]`, `[@width > 1]`, `[@layout.min_width > 1u]`, `[@name ~ "cmp*"]`);
enum/node names are bare identifiers, strings quoted. A bare `axis::test` is existence
(`[attr::layout]`, `[net::clk]`); compared, it tests the matched value (`[param::r > 1k]`).
Boolean `and`/`or`/`not(...)`; `and` binds tighter; sequential predicates are conjoined
left-to-right.

### 6. Evaluation

Per step over node-set S: for each n, follow the axis, keep node-test matches, apply predicates
left-to-right (boolean filters; positional keeps the ordinal), union+dedup by identity. Empty
result is valid (not an error; use `is_empty()`/`one()`). Pure function of the elaborated design
+ staged overrides → deterministic.

### 7. Integration

```piperine
for r in select("//Resistor") { $info("{}", r.param("r").value()); }
select("//dac/param::vref").set(1.8);
select("//leg").set("w", 2.0);
var big = select("//Resistor").where("[param::r > 1k]");
select("//*/net::*[@layout.layer == \"m3\"]").attach( Capacitor { .c = 4.2f } );
```

Adding a POM node type or attribute extends what the selector addresses, with no grammar change.

### 8. Open questions

Reverse axes (`parent::`/`ancestor::`) ship v1 or later. Param shorthand `[r > 1k]` (bare-name
default `param::`) left explicit (no-magic). Terminal value form (`@attr` yielding a value vs a
node) open. Array-replica index alignment after monomorphization to be pinned against the
elaborator.

---

# Part VI — Builtins Reference

*PHDL Builtins Reference*

The exhaustive, implementation-grounded catalog of what a source file may call without declaring
it: math functions, analog operators, `$`-syscalls, diagnostic tasks, `@`-events, and the
always-in-scope prelude/stdlib. This reference is **normative** for the open builtin set;
operators/syscalls/events are extensible via the layer-2 registries (extension model §13).

**Alias policy.** The native canonical spelling is one per meaning (`|` for OR, `ln`, `$info`/
`$warn`/`$error`/`$fatal`, `$finish`, one print). Verilog-AMS aliases (`or`, `log`, `$warning`,
`$stop`, `$strobe`/`$monitor`) are accepted **only** in the AMS ingestion front end, not in
native PHDL.

### 1. Math functions

Expression-position calls on `Real`; symbolically differentiated for the Jacobian.

| Fn | Arity | Computes | d/dx (arg 0, `u'`) |
|---|---|---|---|
| `exp` | 1 | eˣ | `exp(u)·u'` |
| `ln` | 1 | natural log | `u'/u` |
| `log10` | 1 | base-10 log | `u'/(u·ln10)` |
| `sqrt` | 1 | √ | `u'/(2√u)` |
| `abs` | 1 | \|x\| | `sign(u)·u'` |
| `sin`/`cos`/`tan` | 1 | trig | `cos(u)·u'` / `-sin(u)·u'` / `u'/cos²(u)` |
| `asin`/`acos`/`atan` | 1 | inverse trig | `u'/√(1-u²)` / `-u'/√(1-u²)` / `u'/(1+u²)` |
| `atan2` | 2 | 2-arg atan | 0 |
| `pow` | 2 | aᵇ | `b·pow(a,b-1)·a'` (b const) |
| `min`/`max` | 2 | min/max | 0 |
| `floor`/`ceil` | 1 | round | 0 |
| `sinh`/`cosh`/`tanh` | 1 | hyperbolic | `cosh(u)·u'` / `sinh(u)·u'` / `(1-tanh²(u))·u'` |
| `limexp` | 1 | `exp(min(x,80))` (SPICE overflow clamp) | `exp(u)·u'` |

(`log` = alias of `ln`, AMS-only.)

### 2. Analog operators

`analog`-body only. Most allocate a state var and lower to a companion model in the device
compiler.

| Operator | Args (defaults) | Semantics |
|---|---|---|
| `ddt(x)` | | time derivative; reactive stamp `alpha=1/dt`. Fully device-compiled. |
| `idt(x, ic=0)` | | time integral. |
| `idtmod(x, ic=0, modulus=1)` | | integral wrapped mod `modulus` (phase accumulators). |
| `ddx(x, node)` | | partial wrt a node's potential/flow. |
| `delay`/`absdelay(x, dt=0)` | | ideal time delay. |
| `transition(x, td=0, tr=0, tf=0, tol=0)` | | smooth a digital-like signal to continuous. |
| `slew(x, rise=0, fall=0)` | | slew-rate limit. |
| `table(x, xs, ys, mode)` | value, breakpoints, data, interp mode | measured-data lookup + interpolation (1-D; N-D later). |
| `laplace_np/zp/pm/nm/npm(x, num, den)` | | continuous filter H(s)=num/den; suffix = coeff convention. |
| `zi_zd/zp/nd/np(x, num, den, sample_dt)` | | discrete (Z) filter, sampled. |
| `ac_stim(mag=1, phase=0)` | | AC small-signal stimulus (.ac only); lowers to `AcStim`. |
| `white_noise(psd, "label")` | | flat noise source; extracted from a `<+` RHS pre-lowering. |
| `flicker_noise(psd, exp=1, "label")` | | 1/fᵉˣᵖ noise source. |

Device-fidelity today: `ddt` (charge/companion model), `idt`/`idtmod` (implicit-Euler runtime
integrator; DC value = initial condition; AC small-signal admittance not yet stamped), `ddx`
(symbolic), `delay`/`slew` (runtime-serviced) are device-compiled. `transition`/`table`/
`laplace_*`/`zi_*` are recognized in IR but rejected fail-loud at device-compile pending
companion models.

### 3. `$`-syscalls (expression)

| Syntax | Returns |
|---|---|
| `$temperature` | temperature (K) |
| `$vt` / `$vt(temp)` | thermal voltage kT/q |
| `$abstime` | absolute sim time |
| `$mfactor` | instance multiplicity |
| `$xposition`/`$yposition`/`$angle` | layout placement/rotation |
| `$simparam("key", default=0)` | named simulator parameter |
| `$param_given("name")` | was param explicitly passed (frontend/IR only) |
| `$port_connected("name")` | is port externally connected |
| `$limit(x, "kind", ...)` | Newton convergence limiter (`pnjlim`, `fetlim`, …) |
| `$analysis("kind")` | current analysis matches (`dc`/`tran`/`ac`/`noise`) |
| `$random`/`$random(seed)` | uniform PRN |
| `$dist_uniform/normal/exponential(...)` | distribution PRN (same handler, `kind` threaded) |

### 4. Diagnostic / control tasks (statement)

| Syntax | Effect |
|---|---|
| `$bound_step(dt)` | cap next timestep |
| `$finish` | terminate the simulation (`$stop` = AMS alias) |
| `$discontinuity(n=0)` | flag order-n discontinuity; break/re-solve step |
| `$info`/`$warn`/`$error`/`$fatal(fmt, args...)` | log at severity; `{}` interpolates args |
| `$display`/`$write` | print at Info (`$strobe`/`$monitor` = AMS aliases) |

`$fatal` does not auto-`$finish`.

### 5. `@`-events

`analog` rejects digital edges; `digital` rejects analog crossings. **An unrecognized event name
is a compile error** (no silent fallback).

| Form | Class | Fires |
|---|---|---|
| `posedge`/`negedge`/`change(sig)` | digital | edge / any change |
| `cross(expr)` | analog | zero crossing (direction arg parsed; currently either-direction) |
| `above(expr)` | analog | one-shot level crossing |
| `initial`/`final` | both | once at start / end |
| `timer(period)` | analog | periodic (digital `timer` is rejected — the digital kernel has no time-driven events yet) |
| `A | B` | | composite OR (recurses validation) |

Analog event bodies execute at runtime as persistent-variable updates, detected at each
accepted solution (`initial` fires once at instance creation; `final` admits diagnostics
only). `@ above`/`@ cross` updating module state is the ngspice switch idiom and is
device-compiled.

### 6. Prelude / stdlib (`headers/*.phdl`)

Injected into every unit (except `constants`/`disciplines`, which need explicit `use`).

**Disciplines:** `Ground` (reference). Storage-digital: `Bit` (`storage Boolean`), `Logic`
(`storage Quad; resolve tri`), `DDiscrete` (`storage Quad`). Conservative: `Electrical` (v,i),
`Magnetic` (mmf, phi), `Thermal` (temp, pwr), `Kinematic` (pos, f), `KinematicV` (vel, f),
`Rotational` (theta, tau), `RotationalOmega` (omega, tau). Storage-`Real`: `Voltage` (v),
`Current` (i). *(Voltage/Current were signal-flow; now `storage Real`, read by name.)*

**Constants:** math `M_E M_LOG2E M_LOG10E M_LN2 M_LN10 M_PI M_TWO_PI M_PI_2 M_PI_4 M_1_PI M_2_PI
M_2_SQRTPI M_SQRT2 M_SQRT1_2`; physical `P_Q P_C P_K P_H P_EPS0 P_U0 P_CELSIUS0`.

**Capabilities:** `Type`, `Net` (root markers); `Add Sub Mul Div`, `Eq`, `Ord : Eq`, `BitAnd
BitOr BitXor Not`, `Number : Add,Sub,Mul` (default `double`).

**Collections & numeric types:** `map<T,U>(xs: T[N], f) -> U[N]`, `reduce<T>(xs: T[N], op) -> T`,
`concat(...)`; the bundles `UInt[N]`, `SInt[N]`, `Complex`.