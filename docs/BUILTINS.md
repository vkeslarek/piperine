# PHDL builtins reference

Everything a PHDL source file can call or invoke without declaring it
itself: built-in math functions, analog operators, `$`-system functions,
diagnostic tasks, `@`-event kinds, and the always-in-scope prelude/stdlib
(`headers/*.phdl`). This is the exhaustive, per-item reference; for the
grammar shapes see `docs/BNF-AMS.md`, for how these lower to IR see
`docs/CODEGEN-IR.md`, for the extensibility mechanism behind analog
operators/syscalls/events see `docs/EXTENSIBILITY.md`.

## 1. Math functions

Ordinary calls in expression position (`sin(x)`, `pow(a, b)`, …). All
operate on `Real`. Implemented in
`piperine-codegen/src/codegen/cranelift_helpers.rs` (`emit_math`,
`is_builtin_math`) and symbolically differentiated in
`piperine-codegen/src/codegen/ir_emit.rs` (`diff_call`) for the Jacobian.

| Function | Arity | Computes | d/dx (chain rule w.r.t. arg 0, `u'`) |
|---|---|---|---|
| `exp(x)` | 1 | eˣ | `exp(u)·u'` |
| `ln(x)` | 1 | natural log | `u'/u` |
| `log(x)` | 1 | natural log (alias of `ln`) | `u'/u` |
| `log10(x)` | 1 | base-10 log | `u'/(u·ln10)` |
| `sqrt(x)` | 1 | square root | `u'/(2·sqrt(u))` |
| `abs(x)` | 1 | absolute value | `sign(u)·u'` |
| `sin(x)` | 1 | sine | `cos(u)·u'` |
| `cos(x)` | 1 | cosine | `-sin(u)·u'` |
| `tan(x)` | 1 | tangent | `u'/cos(u)²` |
| `asin(x)` | 1 | arcsine | `u'/sqrt(1-u²)` |
| `acos(x)` | 1 | arccosine | `-u'/sqrt(1-u²)` |
| `atan(x)` | 1 | arctangent | `u'/(1+u²)` |
| `atan2(y, x)` | 2 | 2-arg arctangent | not differentiated (0) |
| `pow(a, b)` | 2 | `a^b` | `b·pow(a,b-1)·a'` (`b` treated as constant) |
| `min(a, b)` | 2 | minimum | not differentiated (0) |
| `max(a, b)` | 2 | maximum | not differentiated (0) |
| `floor(x)` | 1 | floor | 0 |
| `ceil(x)` | 1 | ceiling | 0 |
| `sinh(x)` | 1 | hyperbolic sine | `cosh(u)·u'` |
| `cosh(x)` | 1 | hyperbolic cosine | `sinh(u)·u'` |
| `tanh(x)` | 1 | hyperbolic tangent | `(1-tanh(u)²)·u'` |
| `limexp(x)` | 1 | `exp(min(x, 80))` — overflow-clamped exponential, standard SPICE convergence trick | `exp(u)·u'` (same as `exp`) |

`sinh`/`cosh`/`tanh` were added 2026-07-01 (GAPS §I.13) — previously the
NGSPICE headers had to define them as pure PHDL `fn`s.

## 2. Analog operators

Recognized only inside `analog` behavior bodies. Each (except `ac_stim`
and the noise pair) allocates an `IrStateVar` and lowers to
`IrExpr::StateRef(id)`; the actual companion-model math (backward-Euler,
ring buffer, etc.) lives in `piperine-codegen`'s device compiler, not the
frontend. Implemented as a trait+registry
(`piperine-lang/src/lowering/analog_ops.rs`, see `docs/EXTENSIBILITY.md`).

