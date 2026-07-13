# Part III — Interpreted Context (Bench)

PHDL is one strongly-typed language with three execution contexts:

| Context | Driver | Purity | Effects available | Where |
|---------|--------|--------|-------------------|-------|
| Elaboration | compiler | pure, total (bounded) | const eval, structural emission | `mod` body |
| Compiled solve | JIT + event kernel | pure at the stamp level | analog operators, events | `analog` / `digital` body |
| **Interpreted** | tree-walk interpreter | **effectful** | analyses, measurement, override staging, I/O | `bench` body |

The interpreted context is the face of PHDL that runs **after** elaboration, over an
already-elaborated design. It is not a second language. It shares the `fn`-body grammar
of Part I §9 — the same statements, expressions, control flow, types — with three
differences:

1. **Effect availability.** Only an interpreted-context `fn` holds the toolchain handle
   and may run analyses (`$op`, `$tran`, `$ac`, `$noise`), measure (via result objects),
   stage overrides (`.r = ...`, `select(...).set(...)`), or do I/O (`$write`, `$plot`).
2. **System-task set.** The interpreted registry adds the five analysis/writer tasks;
   conversely it rejects solve-only operators (`<+`, `<-`, `ddt`, `idt`, `@` events)
   with named errors (§4).
3. **Interpretation, not inlining.** A pure `fn` (Part I §9) inlines at the call site
   and is differentiated for the Jacobian; an interpreted `fn` is tree-walked with
   lexical scopes and mutable locals. `for x in <expr>` may iterate a runtime `Vec`, and
   `var name = expr;` may omit its type, inferred at interpretation time. Both are only
   valid in this context.

## Contents

- §1 The bench block
- §2 Name resolution in the interpreted context
- §3 Effect gating and purity
- §4 System tasks available in the interpreted context
- §5 Validation: the allowlist
- §6 Measurement is through the result object
- §7 Analyses (configuration is an argument, not state)
- §8 Result and waveform types
- §9 Adjustment through reflection (staging overrides)
- §10 The uniform host-neutral API
- §11 Determinism and isolation
- §12 Sweeps are `for` loops, not tasks
- §13 Worked benches

---

## §1 The bench block

A `bench` block is an item-level declaration that attaches a testbench to a module, the
same way `analog`/`digital` attach behavior. It has no ports of its own — it operates on
the module's elaborated POM (Part IV).

```
BenchDecl ::= "bench" Ident "{" { FnDecl } "}"
```

```phdl
mod SwitchOpenTest () {
    wire signal : Electrical;
    wire gnd    : Ground;
    sw     : Switch ( /* ... */ );
    source : VoltageSource ( /* ... */ );
}
bench SwitchOpenTest {
    fn test_open_circuit() {
        var r = $op();
        $assert(r.v(source.p, gnd) != 0, "source must be active");
    }
}
```

A zero-arg `fn` inside a `bench` is a **runnable entry point**. "Test vs. flow" is a
behavioral distinction, not a syntactic one: a fn that calls `$assert` is a test; a fn
that emits a CSV is a flow. Both are ordinary `fn`s.

A `bench` `fn` is **not pure**. Its body uses the `fn` grammar of Part I §9, including
default parameter values (Part I §9.1).

---

## §2 Name resolution in the interpreted context

A bench is rooted at its module's POM. Inside a bench `fn`, names resolve in this order:

1. **Bench-local** `var`s and parameters of the bench fn itself (lexically scoped, with
   child scopes for `if`/`match`/`for` bodies).
2. **The module's POM** — nets, ports, instances, and params of the module the bench is
   attached to. These are visible as bare names, the same way a `digital` block sees
   its module's ports.
3. **Instance ports and params** via dot-access: `name.port`, `name.param` (Part I
   §7.3). This is how a bench reaches into a child instance.
4. **The standard library** — prelude types, the math catalog, the config bundles.

Post-monomorphization, generics appear in concrete form — the bench sees `Dac__8`, not
`Dac[8]`. The selector (Part IV §8) reaches anywhere in the hierarchy from any bench fn.

`gnd` / `Ground` is the predefined reference node; a bench may name it directly.

---

## §3 Effect gating and purity

The interpreted context is parameterized over a **Host** — the abstraction that provides
analysis execution, reflection lookup, and I/O. The bench `SimHost` is one
implementation; a library host and future Python/Rust hosts are others (§10). The Host
trait is the seam that makes the same tree-walking interpreter serve every embedding.

