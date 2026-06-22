# ngspice Expressions & Data Access Reference

Generated from ngspice source at `~/Git/ngspice/src/frontend/` and `src/spicelib/devices/`.

---

## @device[param] — Operating Point Access

**Syntax:** `@<instance>[<param>]`

After any analysis, device operating-point parameters are accessible via `@`. Works in `print`, `plot`, `let`, and `.meas`.

```
print @M1[vth] @M1[id] @M1[gm]
let gain = @M1[gm] / @M1[gds]
plot @Q1[ic] @Q1[vbe]
.meas tran vthval FIND @M1[vth] AT=1n
```

Special forms:
- `@device[all]` — print all available parameters for that instance
- `@model[param]` — access model parameters (e.g., `@nmos_model[vth0]`)
- `@param` — access a circuit `.param` value

---

### R — Resistor

| Parameter | Description |
|-----------|-------------|
| `i` | Current through resistor (A) |
| `p` | Power dissipated (W) |
| `sens_dc` | DC sensitivity |
| `sens_real` | Real part of AC sensitivity |
| `sens_imag` | Imaginary part of AC sensitivity |
| `sens_mag` | AC sensitivity of magnitude |
| `sens_ph` | AC sensitivity of phase |

---

### C — Capacitor

| Parameter | Description |
|-----------|-------------|
| `i` | Current (A) |
| `p` | Instantaneous power (W) |
| `sens_dc` | DC sensitivity |
| `sens_real` | Real part of AC sensitivity |
| `sens_imag` | Imaginary part of AC sensitivity |
| `sens_mag` | AC sensitivity of magnitude |
| `sens_ph` | AC sensitivity of phase |

---

### L — Inductor

| Parameter | Description |
|-----------|-------------|
| `i` | Current through inductor (A) |
| `v` | Terminal voltage (V) |
| `flux` | Flux through inductor (Wb) |
| `p` | Power dissipated (W) |
| `sens_dc` | DC sensitivity |
| `sens_real` | Real part of AC sensitivity |
| `sens_imag` | Imaginary part of AC sensitivity |
| `sens_mag` | AC sensitivity of magnitude |
| `sens_ph` | AC sensitivity of phase |

---

### V — Voltage Source

| Parameter | Description |
|-----------|-------------|
| `i` | Current through source (A) |
| `p` | Instantaneous power (W) |
| `acreal` | AC real part |
| `acimag` | AC imaginary part |

---

### I — Current Source

| Parameter | Description |
|-----------|-------------|
| `v` | Voltage across source (V) |
| `p` | Power supplied (W) |
| `acreal` | AC real part |
| `acimag` | AC imaginary part |

---

### D — Diode

| Parameter | Description |
|-----------|-------------|
| `vd` | Diode voltage (V) |
| `id` | Diode current (A) |
| `gd` | Diode conductance (S) |
| `cd` | Diode capacitance (F) |
| `charge` | Junction capacitor charge (C) |
| `capcur` | Capacitor current (A) |
| `p` | Power dissipated (W) |
| `sens_dc` | DC sensitivity |
| `sens_real` | Real part of AC sensitivity |
| `sens_imag` | Imaginary part of AC sensitivity |
| `sens_mag` | AC sensitivity of magnitude |
| `sens_ph` | AC sensitivity of phase |

---

### Q — BJT (NPN / PNP)

| Parameter | Description |
|-----------|-------------|
| `ic` | Collector current (A) |
| `ib` | Base current (A) |
| `ie` | Emitter current (A) |
| `vbe` | Base–emitter voltage (V) |
| `vbc` | Base–collector voltage (V) |
| `gm` | Small-signal transconductance (S) |
| `gpi` | Input conductance π (S) |
| `gmu` | Conductance μ (S) |
| `gx` | Base to internal base conductance (S) |
| `go` | Output conductance (S) |
| `cpi` | B–E capacitance (F) |
| `cmu` | B–C capacitance (F) |
| `cbx` | Base–collector extrinsic capacitance (F) |
| `csub` | Substrate capacitance (F) |
| `p` | Power dissipation (W) |
| `sens_dc` | DC sensitivity |
| `sens_real` | Real part of AC sensitivity |
| `sens_imag` | Imaginary part of AC sensitivity |
| `sens_mag` | AC sensitivity of magnitude |
| `sens_ph` | AC sensitivity of phase |

---

### J — JFET

