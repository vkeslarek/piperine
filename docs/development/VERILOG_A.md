# Verilog-A Language Reference

Complete feature map of Verilog-A (IEEE 1364-2001 analog subset + Verilog-A LRM 2.4).
Purpose: decide what Piperine must support in `.ppr` language + ngspice backend to compile
and simulate arbitrary real-world VA models without manual adaptation.

Each section includes:
- Syntax and semantics
- **OpenVAF support** — what the compiler handles (sourced from `hir_def/src/builtin.rs`,
  `hir_def/src/expr.rs`, integration tests)
- **ngspice / OSDI notes** — what the simulator needs to handle on its side

---

## 1. Module declaration

```verilog
module diode(anode, cathode);
    inout  anode, cathode;
    electrical anode, cathode;
    // ...
endmodule
```

Port directions: `input`, `output`, `inout`. All analog ports are typically `inout`.

Port disciplines declared inline:

```verilog
module filter(in, out);
    input  in;
    output out;
    electrical in, out;
```

Or using named discipline on port declaration:

```verilog
module therm_res(p, n, dT);
    inout p, n;
    inout dT;
    electrical p, n;
    thermal   dT;
```

**OpenVAF:** ✅ Full module parsing. Multiple ports, multiple disciplines, inout/input/output.  
**ngspice:** Module name becomes OSDI model name; `N<inst> <nodes> <model> [params]`.

---

## 2. Disciplines and natures

Disciplines classify nets into physical domains. In practice most VA uses
`disciplines.vams` from the standard library.

```verilog
`include "disciplines.vams"
```

This provides:
- `electrical` — potential = Voltage (V), flow = Current (A)
- `thermal`    — potential = Temperature (K), flow = Power (W)
- `rotational` — potential = Angle (rad), flow = Torque (N·m)
- `translational`, `fluidic`, `magnetic` — less common

Access function names come from the nature:

| Discipline | Access fn (potential) | Access fn (flow) |
|------------|----------------------|-----------------|
| `electrical` | `V(...)` | `I(...)` |
| `thermal` | `Temp(...)` | `Pwr(...)` |
| `rotational` | `Theta(...)` | `Tau(...)` |

**OpenVAF:** ✅ Reads discipline/nature declarations, resolves access functions from nature.
`flow()` and `potential()` builtin functions for generic access.  
**ngspice:** Only `electrical` is natively simulated. Other domains work via coupled nets
as floating potential/flow — rarely needed for standard device models.

---

## 3. Parameters

### 3.1 Types and declaration

```verilog
parameter real    is   = 1e-14;         // real, no constraint
parameter integer n    = 1;             // integer
parameter string  type = "nmos";        // string
```

### 3.2 Range constraints

```verilog
parameter real r  = 1e3  from [0:inf];     // closed lower, open upper
parameter real vj = 1.0  from [0.2:2];     // closed both ends
parameter real g  = 0.01 from (0:inf);     // open lower (strictly positive)
parameter real q  = 0.0  from [-inf:0];    // must be ≤ 0
```

Bounds: `[lo:hi]` closed, `(lo:hi)` open, `[lo:hi)` half-open.
`inf` and `-inf` are valid.

```verilog
parameter real gm = 0.01 from (0:inf) exclude 0;   // exclude specific value
```

Multiple exclude clauses and ranges:

```verilog
parameter real x = 1.0 from [-inf:inf] exclude 0 exclude [-1:1];
```

**OpenVAF:** ✅ Parses and enforces `from` ranges and `exclude`. Checked at compile time
for constant-folded values; runtime check emitted into OSDI for computed overrides.

### 3.3 `localparam`

Cannot be overridden from outside. Computed constants:

```verilog
localparam real tau = r * c;
localparam real vt0 = `P_K * 300.0 / `P_Q;
```

**OpenVAF:** ✅ `is_local` flag in item tree; excluded from instance/model parameter lists.

### 3.4 `aliasparam`

Alternate name for an existing parameter (SPICE `.model` parameter compatibility):

```verilog
parameter real resistance = 1e3;
aliasparam r = resistance;    // .r(500) is accepted as .resistance(500)
```

**OpenVAF:** ✅ `AliasParamId` in nameres; resolves at elaboration.

### 3.5 Attributes on parameters

Metadata visible to tools, not compiled:

```verilog
(*desc="Saturation current", units="A", type="model"*) parameter real is = 1e-14;
```

Common keys: `desc`, `units`, `type` (`"model"` vs `"instance"`), `group`.

**OpenVAF:** ✅ Parses `(* ... *)` attributes; passes `type` to OSDI descriptor to
distinguish model-card vs instance-card parameters.  
**ngspice:** Uses model/instance split from OSDI descriptor.

---

## 4. Variables

```verilog
real   vd, vt, id;          // real (double-precision float)
integer count, flag;         // 32-bit signed integer
string label;                // character string (limited use)
```

Arrays:

```verilog
real   coeff[0:7];           // fixed-size real array
integer idx[0:3];
```

Variables are local to the `analog` block scope or function scope.
They hold values between evaluation *within* a time step (not across time steps
— use `idt` or named states for that).

**OpenVAF:** ✅ `real`, `integer`, `string`. Arrays: ✅ (used in `noise_table` inline form).
No dynamic arrays.

---

## 5. Analog block

```verilog
analog begin
    // contribution statements, assignments, control flow
