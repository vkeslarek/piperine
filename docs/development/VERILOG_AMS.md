# Verilog-AMS Language Features — Hardware Specification

Reference for evaluating which features to adopt in Piperine's `.ppr` language.
Organized by feature area; each section notes the Verilog-AMS source (LRM 2.4),
the SPICE/OSDI analog, and a rough adoption signal for Piperine.

Scope: **hardware description only** — natures, disciplines, modules, analog
blocks, parameters, events, mixed-signal interop. Testbench constructs (UDPs,
specify blocks, gate primitives) are omitted.

---

## 1. Natures

A *nature* defines a physical quantity: its units, numerical tolerance, access
function name, and its relationship to an integral or derivative nature.

```verilog
nature Voltage;
    units    = "V";
    access   = V;          // access function used in analog block: V(net)
    abstol   = 1e-6;       // convergence tolerance
    ddt_nature = Charge;   // V is dQ/dt → integral nature is Charge
endnature

nature Charge;
    units    = "C";
    access   = Q;
    abstol   = 1e-14;
    idt_nature = Voltage;  // inverse: Q integrates to V
endnature

nature Current;
    units    = "A";
    access   = I;
    abstol   = 1e-12;
endnature
```

### Nature attributes

| Attribute | Type | Meaning |
|-----------|------|---------|
| `units` | string | Physical unit label (cosmetic) |
| `access` | identifier | Name of the access function in analog blocks |
| `abstol` | real | Absolute convergence tolerance |
| `ddt_nature` | nature | The nature of the time-derivative of this nature |
| `idt_nature` | nature | The nature of the time-integral of this nature |

Nature inheritance — a derived nature overrides individual attributes:

```verilog
nature Rotational_Velocity;
    units  = "rad/s";
    access = Omega;
    abstol = 1e-6;
endnature

nature Rotational_Angle from Rotational_Velocity;
    units  = "rad";
    access = Theta;
    idt_nature = Rotational_Angle;
endnature
```

---

## 2. Disciplines

A *discipline* classifies a net into a physical domain. Every net in
Verilog-AMS has exactly one discipline. The discipline supplies the access
functions for potential (across) and flow (through) quantities.

```verilog
discipline electrical;
    potential Voltage;   // across quantity
    flow      Current;   // through quantity
    domain    continuous;
enddiscipline

discipline logic;
    domain discrete;     // digital — no potential/flow
enddiscipline

discipline thermal;
    potential Temperature;
    flow      Power;
    domain    continuous;
enddiscipline

discipline rotational;
    potential Rotational_Angle;
    flow      Rotational_Torque;
    domain    continuous;
enddiscipline
```

### Discipline attributes

| Attribute | Values | Meaning |
|-----------|--------|---------|
| `domain` | `continuous`, `discrete` | Analog vs. digital |
| `potential` | nature | Across quantity (V, Temp, angle, …) |
| `flow` | nature | Through quantity (I, Power, torque, …) |

### Discipline resolution

When two nets of different (but compatible) disciplines connect, the
simulator resolves to one discipline using resolution rules. Two disciplines
are compatible if they share the same `domain` and their natures are
compatible (same or related abstol).

---

## 3. Net declarations with disciplines

```verilog
module amp(in, out, vdd, gnd);
    electrical in, out, vdd, gnd;    // discipline on ports
    electrical mid;                  // internal net

    // implicit ground reference
    ground gnd;
endmodule
```

`ground` declares a net as the reference node (SPICE `0`). Only one ground
per discipline hierarchy is needed; all `ground` nets of the same discipline
are tied together.

### `wreal` — real-valued wire

`wreal` is a special discipline for mixed-signal: a wire carrying a real
floating-point value, used to connect analog blocks to digital blocks
without a full discipline.

```verilog
wreal out_voltage;   // passes a real value, not a true analog net
```

---

## 4. Module structure

```verilog
`include "disciplines.vams"   // standard discipline/nature library

module filter #(parameter real R = 1e3, parameter real C = 100e-9) (in, out);
    input  in;
    output out;
    electrical in, out;

    real v_mid;           // local variable

    analog begin
        // ...
    end
