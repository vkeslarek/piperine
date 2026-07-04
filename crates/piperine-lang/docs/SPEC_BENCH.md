# Piperine HDL — Specification, Part II: The `bench` Block and the Uniform API

`bench` is the effectful scripting layer of PHDL. It runs **after** elaboration and
monomorphization, over the concrete netlist, and is where a designer runs simulations, measures
results, and adjusts the design through reflection. Verification, parameter sweeps, and the
design-closure loop live here.

PHDL is one **strongly-typed** language with two faces: the *compiled* face (elaborated
`analog`/`digital` behavior) and the *interpreted* face (the `bench`, run interactively over an
elaborated design). The bench body is the **same `fn` grammar as a bundle `impl`** (Part I §9);
only the context differs — effectful, rooted at a module, with the simulation and reflection
tasks available (§10).

The same operations are exposed as a **uniform object-model API** (§8) callable identically from
a Piperine `bench`, from Piperine-as-a-library, from Python, and from Rust. `$op()` inside a
bench and `design.op()` from Python are the same operation with the same types.

```phdl
bench SwitchOpenTest {
    fn test_open_circuit() {
        var r = $op();
        $assert(r.v(vsrc, gnd) != 0, "voltage source should be active");
        $assert(r.i(resistor.p, resistor.n) == 0, "no current with the switch open");
    }
}
```

---

## 1. Position and principles

| Layer | Runs | Pure? |
|-------|------|-------|
| elaboration | once | pure |
| `analog`/`digital` | inside the solver | pure per step |
| **`bench`** | after elaboration | **effectful** |

Effects are **gated by context**: only a `bench` fn holds the toolchain handle and may run
analyses, measure, stage overrides, or do I/O. A `bench` may loop unbounded and is otherwise a
driver like `main`.

Two invariants carry the language's principles into the bench, and they are the reason there are
no special cases:

- **No hidden state.** An analysis takes all of its configuration as an explicit argument (§5),
  never from prior stateful calls. There is no active result, no ambient options, no implicit
  temperature. (This is why `$option`/`$ic`/`$nodeset`-style config tasks do **not** exist — they
  would be hidden state; configuration is a value passed in.)
- **No in-place mutation.** Design changes stage overrides consumed by the next analysis (§7).
  Every analysis is a pure, deterministic elaborate-and-solve of (design + staged overrides +
  config). Reproducibility is a property of the design; the bench sequences reproducible runs.

Everything is **blocking / synchronous** for now; concurrency of analyses is deferred and the
immutable-result model (§9) keeps it safe to add later.

---

## 2. The `bench` block

```phdl
bench ModName {
    fn name() { … }             // entry point (no args) — a test or a flow
    fn helper(x: T) -> U { … }  // reusable
}
```

Attached to a module by name, as `analog ModName`/`digital ModName` are. A **testbench module**
is the common case: a ports-less top instantiating the DUT plus stimulus (§11). Bodies use the
`fn` grammar of Part I §9 (with the default-parameter extension of §10). A zero-argument `fn` is
a runnable **entry point** the toolchain discovers (`piperine test`/`run`); a test asserts, a
flow sweeps/tunes/reports — the split is behavioral, not syntactic. A `bench` fn is **not pure**.

---

## 3. Module context and name resolution

A `bench ModName` is rooted at the elaborated `ModName`. Names resolve: (1) bench-local `var`s
and fn params; (2) the module's POM — its nets, instances, params. So `vsrc`, `resistor`, `sw`
are the module's; `resistor.p` is an instance port net; `sw.ctrl` a param/port. `gnd`/`Ground`
is the reference node. These node references are what a result's `.v`/`.i` take (§4). Post
monomorphization, generics appear in concrete form.

---

## 4. Measurement is through the result object

The bench adds no measurement syntax and does **not** reuse `V(a,b)`/`I(a,b)` — those stay
analog-only. An analysis returns a result object; potentials and flows are read from it by
method:

```phdl
var r = $op();
r.v(a, b)     // potential across (a, b)
r.v(a)        // potential of a vs. ground  (default second argument, §10)
r.i(a, b)     // branch flow
```

