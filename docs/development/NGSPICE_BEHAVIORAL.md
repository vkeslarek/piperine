# ngspice Behavioral / Nonlinear Sources

Source code references: `src/spicelib/devices/asrc/` (B),
`src/spicelib/devices/vcvs/`, `src/spicelib/devices/vccs/`,
`src/spicelib/parser/inpptree.c` (expression engine),
`src/frontend/inpcom.c` (E/G VALUE→B rewrite).

---

## 1. B-source — Arbitrary Nonlinear Source

Device letter `B`. Implemented in `asrc/` (internal device name `ASRC`).

### Syntax

```spice
Bxxx  N+  N-  V=<expr>  [TC1=<val>] [TC2=<val>] [RTC=0|1] [temp=<val>] [dtemp=<val>]
Bxxx  N+  N-  I=<expr>  [TC1=<val>] [TC2=<val>] [RTC=0|1] [temp=<val>] [dtemp=<val>]
```

Exactly one of `V=` or `I=` must be given. `V=` makes a voltage source;
`I=` makes a current source.

### Instance Parameters

| Parameter | Description                                    | Default |
|-----------|------------------------------------------------|---------|
| `V=expr`  | Voltage expression (voltage source mode)       | —       |
| `I=expr`  | Current expression (current source mode)       | —       |
| `TC1`     | First-order temperature coefficient (1/°C)     | 0       |
| `TC2`     | Second-order temperature coefficient (1/°C²)   | 0       |
| `RTC`     | 1 = reciprocal TC model (see below)            | 0       |
| `temp`    | Instance temperature (°C), overrides circuit   | —       |
| `dtemp`   | Delta temperature added to circuit temperature | 0       |

#### Temperature Coefficient

When TC1/TC2 are non-zero, output is multiplied by a factor:

```
factor = 1 + TC1*(T - Tnom) + TC2*(T - Tnom)^2        (RTC=0, default)
factor = 1 / (1 + TC1*(T - Tnom) + TC2*(T - Tnom)^2)  (RTC=1)
```

### Expression Language

Expressions may reference node voltages, branch currents, device
parameters, and built-in variables.

#### Voltage and Current Access

| Syntax            | Meaning                                       |
|-------------------|-----------------------------------------------|
| `v(n)`            | Voltage at node `n` w.r.t. ground             |
| `v(n1,n2)`        | Differential voltage `V(n1) − V(n2)`          |
| `i(Vxxx)`         | Current through voltage source `Vxxx`         |
| `@Rxxx[i]`        | Branch current through device (nutmeg syntax) |
| `@device[param]`  | Device operating-point parameter              |

#### Built-in Variables

| Name     | Type      | Meaning                                              |
|----------|-----------|------------------------------------------------------|
| `time`   | real      | Current simulation time (transient only; 0 in DC/AC) |
| `temper` | real      | Circuit temperature in °C                            |
| `hertz`  | real      | Current AC analysis frequency in Hz (AC only)        |

#### Constants

| Name | Value        |
|------|--------------|
| `e`  | 2.718281828… |
| `pi` | 3.141592653… |

#### Math Functions

| Function      | Description                                         |
|---------------|-----------------------------------------------------|
| `abs(x)`      | Absolute value                                      |
| `acos(x)`     | Arc cosine (radians)                                |
| `acosh(x)`    | Inverse hyperbolic cosine                           |
| `asin(x)`     | Arc sine (radians)                                  |
| `asinh(x)`    | Inverse hyperbolic sine                             |
| `atan(x)`     | Arc tangent (radians)                               |
| `atanh(x)`    | Inverse hyperbolic tangent                          |
| `cos(x)`      | Cosine                                              |
| `cosh(x)`     | Hyperbolic cosine                                   |
| `exp(x)`      | e^x                                                 |
| `ln(x)`       | Natural log; `log(x)` is an alias                  |
| `log10(x)`    | Base-10 log                                         |
| `sin(x)`      | Sine                                                |
| `sinh(x)`     | Hyperbolic sine                                     |
| `sqrt(x)`     | Square root; uses `abs(x)` if x < 0                |
| `tan(x)`      | Tangent                                             |
| `tanh(x)`     | Hyperbolic tangent                                  |
| `sgn(x)`      | Sign: −1, 0, or +1                                 |
| `ceil(x)`     | Ceiling (round toward +∞)                          |
| `floor(x)`    | Floor (round toward −∞)                            |
| `nint(x)`     | Nearest integer                                     |
| `pow(x,y)`    | x^y (also `^` operator)                            |
| `pwr(x,y)`    | abs(x)^y (sign-preserving power)                   |
| `min(x,y)`    | Minimum of two values                               |
| `max(x,y)`    | Maximum of two values                               |

#### Step / Ramp Functions

