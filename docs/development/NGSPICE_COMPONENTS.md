# ngspice Components Reference

Generated from ngspice source at `~/Git/ngspice/src/spicelib/devices/`. Excludes XSpice.

Parameter flags used in source: `IOP` = input/output param, `IOPA` = with area scaling, `IOPU` = user-settable, `IP` = input only, `OP` = output only. Only settable (`IOP*`, `IP`) parameters are listed below.

---

## R — Resistor

**Syntax:** `R<name> <n+> <n-> [model] <value> [params...]`  
**Nodes:** R+ R−

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| resistance | real | — | Resistance (Ω); required unless using model+w+l |
| ac | real | — | AC-only resistance value (Ω) |
| temp | real | circuit temp | Instance operating temperature (°C) |
| dtemp | real | 0 | Temperature delta from circuit (°C) |
| l | real | 10 µm | Length (m) |
| w | real | 10 µm | Width (m) |
| m | real | 1 | Parallel multiplier |
| tc | real | 0 | First-order temperature coefficient (1/°C) |
| tc1 | real | 0 | First-order temperature coefficient (alias) |
| tc2 | real | 0 | Second-order temperature coefficient (1/°C²) |
| scale | real | 1 | Scale factor |
| noisy | int | 1 | Generate thermal noise (0=off) |
| bv_max | real | ∞ | Maximum voltage (V) |

### Model Parameters (`.model <name> R`)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| rsh | real | 0 | Sheet resistance (Ω/□) |
| narrow | real | 0 | Width narrowing (m) |
| dw | real | 0 | Width narrowing alias |
| short | real | 0 | Length shortening (m) |
| dlr | real | 0 | Length shortening alias |
| tc1 | real | 0 | First-order temperature coefficient (1/°C) |
| tc2 | real | 0 | Second-order temperature coefficient (1/°C²) |
| defw | real | 10 µm | Default width (m) |
| l | real | 10 µm | Default length (m) |
| kf | real | 0 | Flicker noise coefficient |
| af | real | 0 | Flicker noise exponent |
| tnom | real | 27°C | Measurement temperature (°C) |
| r | real | — | Model default resistance (Ω) |
| bv_max | real | ∞ | Maximum voltage (V) |

---

## C — Capacitor

**Syntax:** `C<name> <n+> <n-> [model] <value> [params...]`  
**Nodes:** C+ C−

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| capacitance | real | — | Capacitance (F) |
| cap / c | real | — | Capacitance aliases |
| ic | real | 0 | Initial voltage (V) |
| temp | real | circuit temp | Instance temperature (°C) |
| dtemp | real | 0 | Temperature delta (°C) |
| w | real | model defw | Width (m) |
| l | real | model defl | Length (m) |
| m | real | 1 | Parallel multiplier |
| tc1 | real | model tc1 | First-order temp coefficient (1/°C) |
| tc2 | real | model tc2 | Second-order temp coefficient (1/°C²) |
| scale | real | 1 | Scale factor |
| bv_max | real | ∞ | Maximum voltage (V) |

### Model Parameters (`.model <name> C`)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| cap | real | 0 | Model capacitance (F) |
| cj | real | 0 | Bottom capacitance per area (F/m²) |
| cjsw | real | 0 | Sidewall capacitance per metre (F/m) |
| defw | real | 10 µm | Default width (m) |
| defl | real | 0 | Default length (m) |
| narrow | real | 0 | Width correction (m) |
| short | real | 0 | Length correction (m) |
| del | real | 0 | Combined length/width correction (m) |
| tc1 | real | 0 | First-order temp coefficient (1/°C) |
| tc2 | real | 0 | Second-order temp coefficient (1/°C²) |
| tnom | real | 27°C | Measurement temperature (°C) |
| di | real | 0 | Relative dielectric constant |
| thick | real | 0 | Insulator thickness (m) |
| bv_max | real | ∞ | Maximum voltage (V) |

---

## L — Inductor

**Syntax:** `L<name> <n+> <n-> [model] <value> [params...]`  
**Nodes:** L+ L−

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| inductance | real | — | Inductance (H) |
| ic | real | 0 | Initial current (A) |
| temp | real | circuit temp | Instance temperature (°C) |
| dtemp | real | 0 | Temperature delta (°C) |
| m | real | 1 | Parallel multiplier |
| tc1 | real | 0 | First-order temp coefficient (1/°C) |
| tc2 | real | 0 | Second-order temp coefficient (1/°C²) |
| scale | real | 1 | Scale factor |
| nt | real | — | Number of turns |

### Model Parameters (`.model <name> L`)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| ind | real | — | Model inductance (H) |
| tc1 | real | 0 | First-order temp coefficient (1/°C) |
| tc2 | real | 0 | Second-order temp coefficient (1/°C²) |
| tnom | real | 27°C | Measurement temperature (°C) |
| csect | real | — | Cross-sectional area (m²) |
| length | real | — | Physical length (m) |
| nt | real | — | Number of turns |
| mu | real | — | Relative magnetic permeability |

---

## K — Mutual Inductance (Coupling)

**Syntax:** `K<name> L<xxx> L<yyy> <coupling>`  
**Nodes:** (none — references two inductors)

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| k | real | — | Coupling coefficient (0 < k ≤ 1) |
| inductor1 | instance | — | First coupled inductor |
| inductor2 | instance | — | Second coupled inductor |

*(No model card.)*

---

## V — Voltage Source