end
```

Multiple `analog` blocks in one module are merged into one:

```verilog
analog I(p, n) <+ V(p, n) / R;              // single-statement form
analog begin                                  // block form
    I(p, n) <+ V(p, n) / R;
end
```

**OpenVAF:** ✅ Both forms. Multiple `analog` blocks per module merged.

---

## 6. Branch declarations

```verilog
branch (p, n)  br_pn;    // between two nets
branch (p)     br_p;     // net to implicit ground
branch (dT)    br_sht;   // thermal branch (self-heating)
```

Named branches allow cleaner access and can be probed by name:

```verilog
I(br_pn) <+ V(br_pn) / R;
```

**OpenVAF:** ✅ Named branches, two-node and one-node (to ground) forms.

---

## 7. Contribution statements

The `<+` operator adds to the residual of a branch quantity. All `<+`
statements to the same branch are summed by the solver (KCL/KVL).

```verilog
// current contribution: inject id into branch (anode→cathode)
I(anode, cathode) <+ id;

// voltage contribution: set voltage source
V(p, n) <+ vsrc;

// using named branch
I(br_pn) <+ V(br_pn) * G;

// thermal domain
Pwr(br_sht) <+ power_dissipated;

// port flow (sense element — zero current)
I(<probe_port>) <+ 0;
```

**OpenVAF:** ✅ All forms. Resistive and reactive contributions distinguished for Jacobian.

---

## 8. Access functions

Access functions probe a branch quantity without contributing to it (when used
on the right-hand side of `=` or in expressions).

```verilog
// electrical
V(a, b)     // voltage across (a,b)
V(a)        // voltage from a to ground
I(a, b)     // current into node a through (a,b) branch — USE NAMED BRANCH
I(br_name)  // current through named branch

// port flow probe
I(<port>)   // current entering port (sense without contribution)

// thermal
Temp(br_sht)   // temperature difference
Pwr(br_sht)    // power flow

// generic (discipline-agnostic)
potential(br)  // potential quantity for any discipline
flow(br)       // flow quantity for any discipline
```

**OpenVAF:** ✅ `V()`, `I()`, `Temp()`, `Pwr()`, `Theta()`, `Tau()`, `potential()`,
`flow()`. Port flow probe `I(<p>)` ✅.

---

## 9. Mathematical operators

Standard arithmetic: `+`, `-`, `*`, `/`, `**` (power), `%` (modulo for integers).  
Comparison: `<`, `>`, `<=`, `>=`, `==`, `!=`.  
Logical: `&&`, `||`, `!`.  
Bitwise (integer only): `&`, `|`, `^`, `~`, `<<`, `>>`.  
Ternary: `cond ? a : b`.  
Unary: `-`, `+`, `!`, `~`.

**OpenVAF:** ✅ All of the above.

---

## 10. Built-in math functions

All functions take and return `real` unless noted. Called without `$` prefix (unlike system functions).

### Trigonometric

```verilog
sin(x)   cos(x)   tan(x)
asin(x)  acos(x)  atan(x)
atan2(y, x)       // four-quadrant arctangent
sinh(x)  cosh(x)  tanh(x)
asinh(x) acosh(x) atanh(x)
```

### Exponential / logarithmic

```verilog
exp(x)             // e^x
ln(x)              // natural log
log(x)             // log base 10 (NOTE: Verilog-A `log` is log10, not ln!)
log10(x)           // same as log(x) — explicit alias
pow(x, y)          // x^y (real exponent)
sqrt(x)
hypot(x, y)        // sqrt(x²+y²)
limexp(x)          // exp with convergence limiting: safe for large x
```

### Rounding / comparison

```verilog
floor(x)   ceil(x)
abs(x)              // real or integer
min(x, y)   max(x, y)   // real or integer (type-matched)
```

### Integer

```verilog
clog2(n)   // ceiling log base 2 (integer)
```

**OpenVAF:** ✅ All of the above. `log` = log10 (not ln) per LRM — OpenVAF follows LRM.

---

## 11. System functions — simulation state

```verilog
$temperature         // circuit temperature in Celsius (read-only)
$abstime             // current simulation time in seconds
$vt                  // thermal voltage k·T/q at current temperature
$vt(T)               // thermal voltage at explicit T (in Celsius)
$mfactor             // instance multiplicity from M= parameter
```

Layout position (rarely used in SPICE models):

```verilog
$xposition   $yposition   $angle   $hflip   $vflip
```

**OpenVAF:** ✅ `$temperature`, `$abstime`, `$vt`, `$mfactor` (as `ParamSysFun`),
layout functions.  
**ngspice:** Provides temp, abstime, vt via OSDI callbacks. `$mfactor` → M parameter.

---

## 12. Simulator parameter access

```verilog
$simparam("gmin", 1e-12)        // read simulator option; second arg is default
$simparam("tnom")               // nominal temperature (K) — no default
$simparam("minr", 1e-3)
$simparam("scale", 1.0)
$simparam("shrink", 1.0)
```

Common keys: `"gmin"`, `"tnom"`, `"scale"`, `"shrink"`, `"minr"`.

**OpenVAF:** ✅ Two overloads: with and without default. Compiles to OSDI SimParam call.  
**ngspice:** Passes simulator options to OSDI callback. All common keys supported.

```verilog
$simprobe("device", "param")    // probe device operating point parameter
```

**OpenVAF:** ✅ Two overloads (with/without default). Used for self-heating, cross-coupling.

---

## 13. Convergence control

```verilog
$limit(V(p,n), "pnjlim", vt, vcrit)    // built-in PN junction limiter
$limit(expr, user_fn, arg...)           // call user-defined limit function
$limit(expr)                             // identity (no-op limit)

