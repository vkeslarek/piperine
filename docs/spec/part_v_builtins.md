# Part V — Builtins Reference

The exhaustive catalog of what a source file may call without declaring it: math
functions, analog operators, `$`-syscalls, diagnostic tasks, `@`-events, and the
always-in-scope prelude. This reference is **normative** for the builtin set;
operators, syscalls, and events are extensible via the layer-2 registries (Part I §14).

## §0 Declared surface (MD-24)

Every name listed in this Part is a **textual declaration** in a stdlib header, marked
`extern` (Part I §5.4), not a Rust-only registry entry. The implementation backing
each declaration lives in a Rust table consulted *only after* the declaration resolves
the call — so LSP go-to-definition (ctrl+click) on `sin`, `$temperature`, `ddt`,
`@device`, `Real`, or `Real::from` lands on a real declaration line, never silently
dead-ends.

| Section | Header | Form |
|---------|--------|------|
| §1 Math functions | `crates/piperine-lang/headers/math.phdl` | `extern fn` |
| §2 Analog operators (the `Expr::Call`-shaped 8) | `crates/piperine-lang/headers/operators.phdl` | `extern operator` |
| §2 Analog operators (the `EventSpec::Named` 3: `cross`/`above`/`timer`) | same | `extern operator` (textual presence only; resolved by `EventRegistry`) |
| §3 `$`-syscalls (the value-returning analog-context set) | `crates/piperine-lang/headers/tasks.phdl` | `extern task` |
| §4 Diagnostic / control tasks | `crates/piperine-lang/headers/tasks.phdl` | `extern task` |
| `@device`/`@port` attribute schemas | `crates/piperine-lang/headers/device_port.phdl` | `extern attribute` (parsed by `PluginHost::seed_schemas`, not part of every project's prelude) |
| Primitive value types (Part I §6.1) | `crates/piperine-lang/headers/types.phdl` | `extern type` |
| Cast associated functions (`Real::from` etc.) | `crates/piperine-lang/headers/types.phdl` | `extern impl TypeName { fn from(...) -> TypeName; ... }` |
| Plugin-contributed attribute schemas | each plugin's `extern.phdl` stub | `extern attribute` (auto-imported at load time; a schema-contributing plugin that publishes no stub fails loud `PluginError::MissingExternStub`) |

A permanent regression guard
(`crates/piperine-lang/tests/extern_coverage_guard.rs`) iterates every native
implementation table and asserts a matching `extern` declaration exists, so a future
commit adding an entry to `MATH_FNS` or `TaskRegistry::with_builtins()` without
authoring the matching declaration fails this test by name.

## Alias policy

The native canonical spelling is one per meaning: `|` for event OR, `ln` for natural
log, `$info`/`$warn`/`$error`/`$fatal` for diagnostics, `$finish` to terminate, one
print form (`$display`). Verilog-AMS aliases (`or`, `log`, `$warning`, `$stop`,
`$strobe`/`$monitor`) are accepted **only** in the AMS ingestion front end, never in
native PHDL.

## Contents

- §1 Math functions
- §2 Analog operators
- §3 `$`-syscalls (expression)
- §4 Diagnostic / control tasks (statement)
- §5 `@`-events
- §6 Prelude / stdlib
- §7 System-task availability matrix

---

## §1 Math functions

Expression-position calls on `Real`. Symbolically differentiated for the Jacobian.
Callable bare-name (`sqrt(x)`) or `$`-prefixed (`$sqrt(x)`) in any context.

| Fn | Arity | Computes | d/dx (arg 0, `u'`) |
|----|-------|----------|---------------------|
| `exp` | 1 | eˣ | `exp(u)·u'` |
| `ln` | 1 | natural log | `u'/u` |
| `log10` | 1 | base-10 log | `u'/(u·ln10)` |
| `sqrt` | 1 | √ | `u'/(2√u)` |
| `abs` | 1 | \|x\| | `sign(u)·u'` |
| `sin` / `cos` / `tan` | 1 | trig | `cos(u)·u'` / `-sin(u)·u'` / `u'/cos²(u)` |
| `asin` / `acos` / `atan` | 1 | inverse trig | `u'/√(1-u²)` / `-u'/√(1-u²)` / `u'/(1+u²)` |
| `atan2` | 2 | 2-arg atan | 0 |
| `sinh` / `cosh` / `tanh` | 1 | hyperbolic | `cosh(u)·u'` / `sinh(u)·u'` / `(1-tanh²(u))·u'` |
| `asinh` / `acosh` / `atanh` | 1 | inverse hyperbolic | `u'/√(1+u²)` / `u'/√(u²-1)` / `u'/(1-u²)` |
| `pow` | 2 | aᵇ | `b·pow(a,b-1)·a'` (b const) |
| `hypot` | 2 | √(a²+b²) | piecewise |
| `min` / `max` | 2 | min / max | piecewise: `u'` or `v'` of the selected branch |
| `floor` / `ceil` | 1 | round down / up | 0 |
| `limexp` | 1 | `exp(min(x,80))` (SPICE overflow clamp) | `exp(u)·u'` |

`log` is an alias of `ln`, accepted only in the AMS ingestion front end.

---

## §2 Analog operators

`analog`-body only. Each operator allocates a state variable and lowers to a companion
model in the device compiler.

| Operator | Args (defaults) | Semantics |
|----------|-----------------|-----------|
| `ddt(x)` | | time derivative; reactive stamp `α = 1/dt` |
| `idt(x, ic=0)` | | time integral from initial condition `ic` |
| `idtmod(x, ic=0, modulus=1)` | | integral wrapped modulo `modulus` (phase accumulators) |
| `ddx(x, node)` | | partial derivative of `x` with respect to a node's potential/flow |
| `delay` / `absdelay(x, dt=0)` | | ideal time delay of `dt` seconds |
| `transition(x, td=0, tr=0, tf=0, tol=0)` | | smooth a digital-like signal: delay `td`, rise `tr`, fall `tf` |
| `slew(x, rise=0, fall=0)` | | slew-rate limit: `rise` V/s on rising, `fall` on falling |
| `table(x, xs, ys, mode)` | value, breakpoints, data, interp mode | measured-data lookup with interpolation (1-D; N-D planned) |
| `laplace_np` / `laplace_zp` / `laplace_pm` / `laplace_nm` / `laplace_npm(x, num, den)` | | continuous-time Laplace filter H(s) = num/den; suffix selects coefficient convention (numerator/poles form) |
| `zi_zd` / `zi_zp` / `zi_nd` / `zi_np(x, num, den, sample_dt)` | | discrete-time Z-domain filter, sampled every `sample_dt` |
| `ac_stim(mag=1, phase=0)` | | AC small-signal stimulus (`.ac` analysis only) |
| `white_noise(psd, "label")` | | flat-spectrum noise source, extracted from a `<+` RHS |
| `flicker_noise(psd, exp=1, "label")` | | 1/f^exp noise source |

The noise sources (`white_noise`, `flicker_noise`) are extracted from the right-hand
side of a contribution before lowering and stamped as spectral density contributions in
the noise analysis. The `ac_stim` operator marks a contribution as the AC drive source
for small-signal analysis.

The analog-operator set is open: new operators register through the layer-2 extension
mechanism (Part I §14) and appear in this table.

---

## §3 `$`-syscalls (expression)

A `$`-syscall is the surface syntax for every runtime-valued or effectful operation.
The `$` prefix makes it visually distinct from user functions and signals that the call
depends on simulation state, not just its arguments.

Availability is layered by context:

### 3.1 Available in `analog` / `digital` bodies (solve-time)

These return runtime values from the solver:

| Syntax | Returns |
|--------|---------|
| `$temperature` | temperature in Kelvin |
| `$vt` / `$vt(temp)` | thermal voltage kT/q at `temp` (default: `$temperature`) |
| `$abstime` | absolute simulation time |
| `$mfactor` | instance multiplicity (parallel devices) |
| `$xposition` / `$yposition` / `$angle` | layout placement and rotation |
| `$simparam("key", default=0)` | named simulator parameter, `default` if unknown |
| `$param_given("name")` | whether a parameter was explicitly passed at instantiation |
| `$port_connected("name")` | whether a port is externally connected |
| `$limit(x, "kind", ...)` | Newton convergence limiter (`pnjlim`, `fetlim`, ...) |
| `$analysis("kind")` | whether the current analysis matches (`dc`/`tran`/`ac`/`noise`) |
| `$random` / `$random(seed)` | uniform pseudo-random number |
| `$dist_uniform(...)` / `$dist_normal(...)` / `$dist_exponential(...)` | distribution PRNs |

The `$analysis` syscall enables compile-time analysis specialization: a behavior body
may branch on the current analysis and the codegen emits only the taken branch.

### 3.2 Available in every context (pure)

These are available everywhere, including inside pure `fn` bodies:

| Syntax | Returns |
|--------|---------|
| `$assert(cond, msg)` | asserts `cond`, reports `msg` on failure |
| `$info(fmt, ...)` / `$warn(fmt, ...)` / `$error(fmt, ...)` / `$fatal(fmt, ...)` | log at severity; `{}` interpolates args |
| `$display(args...)` / `$write(args...)` | print at Info severity |

Plus the full math catalog (§1), callable with a `$` prefix.

## §4 Diagnostic / control tasks (statement)

| Syntax | Effect | Context |
|--------|--------|---------|
| `$bound_step(dt)` | cap the next timestep to `dt` | analog |
| `$finish` | terminate the simulation | analog, digital |
| `$discontinuity(n=0)` | flag an order-`n` discontinuity; break and re-solve the step | analog |
| `$info` / `$warn` / `$error` / `$fatal(fmt, args...)` | log at severity; `{}` interpolates | all |
| `$display` / `$write(args...)` | print at Info | all |

`$fatal` does not auto-`$finish` — it logs at fatal severity and returns. `$stop` is an
AMS alias of `$finish`.

---

## §5 `@`-events

`analog` blocks reject digital-edge events; `digital` blocks reject analog-crossing
events. An unrecognized event name is a compile error — there is no silent fallback.

| Form | Class | Fires |
|------|-------|-------|
| `posedge(sig)` | digital edge | rising edge of `sig` |
| `negedge(sig)` | digital edge | falling edge of `sig` |
| `change(sig)` | digital edge | any change of `sig` |
| `cross(expr)` | analog crossing | zero crossing of `expr` |
| `above(expr)` | analog crossing | one-shot level crossing of `expr` |
| `timer(period)` / `timer(period, phase)` | analog | periodic, every `period` seconds; optional `phase` offsets the first fire to `phase` (fires at `phase`, `phase+period`, …) so a source can declare both its rise and fall edges with two phased timers |
| `initial` | lifecycle | once, at the start |
| `final` | lifecycle | once, at the end (diagnostics only) |
| `A \| B` | composite | either `A` or `B` fires |

An analog event body (`@ cross`, `@ above`) updating module state is the ngspice switch
idiom: the analog kernel detects the crossing at each accepted solution and updates a
persistent variable, which then feeds back into the continuous system.

The event set is open: new events register through the layer-2 extension mechanism
(Part I §14).

---

## §6 Prelude / stdlib

Injected into every compilation unit. The prelude provides the base scope every source
file sees.

### Disciplines

| Discipline | Kind | Quantities / Storage |
|------------|------|----------------------|
| `Ground` | conservative (reference) | potential, flow |
| `Electrical` | conservative | potential `v` (V), flow `i` (A) |
| `Magnetic` | conservative | mmf, flux |
| `Thermal` | conservative | temp (K), pwr (W) |
| `Kinematic` | conservative | position, force |
| `KinematicV` | conservative | velocity, force |
| `Rotational` | conservative | angle, torque |
| `RotationalOmega` | conservative | angular velocity, torque |
| `Voltage` | storage `Real` | signal-flow potential |
| `Current` | storage `Real` | signal-flow flow |
| `Bit` | storage `Boolean` | 2-state digital |
| `Logic` | storage `Quad`, resolve `tri` | 4-state digital, tri-state |
| `DDiscrete` | storage `Quad` | 4-state digital |

### Constants

**Math:** `M_E`, `M_LOG2E`, `M_LOG10E`, `M_LN2`, `M_LN10`, `M_PI`, `M_TWO_PI`,
`M_PI_2`, `M_PI_4`, `M_1_PI`, `M_2_PI`, `M_2_SQRTPI`, `M_SQRT2`, `M_SQRT1_2`.

**Physical:** `P_Q` (elementary charge), `P_C` (speed of light), `P_K` (Boltzmann),
`P_H` (Planck), `P_EPS0` (vacuum permittivity), `P_U0` (vacuum permeability),
`P_CELSIUS0` (0°C in Kelvin).

### Capabilities

`Type`, `Net` (root markers); `Add`, `Sub`, `Mul`, `Div`, `Eq`, `Ord : Eq`, `BitAnd`,
`BitOr`, `BitXor`, `Not`, `Number : Add+Sub+Mul` (with default `double`).

### Collections and numeric types

`map<T,U>(xs: T[N], f: fn(T)->U) -> U[N]`,
`reduce<T>(xs: T[N], op: fn(T,T)->T) -> T`,
`concat(...)`. The bundles `UInt[N]`, `SInt[N]`, `Complex`.

### The `spice` namespace

The ngspice-faithful device model library ships as builtin stdlib headers
(`headers/spice/`), resolvable from any project without a `Piperine.toml`
dependency:

```phdl
use spice::diode;      // dio
use spice::bjt;        // bjt (Gummel-Poon)
use spice::mos;        // mos1 (Shichman-Hodges)
use spice::jfet;       // jfet
use spice::passives;   // res, cap, ind, mut
use spice::sources;    // vsrc, isrc
use spice::controlled; // vcvs, vccs, ccvs, cccs
use spice::switches;   // sw, csw
use spice::constants;  // ngspice const.h/defines.h values
```

A project or dependency package named `spice` shadows the builtin namespace
(project packages always win).

---

## §7 System-task availability matrix

This matrix shows which constructs are legal in which execution context. A construct
marked "—" in a column is an elaboration error if used there.

| Construct | `analog` | `digital` |
|-----------|----------|-----------|
| Math (`exp`, `abs`, ...) | ✓ | ✓ |
| `ddt` / `idt` / `idtmod` | ✓ | — |
| `ddx` | ✓ | — |
| `delay` / `absdelay` / `slew` | ✓ | — |
| `transition` / `table` / `laplace_*` / `zi_*` | ✓ | — |
| `white_noise` / `flicker_noise` | ✓ | — |
| `ac_stim` | ✓ (`.ac` only) | — |
| `$temperature` / `$vt` / `$abstime` / `$mfactor` / `$xposition` / ... | ✓ | ✓ |
| `$analysis` | ✓ | ✓ |
| `$random` / `$dist_*` | ✓ | ✓ |
| `$bound_step` / `$discontinuity` | ✓ | — |
| `$assert` / `$info` / `$warn` / `$error` / `$fatal` | ✓ | ✓ |
| `$display` / `$write(args...)` | ✓ | ✓ |
| `$finish` | ✓ | ✓ |
| `V(a,b)` / `I(a,b)` | ✓ | ✓ *(read only)* |
| `<+` (contribution) | ✓ | — |
| `<-` (force / drive) | ✓ | ✓ |
| `=` (assign) | — | ✓ |
| `@` events | ✓ | ✓ |

Measurement from a host uses the result objects (Part VIII §4), not the analog
`V`/`I` access functions.
