# Spice Models Specification

**Status**: Draft — awaiting user approval of the interface (`interface-review.md`)

The complete library of ngspice device models, written in MSL, as one package named `spice`. A user who knows SPICE places a resistor, a diode with a model card or a BSIM4 transistor from a foundry card, and gets the behaviour ngspice gives, checked against ngspice by test.

This document holds the problem, the scope, the inventory and the requirements. `design.md` holds the interface rules, the implementation map, the parity plan, the Book outline and the phases. `interface-review.md` is the page the user reads first: the interface, family by family, with the decisions that are theirs. `models/*.md` hold every parameter of every device, generated from the ngspice sources.

## Open questions (for the user)

Twelve decisions are the user's. Each has a recommendation, and the documents are written on the recommended side. `interface-review.md` §12 states each one with its example and the other sides.

| # | Question | Recommendation |
| --- | --- | --- |
| D1 | **Parameter names.** ngspice's own names (`is`, `vto`, `kp`, `tox`), or full words (`saturation_current`)? | ngspice's names for params, full words for everything else (modules, ports, namespaces, enums, functions). A param name is the vocabulary of every model card, PDK and textbook; the full meaning is in the param's `///` text and in the Book. |
| D2 | **Where model params live.** Flat params on the device module, or a `model` struct passed as one param? | Flat params. A model card is a template (`const D1N4148: Diode = Diode { … };`), which the language already has. The host lists, sets and differentiates every param by name; a struct field has none of that today. |
| D3 | **Instance params on a shared card.** A template placement takes no param block today (`G.53`), so `NCH(d, g, s, b) { w: 10u, l: 1u }` is refused. | Ask for the language amendment that lifts `G.53`. Until it lands a card is a function of its instance params (`nch(10u, 1u)(d, g, s, b)`), which is the form §8.4.7 already names for binning. |
| D4 | **Units of temperature and phase.** ngspice's (`tnom`, `temp` in °C; `acphase` in degrees) or MSL's (kelvin, radians)? | ngspice's. One rule covers name, unit and default: a card is copied without arithmetic. |
| D5 | **Where the package lives and how a user depends on it.** | In this repository at `packages/spice/`, tested with the solver. For users, a `git` dependency with a subdirectory key in the manifest, which `msl-packages` left out and this feature needs. |
| D6 | **CIDER numerical devices** (`numd`, `numd2`, `nbjt`, `nbjt2`, `numos`) and the `ndev` socket device. They solve device physics on a mesh; they are not compact models. | Not in this feature. Named here, owned by a follow-up feature `spice-numerical-devices`, to be scheduled or dropped by the user. |
| D7 | **XSPICE code models** (81 of them: `gain`, `summer`, `d_and`, `adc_bridge`, `zener`, …). | Not in this feature. Owned by a follow-up `spice-code-models`. Most are behavioural blocks that MSL's own digital and mixed-signal constructs replace; each needs that judgement, model by model. |
| D8 | **Legacy model versions** (`bsim3v0`, `bsim3v1`, `bsim3v32`, `bsim4v5`, `bsim4v6`, `bsim4v7`). ngspice keeps them for old model cards. | In scope, as the last phase before closure, each a module of its own (`Bsim3V32`). |
| D9 | **The reference version.** The parity oracle installed is ngspice-47; the checkout at `~/Git/ngspice` is ngspice-26 (2015) and lacks `vdmos`, `hicum2` and `hisimhv2`. | ngspice-47 is the reference for sources and oracle. |
| D10 | **Coupled inductors.** `K L1 L2 0.9` names two inductors. An MSL module reaches another only through a node. | Every `Inductor` has a public node `link`; `MutualInductor(l1.link, l2.link) { k: 0.9 }` couples two of them. |
| D11 | **Controlling current of F, H and W elements.** ngspice names a 0 V source. | The device has its own sensing branch: two more ports, placed in series with the path whose current controls it. |
| D12 | **The order of delivery.** | Linear elements and sources, then the diode, then BJT/JFET/MESFET, MOS levels 1 to 9, BSIM3 and BSIM4, lines, VBIC and HICUM, the old BSIMs, SOI, HiSIM. |

## Problem Statement

MSL has no device library. Each test and each example writes its own resistor and its own diode (`tests/e2e/twins/devices.msl` holds five such models), and a user who comes from SPICE finds no transistor at all. The draw of Piperine is mixed-signal simulation, and a mixed-signal design has an analog side made of the devices every foundry and every textbook describes in ngspice's terms. Without them a user rewrites compact models before simulating anything.