$discontinuity()           // signal discontinuity of arbitrary order
$discontinuity(0)          // hard discontinuity (value jump)
$discontinuity(1)          // slope discontinuity

$bound_step(dt_max)        // tell simulator: don't take steps > dt_max
```

`$limit` is critical for BJT/diode models — prevents Newton step from taking
the junction voltage so far negative that `exp(v/vt)` underflows.

**OpenVAF:** ✅ `$limit` (3 overloads: builtin function name string, user function, no-arg),
`$discontinuity` (0 and 1 arg), `$bound_step`.  
**ngspice:** Built-in limiters (`pnjlim`, `fetlim`, `bjt_icvce`, etc.) provided by ngspice.

---

## 14. Analysis type predicates

```verilog
$analysis("dc")      // true during DC/OP sweep
$analysis("ac")      // true during AC small-signal
$analysis("tran")    // true during transient
$analysis("noise")   // true during noise analysis
$analysis("static")  // true during DC or OP
$analysis("ic")      // true during initial condition setup
$analysis("nodeset") // true during nodeset phase
```

Can combine multiple strings (OR logic):

```verilog
if ($analysis("ac", "noise")) begin
    I(p,n) <+ small_signal_contribution;
end
```

Used to provide separate DC and AC models within one module.

**OpenVAF:** ✅ `analysis` builtin; varargs string matching.  
**ngspice:** Calls OSDI `eval` with analysis-type flags; OSDI header defines which bits map to which analysis.

AC stimulus:

```verilog
$ac_stim()                              // unit AC stimulus (mag=1, phase=0)
$ac_stim("vsource")                     // named stimulus
$ac_stim("vsource", magnitude)          // with magnitude
$ac_stim("vsource", magnitude, phase)   // with magnitude and phase
```

**OpenVAF:** ✅ Four overloads.

---

## 15. Analog events

Analog events trigger procedural statements at specific simulation moments.

### 15.1 `@(initial_step)`

Fires once at the beginning of each analysis. Used for initialization.

```verilog
analog begin
    @(initial_step) begin
        v_prev = 0.0;
        count  = 0;
    end
end
```

Can be scoped to specific analyses:

```verilog
@(initial_step("tran"))    // only during transient
@(initial_step("ac", "dc"))
```

**OpenVAF:** ✅

### 15.2 `@(final_step)`

Fires once at the end of each analysis. Used for cleanup or final output.

```verilog
analog begin
    @(final_step) begin
        $display("done, count=%d", count);
    end
end
```

**OpenVAF:** ✅

### 15.3 `@(cross(expr, dir))` — zero-crossing event

Fires when `expr` crosses zero. `dir`: `+1` (rising), `-1` (falling), `0` (both).

```verilog
@(cross(V(a) - Vth, +1))       // V(a) rising through Vth
    count = count + 1;