| Parameter | Description |
|-----------|-------------|
| `vgs` | Gate–source voltage (V) |
| `vgd` | Gate–drain voltage (V) |
| `ig` | Gate current (A) |
| `id` | Drain current (A) |
| `is` | Source current (A) |
| `igd` | Gate–drain current (A) |
| `gm` | Transconductance (S) |
| `gds` | Drain–source conductance (S) |
| `ggs` | Gate–source conductance (S) |
| `ggd` | Gate–drain conductance (S) |
| `p` | Power dissipated (W) |

---

### M — MOSFET (Level 1/2/3)

Instance operating-point parameters (from `mos1.c` — same for Level 2/3):

| Parameter | Description |
|-----------|-------------|
| `id` | Drain current (A) |
| `is` | Source current (A) |
| `ig` | Gate current (A) |
| `ib` | Bulk current (A) |
| `ibd` | B–D junction current (A) |
| `ibs` | B–S junction current (A) |
| `vgs` | Gate–source voltage (V) |
| `vds` | Drain–source voltage (V) |
| `vbs` | Bulk–source voltage (V) |
| `vbd` | Bulk–drain voltage (V) |
| `von` | Turn-on voltage (V) |
| `vdsat` | Saturation drain voltage (V) |
| `gm` | Transconductance (S) |
| `gds` | Drain–source conductance (S) |
| `gmb` | Bulk–source transconductance (S) |
| `gbd` | Bulk–drain conductance (S) |
| `gbs` | Bulk–source conductance (S) |
| `cbd` | Bulk–drain capacitance (F) |
| `cbs` | Bulk–source capacitance (F) |
| `cgs` | Gate–source capacitance (F) |
| `cgd` | Gate–drain capacitance (F) |
| `cgb` | Gate–bulk capacitance (F) |
| `rs` | Source resistance (Ω) |
| `rd` | Drain resistance (Ω) |
| `cbd0` | Zero-bias B–D junction capacitance (F) |
| `cbs0` | Zero-bias B–S junction capacitance (F) |
| `w` | Width (m) |
| `l` | Length (m) |

---

### M — MOSFET (BSIM4 / Level 14)

BSIM4 adds the following (from `b4.c`):

| Parameter | Description |
|-----------|-------------|
| `vth` | Threshold voltage (V) — alias for `von` |
| `vdsat` | Saturation drain voltage (V) |
| `id` | Drain current (A) |
| `gm` | Transconductance (S) |
| `gds` | Drain–source conductance (S) |
| `gmbs` | Bulk–source transconductance (S) |
| `ibd` | B–D junction current (A) |
| `ibs` | B–S junction current (A) |
| `gbd` | B–D conductance (S) |
| `gbs` | B–S conductance (S) |
| `isub` | Substrate current (A) |
| `igidl` | GIDL current (A) |
| `igisl` | GISL current (A) |
| `igs` | Gate–source current (A) |
| `igd` | Gate–drain current (A) |
| `igb` | Gate–bulk current (A) |
| `vbs` | Bulk–source voltage (V) |
| `vgs` | Gate–source voltage (V) |
| `vds` | Drain–source voltage (V) |
| `cgg` | Gate–gate capacitance (F) |
| `cgs` | Gate–source capacitance (F) |
| `cgd` | Gate–drain capacitance (F) |
| `cgb` | Gate–bulk capacitance (F) |
| `capbd` | B–D capacitance (F) |
| `capbs` | B–S capacitance (F) |
| `qg` | Gate charge (C) |
| `qb` | Bulk charge (C) |
| `qd` | Drain charge (C) |
| `qs` | Source charge (C) |
| `qinv` | Inversion charge (C) |
| `gcrg` | Gate charge–resistance conductance (S) |
| `gtau` | Gate time constant conductance (S) |

---

## Nutmeg Vector Math Functions

Available in `let`, `print`, `plot`, `meas PARAM/EXPR`, and anywhere a vector expression is accepted.
Sourced from `src/maths/cmaths/cmath1.c`, `cmath2.c`, `cmath4.c`, and `src/frontend/parse.c`.

### Complex / Magnitude / Phase

| Function | Aliases | Description |
|----------|---------|-------------|
| `mag(v)` | `magnitude(v)`, `abs(v)` | Magnitude (absolute value; for complex: `√(re²+im²)`) |
| `ph(v)` | `phase(v)` | Phase angle (radians) |
| `cph(v)` | `cphase(v)` | Continuous (unwrapped) phase (radians) |
| `unwrap(v)` | — | Unwrap phase vector (removes 2π jumps) |
| `real(v)` | `re(v)` | Real part |
| `imag(v)` | `im(v)` | Imaginary part |
| `j(v)` | — | Multiply by j (90° rotation): `0 + j·v` |
| `db(v)` | — | `20·log10(mag(v))` — dB magnitude |

