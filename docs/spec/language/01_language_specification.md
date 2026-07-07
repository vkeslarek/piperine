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

**Collections and tuples** (interpreted `fn`-body grammar — `bench` and const-eval, §9; not yet
lowered for `analog`/`digital`):

- `(a, b, ...)` — a tuple literal; `.0`/`.1`/... index. `(e)` with no comma is a parenthesized
  group, not a 1-tuple.
- `[a, b, ...]` / `[v; N]` / `[expr | i in a..b]` — a runtime list (`Vec<T>`), value-layer, with
  `.push(v)`, `.len()`, `.get(i) -> Option<T>`. The same array-literal syntax also produces a
  fixed-size elaboration-constant `Array` in a `mod`/`analog`/`digital` context (§7) — which form
  applies follows from context, as with every other dual-position construct in this grammar.
- `Option<T>` — `.is_some()`, `.is_none()`, `.unwrap()`, `.unwrap_or(default)`.
- **Optional types `T?`** — a trailing `?` marks a value that may be absent; `none` is the
  absent value and inhabits any `T?`. Read through `.is_present()` / `.get_or(default)`
  (aliases of `.is_some()` / `.unwrap_or()`). The intended use is optional **parameters**:
  `param rmodel : Real? = none;` then `rmodel.get_or(rfixed)` in the body. On a scalar param
  this lowers onto parameter-presence (`is_present` ≡ `$param_given`, `get_or(d)` ≡
  `param_given ? p : d`), so the choice is per-instance — supplying `.rmodel = 500.0` at an
  instance makes it present. Prefer `T?` over a sentinel default (`1e99`, `0`) + `$param_given`.

- `Map<K, V>` — an association literal `Map { key: value, … }` (`Map {}` is empty), value-layer,
  with `.insert(k, v)`, `.get(k) -> Option<V>`, `.len()`; structural equality. Keys compare by
  value (small-N association list, not a hash table). Backs the `ic:`/`nodeset:` fields of the
  analysis config bundles (bench spec §5.1, `crates/piperine-bench/docs/SPEC.md`).

`Set<T>` and `Result<T, E>` are reserved (named in the reflection API, Part IV) but have no
literal syntax or value-layer operations yet — using one outside Part IV's own read-only
accessors is `NotConst`/`Undefined`, not a silent stub.

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

This `fn`-body grammar (`var`, `if`/`else`, `match`, `for`, `return`, expressions, lambdas) is also
what a bundle `impl` method and a `bench` block (bench spec, `crates/piperine-bench/docs/SPEC.md`)
are written in — same statements and expressions everywhere, interpreted rather than
inlined-and-differentiated in the effectful `bench` context (bench spec §1). Two differences from
the elaboration/analog/digital
positions of this same grammar: `for x in <expr>` may iterate a runtime `Vec` value, not just an
elaboration-constant range (§6.1); and `var name = expr;` may omit its type, inferred from `expr`
at interpretation time — both are only valid where the body is interpreted (`bench`), not
statically elaborated (an `impl`/global `fn` still requires `var name : Type = expr;`).

#### 9.1 Default parameter values

A `fn`/method parameter may carry a default: `fn v(self, a: Net, b: Net = gnd) -> Real`.
**Trailing** parameters only — a defaulted parameter followed by a non-defaulted one is a parse
error. A call may omit trailing defaulted arguments (`r.v(a)` ≡ `r.v(a, gnd)`); arity checking
counts only the non-defaulted prefix. Defaults are elaboration constants, evaluated in the
callee's scope against the already-bound earlier parameters.

This applies uniformly — bundle `impl` methods, global `fn`s, bench helpers, and analog `fn`s
used in contributions — honored by both the interpreter and the IR inliner (defaults are
const-folded at call-site lowering). It replaces overloading-by-arity (which PHDL does not have)
and makes optional config (`op(cfg: OpConfig = OpConfig {})`) expressible.

#### 9.2 Higher-order functions and generation

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

