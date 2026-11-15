# Digitally controlled buck converter: specification

This is the design specification of the `buck` example package, Piperine's flagship
mixed-signal prototype. It was written before any model code. Every number in the MSL sources
cites a section of this file (`SPEC §n`). The numbers are arbitrary but plausible: a 12 V
point-of-load converter with a fully digital voltage-mode loop.

The derivations in §§3 to 7 were checked numerically on a ZOH-discretized averaged model of
the power stage with the exact fixed-point controller of §6 (the only tool used; no SPICE run).
Expected outcomes in §10 come from that model and from first-order hand estimates. They are
targets for the bench, not results of a run of the MSL model: nothing executes yet.

## 1. Conventions

- `Ts` is the switching period. The control loop runs once per period: `fs = fsw`.
- A **code** is one LSB of the output-voltage ADC (§4), 6.25 mV at Vout.
- A **duty LSB** is 1/3200 of the period: the effective DPWM resolution with dither (§5).
- Fixed-point values are written `Qn`: an integer that holds the value times `2^n`.
- "Model" means the averaged model used to derive these numbers.

## 2. Electrical specification

| Quantity | Value | Notes |
| --- | --- | --- |
| Input voltage Vin | 9 V to 16 V, nominal 12 V | operating range |
| UVLO on Vin | on at 8.5 V rising, off at 8.0 V falling | 0.5 V hysteresis (§8.1) |
| Output voltage Vout | 0.8 V to 5.0 V in 25 mV steps | 8-bit setpoint register, codes 0 to 168 (§7.1) |
| Default output | 3.3 V (setpoint code 100) | value after power-up |
| Output current Iout | 0 A to 5 A continuous | forced continuous conduction (FCCM) |
| Switching frequency fsw | 500 kHz (Ts = 2 µs) | fixed, trailing-edge PWM |
| Control clock fclk | 200 MHz (5 ns) | DPWM counter clock (§5) |
| Inductor L | 4.7 µH, DCR 15 mΩ, Isat ≥ 9 A | |
| Output capacitor Cout | 220 µF, ESR 3 mΩ | ceramic bank |
| Input capacitor Cin | 22 µF, ESR 5 mΩ | |
| Input source | ideal source behind 20 mΩ | lab supply plus wiring |
| Power switches | Ron 20 mΩ each, Roff 1 MΩ, Coss 200 pF | behavioral, body diode Vf 0.7 V, 20 mΩ slope |
| Current sense | 5 mΩ shunt in series with L, amplifier gain 20 | 0.1 V/A, bidirectional |
| Dead time | 20 ns at each edge (4 clock counts) | generated in the DPWM (§5.3) |
| Gate driver | 10 ns propagation, 5 ns gate transition | shoot-through interlock (§5.3) |
| Max duty | 90 % | bootstrap refresh margin |
| DC regulation | ±1 % or ±10 mV, whichever is larger | over line, load, setpoint (§4.3) |
| Output ripple | ≤ 10 mV peak-to-peak | over the whole operating range (§3.3) |
| Load-step deviation | ≤ 5 % of Vout for 0.5 A to 4.5 A at 1 A/µs | recovery to ±1 % within 200 µs (§10) |
| Soft-start | reference slew 3.125 V/ms (1 code per period) | 3.3 V in 1.06 ms (§7.2) |
| Cycle-by-cycle current limit | +7.5 A peak (source), −3 A (sink) | §8.2, §8.3 |
| Over-voltage protection | Vout > 112.5 % of reference + 100 mV | §8.4 |
| Hiccup | 8 consecutive current-limited periods, then 2 ms off and retry | §8.2, §8.5 |
| Power-good | window ±6.25 % of reference, 128 µs assert delay, 8 µs deassert deglitch | §8.6 |

## 3. Power stage

### 3.1 Duty cycle

The ideal duty is `D = Vout / Vin`. With conduction losses, to first order,

    D ≈ (Vout + Iout · (DCR + Rshunt + Ron)) / Vin,     DCR + Rshunt + Ron = 15 + 5 + 20 = 40 mΩ

| Corner | D |
| --- | --- |
| minimum: 0.8 V out, 16 V in, no load | 0.8 / 16 = **0.050** |
| nominal: 3.3 V, 12 V, 5 A | (3.3 + 0.20) / 12 = **0.292** |
| maximum: 5.0 V, 9 V, 5 A | (5.0 + 0.20) / 9 = **0.578** |