**Syntax:** `V<name> <n+> <n-> [DC <val>] [AC <mag> [<phase>]] [transient]`  
**Nodes:** V+ V−

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| dc | real | 0 | DC value (V) |
| acmag | real | 0 | AC magnitude (V) |
| acphase | real | 0 | AC phase (°) |
| pulse | realvec | — | PULSE(V1 V2 Td Tr Tf Pw Per) |
| sin / sine | realvec | — | SIN(Vo Va Freq Td Theta) |
| exp | realvec | — | EXP(V1 V2 Td1 Tau1 Td2 Tau2) |
| pwl | realvec | — | PWL(t1 v1 t2 v2 ...) |
| sffm | realvec | — | SFFM(Vo Va Fc Mdi Fs) |
| am | realvec | — | AM(Va Vo Fm Fc Td) |
| trnoise | realvec | — | Transient noise description |
| trrandom | realvec | — | Random source description |
| r | real | — | PWL repeat time (s) |
| td | real | 0 | PWL delay (s) |

*(No model card.)*

---

## I — Current Source

**Syntax:** `I<name> <n+> <n-> [DC <val>] [AC <mag> [<phase>]] [transient]`  
**Nodes:** I+ I−

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| dc / c | real | 0 | DC value (A) |
| m | real | 1 | Parallel multiplier |
| acmag | real | 0 | AC magnitude (A) |
| acphase | real | 0 | AC phase (°) |
| pulse | realvec | — | PULSE(...) |
| sin / sine | realvec | — | SIN(...) |
| exp | realvec | — | EXP(...) |
| pwl | realvec | — | PWL(...) |
| sffm | realvec | — | SFFM(...) |
| am | realvec | — | AM(...) |
| trnoise | realvec | — | Transient noise |
| trrandom | realvec | — | Random source |

*(No model card.)*

---

## B — Arbitrary Source (ASRC)

**Syntax:** `B<name> <n+> <n-> [V=<expr>] [I=<expr>]`  
**Nodes:** src+ src−

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| v | parsetree | — | Voltage expression |
| i | parsetree | — | Current expression |
| temp | real | circuit temp | Instance temperature (°C) |
| dtemp | real | 0 | Temperature delta (°C) |
| tc1 | real | 0 | First-order temp coefficient |
| tc2 | real | 0 | Second-order temp coefficient |
| reciproctc | int | 0 | Use reciprocal temperature behaviour |

*(No model card.)*

---

## E — Voltage-Controlled Voltage Source (VCVS)

**Syntax:** `E<name> <n+> <n-> <nc+> <nc-> <gain>`  
**Nodes:** pos, neg, controlling_pos, controlling_neg

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| gain | real | — | Voltage gain (V/V) |

*(No model card.)*

---

## G — Voltage-Controlled Current Source (VCCS)

**Syntax:** `G<name> <n+> <n-> <nc+> <nc-> <transconductance>`  
**Nodes:** pos, neg, controlling_pos, controlling_neg

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| gain | real | — | Transconductance (A/V) |
| m | real | 1 | Parallel multiplier |

*(No model card.)*

---

## H — Current-Controlled Voltage Source (CCVS)

**Syntax:** `H<name> <n+> <n-> <vsource> <transresistance>`  
**Nodes:** pos, neg

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| gain | real | — | Transresistance (V/A) |
| control | instance | — | Controlling voltage source name |

*(No model card.)*

---

## F — Current-Controlled Current Source (CCCS)

**Syntax:** `F<name> <n+> <n-> <vsource> <gain>`  
**Nodes:** pos, neg

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| gain | real | — | Current gain (A/A) |
| control | instance | — | Controlling voltage source name |
| m | real | 1 | Parallel multiplier |

*(No model card.)*

---

## D — Diode

**Syntax:** `D<name> <anode> <cathode> <model> [params...]`  
**Nodes:** D+ D−  
**Model type keyword:** `D`

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| area | real | 1 | Area factor |
| pj | real | 0 | Perimeter factor |
| w | real | — | Width (m) |
| l | real | — | Length (m) |
| m | real | 1 | Parallel multiplier |
| off | flag | — | Device initially off |
| ic | real | — | Initial voltage (V) |
| temp | real | circuit temp | Instance temperature (°C) |
| dtemp | real | 0 | Temperature delta (°C) |

### Model Parameters (`.model <name> D`)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| level | int | 1 | Model level (1 or 3) |
| is / js | real | 1e-14 | Saturation current (A) |
| jsw | real | 0 | Sidewall saturation current (A) |
| n | real | 1 | Emission coefficient |
| ns | real | 1 | Sidewall emission coefficient |
| rs | real | 0 | Ohmic series resistance (Ω) |
| trs / trs1 | real | 0 | RS first-order temp coefficient (1/°C) |
| trs2 | real | 0 | RS second-order temp coefficient (1/°C²) |
| tt | real | 0 | Transit time (s) |
| cjo / cj0 | real | 0 | Zero-bias junction capacitance (F) |
| vj / pb | real | 1 | Junction built-in potential (V) |
| m / mj | real | 0.5 | Grading coefficient |
| cjp / cjsw | real | 0 | Sidewall junction capacitance (F/m) |
| php | real | 1 | Sidewall junction potential (V) |
| mjsw | real | 0.33 | Sidewall grading coefficient |
| ikf / ik | real | 0 | Forward knee current (A) |
| ikr | real | 0 | Reverse knee current (A) |
| eg | real | 1.11 | Activation energy (eV) |
| xti | real | 3 | IS temperature exponent |
| kf | real | 0 | Flicker noise coefficient |
| af | real | 1 | Flicker noise exponent |
| fc | real | 0.5 | Forward bias capacitance fit parameter |
| bv | real | ∞ | Reverse breakdown voltage (V) |
| ibv / ib | real | 1e-3 | Current at breakdown (A) |
| nbv | real | 1 | Breakdown emission coefficient |
| tcv | real | 0 | Breakdown voltage temp coefficient (V/°C) |
| tnom / tref | real | 27°C | Parameter measurement temperature (°C) |
| area | real | 1 | Default area factor |
| pj | real | 0 | Default perimeter factor |
| fv_max | real | ∞ | Max forward voltage (V) |
| bv_max | real | ∞ | Max reverse voltage (V) |
| tlev | int | 0 | Temperature equation selector |
| tlevc | int | 0 | Capacitance temperature equation selector |
| cta / ctc | real | 0 | Area junction capacitance temp coefficient |
| ctp | real | 0 | Perimeter junction capacitance temp coefficient |
| tpb / tvj | real | 0 | Area junction potential temp coefficient |
| tphp | real | 0 | Perimeter junction potential temp coefficient |