Earlier work ported some of these models in Verilog-A and in PHDL (`spice-stdlib`, `ngspice-parity`, the scaffolds `ngspice-bsim3` and `ngspice-bsim4`). Both languages are retired; none of that source is in the tree. What remains is the parity harness, its tolerance contract, the ngspice netlists of the old fixtures and the record of the traps those ports met.

## Dependency

This feature executes after `code-clarity` merges to `main` and its review ends. It runs beside `device-abi-simplification`: the models are MSL source and touch no crate of the ABI. Paths and crate names in these documents are those of `origin/main` at `7d4d544e0`; the first task re-takes them on the merged `main`.

## Goals

- [ ] Every device model of ngspice-47's `src/spicelib/devices` is accounted for: 52 are written in MSL, 6 are refused by name with an owner.
- [ ] Each MSL model computes what ngspice computes, for every parameter ngspice accepts, in every analysis ngspice supports for it.
- [ ] The interface is MSL's own: modules, params, templates, `where`, `Option`. A user needs no netlist concept that MSL lacks.
- [ ] Each model agrees with ngspice-47 under the existing tolerance contracts, across its operating regions, its analyses and temperature.
- [ ] The Book has a part on the library that a user can work from: what each device is, how to place it, what each parameter means, worked cases.
- [ ] The models ship as one package, `spice`.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| CIDER numerical devices (`numd`, `numd2`, `nbjt`, `nbjt2`, `numos`) | They solve drift-diffusion on a mesh inside the device. That is a solver, not a compact model. Refused by name; owner `spice-numerical-devices` (D6). |
| `ndev` | A socket to an external device simulator. Refused by name; MSL reaches foreign simulators through `extern "cosim"`. |
| XSPICE code models (81) | A behavioural block library, not device physics; `models/excluded.md` lists them. Owner `spice-code-models` (D7). |
| Verilog-A models ngspice loads through OSDI (BSIM-CMG, BSIM-BULK, PSP, EKV, MEXTRAM, HICUM/L0, ASM-HEMT) | They are not ngspice's models: ngspice loads a compiled library. MSL loads the same library with `extern "osdi"` (§8.5). The Book's first chapter of the part says so. |
| A netlist importer (`.cir`, `.lib`, `.model` to MSL) | Its own feature. This one fixes the target such an importer writes to, and the Book gives the translation table. |
| ngspice's Volterra distortion analysis (`.disto`) and the `distof1`, `distof2` source parameters | MSL builds distortion measures from a transient and the host's Fourier analysis (§16.3). The models need nothing for it. Parity compares that route with ngspice's `.four`. |
| Circuit-wide options (`.options tnom`, `defw`, `defl`, `defad`, `defas`, `scale`, `gmin`) | MSL has no circuit-wide model option. Each is a param with ngspice's built-in default; a design shares a value by binding a parent's param. `gmin` is `gmin()`. |
| `uic` | MSL has none (§16 H6). A device's `ic` holds its branch at the transient's operating point (§16 H7). |
| The RF port parameters of `vsrc` (`portnum`, `z0`, `pwr`, `freq`, `phase`), `external`, `snd` | Ports of an S-parameter analysis and host-driven sources belong to the host (§16.3, §23.1). Refused by name. |
| ngspice's bugs | A deviation traced to an ngspice defect is recorded with its evidence and the user's approval; Piperine's value is asserted. |
| Solver performance work | Bypass, the predictor and the device-level convergence shortcuts of ngspice are not model behaviour. What a model needs from the solver is listed in `design.md` §8. |

---

## Assumptions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --- | --- | --- | --- |
| Reference sources | tag `ngspice-47` of the ngspice repository | the installed oracle reports `ngspice-47`; a model must be read from the version it is tested against | n (D9) |
| Local checkout `~/Git/ngspice` | not the reference: it is at ngspice-26 (March 2015) | it lacks `vdmos`, `hicum2`, `hisimhv2` and eleven years of fixes | n (D9) |
| What "every model" covers | the 58 device kinds of `src/spicelib/devices` (57 directories with a `SPICEdev`, plus the mutual inductor inside `ind`) | they are what `.model` and the element letters reach | n (D6, D7) |
| Tolerances | the nine contracts of `piperine-parity/src/contract.rs`, unchanged | "never retune tolerances"; the contract is the only place a bound is written | y (project rule) |
| Licence | the package carries ngspice's Modified BSD notice; every model file names the ngspice files it was written from | the device sources are Modified BSD (`COPYING`); a transliteration keeps the notice | n (reported, see Risks) |
| Units of the generated tables | not in the ngspice tables; added by each family's mapping task from the model's manual | `IFparm` has a name, a type and a description, no unit | n (SPM-06) |
| Where ngspice reads the analysis card for a default (`tr`, `tf`, `pw`, `per` of a pulse) | the MSL parameter is required, or absent means "never" | a model does not know the run's stop time | n (review §4) |

