# ngspice Netlist Reference

Generated from ngspice source at `~/Git/ngspice/src/frontend/` and `~/Git/ngspice/src/spicelib/devices/vsrc/`.
Excludes XSpice. Covers netlist structural dot-cards and source waveform functions.

---

## Dot-Cards

### .subckt — Subcircuit Definition

**Syntax:**
```
.subckt <name> <port1> [port2 ...] [params: <p1>=<default1> <p2>=<default2> ...]
+ [body lines...]
.ends [<name>]
```

- Port names are positional; they map to nets at the call site (`X` line).
- Parameters follow the keyword `params:` (case-insensitive). Defaults are required.
- Parameters can be expressions using `{...}` or `'...'` syntax.
- Instances inside use `X<name>` prefix.
- Nested subcircuits allowed. Each level gets its own parameter scope.

**Instantiation (`X` line):**
```
X<name> <net1> [net2 ...] <subckt_name> [params: <p>=<val> ...]
```

**Example:**
```spice
.subckt inv in out vdd vss params: wn=500n wp=1u l=90n
M1 out in vss vss nmos w=wn l=l
M2 out in vdd vdd pmos w=wp l=l
.ends inv

X1 a b vcc gnd inv params: wn=600n
```

---

### .param — Parameter Definition

**Syntax:**
```
.param <name> = <value_or_expr>
.param <name> = '<expression>'
.param <name> = {expression}
```

- Expressions support arithmetic: `+`, `-`, `*`, `/`, `^` (power), `**` (power).
- Math functions: `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`, `tanh`,
  `exp`, `log` (natural), `log10`, `sqrt`, `abs`, `int`, `nint`, `floor`, `ceil`, `min`, `max`,
  `sgn` (sign), `pwr(x,y)` (= x^y), `pwrs(x,y)` (= sgn(x)·|x|^y), `ternary_fcn(c,a,b)`.
- Special keyword `temper` refers to circuit temperature (°C) inside expressions.
- Parameters are evaluated before subcircuit expansion; they propagate into subcircuit scopes.
- Multiple parameters on one line separated by spaces, or one per line.
- Continuation line `+` supported.

**Example:**
```spice
.param Vdd=1.8 Vhalf='Vdd/2'
.param Cgate={5e-15 * 1.2}
.param freq=1e6 period='1/freq'
```

---

### .func — User-Defined Function

**Syntax:**
```
.func <name>(<arg1>[, arg2 ...]) {<expression>}
.func <name>(<arg1>[, arg2 ...]) '<expression>'
```

- Defines a reusable expression macro, expanded inline wherever called.
- Arguments are positional names used inside the expression body.
- Can be called from `.param`, device lines, or anywhere an expression is valid.
- Zero-argument form also valid: `.func myfunc() {expression}`.

**Example:**
```spice
.func Vt(temp) {1.38e-23 * (temp + 273.15) / 1.602e-19}
.func vdiode(is, vd) {is * (exp(vd / Vt(27)) - 1)}
.param Id = vdiode(1e-14, 0.7)
```

---

### .global — Global Net

**Syntax:**
```
.global <net1> [net2 ...]
```

- Declares nets that are shared across all subcircuit levels without being passed as ports.
- Net `0` (ground) is always global implicitly.
- Commonly used for `vdd`, `vss`, `gnd`.

**Example:**
```spice
.global vdd gnd
```

---

### .include / .inc — File Include

**Syntax:**
```
.include "<filename>"
.inc "<filename>"
```

- Inserts the contents of `<filename>` inline at the point of the directive.
- Path is relative to the including file's directory, or absolute.
- Supports quoted filenames (required if path contains spaces).

---

### .lib — Library File / Section

**Two forms:**

**Reference (include a section from a library file):**
```
.lib "<filename>" <section_name>
```

Includes only the named section from the library file. The library file uses `.lib <section_name>` / `.endl` markers to delimit sections.

**Definition (inside a library file):**
```
.lib <section_name>
[lines...]
.endl [<section_name>]
```

