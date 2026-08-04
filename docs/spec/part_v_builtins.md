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

**One stated exception.** Four analog operators — `idtmod`, `absdelay`, `table`, and
`ac_stim` — carry **no** `extern operator` declaration today; they resolve through
codegen's operator registry rather than through a header, so ctrl+click on them
dead-ends. This is a known MD-24 gap, not a second sanctioned mechanism; §2's table
marks each operator's declaration status explicitly, and `ROADMAP.md` tracks closing
it. The `laplace_*`/`zi_*` family is undeclared *and* unreachable — see §2.

| Section | Header | Form |
|---------|--------|------|
| §1 Math functions | `crates/piperine-lang/headers/math.phdl` | `extern fn` |
| §2 Analog operators (the `Expr::Call`-shaped 8) | `crates/piperine-lang/headers/operators.phdl` | `extern operator` |
| §2 Analog operators (the `EventSpec::Named` 3: `cross`/`above`/`timer`) | same | `extern operator` (textual presence only; resolved by `EventRegistry`) |
| §3 `$`-syscalls (the value-returning analog-context set) | `crates/piperine-lang/headers/tasks.phdl` | `extern task` |
| §4 Diagnostic / control tasks | `crates/piperine-lang/headers/tasks.phdl` | `extern task` |
| `@device`/`@port` attribute schemas | `crates/piperine-lang/headers/device_port.phdl` | `extern attribute` (parsed by `PluginHost::seed_schemas`, not part of every project's prelude) |
| Statistical distributions (dv-core DVC-02) | `crates/piperine-lang/headers/statistics.phdl` | `extern fn` — the names a `tol` clause may name |
| Constraint helpers (dv-core DVC-09) | `crates/piperine-lang/headers/constraints.phdl` | `extern fn` — each carrying its `@quantity(class, analysis)` class |
| Primitive value types (Part I §6.1 — the seven) | `crates/piperine-lang/headers/types.phdl` | `extern type` |
| Cast associated functions (`Real::from` etc.) | `crates/piperine-lang/headers/types.phdl` | `extern impl TypeName { fn from(...) -> TypeName; ... }` |

Plugins contribute **no** declarations of any kind — no `extern`, no attribute
schemas, no per-plugin stub file (Part VI §6). The `@device`/`@port` schemas above are
seeded by the plugin *system*, not by any one plugin, precisely so two device plugins
cannot collide on them.

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
- §6.2 Design-verification headers (dv-core)
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

The **Declared** column is the MD-24 status (§0): *yes* = an `extern operator` line in
`headers/operators.phdl` backs the name; *no* = the name resolves through codegen's
operator registry with no header declaration (a tracked gap); *unreachable* = neither
declared nor resolvable, so a call fails loud at elaboration.

| Operator | Args (defaults) | Declared | Semantics |
|----------|-----------------|----------|-----------|
| `ddt(x)` | | yes | time derivative; reactive stamp `α = 1/dt` |
| `idt(x, ic=0)` | | yes | time integral from initial condition `ic` |
| `idtmod(x, ic=0, modulus=1)` | | **no** | integral wrapped modulo `modulus` (phase accumulators) |
| `ddx(x, node)` | | yes | partial derivative of `x` with respect to a node's potential/flow |
| `delay(x, dt=0)` | | yes | ideal time delay of `dt` seconds |
| `absdelay(x, dt=0)` | | **no** | alias of `delay` |
| `transition(x, td=0, tr=0, tf=0, tol=0)` | | yes | smooth a digital-like signal: delay `td`, rise `tr`, fall `tf` |
| `slew(x, rise=0, fall=0)` | | yes | slew-rate limit: `rise` V/s on rising, `fall` on falling |
| `table(x, xs, ys, mode)` | value, breakpoints, data, interp mode | **no** | measured-data lookup with interpolation (1-D linear only — Part IX §5) |
| `ac_stim(mag=1, phase=0)` | | **no** | AC small-signal stimulus (`.ac` analysis only) |
| `white_noise(psd, "label")` | | yes | flat-spectrum noise source, extracted from a `<+` RHS |
| `flicker_noise(psd, exp=1, "label")` | | yes | 1/f^exp noise source |
| `laplace_np` / `laplace_zp` / `laplace_pm` / `laplace_nm` / `laplace_npm(x, num, den)` | | **unreachable** | continuous-time Laplace filter H(s) = num/den; suffix selects coefficient convention |
| `zi_zd` / `zi_zp` / `zi_nd` / `zi_np(x, num, den, sample_dt)` | | **unreachable** | discrete-time Z-domain filter, sampled every `sample_dt` |

The noise sources (`white_noise`, `flicker_noise`) are extracted from the right-hand
side of a contribution before lowering and stamped as spectral density contributions in
the noise analysis. The `ac_stim` operator marks a contribution as the AC drive source
for small-signal analysis; multiple `ac_stim` calls on one contribution sum as phasors.

**The `laplace_*`/`zi_*` family is specified but not available.** No `extern operator`
declares either family, so MD-24 (Part I §13.1) stops a call at elaboration — it never
reaches codegen. The rows above document the intended contract for when the family
lands; they are not a claim that it works today. This is a language backlog item
tracked in `ROADMAP.md`, and the failure is loud, never a silent zero.

The analog-operator set is open: new operators register through the layer-2 extension
mechanism (Part I §14), declare an `extern operator` line, and appear in this table.

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
| `dc` / `tran` / `ac` | analysis scope | every evaluation point of that analysis — **`constraint` blocks only** (Part I §10.6, Part II §9), never a behavior body |

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
`concat(...)`. The bundles `UInt[N]`, `SInt[N]`. (`Complex` is a primitive `extern
type`, not a bundle — Part I §6.1.)

### §6.2 Design-verification headers (dv-core)

Two stdlib headers back the design-verification surface (DVC-02/09). Both are
`extern` declarations, marked `extern` per MD-24 — the implementation backing
each name lives in a Rust table consulted *only after* the declaration resolves
the call, and the `extern_coverage_guard.rs` regression test fails if a
runtime-impl entry lacks a matching declaration.

**`headers/statistics.phdl` — the distribution declarations (DVC-02).** The
distributions a `tol` clause on a `param` may name. Each distribution carries
two spellings, absolute and relative, distinguished by the keyed-argument name
at the use site — `gauss(sigma = …)` (absolute) vs `gauss(sigma_rel = …)`
(relative). Declarations only: sampling is `dv-gradients`' scope, not this
feature's.

| Declaration | Argument spelling | Computes |
|---|---|---|
| `gauss(sigma: Real) -> Real` | `sigma = …` | a Gaussian draw with absolute std-dev `sigma` |
| `gauss(sigma_rel: Real) -> Real` | `sigma_rel = …` | a Gaussian draw with std-dev `sigma_rel · nominal` |
| `uniform(half: Real) -> Real` | `half = …` | a uniform draw on `[−half, +half]` |
| `uniform(rel: Real) -> Real` | `rel = …` | a uniform draw on `[−rel·nominal, +rel·nominal]` |

A `tol` clause naming a distribution not in this table fails loud at
elaboration (Part II §11.1).

**`headers/constraints.phdl` — the helper declarations (DVC-09).** The
helpers a `var`/`target` expression in a `constraint` block may call, each
carrying its **pointwise/reduced class** and the **analysis it belongs to**
in a `@quantity(class, analysis)` marker the elaborator reads (design §C2:
the class is a property of the *declaration*, never inferred from a use site).
Pointwise helpers exist at every evaluation point and lower to a per-point
margin kernel; reduced helpers exist once per completed analysis and are
computed host-side (Part VIII §7). The schema declaration is
`extern attribute quantity { class: String, analysis: String }`.

| Declaration | Class | Analysis | Computes |
|---|---|---|---|
| `ac_gain(x: Real) -> Real` | pointwise | ac | magnitude of the probed quantity at the current analysis point |
| `ac_phase(x: Real) -> Real` | pointwise | ac | phase of the probed quantity at the current analysis point |
| `ac_unity_gain_freq(x: Real) -> Real` | reduced | ac | the frequency at which the probed magnitude crosses unity |
| `ac_phase_margin(x: Real) -> Real` | reduced | ac | the loop's phase margin, reduced over the sweep |
| `ac_gain_margin(x: Real) -> Real` | reduced | ac | the loop's gain margin, reduced over the sweep |

A `var`/`target` calling a helper not in this table fails loud at
elaboration (Part II §11.1). A `require` reading a reduced helper fails loud
— `require` is a per-point check (Part I §10.6, Part II §8.1). The class
propagates over arithmetic: any reduced subexpression makes the whole
expression reduced.

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
| `transition` / `table` | ✓ | — |
| `laplace_*` / `zi_*` | — (undeclared; fails loud at elaboration, §2) | — |
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

A `constraint` block (Part I §10.6) is a third column this matrix does not carry: it
admits `require`/`target`/`var`/`cover` statements and analysis-scoped `@` blocks, the
math catalog, and the declared helpers of §6.2 — but no contribution, no force, no
assignment, and no runtime operator.

Measurement from a host uses the result objects (Part VIII §7), not the analog
`V`/`I` access functions.