## Inventory

ngspice-47 registers 57 devices (`src/spicelib/devices/dev.c`), one per directory, and the `ind` directory holds a second one, the mutual inductor. The table gives each device, the netlist form that reaches it (`src/spicelib/parser/inpdomod.c`), the MSL module this feature gives it, and its settable parameters. Aliases and read-only quantities are not counted. `models/<family>.md` lists every row.

| ngspice device | Directory | Netlist form | MSL module | Instance params | Model params | C lines |
| --- | --- | --- | --- | --- | --- | --- |
| Resistor | `res` | element R; `.model … r` | `Resistor`, `SemiconductorResistor` | 13 | 16 | 1336 |
| Capacitor | `cap` | element C; `.model … c` | `Capacitor`, `SemiconductorCapacitor` | 11 | 14 | 1392 |
| Inductor, mutual inductor | `ind` | elements L and K; `.model … l` | `Inductor`, `GeometricInductor`, `MutualInductor` | 9 | 12 | 2247 |
| Vsource | `vsrc` | element V | `VoltageSource` and eight waveform modules | 23 | 0 | 2526 |
| Isource | `isrc` | element I | `CurrentSource` | 18 | 0 | 1449 |
| ASRC | `asrc` | element B | `BehavioralVoltageSource`, `BehavioralCurrentSource` | 9 | 0 | 987 |
| VCVS | `vcvs` | element E | `VoltageControlledVoltageSource` | 1 | 0 | 724 |
| VCCS | `vccs` | element G | `VoltageControlledCurrentSource` | 2 | 0 | 661 |
| CCCS | `cccs` | element F | `CurrentControlledCurrentSource` | 3 | 0 | 656 |
| CCVS | `ccvs` | element H | `CurrentControlledVoltageSource` | 2 | 0 | 711 |
| Switch | `sw` | element S; `.model … sw` | `VoltageControlledSwitch` | 4 | 4 | 813 |
| CSwitch | `csw` | element W; `.model … csw` | `CurrentControlledSwitch` | 3 | 4 | 810 |
| Tranline | `tra` | element T | `LosslessTransmissionLine` | 9 | 0 | 986 |
| LTRA | `ltra` | element O; `.model … ltra` | `LossyTransmissionLine` | 5 | 15 | 3575 |
| TransLine | `txl` | element Y; `.model … txl` | `SingleLossyTransmissionLine` | 3 | 5 | 2098 |
| CplLines | `cpl` | element P; `.model … cpl` | `CoupledTransmissionLines<const N: usize>` | 4 | 5 | 3320 |
| URC | `urc` | element U; `.model … urc` | `UniformRcLine` | 2 | 6 | 607 |
| Diode | `dio` | element D; `.model … d`, levels 1 and 3 | `Diode`, `GeometricDiode`, and a self-heating variant of each | 14 | 68 | 4531 |
| BJT | `bjt` | element Q; `.model … npn\|pnp`, level 1 | `Bjt` | 10 | 123 | 8135 |
| VBIC | `vbic` | element Q; levels 4 and 9 | `Vbic`, `VbicSelfHeating` | 8 | 116 | 8325 |
| hicum2 | `hicum2` | element Q; level 8 | `Hicum2`, `Hicum2SelfHeating` | 9 | 139 | 8889 |
| JFET | `jfet` | element J; `.model … njf\|pjf`, level 1 | `Jfet` | 8 | 23 | 3153 |
| JFET2 | `jfet2` | element J; level 2 | `Jfet2` | 8 | 1 | 1765 |
| MES | `mes` | element Z; `.model … nmf\|pmf`, level 1 | `Mesfet` | 4 | 14 | 3246 |
| MESA | `mesa` | element Z; levels 2, 3, 4 | `Mesa` | 8 | 61 | 2918 |
| HFET1 | `hfet1` | element Z; `.model … nhfet\|phfet`, level 5 | `Hfet1` | 7 | 64 | 2650 |
| HFET2 | `hfet2` | element Z; level 6 | `Hfet2` | 7 | 36 | 1797 |
| Mos1 | `mos1` | element M; `.model … nmos\|pmos`, level 1 | `Mos1` | 16 | 32 | 6780 |
| Mos2 | `mos2` | element M; level 2 | `Mos2` | 16 | 39 | 8137 |
| Mos3 | `mos3` | element M; level 3 | `Mos3` | 16 | 45 | 7564 |
| Mos6 | `mos6` | element M; level 6 | `Mos6` | 16 | 37 | 2847 |
| Mos9 | `mos9` | element M; level 9 | `Mos9` | 16 | 43 | 7478 |
| VDMOS | `vdmos` | element M; `.model … vdmos` | `Vdmos`, `VdmosSelfHeating` | 8 | 64 | 4759 |
| BSIM1 | `bsim1` | element M; level 4 | `Bsim1` | 14 | 79 | 6833 |
| BSIM2 | `bsim2` | element M; level 5 | `Bsim2` | 14 | 135 | 4649 |
| BSIM3 | `bsim3` | element M; levels 8 and 49, version 3.3.0 | `Bsim3` | 19 | 423 | 11546 |
| BSIM3v32 | `bsim3v32` | version 3.2 to 3.2.4 | `Bsim3V32` | 15 | 422 | 11946 |
| BSIM3v1 | `bsim3v1` | version 3.1 | `Bsim3V1` | 12 | 361 | 8842 |
| BSIM3v0 | `bsim3v0` | version 3.0 | `Bsim3V0` | 12 | 348 | 7774 |
| BSIM4 | `bsim4` | element M; levels 14 and 54, version 4.8 | `Bsim4` | 43 | 891 | 22990 |
| BSIM4v7 | `bsim4v7` | version 4.7 | `Bsim4V7` | 37 | 886 | 22725 |
| BSIM4v6 | `bsim4v6` | version 4.6 | `Bsim4V6` | 36 | 830 | 21687 |
| BSIM4v5 | `bsim4v5` | version 4.5 | `Bsim4V5` | 36 | 752 | 20204 |
| B3SOIFD | `bsim3soi_fd` | element M; level 55 | `Bsim3SoiFd` | 16 | 389 | 11524 |
| B3SOIDD | `bsim3soi_dd` | element M; level 56 | `Bsim3SoiDd` | 16 | 389 | 12491 |
| B3SOIPD | `bsim3soi_pd` | element M; level 57 | `Bsim3SoiPd` | 25 | 467 | 13178 |
| B4SOI | `bsimsoi` | element M; levels 10 and 58 | `Bsim4Soi` | 37 | 911 | 27924 |
| SOI3 | `soi3` | element M; level 60 | `Soi3` | 25 | 83 | 6819 |
| HiSIM2 | `hisim2` | element M; level 68 | `Hisim2` | 32 | 544 | 18355 |
| HiSIMHV1 | `hisimhv1` | element M; level 73, version 1.x | `HisimHv1` | 38 | 608 | 18052 |
| HiSIMHV2 | `hisimhv2` | element M; level 73, version 2.x | `HisimHv2` | 38 | 728 | 20326 |
| NUMD, NUMD2, NBJT, NBJT2, NUMOS | `numd`, `numd2`, `nbjt`, `nbjt2`, `numos` | `.model … numd\|nbjt\|numos` | refused (D6) | 29 | 1 | 9566 |
| NDEV | `ndev` | `.model … ndev` | refused | 0 | 2 | 637 |

