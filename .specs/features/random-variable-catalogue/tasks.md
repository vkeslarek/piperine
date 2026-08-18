# Random-Variable Catalogue — Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement with the `tlc-spec-driven` skill: activate by name, follow its Execute flow and Critical Rules.

**Design**: `.specs/features/random-variable-catalogue/design.md`
**Status**: Draft

## Test Coverage Matrix

| Layer | Type | Expectation | Location | Command |
|---|---|---|---|---|
| lang | integration | RVCAT-01/07 parse+elab+catalogue carrier | `vams_param_distributions.rs` (new) | `cargo test -p piperine-lang --test vams_param_distributions` |
| api/host | integration | RVCAT-02..06, 08 | migrate `monte_carlo_{sampling,report,compile_once}`, `center_policy`, root `tol_inert` | `cargo test -p piperine-api; cargo test --test tol_inert` |

## Gate Check Commands

Quick: `CARGO_TARGET_DIR=/var/tmp/piperine-target cargo test -p piperine-lang` · Full: `cargo test -p piperine-api && cargo test --test tol_inert` · Build: zero warnings + final workspace sweep.

## Execution Plan

```
T1 → T2 → T3 → T4
```

4 tasks, single inline batch.

## Task Breakdown

### T1: Grammar — `tol` clause on parameter declarations
**Where**: `ast/item.rs`, `grammar/item.rs`
**Req**: RVCAT-01
**Done when**: `parameter real r = 1e3 tol gauss(sigma = 10.0) global;` parses (TolClause on ParamDecl; keyed args; contextual keywords; two `tol` clauses loud). ≥5 parse tests; annex_b untouched.
**Tests**: integration · **Gate**: quick · **Commit**: `feat(lang): parse tol distribution clauses on parameters`

### T2: Elab — Distribution onto the POM
**Where**: `elab/lower/vams.rs`
**Req**: RVCAT-01, 07
**Done when**: keyed args const-eval in module scope; `Param::distribution` set; unknown kind loud; mixed with `from` works. ≥4 elab tests; discrimination check.
**Tests**: integration · **Gate**: quick · **Commit**: `feat(lang): carry tol distributions onto the POM`

### T3: Host migration — MC/center/tol_inert
**Where**: `crates/piperine-api/tests/{monte_carlo_sampling,monte_carlo_report,monte_carlo_compile_once,center_policy}.rs`, `tests/tol_inert.rs`
**Req**: RVCAT-02..06, 08
**Done when**: all named suites green except `high_sigma_sampling` (red, named blocker); fixtures migrated per the established pattern; compile-once asserted (1000 samples, 0 rebuilds).
**Tests**: integration/e2e · **Gate**: full · **Commit**: `test(host): monte carlo and tolerance fixtures on the live producer`

### T4: Docs + final sweep
**Where**: `docs/spec/part_i_language.md` (parameters), `ROADMAP.md`, spec statuses
**Req**: RVCAT-09
**Done when**: docs current; workspace sweep table updated; zero warnings.
**Tests**: none · **Gate**: build · **Commit**: `docs: parameter distributions current, ROADMAP distributions resolved`

## Granularity / Cross-check / Co-location

T1 grammar ✅ · T2 elab ✅ · T3 migration ✅ · T4 docs ✅ — linear deps match the map; test types match the matrix. ✅
