# Critique of `abi_v1.md`

v1 is a different and much better document: one Function concept, state as a pin, a stated
synchronization protocol, typed cells, layouts. The three v0 blockers are closed in substance.
What remains is mostly second-order: rules that contradict each other on a one-line device,
and places where a v0 problem was renamed rather than removed.
0 BLOCKER, 9 MAJOR, 9 MINOR.

---

## 1. Verification of v0 items

| v0 | Verdict | Note |
| --- | --- | --- |
| B1 R2 unsound | **CLOSED** | The §5 protocol is sound: restore before every retake, deterministic replay; irreversible instants run only at accepted analog times. Residue → N3 (wasted solve order), N4 (irreversible instants vs the set of time points). |
| B2 AVR time base | **CLOSED** | `horizon`, `LATER`, `start`, `resolution` do the job; the example now runs. Residue → N6 (`tick`/`horizon` at a non-tick time point; `horizon` is a per-reader value in a "pin"). |
| B3 OSDI-like | **CLOSED, with a hole** | setup-as-function + `RESHAPE` + struct param pins work. The limiting split of §11.8 does not exist in a compiled model → N5. |
| M1 events as convention | **CLOSED** | R1 strict. |
| M2 cross-instance resolve | **REOPENED (narrower)** | Resolver instance is fine. D1's "relaxable net" is not in the shape → N7. |
| M3 typed bytes | **CLOSED** | One `Layout`. Residue: `TEXT` (m-a), canonical unknown (m-b). |
| M4 two ambient channels | **CLOSED as a count, REOPENED as a rule** | One channel now, but `SESSION` pins break three pin rules → N2. |
| M5 Slot = four concepts | **CLOSED** | Typed, named parts, three operators. 9 sub-kinds remain (3+2+1+3), honestly tabled. |
| M6 limiting | **CLOSED** | A2 is right: solver forms `g·p` and the correction; `AGE` replaces `iter`; FD check valid. |
| M7 param in two namespaces | **CLOSED, new hole** | One namespace; but `create` now has no way to know how to encode its input → N1. |
| M8 shape per instance | **REOPENED (contradiction)** | "Instances share one shape" vs P4 "the returned shape lists every `PARAM` pin with its *effective* `init`" → N1. |
| M9 gradients | **CLOSED for what is in scope** | Partials on discrete functions, real fields of any layout. Residue: `seed` is per read cell, not per field (m-c). |
| M10 analog/define asymmetry | **CLOSED, with a bug** | One `eval`. F2's classification is wrong → N8. |
| M11 FFI UB | **CLOSED** | |
| M12 flicker at DC | **CLOSED** | A4 "cells the solve reads". |
| M13 host actions | **CLOSED** | P5 `check`, P6 owner overlay. |
| M14 determinism | **CLOSED** | R3, C5. |
| m1–m7 | CLOSED except m7 (partly, acceptable) | |

## 2. The rejections

| Rejection | Judgment |
| --- | --- |
| `after > 0` for irreversible reactions' commits that reach analog | **Accepted.** Under the kept discontinuity algorithm (context Q7) nothing is undone after acceptance; the argument holds. |
| Partials on reactions (§23.3 2/3/4a) | **Accepted.** Verified: solver-completeness `context.md:90`, `spec.md:30`, analysis row 67 defer it. Condition: the changelog's "one list on `Reaction`" is not enough — a commit must also carry the tangent bytes, so `Host.commit` gains an argument or a sibling. Say so now, so the `Host` table is laid out for it (it has a `size`? it does not: m-d). |
| Watch stays at three kinds | **Accepted**, with a counter-offer in §5 (merge `Source` and `Limit`, not the watches). |
| Sentinels `GROUND`, `abstol = 0`, `CAP = +inf` | **Accepted.** All three are values with a meaning, not flags in disguise. |

---

## 3. MAJOR

### N1. `create` cannot be called: its input encoding is defined by its output, and shared shapes contradict P4

```
host: instantiate Bsim4 with model card {vth0 = 0.42, tox = 2n}, instance {w = 1u}
create(module, values = [ Value{ name: "model", bytes: ??? }, Value{ name: "inst", bytes: ??? } ])
```

