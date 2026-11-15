# Changelog v0 → v1

Against `critique_v0.md`. Verdict per item: **fixed**, **fixed differently**, **partly**, or
**rejected** with the reason.

## The structural moves (most items fall out of these)

| Move | Replaces | Items it answers |
| --- | --- | --- |
| `analog` + `define` → one **Function** (pure, declared reads/writes/partials), several per instance, run "when an output is needed and an input changed" | `analog`, `define`, `Slot`, `Definition`, `want`, `analog_reads` | M10, M9, B3, M5 (static args), M12, m6, m7 |
| `Slot` (9 kinds, `v/a/b`, overloaded `plus`) → four typed analog cells: **Term**, **Source**, **Limit**, **Watch**, with named parts | `Slot`, `SlotOut`, `Call.last` | M5, M6 |
| `Env` + role `Global` → role **Session** (declared, by name, never an event) | `Env`, `Global` | M4, B2 (tick resolution, horizon, start) |
| `mem` → a private discrete pin written by commit | `Mem`, `mem_size/align` | M14, B1 (everything is rollbackable by the owner) |
| `emit` + `Site` → pin kind **Stream**; a record is a commit to it | `emit`, `Site`, `Field`-in-site | entry point −1, M3 |
| `lane` → a declared **Statement** of the reaction (index = source order; `LATER`/`REPLACE` static) | `lane`, `WRITES`, `Reaction.writes: List<u32>` | B2.2, m2, m4 |
| `tag` strings → **Layout** (size, align, fields over closed leaves, optional known byte) | `tag`, `Arg`, `ArgValue`, `Site`, `Field` | M3, M7, M8 |
| `Arg` → `PARAM` pins with `STRUCTURAL`; `create(values by name)`; status `RESHAPE` | `Arg`, `ArgValue`, `Module.args` | M7, B3 |

Entry points 8 → 6 (`define` and `emit` gone). Concepts, counted the critic's way, 16 → 13.

## BLOCKER

**B1 — R2 unsound. Fixed.** R2 as a global promise is deleted. §5 now states the
synchronization protocol normatively: the owner checkpoints its own discrete store, queue and
record buffer at each accepted analog time, runs discrete instants speculatively, and
restores before every analog retake, so a crossing found by a retake is applied before
anything later. This costs no entry point because reactions have no memory (state is a
private pin). Foreign engines declare `IRREVERSIBLE` on the reaction; the owner makes each of
their instants an accepted analog time first.
*Rejected sub-point:* "any commit an irreversible reaction makes that can reach
`analog_reads` must have `after > 0`". Not needed: under the kept discontinuity algorithm
(context Q7) nothing is undone after an analog time is accepted; a zero-delay commit there is
a boundary event at an accepted time and restarts the *next* step.

**B2 — AVR has no time base. Fixed.**
1. *Horizon*: session pin `horizon` (R5), with its exact definition, including the exclusion
   of consequences of the call's own commits and the tentative/exact distinction.
2. *Every wake a time point*: statements carry `LATER`; R4 tests only statements that can
   apply at this instant. The due instant of a delayed pad write is what gets tested.
3. *Never starts*: session event `start`. (`at(param)` uses the same event.)
4. *Tick resolution*: session pins `tick`, `resolution`.
The example (§11.7) is rewritten: state is a pin, one reaction runs to
min(horizon, first own output), a discrete function turns pad changes into `irq`.
Honest residue, listed as weak point 3: an `IRREVERSIBLE` emulator reachable from a crossing
gets `horizon = now`.

**B3 — OSDI-like model. Fixed.** §11.8. Params are pins (one struct pin per model card and
per instance, so 900 params are one pin and one shared net); `setup` is a discrete function
writing a private cache pin, run only when a param or the temperature changes; value-dependent
collapse is `RESHAPE` (the adapter cannot know statically which params are structural);
`STRUCTURAL` exists for the cases that are known. Gradients: discrete functions declare
partials (F4), so an MSL model's derived values are exact; a foreign opaque cache has none
and the host uses a finite difference — stated in §10 instead of pretended away.

## MAJOR

- **M1 — fixed, the strict way.** Reactions listen to event pins only (R1). `changed()` and
  edges are discrete functions reading `PREVIOUS`/`CHANGED` parts. §11.4 now follows it.
- **M2 — fixed differently.** A resolver is an *instance* generated from the nature (drive
  pins in instance-path order, value pin, "others" pins); an external device joins by
  ordinary binding and declares nothing. Cross-instance loops are solver-visible cycles and
  D1 gives the solver one generic rule on bytes and fields (start at zero, pins + 1 passes,
  zero the fields that still move, log). The solver learns "zero bytes mean nothing is held"
  (P2), not the MSL rule. Listed as weak point 6.
- **M3 — fixed.** One `Layout` for pins, params, state, records, watch values. Closed leaf
  set, optional known byte per field, equality is structural. No reference to MSL Part I.
