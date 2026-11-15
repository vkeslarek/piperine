# Changelog v1 → v2

Against `critique_v1.md`. The instruction for this round was: merge, do not add; give
`commit` one contract.

## What was merged or removed

| v1 | v2 | Why |
| --- | --- | --- |
| `Source`, `Limit`, role `SESSION` (per-call values) — three concepts | **Input**: "a real value the solver supplies to a call; not a net" (kinds `LIMIT`, `NOISE`, `STIM`, `TIME`, `FREQ`, `STATIC`, `GMIN`, `SOURCE_SCALE`, `TICK`, `HORIZON`) | the critic's §5 merge, extended to the per-call ambient values N2 said were call arguments, not pins |
| role `SESSION` (slow values, events) | role `GLOBAL`: real design-wide nets with a `u32` constant; they obey every pin rule | N2 |
| pin kind `STREAM`; record and trace paths of `commit`; flag `FINISH` | a record is a commit to a `RECORD` pin; a trace is a `TRACE` watch; `FINISH` is a `Status` | N9 |
| parts `ABSTOL`, `MAX`, `TTOL`, `ETOL` | pin references on `Term` / `Watch` | the critic's optional proposal; 17 parts → 12 |
| leaf `TEXT` | `U8[count]` | m-a |
| `Eval.host` | gone: functions have no effect channel at all | N9 |
| A2 + A3 (limit; sources) | one rule A2 "inputs with a perturbation" | §5 of the critique |

Concepts (critic's method, Cell and Partial counted): 15 → **13**. Entry points: 6, and
`commit` now has exactly one contract (queue a write of the calling reaction).

## MAJOR

- **N1 — fixed.** `Module.prototype` is the shape at default values: it lists `PARAM` pins,
  layouts and defaults, so the host can encode `Value`s (now addressed by pin index) and the
  elaborator can check an `extern` declaration without creating anything. A shape depends on
  structure only: it holds defaults; the solver seeds each param net from the values it
  passed (P5). Two resistors of different value share one shape.
- **N2 — fixed.** Per-call values are inputs (never events, never read by a discrete
  function, so the `time`-reading definition is a bind error). Values that exist at instants
  are `GLOBAL` nets under P1–P4; a rewrite between analyses or sweep points changes at
  microstep 0 of the next point (P2), which answers the `setup`/`cache` example. Reserved
  names are `u32` constants; a host session parameter is `NAMED` and cannot collide.
- **N3 — fixed.** The protocol runs the discrete instants first (they cannot read analog),
  finds the landing, and solves once; a restore replays discrete work only (§5).
- **N4 — fixed.** A landing forced by an `IRREVERSIBLE` reaction is not a time point: no
  settle phase unless R4 makes it one. R6 also states that such a session cannot be rewound
  and that every instant of the reaction costs a landing in any design with a crossing.
- **N5 — fixed.** A3 states the allowed one-call form (limit and terms written together, no
  partial against the limit, values linearized at `x̂` and evaluated at `x`); the verifier
  checks where the limit is inactive (§8). §11.8 is rewritten to that form, with the model
  setup in its own instance writing a shared net, and the mutable instance data kept as
  scratch refreshed by hash, so a refused shadow `set` corrupts nothing.
- **N6 — fixed.** `TICK` is the last tick not after the instant; `HORIZON` is the last tick
  through which nothing can change. Both are defined at a time point that is not a tick.
- **N7 — fixed.** Pin flag `RELAX`, set by the resolver on its drive pins. A cycle through no
  `RELAX` net is a bind error (the cross-coupled gates). On non-convergence a field with a
  known byte is zeroed; a field without one is a fault.
- **N8 — fixed.** Functions are classified by what they write (F2); F3 says when a
  param-only analog function runs. A current source is in §11.1.
- **N9 — fixed, the critic's second option.** `STREAM` is gone. A record is a commit to a
  discrete `RECORD` pin and follows C1–C4 unchanged; the sink (which sees every commit
  anyway) hands it to the host with the instant it was made; it vanishes with an undone
  instant for free. Traces come out of `eval` as a `TRACE` watch, so no function calls
  anything. The kind × role matrix is in §3.1 and checked at bind.

## MINOR

- **m-a** fixed (`TEXT` removed). **m-b** fixed (canonical form checked in debug sessions;
  `BIT`, `REAL`, `BYTE` shipped as constants). **m-c** fixed (`seed` is per partial).
  **m-d** fixed (`Host.size`; weak point 4 says what will be appended). **m-e** fixed
  (`PREVIOUS`/`CHANGED` only in functions that write event pins only, and in reactions).
  **m-f** fixed (`QUIET`: no previous value, no change detection, never prolongs an instant).
  **m-g** fixed (a shape lives until its last instance is destroyed; only prototypes are
  permanent). **m-h** fixed (`RESHAPE` on a shadow refuses the write; it cannot arise in a
  run otherwise). **m-i** fixed (a reaction reads pins and four inputs).

## Rejected / not done

- **Removing "part" entirely** (§5, last row, marked optional by the critic): four static
  parts moved to pin references, but `DELAY.TIME` may be a continuous expression and the
  limit's `PROPOSED/LIMITED/LAST/AGE` are intrinsic, so `part` stays, with 12 values.
- **Counting `commit` as 6 + 1** (N9, first option): not needed, since the record and trace
  contracts were removed rather than kept.
- Still deferred, as accepted last round: partials on reactions (§14.4 states the condition
  the critic attached: a commit that also carries tangent bytes, hence `Host.size`).

## Added in v2 (each replaces something larger)

`Module.prototype`, `Pin.global`, flags `QUIET`/`RECORD`/`RELAX`, watch kind `TRACE`, status
`FINISH`, `Host.size`, input table. No new concept, no new entry point.

## Counts

| | v0 (critic) | v1 (critic) | v2 (critic's method) |
| --- | --- | --- | --- |
| concepts | 16 | 15 | 13 |
| entry points | 8 | 6, `commit` × 3 contracts | 6, one contract each |