`Module.args` was deleted. The only description of a `PARAM` pin — its layout, the offset of
`vth0` inside the `model` struct pin, whether the pin exists — is in the `Shape`, which is what
`create` returns. The elaborator also cannot check an `extern` declaration against the foreign
descriptor (E0373) without creating an instance. Either `Module` carries the param prototype
(`List<Pin>` restricted to `PARAM`, with layouts and defaults), or the protocol is "create with
no values, read the shape, create again" and must be stated (and then the first `create` of a
model with a required param fails).

Second half: P4 says the shape reports each param's *effective* `init`; §9 says shapes are
interned "one per module and structural values". Two resistors, 1 kΩ and 2 kΩ: one shape or
two? If one, `init` is wrong for one of them; if two, M8 is back (one shape per distinct
parameter set). Fix: the shape holds defaults only; the solver seeds each param net from the
values it passed to `create`.

### N2. `SESSION` is `Env` spelled with strings, and it breaks the pin rules it was folded into

A `SESSION` discrete pin: is "not a net", has no one-writer rule, "its change is never an
event", is rewritten "between calls", and — for `horizon`, `static`, `time` — has a **different
value for each reader in the same microstep**. That is a call argument, not a pin. Concretely
unspecified:

```
fn "setup":  reads [model, temperature]  writes [cache]        (discrete function, §11.8)
Dc sweep of temperature:  `temperature` is rewritten; "its change is never an event".
  Is `setup` re-run?  (F: "an input changed" — yes.)  `cache` then differs: does cache.CHANGED fire?
  At which (t, n)?  Is it a boundary event for the analog functions reading `cache`?
fn "bad":    reads [time]  writes [some discrete pin]          (legal by F2; MSL forbids it, E0336)
  A discrete pin that changes continuously with time: at which instants does it "change"?
```

Needed: one rule for discrete functions over session pins (which names they may read — the
analysis-constant ones only — and that a rewrite between analyses/sweep points is collected at
microstep 0 like a host commit). Also the namespace: reserved names and host `simparam` names
share one string space, and §12 lets a minor version add a session name — a user's `simparam
"vdd"` or `"horizon"` silently changes meaning. Reserved names should be `u32` constants (like
every other enumeration in v1) and `simparam` a name under its own role or flag.

### N3. The protocol solves the analog step before it knows where it must land

§5: (1) solve tentatively to `t_c`; (2) run the discrete instants in `(t, t_c)`; (3) if one is a
time point `s`, restore, `t_c := s`, solve again. v0 bounded `t_c` by the queue first; v1 dropped
that. In the common mixed-signal case (a PWM, a DAC update, a clock into a `D2A` every few ns)
*every* step is solved twice and the first Newton solve is thrown away. The discrete instants
of `(t, t_c)` never read analog (else they would be time points), so they can be run
speculatively **first**: find `s`, then solve once to `min(t_c, s)`; only a crossing located
earlier forces a restore, and what is replayed is cheap discrete work, not Newton. Same
soundness, half the solves. Since the protocol is declared normative, the order matters.

### N4. `IRREVERSIBLE` changes the set of time points, which the language makes observable

R6: the owner "makes every instant at which it fires an accepted analog time". An accepted
analog time is a time point (§15), and `settle` fires once per time point (§17.2 resolution:
"`settle` observes it"). So the same co-simulated block, declared reversible or irreversible,
gives a design with `on settle` monitors a different number of settle phases, records and
settle commits. §8.5 says a cosim's requested time "is a scheduled time", a time point only
under §17.2.2. State it: an accepted analog time forced by an irreversible reaction is **not**
a time point (no settle phase, no `settle` commits delivered there) — or admit the deviation.
Also unstated for irreversible devices: `pss` shooting and a host that rewinds (they cannot);
and the price is larger than weak point 3 says — with any `CROSS` in the design, every instant
of an irreversible device costs an analog solve, crossing-reachable or not.

### N5. A compiled OSDI model cannot be split as §11.8 splits it

OSDI `eval` performs limiting *inside* the one call (the simulator's `pnjlim` is called back
with old/new state) and returns residuals already corrected (`lim_rhs`) plus total Jacobian
entries. There is no `∂/∂x̂` and no way to feed it an externally held `x̂`. So "fn 2 limit" /
"fn 1 eval reads `L*.LIMITED`" is fiction for the brief's device. What the adapter can do is one
function that reads `L*.LAST/AGE`, writes `L*.PROPOSED/LIMITED` **and** the terms, with the
correction already applied and no partial against `LIMITED` — i.e. v0's A4. Is that legal in
v1? A2 does not say; the FD verifier then fails on it. Also glossed: OSDI `eval` *mutates* its
instance data while the `cache` pin arrives as `*const u8` (a multi-KB copy per Newton
iteration, or hidden state that P5's shadow evaluation corrupts when a `set` is refused);
`setup_model` is per model, but fn 0 is per instance. State the allowed form ("a function that
writes a limit and terms in one call owns its correction; declare it with a flag so the
verifier skips it") and put the model card's setup in its own instance writing a shared net.

### N6. `tick` and `horizon` at a time point that is not a tick

v0 had a sentinel for "this time is not a tick" (T5: a step chosen by step control); v1 removed
the sentinel and the case with it. A reaction on `settle` (always at a time point, usually not
a tick) reading `tick` gets what? Settle commits apply at `(t_next, 0)` with `t_next` not a
tick and can change a pad the AVR listens to: `horizon: I64` in ticks cannot name that instant.
Define both (e.g. `tick` = the last tick ≤ t plus a `BOOL` session pin `on_tick`; `horizon` =
the last tick strictly before the earliest possible change), or make them `F64` seconds.

### N7. D1: "relaxable net" is not in the shape, and the generic rule is not harmless

Bind validation rejects "a dependency cycle … that is not closed by a relaxable net". Nothing
in `Pin`, `Function` or the binding says which nets are relaxable; the resolver is "an
instance like any other". So the solver cannot tell a resolved-drive loop from this:

```
two external NAND gates, each one discrete function, cross-coupled (an SR latch)
D1: "every pin of the cycle starts the microstep at zero bytes" -> the latch forgets its state
    on every evaluation, silently.   (MSL rejects this loop at elaboration; the ABI cannot.)