The maximum, 0.58, leaves margin under the 90 % clamp for the 20 mΩ source drop and load
transients. The minimum on-time is 0.05 · 2 µs = 100 ns, 20 DPWM counts: no minimum on-time
limit is needed.

### 3.2 Inductor ripple current

    ΔIL = Vout · (1 − D) / (L · fsw),     L · fsw = 4.7 µH · 500 kHz = 2.35 Ω

| Vin, Vout | D | ΔIL p-p |
| --- | --- | --- |
| 12 V, 3.3 V (nominal) | 0.275 | 1.02 A |
| 12 V, 5.0 V | 0.417 | 1.24 A |
| 16 V, 5.0 V (worst) | 0.313 | **1.46 A** |
| 9 V, 5.0 V | 0.556 | 0.95 A |
| 16 V, 0.8 V | 0.050 | 0.32 A |

L was chosen for ripple ≤ 30 % of full load at the worst corner: `L ≥ 5 · (1 − 5/16) / (500 kHz ·
1.5 A) = 4.58 µH`, rounded up to the standard 4.7 µH. Peak current at full load is
`5 + 1.46 / 2 = 5.73 A`, 24 % below the 7.5 A current limit.

### 3.3 Output ripple

    ΔVcap = ΔIL / (8 · fsw · Cout) = ΔIL · 1 / (8 · 500 kHz · 220 µF) = ΔIL · 1.136 mΩ
    ΔVesr = ΔIL · ESR = ΔIL · 3 mΩ

| Corner | ΔVcap | ΔVesr | Sum (upper bound) |
| --- | --- | --- | --- |
| 12 V, 3.3 V | 1.16 mV | 3.05 mV | 4.2 mV |
| 16 V, 5.0 V (worst) | 1.66 mV | 4.39 mV | **6.1 mV** |

The two terms are 90° apart, so the sum is an upper bound. 6.1 mV < 10 mV target.

### 3.4 LC filter

    f0   = 1 / (2π · √(L · Cout)) = 1 / (2π · √(4.7 µH · 220 µF)) = 4.95 kHz
    Z0   = √(L / Cout) = 146 mΩ
    fesr = 1 / (2π · ESR · Cout) = 241 kHz       (above fsw/2: no help to the loop)

Series damping from the 40 mΩ path gives Q ≈ 146 / 40 ≈ 3.7 at no load. At 3.3 V / 5 A the load
(0.66 Ω) adds parallel damping, Q ≈ 1.1.

## 4. Measurement chain

### 4.1 Output divider and ADC

| Item | Value |
| --- | --- |
| Divider | 30 kΩ over 10 kΩ, ratio k = 1/4 |
| ADC | 10-bit SAR, Vref 1.6 V, mid-tread (rounds to nearest) |
| LSB | 1.6 V / 1024 = 1.5625 mV at the ADC, **6.25 mV at Vout** |
| Full scale at Vout | 6.4 V, which covers the 5.0 V maximum and its OVP threshold (5.73 V, §8.4) |
| Sampling | once per switching period, at the start of the period (DPWM count 0) |
| Conversion time | 500 ns (100 clock counts), the code and a `done` strobe appear together |
| Gain Kadc | 1024 / 1.6 / 4 = **160 codes/V** at Vout |

The ADC LSB was chosen so that the setpoint step (25 mV) is exactly 4 codes and 0.8 V is
exactly 128 codes. The reference in codes is then `ref = 128 + 4 · setpoint_code`: no
multiplier and no rounding.

### 4.2 Sampling rate

The loop samples at fs = fsw = 500 kHz, 33 × the crossover of §6 (15.9 kHz nominal): the loop
is designed directly in z. Sampling at a fixed point of the period, the start, aliases the
switching ripple to DC as a fixed offset of at most half the ESR ripple (2.2 mV worst), which is
inside the error budget below. No anti-alias filter is modelled.

### 4.3 Regulation budget

