# dv-core Tasks

## Execution Protocol (MANDATORY — do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and
follow its Execute flow and Critical Rules.** Do not search for skill files by
filesystem path. The skill is the source of truth for the full flow (per-task
cycle, sub-agent delegation, adequacy review, Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed without it.**

---

**Spec**: `.specs/features/dv-core/spec.md` (DVC-01..23)
**Design**: `.specs/features/dv-core/design.md`
**Vision**: `.specs/features/design-verification/ideal.md` (decisions D1–D16 binding)
**Status**: Draft — awaiting approval

---

## Test Coverage Matrix

> Generated from codebase sampling and project guidelines — confirm before Execute.
> Guidelines found: `CLAUDE.md` ("Where the tests are", MD-28 placement rule, the
> fail-loud rule, "zero warnings is the bar"), `AGENTS.md` (MD-13 idiom rules),
> `tests/suite_hygiene.rs` (the enforcing guard: no switched-off test code, no
> `#[ignore]`, every target has a `//!` scope header, no file-scope lint
> suppression, no dead-architecture identifiers, `mod.rs` declares only, root-test
> naming).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
|---|---|---|---|---|
| `piperine-lang` parse/elab | integration | 1:1 to spec ACs; **every fail-loud clause has a negative fixture** | `crates/piperine-lang/tests/*.rs` (+ `tests/examples/*.phdl`) | `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p piperine-lang` |
| `piperine-lang` headers (`.phdl`) | integration | every declared name resolves; `extern_coverage_guard.rs` extended | `crates/piperine-lang/tests/*.rs` | same as above |
| `piperine-codegen` lowering/kernel | integration | margin sign/normalization per AC; one kernel per *declared* margin; classification enforced | `crates/piperine-codegen/tests/*.rs` | `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p piperine-codegen` |
| `piperine-solver` ABI + analyses | integration | every evaluation-point rule (design §C3) has a test; all three postures; capability-bit gating | `crates/piperine-solver/tests/*.rs` | `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p piperine-solver` |
| `piperine-api` result surface | integration | pointwise and reduced margin shapes; loud unknown-name paths | `crates/piperine-api/tests/*.rs` | `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p piperine-api` |
| `piperine-python` bindings | integration | every Python-visible name added; parity with Rust | `crates/piperine-python/tests/*.rs` | `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p piperine-python` |
| Host surface / cross-crate (MD-28) | integration | end-to-end SOA-on-model; both-host parity; **gallery and ngspice numerics unchanged** | `tests/*.rs` | `CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace` |
| `docs/spec/` (markdown) | none | build gate only — reviewed against the tree, not tested | `docs/spec/*.md` | build gate only |

**Numeric oracles that must not move** (CLAUDE.md names these by name):
`tests/ngspice_validation.rs` (+ `tests/ngspice/`), `tests/run_examples.rs`,
`tests/compile_once_sweep.rs`, `tests/urc_compile_count.rs`, `tests/host_parity.rs`.

## Gate Check Commands

> Generated from codebase — confirm before Execute.

| Gate Level | When to Use | Command |
|---|---|---|
| Quick | Task touching exactly one crate's internals | `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p <crate>` |
| Full | Task touching the host surface, Python, or more than one crate | `CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace` |
| Build | Phase completion, doc-only tasks, header-only tasks | `CARGO_PROFILE_DEV_DEBUG=0 cargo build --workspace && CARGO_PROFILE_DEV_DEBUG=0 cargo clippy --workspace --all-targets -- -D warnings && CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace && CARGO_PROFILE_DEV_DEBUG=0 cargo doc --workspace --no-deps --exclude piperine-python --exclude piperine-cli` |

**Three binding execution rules, learned the hard way in `p6-cleanup-architecture`:**

1. **`CARGO_PROFILE_DEV_DEBUG=0` is mandatory on every cargo invocation.** Without
   it the workspace test build is ~63 GB across 166 targets and fills the disk.
   Run `df -h /home` before any full gate; stop and report under ~10 GB free.
2. **The doc gate excludes `piperine-python` and `piperine-cli`** — a rustdoc ICE
   inside `numpy 0.23.0` makes them exit 101 at baseline. Pre-existing, not ours.
3. **`cargo test` bare at the repo root only runs the root package.** Always pass
   `--workspace`.

**Baseline to preserve:** capture the pre-feature `cargo test --workspace` count in
T1 and assert it never *drops* — only grows by tests this feature adds.

---

## Execution Plan

Phases run sequentially; tasks within a phase run in order.

### Phase 1: Language surface — keywords and `tol`
```
T1 → T2 → T3 → T4 → T5
```

### Phase 2: The `constraint` block — grammar and POM
```
T6 → T7 → T8 → T9 → T10
```

### Phase 3: The ABI findings channel
```
T11 → T12 → T13
```

### Phase 4: Margins — lowering, kernel, evaluation
```
T14 → T15 → T16 → T17
```

### Phase 5: Evaluation points
```
T18 → T19 → T20
```

### Phase 6: Host surface
```
T21 → T22 → T23 → T24
```

### Phase 7: SOA on the shipped models
```
T25 → T26 → T27
```

### Phase 8: Scoping
```
T28 → T29
```

### Phase 9: The formal spec
```
T30 → T31 → T32
```

---

## Task Breakdown

### T1: Baseline + reserve the six keywords

**What**: Capture the pre-feature test baseline, then add
`constraint`/`require`/`target`/`tol`/`cover`/`global` to the parser's reserved
words and rename the two existing uses of `target` as a variable.
**Where**: `crates/piperine-lang/src/parse/`, `crates/piperine-lang/tests/examples/ring_oscillator.phdl`, `crates/piperine-lang/tests/examples/oscillator.phdl`, `.specs/features/dv-core/baseline.md`
**Depends on**: None
**Reuses**: the existing reserved-word list in the lexer
**Requirement**: DVC-06

**Done when**:
- [ ] `baseline.md` records the exact per-crate and workspace test counts, the clippy result, and the doc-warning count for the four documenting crates
- [ ] the six words are reserved; a negative fixture per word proves each is rejected as an identifier
- [ ] `ring_oscillator.phdl:5` and `oscillator.phdl:16` renamed (`target` → `v_target`), and **their numerics are identical before and after** — a rename may not move a waveform
- [ ] `cargo test --workspace` at baseline + the new negative fixtures

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang)!: reserve the verification keywords`

---

### T2: `headers/statistics.phdl` — declared distributions

**What**: Declare the statistical distributions as `extern fn` so `tol` resolves a
name instead of a magic string (MD-24).
**Where**: `crates/piperine-lang/headers/statistics.phdl`, `crates/piperine-lang/tests/extern_coverage_guard.rs`
**Depends on**: None
**Reuses**: `headers/math.phdl`'s `extern fn` shape; the `CallableRegistry`
**Requirement**: DVC-02

**Done when**:
- [ ] `gauss(sigma)`, `gauss(sigma_rel)`, `uniform(half)`, `uniform(rel)` declared with the argument spelling the spec uses
- [ ] `extern_coverage_guard.rs` covers the new header — an undeclared distribution name cannot pass
- [ ] a fixture referencing an undeclared distribution fails loud naming it

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): declare the statistical distributions`