```

And where the loop does not settle, a field *without* `HAS_KNOWN` (a real `SumNet`) is "zeroed":
that is the value 0.0, not "no value". Needed: a pin flag (`DRIVE`/`RELAXES`) set by whoever
generates the resolver, cycles through unflagged pins are a bind error, and non-convergence on
a field with no known byte is a fault, not a zero.

### N8. F2 classifies by reads; a current source is then illegal

F2: "a function that reads a continuous pin, a source or a limit is *analog* … any other
function is *discrete*: it writes discrete and event pins"; §9: "a discrete function writing an
analog cell" is a bind error.

```
Isource:  fn 0 reads [idc]            writes [T0.VALUE]      -> discrete by F2 -> bind error
PWL src:  fn 0 reads [time, table]    writes [T0.VALUE]      -> same
§4 itself: "parts that depend only on params (ABSTOL, MAX, TTOL…) belong in a function that
           reads only params"                                -> same
```

Classify by **writes** (a function writes analog cells or pins, never both), and state when a
param-only analog function runs (when its inputs change; its outputs are held).

### N9. `commit` is three operations and `STREAM`/`EVENT` are pins that satisfy no pin rule

`commit` now means: queue a write (reaction), deliver a record (reaction, not queued, `after`
ignored, `FINISH` meaningful only here), deliver a trace (called from `eval`, where "statement
indexes the calling reaction's `writes`" has no referent, stamped differently, never undone).
A `STREAM` pin "holds nothing", "never changes", has "any number" of writers, nobody reads it:
it is exempt from kind ("what it holds"), P1 (one writer), P3-by-net and C1–C4. The entry
point was not removed; it was hidden behind an overloaded one. Either keep it and say so
(count 6 + 1 honest), or make the record a real instance of the rule: a discrete pin whose
*commits* (not whose value) the sink observes — then C1–C4 apply unchanged, "a record at an
undone instant is dropped" is free, and only the trace from `eval` is special. Also missing:
the legal kind × role matrix (4 × 4 = 16 combinations; `CONTINUOUS × PARAM`, `EVENT × PARAM`,
`STREAM × PORT`, `CONTINUOUS × SESSION` are presumably errors — the document does not say).

---

## 4. MINOR

- **m-a. `TEXT` leaf** ("Param pins only", `count` bytes): a variable-length value in a
  fixed-size layout; the one leaf with a role restriction. A firmware path is `create` data;
  give it its own home or fix the length rule.
- **m-b. Canonical "no value"**: P2 requires value bytes zero when the known byte is 0, the
  solver compares bytes, and nobody checks. `[1,0]` on a `bit` is a phantom change. The solver
  knows `HAS_KNOWN` offsets (D1 uses them): validate in debug sessions. And ship the canonical
  layouts (`BIT`, real-with-known) as constants in `piperine-abi`, or every external author
  re-derives what the elaborator prints.
- **m-c. `seed` is per read cell, not per field.** With the recommended struct param pin
  (P4, §11.8) seeding `model` asks for all 900 partials again — v0's M9 cost, moved.
- **m-d. `Host` has no `size`** while `Eval`/`React`/`Shape`/`Library` do; the deferred
  gradient commit will need to extend it.
- **m-e. `PREVIOUS`/`CHANGED` in a function that writes a *discrete* pin** go stale one
  microstep later with no input "changing". Restrict them to functions that write event pins.
- **m-f. Cost of state-as-pin** (weak point 4) is understated: per call an 8 KB `cpu` pin is
  copied in, committed out, compared for `CHANGED`, kept as `PREVIOUS`, and checkpointed; and
  every `run` adds a microstep at its instant just to apply the state. Allow a pin flag "no
  `PREVIOUS`, no change detection" for state nobody listens to.
- **m-g. Shapes are `'static` and "live until the process ends"**: a Monte Carlo or optimizer
  over a structural param leaks one shape per draw.
