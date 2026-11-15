# Critique of `abi_v3.md`

Tight round, tight result. The three v2 MAJORs are closed. One new MAJOR comes from the fix of
M3 (P6 is now too wide, and still leaves one path open); the rest is precision.
The larger risk has moved out of the ABI and into the list of spec amendments: four of the
nine are not minimal as proposed. 0 BLOCKER, 1 MAJOR, 7 MINOR.

---

## 1. Verification of v2 items

| v2 | Verdict | Note |
| --- | --- | --- |
| M1 host waiter, `FINISH` | **CLOSED** | R7 is the right sentence: the host is an irreversible reaction, called once at a final instant. `FINISH` branch is sound (the reaction is reversible, re-run at step 5). Residue: m1, m2. |
| M2 `PREVIOUS` over-restricted | **CLOSED** | Only `CHANGED` restricted. |
| M3 `RESHAPE` paths, transitive shadow | **CLOSED for host writes, widened too far** → N1 | |
| m1 given / derived defaults | CLOSED | known byte 0 = not given. |
| m2 pin indices | CLOSED | `PARAM` pins first, prototype order. |
| m3 stream identity, make-time delivery | CLOSED | |
| m4 watch sentence | CLOSED | "an analog output that enters no row". |
| m5 Input two families | CLOSED | Held (shape entries) / ambient (constants, cell kind `AMBIENT`). The split is now in the data, not only in the table. |
| m6 two limit forms | CLOSED | `OWNS_CORRECTION`, both mismatches are bind errors. |
| m7 fat `Pin` | accepted as weak point 7 | Agreed: plain data, one struct. |
| m8, m9 | CLOSED | |

---

## 2. MAJOR

### N1. P6 "every host write is checked on a shadow first" is both too wide and not wide enough

**Too wide.** "Any net": a testbench `sim.drive("clk", …)` or `sim.clock(...)` is a host write.
Its transitive downstream closure of discrete functions is the combinational logic of the
whole design. As written, every host-driven edge evaluates the design twice (shadow, then
committed), for a check that can only matter if a `check` pin or a `RESHAPE`-capable function
is downstream — and nothing in the shape says which functions can return `RESHAPE`, so the
owner cannot prune. Weak point 8 prices only the model-card case.

**Not wide enough.** "So no `eval` of a committed store ever returns `RESHAPE`" holds for host
writes only:

```
Osdi-like device whose `inst` data arrives on a PORT discrete net (a trim word from a register):
   reaction commits trim <- 0x3   ->  setup re-runs  ->  collapse differs  ->  RESHAPE
   from a committed store, mid-transient. No host write, no shadow.
```

Both ends close with one bind-time rule and no new data: **the parameter cone**. A function may
return `RESHAPE`, and a `check` pin may exist, only inside the set of discrete functions whose
inputs derive — through functions only — from `PARAM` and `GLOBAL` nets (derivable from the
shapes; a function with any input reachable from a statement or a `PORT` the netlist binds to
a written net is outside). Then: P6 shadows a host write only when it lands on a `PARAM` or
`GLOBAL` net, over the cone only; a host drive of a port is an ordinary commit; `RESHAPE` from
outside the cone is `FAULT`. The sentence "no `eval` of a committed store returns `RESHAPE`"
becomes true.

---

## 3. MINOR

- **m1. Partial instants in step 2.** "Stopping BEFORE the first instant `s` that R4 makes a
  time point or that fires an `IRREVERSIBLE` reaction": R4 is tested "after its commits and
  functions are applied", and a firing in microstep `n > 0` is known only after microsteps
  `0 … n−1` of `s` ran. So `s` has been partly executed when the protocol says it was not
  entered. State it: the owner sub-checkpoints at the start of each instant and restores it
  when the instant turns out to be a landing (step 5 then runs `s` whole).
- **m2. The `final` mark must be dropped** when step 3 lands earlier (`t' < s`); as written
  step 5 would end the analysis at `t'`.
