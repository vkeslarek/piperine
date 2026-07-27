# dv-core Design

**Spec:** `spec.md` (DVC-01..22). **Vision:** `../design-verification/ideal.md`.
Every claim about the current tree below is cited by `file:line`.

## A. Where each piece lands in the existing pipeline

```
PHDL  `param … tol gauss(…)`  +  `constraint Mod { … }`
  │
  ├─ parse/          ParamDecl grows a TolClause; new ConstraintBlock item
  │                  (six new reserved words — §C1)
  ▼
POM   Param.distribution()   Module::constraints           ← authored, walkable
  │   elab/: name resolution, type check, class check (§C2)
  ▼
resolve/  margin expressions with interned ids
  │   (resolve/diff.rs differentiates them later — dv-gradients)
  ▼
flatten/ → emit/   one kernel function per *pointwise* margin, CSE'd with
  │                the residual it shares subexpressions with
  ▼
kernel/analog/constraints.rs      new capability sub-struct behind Option
  ▼
device/analog/constraints.rs      evaluated at §C3's points, like limits.rs
  ▼
solver   margins + argmin; validation_reports() gated by EMITS_VALIDATION
  ▼
piperine-api   MarginsResult channel (D3); host reduction for reduced metrics (D7)
```

Every arrow lands on a file or pattern that exists. The `Option` sub-struct is
how `kernel/analog/` already organizes `forces.rs`/`limits.rs`/`operators.rs`/
`events.rs`, so a circuit with no constraints allocates nothing and calls nothing.

## B. Component breakdown

| Component | Home | Notes |
|---|---|---|
| `TolClause` parse + AST | `piperine-lang/src/parse/` | `tol` after a param default |
| `headers/statistics.phdl` | `piperine-lang/headers/` | `extern fn gauss(...)`, `uniform(...)`; MD-24 |
| `Param.distribution()` | `piperine-lang/src/pom/` | additive; no `Invalidation` change |
| `ConstraintBlock` parse + AST | `piperine-lang/src/parse/` | new top-level item, sibling of `analog`/`digital` |
| `Module::constraints` | `piperine-lang/src/pom/` | authored form; monomorphization carries it (D4) |
| `headers/constraints.phdl` | `piperine-lang/headers/` | margin/analysis helpers, **each declaring its class** (§C2) |
| Margin lowering | `piperine-codegen/src/resolve/pom/` | comparison → signed expression |
| Margin kernel | `piperine-codegen/src/kernel/analog/constraints.rs` | new file, `Option` sub-struct |
| Margin evaluation | `piperine-codegen/src/device/analog/constraints.rs` | new file, mirrors `limits.rs` |
| `ValidationFinding`/`ValidationReport` | `piperine-solver/src/core/` | next to `LimitingReport` |
| `validation_reports()` + `EMITS_VALIDATION` | `piperine-solver/src/core/element.rs` | `:173` shape, `1 << 15` |
| Posture + scope default | `piperine-solver/src/analyses/context.rs` | `checks`, `require_scope` |
| `MarginsResult` | `piperine-api/src/` | its own channel (D3) |
| Host reductions | `piperine-api/src/` | reduced metrics (D7) |
| SOA blocks | `piperine-lang/headers/spice/` | `mos.phdl`, `diode.phdl`, `bjt.phdl` |
| `enum Region` | `piperine-lang/headers/spice/constants.phdl` | PHDL already has enums (`prelude.phdl:13`) |

## C. The five decisions that shape the implementation

### C1. Six reserved keywords, and two examples get renamed

`constraint`/`require`/`target`/`tol`/`cover`/`global` are reserved at the parser
level and join the list at `docs/spec/part_i_language.md:232`. A contextual-keyword
path was considered and rejected (user, 2026-07-27): it buys nothing but parser
complexity in exchange for not editing two example files.

The only collision measured in the corpus is `target`, used as a variable in
`crates/piperine-lang/tests/examples/ring_oscillator.phdl:5` and
`oscillator.phdl:16` (`var target : Real = -gain * V(a, gnd);`). Both get renamed,
and their numerics must be identical before and after (DVC-06) — a rename is not
allowed to move a waveform.

