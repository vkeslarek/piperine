# `std::spice`: the interface catalogue

**Status**: Approved, in its decided state.
**Approved**: 2026-11-27 - user (translated, relayed by the coordinator): "I accept the recommended options, except what I told you (no CIDER and no XSPICE)." The decisions are in section 12.
**Reference**: ngspice-47. **Language**: MSL as specified in `docs/spec/`, with the `alias` item this feature adds (section 2.5).

This page shows how a user places every kind of ngspice device in MSL: the modules, their ports, their parameters, how a model card is written and shared. Sections 1 and 2 give the library and the rules that hold for every device. Sections 3 to 10 go family by family, with examples, the parameter tables and the choices made. Section 11 lists what the library does differently from ngspice on purpose. Section 12 records the decisions.

**What was asked for:**

- every model ngspice has, faithful to its behaviour and its equations;
- freedom to reshape the interface so the models are easier to use and to read;
- in this order: map the models and every parameter; design the user interface on top of that; then map the calculations; then prove each model against ngspice;
- a part of the Book that is a user manual of the library;
- the models shipped with the language, in the standard library, under the name `spice`, so that it is plain they come from ngspice.

**How complete this page is.** The complete parameter map of all 52 in-scope devices (11,027 parameters) is in `models/*.md`, generated from the ngspice-47 sources: name, type, default, meaning, and whether ngspice reads the parameter's "given" flag. The tables in this page are written by hand, with units, for the passive elements, the sources, the switches, the lines and the diode, and for the instance parameters and the main model parameters of the transistors. The remaining model parameters of the large transistor models keep their ngspice names under the rules of section 2. Their units, defaults and given-ness are written family by family in `interface/<family>.md` by the planner of that family's phase, who decides what the rules leave open; you are told what was decided and do not review those pages (your decision of 2026-11-27).

---

## 1. The library at a glance

The models are the namespace `std::spice` of the standard library. They ship with the language: a project needs no dependency, only a `use`.

```rust
use std::spice::passive::{Resistor, Capacitor};
use std::spice::source::{VoltageSource, pulse};
use std::spice::diode::Diode;

/// A clipper: a pulse through a resistor into a diode and a capacitor.
module Clipper() {
    signal vin: Electrical;
    signal vout: Electrical;

    let edge = pulse() { v1: 0.0, v2: 5.0, td: 1n, tr: 100p, tf: 100p, pw: 5n, per: 10n };
    let v1 = VoltageSource(vin, gnd, edge);
    let r1 = Resistor(vin, vout) { r: 1k };
    let c1 = Capacitor(vout, gnd) { c: 1p };
    let d1 = Diode(vout, gnd) { is: 2.52n, rs: 0.568, n: 1.752, cjo: 4p, mj: 0.4, tt: 20n };
}
```

The namespaces follow the device categories of the Book:

| Namespace | Holds | ngspice elements |
| --- | --- | --- |
| `std::spice::passive` | `Resistor`, `SemiconductorResistor`, `Capacitor`, `SemiconductorCapacitor`, `Inductor`, `GeometricInductor`, `MutualInductor` | R, C, L, K |
| `std::spice::source` | `VoltageSource`, `CurrentSource`, `BehavioralVoltageSource`, `BehavioralCurrentSource`, and the waveforms `pulse`, `sine`, `exponential`, `piecewise_linear`, `frequency_modulated`, `amplitude_modulated`, `transient_noise`, `transient_random` | V, I, B |
| `std::spice::controlled` | `VoltageControlledVoltageSource`, `VoltageControlledCurrentSource`, `CurrentControlledCurrentSource`, `CurrentControlledVoltageSource` | E, G, F, H |
| `std::spice::switch` | `VoltageControlledSwitch`, `CurrentControlledSwitch` | S, W |
| `std::spice::line` | `LosslessTransmissionLine`, `LossyTransmissionLine`, `SingleLossyTransmissionLine`, `CoupledTransmissionLines`, `UniformRcLine` | T, O, Y, P, U |
| `std::spice::diode` | `Diode`, `GeometricDiode`, `DiodeSelfHeating`, `GeometricDiodeSelfHeating` | D |
| `std::spice::bipolar` | `Bjt`, `Vbic`, `VbicSelfHeating`, `Hicum2`, `Hicum2SelfHeating` | Q |
| `std::spice::jfet` | `Jfet`, `Jfet2` | J |
| `std::spice::mesfet` | `Mesfet`, `Mesa`, `Hfet1`, `Hfet2` | Z |
| `std::spice::mosfet` | `Mos1`, `Mos2`, `Mos3`, `Mos6`, `Mos9`, `Vdmos`, `VdmosSelfHeating` | M |
| `std::spice::bsim` | `Bsim1`, `Bsim2`, `Bsim3`, `Bsim3V32`, `Bsim3V1`, `Bsim3V0`, `Bsim4`, `Bsim4V7`, `Bsim4V6`, `Bsim4V5` | M |
| `std::spice::soi` | `Bsim3SoiFd`, `Bsim3SoiDd`, `Bsim3SoiPd`, `Bsim4Soi`, `Soi3` | M |
| `std::spice::hisim` | `Hisim2`, `HisimHv1`, `HisimHv2` | M |
| `std::spice::constants` | ngspice's physical constants (`CHARGE`, `BOLTZMANN`, `REFERENCE_TEMPERATURE`, …) | |

Every item is an ordinary MSL module with an `analog` block (and a `digital` block where the device has events). The library uses nothing a package of yours could not use, and none of its names is in the prelude: `Diode` means the library's diode only where a `use` says so.

---

## 2. The rules that hold for every device

### 2.1 Names

Two kinds of name, two rules.

**A parameter keeps its ngspice name**, in lower case: `is`, `n`, `rs`, `cjo`, `vto`, `kp`, `tox`, `vth0`. These names are the vocabulary of the domain. Every model card, every foundry kit, every textbook and ngspice's own manual index a parameter by them, and a BSIM4 card has 891 of them: a card can be copied with no dictionary, and a search for `vth0` finds the same thing everywhere. The full meaning and the unit are in the parameter's `///` text, which the host lists (§21) and the Book prints.

**Everything else is spelled out**: modules (`VoltageControlledSwitch`, not `Sw`), ports (`reference`, not `ref`), namespaces, enums and their variants, functions. A device's established name counts as a word: `Bjt`, `Jfet`, `Mosfet`, `Mesfet`, `Bsim4`, `Vbic`, `Hicum2`, `Hisim2`.

A few ngspice names cannot be MSL names, or mean two things in one device. The complete list:

| ngspice | MSL | Where | Why |
| --- | --- | --- | --- |
| `as` | `as_` | every MOSFET: source area | `as` is a keyword |
| `pub` | `pub_` | BSIM3, BSIM4, SOI: a binning coefficient of `ub` | `pub` is a keyword |
| `on`, `off` (switches) | `initial: Option<SwitchState>` | S, W | `on` is a keyword; two flags for one state |
| `type` | `polarity` | `hicum2` | `type` is reserved |
| the type keywords `npn`, `pnp`, `nmos`, `pmos`, `njf`, `pjf`, `nmf`, `pmf`, … | `polarity: <Family>Polarity` | every transistor | flags in ngspice, one enum here |
| `m` of the diode model (grading coefficient) | `mj` | D | `m` is the instance multiplier; `mj` is ngspice's own alias |
| `m` of the VDMOS model (body-diode grading) | `mj` | `vdmos` | the same |
| `m` of the MESA and HFET models (knee shape) | `m_knee` | `mesa`, `hfet1`, `hfet2` | the same; ngspice has no alias |
| `ps` of the Mos6 model (saturation-current modifier) | `ps_sat` | `mos6` | `ps` is the source perimeter of the instance |
| `ic-vds`, `ic-vgs` | `icvds`, `icvgs` | `jfet`, `jfet2` | `-` is not part of an identifier; the MOSFETs already spell it this way |

