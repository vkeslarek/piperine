# Constraints on VAMS — Context

**Gathered:** 2026-08-17
**Spec:** `.specs/features/constraints-on-vams/spec.md`
**Status:** Ready for design

---

## Feature Boundary

Restore the dv-core constraint/margin surface, re-anchored on Verilog-AMS.
MD-45 (user decision, 2026-08-12) deliberately gutted PHDL's value layer at
T21, taking `constraint Mod { require/var/target/cover }` with it, and
recorded the plan to rebuild it later on VAMS. This feature is that rebuild.

The frontend is entirely dead today (no grammar production, not a keyword,
`pom::ConstraintBlock`/`QuantityClass`/`Module::constraints()` deleted,
`resolve/pom/body.rs` sets `LoweredBody::constraints = None` permanently).
The **backend survives and was deliberately preserved for exactly this
re-anchor** — reconnecting to it, rather than rewriting it, is the shape of
this feature.

---

## Implementation Decisions

### Syntax — a `constraint begin … end` block inside the module

The user's call, and it supersedes MD-45's own tentative "rebuild on
`(* attribute *)`" wording: constraints return as a **block**, but spelled
the way Verilog-AMS spells its other bodies — `constraint begin … end`,
exactly mirroring `analog begin … end`.

```verilog-ams
module mos1 (d, g, s, b);
    inout electrical d, g, s, b;
    parameter real vgs_max = 1e99;

    analog begin
        // … device behavior …
    end

    constraint begin
        require vgs_ox : abs(V(g, s)) <= vgs_max;
        require vds_ox : abs(V(d, s)) <= vds_max;
    end
endmodule
```

Why this over the alternatives that were on the table:

- **vs. PHDL's original `constraint <ModuleName> { … }`** (a block *outside*
  the module, attaching by name): the inside-the-module form is
  VAMS-idiomatic — it sits beside `analog`/`initial`/`always` as one more
  module body — and it needs no name-matching indirection, because ports,
  parameters, and branches are simply in scope.
- **vs. `$require("name", pred)` system tasks**: technically the cheapest
  (verified during discussion: `Stmt::TaskCall` already carries arbitrary
  expressions, so it would need zero grammar change, and it stays portable
  to other simulators, which ignore unknown tasks). Rejected on ergonomics —
  the user explicitly wants the block form back, and constraints
  interleaved into the analog body read worse than a dedicated section.
- **vs. `(* attr = "predicate string" *)`**: rejected. Attribute values go
  through const-eval, so a runtime predicate would have to travel as an
  opaque string needing its own mini-parser — losing type checking, hover,
  go-to-definition, and turning syntax errors into runtime surprises.

### Severity — anchored on the LRM's own assertion model

Researched at the user's direction ("olha primeiro como é feito lá e como
fica mais idiomático") before fixing the semantics, and it changed the
design for the better. **Verilog-AMS already has a validation vocabulary,
and the LRM's own word for it is *assertion*** — §9.7.3's grammar production
is literally `assert_severity_task`, covering `$fatal`/`$error`/`$warning`/
`$info`.

Critically, the LRM already specifies the hard part:

> "Non-fatal system severity tasks (`$error`, `$warning`, `$info`) called
> during a rejected iteration shall have no effect. `$fatal` terminates the
> simulation without checking whether the iteration would be rejected."

That is exactly the subtlety a margin checker must get right — a violation
observed inside a Newton iteration that is subsequently thrown away must not
be reported — and it is precisely where a naive implementation would be
wrong. Anchoring on the LRM inherits it instead of re-deriving it.

**Decision:** the three postures map onto the LRM's severity model rather
than standing beside it.

| Posture | LRM behavior |
|---|---|
| `strict` (default) | violated `require` behaves as `$error` — run-time error; simulation does not proceed past initialization |
| `collect` | behaves as `$warning` — recorded in the findings channel, run continues |
| `off` | emits nothing, zero cost |

The rejected-iteration rule applies as the LRM states it, so margins are
only reported from accepted iterations.

### Scope — the core first

`require` + the margin convention + the three postures. Specifically in:

- `require <name> : <predicate>;` — a named per-point predicate
- The margin convention `m >= 0` ⟺ satisfied
- `tol`-based normalization of the signed margin
- Worst margin + argmin (over time/frequency/sweep, plus instance path)
- The three postures: `strict` (default, aborts), `collect` (records),
  `off` (zero cost)

Deferred to follow-up features (explicitly NOT this one): `var`, `target`,
`cover`, analysis-scoped event blocks (`@ dc`/`@ tran`/`@ ac`), and temporal
windows.

### Failure behavior — fail loud, naming everything

When a constraint references a quantity the running analysis cannot provide
(the canonical case: an AC-defined margin read during a DC run), it is a
**typed, loud error naming the constraint, the quantity, and why it is
unavailable in this analysis**.

Consistent with the project's "fail loud, never silently" rule, and the
original dv-core already carried the machinery for it — `MarginsResult`
still has `not_defined` and `digital` lists for exactly this distinction.

### Agent's Discretion

- How the margin is computed and normalized numerically, and how worst/argmin
  are tracked across sweep points — reconnect to whatever the surviving
  backend types already expect rather than inventing a second convention.
- Whether the `constraint` block is parsed by extending the existing module
  body-item production or by a dedicated one.