endmodule
```

Ports may have explicit directions (`input`, `output`, `inout`) or inherit
from the connection. Port discipline = the discipline of the net connected
to that port (resolved at instantiation).

---

## 5. Parameters

### Real and integer parameters

```verilog
parameter real    Vth  = 0.026;                     // no range
parameter real    R    = 1e3   from [0:inf);         // R > 0, unbounded above
parameter integer N    = 4     from [1:16];          // integer, 1..16
parameter real    Tnom = 27.0  from (-273.15:inf);   // open lower bound
parameter string  model_name = "default";
```

Range specifiers: `[lo:hi]` (closed), `(lo:hi)` (open), `[lo:hi)` (half-open).
`inf` and `-inf` are legal bounds.

`exclude` blocks a specific value:
```verilog
parameter real gm = 0.01 from [0:inf) exclude 0;  // must be strictly positive
```

### `localparam`

Computed constants — cannot be overridden from outside:
```verilog
localparam real tau = R * C;
```

### `aliasparam`

Exposes an alternate name for an existing parameter (SPICE compatibility):
```verilog
parameter real resistance = 1e3;
aliasparam r = resistance;   // `.r(500)` is the same as `.resistance(500)`
```

### `defparam` (deprecated)

Hierarchical parameter override from outside the module. Deprecated; use
`#(.param(value))` instantiation overrides instead.

---

## 6. Analog block

The `analog` block describes continuous-time behavior. All contribution
statements run every time the simulator evaluates the module.

```verilog
analog begin
    I(res) <+ V(res) / R;        // Ohm's law: through = across / R

    I(cap) <+ C * ddt(V(cap));   // capacitor: I = C · dV/dt
end
```

Multiple `analog` blocks in one module are allowed and are merged.

### 6.1 Contribution statements

```verilog
V(a, b) <+ expr;   // set voltage across branch (a,b)
I(a, b) <+ expr;   // set current into node a through branch
V(n)    <+ expr;   // voltage from n to implicit reference
```

The `<+` operator **adds** to the existing contribution — multiple statements
accumulate. The simulator solves KCL/KVL with all contributions summed.

### 6.2 Named branches

```verilog
branch (p, n) res_branch;    // named branch for clarity
V(res_branch) <+ I(res_branch) * R;
```

### 6.3 Indirect branch assignment (port branch)

Forces a quantity on a branch while solving for the other:

```verilog
// Voltage source: fix V, solve for I
analog V(a, b) <+ vsrc;

// Current-controlled: fix I, solve for V (implicit voltage source idiom)
analog I(probe) <+ 0;   // zero-current sense element
```

The standard idiom for a voltage source in VA:
```verilog
analog V(p, n) <+ dc_value;
```

---

## 7. Analog operators

### 7.1 Time derivative and integral

```verilog
ddt(expr)               // d/dt — time derivative
idt(expr)               // ∫ dt — time integral (from t=0)
idt(expr, ic)           // integral with initial condition
idt(expr, ic, assert)   // integral with IC and a boolean enable
idtmod(expr, ic, modulus, offset)  // modulo integral (phase wrap)
```

`ddt` is the dual of `idt`. The simulator handles both via modified nodal
analysis; the LRM guarantees correct AC and DC semantics.

### 7.2 Laplace-domain analog filters

These insert a linear transfer function into the signal path. The simulator
handles initialization and frequency-domain analysis automatically.

```verilog
laplace_zp(expr, zeros, poles)   // zeros/poles as {re,im} pairs
laplace_zd(expr, zeros, den)     // zeros + denominator coefficients
laplace_np(expr, num, poles)     // numerator coefficients + poles
laplace_nd(expr, num, den)       // numerator + denominator coefficients
```

Example — first-order low-pass filter with fc = 1/(2π·R·C):
```verilog
parameter real tau = R * C;
analog V(out) <+ laplace_nd(V(in), {1.0}, {tau, 1.0});
// H(s) = 1 / (tau·s + 1)
```

### 7.3 z-domain (sampled-data) filters

```verilog
zi_zp(expr, zeros, poles, tstep, delay)
zi_zd(expr, zeros, den,   tstep, delay)
zi_np(expr, num,   poles, tstep, delay)
zi_nd(expr, num,   den,   tstep, delay)
```

### 7.4 Waveform shaping

```verilog
// transition filter — smooths digital edges into analog ramps
transition(expr, delay, rise_time, fall_time)
transition(expr, delay, rise_time)   // fall_time = rise_time

// slew rate limiter
slew(expr, max_pos_rate, max_neg_rate)
slew(expr, max_rate)

// absolute delay (transport delay)
absdelay(expr, delay_time)
absdelay(expr, delay_time, max_delay)  // max_delay for memory allocation
```

### 7.5 Partial derivative

```verilog
ddx(expr, var)    // partial derivative of expr w.r.t. var
// var must be a potential or flow access function result
```

Used in convergence aid / Newton-step control.

---

## 8. Noise sources