**Correction on record:** an earlier draft called these a frozen corpus, citing
CLAUDE.md's "`headers/`, `tests/fixtures*` — frozen test corpora". Two things were
wrong. These files live in `tests/examples/`, not `tests/fixtures*`; and **no
`tests/fixtures*` directory exists anywhere in the tree** — CLAUDE.md's
files-not-to-edit list names a dead path. That line should be fixed or dropped
(tracked as a separate doc chore, not part of this feature).

### C2. Pointwise vs reduced is a property of the *declaration*

`ac_unity_gain_freq` cannot be computed at a frequency point — at any single
frequency the answer does not exist yet. So `headers/constraints.phdl` declares
each helper's class, and the elaborator uses it:

- **pointwise** (`ac_gain`, `V()`, `I()`, `m1.region`) → lowers to a margin kernel.
- **reduced** (`ac_unity_gain_freq`, `ac_phase_margin`) → no kernel; the host
  applies the reduction over the points the solver returns (D7).

A `require`/`target` reading a reduced quantity inside a pointwise scope is a
loud elaboration error (DVC-09). This is the check that keeps someone from
discovering the problem at JIT time.

Consequence for the result surface: a pointwise margin has an argmin, a reduced
one does not, and `MarginsResult` must represent both without inventing a `t = 0`.

### C3. Evaluation points — the definition that keeps `strict` honest

| Analysis | Evaluate at | Never at |
|---|---|---|
| DC / OP | the **final converged solution** | any homotopy stage. Gmin stepping adds shunt conductance and source stepping scales sources — each converges, and each intermediate is non-physical. A headroom `require` checked there fails on a solve that is converging correctly |
| Transient | each **accepted** step | rejected steps (`SUPPORTS_ROLLBACK`, `accept_timestep` at `element.rs:482`) |
| Transient `t = 0` | the OP, if computed | the UIC/`@initial` state — a forced initial condition may legitimately sit outside SOA |
| AC / noise | each frequency point (pointwise); once per sweep (reduced) | the DC solve underneath, covered by the DC row |
| Sweep / `sweep_grid` | each swept point, reduced to worst-across-sweep with the swept coordinate in the argmin | — |

The sweep row is load-bearing: `sweep`/`sweep_grid` over a compiled session is
the loop `dv-gradients`' optimizer, corner runs, and Monte Carlo are all built
from, so "worst margin over the sweep, and where" is a first-class result rather
than something the host reconstructs.

### C4. The findings channel costs nothing by being *unasked*

`validation_reports() -> Option<ValidationReport>` follows
`limiting_report()` (`core/element.rs:173`) — `Option`, not `Vec`, because `None`
is the common case.

Polling is gated by `EMITS_VALIDATION`. A defaulted method returning `None` still
costs a virtual call per element per accepted point; at 10 k elements over 10⁶
steps that is 10¹⁰ calls to learn nothing. `EMITS_NOISE` already establishes the
pattern of asking only the elements that declare they have something to say.
`ElementCapabilities` is a `u32` whose highest used bit is
`NUMERIC_JACOBIAN = 1 << 14`, so `1 << 15` is free.

**Margins are values; findings are events.** A margin crossing zero *generates* a
finding. The margin channel is continuous and feeds `dv-gradients`; the finding
channel is discrete and postural. Digital verification rides findings **only** —
there is no signed distance for a `Bit` or a `Quad` — which makes this channel
the primary digital mechanism rather than an accessory.

### C5. SOA limits are absent by default

The `headers/spice/` models are **ngspice-faithful**, and ngspice has no SOA data.
Inventing a `vds_max` would fabricate an unsupported number, break the
faithfulness contract the ngspice cross-check defends, and — with `strict` as the
default posture — trip the whole example gallery on limits nobody wrote.

So the limits are `Real?`/`+inf` by default and a `require` reading an unset limit
is **vacuously satisfied**. A foundry deck that sets them gets enforcement on
every instance. What is quiet by default is an *undeclared limit*, never the check.

## D. Data flow for one `require`, end to end

1. **Parse** `require vds : abs(V(m1.d, m1.s)) <= vds_max;` → AST statement.
2. **Elaborate** — resolve `m1.d`/`m1.s` through the authored instance tree
   (instance *ports*; internal nodes are loud), type-check Boolean, classify the
   expression pointwise, register the label.