**Example (library file `models.lib`):**
```spice
.lib nmos_fast
.model nfet nmos level=14 ...
.endl nmos_fast

.lib pmos_fast
.model pfet pmos level=14 ...
.endl pmos_fast
```

**Example (netlist referencing it):**
```spice
.lib "models.lib" nmos_fast
.lib "models.lib" pmos_fast
```

---

### .options / .option / .opt — Solver Options

**Syntax:**
```
.options [<key>=<value> ...] [<flag> ...]
```

All settable options with defaults (sourced from `cktntask.c`):

#### Tolerance / Convergence

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `reltol` | real | `1e-3` | Relative error tolerance |
| `abstol` | real | `1e-12` | Absolute current error tolerance (A) |
| `vntol` | real | `1e-6` | Voltage error tolerance (V) |
| `chgtol` | real | `1e-14` | Charge error tolerance (C) |
| `trtol` | real | `7` | Truncation error overestimation factor |
| `gmin` | real | `1e-12` | Minimum conductance (S) added to every pn junction |
| `gshunt` | real | `0` | Shunt conductance from every node to ground (S) |
| `pivtol` | real | `1e-13` | Minimum absolute pivot value |
| `pivrel` | real | `1e-3` | Minimum relative pivot value |

#### Iteration Limits

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `itl1` | int | `100` | DC operating point iteration limit |
| `itl2` | int | `50` | DC transfer curve iteration limit |
| `itl4` | int | `10` | Transient analysis iteration limit per timepoint |
| `itl6` / `srcsteps` | int | `1` | Number of source-stepping steps |
| `gminsteps` | int | `1` | Number of Gmin-stepping steps |
| `gminfactor` | real | `10` | Factor per Gmin step |

#### Integration

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `method` | string | `trap` | Integration method: `trap` (trapezoidal) or `gear` |
| `maxord` | int | `2` | Maximum integration order (1–6; Gear only) |
| `minbreak` | real | — | Minimum time between breakpoints (s) |

#### Temperature

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `temp` | real | `27` (°C) | Circuit operating temperature (stored as 300.15 K) |
| `tnom` | real | `27` (°C) | Nominal temperature for model parameters |

#### MOSFET Defaults

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `defm` | real | `1` | Default MOSFET multiplier `m` |
| `defl` | real | `100 µm` | Default MOSFET channel length (m) |
| `defw` | real | `100 µm` | Default MOSFET channel width (m) |
| `defad` | real | `0` | Default MOSFET drain area (m²) |
| `defas` | real | `0` | Default MOSFET source area (m²) |

#### Behavioral / Accuracy Flags

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `noopiter` | flag | off | Skip operating point iteration; go directly to Gmin stepping |
| `keepopinfo` | flag | off | Retain OP linearization for each small-signal analysis |
| `bypass` | int | `0` | Skip recalculation of unchanged elements (0=off) |
| `trytocompact` | flag | off | Enable LTRA compaction |
| `badmos3` | flag | off | Use old discontinuous MOS3 model |
| `copynodesets` | flag | off | Copy nodesets from device terminals to internal nodes |
| `nodedamping` | flag | off | Limit node voltage change between iterations |
| `absdv` | real | `0.5` | Max absolute node voltage change per iteration (V) |
| `reldv` | real | `2.0` | Max relative node voltage change per iteration |
| `noopac` | flag | off | Skip OP calculation for linear AC circuits |

#### Output / Listing Flags (output only, not stored in task)

| Option | Type | Description |
|--------|------|-------------|
| `acct` | flag | Print accounting summary after run |
| `list` | flag | Print netlist listing |
| `nomod` | flag | Suppress model summary |
| `nopage` | flag | Suppress page breaks in output |
| `node` | flag | Print node connection table |
| `opts` | flag | Print active options |
| `numdgt` | int | Number of digits in output |

---

### .temp — Circuit Temperature

**Syntax:**
```
.temp <value_in_celsius>
```