---

## Q — BJT (Bipolar Junction Transistor)

**Syntax:** `Q<name> <collector> <base> <emitter> [substrate] <model> [params...]`  
**Nodes:** collector, base, emitter, (substrate)  
**Model type keywords:** `NPN`, `PNP`

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| area | real | 1 | Emitter area factor |
| areab | real | 1 | Base area factor |
| areac | real | 1 | Collector area factor |
| m | real | 1 | Parallel multiplier |
| off | flag | — | Device initially off |
| icvbe | real | — | Initial VBE (V) |
| icvce | real | — | Initial VCE (V) |
| temp | real | circuit temp | Instance temperature (°C) |
| dtemp | real | 0 | Temperature delta (°C) |

### Model Parameters (`.model <name> NPN` or `PNP`)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| is | real | 1e-16 | Saturation current (A) |
| bf | real | 100 | Ideal forward beta |
| nf | real | 1 | Forward emission coefficient |
| vaf / va | real | ∞ | Forward Early voltage (V) |
| ikf / ik | real | ∞ | Forward beta roll-off current (A) |
| ise | real | 0 | B-E leakage saturation current (A) |
| ne | real | 1.5 | B-E leakage emission coefficient |
| br | real | 1 | Ideal reverse beta |
| nr | real | 1 | Reverse emission coefficient |
| var / vb | real | ∞ | Reverse Early voltage (V) |
| ikr | real | ∞ | Reverse beta roll-off current (A) |
| isc | real | 0 | B-C leakage saturation current (A) |
| nc | real | 2 | B-C leakage emission coefficient |
| rb | real | 0 | Zero-bias base resistance (Ω) |
| irb | real | ∞ | Current for base resistance midpoint (A) |
| rbm | real | rb | Minimum base resistance (Ω) |
| re | real | 0 | Emitter resistance (Ω) |
| rc | real | 0 | Collector resistance (Ω) |
| cje | real | 0 | B-E zero-bias depletion capacitance (F) |
| vje / pe | real | 0.75 | B-E built-in potential (V) |
| mje / me | real | 0.33 | B-E grading coefficient |
| tf | real | 0 | Forward transit time (s) |
| xtf | real | 0 | TF bias dependence coefficient |
| vtf | real | ∞ | VBC voltage for TF dependence (V) |
| itf | real | 0 | TF high-current dependence (A) |
| ptf | real | 0 | Excess phase (°) |
| cjc | real | 0 | B-C zero-bias depletion capacitance (F) |
| vjc / pc | real | 0.75 | B-C built-in potential (V) |
| mjc / mc | real | 0.33 | B-C grading coefficient |
| xcjc | real | 1 | Fraction of CJC to internal base |
| tr | real | 0 | Reverse transit time (s) |
| cjs / ccs | real | 0 | Substrate zero-bias capacitance (F) |
| vjs / ps | real | 0.75 | Substrate junction potential (V) |
| mjs / ms | real | 0 | Substrate grading coefficient |
| xtb | real | 0 | Beta temperature exponent |
| eg | real | 1.11 | Energy gap (eV) |
| xti | real | 3 | IS temperature exponent |
| fc | real | 0.5 | Forward-bias capacitance fit parameter |
| kf | real | 0 | Flicker noise coefficient |
| af | real | 1 | Flicker noise exponent |
| iss | real | 0 | Substrate junction saturation current (A) |
| ns | real | 1 | Substrate emission coefficient |
| tnom / tref | real | 27°C | Parameter measurement temperature (°C) |
| tlev | int | 0 | Temperature equation selector |
| tlevc | int | 0 | Capacitance temperature equation selector |

*(Many extended temperature coefficients: tbf1, tbf2, tbr1, tbr2, tikf1/2, tikr1/2, tirb1/2, tnc1/2, tne1/2, tnf1/2, tnr1/2, trb1/2, trc1/2, tre1/2, trm1/2, tvaf1/2, tvar1/2, ctc, cte, cts, tvjc, tvje, tvjs, titf1/2, ttf1/2, ttr1/2, tmje1/2, tmjc1/2 — all default 0.)*

---

## J — JFET (Junction Field-Effect Transistor)

**Syntax:** `J<name> <drain> <gate> <source> <model> [params...]`  
**Nodes:** drain, gate, source  
**Model type keywords:** `NJF`, `PJF`

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| area | real | 1 | Area factor |
| m | real | 1 | Parallel multiplier |
| off | flag | — | Device initially off |
| ic | realvec | — | [VDS, VGS] initial conditions (V) |
| temp | real | circuit temp | Instance temperature (°C) |
| dtemp | real | 0 | Temperature delta (°C) |