| Contribution | At 0.8 V | At 5.0 V |
| --- | --- | --- |
| ADC quantization, ±½ LSB | ±3.1 mV | ±3.1 mV |
| Ripple aliasing (§4.2) | ±1.0 mV | ±2.2 mV |
| Divider, 0.1 % resistors | ±1.6 mV | ±10 mV |
| ADC reference, ±0.1 % | ±0.8 mV | ±5 mV |
| **Total, worst case** | **±6.5 mV** (target ±10 mV) | **±20 mV = 0.41 %** (target ±1 %) |

The model reproduces the quantization part: the steady-state error stays within ±2.4 mV at
every corner of Vin {9, 12, 16} V × setpoint {0.8, 3.3, 5.0} V.

## 5. Digital PWM

### 5.1 Counter

A 9-bit counter runs at fclk = 200 MHz and wraps at **N = 400**: `fsw = 200 MHz / 400 = 500
kHz`. The high-side command is on while `count < width`. Native duty resolution is 1/400
(5 ns), a step of `Vin / 400` = 30 mV at Vout at 12 V.

### 5.2 Dither

30 mV is coarser than one ADC code (6.25 mV): with only the counter, the loop would limit-cycle
between two DPWM levels (§6.5). A **3-bit temporal dither** adds one count in `fine` of every
8 periods, where the duty word is `duty = 8 · coarse + fine`. The periods that get the extra
count follow a bit-reversed 3-bit frame counter, which spreads them over the 8-period frame.

| Item | Value |
| --- | --- |
| Duty word | 12 bits, 0 to 2880, one duty LSB = 1/3200 of the period |
| Effective resolution | 1/3200, so Vin / 3200 at Vout: 2.81 mV at 9 V, 3.75 mV at 12 V, **5.0 mV at 16 V** |
| Dither tone | fsw / 8 = 62.5 kHz, attenuated by (f0 / 62.5 kHz)² = 0.0063: ≤ 0.19 mV at 12 V |
| Max duty | 2880 / 3200 = 90 % |

### 5.3 Dead time and the gate driver

Dead time is generated in the counter domain, **4 counts = 20 ns** at each edge:

    high side on:   0 ≤ count < width
    low side on:    width + 4 ≤ count < 400 − 4

The low side releases 4 counts before the wrap, so the next high-side turn-on also sees 20 ns.
During dead time the inductor current freewheels through the low-side body diode (or, for
negative current, the high-side one). A cycle-by-cycle current limit (§8.2) ends the high-side
pulse early; the low side then turns on 4 counts after the actual high-side turn-off.

The gate driver adds 10 ns of propagation delay to both gates and an interlock: a gate turns on
only while the other command is off. The switch channel ramps between Roff and Ron in 5 ns. A
`Never` monitor (std) checks at each clock edge that both commands are never on together.

## 6. Control law

### 6.1 Plant

Voltage mode. From the duty word `u` (duty LSBs) to the ADC code:

    P(s) = (Vin / 3200) · Kadc · (1 + s/ωesr) / (1 + s/(Q·ω0) + s²/ω0²)
    DC gain Kloop = Vin · 160 / 3200 = Vin / 20:   0.45 at 9 V, 0.60 at 12 V, 0.80 at 16 V

The loop is evaluated in z: the averaged power stage is ZOH-discretized at Ts = 2 µs and
multiplied by **one sample of computational delay** (the sample taken at the start of period n
sets the duty of period n + 1, §9).

### 6.2 Compensator

A type-III equivalent, written as a discrete PID with a filtered derivative (parallel form, so
that the integrator can be clamped on its own):

    C(z) = Kp + Ki / (1 − z⁻¹) + Kd · (1 − z⁻¹) / (1 − a · z⁻¹)

    Kp = 4,   Ki = 0.5,   Kd = 64,   a = 1/4

Its poles and zeros:

| Item | Location | Role |
| --- | --- | --- |
| integrator | z = 1 | zero steady-state error |
| complex zero pair | 5.97 kHz, ζ = 0.35 | cancels the LC double pole at 4.95 kHz (type-III zeros) |
| real pole | z = 1/4, 110 kHz | rolls off the derivative before fs/2 |

**How the gains were found.** The candidate set is every gain representable in Q4 (§6.3) on a
grid around a type-III placement. A candidate is kept when the closed loop is stable (all poles
inside the unit circle) at every corner of Vin {9, 12, 16} V × Vout {0.8, 3.3, 5.0} V × Iout {0,
0.5, 5} A with phase margin ≥ 45° and gain margin ≥ 6 dB. The kept candidates were ranked by
the model's recovery time after the 0.5 A to 4.5 A load step. The derivative pole was chosen as a
power of two (a = 2⁻²) so that it is a shift.