`r.v(a)` and `r.v(a, b)` are the same method with a defaulted second argument (§10). Because a
result is a value, there is no active-result state: two analyses are two values
(`var dc = $op(); var tr = $tran(TranConfig { .stop = 1e-3 });`). Results are immutable snapshots
(§9). The measurement return type follows the analysis: `OpResult` yields `Real`, a `Trace`
yields `Waveform` (§6).

---

## 5. Analyses (configuration is an argument, not state)

Four analyses. Each takes a **config bundle** and returns a result object. In a bench the `$`
form is sugar for the implicit design's method (`$op(cfg)` ≡ the rooted `Module.op(cfg)`, §8):

| Analysis | Signature | Result |
|----------|-----------|--------|
| operating point | `op(cfg: OpConfig = OpConfig {}) -> OpResult` | scalar `.v`/`.i` |
| transient | `tran(cfg: TranConfig) -> Trace` | `Waveform<Real>` over time |
| AC small-signal | `ac(cfg: AcConfig) -> Trace` | `Waveform<Complex>` over frequency |
| noise | `noise(cfg: NoiseConfig) -> NoiseTrace` | PSD over frequency |

Config with all-default fields lets `$op()` run with no argument; a config with required fields
(`tran` needs `stop`) must be given. A **sweep** is not a task — it is a bounded `for` that stages
a value and re-runs (§7); corners and Monte-Carlo are library patterns over these four.

### 5.1 Config bundles

Ordinary value bundles (Part I §6.5) with defaults — extensive parameters modeled as data, not
as stateful setter calls. Per-node hints (`ic`, `nodeset`) are maps, not hidden state:

```phdl
bundle Solver {
    temperature : Real = 300.15,     // K
    reltol : Real = 1e-3,  abstol : Real = 1e-12,  gmin : Real = 1e-12,
    max_iter : Natural = 100,
}
bundle OpConfig    { solver : Solver = Solver {},  nodeset : Map<Net, Real> = {} }
bundle TranConfig  { stop : Real,  step : Real = 0.0 /*auto*/,  start : Real = 0.0,
                     ic : Map<Net, Real> = {},  solver : Solver = Solver {} }
bundle AcConfig    { fstart : Real,  fstop : Real,  points : Natural = 100,
                     scale : Scale = Dec,  solver : Solver = Solver {} }
bundle NoiseConfig { out : Branch,  fstart : Real,  fstop : Real,  points : Natural = 100,
                     scale : Scale = Dec,  solver : Solver = Solver {} }

enum Scale { Lin, Dec, Oct }
```

These are stdlib bundles; a project may define its own config bundles and pass them, since the
analyses are ordinary methods taking bundle arguments.

---

## 6. Result and waveform types

```
OpResult
  v(a: Net, b: Net = gnd) -> Real
  i(a: Net, b: Net = gnd) -> Real

Trace                                       // $tran and $ac
  v(a: Net, b: Net = gnd) -> Waveform<T>    // T = Real ($tran) | Complex ($ac)
  i(a: Net, b: Net = gnd) -> Waveform<T>
  axis() -> Waveform<Real>                  // time or frequency

NoiseTrace
  psd()   -> Waveform<Real>
  total() -> Real

Waveform<T>                                 // a generic series over the analysis axis
  at(x: Real) -> T
  points() -> Vec<(Real, T)>
  len() -> Natural
  map(f: fn(T) -> U) -> Waveform<U>         // arbitrary transforms
  // T = Real:
  min() / max() / mean() / rms() / peak_to_peak() -> Real
  cross(level: Real, dir: CrossDir = Either) -> Option<Real>
  rise_time(lo: Real, hi: Real) -> Option<Real>   ;  fall_time(...) -> Option<Real>
  fft() -> Waveform<Complex>
  // T = Complex:
  mag() / phase() / db() -> Waveform<Real>
```

`Waveform<T>` is a generic value-layer type (Part I §6.6). It is the point of the design: signal
post-processing is **library** work over `Waveform`, not built-in tasks. Beyond the methods
above, FFT-derived measures (THD, SNR, spectral peaks), eye diagrams, and windowing are library
functions over `points()`/`fft()`, added without touching the language.

---

## 7. Adjustment through reflection

A bench tunes by staging overrides on the POM, then re-running:

