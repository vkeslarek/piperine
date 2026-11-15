# Changelog v2 → v3

Against `critique_v2.md`. Tight edits only: no concept and no entry point added or removed.
Counts stay 13 concepts / 6 entry points (one contract each).

## MAJOR

- **M1 — fixed.** New rule **R7 "the host is a reaction"**: a host waiter on an event is an
  `IRREVERSIBLE` reaction on that event pin whose statements are the nets the host writes; a
  host probe at a scheduled time is one that reads a continuous pin (R4 makes it a time
  point). The host is called once, at a final instant; its answer is a commit due at the next
  microstep. R5 says nets the host may write in answer count for `HORIZON`. The protocol (§5)
  gains the `FINISH` branch: a speculative reaction returning `FINISH` at `s` → restore,
  replay up to `s`, land and accept at `s`, run the instant, end; step 2 names host waiters
  among the things it stops before.
- **M2 — fixed.** Only `CHANGED` is restricted (reactions, and functions that write event
  pins only). `PREVIOUS` may be read by any function or reaction: it changes exactly when
  `VALUE` does. §4.1 and the bind validation list updated.
- **M3 — fixed.** P6 is now "every host write is checked on a shadow first": the transitive
  closure of downstream discrete functions, across instances, for `PARAM`, `GLOBAL` and
  sweep-point writes alike. On `RESHAPE`: refused inside a transient; at a sweep point or
  between analyses the owner re-creates and re-binds, then commits (legal because the device
  has no memory and nets keep their values). The false sentence in §9 is replaced; §11.8
  walks the model-card case.

## MINOR

- **m1** fixed (P5: a "not given" param is a field with known byte 0; the device's setup
  resolves its default). **m2** fixed (P5: `PARAM` pins come first in every shape of a
  module, in prototype order). **m3** fixed (C5/P7: the stream is `<instance path>.<pin
  name>`; global names are the host's convention; delivery is at make-time, so transport
  removal loses no record). **m4** fixed (Watch is "an analog output that enters no row";
  §4.3 prints the watches as their own table). **m5** fixed (Input printed as two families;
  ambient inputs have no shape entry and are addressed by constant through cell kind
  `AMBIENT`; "real" and "of the analog solver" dropped). **m6** fixed (`Function.flags`,
  `OWNS_CORRECTION`; writing a limit and a term in one function without it is a bind error).
  **m8** fixed (P5: binding a `GLOBAL` before the run is not a change; an analysis at a
  transient's last instant flips the analysis pins without firing). **m9** fixed (P7: a
  `QUIET` commit no microstep picks up applies as the instant exits).
- **m7 — not changed, acknowledged** (weak point 7). `Pin` stays one struct; splitting it per
  kind × role would add a table per role for fields that are plain data.

## Additions to the structs (data only)

`Function.flags` (`OWNS_CORRECTION`); cell kind `AMBIENT` with constants `TIME`, `FREQ`,
`STATIC`, `GMIN`, `SOURCE_SCALE`, `TICK`, `HORIZON` (moved out of `Shape.inputs`, which now
lists held inputs only). Nothing else.

## Spec sentences the ABI needs amended

The ABI is implementable against the language specification as written except where listed.
Paths are in `docs/spec/` of the codegen-plan repo. None has been edited; this is the list to
approve.

| # | Where | Today | Needed | Why |
| - | ----- | ----- | ------ | --- |
| 1 | `part_v_execution.md` §17.2 rule 2, second bullet | "a reaction that counts there probes, in its guard or body, or can change a carrier that `analog` or a probing reaction reads" | "… or can change, **at that instant**, a carrier that `analog` or a probing reaction reads. A commit whose delay always advances time changes nothing at the instant that makes it; its due time is a scheduled time tested on its own." | `LATER` (R4). Without it every wake of a delayed driver (a clock generator into a `D2A`, an emulator) is a time point. Observable through `settle`. |
| 2 | `part_v_execution.md` §15, definition of *time point* | "A **time point** is a time at which the analog solver produces an accepted solution." | "… produces an accepted solution **and runs the time-point procedure**. A simulator MAY accept solutions at other times (to hand a final instant to a co-simulated module or to the host); those are not time points and `settle` does not fire there." | R6/R7: a landing forced by an irreversible reaction or a host waiter must not change the set `settle` observes. |
| 3 | `part_iii_structure.md` §8.5.8 and the `"cosim"` row of the contract table; `appendix_g_scope.md` G.65 | "The ABI string is `"osdi"` or `"cosim"`. The set is closed." / "It never probes. It is never advanced into a step that can be rejected, so it needs no rollback." / G.65: the C interface is not specified | The C interface of an extern module is the device ABI. `"osdi"` names the adapter. A `"cosim"` module is a module whose reaction is irreversible; whether it may read continuous pins follows the ABI (it may, at the price of a time point). §8.5.11's port restriction becomes a property of the adapter, not of the language. | closes G.65; the AVR of §11.7 has an analog pin and is reversible. |
| 4 | `part_iii_structure.md` §8.5.10, `part_viii_host.md` §23.1 rule 7 (`E0803`) | a `set` of a *structural* param during a run is refused | add: "a `set` that would change the structure of an extern module (its foreign descriptor decides) is refused the same way; in a `Dc` sweep such a point re-elaborates that instance." | `RESHAPE` and P6: foreign models do not mark which params are structural. |
| 5 | `part_vi_io.md` §19 rule 3, and `part_iv_behavior.md` §11 rule 12 | `emit!` in `analog` runs "at every evaluation"; a loop that contains a contribution or a placement must have a constant trip count | add `emit!` to the constructs that require a constant trip count in an `analog` loop; one record per site per evaluation. | a `TRACE` watch is a fixed output cell (A5). |
| 6 | `part_i_values.md` §2 (`Repr`) and `part_ii_carriers.md` §5.3.4–§5.3.6 | the layout of a value and its known flags is not fixed | state the canonical layout: one known byte per level, 0 = unknown; an unknown level's value bytes are zero; all-zero bytes are the all-unknown value; a drive that is "not driving" (§8.2.3 step 1) is all zero. | P3: the solver compares bytes and starts storage at zero; an external device must be able to produce an MSL `Bit`. |
| 7 | `part_iii_structure.md` §8.2.3 step 4 | "if drives still move after that, the levels that move are unknown" | add: "a level of a type with no unknown (an always-known real) that still moves is a run error." | D1: the solver can zero a known byte; it cannot invent "unknown" for a field that has none. |
| 8 | `part_iv_behavior.md` §14.4.6 | "This specification names no session parameter of `simparam`." | reserve `simparam("gmin")` and `simparam("source_scale")` (or give them core functions): they are the continuation variables, constant in the last solve, and not events. | stdlib sources must scale under source stepping; in the ABI these are ambient inputs, not nets. |
| 9 | `part_vi_io.md` §19 rule 1, second resolution ("A record from the settle phase carries … the index of its last microstep") | records carry `(t, n)` | no change of text; note that the reference implementation carries records as commits to quiet pins, which never add a microstep. | confirms `QUIET` keeps `n` as the spec counts it. |

Already decided elsewhere and assumed here: Appendix C.2 and §15.2 (the jump) are amended to
the solver's present discontinuity algorithm (solver-completeness Q7); §17.1.2 keeps the
present tolerance mapping (Q9).

## Counts

| | v0 (critic) | v1 (critic) | v2 (accepted) | v3 |
| --- | --- | --- | --- | --- |
| concepts | 16 | 15 | 13 | 13 |
| entry points | 8 | 6 (`commit` × 3) | 6 | 6 |