### Model Parameters (`.model <name> NJF` or `PJF`)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| vt0 / vto | real | -2 | Threshold (pinch-off) voltage (V) |
| beta | real | 1e-4 | Transconductance parameter (A/V²) |
| lambda | real | 0 | Channel-length modulation (1/V) |
| rd | real | 0 | Drain ohmic resistance (Ω) |
| rs | real | 0 | Source ohmic resistance (Ω) |
| cgs | real | 0 | Gate-source capacitance (F) |
| cgd | real | 0 | Gate-drain capacitance (F) |
| pb | real | 1 | Gate junction potential (V) |
| is | real | 1e-14 | Gate junction saturation current (A) |
| fc | real | 0.5 | Forward-bias capacitance fit parameter |
| b | real | 1.0 | Doping tail parameter |
| kf | real | 0 | Flicker noise coefficient |
| af | real | 1 | Flicker noise exponent |
| tnom | real | 27°C | Parameter measurement temperature (°C) |
| tcv | real | 0 | VT0 temperature coefficient (V/°C) |
| bex | real | 0 | Mobility temperature exponent |
| nlev | int | 2 | Noise equation selector |
| gdsnoi | real | 1.0 | Channel noise coefficient |

---

## M — MOSFET (MOS Levels 1, 2, 3, 6, 9)

**Syntax:** `M<name> <drain> <gate> <source> <bulk> <model> [params...]`  
**Nodes:** drain, gate, source, bulk  
**Model type keywords:** `NMOS`, `PMOS`

### Instance Parameters (common to all levels)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| l | real | model defl | Channel length (m) |
| w | real | model defw | Channel width (m) |
| ad | real | circuit default | Drain diffusion area (m²) |
| as | real | circuit default | Source diffusion area (m²) |
| pd | real | 0 | Drain perimeter (m) |
| ps | real | 0 | Source perimeter (m) |
| nrd | real | 0 | Drain squares |
| nrs | real | 0 | Source squares |
| m | real | 1 | Parallel multiplier |
| off | flag | — | Device initially off |
| icvds | real | 0 | Initial VDS (V) |
| icvgs | real | 0 | Initial VGS (V) |
| icvbs | real | 0 | Initial VBS (V) |
| temp | real | circuit temp | Instance temperature (°C) |
| dtemp | real | 0 | Temperature delta (°C) |

### Model Parameters — Level 1 (`.model <name> NMOS level=1`)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| level | int | 1 | MOSFET model level |
| vto / vt0 | real | 0 | Threshold voltage (V) |
| kp | real | 20 µA/V² | Transconductance (A/V²) |
| gamma | real | 0 | Bulk threshold parameter (V^½) |
| phi | real | 0.6 | Surface potential (V) |
| lambda | real | 0 | Channel-length modulation (1/V) |
| rd | real | 0 | Drain ohmic resistance (Ω) |
| rs | real | 0 | Source ohmic resistance (Ω) |
| cbd | real | 0 | B-D junction capacitance (F) |
| cbs | real | 0 | B-S junction capacitance (F) |
| is | real | 1e-14 | Bulk junction saturation current (A) |
| pb | real | 0.8 | Bulk junction potential (V) |
| cgso | real | 0 | Gate-source overlap cap/width (F/m) |
| cgdo | real | 0 | Gate-drain overlap cap/width (F/m) |
| cgbo | real | 0 | Gate-bulk overlap cap/length (F/m) |
| rsh | real | 0 | Sheet resistance (Ω/□) |
| cj | real | 0 | Bottom junction cap/area (F/m²) |
| mj | real | 0.5 | Bottom grading coefficient |
| cjsw | real | 0 | Sidewall junction cap/length (F/m) |
| mjsw | real | 0.5 | Sidewall grading coefficient |
| js | real | 0 | Bulk junction sat. current density (A/m²) |
| tox | real | ∞ | Oxide thickness (m) |
| ld | real | 0 | Lateral diffusion (m) |
| u0 / uo | real | 600 (N), 250 (P) | Surface mobility (cm²/V·s) |
| fc | real | 0.5 | Forward-bias capacitance fit parameter |
| nsub | real | — | Substrate doping (1/cm³) |
| tpg | int | — | Gate material type (1=opp., -1=same, 0=Al) |
| nss | real | 0 | Surface state density (1/cm²) |
| tnom | real | 27°C | Measurement temperature (°C) |
| kf | real | 0 | Flicker noise coefficient |
| af | real | 1 | Flicker noise exponent |

*(Level 2 and 3 add: nfs, xj, ucrit, uexp, utra, vmax, neff, delta, eta, theta, kappa, tpg — see ngspice manual ch. 6)*

---

## M — MOSFET BSIM3v32 (Level 8 / BSIM3)

**Syntax:** `M<name> <d> <g> <s> <b> <model> [params...]`  
**Model type keywords:** `NMOS`, `PMOS` with `level=8`

### Key Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| l | real | — | Channel length (m) |
| w | real | — | Channel width (m) |
| m | real | 1 | Parallel multiplier |
| ad, as | real | — | Drain/Source area (m²) |
| pd, ps | real | — | Drain/Source perimeter (m) |
| nrd, nrs | real | — | Drain/Source squares |

### Key Model Parameters (`.model <name> NMOS level=8`)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| capmod | int | 3 | Capacitance model (0–3) |
| mobmod | int | 1 | Mobility model (1–3) |
| noimod | int | 1 | Noise model |
| nqsmod | int | 0 | Non-quasi-static selector |
| tox | real | — | Gate oxide thickness (m) |
| nch | real | 1.7e17 | Channel doping (1/cm³) |
| ngate | real | 0 | Poly-gate doping (1/cm³) |
| vth0 | real | 0.5 (N) / -0.5 (P) | Threshold voltage (V) |
| k1 | real | computed | First-order body effect coefficient |
| k2 | real | computed | Second-order body effect |
| k3 | real | 80 | Narrow-width effect coefficient |
| vsat | real | 8e4 | Saturation velocity (m/s) |
| u0 | real | 670 (N) / 250 (P) | Low-field mobility (cm²/V·s) |
| dvt0 | real | 2.2 | Short-channel effect coeff 0 |
| dvt1 | real | 0.53 | Short-channel effect coeff 1 |
| eta0 | real | 0.08 | DIBL subthreshold coefficient |
| pclm | real | 1.3 | Channel-length modulation coefficient |
| rdsw | real | 0 | Source-drain resistance/width (Ω·µm) |
| cgso | real | computed | Gate-source overlap cap (F/m) |
| cgdo | real | computed | Gate-drain overlap cap (F/m) |
| cgbo | real | 0 | Gate-bulk overlap cap (F/m) |
| xpart | real | 0 | Charge partition (0=40/60, 1=0/100) |
| tnom | real | 27°C | Measurement temperature (°C) |
| kf | real | 0 | Flicker noise coefficient |
| ef | real | 1 | Flicker noise frequency exponent |
| em | real | 4.1e7 | Saturation field for flicker noise (V/m) |

