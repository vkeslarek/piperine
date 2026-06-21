# Piperine — Design Document (v1.6)

**Piperine is Verilog-A in full (as accepted by OpenVAF), plus a curated set of
SystemVerilog concepts borrowed without a wholesale syntax change, run by a Rust
interpreter over a pooled ngspice backend.** Device models stay pure Verilog-A and
compile through OpenVAF to OSDI; testbenches and automation use familiar
Verilog/SystemVerilog syntax and run in the interpreter. No new surface syntax is
invented — the value is the unified toolchain and the extension mechanism, not a
reinvented language.

This document consolidates the full design discussion. It supersedes the
exploratory `piperine-sketch.md` (fresh Go/Rust-flavored surface) and
`piperine-rust-sketch.md` (proc-macro eDSL), both of which were considered and
set aside (§1).

---

## 1. Decision and rationale

**What Piperine is:** a thin superset — Verilog-A for the analog core, a handful
of SystemVerilog constructs for the testbench/verification layer, and a Rust
runtime that drives ngspice with OpenVAF-compiled OSDI device models.

**Paths considered and set aside:**

- *A fresh modern surface (Go/Rust-flavored).* Clean, but it forces a complete
  syntax change, throws away direct Verilog-A compatibility, and means designing
  and tooling a whole new language. The simplicity win didn't justify cutting the
  cord from the existing model ecosystem.
- *An embedded DSL / proc-macros in Rust.* Tempting for free tooling, but the
  analog math reads badly inside Rust macros, and embedded HDLs hit the wall where
  the target language constrains expressiveness. Square peg, round hole.

**Why this landing wins:** maximal backward compatibility (it *is* Verilog-A, so
every existing compact model and OpenVAF just work), minimal new syntax to learn
or maintain, and the genuinely novel parts — the extension mechanism and the live
mixed-signal runtime — sit on top without disturbing the familiar base.

**The non-negotiable fact that shapes everything:** OpenVAF compiles only the
**analog (Verilog-A) subset** of Verilog-AMS — it cannot parse digital
`initial`/`always`, and OSDI has no representation for them. So the analog device
models go to OpenVAF, and the procedural testbench layer is the interpreter's job.
That split is not a design choice; it falls out of what the backend accepts.

---

## 2. Language model

### 2.1 Base — Verilog-A, unchanged

Device models are written in standard Verilog-A and compiled by OpenVAF to OSDI.
Nothing here is modified; existing models paste in verbatim.

```verilog
module resistor(p, n);
  inout p, n;
  electrical p, n;
  parameter real r = 1e3 from (0:inf);
  analog
    I(p, n) <+ V(p, n) / r;
endmodule
```

The analog body lives within OpenVAF's supported subset (no module-internal
arrays, no `genvar`/generate, analog events limited to `@(initial_step)` /
`@(final_step)`). Behavioral/large-signal sources that fall outside it are
expressed as `extern` modules backed by ngspice B-sources / XSPICE, or in the
procedural layer.

### 2.2 What we take from SystemVerilog — and the strategy

**Principle: make the procedural language inside `initial` faithful to
SystemVerilog first, then grow the library and feature set on a system that
already runs.** A small, correct imperative core is worth more right now than a
broad-but-shaky surface; because the core is faithful SV, every later feature is
added without reworking what already works.

**Supported now — the procedural core** (full syntax in §4):

- Built-in types: `bit`, `logic`, `int`, `integer`, `real`, `time`, `string`, and
  packed vectors `[msb:lsb]`.
- Arrays: fixed unpacked arrays, dynamic arrays `[]`, and queues `[$]` with their
  built-in methods — the workhorses for sweeps and statistics.
- The full operator set, sized/based literals, concatenation/replication, ternary.
- Control flow: `if`/`else`, `case`/`casez`/`casex`, `for`, `foreach`, `while`,
  `do…while`, `repeat`, `forever`, `break`/`continue`/`return`.
- Subroutines: `function`/`task` with `input`/`output`/`inout`/`ref` args.
- System tasks/functions: the `$display` family, `$finish`, severities
  (`$fatal`/`$error`/`$warning`/`$info`), `$time`, math functions — plus
  Piperine's `extern` control/access tasks (`$tran`, `$set_param`, `$V`, …).