```phdl
sw.ctrl = 1;                               // stage
select("//resistor").resistance = 2e6;     // stage across a set
var r = $op();                              // deterministic re-elaborate + solve
```

The design-closure loop is `measure → adjust → re-run`:

```phdl
fn tune_bias() {
    for _ in 0..20 {
        var r = $op();
        var err = r.v(out) - 1.0;
        if (abs(err) < 1e-3) { return; }
        bias.trim = bias.trim - 0.1 * err;
    }
    $error("bias did not converge");
}
```

Plugin-driven closure (extract parasitics → re-simulate) is the same loop with `extract(...)` and
`attach`/`meta` from the extensibility spec.

---

## 8. The uniform API (host-neutral)

The complete operation set, modeled once and exposed identically in every host. This is the QoL
payoff: Piperine-as-a-library, Python, and Rust drive the same interface with the same types.

```
// entry
load(path: String) -> Result<Design, LoadError>

// Design — reflection root (reflection spec §2) + analyses at design scope
Design
  top() -> Module
  module(name: String) -> Option<Module>
  modules() -> Selection<Module>
  select(path: String) -> Selection<Node>

// Module — reflection nav + staging (reflection spec) + the four analyses
Module
  // navigation / staging (reflection spec): ports() nets() instances() params()
  //   net(n) param(n) instance(n) ; param.set(v) ; select(path)
  op(cfg: OpConfig = OpConfig {}) -> OpResult
  tran(cfg: TranConfig) -> Trace
  ac(cfg: AcConfig) -> Trace
  noise(cfg: NoiseConfig) -> NoiseTrace
```

**In a bench**, the design is the implicit root and `$op(cfg)` ≡ `<this module>.op(cfg)`; names
resolve against the module (§3).

**As a library**, the same calls are explicit and chain identically across languages:

```
Piperine :  load("chip.ppr").module("Amp").op(OpConfig { .solver = Solver { .temperature = 350.0 } })
Python   :  load("chip.ppr").module("Amp").op(OpConfig(solver=Solver(temperature=350.0)))
Rust     :  load("chip.ppr")?.module("Amp")?.op(OpConfig { solver: Solver { temperature: 350.0, ..default() }, ..default() })?
```

Each returns the identical `OpResult` interface; `r.v(out)` reads the same value everywhere. The
result and waveform types, config bundles, and reflection surface are the one contract the ABI
(reflection spec §7) serializes; each host presents it idiomatically (Piperine/Python property
sugar, Rust explicit `..default()`), never a different shape.

---

## 9. Determinism and isolation

- Each entry-point fn runs against a fresh view; staged overrides do not leak between entry
  points.
- Within a fn, staged overrides accumulate until the next analysis, which re-elaborates from them
  and returns a new result.
- Result objects are **immutable snapshots**: a result computed before a staged change stays a
  valid value afterward, describing the earlier solve — nothing to invalidate.
- Analyses are pure functions of (elaborated design + staged overrides + config bundle);
  identical inputs give identical results and verdicts. Execution is blocking.

---

## 10. Language extensions (belong to Part I §9)

The bench needs one genuine language change; the rest rides existing features.

- **Default parameter values.** A `fn`/method parameter may carry a default:
  `fn v(self, a: Net, b: Net = gnd) -> Real`. Trailing parameters may have defaults; a call may
  omit them (`r.v(a)` ≡ `r.v(a, gnd)`). Defaults are elaboration constants. This applies to all
  functions and methods, in bundle `impl`s and benches alike, so it is a Part I §9 addition, not
  a bench-only rule. It replaces overloading-by-arity (which PHDL does not have) and makes
  optional config (`op(cfg: OpConfig = OpConfig {})`) expressible.
- **Config bundles** ride Part I §6.5 (bundles with field defaults) — no new syntax; "extensive
  parameters" become a bundle argument rather than a stateful setter, which is what keeps analyses
  free of hidden state.
- **`Waveform<T>`** rides Part I §6.6 generics — a generic stdlib value type, with signal
  processing as library functions over it.

No other construct is added; the bench is the existing `fn` language in an effectful context.

---

## 11. System-task availability