Purity is enforced through a **depth counter**. While the interpreter is inside a pure
`fn` body (Part I §9 — a `fn` or method called from a bench), the counter is nonzero.
Any `$`-syscall that requires Host access (analyses, writers, override staging)
encountered while the counter is nonzero is rejected as `TaskUnavailable`. This is how
the language enforces "only bench fns may run analyses": a pure `fn` invoked from a
bench still cannot call `$op`, because the call transiently raises the purity depth.

The practical consequence: you can call any pure `fn` from a bench, and it behaves
exactly as it would in an `analog` or `digital` body. The purity contract is the same
everywhere; the bench just adds an effectful layer on top.

---

## §4 System tasks available in the interpreted context

The interpreted context sees **three layered registries**, tried in order:

1. **Pure tasks** — available in *every* context, including pure `fn` bodies:
   `$assert`, `$info`, `$warn`, `$error`, `$fatal`, `$display`.
2. **Math catalog** — callable bare-name (`sqrt(x)`) or `$`-prefixed (`$sqrt(x)`):
   `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`, `tanh`,
   `asinh`, `acosh`, `atanh`, `exp`, `limexp`, `ln`, `log10`, `sqrt`, `pow`, `hypot`,
   `abs`, `min`, `max`, `floor`, `ceil` (25 functions; `log` is an alias of `ln`).
3. **BenchTasks** — **bench-only**: `$op`, `$tran`, `$ac`, `$noise`, `$write`. These
   require the Host and are unavailable in pure contexts.

Additionally, the `select(...)` host function is available for querying and staging
(Part IV §14).

### What the interpreted context does not get

- **`<+`, `<-`, `=` on nets** — those are solve-time (analog/digital). The bench
  measures; it does not contribute.
- **`ddt`, `idt`, `laplace_*`, `zi_*`, `transition`, `slew`, ...** — analog operators
  (Part V §2). Not callable from bench.
- **`V(a,b)`, `I(a,b)`** — branch access is analog-only. Measurement is through the
  result object (`r.v(a,b)`, `r.i(a,b)` — §6).
- **`@` events** — events belong to the solve.

Calling any of these from a bench is an error.

---

## §5 Validation: the allowlist

A `bench` fn may call only tasks in the `bench_task_implemented` allowlist. The
allowlist contains 11 task names:

| Category | Tasks |
|----------|-------|
| Diagnostics | `assert`, `info`, `warn`, `error`, `fatal`, `display` |
| Analyses | `op`, `tran`, `ac`, `noise` |
| Artifacts | `write` |

A `$`-task not in this list, called from a bench fn, is an **elaboration error** — it
fails before any analysis ever runs, never silently no-ops. This is the fail-loud
contract: an unimplemented task is a named error, not a stub. (`$plot` is specified in
Part V §3.3 but is allowlist-gated until its `BenchTask` lands — calling it today is the
elaboration error above, by design.)

Adding a new bench task requires three changes in the same commit: the allowlist entry,
the `BenchTask` implementation, and this spec's availability matrix (Part V §7).
Plugin-registered bench tasks (Part VI §9) extend the allowlist at plugin-load time —
a loaded plugin's `bench_task` contributions are callable as `$name(...)` exactly like
builtins.

---

## §6 Measurement is through the result object

The interpreted context adds **no measurement syntax** and does not reuse `V(a,b)`/
`I(a,b)` — those stay analog-only (Part I §10.1). Measurement is by methods on the
result object returned by an analysis:

```phdl
var r = $op();
var v = r.v(vsrc, gnd);              // voltage across two terminals
var i = r.i(resistor.p, resistor.n); // current through a two-terminal device
```

`r.v(a, b)` and `r.i(a, b)` take a defaulted second argument (Part I §9.1): `r.v(a)` is
equivalent to `r.v(a, gnd)`. Two analyses produce two independent result values — there
is no "active result" global state.

**`Trace.i`** — for transient and AC results, current is recomputed per timestep from
solved voltages. Resistive current comes from the device's residual; reactive current
from `dQ/dt` of its charge expression; ideal sources read the exact branch unknown.
Devices that read runtime state or internal vars fail loud rather than guessing. A
two-terminal match with more than one candidate instance is a fail-loud error.

---

## §7 Analyses (configuration is an argument, not state)

Four analyses are available, each taking a config bundle and returning a typed result.
There are **no** configuration-setter tasks — `$option`, `$temperature(set)`, `$ic`,
`$nodeset` do not exist. Configuration lives as fields of the config bundle, passed as
an argument. This is the "no hidden state" invariant (§11).