- Immediate assertions: `assert (expr) else …` (the SOA mechanism, §4.8).

**Deferred — grows later, on a working base:**

- `class` and OOP. Encapsulation is nice but not needed to get running.
- User-defined types: `typedef`, `struct`, `union`, `enum`.
- Constrained randomization: `rand`/`randc`/`constraint`/`randomize()`.
- Concurrency (`fork`/`join`, events), concurrent assertions (SVA), interfaces,
  clocking blocks, coverage, UVM.

Each deferred item lands as a library/feature increment; the testbench language
itself stays stable because the imperative core does not change.

### 2.3 Piperine's own additions (minimal)

- **`extern` declarations with AST-passthrough dispatch** — the extensibility seam
  and the simulator-control surface (§3). Uses ordinary Verilog `extern` syntax.
- **The runtime** — an interpreter that elaborates a circuit once and runs the
  procedural layer against a live ngspice/OSDI session, reusing the loaded circuit
  across analyses (§6).

### 2.4 The compile-vs-interpret split

A module is routed by content:

| Module content                         | Treatment                                  |
|-----------------------------------------|--------------------------------------------|
| `analog` block (device model)           | compiled by **OpenVAF → OSDI**             |
| structural (instances/wires only)       | flattened to a **SPICE netlist**           |
| `initial`/`always` (testbench)          | run by the **interpreter** (never OpenVAF) |

OpenVAF only ever sees pure-analog modules. This is the whole architecture.

---

## 3. The `extern` mechanism (AST-passthrough)

`extern` binds a language symbol to a backend-provided implementation. Three
kinds, all in ordinary Verilog syntax: `extern module`, `extern task`,
`extern function`.

```verilog
// ngspice primitives surfaced as modules
extern module spice_res    (inout p, inout n, parameter real r);
extern module spice_vsource (inout p, inout n, parameter string kind, parameter real val);

// simulator controls and helpers, as system tasks/functions
extern task $tran      (real tstep, real tstop);
extern task $pre_osdi  (string library);
extern task $set_param (string target, real value);
extern function real $rms (string node);
```

**The key rule:** a call to an `extern` symbol does **not** evaluate its arguments
— it hands the argument AST to the bound handler, which interprets it. This single
rule removes the need for special syntax: node access (`$V`/`$I`), measurements,
and controls are all ordinary system-task calls whose plugin walks the AST it
receives. A plugin extends the language by shipping a header (`extern`
declarations) that an `include` pulls into the symbol table; bindings resolve
dynamically against the backend registry. No compiler change, no new syntax.

---

## 4. The testbench layer — supported SystemVerilog subset

A testbench is a module with an `initial` block; its elaborated circuit is the
context, and analyses reuse it (no per-call reload). The procedural language below
is the **faithful SystemVerilog core supported now** — the syntax is exactly SV's.

### 4.1 Types and literals

```verilog
bit  logic                      // 2-state / 4-state scalars
logic [7:0] bus;                // packed vector
int  integer  longint  byte     // integers
real  realtime  time            // floating / time
string s;                       // string

42   8'hFF   4'b1010   1'bx      // integer literals (sized / based)
3.14   1.5e-3                    // real literals
"hello"                         // string literal
```

### 4.2 Operators and expressions

```
arithmetic   + - * / % **
relational   <  <=  >  >=
equality     ==  !=  ===  !==
logical      &&  ||  !
bitwise      &  |  ^  ~  ~^      (and reductions over vectors)
shift        <<  >>  <<<  >>>
concat/rep   {a, b}    {n{a}}
ternary      cond ? a : b
assignment   =  +=  -=  *=  /=  …      (blocking, procedural)
```

### 4.3 Control flow

```verilog
if (c) … else …
case (x) 0: …; 1: …; default: …; endcase      // casez / casex too
for (int i = 0; i < n; i++) …
foreach (arr[i]) …
while (c) …
do … while (c);
repeat (n) …
forever …                                       // used with care under the interpreter
break;  continue;  return expr;
```