---

### T3: `tol` clause — parse and AST

**What**: `ParamDecl` grows an optional `TolClause` (`tol <dist>(args) [global]`).
**Where**: `crates/piperine-lang/src/parse/`
**Depends on**: T1, T2
**Reuses**: the attribute-argument parser's keyed-argument shape
**Requirement**: DVC-01

**Done when**:
- [ ] `param vto : Real = 0.7 tol gauss(sigma = 0.005);` parses, with and without `global`
- [ ] the clause is optional everywhere a `param` appears
- [ ] a malformed clause fails loud pointing at the offending token
- [ ] round-trips through `piperine fmt` unchanged

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): parse the tol clause on param`

---

### T4: `tol` elaboration → `Param.distribution()`

**What**: Resolve the distribution against the header, evaluate its args as
elaboration constants in module param scope, and expose it on the POM `Param`.
**Where**: `crates/piperine-lang/src/elab/`, `crates/piperine-lang/src/pom/`
**Depends on**: T3
**Reuses**: parameter-default evaluation (already elaboration-constant)
**Requirement**: DVC-02

**Done when**:
- [ ] `Param.distribution()` returns the kind, its evaluated args, and the process-vs-mismatch marker
- [ ] `sigma = 0.005 / sqrt(w * l)` referencing sibling params evaluates correctly
- [ ] `tol` on an `Integer`/`Natural` param fails loud
- [ ] an undeclared distribution fails loud (proves T2's wiring reaches elaboration)

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): expose param distributions on the POM`

---

### T5: `tol` is inert at solve

**What**: Prove the declaration changes no numerics and interacts correctly with
staging.
**Where**: `crates/piperine-lang/tests/`, `tests/`
**Depends on**: T4
**Reuses**: an existing gallery circuit as the untoleranced twin
**Requirement**: DVC-03

**Done when**:
- [ ] a design with `tol` clauses produces **bit-identical** `op()`/`tran()`/`ac()` results to the same design without them
- [ ] a param carrying both `tol` and a staged override: the override wins for the nominal value and the distribution stays attached
- [ ] no `Invalidation` class changed by the presence of a `tol`

