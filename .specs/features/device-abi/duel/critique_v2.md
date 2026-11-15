# Critique of `abi_v2.md`

v2 did what was asked: it merged instead of adding. Every v1 MAJOR is closed in substance.
What is left is three holes of one sentence each in normative text, and a handful of places
where a single name still covers two rules. 0 BLOCKER, 3 MAJOR, 9 MINOR.

---

## 1. Verification of v1 items

| v1 | Verdict | Note |
| --- | --- | --- |
| N1 `create` circular; shared shape vs effective init | **CLOSED** | `Module.prototype`; shape = structure only; solver seeds nets. Residue: derived defaults / "given" (m1), prototype-vs-shape pin indices (m2). |
| N2 `SESSION` is Env with strings | **CLOSED** | Per-call values are inputs; instant values are `GLOBAL` nets under P1–P4; `u32` names. The `setup`/`cache` case is answered by P2. |
| N3 solve before knowing the landing | **CLOSED** | Discrete first, one Newton solve per landing. Two stoppers missing → M1. |
| N4 irreversible changes the time-point set | **CLOSED** | "Not a time point" stated; price stated. |
| N5 OSDI limiting split | **CLOSED** | A3 one-call form; verifier scope stated; model setup in its own instance; scratch by hash. Residue → M3 (`RESHAPE` paths). |
| N6 `tick`/`horizon` off-tick | **CLOSED** | |
| N7 relaxable net not in the shape | **CLOSED** | `RELAX`; no-known-byte non-convergence is a fault. |
| N8 F2 by reads | **CLOSED** | By writes; F3; the current source is in §11.1. |
| N9 `commit` × 3, `STREAM` | **CLOSED** | One contract. Record = commit to a `RECORD` pin; trace = watch. Residue: stream identity (m3), `TRACE` vs the watch rule (m4). |
| m-a … m-i | CLOSED, except **m-e over-corrected** → M2 | |

## 2. The rejections and the builder's own doubts

| Item | Judgment |
| --- | --- |
| "part" stays (12 values) | **Accepted.** `DELAY.TIME` is genuinely per-evaluation and the limit's four parts are intrinsic. |
| Input has ten kinds (weak point 1) | **Accepted with a condition** (m5): the name is honest, the table is not. |
| `QUIET` / `RECORD` | **Accepted.** `QUIET` is a real capability removal with one rule; `RECORD` is a label the sink filters on. Keep both; fix m3. |
| `LATER` narrows §17.2.2 (weak point 2) | **Accepted as an ABI rule.** It is the only reading under which a delayed driver is cheap, and the due instant is itself tested by R4's first clause. The spec sentence needs the words "at that instant"; that is a note for the orchestrator, not a defect of the ABI. |
| Reaction partials still deferred | Accepted last round; `Host.size` satisfies the condition. |

---

## 3. MAJOR

### M1. The normative protocol omits two things that stop it: a waiting host, and `FINISH`

**Host waiters.** §23.1 rule 2: the host may wait for any event and act in response, "due at
`(t, n + 1)`". The host is code the owner cannot undo.

```
tb:   await sim.rise("dv");  sim.drive("ack", 1)          # Python, side effects, cannot be replayed
t = 0, t_c = 10 ns.  Step 2 runs the discrete instants of (0, 10 ns) speculatively:
      4 ns: dv rises.  Is the host called now?
        yes -> its drive of `ack` at (4 ns, n+1) is in; step 3 then lands at t' = 3 ns (a crossing):
               restore(C) cannot un-call the host; replay calls it again -> I1 broken.
        no  -> instants 5..10 ns have run without `ack`; the host's answer arrives too late -> I3 broken.
```

The fix is already in the design and costs no concept: **a host waiter on an event pin is an
`IRREVERSIBLE` reaction on that pin** (and a host probe at a scheduled time is one that reads a
continuous pin). One sentence in R6/§10; today "host actions are owner operations on nets" does
not say when the host runs. `HORIZON` must also count pins the host may drive in response.

**`FINISH`.** §17.2.2: the instant at which a `finish` record ends the analysis is a time
point. `FINISH` is a status, known only after the reaction ran; no flag says which reactions
can return it. In step 2 a speculative reaction at `s < t_c` returns `FINISH`: the protocol
has no branch. State it: stop, `t_c := s`, solve and accept, re-run the instant (it is
reversible, so this is sound), then end.

### M2. `PREVIOUS` was over-restricted: MSL's `prev()` in a definition or in `analog` no longer lowers

§4.1/§9: `PREVIOUS` and `CHANGED` may be read "only for a function that writes event pins
only, and for reactions"; anything else is a bind error. My m-e asked this for staleness; it
is right for `CHANGED` and wrong for `PREVIOUS`. `PREVIOUS` changes exactly when `VALUE`
changes, so a function reading it is re-run at the right microstep and is never stale. And the
language needs it (§14.12: `s.prev()` is allowed "wherever the carrier is read"):

```
module Edge(in d: Bit, out step: Bit<i8>) { step = (d as i8) - (d.prev() as i8); }   // a definition
analog for Slew { i(out) +> gain * (code.level() - code.prev().level()); }             // analog reads prev
```

Both write a discrete pin or a term, so both are bind errors in v2. Restrict `CHANGED` only.

### M3. `RESHAPE` has paths the text says cannot exist, and P6's shadow is not transitive

