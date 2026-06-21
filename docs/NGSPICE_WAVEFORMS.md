# ngspice Source Waveforms

Transient waveform functions for independent voltage (`V`) and current (`I`) sources.

## Table of Contents

1. [Source Syntax Overview](#source-syntax-overview)
2. [PULSE](#pulse)
3. [SIN](#sin)
4. [EXP](#exp)
5. [PWL (Piecewise Linear)](#pwl-piecewise-linear)
6. [SFFM (Single-Frequency FM)](#sffm-single-frequency-fm)
7. [AM (Amplitude Modulation)](#am-amplitude-modulation)
8. [TRNOISE (Transient Noise)](#trnoise-transient-noise)
9. [TRRANDOM (Transient Random)](#trrandom-transient-random)
10. [DC / AC Combined](#dc--ac-combined)

---

## Source Syntax Overview

```spice
V<name> <n+> <n-> [DC <dcval>] [AC <acmag> [<acphase>]] [<waveform>]
I<name> <n+> <n-> [DC <dcval>] [AC <acmag> [<acphase>]] [<waveform>]
```

- `DC` value is used during DC operating point and as initial value before transient starts.
- `AC` value is used only in AC analysis (linear, complex amplitude).
- Waveform applies during transient (`.tran`) analysis.
- A source can have both `DC`, `AC`, and a transient waveform simultaneously.

---

## PULSE

Repeating rectangular pulse with configurable rise/fall times.

```spice
V<name> <n+> <n-> PULSE(<V1> <V2> [<TD> [<TR> [<TF> [<PW> [<PER> [<PHASE>]]]]]]])
```

| Position | Parameter | Default | Description |
|----------|-----------|---------|-------------|
| 1 | `V1` | required | Initial (low) value |
| 2 | `V2` | required | Pulsed (high) value |
| 3 | `TD` | `0` | Delay time (s) |
| 4 | `TR` | `tstep` | Rise time (s) |
| 5 | `TF` | `tstep` | Fall time (s) |
| 6 | `PW` | `tstop` | Pulse width (s) |
| 7 | `PER` | `tstop` | Period (s) |
| 8 | `PHASE` | `0` | Phase shift (degrees, XSpice only) |

**Waveform shape** (per period, after delay `TD`):
- `[0, TR)` → rises linearly from V1 to V2
- `[TR, TR+PW]` → holds V2
- `(TR+PW, TR+PW+TF)` → falls linearly from V2 to V1
- `[TR+PW+TF, PER)` → holds V1

**Example — 1 MHz clock, 5 V, 50% duty cycle, 1 ns edges:**
```spice
Vclk clk 0 PULSE(0 5 0 1n 1n 499n 1u)
```

---

## SIN

Damped sinusoid with optional delay.

```spice
V<name> <n+> <n-> SIN(<VO> <VA> [<FREQ> [<TD> [<THETA> [<PHASE>]]]])
```

| Position | Parameter | Default | Description |
|----------|-----------|---------|-------------|
| 1 | `VO` | required | DC offset |
| 2 | `VA` | required | Amplitude |
| 3 | `FREQ` | `1/tstop` | Frequency (Hz) |
| 4 | `TD` | `0` | Delay time (s) |
| 5 | `THETA` | `0` | Damping factor (1/s) |
| 6 | `PHASE` | `0` | Phase shift (degrees, XSpice only) |

**Formula:**
- `t ≤ TD`: `V = VO`
- `t > TD`: `V = VO + VA × sin(2π × FREQ × (t−TD)) × exp(−(t−TD) × THETA)`

**Example — 1 kHz sine, 1 V amplitude, no offset:**
```spice
Vin in 0 SIN(0 1 1k)
```

**Example — decaying sine, 10 kHz, 5 V, damping 1000/s:**
```spice
Vsig sig 0 SIN(0 5 10k 0 1000)
```

---

## EXP

Two exponential segments: rising then falling (or vice versa).

```spice
V<name> <n+> <n-> EXP(<V1> <V2> [<TD1> [<TAU1> [<TD2> [<TAU2>]]]])
```

| Position | Parameter | Default | Description |
|----------|-----------|---------|-------------|
| 1 | `V1` | required | Initial value |
| 2 | `V2` | required | Target value after first rise/fall |
| 3 | `TD1` | `tstep` | Rise delay time (s) |
| 4 | `TAU1` | `tstep` | Rise time constant (s) |
| 5 | `TD2` | `TD1 + tstep` | Fall delay time (s) |
| 6 | `TAU2` | `tstep` | Fall time constant (s) |

**Formula:**
- `t ≤ TD1`: `V = V1`
- `TD1 < t ≤ TD2`: `V = V1 + (V2−V1) × (1 − exp(−(t−TD1)/TAU1))`
- `t > TD2`: `V = V1 + (V2−V1) × (1 − exp(−(t−TD1)/TAU1)) + (V1−V2) × (1 − exp(−(t−TD2)/TAU2))`

**Example — capacitor charge/discharge pulse:**
```spice
Vpulse in 0 EXP(0 5 1u 0.5u 3u 0.5u)
```

---

## PWL (Piecewise Linear)

Arbitrary waveform defined by time-value pairs.

```spice
V<name> <n+> <n-> PWL(<t1> <v1> <t2> <v2> ... <tN> <vN>) [TD=<delay>] [R=<repeat_time>]
```

Or load from file:
```spice
V<name> <n+> <n-> PWL FILE=<filename> [TD=<delay>] [R=<repeat_time>]
```

**Parameters:**

| Parameter | Description |
|-----------|-------------|
| `t1 v1 ...` | Time-value pairs; times must be monotonically increasing |
| `TD=<t>` | Delay: shift entire waveform right by `t` seconds |
| `R=<t>` | Repeat: after last point, restart from time point `t` (must match an existing time point) |

**Behavior:**
- Before first time point: holds `v1`
- After last time point: holds `vN`
- Between points: linear interpolation
- With `R=<t>`: the segment from `t` to final point repeats cyclically

**File format** (one `time value` pair per line or space-separated):
```
0 0
1e-6 1
2e-6 0.5
3e-6 0
```

**Example — trapezoidal pulse:**
```spice
Vtrap out 0 PWL(0 0  1n 0  2n 5  8n 5  9n 0  10n 0)
```

**Example — repeating waveform:**
```spice
Vrep out 0 PWL(0 0  1u 1  2u 0  3u -1  4u 0) R=0
```

---

## SFFM (Single-Frequency FM)

Frequency-modulated sinusoid.

```spice
V<name> <n+> <n-> SFFM(<VO> <VA> [<FC> [<MDI> [<FS> [<PHASEC> [<PHASES>]]]]])
```

| Position | Parameter | Default | Description |
|----------|-----------|---------|-------------|
| 1 | `VO` | required | DC offset |
| 2 | `VA` | required | Amplitude |
| 3 | `FC` | `1/tstop` | Carrier frequency (Hz) |
| 4 | `MDI` | `0` | Modulation index |
| 5 | `FS` | `1/tstop` | Signal (modulating) frequency (Hz) |
| 6 | `PHASEC` | `0` | Carrier phase shift (degrees, XSpice) |
| 7 | `PHASES` | `0` | Signal phase shift (degrees, XSpice) |

**Formula:**
```
V = VO + VA × sin(2π × FC × t + MDI × sin(2π × FS × t))
```

**Example — FM signal, 1 MHz carrier, 10 kHz modulation, MDI=5:**
```spice
Vfm out 0 SFFM(0 1 1Meg 5 10k)
```

---

## AM (Amplitude Modulation)

Double-sideband AM signal.

```spice
V<name> <n+> <n-> AM(<VA> <VO> [<MF> [<FC> [<TD>]]])
```

| Position | Parameter | Default | Description |
|----------|-----------|---------|-------------|
| 1 | `VA` | required | Amplitude (carrier peak) |
| 2 | `VO` | required | Modulation offset (depth) |
| 3 | `MF` | `1/tstop` | Modulating frequency (Hz) |
| 4 | `FC` | `0` | Carrier frequency (Hz) |
| 5 | `TD` | `0` | Delay time (s) |

**Formula (after delay TD):**
```
V = VA × (VO + sin(2π × MF × t)) × sin(2π × FC × t)
```
- For `t ≤ TD`: `V = 0`

**Example — AM signal, 100 kHz carrier, 1 kHz modulation:**
```spice
Vam out 0 AM(1 1 1k 100k)
```

---

## TRNOISE (Transient Noise)

Generates white Gaussian noise, 1/f noise, and/or random telegraph signal (RTS) noise.

```spice
V<name> <n+> <n-> [DC <dc>] TRNOISE(<NA> <TS> [<NALPHA> [<NAMP> [<RTSAM> [<RTSCAPT> [<RTSEMT>]]]]])
```

| Position | Parameter | Default | Description |
|----------|-----------|---------|-------------|
| 1 | `NA` | required | White noise RMS amplitude (V or A) |
| 2 | `TS` | required | Noise time step (s); set to `tstep` for full bandwidth |
| 3 | `NALPHA` | `0` | 1/f noise exponent (0 = white only, typically 1–2) |
| 4 | `NAMP` | `0` | 1/f noise RMS amplitude (only if NALPHA ≠ 0) |
| 5 | `RTSAM` | `0` | RTS noise amplitude (V or A) |
| 6 | `RTSCAPT` | `0` | RTS mean capture time (s) |
| 7 | `RTSEMT` | `0` | RTS mean emission time (s) |

**Noise types:**

| Type | Params | Description |
|------|--------|-------------|
| White Gaussian | NA, TS | Gaussian random samples, interpolated at step TS |
| 1/f (flicker) | NALPHA, NAMP | Spectral density ∝ 1/f^NALPHA |
| RTS | RTSAM, RTSCAPT, RTSEMT | Two-state random telegraph, exponential capture/emission |

**Examples:**
```spice
* White noise only (10 nV/√Hz)
VNoise1 n1 0 DC 0 TRNOISE(10n 0.5n 0 0n)

* 1/f noise only (exponent=1, amplitude 10 nV)
VNoise2 n2 0 DC 0 TRNOISE(0 0.5n 1 10n)

* RTS noise only (15 mV amplitude, 22 µs capture, 50 µs emission)
VNoise3 n3 0 DC 0 TRNOISE(0 0 0 0 15m 22u 50u)

* White + 1/f combined
VNoise4 n4 0 DC 0 TRNOISE(10n 0.5n 1 5n)
```

> **Note:** Noise is generated during `.tran` only. DC operating point uses the `DC` value.

---

## TRRANDOM (Transient Random)

Generates piecewise-constant random values updated at fixed time steps. Useful for Monte Carlo stimulus or digital data patterns.

```spice
V<name> <n+> <n-> [DC <dc>] TRRANDOM(<TYPE> <TS> [<TD> [<PARAM1> [<PARAM2>]]])
```

| Position | Parameter | Default | Description |
|----------|-----------|---------|-------------|
| 1 | `TYPE` | required | Distribution type (integer 1–4, see below) |
| 2 | `TS` | required | Time step between new random values (s) |
| 3 | `TD` | `0` | Delay before randomization starts (s) |
| 4 | `PARAM1` | `1` | Primary distribution parameter |
| 5 | `PARAM2` | `0` | Secondary parameter (offset/mean) |

**Distribution types:**

| TYPE | Distribution | PARAM1 | PARAM2 |
|------|-------------|--------|--------|
| 1 | Uniform | Half-range (output in `[−PARAM1, +PARAM1]`) | Offset added to result |
| 2 | Gaussian | Standard deviation | Mean |
| 3 | Exponential | Mean | Offset |
| 4 | Poisson | Lambda (mean count) | Offset |

**Examples:**
```spice
* Uniform random [-1 V, +1 V], change every 1 µs
Vrand1 n1 0 TRRANDOM(1 1u 0 1)

* Gaussian, σ=0.5 V, mean=2.5 V, step 100 ns
Vrand2 n2 0 TRRANDOM(2 100n 0 0.5 2.5)

* Exponential, mean=1 µs, step 10 ns
Vrand3 n3 0 TRRANDOM(3 10n 0 1u)

* Poisson, lambda=3, step 1 µs
Vrand4 n4 0 TRRANDOM(4 1u 0 3)
```

> **Note:** Output is constant between steps (sample-and-hold). DC offset (`DC` keyword) is added on top of the random value.

---

## DC / AC Combined

A single source can carry DC bias, AC small-signal, and transient waveform simultaneously:

```spice
* 5 V DC bias + 1 V AC at 0° + 1 kHz sinusoidal transient
V1 in 0 DC 5 AC 1 0 SIN(0 0.1 1k)
```

- `DC 5` → operating point / DC sweep value
- `AC 1 0` → AC analysis: 1 V amplitude, 0° phase
- `SIN(...)` → `.tran` waveform

---

## Quick Reference

| Waveform | Min Params | Key Use |
|----------|-----------|---------|
| `PULSE` | 2 (V1, V2) | Clocks, digital, power-on |
| `SIN` | 2 (VO, VA) | RF, audio, AC test |
| `EXP` | 2 (V1, V2) | Charge/discharge |
| `PWL` | 2 pairs | Arbitrary, measured data |
| `SFFM` | 2 (VO, VA) | FM modulation test |
| `AM` | 2 (VA, VO) | AM modulation test |
| `TRNOISE` | 2 (NA, TS) | Noise margin, sensitivity |
| `TRRANDOM` | 2 (TYPE, TS) | Monte Carlo stimulus, data patterns |
