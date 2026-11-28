| ngspice device | Directory | Netlist form | MSL module | Instance params | Model params | C lines |
| --- | --- | --- | --- | --- | --- | --- |
| Resistor | `res` | element R; `.model … r` | `Resistor, SemiconductorResistor` | 13 | 16 | 1336 |
| Capacitor | `cap` | element C; `.model … c` | `Capacitor, SemiconductorCapacitor` | 11 | 14 | 1392 |
| Inductor | `ind` | elements L and K; `.model … l` | `Inductor, GeometricInductor, MutualInductor` | 9 | 12 | 2247 |
| Vsource | `vsrc` | element V | `VoltageSource` | 23 | 0 | 2526 |
| Isource | `isrc` | element I | `CurrentSource` | 18 | 0 | 1449 |
| ASRC | `asrc` | element B | `BehavioralVoltageSource, BehavioralCurrentSource` | 9 | 0 | 987 |
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
| Diode | `dio` | element D; `.model … d`, level 1 and level 3 | `Diode, GeometricDiode (+ `…SelfHeating` variants)` | 14 | 68 | 4531 |
| BJT | `bjt` | element Q; `.model … npn|pnp`, level 1 (and 0, 2) | `Bjt` | 10 | 123 | 8135 |
| VBIC | `vbic` | element Q; `.model … npn|pnp`, level 4 and 9 | `Vbic (+ VbicSelfHeating)` | 8 | 116 | 8325 |
| hicum2 | `hicum2` | element Q; `.model … npn|pnp`, level 8 | `Hicum2 (+ Hicum2SelfHeating)` | 9 | 139 | 8889 |
| JFET | `jfet` | element J; `.model … njf|pjf`, level 1 | `Jfet` | 8 | 23 | 3153 |
| JFET2 | `jfet2` | element J; `.model … njf|pjf`, level 2 | `Jfet2` | 8 | 1 | 1765 |
| MES | `mes` | element Z; `.model … nmf|pmf`, level 1 | `Mesfet` | 4 | 14 | 3246 |
| MESA | `mesa` | element Z; `.model … nmf|pmf`, levels 2, 3, 4 | `Mesa` | 8 | 61 | 2918 |
| HFET1 | `hfet1` | element Z; `.model … nhfet|phfet`, level 5 | `Hfet1` | 7 | 64 | 2650 |
| HFET2 | `hfet2` | element Z; `.model … nhfet|phfet`, level 6 | `Hfet2` | 7 | 36 | 1797 |
| Mos1 | `mos1` | element M; `.model … nmos|pmos`, level 1 | `Mos1` | 16 | 32 | 6780 |
| Mos2 | `mos2` | element M; level 2 | `Mos2` | 16 | 39 | 8137 |
| Mos3 | `mos3` | element M; level 3 | `Mos3` | 16 | 45 | 7564 |
| Mos6 | `mos6` | element M; level 6 | `Mos6` | 16 | 37 | 2847 |
| Mos9 | `mos9` | element M; level 9 | `Mos9` | 16 | 43 | 7478 |
| VDMOS | `vdmos` | element M; `.model … vdmos|vdmosn|vdmosp` | `Vdmos (+ VdmosSelfHeating)` | 8 | 64 | 4759 |
| BSIM1 | `bsim1` | element M; level 4 | `Bsim1` | 14 | 79 | 6833 |
| BSIM2 | `bsim2` | element M; level 5 | `Bsim2` | 14 | 135 | 4649 |
| BSIM3 | `bsim3` | element M; level 8 and 49, `version` 3.3.0 | `Bsim3` | 19 | 423 | 11546 |
| BSIM3v32 | `bsim3v32` | element M; level 8 and 49, `version` 3.2 to 3.2.4 | `Bsim3V32` | 15 | 422 | 11946 |
| BSIM3v1 | `bsim3v1` | element M; level 8 and 49, `version` 3.1 | `Bsim3V1` | 12 | 361 | 8842 |
| BSIM3v0 | `bsim3v0` | element M; level 8 and 49, `version` 3.0 | `Bsim3V0` | 12 | 348 | 7774 |
| BSIM4 | `bsim4` | element M; level 14 and 54, `version` 4.8 | `Bsim4` | 43 | 891 | 22990 |
| BSIM4v7 | `bsim4v7` | element M; level 14 and 54, `version` 4.7 | `Bsim4V7` | 37 | 886 | 22725 |
| BSIM4v6 | `bsim4v6` | element M; level 14 and 54, `version` 4.6 | `Bsim4V6` | 36 | 830 | 21687 |
| BSIM4v5 | `bsim4v5` | element M; level 14 and 54, `version` 4.5 | `Bsim4V5` | 36 | 752 | 20204 |
| B3SOIFD | `bsim3soi_fd` | element M; `.model … nmos|pmos`, level 55 | `Bsim3SoiFd` | 16 | 389 | 11524 |
| B3SOIDD | `bsim3soi_dd` | element M; level 56 | `Bsim3SoiDd` | 16 | 389 | 12491 |
| B3SOIPD | `bsim3soi_pd` | element M; level 57 | `Bsim3SoiPd` | 25 | 467 | 13178 |
| B4SOI | `bsimsoi` | element M; level 10 and 58 | `Bsim4Soi` | 37 | 911 | 27924 |
| SOI3 | `soi3` | element M; level 60 | `Soi3` | 25 | 83 | 6819 |
| HiSIM2 | `hisim2` | element M; level 68 | `Hisim2` | 32 | 544 | 18355 |
| HiSIMHV1 | `hisimhv1` | element M; level 73, `version` 1.x | `HisimHv1` | 38 | 608 | 18052 |
| HiSIMHV2 | `hisimhv2` | element M; level 73, `version` 2.x | `HisimHv2` | 38 | 728 | 20326 |
| NUMD | `numd` | `.model … numd` level 1 | `—` | 5 | 1 | 1625 |
| NUMD2 | `numd2` | `.model … numd` level 2 | `—` | 6 | 0 | 1669 |
| NBJT | `nbjt` | `.model … nbjt` level 1 | `—` | 5 | 0 | 1955 |
| NBJT2 | `nbjt2` | `.model … nbjt` level 2 | `—` | 6 | 0 | 1950 |
| NUMOS | `numos` | `.model … numos` | `—` | 7 | 0 | 2367 |
| NDEV | `ndev` | `.model … ndev` | `—` | 0 | 2 | 637 |

In scope: instance 757, model 10232, total 10989