**Crossover.** A 25 kHz crossover at 12 V (fsw/20) was tried first: it leaves only 33° of phase
margin at 16 V, because without input feedforward the loop gain follows Vin (±33 % over the
range). The chosen design trades crossover for margin over the whole range:

| Vin | Crossover fc | Worst phase margin | Worst gain margin |
| --- | --- | --- | --- |
| 9 V | 10.2 to 12.1 kHz | 56.3° | 15.4 dB |
| 12 V | 14.9 to 16.1 kHz | 55.9° | 12.9 dB |
| 16 V | 20.4 to 21.3 kHz | **51.6°** | **10.4 dB** |

(ranges over Vout and Iout). Nominal (12 V, 3.3 V, 5 A): **fc = 15.9 kHz, PM = 60.3°, GM =
13.0 dB**.

### 6.3 Fixed point

| Signal | Type | Scaling |
| --- | --- | --- |
| reference, measurement | `u10` | ADC codes |
| error e = ref − meas | `i16`, saturated to −256 … 255 | codes (±1.6 V window) |
| gains | integers | Q4: KP = 64, KI = 8, KD = 1024 |
| proportional, integral, derivative terms | `i32` | Q4 duty LSBs |
| derivative filter | `d ← (d >> 2) + KD · (e − e₋₁)` | a = 1/4 is an arithmetic shift |
| output | `u = (p + i + d) >> 4`, saturated to 0 … 2880 | duty LSBs, `u12` |

All three gains are exact in Q4: no coefficient rounding. Worst-case magnitudes: `KD · 512 =
524 288` and the integrator clamp `2880 · 16 = 46 080` fit easily in `i32`.

### 6.4 Saturation and anti-windup

- The output saturates to 0 … 2880 (90 % duty).
- The integrator is clamped to 0 … 2880 · 16 (it can never ask for more than the output limit).
- Conditional integration: the integrator holds while the output is saturated high and e > 0,
  or saturated low and e < 0.
- When the converter is not running (§8) the integrator, the derivative state and the output
  are reset to zero, so every soft-start begins from a clean state.

### 6.5 Quantization and limit cycles

The three classic conditions for a digitally controlled converter without limit cycles hold:

1. **DPWM finer than the ADC.** At Vout, one duty LSB is at most 5.0 mV (16 V, §5.2), one ADC
   code is 6.25 mV. At least one DPWM level falls in the zero-error bin at every Vin.
2. **Integral action.** Ki > 0, so the only equilibrium is e = 0.
3. **No integrator dead band.** One code of error adds KI = 8 Q4 units = 0.5 duty LSB per
   sample. The Q4 fraction keeps sub-LSB integral increments instead of rounding them away.

In the model, with the mid-tread ADC, the steady state is a constant code at every tested
corner (no idle tone). A truncating ADC was also tried: it adds a +3.1 mV bias, which is why the
ADC rounds.

## 7. Reference

### 7.1 Setpoint register

An 8-bit register written by the host through `sp_data` and a rising edge of `sp_wr`.

| Item | Value |
| --- | --- |
| Codes | 0 … 168; a larger write saturates to 168 |
| Vout | 0.8 V + 25 mV · code: 0.8 V at 0, 3.3 V at 100, 5.0 V at 168 |
| Target in ADC codes | 128 + 4 · code: 128 … 800 |
| Reset value | 100 (3.3 V) |
| Unknown data on a write | ignored, the register keeps its value |

### 7.2 Soft-start and slewing

The loop never sees the target directly. A ramp moves the reference **one code per sample**
towards the target, up or down:

    slew = 6.25 mV / 2 µs = 3.125 V/ms

| Event | Ramp | Duration |
| --- | --- | --- |
| startup to 3.3 V | 0 → 528 codes | 1.056 ms |
| startup to 5.0 V | 0 → 800 | 1.600 ms |
| startup to 0.8 V | 0 → 128 | 0.256 ms |
| setpoint 3.3 V → 5.0 V | 528 → 800 | 0.544 ms |
| setpoint 5.0 V → 1.0 V | 800 → 160 | 1.280 ms |