*(BSIM3v32 has ~300 model parameters. The above are the most commonly specified. See b3v32.c and ngspice manual ch. 18 for the complete list.)*

---

## M — MOSFET BSIM4 (Level 14 / BSIM4)

**Syntax:** `M<name> <d> <g> <s> <b> <model> [params...]`  
**Model type keywords:** `NMOS`, `PMOS` with `level=14`

### Key Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| l | real | — | Channel length (m) |
| w | real | — | Channel width (m) |
| m | real | 1 | Parallel multiplier |
| nf | real | 1 | Number of fingers |
| sa, sb | real | — | Distance OD-edge to poly (m) |
| ad, as | real | — | Drain/Source area (m²) |
| pd, ps | real | — | Drain/Source perimeter (m) |

### Key Model Parameters (`.model <name> NMOS level=14`)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| capmod | int | 2 | Capacitance model selector |
| mobmod | int | 0 | Mobility model selector |
| diomod | int | 1 | Diode IV model selector |
| rdsmod | int | 0 | Bias-dependent S/D resistance model |
| rgatemod | int | 0 | Gate resistance model |
| rbodymod | int | 0 | Body resistance model |
| toxe | real | — | Electrical gate oxide thickness (m) |
| toxp | real | — | Physical gate oxide thickness (m) |
| epsrox | real | 3.9 | Oxide dielectric constant |
| ndep | real | 1.7e17 | Channel doping (1/cm³) |
| vth0 | real | 0.5 (N) / -0.5 (P) | Threshold voltage (V) |
| u0 | real | 670 (N) / 250 (P) | Low-field mobility (cm²/V·s) |
| vsat | real | 8e4 | Saturation velocity (m/s) |
| nfactor | real | 1 | Subthreshold swing coefficient |
| pclm | real | 1.3 | Channel-length modulation coefficient |
| rdsw | real | 200 | S/D resistance per width (Ω·µm) |
| cgso | real | computed | Gate-source overlap cap (F/m) |
| cgdo | real | computed | Gate-drain overlap cap (F/m) |
| cgbo | real | 0 | Gate-bulk overlap cap (F/m) |
| xpart | real | 0 | Charge partition |
| tnom | real | 27°C | Measurement temperature (°C) |
| fnoimod | int | 1 | Flicker noise model |
| kf | real | 0 | Flicker noise coefficient |
| ef | real | 1 | Flicker noise exponent |

*(BSIM4 has ~500+ model parameters. See b4.c and ngspice manual ch. 18 for the complete list.)*

---

## Z — MESFET (Metal-Semiconductor Field-Effect Transistor)

**Syntax:** `Z<name> <drain> <gate> <source> <model> [params...]`  
**Nodes:** drain, gate, source  
**Model type keywords:** `NMF`, `PMF`

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| area / m | real | 1 | Area / multiplier |
| off | flag | — | Device initially off |
| icvds | real | — | Initial VDS (V) |
| icvgs | real | — | Initial VGS (V) |

### Model Parameters (`.model <name> NMF` or `PMF`)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| vt0 / vto | real | -2 | Pinch-off voltage (V) |
| alpha | real | 2 | Saturation voltage parameter (1/V) |
| beta | real | 2.5e-3 | Transconductance parameter (A/V²) |
| lambda | real | 0 | Channel-length modulation (1/V) |
| b | real | 0.3 | Doping tail parameter |
| rd | real | 0 | Drain ohmic resistance (Ω) |
| rs | real | 0 | Source ohmic resistance (Ω) |
| cgs | real | 0 | Gate-source capacitance (F) |
| cgd | real | 0 | Gate-drain capacitance (F) |
| pb | real | 1 | Gate junction potential (V) |
| is | real | 1e-14 | Gate junction saturation current (A) |
| fc | real | 0.5 | Forward-bias capacitance fit parameter |
| kf | real | 0 | Flicker noise coefficient |
| af | real | 1 | Flicker noise exponent |

---

## S — Voltage-Controlled Switch

**Syntax:** `S<name> <n+> <n-> <nc+> <nc-> <model> [ON|OFF]`  
**Nodes:** pos, neg, controlling_pos, controlling_neg  
**Model type keyword:** `SW`

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| on | flag | — | Initially closed |
| off | flag | — | Initially open |

### Model Parameters (`.model <name> SW`)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| vt | real | 0 | Threshold voltage (V) |
| vh | real | 0 | Hysteresis voltage (V) |
| ron | real | 1 Ω | On (closed) resistance (Ω) |
| roff | real | 1/Gmin | Off (open) resistance (Ω) |

---

## W — Current-Controlled Switch

**Syntax:** `W<name> <n+> <n-> <vsource> <model> [ON|OFF]`  
**Nodes:** pos, neg  
**Model type keyword:** `CSW`

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| control | instance | — | Controlling voltage source |
| on | flag | — | Initially closed |
| off | flag | — | Initially open |