**Tests**: integration · **Gate**: full
**Commit**: `test(lang): prove tol declarations are inert at solve`

---

### T6: `constraint` block — parse and AST

**What**: A new top-level item, sibling of `analog`/`digital`, holding
`require`/`var`/`target` statements.
**Where**: `crates/piperine-lang/src/parse/`
**Depends on**: T1
**Reuses**: the `analog`/`digital` block parser's shape; the existing `var` statement parser
**Requirement**: DVC-04

**Done when**:
- [ ] `require <name> : <pred>;`, `var <name> : <Type> = <expr>;`, `target <expr> <cmp> <level> [tol <scale>];` all parse
- [ ] `var` **requires** its type annotation, exactly as every other `var` in the language does
- [ ] `piperine fmt` round-trips a constraint block unchanged
- [ ] each malformed statement form fails loud at the offending token

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): parse the constraint block`

---

### T7: `headers/constraints.phdl` — helpers with their class

**What**: Declare the constraint helpers, each marked **pointwise** or **reduced**
— the distinction that decides whether it can lower to a per-point kernel.
**Where**: `crates/piperine-lang/headers/constraints.phdl`, `crates/piperine-lang/tests/extern_coverage_guard.rs`
**Depends on**: T2
**Reuses**: `headers/math.phdl`'s declaration shape
**Requirement**: DVC-09

**Done when**:
- [ ] pointwise helpers declared (`ac_gain`, …) and reduced helpers declared (`ac_unity_gain_freq`, `ac_phase_margin`, …), each carrying its class
- [ ] the class is readable by the elaborator, not folklore in a comment
- [ ] `extern_coverage_guard.rs` covers the header
- [ ] an undeclared helper in a constraint fails loud

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): declare the constraint helpers and their classes`

---

### T8: Constraint elaboration — resolution and type checks

**What**: Resolve every reference, type-check predicates and comparisons, and fail
loud on each spec'd violation.
**Where**: `crates/piperine-lang/src/elab/`
**Depends on**: T6, T7
**Reuses**: the existing name-resolution pass and instance-port resolution (`tests/examples/sar_adc.phdl:29` already resolves `dac.out`)
**Requirement**: DVC-04, DVC-07

**Done when**:
- [ ] unknown net/port/instance-path/`var` → loud, naming the reference **and** the use site
- [ ] a child instance's **port** net (`V(m1.d, m1.s)`) resolves through the authored tree
- [ ] a child's **internal** node → loud (it crosses encapsulation and depends on optional parameters)
- [ ] non-Boolean `require` predicate → loud; non-`Real` `target` operand → loud; missing `var` annotation → loud
- [ ] duplicate statement label in one block → loud
- [ ] one negative fixture per clause above

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): elaborate and validate constraint blocks`

---

### T9: POM `Module::constraints`, carried by monomorphization

**What**: Expose the authored block on the POM and make monomorphized variants
carry it as they carry their behavior bodies.
**Where**: `crates/piperine-lang/src/pom/`, `crates/piperine-lang/src/elab/`
**Depends on**: T8
**Reuses**: however `analog`/`digital` bodies are already carried into variants
**Requirement**: DVC-05

**Done when**:
- [ ] `Module::constraints` returns the block in **authored** form (UNBREAKABLE rule — no flattened structure leaks)
- [ ] `Dac[8]` → `Dac__8` carries the block; the POM still holds exactly one authored block per authored module
- [ ] a selector walk reaches the block on both the authored module and a variant
- [ ] no accessor added here reads `flat_modules`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): carry constraint blocks through monomorphization`

---

### T10: Pointwise/reduced classification is enforced

**What**: Classify each `var`/`target` expression from its helpers' declared
classes and reject the impossible combination.
**Where**: `crates/piperine-lang/src/elab/`
**Depends on**: T7, T8
**Reuses**: T7's declared classes
**Requirement**: DVC-09

**Done when**:
- [ ] every `var`/`target` carries its class in the POM
- [ ] a `require`/`target` reading a **reduced** quantity inside a pointwise scope fails loud with the spec's wording ("a reduced quantity is checkable once per analysis, not per point")
- [ ] a mixed expression (pointwise arithmetic over a reduced value) is classified reduced, not pointwise
- [ ] the analysis a helper belongs to is recorded, so a cross-analysis read can be refused later (DVC-08 AC6)

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `feat(lang): classify constraint quantities pointwise or reduced`

---

### T11: `ValidationFinding` + `validation_reports()` on `Element`

