# ngspice-models — Idea (backlog, not planned)

**Status:** idea recorded only; no design, no tasks. Scheduled **well after** item 13 and
`transient-performance` (user, 2026-11-21: "coloca no backlog para bem depois: ngspice models.
Gostaria que na stdlib tenha os models iguais ao ngspice para podermos entrar fácil").

## Idea

The MSL standard library (`crates/piperine-lang/std/`) ships the ngspice device models, written
in MSL, so a SPICE user can move a netlist to Piperine without writing a model:

- the same model names and the same parameter names, defaults and units as ngspice
  (`.model d1 D(is=1e-14 n=1.05)` maps one-to-one onto a stdlib `Diode` with the same params);
- the same equations, so a twin circuit agrees with ngspice under the existing
  `ToleranceContract` (the parity harness, `piperine-parity`, proves each model);
- the passive and source elements (R, C, L, K, V/I sources with DC/AC/PULSE/SIN/PWL/EXP/SFFM,
  controlled sources E/F/G/H, switches S/W, the B source) first, then the semiconductors
  (diode, BJT Gummel-Poon, JFET, MOS levels 1–3, then BSIM3/BSIM4 and the rest as demand
  shows).

## Notes for when it is picked

- The PHDL-era `spice-stdlib`, `ngspice-bsim3` and `ngspice-bsim4` feature folders and the
  memories `spice-stdlib-migration`/`piperine-spice-lib` hold the earlier ngspice-faithful
  model work (Verilog-A era) and its known correctness traps (MOS1 current, JFET offset,
  BJT saturation; junction convergence through gmin stepping). Read them first.
- Fits the "no language special cases" rule: every model is an ordinary stdlib module.
- Open questions (to ask then): which models in the first wave; a `spice::` namespace or the
  top-level stdlib; whether a netlist importer (`.cir` → MSL) belongs here or in its own
  feature; temperature and noise parity per model.