Where ngspice accepts an alias (`js` for `is`, `tref` for `tnom`, `vt0` for `vto`), MSL has the first name only. The Book's translation table lists every alias.

### 2.2 Units

A parameter has the unit ngspice gives it. Lengths are in metres, areas in square metres, currents in amperes, capacitances in farads. Two units differ from what the MSL core uses, and the rule of 2.1 decides them the same way (decision D4):

- `tnom` and `temp` are in **degrees Celsius**, as in every model card (`tnom: 27.0`). `dtemp` is a difference, the same in both scales. The model converts to kelvin inside.
- `acphase` is in **degrees**.

MSL literals take SI suffixes, so a card reads as it does in SPICE: `1k`, `2.2u`, `100p`, `1G`. One trap: `m` is milli in MSL and in SPICE, and mega is `M` in MSL where SPICE writes `meg`.

### 2.3 Defaults and "given"

In ngspice a parameter that is not written takes a default, and for some parameters the model also behaves differently when it was not written. MSL has `Option` for that. The rule is mechanical:

| In ngspice | In MSL | Example |
| --- | --- | --- |
| a constant default, and nothing else reads whether it was given | a param with that default | `n: f64 = 1.0;` |
| the default is computed from other parameters | `Option`, resolved in the body | `vto: Option<f64>;` (from `nsub`, `tox`, `nss` when left out) |
| "not given" switches something off or on | `Option` | `bv: Option<f64>;` (no breakdown), `vaf: Option<f64>;` (no Early effect), `ikf: Option<f64>;` (no high-injection roll-off) |
| a flag | `bool` | `off: bool = false;` |
| required | a param with no default | `r: f64;` |

Nothing is `inf` or `0` "meaning absent": absent is `None`.

### 2.4 Refused values

Where ngspice stops with an error for a value, the param has a `where` that refuses the same values, at elaboration and on every host `set`. Where ngspice replaces a value (a resistance under 1 pΩ becomes 1 pΩ), the model does the same and the parameter's text says so. The package invents no bound ngspice does not have.

### 2.5 Model cards: `alias`

**A model card is an `alias`.** The feature adds one item to the language, for every module and not only for these: an alias names a module with some of its params bound.

```spice
.model D1N4148 D (is=2.52n rs=0.568 n=1.752 cjo=4p m=0.4 tt=20n bv=100 ibv=100u)
D1 a k D1N4148
D2 b k D1N4148 area=2
```

```rust
/// 1N4148 switching diode.
pub alias D1N4148 = Diode { is: 2.52n, rs: 0.568, n: 1.752, cjo: 4p, mj: 0.4, tt: 20n, bv: 100.0, ibv: 100u };

let d1 = D1N4148(a, k);
let d2 = D1N4148(b, k) { area: 2.0 };
```

Three rules cover its use:

1. **A placement binds what the alias left.** A param the module requires and the alias did not bind must be given at the placement; a param with a default or of an `Option` type may be.
2. **A placement may replace what the alias bound.** `D1N4148(a, k) { bv: 75.0 }` is this one diode with another breakdown voltage.
3. **An alias may refine an alias.** `alias D1N4148_HOT = D1N4148 { tnom: 85.0 };`.

```rust
/// The NMOS of a 0.8 µm process.
pub alias NCH = Mos1 { polarity: MosfetPolarity::Nmos, vto: 0.75, kp: 110u, gamma: 0.4, lambda: 0.04, tox: 15n };

let m1 = NCH(d, g, s, b) { w: 10u, l: 1u };       // M1 d g s b NCH W=10u L=1u
```