**What**: The findings type and the cross-cutting ABI hook, shaped after
`limiting_report()`.
**Where**: `crates/piperine-solver/src/core/`, `crates/piperine-solver/src/core/element.rs`
**Depends on**: None (parallel to Phases 1–2, but sequenced here for batch cohesion)
**Reuses**: `limiting_report()` at `core/element.rs:173` — `Option`, not `Vec`
**Requirement**: DVC-13

**Done when**:
- [ ] `ValidationFinding { severity: Warning|Error, label, message, value, time, instance_path }` and a `ValidationReport` container
- [ ] `fn validation_reports(&self) -> Option<ValidationReport> { None }` on `Element`, **not** `AnalogDevice`-gated
- [ ] a recording test element emits one `Warning` and one `Error` at known times and both are readable through the hook
- [ ] `abi.rs`/`prelude.rs` export the new types per MD-17's two-tier surface

**Tests**: integration · **Gate**: quick
**Commit**: `feat(solver): add the validation findings channel`

---

### T12: `EMITS_VALIDATION` gates the polling

**What**: The capability bit, and the analysis-side polling that only asks elements
which declare it.
**Where**: `crates/piperine-solver/src/core/element.rs`, `crates/piperine-solver/src/analyses/`
**Depends on**: T11
**Reuses**: `EMITS_NOISE`'s gating pattern; bits are `u32` with `NUMERIC_JACOBIAN = 1 << 14` highest used
**Requirement**: DVC-14

**Done when**:
- [ ] `EMITS_VALIDATION = 1 << 15`
- [ ] an element **without** the bit is never polled — proven by a counting test element, not by inspection
- [ ] an element with the bit that always returns `None` is legal and costs one poll
- [ ] a circuit with no validating elements shows no measurable per-step cost change

**Tests**: integration · **Gate**: quick
**Commit**: `feat(solver): gate validation polling on a capability bit`

---

### T13: Postures and the `require` scope default

**What**: `checks: Strict|Collect|Off` and `require_scope` on `Context`, mirrored on
both hosts.
**Where**: `crates/piperine-solver/src/analyses/context.rs`, `crates/piperine-api/src/session/`, `crates/piperine-python/src/`
**Depends on**: T11
**Reuses**: the canonical `Solver`/`SolverConfig` field set (HOST-20 parity)
**Requirement**: DVC-12
**Note**: `analyses/context.rs` is where `p6-cleanup-architecture` T15 moved `Context`.

**Done when**:
- [ ] `checks` defaults to `Strict`; `require_scope` defaults to "every analysis" (D2)
- [ ] both fields readable and settable identically on Python `Solver` and Rust `SolverConfig`/`Context`
- [ ] changing the posture between analyses in one session triggers **no** recompilation
- [ ] `host_parity.rs` covers the new fields

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `feat(api): add the check posture and require-scope knobs`

---

### T14: Margin lowering

**What**: Lower each comparison to the signed, scale-normalized margin expression.
**Where**: `crates/piperine-codegen/src/resolve/pom/`
**Depends on**: T10
**Reuses**: the existing POM→resolved expression lowering
**Requirement**: DVC-08

**Done when**:
- [ ] `a <= b` → `(b−a)/scale`; `a >= b` → `(a−b)/scale`; `a in [l,u]` → `min(u−a, a−l)/scale`
- [ ] `scale = 1` when no `tol` is given — no implicit normalization
- [ ] `m ≥ 0` ⟺ satisfied holds for every form, asserted on values, not by inspection
- [ ] a compound predicate (`&&`) lowers to the min of its parts' margins

**Tests**: integration · **Gate**: quick
**Commit**: `feat(codegen): lower comparisons to signed margins`

---

### T15: The margin kernel

**What**: Emit one JIT function per **declared** margin, parameterized by instance
— never one per instance × margin.
**Where**: `crates/piperine-codegen/src/kernel/analog/constraints.rs` (new)
**Depends on**: T14
**Reuses**: the `Option` capability sub-struct pattern of `forces.rs`/`limits.rs`; CSE against the residual
**Requirement**: DVC-10
**Risk**: this is where `.disto`'s function-count explosion (`ROADMAP.md` P1, `TryFromIntError` on MOS2/MOS3) would repeat if the emission is per-instance.

**Done when**:
- [ ] one function per declared margin; a circuit with 100 instances of a constrained model emits the same number of functions as one instance
- [ ] margin kernels CSE against the residual that already computes their subexpressions
- [ ] a circuit with no constraint block emits nothing and allocates nothing (`Option` stays `None`)
- [ ] only **pointwise** margins reach the kernel; reduced ones do not

**Tests**: integration · **Gate**: quick
**Commit**: `feat(codegen): emit margin kernels behind an option`