Noise contributions use dedicated functions that the simulator handles
specially for noise analysis (`.noise`).

```verilog
// White (thermal) noise — power spectral density = 4·k·T·R
I(r) <+ white_noise(4 * `P_K * $temperature * R, "thermal");

// Flicker (1/f) noise — PSD = KF · I^AF / f
I(mos) <+ flicker_noise(KF * pow(I(mos), AF), 1.0, "flicker");

// Arbitrary noise from table — (freq, PSD) pairs
I(n) <+ noise_table([0, 1e-18,   1e3, 1e-19,   1e6, 1e-20], "table");

// Same but log-frequency spaced
I(n) <+ noise_table_log([0, 1e-18, 3, 1e-19, 6, 1e-20], "log_table");
```

The string argument is a label — appears in noise output by source.

---

## 9. Analog events

Analog events trigger behavior at specific simulation moments. They allow
the analog block to react to crossing conditions without polling.

```verilog
analog begin
    @(initial_step)        // fired once at the start of each analysis
        v_init = 0.0;

    @(final_step)          // fired once at the end of each analysis
        $strobe("done");

    @(cross(V(a) - threshold, +1))   // rising zero-crossing of (V(a) - threshold)
        count = count + 1;

    @(cross(V(a) - Vhi, -1, 1e-9))   // falling crossing, time tolerance 1 ns
        fell = 1;

    @(above(V(a) - Vth, +1))   // triggers once when V(a) exceeds Vth
        exceeded = 1;

    @(timer(0, period))         // fires at t=0 and then every `period`
        sample = V(in);

    @(analysis("ac"))      // true only during AC analysis
        // AC-specific contribution
end
```

### `cross` vs `above`

| Event | Triggers | Re-arms |
|-------|----------|---------|
| `cross(expr, dir)` | on every zero-crossing | yes — fires again next crossing |
| `above(expr, dir)` | once when condition becomes true | no — must go false then true again |

### Analysis type predicates

```verilog
analysis("ac")        // AC small-signal
analysis("dc")        // DC operating point
analysis("tran")      // transient
analysis("noise")     // noise analysis
analysis("static")    // DC or operating point
analysis("ic")        // initial condition setup
analysis("nodeset")   // nodeset phase
```

Can combine: `analysis("ac", "noise")`.

---

## 10. System functions available in analog blocks

### Time and simulation state

```verilog
$realtime          // current simulation time (real)
$abstime           // same — preferred in VA
$temperature       // circuit temperature in Celsius
$vt                // thermal voltage = k·T/q (at $temperature)
$vt(T)             // thermal voltage at explicit temperature T
$mfactor           // instance multiplicity factor (from M= parameter)
```

### Simulator control hints

```verilog
$bound_step(max_dt)       // request simulator limit time step to max_dt
$discontinuity(hint)      // signal a discontinuity; hint = 0..N for order
$limit(expr, func, ...)   // convergence limiting (e.g. $limit(V(g),"pnjlim",...))
$simparam("tnom")         // read simulator parameter (tnom, gmin, scale, …)
$simparam("gmin", default)
```

### Table lookup

```verilog
$table_model(in, "file.tbl", "L")   // 1-D piecewise-linear table from file
$table_model(x, y, "file.tbl", "L,L")  // 2-D table
```

Extrapolation codes: `"L"` (linear), `"C"` (cubic spline), `"A"` (akima spline).

### Math (same as SPICE but via `$`)

```verilog
$sin(x)   $cos(x)   $tan(x)
$asin(x)  $acos(x)  $atan(x)  $atan2(y,x)
$hypot(x,y)
$exp(x)   $log(x)   $log10(x)
$sqrt(x)  $pow(x,y)
$floor(x) $ceil(x)  $round(x)  $int(x)
$abs(x)   $min(x,y) $max(x,y)  $mod(x,y)
$limexp(x)          // exp with convergence limiting: exp(x) for x < threshold
```

### String / display (limited in analog context)

```verilog
$display("fmt", args...)   // print at each eval (noisy — use sparingly)
$strobe("fmt", args...)    // print at end of timestep
$warning("msg")
$error("msg")
$fatal(0, "msg")           // terminate simulation
```

---

## 11. Analog functions

Reusable analog computations. No contribution statements inside — return
a real value.

```verilog
analog function real diode_current;
    input Vd, Is, VT;
    real  Vd, Is, VT;
    begin
        diode_current = Is * ($limexp(Vd / VT) - 1.0);
    end