### 4.4 Subroutines

```verilog
function real db20(real x);          return 20.0 * $log10(x); endfunction
function void note(string m);        $display(m);             endfunction
task   run(input real lo, input real hi, output real worst);  …  endtask
// argument directions: input / output / inout / ref
```

### 4.5 System tasks and functions

```
output    $display  $write  $strobe
control   $finish   $fatal  $error  $warning  $info
time      $time     $realtime
math      $sqrt $pow $exp $ln $log10 $sin $cos $floor $ceil $abs …
rng       $random   $urandom            (basic; full randomize() deferred)

// Piperine extern control/access tasks — v1 mirrors ngspice's command + vector
// vocabulary 1:1 (backend-dispatched, same syntax):
$pre_osdi  $tran  $ac  $dc  $op  $noise  $alter  $set_param  $get
$V  $I   $rms  $vecmax  $vecmin  $integ      // ngspice vector/measure functions
```

The v1 control surface is a thin, faithful mirror of ngspice: analyses
(`$tran`/`$ac`/`$dc`/`$op`/…), parameter mutation (`$alter`/`$set_param`), and
data access via ngspice's own vector/measure functions. Richer result handling —
object-oriented result types, refined data analysis, and multi-analysis
composition — is deferred (§5.4, §10).

### 4.6 Arrays and queues

```verilog
real fixed[16];                 // fixed unpacked array
real dyn[];   dyn = new[n];     // dynamic array
real q[$];                      // queue
q.push_back(x);  q.pop_front();  q.size();
q.min();  q.max();  q.sum();     // built-in reductions
```

### 4.7 Worked example (core only)

```verilog
module tb;
  spice_res     #(.r(1e3))               r1(vout, gnd);
  spice_vsource #(.kind("dc"), .val(5))  v1(vout, gnd);
  my_diode                                d1(vout, gnd);  // OpenVAF-compiled OSDI

  real samples[$];

  initial begin
    $pre_osdi("my_diode.osdi");
    for (real rr = 1e3; rr <= 10e3; rr = rr + 1e3) begin
      $set_param("r1.r", rr);
      $tran(1n, 5m);                                       // reuses the loaded circuit
      samples.push_back($rms("vout"));
    end
    $display("worst-case rms = %g over %0d points", samples.max(), samples.size());
  end
endmodule
```

### 4.8 Safe-operating-area monitoring

SOA lives in the **testbench**, where the interpreter sees every accepted
timepoint — so an `always`-style monitor is its natural home, and it is **not**
limited by OpenVAF (which, inside a compiled device model, supports only
`@(initial_step)`/`@(final_step)`). Because the testbench is interpreted, the full
analog-event vocabulary is available. Two forms, both standard-flavored:

Per accepted timepoint (`step` = a Piperine event meaning each accepted analog
timepoint):

```verilog
always @(step) begin
  assert (V("d","s") <= 35.0) else $error("Vds overvoltage");
end
```

Event-based (standard Verilog-AMS analog events — fires only on the crossing, so
cheaper than every-point):

```verilog
always @(above(V("d","s") - 25.0)) $warning("approaching Vds breakdown");
always @(above(V("d","s") - 35.0)) $error("Vds overvoltage");
```

Both are host-side observers on the per-timepoint data stream (`$error` halts the
run; `$warning` records and continues) — not solver constraints (P5). The
analog-event forms (`above`/`cross`/`timer`) work here precisely because the
interpreter evaluates them, even though OpenVAF would reject them inside a compiled
model. No bespoke `soa` keyword is needed.

---

## 5. Devices, components, models, measures, behavioral sources

This is the layer where the Verilog-A/OSDI wrapping actually meets ngspice. The
one idea to internalize first:

### 5.0 The two-tier SPICE structure (instance vs model)

SPICE separates a **component (instance)** from a **model**:

- A **device type** is the physics/behavior — a Verilog-A `module` (compiled to
  OSDI) or an ngspice built-in.
- A **model** is a *named, shared set of model-level parameters* for a device type
  (the `.model` card) — the process/technology parameters.
