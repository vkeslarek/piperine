# Standard Library Types

Piperine provides built-in value types returned by the simulator APIs. These types
are `ExternObject` values — they expose methods via `obj.method()` syntax and can be
stored in variables using their type name (purely for documentation; the type system
is structural at runtime).

---

## Complex

A complex number returned by AC analysis signal vectors (and other frequency-domain analyses).

```verilog
AcResult ac = $ac("dec", 20, 1.0, 1e9);
Signal vout = ac.signal("v(out)");
// For complex-vector capable backends, individual samples can be Complex.
```

### Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `.real()` | real | Real part (Re) |
| `.imag()` | real | Imaginary part (Im) |
| `.magnitude()` | real | \|z\| = sqrt(Re² + Im²) |
| `.phase()` | real | Angle in degrees, atan2(Im, Re) |
| `.phase_rad()` | real | Angle in radians |
| `.db20()` | real | 20·log₁₀(\|z\|) — voltage/current dB |
| `.db10()` | real | 10·log₁₀(\|z\|²) — power dB |
| `.conjugate()` | Complex | Re - Im·j |

### Example

```verilog
Complex z = some_ac_point;
real mag  = z.magnitude();    // |z|
real deg  = z.phase();        // angle in degrees
real dBv  = z.db20();         // dB relative to 1V
Complex zc = z.conjugate();   // Re - Im*j
```

---

## Signal

A named vector from an analysis result. Obtained via `result.signal("name")`.

See [analyses.md](../ngspice/analyses.md#signal-methods) for the full method list including
`.max()`, `.min()`, `.mean()`, `.rms()`, `.peak_to_peak()`, `.integral()`,
`.bandwidth_3db()`, `.phase_margin()`, `.at(x)`, `.values()`, `.len()`.

---

## Analysis result types

| Type name | Returned by | Description |
|-----------|-------------|-------------|
| `OpResult` | `$op()` | DC operating point |
| `TranResult` | `$tran(...)` | Transient analysis |
| `AcResult` | `$ac(...)` | AC small-signal sweep |
| `DcResult` | `$dc(...)` | DC sweep |
| `NoiseResult` | `$noise(...)` | Noise analysis |
| `TfResult` | `$tf(...)` | Transfer function |
| `SensResult` | `$sens(...)`, `$sens_ac(...)` | Sensitivity |
| `PzResult` | `$pz(...)` | Pole-zero |
| `DistoResult` | `$disto(...)` | Distortion |
| `PssResult` | `$pss(...)` | Periodic steady state |
| `SpResult` | `$sp(...)` | S-parameters |

All result types share the same interface:

| Method | Returns | Description |
|--------|---------|-------------|
| `.plot_name()` | string | ngspice plot name |
| `.ok()` | integer | 1 = clean run, 0 = errors occurred |
| `.signal(name)` | Signal | Named vector by ngspice vector name |
| `.scale()` | Signal | Scale vector (time, frequency, …) |

---

## Named/optional arguments

System functions accept named arguments after mandatory positional ones:

```verilog
// Positional
TranResult t = $tran(1e-9, 1e-6);

// Named overrides
TranResult t = $tran(1e-9, 1e-6, tstart = 100e-9);
NoiseResult ns = $noise("v(out)", "v1", "dec", 20, 1.0, 1e9, ptspersum = 5);
AcResult ac = $ac("dec", fstart = 1e3, fstop = 1e6, points = 100);
```

The syntax is `name = value` — NOT `.name(value)` (that's paramset syntax). Named
args can appear in any order after the positional ones, and earlier positional positions
can be skipped if all remaining args are named.

---

## Math system functions

Real-valued math, usable anywhere in an expression.

| Function | Returns | Notes |
|----------|---------|-------|
| `$sqrt(x)` | real | |
| `$pow(x, y)` | real | x raised to y |
| `$exp(x)` `$ln(x)` `$log10(x)` | real | |
| `$sin(x)` `$cos(x)` `$tan(x)` | real | radians |
| `$asin(x)` `$acos(x)` `$atan(x)` | real | radians |
| `$atan2(y, x)` | real | full-quadrant arctangent |
| `$sinh(x)` `$cosh(x)` `$tanh(x)` | real | |
| `$hypot(x, y)` | real | `sqrt(x² + y²)` |
| `$floor(x)` `$ceil(x)` | real | |
| `$abs(x)` | real/integer | preserves type |
| `$min(a, b)` `$max(a, b)` | real | |
| `$clog2(n)` | integer | ceil(log2 n) — bit width to index `n` values |

```verilog
real gain_db = 20.0 * $log10(vout / vin);
real fc      = 1.0 / (2.0 * 3.14159265 * $sqrt(l * c));
integer addr_bits = $clog2(depth);
```