endfunction
```

Called like a normal expression:
```verilog
I(diode) <+ diode_current(V(a,b), Is, $vt);
```

Restrictions vs. standard Verilog functions:
- No `always` blocks, no event triggers
- No `$display` with side effects during Newton iteration (timing undefined)
- Arguments and return value are `real` or `integer`

---

## 12. Compiler directives

```verilog
`include "disciplines.vams"   // include standard or custom header
`include "constants.vams"     // physical constants

`define BETA 200.0            // text macro
`undef BETA

`ifdef FAST_MODEL
    // ...
`elsif DETAIL_MODEL
    // ...
`else
    // ...
`endif

`timescale 1ns/1ps            // time unit/precision (digital side)

`default_nettype none         // undeclared nets = error
`default_nettype wire         // undeclared nets = wire (default digital)
`default_discipline electrical // undeclared net discipline

`default_transition 1e-9      // default rise/fall for transition() filter

`resetall                     // reset all directives to LRM defaults
```

### Standard constants (`constants.vams`)

```verilog
`M_PI       // π
`M_TWO_PI   // 2π
`M_PI_2     // π/2
`M_SQRT2    // √2
`M_LN2      // ln(2)

`P_K        // Boltzmann constant  1.3806505e-23 J/K
`P_Q        // electron charge     1.602176462e-19 C
`P_C        // speed of light      2.99792458e8 m/s
`P_H        // Planck constant
`P_EPS0     // vacuum permittivity
`P_U0       // vacuum permeability
`P_CELSIUS0 // 0°C in Kelvin = 273.15
```

---

## 13. Connectrules (discipline mismatch resolution)

When nets of different disciplines connect, a *connectmodule* is automatically
inserted by the elaborator to adapt between domains.

```verilog
connectrules myRules;
    connect electrical, logic   with e2l_connect;
    connect logic, electrical   with l2e_connect;
endconnectrules
```

The `connectmodule` itself is an ordinary module with special port disciplines:
```verilog
connectmodule e2l_connect(a, d);
    electrical a;
    logic      d;
    parameter real vhi = 3.3, vlo = 0.0, vth = 1.65;

    analog begin
        d = (V(a) > vth) ? 1 : 0;   // A→D conversion
    end
endconnectmodule
```

Piperine currently has no connectrule concept — all nets are untyped SPICE
nodes. This becomes relevant when typed nets land (see ROADMAP.md backlog).

---

## 14. Generate blocks

Conditional and loop-based module structure at elaboration time.

```verilog
genvar i;
generate
    for (i = 0; i < N; i = i + 1) begin : stage
        filter #(.R(R_arr[i]), .C(C_arr[i])) F (.in(chain[i]), .out(chain[i+1]));
    end
endgenerate

generate
    if (FAST_MODE) begin
        // simplified model
    end else begin
        // full model
    end
endgenerate
```

`genvar` is an elaboration-time integer — not a simulation variable. Array
parameters enable `R_arr[i]` style indexing.

---

## 15. Array parameters and variables

```verilog
parameter real R[0:3] = '{1e3, 2e3, 4e3, 8e3};  // array parameter
real           x[0:7];                             // local real array

analog begin
    x[0] = V(a);
    x[1] = ddt(V(a));
end
```

Array parameters can be passed per-element at instantiation:
```verilog
filter #(.R[0](500), .R[1](1e3)) F(...);
```

---

## 16. Hierarchical net access (`$cmos_*`, `$cmosn`, `$cmosp` — probes)

Verilog-AMS allows reading potentials/flows from named instances via
hierarchical references:

```verilog
V(top.amp.Q1.c)    // access collector voltage of Q1 in submodule amp
I(top.amp.Rc)      // current through Rc
```

This is read-only — hierarchical contribution is not allowed.

---

## 17. Port branches vs. net branches

```verilog
// net branch — between two nets
branch (a, b) B1;
I(B1) <+ V(B1) / R;

// port branch — through a single-port terminal (for flow sensing)
I(<p>)    // current entering port p
V(<p>)    // voltage at port p (to ground)
```

Port branch probes are useful for current-controlled sources.

---

## 18. Multiplicity (`$mfactor`)

The `M=` instance parameter multiplies all currents (flows) by M, modeling
N parallel identical devices without instantiating N copies.

```verilog
analog begin
    I(d, s) <+ $mfactor * Ids(V(g,s), V(d,s));
end
```

The simulator divides voltages and multiplies currents by M automatically if
the module properly uses `$mfactor`.

---

## 19. `$simparam` — read simulator parameters

```verilog
real gmin;
analog begin
    gmin = $simparam("gmin", 1e-12);   // read gmin; default 1e-12 if not set
    // ...