---

### T16: Margin evaluation with provenance

**What**: Evaluate the kernels per point and track worst value + argmin per
constraint per instance.
**Where**: `crates/piperine-codegen/src/device/analog/constraints.rs` (new)
**Depends on**: T15
**Reuses**: `device/analog/limits.rs`'s structure
**Requirement**: DVC-10

**Done when**:
- [ ] worst margin and its argmin (time, or frequency, plus instance path) tracked per constraint
- [ ] a non-finite margin at an evaluated point is reported as a violation, never clamped
- [ ] the reported instance path is the authored hierarchical path, not a flattened label
- [ ] `checks=off` means the kernels are not called at all

**Tests**: integration · **Gate**: quick
**Commit**: `feat(codegen): evaluate margins and track their argmin`

---

### T17: Findings from margin crossings, and posture behavior

**What**: A `require` margin crossing zero emits an `Error` finding; the posture
decides abort vs record.
**Where**: `crates/piperine-codegen/src/device/analog/constraints.rs`, `crates/piperine-solver/src/analyses/`
**Depends on**: T12, T13, T16
**Reuses**: T11's channel
**Requirement**: DVC-15

**Done when**:
- [ ] `strict` + `m < 0` → the analysis fails loud naming constraint, instance path, time, and margin value
- [ ] `collect` → the analysis completes and records the worst margin with argmin
- [ ] `off` → nothing is called and nothing is reported
- [ ] a `Warning` finding never aborts, even in `strict`
- [ ] a divider fixture exercises all three postures against the same violated `require`

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `feat(solver): fail loud on require violations under strict posture`

---

### T18: DC evaluation points — final solution only

**What**: Evaluate margins at the converged DC solution and **never** at a homotopy
stage.
**Where**: `crates/piperine-solver/src/analyses/dc.rs`, `crates/piperine-solver/src/analyses/convergence.rs`
**Depends on**: T17
**Reuses**: `ConvergencePlan`/`HomotopyStrategy`'s existing stage structure
**Requirement**: DVC-11

**Done when**:
- [ ] gmin-stepping and source-stepping intermediate solutions are never evaluated
- [ ] a fixture that **requires** gmin stepping and whose intermediate stages violate a `require` passes in `strict` — this is the acceptance test that the rule is real
- [ ] the final converged solution is evaluated exactly once

**Tests**: integration · **Gate**: quick
**Commit**: `feat(solver): evaluate DC margins only at the converged solution`

---

### T19: Transient evaluation points — accepted steps, and UIC

**What**: Evaluate only on accepted steps; skip `t = 0` when the state came from
UIC/`@initial`.
**Where**: `crates/piperine-solver/src/analyses/transient.rs`
**Depends on**: T18
**Reuses**: `accept_timestep` (`core/element.rs:482`), `SUPPORTS_ROLLBACK`
**Requirement**: DVC-11

**Done when**:
- [ ] a rejected step evaluates no margin and emits no finding — proven on a fixture that provokes rejection
- [ ] under UIC, `t = 0` is skipped; with a computed OP, `t = 0` is evaluated
- [ ] the same fixture run with and without UIC differs only in that first point

**Tests**: integration · **Gate**: quick
**Commit**: `feat(solver): evaluate transient margins on accepted steps only`

---

### T20: AC points and worst-across-sweep

**What**: Per-frequency evaluation for AC, and a worst-across-sweep reduction whose
argmin carries the swept coordinate.
**Where**: `crates/piperine-solver/src/analyses/ac.rs`, `crates/piperine-api/src/session/`
**Depends on**: T19
**Reuses**: the existing `sweep`/`sweep_grid` drivers
**Requirement**: DVC-11
**Note**: this is the row the optimizer, corner runs, and Monte Carlo in `dv-gradients` all consume — the shape matters beyond this feature.

**Done when**:
- [ ] AC evaluates pointwise margins at each frequency point, with frequency as the argmin coordinate
- [ ] a `sweep`/`sweep_grid` result carries the worst margin across swept points **and** the swept coordinate at which it occurred
- [ ] the DC solve underneath an AC analysis is not double-counted

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `feat(api): report the worst margin across a sweep`

---

### T21: `MarginsResult` — the margin channel

**What**: A result channel carrying both margin shapes, separate from waveform rows
(D3).
**Where**: `crates/piperine-api/src/`
**Depends on**: T20
**Reuses**: the existing result-object family (`results.rs`)
**Requirement**: DVC-18