### Model Parameters (`.model <name> CSW`)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| it | real | 0 | Threshold current (A) |
| ih | real | 0 | Hysteresis current (A) |
| ron | real | 1 Ω | On (closed) resistance (Ω) |
| roff | real | 1/Gmin | Off (open) resistance (Ω) |

---

## T — Lossless Transmission Line

**Syntax:** `T<name> <A+> <A−> <B+> <B−> [params...]`  
**Nodes:** pos1, neg1, pos2, neg2

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| z0 / zo | real | — | Characteristic impedance (Ω) |
| td | real | — | One-way delay (s) |
| f | real | — | Frequency for `nl` (Hz) |
| nl | real | 0.25 | Normalized length at frequency f |
| v1 | real | 0 | Initial voltage end 1 (V) |
| v2 | real | 0 | Initial voltage end 2 (V) |
| i1 | real | 0 | Initial current end 1 (A) |
| i2 | real | 0 | Initial current end 2 (A) |

*(No model card. Either `td` or both `f` and `nl` must be specified.)*

---

## O — Lossy Transmission Line (LTRA)

**Syntax:** `O<name> <A+> <A−> <B+> <B−> <model>`  
**Nodes:** pos1, neg1, pos2, neg2  
**Model type keyword:** `LTRA`

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| v1, v2 | real | 0 | Initial voltages (V) |
| i1, i2 | real | 0 | Initial currents (A) |

### Model Parameters (`.model <name> LTRA`)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| r | real | 0 | Resistance per metre (Ω/m) |
| l | real | — | Inductance per metre (H/m) |
| g | real | 0 | Conductance per metre (S/m) |
| c | real | — | Capacitance per metre (F/m) |
| len | real | — | Length (m) |
| nocontrol | flag | — | Disable timestep control |
| steplimit | flag | — | Limit step to 0.8×delay |
| lininterp | flag | — | Use linear interpolation |
| quadinterp | flag | — | Use quadratic interpolation |

---

## U — Uniform RC Line

**Syntax:** `U<name> <p1> <p2> <ref> <model> <l> [n=<lumps>]`  
**Nodes:** P1, P2, Ref  
**Model type keyword:** `URC`

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| l | real | — | Length of line (m) |
| n | int | computed | Number of lumped sections |

### Model Parameters (`.model <name> URC`)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| k | real | 1.5 | Propagation constant |
| fmax | real | 1e9 | Maximum frequency of interest (Hz) |
| rperl | real | 1000 | Resistance per unit length (Ω/m) |
| cperl | real | 1e-15 | Capacitance per unit length (F/m) |
| isperl | real | 0 | Saturation current per length (A/m) |
| rsperl | real | 0 | Diode resistance per length (Ω·m) |

---

## P — Coupled Multiconductor Line (CPL)

**Syntax:** `P<name> <in_nodes> <out_nodes> <model> length=<L>`  
**Nodes:** P+ P− pairs  
**Model type keyword:** `CPL`

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| length | real | — | Line length (m) |
| dimension | int | — | Number of coupled lines |

### Model Parameters (`.model <name> CPL`)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| r | realvec | — | Resistance matrix per length (Ω/m) |
| l | realvec | — | Inductance matrix per length (H/m) |
| c | realvec | — | Capacitance matrix per length (F/m) |
| g | realvec | — | Conductance matrix per length (S/m) |

---

## Y — Single Transmission Line (TXL)

**Syntax:** `Y<name> <y1+> <y1−> <model> length=<L>`  
**Nodes:** Y+ Y−  
**Model type keyword:** `TXL`

### Instance Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| length | real | — | Line length (m) |

### Model Parameters (`.model <name> TXL`)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| r | real | 0 | Resistance per length (Ω/m) |
| l | real | — | Inductance per length (H/m) |
| c | real | — | Capacitance per length (F/m) |
| g | real | 0 | Conductance per length (S/m) |

---

## N — OSDI Device (OpenVAF-Compiled Verilog-A)

**Syntax:**
```
.model <modelname> <va_module_name> [param=val ...]
N<name> <node1> ... <nodeN> <modelname>
```
**Nodes:** As defined by the VA module's port list.

The `.osdi` shared library must be loaded before the netlist via:
```
osdi /path/to/module.osdi
```
No built-in model parameters — all parameters are defined by the Verilog-A source.

---

## Analysis Commands (reference)

| Command | Description |
|---------|-------------|
| `op` | DC operating point |
| `dc <src> <start> <stop> <step>` | DC sweep |
| `ac <dec|oct|lin> <N> <fstart> <fstop>` | AC small-signal |
| `tran <tstep> <tstop> [tstart]` | Transient |
| `noise V(<out>) <src> <dec> <N> <f1> <f2>` | Noise analysis |
| `tf V(<out>) <src>` | Transfer function |
| `sens <var>` | DC sensitivity |
| `pz <n1> <n2> <n3> <n4> vol|cur pol|zer|pz` | Pole-zero |

---

## Notes

- `Gmin` (default 1e-12 S): the global minimum conductance added across every p-n junction. Also used as the default `roff` for switches.
- `tnom` (default 27°C): global parameter measurement temperature, overridden per model.
- Parameter aliases (e.g. `vto`/`vt0`, `cj0`/`cjo`) are interchangeable in netlists.

---

## Advanced and Specialty Models

### VBIC — Vertical Bipolar Inter-Company Model

Improved bipolar model with parasitic substrate transistor, self-heating,
and weak avalanche. Replaces Gummel-Poon for modern processes.

**Device letter:** `Q`. **Model type:** `vbic` with `npn` or `pnp` flag.