- Sets the circuit operating temperature. Equivalent to `.options temp=<value>`.
- Affects all temperature-dependent model parameters.

**Example:**
```spice
.temp 85
```

---

### .nodeset — Initial Node Voltage Hints

**Syntax:**
```
.nodeset V(<node>)=<value> [V(<node2>)=<value2> ...]
```

- Provides initial voltage hints to help DC operating point convergence.
- Applied only during the first few iterations; removed once convergence is underway.
- Does **not** constrain the final solution (unlike `.ic`).
- Multiple statements allowed; continuation with `+`.

**Example:**
```spice
.nodeset V(out)=0.5 V(fb)=1.2
```

---

### .ic — Initial Conditions

**Syntax:**
```
.ic V(<node>)=<value> [V(<node2>)=<value2> ...]
```

- Sets the initial voltage conditions for transient analysis.
- Forces node voltages to the specified values at `t=0` when `uic` is given on `.tran`.
- Without `uic`, the DC operating point is computed first, then `.ic` values are used
  to initialize the transient only if DC OP fails.
- More constraining than `.nodeset` — the values are held during the initial transient step.

**Example:**
```spice
.ic V(out)=0 V(cap_top)=1.8
.tran 1n 100n uic
```

---

### .save — Select Vectors to Save

**Syntax:**
```
.save <vec1> [vec2 ...]
.save all
.save allv
.save alli
```

- By default ngspice saves only node voltages and voltage-source currents.
- `.save all` — save all node voltages and branch currents.
- `.save allv` — save all node voltages only.
- `.save alli` — save all branch currents only.
- Named vector forms: `v(node)`, `i(vsrc)`, `@device[param]`.

**Example:**
```spice
.save v(out) v(in) i(v1) @M1[id]
```

---

### .probe — Alias for .save

**Syntax:**
```
.probe <vec1> [vec2 ...]
```

Functionally identical to `.save` in ngspice (parsed identically).

---

### .csparam — Control-Script Parameter

**Syntax:**
```
.csparam <name> = <value_or_expr>
```

- Like `.param`, but the variable is available inside `.control`/`.endc` script blocks
  as a nutmeg vector in the `const` plot.
- Evaluated at netlist load time before the control script runs.
- Useful for passing netlist parameter values into interactive/scripted analysis.

**Example:**
```spice
.param Vdd=1.8
.csparam Vdd_ctrl = Vdd

.control
echo Vdd is $Vdd_ctrl
.endc
```

---

### .if / .elseif / .else / .endif — Netlist Conditionals

**Syntax:**
```
.if (<boolean_expression>)
  [netlist lines...]
.elseif (<boolean_expression>)
  [netlist lines...]
.else
  [netlist lines...]
.endif
```

- The expression is evaluated by numparam after `.param` substitution.
- Non-zero result = true. Zero = false.
- Supports comparison operators: `==`, `!=`, `<`, `<=`, `>`, `>=`.
- Logical operators: `&&`, `||`, `!`.
- Commonly used to select between process corners or model levels.

**Example:**
```spice
.param corner=1
.if (corner == 1)
  .lib "models.lib" fast
.elseif (corner == 2)
  .lib "models.lib" slow
.else
  .lib "models.lib" typical
.endif
```

---

### .model — Device Model Card

**Syntax:**
```
.model <name> <type> [(<param>=<value> ...)]
```

- `<name>` — model name referenced by device instances.
- `<type>` — device type: `R`, `C`, `D`, `NPN`, `PNP`, `NJF`, `PJF`, `NMOS`, `PMOS`,
  `GASFET`, `SW`, `CSW`, etc.
- Parameters as `key=value` pairs; may span continuation lines with `+`.

---

### .measure / .meas — Measurement

**Syntax:**
```
.meas {tran|ac|dc|sp} <result> <meas_type> [args...]
```

#### Measurement Types