- **m3. `LATER` and tick rounding.** C1: a positive delay that rounds to no advance is the next
  microstep (T3); "the solver refuses a commit that breaks the promise". Whether `after 1n`
  advances depends on the session's resolution, which no library knows when it is compiled. So
  a correct device faults under a coarse resolution. Say what "refuses" is (a `FAULT` naming
  the statement) and that a generator sets `LATER` only where the delay is proved `≥` one tick
  at bind (the shape is checked against `RESOLUTION` there), else leaves the flag off.
- **m4. R7's cost is unstated for the common testbench.** Every firing of an event a host
  waiter listens to is a landing "in any design with a `CROSS` watch" (R6). A testbench that
  awaits every `clk.rise()` turns each clock edge into an analog solve. That is the correct
  price of irreversibility; the document should say it next to R7, because it is the first
  thing a user will hit.
- **m5. A function that both reads a held limit's `VALUE` and has `OWNS_CORRECTION` for another
  limit** (a foreign model wrapped next to an MSL `limit`): the flag is per function, the two
  forms are per limit. Either forbid the mix or say the flag covers exactly the limits the
  function writes (the bind text implies the latter; say it in A3).
- **m6. `TRACE` all-zero = "not written"** is the one sentinel left in a data cell. A record
  layout already has known bytes; define "not written" as "every known byte 0" (same bytes,
  stated through P3 instead of as a special case).
- **m7. Weak point 2 says "six sentences", the changelog lists nine.**

---

## 4. The nine proposed spec amendments

| # | Subject | Necessary? | Minimal? | Risk | Verdict |
| - | ------- | ---------- | -------- | ---- | ------- |
| 1 | §17.2 rule 2: "at that instant" (`LATER`) | **Yes.** Without it every wake of a delayed driver is a time point; the AVR and every clock generator into a `D2A` pay an analog solve per wake. | Nearly. "A commit whose delay always advances time" must read "advances by at least one tick" (m3; T3). | **Medium, observable** (the set `settle` sees shrinks). It changes what an MSL author's `on settle` monitor counts. | **Approve, with the tick wording. User decision** — it is a semantic change of the language, the only one here with observable weight. |
| 2 | §15: a time point "runs the time-point procedure"; other accepted solutions are not time points | **Yes**, and independently of cosim: §23.1 host waiters already need it. | Yes. | Low. Extra accepted solutions change the step sequence within tolerance, not the semantics. Add one clause: instance state (`ddt`, `delay`, `limit` history) advances on *every* accepted solution, time point or not (§17.2.4 says "accepted steps", so it already holds). | **Approve.** |
| 3 | §8.5: the extern C interface is the device ABI; cosim may read continuous pins; port restriction moves to the adapter | Half. Closing G.65 with "the interface is the device ABI" is necessary. Letting `"cosim"` probe and lifting §8.5.11 is a *language* scope change the ABI does not need. | **No.** Split it. And it is incomplete: §8.5.16 (an unknown input → the foreign step is not run, every `out` commits `Unknown`) and §8.5.15 (analog pins sit behind `A2D`/`D2A`) contradict the new text and are not listed. | Medium: it rewrites the contract table of a closed set. | **3a approve** (G.65 := the device ABI; `"cosim"` = an irreversible reaction; `"osdi"` = the adapter of §11.8). **3b defer to the user** (continuous ports on cosim, §8.5.11, §8.5.15, §8.5.16). |
| 4 | §8.5.10 / E0803: a `set` that changes an extern's structure is refused; a `Dc` point re-elaborates the instance | **Yes** (`RESHAPE`, foreign models do not mark structural params). | Yes. | Low. | **Approve.** |
| 5 | §19 r3 / §11 r12: `emit!` in an `analog` loop needs a constant trip count | **No.** It restricts the language to fit a cell: the direction the brief forbids ("nothing the language decides leaks into the ABI" — here the ABI leaks into the language). | No. A `TRACE` layout with an array field and a count (fields already have `count`) carries a bounded loop; the bound is the elaborator's. | Low in practice, wrong in principle. | **Reject as proposed.** Keep `emit!` as it is; make the trace cell an array with a declared maximum, a run error (or truncation with a flag field) beyond it. |
| 6 | Part I §2 / §5.3.4–6: fix the canonical layout (one known byte per level, zero = unknown) | Partly. Needed normatively: *all-zero bytes are the all-unknown value* and *an unknown level's value bytes are zero* (bytewise comparison, zero-start storage). **Not** needed in the language: where the known flags live. That is exactly what `Layout` prints. | **No.** "One known byte per level" freezes an implementation choice in Part I (a `[Bit; 64]` becomes 128 bytes forever; C.1.2 speaks of a mask word and elision). | **Medium-high**: an ABI detail becomes a language guarantee. | **Approve the two sentences about zero; reject the byte-per-level sentence.** If the ABI wants byte-per-element it says so in `Field.known` (it already does); the language stays silent. |
| 7 | §8.2.3 step 4: a type with no unknown that still moves is a run error | **No.** Every MSL discrete carrier level can be unknown (§16 step 1: "every level of every discrete carrier is unknown"); a field without a known byte can only come from an external device's layout. | — | None, but it adds a sentence about a case the language cannot produce. | **Reject.** It is D1's own rule; keep it in the ABI. |
| 8 | §14.4.6: reserve `simparam("gmin")`, `simparam("source_scale")` | Something is necessary: stdlib sources must scale under source stepping, junction models read gmin. | **No, and as spelled it is wrong**: `simparam` is in the class "constant during an analysis" and "forms discrete expressions" (§14.4.1); continuation variables change between solves of one instant and the ABI itself makes them ambient inputs no discrete function may read. They must be two core functions of the class "environment of the solve" (with `now()`, `freq()`), or not exist in the language at all. | Medium: two new core names, or a contradiction inside §14.4. | **Rework. User decision** between (a) two core environment functions `gmin()`, `source_scale()`; (b) no language change: independent sources carry a term flag the solver scales, and gmin is the solver's shunt only (ngspice parity for junction gmin argues for (a)). |
| 9 | §19 rule 1: no text change, a note on `QUIET` | — | Yes. | None. | **Fine**; it belongs in Appendix C, not in the amendment list. |