Capacitor charging current during the ramp: `Cout · slew = 220 µF · 3.125 V/ms = 0.69 A`. A
downward ramp sinks the same current through the low side (FCCM), well inside the −3 A sink
limit. Startup into a pre-biased output is not supported: the ramp starts from 0 V.

## 8. Protections and supervision

A supervisor state machine runs once per sample (on the ADC `done` strobe):

| Mode | Switching | Reference | Leaves to |
| --- | --- | --- | --- |
| `Off` | both switches off | held at 0 | `SoftStart` when enabled and Vin above UVLO |
| `SoftStart` | on | ramps to target | `Regulate` when the ramp reaches the target; `Hiccup` on a fault |
| `Regulate` | on | follows the target at the slew rate | `Hiccup` on a fault |
| `Hiccup` | both switches off | held at 0 | `SoftStart` after 2 ms |

Leaving the enabled state (enable low, or UVLO) goes to `Off` from any mode, at once.

### 8.1 UVLO

Vin is divided by 10 (90 kΩ over 10 kΩ) into a std `A2D` comparator with `vth = 0.825 V`,
`hyst = 50 mV`. It switches high at 0.85 V (Vin = 8.5 V) and low at 0.80 V (Vin = 8.0 V). UVLO is
not a fault: the converter goes to `Off` and the fault code is untouched.

### 8.2 Cycle-by-cycle overcurrent

The current-sense output (0.1 V/A) feeds a std `A2D` with `vth = 0.74 V`, `hyst = 20 mV`: it
trips at 0.75 V (**7.5 A**) and releases at 0.73 V. While it is high during the high-side
on-time, the DPWM ends the high-side pulse for the rest of the period. A period that was cut is
flagged to the supervisor. **8 consecutive cut periods (16 µs)** are a hard overcurrent: the
supervisor enters `Hiccup` with the fault code `OverCurrent`.

### 8.3 Sink current limit

The inverted current-sense output feeds a second `A2D` with `vth = 0.29 V`, `hyst = 20 mV`: it
trips at **−3 A**. While it is high during the low-side on-time, the DPWM ends the low-side
pulse for the rest of the period. This bounds the current FCCM can pull back from the output,
so a source that back-drives the output raises Vout instead of being absorbed (§10, S6).

### 8.4 Over-voltage

On each sample, with `ref` the ramped reference in codes:

    ovp_threshold = ref + ref / 8 + 16          (112.5 % of the reference plus 100 mV)

2 consecutive samples above it (4 µs) enter `Hiccup` with the fault code `OverVoltage`. The
threshold follows the ramped reference, not the target, so a downward setpoint change never
trips it: the output follows the ramp within the loop's tracking error. At 3.3 V the threshold
is 3.81 V; at 5.0 V it is 5.73 V, below the 6.4 V ADC full scale.

### 8.5 Fault and retry

`Hiccup` turns both switches off, resets the loop and counts **1000 samples (2 ms)**, then
retries through `SoftStart`. The fault code (`None`, `OverCurrent`, `OverVoltage`) is latched on
entry to `Hiccup` and cleared when a retry reaches `Regulate`.

### 8.6 Power-good

| Item | Value |
| --- | --- |
| Window | `ref ± ref / 16` (±6.25 %): ±206 mV at 3.3 V |
| Assert | in `Regulate` and 64 consecutive samples in the window: 128 µs |
| Deassert | 4 consecutive samples outside the window (8 µs), or at once on leaving `Regulate` |

The window is wider than the specified load-step and line-step deviations (5 %), so a rated
transient does not drop power-good.

## 9. Timing of one period

| Count | Time | Event |
| --- | --- | --- |
| 0 | 0 | counter wraps; the duty word of the previous sample is latched; high side on; ADC samples Vout |
| width | D · 2 µs | high side off (or earlier, on a current-limit cut) |
| width + 4 | + 20 ns | low side on |
| 100 | 500 ns | ADC code and `done`; supervisor, reference ramp and compensator update |
| 396 | 1.98 µs | low side off |
| 400 = 0 | 2 µs | next period uses the new duty word |

The duty word decided from the sample at the start of period n is applied at the start of
period n + 1: a delay of exactly one sample, as modelled in §6.1.

## 10. Bench scenarios