| Function         | Description                                              |
|------------------|----------------------------------------------------------|
| `u(x)`           | Unit step: 0 for x ≤ 0, 1 for x > 0                    |
| `uramp(x)`       | Unit ramp integral: 0 for x ≤ 0, x for x > 0           |
| `u2(x)`          | Clamp: 0 if x ≤ 0, x if 0 < x < 1, 1 if x ≥ 1        |

#### Piecewise-Linear

```spice
pwl(expr, x0, y0, x1, y1, x2, y2, ...)
pwl_derivative(expr, x0, y0, x1, y1, ...)
```

Linearly interpolates `y` from `x` breakpoint table. Extrapolates flat
beyond endpoints. `pwl_derivative` returns the slope.

#### Comparison Functions (return 0 or 1)

| Function  | Condition |
|-----------|-----------|
| `eq0(x)`  | x == 0    |
| `ne0(x)`  | x != 0    |
| `gt0(x)`  | x > 0     |
| `lt0(x)`  | x < 0     |
| `ge0(x)`  | x >= 0    |
| `le0(x)`  | x <= 0    |

#### Operators

```
+   -   *   /   ^   unary-   { }
```

Braces `{ }` are equivalent to `( )`. Precedence follows standard math.

If `log`, `ln`, or `sqrt` receive a negative argument, `abs(x)` is used
silently. Division by zero is an error.

### Examples

```spice
* Voltage source: sum of two node voltages
B1  out  0  V = v(a) + v(b)

* Diode-like current source
B2  a    0  I = 1e-14 * (exp(v(a)/0.026) - 1)

* Time-domain ramp
B3  ramp 0  V = time * 1e3

* Temperature-sensitive voltage
B4  vt   0  V = temper * 1e-3

* AC-domain: frequency-dependent voltage
B5  out  0  V = hertz / 1e6

* Piecewise linear lookup table
B6  out  0  V = pwl(v(in), 0, 0, 1, 2, 2, 3)

* Conditional: half-wave rectifier
B7  out  0  V = v(in) * u(v(in))
```

### AC Analysis

ngspice linearizes the B-source at the DC operating point: it
auto-differentiates the expression symbolically and stamps those
derivatives as linear dependent sources. No extra steps needed.

---

## 2. E-source — Voltage-Controlled Voltage Source (VCVS)

### Linear Form

```spice
Exxx  N+  N-  NC+  NC-  gain
```

Output voltage: `V(N+,N−) = gain × V(NC+,NC−)`.

```spice
E1  2  3  14  1  2.0    ; 2× voltage amplifier
```

The keyword `VCVS` is accepted before `NC+` for SPICE2 compatibility and
silently removed.

### Nonlinear Behavioral Form

ngspice rewrites these syntaxes at parse time into a B-source.

#### VALUE= / value=

```spice
Exxx  N+  N-  VALUE={expr}
```

Internally becomes:

```spice
Exxx  N+  N-  Exxx_int1  0  1
BExxx Exxx_int1  0  V = {expr}
```

#### VOL= form

```spice
Exxx  N+  N-  VOL = {expr}
```

Same rewrite as `VALUE=`.

#### TABLE form

```spice
Exxx  N+  N-  TABLE  {expr}  =  (x0,y0) (x1,y1) (x2,y2) ...
```

Rewrites to a `pwl()` B-source. Extrapolation is linear using the slope
from the first two and last two breakpoints.

### Examples

```spice
E1  out  0  in  0  10        ; gain-of-10 linear VCVS
E2  out  0  VALUE={v(in)*v(ctl)}   ; multiplier
E3  out  0  TABLE {v(in)} = (0,0) (1,2) (2,3) (5,4.5)
```

---

## 3. G-source — Voltage-Controlled Current Source (VCCS)

### Linear Form

```spice
Gxxx  N+  N-  NC+  NC-  transconductance
```

Output current: `I = gm × V(NC+,NC−)`.

```spice
G1  2  0  5  0  0.1e-3    ; 0.1 mS transconductance
```

The keyword `VCCS` is accepted for SPICE2 compatibility.

### Nonlinear Behavioral Form

#### VALUE= / value=

```spice
Gxxx  N+  N-  VALUE={expr}
```

#### CUR= form

```spice
Gxxx  N+  N-  CUR = {expr}
```

Both rewrite to a B-source, same pattern as E-source.

#### TABLE form

```spice
Gxxx  N+  N-  TABLE {expr} = (x0,y0) (x1,y1) ... [m=multiplier]
```

`m=` allows an optional multiplier expression (e.g., instance count scaling).

### Examples

```spice
G1  out  0  in  0  2e-3           ; linear VCCS
G2  out  0  VALUE={v(in)^2 * 1e-3}  ; nonlinear gm
G3  out  0  TABLE {v(gate)} = (0,0) (1,1e-3) (2,3e-3) (3,5e-3) m=4
```

---