**Totals.** 58 device kinds in ngspice-47. In scope: 52, in 51 directories, with **757 instance parameters and 10,232 model parameters (10,989 settable parameters)** and about 400,000 lines of C behind them. Refused by name: 6. Beside the device library, ngspice-47 ships 81 XSPICE code models in seven libraries (`models/excluded.md`), which D7 leaves to a follow-up.

**What each analysis needs of a model.** In ngspice a device has one C function per analysis. In MSL one `analog` block serves every analysis (§11.1), so a model that is right at DC and has its charges and its noise sources written is right in AC, in pole-zero, in transfer function, in sensitivity and in a periodic analysis without further code. The table of `design.md` §5 gives, per model, which ngspice entry points exist; a model for which ngspice has no noise function gets no noise source here.

---

## User Stories

### P1: The inventory is complete and true ⭐ MVP

**User Story**: As a user who comes from ngspice, I want every device of ngspice accounted for with every parameter it accepts, so I know before I start whether my circuit can be written.

**Why P1**: The user's first priority: map the models before designing or implementing anything.

**Acceptance Criteria** (SPM-01 to SPM-06):

1. The file `models/<family>.md` SHALL list, for each of the 58 device kinds, every entry of its instance and model `IFparm` tables in ngspice-47, with its name, type, direction, description and the default ngspice assigns (SPM-01)
2. WHEN `tools/extract.py` and `tools/render.py` run against the tag `ngspice-47` THEN they SHALL reproduce `models/*.md` byte for byte, and a test SHALL fail when a committed file differs from the rendering (SPM-01)
3. The package SHALL declare one MSL param for each settable ngspice parameter of an in-scope device, or the parameter SHALL be in the closed list of `design.md` §3.9 with the reason it has no param; a guard SHALL fail when a parameter is in neither (SPM-02)
4. IF a parameter of the closed list is one a user could write in ngspice THEN the list SHALL name the MSL form that replaces it or the owner of the refusal (SPM-02)
5. Each in-scope device SHALL state the analyses ngspice supports for it (its `SPICEdev` entry points) and how MSL provides each one (SPM-03)
6. The six refused devices and the 81 code models SHALL be listed by name with their owner in `models/excluded.md`, in the Book and in `GAPS.md` (SPM-04)
7. The ngspice version the inventory was taken from SHALL be recorded in the package and SHALL equal the version the parity harness asserts (SPM-05)
8. WHEN a family's interface is submitted for approval THEN every param of the family SHALL have its unit, its default, its given-ness rule and its `where` condition written in the family's page, taken from the ngspice sources and the model's reference manual (SPM-06)