---

### Logarithm / Exponential

| Function | Aliases | Description |
|----------|---------|-------------|
| `log(v)` | `ln(v)` | Natural logarithm |
| `log10(v)` | — | Base-10 logarithm |
| `exp(v)` | — | `eᵛ` |
| `sqrt(v)` | — | Square root |

---

### Trigonometric

| Function | Description |
|----------|-------------|
| `sin(v)` | Sine (radians) |
| `cos(v)` | Cosine (radians) |
| `tan(v)` | Tangent (radians) |
| `sinh(v)` | Hyperbolic sine |
| `cosh(v)` | Hyperbolic cosine |
| `tanh(v)` | Hyperbolic tangent |
| `atan(v)` | Arctangent (radians) |

---

### Rounding / Integer

| Function | Aliases | Description |
|----------|---------|-------------|
| `floor(v)` | — | Floor (largest integer ≤ v) |
| `ceil(v)` | — | Ceiling (smallest integer ≥ v) |
| `nint(v)` | — | Nearest integer (round half away from zero) |
| `pos(v)` | — | 1.0 where v > 0, else 0.0 (positive indicator) |

---

### Statistical / Distribution

| Function | Description |
|----------|-------------|
| `mean(v)` | Arithmetic mean of all elements → scalar vector |
| `avg(v)` | Cumulative/running average of each element (same length as v) |
| `norm(v)` | Normalize: divide each element by the maximum absolute value |
| `rnd(v)` | Round each element to nearest integer |
| `sunif(v)` | Uniform random noise scaled by v (seed from `v`'s length) |
| `sgauss(v)` | Gaussian random noise scaled by v |
| `poisson(v)` | Poisson-distributed random values with mean v |
| `exponential(v)` | Exponentially-distributed random values with mean v |
| `sortorder(v)` | Indices that would sort v in ascending order |

---

### Vector Construction / Info

| Function | Description |
|----------|-------------|
| `length(v)` | Number of elements in v → scalar |
| `vector(n)` | Create vector `[0, 1, 2, …, n-1]` |
| `unitvec(n)` | Create vector of `n` ones |
| `vecmin(v)` / `minimum(v)` | Minimum value → scalar |
| `vecmax(v)` / `maximum(v)` | Maximum value → scalar |
| `vecd(v)` | Finite differences: `v[i+1] - v[i]` (length - 1) |

---

### Signal Analysis

These take the current analysis scale (time/frequency) into account.

| Function | Description |
|----------|-------------|
| `deriv(v)` | Numerical derivative dv/dt (or dv/dx for DC) |
| `interpolate(v)` | Interpolate v onto the current scale |
| `group_delay(v)` | Group delay: `-d(phase)/dω` (for AC vectors) |
| `fft(v)` | Fast Fourier Transform (tran → frequency domain) |
| `ifft(v)` | Inverse FFT (frequency domain → time domain) |
| `avg(v)` | Running average (see above) |

Note: `spec` and `psd` are standalone commands, not inline functions.

---

### Arithmetic Binary Operators

| Operator | Description |
|----------|-------------|
| `+` `-` `*` `/` | Element-wise arithmetic |
| `^` or `**` | Exponentiation |
| `%` | Modulo |
| `==` `!=` `<` `<=` `>` `>=` | Comparison (returns 0.0 or 1.0 per element) |
| `&&` `\|\|` `!` | Logical (returns 0.0 or 1.0 per element) |
| `&` `\|` `~` `^` `<<` `>>` | Bitwise (integer elements only) |

---

### Expression Context

```spice
* In .meas PARAM (uses expressions, not vector math):
.meas tran result PARAM='@M1[gm] * 1k'

* In let (full vector expression):
let gain_db = db(v(out) / v(in))

* Access device param in let:
let id = @M1[id]

* Use v(), i() as named-vector accessors:
let vout = v(out)
let ivs = i(v1)         * or v1#branch if i(v1) unavailable

* Vector slice (not yet supported by all backends):
let sub = v(out)[10:50]
```

---

## `.meas` / `.measure` — Complete Reference

**Syntax on the netlist dot-card:**
```
.meas[ure] <analysis> <result_name> <type> [arguments]
```

**Equivalent interactive command:**
```
meas <analysis> <result_name> <type> [arguments]
```

`<analysis>` = `tran` | `dc` | `ac` | `sp`  
`<result_name>` = user-defined name; becomes a circuit parameter after measurement  
`<type>` = one of the measurement types below

---

### Measurement Types

| Type | Description |
|------|-------------|
| `TRIG … TARG …` | Delay between two events (time/value at TRIG to TARG) |
| `DELAY` | Synonym for TRIG…TARG |
| `FIND <vec> AT=<x>` | Value of vector at a specific x-axis point |
| `FIND <vec> WHEN <vec2>=<val>` | Value of vec when vec2 crosses val |
| `WHEN <vec>=<val>` | x-axis value (time/frequency/voltage) when vec equals val |
| `MAX` | Maximum value of vector over window |
| `MIN` | Minimum value of vector over window |
| `MAX_AT` | x-axis value where maximum occurs |
| `MIN_AT` | x-axis value where minimum occurs |
| `AVG` | Arithmetic average over window |
| `RMS` | RMS value over window |
| `PP` | Peak-to-peak (max − min) over window |
| `INTEG` / `INTEGRAL` | Integral of vector over window |
| `DERIV` | Derivative at a point (requires `AT=`) |
| `ERR` | RMS error between two vectors |
| `ERR1` | Error variant 1 |
| `ERR2` | Error variant 2 |
| `ERR3` | Error variant 3 |
| `PARAM '<expr>'` | Compute expression from other `.meas` results (second pass) |
| `EXPR '<expr>'` | Alias for `PARAM` |

---

### Standard Keyword Arguments

All measurement types accept the following qualifiers where applicable:

| Keyword | Description |
|---------|-------------|
| `AT=<x>` | Measure at exact x-axis value (time/frequency/voltage) |
| `FROM=<x>` | Start of measurement window |
| `TO=<x>` | End of measurement window |
| `TD=<x>` | Time delay before measurement starts (ignore until x) |
| `VAL=<v>` | Threshold value for RISE/FALL/CROSS detection (default 0) |
| `RISE=<n>` | Trigger on nth rising edge through VAL |
| `FALL=<n>` | Trigger on nth falling edge through VAL |
| `CROSS=<n>` | Trigger on nth crossing (either direction) |
| `RISE=LAST` / `FALL=LAST` / `CROSS=LAST` / `LAST` | Use last transition found |

---

### Syntax Examples

```spice
* Propagation delay (trig→targ):
.meas tran tpd TRIG v(in)  VAL=0.5 RISE=1
+              TARG v(out) VAL=0.5 FALL=1

* Rise time:
.meas tran trise TRIG v(out) VAL=0.1 RISE=1
+                TARG v(out) VAL=0.9 RISE=1

* Value at a specific time:
.meas tran vout_1n FIND v(out) AT=1n

* Time when output reaches threshold:
.meas tran t50 WHEN v(out)=0.5

* Find v(out) when v(in) = 1.5:
.meas tran vout_at_vin15 FIND v(out) WHEN v(in)=1.5 RISE=2

* RMS over window:
.meas tran vrms RMS v(out) FROM=10n TO=100n

* Average:
.meas tran vavg AVG v(out) FROM=10n TO=100n

* Peak-to-peak:
.meas tran vpp PP v(out) FROM=10n TO=100n

* Maximum and where it occurs:
.meas tran vmax MAX v(out)
.meas tran tmax MAX_AT v(out)

* Integral:
.meas tran energy INTEG v(out) FROM=0 TO=100n

* Expression from other meas results:
.meas tran duty PARAM='tpd_hi / (tpd_hi + tpd_lo)'

* DC sweep - find vgs where id = 1mA:
.meas dc vgs_1ma WHEN i(v_gs)=1m

* AC - 3 dB bandwidth:
.meas ac f3db WHEN vdb(out)=-3 FALL=1
```

---

### Result Access

After a `.meas` statement succeeds, the result is available:
- As a circuit parameter: use in `.param` expressions or subsequent `.meas PARAM`
- Via `print` / `let`: `print tpd` (interactive only)
- Precision controlled by env var `NGSPICE_MEAS_PRECISION` (default 5 significant figures)
- Failed measurements print `<name> failed!` and do not set the parameter

---

### ERR Types

Used to compare simulation results against a reference vector:

| Type | Formula |
|------|---------|
| `ERR` | `sqrt(mean((v1-v2)²))` — RMS error |
| `ERR1` | `mean(abs(v1-v2))` — mean absolute error |
| `ERR2` | `max(abs(v1-v2))` — max absolute error |
| `ERR3` | `mean(abs(v1-v2)/abs(v2))` — mean relative error |

```spice
* Compare two circuits:
.meas tran err_val ERR v(out1) v(out2) FROM=10n TO=100n
```
