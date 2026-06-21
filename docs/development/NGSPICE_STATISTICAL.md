# ngspice Statistical Analysis (Monte Carlo)

Source: `src/frontend/numparam/xpressn.c`, `src/maths/misc/randnumb.c`.

ngspice does not have a built-in `.mc` card. Monte Carlo is done by
parameterizing the circuit with distribution functions inside `.param`
statements and repeating runs with a `.control` script loop.

---

## 1. Distribution Functions (`.param` Context)

These functions are available only inside `.param` expressions (numparam
preprocessor, not in B-source expressions).

### `agauss` — Absolute Gaussian

```spice
agauss(nominal, abs_variation, sigma)
```

Returns `nominal + (abs_variation / sigma) * N(0,1)`.

- `abs_variation`: absolute spread (same units as nominal)
- `sigma`: number of standard deviations that `abs_variation` represents

Example — 1 kΩ ± 5% at 3σ:

```spice
.param R1val = agauss(1000, 50, 3)
```

### `gauss` — Relative Gaussian

```spice
gauss(nominal, rel_variation, sigma)
```

Returns `nominal + nominal * rel_variation / sigma * N(0,1)`.

- `rel_variation`: fractional spread (e.g., 0.05 for ±5%)
- `sigma`: number of standard deviations

Example — same as above, relative form:

```spice
.param R1val = gauss(1000, 0.05, 3)
```

### `aunif` — Absolute Uniform

```spice
aunif(nominal, abs_variation)
```

Returns `nominal + abs_variation * U(−1, +1)`.

`U(−1,+1)` is a uniform random number on [−1, +1].

Example — capacitor ±10 pF:

```spice
.param C1val = aunif(100p, 10p)
```

### `unif` — Relative Uniform

```spice
unif(nominal, rel_variation)
```

Returns `nominal + nominal * rel_variation * U(−1, +1)`.

Example — ±5% relative spread:

```spice
.param Cval = unif(100p, 0.05)
```

### `limit` — Absolute Binary (Worst-Case)

```spice
limit(nominal, abs_variation)
```

Returns `nominal + abs_variation` or `nominal − abs_variation` with equal
probability. Useful for worst-case cornering.

Example:

```spice
.param Vth = limit(0.5, 0.05)    ; either 0.45 or 0.55
```

---

## 2. Random Seed Control

The PRNG (combined Tausworthe + LCG) is seeded from `getpid()` at startup
by default, giving different results each run.

To make runs reproducible, set `rndseed` before loading the circuit:

```spice
* In .spiceinit or at top of .control block:
set rndseed = 42
```

The seed is re-applied at the first distribution function call. Changing
`rndseed` between runs in the same session reseeds the generator.

| Variable  | Type    | Description                                             |
|-----------|---------|---------------------------------------------------------|
| `rndseed` | integer | Fixed seed > 0; if absent, seed is random at startup   |

---

## 3. Monte Carlo Workflow

ngspice has no single `.mc` card. The standard approach is a `.control`
loop that re-runs the simulation `N` times:

```spice
* mycirc.spi
.title Monte Carlo example

.param Rval = agauss(1k, 50, 3)    ; 1 kΩ ± 50 Ω at 3σ
.param Cval = aunif(100n, 5n)      ; 100 nF ± 5 nF

R1  in   out  {Rval}
C1  out  0    {Cval}
V1  in   0    PULSE(0 1 0 1n 1n 500n 1u)

.tran 10n 2u

.control
  set num_mc_runs = 100
  let mc_run = 0
  dowhile mc_run < num_mc_runs
    run
    let mc_run = mc_run + 1
  end
  * All plots are now named tran1, tran2, ...
  plot tran1.v(out) tran2.v(out) tran3.v(out)
.endc

.end
```

Each `run` invocation re-evaluates `.param` lines (numparam evaluates
distribution functions fresh each call), producing a different realization.

### Naming of Result Plots

Each run creates a new plot with an auto-incremented name
(`tran1`, `tran2`, ...). Access waveforms as `<plotname>.<vector>`:

```spice
.control
  let n = 0
  dowhile n < 10
    run
    let n = n + 1
  end
  * Overlay all 10 transient runs:
  foreach i 1 2 3 4 5 6 7 8 9 10
    plot $i.v(out) vs $i.time
  end
.endc
```

### Statistics on Results

After N runs, collect scalar measurements with `.meas`:

```spice
.meas tran Vmax MAX v(out)
```

Each run stores `Vmax` for that plot. To aggregate, collect into a vector:

```spice
.control
  let results = []
  let n = 0
  dowhile n < 50
    run
    meas tran vp MAX v(out)
    let results = results vp
    let n = n + 1
  end
  * results now holds 50 peak values
  echo mean: {mean(results)}
  echo stddev: {std(results)}
.endc
```

---

## 4. Lot / Device Tolerance Pattern

Standard industry practice: separate lot-to-lot variation (`LOT`) from
device-to-device variation (`DEV`) using two `.param` levels:

```spice
* Lot-level offset (same for all devices in a lot)
.param lot_R = agauss(0, 1, 3)        ; normalized lot shift

* Device-level individual variation
.param R1 = 1k * (1 + 0.01*lot_R + agauss(0, 0.005, 3))
.param R2 = 1k * (1 + 0.01*lot_R + agauss(0, 0.005, 3))
```

Each run re-draws `lot_R` and each device's individual term.

---

## 5. Functions Available in `.param` (numparam)

The numparam engine has its own function set, broader than B-source
expressions:

| Function        | Description                                  |
|-----------------|----------------------------------------------|
| `agauss(n,a,s)` | Absolute Gaussian (see §1)                   |
| `gauss(n,r,s)`  | Relative Gaussian (see §1)                   |
| `aunif(n,a)`    | Absolute uniform (see §1)                    |
| `unif(n,r)`     | Relative uniform (see §1)                    |
| `limit(n,a)`    | Binary ±abs_variation (see §1)               |
| `ternary_fcn(c,a,b)` | If c≠0 return a, else b (ternary op)   |
| `nint(x)`       | Nearest integer                              |
| `int(x)`        | Truncate to integer                          |
| `sqr(x)`        | x² (not square root — use `sqrt`)            |
| `arctan(x)`     | Arc tangent                                  |
| Standard math   | sin, cos, exp, ln, log, log10, sqrt, abs, pow, pwr, max, min, ceil, floor, sgn, sinh, cosh, tanh, asin, acos, atan, asinh, acosh, atanh, tan |

Note: `sqr(x) = x*x`, not `sqrt(x)`. This differs from the B-source where
`sqr` is not available.

---

## 6. Notes and Caveats

| Topic                  | Notes                                                               |
|------------------------|---------------------------------------------------------------------|
| Re-evaluation          | `.param` is re-evaluated per `run`, not per `alter`                |
| `alter` vs `run`       | `alter R1 1.1k` changes a fixed value; does not re-sample           |
| Session isolation      | Each `run` creates an independent plot; use `setplot` to switch     |
| Seed per session       | `rndseed` applies session-wide; change it between runs for sub-seeding |
| `.options SEED`        | Not a real ngspice option; use `set rndseed=` in `.control`        |
| Memory                 | 1000 MC runs × large circuit → large memory; `destroy` old plots   |
| Batch mode             | In `-b` mode, use `write` to save each run's rawfile               |
