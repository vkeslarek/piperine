# Spice Models Design

**Spec**: `.specs/features/spice-models/spec.md`
**Status**: Approved
**Approved**: 2026-11-27 - user (translated, relayed by the coordinator): "I accept the recommended options, except what I told you (no CIDER and no XSPICE). … Please write the design and break it into tasks!" The full words and the decision log are in `spec.md`.

The design has six parts, in the order the request gives them: the parameter and usage map (§4), the interface (§2, §3), the implementation map (§5), the parity plan (§6), the Book (§7), and the phases and risks (§9, §10). §8 lists what the models need of the language, the solver and the device ABI. §12 is the `alias` amendment, §13 how the library ships in `std`, §14 who decides a family's page, and §15 the `convolve` primitive the lossy line needs. `tasks.md` is the plan (148 tasks in 18 phases).

The feature executes after `code-clarity` and `device-abi-simplification` have merged. Paths and crate names below are those the two features leave (`piperine-language`, `piperine-interpreter`, `piperine-repository-guards`); T1 re-takes every path on the merged `main`.

---

## 1. Overview

One model goes through five steps, and a family is a batch of models that go through them together.

```mermaid
flowchart LR
    S["ngspice-47 sources\n<dev>.c, *set.c, *temp.c, *load.c, *noi.c"] --> M["Map\nmodels/<family>.md\n(generated + the mapping task)"]
    M --> I["Interface\nthe family's page,\ndecided by the phase's planner"]
    I --> C["MSL module\nstd/spice/<family>"]
    C --> P["Parity twins\nthe model's matrix,\nboth backends"]
    P --> B["Book chapter\nlistings run, tables generated"]
```

Four things are settled once, before the first family: the `alias` item (§12), the library's place in `std` and its rules (§2, §3, §13), the parity scaffold with its gates (§6), and the capabilities the later families need (§8).

**What makes the work tractable.** In ngspice a device is some thirty C files: one function per analysis, each with hand-written derivatives. In MSL a device is one module: its params, one `analog` block that states the currents, the charges and the noise sources, and for some devices a `digital` block. The derivatives, the AC and pole-zero stamps, the truncation-error control and the sensitivities come from the language. Of the 376,000 lines of C behind the in-scope devices, what a model must restate is the setup and temperature code (the params and the body `let`s) and the current and charge part of the load function. Appendix D of the language specification shows the size this gives: a junction diode is about ten lines.

---

## 2. The library

### 2.1 Location

The models are a namespace of the standard library (decision D5; §13 gives the reasoning and the loading rule).

```
crates/piperine-language/std/
    lib.msl                       gains `pub mod spice;` (not re-exported by `prelude`)
    spice/
        mod.msl                   the namespaces, pub; `pub const NGSPICE_VERSION: str = "ngspice-47";`
        NOTICE                    ngspice's Modified BSD notice and the list of the ngspice files the library derives from
        constants.msl             ngspice's physical constants
        temperature.msl           the instance-temperature rule of interface-review §2.7
        junction.msl              what the junction devices share: depletion charge, the temperature laws, critical voltage
        passive.msl
        source.msl
        controlled.msl
        switch.msl
        line.msl
        diode.msl
        bipolar/{mod,bjt,vbic,hicum2}.msl
        jfet.msl
        mesfet/{mod,mesfet,mesa,hfet1,hfet2}.msl
        mosfet/{mod,mos1,mos2,mos3,mos6,mos9,vdmos,meyer}.msl
        bsim/{mod,bsim1,bsim2,bsim3,bsim3_v32,bsim3_v1,bsim3_v0,bsim4,bsim4_v7,bsim4_v6,bsim4_v5}.msl
        soi/{mod,bsim3_soi_fd,bsim3_soi_dd,bsim3_soi_pd,bsim4_soi,soi3}.msl
        hisim/{mod,hisim2,hisim_hv1,hisim_hv2}.msl
```

A module and its behavior blocks are in one file (§8 rule 5), so one device is one file, and a large one (BSIM4) is a large file. What devices share is in functions of `junction.msl`, `temperature.msl` and `mosfet/meyer.msl`; a `fn` keeps no state (§3), so sharing costs nothing at run time. Every file is added to the embedded table of the standard library (`pom/stdlib.rs` today) in the commit that adds it.

### 2.2 Namespaces

`spice/mod.msl` declares one `pub mod` per row of `interface-review.md` §1 and re-exports nothing: a user writes `use std::spice::diode::Diode;`. No item of `std::spice` is in the prelude: `Diode` and `Resistor` are common names in a user's own code and in the specification's examples, and the path `std::spice` is what says where a model comes from. The shared namespaces (`constants`, `junction`, `temperature`) are `pub`, so a user's own model can use ngspice's temperature laws and constants.

`std::spice::constants` holds ngspice's values, which differ from `std::constants` in the last digits: `CHARGE = 1.6021766208e-19`, `BOLTZMANN = 1.38064852e-23`, `CELSIUS_TO_KELVIN = 273.15`, `REFERENCE_TEMPERATURE = 300.15`, and the derived `BOLTZMANN_OVER_CHARGE`, `ROOT_TWO`, `VT0` (`src/include/ngspice/const.h`). A model reads these, never `std::constants::Q` or `K_B`, so a thermal voltage has ngspice's digits.

### 2.3 What the library does not hold

No `extern` module, no intrinsic, no Rust, no host code, no netlist reader, no model card. It is MSL source that the ordinary pipeline elaborates, plans and runs on both backends, written under the rules a user's package is written under.

---

## 3. The interface rules

`interface-review.md` shows these rules at work and asks for the decisions. This section states them as the implementer follows them. Where a rule depends on a decision, it is written on the recommended side and the decision is named.

### 3.1 One module per model