Every scenario starts from Vin = 12 V, setpoint 100 (3.3 V), 0.5 A resistive load (6.6 Ω), unless
it says otherwise. Times are from the stimulus edge. The tolerances are the bench's pass
criteria; the model column is the averaged-model prediction.

| # | Scenario | Stimulus | Expected outcome | Model |
| --- | --- | --- | --- | --- |
| S1 | Startup | `en` rises | `mode` Off → SoftStart at the next sample; Vout reaches 3.3 V ±1 % at 1.06 ms ±0.05 ms; overshoot ≤ 15 mV; peak inductor current ≤ 1.6 A; `pgood` rises 128 µs after `mode` = Regulate (≈ 1.19 ms) | 1.054 ms, 7.3 mV, 1.27 A |
| S1b | UVLO startup | Vin ramps 0 → 12 V in 2 ms, `en` high | no switching below 8.5 V; soft-start begins at the sample after Vin crosses 8.5 V | — |
| S2 | Setpoint up | write 168 (5.0 V) | ramp of 0.544 ms; Vout within ±1 % of 5.0 V at 0.54 ms ±0.05 ms; overshoot ≤ 15 mV; `pgood` stays high | 0.536 ms, 7.0 mV |
| S2b | Setpoint down | from 5.0 V, write 8 (1.0 V) | ramp of 1.28 ms; within ±1 % of 1.0 V at 1.29 ms; inductor current goes negative (≈ −0.6 A, FCCM sinking) without hitting the −3 A limit; no OVP | 1.286 ms, −0.62 A |
| S3 | Load step | 0.5 A → 4.5 A → 0.5 A, 1 A/µs | deviation ≤ 165 mV (5 %) each way; back within ±1 % in ≤ 200 µs; `pgood` stays high; no current limit | 150 mV / 155 mV, 174 / 178 µs |
| S4 | Line step | 12 V → 16 V → 12 V in 10 µs, 2 A load | deviation ≤ 165 mV; back within ±1 % in ≤ 200 µs; no fault | +151 / −153 mV |
| S5 | Overcurrent | load step to 0.3 Ω (11 A demand) | high side cut at 7.5 A every period; `fault` = OverCurrent and `mode` = Hiccup within 16 to 20 µs; `pgood` low within 8 µs of the deviation; retry every 2 ms + soft-start while the overload stays; a retry after the overload is removed reaches Regulate and clears `fault` | hand estimate |
| S6 | Over-voltage | inject 5 A into the output (back-drive) | the sink limit holds the low-side current at −3 A; Vout rises at ≈ 7 mV/µs; `fault` = OverVoltage once Vout > 3.81 V (≈ 80 to 120 µs); both switches off | hand estimate |
| S7 | Brown-out | Vin ramps 12 V → 7.5 V → 12 V | `mode` = Off below 8.0 V, both switches off, `pgood` low, `fault` unchanged; soft-start again above 8.5 V | — |
| S8 | Disable | `en` falls in Regulate | both switches off at the next sample; `pgood` low; Vout decays through the load | — |
| S9 | Steady state | 9 corners: Vin {9, 12, 16} V × setpoint {0.8, 3.3, 5.0} V at full load | DC error within §4.3; ripple ≤ 10 mV p-p; no sub-harmonic idle tone above 1 code | max 2.4 mV error, constant code |
| S10 | Shoot-through | any of the above | the `Never` monitor never fails (`checks` stream has no failed `never`) | — |

## 11. Model map

| Section | MSL item |
| --- | --- |
| §2, §3 | `power.msl`: `Resistor`, `Capacitor`, `Inductor`, `Supply`, `PowerSwitch`, `GateDriver`, `CurrentSense`, `Load`, `PowerStage` |
| §4, §8.1 to §8.3 | `sense.msl`: `Divider`, `Adc`, `Sensing` (dividers, ADC, three `A2D` comparators) |
| §5 | `dpwm.msl`: `Osc`, `Dpwm` |
| §6, §7 | `control.msl`: `SetpointReg`, `RefRamp`, `Pid` |
| §8 | `supervisor.msl`: `Mode`, `Fault`, `Supervisor` |
| top | `top.msl`: `Controller` (all digital), `Buck` (the whole converter) |
| constants | `spec.msl`: every structural number of this file as a `pub const` |
