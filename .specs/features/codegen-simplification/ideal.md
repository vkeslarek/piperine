# codegen-simplification IDEAL — separate runtime semantics from compile-time machinery

**Status:** surface design sketch, 2026-07-31. Superseded where it disagrees with
`spec.md` (2026-08-02), whose §0 lists the three claims here that measurement
falsified: capability parity already landed with `dv-tester` (§2 is stale), the
four god-files do not shrink (§0 is stale), and the limiter is not "already
pure" — its formula is emitted Cranelift IR, not Rust (§1's table is stale).
A full sweep (2026-08-03) also found a **fifth** family this document never
named — digital edge detection, duplicated across codegen and the host
sequencer and already **divergent** on `X→1`/`Z→1` — and established that §1's
`signal::noise` / `signal::companion` candidates would receive nothing at all.
Kept unedited as the reasoning of record at the moment the decision was taken.
**Depends on:** `dv-tester` — `piperine-signal` must exist and be proven in
production use before codegen is rewired to it. This feature is the **next
priority after `dv-tester`** (user, 2026-07-31).
**Not to be confused with:** `codegen-architecture` (delivered, MD-23), which
reshaped the *module tree* to name the pipeline stages. That was about where
files sit. **This is about what is inside them** — and specifically about moving
a whole category of content out of the crate entirely.

---

## 0. The thesis — codegen holds two different kinds of thing

`piperine-codegen` is 16 144 lines, and its four largest files are
`kernel/analog/compile.rs` (1 694), `device/analog/instance.rs` (1 416),
`flatten/analog.rs` (1 329) and `kernel/analog/kernel.rs` (982). `codegen-architecture`
already fixed the tree; the files stayed big, and the crate still reads as
*mystery* rather than as *machinery*.

The audit done for `dv-tester` (`../dv-tester/ideal.md` §3.9) found why. Codegen
mixes two categories that have nothing to do with each other:

| Category | Examples | Is it codegen's job? |
|---|---|---|
| **Compile-time machinery** | symbolic differentiation, Cranelift emission, CSE, the `SimCtx` ABI, POM→resolved lowering, monomorphization | **Yes.** This is literally what a code generator is. |
| **Runtime semantics** | what `transition` *means*; how pnjlim converges a junction; how a `ddt` companion discretizes; what `table` interpolates; how a margin reduces over time; how a crossing is detected | **No.** This is signal-domain knowledge that lives here only because codegen was the first consumer. |

The second category is the mystery. It is domain physics and numerics written in
the idiom of a compiler backend, buried in files whose stated job is JIT
emission — so a reader looking for "how does slew rate limiting work" has to
first understand Cranelift plumbing to find it.

**The thesis: move category two out. `piperine-codegen` should compile the POM to
executable code and know nothing about what the code means.**

---

## 1. What moves, what stays

`dv-tester` establishes `piperine-signal` as a **pure** crate — values,
derivatives, event predicates, running reductions; no MNA, no `Element`, no JIT,
no dependency on anything in the tree. That purity is the sorting rule: if a piece
of logic can be stated without reference to a matrix or a compiler, it belongs in
`piperine-signal`.

### Moves (already proven pure by the `dv-tester` audit)

| From | To | Why it is easy |
|---|---|---|
| `device/analog/operators.rs` — `delay`/`slew`/`transition`/`idt` | `signal::ops` | already `f64` + `VecDeque`, zero solver types |
| `device/analog/events.rs` — `EventDetector` | `signal::events` | already pure; gains tiers 2/3 in `dv-tester` |
| `device/analog/limits.rs` — `Limiter` (pnjlim/fetlim) | `signal::limit` | already pure |
| `resolve/pom/analog_ops.rs` — `Table` | `signal::behavior::table` | already lowers to a pure piecewise expression |

These four are `dv-tester`'s **extraction** set — they are lifted into the crate
by that feature. This feature's contribution for them is only the **delegation**:
codegen deletes its copies and calls the crate.

### Candidate moves (need the same audit before they are promised)

| From | Candidate | The question to answer first |
|---|---|---|
| `device/analog/constraints.rs` + `kernel/analog/constraints.rs` | `signal::measure` | this is `dv-core`'s margin reduction. Sharing it retires §8.1's "two definitions of peak" debt — the single biggest win available here |
| `kernel/analog/reactive.rs` + the `ddt` companion coefficients | `signal::companion` | the *formula* is math (`TR-BDF2` coefficients × charge); the *stamping* is not. Split them and only the formula moves |
| `device/analog/forces.rs` | — | branch-unknown allocation is MNA-shaped. Probably stays |
| `kernel/analog/noise.rs` | `signal::noise` | `S(f) = psd·(1/f)^exp` is one formula; the per-source terminal wiring is not |
| `flatten/analog.rs` — the `ddt`/`idt`/`ddx` marker split rules | — | these are *lowering* rules (POM→resolved). They stay. Their **loud-failure catalogue** may be worth sharing as a documented contract |

### Stays, permanently

`resolve/diff.rs` (symbolic differentiation), `emit/` (Cranelift), the resolved
form, `flatten/`'s lowering, monomorphization, `device/circuit.rs` +
`builder.rs` + `fusion.rs` (circuit construction), the digital compiler. This is
codegen's actual deliverable and none of it is mysterious once the semantics stop
sharing the same files.