1. Each ngspice device whose `level` or `version` selects a separate parameter table is a module per table (D8 for the legacy versions). `level` and `version` are then not params.
2. A device whose levels share one parameter table keeps the level as an enum param (`Mesa`, `level: MesaLevel`).
3. A device that accepts two sets of required parameters is two modules, each requiring its own set (resistor, capacitor, inductor, diode levels 1 and 3). The module that takes the geometry accepts everything the other does, so each combination of parameters ngspice accepts is expressible in exactly one of them.
4. Each list of terminals a device accepts is a module (`…SelfHeating`, the SOI body-contact variants), because a carrier port is never optional (§8.1.8). The variants of one device share their equations through functions; the thermal variant differs by reading its temperature from a node.
5. A terminal ngspice defaults to ground (the BJT's substrate) is a port; the user passes `gnd`.

### 3.2 Names

1. A param's name is the name of the first entry of the ngspice table for that quantity, in lower case. Aliases (the entries ngspice marks redundant) are not names.
2. The rename table is closed: `as` to `as_`, `pub` to `pub_` (keywords); `ic-vds`, `ic-vgs` to `icvds`, `icvgs`; the model's `m` to `mj` (diode, VDMOS) and to `m_knee` (MESA, HFET1, HFET2); Mos6's model `ps` to `ps_sat`; `on` and `off` of the switches to `initial`; the type keywords and hicum's `type` to `polarity`. A guard compares every other param name with the ngspice table.
3. Where a name is in both the instance and the model table with one meaning (some 130 cases, most of them in BSIM4 and HiSIM_HV), the module has one param.
4. Module names, port names, namespaces, enums, variants and functions are full words, or the device's established name written as a word.
5. Ports: `p, n` for two-terminal elements; `a, k` for diodes; `c, b, e, s` for bipolar transistors; `d, g, s, b` for MOSFETs; `d, g, s` for JFETs and MESFETs; `cp, cn` for a controlling voltage; `sp, sn` for a current-sensing branch; `p1, n1, p2, n2` for a two-port line; `tj`, `dt`, `tcase` for thermal terminals, as their model names them.

### 3.3 Types

| ngspice type | MSL type |
| --- | --- |
| real | `f64` or `Option<f64>` |
| integer that counts (`n` of URC, `np` of a pulse) | `u32` or `Option<u32>` |
| integer that selects (`capmod`, `mobmod`, `tlev`, `nlev`, `noimod`) | `i32`, with a `where` listing the values the model accepts. An enum per selector would give 200 enums whose variants are numbers; the numbers are the documented vocabulary of each model. |
| flag | `bool` |
| a group of exclusive flags | one enum param |
| real vector that is a list of scalars (`ic`) | the scalar params that already exist |
| real vector that is data (`pwl`, the matrices of `cpl`) | an array param with a const-generic length |
| expression (`asrc`) | a value input |
| instance name (`control` of F, H, W; the inductors of K) | ports |
| string (`version`) | not a param: the module |

### 3.4 Defaults and given-ness

`models/*.md` classifies each parameter from the sources: `default`, `given-ness read`, `computed default`, `unresolved`. The mapping task of a family turns the classification into a declaration:

1. `default` becomes `name: f64 = <ngspice's constant>;`.
2. `computed default` becomes `name: Option<f64>;` and a body `let` that computes the effective value, in the order ngspice computes it.
3. `given-ness read` is read case by case: when the flag is read only to print a warning, the param takes rule 1; when it changes a result, rule 2.
4. `unresolved` (354 of 11,027 at extraction: the default is assigned outside the `if (!given)` idiom) is resolved by reading the setup function.
5. A parameter with no default in ngspice and no meaning when absent is required.
6. A default that ngspice takes from a circuit option (`defw`, `defl`, `tnom`) is ngspice's built-in value of that option.
7. A default that ngspice takes from the analysis card is refused as a default: the param is required or `Option` with "absent" given a meaning that does not depend on the analysis (interface-review §4.1).

### 3.5 `where`

1. A `where` is written where ngspice's setup or check function returns an error for the value. The diagnostic is the language's (`E0317`, `E0802`).
2. Where ngspice warns and continues, the model continues with the same value. Where ngspice replaces the value, the effective value is a body `let` that makes the same replacement.
3. One `where` per module states a rule that spans params ("`td` or `f`"), written on the last of them.
4. The range checks of BSIM3, BSIM4 and HiSIM (`b3check.c`, `b4check.c`, `hsm2set.c`) are fatal for some parameters and warnings for others; the mapping task lists which, and only the fatal ones are `where`.

### 3.6 Model cards

1. A card is an `alias` of the device module (§12): `pub alias D1N4148 = Diode { is: 2.52n, rs: 0.568, n: 1.752 };`.
2. A placement through the card binds what the card left and may replace what it bound: `NCH(d, g, s, b) { w: 10u, l: 1u }`.
3. A card that refines another is an alias of an alias: `alias NCH_SLOW = NCH { vto: 0.82 };`.
4. Binning is a function that returns a template (§8.4.7), choosing among aliases: `if l < 0.1u { NCH_1 { w, l } } else { NCH_2 { w, l } }`.
5. The library ships no cards. The Book and the parity fixtures have cards of public parts, each with its source.

### 3.7 Temperature, multiplicity, initial state

1. `spice::temperature::device_kelvin(temp, dtemp)` returns the instance temperature in kelvin: `temp + 273.15` when `temp` is given, else `temperature().unwrap_or(REFERENCE_TEMPERATURE) + dtemp`. Every model calls it; none reads `temperature()` itself.
2. Every temperature-dependent quantity is a body `let` over params and that temperature. It is a discrete expression (§8.2), so it is evaluated when a param or the run changes, not at every Newton iteration: this is ngspice's temperature function.
3. A self-heating variant computes the same quantities from `device_kelvin(…) + potential(tj)`; they are then continuous expressions, differentiated like any other, and the thermal node receives the dissipated power.
4. `m` follows §8.5.5: `m.or(multiplicity()).unwrap_or(DEFAULT_MULTIPLICITY)`.
5. `off` gives the seed of each `limit` instance of the model: 0 when `off`, the critical voltage otherwise (§18 rules 1 and 2, ngspice's `MODEINITJCT`).
6. `ic`-family params follow §16 H7: under `analysis(Transient) && analysis(Static)` the branch is a potential source at the given value.

### 3.8 What a model exposes

1. Each read-only quantity of the ngspice instance table that is not a node number or a sensitivity is a named `let` of the `analog` block with the ngspice name, hence an operating-point variable (§11 rule 6). Small-signal conductances are `ddx` of the current; capacitances are `ddx` of the charge.
2. Each noise source has the name ngspice's noise function gives it.
3. Internal nodes are `signal`s named as ngspice names them where the name is a word (`drain`, `source`, `base`, `collector`, `emitter`), and are not `pub`. A user's module cannot attach to them; the host observes them by path (§21).
4. Each param has a `///` text: one sentence of meaning, the unit in brackets, and "ngspice: `name`" when the MSL name differs.

### 3.9 ngspice parameters with no MSL param

The closed list. A guard fails when a settable ngspice parameter is neither a param nor here.

| Parameters | Devices | In MSL |
| --- | --- | --- |
| `sens_*` | every device | the host's gradients over every param (§21 rule 7) |
| `level`, `version` | diode, BJT, JFET, MESFET, MOSFET | the module (§3.1) |
| type keywords (`npn`, `pnp`, `nmos`, `pmos`, `njf`, `pjf`, `nmf`, `pmf`, `nhfet`, `phfet`, `vdmosn`, `vdmosp`, `nsoi`, `psoi`) | transistors | `polarity` |
| model keywords that only name the device (`d`, `r`, `c`, `l`, `sw`, `csw`, `ltra`, `txl`, `cpl`, `urc`) | their devices | the module |
| `ic` vectors | transistors, lines | the scalar `ic…` params |
| `pulse`, `sin`, `exp`, `pwl`, `sffm`, `am`, `trnoise`, `trrandom`, and `r`, `td` of a PWL | `vsrc`, `isrc` | the waveform modules |
| `ac` vector | `vsrc`, `isrc` | `acmag`, `acphase` |
| `distof1`, `distof2` | `vsrc`, `isrc` | refused: no `.disto` (§16.3); owner: none, by the language's design |
| `portnum`, `z0`, `pwr`, `freq`, `phase` | `vsrc` | refused: ports belong to the host's S-parameter analysis; owner: the host surface, a `GAPS.md` row |
| `external`, `snd` | `vsrc`, `isrc` | refused: a host drive (§23.1); owner: the same row |
| `control`, `inductor1`, `inductor2` | F, H, W, K | ports (D10, D11) |
| `pos_node`, `neg_node`, `pos_nodes`, `neg_nodes`, `dimension` | switches, `txl`, `cpl` | ports and the const generic `N` |
| `i`, `v` of `asrc` | B | the module (`BehavioralCurrentSource`, `BehavioralVoltageSource`) and its value input |
| `thermal` flag | diode, VDMOS | the `…SelfHeating` module |
| `on`, `off` | switches | `initial` |
| the eight exclusive option flags of `ltra` | `ltra` | two enum params |

---

## 4. The parameter and usage map

`models/<family>.md` is the map: thirteen files, one per family, every device of ngspice-47, every entry of its instance and model tables.

**How it is made.** `tools/extract.py <ngspice tree> inventory.json` reads each device directory: the `IFparm` tables (name, id, type, direction, redundancy, description), the terminal names, the `SPICEdev` entry points, the field each parameter is stored in (the `*par*.c` files), the default assigned under `if (!…Given)` anywhere in the directory, how many times the given flag is read outside the parameter files, the limiting functions called, the internal unknowns created, the noise source names and the length of the state vector. `tools/render.py inventory.json models/` writes the files. Both are committed; a test of the feature renders again from a checkout of the tag and compares (SPM-01).

**What the extraction does not give.** Units (the tables have none), the meaning of "absent" for each `Option`, and the 354 defaults assigned outside the idiom. These are the **mapping task** of each family: its output is the family's interface page (a hand-written table per device in the form of `interface-review.md` §3.1, with unit, type, default and meaning for every param), written and decided by the phase's planner before the family's modules (§14; SPM-06, SPM-42). The user is informed, not asked. The unit of each BSIM, HiSIM, VBIC and HICUM parameter is taken from the model's reference manual, which the page cites.

**Usage rules** are recorded per device in the same page: which parameters are required, which exclude each other, what a model selector switches, which terminals may be tied. For the families of `interface-review.md` §3 to §7 they are in that page now.

**Analyses.** §5's table gives, per device, the ngspice entry points. In MSL:

| ngspice entry | In MSL | Needs of the model |
| --- | --- | --- |
| `DEVload` (OP, DC, transient) | the `analog` block | the currents and the charges under `ddt` |
| `DEVacLoad`, `DEVpzLoad` | the small-signal rule over the same block (§11.1.4); pole-zero and transfer function are host analyses (§16.3) | nothing more |
| `DEVnoise` | `noise`, `white_noise`, `flicker_noise` instances | one per ngspice source |
| `DEVtemperature` | body `let`s over params and the instance temperature | §3.7 |
| `DEVtrunc` | the truncation-error test of each `ddt` state (§17.2.1) | `abstol` of a charge where ngspice uses `chgtol` |
| `DEVconvTest` | the solver's convergence test and the `limit` veto (§17.1, §18) | nothing; a deviation of path, never of answer |
| `DEVsetic` | §16 H7 | the `ic…` params |
| `DEVaccept` (breakpoints, line history) | `at`, `delay`, `convolve` | scheduled corners; §15 for the lossy line |
| `DEVsen*` | the host's gradients | nothing |
| `DEVdisto` | none: distortion is a transient and a Fourier analysis | nothing |
| `DEVsoaCheck` | a reaction `on settle` that writes a warning record, placed only when a limit is given | the `…_max` params |
| `DEVask` | operating-point variables | §3.8 |

A device for which ngspice has no noise entry (`mos6`, `mesa`, `hfet1`, `hfet2`, the lines) places no noise source (SPM-21).

---

## 5. The implementation map

One row per in-scope device, generated from the sources (the same extraction as §4). *Equations* names the files whose current and charge code the model restates; the AC, pole-zero and sensitivity files are listed only to show they exist and are not restated. *Internal unknowns* are ngspice's; a node that exists only when a resistance is positive is a `signal` whose series branch is a param-selected short (§11 rule 3). *States* is the length of ngspice's state vector, an upper bound on the charges and held values the model has.

| Device | Equations | Temperature | Noise sources | Limiting | Internal unknowns | States | AC | PZ | Noise | Trunc | IC | Accept | SOA |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `res` | `resload.c`, `respzld.c`, `ressload.c` | `restemp.c` | thermal, 1overf | — | — | — | yes | yes | yes | — | — | — | yes |
| `cap` | `capacld.c`, `capload.c`, `cappzld.c` | `captemp.c` | — | — | — | 2 | yes | yes | — | yes | yes | — | yes |
| `ind` | `indacld.c`, `indload.c`, `indpzld.c` | `indtemp.c`, `muttemp.c` | — | — | branch branch | 2 | yes | yes | — | yes | — | — | — |
| `vsrc` | `vsrcacld.c`, `vsrcload.c`, `vsrcpzld.c` | `vsrctemp.c` | — | — | branch branch, node res | — | yes | yes | — | — | — | yes | — |
| `isrc` | `isrcacld.c`, `isrcload.c` | `isrctemp.c` | — | — | — | — | yes | — | — | — | — | yes | — |
| `asrc` | `asrcacld.c`, `asrcload.c`, `asrcpzld.c` | `asrctemp.c` | — | — | branch branch | — | yes | yes | — | — | — | — | — |
| `vcvs` | `vcvsload.c`, `vcvspzld.c`, `vcvssld.c` | — | — | — | branch branch | — | yes | yes | — | — | — | — | — |
| `vccs` | `vccsload.c`, `vccspzld.c`, `vccssld.c` | — | — | — | — | — | yes | yes | — | — | — | — | — |
| `cccs` | `cccsload.c`, `cccspzld.c`, `cccssld.c` | — | — | — | — | — | yes | yes | — | — | — | — | — |
| `ccvs` | `ccvsload.c`, `ccvspzld.c`, `ccvssld.c` | — | — | — | branch branch | — | yes | yes | — | — | — | — | — |
| `sw` | `swacload.c`, `swload.c`, `swpzload.c` | — | — | — | — | — | yes | yes | yes | yes | — | — | — |
| `csw` | `cswacld.c`, `cswload.c`, `cswpzld.c` | — | — | — | — | — | yes | yes | yes | yes | — | — | — |
| `tra` | `traacld.c`, `traload.c` | `tratemp.c` | — | — | node i1, node i2, node int1, node int2 | — | yes | — | — | yes | — | yes | — |
| `ltra` | `ltraacld.c`, `ltraload.c` | `ltratemp.c` | — | — | node i1, node i2 | — | yes | — | — | yes | — | yes | — |
| `txl` | `txlload.c` | — | — | — | branch branch, branch branch1, branch branch2 | — | yes | — | — | — | — | — | — |
| `cpl` | `cplload.c` | — | — | — | — | — | — | — | — | — | — | — | — |
| `urc` | — | — | — | — | — | — | — | — | — | — | — | — | — |
| `dio` | `dioacld.c`, `dioload.c`, `diopzld.c` | `diotemp.c` | rs, id, 1overf, rsw, idsw, 1overfsw | limitlog, pnjlim | node internal, node internal_sw, node qp | 22 | yes | yes | yes | yes | yes | — | yes |
| `bjt` | `bjtacld.c`, `bjtload.c`, `bjtpzld.c` | `bjttemp.c` | rc, rb, re, ic, ib, 1overf | pnjlim | node collCX, node collector, node base, node emitter | 33 | yes | yes | yes | yes | yes | — | yes |
| `vbic` | `vbicacld.c`, `vbicload.c`, `vbicpzld.c` | `vbictemp.c` | rc, rci, rb, rbi, re, rbp, rs, ic, ib, ibep, iccp, 1overfbe, 1overfbep | limitlog, pnjlim | node collector, node base, node emitter, node substrate, node collCI, node baseBP, node baseBI, node xf1, node xf2 | 86 | yes | yes | yes | yes | yes | — | yes |
| `hicum2` | `hicum2acld.c`, `hicum2pzld.c` | `hicumL2temp.cpp` | rcx, rbx, rbi, re, rsu, iavl, ibci, ibep, ijbcx, ijsc, it, ibei, 1overfbe, 1overfre | limitlog, pnjlim | node collCI, node baseBP, node emitEI, node subsSI, node baseBI, node dT, node xf, node xf1, node xf2 | 126 | yes | yes | yes | yes | yes | — | yes |
| `jfet` | `jfetacld.c`, `jfetload.c`, `jfetpzld.c` | `jfettemp.c` | rd, rs, id, 1overf | fetlim, pnjlim | node source, node drain | 13 | yes | yes | yes | yes | yes | — | — |
| `jfet2` | `jfet2acld.c`, `jfet2load.c` | `jfet2temp.c` | rd, rs, id, 1overf | fetlim, pnjlim | node source, node drain | 19 | yes | — | yes | yes | yes | — | — |
| `mes` | `mesload.c`, `mespzld.c` | `mestemp.c` | rd, rs, id, 1overf | fetlim, pnjlim | node source, node drain | 13 | yes | yes | yes | yes | yes | — | — |
| `mesa` | `mesaload.c` | `mesatemp.c` | — | fetlim, pnjlim | node source, node drain, node gate, node gs, node gd | 20 | yes | yes | — | yes | yes | — | — |
| `hfet1` | `hfetload.c` | `hfettemp.c` | — | fetlim | node source, node drain, node gate, node gd, node gs | 24 | yes | yes | — | yes | yes | — | — |
| `hfet2` | `hfet2load.c` | `hfet2temp.c` | — | fetlim, pnjlim | node source, node drain | 13 | yes | yes | — | yes | yes | — | — |
| `mos1` | `mos1acld.c`, `mos1load.c`, `mos1pzld.c` | `mos1temp.c` | rd, rs, id, 1overf | fetlim, limvds, pnjlim | node drain, node source | 17 | yes | yes | yes | yes | yes | — | — |
| `mos2` | `mos2acld.c`, `mos2load.c`, `mos2pzld.c` | `mos2temp.c` | rd, rs, id, 1overf | fetlim, limvds, pnjlim | node internal#drain, node internal#source | 17 | yes | yes | yes | yes | yes | — | — |
| `mos3` | `mos3acld.c`, `mos3load.c`, `mos3pzld.c` | `mos3temp.c` | rd, rs, id, 1overf | fetlim, limvds, pnjlim | node internal#drain, node internal#source | 17 | yes | yes | yes | yes | yes | — | — |
| `mos6` | `mos6load.c` | `mos6temp.c` | — | fetlim, limvds, pnjlim | node drain, node source | 17 | — | — | — | yes | yes | — | — |
| `mos9` | `mos9acld.c`, `mos9load.c`, `mos9pzld.c` | `mos9temp.c` | rd, rs, id, 1overf | fetlim, limvds, pnjlim | node internal#drain, node internal#source | 17 | yes | yes | yes | yes | yes | — | — |
| `vdmos` | `vdmosacld.c`, `vdmosload.c`, `vdmospzld.c` | `vdmostemp.c` | rd, rs, id, 1overf | fetlim, limitlog, limvds, pnjlim | node drain, node gate, node source, node body_diode, node Tj, node Tc, branch VdevTemp, node cktTemp, node qp | 25 | yes | yes | yes | yes | yes | — | yes |
| `bsim1` | `b1acld.c`, `b1eval.c`, `b1ld.c` | `b1temp.c` | rd, rs, id, 1overf | fetlim, limvds, pnjlim | node drain, node source | 35 | yes | yes | yes | yes | yes | — | — |
| `bsim2` | `b2acld.c`, `b2eval.c`, `b2ld.c` | `b2temp.c` | rd, rs, id, 1overf | fetlim, limvds, pnjlim | node drain, node source | 35 | yes | yes | yes | yes | yes | — | — |
| `bsim3` | `b3acld.c`, `b3ld.c`, `b3pzld.c` | `b3temp.c` | rd, rs, id, 1overf | fetlim, limvds, pnjlim | node drain, node source, node charge | 17 | yes | yes | yes | yes | yes | — | yes |
| `bsim3v32` | `b3v32acld.c`, `b3v32ld.c`, `b3v32pzld.c` | `b3v32temp.c` | rd, rs, id, 1overf | fetlim, limvds, pnjlim | node drain, node source, node charge | 17 | yes | yes | yes | yes | yes | — | yes |
| `bsim3v1` | `b3v1acld.c`, `b3v1ld.c`, `b3v1pzld.c` | `b3v1temp.c` | rd, rs, id, 1overf | fetlim, limvds, pnjlim | node drain, node source, node charge | 18 | yes | yes | yes | yes | yes | — | — |
| `bsim3v0` | `b3v0acld.c`, `b3v0ld.c`, `b3v0pzld.c` | `b3v0temp.c` | rd, rs, id, 1overf | fetlim, limvds, pnjlim | node drain, node source, node charge | 18 | yes | yes | yes | yes | yes | — | — |
| `bsim4` | `b4acld.c`, `b4ld.c`, `b4pzld.c` | `b4temp.c` | rd, rs, rg, rbps, rbpd, rbpb, rbsb, rbdb, id, 1overf, igs, igd, igb, corl | fetlim, limvds, pnjlim | node drain, node source, node gate, node midgate, node dbody, node body, node sbody, node charge | 29 | yes | yes | yes | yes | yes | — | yes |
| `bsim4v7` | `b4v7acld.c`, `b4v7ld.c`, `b4v7pzld.c` | `b4v7temp.c` | rd, rs, rg, rbps, rbpd, rbpb, rbsb, rbdb, id, 1overf, igs, igd, igb, corl | fetlim, limvds, pnjlim | node drain, node source, node gate, node midgate, node dbody, node body, node sbody, node charge | 29 | yes | yes | yes | yes | yes | — | yes |
| `bsim4v6` | `b4v6acld.c`, `b4v6ld.c`, `b4v6pzld.c` | `b4v6temp.c` | rd, rs, rg, rbps, rbpd, rbpb, rbsb, rbdb, id, 1overf, igs, igd, igb | fetlim, limvds, pnjlim | node drain, node source, node gate, node midgate, node dbody, node body, node sbody, node charge | 29 | yes | yes | yes | yes | yes | — | yes |
| `bsim4v5` | `b4v5acld.c`, `b4v5ld.c`, `b4v5pzld.c` | `b4v5temp.c` | rd, rs, rg, rbps, rbpd, rbpb, rbsb, rbdb, id, 1overf, igs, igd, igb | fetlim, limvds, pnjlim | node drain, node source, node gate, node midgate, node dbody, node body, node sbody, node charge | 29 | yes | yes | yes | yes | yes | — | yes |
| `bsim3soi_fd` | `b3soifdacld.c`, `b3soifdld.c`, `b3soifdpzld.c` | `b3soifdtemp.c` | rd, rs, id, 1overf, fb | — | node drain, node source, node Body, node Temp | 25 | yes | yes | yes | yes | yes | — | — |
| `bsim3soi_dd` | `b3soiddacld.c`, `b3soiddld.c`, `b3soiddpzld.c` | `b3soiddtemp.c` | rd, rs, id, 1overf, fb | — | node drain, node source, node Body, node Temp | 25 | yes | yes | yes | yes | yes | — | — |
| `bsim3soi_pd` | `b3soipdacld.c`, `b3soipdld.c`, `b3soipdpzld.c` | `b3soipdtemp.c` | rd, rs, id, 1overf, fb | — | node drain, node source, node Body, node Temp | 25 | yes | yes | yes | yes | yes | — | — |
| `bsimsoi` | `b4soiacld.c`, `b4soild.c`, `b4soipzld.c` | `b4soitemp.c` | rd, rs, rg, id, 1overf, fb_ibs, fb_ibd, igs, igd, igb, rbsb, rbdb, rbody | — | node drain, node source, node Body, node Temp, node gate, node midgate, node dbody, node sbody | 38 | yes | yes | yes | yes | yes | — | yes |
| `soi3` | `soi3acld.c`, `soi3load.c` | `soi3temp.c` | rd, rs, id, 1overf | fetlim, limvds, soipnjlim | node drain, node source, branch branch, node tout1, node tout2, node tout3, node tout4 | 64 | yes | — | yes | yes | yes | — | — |
| `hisim2` | `hsm2acld.c`, `hsm2eval.c`, `hsm2ld.c` | `hsm2temp.c` | rd, rs, id, 1ovf, igs, igd, igb, ign | fetlim, limvds, pnjlim | node drain, node source, node gate, node dbody, node body, node sbody | 18 | yes | yes | yes | yes | yes | — | yes |
| `hisimhv1` | `hsmhvacld.c`, `hsmhveval.c`, `hsmhvld.c` | `hsmhvtemp.c` | rd, rs, id, 1ovf, ign | fetlim, limvds, pnjlim | node drain, node source, node gate, node dbody, node body, node sbody, node temp, node qi_nqs, node qb_nqs | 31 | yes | yes | yes | yes | yes | — | yes |
| `hisimhv2` | `hsmhv2acld.c`, `hsmhv2eval.c`, `hsmhv2ld.c` | `hsmhv2temp.c` | rd, rs, id, 1ovf, ign | fetlim, limvds, pnjlim | node drain, node source, node gate, node dbody, node body, node sbody, node temp, node qi_nqs, node qb_nqs | 31 | yes | yes | yes | yes | yes | — | yes |

### 5.1 How each ngspice mechanism is written in MSL

| ngspice | MSL | Reference |
| --- | --- | --- |
| a current stamped between two nodes | `i(a, b) +> expr;` | §11 rule 1 |
| a charge in the state vector, integrated by `NIintegrate` | `i(a, b) +> ddt(q);` | §14.5 |
| a capacitance integrated as `C·dV` (the Meyer model of `mos1` to `mos9`, `DEVqmeyer`) | `i(a, b) +> c * ddt(v(a, b));` with `c` the Meyer capacitance at the present solution | §14.5; risk R4 |
| a voltage source or an inductor branch | `v(a, b) +> expr;` | §11 rule 1 |
| a series resistance with an internal node, absent when the resistance is zero | `signal`, and `if rs > 0.0 { i(a, ai) +> v(a, ai) / rs; } else { v(a, ai) +> 0.0; }` | §11 rule 3, the `Diode` of §18 |
| `DEVpnjlim`, `DEVfetlim`, `DEVlimvds` | `limit(x, …)` with `std`'s `pnjlim`, `fetlim`, `limvds` | §18, §22.2 |
| `DEVlimitlog` (diode, VBIC, HICUM, VDMOS thermal and tunnel branches), `DEVsoipnjlim` | functions of `spice::junction`, transliterated from `devsup.c`, passed to `limit` | §14.7 |
| `MODEINITJCT`, `off` | the `seed` of `limit` | §18 rules 1, 2 |
| `CKTgmin` across a junction | `gmin() * v` | §14.4 |
| source stepping | `source_scale()` in the independent sources | §14.4 |
| the temperature function | body `let`s | §3.7 |
| hand-written derivatives (`gm`, `gds`, `capbd`, …) | nothing: the language differentiates (§11 rule 5); the reported values are `ddx` | §14.2 |
| white noise `4kT·g`, shot noise `2qI`, flicker `kf·I^af/f^ef` | `white_noise`, `flicker_noise`, or `noise(psd)` when the density has another shape | §14.8, §22.2 |
| correlated noise (induced gate noise of BSIM4 `tnoimod`) | one `noise` instance read in two contributions | §11.1.8 |
| excess phase (`ptf` of the BJT), NQS charge nodes (`BSIM3`, `BSIM4`, HiSIM) | internal nodes with their own equation, as ngspice has them | §11 rule 14 |
| breakpoints of a source | `at(t)` in the waveform module's `digital` block | §13 |
| the delay of a lossless line | `delay(x, td)` | §14.6 |
| a switch's state | a `signal` of `Bit`, changed by reactions on `above` | §12, §11.2 |
| lumps of the URC line | generation `for` over a structural param | §8.2.7 |
| a self-heating node | a port of nature `Thermal`; the power is a flow contribution | §4 |
| `bypass`, `DEVpred`, per-device convergence shortcuts | nothing: solver matters | spec, Out of Scope |

### 5.2 Per-family notes

**Passive.** `res`: the conductance and its temperature factor are discrete `let`s; noise is thermal `4kT/R` scaled by `m`, plus flicker over the effective noise area. `cap`: `ddt(c_effective * v)`; the semiconductor capacitor's value is a `let` from geometry. `ind`: `v +> ddt(l * i + sqrt(l) * coupled_flux)`, where the coupled flux arrives through the `link` node (D10): the inductor pins `potential(link)` to `sqrt(l)·i` and reads the branch's flow, and each `MutualInductor` contributes `k · potential(other)` into it, which gives `M = k·sqrt(l1·l2)`.

**Sources.** `vsrc` and `isrc`: `dc * source_scale()`, `ac_stim(acmag, acphase in radians)`, and the waveform under `analysis(Transient)`. Each waveform module is a result module with params; `pulse` and `piecewise_linear` keep the index of the present segment in a signal and schedule the next corner with `at` (the `Pwl` of Appendix D.1). `transient_noise` and `transient_random` draw from `std`'s `normal` with a seeded state carried in a signal.

**Switches.** A `Bit` state with hysteresis; conductance `1/ron` or `1/roff`; thermal noise of the present resistance. The operating point settles the state by the rounds of §16.

**Lines.** `tra`: the two ports of the method of characteristics, each a potential contribution `z0 * i + delay(v_other + z0 * i_other, td)`. `urc`: generation. `txl` and `cpl`: ngspice approximates the line by a delay and a rational (Padé) function per mode; in MSL the same poles and residues are a `laplace_*` filter per mode and a `delay`, with the coefficients computed in body `let`s by the algorithm of `txlsetup.c` and `cplsetup.c`. `ltra`: a convolution of the port quantities with the line's impulse responses over the whole accepted history, whose kernels ngspice integrates anew for each step. This is the core primitive `convolve` (§15), which the feature adds; the line supplies the closed-form integrals of `ltramisc.c`.

**Junction devices** (`dio`, `bjt`, `jfet`, `mes`, the bulk diodes of every MOSFET). The shared functions of `spice::junction`: depletion charge with the `fc` linearization, the temperature laws of saturation current, built-in potential and capacitance for each `tlev`/`tlevc`, the critical voltage, `limitlog`. A device calls them with its own parameters. Known traps from the earlier ports, to be re-checked by twins: the BJT's substrate junction polarity for lateral devices, the JFET's `b` (doping tail) term, the diode's breakdown matching (`bv` is adjusted so the two exponentials meet at `ibv`), and the dependence of convergence on `gmin` stepping.

**MOS levels 1 to 9.** Each has ngspice's source/drain interchange (the model is evaluated with the terminals swapped when `vds < 0`): in MSL an `if` on the probe between two expressions, which §11 rule 5 allows. The Meyer gate capacitances are the one place where ngspice's charge is not a function of the present voltages (risk R4). `mos2` computes its saturation voltage by solving a quartic; `mos6` has no AC, pole-zero or noise entry in ngspice, and in MSL has AC by construction and no noise.

**BSIM, HiSIM, SOI, VBIC, HICUM.** Each is one large `analog` block, written as a sequence of `let`s that follows the load function statement by statement, so that a reader can hold the two side by side. The size-dependent and temperature-dependent parameters (the `pParam` structure of BSIM, computed once per instance in `b4temp.c`) are body `let`s over params. Selector params (`capmod`, `mobmod`, …) choose among expressions; a selector that adds or removes a node (`rgatemod`, `rbodymod`, `trnqsmod`) is structural. `hicum2` in ngspice-47 is C++ with dual numbers (`hicumL2.cpp`): its currents are plain expressions and its derivatives are automatic there as here.

---

## 6. The parity plan

### 6.1 What is compared

The oracle is ngspice-47, run at test time by `piperine-parity` on a `.cir`; the MSL twin is run by `piperine` on both backends; each printed quantity is compared under one `ToleranceContract`. This is the harness of today (`crates/piperine-parity`), with more fixtures and three new gates.

| Analysis | Compared | Contract |
| --- | --- | --- |
| operating point | node voltages, terminal currents | `DC_VOLTAGE`, `DC_CURRENT` |
| DC sweep | terminal currents along the sweep | `DC_CURRENT` |
| AC | magnitude and phase of a transfer or a port admittance over frequency | `AC_MAGNITUDE`, `AC_PHASE` |
| transient | node voltages at ngspice's print points | `TRAN_VOLTAGE` |
| noise | output noise density per frequency, per source and total; band totals | `NOISE` |
| transfer function | gain, input and output resistance | `TF_RATIO` |
| pole-zero | poles and zeros | `PZ_ROOT` |
| sensitivity | DC sensitivities to model parameters | `SENSITIVITY` |
| distortion | harmonics of a transient, both sides by Fourier analysis (`.four` in ngspice) | `TRAN_VOLTAGE` on the waveform; the harmonics by the host's measure under `AC_MAGNITUDE` |
| temperature | each of the above at three temperatures | the same contracts |

No contract is added and none is changed (SPM-28). A comparison that fails is a defect of the model or of the solver, or a deviation recorded with its cause (`piperine-parity/src/deviation.rs`) and approved by the user.

### 6.2 The matrix of a model

Each model has a matrix: rows are conditions, columns are analyses. A cell is one twin or a recorded refusal. The rows by family:

| Family | Rows (conditions) |
| --- | --- |
| passive | value; geometry; temperature coefficients at three temperatures; `m` and `scale`; `ic`; flicker noise; coupling (`k` at 0.1, 0.9, 1) |
| sources | each waveform with defaults and with every parameter set; corners that fall between print points; `acmag`/`acphase`; source stepping in a hard operating point |
| switches | on, off, hysteresis up and down, `initial` each way, noise in each state |
| lines | matched, open and shorted terminations; a step and a sine; lossy cases at three lengths; coupled pairs with near- and far-end crosstalk |
| diode | forward (low injection, knee, series-resistance limited), reverse leakage, breakdown, tunnelling, recombination; `area` and `pj` scaling; sidewall; each `tlev`/`tlevc`; reverse recovery (`tt`, `vp`); depletion and diffusion capacitance in AC; shot, thermal and flicker noise; self-heating |
| bipolar | Gummel plot; output characteristics with Early effect; forward and reverse active, saturation, cut-off; quasi-saturation; both polarities; lateral and vertical substrate; `ft` versus `ic` in AC; storage time in transient; each noise source; three temperatures; for VBIC and HICUM the same plus self-heating and avalanche |
| JFET, MESFET, HFET | transfer and output curves; pinch-off, ohmic, saturation; gate forward conduction; both polarities; capacitances in AC; noise at `nlev` 0, 1, 2, 3 where the model has it |
| MOS 1 to 9, VDMOS | `id`–`vgs` at low and high `vds`, `id`–`vds` at four `vgs`, each at three body biases; subthreshold, linear, saturation; reverse mode (`vds < 0`); both polarities; process-derived versus given `vto`, `kp`, `gamma`, `phi`; junction diodes forward and reverse; overlap and Meyer capacitances in AC at five biases; an inverter and a ring oscillator in transient; noise per `nlev`; three temperatures; VDMOS body diode and self-heating |
| BSIM, HiSIM, SOI | the above; plus a geometry grid (short, long, narrow, wide, and the four corners); each model selector at each value; gate current; the NQS options in AC and transient; stress and well-proximity parameters; binned cards; the model's own regression decks from `ngspice/tests/` |

A twin is small: one device, its sources, at most a load. The existing fixtures of `tests/e2e/twins/` (66 netlists, 24 with an MSL twin today) are kept, and their hand-written `devices.msl` is replaced by `std::spice` once the models it holds have landed (phases 1 and 4).

### 6.3 Layout and generation

```
crates/piperine-parity/tests/spice/<family>/<model>/<case>.cir      the ngspice netlist
crates/piperine-parity/tests/spice/<family>/<model>/<case>.msl      the MSL twin
crates/piperine-parity/tests/spice/<family>/<model>/matrix.toml     the cells: case, analysis, quantities, contract, or the refusal and its reason
crates/piperine-parity/tests/spice_<family>.rs                      one target per family; it walks the matrices
```

The `.cir` and the `.msl` of a twin are written by hand for the first families. From the MOSFETs on, a card has hundreds of parameters, and a twin's two files must carry the same card: a small generator in `piperine-parity` writes both from one table of `name = value` pairs (the ngspice spelling and the MSL spelling come from the rename table). The generator is test support; it is the seed of the netlist importer that the spec leaves out.

### 6.4 The gates

| Gate | Fails when | Requirement |
| --- | --- | --- |
| matrix completeness | a cell of a model's matrix has no twin and no recorded refusal | SPM-27 |
| parameter coverage | a param of a model is set to a non-default value by no twin (the list is printed; the gate holds a ceiling per model that only goes down, and is zero at the model's closure) | SPM-33 |
| parameter completeness | an ngspice parameter is neither a param of the module nor in the closed list of §3.9 | SPM-02 |
| name fidelity | a param's name is not the ngspice name and not in the rename table | SPM-09 |
| contract only | a parity test compares with a literal (the existing rule of `contract.rs`) | SPM-28 |
| oracle version | the installed ngspice does not report `std::spice::NGSPICE_VERSION` | SPM-05, SPM-36 |
| map freshness | `models/*.md` differs from a fresh rendering | SPM-01 |

Per the project's gate policy, a task runs the targets of the family it touches; the full suite runs once, at the family's final verification.

### 6.5 Where the two simulators differ by design

Known before starting, and each needs a decision recorded in the deviation ledger when a twin meets it:

| Difference | Effect | Handling |
| --- | --- | --- |
| time stepping (TR-BDF2 against ngspice's trapezoidal with its own step control) | transient waveforms differ within the truncation tolerance | `TRAN_VOLTAGE` already accounts for it; fixtures use ngspice's `maxstep` small enough that ngspice's own error is below the contract |
| located crossings against step-quantized switching | a switch closes at slightly different times | the same; twins measure after the event, not at it |
| the Meyer capacitance averaging | small differences of gate charging in transient | risk R4 |
| ngspice's derivative not being the derivative of its current | AC and noise differ where it happens | found by the AC twins; recorded per model (SPM-24) |
| physical constants | none: the library uses ngspice's | |
| random sources | different samples | compare mean, variance and spectrum, not samples |

---

## 7. The Book

The chapter "Modules and hierarchy" gains a section on `alias` (§12.7), and the cheat sheet a row. Then a new part, after "In depth": **Part 3, The SPICE model library**. It is a manual: each chapter explains its devices to a reader who has used a transistor and not necessarily a SPICE model, and it is complete without ngspice's manual. Every listing is run by the `book` target; every parameter table is generated from the package's `///` texts, so the Book cannot drift from the source.

Each device section has the same seven parts, in this order:

1. **What it is**: the physical device, and what the model captures and leaves out.
2. **Choosing it**: when this model, when its neighbours (level 1 against BSIM4; Gummel-Poon against VBIC).
3. **Placing it**: the minimal listing, with its result.
4. **A model card**: a card of a real part or process, as a `const` or a function, and how to share it.
5. **The parameters, explained**: grouped by what they affect, each group with its equation and a figure where a curve says it better (the knee of a diode, the output curves against `lambda`); then the generated reference table with unit, default and ngspice name.
6. **Use cases**: one to three small circuits, each run, with what to look at.
7. **Reading it back, and limits**: the operating-point variables, the noise sources, what differs from ngspice, what is refused.

| Chapter | Sections | Use cases (each a run listing) |
| --- | --- | --- |
| **Using the SPICE library** | `use std::spice::…`; the naming rule; a card is an `alias`; binning functions; units and SI suffixes; temperature, `dtemp`, `tnom`; multiplicity; initial conditions; reading operating-point variables and noise by source; **from an ngspice netlist to MSL**: the element-by-element translation table, the aliases, the expression functions of the B source; what differs on purpose; what is not provided (CIDER, XSPICE code models, `ndev`) and what is refused; models ngspice loads through OSDI and how MSL loads them | a netlist translated line by line and run to the same answers |
| **Passive elements** | resistor (by value, by geometry, temperature, noise); capacitor; inductor; coupled inductors and transformers; the uniform RC line | a divider over temperature; a resistor's noise; a transformer; an RC line's delay |
| **Sources and switches** | DC and AC; the eight waveforms, each with its parameters drawn on its curve; a waveform of your own; behavioural sources; the four controlled sources; current sensing; switches and hysteresis | a pulse train into an RC; an AM signal; an ideal op-amp from a controlled source; a relaxation oscillator with a switch |
| **Transmission lines** | lossless; reflections and termination; lossy and coupled lines; which model for which problem | a mismatched line's ringing; crosstalk on a coupled pair |
| **Diodes** | the junction; DC parameters; breakdown; charge storage and recovery; temperature; noise; the geometric diode; self-heating | a rectifier; a Zener regulator; reverse recovery; a varactor in AC |
| **Bipolar transistors** | Gummel-Poon from the ideal transistor up; Early effect, high injection, base resistance; charge and `ft`; temperature; noise; VBIC and what it adds; HICUM; self-heating | a common-emitter stage (bias, gain, noise); a current mirror over temperature; a switching transistor's storage time |
| **JFETs, MESFETs and HFETs** | Shichman-Hodges JFET; Parker-Skellern; Statz MESFET; MESA; HFET | a JFET source follower; a MESFET amplifier's gain against bias |
| **MOSFETs: the classic levels** | level 1 from the square law; process-derived parameters; body effect; junctions and overlap; the Meyer capacitances; levels 2, 3, 6, 9 and what each adds; the power MOSFET (VDMOS) | an inverter's transfer curve and delay; a differential pair; a power switch with its body diode |
| **MOSFETs: BSIM** | what BSIM3 and BSIM4 model; **working with a foundry card** (an alias per card, binning, corners, mismatch); the parameter groups; the model selectors; layout effects; noise models; the earlier versions and BSIM1, BSIM2 | a ring oscillator from a card; `gm/id` curves; `ft` against bias; a binned card |
| **SOI and HiSIM** | body effects in SOI, the body contact, self-heating; the BSIMSOI family; STAG; HiSIM2 and HiSIM_HV | a floating-body kink; a high-voltage switch |
| **Reference** | every parameter of every model, generated; the translation table; the index of ngspice names | |

Each phase ships the sections of its devices (SPM-43); a chapter grows phase by phase and is never written at the end. The Book's rules hold: self-contained, respectful, every listing run, values allowed to move with the solver while they stay close to ngspice.

---

## 8. What the models need

Checked against the language specification, `GAPS.md` at `7d4d544e0` and the capabilities C1 to C98 of the device ABI (proposal §22.7). The ABI is what a compiled MSL module is expressed in; a model never names it.

| # | Need | Used by | Language | ABI capability | Status |
| --- | --- | --- | --- | --- | --- |
| N1 | flow and potential contributions, param-selected shorts | every device | §11 rules 1 to 3 | C1, C4, C23 | there |
| N2 | charges under `ddt` with a per-state tolerance | every dynamic device | §14.5 | C2 | there |
| N3 | **a convolution over the accepted history with a kernel that depends on the present step** | `ltra` | none | none (C3 is a fixed delay) | **missing.** Created in this feature: the core primitive `convolve` (§15, phase 9, T84 to T88). |
| N4 | Newton limiting with a seed and a veto | every junction and FET | §18 | C9, C10, C95 | there |
| N5 | noise sources, named, correlated through shared instances, frequency-dependent | every noisy device | §14.8, §11.1.8 | C12, C96 | there |
| N6 | AC stimulus | sources | §14.3 | C13 | there |
| N7 | operating-point variables, `ddx` | every device | §11 rule 6, §14.2 | C14 | there; `ddx` through the long `let` chains of BSIM is the first thing the BSIM spike measures |
| N8 | `gmin()`, `source_scale()`, `temperature()`, `analysis(…)` | every device | §14.4 | C18, C56, C76 | there |
| N9 | scheduled events and located crossings | waveforms, switches | §13 | C15, C46 | there |
| N10 | `delay` | `tra`, `txl`, `cpl` | §14.6 | C3 | there |
| N11 | `laplace_*` filters with coefficients from params | `txl`, `cpl` | §22.2 | C24 | there for constant coefficients; coefficients computed in a body `let` to be verified |
| N12 | generation over a structural param, arrays of nodes | `urc`, `cpl` | §8.2.7 | C32 | there |
| N13 | a `Thermal` port | self-heating variants | §4 | C27 | there |
| N14 | templates, functions returning templates | binning | §8.4 | C73 | specified; verified in phase 0 |
| N15 | the `alias` item | every card | amendment, §12 | none: an alias is gone before a shape is built | this feature, phase 0 |
| N16 | a `pub` node of an instance passed as a sibling's connection | `MutualInductor` (D10) | §8.2.8, §8.1.10 | C28 | specified; to be verified in phase 0 |
| N17 | enum, `bool`, `i32` and array params read in `analog`; a param that selects a branch kind | every transistor | §8.1.15, §11 rule 3 | C30 `STRUCTURAL` | there (`GAPS.md` L93 refuses `str` params, which the package does not use) |
| N18 | modules of 900 params and analog blocks of several thousand `let`s, on both backends, in acceptable compile time | BSIM4, BSIMSOI, HiSIM | no limit stated | no limit stated | **unknown.** Measured by the phase-0 spike (T17); risk R1 |
| N19 | a record written at accepted solutions (`on settle` with a probe) | safe-operating-area limits | §12.1.5, §19 | C44, C47 | there |
| N20 | host `set`, sweep and gradient of any model param | fitting, corners, `.sens` parity | §21, §23 | C26, C64 | there; `GAPS.md` L61 (a param folded at elaboration cannot be swept after `open`) bites structural params only |
| N21 | a namespace of `std` that costs nothing when unused | every design that uses no SPICE model | §13 | none | this feature, phase 0 |

Nothing here asks the device ABI for a new capability except N3, which this feature creates: §15 specifies the primitive and what it adds to the ABI and the solver.

---

## 9. Phases

Decision D12: from the simplest model to the most complex, each phase shippable by itself. A phase delivers its modules in `std::spice`, their filled parity matrices on both backends, and their Book sections, and merges to `main` when its last task is committed. The order follows the size and the number of mechanisms of each model: a linear element; then state; then events; then a junction; then a transistor; then the compact models by size.

| Phase | Ships | Models | New mechanism it exercises |
| --- | --- | --- | --- |
| 0 | **Ground**: `alias` in the language, specification and Book; the `std::spice` skeleton with on-demand loading and its notice; the parity scaffold and its gates; the scale spike | — | — |
| 1 | Resistors, capacitors, DC and AC sources, linear controlled sources | `res`, `cap`, `vsrc`, `isrc` (no waveform), `vcvs`, `vccs`, `cccs`, `ccvs` | params, temperature, `ddt`, thermal and flicker noise, a sensing branch |
| 2 | Inductors and coupling, the eight waveforms, behavioural sources | `ind`, `K`, the waveforms, `asrc` | a potential branch with state, a public node, scheduled corners, a value input |
| 3 | Switches and the lossless line | `sw`, `csw`, `tra` | discrete state with hysteresis, `delay` |
| 4 | Diodes and the uniform RC line | `dio` (four modules), `urc` | limiting, junction charge, temperature laws, self-heating, generation |
| 5 | JFETs and the Statz MESFET | `jfet`, `jfet2`, `mes` | a three-terminal device, polarity |
| 6 | The bipolar transistor | `bjt` | two junctions, base resistance modulation, excess phase |
| 7 | MOS levels 1, 2, 3, 6, 9 | `mos1`, `mos2`, `mos3`, `mos6`, `mos9` | source/drain interchange, Meyer capacitances, process-derived parameters |
| 8 | The power MOSFET, MESA and HFET | `vdmos`, `mesa`, `hfet1`, `hfet2` | self-heating in a transistor |
| 9 | **The history convolution primitive** `convolve` (§15): specification, device ABI, analog solver, language and both backends, the Book | — | capability N3, created here |
| 10 | Lossy and coupled lines | `txl`, `cpl`, `ltra` | rational approximations, a const-generic device, `convolve` |
| 11 | BSIM1 and BSIM2 | `bsim1`, `bsim2` | a compact model of 80 to 135 params: the rehearsal for BSIM3 |
| 12 | BSIM3 | `bsim3` (3.3.0), then `bsim3v32`, `bsim3v1`, `bsim3v0` | 423 params, model selectors, the NQS node |
| 13 | VBIC and HICUM | `vbic`, `hicum2`, each with its self-heating module | advanced bipolar |
| 14 | BSIM4 | `bsim4` (4.8), then `bsim4v7`, `bsim4v6`, `bsim4v5` | 891 params, gate and body networks, correlated noise |
| 15 | SOI | `soi3`, `bsim3soi_fd`, `bsim3soi_dd`, `bsim3soi_pd`, `bsimsoi` | body and thermal terminals |
| 16 | HiSIM | `hisim2`, `hisimhv1`, `hisimhv2` | surface-potential models |
| 17 | **Close**: the Book's reference and translation table complete, every gate at zero, the excluded list in `GAPS.md`, the full suite once | — | — |

Phases 1 to 4 are one lane, in order: each uses the devices of the one before in its twins. From phase 5 on, a phase needs only phase 4 and may run as a parallel lane, in its own worktree with its own target directory; phase 9 needs phase 3 and phase 10 needs phase 9. The BSIM phases (11, 12, 14) are in order because each reuses the structure of the one before; 15 and 16 follow 14. `tasks.md` has 148 tasks.

**Size.** Phases 1 to 8 restate about 80,000 lines of C in an estimated 8,000 lines of MSL. Phases 11 to 16 are where the volume is: 268,000 lines of C, of which the load and temperature functions that must be restated are about a third. A large model is a sequence of tasks (the interface page; setup and temperature; DC current; charge; noise and reported quantities; parity; the Book), as the `ngspice-bsim3` scaffold foresaw.

---

## 10. Risks

| # | Risk | What it would cost | Mitigation |
| --- | --- | --- | --- |
| R1 | **Scale of the compact models.** A module with 900 params and a block of thousands of `let`s, differentiated symbolically, may exceed what the planner, the interpreter or the generated Rust handle in reasonable time or memory (the earlier Cranelift path overran on `mos2`/`mos3` distortion kernels) | phases 5, 8, 9, 10 blocked | the phase-0 spike measures it with a synthetic module before any large model is planned; a finding becomes a solver feature with its own owner, not a workaround in the model |
| R2 | **Transliteration errors** in the equations of 52 devices | wrong models that look right | the parity matrix with a geometry grid and every selector; ngspice's own regression decks; the parameter-coverage gate, which makes an unused parameter visible |
| R3 | **ngspice's derivative is not the derivative of its current** in some model | AC or noise outside the contract with a correct model | found per model by the AC twins; recorded as a deviation with the source lines; never a wider contract |
| R4 | **Meyer capacitances** (`mos1` to `mos9`): ngspice integrates the average of the capacitance at two time points, which is not `ddt` of a charge | transient gate currents differ slightly; charge is not conserved in either simulator | `c * ddt(v)` with the present capacitance; twins that charge a gate through a resistor; if the contract fails, the exact averaging needs the previous value of `c`, a signal updated `on settle`, which is exact for ngspice's rule at accepted steps |
| R5 | **The lossy line** needs N3, and N3 changes the device ABI and the analog solver | a core change inside a model feature | `convolve` is specified first (§15, T84), built as its own phase before the lines, and proven against an RC filter before any line uses it |
| R6 | **Convergence**: a faithful model with ngspice's limiting may still need ngspice's shortcuts (bypass, its initial-junction sequence) to converge on hard circuits | twins that do not solve | `off`, the `limit` seeds and `gmin`/source stepping are in the language; the earlier ports found junction convergence solvable by gmin stepping; a failure is a solver item |
| R7 | **Licence.** The device sources are Modified BSD (`COPYING` lists no device as an exception); a transliteration is a derived work | a standard library that cannot be redistributed as it is | `std/spice/NOTICE` carries ngspice's notice and each file names its sources (SPM-25); the repository's own licence is MIT, which the notice is compatible with. Approved by the user on 2026-11-27 (§13, T10) |
| R8 | **Units and meanings of 10,000 parameters** written by hand per family | a slow mapping task, or wrong units in the Book | the unit is metadata, never read by a model, so an error there cannot change a result; the reference manuals are cited per parameter group; the tables are generated from one place |
| R9 | **The interface freezes too early.** A rule approved on small devices may not fit BSIMSOI's terminals or HiSIM's options | a late change of names users already wrote | the rules of §3 are the user's and the same for all; a family's page applies them and its planner decides what they leave open (§14), so a late finding changes a page and not a rule; the known open point (SOI and HiSIM_HV terminal variants) is named now |
| R10 | **Two ports of the same model disagree across backends** (`interp` and `rust`) on a large model | a twin green on one backend | SPM-35: each twin runs on both |
| R11 | **The oracle moves.** A machine with another ngspice version gives other values for the newer models | false failures or false passes | the oracle-version gate (SPM-36); the version is recorded with every compared point, as today |
| R12 | **`code-clarity` and the ABI refactor move the ground** under this feature | rebases | the feature starts after both merge; T1 re-takes every path |
| R13 | **The standard library grows by tens of thousands of lines** and every run parses `std` | every design pays for BSIM4 | SPM-07: a namespace file of `std` is read when a path first reaches it (§13); a guard measures the time of an empty design before and after |
| R14 | **`alias` touches the parser, the name resolver, the elaborator, the object model, the printer, the language server and the host metadata** | a language change larger than its grammar | §12 keeps it to one item kind and one binding rule; the instance an alias places is an ordinary instance, so nothing after elaboration changes |

---

## 11. Tech decisions (only the non-obvious ones)

| Decision | Reason |
| --- | --- |
| The map is generated and committed, with the tools | 11,027 rows cannot be kept true by hand, and a reviewer must be able to see where a row came from |
| Selector parameters stay integers | the values are each model's documented vocabulary; two hundred enums of numbers would add names and remove nothing |
| No card library in `std::spice` | a card is a claim about a part or a process; the library claims only ngspice's models. Cards of public parts live in the Book and the fixtures, each with its source |
| `std::spice::constants`, not `std::constants` | parity to the last digit of a thermal voltage; a user's own model may use either |
| One file per device even when large | §8 rule 5 requires a module and its blocks in one file; splitting BSIM4's block would mean splitting its equations into functions by an order ngspice does not have, which defeats the side-by-side reading |
| Twins and runner both in `piperine-parity` | the library is in `std`, which holds source only; the fixtures, the harness and the contracts stay in one crate |
| Refused forms are diagnostics with a name, never a silent default | project rule: every refused form named and owned |

## 12. The `alias` amendment

The user's amendment (2026-11-27): "`alias CMim = SemiconductorCapacitor { cj: 2m, ... };` … at instantiation it requires only the necessary params, with replace allowed … it makes sense for all models." It is a change to the language, for every module, owned by this feature (tasks T2 to T9).

### 12.1 What it is

An **alias** is an item that gives a name to a module with some of its params bound.

```rust
/// The metal-insulator-metal capacitor of the process.
pub alias CMim = SemiconductorCapacitor { cj: 2m, cjsw: 0.1n };

let c1 = CMim(a, b) { l: 10u, w: 10u };            // binds what the alias left
let c2 = CMim(a, b) { l: 10u, w: 10u, cj: 2.2m };  // and may replace what it bound
```

**The keyword.** `alias`, not `template`. In the specification a *template* is already a value: `Name { params }`, complete, placed with connections only (§8.4). The new item is not that value: it may leave required params unbound, and a placement through it takes a param block. One word for both would give "template" two meanings in one section. `alias` says what the item adds, a second name, and the user's own example uses it. The language keeps no *type* alias (B.3.6): an alias names a module, and its right side is always a module or another alias.

### 12.2 Grammar

```
alias_decl     ::= attrs 'pub'? 'alias' IDENT generics? '=' path generic_args? ( '{' ( IDENT ( ':' expr )? ',' )* '}' )? ';'
```

`alias_decl` joins the items of a namespace (B.3.1). `alias` is a **contextual keyword**, like `analog` and `digital` (A.1): it is a keyword only at item position before an identifier, and an ordinary identifier elsewhere, so no existing model loses a name. The binding list is the one of `template` (§8): `name: expr`, or `name` alone for `name: name`. Without a block, `alias N = M;` binds nothing.

### 12.3 Static semantics (a new §8.6 of Part III)

1. An alias is an item at namespace scope, beside `module`. Its name is in the type namespace of its file and **MUST** be unique in it. It **MUST NOT** appear in a module body, a block or a function.
2. The **target** is the path on the right: a module declared with `module` (an `extern` module included) or another alias. A core primitive, a function, a type or a nature is not a target.
3. Each name of the block **MUST** be a param of the target module and **MUST NOT** repeat in the block (`E0320`, as in a placement).
4. Each value is a const expression over constants and the alias's own const generics (§3.9), of the param's type under the call rule (§3.2: a `T` for an `Option<T>` is `Some`, and `None` binds an `Option` param to absent). A namespace has no params, so a bound value never reads one.
5. The **bindings of an alias** are those of its target, when the target is an alias, with each name of its own block added or replaced. An alias **MUST NOT** depend on itself.
6. **A placement whose head is an alias** takes connections and a param block, as a placement whose head is a module does: `Alias(ports) { params }`. The instance's params are bound in this order, a later one replacing an earlier one: the module's defaults, the bindings of the alias, the placement's block. The block's values follow §8.1.19: they may read the parent's params.
7. A param with no default and a type other than `Option` that is still unbound after rule 6 is `E0319`, naming the param, the module and the alias.
8. Every `where` of the module is checked for each instance with its final values (§8.1.1). An implementation **MAY** also report, at the alias, a `where` that the alias's own bindings already make false whatever a placement adds.
9. **An alias as a value.** `Alias { params }` is a template expression (§8.4): a value of the *module's* type, bound by rules 5 and 6, which **MUST** be complete (§8.4.2). The bare name `Alias` is `Alias {}`. So an alias goes wherever a template of its module goes: a template-typed param, a function's result, a `C: Fn(P)` argument.
10. A placement whose head is a template *value* still takes no param block (§8.4.4, `G.53`). An alias is a name, known where it is written, and its bindings are text the reader can find; a value may have come from anywhere.
11. An alias is not a type. In a type position the module's name is written; an alias there is `E0382`.
12. An alias **MAY** have generic parameters, which it passes to its target: `alias Fifo8<T: Repr> = AsyncFifo::<T, 3> { almost_full: 6 };`, used as `Fifo8::<u8>(…)`. Their bounds are checked as a module's are.
13. `pub`, `use`, `pub use` and paths treat an alias as any item (§3.4, §3.5). `///` on an alias is metadata (§21).
14. An instance placed through an alias is an instance of the module in every respect: `inst.name`, traits, `Fn(P)`, structural params (§8.1.2: a param is structural by what reads it, never by who bound it).

### 12.4 Dynamic semantics

15. An alias has no run-time existence. An instance placed through one is identical to the instance `Module(ports) { every final binding }` places.

### 12.5 Errors

| Code | Condition | Phase |
| --- | --- | --- |
| `E0380` | the target of an alias is not a module or an alias (rule 2) | elaboration |
| `E0381` | an alias that depends on itself (rule 5) | elaboration |
| `E0382` | an alias used as a type (rule 11) | elaboration |
| `E0383` | an alias declared in a module body, a block or a function (rule 1) | parse |
| §8.1 | a block that names no param of the module, or names one twice (`E0320`); a value that is not a const expression over constants and const generics (`E0321`); a required param unbound at a placement or in a template expression (`E0319`); a `where` false for an instance (`E0317`) | elaboration |
| §3.4 | a second item of the same name in the namespace (the existing code, as for any item) | elaboration |

The numbers `E0380` to `E0383` are free at `7d4d544e0`; T2 confirms them on the merged tree.

### 12.6 The host and the object model

**The object model mirrors the source** (the project's rule). The POM gains an item kind, `Alias`: its name, visibility, docs, generics, the target path as written, and the bindings as written, in order. An instance records its head as written: the alias name, or the module name. Nothing replaces an alias by its expansion in what the POM shows: the chain alias → alias → module is walked through the items. The *effective* bindings of an instance are what elaboration computes from the chain, a derived result beside the authored structure.

**Metadata (§21).** Two additions, both read-only:

- each param of an instance has an **origin**: `default`, `alias <path>`, `placement`, or `host` after a `set`;
- each alias is listed with its target, its bindings and the paths of the instances placed through it.

**`set`, sweeps, gradients.** Nothing changes. A param bound by an alias is a param of the instance: `set top.m1.vto` sets it for that instance, with the `where` check; a sweep and a gradient address it by the same path. There is no `set` of an alias: an alias is text fixed at elaboration. A host that wants one value for every instance of a card iterates the alias's instance list; a design that wants it as a design param declares the param in the parent and binds it at the placements (`NCH(d, g, s, b) { vto: vto_corner }`), and §8.1.23 then recomputes every instance on one `set`.

### 12.7 Where it is written down

| Document | Change |
| --- | --- |
| `docs/spec/part_iii_structure.md` | a new §8.6 "Aliases" with the rules above; §8 syntax block and the forms table gain the item; §8.1.18 says "a module or an alias"; §8.4.4 cites rule 10 |
| `docs/spec/appendix_a_lexical.md` | `alias` in the row of contextual keywords |
| `docs/spec/appendix_b_grammar.md` | `alias_decl` in B.1 and B.3; the placement matrix B.2 (namespace scope only); B.3.6 keeps "type aliases" as not part of the language and points at §8.6 for modules |
| `docs/spec/part_vi_io.md` §21 | the origin of a param and the list of aliases |
| `docs/spec/appendix_g_scope.md` | `G.53` annotated: a placement through an alias takes a block (§8.6); a closed row for the amendment with the user's words |
| `docs/spec/index.md` | concept 6 (module) covers aliases; the glossary gains "alias, target" |
| the Book, "Modules and hierarchy" | a section "Naming a configured module: `alias`": the motivation (the same params written at every placement), the item, binding what is left, replacing, an alias of an alias, an alias as a template value, what the host sees; three run listings |
| the Book, cheat sheet | one row |
| the language server, the printer (`piperine fmt`), the syntax files under `editors/` | the item is parsed, printed and highlighted; go-to-definition on an alias head |

### 12.8 What the implementation touches

The lexer (contextual keyword), the parser (one item), the syntax tree and the POM (one item kind, the instance's head), name resolution (an alias in the type namespace), elaboration (the binding chain of rule 6, the checks of rules 7 and 8), template expressions (rule 9), the metadata the host reads, the printer, the language server. Nothing after elaboration: the plan, both backends, the solvers and the device ABI see ordinary instances.

---

## 13. Shipping in the standard library

**The decision (D5).** The user: "the package I meant is in the stdlib… ship the standard spice models as part of the language".

**The spelling: `std::spice`**, a `pub mod` of the standard library, not a second shipped crate named `spice`.

| | `std::spice::diode::Diode` (chosen) | a second implicit crate, `spice::diode::Diode` |
| --- | --- | --- |
| New rule in the language | none: §3.5.3 already says `std` is a dependency of every crate, and `spice` is a namespace inside it, as `std::analog` is | §3.5.3 would name two crates no manifest names, and two names no dependency may take |
| Who could have written it | anyone: ordinary MSL items under ordinary rules | the same |
| Shorter path | no | by five characters; `use` removes the difference |
| "The models come from ngspice" | the word `spice` is in every path | the same |

The project's rule is that builtins follow the rules of user items, with no special case. `std` is the one crate the toolchain ships, and it is a crate like any other: a tree of namespaces of MSL source. Putting the models in a namespace of it adds nothing to the language. A second implicit crate would add a special case to gain five characters.

**Not in the prelude.** The prelude re-exports the items of the other `std` namespaces. `spice` is excluded: `lib.msl`'s header sentence and §22's index are changed to say so, and a guard checks that no item of `std::spice` is reachable without its path.

**The specification.** §22 gains §22.7 "The SPICE model library": the namespaces, the interface rules of §3 (names, units, given-ness, polarity, temperature, multiplicity, initial conditions), and the statement that each model computes what the model of the same name computes in ngspice-47. The catalogue of record, every module with its ports and every param with its type, default, `where` and text, is a new **Appendix I**, generated from the library's sources; a test fails when the two differ. The equations are not restated in the specification: the reference is ngspice-47's source, and the proof is the parity matrix.

**Loading on demand (SPM-07).** Today the standard library is a table of embedded files, all parsed for every design. With BSIM4, BSIMSOI and HiSIM the library grows by tens of thousands of lines. The rule of §3.6.2 already says how a file is found: `mod name;` loads `name.msl`. The change is *when*: a namespace file of `std` is read, parsed and checked when a path first resolves into it. A design that names no SPICE model reads none of `spice/`; a design that uses `Diode` reads `spice/mod.msl`, `diode.msl` and what `diode.msl` names. The language's meaning does not change (an item not reached has no effect on a conforming model); the errors of an unreached `std` file are found by the library's own test, which reaches every namespace. A guard measures an empty design's elaboration before and after phase 0 and keeps the difference under a bound it states.

**The version.** `std::spice::NGSPICE_VERSION` is the tag the models were written from; the parity harness asserts the installed oracle reports it.

**The notice (user, 2026-11-27).** ngspice's Modified BSD notice is the file `std/spice/NOTICE`, at the root of the spice module. Each model file starts with a header that names the ngspice files it restates and points at `NOTICE`, and each model's `///` ends with the same reference, so the host's listing and the Book carry it. A guard requires both (T10).

---

## 14. The per-family interface pages

The user does not review a family's page (2026-11-27). The planner of each phase writes `interface/<family>.md` under the rules of §3, decides what the rules leave open, records each such decision in the page under "Decisions taken" with its reason, and the phase's report tells the user. The rules themselves (§3, and the rename table) change only by the user.

---

## 15. The `convolve` primitive (capability N3)

The user (2026-11-27): create the capability the lossy line needs inside this feature. Tasks T84 to T88; it runs after `device-abi-simplification`, on the ABI that feature leaves.

### 15.1 What the lossy line needs

ngspice's LTRA model writes each port of a lossy line as the sum of three things: a term in the present port quantities, a term in the far port's quantities one delay earlier, and convolutions of port quantities with impulse responses that have no finite-state form (they hold Bessel functions). ngspice evaluates a convolution by taking the signal as piecewise linear between accepted time points and integrating the kernel exactly over each piece; the model supplies the kernel's integrals in closed form (`ltramisc.c`). The weight of the present time point multiplies an unknown, so it is in the matrix; the rest is history.

`ddt` has a state, `delay` has a history at one lag, `laplace_*` has finitely many states. None reads the whole accepted history against a kernel. That is the missing primitive.

### 15.2 The primitive

A core module with a result (§14), beside `ddt`, `delay`, `limit` and `noise`:

```
module convolve(x: f64, second_integral, steady: f64, response,
                reltol: Option<f64>, abstol: Option<f64>) -> f64
    // second_integral: fn(lag: f64) -> f64
    // response:        fn(frequency: f64) -> (f64, f64)
```

It is `y(t) = ∫₀^∞ k(τ) · x(t − τ) dτ` for a kernel `k` the model describes three ways, one per kind of solve:

| Argument | What the model gives | Read in |
| --- | --- | --- |
| `second_integral` | `K(τ)`, with `K'' = k`, `K(0) = 0`, `K'(0) = 0`: a function or closure that keeps no state, over the lag and what is constant during an analysis | transient steps |
| `steady` | `∫₀^∞ k`, the gain for a constant signal | equilibrium solves |
| `response` | the kernel's frequency response at `frequency`, as (real, imaginary) | small-signal sweeps |
| `reltol`, `abstol` | the tolerances under which a sample of the history that lies on the line between its neighbours is dropped; left out, the simulator's | history upkeep |

The model supplies all three because no one of them gives the others cheaply: the solver does not integrate a kernel numerically or transform it.

### 15.3 Semantics

**Static.**

1. `x` and `steady` are value inputs: `f64` expressions (§14 rule 5). `second_integral` and `response` are functions or closures that keep no state (§3), as the `f` of `limit` is; what they capture **MUST** be constant during an analysis (params, constants, the environment of §14.4.1). A capture that reads a probe or a carrier is an error at the placement.
2. `convolve` is allowed where instances go (§8.3.4) and `analog` reads them. Its result is a continuous expression with a direct dependence on `x` (§8.3.6).
3. For `ddx`, the result of `convolve` is held constant, as the result of any instance is (§14.2.3). For the solver's own linearization the partial is rule 6's.

**Dynamic.** Per kind of solve (the rows §16.2 gains):

4. *Equilibrium solve*: `steady · x`.
5. *History.* The state is the accepted pairs `(tⱼ, xⱼ)` since the operating point. Before it, `x` is taken as constant at its operating-point value `x₀`: the kernel's memory of the time before the run is `steady · x₀`.
6. *Transient step* to `t`, with `x` piecewise linear through the accepted points and the present iterate `(t, x)`: `y = steady · x₀ + Σⱼ wⱼ · (xⱼ − x₀)`, where the weight of a sample comes from `K` at the lags to it and to its two neighbours (the exact integral of the kernel against the two linear pieces that meet at it). The present sample's weight is `K(h) / h` for a step `h`: that is `∂y/∂x`, the term's entry in the Jacobian. The sum over past samples does not change during the Newton iterations of a step and is computed once per step.
7. *Jump solve*: a transient step (§15.2); the history is kept across a jump, as `delay`'s is.
8. *Small-signal sweep*: `response(freq()) · x`, complex. In `Noise` it applies to a noise result as any operator does (§11.1.9).
9. The history advances on accepted steps only and is restored by a rejected one (§14 rule 8). A sample is dropped when it lies within `reltol` and `abstol` of the line through its neighbours.
10. Read in a reaction or an event argument, the result is its value in the solution read.
11. `convolve` adds no time point. A kernel that is zero up to a lag `T` (a delayed kernel) makes the result depend on nothing more recent than `T` ago; the model caps its step with `bound_step` where its accuracy needs it, as ngspice's line does.
12. A non-finite value of `second_integral`, `steady` or `response` fails the solve (`E0507`), naming the instance.

**Errors.** `E0450` covers a placement where the table does not allow it. One new code, `E0458`: a kernel or response closure that captures what is not constant during an analysis (rule 1). T84 confirms the number.

**A worked case.** A first-order low-pass with time constant `τ` has `k(t) = exp(−t/τ)/τ`, so `K(t) = t − τ·(1 − exp(−t/τ))`, `steady = 1`, and the response `1/(1 + jωτ)`. `convolve(v(a), |lag| lag − tau * (1.0 − (−lag / tau).exp()), 1.0, |f| …)` must equal an RC filter in DC, AC and transient: that is the primitive's first proof (T86, T87).

### 15.4 What it needs below the language

Checked against the design of `device-abi-simplification` (proposal v6, Part II), which this feature runs after. That contract has three term operators (`DIRECT`, `TIME_DERIVATIVE`, `DELAY`); for `DELAY` the solver keeps a history of values up to a per-term `maximum_delay` and the device writes the delay through the aspect `TERM_DELAY` (capability C3). A device is immutable data and pure code with no memory, so the history is the solver's and the kernel is the device's.

| Layer | Addition |
| --- | --- |
| Device ABI, catalogue of term operators | a fourth operator, `CONVOLUTION`: "the solver keeps the accepted history of the term's value and adds its convolution with the device's kernel" |
| Device ABI, `Term` | references to two functions of the shape (the kernel and the response) and the two compaction tolerances, with `SESSION_DEFAULT` as for `state_tolerance` |
| Device ABI, aspects | `TERM_KERNEL_LAG` (written by the solver, read by the kernel function), `TERM_KERNEL_VALUE` (written by the kernel function), `TERM_STEADY_GAIN` (written by a function of the device), `TERM_RESPONSE` (two reals, written by the response function, which reads the frequency of the solve's environment, C18) |
| Device ABI, validation | a kernel function reads the lag and what is constant during an analysis, and writes `TERM_KERNEL_VALUE` only; a `CONVOLUTION` term has both functions; the access table gains the four aspects |
| Device ABI, version | unchanged: the contract is not released, and the change is in place |
| Analog solver | the history store with marks, restore and release (C51), the weights and the once-per-step history sum, the partial `K(h)/h` in the sparsity pattern built at bind (C5), compaction, the equilibrium and small-signal forms (C24 is the place small-signal operator responses already live) |
| Plan and backends | the lowering of the core module to a term with operator `CONVOLUTION` and two device functions, in the interpreter and in the generated Rust |
| Language | the core name `convolve`, its typing, rule 1's capture check |

The names of the aspects and fields are this design's proposal; T84 fixes them in `docs/abi/` in that document's vocabulary before any code.

### 15.5 What it does not do

It does not integrate a kernel numerically, find a kernel's response, or control the step by the convolution's own truncation error. ngspice's line has options for the last (`truncnr`, `truncdontcut`); the module maps what it can to `bound_step` and the compaction tolerances, and its page records what has no counterpart.

---

## 16. Open questions

None is the user's at this point. The decisions of 2026-11-27 are in `spec.md`. What each phase's planner decides (a family's page, the SOI and HiSIM_HV terminal variants, the form chosen for the Meyer capacitances) is recorded where it is decided and reported.