**TRIG-TARG (time/value between two events):**
```
.meas tran <result> TRIG <vec1> VAL=<v1> [TD=<td>] [CROSS=<n>|LAST] [RISE=<n>|LAST] [FALL=<n>|LAST]
+                   TARG <vec2> VAL=<v2> [TD=<td>] [CROSS=<n>|LAST] [RISE=<n>|LAST] [FALL=<n>|LAST]
```

**WHEN (time/value when expression is true):**
```
.meas tran <result> WHEN <vec>=<val_or_vec2>
+ [TD=<td>] [FROM=<f>] [TO=<t>]
+ [CROSS=<n>|LAST] [RISE=<n>|LAST] [FALL=<n>|LAST]
```

**FIND … WHEN (value of one vector when another crosses):**
```
.meas tran <result> FIND <vec1> WHEN <vec2>=<val_or_vec3>
+ [TD=<td>] [FROM=<f>] [TO=<t>]
+ [CROSS=<n>|LAST] [RISE=<n>|LAST] [FALL=<n>|LAST]
```

**FIND … AT (value at a specific x-axis point):**
```
.meas tran <result> FIND <vec> AT=<xval>
```

**Interval measurements (FROM/TO range):**
```
.meas tran <result> AVG   <vec> [FROM=<f>] [TO=<t>]
.meas tran <result> RMS   <vec> [FROM=<f>] [TO=<t>]
.meas tran <result> MIN   <vec> [FROM=<f>] [TO=<t>]
.meas tran <result> MAX   <vec> [FROM=<f>] [TO=<t>]
.meas tran <result> PP    <vec> [FROM=<f>] [TO=<t>]   ← peak-to-peak
.meas tran <result> INTEG <vec> [FROM=<f>] [TO=<t>]
.meas tran <result> DERIV <vec> [FROM=<f>] [TO=<t>]  ← or DERIVATIVE
```

**Parameter expression:**
```
.meas tran <result> PARAM '<expression using other meas results>'
```

#### Qualifier Keywords

| Keyword | Description |
|---------|-------------|
| `VAL=<v>` | Threshold value for crossing detection |
| `TD=<t>` | Delay — ignore events before this time |
| `FROM=<f>` | Start of integration/search window |
| `TO=<t>` | End of integration/search window |
| `RISE=<n>` | Match the nth rising crossing (or `LAST`) |
| `FALL=<n>` | Match the nth falling crossing (or `LAST`) |
| `CROSS=<n>` | Match the nth crossing in either direction (or `LAST`) |
| `AT=<x>` | Evaluate at exact x-axis value |

---

## Source Waveforms

Applied to `V<name>` and `I<name>` sources. Multiple waveforms can be combined by listing both
`DC` and a transient waveform; `AC` is always separate for small-signal.

### DC — DC Bias

**Syntax:**
```
V<name> <n+> <n-> DC <value>
```

`DC` keyword is optional; a bare number is treated as DC value.

---

### AC — Small-Signal Stimulus

**Syntax:**
```
V<name> <n+> <n-> AC <magnitude> [<phase_degrees>]
```

| Argument | Default | Description |
|----------|---------|-------------|
| magnitude | — | AC amplitude (V or A) |
| phase | `0` | Phase offset (degrees) |

Only active during `.ac` analysis. Can be combined with DC and transient waveforms on same source.

---

### PULSE — Trapezoidal Pulse

**Syntax:**
```
V<name> <n+> <n-> PULSE(<v1> <v2> [td [tr [tf [pw [per [np]]]]]])
```

| Parameter | Default | Description |
|-----------|---------|-------------|
| `v1` | — | Initial value |
| `v2` | — | Pulsed value |
| `td` | `0` | Delay before first transition (s) |
| `tr` | `tstep` | Rise time v1→v2 (s) |
| `tf` | `tstep` | Fall time v2→v1 (s) |
| `pw` | `tstop` | Pulse width at v2 (s) |
| `per` | `tstop` | Period (s); `per` = `td+tr+pw+tf` for single pulse |
| `np` | — | Number of pulses (optional, ngspice extension) |