```spice
.model Q2N3904 vbic npn
+ is=1e-16 nf=1.0 nr=1.0 fc=0.5
+ rcx=10 rci=60 vo=2 gamm=5e-8 hrcf=2
+ rbx=10 rbi=20 re=1 rs=100 rbp=0
+ cje=4e-13 pe=0.75 me=0.33 aje=-0.5
+ cjc=3.5e-13 pc=0.7 mc=0.33 ajc=-0.5
+ tf=3e-10 itf=0.4 xtf=6 vtf=1.7 td=5e-11
+ ea=1.12 eaie=1.12 eaic=1.12
+ rth=300 cth=1e-9
```

**Key Model Parameters:**

| Parameter | Description                                      | Default |
|-----------|--------------------------------------------------|---------|
| `is`      | Transport saturation current                     | 1e-16   |
| `nf`      | Forward emission coefficient                     | 1.0     |
| `nr`      | Reverse emission coefficient                     | 1.0     |
| `vef`     | Forward Early voltage                            | 1e99    |
| `ver`     | Reverse Early voltage                            | 1e99    |
| `ikf`     | Forward knee current                             | 1e99    |
| `ikr`     | Reverse knee current                             | 1e99    |
| `rcx`     | Extrinsic collector resistance (Ω)              | 0       |
| `rci`     | Intrinsic collector resistance (Ω)              | 0       |
| `rbx`     | Extrinsic base resistance (Ω)                   | 0       |
| `rbi`     | Intrinsic base resistance (Ω)                   | 0       |
| `re`      | Emitter resistance (Ω)                          | 0       |
| `rs`      | Substrate resistance (Ω)                        | 0       |
| `vo`      | Epi drift saturation voltage                     | 1e99    |
| `gamm`    | Epi doping parameter                             | 0       |
| `hrcf`    | High-current RC factor                           | 1       |
| `cje`     | B-E zero-bias junction capacitance (F)           | 0       |
| `pe`      | B-E built-in potential (V)                       | 0.75    |
| `me`      | B-E junction grading coefficient                 | 0.33    |
| `cjc`     | B-C zero-bias junction capacitance (F)           | 0       |
| `pc`      | B-C built-in potential (V)                       | 0.75    |
| `mc`      | B-C junction grading coefficient                 | 0.33    |
| `cjcp`    | S-C zero-bias capacitance (F)                    | 0       |
| `tf`      | Ideal forward transit time (s)                   | 0       |
| `itf`     | High-current TF coefficient                      | 0       |
| `xtf`     | Bias-dependence coefficient of TF                | 0       |
| `vtf`     | Voltage for VBC dependence of TF (V)             | 1e99    |
| `td`      | Forward excess-phase delay (s)                   | 0       |
| `tr`      | Reverse transit time (s)                         | 0       |
| `avc1`    | B-C weak avalanche parameter 1                   | 0       |
| `avc2`    | B-C weak avalanche parameter 2                   | 0       |
| `isp`     | Parasitic substrate saturation current           | 0       |
| `rth`     | Thermal resistance (°C/W)                        | 0       |
| `cth`     | Thermal capacitance (J/°C)                       | 0       |
| `ea`      | Activation energy for IS (eV)                    | 1.12    |
| `kfn`     | Flicker noise coefficient                        | 0       |
| `afn`     | Flicker noise exponent                           | 1       |
| `tnom`    | Parameter measurement temperature (°C)           | 27      |

---

### HICUM Level 0 and Level 2 — High-Current Model for BJT

Physics-based bipolar model for RF and high-current operation. Two levels:
- `hicum0` — simplified high-speed model (level 0)
- `hicum2` — full model (level 2, industry standard)

Both are implemented via ADMS-compiled Verilog-A. Model type names:

```spice
.model Q1 hicum2 npn
+ is=...  (see HICUM2 documentation)
```

Source: `src/spicelib/devices/adms/hicum0/`, `adms/hicum2/hicum2.va`.
Parameters are numerous (>100); refer to the HICUM2 v2.24 documentation.

---

### BSIMSOI — BSIM Silicon-on-Insulator MOSFET

SOI MOSFET model from UC Berkeley. Supports partially-depleted (PD) and
fully-depleted (FD) operation.

**Device letter:** `M`. **Model type:** `bsimsoi`.

```spice
.model nmos1 bsimsoi nmos
+ version=4 tsi=10e-9 toxe=2e-9 ...
```

Key additional parameters over BSIM4:

| Parameter | Description                                       |
|-----------|---------------------------------------------------|
| `tsi`     | Silicon body thickness (m)                        |
| `tbox`    | Buried oxide thickness (m)                        |
| `tb`      | Body contact thickness (m)                        |
| `rbody`   | Body resistance (Ω)                               |
| `rbsh`    | Body sheet resistance (Ω/□)                       |
| `ntox`    | Nitride/oxide interface density                   |
| `fbjtii`  | Floating body BJT impact ionization factor        |

Source: `src/spicelib/devices/bsimsoi/`.

---

### SOI3 — Level 3 SOI MOSFET

Simpler SOI model. **Model type:** `soi3`.

```spice
.model n1 soi3 nmos
+ vt0=0.4 kp=100e-6 gamma=0.5 tox=5e-9 ...
```

Source: `src/spicelib/devices/soi3/`.

---

### HiSIM2 — Hiroshima-University STARC IGFET Model

Surface-potential-based MOSFET model from Hiroshima University.
**Model type:** `hisim2`.

```spice
.model n1 hisim2 nmos
+ version=2 type=1 tox=5e-9 ...
```

Source: `src/spicelib/devices/hisim2/`.

---

### HiSIMHV — HiSIM High-Voltage Model

Extension of HiSIM2 for high-voltage/power devices.
**Model type:** `hisimhv1`.

Source: `src/spicelib/devices/hisimhv1/`.

---

### MOS6 — MOSFET Level 6

Empirical MOSFET model with enhanced velocity saturation and body effects.
**Device letter:** `M`. **Model type:** `nmos` or `pmos` with `level=6`.

