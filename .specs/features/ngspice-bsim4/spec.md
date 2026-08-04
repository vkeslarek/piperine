# ngspice-bsim4 Specification

> **STATUS: NOT YET SPECIFIED — scaffold only.**
>
> Opened during `ngspice-parity` specification (2026-08-04) so the split is
> recorded and the measurements taken there are not re-derived. This file holds
> **what is already known and decided**; the User Stories, Acceptance Criteria
> and Requirement IDs are deliberately absent until the feature is refined.
>
> **Do not run Design or Tasks against this file as it stands.** Refine first —
> and refine it *after* `ngspice-bsim3` delivers, since its porting method is
> this feature's main input.

**Reference:** `~/Git/ngspice/src/spicelib/devices/bsim4/` (BSIM4.8).
**Split from:** `.specs/features/ngspice-parity/spec.md` (§Follow-on features).
**Governing decisions:** MD-38 (parity is measured, not asserted), MD-39 (no
device that PHDL already is), MD-24 (declared language surface).

---

## Why this is its own feature

The largest single item in the model backlog. Measured, not estimated:

| Metric | Value |
| ------ | ----- |
| ngspice C source | **22,564 LOC** across `bsim4/` |
| Largest single file | `b4ld.c` — 5,601 LOC (the load/stamp path alone) |
| Model parameters | **~236** model-card parameters |
| Comparison | **Larger than mos1+mos2+mos3 combined** (23,272 LOC — itself a whole feature) |
| Comparison | ~2× BSIM3 (11,442) |

## Hard dependencies

| Depends on | Why | Status |
| ---------- | --- | ------ |
| `ngspice-bsim3` **delivered** | This feature reuses a *proven* porting method rather than inventing one at 22k LOC. BSIM4 is structurally BSIM3 plus additional physics; porting it first would be the expensive way to learn the method. | Pending |
| `ngspice-parity` P1 + P4 (transitively, via bsim3) | Measured oracle in every domain; array-net expansion. | Pending |

## What BSIM4 adds over BSIM3 (the real cost driver)

Not just "more parameters" — additional *structure*, which is what makes it
roughly double rather than incrementally larger:

| Addition | Evidence | Structural implication |
| -------- | -------- | ---------------------- |
| Gate resistance network (`rgateMod`) | `bsim4def.h:140` | Extra internal node(s) + branch unknowns |
| Body resistance network (`rbodyMod`) | `bsim4def.h:139` | Extra internal nodes (`dbNode`, `sbNode` — `bsim4def.h:47-48`) |
| Transient NQS (`trnqsMod`) | `bsim4def.h:137` | Charge node (`qNode`, `bsim4def.h:49`) |
| Parameter checking pass | `b4check.c`, 951 LOC | A whole validation surface with no BSIM3 equivalent |

Each mode is switchable per model card, so the port must handle the
node-topology variation — this is the part most likely to stress `gap-3` and
the internal-unknown allocation seam (`allocate_unknowns`, Part VII §5.2).

## Known scope boundaries (inherited, already decided)

| Item | Decision | Source |
| ---- | -------- | ------ |
| Version target | `bsim4` directory = **BSIM4.8** | ngspice-parity Assumptions |
| Legacy forks (`bsim4v5`, `bsim4v6`, `bsim4v7`) | **Out of scope** — back-compat forks for pinned cards | ngspice-parity Out of Scope |
| SOI variant (`bsimsoi`) | **Out of scope** — distinct device physics | ngspice-parity Out of Scope |
| Tolerance contract | Inherits `ngspice-parity` P1's per-domain contract | MD-38 |

## Open questions to resolve at refinement

1. **Which optional modes are MVP?** `rgateMod`/`rbodyMod`/`trnqsMod` each add
   nodes. Is the first delivery the simplest topology (all modes off), with
   each mode a follow-up increment?
2. **Parameter coverage bar.** ~236 parameters — same question as BSIM3, with
   more weight: declared subset + fail loud on the rest, or full coverage?
3. **`b4check.c`'s validation pass.** Port it as PHDL-level validation, as a
   host-side check, or rely on fail-loud parameter resolution (MD-24)?
4. **Golden model card.** Which published BSIM4 card, and is it committable?
5. **Phasing.** 22k LOC needs an internal split; the shape should follow
   whatever worked for BSIM3 rather than being decided in advance.
6. **Does the internal-unknown seam suffice?** Variable node topology per model
   card may stress `allocate_unknowns`/`HAS_INTERNAL_UNKNOWNS`; verify against
   the delivered BSIM3 before assuming.

## What this feature does NOT need to re-derive

- The per-domain tolerance contract (from `ngspice-parity` P1).
- The harness plumbing for every analysis domain (same).
- The porting method, phasing shape, and parameter-coverage policy — all
  expected to come from `ngspice-bsim3`'s delivery.
