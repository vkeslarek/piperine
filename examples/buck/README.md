# `buck`: a digitally controlled buck converter in MSL

Piperine's flagship mixed-signal prototype: a 9-16 V to 0.8-5.0 V, 5 A, 500 kHz synchronous buck
converter whose loop is entirely digital. The analog power stage, the measurement chain, the
200 MHz counter-based DPWM, the Q4 fixed-point PID, the soft-start, the protections and
power-good are all MSL, in one package.

- `SPEC.md` is the design: every number, how it was derived, and the bench scenarios with the
  expected outcomes. It was written before the model; the sources cite it as `SPEC §n`.
- `GAPS.md` lists what the language or the front end could not express directly, and what the
  model does instead.
- `src/` is the crate (root `src/lib.msl`):

| File | Contents |
| --- | --- |
| `spec.msl` | the structural numbers of `SPEC.md` as `pub const`s |
| `power.msl` | `Supply`, `PowerSwitch` (Ron/Roff, body diode, Coss), `GateDriver`, `Inductor`, `Capacitor`, `CurrentSense`, `Load`, `PowerStage` |
| `sense.msl` | `Divider`, `Adc` (10-bit, sampled by a reaction that probes), `Sensing` (ADC plus three std `A2D` comparators: UVLO, current limit, sink limit) |
| `dpwm.msl` | `Osc` (200 MHz), `Dpwm` (9-bit counter, 3-bit dither, dead time, cycle-by-cycle cuts, a std `Never` shoot-through monitor) |
| `control.msl` | `SetpointReg`, `RefRamp` (soft-start and slew), `Pid` (Q4, anti-windup) |
| `supervisor.msl` | `Mode`, `Fault`, `Supervisor` (UVLO/OCP/OVP, hiccup, power-good) |
| `top.msl` | `Controller` (all the digital blocks) and `Buck` (the whole converter) |

## Status

Nothing executes MSL yet: there is no type checker, elaborator or compiler to the solver. What
holds today is the front end's bar: the package lexes, parses and resolves through the POM
against the embedded `std` with zero diagnostics, and its hierarchy reads back as written.
`crates/piperine-lang/tests/buck_example.rs` is that gate:

```sh
cargo test -p piperine-lang --test buck_example
```

## Driving it from a host

There is no testbench in MSL: stimulus, sequencing and judgment live in the host (Part VIII of
the spec). The host elaborates `Buck` as the top module, drives its `in` ports, `set`s its
params during a run, and reads `pgood`, `fault`, `mode` and any node or carrier by path.

| Knob | Kind | Used by |
| --- | --- | --- |
| `en` | top `in` port | S1, S8 |
| `sp_wr`, `sp_data` | top `in` ports: the setpoint register write | S2 |
| `load_step` | top `in` port: switches `r_step` in at 1 A/µs | S3, S5 |
| `inject` | top `in` port: back-drives `i_inject` into the output | S6 |
| `vin_nom` | param, `set` during the run | S1b, S4, S7 |
| `r_step` | param, `set` before the step | S5 (0.3 Ω) |
| `vout`, `vin`, `isense`, `power.sw`, `power.lx` | nodes to probe | all |
| `ctl.reference`, `ctl.duty`, `ctl.setpoint`, `mode`, `fault`, `pgood` | carriers to read | all |

A sketch in Python of three scenarios of `SPEC.md` §10. The host API names are illustrative, in
the style of the spec's own sketches, not a library that exists today:

```python
import piperine as pp

sim = pp.Session("examples/buck", top="Buck")
sim.drive("en", 0); sim.drive("sp_wr", 0); sim.drive("sp_data", 100)
sim.drive("load_step", 0); sim.drive("inject", 0)

async def s1_startup():
    sim.drive("en", 1, at=100e-6)
    await sim.until(lambda: sim.read("mode") == "Regulate")
    t_reg = sim.now()
    assert abs(t_reg - 100e-6 - 1.056e-3) < 50e-6           # ramp of 528 codes at 2 µs each
    await sim.rise("pgood")
    assert abs(sim.now() - t_reg - 128e-6) < 4e-6          # 64 in-window samples
    assert abs(sim.probe("vout") - 3.3) < 0.033

async def s3_load_step():
    sim.drive("load_step", 1)
    dip = await sim.min_over("vout", 300e-6)
    assert 3.3 - dip <= 0.165                              # 5 %
    assert sim.read("pgood") == 1                          # the ±6.25 % window holds

async def s5_overcurrent():
    sim.set("load.r_step", 0.3)                            # 11 A demand
    sim.drive("load_step", 1)
    await sim.until(lambda: sim.read("fault") == "OverCurrent", timeout=25e-6)
    assert sim.read("mode") == "Hiccup"
```

The `checks` stream carries the `Never` monitor of `Dpwm`: a bench passes only if it holds no
failed `never` record (S10).