Summary for the orchestrator: approve 2, 4, 3a, 9; approve 1 with the tick wording (user);
trim 6 to two sentences; rework 8 (user); reject 5 and 7; 3b is a separate language decision.
Missing from the list: §8.5.15/§8.5.16 if 3b goes ahead.

---

## 5. Score

**Concept count: 13** — accepted (Module, Shape, Pin, Layout, Function, Cell, Partial, Term,
Input, Watch, Reaction, Commit, Status).

**Entry-point count: 6**, one contract each — accepted.

**Elegance: 8.1 / 10** (v0 5.5 → v1 7.0 → v2 7.75).

- Why it crosses 8: every concept now has one sentence that is true without a footnote. A pin
  is a net and obeys the net rules, no exemption. What is not a net belongs to a call: an
  input, a term or a watch. Functions are pure by construction (no effect channel), reactions
  have no memory, so checkpoint, speculation, replay and the differential oracle cost no entry
  point. The host, a foreign process and an MSL reaction are one thing with one flag between
  them. Analog and discrete code share one concept and one call. An OSDI blob, an AVR and a
  resistor are written with the same six entry points and no capability flag.
- Why it is not higher: P6 is the one rule still stated wider than its reason (N1); Input and
  Watch are each one name over kinds with different timing, honest now but still the two
  widest tables; `Pin` has seven flags; two limit forms exist because one foreign format
  demands it; and the design leans on spec amendments, two of which (5, 6) push ABI
  convenience into the language instead of the other way round.

Stop rule: elegance ≥ 8 and no BLOCKER — **met**. Before `abi_final.md`: apply N1 (the
parameter cone), m1–m3, and settle amendments 1, 5, 6, 8 with the user.
