# Solver Commit/Rollback Lifecycle

**Implements:** MD-12 (ABI vs policy)
**SOLVER_GAPS reference:** §1 commit/rollback, §1 per-element hidden-state vector

## Problem

Transient checkpoints digital state (`DigitalState::checkpoint/rollback/
commit`), but mixed-signal devices can also keep analog event detector state,
D2A cached state, delayed digital outputs, random-source state, or co-sim
state. A rejected timestep only restores the digital net array — everything
else is left as-is, making retries non-deterministic.

## Goals

- `Element::checkpoint_state`, `rollback_state`, `commit_state` hooks
  (default no-op; elements with hidden state override)
- Per-element hidden-state vector, sized at construction
- Solver drives checkpoint/rollback/commit around candidate timesteps

## Acceptance Criteria

1. WHEN a transient step is rejected THEN every element that declared `SUPPORTS_ROLLBACK` SHALL have its state restored to the checkpoint
2. WHEN a step is accepted THEN `commit_state` SHALL be called on every element
3. WHEN an A2D model records a crossing during a rejected step THEN the crossing SHALL be rolled back
4. WHEN `cargo test --workspace` runs THEN all targets SHALL pass

---

## Requirement Traceability

| ID | AC | Status |
|----|----|--------|
| ROLL-01 | AC1 — rollback restores element state | Pending |
| ROLL-02 | AC2 — commit on accept | Pending |
| ROLL-03 | AC3 — A2D crossing rolled back | Pending |
| ROLL-04 | AC4 — tests green | Pending |

**Coverage:** 4 total, 0 mapped to tasks ⚠️