---

### SIN — Sinusoidal

**Syntax:**
```
V<name> <n+> <n-> SIN(<vo> <va> [freq [td [theta [phase]]]])
```

| Parameter | Default | Description |
|-----------|---------|-------------|
| `vo` | — | DC offset (V) |
| `va` | — | Amplitude (V) |
| `freq` | `1/tstop` | Frequency (Hz) |
| `td` | `0` | Delay (s); before `td`, output is `vo` |
| `theta` | `0` | Damping factor (1/s); envelope = `exp(-theta·t)` |
| `phase` | `0` | Phase (degrees) |

**Formula (t > td):** `vo + va · sin(2π·freq·(t-td) + phase·π/180) · exp(-(t-td)·theta)`

---

### EXP — Double Exponential

**Syntax:**
```
V<name> <n+> <n-> EXP(<v1> <v2> [td1 [tau1 [td2 [tau2]]]])
```

| Parameter | Default | Description |
|-----------|---------|-------------|
| `v1` | — | Initial value |
| `v2` | — | Peak value |
| `td1` | `tstep` | Rise delay (s) |
| `tau1` | `tstep` | Rise time constant (s) |
| `td2` | `td1+tstep` | Fall delay (s) |
| `tau2` | `tstep` | Fall time constant (s) |

**Formula:**
- `t ≤ td1`: `v1`
- `td1 < t ≤ td2`: `v1 + (v2-v1)·(1 - exp(-(t-td1)/tau1))`
- `t > td2`: above + `(v1-v2)·(1 - exp(-(t-td2)/tau2))`

---

### PWL — Piecewise Linear

**Syntax:**
```
V<name> <n+> <n-> PWL(<t1> <v1> <t2> <v2> ... [tn vn])
+ [td=<delay>] [r=<repeat_start_time>]
```

| Argument | Description |
|----------|-------------|
| `t1 v1 t2 v2 ...` | Time-value breakpoint pairs; times must be strictly increasing |
| `td=<t>` | Global delay — shifts the entire waveform by `td` seconds |
| `r=<t>` | Repeat: after the last point, restart from the time `r` within the waveform |

**Example:**
```spice
V1 in 0 PWL(0 0 1n 1.8 3n 1.8 4n 0 10n 0)
V2 clk 0 PWL(0 0 5n 1.8 10n 1.8 15n 0) r=0
```

---

### SFFM — Single-Frequency FM

**Syntax:**
```
V<name> <n+> <n-> SFFM(<vo> <va> [fc [mdi [fs]]])
```

| Parameter | Default | Description |
|-----------|---------|-------------|
| `vo` | — | DC offset (V) |
| `va` | — | Amplitude (V) |
| `fc` | `1/tstop` | Carrier frequency (Hz) |
| `mdi` | `0` | Modulation index |
| `fs` | `1/tstop` | Signal (modulating) frequency (Hz) |

**Formula:** `vo + va · sin(2π·fc·t + mdi · sin(2π·fs·t))`

---

### AM — Amplitude Modulation

**Syntax:**
```
V<name> <n+> <n-> AM(<sa> <oc> <mf> <fc> [td])
```

| Parameter | Default | Description |
|-----------|---------|-------------|
| `sa` | — | Signal amplitude (modulating wave amplitude) |
| `oc` | — | Offset constant (carrier DC component) |
| `mf` | `1/tstop` | Modulating frequency (Hz) |
| `fc` | — | Carrier frequency (Hz) |
| `td` | `0` | Delay (s) |

**Formula (t > td):** `sa · (oc + sin(2π·mf·(t-td))) · sin(2π·fc·(t-td))`

---

### trnoise — Transient Noise

**Syntax:**
```
V<name> <n+> <n-> trnoise(<na> <nt> [nalpha [namp [rtsam [rtscapt [rtsemt]]]]])
```