| Task | Config bundle | Returns |
|------|---------------|---------|
| `$op(cfg)` | `OpConfig { .solver, .nodeset }` | `OpResult` |
| `$tran(cfg)` | `TranConfig { .stop, .step, .start, .solver, .ic }` | `Trace` |
| `$ac(cfg)` | `AcConfig { .fstart, .fstop, .points, .scale, .solver }` | `Trace` (complex) |
| `$noise(cfg)` | `NoiseConfig { .out, .fstart, .fstop, .points, .scale, .solver }` | `NoiseTrace` |

`$op(cfg)` is sugar for `Module.op(cfg)` (§10). Positional conveniences exist for
common cases: `$tran(stop, step)`, and `$noise(out, cfg)` (the latter is a deprecated
alias, kept for one release).

### 7.1 Config bundles

Config bundles are ordinary bundle declarations in the prelude:

```phdl
bundle Solver {
    temperature : Real = 300.0,
    gmin : Real = 1e-12,
    abstol_i : Real = 1e-9,
    reltol : Real = 1e-4,
    ...
}
enum Scale { Lin, Dec, Oct }
bundle OpConfig    { solver : Solver = Solver {}, nodeset : Map<String, Real> = Map {} }
bundle TranConfig  { stop : Real, step : Real = 0.0, start : Real = 0.0,
                     solver : Solver = Solver {}, ic : Map<String, Real> = Map {} }
bundle AcConfig    { fstart : Real, fstop : Real, points : Natural,
                     scale : Scale = Dec, solver : Solver = Solver {} }
bundle NoiseConfig { out : NetRef, fstart : Real, fstop : Real, points : Natural,
                     scale : Scale = Dec, solver : Solver = Solver {} }
```

`NetRef` is the host-side net-reference type: in a bench, naming a net or an instance
port (`vout`, `amp.out`) evaluates to a `NetRef`, so `.out` is written as a bare net
name. Two `NetRef`s compare equal when they refer to the same net.

`step = 0.0` in `TranConfig` means adaptive timestep selection (the solver picks). The
`scale` field controls the frequency sweep spacing: linear (`Lin`), logarithmic per
decade (`Dec`), or logarithmic per octave (`Oct`).

---

## §8 Result and waveform types

The result-type surface:

```
OpResult   : v(a, b = gnd) -> Real          // node voltage
           ; i(a, b) -> Real                // device current
           ; nodes() -> Map<String, Real>   // all node voltages

Trace      : v(a, b = gnd) -> Waveform<Real>   // voltage over time/frequency
           ; i(a, b) -> Waveform<Real>          // current over time/frequency
           ; t -> Waveform<Real>                // time axis
           ; points -> Natural ; len -> Natural

NoiseTrace : psd -> Waveform<Real>             // noise spectral density
           ; total -> Real                      // integrated noise
```

`Waveform<T>` methods:

| Method | Returns | Notes |
|--------|---------|-------|
| `at(t)` | `T` | value at point t |
| `points` | `Natural` | number of samples |
| `len` | `Natural` | alias for `points` |
| `min()` / `max()` | `Real` | extremes |
| `mean()` / `rms()` | `Real` | averages |
| `peak_to_peak()` | `Real` | max − min |
| `cross(level)` | `Vec<Real>` | crossing times |
| `rise_time()` / `fall_time()` | `Real` | 10–90% edges |
| `fft()` | `Waveform<Complex>` | spectral transform |
| `map(f)` | `Waveform<U>` | per-sample transform |

`Waveform<Complex>` (AC results) adds: `mag()` → magnitude, `phase()` → phase,
`db()` → magnitude in dB.

Post-processing — windowing, filtering, statistical reduction, custom transforms — is
library work over `Waveform`, not built-in tasks. The philosophy: the result object
gives you the raw data and a few universal methods; everything else is a `fn` you write
or import.

---

## §9 Adjustment through reflection (staging overrides)

Writing a parameter **stages an override** — it does not edit the design in place. The
next analysis re-elaborates purely from the staged overrides. A structural param change
(width, instance count) triggers full re-elaboration; a non-structural one (a resistor
value) is a netlist patch. The engine decides which; the user just writes the
assignment.

```phdl
sw.ctrl = 1.0;                              // bare-name staging
select("//resistor").resistance = 2e6;      // bulk staging via selector
select("//leg").set("w", 2.0);              // .set sugar
```