There is no separate "model" type and no second list of parameters: the device module has every parameter, instance and model alike, and a card binds the ones it knows (decision D2). An instance placed through an alias is an instance of the module: the host lists every one of its params with where its value came from (the module's default, the alias, the placement), and sets, sweeps and differentiates any of them.

**Binning stays a function.** An alias is fixed text; choosing among cards by geometry is code, and the language already names that form (§8.4.7). The function returns a template made from an alias:

```rust
pub fn nch(w: f64, l: f64) -> Mos1 {
    if l < 1u { NCH_SHORT { w, l } } else { NCH_LONG { w, l } }
}
let m2 = nch(10u, 0.8u)(d, g, s, b);
```

**One parameter for one quantity.** In ngspice some 130 parameters exist twice, once on the model and once on the instance, and the instance value wins (`tc1` of a resistor, `area` of a diode, `rbdb` of BSIM4). In MSL each is one param: a card binds it, and a placement that binds it again replaces the card's value. That is rule 2.

`design.md` §12 has the amendment in full: grammar, the rules of elaboration, errors, what the host and the object model show.

### 2.6 Polarity

`.model X npn` becomes a param: `polarity: BjtPolarity = BjtPolarity::Npn`. Each transistor family has its enum, with ngspice's keywords as variants and ngspice's default:

| Enum | Variants | Default | Modules |
| --- | --- | --- | --- |
| `BjtPolarity` | `Npn`, `Pnp` | `Npn` | `Bjt`, `Vbic`, `Hicum2` |
| `JfetPolarity` | `Njf`, `Pjf` | `Njf` | `Jfet`, `Jfet2` |
| `MesfetPolarity` | `Nmf`, `Pmf` | `Nmf` | `Mesfet`, `Mesa` |
| `HfetPolarity` | `Nhfet`, `Phfet` | `Nhfet` | `Hfet1`, `Hfet2` |
| `MosfetPolarity` | `Nmos`, `Pmos` | `Nmos` | every MOSFET, bulk and SOI |
| `VdmosPolarity` | `Vdmosn`, `Vdmosp` | `Vdmosn` | `Vdmos` |

### 2.7 Temperature

The run sets one temperature, read by `temperature()` in kelvin (§8.5). Each device adds what ngspice adds:

| Param | Type | Meaning |
| --- | --- | --- |
| `temp` | `Option<f64>`, °C | the temperature of this instance; given, it replaces the run's |
| `dtemp` | `f64 = 0.0`, K | added to the run's temperature when `temp` is not given |
| `tnom` | `f64 = 27.0`, °C | the temperature at which the card's values were measured |

The device temperature is `temp`, or else the run's temperature (27 °C when the run sets none) plus `dtemp`: ngspice's rule. There is no circuit-wide `tnom`: a design that measured its cards at 25 °C binds `tnom: 25.0` in its cards.

A device with self-heating has a thermal terminal and is a module of its own (`DiodeSelfHeating(a, k, tj)`, `VbicSelfHeating(c, b, e, s, dt)`), because a port is never optional in MSL (§8.1.8). The terminal is a node of the standard `Thermal` nature: its potential is the temperature rise and its flow the dissipated power, so a heat sink is a thermal resistor and capacitor on it.

### 2.8 Multiplicity

`m: Option<f64>` on every device that has it in ngspice. The effective count is `m`, or else the run's `multiplicity()`, or else 1 (§8.5.5). The model scales its currents, charges and noise by it, as ngspice does.

### 2.9 Initial conditions and `off`

- `off: bool = false` starts the Newton iteration with the device's junctions at 0 V. It changes the path to the operating point, never the answer.
- `ic` on a capacitor or an inductor, and `icvds`, `icvgs`, `icvbs`, `icvbe`, `icvce` on transistors, are `Option<f64>`. Given, the branch is **held at that value in the operating point of a transient** and released at the first step (§16 H7). ngspice reads these values only under `uic`, which MSL does not have (§16 H6); the start is the same, and it is a solved, consistent one.
- The vector forms (`ic=1,2,3`) are not parameters: the scalar ones are.

### 2.10 What you can read back

Every quantity ngspice reports for an instance is an operating-point variable of the same name, read by path after an analysis (§21): `m1.gm`, `m1.gds`, `m1.vth`, `m1.vdsat`, `m1.cgs`, `q1.gpi`, `d1.gd`. Node numbers and ngspice's sensitivity outputs are not: the host addresses nodes by path and computes sensitivities from gradients.

Noise sources carry ngspice's names (`rd`, `rs`, `id`, `1overf`, `thermal`, `shot`), so a noise report has the series a SPICE user expects.

---

## 3. Passive elements

### 3.1 Resistor

ngspice's resistor is specified in two ways: by its value, or by a sheet resistance and its drawn geometry. They are two modules, because they have different required parameters and different parameters that mean something.

```rust
let r1 = Resistor(a, b) { r: 4.7k };                                // by value
let r2 = Resistor(a, b) { r: 4.7k, tc1: 1.2m, tc2: 3u };            // with temperature coefficients
let r3 = Resistor(a, b) { r: 50.0, noisy: false };                  // a noiseless termination

/// The high-resistance polysilicon of a process.
pub alias RPoly = SemiconductorResistor { rsh: 1k, narrow: 40n, short: 20n, tc1: -0.9m, kf: 2e-25, af: 2.0 };
let r4 = RPoly(a, b) { l: 20u, w: 1u };                             // (20u - 2·20n) / (1u - 2·40n) · 1k
let r5 = RPoly(a, b) {};                                            // refused: `l` is required and unbound
```

`Resistor`: by value.

| Param | ngspice | Type and default | Unit | Meaning |
| --- | --- | --- | --- | --- |
| `r` | `resistance`, `r` | `f64`, required | Ω | Resistance at `tnom`. A value from 0 to 1 pΩ is taken as 1 pΩ, as ngspice does. |
| `ac` | `ac` | `Option<f64>` | Ω | Resistance used in small-signal analyses, when it differs. |
| `tc1` | `tc`, `tc1` | `f64 = 0.0` | 1/K | First-order temperature coefficient. |
| `tc2` | `tc2` | `f64 = 0.0` | 1/K² | Second-order temperature coefficient. |
| `tce` | `tce` | `Option<f64>` | %/K | Exponential coefficient: given, the factor is `1.01^(tce·ΔT)` and `tc1`, `tc2` are not used. |
| `tnom` | `tnom` | `f64 = 27.0` | °C | Temperature at which `r` holds. |
| `temp`, `dtemp` | the same | section 2.7 | | |
| `m` | `m` | `Option<f64>` | | Parallel multiplier. |
| `scale` | `scale` | `f64 = 1.0` | | Divides the conductance: `g = m / (r · factor · scale)`. |
| `noisy` | `noisy`, `noise` | `bool = true` | | Whether the resistor has noise. |
| `kf`, `af`, `ef` | the same | `0.0`, `1.0`, `1.0` | | Flicker noise: `kf · I^af / f^ef`. |
| `bv_max` | `bv_max` | `Option<f64>` | V | Safe-operating-area limit; exceeded, a warning record is written. |

`SemiconductorResistor`: by geometry. It has every param of `Resistor` except that `r` is optional, plus:

| Param | ngspice | Type and default | Unit | Meaning |
| --- | --- | --- | --- | --- |
| `rsh` | `rsh` | `f64`, required | Ω/□ | Sheet resistance. |
| `l` | `l` | `f64`, required | m | Drawn length. |
| `w` | `w` (default: model `defw`) | `f64 = 10u` | m | Drawn width. |
| `narrow` | `narrow`, `dw` | `f64 = 0.0` | m | Narrowing per side: effective width `w − 2·narrow`. |
| `short` | `short`, `dlr` | `f64 = 0.0` | m | Shortening per side: effective length `l − 2·short`. |
| `r` | `r` | `Option<f64>` | Ω | Given, it is the resistance, and the geometry sizes the flicker noise only. |
| `lf`, `wf` | the same | `1.0`, `1.0` | | Exponents of the flicker-noise area `(l−2·short)^lf · (w−2·narrow)^wf`. |

Each combination ngspice accepts lands in one module: a resistor with a model card that has `rsh`, or with `l` and `w`, is a `SemiconductorResistor`; any other is a `Resistor`.

**Alternatives considered.** One module with `r`, `rsh`, `l`, `w` all optional and ngspice's precedence rule in prose: the common case then carries eight parameters that mean nothing, and "one of the two must be given" is a run-time message, not the signature. A `model` struct param inside the resistor: see D2.

### 3.2 Capacitor

```rust
let c1 = Capacitor(a, b) { c: 100n };
let c2 = Capacitor(a, b) { c: 1p, ic: 1.8 };                        // starts a transient charged to 1.8 V

pub alias CMim = SemiconductorCapacitor { cj: 2m, cjsw: 0.1n };     // 2 fF/µm², 0.1 fF/µm
let c3 = CMim(a, b) { l: 10u, w: 10u };
let c4 = CMim(a, b) { l: 10u, w: 10u, cj: 2.2m };                   // this one with another density
```

`Capacitor`: `c` (F, required), `ic: Option<f64>` (V), `tc1`, `tc2`, `tnom`, `temp`, `dtemp`, `m`, `scale`, `bv_max`. `SemiconductorCapacitor` adds `cj` (F/m², bottom), `cjsw` (F/m, sidewall), `l`, `w` (m; `w` defaults to 10u), `narrow`, `short`, `del` (m), and, in place of `cj`, `di` (relative permittivity) with `thick` (m): the same split and the same reasons as the resistor.

### 3.3 Inductor and coupling

```rust
let l1 = Inductor(a, b) { l: 10u };
let l2 = Inductor(c, d) { l: 40u, ic: 1m };
let k1 = MutualInductor(l1.link, l2.link) { k: 0.98 };              // K1 L1 L2 0.98
```

`Inductor`: `l` (H, required), `ic: Option<f64>` (A), `tc1`, `tc2`, `tnom`, `temp`, `dtemp`, `m`, `scale`. `GeometricInductor` computes the inductance from `nt` (turns), `dia` and `length` (m), `csect` (m²) and `mu` (relative permeability), as ngspice's inductor model does. `MutualInductor` has one param, `k`, `where k.abs() <= 1.0`.

`K` in ngspice names two inductors. In MSL a module reaches another only through a node, so each inductor has a public node, `link`, and the coupling connects two of them. Three inductors on one core are three `MutualInductor`s, as in SPICE. This is decision D10.

### 3.4 Uniform RC line

`UniformRcLine(p1, p2, reference)` is ngspice's URC: at elaboration it becomes a ladder of resistor and capacitor lumps whose sizes grow geometrically toward the middle, by the constant `k`. Its params: `l` (m, required), `n: Option<u32>` (lumps; left out, computed from `fmax` as ngspice does), `k: f64 = 1.5`, `fmax: f64 = 1G`, `rperl: f64 = 1k` (Ω/m), `cperl: f64 = 1p` (F/m), and `isperl`, `rsperl` (`Option`): given, the capacitors are reverse-biased diodes. `l` and `n` fix the netlist, so they are structural: a host changes them by elaborating again.

---

## 4. Sources

### 4.1 Independent sources

One module for a voltage source and one for a current source. The DC value and the small-signal stimulus are params; the time-domain waveform is the optional third connection.

```rust
let vsupply = VoltageSource(vdd, gnd) { dc: 1.8 };
let vin = VoltageSource(inp, gnd) { dc: 0.9, acmag: 1.0 };           // V1 inp 0 DC 0.9 AC 1
let ibias = CurrentSource(vdd, bias) { dc: 10u };

let step = pulse() { v1: 0.0, v2: 1.8, tr: 50p, tf: 50p, pw: 1n, per: 2n };
let vclk = VoltageSource(clk, gnd, step);                            // V2 clk 0 PULSE(0 1.8 0 50p 50p 1n 2n)

let tone = sine() { vo: 0.9, va: 10m, freq: 1M };
let vsig = VoltageSource(sig, gnd, tone) { acmag: 1.0 };
```

`VoltageSource(p, n, waveform: Option<f64>)` and `CurrentSource(p, n, waveform: Option<f64>)`:

| Param | ngspice | Type and default | Unit | Meaning |
| --- | --- | --- | --- | --- |
| `dc` | `dc` | `Option<f64>` | V or A | DC value. Left out with a waveform, the DC value is the waveform at time 0, as in ngspice. Left out with none, 0. |
| `acmag` | `acmag`, `ac` | `f64 = 0.0` | V or A | Small-signal magnitude. |
| `acphase` | `acphase`, `ac` | `f64 = 0.0` | degrees | Small-signal phase. |
| `m` | `m` (current source only) | `Option<f64>` | | Parallel multiplier. |

The waveform is a value: any `f64` expression. The library gives ngspice's eight functions as modules with a result. In ngspice their parameters are positions in a list; each param here has the name ngspice's sources and manual give that position.

| Waveform | ngspice | Params (ngspice's order) |
| --- | --- | --- |
| `pulse` | `PULSE(V1 V2 TD TR TF PW PER NP)` | `v1`, `v2` required; `td = 0.0`; `tr`, `tf` required and positive; `pw`, `per: Option<f64>` (left out: the pulse does not fall, does not repeat); `np: Option<u32>` |
| `sine` | `SIN(VO VA FREQ TD THETA PHASE)` | `vo`, `va`, `freq` required; `td = 0.0`; `theta = 0.0` (1/s); `phase = 0.0` (degrees) |
| `exponential` | `EXP(V1 V2 TD1 TAU1 TD2 TAU2)` | `v1`, `v2`, `tau1`, `td2`, `tau2` required; `td1 = 0.0` |
| `piecewise_linear<const N: usize>` | `PWL(T1 V1 … ) r= td=` | `points: [(f64, f64); N]` required; `r: Option<f64>` (repeat from this time); `td = 0.0` |
| `frequency_modulated` | `SFFM(VO VA FC MDI FM TD PHASEM PHASEC)` | `vo`, `va`, `fc`, `fm` required; `mdi = 90.0` (limited to `fc / fm`, as ngspice); `td = 0.0`; `phasem`, `phasec = 0.0` (degrees) |
| `amplitude_modulated` | `AM(VO VMO VMA FM FC TD PHASEM PHASEC)` | `vo`, `vmo`, `fm`, `fc` required; `vma = 1.0`; `td`, `phasem`, `phasec = 0.0` |
| `transient_noise` | `TRNOISE(NA NT NALPHA NAMP RTSAM RTSCAPT RTSEMT)` | `na`, `nt` required; `nalpha`, `namp`, `rtsam`, `rtscapt`, `rtsemt: Option<f64>`; `seed: u64 = 1` |
| `transient_random` | `TRRANDOM(TYPE TS TD PARAM1 PARAM2)` | `kind: RandomKind` (`Uniform`, `Gaussian`, `Exponential`, `Poisson`), `ts` required; `td = 0.0`; `param1 = 1.0`, `param2 = 0.0`; `seed: u64 = 1` |

**Two places where ngspice's default cannot be kept.** ngspice takes the default of `tr`, `tf`, `pw`, `per`, `tau1`, `freq` and others from the `.tran` card (the print step and the stop time). A model does not know the run's stop time, and a waveform that changes with the analysis card is a trap. Where ngspice's default is the print step the param is required; where it is the stop time, left out means "never" or the param is required (the table says which). The two random waveforms take a `seed`, so a run is reproducible (§0.1 rule 1); ngspice seeds from the clock unless told otherwise, so parity for them compares statistics, not samples.

**Every corner of a waveform is a time point.** `pulse` and `piecewise_linear` schedule their corners with `at`, so the solver lands on each one, as ngspice does with breakpoints.

**Choices made here.**

- *One source module and separate waveforms*, in place of one module per waveform (`PulseVoltageSource`, `SineVoltageSource`, …). Ten modules in place of eighteen; a waveform is a value a user can add to another, scale, or write themselves; and the same `pulse` drives a voltage source, a current source or a behavioural expression.
- *Waveform parameters are params of the waveform module* (`pulse() { v1: 0.0, v2: 1.8, tr: 50p, tf: 50p }`), not positional arguments, so the host lists, sets and sweeps them by name (`top.step.v2`) and each has a `where`.

### 4.2 Behavioural sources

ngspice's B element is a source whose value is an expression over node voltages and branch currents. In MSL an expression is what a value input takes (§8.1.13):

```rust
let b1 = BehavioralVoltageSource(out, gnd, 2.0 * v(a) * v(b));       // B1 out 0 V = 2*v(a)*v(b)
let b2 = BehavioralCurrentSource(n1, n2, 1m * v(ctl).tanh());        // B2 n1 n2 I = 1m*tanh(v(ctl))
```

Params, as ngspice: `tc1`, `tc2` (`0.0`), `reciproctc: bool = false`, `temp`, `dtemp`, `m`, `reciprocm: bool = false`. The expression is MSL: the functions of ngspice's expression parser have MSL spellings, listed in the Book's translation table. A polynomial source (`POLY(n)`) is such an expression.

### 4.3 Linear controlled sources

```rust
let e1 = VoltageControlledVoltageSource(out, gnd, inp, inn) { gain: 1e5 };        // E1 out 0 inp inn 1e5
let g1 = VoltageControlledCurrentSource(out, gnd, inp, inn) { gain: 2m };         // G1 out 0 inp inn 2m

// F1 out 0 Vsense 10   with   Vsense a b 0
let f1 = CurrentControlledCurrentSource(out, gnd, a, b) { gain: 10.0 };
// H1 out 0 Vsense 1k
let h1 = CurrentControlledVoltageSource(out, gnd, a, b) { gain: 1k };
```

Ports `(p, n, cp, cn)` for the voltage-controlled pair: output, then control. Ports `(p, n, sp, sn)` for the current-controlled pair: `(sp, sn)` is the device's own sensing branch, a short whose current is the control. Where ngspice inserts a 0 V source and names it, here the device is placed in the path (decision D11). Params: `gain` (required), and `m: Option<f64>` on the two current outputs.

---

## 5. Switches

```rust
let s1 = VoltageControlledSwitch(a, b, ctl, gnd) { vt: 0.9, vh: 0.1, ron: 1.0, roff: 1G };
let w1 = CurrentControlledSwitch(a, b, x, y) { it: 1m, ron: 10m };
```

| Param | ngspice | Type and default | Unit | Meaning |
| --- | --- | --- | --- | --- |
| `vt` (`it` for the current switch) | `vt`, `it` | `f64 = 0.0` | V, A | Threshold. |
| `vh` (`ih`) | `vh`, `ih` | `f64 = 0.0` | V, A | Hysteresis: closes above `vt + vh`, opens below `vt − vh`. |
| `ron` | `ron` | `f64 = 1.0` | Ω | Resistance when closed. |
| `roff` | `roff` | `f64 = 1T` | Ω | Resistance when open (ngspice: 1/gmin). |
| `initial` | `on`, `off` | `Option<SwitchState>` | | State before the operating point is known; left out, it follows the control value. |

The switch keeps its state in a signal and changes it at the crossing of the control, which the solver locates. ngspice changes it at the first accepted step past the crossing; the two agree to the step size, and section 11 lists it.

---

## 6. Transmission lines

```rust
let t1 = LosslessTransmissionLine(a, gnd, b, gnd) { z0: 50.0, td: 1n };               // T1 a 0 b 0 Z0=50 TD=1n
let t2 = LosslessTransmissionLine(a, gnd, b, gnd) { z0: 75.0, f: 1G, nl: 0.25 };      // a quarter wave at 1 GHz
let o1 = LossyTransmissionLine(a, gnd, b, gnd) { r: 10.0, l: 250n, c: 100p, len: 0.1 };
let p1 = CoupledTransmissionLines::<2>([a1, a2], [b1, b2]) {
    r: [[1.0, 0.0], [0.0, 1.0]], l: [[300n, 60n], [60n, 300n]],
    g: [[0.0, 0.0], [0.0, 0.0]], c: [[100p, -10p], [-10p, 100p]], length: 0.05,
};
```

| Module | ngspice | Ports | Params |
| --- | --- | --- | --- |
| `LosslessTransmissionLine` | T (`tra`) | `p1, n1, p2, n2` | `z0` (Ω, required); `td: Option<f64>` (s) or `f: Option<f64>` (Hz) with `nl: f64 = 0.25`, one of the two (`where`); `v1`, `i1`, `v2`, `i2: Option<f64>` initial conditions |
| `LossyTransmissionLine` | O (`ltra`) | `p1, n1, p2, n2` | `r`, `l`, `g`, `c` per metre (`0.0`), `len` (m, required); the step-control and interpolation options of the LTRA model as enums (`interpolation: LineInterpolation`, `step_control: LineStepControl`), `compactrel`, `compactabs` |
| `SingleLossyTransmissionLine` | Y (`txl`) | `p1, p2`, referred to ground as in ngspice | `r`, `l`, `g`, `c` per metre, `length` (m, required) |
| `CoupledTransmissionLines<const N: usize>` | P (`cpl`) | `near: [Electrical; N], far: [Electrical; N]`, referred to ground as in ngspice | `r`, `l`, `g`, `c: [[f64; N]; N]`, symmetric matrices per metre (ngspice writes their lower triangle), `length` (m) |
| `UniformRcLine` | U (`urc`) | section 3.4 | |

`td` and `f`/`nl` are one number written two ways, so they are one module with a `where` that asks for one of them, unlike the resistor, where the two forms bring different parameters with them. The LTRA model's eleven flags (`lininterp`, `quadinterp`, `mixedinterp`, `nocontrol`, `steplimit`, `nosteplimit`, `truncnr`, `truncdontcut`) are mutually exclusive groups in ngspice; here each group is one enum param.

`LossyTransmissionLine` needs a convolution of the port quantities over the whole accepted history, which no MSL primitive gave. The feature adds it: the core primitive `convolve` (`design.md` §15), built as its own phase before the lines.

---

## 7. Diodes

```rust
let d1 = Diode(a, k) { is: 1e-14 };                                              // the ideal junction
let d2 = Diode(a, k) { is: 2.52n, rs: 0.568, n: 1.752, cjo: 4p, mj: 0.4, tt: 20n, bv: 100.0, ibv: 100u };

pub alias D1N4148 = Diode { is: 2.52n, rs: 0.568, n: 1.752, cjo: 4p, mj: 0.4, tt: 20n, bv: 100.0, ibv: 100u };
let d3 = D1N4148(a, k);

/// A junction of a process, by area and perimeter.
pub alias NDiode = Diode { is: 1.2u, jsw: 0.3n, cjo: 0.9m, cjp: 0.25n, mj: 0.34, mjsw: 0.2, vj: 0.7 };
let d4 = NDiode(a, k) { area: 100p, pj: 40u };

let d5 = DiodeSelfHeating(a, k, tj) { is: 1e-12, rth0: 50.0, cth0: 1m };          // tj: a Thermal node
```

Four modules from ngspice's one device:

| Module | ngspice | Ports | For |
| --- | --- | --- | --- |
| `Diode` | level 1 | `a, k` | The junction scaled by `area` and `pj`. |
| `GeometricDiode` | level 3 | `a, k` | The junction scaled by drawn `l` and `w`, with the metal and polysilicon overlap capacitances (`lm`, `lp`, `wm`, `wp`, `xom`, `xoi`, `xm`, `xp`, `xw`). |
| `DiodeSelfHeating`, `GeometricDiodeSelfHeating` | `thermal` flag, `rth0`, `cth0` | `a, k, tj` | The same with a thermal terminal. |

The parameters of `Diode`, by what they do. Every one is listed; `models/diode.md` has ngspice's descriptions.

| Group | Params (unit, default) |
| --- | --- |
| Instance | `area` (1.0), `pj` (perimeter factor, 0.0), `m`, `off`, `ic` (V), `temp`, `dtemp` |
| DC current | `is` (A, 1e-14), `jsw` (A, 0.0, sidewall), `n` (1.0), `ns` (1.0, sidewall), `rs` (Ω, 0.0), `ikf`, `ikr`, `ikp: Option<f64>` (A, knee currents), `isr` (A, 1e-14, recombination) with `nr` (2.0) |
| Breakdown | `bv: Option<f64>` (V), `ibv` (A, 1m), `nbv: Option<f64>` (default `n`), `tcv` (1/K, 0.0) |
| Tunnelling | `jtun`, `jtunsw` (A, 0.0), `ntun` (30.0), `xtitun` (3.0), `keg` (1.0) |
| Charge | `cjo` (F, 0.0), `vj` (V, 1.0), `mj` (0.5; ngspice `m`), `cjp` (F, 0.0, sidewall), `php` (V, 1.0), `mjsw` (0.33), `fc`, `fcs` (0.5), `tt` (s, 0.0), `vp` (0.0, soft recovery) |
| Temperature | `tnom` (°C), `tlev`, `tlevc` (0, equation selectors), `eg` (eV, 1.11), `xti` (3.0), `gap1`, `gap2`, `trs`, `trs2`, `tm1`, `tm2`, `ttt1`, `ttt2`, `cta`, `ctp`, `tpb`, `tphp` |
| Noise | `kf` (0.0), `af` (1.0) |
| Safe operating area | `fv_max`, `bv_max`, `id_max`, `te_max`, `pd_max: Option<f64>` |

`rs` at zero: the diode has no internal node, as in ngspice (the param selects, so it is structural). `bv: Option<f64>`: left out, no breakdown.

**Choices made here.** Level 1 and level 3 are two modules for the reason the resistor is: level 3 is the same junction specified by geometry, and its nine layout parameters mean nothing at level 1. The parameter `level` therefore does not exist in MSL: the module is the level.

---

## 8. Bipolar transistors

```rust
pub alias Q2N2222 = Bjt {
    polarity: BjtPolarity::Npn, is: 14.34f, bf: 255.9, nf: 1.0, vaf: 74.03, ikf: 0.2847,
    ise: 14.34f, ne: 1.307, br: 6.092, rb: 10.0, rc: 1.0,
    cje: 22.01p, vje: 0.75, mje: 0.377, cjc: 7.306p, vjc: 0.75, mjc: 0.3416, tf: 411.1p, tr: 46.91n, xtb: 1.5,
};
let q1 = Q2N2222(c, b, e, gnd);                                    // Q1 c b e 2N2222   (substrate to ground)
```

| Module | ngspice | Ports | Model |
| --- | --- | --- | --- |
| `Bjt` | `.model … npn\|pnp`, level 1 | `c, b, e, s` | Gummel-Poon, with ngspice's extensions (quasi-saturation, lateral/vertical substrate). 10 instance and 123 model params. |
| `Vbic`, `VbicSelfHeating` | levels 4 and 9 | `c, b, e, s` (`dt`) | VBIC 1.2. 8 and 116. |
| `Hicum2`, `Hicum2SelfHeating` | level 8 | `c, b, e, s` (`tj`) | HICUM level 2. 9 and 139. |

**The substrate is always a port.** In ngspice the fourth node may be left out and is then ground; in MSL a port is never optional, so a three-terminal use passes `gnd`, as above.

Instance params of `Bjt`: `area` (1.0), `areab`, `areac` (`Option<f64>`, default `area`), `m`, `off`, `icvbe`, `icvce` (`Option<f64>`), `temp`, `dtemp`.

Main model params of `Bjt` (the full list of 123 is `models/bipolar.md`):

| Group | Params |
| --- | --- |
| Type | `polarity`, `subs: Option<BjtSubstrate>` (`Vertical`, `Lateral`; left out, vertical for NPN and lateral for PNP, as ngspice) |
| Forward and reverse current | `is` (A, 1e-16), `bf` (100), `nf` (1), `br` (1), `nr` (1), `ise`, `isc` (A, 0), `ne` (1.5), `nc` (2), `ibe`, `ibc: Option<f64>` |
| Early effect and high injection | `vaf`, `var: Option<f64>` (V), `ikf`, `ikr: Option<f64>` (A), `nkf` (0.5) |
| Resistances | `rb` (Ω, 0), `irb: Option<f64>` (A), `rbm: Option<f64>` (default `rb`), `re`, `rc` (0) |
| Junction charge | `cje`, `vje` (0.75), `mje` (0.33), `cjc`, `vjc`, `mjc`, `xcjc` (1), `cjs`, `vjs`, `mjs` (0), `fc` (0.5) |
| Transit time | `tf`, `xtf`, `vtf: Option<f64>`, `itf`, `ptf` (degrees), `tr` |
| Temperature | `tnom`, `eg` (1.11), `xti` (3), `xtb` (0), `tlev`, `tlevc`, and the per-parameter coefficients `tbf1`, `tbf2`, `trb1`, … |
| Noise | `kf` (0), `af` (1) |
| Quasi-saturation | `rco`, `vo`, `gamma`, `qco` |

`vaf: Option<f64>` is the example of 2.3: ngspice stores the inverse and uses 0 for "not given"; here, left out means no Early effect.

VBIC and HICUM keep their own parameter names (`ibei`, `ibci`, `rcx`, `c10`, `qp0`, …), which are those of their reference manuals.

---

## 9. JFETs, MESFETs and HFETs

```rust
pub alias J2N5457 = Jfet { polarity: JfetPolarity::Njf, vto: -1.5, beta: 1.2m, lambda: 3m, rd: 5.0, rs: 5.0, cgs: 4p, cgd: 2p, is: 1p };
let j1 = J2N5457(d, g, s);
```

| Module | ngspice | Ports | Model | Params (instance, model) |
| --- | --- | --- | --- | --- |
| `Jfet` | `njf\|pjf`, level 1 | `d, g, s` | Shichman-Hodges with the Sydney University extensions | 8, 23 |
| `Jfet2` | level 2 | `d, g, s` | Parker-Skellern | 8, 39 |
| `Mesfet` | `nmf\|pmf`, level 1 | `d, g, s` | Statz | 4, 14 |
| `Mesa` | levels 2, 3, 4 | `d, g, s` | the MESA models | 8, 61 |
| `Hfet1` | `nhfet\|phfet`, level 5 | `d, g, s` | HFET | 7, 64 |
| `Hfet2` | level 6 | `d, g, s` | HFET, simplified | 7, 36 |

Instance params of `Jfet`: `area` (1.0), `m`, `off`, `icvds`, `icvgs` (`Option<f64>`), `temp`, `dtemp`. Model params: `vto` (V, −2.0), `beta` (A/V², 1e-4), `lambda` (1/V, 0), `rd`, `rs` (Ω, 0), `cgs`, `cgd` (F, 0), `pb` (V, 1), `is` (A, 1e-14), `n` (1), `fc` (0.5), `b` (1, doping tail), `tnom`, `tcv`, `vtotc`, `bex`, `betatce`, `xti`, `eg`, `kf`, `af`, `nlev`, `gdsnoi`.

`Mesa` covers three ngspice levels with one parameter table and three sets of equations. It keeps a param `level: MesaLevel` (`Level2`, `Level3`, `Level4`), because the table is one and a card for one level is valid for another: the one device where a level stays a param. Where a level changes the parameters that exist, it is a module (the diode, the MOSFETs).

---

## 10. MOSFETs

### 10.1 One module per model

In ngspice a MOSFET's `level` (and, for BSIM3 and BSIM4, its `version`) selects one of 23 separate models with separate parameter tables. In MSL each is a module, so the parameters a module accepts are exactly those of its model, and a card for the wrong model does not compile.

| ngspice `level` | Module | Model | Params (instance, model) |
| --- | --- | --- | --- |
| 1 | `Mos1` | Shichman-Hodges | 16, 32 |
| 2 | `Mos2` | Grove-Frohman | 16, 39 |
| 3 | `Mos3` | semi-empirical | 16, 45 |
| 6 | `Mos6` | Sakurai-Newton | 16, 37 |
| 9 | `Mos9` | modified level 3 | 16, 43 |
| `vdmos` | `Vdmos`, `VdmosSelfHeating` | power MOSFET (`d, g, s`; `tj, tcase`) | 8, 64 |
| 4 | `Bsim1` | BSIM1 | 14, 79 |
| 5 | `Bsim2` | BSIM2 | 14, 135 |
| 8, 49 | `Bsim3`; `Bsim3V32`, `Bsim3V1`, `Bsim3V0` | BSIM3 v3.3.0; the earlier versions | 19, 423 |
| 14, 54 | `Bsim4`; `Bsim4V7`, `Bsim4V6`, `Bsim4V5` | BSIM4 v4.8; the earlier versions | 43, 891 |
| 55, 56, 57 | `Bsim3SoiFd`, `Bsim3SoiDd`, `Bsim3SoiPd` | BSIM3SOI | 16–25, 389–467 |
| 10, 58 | `Bsim4Soi` | BSIMSOI 4 | 37, 911 |
| 60 | `Soi3` | STAG | 25, 83 |
| 68 | `Hisim2` | HiSIM2 | 32, 544 |
| 73 | `HisimHv1`, `HisimHv2` | HiSIM_HV 1 and 2 | 38, 608–728 |

Bulk MOSFETs have the ports `(d, g, s, b)`. The bulk is always a port.

### 10.2 Levels 1 to 9

```rust
pub alias NCH = Mos1 { polarity: MosfetPolarity::Nmos, vto: 0.75, kp: 110u, gamma: 0.4, phi: 0.7, lambda: 0.04,
                       tox: 15n, cgso: 0.3n, cgdo: 0.3n, cj: 0.3m, mj: 0.5, cjsw: 0.25n, mjsw: 0.33 };
pub alias PCH = Mos1 { polarity: MosfetPolarity::Pmos, vto: -0.85, kp: 40u, gamma: 0.5, phi: 0.7, lambda: 0.05, tox: 15n };

module Inverter(inp: Electrical, outp: Electrical, vdd: Electrical) {
    let mp = PCH(outp, inp, vdd, vdd) { w: 20u, l: 1u };
    let mn = NCH(outp, inp, gnd, gnd) { w: 10u, l: 1u };
}
```

Instance params, the same for `Mos1`, `Mos2`, `Mos3`, `Mos6` and `Mos9`:

| Param | ngspice | Type and default | Unit | Meaning |
| --- | --- | --- | --- | --- |
| `w`, `l` | `w`, `l` | `f64 = 100u` | m | Channel width and length (ngspice's `defw`, `defl`). |
| `ad`, `as_` | `ad`, `as` | `f64 = 0.0` | m² | Drain and source diffusion areas. |
| `pd`, `ps` | `pd`, `ps` | `f64 = 0.0` | m | Drain and source perimeters. |
| `nrd`, `nrs` | `nrd`, `nrs` | `f64 = 1.0` | squares | Drain and source resistance in squares of `rsh`. |
| `m` | `m` | `Option<f64>` | | Parallel multiplier. |
| `off` | `off` | `bool = false` | | Start the iteration with the device off. |
| `icvds`, `icvgs`, `icvbs` | the same | `Option<f64>` | V | Held at the transient's operating point. |
| `temp`, `dtemp` | the same | section 2.7 | | |

Model params of `Mos1` (all 32):

| Group | Params |
| --- | --- |
| Type | `polarity` |
| Threshold and current | `vto`, `kp`, `gamma`, `phi: Option<f64>` (computed from the process params when left out), `lambda` (0) |
| Process | `tox: Option<f64>` (m), `nsub: Option<f64>` (cm⁻³), `nss` (cm⁻², 0), `tpg` (gate type, 1), `u0` (cm²/V·s, 600), `ld` (m, 0) |
| Parasitic resistance | `rd`, `rs: Option<f64>` (Ω), `rsh: Option<f64>` (Ω/□) |
| Junctions | `is` (A, 1e-14), `js` (A/m², 0), `pb` (V, 0.8), `cbd`, `cbs: Option<f64>` (F), `cj: Option<f64>` (F/m²), `mj` (0.5), `cjsw: Option<f64>` (F/m), `mjsw` (0.5), `fc` (0.5) |
| Overlap | `cgso`, `cgdo`, `cgbo` (F/m, 0) |
| Temperature | `tnom` |
| Noise | `kf` (0), `af` (1), `nlev` (2), `gdsnoi` (1) |

`vto`, `kp`, `gamma` and `phi` are the case "computed default" of 2.3: given, the value is used; left out, ngspice derives it from `tox`, `nsub`, `nss`, `u0` and `tpg`, and so does the module.

### 10.3 BSIM3 and BSIM4: nine hundred parameters

A BSIM4 card has up to 891 model parameters. No interface makes that small; what the interface can do is keep the cost in one place and make the use one line.

**The cost is paid once, in the card.** A process is a file of aliases, written by hand from the foundry's `.lib` or, later, by an importer. The card binds what the foundry gives and nothing else: the module's defaults are BSIM4's.

```rust
// pdk/transistors.msl
use std::spice::bsim::{Bsim4, MosfetPolarity};

/// Core NMOS, typical corner.
pub alias NCH = Bsim4 {
    polarity: MosfetPolarity::Nmos,
    toxe: 1.85n, toxp: 1.6n, vth0: 0.4, k1: 0.55, k2: -0.02, u0: 0.035, vsat: 1.2e5,
    rdsw: 150.0, cgso: 2.5e-10, cgdo: 2.5e-10, cjs: 1m, cjd: 1m,
    // … the rest of the card
};
```

**The use is one line**, and it reads as the SPICE line does.

```rust
let m1 = NCH(d, g, s, b) { w: 1u, l: 60n, nf: 2.0 };       // M1 d g s b nch W=1u L=60n NF=2
```

**A corner is an alias of an alias.**

```rust
pub alias NCH_SLOW = NCH { vth0: 0.44, u0: 0.032 };
pub alias NCH_FAST = NCH { vth0: 0.36, u0: 0.038 };
```

**Binning is an `if`.** ngspice selects among cards `nch.1`, `nch.2`, … by `lmin`, `lmax`, `wmin`, `wmax`. Here the selection is ordinary code (§8.4.7) over aliases, and a geometry outside every bin is an error the PDK author writes:

```rust
pub fn nch(w: f64, l: f64, nf: f64) -> Bsim4 {
    if l < 0.1u { NCH_BIN1 { w, l, nf } } else if l < 1u { NCH_BIN2 { w, l, nf } } else { NCH_BIN3 { w, l, nf } }
}
let m2 = nch(1u, 60n, 2.0)(d, g, s, b);
```

The interpolating binning parameters of a card (`lvth0`, `wvth0`, `pvth0`: the `l`, `w` and `p` variants of most parameters, which is why there are 891) are ordinary params and work inside one card as in ngspice.

**Mismatch and design-level variation are params of your own.** A placement's block may read the parent's params, so a value a design varies is bound where the device is placed: `NCH(d, g, s, b) { w: 1u, l: 60n, delvto: offset }`.

**Reading the list.** Nobody reads 891 parameters in a row. `models/bsim.md` has them in ngspice's order; the Book's BSIM chapter groups them (threshold, mobility, saturation, output resistance, gate current, junctions, overlap and intrinsic capacitance, layout-dependent stress, noise, temperature, the model selectors, the binning variants) and explains the groups and the selectors (`capmod`, `mobmod`, `rdsmod`, `igcmod`, `rgatemod`, `rbodymod`, `trnqsmod`, `acnqsmod`, `fnoimod`, `tnoimod`, `geomod`, `permod`, …), which are the parameters a user of a foundry card actually touches.

Instance params of `Bsim4` (all 43 in `models/bsim.md`): `w`, `l` (m, 5u), `nf` (1), `m`, `ad`, `as_`, `pd`, `ps`, `nrd`, `nrs`, `sa`, `sb`, `sd`, `sca`, `scb`, `scc`, `sc` (stress and well proximity), `min`, `delvto`, `mulu0`, `xgw`, `ngcon`, `rbdb`, `rbsb`, `rbpb`, `rbps`, `rbpd`, `rbodymod`, `rgatemod`, `geomod`, `rgeomod`, `trnqsmod`, `acnqsmod`, `wnflag`, `mult_i`, `mult_q`, `mult_fn`, `off`, `icvds`, `icvgs`, `icvbs`, `dtemp`.

**The versions.** `Bsim4` is version 4.8, what `.model … level=54` gives in ngspice-47 with no `version`. A card written for 4.5, 4.6 or 4.7 says so (`version=4.5`), and ngspice then runs a different model; here it is a different module (`Bsim4V5`). The parameter `version` therefore does not exist: the module is the version. The same holds for `Bsim3` (3.3.0) and `Bsim3V32`, `Bsim3V1`, `Bsim3V0`.

### 10.4 SOI and HiSIM

These follow the same rules. Two things are particular; the planner of their phase decides them in the family's page and reports:

- the SOI models take four to seven terminals in ngspice (`d, g, s, e`, then an optional body contact `p`, body `b` and thermal node `t`). Each terminal list ngspice accepts is a module (`Bsim4Soi`, `Bsim4SoiBodyContact`, …); the list is fixed in the SOI family's page;
- HiSIM_HV has its substrate and thermal nodes as options of the same kind.

---

## 11. What differs from ngspice, on purpose

A user who knows ngspice should read this list once. Each line is in the Book.

| ngspice | `spice` in MSL | Why |
| --- | --- | --- |
| `.model` and instance parameters are two lists | one list of params on the module; a card is an `alias` | one mechanism, for every module |
| `level`, `version` select a model | the module is the model | a card cannot be given to the wrong model |
| `.options tnom`, `defw`, `defl`, `defad`, `defas`, `scale` | params with ngspice's built-in defaults | MSL has no circuit-wide model option |
| `uic` and `ic=` | `ic` holds the branch at the transient's operating point | §16 H6, H7 |
| `tr`, `tf`, `pw`, `per`, … default to the print step or the stop time | required, or "never" | a waveform does not depend on the analysis card |
| F, H and W name a voltage source | the device has its own sensing branch | a module reaches another only through nodes |
| K names two inductors | `MutualInductor(l1.link, l2.link)` | the same |
| an omitted substrate or bulk node is ground | the port is always connected; pass `gnd` | a port is never optional |
| an optional thermal node | a module with the thermal terminal | the same |
| `.disto` | a transient and the host's Fourier analysis | §16.3 |
| `.sens` flags on a device (`sens_l`, …) | the host's gradients over every param | §21 |
| a switch changes state at the first step after the threshold | at the located crossing | the solver locates events |
| random sources seeded from the clock | a `seed` param | a run is a function of its source |
| a model's hand-written derivative | the derivative of the model's own equations | §11 rule 5; a difference that shows is recorded |
| `temp`, `tnom` in °C; `acphase` in degrees | the same | D4 |

---

## 12. Decisions (recorded 2026-11-27)

| # | Question | Decided |
| --- | --- | --- |
| D1 | Parameter names | ngspice's names for params; full words for everything else (2.1). |
| D2 | Where model parameters live | Flat params on the device module; no `model` struct (2.5). |
| D3 | Instance parameters on a shared card | **The user's amendment:** the `alias` item. A placement through an alias binds what is left and may replace what was bound (2.5; `design.md` §12). The restriction on template *values* (`G.53`) stays. A function returning a template stays valid and is the form for binning. |
| D4 | Units of temperature and phase | ngspice's: °C and degrees (2.2). |
| D5 | Where the models live | **The user's change:** in the standard library, `std::spice`, shipped with the language; no dependency line (1; `design.md` §13). |
| D6 | CIDER numerical devices and `ndev` | **Not provided.** Listed by name as excluded; no follow-up. |
| D7 | XSPICE code models | **Not provided.** Listed by name as excluded; no follow-up. |
| D8 | Legacy BSIM3 and BSIM4 versions | In scope, each a module, delivered right after its current version. |
| D9 | Reference version | ngspice-47. |
| D10 | Coupled inductors | A public `link` node on every `Inductor` (3.3). |
| D11 | The controlling current of F, H, W | The device's own sensing branch (4.3). |
| D12 | Order of delivery | **The user's change:** from the simplest to the most complex, each phase a delivery (`design.md` §9). |
| Licence | ngspice's Modified BSD notice | Approved: the notice at the root of the spice module of the standard library; each model's description names its ngspice source and the notice. |
| Family pages | Units, defaults and given-ness of the large models | Decided by the planner of each phase and recorded in `interface/<family>.md`; the user is informed. |
| Lossy line | The missing capability | Created in this feature: `convolve` (`design.md` §15). |

The order of delivery: 1 resistors, capacitors, DC and AC sources, controlled sources; 2 inductors, waveforms, behavioural sources; 3 switches and the lossless line; 4 diodes and the RC line; 5 JFETs and the Statz MESFET; 6 the bipolar transistor; 7 MOS levels 1 to 9; 8 VDMOS, MESA, HFET; 9 the `convolve` primitive; 10 lossy and coupled lines; 11 BSIM1 and BSIM2; 12 BSIM3; 13 VBIC and HICUM; 14 BSIM4; 15 SOI; 16 HiSIM. Phase 0 before them is the ground: `alias`, the library's place in `std`, the parity machinery.

### The per-family choices, accepted

| # | Choice | Section |
| --- | --- | --- |
| C1 | A resistor, capacitor or inductor by value and one by geometry are two modules | 3.1–3.3 |
| C2 | One source module; waveforms are separate modules with a result and their own params | 4.1 |
| C3 | Parameters ngspice defaults from the `.tran` card are required or mean "never" | 4.1 |
| C4 | The B source takes its expression as a value input | 4.2 |
| C5 | A switch's `on`/`off` flags are one `initial: Option<SwitchState>` | 5 |
| C6 | `td` or `f`/`nl` of a lossless line: one module with a `where` | 6 |
| C7 | The LTRA option flags become enum params | 6 |
| C8 | Diode level 1 and level 3 are `Diode` and `GeometricDiode`; no `level` param | 7 |
| C9 | A device with a thermal terminal is a `…SelfHeating` module | 2.7 |
| C10 | The substrate and the bulk are always ports | 8, 10 |
| C11 | Polarity is an enum param per family, with ngspice's keywords | 2.6 |
| C12 | Each MOSFET level and each BSIM version is a module; no `level` or `version` param | 10 |
| C13 | `Mesa` keeps `level` as an enum param | 9 |
| C14 | The renamed parameters of 2.1 | 2.1 |
| C15 | `ic` holds a branch at the transient's operating point | 2.9 |
| C16 | Safe-operating-area limits write a warning record; they never change a result | 3.1 |
