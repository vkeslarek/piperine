# Verilog-AMS Built-in Tasks and Functions

Reference for VA/AMS system tasks and analog functions to implement in Piperine's runtime, JIT, and IR.

---

## 1. Display and Diagnostics

### `$display`, `$write`, `$strobe`, `$monitor`

Standard output tasks, identical to SystemVerilog. Called from `analog initial` or procedural blocks.

```verilog
$display("format string", arg1, arg2, ...);
$write("no newline");
$strobe("end-of-timestep");
$monitor("continuous watch");
```

| Task | Timing | Newline |
|------|--------|---------|
| `$display` | Immediate | Yes |
| `$write` | Immediate | No |
| `$strobe` | End of time step | Yes |
| `$monitor` | Whenever args change | Yes |

**IR mapping**: `IrAnalogStmt::Display { format, args }` — already in IR. JIT currently ignores it.  
**Runtime implementation**: in debug builds, route to stderr; no-op in release.

### `$warning`, `$error`, `$fatal`, `$info`

Severity-tagged messages introduced in Verilog-AMS 2.4:

```verilog
$warning("v(drain) = %g exceeds supply", V(drain));
$error("model parameter out of range");
$fatal(1, "irrecoverable condition");
```

**IR mapping**: extend `IrAnalogStmt::Display` with an optional `severity: Option<IrSeverity>` field (`Info`, `Warning`, `Error`, `Fatal`).  
`$fatal` must call into the solver's abort hook — model the exit code as arg 0.

---

## 2. Simulation State Queries

These return values from the current simulation context. In VA they appear as zero-argument functions in expressions.

### `$temperature`

Returns the current ambient temperature in Kelvin.

```verilog
parameter real tnom = 27; // nominal temperature
real mu0 = mu0_nom * pow($temperature / (tnom + 273.15), 1.5);
```

**IR mapping**: `IrExpr::SimQuery(SimQuery::Temperature)` — new variant.  
**JIT**: thread as a `f64` field in the parameter array or a separate scalar pointer.  
**Solver**: inject from `SimInfo.temperature` (default 300.15 K).

### `$vt`

Thermal voltage `k*T/q`. Optional temperature argument: `$vt(T)` computes `k*T/q` for explicit T.

```verilog
real id = is * (exp(vgs / (n * $vt)) - 1.0);
real id2 = is * (exp(vgs / (n * $vt(temp + 273.15))) - 1.0);
```

**IR mapping**: `IrExpr::SimQuery(SimQuery::Vt { temp_arg: Option<Box<IrExpr>> })`.  
`$vt` with no arg lowers to `SimQuery::Temperature * (k/q)` as a constant fold; `$vt(expr)` lowers to `expr * (k/q)`.  
k/q ≈ 8.617333e-5 eV/K = 8.617333e-5 V/K.

### `$simparam`

Queries a named simulation parameter by string. Used by models to adapt to the analysis type.

```verilog
real freq = $simparam("freq", 0.0);    // AC analysis frequency; 0 outside AC
real scale = $simparam("scale", 1.0);  // instance scaling
```

**IR mapping**: `IrExpr::SimQuery(SimQuery::Simparam { key: String, default: Box<IrExpr> })`.  
**Solver**: maintain a `HashMap<String, f64>` in `SimInfo`. Populate with `"freq"`, `"scale"`, `"tnom"` etc. per analysis type.

### `$param_given`

Returns 1.0 if a parameter was explicitly set by the user, 0.0 if it uses its default.

```verilog
if ($param_given(w))
    weff = w;
else
    weff = w_default;
```

**IR mapping**: `IrExpr::SimQuery(SimQuery::ParamGiven { param: String })`.  
**Lowering**: during elaboration, record which parameters were set by the user. Store a bitset in `AnalogIrInstance.given_params: HashSet<String>`. At JIT compile time, fold `$param_given(x)` to `Literal(1.0)` or `Literal(0.0)` — no runtime query needed.

### `$port_connected`

Returns 1.0 if a port is actually connected (vs. floating). Used for optional ports.

```verilog
if ($port_connected(sub))
    I(sub) <+ ...;
```

**IR mapping**: `IrExpr::SimQuery(SimQuery::PortConnected { port: String })`.  
**Lowering**: fold at elaboration time — a port is connected iff its `PortBinding.net_id != NodeId::GND_FLOATING` (design a sentinel for unconnected ports).

### `$abstime`

Current simulation time (transient) or 0 (DC/AC).

```verilog
I(p,n) <+ pulse_amplitude * (($abstime > t_on) ? 1.0 : 0.0);
```

**IR mapping**: `IrExpr::SimQuery(SimQuery::Abstime)`.  
**JIT**: add a `time: f64` field alongside `node_voltages` and `params` in the compiled function ABI.  
**Solver**: inject from `SimInfo.time`.