The **closure loop** — measure, adjust, re-run — is an ordinary `for` in a bench fn:

```phdl
fn tune_bias() {
    for i in 0..20 {
        var r = $op();
        var err = r.v(out) - 1.0;
        if (abs(err) < 1e-3) { return; }
        bias.trim = bias.trim - 0.1 * err;
    }
    $error("bias did not converge");
}
```

Plugin-driven closure via `extract` / `.attach` / `.meta` is the layer-4 extension
mechanism (Part I §14): a plugin reflects over the POM, runs extraction, and returns
**annotations** (parasitics keyed by `NetId`, fused into the netlist by KCL) and
**overrides** (consumed by pure re-elaboration).

---

## §10 The uniform host-neutral API

The complete operation set is modeled once and exposed identically from every host:
the bench interpreter, Piperine-as-a-library, Python, and Rust. The Host trait (§3) is
the seam; the bench `SimHost` is one implementation.

```
load(path: String) -> Result<Design, LoadError>

Design  : top() -> Module
        ; module(name) -> Option<Module>
        ; modules() -> Selection<Module>
        ; select(path) -> Selection<Node>
        ; const_(name) -> Option<Value>

Module  : ports() / nets() / instances() / params() / behaviors() -> Selection
        ; net(n) / param(n) / instance(n) -> Option
        ; param.set(v)
        ; select(path)
        ; op(cfg: OpConfig = OpConfig {}) -> OpResult
        ; tran(cfg: TranConfig) -> Trace
        ; ac(cfg: AcConfig) -> Trace
        ; noise(cfg: NoiseConfig) -> NoiseTrace
```

In a bench (§2), the design is implicit and `$op(cfg)` is equivalent to
`<this module>.op(cfg)`. As a library, the same calls are explicit:

```
Piperine :  load("chip.ppr").module("Amp").op(OpConfig { .solver = Solver { .temperature = 350.0 } })
Python   :  load("chip.ppr").module("Amp").op(OpConfig(solver=Solver(temperature=350.0)))
Rust     :  load("chip.ppr")?.module("Amp")?.op(OpConfig { solver: Solver { temperature: 350.0, ..default() }, ..default() })?
```

Each returns the identical `OpResult` interface; `r.v(out)` reads the same value
everywhere. The result types, config bundles, and reflection surface are the one
contract serialized over the ABI (Part IV §7). Each host presents it idiomatically —
property sugar in Piperine/Python, explicit `..default()` in Rust — but never a
different shape.

---

## §11 Determinism and isolation

Four invariants govern every bench execution:

1. **Fresh view per entry point.** Each bench fn invocation starts with a clean
   interpreter state. Only staged overrides persist across calls.
2. **Staged overrides accumulate** until the next analysis consumes them. Writing the
   same param twice before an analysis keeps the last value.
3. **Results are immutable snapshots.** A `Trace` is a value; reading it twice yields
   the same data. There is no "live" result that changes under you.
4. **Analyses are pure functions** of (elaborated design + staged overrides + config
   bundle). Identical inputs always produce identical results and verdicts.

There is no global mutable simulator state reachable from bench. A bench fn that runs
an analysis twice with the same inputs gets the same answer both times — no hidden
warm-up, no accumulator, no side channel.

---

## §12 Sweeps are `for` loops, not tasks

A parameter sweep is an ordinary `for` loop over a list, staging an override and running
an analysis each iteration. There is no `$sweep` task — the language already has loops,
and loops already work.

```phdl
bench AmpSweep {
    fn dc_gain_vs_load() {
        var curve : Vec<(Real, Real)> = [];
        for rl in [1e3, 1e4, 1e5, 1e6] {
            load.resistance = rl;
            var r = $op();
            curve.push((rl, r.v(out) / r.v(in_)));
        }
        $write("gain_vs_load.csv", curve);
    }
}
```

A `for x in <runtime Vec>` is only valid in the interpreted context — in a compiled
`analog`/`digital` body, the loop bound must be an elaboration constant.

---

## §13 Worked benches

Cross-reference to Appendix A:

- **A.3.1** — Open-circuit vs. closed-circuit test (DC operating point, `$op`).
- **A.3.2** — Transient with a warm-corner config (`$tran` at 358.15 K).
- **A.3.3** — AC bandwidth via `Waveform<Complex>.db().at(f)`.
- **A.3.4** — Sweep as a `for` loop, CSV output.
- **A.3.5** — Closure loop (`tune_bias` — measure, adjust, re-run).