**Done when**:
- [ ] a **pointwise** margin carries worst + argmin (time/frequency/swept coordinate) + instance path
- [ ] a **reduced** margin carries one value and **no** argmin, and the channel states which kind it is — no invented `t = 0`
- [ ] margins do not appear as waveform rows
- [ ] `r.requires_ok` and `r.first_violation` are available for the common assertion shape

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): add the margins result channel`

---

### T22: Host-side reductions for reduced metrics

**What**: Compute reduced quantities (`ac_unity_gain_freq`, `ac_phase_margin`) on
the host from the points the solver returns (D7).
**Where**: `crates/piperine-api/src/`
**Depends on**: T21
**Reuses**: the existing waveform/measure machinery
**Requirement**: DVC-09
**Note**: the solver stays pointwise-pure; this is the reason it can.

**Done when**:
- [ ] each reduced helper declared in T7 has a host implementation
- [ ] a reduction over an AC sweep produces the analytically expected value on a known single-pole fixture
- [ ] a reduction over too few points fails loud rather than extrapolating
- [ ] no sweep-shaped state was added to the solver

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): compute reduced constraint metrics host-side`

---

### T23: Findings and loud reads on the host surface

**What**: `r.violations`, and loud failure with candidate listing for unknown names.
**Where**: `crates/piperine-api/src/`
**Depends on**: T21
**Reuses**: the `UnknownNet`/`Error::Measurement` error family
**Requirement**: DVC-15, DVC-18

**Done when**:
- [ ] `r.violations` carries every finding with severity and provenance
- [ ] an unknown constraint or `var` name fails loud **listing candidates**
- [ ] a `var` read from an analysis where it is not defined fails loud with "not defined in this analysis"
- [ ] a `require` over a digital quantity has no margin and asking for one is loud (margins are an analog notion)

**Tests**: integration · **Gate**: quick
**Commit**: `feat(api): expose validation findings on results`

---

### T24: Python parity for the whole surface

**What**: Bind margins, violations, and the postures with identical names, and
extend the parity guard.
**Where**: `crates/piperine-python/src/`, `tests/host_parity.rs`
**Depends on**: T21, T22, T23
**Reuses**: `host_parity.rs`'s enumeration mechanism; the delegation style of the lifted model
**Requirement**: DVC-18

**Done when**:
- [ ] every name added in Phase 6 exists on both hosts with identical spelling, shape, and values
- [ ] `host_parity.rs` enumerates the new surface — parity is guarded, not asserted in prose
- [ ] one fixture driven through both hosts asserts identical margins, argmins, and violations

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `feat(python): bind the margin and findings surface`

---

### T25: `enum Region` + the operating-region opvar

**What**: A declared enum and a region opvar on the MOS models, through the
existing introspection surface.
**Where**: `crates/piperine-lang/headers/spice/constants.phdl`, `crates/piperine-lang/headers/spice/mos.phdl`
**Depends on**: T10
**Reuses**: `headers/prelude.phdl:13`'s `enum Scale { Lin, Dec, Oct }` shape; the `@opvar` attribute
**Requirement**: DVC-17

**Done when**:
- [ ] `enum Region { Cutoff, Triode, Saturation }` declared — not integer codes
- [ ] `m1.region` readable from a constraint and from the host
- [ ] the region matches ngspice's own region classification on a swept fixture
- [ ] no numeric behavior of the model changed by adding the opvar

**Tests**: integration · **Gate**: quick
**Commit**: `feat(spice): expose the MOS operating region as an enum opvar`

---

### T26: `constraint Mos1` with absent-by-default limits

**What**: SOA requires on the MOS model, reading limits that default to absent so
the check is inert until a model card sets them.
**Where**: `crates/piperine-lang/headers/spice/mos.phdl`
**Depends on**: T17, T25
**Reuses**: `bundle Mos1Model`'s parameter style; PHDL's optional-type marker
**Requirement**: DVC-16
**Critical**: the shipped models are **ngspice-faithful** and ngspice has no SOA data. Inventing a limit would fabricate an unsupported number and break the faithfulness contract.

**Done when**:
- [ ] `vgs_max`/`vds_max` and the bulk limits default to absent; an unset limit makes its `require` **vacuously satisfied**
- [ ] the **entire unedited gallery** elaborates and simulates unchanged in default `strict` posture
- [ ] `tests/ngspice_validation.rs` numerics are byte-identical
- [ ] a model card that **sets** `vds_max`, driven past it, fails loud naming the device instance and the constraint

**Tests**: integration · **Gate**: full
**Commit**: `feat(spice): ship SOA constraints on the MOS models`

---

### T27: Diode and BJT blocks, and the no-regression proof

**What**: The same pattern on the remaining junction models, plus the end-to-end
SOA-on-model proof.
**Where**: `crates/piperine-lang/headers/spice/diode.phdl`, `crates/piperine-lang/headers/spice/bjt.phdl`, `tests/`
**Depends on**: T26
**Reuses**: T26's pattern exactly
**Requirement**: DVC-16