```spice
.model n6 nmos level=6
+ vto=0.8 kv=0.65 nv=0.5 kc=5e-5 nc=1
+ gamma=0.6 phi=0.65 lambda=0.02 tox=40e-9
```

**Model Parameters:**

| Parameter | Description                                  | Default |
|-----------|----------------------------------------------|---------|
| `vto`/`vt0` | Threshold voltage (V)                      | 0       |
| `kv`      | Saturation voltage factor                    | 2       |
| `nv`      | Threshold voltage coefficient                | 0.5     |
| `kc`      | Saturation current factor (A/V²)             | 5e-5    |
| `nc`      | Saturation current coefficient               | 1       |
| `nvth`    | Threshold voltage temperature exponent       | 0       |
| `ps`      | Saturation gate voltage factor               | 0       |
| `gamma`   | Bulk threshold parameter (V^0.5)             | 0       |
| `gamma1`  | Alternate bulk threshold                     | 0       |
| `sigma`   | Static feedback factor                       | 0       |
| `phi`     | Surface potential (V)                        | 0.6     |
| `lambda`  | Channel-length modulation (1/V)              | 0       |
| `lambda0` | Channel-length modulation, bias-independent  | 0       |
| `lambda1` | Channel-length modulation, bias-dependent    | 0       |
| `rd`      | Drain resistance (Ω)                         | 0       |
| `rs`      | Source resistance (Ω)                        | 0       |
| `tox`     | Oxide thickness (m)                          | 1e-7    |
| `u0`/`uo` | Surface mobility (cm²/V·s)                   | 600     |
| `fc`      | Forward bias depletion capacitance factor     | 0.5     |
| `tpg`     | Gate material type (1, -1, 0)                | 1       |
| `nsub`    | Substrate doping (1/cm³)                     | 0       |
| `nss`     | Surface state density                        | 0       |

---

### MOS9 — MOSFET Level 9 (Philips MM9)

Compact MOSFET model from Philips. **Model type:** `nmos`/`pmos` with `level=9`.

```spice
.model n9 nmos level=9
+ vfb=-0.3 phi=0.7 k0=1e-2 k1=0.3 ...
```

| Parameter | Description                     |
|-----------|---------------------------------|
| `vfb`     | Flat-band voltage (V)           |
| `phi`     | Strong inversion surface potential (V) |
| `k0`      | Body effect coefficient 0       |
| `k1`      | Body effect coefficient 1       |
| `tox`     | Oxide thickness (m)             |
| `et`      | Threshold voltage correction    |
| `mu0`     | Low-field mobility (m²/V·s)     |
| `mu0b`    | Body bias mobility coefficient  |

Source: `src/spicelib/devices/mos9/`.

---

### JFET2 — JFET Level 2 (Shichman-Hodges Enhanced)

Enhanced JFET model. **Device letter:** `J`. **Model type:** `njf`/`pjf` with `level=2`.

```spice
.model J1 njf level=2
+ vto=-2.0 beta=1e-4 ...
```

Source: `src/spicelib/devices/jfet2/`.

---

### HFET1 / HFET2 — Heterostructure FET Models

HEMT / GaAs FET models. **Device letter:** `Z` (same as MESFET).
**Model types:** `hfet1`, `hfet2`.

```spice
.model M1 hfet1
+ vto=-0.7 alpha=2 beta=0.04 delta=0.2 ...
```

Source: `src/spicelib/devices/hfet1/`, `hfet2/`.

---

### EKV — Enz-Krummenacher-Vittoz MOSFET

Charge-based MOSFET model for weak-to-strong inversion.
**Device letter:** `M`. **Model type:** `ekv`.
Compiled from `src/spicelib/devices/adms/ekv/`.

```spice
.model mn ekv nmos
+ vto=0.5 kp=100e-6 gamma=0.7 phi=0.7 ...
```

---

### PSP102 — PSP Surface-Potential MOSFET

Industry-standard compact MOSFET model (NXP/TU Delft).
**Model type:** `psp102`. Compiled from Verilog-A via ADMS.

Source: `src/spicelib/devices/adms/psp102/`.

---

### Mextram — Most EXact TRAnsistor Model

Advanced bipolar transistor model (NXP).
**Model type:** `mextram`. Compiled via ADMS.

Source: `src/spicelib/devices/adms/mextram/`.

---

### BSIM1 / BSIM2 — Early BSIM Models

Older BSIM versions for legacy netlist compatibility.

| Model | Type    | Level | Source directory        |
|-------|---------|-------|-------------------------|
| BSIM1 | `nmos`/`pmos` | `level=4` | `src/spicelib/devices/bsim1/` |
| BSIM2 | `nmos`/`pmos` | `level=5` | `src/spicelib/devices/bsim2/` |

Not recommended for new designs; use BSIM3v3 or BSIM4.

---

## Safe Operating Area (SOA) Checks

ngspice can check device operating points against SOA limits and warn when
they are exceeded. Enable with `.options SOA_LOG`:

```spice
.options SOA_LOG=soa_warnings.txt
```

Or via CLI: `ngspice --soa-log=soa.txt mycirc.spi`

SOA checks are defined in the model; relevant parameters include:

| Device | SOA Parameters                                        |
|--------|-------------------------------------------------------|
| MOSFET | `vgsmax`, `vgdmax`, `vdsmax`, `vbsmax`, `imax`, `pmax` |
| BJT    | `vceomax`, `vcemax`, `vbemax`, `icmax`, `ptotmax`     |
| Diode  | `vmax`, `imax`, `pmax`                                |

Set `write soa_log` path with `--soa-log=FILE` on the command line.
Source: `src/spicelib/devices/*/` (`*soachk.c` files).