- **M4 — fixed, option (a).** Everything ambient is a `SESSION` pin with one rule ("rewritten
  between calls; its change is never an event"). `Env` is gone, so is `iter`. The solver
  knows which functions read `freq`/`time`/`temperature`.
- **M5 — fixed in substance, partly in count.** Terms have exactly three operators; sources,
  limits and watches are separate tables; `a`/`b` are named parts; static arguments live in
  param-only functions; `plus` is always a row; A2 (switch branches) is deleted.
  *Not reduced further:* watch keeps three kinds (`VAR`, `CROSS`, `CAP`) — each is one row
  of the solver's table and removing any loses a device.
- **M6 — fixed.** Limit is a solver-held cell read back as an input; the solver forms the
  chain product and the right-hand-side correction (A2). `Env.iter` → the `AGE` part. The
  not-evaluated case is stated (`LIMITED = PROPOSED = LAST`). The FD verifier is valid with
  limiters active because `x̂` is an input.
- **M7 — fixed.** One namespace: `PARAM` pins; `create` takes their initial values by name;
  introspection reads `Shape.pins` only.
- **M8 — fixed.** Shapes are interned and shared; struct param pins; model-card nets shared
  by binding (P4, §9).
- **M9 — partly.** Partials on every function, analog and discrete; `seed` selects inputs
  per read, not classes; real fields of any layout are differentiable (`to`/`at` byte
  offsets), which covers `Bit<f64>`. The coverage table no longer claims more.
  *Rejected for v1:* partials on reactions (§23.3 4a, saltation). The solver-completeness
  spec puts trajectory sensitivities out of scope (rows 67–68, "Deferred"); the extension is
  one list on `Reaction` and a minor version. Weak point 5.
- **M10 — fixed.** One Function concept, one `eval`.
- **M11 — fixed.** `u32` constants, opaque zero-sized `Instance`, `Pod` bound in the lens,
  alignment kept on every copy, `Option` for optional pins.
- **M12 — fixed.** A4 applies to cells the solve reads; strengths are written by functions
  the solver needs only in the sweep; the flicker function is not run at `freq == 0`.
- **M13 — fixed.** `force`/`release` are stated as the owner's overlay on a net (P6), outside
  the one-writer rule and invisible to devices; a forced node is the owner's own potential
  source. `where` is found through `Pin.check` (P5).
- **M14 — fixed.** No shared `mem` (R3); records are buffered per call and merged in a stated
  order (C5); the contradiction on dropped records is resolved by B1 (a reaction's record is
  dropped when its instant is undone; a trace never).

## MINOR

- **m1 (sentinels) — mostly.** Gone: `iter == MAX`, `tick == MIN`, `a = NaN`, zeroed `mem`,
  empty `init` (now `HAS_INIT`), `NONE`. Kept: `GROUND` as a row target (it names the
  reference, it is not "none"); `abstol = 0` for "session default" (0 is not a legal
  tolerance); `CAP = +inf` (it is the identity of `min`, not a flag).
- **m2 (redundancy) — fixed.** `Shape.ports`, `WRITES`, `analog_reads` deleted; writers are
  derived (P1). An event pin still has several possible *kinds* of writer but exactly one
  writer.
- **m3 (union frame) — fixed.** Two frames, every pointer non-null.
- **m4 (MSL error numbers, magic lanes) — fixed.** `Status` has three values; the code in the
  note is the device's and opaque to the solver; statements are indexed, not numbered by hand.
- **m5 (provenance) — fixed.** `origin` on every entry; `VAR` watches have a layout.
- **m6 (`want` cannot select) — fixed** by function granularity and per-read `seed`.
- **m7 (digital cost) — partly.** F3: a library may export a fused function; granularity is
  the library's. No fusion *across* libraries is specified.

## Deleted from v0

`define`, `emit`, `Env`, `Call`, `Slot`, `SlotOut`, `Definition`, `Site`, `Arg`, `ArgValue`,
`tag`, `lane`, `want`, `last`, `mem`, `Shape.ports`, `analog_reads`, role `Global`, flag
`WRITES`, rules A2, A7, R2 (as a promise), D3 (as a convention).

## Added in v1

`Layout`/`Field`, `Cell`, `Statement`, `Term`/`Source`/`Limit`/`Watch` tables, pin kind
`STREAM`, role `SESSION` and its name table, flags `STRUCTURAL`, `HAS_INIT`, `HAS_CHECK`,
`LATER`, `IRREVERSIBLE`, status `RESHAPE`, `origin`, the synchronization protocol, rule D1.

## Counts

| | v0 claimed | v0 by the critic | v1 (critic's method) |
| --- | --- | --- | --- |
| concepts | 10 | 16 | 13 |
| entry points | 8 | 8 | 6 |