### `$mfactor`

Multiplicity factor — number of parallel instances. Normally 1; set via `.m=N` in SPICE netlist.

```verilog
I(p,n) <+ $mfactor * id_core;
```

**IR mapping**: fold into `AnalogIrInstance.parameters` as a special `"$mfactor"` key. Default 1.0. Available in IR as `IrExpr::Var("$mfactor")`.

---

## 3. Differential and Integral Operators

### `ddt(x)` — time derivative

Computes `dx/dt`. In transient analysis returns the finite-difference approximation; in DC returns 0; in AC returns `jω*x`.

```verilog
I(p,n) <+ C * ddt(V(p,n));  // capacitor
```

**IR mapping**: `IrAnalogFn::Ddt` — already in IR.  
**State variable allocation**: each `ddt(expr)` introduces a state variable slot. During lowering, allocate `IrStateVar { id: StateVarId, expr: IrExpr }` and store in `IrAnalogBlock.state_vars`. The JIT emits `load_residual_react` and `load_jacobian_react` for the reactive stamp.

**Analysis-dependent behavior**:
- DC: `ddt(x) = 0` — reactive stamps are zero, only resistive stamps apply.
- Transient: `ddt(x) ≈ (x - x_prev) / h` via implicit Euler or trapezoidal; reactive stamp = `C/h`.
- AC: `ddt(x)` → `jω * X` where X is the AC phasor of x. Reactive Jacobian entries are the conductance stamp multiplied by `jω`.

### `idt(x, ic)` — time integral

Computes `∫x dt` with initial condition `ic`.

```verilog
real q;
V(p,n) <+ idt(I(p,n) / C, 0.0);  // integrating capacitor
```

**IR mapping**: `IrAnalogFn::Idt { ic: Option<Box<IrExpr>> }` — extend current IR.  
**State variable**: same as `ddt` — one slot per `idt` call. Backward Euler: `x_new = x_prev + h * integrand`.  
**DC**: use the `ic` initial condition as the value (or 0.0 if none).

### `idtmod(x, ic, modulus)` — modular integrator

Like `idt` but wraps the result modulo `modulus`. Used for ring oscillator phase.

**IR mapping**: `IrAnalogFn::IdtMod { ic: Box<IrExpr>, modulus: Box<IrExpr> }`.

---

## 4. Noise Contributions

### `white_noise(pwr)` and `white_noise(pwr, "name")`

Contributes white (frequency-independent) noise current with power spectral density `pwr` A²/Hz.

```verilog
I(p,n) <+ white_noise(4 * `P_K * $temperature * gds, "thermal");
```

### `flicker_noise(pwr, exp)` and `flicker_noise(pwr, exp, "name")`

Contributes 1/f noise with PSD `pwr / f^exp` A²/Hz.

```verilog
I(p,n) <+ flicker_noise(kf * ids^af / Cox, af, "flicker");
```

**IR mapping**: both already in `IrAnalogFn`. Currently emitted as 0.0 in resistual/jacobian.  
**Separate noise pass**: noise analysis requires a dedicated `load_noise(freq)` stamp:
- Collect all `white_noise` and `flicker_noise` contributions.
- For each, record the branch plus/minus nodes and the PSD expression.
- During noise analysis: evaluate PSD at current operating point and stamp into the noise correlation matrix.
- State in `IrAnalogBlock`: add `noise_sources: Vec<IrNoiseSource>` with `{ branch, kind: IrNoise }`.

```rust
pub enum IrNoise {
    White  { psd: IrExpr },
    Flicker { psd: IrExpr, exponent: IrExpr },
}
```

---

## 5. Limiting Functions

### `$limit(x, "function", args...)` and `$limit(x, prev_x, "pnjlim", vt, vcrit)`

Applies a circuit-solver-specific limiting function to help Newton-Raphson converge on nonlinear devices.

```verilog
vbe = $limit(vbe, "pnjlim", vt, vcrit);
```

**Built-in limiting functions**:
- `"pnjlim"`: PN junction limiting. Limits exponential argument to prevent overflow.
  ```
  pnjlim(vnew, vold, vt, vcrit):
    if vnew > vcrit and |vnew - vold| > 2*vt:
        if vold > 0:
            dv = log((vnew - vold) / vt + 1) * vt
            return vold + dv
        return vt * log(vnew/vt)
    return vnew
  ```
- `"fetlim"`: FET VGS limiting.

**IR mapping**: `IrAnalogFn::Limit { func: String, args: Vec<IrExpr> }`.  
**Implementation**: `$limit` must see the previous Newton step value (`vold`). This requires a state slot — same mechanism as `ddt`. Add `IrStateVar` for each `$limit` site, update after convergence.

### `limexp(x)`

Exponentially-limited exp: `exp(min(x, 80))`. Already in IR as `IrAnalogFn::LimExp`. Already JIT-compiled.

---

## 6. Simulation Control

### `$bound_step(dt)`

Requests that the transient solver use a step no larger than `dt` for the next time point.

```verilog
@ (cross(V(p,n) - threshold, 0)) $bound_step(1e-12);
```

**IR mapping**: `IrAnalogStmt::BoundStep { expr: IrExpr }`.  
**Runtime**: `bound_step_hint()` on `AnalogDevice` already returns this. The JIT device's `bound_step_hint()` must evaluate the `IrExpr` and return the minimum across all active `$bound_step` calls.

### `$finish`

Terminates simulation immediately.

**IR mapping**: `IrAnalogStmt::Finish`.  
**Runtime**: signal abort via a shared `AtomicBool` in `SimInfo`; solver checks after each Newton iteration.

---

## 7. Event Detection

These functions return 1.0 at the moment an event occurs, 0.0 otherwise. They implicitly request that the solver insert a time point exactly at the event.

### `cross(expr, dir)`

Detects zero-crossing of `expr`. `dir`: 1 = rising, -1 = falling, 0 = both.

```verilog
if (cross(V(clk) - 0.9, 1))
    Q <= D;