**Independent Test**: run the two tools against a fresh checkout of `ngspice-47`; `git diff` is empty. Pick ten parameters from ngspice's manual at random; each is in a table.

---

### P1: The interface is MSL's own ⭐ MVP

**User Story**: As a designer, I want to place a device and give it a model card with the constructs MSL already has, so I learn nothing that is not the language.

**Why P1**: The user's second priority, and the part they approve first.

**Acceptance Criteria** (SPM-07 to SPM-18):

1. The package SHALL be named `spice`, with its crate root at `packages/spice/src/lib.msl`, and its namespaces SHALL be those of `design.md` §2 (SPM-07)
2. Every model SHALL be an ordinary MSL module with behavior blocks; the package SHALL contain no `extern` module and SHALL use no construct that a user's package could not use (SPM-08)
3. A param SHALL carry the ngspice name of the parameter in lower case; the only other spellings SHALL be those of the rename table of `design.md` §3.2 (SPM-09)
4. Every module, port, namespace, enum, variant, function and operating-point variable the package adds SHALL be a full word or an acronym that is the device's established name (`Bjt`, `Mosfet`, `Bsim4`, `Vbic`) (SPM-09)
5. Every param SHALL have a `///` text that gives its meaning, its unit and its ngspice name where the two differ (SPM-10)
6. WHEN ngspice assigns a constant default and reads the parameter's given flag nowhere else THEN the param SHALL be declared with that default; WHEN ngspice computes the default from other parameters, or behaves differently when the parameter is not written, THEN the param SHALL be an `Option` and the computation SHALL be a body `let` (SPM-11)
7. WHEN ngspice refuses a parameter value with an error THEN the param SHALL carry a `where` that refuses the same values; WHEN ngspice replaces or clamps a value THEN the model SHALL do the same and the param's text SHALL say so (SPM-12)
8. A model card SHALL be a template of the device module, and a shared card SHALL be a `const` or a function that returns the template; the package SHALL define no second type that mirrors a module's params (SPM-13)
9. WHEN one ngspice device accepts two sets of required parameters (a resistor by value or by sheet resistance and geometry) THEN the package SHALL give each set a module of its own, and each module SHALL declare as required exactly the parameters its set requires (SPM-14)
10. The polarity of a device SHALL be a param of an enum type whose variants are ngspice's type keywords, with ngspice's default (SPM-15)
11. WHEN ngspice lets a device have an optional terminal (a thermal node, a body contact) THEN each terminal list SHALL be a module of its own (SPM-16)
12. Every quantity ngspice reports for an instance (`gm`, `gds`, `vth`, `vdsat`, `cgs`, …) SHALL be an operating-point variable of the same name, except node numbers and sensitivity outputs (SPM-17)
13. The instance temperature SHALL follow ngspice's rule from the params `temp` and `dtemp` and the run's `temperature()`, and the multiplier `m` SHALL combine with `multiplicity()` as §8.5.5 gives (SPM-18)