3. **Resolve/lower** — comparison becomes `m = (vds_max − abs(V(d,s))) / 1`.
4. **Emit** — one kernel function, CSE'd against the residual that already
   computes `V(d,s)`.
5. **Evaluate** at §C3's points; track worst `m` and its argmin per instance.
6. **On `m < 0`** — emit an `Error` finding; `strict` aborts the analysis naming
   constraint + instance + time + value, `collect` records, `off` never ran.
7. **Report** — `MarginsResult` carries worst + argmin; `r.violations` carries the
   findings.

## E. Risks

| Risk | Mitigation |
|---|---|
| Margin kernels multiply per instance and approach the Cranelift function-count wall that `.disto` hits on MOS2/MOS3 (ROADMAP P1) | One kernel per *declared* margin, parameterized by instance — never per instance × margin. The `.disto` failure is the standing proof of what happens otherwise |
| A `require` on a spice model fires across the whole gallery | §C5: absent limits are inert. Acceptance test is the unedited gallery + unchanged `ngspice_validation.rs` |
| False violations during homotopy make the first user experience a bug report | §C3 is an acceptance criterion (DVC-11), with a gmin-stepping fixture that must not report |
| Per-point cost regresses transient performance | `Option` sub-struct + `EMITS_VALIDATION` gating; a no-constraint circuit must be bit-identical in cost (DVC-12 AC3) |
| Contextual-keyword parsing is subtle and `parse/` is marked "not to edit casually" | Both frozen `target` fixtures are acceptance tests; the parser change is one clause position, not a grammar restructure |
| `MarginsResult` becomes a frankenstein carrying two shapes | D3 gave it its own channel precisely so it can model pointwise and reduced properly instead of being bent onto waveform rows |

## F. Test strategy

| Layer | Targets |
|---|---|
| Parse/elab | `piperine-lang/tests/` — `tol` grammar, constraint grammar, every fail-loud clause, monomorphization carrying the block, both frozen `target` fixtures |
| Lowering/kernel | `piperine-codegen/tests/` — margin sign and normalization, pointwise vs reduced classification, one-kernel-per-declared-margin |
| Solver | `piperine-solver/tests/` — evaluation points (incl. the gmin fixture), postures, the findings channel with a recording element, `EMITS_VALIDATION` gating |
| Host | root `tests/` — `MarginsResult` parity across hosts, SOA-on-model end to end, unedited gallery, `ngspice_validation.rs` unchanged, `compile_once_sweep.rs` unchanged |

## G. Spec-document updates (`docs/spec/`)

The formal spec is normative, so a grammar addition that is not written there is
undocumented language. This feature touches five Parts and one appendix:

| Document | What changes |
|---|---|
| `part_i_language.md` | the reserved-word list (`:232`); a new section for the `constraint` block as a third body kind beside `analog`/`digital`; the `tol` clause on `ParamDecl`; the `require`/`target`/`var` statement forms and the margin convention |
| `appendix_b_grammar.md` | `ParamDecl` grows `TolClause`; new `ConstraintBlock`, `ConstraintStmt`, `RequireStmt`, `TargetStmt`; the `EventBlock` reuse for analysis scoping |
| `part_ii_elaboration.md` | constraint-block resolution; the pointwise/reduced classification; monomorphized variants carrying the block; `tol` distribution resolution against `headers/statistics.phdl`; the fail-loud catalog |
| `part_v_builtins.md` | `headers/statistics.phdl` and `headers/constraints.phdl` as declared stdlib, with each helper's class |
| `part_vii_solver.md` | `validation_reports()` + `ValidationFinding`; `EMITS_VALIDATION`; the evaluation-point table (§C3) as normative solver behavior; the three postures and the `require`-scope default on `Context` |
| `part_viii_host_api.md` + `appendix_c_host_surface.md` | the margins channel (D3) and `r.violations` on result objects, both hosts |
| `part_iv_reflection_selector.md` | whether the selector reaches constraint blocks — decide and document either way |

Two pre-existing problems in that directory, surfaced earlier and **not** this
feature's to fix: `mkdocs build` fails on a missing `material` theme, and
`appendix_c_host_surface.md` + `part_viii_host_api.md` are in neither mkdocs nav.
Both are tracked in `p6-cleanup-architecture`'s deferred list; if the nav gap is
still open when this feature lands, the host-surface documentation it adds will be
invisible to the built site.