```

**IR mapping**: `IrAnalogFn::Cross` — already in IR. Currently emits 0.0.  
**Runtime implementation**:
1. State variable tracks previous value of `expr`.
2. After each accepted time point, check if a zero crossing occurred between `t_prev` and `t`.
3. If so, request event insertion via `bound_step_hint` and bisect to `t_event`.
4. Return 1.0 at `t_event`.

### `above(expr)`

Returns 1.0 if `expr > 0`, with event insertion at the crossing moment.

**IR mapping**: `IrAnalogFn::Above` — already in IR.

### `timer(period)` and `timer(start, period)`

Fires at regular intervals.

**IR mapping**: `IrAnalogFn::Timer` — already in IR.  
**Runtime**: state tracks `t_next`; `bound_step_hint` returns `t_next - t_current`.

---

## 8. Waveform Shaping

### `transition(x, td, tr, tf)`

Produces a smoothed version of a piecewise-constant signal `x` with rise/fall transitions.

```verilog
V(out) <+ transition(ctrl > 0.5 ? vdd : 0, 0, rise_t, fall_t);
```

**IR mapping**: `IrAnalogFn::Transition { td: Box<IrExpr>, tr: Box<IrExpr>, tf: Box<IrExpr> }`.  
**State**: stores a waveform queue of pending transitions. Complex to implement correctly; lower to a piecewise-linear model backed by state slots.

### `slew(x, rising_rate, falling_rate)`

Rate-limits a signal.

**IR mapping**: `IrAnalogFn::Slew`.  
**State**: one state slot for current output value.

### `ac_stim(discipline, mag, phase)`

AC analysis stimulus. Returns `mag * exp(j*phase)` at the `ac_stim` frequency, 0.0 in DC/transient.

**IR mapping**: `IrAnalogFn::AcStim { magnitude: Box<IrExpr>, phase: Box<IrExpr> }`.  
**AC analysis**: the AC small-signal source; requires complex phasor evaluation path.

---

## 9. Laplace Domain Filters

Continuous-time filters defined by their Laplace transfer function. Each introduces state via a state-space expansion.

| Function | Transfer function | Notes |
|----------|-----------------|-------|
| `laplace_zp(x, zr, zc, pr, pc)` | H(s) = `∏(s−z_i) / ∏(s−p_i)` | Zeros + poles |
| `laplace_nd(x, num, den)` | H(s) = N(s)/D(s) | Numerator/denominator coefficient arrays |
| `laplace_np(x, num, pr, pc)` | H(s) = N(s)/∏(s−p_i) | |
| `laplace_zd(x, zr, zc, den)` | H(s) = ∏(s−z_i)/D(s) | |

**IR mapping**: `IrAnalogFn::LaplaceFilter { kind: LaplaceKind, signal: Box<IrExpr>, coeffs: Vec<IrExpr> }`.  
**State**: expanded to state-space form (companion model). Each pole → one state variable. Complex to implement; Wave D priority.

---

## 10. Random Distributions

Used in Monte Carlo and statistical simulations.

| Function | Distribution |
|----------|-------------|
| `$random` | Uniform integer |
| `$random(seed)` | Seeded uniform integer |
| `$dist_uniform(seed, start, end)` | Uniform real |
| `$dist_normal(seed, mean, std)` | Normal (Gaussian) |
| `$dist_exponential(seed, mean)` | Exponential |
| `$dist_poisson(seed, mean)` | Poisson |
| `$dist_chi_square(seed, dof)` | Chi-squared |
| `$dist_t(seed, dof)` | Student-t |
| `$dist_erlang(seed, k, mean)` | Erlang |

**IR mapping**: `IrExpr::SimQuery(SimQuery::Random { dist: IrDist, seed: Option<Box<IrExpr>>, params: Vec<IrExpr> })`.  
**Runtime**: maintain a per-seed `SmallRng` in `SimInfo`. Not Jacobian-differentiated (treat as constant within Newton iteration).

---

## 11. Implementation Priority

### Wave A — Core analog simulation (required for basic device models)

| Feature | Notes |
|---------|-------|
| `$temperature` | Required by virtually all semiconductor models |
| `$vt` | MOSFET, BJT, diode |
| `ddt` | Capacitor, inductor |
| `white_noise`, `flicker_noise` | Noise analysis |
| `limexp` | Already done |

### Wave B — Convergence and control

| Feature | Notes |
|---------|-------|
| `$limit` / `pnjlim` | BJT, diode convergence |
| `$param_given` | Conditional model parameters |
| `$bound_step` | `bound_step_hint()` already in device trait |
| `$simparam` | Analysis-type detection |

### Wave C — Events and mixed-signal

| Feature | Notes |
|---------|-------|
| `cross` | AMS zero-crossing |
| `above` | Level detection |
| `timer` | Periodic events |
| `transition` | D2A interfaces |
| `slew` | Ramp generators |

### Wave D — Advanced features

| Feature | Notes |
|---------|-------|
| `idt`, `idtmod` | Integrators |
| `ac_stim` | AC sources |
| `laplace_*` | Filter models |
| `$port_connected` | Optional ports |
| `$mfactor` | Parallel scaling |

### Wave E — Statistical

| Feature | Notes |
|---------|-------|
| `$random`, `$dist_*` | Monte Carlo |

---

## 12. IR Changes Required

Additions to `IrExpr` and `IrAnalogStmt` to support all built-ins:

```rust
// New IrExpr variant
IrExpr::SimQuery(SimQuery),