@(cross(V(a) - Vhi, -1, 1e-9)) // falling, with 1 ns time tolerance
@(cross(V(a), 0, 1e-9, 1e-3))  // both directions, time+expr tolerance
```

**OpenVAF:** ❌ Not implemented. `Event` enum is `#[non_exhaustive]` with only
`InitialStep`/`FinalStep`. `cross` is parsed by LRM but missing from OpenVAF HIR.

### 15.4 `@(above(expr))` — threshold event

Fires once when `expr` becomes positive. Does not re-arm until it goes negative.

```verilog
@(above(V(out) - 0.5))   vout_high = 1;
```

**OpenVAF:** ❌ Not implemented.

### 15.5 `@(timer(start, period))` — periodic timer

```verilog
@(timer(0.0, 1e-9))     // fires at t=0, t=1ns, t=2ns, ...
    sample = V(in);
```

**OpenVAF:** ❌ Not implemented.

> **Summary:** only `initial_step` and `final_step` events work with OpenVAF.
> `cross`, `above`, and `timer` require simulator-level event scheduling and are
> not yet lowered to OSDI by OpenVAF.

---

## 16. Control flow in analog blocks

### 16.1 `if/else`

```verilog
if (V(g,s) > Vth) begin
    Id = Id_sat;
end else begin
    Id = Id_lin;
end
```

**OpenVAF:** ✅

### 16.2 `case`

```verilog
case (flag)
    0: I(p,n) <+ 0;
    1: I(p,n) <+ Imax;
    default: I(p,n) <+ Imax / 2;
endcase
```

**OpenVAF:** ✅

### 16.3 `for`

```verilog
for (k = 0; k < N; k = k + 1) begin
    sum = sum + coeff[k] * pow(Vg, k);
end
```

**OpenVAF:** ✅

### 16.4 `while`

```verilog
while (abs(err) > tol) begin
    // Newton iteration
    err = err - f / dfdx;
end
```

**OpenVAF:** ✅

### 16.5 `repeat`

```verilog
repeat (8) begin
    val = val * 2;
end
```

**OpenVAF:** ✅ (lowered to `for` internally)

---

## 17. Analog operators — time domain

### 17.1 `ddt` — time derivative

```verilog
I(c) <+ C * ddt(V(c));          // capacitor: I = C · dV/dt
I(c) <+ ddt(Q);                 // charge-based: I = dQ/dt (preferred for accuracy)

// with tolerance hint (helps convergence)
ddt(V(c), 1e-12)                // absolute tolerance
ddt(V(c), Charge)               // nature-based tolerance
```

**OpenVAF:** ✅ Three overloads (no-tol, real-tol, nature-tol).

### 17.2 `idt` — time integral

```verilog
V(ind) <+ L * ddt(I(ind));      // inductor via ddt (most common)
// or equivalently:
V(ind) <+ idt(V(ind), 0.0) / L; // idt form (less common)

idt(expr)                         // integral from 0, no IC
idt(expr, ic)                     // initial condition at t=0
idt(expr, ic, assert)             // assert != 0 enables IC
idt(expr, ic, assert, tol)        // with tolerance
idt(expr, ic, assert, Nature)     // nature-based tolerance
```

**OpenVAF:** ✅ Five overloads.

### 17.3 `idtmod` — modulo integral

Integral that wraps at a modulus. Used for phase accumulation:

```verilog
phase <+ idtmod(2*`M_PI*freq, 0.0, 2*`M_PI, 0.0);
```

```verilog
idtmod(expr, ic)
idtmod(expr, ic, modulus)
idtmod(expr, ic, modulus, offset)
idtmod(expr, ic, modulus, offset, tol)
idtmod(expr, ic, modulus, offset, Nature)
```

**OpenVAF:** ✅ Six overloads.

### 17.4 `ddx` — partial derivative

```verilog
ddx(expr, V(node))       // ∂expr/∂V(node) — for convergence aids
ddx(expr, I(branch))
ddx(expr, Temp(br))      // thermal domain
```

Used to manually supply Jacobian contributions:

```verilog
I(out) <+ Gm * V(in);
I(out) <+ ddx(Gm * V(in), V(in));   // explicit conductance (redundant but sometimes needed)
```

**OpenVAF:** ✅ Four overloads (wrt Temp, wrt V two-node, wrt V one-node, wrt I).

---

## 18. Waveform shaping operators

### 18.1 `transition` — digital edge smoothing

Takes an integer/boolean signal and produces a smoothly ramped analog waveform.