end
```

Common keys: `"gmin"`, `"tnom"`, `"scale"`, `"shrink"`, `"minr"`.

---

## 20. Conditional analog contributions (`if` inside `analog`)

```verilog
analog begin
    if (analysis("ac"))
        I(in)  <+ V(in) * G_ac;    // small-signal conductance
    else
        I(in)  <+ tanh(V(in) * slope) * Imax;  // nonlinear DC
end
```

Also `case`, `for`, `while`, `repeat` — all legal inside `analog begin...end`.
Loop bounds must be static (constant at elaboration) or bounded by simulator
state (not generally required).

---

## 21. Verilog-AMS vs. Verilog-A

Verilog-AMS = Verilog-A + IEEE 1364 Verilog + mixed-signal glue.

| Feature | Verilog-A (analog only) | Verilog-AMS addition |
|---------|------------------------|----------------------|
| Natures + disciplines | ✅ | — |
| `analog` blocks | ✅ | — |
| `always`, `initial` | no | ✅ (digital side) |
| `wreal` | no | ✅ |
| Connect rules + connectmodule | no | ✅ |
| Mixed-mode events (`posedge`, `negedge`) | no | ✅ |
| `generate` | no | ✅ |
| Discrete-event scheduling | no | ✅ |
| Digital net types (`wire`, `reg`, …) | no | ✅ |

OpenVAF / OSDI targets **Verilog-A** (the analog-only subset). Full
Verilog-AMS mixed-signal requires a co-simulator (e.g. Spectre AMS, Xyce).

---

## 22. Standard library files

`disciplines.vams` and `constants.vams` are bundled with every Verilog-AMS
tool. Content:

**`disciplines.vams`** — predefined disciplines: `electrical`, `thermal`,
`rotational`, `translational`, `fluidic`, `magnetic`, `logic`, `ddiscrete`.

**`constants.vams`** — all `\`P_*` and `\`M_*` constants above.

Both are `` `include ``-able in any `.vams` or `.va` file.

---

## Adoption notes for Piperine

| Feature | Status in `.ppr` today | Adoption signal |
|---------|------------------------|-----------------|
| Natures | parsed (inert) | Worth activating for unit checking — see ROADMAP §backlog |
| Disciplines | parsed (inert), `electrical` accepted | Needed for typed nets |
| `ground` declaration | implicit (`gnd` → `0`) | Could keep implicit + add explicit |
| `parameter … from [lo:hi]` | not parsed | Useful for validation |
| `parameter … exclude` | not parsed | Useful for validation |
| `aliasparam` | not parsed | Useful for SPICE compat (`.r` ↔ `.resistance`) |
| `analog` block + contributions | ✅ (VA modules) | Done |
| `ddt`, `idt` | ✅ (VA, lowered to OpenVAF) | Done |
| `laplace_*`, `zi_*` | not in `.ppr`; OpenVAF handles in VA | Can stay in pure VA |
| `transition`, `slew`, `absdelay` | not in `.ppr`; OpenVAF handles in VA | Can stay in pure VA |
| `white_noise`, `flicker_noise` | not in `.ppr`; OpenVAF handles in VA | Can stay in pure VA |
| `@(initial_step)` / `@(final_step)` | not parsed | Useful for VA init |
| `@(cross(...))` | not parsed | Useful for event-driven VA |
| `@(above(...))` | parsed (limited), `always @(step)` adopted | SOA path already works |
| `@(timer(...))` | not parsed | Niche |
| `analysis("ac")` predicate | not parsed | Useful for DC/AC split models |
| `$bound_step` | not parsed | Useful for stiff models |
| `$discontinuity` | not parsed | Useful for switched models |
| `$limit` / `$limexp` | not in `.ppr`; OpenVAF handles in VA | Can stay in pure VA |
| `$table_model` | not parsed | Useful for lookup-table devices |
| `$simparam` | not parsed | Rarely needed at `.ppr` level |
| `$mfactor` | not parsed | Useful if multi-finger devices in structural `.ppr` |
| `localparam` | not parsed | Nice alias for `parameter` with no override |
| `aliasparam` | not parsed | SPICE compat |
| `generate` blocks | not parsed | High value for N-stage filters, arrays |
| Array parameters | not parsed | Needed for `generate` |
| `connectrules` | not parsed | Only needed with full typed nets |
| `wreal` | not parsed | Mixed-signal; future |
| Named branches | not parsed | Useful for current-sensing idioms in VA |
| Port branches `I(<p>)` | not parsed | Useful in VA |