pub enum SimQuery {
    Temperature,
    Vt { temp_arg: Option<Box<IrExpr>> },
    Abstime,
    Mfactor,
    ParamGiven { param: String },           // foldable at elaboration
    PortConnected { port: String },         // foldable at elaboration
    Simparam { key: String, default: Box<IrExpr> },
    Random { dist: IrDist, seed: Option<Box<IrExpr>>, params: Vec<IrExpr> },
}

// New IrAnalogStmt variants
IrAnalogStmt::BoundStep { expr: IrExpr },
IrAnalogStmt::Finish,

// Extended IrAnalogBlock
pub struct IrAnalogBlock {
    pub branches:    Vec<IrBranch>,
    pub state_vars:  Vec<IrStateVar>,       // NEW: ddt/idt/limit/event slots
    pub noise_sources: Vec<IrNoiseSource>,  // NEW: white_noise/flicker_noise
    pub statements:  Vec<IrAnalogStmt>,
}

pub struct IrStateVar {
    pub id:      StateVarId,
    pub kind:    IrStateKind,
    pub expr:    IrExpr,    // the argument to ddt/idt/etc.
}

pub enum IrStateKind {
    Ddt,
    Idt { ic: IrExpr },
    LimitPnjlim { vt: IrExpr, vcrit: IrExpr },
    Cross { dir: i8 },
    Timer { period: IrExpr },
}

pub struct IrNoiseSource {
    pub branch: String,
    pub kind:   IrNoise,
}

pub enum IrNoise {
    White   { psd: IrExpr },
    Flicker { psd: IrExpr, exponent: IrExpr },
}
```

---

## 13. JIT ABI Extension for Sim Queries

Current residual/jacobian ABI: `fn(voltages, params, output)`.

Extended ABI for Wave A+B:

```c
void residual(
    const double *node_voltages,
    const double *params,
    const double *state,     // NEW: state_vars[i].value
    double        temperature, // NEW
    double        abstime,     // NEW
    double       *rhs,
    double       *state_next   // NEW: updated state after eval
);
```

Alternative: pack `temperature`, `abstime`, `simparam_map_ptr` into a `SimContext` struct and pass one pointer — cleaner extension point.

```c
typedef struct {
    double temperature;
    double abstime;
    double *simparam;  // indexed by SimparamId enum
    double *state;
    double *state_next;
} SimContext;

void residual(const double *voltages, const double *params,
              const SimContext *ctx, double *rhs);
```

This is the recommended approach — adding one pointer to ABI is forward-compatible as `SimContext` grows.