- A **component (instance)** is one device in the netlist that references a model
  (when the type has one) and sets *instance-level* parameters (geometry, value)
  plus node connections.

Verilog-A/OSDI honors this split: a parameter is model-level by default and
instance-level when tagged (`(* type="instance" *)` in Verilog-A, or an `instance`
qualifier in Piperine). Simple devices (R, C, L, V, I) have no model — just
instance parameters. Everything below follows from this split.

### 5.1 Device type — a Verilog-A module

Pure Verilog-A, compiled by OpenVAF; valid `openvaf` input directly:

```verilog
module diode(a, c);
  inout a, c;  electrical a, c;
  parameter real is = 1e-14;
  parameter real n  = 1.0;
  analog
    I(a, c) <+ is * (exp(V(a, c) / (n * $vt)) - 1);
endmodule
```

### 5.2 Components — defaults, mandatory params, partial override

Parameters declared with a default are optional; a parameter with **no default is
mandatory** (must be overridden). Instantiation overrides only what you name
(SystemVerilog named-override syntax), the rest keep defaults:

```verilog
// in the device/extern declaration:
//   parameter real r = 1k;     // optional, default 1k
//   parameter real r;          // mandatory — no default
resistor #(.r(2k)) R1 (.p(a), .n(b));     // override only r; named ports
resistor           R3 (.p(c), .n(d));     // r mandatory → error if undeclared default
```

For **ngspice built-in devices** (surfaced as `extern module`), you do **not**
enumerate the hundreds of model parameters. The header declares the ports and the
mandatory/common params; any other named parameter **passes through** to ngspice
as AST, and ngspice supplies its own defaults and validates. This is how "100% of
ngspice" stays tractable — the long tail of parameters is forwarded, not mirrored.

### 5.3 Models — `paramset` (Verilog-AMS standard), inline

**Not our invention.** Verilog-AMS provides `paramset` (LRM 2.4.0 §Paramsets,
Annex A.1.9) — a named parameter set bound to a base module, with
overloading/binning. It is the standard analog of a SPICE `.model`. Plain
Verilog/SystemVerilog have nothing like it; Verilog-AMS (our analog base) does.

It is declared **inline** — same source, no separate file:

```verilog
paramset nmos18 mosfet_va;          // model `nmos18` over base module `mosfet_va`
  parameter real L = 0.18u;         // params the paramset exposes to instances
  parameter real W = 1u;
  .vth0 = 0.4;                      // bind base-module (model-level) params
  .tox  = 4n;
endparamset

nmos18 #(.L(0.18u), .W(1u)) M1 (.d(d), .g(g), .s(s), .b(b));   // instance uses it
```

Paramset overloading (several `paramset`s sharing a name, selected by which
parameters/ranges match) gives **binning** for free, and the paramset is exactly
where the instance-vs-model parameter split is expressed (the declared params are
instance-facing; the `.x = …` bindings set model-level params).

**OpenVAF does not yet compile `paramset` — and does not need to.** The base
device module goes to OpenVAF (→ OSDI); the `paramset` is resolved by Piperine's
elaboration into ngspice model/instance cards. The standard syntax works today
regardless of OpenVAF's roadmap.

For a one-off device you can still inline all params at the instance (Piperine
synthesizes an anonymous binding); `paramset` is for sharing and binning. A lighter
`model name = base #(…)` sugar can be offered that lowers to a `paramset`.
Consuming a foundry `.lib` remains the optional file-based interop path.

### 5.4 Measures — ngspice-mirroring now, object-oriented later

**v1 mirrors ngspice.** Measurements use ngspice's own vector/measure functions,
surfaced as system functions over a named result vector — simple and faithful:

```verilog
$tran(1n, 5m);
real r  = $rms("vout");          // ngspice vector function
real mx = $vecmax("vout");
// (native ngspice `.meas` can also be emitted via a pass-through extern for parity)
```

**Deferred — refined analysis + object-oriented results.** A later version
introduces typed result objects and richer host-side analysis, e.g.
`$tran(...)` returning a `TranResult`, with `Signal` methods:

