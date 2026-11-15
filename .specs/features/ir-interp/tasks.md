# IR Interpreter (reference backend, M1 subset) Tasks

**Approved**: 2026-11-15 - user: "Ok, deixo tu ir tocando, pode ir disparando implementações em seguida ok?"

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/ir-interp/design.md`
**Status**: Approved
**Build**: `cargo build -p piperine-abi --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo build -p piperine-solver --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `test ! -d crates/piperine-interp || cargo build -p piperine-interp --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`
**Run factory**: `cargo test -p piperine-abi --test factory`
**Run lib**: `cargo test -p piperine-interp --lib`
**Run shape**: `cargo test -p piperine-interp --test shape`
**Run terms**: `cargo test -p piperine-interp --test terms`
**Run limit**: `cargo test -p piperine-interp --test limit`
**Run stim**: `cargo test -p piperine-interp --test stim`
**Run circuit**: `cargo test -p piperine-interp --test circuit`
**Run partials**: `cargo test -p piperine-interp --test partials`

Executors: Opus implements T1 and T5 to T9 (the one task with `unsafe`, and the tasks that decide numbers: the shape plan, the `Terms` walk, the affine rule, `limit`); Sonnet implements T2 to T4 and T10 to T12; a different Opus verifies the feature. The worktree and branch are told at dispatch. This feature runs in wave 2: it needs `msl-ir` merged into the worktree first (the crate `piperine-ir` and `crates/piperine-ir/tests/support/devices.rs`). It does not need `piperine-lang`: every test runs on IR built by hand.

Rules every task follows:

- **abi.md is the contract** (`.specs/features/device-abi/abi.md`). The interpreter meets the solver through shapes and `eval` only. `piperine-interp` holds no `unsafe`; the only `unsafe` this feature writes is in `crates/piperine-abi/src/thunk.rs` (T1).
- **`piperine-abi`'s existing surface does not change.** `Device`, `module::<D>`, every existing test of `piperine-abi` and `piperine-solver` stay as they are and green (INT-08 AC 4). No solver crate is edited: a defect found there is reported, not patched here.
- **Shapes follow the mapping table of `design.md`** ("From a body to a shape"): pin order, term names, function order, read order. The comments above `R`, `C`, `Vsrc`, `Iac` in `crates/piperine-solver/tests/helpers/abi_devices.rs` are the reference shapes.
- **Partials are exact.** Every function of every M1 body passes `piperine_abi::verify::verify_partials` with `rel_tol` 1e-6; a partial that needs a looser tolerance is a bug.
- **Tests run on hand-built IR**: `#[path = "../../piperine-ir/tests/support/devices.rs"] mod devices;` plus this crate's `tests/support/circuits.rs`. Functions are called through `piperine_abi::caller::Handle`, circuits through `piperine_solver::prelude`.
- `Dual`'s operators are trait impls and methods of the struct. A tangent that is empty means zero.
- **Fail loud.** A form the code does not handle is a diagnostic, a `Fault` or an entry of `check()`, never a silent default.
- **AGENTS.md idiom rules bind**: no loose function (every `fn` is a method of a struct or a trait), no macro of your own (`macro_rules!`, proc-macros; derives are fine), modules named after what they do, no `unwrap()`/`expect()` on a user-input path.
- Tests derive from the spec's criteria, never from the implementation. Test functions have exactly the names `## Checks` cites. Every new target opens with a `//!` scope header, has no `#[ignore]` and no file-scope `#![allow]`.
- Zero warnings on every crate the `**Build**` line names. Cargo only with `-p <crate>`: never `--workspace`, never `cargo clean`, never set `CARGO_TARGET_DIR`.
- A numeric assertion or tolerance of an existing test is never edited. A number you cannot explain: stop and report, never retune.
- One atomic commit per task, message as given, with the `Co-Authored-By:` trailer the session gives. No push, no merge.
- If `lessons.py` is run, `git status` afterwards and restore `.specs/lessons.json` and `.specs/LESSONS.md` if they changed.

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `CLAUDE.md` and `AGENTS.md` (zero warnings; fail loud; the five Rust idiom rules of MD-13; test placement MD-28; every target opens with a `//!` scope header), `crates/piperine-lang/tests/suite_hygiene.rs` (no `#[ignore]`, no file-scope lint suppression), `.specs/features/codegen/context.md` (S1 to S10; ngspice parity is the proof of M1).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| `Factory` and `module_with` (in `piperine-abi`) | unit | each AC of INT-08; a panic in the factory's `eval` becomes a `FAULT` | `crates/piperine-abi/tests/factory.rs` | `cargo test -p piperine-abi --test factory` |
| `Dual` and the value walk | unit | each AC of INT-02, INT-03 AC 5, INT-05 AC 2; every operator and method, value and tangent | unit tests in `crates/piperine-interp/src/{dual,walk}.rs` | `cargo test -p piperine-interp --lib` |
| `BodyPlan`: shapes | unit | each AC of INT-09, INT-10 AC 5, INT-01 AC 5, INT-05 AC 1, edge cases 1 and 2; all five device shapes entry by entry | `crates/piperine-interp/tests/shape.rs` | `cargo test -p piperine-interp --test shape` |
| Analog evaluation, affine rule | unit | each AC of INT-03 (1 to 3), INT-12, INT-10 AC 4, INT-11 AC 2, edge case 4; closed forms at fixed bias | `crates/piperine-interp/tests/terms.rs` | `cargo test -p piperine-interp --test terms` |
| `limit`, `ac_stim` functions | unit | each AC of INT-10 (1 to 3) and INT-11 AC 1; every `AGE`/`STATIC` combination | `crates/piperine-interp/tests/{limit,stim}.rs` | `cargo test -p piperine-interp --test limit --test stim` |
| `Interpreter::circuit` on the solver | integration | INT-01, INT-14, edge case 3; divider, diode, RC in `Dc`, `Ac`, transient with a held node | `crates/piperine-interp/tests/circuit.rs` | `cargo test -p piperine-interp --test circuit` |
| Partials of every M1 function | unit | INT-03 AC 4 over all five bodies, at three bias points each | `crates/piperine-interp/tests/partials.rs` | `cargo test -p piperine-interp --test partials` |