| Parameter | Default | Description |
|-----------|---------|-------------|
| `na` | — | RMS amplitude of white Gaussian noise (V or A) |
| `nt` | — | Time step for noise sample generation (s) |
| `nalpha` | `0` | 1/f noise exponent (0 = white only) |
| `namp` | `0` | 1/f noise amplitude (only used if `nalpha ≠ 0`) |
| `rtsam` | `0` | RTS (Random Telegraph Signal) amplitude (V or A) |
| `rtscapt` | — | RTS mean capture time (s) (required if `rtsam ≠ 0`) |
| `rtsemt` | — | RTS mean emission time (s) (required if `rtsam ≠ 0`) |

- White noise samples drawn every `nt` seconds and interpolated.
- 1/f noise added when `nalpha > 0` and `namp > 0`.
- RTS noise (two-state Markov switching) added when `rtsam > 0`.
- Can be combined with DC: `V1 in 0 DC 0 trnoise(1m 1n)`.

---

### trrandom — Transient Random Source

**Syntax:**
```
V<name> <n+> <n-> trrandom(<type> <ts> [td [param1 [param2]]])
```

| Parameter | Default | Description |
|-----------|---------|-------------|
| `type` | — | Distribution type (integer): `1`=uniform, `2`=Gaussian, `3`=exponential, `4`=Poisson |
| `ts` | — | Duration of each sample (s) |
| `td` | `0` | Delay before generating values (s) |
| `param1` | `1.0` | Distribution parameter 1 (range for uniform; std dev for Gaussian; mean for exp/Poisson) |
| `param2` | `0.0` | Distribution parameter 2 (offset for uniform; mean for Gaussian) |

**Distribution details:**

| type | Distribution | param1 | param2 | Output range |
|------|-------------|--------|--------|-------------|
| 1 | Uniform | range | offset | `[offset, offset+range]` |
| 2 | Gaussian | std deviation | mean | continuous |
| 3 | Exponential | mean | — | `[0, ∞)` |
| 4 | Poisson | mean | — | non-negative integers |

---

### port — RF Port Source (S-parameter)

**Syntax:**
```
V<name> <n+> <n-> portnum <n> [Z0=<impedance>]
```

or inside `.sp` analysis context:
```
V<name> <n+> <n-> AC 1 port <n> [z0=<impedance>]
```

| Parameter | Default | Description |
|-----------|---------|-------------|
| `portnum` / `port` | — | Port number (1-based) for S-parameter matrix |
| `Z0` / `z0` | `50` | Reference impedance for the port (Ω) |

Used with `.sp` analysis to compute S-parameters. Each port is driven individually while others are terminated.

---

## Device Operating Point Access

After any analysis, internal device parameters are accessible via:

```
@<device_name>[<parameter>]
```

**Examples:**
```
@M1[vth]      ← threshold voltage of MOSFET M1
@M1[id]       ← drain current
@M1[vgs]      ← gate-source voltage
@D1[id]       ← diode current
@Q1[ic]       ← BJT collector current
@R1[p]        ← power dissipated in resistor
```

Use with `save`, `print`, `meas`, or `let`:
```spice
.save @M1[id] @M1[vth]
.meas tran idsat FIND @M1[id] AT=10n
```

Available parameters depend on device type. Common ones:
- MOSFET: `id`, `vgs`, `vds`, `vbs`, `vth`, `gm`, `gds`, `cgs`, `cgd`, `cgb`
- BJT: `ic`, `ib`, `ie`, `vbe`, `vce`, `gm`, `gpi`, `go`
- Diode: `id`, `vd`, `gd`, `cd`
- Resistor: `i`, `p` (power), `v`

See `NGSPICE_EXPRESSIONS.md` for the full per-device parameter tables.

---

## Vector Math Functions

Available in `plot`, `print`, `let`, `.meas` expressions, and `.control` scripts.
Sourced from `src/frontend/parse.c` (`ft_funcs` table).

### Complex / Magnitude