---

## 2. The checkable goal — capability parity

The readability argument is real but soft. This feature also has a **hard,
testable** goal, and it is the one that makes it worth doing rather than merely
nice:

> **A device written in the host language can do everything a PHDL device can
> do**, because both read their semantics from the same crate.

Today that is false, and invisibly so: a PHDL device gets `transition`, `slew`,
`delay`, `idt`, `cross`, `above`, `timer`, `table`, `$limit`, `$bound_step`,
`ddt` companions and noise PSDs — and a host-written device gets whatever the
host library happened to reimplement. Two vocabularies, one of them silently
smaller.

**The acceptance test writes the same device twice** — once in PHDL, once as a
host device — and asserts identical results on the same fixture. Do it for a
device exercising each family (a `transition`-driven source, a pnjlim junction, a
`table` nonlinearity, a `cross`-triggered element) and capability parity stops
being a claim.

That test is only *possible* if the semantics are shared. Which is the argument
for delegation over duplication, stated as a gate rather than a preference.

---

## 3. What emptying actually buys

1. **Codegen becomes legible.** "POM in, executable device out" — one job, and
   the four god-files shrink to the machinery that justifies their complexity.
2. **The semantics become testable in isolation.** `transition`'s re-anchor rule
   is currently proven by unit tests inside a JIT crate. In `piperine-signal` it
   is a pure state machine with no build ceremony around it.
3. **One definition of everything.** §8.1's `Measure`-vs-`constraint` debt is
   retired rather than managed, *if* the constraint reduction moves (§1).
4. **Host devices reach PHDL's vocabulary** (§2).
5. **The semantics become documentable.** A reader wanting "what does Piperine
   mean by `slew`" gets a crate with one file per family instead of a grep
   through a 1 400-line instance.

Note what it does **not** buy: performance. Every extracted family is already
native Rust called from native Rust; moving it across a crate boundary is
neutral (and `signal` is a compile-time dependency, so inlining survives). Anyone
arguing this feature on speed is arguing the wrong case.

---

## 4. Sequencing, and the interim cost

**Decision (user, 2026-07-31): `dv-tester` extracts; this feature delegates.**
Option (b) of `../dv-tester/ideal.md` §12.1. The rationale is risk isolation —
lifting pure code into a new crate is mechanical, but rewiring codegen touches
files CLAUDE.md names as "not to edit casually", and bundling both into
`dv-tester` would put a correctness-critical refactor on the critical path of a
feature whose value is elsewhere.

**The interim cost is real and must not be forgotten:** between `dv-tester`
landing and this feature landing, four families exist **twice** — once in
`piperine-signal`, once in codegen. The holding measure is a per-family
cross-check fixture (one fixture, both paths, asserted equal), authored in
`dv-tester` and deleted here when the codegen copy goes away. Without it the two
copies drift and this feature turns from "delete a copy" into "reconcile two
behaviors", which is a much worse job.

Ordering within this feature, roughly: delegate the four proven families first
(each is delete-a-copy plus a call-site change, individually revertible), audit
the candidates in §1 second, and only then consider whether the constraint
reduction moves — that one crosses into `dv-core`'s territory and deserves its
own decision.

---

## 5. Risks

| Risk | Containment |
|---|---|
| The two copies drift before delegation lands | §4 — cross-check fixtures authored in `dv-tester`, deleted here |
| Rewiring `instance.rs`/`compile.rs` breaks a device silently | The numeric oracles already exist: `tests/ngspice_validation.rs` is a 30-test cross-check against ngspice, and every extracted family has behavior tests. Nothing here should change a single number — that is the whole acceptance bar |
| "Emptying" turns into re-architecting | Scope discipline: this feature **deletes duplicated semantics and calls the crate**. Reshaping what remains is `codegen-architecture`'s job and it is done |
| A family looks pure but is not | §1's candidate table exists precisely so the audit precedes the promise. Anything MNA-shaped stays |
| The `Measure`/`constraint` unification balloons the scope | Explicitly optional and last (§4). The feature is worth doing without it |
| `piperine-signal` becomes a dumping ground | The purity rule (§1) is the filter, and it is mechanical: if it needs a matrix or a compiler, it does not go in |

---

## 6. Open questions

1. **Does the `dv-core` constraint reduction move?** The biggest single win
   (retires the two-definitions-of-peak debt) and the biggest scope increase,
   crossing into another feature's territory. Decide after `dv-tester` proves
   `Measure` in use.
2. **Do the `ddt`/`idt` companion *formulas* separate cleanly from the
   stamping?** If yes, `signal::companion` gives a host device real reactive
   elements — which is a capability-parity item, not just tidiness.
3. **How far does capability parity go as a promise?** Every family, or a named
   subset with the rest documented as PHDL-only? A promise that erodes is worse
   than a scoped one.
4. **Does anything outside codegen and the host library want `piperine-signal`?**
   Post-processing a stored `Waveform`, the CLI, plotting, the LSP folding a
   `Wave` for a hover — each would confirm the crate boundary. None is required.