- **m-h. `RESHAPE` on a shadow evaluation** (P5: a refused `set`) is unspecified: refuse the
  write, presumably. And `RESHAPE` "during a run ends the analysis" makes a legal live `set` of
  an OSDI param fatal depending on its value.
- **m-i. What a reaction may read** is `List<Cell>` with no rule: a `WATCH`, a `TERM`, a
  `LIMIT.LAST`? State "pins only" (spec §14 rule 7 values are recomputed from the solution).

---

## 5. Remaining bloat and a path to ≥ 8

| Now | Proposal |
| --- | --- |
| `Source` and `Limit` as two concepts | Both are "a real input the solver holds; functions declare partials against it; the solver supplies the perturbation" (a sweep's injection, Newton's `x − x̂`). One concept *held input* with kinds `NOISE`, `STIM`, `LIMIT`; analog cells become term (output into a row) / input (held) / watch (output observed). −1 concept, and A2/A3 become one rule. |
| `Session names` as a string registry with per-reader values | `u32` names; split what is per-call (`horizon`, `static`, `time` as read by a reaction) from what is a slowly varying value; one rule for discrete functions (N2). |
| `commit` × 3, `STREAM` | N9. |
| `Status::RESHAPE` + `STRUCTURAL` + `create(values)` | keep, but fix N1 so there is one way to learn params. |
| 17 parts | fine once Source/Limit merge; `ABSTOL`/`MAX`/`TTOL`/`ETOL` could be plain `PARAM`-like private pins the cell names, removing "part" for everything but `VALUE`. Optional. |

---

## 6. Score

**Honest concept count: 15** (v1 claims 13). The 13 listed, plus **Cell/part** (the addressing
scheme: 5 cell kinds, 17 parts, read/write meaning each — it is the densest rule in the
document) and **Partial/seed**. `Statement` is counted inside Commit; Library inside Module.
With the Source/Limit merge and N9 resolved honestly this reaches 13–14.

**Entry-point count: 6** as claimed (`piperine_abi_1`, `create`, `destroy`, `eval`, `react`,
`commit`), with `commit` carrying three contracts (N9).

**Elegance: 7.0 / 10** (v0: 5.5).

- Up: device = shape + pure functions + reactions with no memory is now actually true, so
  checkpoint, replay, speculation and the differential oracle fall out with no entry point;
  analog and discrete code share one concept and one call; limiting and sources are both
  "differentiate against a solver-held input"; external and generated devices are the same
  thing down to the AVR's state being an inspectable pin; no MSL citation is left.
- Not yet 8: the unifications were pushed one step past what the rules support — `SESSION`
  and `STREAM` are called pins but are exempt from the pin rules (N2, N9); F2 forbids a current
  source (N8); `create` is circular and sharing contradicts P4 (N1); the normative protocol
  wastes a solve per boundary event and silently changes the time-point set for irreversible
  devices (N3, N4); the OSDI example describes a model that does not exist (N5); D1 depends on
  a property no shape declares (N7). None is structural; each is one rule or one flag.