§9: "during a run it only arises on the shadow of P6". Counter-examples from §11.8 itself:

```
Dc sweep of TEMPERATURE (a GLOBAL net, P2: changes at microstep 0 of the next point)
   -> Osdi.setup re-runs -> collapse differs (rs(T) crosses its threshold) -> RESHAPE.  No `set`, no shadow.
host set of card.rsh (a field of the shared model card):
   P6 runs "the functions that read the param": OsdiModel.setup_model.  It succeeds.
   The commit is accepted; mdata changes; 10 000 Osdi.setup functions run; one returns RESHAPE mid-run.
```

Needed: (a) P6's shadow evaluation is the *transitive* closure of discrete functions
downstream of the written net, across instances; (b) the same shadow check applies to every
host write, `GLOBAL` included, sweep points included; (c) what a sweep does when a point needs
a reshape (re-create and bind between points is legal there — say so, or say the sweep fails).
Without this an analysis can end for a reason the document denies.

---

## 4. MINOR

- **m1. "Given" and derived defaults.** The shape holds defaults; the solver seeds a param net
  "with what it passed". A foreign default that depends on other params (BSIM, OSDI
  `param_given`) is then unknowable to the solver. The design already has the answer — a param
  field with a known byte, 0 = not given, the device's setup resolves it — but P5 does not say it.
- **m2. Pin indices across shapes.** `Value.pin` indexes the *prototype*; the created shape may
  differ in structure. State that `PARAM` pins keep their prototype indices in every shape of a
  module (or address by name again).
- **m3. A record's stream has no identity.** v1 had `sim`/`checks`/`log` as names; v2 has "a
  `RECORD` pin (one per stream and instance)" and the example calls it `log`. Is the stream the
  pin's `name`? Say so. Also say that delivery is at make-time (the sink "sees every commit"),
  so C3's transport removal — a loop emitting eight records through one statement removes
  seven pending commits — loses no record; as written, "follows C1–C4 unchanged" suggests it does.
- **m4. `TRACE` breaks the watch sentence.** Concept 10: "the solver … does nothing with but
  look", read at accepted solutions; `CROSS` fires an event, `CAP` caps a step, `TRACE` is read
  at every evaluation and has an all-zero "not written" sentinel. The honest rule is "an analog
  output that enters no row"; the table (§4.3) already carries the four behaviors.
- **m5. Input: one name, two families, and the rule misstates both edges.** "A *real* value the
  solver supplies": `TICK`/`HORIZON` are `I64`; `LIMIT`/`NOISE`/`STIM` have parts a function
  *writes*. Three kinds are declared instances with a perturbation; seven are singletons, yet
  each must be listed as a named shape entry (two `FREQ` inputs in one shape mean nothing).
  Keep the concept; print the table as two blocks (held: `LIMIT`/`NOISE`/`STIM`; ambient: the
  rest, addressed by constant, no shape entry), and drop "real" and "of the analog solver"
  (§0.4) — `TICK`/`HORIZON` are the event owner's.
- **m6. Two limit contracts** (A2 solver-corrected, A3 self-corrected). Justified by N5, but
  nothing in the shape tells them apart except the absence of a partial; a forgotten partial in
  an A2 device silently becomes A3 with wrong values. A flag on the function (`OWNS_CORRECTION`)
  makes the verifier's scope and the error detectable.
- **m7. `Pin` is carrying role-specific fields**: `global`, `check`, `init`, `abstol`, seven
  flags; five of twelve fields apply to a subset. Not wrong; it is where the next field will
  land. `HAS_INIT`/`HAS_CHECK` could be the kind × role matrix's business.
- **m8. `GLOBAL` at the start.** Is the host's first write of `TEMPERATURE` (or `DC`/`TRAN`) a
  change at `(0,0)`? P5 says binding a param before the run is not; say the same for globals,
  and that an analysis run at a transient's last instant flips `TRAN`/`AC` without firing
  (§16.4: "no `changed()` fires").
- **m9. `QUIET` application time.** "Never keeps the microstep loop alive": state when the
  commit applies if no further microstep runs (at the instant's exit), so the next call reads it.

---

## 5. Score

**Concept count: 13** — accepted as claimed (Module, Shape, Pin, Layout, Function, Cell,
Partial, Term, Input, Watch, Reaction, Commit, Status). Input is one name over two families
(m5); I count it once because one sentence is true of both.

**Entry-point count: 6**, one contract each — accepted.

**Elegance: 7.75 / 10** (v0 5.5, v1 7.0).

- Up: the pin rule now has no exemption (streams gone, session values split by whether they
  exist at instants); `commit` means one thing; functions have no effect channel at all, so
  "pure" is structural, not promised; limit, noise and stimulus are one rule (A2); the
  protocol does one solve per landing; a current source, an OSDI blob and an AVR are written
  with the same six words.
- Not yet 8: the normative protocol is silent about the one participant that is always
  irreversible, the host, and about `FINISH` (M1); a language feature (`prev()` outside
  events) was cut off by an over-correction (M2); the document asserts a `RESHAPE` invariant
  its own example violates (M3). And three names still cover two rules each without the table
  admitting it: Input (held / ambient), Watch (observed at acceptance / traced per
  evaluation), Limit (A2 / A3).

**What reaches ≥ 8:** M1–M3 (three sentences and one un-restriction), plus m3, m5 and m6 —
none adds a concept or an entry point.