| Task / form | analog | digital | bench | Meaning in bench |
|-------------|:------:|:-------:|:-----:|------------------|
| `$assert(cond, msg)` | ✓ | ✓ | ✓ **implemented** | fails the test/flow |
| `$info/$warn/$error/$fatal` | ✓ | ✓ | ✓ **implemented** | run log |
| math (`exp`, `abs`, …) | ✓ | ✓ | ✓ **implemented** | same |
| `$op(cfg)` | — | — | ✓ **implemented** (`$op()` and `$op(OpConfig { .solver = Solver { … } })`) | DC operating point → `OpResult` |
| `$tran(cfg)` | — | — | ✓ **implemented** (`TranConfig { .stop, .step /*0 = adaptive auto*/, .start /*delayed-start: solve from 0, record from .start*/, .solver }`; positional `(stop, step)` kept as convenience; `ic:` maps not yet) | transient → `Trace` |
| `$ac(cfg)` | — | — | ✓ **implemented** (`AcConfig { .fstart, .fstop, .points, .scale, .solver }`; `Oct` maps onto the solver's log sweep) | frequency sweep → complex `Trace` |
| `$noise(cfg)` | — | — | ✓ **implemented** (`NoiseConfig { .out = Net \| (Net, Net), .fstart, .fstop, .points, .scale, .solver }` — the spec's `out : Branch` field, a bare Net meaning `(net, gnd)` or a `(Net, Net)` pair; the positional `$noise(out, cfg)` alias is kept for one release) | `NoiseTrace.{psd,total}` |
| result `.v/.i` | — | — | ✓ **implemented** on `OpResult`, `Trace`, and the AC `Trace` (`Trace.i` recomputes a two-terminal device's current per step from the solved voltages — resistive via `eval_residual`, reactive via `dQ/dt` of `eval_charge`; ideal sources read the exact branch unknown; devices reading runtime state/vars fail loud) | measurement (§4, §6) |
| `Waveform` methods | — | — | ✓ **implemented**: `at/min/max/mean/rms/peak_to_peak/len/points/cross/rise_time/fall_time/fft`, `mag/phase/db` on `Waveform<Complex>`, and `map(f)` (a closure-taking method — the interpreter invokes the closure per sample; Real result stays `Waveform`, Complex result stays `ComplexWaveform`) | measurement (§6) |
| `select`, name/`.set` staging | — | — | ✓ **implemented**: bare-name staging (`sw.ctrl = 1`), `select("...").param = v` bulk staging (string-literal paths), and `select("...")` in *expression* position returning a `SelectionRef` (`len`/`labels`/field-read; staging via a held selection re-runs against the live design) | reflection + override |
| `extract`, `.attach`, `.meta` | — | — | ✓ *not yet implemented* | plugin annotations (extensibility spec) |
| `$write(path, …)` | — | — | ✓ **implemented** (CSV of lists/tuples/scalars) | emit artifacts |
| `$plot(w, title)` | — | — | ✓ *not yet implemented* | emit artifacts |
| `V(a,b)`/`I(a,b)` branch access | ✓ | — | ✗ | analog-only; bench uses a result object |
| `<+`, `<-`, `ddt`, `idt`, operators | ✓ | — | ✗ | measure, not contribute |
| `@` events, `posedge`/`cross` | ✓ | ✓ | ✗ | events belong to the solve |

There are no configuration setter tasks (`$option`/`$temperature(set)`/`$ic`/`$nodeset`) — that
configuration is fields of the analysis config bundle (§5.1). A task the toolchain does not
implement is a compile error in a bench, not a silent no-op — calling an unimplemented task fails
at elaboration, before any analysis ever runs.

The §5.1 config bundles (`Solver`, `OpConfig`, `TranConfig`, `AcConfig`, `NoiseConfig`) and the
`Scale`/`CrossDir` enums are **defined in the stdlib prelude** and consumed by the analyses; the
spec's `ic`/`nodeset` `Map<Net, Real>` fields await the `Map` value type. Default parameter values
on user-defined `fn`/method signatures (Part I §9/§10) are **implemented**: trailing params may
carry a default (`fn foo(x: Real, k: Real = 2.0)`), a call may omit them, and defaults are
elaboration constants honored by both the interpreter (bench/POM fns) and the IR inliner (analog
fns used in contributions).

---

## 12. Worked examples

**12.1 Open-circuit test.**

```phdl
mod SwitchOpenTest() {
    wire gnd : Electrical;
    wire signal : Electrical;
    wire vsrc : Electrical;
    sw       : Switch        ( .a = signal, .b = gnd ) { .ctrl = 0.0 };
    source   : VoltageSource ( .p = vsrc, .n = gnd ) { .voltage = 5.0 };
    resistor : Resistor      ( .p = vsrc, .n = signal ) { .resistance = 1e6 };
}
bench SwitchOpenTest {
    fn test_open_circuit() {
        var r = $op();
        $assert(r.v(vsrc, gnd) > 4.9, "voltage source should be active");
        $assert(r.i(resistor.p, resistor.n) < 1e-8, "no current with the switch open");
    }
    fn test_closed_circuit() {
        sw.ctrl = 1.0;
        var r = $op();
        $assert(r.i(resistor.p, resistor.n) > 4e-6, "current should flow when closed");
    }
}
```

(Ports bind positionally/named in the instance's `(...)` list; params bind in a trailing `{...}`
block — Part I §7.3. A `wire` declares one net per statement; there is no comma-separated form.
Numeric comparisons use a tolerance, not exact equality, following a solved `Real` — SPEC
Part I's `!=`/`==` are exact, and `Real`-valued voltages are never exactly a target value.)

**12.2 Transient with a warm-corner config.**

```phdl
bench OscTest {
    fn test_frequency() {
        var r = $tran(TranConfig { .stop = 1e-3, .step = 1e-7,
                                   .solver = Solver { .temperature = 358.15 } });
        var out = r.v(out, gnd);
        $assert(out.peak_to_peak() > 1.0, "oscillation should start");
    }
}
```

**12.3 AC bandwidth via a library FFT-free magnitude read.**

```phdl
bench FilterTest {
    fn test_bandwidth() {
        var r = $ac(AcConfig { .fstart = 1.0, .fstop = 1e9, .points = 100 });
        $assert(r.v(out).db().at(1e3) > -3.0, "passband flat at 1 kHz");
    }
}
```

**12.4 Sweep — a `for`, not a task.**

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

---

## 13. Relationship to the companion specifications

- **Reflection API (POM)** — name resolution (§3), staging (§7), and the `Design`/`Module` handles
  (§8) are the object model; the result/waveform types are the simulation surface the reflection
  spec deferred here.
- **Selector** — `select(...)` for bulk staging and measurement.
- **Extensibility** — `extract`/`.attach`/`.meta` and plugin invocation, bench-only.
- **IR spec** — an analysis runs the codegen'd device over the solver; the bench never sees the IR.

---

## 14. Open questions

- **Sweep sugar** — a `$dc(param, from, to, step)` convenience vs. the `for` idiom (no-bloat).
- **Waveform algebra scope** — which measures are built-in `Waveform` methods vs. library
  functions over `points()`/`fft()`.
- **Node reference type — resolved for milestone 1.** `.v(a, b)`/`.i(a, b)` take bare names,
  resolved against the bench's module POM (§3) into `Net`/`Instance` handles; only exposed
  top-level nets and instance ports are addressable (encapsulation holds — a device-internal node
  that never reaches a port is not nameable from a bench). `.i(a, b)` is defined as the *unique*
  two-terminal instance whose ports connect exactly to nets `a` and `b`; the instance-port form
  (`.i(resistor.p, resistor.n)`) is preferred and always unambiguous. A device-internal current
  with no MNA branch unknown (no ideal-source `<-`) is recomputed from the solved terminal
  voltages via the device's own residual, not read as a separate solver variable; a two-terminal
  match with more than one candidate instance is a fail-loud error, not a guess.
- **Default-argument ordering** — confirming trailing-only defaults (no keyword-argument calls) is
  enough, or whether named arguments at call sites are wanted (they would generalize `.name =`).
- **Override addressing** — milestone 1 stages by bare instance label within the bench's own
  module (`sw.ctrl = 1` stages against instance `sw`); hierarchical dotted paths into a nested
  DUT (`select("//dac/rseg[3]")`-style) are `select`'s job once it exists, not bare-name staging's.