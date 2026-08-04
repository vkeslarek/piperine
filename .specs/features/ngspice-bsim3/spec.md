# ngspice-bsim3 Specification

> **STATUS: NOT YET SPECIFIED — scaffold only.**
>
> Opened during `ngspice-parity` specification (2026-08-04) so the split is
> recorded and the measurements taken there are not re-derived. This file holds
> **what is already known and decided**; the User Stories, Acceptance Criteria
> and Requirement IDs are deliberately absent until the feature is refined.
>
> **Do not run Design or Tasks against this file as it stands.** Refine first.

**Reference:** `~/Git/ngspice/src/spicelib/devices/bsim3/` (BSIM3v3.3).
**Split from:** `.specs/features/ngspice-parity/spec.md` (§Follow-on features).
**Governing decisions:** MD-38 (parity is measured, not asserted), MD-39 (no
device that PHDL already is), MD-24 (declared language surface).

---

## Why this is its own feature

Measured, not estimated:

| Metric | Value |
| ------ | ----- |
| ngspice C source | **11,442 LOC** across `bsim3/` |
| Largest single file | `b3ld.c` — 3,190 LOC (the load/stamp path) |
| Model parameters | **~166** model-card parameters |
| Comparable already-ported work | mos1 (7,017) + dio (3,504) = 10,521 — which was a whole feature |

Keeping it inside `ngspice-parity` would have held that feature's oracle (P1)
and its model fixes (P2) hostage to the single largest port in the backlog.

## Hard dependencies

| Depends on | Why | Status |
| ---------- | --- | ------ |
| `ngspice-parity` **P1** (measured oracle: AC, noise, transient, temperature) | A port of this size is untrustworthy without a falsifiable cross-check in every domain. This is MD-38 applied: do not port 11k LOC against a DC-only oracle. | Pending |
| `ngspice-parity` **P4** (`gap-3`, array-net expansion) | BSIM3 carries an internal charge node (`BSIM3qNode`, `bsim3def.h:31`) for the NQS path; multi-segment/internal-node authoring wants generic array nets rather than hand-unrolled scalars. | Pending |

## Known scope boundaries (inherited, already decided)

| Item | Decision | Source |
| ---- | -------- | ------ |
| Version target | `bsim3` directory = **BSIM3v3.3** | ngspice-parity Assumptions |
| Legacy forks (`bsim3v0`, `bsim3v1`, `bsim3v32`) | **Out of scope** — kept by ngspice only for back-compat against pinned model cards | ngspice-parity Out of Scope |
| SOI variants (`bsim3soi_dd/fd/pd`) | **Out of scope** — distinct device physics, own feature | ngspice-parity Out of Scope |
| Tolerance contract | Inherits `ngspice-parity` P1's per-domain contract; does not define its own | MD-38 |
| Bug-for-bug fidelity | Not the goal — a deviation traced to an ngspice defect is documented and piperine's value asserted | MD-38 |

## Open questions to resolve at refinement

None of these are answered yet; they are recorded so refinement starts from a
known list rather than a blank page.

1. **NQS path in or out of the MVP?** `BSIM3qNode` exists for non-quasi-static
   charge. Is the first delivery quasi-static only (`nqsMod = 0`), with NQS a
   follow-up?
2. **Parameter coverage bar.** ~166 model parameters. Is the acceptance bar
   *all* of them, or a declared subset with the remainder failing loud when
   supplied (MD-24's fail-loud discipline applied to model cards)?
3. **Which model card is the golden fixture?** A published/open PDK card is
   needed for the cross-check. Which one, and can it be committed to
   `tests/ngspice/`?
4. **Binning.** BSIM3 supports geometry binning (`lmin`/`lmax`/`wmin`/`wmax`
   model selection). In scope, or a follow-up?
5. **Authoring route.** One PHDL module with internal nodes, or a composed
   structure? Depends on what `gap-3` actually delivers.
6. **Phasing.** 11k LOC does not land in one execution pass either. What is the
   internal split — e.g. DC/temperature first, then charge/AC, then noise?

## What this feature does NOT need to re-derive

- The per-domain tolerance contract (comes from `ngspice-parity` P1).
- The harness plumbing for AC/noise/transient/temperature fixtures (same).
- Whether the parity claim needs a test behind it (MD-38: it does).