## Gate Check Commands

> Generated from codebase - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Quick | while iterating on one target | `cargo test -p piperine-interp --test <the task's target>` (T1: `cargo test -p piperine-abi --test factory`) |
| Full | every task | T1: `cargo test -p piperine-abi`; `cargo test -p piperine-solver`. From T2: `cargo test -p piperine-interp` |
| Build | T1 and the last task of each phase | `cargo test -p piperine-interp`; `cargo test -p piperine-abi`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries` |

---

## Execution Plan

Four phases, run in order. Four batches, one per phase: T1 (Opus), T2 to T4 (Sonnet), T5 to T9 (Opus), T10 to T12 (Sonnet).

### Phase 1: The ABI carries a module's data

Executor: Opus.

```
T1
```

### Phase 2: Crate, duals, value walk

Executor: Sonnet.

```
T2 → T3 → T4
```

### Phase 3: Shapes and analog evaluation

Executor: Opus.

```
T5 → T6 → T7 → T8 → T9
```

### Phase 4: Stimulus, the circuit, guards

Executor: Sonnet.

```
T10 → T11 → T12
```

---

## Task Breakdown

## Phase 1 tasks

### T1: Factory and module_with in piperine-abi

**What**: `device::Factory` (design "Factory and module_with") and `thunk::module_with::<F>(name, &'static F) -> Module` whose `data` is the factory; `create` reads it, `eval`/`react`/`destroy` run the factory's instance; value checks and shape validation shared with `create::<D>` through one private function; one sentence added to abi.md §7 (Q1, decided).
**Where**: `crates/piperine-abi/src/thunk.rs`
**Depends on**: None
**Reuses**: `thunk.rs` (`guarded`, `cut`, `cut_mut`, `write_note`, `create::<D>`), `device.rs` (`Device`, `Values`, `Eval`, `React`)
**Requirement**: INT-08
**Executor**: Opus

**Done when**:

- [x] The test factory `Gain { k }` builds two modules with different `k` from one device type; each evaluates with its own `k`
- [x] A panic in the factory's `eval` is a `FAULT` with the message in the note; nothing unwinds
- [x] `unsafe` stays in `thunk.rs`, every block with its `// SAFETY:` line citing abi.md
- [x] `Device`, `module::<D>` and every existing test are untouched: `git diff` shows only additions in `device.rs`/`thunk.rs`, the new target and abi.md §7
- [x] Tests with exactly these names pass in `cargo test -p piperine-abi --test factory`: `module_with_carries_the_factory`, `create_goes_through_the_factory_and_validates`, `eval_and_react_run_the_created_instance`
- [x] Exits 0: `cargo test -p piperine-abi && cargo test -p piperine-solver`
- [x] Gate check passes: `cargo test -p piperine-abi`; `cargo test -p piperine-solver`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(abi): Factory and thunk::module_with, a module that carries its data`

---

## Phase 2 tasks

### T2: The crate and Interpreter::new

**What**: The crate `piperine-interp` (dependencies `piperine-ir`, `piperine-abi`, `piperine-solver`), `Interpreter::new(Arc<Ir>) -> Result<Interpreter, Rejected>` refusing an `Ir` whose `check()` is not empty, joined to the workspace.
**Where**: `crates/piperine-interp/` (the new crate)
**Depends on**: None (and, outside this feature: `msl-ir` merged)
**Reuses**: `piperine_ir::Ir::check`; `crates/piperine-solver/tests/workspace_shape.rs` (`DEFAULT_MEMBERS`)
**Requirement**: INT-01
**Executor**: Sonnet

**Done when**:

- [x] Root `Cargo.toml` lists the crate in `members` and `default-members`; `workspace_shape.rs::DEFAULT_MEMBERS` gains it; `crate_boundaries` stays green (no solver crate gains an edge)
- [x] `Rejected` carries the `check()` entries
- [x] `the_crate_spawns_nothing_and_reads_no_file` scans `crates/piperine-interp/src` for `std::process`, `std::fs` and `unsafe` and finds none
- [x] `crates/piperine-interp/tests/support/circuits.rs` builds, over `devices.rs`: the divider, a diode behind a source and 1 Ω, an RC low-pass with an AC source, an RC with the node `out` held at 0
- [x] Tests with exactly these names pass in `cargo test -p piperine-interp --test circuit`: `an_ir_that_fails_check_is_rejected`, `the_crate_spawns_nothing_and_reads_no_file`
- [x] Gate check passes: `cargo test -p piperine-interp`; `cargo test -p piperine-abi`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(interp): the piperine-interp crate and Interpreter::new`

---

### T3: Dual numbers

**What**: `Dual { v, d }` with an empty tangent meaning zero: `Add`, `Sub`, `Mul`, `Div`, `Neg`, and methods `exp`, `ln`, `sqrt`, `abs`, `constant`, `seeded(v, slot, slots)`.
**Where**: `crates/piperine-interp/src/dual.rs`
**Depends on**: T2
**Reuses**: design "Dual numbers"
**Requirement**: INT-03
**Executor**: Sonnet

**Done when**:

- [x] Each operator and method is tested for value and tangent against the analytic formula at three points
- [x] Arithmetic between two constants allocates no tangent
- [x] `abs` at a negative value gives the negated tangent
- [x] Tests with exactly these names pass in `cargo test -p piperine-interp --lib`: `dual::tests::abs_has_the_derivative_of_sign`
- [x] Gate check passes: `cargo test -p piperine-interp`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(interp): forward-mode dual numbers`

---

### T4: The value walk

**What**: `Walk` over value code: constants, locals, params, operators with short-circuit `&&`/`||`, `if`, blocks, `let`, function calls, `Math`, closures called with plain reals; an IR form it does not execute is a `Fault` naming the form and its origin.
**Where**: `crates/piperine-interp/src/walk.rs`
**Depends on**: T3
**Reuses**: `Dual`; `piperine_ir` handles; `std`'s `pnjlim` as built by `devices.rs`
**Requirement**: INT-02, INT-05
**Executor**: Sonnet

**Done when**:

- [x] `pnjlim` evaluated at the six branch points of its source equals the same formula written in the test
- [x] `1.0 / 0.0` is `inf` and nothing fails
- [x] A closure reached outside a `limit` faults with the closure's origin in the message
- [x] Tests with exactly these names pass in `cargo test -p piperine-interp --lib`: `walk::tests::f64_arithmetic_and_methods_match_rust`, `walk::tests::division_by_zero_is_ieee`, `walk::tests::short_circuit_and_if_evaluate_one_side`, `walk::tests::calls_bind_parameters_and_closures_see_locals`, `walk::tests::an_unexecuted_form_faults_with_its_origin`
- [x] Gate check passes: `cargo test -p piperine-interp`; `cargo test -p piperine-abi`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [x] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(interp): value evaluation over the IR`

---

## Phase 3 tasks

### T5: BodyPlan: pins, terms and inputs

**What**: `BodyPlan::new(body)` writes through `ShapeBuf` the pins (params, body nodes as `PORT` with nature `abstol` and `HAS_INIT`, one `PRIVATE` pin per potential branch, the `TEMPERATURE` global when read), the terms (`FLOW`, `DDT`, the two terms of a potential branch) and the inputs (`LIMIT`, `STIM`), in the order of design "From a body to a shape"; `BodyFactory` and `Interpreted` exist and `Interpreter::module(body)` returns the leaked module (its `eval` still faults).
**Where**: `crates/piperine-interp/src/plan.rs`
**Depends on**: T1, T4
**Reuses**: `piperine_abi::shape` (`ShapeBuf`, `PinSpec`, `TermSpec`, `InputSpec`), `layout` (`F64`, `BIT`), `thunk::module_with`; `Body::dependencies`
**Requirement**: INT-09, INT-01, INT-05
**Executor**: Opus

**Done when**:

- [ ] Resistor pins `r, p, n` and term `FLOW{p,n}`; Capacitor term `DDT{p,n}`; Vsource pins `dc, ac, p, n, b0.flow` and terms `FLOW{p,n}`, `FLOW{b0.flow, GROUND}`; Diode pins `is, n, cj0, a, k, temperature`, one `LIMIT` input `vd`, terms `FLOW{a,k}` and `DDT{a,k}`
- [ ] A node with `= value` has `HAS_INIT` and the value as `init`; continuous pins carry `abstol = [1e-6, 1e-12]` for `Electrical`
- [ ] `Handle::create` with 4 bytes for an `f64` param faults naming the param
- [ ] Tests with exactly these names pass in `cargo test -p piperine-interp --test shape`: `a_held_node_gives_has_init`, `pins_come_in_the_declared_order`, `a_flow_branch_declares_flow_and_ddt_terms`, `a_potential_branch_declares_its_pin_and_two_terms`, `sites_declare_limit_and_stim_inputs`, `a_value_of_the_wrong_layout_faults_naming_the_param`, `a_block_without_contributions_gives_no_term`, `a_held_node_without_analog_is_ports_only`
- [ ] Gate check passes: `cargo test -p piperine-interp`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(interp): plan a body's pins, terms and inputs as a shape`

---

### T6: BodyPlan: functions, reads and partials

**What**: The shape's functions (`limit:<name>` per site, `stim:<k>` per site, `terms`), each with exactly the read cells of its dependency set in the read order of the design, the partials of each written term or `PROPOSED` against each differentiable read of its own set, origins on every entry, and the `FunctionPlan` table the walk uses.
**Where**: `crates/piperine-interp/src/plan.rs` (modify)
**Depends on**: T5
**Reuses**: `FunctionSpec`, `Cell`, `Partial`; `Body::dependencies`, `Body::is_affine`; `piperine_abi::validate::validate`
**Requirement**: INT-09, INT-10
**Executor**: Opus

**Done when**:

- [ ] Diode: `limit:vd` reads `[a, k, is, n, temperature, vd.LAST, vd.AGE, STATIC]`, writes `[vd.PROPOSED, vd.LIMITED]`, partials `(PROPOSED←a)`, `(PROPOSED←k)`; `terms` reads `[vd.VALUE, is, n, cj0, temperature, GMIN]`, partials `(flow←vd)`, `(charge←vd)`
- [ ] Resistor `terms`: reads `[p, n, r]`, partials `(flow←p)`, `(flow←n)`, none against `r`
- [ ] No function writes both a limit and a term
- [ ] `validate` accepts all five device shapes and the two edge-case shapes
- [ ] Tests with exactly these names pass in `cargo test -p piperine-interp --test shape`: `one_function_per_site_and_one_for_terms`, `reads_and_partials_are_the_dependency_sets`, `every_entry_carries_its_origin`, `every_m1_shape_validates`, `no_function_writes_a_limit_and_a_term`
- [ ] Gate check passes: `cargo test -p piperine-interp`; `cargo test -p piperine-abi`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(interp): plan functions, reads and partials from the dependency sets`

---

### T7: The Terms walk

**What**: `Mode::Terms`: one walk of the `analog` block over `Dual`, probes reading pin differences (ground is 0), `Env` from the `TEMPERATURE` pin and the ambients, addends accumulated per term with their signs, values written to `output` and tangents to `jac` for seeded partials; a `limit` or `ac_stim` site answers its input's `VALUE` with tangent 1 (the site functions themselves land in T9 and T10).
**Where**: `crates/piperine-interp/src/device.rs`
**Depends on**: T6
**Reuses**: `Walk`, `Dual`, `FunctionPlan`; `piperine_abi::device::Eval`
**Requirement**: INT-03, INT-02
**Executor**: Opus

**Done when**:

- [ ] Capacitor at `v = 2`, `c = 1e-9`: charge `2e-9`, partials `c`, `-c`
- [ ] Diode at `vd.VALUE = 0.6`, 300.15 K: flow `is·(exp(vd/vt) − 1) + gmin·vd` and partial `is·exp(vd/vt)/vt + gmin`, both to 1e-12 relative; charge `cj0·vd`
- [ ] Vsource: `b0.flow` term equals the private pin with partial 1; `b0.eq` equals `vp − vn − dc·source_scale` with `source_scale` read from the ambient
- [ ] An unseeded partial is left zero; two evaluations with equal inputs write equal bytes
- [ ] Tests with exactly these names pass in `cargo test -p piperine-interp --test terms`: `evaluating_twice_writes_the_same_bytes`, `the_terms_function_sums_addends_with_signs`, `seeded_partials_are_the_dual_derivatives`, `environment_reads_come_from_pin_and_ambients`, `a_non_finite_value_is_written_as_is`
- [ ] Gate check passes: `cargo test -p piperine-interp`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(interp): evaluate a body's terms with exact partials`

---

### T8: The affine rule

**What**: For a `FLOW` term the plan marks affine: the walk runs with every continuous pin read as 0, giving `c` and the partials `g`, and the value written is `c` plus the fold of `g_j·x_j` from `0.0` in the order of the term's declared partials (design "How one function runs"; the T5 lesson).
**Where**: `crates/piperine-interp/src/device.rs` (modify)
**Depends on**: T7
**Reuses**: `crates/piperine-analog-solver/src/device/flow.rs` (`Linear::dot`, read only: the fold this must match)
**Requirement**: INT-12
**Executor**: Opus

**Done when**:

- [ ] For 1000 pseudo-random `(vp, vn, r)` from a fixed seed, the resistor's value minus `0.0 + g*vp + (-g)*vn` is exactly `0.0`, and the value equals `g*vp - g*vn` bit for bit with `g = 1.0/r`
- [ ] The Vsource's `b0.flow` and `b0.eq` terms follow the same rule
- [ ] The diode's flow term, not affine, is evaluated at the iterate as in T7
- [ ] A body with affine and non-affine terms evaluates each by its own rule (hand-built test body)
- [ ] Tests with exactly these names pass in `cargo test -p piperine-interp --test terms`: `an_affine_term_is_constant_plus_folded_products`, `the_resistor_matches_the_hand_written_products_bit_for_bit`
- [ ] Gate check passes: `cargo test -p piperine-interp`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(interp): affine terms are written as the products the assembly subtracts`

---

### T9: limit

**What**: `Mode::Limit(k)`: walk to site `k`, evaluate `x` over `Dual`, choose `old` (seed, or `x` with no seed, when `AGE` is 0 or `STATIC` and `AGE` is 1; else `LAST`), `LIMITED = f(x, old)` over plain reals, raised to the seed when it starts from the seed and falls below it (§18 rule 2); write `PROPOSED` with its partials and `LIMITED`.
**Where**: `crates/piperine-interp/src/device.rs` (modify)
**Depends on**: T8
**Reuses**: `.specs/features/device-abi/abi.md` A2, A3, §11.2; `docs/spec/part_v_execution.md` §18; `crates/piperine-solver/tests/limiting_report.rs` (how a `LIMIT` device is driven)
**Requirement**: INT-10
**Executor**: Opus

**Done when**:

- [ ] Diode `limit:vd`: `PROPOSED = va − vk` with partials `1`, `-1`
- [ ] Each of the four `(AGE, STATIC)` cases picks the `old` the criterion states; a site with no seed uses `x`
- [ ] With `x` below `vcrit` on the first iteration, `LIMITED` is `vcrit`
- [ ] In `terms`, the site's `x`, `f` and seed are not evaluated: a test body whose `x` would fault proves it
- [ ] Tests with exactly these names pass in `cargo test -p piperine-interp --test limit`: `a_limit_writes_proposed_and_limited`, `old_is_seed_or_last_by_age_and_static`, `the_seed_is_kept_when_the_limited_value_falls_below`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-interp --test terms`: `the_terms_function_reads_the_limit_value_untouched`
- [ ] Gate check passes: `cargo test -p piperine-interp`; `cargo test -p piperine-abi`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: build

**Commit**: `feat(interp): limit sites with the seed rules of abi.md A2 and §18`

---

## Phase 4 tasks

### T10: ac_stim

**What**: `Mode::Stim(k)`: walk to site `k`, evaluate `mag` and `phase` as plain reals, write `RE = mag·cos(phase)` and `IM = mag·sin(phase)`, or `RE = mag`, `IM = 0` exactly when the site has no phase.
**Where**: `crates/piperine-interp/src/device.rs` (modify)
**Depends on**: None
**Reuses**: `Iac` in `crates/piperine-solver/tests/helpers/abi_devices.rs` (the reference `STIM` device)
**Requirement**: INT-11
**Executor**: Sonnet

**Done when**:

- [ ] Vsource `stim:0` with `ac = 1` writes `RE = 1.0`, `IM = 0.0`
- [ ] A hand-built site with phase `π/2` writes `RE` within 1e-16 of 0 and `IM = mag`
- [ ] In `terms`, the Vsource's `b0.eq` has partial `-1` against the stim's `VALUE`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-interp --test stim`: `a_stim_writes_its_phasor`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-interp --test terms`: `the_terms_function_reads_the_stim_value`
- [ ] Gate check passes: `cargo test -p piperine-interp`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: unit
**Gate**: full

**Commit**: `feat(interp): ac_stim sites write their phasor`

---

### T11: Interpreter::circuit

**What**: `Interpreter::circuit()`: one solver node per IR node by path, one `CircuitBuilder::device` per instance whose body has an `analog` block or a held node, ports bound by body-node position, params by name as `ParamValue`; opens with a spike test that a shape of `PORT` pins and no function binds and holds its node.
**Where**: `crates/piperine-interp/src/netlist.rs`
**Depends on**: T10
**Reuses**: `piperine_solver::prelude` (`CircuitBuilder::{node, ground, device, build}`, `Port`, `ParamValue`, the analysis drivers); `tests/support/circuits.rs`
**Requirement**: INT-01
**Executor**: Sonnet

**Done when**:

- [ ] `the_divider_reads_seven_and_a_half_volts`: `Divider.mid` is 7.5 V within 1e-9 in `dc()`
- [ ] `a_diode_converges_to_the_shockley_point`: 0.8 V, 1 Ω, the current equals the fixed point of the Shockley equation with `gmin` to 1e-9 relative
- [ ] `an_rc_low_pass_is_at_minus_three_db_at_its_corner`: magnitude `1/√2` to 1e-9 at 100 kHz
- [ ] `a_held_node_starts_the_transient`: `out` is 0 V at the first step and follows `1 − exp(−t/RC)` within 1e-3 at `t = RC`
- [ ] If the spike shows bind rejects a function-less shape: stop and report; the design's fallback (`TransientSolver::apply_initial_conditions` from the IR's node inits) is taken only with the orchestrator's word
- [ ] Tests with exactly these names pass in `cargo test -p piperine-interp --test circuit`: `one_module_per_body_with_behaviour_or_a_held_node`, `a_circuit_binds_instances_nodes_and_params`, `an_unset_param_keeps_its_default`
- [ ] Gate check passes: `cargo test -p piperine-interp`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: integration
**Gate**: full