## 4. F-source — Current-Controlled Current Source (CCCS)

Linear only. Current gain.

```spice
Fxxx  N+  N-  Vname  gain
```

`Vname` is a zero-volt voltage source inserted as a current sensor.
Positive controlling current flows from `+` to `−` of `Vname`.

```spice
Vsens  mid  0  DC 0
F1     out  0  Vsens  10    ; 10× current mirror
```

POLY(N) multi-input form is handled by the XSpice `spice2poly.cm`
codemodel (outside scope of this document).

---

## 5. H-source — Current-Controlled Voltage Source (CCVS)

Linear only. Transresistance.

```spice
Hxxx  N+  N-  Vname  transresistance
```

```spice
Vsens  probe  0  DC 0
HX    5  17  Vsens  500    ; 500 Ω transresistance
```

---

## 6. POLY Sources (SPICE2 Compatibility)

`E`, `G`, `F`, `H` sources accept SPICE2-style `POLY(N)` polynomial
dependencies. This requires the XSpice `spice2poly.cm` codemodel to be
loaded. Syntax:

```spice
Exxx  N+  N-  POLY(dim)  NC1+  NC1-  [NC2+  NC2-  ...]  p0  p1  p2  ...
```

`dim` = number of controlling inputs. Coefficients `p0, p1, p2, ...`
follow standard multi-variate polynomial ordering.

Example (1D, linear + quadratic):

```spice
* V_out = p0 + p1*v1 + p2*v1^2
E1  out  0  POLY(1)  in  0   0.0  2.0  0.5
```

Because POLY translation requires XSpice, prefer B-source expressions for
new designs.

---

## 7. Nonlinear Resistors, Capacitors, and Inductors

ngspice has no direct `R=f(v)` syntax. Synthesize nonlinear R/C/L using
the B-source change-of-variables technique.

### Nonlinear Resistor

```spice
* R(V) = f(v(pos,neg))
* Use a voltage-controlled current source:
Bxxx  pos  neg  I = v(pos,neg) / f(v(pos,neg))
```

Or with a subcircuit:

```spice
.subckt nlres  pos  neg
* B1 computes f(v) as a voltage
B1   1  0  V = f(v(pos,neg))
* G1 drives I = V(pos,neg) / B1_output
G1   pos  neg  pos  neg  1
* Vfb: sense current through the nonlinear element
* (adjust as needed for the specific nonlinearity)
.ends
```

### Nonlinear Capacitor

Standard technique using charge-controlled formulation:

```spice
.subckt nlcap  pos  neg
* Compute Q(V) = C(v) * v, expressed as a voltage on Cx
Bx   1  0  V = Q(v(pos,neg))
* Cx integrates dQ/dt → current
Cx   2  0  1
* Vx measures current into Cx
Vx   2  1  DC 0
* Feed that current back into the circuit
Fx   pos  neg  Vx  1
.ends
```

Example — voltage-dependent capacitor `C(v) = C0 + C1*v`:

```spice
.subckt nlcap  pos  neg  params: C0=1n C1=1p
Bx   1  0  V = C0*v(pos,neg) + 0.5*C1*v(pos,neg)^2
Cx   2  0  1
Vx   2  1  DC 0
Fx   pos  neg  Vx  1
.ends
```

### Nonlinear Inductor

Same pattern as nonlinear capacitor but for flux linkage `λ(i)`:

```spice
.subckt nlinductor  pos  neg
* Compute λ(i) = L(i) * i as a voltage on Lsyn
B_flux   1  0  V = lambda(i(Vsense))
Lsyn     2  0  1
Vsense   pos  1  DC 0          ; current sensor
Vsyn     2  3  DC 0            ; flux sense
E_drive  pos  neg  3  0  1     ; enforce v = dλ/dt
.ends
```

---

## 8. Expression Tips and Caveats

| Topic                          | Notes                                                           |
|--------------------------------|-----------------------------------------------------------------|
| `log` vs `ln`                  | Both compute natural log; `log10` for base-10                  |
| Negative sqrt / log argument   | `sqrt` and `log`/`ln` take `abs(x)` silently, no error        |
| Division by zero               | Simulation error — guard with `max(x, tiny)`                   |
| `time` in DC                   | Always 0; use only in `.tran`                                  |
| `hertz` in transient           | Undefined / 0 outside AC; not useful in `.tran`                |
| `temper` units                 | Degrees Celsius (not Kelvin)                                   |
| AC linearization               | B-source is linearized at OP; expression derivatives are exact |
| Convergence                    | Highly nonlinear expressions (switches via `u()`) can stall    |
| `pwl` extrapolation            | Flat (constant) beyond endpoints                               |
| Operator precedence            | Standard math; `^` is right-associative                        |
| Braces vs parentheses          | `{expr}` identical to `(expr)`; braces common for VALUE= form  |