| Operator | Args (defaults) | Semantics |
|---|---|---|
| `ddt(x)` | `x` | Time derivative of `x` (companion model: `(x_new − x_old)/dt`, stamped as a reactive contribution with `alpha = 1/dt`). The only analog operator with a fully faithful device-level implementation today. |
| `idt(x, ic=0.0)` | `x`, initial condition | Time integral of `x`: `state = state_prev + x·dt`. |
| `idtmod(x, ic=0.0, modulus=1.0)` | `x`, ic, modulus | Same as `idt` but wraps the result modulo `modulus` (phase accumulators, etc.). |
| `ddx(x, node)` | `x`, a bare node/branch identifier | Partial derivative of `x` with respect to the potential/flow at `node`. |
| `delay(x, dt=0.0)` / `absdelay(x, dt=0.0)` | `x`, delay | Ideal time delay of `x` by `dt` seconds. Both spellings are the same operator. |
| `transition(x, td=0.0, tr=0.0, tf=0.0, tol=0.0)` | signal, delay, rise, fall, tolerance | Smooths a piecewise/digital-like `x` into a continuous waveform. |
| `slew(x, rise=0.0, fall=0.0)` | signal, rise rate, fall rate | Slew-rate limits `x` — output changes no faster than `rise`/`fall` per second. |
| `laplace_np(x, num, den)` / `laplace_zp` / `laplace_pm` / `laplace_nm` / `laplace_npm` | signal, numerator coeff array, denominator coeff array | Continuous-time linear filter `H(s) = num(s)/den(s)`. The suffix selects the coefficient convention (mirrors Verilog-A's `laplace_nd/zd/zp/np` family): `np` = negative powers of s, `zp` = zero/pole form, `pm`/`nm`/`npm` = pole-magnitude / normalized variants. One Rust struct (`Laplace{variant}`) backs all five. |
| `zi_zd(x, num, den, sample_dt)` / `zi_zp` / `zi_nd` / `zi_np` | signal, numerator, denominator, sample period | Discrete-time (Z-transform) filter `H(z) = num(z)/den(z)` sampled every `sample_dt`. Suffix selects direct/pole and normalized/unnormalized form, same naming convention as Verilog-A's `zi_*` family. |
| `ac_stim(mag=1.0, phase=0.0)` | magnitude, phase (degrees) | AC small-signal stimulus for `.ac` analysis only; contributes nothing in DC/transient. Lowers straight to `IrExpr::AcStim`, not a state var. |
| `white_noise(psd, "label")` | PSD [unit²/Hz], optional label | Flat-spectrum noise source across the enclosing contribution's terminals. Must appear inside a `<+` RHS — extracted by a pre-pass (`scan_noise`) before lowering; evaluates to `0.0` as a plain expression. |
| `flicker_noise(psd, exponent=1.0, "label")` | PSD coefficient, 1/f exponent, optional label | 1/f^exponent ("pink") noise source, same injection mechanism as `white_noise`. |

**Known fidelity gaps** (tracked in `docs/GAPS.md`):
device-level (JIT) support today only covers `ddt`/`idt`/`idtmod` fully;
`ddx`/`delay`/`transition`/`slew`/`laplace_*`/`zi_*` are recognized in the
IR but rejected fail-loud at `ir_analog_to_device` compile time
(`CodegenError::Unsupported`) pending their own companion-model work.

## 3. `$`-system functions (expression position)

Looked up case-insensitively (`$` stripped, lower-cased). Implemented as a
trait+registry (`piperine-lang/src/lowering/syscalls.rs`).

| Syntax | Args | Returns |
|---|---|---|
| `$temperature` | — | Circuit/instance temperature (Kelvin). |
| `$vt` / `$vt(temp)` | optional temp override | Thermal voltage `kT/q`, at `temp` if given, else simulation temperature. |
| `$abstime` | — | Current absolute simulation time (transient analysis). |
| `$mfactor` | — | The instance multiplicity factor `m`. |
| `$xposition` / `$yposition` / `$angle` | — | Instance layout placement/rotation (layout-aware parameters). |
| `$simparam("key", default=0.0)` | key, default | Named simulator parameter (e.g. `"gmin"`), falling back to `default` if unset. |
| `$param_given("name")` | param name | True if `name` was explicitly passed by the caller (vs. defaulted). **Frontend/IR only** — rejected fail-loud at device-compile time today (GAPS §A.15/§N.2); not yet wired to real per-instance metadata. |
| `$port_connected("name")` | port name | True if the named port has an external connection. |
| `$limit(x, "kind", ...args)` | value, limiter name, limiter args | Newton-iteration convergence limiter/damping function (e.g. `"pnjlim"`, `"fetlim"`), as in Verilog-A `$limit`. |
| `$analysis("kind")` | analysis kind string (default `"dc"`) | True if the current analysis matches `kind` (`"dc"`, `"tran"`, `"ac"`, `"noise"`). |
| `$random` / `$random(seed)` | optional seed | Uniform pseudo-random value. |
| `$dist_uniform(...)`, `$dist_normal(...)`, `$dist_exponential(...)`, … | distribution-specific | Whole `$dist_*` family routes through the same handler as `$random`; the syscall name itself (lowercased) is threaded through as the `kind`. |

## 4. Diagnostic / simulator-control tasks (statement position)

Statement-form `$name(...)`, not expressions. Implemented in
`piperine-lang/src/lowering/stmt.rs`.

| Syntax | Args | Effect |
|---|---|---|
| `$bound_step(dt)` | max timestep (default `0.0`) | Caps the solver's next timestep to at most `dt`. |
| `$finish` / `$stop` | — | Terminates the simulation. Both spellings lower to the same `Finish` node. |
| `$discontinuity(n)` | order (integer literal, default `0`) | Signals a discontinuity of order `n` (0 = value, 1 = derivative, …), forcing the solver to break/re-solve the timestep. |
| `$display(...)` / `$write(...)` / `$strobe(...)` / `$monitor(...)` | optional format string, then value args | Print to the simulator log at `Info` severity. All four collapse to the same IR node — the timing distinctions Verilog-A gives `$strobe`/`$monitor` aren't preserved at this level. |
| `$warning` / `$warn` | same | `Warning` severity. |
| `$error` | same | `Error` severity. |
| `$fatal` | same | `Fatal` severity (does not automatically chain to `$finish` in the IR — a fatal message doesn't by itself stop the run). |
| any other `$name(...)` | same | Falls back to `Info` severity. |

## 5. `@`-event kinds

Implemented as a trait+registry (`piperine-lang/src/elab/event.rs`
`EventKind`/`EventRegistry` — the reference implementation for the pattern
described in `docs/EXTENSIBILITY.md`). Enforcement: `analog` blocks reject
digital-edge events; `digital` blocks reject analog-crossing events.

| Form | Class | Semantics |
|---|---|---|
| `@ posedge(sig)` | digital edge | Fires on a low→high transition of digital `sig`. Analog-only illegal. |
| `@ negedge(sig)` | digital edge | Fires on a high→low transition. |
| `@ change(sig)` | digital edge | Fires on any value change of `sig`. |
| `@ cross(expr)` | analog crossing | Fires when `expr` crosses zero. Digital-only illegal. **Note:** the grammar allows an optional direction argument but the lowering currently always emits `dir: 0` (either direction) regardless — a direction-selective `cross` is not yet wired through. |
| `@ above(expr)` | analog crossing | Fires when `expr` becomes greater than its threshold (one-shot level crossing, as Verilog-A's `above()`). |
| `@ initial` | — | Fires once at the start of the analysis. |
| `@ final` | — | Fires once at the end of the analysis. |
| `@ timer(period)` | — | Periodic timer firing every `period` seconds. |
| `A @ B` (event OR) | — | Composite: fires whenever either combined spec fires. Recurses through validation. |

**Gap to know about:** an unrecognized `@name(...)` does not error — it
silently lowers to `@initial`. Treat any typo'd or as-yet-unregistered
event name as a silent no-op-at-the-wrong-time bug, not a compile error,
until this is tightened.

## 6. Prelude / stdlib (`headers/*.phdl`)

Always in scope — injected by the resolver into every compilation unit,
no `use` required (except `constants`/`disciplines`, which need an
explicit `use piperine::constants;` / `use piperine::disciplines;`).

### Disciplines (`disciplines.phdl`, `prelude.phdl`)

- `Ground` — implicit reference potential, always in scope (`prelude.phdl`).
- `Bit` — digital, `storage Boolean`.
- `Logic` — digital, 4-state (`storage Quad`, `resolve tri`).
- `DDiscrete` — digital, `storage Quad`.
- `Electrical` — `potential v (V)`, `flow i (A)`.
- `Voltage` — potential-only `v (V)` (signal-flow).
- `Current` — potential-only `i (A)` (signal-flow).
- `Magnetic` — `potential mmf (A·turn)`, `flow phi (Wb)`.
- `Thermal` — `potential temp (K)`, `flow pwr (W)`.
- `Kinematic` — `potential pos (m)`, `flow f (N)`.
- `KinematicV` — `potential vel (m/s)`, `flow f (N)`.
- `Rotational` — `potential theta (rad)`, `flow tau (N·m)`.
- `RotationalOmega` — `potential omega (rad/s)`, `flow tau (N·m)`.

### Constants (`constants.phdl`)

Math: `M_E`, `M_LOG2E`, `M_LOG10E`, `M_LN2`, `M_LN10`, `M_PI`, `M_TWO_PI`,
`M_PI_2`, `M_PI_4`, `M_1_PI`, `M_2_PI`, `M_2_SQRTPI`, `M_SQRT2`, `M_SQRT1_2`.

Physical (SI, NIST-2010-equivalent): `P_Q` (elementary charge, C), `P_C`
(speed of light, m/s), `P_K` (Boltzmann, J/K), `P_H` (Planck, J·s),
`P_EPS0` (vacuum permittivity, F/m), `P_U0` (vacuum permeability, H/m —
derived as `4e-7·M_PI`), `P_CELSIUS0` (273.15, 0 °C in Kelvin).

### Capabilities (`capabilities.phdl`)

- `Type`, `Net` — root marker capabilities (no methods; special-cased by
  the elaborator for bound validation).
- `Add`, `Sub`, `Mul`, `Div` — one operator method each (`add`, `sub`, …).
- `Eq` — `eq(self, Self) -> Boolean`.
- `Ord : Eq` — `lt(self, Self) -> Boolean`, requires `Eq`.
- `BitAnd`, `BitOr`, `BitXor`, `Not` — bitwise operator methods.
- `Number : Add, Sub, Mul` — composite capability with a default method
  `double(self) -> Self { self.add(self) }`.

### Collections (`collections.phdl`)

- `map<T, U>(xs: T[N], f: fn(T) -> U) -> U[N]` — element-wise map.
- `reduce<T>(xs: T[N], op: fn(T, T) -> T) -> T` — divide-and-conquer fold
  (base case `N==1`; otherwise splits in half and recombines with `op`).

## Cross-references

- `docs/piperine-hdl-spec.md` — language spec narrative; introduces `ddt`,
  `idt`, `$bound_step`, and `@posedge` by example but doesn't enumerate
  the full builtin surface (this doc supersedes it for that purpose).
- `docs/BNF-AMS.md` — grammar-conformance checklist for these constructs.
- `docs/CODEGEN-IR.md` — how each construct lowers to `IrExpr`/`IrStmt`
  and what the device compiler does with it.
- `docs/GAPS.md` — fidelity gaps (which of the above are frontend-only
  vs. fully device-compiled).
- `docs/EXTENSIBILITY.md` — the trait+registry mechanism behind analog
  operators, syscalls, and event kinds, and how to add a new one.