**Independent Test**: the examples of `interface-review.md` are compiled and run by a test of the `book` target, unchanged.

---

### P1: Each model is ngspice's model ⭐ MVP

**User Story**: As a designer, I want the model to compute what ngspice computes, so that a circuit I trust in ngspice I can trust here.

**Why P1**: A requirement in the user's words: faithful to its behaviour and its equations.

**Acceptance Criteria** (SPM-19 to SPM-26):

1. Each model SHALL compute the currents and charges of its ngspice load function for every combination of parameters ngspice accepts, including every `level`, `version` and model-selector parameter of the device (SPM-19)
2. Each model SHALL apply the temperature update of its ngspice temperature function, with ngspice's physical constants (`spice::constants`), at every instance temperature (SPM-20)
3. Each model SHALL place a `noise` source for each source of its ngspice noise function, with the same density and the same name, and SHALL place none where ngspice has none (SPM-21)
4. Each junction and each gate voltage that ngspice limits SHALL be limited with the same function (`pnjlim`, `fetlim`, `limvds`, `limitlog`) and the same critical voltage (SPM-22)
5. WHEN ngspice adds an internal node for a parasitic resistance THEN the model SHALL have that node when the resistance is positive and SHALL merge it when it is zero, as ngspice does (SPM-23)
6. The Jacobian SHALL be the derivative of the model's own contributions (§11 rule 5); IF ngspice's hand-written derivative differs from the derivative of its current so that an AC or noise result differs beyond the contract THEN the difference SHALL be recorded as a deviation with its evidence (SPM-24)
7. Each model file SHALL name the ngspice files and functions it was written from and SHALL carry ngspice's licence notice (SPM-25)
8. IF a model needs a capability the language or the solver lacks THEN the model SHALL be refused with a diagnostic that names the capability, and `GAPS.md` SHALL list it with an owner; no model SHALL approximate silently (SPM-26)

**Independent Test**: for one model per family, set every model-selector parameter to each value ngspice accepts and compare the operating point with ngspice.

---

### P1: Parity with ngspice, for every model ⭐ MVP

**User Story**: As the maintainer, I want each model compared with ngspice in every region and every analysis, so that a wrong equation fails a test.

**Why P1**: The user's fourth priority: correctness in all conditions.

**Acceptance Criteria** (SPM-27 to SPM-36):

1. Each in-scope model SHALL have twins (`.cir` and `.msl`) for every cell of its parity matrix in `design.md` §6, and a guard SHALL fail when a cell has no twin and no recorded refusal (SPM-27)
2. WHEN a twin is compared THEN the comparison SHALL use one of the existing `ToleranceContract` constants and no other bound (SPM-28)
3. IF a model does not meet a contract THEN the model SHALL be corrected, or the difference SHALL be recorded in the deviation ledger with the ngspice source lines that cause it and the user's approval; no contract SHALL be widened (SPM-28)
4. Each model's twins SHALL cover each operating region of the device, each polarity, and DC sweeps on each terminal pair that the device's characteristic curves use (SPM-29)
5. Each model with charge storage SHALL have AC twins at three bias points at least, and transient twins that switch the device through its regions (SPM-30)
6. Each model with a noise function SHALL have noise twins at a thermal-dominated and a flicker-dominated frequency, per source (SPM-31)
7. Each model SHALL have twins at −40 °C, 27 °C and 125 °C, and one with `tnom` different from 27 °C (SPM-32)
8. Each settable param of a model SHALL be given a value different from its default in one twin at least, and a guard SHALL list the params no twin sets (SPM-33)
9. WHEN ngspice ships a test deck for a model (`tests/bsim3`, `tests/bsim4`, `tests/hisim`, `tests/vbic`, …) THEN the parity corpus SHALL include its cases (SPM-34)
10. Each twin SHALL pass on both backends (`interp` and `rust`) (SPM-35)
11. WHEN ngspice is not installed, or reports another version than the reference, THEN the parity targets SHALL say so and SHALL not report a pass (SPM-36)