```verilog
transition(digital_sig)                         // default rise/fall from `default_transition
transition(digital_sig, delay)                   // propagation delay
transition(digital_sig, delay, rise_time)        // same rise and fall
transition(digital_sig, delay, rise_time, fall_time)
transition(digital_sig, delay, rise_time, fall_time, tol)
```

**OpenVAF:** ✅ Five overloads. Requires `integer` input (not `real`).

### 18.2 `slew` — slew rate limiter

Limits the rate of change of a signal:

```verilog
slew(expr)                         // unlimited (no-op)
slew(expr, max_pos_rate)           // same limit both directions
slew(expr, max_pos_rate, max_neg_rate)   // separate pos/neg rate limits
```

**OpenVAF:** ✅ Three overloads.

### 18.3 `absdelay` — transport delay

Pure delay (unlike `transition` which shapes the waveform):

```verilog
absdelay(expr, delay)              // delay_time in seconds
absdelay(expr, delay, max_delay)   // max_delay pre-allocates memory
```

**OpenVAF:** ✅ Two overloads.

### 18.4 `last_crossing`

Returns the time of the most recent zero-crossing of `expr`:

```verilog
last_crossing(expr)          // last crossing, either direction
last_crossing(expr, dir)     // dir: +1 rising, -1 falling, 0 either
```

**OpenVAF:** ✅ Two overloads. (Simulator-state function — requires simulator support.)  
**ngspice:** Available via OSDI `SimParam` cross-coupling.

---

## 19. Laplace-domain filters

Insert a linear continuous-time transfer function into the signal path.
The simulator handles DC, AC, and transient analysis consistently.

All have the form `laplace_XX(input, num_array, den_array [, tol_or_nature])`.

```
XX legend:
  nd — numerator/denominator polynomial coefficient arrays
  np — numerator coefficients / pole pairs {re, im}
  zd — zero pairs {re, im} / denominator coefficients
  zp — zero pairs {re, im} / pole pairs {re, im}
```

```verilog
// H(s) = 1 / (τ·s + 1)   — first-order low-pass
V(out) <+ laplace_nd(V(in), {1.0}, {tau, 1.0});

// H(s) = s·τ / (τ·s + 1) — first-order high-pass
V(out) <+ laplace_nd(V(in), {0.0, tau}, {tau, 1.0});

// H(s) = Kp·(s + z) / (s + p) — lead-lag compensator
V(out) <+ laplace_zp(V(in), {{-z, 0.0}}, {{-p, 0.0}});
```

With tolerance:

```verilog
laplace_nd(x, num, den, tol)          // real tolerance
laplace_nd(x, num, den, Nature)       // nature-based tolerance
```

**OpenVAF:** ✅ All four variants (`laplace_nd`, `laplace_np`, `laplace_zd`, `laplace_zp`),
each with 3 overloads (no-tol, real-tol, nature-tol) = 12 total.

---

## 20. z-domain (sampled-data) filters

Discrete-time transfer function — sampled at `tstep`, with optional delay.

```
ZI_XX(input, num_array, den_array, tstep [, delay [, tol_or_nature]])
```

```verilog
// H(z) = 1 — identity (useful as a sample-and-hold)
V(out) <+ zi_nd(V(in), {1.0}, {1.0}, Ts);

// IIR first-order: H(z) = b0 / (1 - a1·z^-1)
V(out) <+ zi_nd(V(in), {b0}, {1.0, -a1}, Ts);
```

```verilog
zi_nd(x, num, den, tstep)
zi_nd(x, num, den, tstep, delay)
zi_nd(x, num, den, tstep, delay, tol)
zi_nd(x, num, den, tstep, delay, Nature)
```

**OpenVAF:** ✅ All four variants (`zi_nd`, `zi_np`, `zi_zd`, `zi_zp`), each with 3
overloads = 12 total.

---

## 21. Noise sources

Noise contributions are specially handled by the simulator for noise analysis.
They do not affect DC or transient (unless the simulator has a noise injection mode).

```verilog
// White (thermal) noise — PSD = 4kTR
I(r) <+ white_noise(4 * `P_K * $temperature * R);
I(r) <+ white_noise(4 * `P_K * $temperature * R, "thermal");  // with label

// Flicker (1/f) noise — PSD = KF * |I|^AF / f
I(mos) <+ flicker_noise(KF * pow(abs(Id), AF), 1.0, "flicker");
// flicker_noise(power, exponent [, label])

// Noise from inline table — (freq, PSD) interleaved real array
I(n) <+ noise_table({1e3, 1e-18,  1e6, 1e-20,  1e9, 1e-22}, "table");
// noise_table(array [, label])
// noise_table(filename_string [, label])