```verilog
// later:
TranResult t = $tran(1n, 5m);
real bw = $ac("dec", 20, 1, 1e9).signal("vout").bandwidth_3db();
```

This OO/refined layer, together with multi-analysis composition, lands on top of
the working ngspice-mirroring base (§10) — not in v1.

### 5.5 Behavioral sources — B-source via `extern` + AST passthrough

Two kinds of "behavioral":

- **Physics / compact device** → write a Verilog-A `module` → OpenVAF → OSDI
  (§5.1). For models that fit the compact-modeling subset.
- **Arbitrary expression** → an ngspice **B-source**. In Piperine a behavioral
  source is an `extern module` whose expression argument is passed as **AST** and
  lowered to ngspice's `V=`/`I=` expression syntax:

```verilog
// expression is AST, serialized to a ngspice B-source (B1 out 0 V=v(in)*v(in))
bsource #(.V( V("in") * V("in") )) B1 (.p(out), .n(gnd));
```

This is exactly where AST-passthrough earns its keep: the behavioral expression
needs no special grammar — it is an ordinary expression handed to the B-source
handler, which serializes it to ngspice. Physics-based modeling goes to OpenVAF;
free-form expressions go to B-sources; both are "just modules" at the surface.

---

## 6. Runtime architecture

```
analog modules   --openvaf-->  .osdi (cached by source-hash + OpenVAF ver + triple)
structural + tb  --elaborate-> SPICE netlist (.cir)
runtime          spawns an isolated ngspice process, $pre_osdi loads the OSDI,
                 loads the netlist, then the interpreter runs the initial program
analyses         $tran/$ac/... are blocking calls into libngspice; results read
                 back as vectors; the same loaded circuit is reused across calls
assertions       SOA monitors wired to ngspice's per-timepoint data callback
```

- **One ngspice instance per process.** ngspice is built on global state, so
  parallelism and crash isolation come from a **process-isolated worker pool**
  (the pool already exists in the project), not from threads sharing one library.
- **OSDI** loads at runtime via `pre_osdi`; OSDI devices instantiate with the `N`
  prefix. Pin the toolchain triple (OpenVAF build, OSDI API version, ngspice
  version) into the model-cache key.
- **External (host-computed) sources** via ngspice's sync callbacks fire once per
  timepoint and are a performance cliff; reserve them for genuinely closed-loop
  cases and prefer netlist-defined sources (PWL/PULSE/SIN/B-source) otherwise.

---

## 7. Backward compatibility

Maximal, because the analog language *is* Verilog-A:

- Existing `.va` models compile unchanged (Piperine routes them straight to
  OpenVAF) or are referenced as precompiled `.osdi`.
- Every analog module Piperine emits is Verilog-A, so the OpenVAF/OSDI round-trip
  is faithful.
- The borrowed SystemVerilog constructs are syntactically native, so engineers
  read the testbench layer without learning a new surface.

---

## 8. Implementation strategy

- **Parser:** extend the existing **hand-written recursive-descent Verilog-A /
  OpenVAF parser** with the supported SystemVerilog core (§4: `initial`/`always`,
  control flow, subroutines, arrays/queues, system tasks, immediate assertions) and
  the `extern` forms. A parser generator is not an option here: Verilog/SystemVerilog
  is context-sensitive (type-vs-identifier disambiguation needs the symbol table
  during parsing, à la the C "lexer hack"), needs unbounded lookahead, and has
  ambiguities no LALR(1)/LL(k) generator (lalrpop, parol, yacc) can resolve without
  semantic feedback — which is why every serious SV parser (slang, Verible,
  `sv-parser`/nom) is hand-written or PEG/backtracking. The Verilog-A core is reused
  as-is. The procedural-core additions are the *parser-friendly* part; the
  context-sensitive constructs (`typedef`/`struct`/`enum`, `class`) sit in the
  deferred phase, so the hand-written parser only grows the hard cases later.