**Independent Test**: `cargo test -p piperine-parity` with ngspice-47 installed; change one coefficient of one model and see its twin fail with the fixture, the quantity and the point.

---

### P1: The Book has a manual of the library ⭐ MVP

**User Story**: As a newcomer, I want a manual that says what each device is, how to place it and what each parameter does, so I can use the library without reading ngspice's sources.

**Why P1**: Asked for in the request, as a user manual.

**Acceptance Criteria** (SPM-37 to SPM-41):

1. The Book SHALL gain a part "The SPICE model library" with the chapters of `design.md` §7, one per device category (SPM-37)
2. Each chapter SHALL explain each device before its tables: what it models, when to choose it, a minimal placement, a model card, and one use case at least with its result (SPM-38)
3. Each chapter SHALL explain every param of its devices, grouped by what it affects, with its unit, default and meaning; the reference tables SHALL be generated from the `///` texts of the package, and a test SHALL fail when the Book and the package differ (SPM-39)
4. Every listing of the part SHALL be compiled and run by the `book` test target (SPM-40)
5. The part SHALL hold the translation table from ngspice netlist forms to MSL, the list of what differs from ngspice on purpose, and the list of what is refused with its owner (SPM-41)

**Independent Test**: a reader places a diode with a datasheet card and reads its operating point, from the Book alone.

---

### P2: The process keeps a family honest

**Acceptance Criteria** (SPM-42 to SPM-44):

1. A family SHALL be implemented only after the user approved its interface page; the approval SHALL be recorded with its date and words (SPM-42)
2. WHEN a family closes THEN its Book chapter, its parity matrix and its `GAPS.md` rows SHALL be in the same merge (SPM-43)
3. The capability a model family needs of the language, the solver and the device ABI SHALL be checked against `design.md` §8 before the family starts, and a missing one SHALL be owned by a feature before the family is scheduled (SPM-44)

---

## Requirement Traceability

| ID | Story | Phase | Status |
| --- | --- | --- | --- |
| SPM-01 … SPM-06 | P1: inventory | Design | Draft |
| SPM-07 … SPM-18 | P1: interface | Design | Draft — awaiting approval |
| SPM-19 … SPM-26 | P1: fidelity | Design | Draft |
| SPM-27 … SPM-36 | P1: parity | Design | Draft |
| SPM-37 … SPM-41 | P1: the Book | Design | Draft |
| SPM-42 … SPM-44 | P2: process | Design | Draft |

**Coverage:** 44 requirements. No `tasks.md` yet: tasks are written after the interface is approved.

## Sources

- ngspice-47: `src/spicelib/devices/*` (the device sources; `<dev>.c` holds the parameter tables, `<dev>init.c` the entry points, `*set*.c` and `*temp.c` the defaults, `*load.c` the equations, `*noi*.c` the noise sources), `src/spicelib/parser/inpdomod.c` (model types and levels), `src/include/ngspice/const.h` (physical constants), `COPYING` (licences), `tests/` (the model decks).
- `docs/spec/` Parts I to VIII: the language. §8 (modules, params, templates, run settings), §11 (analog), §14 (core primitives), §16 (operating point, analyses the host builds), §18 (limiting), §21 (metadata), §22 (standard library), §3.5 and §3.6 (crates and packages).
- `.specs/features/device-abi-simplification/` on its branch: proposal §22.7, capabilities C1 to C98.
- `crates/piperine-parity/src/{contract,ngspice,corpus,deviation}.rs`, `tests/e2e/twins/`: the parity harness and the fixtures it has.
- Earlier work, read for its findings and not reused: `.specs/features/{ngspice-models,ngspice-bsim3,ngspice-bsim4,ngspice-parity,spice-stdlib}/`.
- The user's request of 2026-11-27, restated at the head of `interface-review.md`.

## Success Criteria

- [ ] A user translates an ngspice netlist of in-scope devices line by line with the Book's table and gets ngspice's answers under the contract.
- [ ] No ngspice parameter of an in-scope device is unknown to the package: each is a param or a named entry of the closed list.
- [ ] Every model has a filled parity matrix, on both backends, with no contract changed.
- [ ] A foundry BSIM4 card is one MSL function, and placing a transistor from it is one line.
- [ ] The Book's part is enough to use every device without opening ngspice's manual.