**Commit**: `feat(interp): bind an elaborated design to the solver`

---

### T12: where check pins and the partials sweep

**What**: For a param with a `where`: a `PRIVATE` discrete `BIT` pin `check:<p>`, a discrete function `check:<p>` reading the params the condition names, `HAS_CHECK` on the param's pin (`Mode::Check`); and the `verify_partials` sweep over every function of the five device bodies.
**Where**: `crates/piperine-interp/src/plan.rs` (modify)
**Depends on**: T11
**Reuses**: `.specs/features/device-abi/abi.md` P6; `piperine_abi::verify::verify_partials`; `crates/piperine-solver/tests/helpers/partials.rs` (how the solver's tests sweep); `CircuitInstance::set`
**Requirement**: INT-14, INT-03
**Executor**: Sonnet

**Done when**:

- [ ] `circuit.set("Divider.r1", "r", ParamValue::Real(-1.0))` is an error and the next `dc()` reads 7.5 V
- [ ] `set` of a legal value changes the result
- [ ] Every function of every body passes `verify_partials` at three bias points with `rel_tol` 1e-6, the diode's limit probed where `LIMITED = PROPOSED`
- [ ] All shapes still validate with the check pins
- [ ] Tests with exactly these names pass in `cargo test -p piperine-interp --test partials`: `every_m1_function_passes_verify_partials`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-interp --test shape`: `a_where_declares_a_check_pin_and_function`
- [ ] Tests with exactly these names pass in `cargo test -p piperine-interp --test circuit`: `a_set_that_breaks_a_where_is_refused`
- [ ] Gate check passes: `cargo test -p piperine-interp`; `cargo test -p piperine-abi`; the `**Build**` commands; `cargo test -p piperine-lang --test suite_hygiene`; `cargo test -p piperine-solver --test workspace_shape --test crate_boundaries`
- [ ] Test count: the tests named above, and no existing test removed

**Tests**: integration
**Gate**: build

**Commit**: `feat(interp): where guards as check pins; every M1 partial verified`

---

## Checks

One check per acceptance criterion and per edge case of `spec.md`. A test function has exactly the name its check cites.

**C1** - WHEN `Interpreter::new` is given an `Ir` whose `check()` is empty THEN the system SHALL return an interpreter that gives one `&'static Module` per body that has an `analog` block or a node with an initial potential. (INT-01, AC 1; T11)
Proof: run circuit | test one_module_per_body_with_behaviour_or_a_held_node ... ok

**C2** - WHEN `Interpreter::circuit` is called THEN the system SHALL return a `CircuitInstance` in which each such instance is one device bound by `CircuitBuilder::device` under its path, each design node is one solver node named by its path, and each param holds its bound value. (INT-01, AC 2; T11)
Proof: run circuit | test a_circuit_binds_instances_nodes_and_params ... ok

**C3** - IF the `Ir` fails `check()` THEN the system SHALL return no interpreter and SHALL return the check's entries. (INT-01, AC 3; T2)
Proof: run circuit | test an_ir_that_fails_check_is_rejected ... ok

**C4** - The system SHALL invoke no external program and read no file. (INT-01, AC 4; T2)
Proof: run circuit | test the_crate_spawns_nothing_and_reads_no_file ... ok

**C5** - WHEN a body node has an initial potential THEN the system SHALL give its pin `HAS_INIT` with that value. (INT-01, AC 5; T5)
Proof: run shape | test a_held_node_gives_has_init ... ok

**C6** - WHEN `thunk::module_with` is given a name and a `&'static` factory THEN the system SHALL return a `Module` whose `data` is that factory and whose `prototype` is the factory's. (INT-08, AC 1; T1)
Proof: run factory | test module_with_carries_the_factory ... ok

**C7** - WHEN `create` of that module is called THEN the system SHALL call the factory's `create` with the given values and SHALL validate the returned shape as `module::<D>` does. (INT-08, AC 2; T1)
Proof: run factory | test create_goes_through_the_factory_and_validates ... ok

**C8** - WHEN `eval` or `react` of that module is called THEN the system SHALL run the instance the factory created, with a panic turned into a `FAULT` as in `module::<D>`. (INT-08, AC 3; T1)
Proof: run factory | test eval_and_react_run_the_created_instance ... ok

**C9** - The system SHALL leave `Device`, `module::<D>` and every existing test of `piperine-abi` and `piperine-solver` unchanged and green. (INT-08, AC 4; T1)
Proof: `cargo test -p piperine-abi && cargo test -p piperine-solver`

**C10** - The system SHALL declare the pins of a body in this order: one `PARAM` pin per param in declaration order with its default as `init`; one `PORT` continuous pin per body node in node order with the `abstol` of its nature's potential and flow; one `PRIVATE` continuous pin per potential branch; one `GLOBAL` `TEMPERATURE` pin when the body reads `temperature()`. (INT-09, AC 1; T5)
Proof: run shape | test pins_come_in_the_declared_order ... ok

**C11** - The system SHALL declare, per flow branch, a `FLOW` term when it has a direct addend and a `DDT` term when it has an addend under `ddt`, both on the branch's terminals. (INT-09, AC 2; T5)
Proof: run shape | test a_flow_branch_declares_flow_and_ddt_terms ... ok

**C12** - The system SHALL declare, per potential branch with private pin `f`, a `FLOW` term on the branch's terminals whose value is `f`, and a `FLOW` term `{f, GROUND}` whose value is the potential difference minus the sum of the contributions (abi.md A1). (INT-09, AC 3; T5)
Proof: run shape | test a_potential_branch_declares_its_pin_and_two_terms ... ok

**C13** - The system SHALL declare one `LIMIT` input per `limit` site and one `STIM` input per `ac_stim` site. (INT-09, AC 4; T5)
Proof: run shape | test sites_declare_limit_and_stim_inputs ... ok

**C14** - The system SHALL declare one function per `limit` site that writes its `PROPOSED` and `LIMITED`, one function per `ac_stim` site that writes its `RE` and `IM`, and one function that writes every term of the body. (INT-09, AC 5; T6)
Proof: run shape | test one_function_per_site_and_one_for_terms ... ok

**C15** - The system SHALL declare as the reads of each function exactly the cells of its static dependency set (`msl-ir` IR-09), and as its partials each written term or `PROPOSED` against each continuous pin, `LIMIT` value and `STIM` value of that set. (INT-09, AC 6; T6)
Proof: run shape | test reads_and_partials_are_the_dependency_sets ... ok

**C16** - The system SHALL give every pin, term, input and function the `origin` of its IR item. (INT-09, AC 7; T6)
Proof: run shape | test every_entry_carries_its_origin ... ok

**C17** - WHEN a shape is produced for any M1 body THEN `piperine_abi::validate::validate` SHALL accept it. (INT-09, AC 8; T6)
Proof: run shape | test every_m1_shape_validates ... ok

**C18** - The system SHALL evaluate `f64` arithmetic, comparisons and the methods `exp`, `ln`, `sqrt` and `abs` with the results of Rust's `f64`. (INT-02, AC 1; T4)
Proof: run lib | test walk::tests::f64_arithmetic_and_methods_match_rust ... ok

**C19** - WHEN a float is divided by zero THEN the system SHALL yield the IEEE 754 result and SHALL not fail (§1 rule 12). (INT-02, AC 2; T4)
Proof: run lib | test walk::tests::division_by_zero_is_ieee ... ok

**C20** - The system SHALL evaluate `&&` and `||` without evaluating the right operand when the left decides, and an `if` by evaluating only the taken block. (INT-02, AC 3; T4)
Proof: run lib | test walk::tests::short_circuit_and_if_evaluate_one_side ... ok

**C21** - WHEN a function is called THEN the system SHALL evaluate its body with its parameters bound to the argument values, and a closure with the locals of the block that wrote it. (INT-02, AC 4; T4)
Proof: run lib | test walk::tests::calls_bind_parameters_and_closures_see_locals ... ok

**C22** - WHEN a function is evaluated twice with the same inputs THEN the system SHALL write the same output bytes and the same partials. (INT-02, AC 5; T7)
Proof: run terms | test evaluating_twice_writes_the_same_bytes ... ok

**C23** - WHEN the terms function is evaluated THEN the system SHALL execute the `analog` block once, with each probe reading the difference of its terminals' pins, ground reading 0, and SHALL write to each term the sum of its branch's addends with their signs. (INT-03, AC 1; T7)
Proof: run terms | test the_terms_function_sums_addends_with_signs ... ok

**C24** - The system SHALL write, for every seeded partial, the derivative of the written value with respect to the read cell, computed by dual numbers through operators, methods, function calls and locals. (INT-03, AC 2; T7)
Proof: run terms | test seeded_partials_are_the_dual_derivatives ... ok

**C25** - The system SHALL read `temperature()` from the `TEMPERATURE` pin, `gmin()` from the ambient `GMIN` and `source_scale()` from the ambient `SOURCE_SCALE`. (INT-03, AC 3; T7)
Proof: run terms | test environment_reads_come_from_pin_and_ambients ... ok

**C26** - WHEN a function of any M1 body is given to `verify_partials` at a point where its limits hold `LIMITED = PROPOSED` THEN the verifier SHALL pass every declared partial with `rel_tol` 1e-6. (INT-03, AC 4; T12)
Proof: run partials | test every_m1_function_passes_verify_partials ... ok

**C27** - The system SHALL give `abs` the derivative `sign(x)` and every other method of the catalogue its analytic derivative. (INT-03, AC 5; T3)
Proof: run lib | test dual::tests::abs_has_the_derivative_of_sign ... ok

**C28** - WHEN a `FLOW` term's addends are affine in the node potentials (`msl-ir` IR-09 AC 3) THEN the system SHALL evaluate them with every continuous pin read as 0 to get the constant `c` and the partials `g`, and SHALL write the value `c` added to the sum of `g_j·x_j` folded from `0.0` in the order of the term's declared partials. (INT-12, AC 1; T8)
Proof: run terms | test an_affine_term_is_constant_plus_folded_products ... ok

**C29** - WHEN the `Resistor` body is evaluated at any `vp`, `vn` and `r` THEN the system SHALL write a value equal bit for bit to `g*vp - g*vn` with `g = 1.0/r`, and the partials `g` and `-g`. (INT-12, AC 2; T8)
Proof: run terms | test the_resistor_matches_the_hand_written_products_bit_for_bit ... ok

**C30** - WHEN the function of a `limit` site is evaluated THEN the system SHALL write `PROPOSED` as the site's input `x` with its partials against the continuous pins, and `LIMITED` as `f(x, old)`. (INT-10, AC 1; T9)
Proof: run limit | test a_limit_writes_proposed_and_limited ... ok

**C31** - WHEN `AGE` is 0, or the ambient `STATIC` is true and `AGE` is 1, THEN the system SHALL take `old` as the seed, or as `x` when the site has no seed; otherwise it SHALL take `old` as `LAST` (abi.md A2). (INT-10, AC 2; T9)
Proof: run limit | test old_is_seed_or_last_by_age_and_static ... ok

**C32** - WHEN `old` is the seed and `f(x, seed)` is below the seed THEN the system SHALL write the seed as `LIMITED` (§18 rule 2). (INT-10, AC 3; T9)
Proof: run limit | test the_seed_is_kept_when_the_limited_value_falls_below ... ok

**C33** - WHEN the terms function reaches a `limit` site THEN the system SHALL use the input's `VALUE` as the site's result with derivative 1 against that read, and SHALL not evaluate `x`, `f` or the seed (§18 rule 3, abi.md A2). (INT-10, AC 4; T9)
Proof: run terms | test the_terms_function_reads_the_limit_value_untouched ... ok

**C34** - The system SHALL declare no function that writes both a limit and a term (abi.md A3). (INT-10, AC 5; T6)
Proof: run shape | test no_function_writes_a_limit_and_a_term ... ok

**C35** - WHEN the function of an `ac_stim` site is evaluated THEN the system SHALL write `RE` as `mag·cos(phase)` and `IM` as `mag·sin(phase)`, and `RE = mag`, `IM = 0` when the site has no phase. (INT-11, AC 1; T10)
Proof: run stim | test a_stim_writes_its_phasor ... ok

**C36** - WHEN the terms function reaches an `ac_stim` site THEN the system SHALL use the input's `VALUE` as the call's result with derivative 1 against that read. (INT-11, AC 2; T10)
Proof: run terms | test the_terms_function_reads_the_stim_value ... ok

**C37** - IF `create` is given a value for a param whose layout it does not match THEN the system SHALL return a `FAULT` naming the param. (INT-05, AC 1; T5)
Proof: run shape | test a_value_of_the_wrong_layout_faults_naming_the_param ... ok

**C38** - IF an evaluation reaches an IR form the interpreter does not execute THEN the system SHALL return a `FAULT` whose message names the form and its origin, and SHALL not write a value. (INT-05, AC 2; T4)
Proof: run lib | test walk::tests::an_unexecuted_form_faults_with_its_origin ... ok

**C39** - WHEN a param has a `where` condition THEN the system SHALL declare a private discrete pin that holds a `BOOL` at byte 0, a discrete function that reads the params the condition names and writes it, and SHALL flag the param's pin `HAS_CHECK` with that pin as its `check`. (INT-14, AC 1; T12)
Proof: run shape | test a_where_declares_a_check_pin_and_function ... ok

**C40** - WHEN `CircuitInstance::set` writes a value that makes the condition false THEN the solver SHALL refuse the write. (INT-14, AC 2; T12)
Proof: run circuit | test a_set_that_breaks_a_where_is_refused ... ok

**C41** - WHEN a body has an `analog` block that contributes nothing THEN the system SHALL give a shape with no term and no function, and the instance SHALL bind. (INT edge case 1; T5)
Proof: run shape | test a_block_without_contributions_gives_no_term ... ok

**C42** - WHEN a body has no `analog` block and a node with an initial potential THEN the system SHALL give a shape of `PORT` pins only. (INT edge case 2; T5)
Proof: run shape | test a_held_node_without_analog_is_ports_only ... ok

**C43** - WHEN a param is not given at a placement THEN the system SHALL pass no value for it and the pin SHALL hold its default. (INT edge case 3; T11)
Proof: run circuit | test an_unset_param_keeps_its_default ... ok

**C44** - IF a contribution evaluates to a non-finite value THEN the system SHALL write it as it is; the solver fails the solve (abi.md A4, `E0507`). (INT edge case 4; T7)
Proof: run terms | test a_non_finite_value_is_written_as_is ... ok

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** none
**Dispatched:** 2026-11-15 batch 1 (T1, Opus) done 673c7ce8; batch 2 (T2-T4, Sonnet) in ../piperine-ir-interp
**Walked:** not applicable - the feature puts no screen in front of a user; each story's Independent Test is a test named in `## Checks`

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4

Phase 1 (Opus):  T1
Phase 2 (Sonnet):  T2 → T3 → T4
Phase 3 (Opus):  T5 → T6 → T7 → T8 → T9
Phase 4 (Sonnet):  T10 → T11 → T12
```

Batches: B1 = Phase 1 (1 task, Opus); B2 = Phase 2 (3 tasks, Sonnet); B3 = Phase 3 (5 tasks, Opus); B4 = Phase 4 (3 tasks, Sonnet). The Verifier is an Opus that authored none of B1 and B3.

## Task Granularity Check

| Task | Scope | Status |
| ---- | ----- | ------ |
| T1: Factory and module_with in piperine-abi | 1 trait + 1 constructor | ✅ Granular |
| T2: The crate and Interpreter::new | 1 crate skeleton + its entry point | ✅ Granular |
| T3: Dual numbers | 1 number type | ✅ Granular |
| T4: The value walk | 1 evaluator | ✅ Granular |
| T5: BodyPlan: pins, terms and inputs | 1 shape planner (data half) | ✅ Granular |
| T6: BodyPlan: functions, reads and partials | 1 shape planner (function half) | ✅ Granular |
| T7: The Terms walk | 1 evaluation mode | ✅ Granular |
| T8: The affine rule | 1 evaluation rule | ✅ Granular |
| T9: limit | 1 evaluation mode | ✅ Granular |
| T10: ac_stim | 1 evaluation mode | ✅ Granular |
| T11: Interpreter::circuit | 1 netlist step | ✅ Granular |
| T12: where check pins and the partials sweep | 1 capability + 1 sweep target | ✅ Granular |

## Diagram-Definition Cross-Check

| Task | Depends On (task body) | Diagram Shows | Status |
| ---- | ---------------------- | ------------- | ------ |
| T1 | None | first of its phase | ✅ Match |
| T2 | None | first of its phase | ✅ Match |
| T3 | T2 | T2 → T3 | ✅ Match |
| T4 | T3 | T3 → T4 | ✅ Match |
| T5 | T1, T4 | first of its phase; T1, T4 in an earlier phase | ✅ Match |
| T6 | T5 | T5 → T6 | ✅ Match |
| T7 | T6 | T6 → T7 | ✅ Match |
| T8 | T7 | T7 → T8 | ✅ Match |
| T9 | T8 | T8 → T9 | ✅ Match |
| T10 | None | first of its phase | ✅ Match |
| T11 | T10 | T10 → T11 | ✅ Match |
| T12 | T11 | T11 → T12 | ✅ Match |

## Test Co-location Validation

| Task | Code Layer Created/Modified | Matrix Requires | Task Says | Status |
| ---- | --------------------------- | --------------- | --------- | ------ |
| T1: Factory and module_with in piperine-abi | `Factory` and `module_with` | unit | unit | ✅ OK |
| T2: The crate and Interpreter::new | `Interpreter::circuit` on the solver | unit | unit | ✅ OK |
| T3: Dual numbers | `Dual` and the value walk | unit | unit | ✅ OK |
| T4: The value walk | `Dual` and the value walk | unit | unit | ✅ OK |
| T5: BodyPlan: pins, terms and inputs | `BodyPlan`: shapes | unit | unit | ✅ OK |
| T6: BodyPlan: functions, reads and partials | `BodyPlan`: shapes | unit | unit | ✅ OK |
| T7: The Terms walk | Analog evaluation | unit | unit | ✅ OK |
| T8: The affine rule | Analog evaluation, affine rule | unit | unit | ✅ OK |
| T9: limit | `limit`, `ac_stim` functions | unit | unit | ✅ OK |
| T10: ac_stim | `limit`, `ac_stim` functions | unit | unit | ✅ OK |
| T11: Interpreter::circuit | `Interpreter::circuit` on the solver | integration | integration | ✅ OK |
| T12: where check pins and the partials sweep | `BodyPlan`: shapes; partials of every M1 function | integration | integration | ✅ OK |