| Function | Aliases | Description |
|----------|---------|-------------|
| `mag(x)` | `abs(x)` | Magnitude; for complex: √(re²+im²) |
| `ph(x)` | `phase(x)` | Phase angle in degrees |
| `cph(x)` | `cphase(x)` | Continuous (unwrapped) phase in degrees |
| `unwrap(x)` | | Unwrap phase discontinuities |
| `real(x)` | `re(x)` | Real part |
| `imag(x)` | `im(x)` | Imaginary part |
| `db(x)` | | 20·log10(|x|) — magnitude in dB |
| `j(x)` | | Multiply by j (90° rotation in complex plane) |

### Math Functions

| Function | Description |
|----------|-------------|
| `log(x)` / `ln(x)` | Natural logarithm |
| `log10(x)` | Base-10 logarithm |
| `exp(x)` | e^x |
| `sqrt(x)` | Square root |
| `abs(x)` | Absolute value (alias for `mag`) |
| `sin(x)` | Sine (radians) |
| `cos(x)` | Cosine (radians) |
| `tan(x)` | Tangent (radians) |
| `atan(x)` | Arctangent (radians) |
| `sinh(x)` | Hyperbolic sine |
| `cosh(x)` | Hyperbolic cosine |
| `tanh(x)` | Hyperbolic tangent |
| `floor(x)` | Round down to integer |
| `ceil(x)` | Round up to integer |
| `nint(x)` | Round to nearest integer |
| `pos(x)` | Positive part: max(x, 0) |

### Statistical / Random

| Function | Description |
|----------|-------------|
| `rnd(x)` | Random integer in [0, x) per element |
| `sunif(x)` | Uniform random in [−1, +1] (x ignored) |
| `sgauss(x)` | Standard normal N(0,1) random (x ignored) |
| `poisson(x)` | Poisson random with mean x |
| `exponential(x)` | Exponential random with mean x |

### Vector / Reduction

| Function | Aliases | Description |
|----------|---------|-------------|
| `mean(x)` | | Arithmetic mean of all elements |
| `avg(x)` | | Incremental (running) average |
| `norm(x)` | | Normalize by max: x/max(x) |
| `sortorder(x)` | | Index permutation for ascending sort |
| `length(x)` | | Number of elements in vector |
| `vecmin(x)` | `minimum(x)` | Minimum value |
| `vecmax(x)` | `maximum(x)` | Maximum value |
| `vector(n)` | | Create [0, 1, 2, ..., n−1] |
| `unitvec(n)` | | Create [1, 1, ..., 1] of length n |
| `vecd(x)` | | Element-wise differences (length n−1) |

### Calculus / Frequency Domain

| Function | Description |
|----------|-------------|
| `deriv(x)` | Numerical derivative w.r.t. scale (time or frequency) |
| `group_delay(x)` | Group delay: −d(phase)/dω |
| `interpolate(x)` | Interpolate onto the current plot's scale |
| `fft(x)` | Fast Fourier Transform (use linear time steps in `.tran`) |
| `ifft(x)` | Inverse FFT |

### Binary Operators

| Operator | Description |
|----------|-------------|
| `+` `-` `*` `/` | Arithmetic |
| `^` or `**` | Power |
| `>` `<` `>=` `<=` `==` `!=` | Comparison (returns 0 or 1 per element) |
| `&&` `\|\|` `!` | Logical |
| `? :` | Ternary: `cond ? a : b` |

### Examples

```spice
* Magnitude and phase of transfer function
let H = v(out) / v(in)
plot mag(H) ph(H)

* Power spectral density in dB
plot db(v(out))

* Group delay
plot group_delay(v(out))

* FFT of transient output
tran 1n 10u
fft v(out)
plot mag(v(out))

* RMS using mean
let vrms = sqrt(mean(v(out)^2))
print vrms

* Define distribution functions for MC (also defined in MonteCarlo.sp example)
define unif(nom, rvar) (nom + (nom*rvar) * sunif(0))
define agauss(nom, avar, sig) (nom + avar/sig * sgauss(0))
```
