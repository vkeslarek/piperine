# Solver OSDI-inspired ABI Completion

**Implements:** MD-11 (OSDI as checklist), MD-12 (ABI vs policy classification)
**SOLVER_GAPS reference:** §1 OSDI-inspired, §1 model/instance separation, §1 parameter/query (partial)

## Problem

The `Element` trait has basic introspection (`list_params`/`get_param`/
`set_param`/`query`/`list_queries`/`list_terminals`), but the contract is
incomplete: no explicit lifecycle hooks, no richer terminal descriptors, no
internal-unknown allocation, no noise metadata, no temperature protocol, no
formal limiting API, no Jacobian capability declaration, no model/instance
separation.

## Goals (each is an independent increment)

- Explicit lifecycle hooks (setup → temperature → load → accept → rollback → commit → destroy)
- Richer terminal descriptors (domain, discipline, direction, required/optional, sign convention)
- Internal unknown allocation API (pre-finalization, matrix frozen before analysis)
- Opvar catalog (declared, not just `read_opvars` default scan)
- Noise metadata (named sources, type, per-source reportability)
- Temperature protocol (nominal/instance/delta, invalidation rules)
- Parameter invalidation rules (Restamp/Temperature/OperatingPoint/Rebuild)
- Formal limiting API (proposed/limited values, limiter name, reason)
- Discontinuity/breakpoint notifications (request breakpoint, order reduction)
- Jacobian/stamp capability declaration (analytic/numeric/linear/charge/AC/noise)
- Model/instance query separation (`ModelHandle` vs `ElementInstance`)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Commit/rollback lifecycle | `solver-commit-rollback` |
| Unified event model | `solver-unified-events` |

---

## Acceptance Criteria

1. WHEN a model wraps an OSDI library THEN the wrapper SHALL be able to expose parameters, queries, terminals, and lifecycle through the native ABI without special-casing
2. WHEN a parameter changes THEN the invalidation rule SHALL tell the caller whether to restamp, recompute temperature, restart, or rebuild
3. WHEN a noise analysis runs THEN the solver SHALL be able to return per-source contributions, not only total PSD
4. WHEN `cargo test --workspace` runs THEN all targets SHALL pass

---

## Requirement Traceability

| ID | AC | Status |
|----|----|--------|
| OSDI-01 | AC1 — wrapper can expose full metadata | Pending |
| OSDI-02 | AC2 — invalidation rules | Pending |
| OSDI-03 | AC3 — per-source noise | Pending |
| OSDI-04 | AC4 — tests green | Pending |

**Coverage:** 4 total, 0 mapped to tasks ⚠️