- The exact `ElabErrorKind`/error-code assignment for the new failures.

### Declined / Undiscussed Gray Areas → Assumptions

- **`tol` placement.** The user's chosen scope includes tol-based margin
  normalization, but in PHDL `tol <scale>` was a clause on `target`, not on
  `require` — and `target` is deferred. Assumption recorded in the spec:
  `require` gains an optional trailing `tol <scale>` clause, since the
  normalization is what makes margins comparable and the user explicitly put
  it in the core.
- **Whether constraints participate in the `.vsix`/LSP surface** (hover,
  goto on a `require` name). Assumed out of scope for this feature — the
  block will parse and elaborate, and LSP support follows naturally from the
  POM spans but is not an acceptance criterion here.

---

## Specific References

- The original SOA blocks, as the ergonomic target the user liked:
  ```
  pub constraint mos1 {
      require vgs_ox   : abs(V(g, s)) <= model_vgs_max;
      require vgb_ox   : abs(V(g, b)) <= model_vgb_max;
  }
  ```
  (from the pre-MD-45 `headers/spice/mos.phdl`)
- `analog begin … end` is the explicit shape reference for the new block.

---

## Deferred Ideas

- **Parameter distributions (PHDL's `tol gauss(…)`) — the sampling axis.**
  Explicitly deferred by the user, but the research found the LRM already
  supplies the primitives and **piperine already implements them**:
  §9.13.2's `$rdist_normal(seed, mean, stddev)`, `$rdist_uniform`,
  `$random`, and the `$dist_*` family are live in
  `crates/piperine-lang/src/eval/tasks.rs`. So the follow-up that restores
  Monte-Carlo/`monte_carlo_*` coverage does not need new sampling syntax —
  it re-anchors `tol gauss(…)` onto the LRM functions that already exist.
  Recorded here so that follow-up starts from this finding rather than
  re-deriving it.

  **The POM interface must change: a first-class catalogue of random
  variables** (user's direction, recorded here so the follow-up starts from
  it). PHDL modelled uncertainty as a per-parameter clause, and the POM
  still carries the vestige — `Param::distribution` exists but
  `elab/lower/vams.rs` writes `None` unconditionally. A per-parameter
  `Option` is the wrong shape for what Monte Carlo and optimization
  actually need, for four reasons that are all cross-parameter facts:

  1. **Enumeration.** Drawing a sample means drawing a *vector*. With
     per-param clauses that is a hierarchy walk; with a catalogue it is a
     list with stable ordering.
  2. **Correlation and scope.** Real mismatch is not independent per
     instance. dv-core already had both cases as tests — a global parameter
     gets *one* draw shared across instances, a mismatch parameter gets
     *independent* draws per instance. "These two share a draw" cannot be
     expressed in a field that lives on one parameter; it belongs to the
     catalogue.
  3. **Reproducibility.** A stable catalogue index turns a sample into
     `(seed, index) → vector`, which is what makes dv-core's
     "a named failing sample replays as an ordinary session" possible.
  4. **Compile-once (MD-18).** Elaboration fixes devices; sweeps restamp.
     If the catalogue is resolved at elaboration and only its *values* vary
     per draw, an N-sample Monte Carlo is N restamps of one compiled
     circuit — which is exactly what the existing `monte_carlo_compile_once`
     test asserts. This forces the catalogue to be a compile-time structure
     with runtime values, the same shape as the existing
     `SimCtx::param_given_mask`.

  **How this influences optimization** — the catalogue is the missing joint
  between this feature and the optimizer, not a separate concern:

  - **Design vars and random vars are the same POM surface in different
    roles.** The optimizer needs a typed list of knobs with bounds (already
    exposed as `param("r").bounds`); the sampler needs a typed list of
    uncertainties. Catalogue both uniformly and each consumer takes the
    projection it wants.
  - **Design centering is the join.** Maximizing yield is: maximize
    `P(all constraints satisfied)` over design variables, where the
    probability is taken over the random-variable catalogue. That needs
    *three* pieces — the margin (this feature), the distribution (the
    catalogue), and the optimizer (the compile-once `Session`, which
    `README.md` already calls "the foundation the design-centering
    optimizer builds on"). Constraints alone are not enough, and sampling
    alone is not enough.
  - **Gradients get a second, more valuable axis.** dv-gradients computes
    `∂margin/∂param` for design variables. With random variables
    catalogued, `∂margin/∂(random var)` becomes available too — which turns
    "yield is 92%" into "yield is 92%, dominated by M1's Vth mismatch".
    That is the answer a designer can act on; the scalar alone is not.
  - **Ordering consequence.** Because the optimizer consumes the same
    catalogue, its shape should be settled *before* the optimizer work
    hardens around a per-parameter view — otherwise the correlation and
    global/mismatch scoping have to be retrofitted through an API that
    already assumed independence.
- Restoring the SOA constraint blocks on the spice stdlib models
  (mos1/mos2/mos3/diode/bjt) with absent-by-default sentinel limits. The
  user considered it (it was an offered scope option) and chose the core
  first — this is the natural first follow-up once the core lands.
- `var` / `target` / `cover` statements.
- Analysis-scoped event blocks and temporal windows.
- LSP support for constraint names (hover/goto/rename).