// Same but frequency axis is log10
I(n) <+ noise_table_log({3, 1e-18,  6, 1e-20,  9, 1e-22}, "log_table");
```

**OpenVAF:** ✅ `white_noise` (with/without label), `flicker_noise` (with/without label),
`noise_table` (inline array or file, with/without label), `noise_table_log` (same).
All 8 overloads present.  
**ngspice:** Noise contributions flow through OSDI `noise` function pointer per analysis
frequency point.

---

## 22. Randomization

Integer random distributions (for Monte Carlo models):

```verilog
$random()                   // uniform integer, no seed
$random(seed)               // seed is inout integer variable

$arandom()                  // analog random (instance-deterministic)
$arandom(seed)
$arandom(seed, "label")     // labeled for reproducibility
$arandom(param_seed)        // const seed from parameter
$arandom(param_seed, "label")
```

Real distributions (`$rdist_*` — return real):

```verilog
$rdist_uniform(seed, lo, hi)
$rdist_normal(seed, mean, sigma)
$rdist_erlang(seed, k, mean)
$rdist_exponential(seed, mean)
$rdist_poisson(seed, mean)
$rdist_chi_square(seed, dof)
$rdist_t(seed, dof)
```

Each has 4 overloads: `(seed, ...)`, `(const_seed, ...)`, `(seed, ..., "name")`, `(const_seed, ..., "name")`.

Integer distributions (`$dist_*` — return integer):

```verilog
$dist_uniform(seed, lo, hi)
$dist_normal(seed, mean, sigma)
$dist_erlang(seed, k, mean)
$dist_exponential(seed, mean)
$dist_poisson(seed, mean)
$dist_chi_square(seed, dof)
$dist_t(seed, dof)
```

**OpenVAF:** ✅ All above. 4 overloads × 7 rdist + 4 overloads × 7 dist + random + arandom
(5 overloads each).

---

## 23. Introspection — parameter and port queries

```verilog
$param_given(param_name)    // 1 if parameter was explicitly set by user, 0 if default
$port_connected(port_name)  // 1 if port is connected in the schematic
```

Used to implement optional ports (e.g., a temperature port that is only active
when connected):

```verilog
analog begin
    if ($port_connected(dT)) begin
        tdev = $temperature + Temp(dT);
    end else begin
        tdev = $temperature;
    end