- **Crates (single project — no separate runtime):**
  - parser/AST (the hand-written recursive-descent parser),
  - netlist elaboration + module routing (analog→OpenVAF, structural→netlist),
  - OpenVAF driver (`.va` → `.osdi`, cached; diagnostics mapped to source),
  - the ngspice execution pool (process-isolated sessions, typed analyses, vector
    store) — already built,
  - the AST-walking interpreter (procedural layer, `extern` dispatch,
    assertion/SOA wiring).
- **OpenVAF runs at runtime on first use, cached.** This keeps the build free of
  OpenVAF and sidesteps its **lack of a macOS build** by delegating compilation to
  a Linux worker (container or the home-lab server); device authoring (rare) is
  thereby decoupled from simulation (frequent).

---

## 9. Constraints and pitfalls

- **P1 — OpenVAF is analog-only.** No digital `initial`/`always`; testbench
  procedural code is interpreted, never compiled. The compile/interpret split is
  mandatory, not optional.
- **P2 — OpenVAF subset limits.** No module-internal arrays, no `genvar`/generate,
  analog events only `@(initial_step)`/`@(final_step)`. Surface violations as
  Piperine diagnostics, not raw backend errors.
- **P3 — OpenVAF has no macOS build.** Delegate `.osdi` compilation to Linux;
  cache aggressively.
- **P4 — ngspice global state.** One instance per process; the worker pool is
  required for correctness, not just isolation.
- **P5 — SOA assertions are host-side observers**, sampled per-timepoint; they
  report and can halt but are not solver constraints.
- **P6 — OSDI version skew.** Pin OpenVAF build + OSDI API + ngspice version in
  the cache key.
- **P7 — External-source callback latency.** Per-timepoint host callbacks dominate
  runtime; prefer netlist sources.

---

## 10. Phased roadmap

- **Phase 0 — backend spike.** Drive libngspice from Rust: run a hardcoded
  netlist, read a transient vector, load one `.osdi` via `pre_osdi`. Prove
  FFI + OSDI end-to-end on Linux.
- **Phase 1 — runtime.** Process-isolated session pool, typed `$tran`, vector
  store, parametric `alter`, crash recovery. (Largely exists.)
- **Phase 2 — parser + elaboration.** Extend the hand-written recursive-descent
  parser; route analog →
  OpenVAF, structural → netlist; `extern module`/`extern task`; `$V`/`$I`.
- **Phase 3 — procedural core + ngspice-mirroring control.** The full §4 subset
  (`initial`, types, operators, control flow, subroutines, arrays/queues, system
  tasks) with a control surface that mirrors ngspice's commands and vector
  functions 1:1 (`$tran`/`$ac`/`$dc`/`$op`, `$alter`, `$rms`, …); parametric sweeps
  against the reused circuit. This is the "working system" milestone — the
  simplified first version.
- **Phase 4 — assertions + analyses.** Immediate assertions / SOA `always`
  monitors wired to the data stream; the remaining AC/DC/noise analyses.
- **Phase 5 — refinement on the working base.** Object-oriented results
  (`TranResult`/`Signal`), refined host-side data analysis, and multi-analysis
  composition; then `class`/OOP and user-defined types (`typedef`/`struct`/`enum`);
  then `rand`/`constraint` randomization (solver); then XSPICE/mixed-signal. Each
  lands without disturbing the procedural core or the ngspice-mirroring base.

---

## 11. Open decisions (short)

- **D1 — Control surface → RESOLVED:** v1 **mirrors ngspice** — analyses and data
  access surfaced as system tasks/functions matching ngspice's command + vector
  vocabulary 1:1 (`$tran`, `$ac`, `$alter`, `$rms`, …), with the testbench module
  as the implicit circuit context. Object-oriented results, refined data analysis,
  and multi-analysis composition are deferred to Phase 5.
- **D2 — SOA expression → RESOLVED:** a testbench `always` monitor — `@(step)`
  per timepoint or `@(above/cross/...)` analog-event — interpreter-evaluated, not a
  bespoke `soa` keyword (§4.8).
- **D3 — Deferred-feature ordering:** within the Phase 5 growth, which lands
  first — `class` (encapsulation) or user-defined types (`typedef`/`struct`)?
  Recommend user-defined types first (lighter, and `class` builds on them).
