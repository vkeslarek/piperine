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