end
```

**OpenVAF:** ✅ Both builtins.

---

## 24. Analog functions

User-defined functions called from the analog block. Return a `real` or `integer`.
No contribution statements inside.

```verilog
analog function real pnjlim_manual;
    input v, vold, vt, vcrit;
    real  v, vold, vt, vcrit;
    real  vte, vlim;
    begin
        vte   = vt * 1.02;
        vlim  = vt * ln(vt / (`M_SQRT2 * vte));
        if (v > vcrit && abs(v - vold) > 2.0 * vte) begin
            if (vold > 0.0)
                v = vold + 2.0 * vte * ((v - vold) / abs(v - vold));
            else
                v = vte;
        end
        pnjlim_manual = v;
    end
endfunction
```

Rules:
- Arguments: `input` (by value) only. No `output` or `inout` args in analog functions.
- Body: `begin...end` block with ordinary variable assignments and `if/for/while/case`.
- No `<+` contributions allowed.
- Recursion: allowed in LRM; OpenVAF generates non-recursive IR (bounded depth).

**OpenVAF:** ✅ Full analog function support including recursion.

---

## 25. Display and file I/O

### Console output (side effects — called only at `initial_step`/`final_step`)

```verilog
$display(fmt, args...)    // print with newline
$write(fmt, args...)      // print without newline
$strobe(fmt, args...)     // print at end of time step
$monitor(fmt, args...)    // print whenever args change
$debug(fmt, args...)      // vendor-specific debug output
```

Format specifiers: `%d` (integer), `%g` (real), `%e`, `%f`, `%s` (string), `%m` (hierarchy).

### Severity

```verilog
$info("msg")       // informational
$warning("msg")    // warning
$error("msg")      // non-fatal error
$fatal(0, "msg")   // fatal, exit with code 0
$finish()          // terminate simulation normally
$finish(1)         // terminate with diagnostics
$stop()            // pause (interactive) or terminate
```

**OpenVAF:** ✅ All above. `$fatal`, `$finish`, `$stop` added in OpenVAF-Reloaded.

### File I/O

```verilog
fd = $fopen("filename")             // open for write
fd = $fopen("filename", "r")        // mode string: "r", "w", "a", "rb", etc.
$fclose(fd)
$fdisplay(fd, fmt, args...)
$fwrite(fd, fmt, args...)
$fstrobe(fd, fmt, args...)
$fmonitor(fd, fmt, args...)
$fgets(str_var, fd)                  // read line
$fscanf(fd, fmt, var...)             // formatted read
$fseek(fd, offset, whence)
$ftell(fd)
$rewind(fd)
$fflush(fd)          // flush all if no arg
$fflush()
$feof(fd)
$ferror(fd, str_var)
```

### String operations

```verilog
$swrite(str_var, fmt, args...)     // format into string variable
$sformat(str_var, fmt, args...)    // same (alias)
$sscanf(str_var, fmt, var...)      // scan from string
```

**OpenVAF:** ✅ All file I/O and string functions.

---

## 26. Table model (lookup table device)

```verilog
$table_model(x, "file.tbl", "L")          // 1-D, linear interp
$table_model(x, y, "file.tbl", "L,L")     // 2-D, both linear
$table_model(x, y, z, "file.tbl", "C,C,C") // 3-D, cubic spline
```

Extrapolation modes: `"L"` (linear extrapolation), `"C"` (cubic), `"A"` (Akima).

**OpenVAF:** ❌ `// TODO TABLE_MODEL` comment in `builtin.rs`. Not yet implemented.  
**ngspice:** Supports `.table` within B-source; OSDI `$table_model` not tested.

---

## 27. Compiler directives

```verilog
`include "disciplines.vams"    // include another file
`include "constants.vams"      // physical constants

`define MACRO_NAME value        // text substitution macro
`define MACRO_NAME(args) body   // parameterized macro
`undef MACRO_NAME

`ifdef MACRO_NAME
    // ...
`elsif OTHER_MACRO
    // ...
`else
    // ...
`endif

`ifndef MACRO_NAME
    // ...
`endif

`default_transition 1e-9    // default rise/fall time for transition()
`resetall                   // reset all compiler state to defaults
```

**OpenVAF:** ✅ `include`, `define`/`undef`, `ifdef`/`elsif`/`else`/`endif`, `ifndef`.
`default_transition` ✅. `resetall` ✅.

---

## 28. Standard library files

### `disciplines.vams`

Predefined disciplines. Always `include` at top:

```verilog
`include "disciplines.vams"
```

Provides: `electrical`, `thermal`, `rotational`, `translational`, `fluidic`,
`magnetic`, `logic`, `ddiscrete`.

**OpenVAF:** ✅ Bundled; resolved from include search path.

### `constants.vams`

Physical and mathematical constants:

```verilog
`include "constants.vams"
```

| Constant | Value | Meaning |
|----------|-------|---------|
| `` `M_PI `` | 3.14159… | π |
| `` `M_TWO_PI `` | 6.28318… | 2π |
| `` `M_PI_2 `` | 1.5707… | π/2 |
| `` `M_SQRT2 `` | 1.41421… | √2 |
| `` `M_LN2 `` | 0.69314… | ln(2) |
| `` `M_LN10 `` | 2.30258… | ln(10) |
| `` `M_E `` | 2.71828… | e |
| `` `P_K `` | 1.38064e-23 | Boltzmann (J/K) |
| `` `P_Q `` | 1.60218e-19 | Electron charge (C) |
| `` `P_C `` | 2.99792e8 | Speed of light (m/s) |
| `` `P_H `` | 6.62607e-34 | Planck (J·s) |
| `` `P_EPS0 `` | 8.85419e-12 | Vacuum permittivity (F/m) |
| `` `P_U0 `` | 1.25664e-6 | Vacuum permeability (H/m) |
| `` `P_CELSIUS0 `` | 273.15 | 0°C in Kelvin |

**OpenVAF:** ✅ Bundled; all constants available.

---

## 29. Instance attributes `(* ... *)`

Metadata on module items — not compiled into logic, visible to tools and OSDI:

```verilog
(*desc="Saturation current", units="A", type="model"*) parameter real is = 1e-14;
(*desc="Dummy", type="instance"*) parameter string dummy = "abc";
```

Common attributes:
- `desc` — human-readable description
- `units` — physical unit string
- `type` — `"model"` (goes on `.model` card) vs `"instance"` (on element line)
- `group` — grouping for GUI tools

**OpenVAF:** ✅ Parsed; `type` attribute propagated to OSDI model/instance param split.

---

## 30. `$test_plusargs` and `$value_plusargs`

Command-line simulation argument access:

```verilog
if ($test_plusargs("verbose"))
    $display("debug mode");

if ($value_plusargs("rth=%s", str_val))
    rth = str_to_real(str_val);
```

**OpenVAF:** ✅ Both. Useful for parameterizing models at runtime without recompilation.

---

## 31. Node alias functions

Debugging / tool integration — map a node to a string name:

```verilog
$analog_node_alias(node, "alias_name");
$analog_port_alias(port, "alias_name");
```

**OpenVAF:** ✅ Both. Rarely needed in production models.

---

## 32. Multiplicity — `$mfactor`

The `M=` instance parameter in SPICE multiplies a device, modeling N parallel
copies. In VA, `$mfactor` lets the model scale currents correctly:

```verilog
analog begin
    Id = $mfactor * Ids_formula(...);
    I(d, s) <+ Id;
end
```

The simulator divides node voltages (potentials) by M — the model handles
current (flow) scaling explicitly.

**OpenVAF:** ✅ As `ParamSysFun::mfactor`. Compiled to OSDI instance scale factor.  
**ngspice:** M parameter handled at OSDI layer.

---

## Summary — OpenVAF support matrix

| Feature | OpenVAF |
|---------|---------|
| Module, ports, disciplines | ✅ |
| `parameter real/integer/string` | ✅ |
| `from [lo:hi]` / `exclude` ranges | ✅ |
| `localparam` | ✅ |
| `aliasparam` | ✅ |
| `(* attributes *)` | ✅ |
| `real`, `integer`, `string` variables | ✅ |
| Fixed-size arrays | ✅ |
| `analog begin...end` | ✅ |
| Branch declarations | ✅ |
| `<+` contributions (resistive + reactive) | ✅ |
| `V()`, `I()`, `Temp()`, port flow | ✅ |
| `potential()`, `flow()` generic access | ✅ |
| All math operators | ✅ |
| All trig / exp / log builtins | ✅ |
| `$temperature`, `$abstime`, `$vt`, `$mfactor` | ✅ |
| `$simparam`, `$simprobe` | ✅ |
| `$limit`, `$discontinuity`, `$bound_step` | ✅ |
| `$analysis(...)` predicate | ✅ |
| `$ac_stim` | ✅ |
| `@(initial_step)` | ✅ |
| `@(final_step)` | ✅ |
| `@(cross(...))` | ❌ not implemented |
| `@(above(...))` | ❌ not implemented |
| `@(timer(...))` | ❌ not implemented |
| `if`, `case`, `for`, `while`, `repeat` | ✅ |
| `ddt`, `idt`, `idtmod` | ✅ (3–6 overloads each) |
| `ddx` | ✅ |
| `transition`, `slew`, `absdelay` | ✅ |
| `last_crossing` | ✅ |
| `laplace_nd/np/zd/zp` | ✅ (12 overloads total) |
| `zi_nd/np/zd/zp` | ✅ (12 overloads total) |
| `white_noise`, `flicker_noise` | ✅ |
| `noise_table`, `noise_table_log` | ✅ (inline + file) |
| `$random`, `$arandom` | ✅ |
| `$rdist_*` (7 distributions) | ✅ |
| `$dist_*` (7 distributions) | ✅ |
| `$param_given`, `$port_connected` | ✅ |
| `$display`, `$write`, `$strobe`, etc. | ✅ |
| `$fatal`, `$finish`, `$stop` | ✅ |
| File I/O (`$fopen`, `$fclose`, `$fdisplay`, …) | ✅ |
| String ops (`$swrite`, `$sformat`, `$sscanf`) | ✅ |
| Analog functions | ✅ |
| `$table_model` | ❌ TODO in OpenVAF |
| `include`, `define`, `ifdef` | ✅ |
| `disciplines.vams`, `constants.vams` | ✅ (bundled) |
| `$test_plusargs`, `$value_plusargs` | ✅ |
| `$analog_node_alias`, `$analog_port_alias` | ✅ |

---

## What Piperine `.ppr` must transparently pass through

For a device VA file to compile via `piperine-openvaf` with zero adaptation:

1. **`disciplines.vams` + `constants.vams`** — bundled by OpenVAF, no action needed.
2. **All parameters with `from`/`exclude`** — parsed by OpenVAF, transparent.
3. **`aliasparam`** — transparent to OpenVAF; Piperine only sees the `piperine_ngspice`
   side if the `.ppr` instantiates the device.
4. **`@(initial_step)`/`@(final_step)`** — compiled to OSDI init callback; ngspice calls it.
5. **`@(cross)` / `@(above)` / `@(timer)`** — ❌ blocked by OpenVAF. Models using these
   will fail to compile. Most production models (BSIM, HiCuM, PSP) avoid them.
6. **`$table_model`** — ❌ blocked by OpenVAF. Models using lookup tables need a workaround
   (inline polynomial approximation or precomputed array).
7. **ngspice OSDI version** — ngspice ≥ 43 handles OSDI 0.3; ngspice 44+ handles 0.4.
   OpenVAF-Reloaded emits OSDI 0.4 by default. Verify ngspice version at runtime.