**Done when**:
- [ ] diode and BJT carry their own constraint blocks with absent-by-default limits
- [ ] a root target proves SOA-on-model end to end with **zero user-written constraint code**
- [ ] gallery + `run_examples.rs` + `ngspice_validation.rs` all unchanged
- [ ] `compile_once_sweep.rs` and `urc_compile_count.rs` unchanged (MD-18 intact)

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `feat(spice): extend SOA constraints to the junction models`

---

### T28: Analysis-scoped event blocks

**What**: `@ dc { … }` / `@ tran` / `@ ac` / `@ (dc | tran)` inside a constraint
body, through the existing `EventBlock` production and `EventRegistry`.
**Where**: `crates/piperine-lang/src/parse/`, `crates/piperine-lang/src/elab/event.rs`, `crates/piperine-codegen/src/device/analog/constraints.rs`
**Depends on**: T17
**Reuses**: the `EventBlock` production; `EventRegistry` (which already resolves `cross`/`above`/`timer`)
**Requirement**: DVC-19

**Done when**:
- [ ] `dc`/`tran`/`ac` resolve as event terms through the **existing** registry, not a new one
- [ ] a scoped `require` is evaluated only in its analyses; `|` composes
- [ ] a top-level `require` holds everywhere, subject to `Context::require_scope`
- [ ] a scoped require whose analysis never ran reports **not exercised** — never a vacuous pass
- [ ] a fixture whose `@ dc` rule is violated during transient startup: strict tran passes, strict dc fails

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): scope constraints by analysis with event blocks`

---

### T29: Event windows and the window algebra

**What**: `after =`/`dur =` named arguments on event terms, and `|`/`&`/`not` over
evaluation-point sets.
**Where**: `crates/piperine-lang/src/parse/`, `crates/piperine-lang/src/elab/event.rs`, `crates/piperine-codegen/src/device/analog/constraints.rs`
**Depends on**: T28
**Reuses**: `timer(period, phase)`'s existing second argument as the named-argument precedent
**Requirement**: DVC-20 (P3)

**Done when**:
- [ ] `after` accepts a time or an event term; `dur` closes the window
- [ ] `&` and `not` compose as set intersection and complement
- [ ] a settling fixture evaluates its margin only inside the window
- [ ] a misspelled or never-firing trigger reports not-exercised rather than passing

**Tests**: integration · **Gate**: build (phase completion)
**Commit**: `feat(lang): add temporal windows to constraint scoping`

---

### T30: `docs/spec/` — language and grammar

**What**: Document the two grammar additions normatively.
**Where**: `docs/spec/part_i_language.md`, `docs/spec/appendix_b_grammar.md`
**Depends on**: T29
**Reuses**: the existing Part I structure and the appendix's EBNF style
**Requirement**: DVC-23

**Done when**:
- [ ] the six reserved words added to the list at `part_i_language.md:232`
- [ ] a Part I section for the `constraint` block as a third body kind, the three statement kinds, and the margin convention
- [ ] the `tol` clause documented on `ParamDecl`
- [ ] appendix B carries `TolClause`, `ConstraintBlock`, `ConstraintStmt`, `RequireStmt`, `TargetStmt`, and the `EventBlock` reuse
- [ ] every production matches the parser as implemented — verified against the code, not from the spec draft

**Tests**: none · **Gate**: build
**Commit**: `docs(spec): document the constraint block and tol clause`

---

### T31: `docs/spec/` — elaboration and builtins

**What**: Document how constraints elaborate and what the new headers declare.
**Where**: `docs/spec/part_ii_elaboration.md`, `docs/spec/part_v_builtins.md`
**Depends on**: T30
**Reuses**: Part II's resolution-order structure
**Requirement**: DVC-23

**Done when**:
- [ ] constraint resolution, the pointwise/reduced classification, and monomorphized carriage documented
- [ ] `tol` distribution resolution against `headers/statistics.phdl` documented
- [ ] the complete fail-loud catalog for this feature listed in one place
- [ ] Part V documents `headers/statistics.phdl` and `headers/constraints.phdl`, including each helper's class

**Tests**: none · **Gate**: build
**Commit**: `docs(spec): document constraint elaboration and the new headers`

---

### T32: `docs/spec/` — solver and host surface

**What**: Document the ABI channel, the evaluation-point rules as normative solver
behavior, and the host margin surface.
**Where**: `docs/spec/part_vii_solver.md`, `docs/spec/part_viii_host_api.md`, `docs/spec/appendix_c_host_surface.md`
**Depends on**: T31
**Reuses**: Part VII's numbered-section structure
**Requirement**: DVC-23

**Done when**:
- [ ] `validation_reports()`, `ValidationFinding`, and `EMITS_VALIDATION` documented in the ABI section
- [ ] the **evaluation-point table** (design §C3) written as normative behavior — including that homotopy stages, rejected steps, and UIC `t = 0` are excluded
- [ ] the three postures and `require_scope` documented on `Context`
- [ ] the margins channel and `r.violations` documented for both hosts
- [ ] a note that `part_viii_host_api.md` and `appendix_c_host_surface.md` are in neither mkdocs nav (`p6-cleanup-architecture` deferred item) — if still open, this documentation is unpublished

**Tests**: none · **Gate**: build
**Commit**: `docs(spec): document the validation channel and margin surface`

---

## Validation Tables

### Check 1: Task granularity

Every task is one deliverable in one area: a parse addition, a header, an
elaboration pass, one kernel file, one device file, one analysis's evaluation
points, one result channel, one model's constraint block, or one pair of spec
documents. No task spans two crates except where the deliverable is inherently a
seam (T13 posture parity, T17 findings wiring, T24 host parity) — each of those is
still a single concept.

### Check 2: Diagram ↔ `Depends on` cross-check

| Task | Diagram predecessor | `Depends on` | ✅ |
|---|---|---|---|
| T1 | — | None | ✅ |
| T2 | T1 | None (independent header; ordered for cohesion) | ✅ |
| T3 | T2 | T1, T2 | ✅ |
| T4 | T3 | T3 | ✅ |
| T5 | T4 | T4 | ✅ |
| T6 | T5 (phase order) | T1 | ✅ |
| T7 | T6 | T2 | ✅ |
| T8 | T7 | T6, T7 | ✅ |
| T9 | T8 | T8 | ✅ |
| T10 | T9 | T7, T8 | ✅ |
| T11 | T10 (phase order) | None (ABI is independent of the grammar) | ✅ |
| T12 | T11 | T11 | ✅ |
| T13 | T12 | T11 | ✅ |
| T14 | T13 (phase order) | T10 | ✅ |
| T15 | T14 | T14 | ✅ |
| T16 | T15 | T15 | ✅ |
| T17 | T16 | T12, T13, T16 | ✅ |
| T18 | T17 | T17 | ✅ |
| T19 | T18 | T18 | ✅ |
| T20 | T19 | T19 | ✅ |
| T21 | T20 | T20 | ✅ |
| T22 | T21 | T21 | ✅ |
| T23 | T22 | T21 | ✅ |
| T24 | T23 | T21, T22, T23 | ✅ |
| T25 | T24 (phase order) | T10 | ✅ |
| T26 | T25 | T17, T25 | ✅ |
| T27 | T26 | T26 | ✅ |
| T28 | T27 (phase order) | T17 | ✅ |
| T29 | T28 | T28 | ✅ |
| T30 | T29 | T29 | ✅ |
| T31 | T30 | T30 | ✅ |
| T32 | T31 | T31 | ✅ |

Tasks whose `Depends on` is looser than their diagram position (T2, T6, T11, T25,
T28) are ordered for **batch cohesion**, not necessity — a worker may reorder
within its phase if a real dependency is not violated.

### Check 3: Test co-location

| Task | Layer touched | Matrix requires | Task's `Tests` | ✅ |
|---|---|---|---|---|
| T1 | lang parse + examples | integration | integration | ✅ |
| T2, T7 | lang headers | integration | integration | ✅ |
| T3, T4, T6, T8, T9, T10 | lang parse/elab | integration | integration | ✅ |
| T5 | cross-crate numerics | integration | integration | ✅ |
| T11, T12, T13 | solver ABI + api/python | integration | integration | ✅ |
| T14 | codegen resolve | integration | integration | ✅ |
| T15, T16 | codegen kernel/device | integration | integration | ✅ |
| T17, T18, T19, T20 | solver analyses | integration | integration | ✅ |
| T21, T22, T23 | api result surface | integration | integration | ✅ |
| T24 | python + host parity | integration | integration | ✅ |
| T25, T26, T27 | lang headers + cross-crate | integration | integration | ✅ |
| T28, T29 | lang + codegen | integration | integration | ✅ |
| T30, T31, T32 | `docs/spec/` markdown | none (build gate) | none | ✅ |

---

## Tools

**MCP**: none required. **Skill**: `tlc-spec-driven` (mandatory, per the Execution
Protocol above).

Confirm before Execute: no task in this plan needs an MCP server or a second
skill. If that is wrong for your environment, say so at approval time.
