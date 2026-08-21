# LRM Conformance Closure Tasks

## Execution Protocol (MANDATORY — do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, adequacy review, Verifier, discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed without it.**

---

**Spec**: `.specs/features/lrm-conformance-closure/spec.md`
**Design**: `.specs/features/lrm-conformance-closure/design.md`
**Status**: Draft (awaiting approval)

---

## Test Coverage Matrix

> Generated from codebase sampling + project guidelines. Guidelines found: `CLAUDE.md` ("Where the tests are", MD-28 placement rule, MD-31 gate rule, fail-loud rule), `.specs/STATE.md` (MD-28/MD-31), `tests/suite_hygiene.rs` (mechanical enforcement: no disabled test code, no `#[ignore]`, every target has a `//!` scope header), `tests/support/ngspice.rs` (the only place a numeric tolerance is written).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
|---|---|---|---|---|
| Lexer / preprocessor / grammar (`piperine-lang/src/{lexer,preprocessor,grammar}`) | integration | Every AC + every LRM worked example for the clause; illegal-form rejection asserts the typed error kind, not just "is_err" | `crates/piperine-lang/tests/*.rs` | `cargo test -p piperine-lang` |
| Elaborator / POM (`piperine-lang/src/{elab,pom,eval}`) | integration | 1:1 to spec ACs; every listed edge case; POM-navigability assertion where structure is touched (MD-25) | `crates/piperine-lang/tests/*.rs` | `cargo test -p piperine-lang` |
| Pure semantics helpers (`piperine-signal`, `eval/tasks.rs` functions, `value/logic.rs`) | unit (inline `#[cfg(test)]`) | All branches; LRM-published reference values where the LRM prints them | inline in the source file | `cargo test -p <crate>` |
| Codegen (`piperine-codegen/src/{resolve,flatten,emit,kernel,device}`) | integration | Every AC that reaches executable code; JIT ABI changes carry an offset assertion | `crates/piperine-codegen/tests/*.rs` | `cargo test -p piperine-codegen` |
| Solver (`piperine-solver/src/{digital,core,analyses}`) | integration | Ordering/numerical behavior asserted against the LRM's own worked example where one exists | `crates/piperine-solver/tests/*.rs` | `cargo test -p piperine-solver` |
| Host API (`piperine-api`, root `piperine`) | integration | Host-visible behavior end-to-end (output capture, session wiring) | `tests/*.rs` (root) | `cargo test --workspace` |
| Plugin (`piperine-plugin`) | integration | Every VPI category row the task flips to `Native` | `crates/piperine-plugin/tests/*.rs` | `cargo test -p piperine-plugin` |
| Guards / policy invariants (MD-31) | integration | Registry + exhaustiveness; guard must be proven able to fail (inject, watch, revert) | `tests/*.rs` (root) | `cargo test --workspace` |
| Headers (`crates/piperine-lang/headers/*.vams`) | integration | Content match against the LRM's own listing | `crates/piperine-lang/tests/*.rs` | `cargo test -p piperine-lang` |

**Fixture rule (spec Edge Cases):** every regression test derives from the LRM's own worked example for the clause where the LRM provides one — not from a synthetic minimal repro.

## Gate Check Commands

| Gate Level | When to Use | Command |
|---|---|---|
| Quick | Task touching one crate, unit/integration in that crate only | `cargo test -p <crate>` |
| Full | Task crossing crates, or touching JIT ABI / solver / host surface | `cargo build --workspace && cargo test --workspace` |
| Build | Phase close, header-only or doc-only tasks | `cargo build --workspace` (zero warnings) + `cargo test --workspace` |

**Baseline rule:** T1 captures the pre-feature pass/fail set target-by-target (the MD-47 A/B precedent). "No regression" means byte-identical-or-better against *that* baseline, never an aggregate count — the tree currently has known-red targets (e.g. `system_task_migration_completeness.rs`, 12/12 failing, closed by T45).

---

## Execution Plan

Phases run sequentially; tasks within a phase run in order.

### Phase 0 — Spine (new mechanism; closes C10-01/18/19 as a side effect)
```
T1 → T2 → T3 → T4 → T5 → T6 → T7 → T8 → T9
```

### Phase 1a — ch.1-4 lexical/grammar FORM
```
T10 → T11 → T12 → T13 → T14 → T15
```

### Phase 1b — ch.1-4 access functions and expression semantics
```
T16 → T17 → T18 → T19 → T20 → T21
```

### Phase 2 — ch.5-6 analog behavior + hierarchy
```
T22 → T23 → T24 → T25 → T26 → T27 → T28 → T29 → T30
```

### Phase 3a — ch.7 connect modules and driver access
```
T31 → T32 → T33 → T34 → T35 → T36 → T37
```

### Phase 3b — ch.8 scheduling
```
T38 → T39 → T40 → T41 → T42
```

### Phase 3c — ch.9 system task catalogue
```
T43 → T44 → T45 → T46 → T47 → T48 → T49 → T50 → T51
```

### Phase 4 — ch.10 directives + Annex D/E
```
T52 → T53 → T54 → T55 → T56 → T57 → T58
```

### Phase 5 — VPI native equivalents
```
T59 → T60 → T61 → T62 → T63 → T64
```

### Phase 6 — Closure
```
T65 → T66 → T67
```

---

## Task Breakdown

### T1: Capture the pre-feature test baseline

**What**: Record the current pass/fail set target-by-target into a checked-in baseline file, so "no regression" is falsifiable.
**Where**: `.specs/features/lrm-conformance-closure/baseline.md`
**Depends on**: None
**Reuses**: the MD-47 A/B methodology (`.specs/STATE.md` MD-47)
**Requirement**: spec Goal 3

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `cargo build --workspace` warning count recorded (expected: 0)
- [ ] Every `Running tests/<name>.rs` target listed with its pass/fail/ignored counts
- [ ] Known-red targets listed by name with their failure reason (incl. `system_task_migration_completeness.rs` 12/12)
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace` (result recorded, not required green)

**Tests**: none (baseline artifact) · **Gate**: build
**Commit**: `docs(specs): capture pre-LRMC test baseline`

---

### T2: `LogicVec` four-state value type

**What**: The width-carrying four-state bit-vector value and its algebra.
**Where**: `crates/piperine-lang/src/value/logic.rs` (new), `value.rs` (`Value::Logic`), `pom/net_type.rs` (`ValueType::Logic(u32)`)
**Depends on**: T1
**Reuses**: `piperine_solver::abi::LogicValue` (`digital/events.rs:11`) as the 1-bit projection; existing `Value` serde contract
**Requirement**: LRMC-C1-02

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `LogicVec { width, val, unk }` with `from_based_literal`, `bit(i) -> LogicValue`, `has_unknown`, `to_i64_exact`, `case_eq`
- [ ] `Value::Logic` + `ValueType::Logic(w)` added; serde round-trip test passes
- [ ] Unit tests cover every bit-state combination + width edge (1, 63, 64, 65 bits)
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): four-state LogicVec value type (LRMC-C1-02)`

---

### T3: Four-state literals through const-eval

**What**: A based literal with `x`/`z`/`?` digits evaluates to `Value::Logic`, never a silent `0`; four-state in a two-state-only context fails loud citing the clause.
**Where**: `crates/piperine-lang/src/elab/const_eval.rs:301-315`
**Depends on**: T2
**Reuses**: `LogicVec::from_based_literal`
**Requirement**: LRMC-C1-02, spec Edge Case 2

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] LRM §2.6.1 Example 2/4 literals (`16'hx`, `12'hz`, `3'b01x`) evaluate to the right `LogicVec`
- [ ] `unwrap_or(0)` coercion is gone (grep-asserted in the test)
- [ ] Four-state at an array bound / bit width fails with a typed error naming the LRM clause
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `fix(lang): four-state literals no longer coerce to 0 (LRMC-C1-02)`

---

### T4: `SimOutput` sink and `FileTable`

**What**: The solver-side runtime output contract plus its three sinks.
**Where**: `crates/piperine-solver/src/core/output.rs` (new), `core/mod.rs`, `prelude.rs`
**Depends on**: T1
**Reuses**: nothing existing (new surface); `Channel`/descriptor shape follows §9.5
**Requirement**: LRMC-C7-07 (enabler), C10-14 (enabler)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `trait SimOutput`, `Channel::{Stdout,Stderr,File(FileId)}`, `StdoutSink`, `BufferSink`, `FileTable`
- [ ] `FileTable` open/close/write/read with typed errors (no silent zero descriptor)
- [ ] `Context` carries `Rc<dyn SimOutput>`, defaulting to `StdoutSink`
- [ ] Unit tests: buffer capture is deterministic; file open failure is typed
- [ ] Gate check passes: `cargo test -p piperine-solver`

**Tests**: unit · **Gate**: quick
**Commit**: `feat(solver): SimOutput sink and FileTable`

---

### T5: JIT task trampolines + `SimCtx.io`

**What**: Emit real calls from analog and digital JIT bodies into Rust trampolines that reach the sink.
**Where**: `crates/piperine-codegen/src/emit/runtime_io.rs` (new), `emit/abi.rs` (append `io`), `kernel/{analog,digital}/compile.rs` (symbol registration)
**Depends on**: T4
**Reuses**: `jit_builder.symbol(...)` math-intrinsic registration (`kernel/analog/compile.rs:185-191`); `AnalogKernel::diagnostics()` (`kernel/analog/kernel.rs:693`) as the compile-time half; `eval/tasks.rs::format_display` for rendering
**Requirement**: LRMC-C7-07 (enabler)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `FormatPool` interns format strings + arg descriptors at compile time (no per-call allocation)
- [ ] `pip_task_display`/`_write`/`_finish`/`_fatal` trampolines registered in both JITs
- [ ] `SimCtx.io` appended **last**; ABI field-offset assertion test added
- [ ] Analog diagnostics fire only on accepted (converged, committed) steps — asserted by test
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): JIT task trampolines into the SimOutput sink`

---

### T6: `DirectiveState` + `` `default_discipline ``/`` `default_transition `` parse

**What**: Preprocessor state struct carried on the parsed source, plus the two Verilog-AMS-specific directives that write it.
**Where**: `crates/piperine-lang/src/directives.rs` (new), `preprocessor.rs:191-271`, `ast::SourceFile`
**Depends on**: T1
**Reuses**: existing `handle_directive_or_macro` dispatch shape
**Requirement**: LRMC-C10-01, LRMC-C10-02 (parse half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `` `default_discipline ``/`` `default_transition `` parse instead of raising `UndefinedMacro`
- [ ] `DirectiveState` reachable from `parse_and_elaborate`'s result
- [ ] Scope/clearing semantics per §10.2/§10.3 tested (set, override, clear)
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): DirectiveState and the default_discipline/transition directives (LRMC-C10-01/02)`

---

### T7: Discipline domains — continuous wins, `` `default_discipline `` fallback

**What**: `ResolveDisciplines` learns domain (continuous/discrete) and Annex F §7.4.4.1's headline rule + step 4.b.
**Where**: `crates/piperine-lang/src/elab/lower/disciplines.rs:99-150`, `pom/nature.rs`
**Depends on**: T6
**Reuses**: Annex D.1 disciplines already declare `domain`; existing `E4003` for the genuine-incompatibility path
**Requirement**: LRMC-C10-18, LRMC-C10-19

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] LRM Figure 7-3's `NetC`/`NetD` worked example resolves to continuous
- [ ] Unresolved net consults `DirectiveState.default_discipline` before failing
- [ ] Two same-domain incompatible disciplines still fail `E4003` (spec Edge Case 3) — asserted
- [ ] `disciplines.rs`'s doc comment updated to describe what it now does
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `fix(lang): Annex F continuous-over-discrete resolution and default_discipline fallback (LRMC-C10-18/19)`

---

### T8: Shared connect-rule eligibility check

**What**: Extract `connect.rs`'s rule matching into a function both `ResolveDisciplines` and `InsertConnectModules` call, so a bridgeable mismatch is no longer vetoed before insertion.
**Where**: `crates/piperine-lang/src/elab/lower/connect.rs`, `disciplines.rs`
**Depends on**: T7
**Reuses**: `InsertConnectModules`'s existing selection algorithm and its bypass tests (`connect.rs:319-368`)
**Requirement**: LRMC-C7-01 (enabler)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `eligible_bridge(&Elaborator, a, b) -> Option<RuleId>` shared by both passes
- [ ] `PASSES` order unchanged (`passes.rs:55-64`) — asserted by test
- [ ] The two hand-built bypass tests now run without bypassing `ResolveDisciplines`
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `refactor(lang): share connect-rule eligibility between discipline passes`

---

### T9: `lrm_conformance_matrix.rs` gate skeleton

**What**: The registry+exhaustiveness guard listing all 51 LRMC-IDs, each bound to its closing test name (unclosed IDs carry an explicit `Pending` marker until their task lands).
**Where**: `tests/lrm_conformance_matrix.rs` (new)
**Depends on**: T1
**Reuses**: `tests/capabilities_contract.rs` / `tests/spec_failure_rules_guard.rs` registry pattern (MD-31)
**Requirement**: spec Success Criteria 1

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] All 51 IDs present with LRM clause, story, and status (`Pending` or a named test)
- [ ] Test fails if an ID is missing, duplicated, or names a non-existent test
- [ ] Guard proven able to fail (inject a bogus test name, watch it fail, revert) — recorded in the commit body
- [ ] `//!` scope header present (suite_hygiene)
- [ ] Gate check passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `test(lrmc): conformance matrix gate over the 51 audit findings`

---

### T10: Per-base digit validation

**What**: Reject digits illegal for the declared base.
**Where**: `crates/piperine-lang/src/lexer.rs:437-443,575-577`
**Depends on**: T3
**Reuses**: existing `lex_based_digits` structure
**Requirement**: LRMC-C1-01

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `is_based_digit(base, ch)`; `'d1F`, `'o89`, `'b29a` rejected with `IllegalBasedDigit { base, digit }`
- [ ] Legal four-state digits (`x`/`z`/`?`) still accepted for every base per §2.6.1
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `fix(lang): per-base digit validation in based literals (LRMC-C1-01)`

---

### T11: Whitespace between base letter and digits

**What**: Accept `` 'D 3 `` (LRM §2.6.1 Example 2).
**Where**: `crates/piperine-lang/src/lexer.rs:435-436`
**Depends on**: T10
**Reuses**: the existing-but-uncalled `skip_inline_whitespace` (`lexer.rs:171-179`)
**Requirement**: LRMC-C1-03

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every based-literal form in the LRM's Example 2 lexes
- [ ] A newline between base and digits still errors (whitespace ≠ line break per the production)
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `fix(lang): allow whitespace after a literal's base letter (LRMC-C1-03)`

---

### T12: Underscore placement rules

**What**: Reject the two illegal underscore positions instead of silently stripping them.
**Where**: `crates/piperine-lang/src/lexer.rs:437-443,461,478-486`
**Depends on**: T11
**Reuses**: `consume_decimal_digits`
**Requirement**: LRMC-C1-04

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `1._5` and `` 'h_FF `` rejected with a typed error
- [ ] Legal separators (`1_000`, `` 'h FF_FF ``) still accepted
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `fix(lang): underscore placement rules in number literals (LRMC-C1-04)`

---

### T13: Assignment pattern `'{...}` — lex, AST, parse

**What**: Lex `` '{ `` and parse the assignment-pattern production into a new AST node.
**Where**: `crates/piperine-lang/src/lexer.rs:399-455`, `ast/expr.rs`, `grammar/expr.rs`
**Depends on**: T12
**Reuses**: existing `'` dispatch structure
**Requirement**: LRMC-C1-06 (FORM half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `Expr::AssignmentPattern` parses for §3.4.4/§3.4.6/§3.4.8's own examples
- [ ] `annex_a_grammar.rs`'s A.8.x entry updated from `OutOfScope` to `Implemented` if it names this production
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): assignment-pattern syntax '{...} (LRMC-C1-06)`

---

### T14: Assignment pattern — elaboration

**What**: Evaluate assignment patterns for parameter arrays, string valid-value lists, and multidimensional initializers.
**Where**: `crates/piperine-lang/src/elab/const_eval.rs`, `elab/lower/vams.rs`
**Depends on**: T13
**Reuses**: existing `ValueType::Array` and parameter `from`/`exclude` range machinery
**Requirement**: LRMC-C1-06 (FUNCTIONALITY half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] §3.4.4's `parameter real poles[0:3] = '{1.0, 3.198, 4.554, 2.00};` elaborates with the right values
- [ ] §3.4.6's `from '{ "NPN", "PNP" }` constrains a string parameter (illegal value rejected)
- [ ] §3.4.8's multidimensional example elaborates
- [ ] Arity/shape mismatch fails loud
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): elaborate assignment patterns for arrays and valid-value lists (LRMC-C1-06)`

---

### T15: Expression-level attribute instances

**What**: Accept `(* ... *)` at the expression sites Syntax 2-5..2-10 permits.
**Where**: `crates/piperine-lang/src/grammar/expr.rs` (`parse_unary`/`parse_expr_bp`/`parse_primary`)
**Depends on**: T14
**Reuses**: `cursor.rs:109-123`'s `parse_attr_instances`
**Requirement**: LRMC-C1-05

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] §2.9's Examples 6-8 parse (`b + (* mode="cla" *) c`, `add (* mode="cla" *) (b,c)`, `b ? (* no_glitch *) c : d`)
- [ ] Attributes attach to the expression node and are otherwise inert (no semantic change asserted)
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): expression-level attribute instances (LRMC-C1-05)`

---

### T16: Access-function grammar generalization

**What**: Stop hardcoding `V`/`I` — any call-shaped name becomes a candidate access function for the elaborator to promote.
**Where**: `crates/piperine-lang/src/grammar/expr.rs:192-195,221-232`
**Depends on**: T15
**Reuses**: existing `parse_access_function`
**Requirement**: LRMC-C1-08

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `potential(b1)`/`flow(n1)` and a custom-`access`-named call parse into the shape the elaborator can promote
- [ ] `V`/`I` behavior byte-identical to today for existing fixtures
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `refactor(lang): generalize access-function parsing beyond V/I (LRMC-C1-08)`

---

### T17: Custom access-name resolution in the elaborator

**What**: Resolve a call to the discipline's own `access` name (and the generic `potential`/`flow`) to the correct branch and contribution kind.
**Where**: `crates/piperine-lang/src/elab/lower/vams.rs:761-764`
**Depends on**: T16
**Reuses**: `Design::disciplines`' recorded `access` attribute (§3.6.1.2), existing branch-conflict checks
**Requirement**: LRMC-C1-09

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] A thermal (or other non-electrical) discipline's accessor resolves potential/flow correctly
- [ ] `BranchKindConflict` (§5.4.2) now fires for non-electrical disciplines too
- [ ] The stale doc comment at `vams.rs:761-764` is replaced by what the code now does
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): resolve custom discipline access names (LRMC-C1-09)`

---

### T18: Port access operator `I(<port>)`

**What**: Parse and elaborate the `<...>` port-access form.
**Where**: `crates/piperine-lang/src/grammar/expr.rs`, `ast/expr.rs`, `elab/lower/vams.rs`
**Depends on**: T17
**Reuses**: the access-function path from T16/T17
**Requirement**: LRMC-C1-10

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] §5.4.3's `diode` module example (`if (I(<a>) > imax)`) parses, elaborates, and simulates
- [ ] Port flow differs from branch flow where the LRM says it must
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): port access function I(<port>) (LRMC-C1-10)`

---

### T19: Real→integer implicit rounding

**What**: Round per §4.2.1.1 (ties away from zero) instead of erroring on a fractional value.
**Where**: `crates/piperine-lang/src/elab/const_eval.rs:242,252`
**Depends on**: T18
**Reuses**: existing `eval_nat`/`eval_int`
**Requirement**: LRMC-C1-11

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] 35.7→36, −1.5→−2, 2.5→3 asserted
- [ ] Negative value where a `Natural` is required still fails loud
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `fix(lang): round real to integer per §4.2.1.1 (LRMC-C1-11)`

---

### T20: `aliasparam` type from the aliased parameter

**What**: Type an alias from its target instead of hardcoding `Real`.
**Where**: `crates/piperine-lang/src/elab/lower/vams.rs:95-104`
**Depends on**: T19
**Reuses**: existing `bundle_origin` value mirroring
**Requirement**: LRMC-C1-07

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `aliasparam` over `integer` and `string` parameters carries the right `ValueType`
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `fix(lang): aliasparam inherits the aliased parameter's type (LRMC-C1-07)`

---

### T21: `laplace_*` registry + registry/keyword guard

**What**: Register the four real LRM spellings, drop the three fabricated ones, and add the guard that makes this bug class mechanical.
**Where**: `crates/piperine-codegen/src/resolve/pom/analog_ops.rs:394-396`, new guard in `crates/piperine-codegen/tests/`
**Depends on**: T20
**Reuses**: `zi_*`'s correct registration two lines below (`analog_ops.rs:397-399`)
**Requirement**: LRMC-C1-12, LRMC-C5-01

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `laplace_zd`/`laplace_nd`/`laplace_zp`/`laplace_np` all resolve and simulate; §4.5.11.5's canonical example runs
- [ ] `laplace_pm`/`nm`/`npm` are gone and now fail loud as unknown operators
- [ ] Guard: every operator-registry key is a reserved keyword in `keywords.rs`, and every `laplace_*`/`zi_*` keyword is registered (proven able to fail)
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `fix(codegen): register the four LRM laplace_* variants (LRMC-C1-12/C5-01)`

---

### T22: `cross()` direction argument

**What**: Honor the direction argument instead of always `Either`.
**Where**: `crates/piperine-codegen/src/flatten/analog.rs:76-82`
**Depends on**: T21
**Reuses**: existing `CrossDir` + `device/analog/events.rs:21-25`'s `to_signal_cross_dir`
**Requirement**: LRMC-C5-02

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `+1`/`-1`/`0` fire on exactly the specified edges in a transient run
- [ ] Default (argument omitted) stays `Either`
- [ ] Gate check passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: quick
**Commit**: `fix(codegen): honor cross() direction argument (LRMC-C5-02)`

---

### T23: `absdelta` semantics in `piperine-signal`

**What**: The §5.10.3.4 monitored-event function as a pure, tested detector.
**Where**: `crates/piperine-signal/src/events.rs`
**Depends on**: T22
**Reuses**: the crate's existing `cross`/`above`/`timer` detector shape and `predict_crossing`
**Requirement**: LRMC-C5-03 (semantics half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `absdelta(expr, tol, min_var, max_rate)` fires per §5.10.3.4, all four argument arities
- [ ] Unit tests cover tol/min_var/max_rate interaction and the no-trigger case
- [ ] `tests/purity.rs` still green (no new dependency)
- [ ] Gate check passes: `cargo test -p piperine-signal`

**Tests**: unit · **Gate**: quick
**Commit**: `feat(signal): absdelta monitored-event detector (LRMC-C5-03)`

---

### T24: `absdelta` wiring into codegen

**What**: Give `@(absdelta(...))` an event source instead of silently producing none.
**Where**: `crates/piperine-codegen/src/flatten/analog.rs:71-91`, `kernel/analog/`, `device/analog/events.rs`
**Depends on**: T23
**Reuses**: T23's detector; the `cross` event-source path
**Requirement**: LRMC-C5-03 (wiring half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] An analog block sensitive to `absdelta` re-evaluates at the LRM-specified points
- [ ] The `_ => vec![]` fallthrough can no longer swallow a known event function (asserted: unknown event fn fails loud)
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): wire absdelta event sources (LRMC-C5-03)`

---

### T25: Named events — grammar and POM

**What**: Parse `-> event;` and lower `event` declarations into the POM instead of dropping them.
**Where**: `crates/piperine-lang/src/grammar/stmt.rs`, `elab/lower/vams.rs:142`, `pom/module.rs`
**Depends on**: T24
**Reuses**: the already-lexed `->` token (`lexer.rs:88`)
**Requirement**: LRMC-C5-04 (FORM half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `event e;` reaches `Module` as an authored declaration (MD-25: navigable as written)
- [ ] `-> e;` parses in both analog and digital statement positions
- [ ] Trigger of an undeclared event fails loud
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): named event declarations and the -> trigger (LRMC-C5-04)`

---

### T26: Named events — runtime

**What**: `@(my_event)` sensitivity actually fires on `-> my_event`, in both engines.
**Where**: `crates/piperine-codegen/src/{flatten/analog.rs,kernel/digital/compile.rs}`, `piperine-solver/src/digital/events.rs`
**Depends on**: T25
**Reuses**: existing event-source machinery
**Requirement**: LRMC-C5-04 (FUNCTIONALITY half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] §5.10.4's own example produces the specified trigger ordering in a transient run
- [ ] A digital `->` triggering an analog `@(e)` works (cross-domain case)
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): named event triggering at runtime (LRMC-C5-04)`

---

### T27: Annex C narrowing for §5.10.5

**What**: Permit digital edge/state-change/named-event sensitivity inside `analog`, keep the rest of Annex C restricted.
**Where**: `crates/piperine-lang/src/elab/typecheck.rs:67-68`, `crates/piperine-codegen/src/flatten/analog.rs:56-60`
**Depends on**: T26
**Reuses**: existing Annex C enforcement structure
**Requirement**: LRMC-C5-05

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] §5.10.5's paradigm mixed-signal idiom (`@(posedge clk)` inside `analog`) elaborates and simulates
- [ ] `<=`, `casex`, `casez` inside `analog` still rejected (`vams_annex_c.rs` stays green)
- [ ] `flatten/analog.rs:56-60`'s "defensive empty list" comment removed — it is a live path now
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `fix(lang): permit digital events in analog behavior per §5.10.5 (LRMC-C5-05)`

---

### T28: `defparam` application pass

**What**: A pass that applies hierarchical parameter overrides instead of discarding them.
**Where**: `crates/piperine-lang/src/elab/lower/defparam.rs` (new), `passes.rs`
**Depends on**: T27
**Reuses**: existing instance-override application in `vams.rs`
**Requirement**: LRMC-C5-06

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] §6.3.1's example changes the named parameter's effective value
- [ ] A `defparam` naming a non-existent path fails loud
- [ ] POM navigability assertion: `Design::modules` structure unchanged, only param values (MD-25)
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): apply defparam overrides (LRMC-C5-06)`

---

### T29: `paramset` instantiation

**What**: An instance naming a paramset resolves to its module with the bound parameters applied.
**Where**: `crates/piperine-lang/src/elab/lower/{register.rs:40,vams.rs}`
**Depends on**: T28
**Reuses**: `Design::paramsets` storage (already populated, `pom/design.rs:152`); the `urc__5` monomorphization naming precedent
**Requirement**: LRMC-C5-07

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] §6.4's example instantiates with the paramset's bound values
- [ ] Paramset `.model`-style filtering/type-check against the target module fails loud on mismatch
- [ ] POM navigability assertion: instance → submodule → sub-instances still walkable (MD-25)
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): consume paramsets at instantiation (LRMC-C5-07)`

---

### T30: Array of instances

**What**: Parse `name [msb:lsb] (ports)` and expand it with per-element port slicing.
**Where**: `crates/piperine-lang/src/grammar/item.rs:842-855`, `ast/item.rs`, `elab/lower/`
**Depends on**: T29
**Reuses**: `UnrollGenerate`'s expansion machinery and `flat_modules` derived form
**Requirement**: LRMC-C5-08

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `buf8 [7:0] (out[7:0], in[7:0]);` parses, elaborates to 8 instances with correct per-element connections
- [ ] Width mismatch between range and connected bus fails loud
- [ ] POM navigability assertion: authored form stays a single ranged instance; expansion only in the derived flat form (MD-25)
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): array-of-instances syntax (LRMC-C5-08)`

---

### T31: `connectrules` body syntax (§7.7.1)

**What**: Parse the auto-insertion statement forms beyond the bare `connect <disc>, <disc>` rule.
**Where**: `crates/piperine-lang/src/grammar/item.rs:1086-1121`
**Depends on**: T30
**Reuses**: existing `connectrules` production
**Requirement**: LRMC-C7-02

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Syntax 7-6's forms parse into the AST
- [ ] `annex_a_grammar.rs`'s A.7.x entry updated if it names this production
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): connectrules auto-insertion statement syntax (LRMC-C7-02)`

---

### T32: `connect_mode` and the parameter-passing attribute

**What**: `merged`/`split` semantics and §7.7.3's parameter passing applied at insertion.
**Where**: `crates/piperine-lang/src/grammar/item.rs`, `elab/lower/connect.rs`
**Depends on**: T31
**Reuses**: T8's shared eligibility check; existing keywords (`merged`/`split` already lexed)
**Requirement**: LRMC-C7-03

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `merged` inserts one connect module per net; `split` segments per §7.8.2/§7.8.3 — asserted on the LRM's own example
- [ ] Parameters pass through to the inserted module
- [ ] Ambiguous competing rules still fail loud (the message at `pom/error.rs:299` now names a real mechanism)
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): connect_mode and connect-module parameter passing (LRMC-C7-03)`

---

### T33: `connect ... resolveto`

**What**: Parse and apply the forced-resolution statement.
**Where**: `crates/piperine-lang/src/grammar/item.rs`, `elab/lower/disciplines.rs:19`
**Depends on**: T32
**Reuses**: T7's resolution rules
**Requirement**: LRMC-C7-04, LRMC-C10-20

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] §7.4.4.3's form parses and forces the named discipline
- [ ] `disciplines.rs`'s "future work" doc comment removed
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): connect ... resolveto (LRMC-C7-04/C10-20)`

---

### T34: `DriverTable` in the digital state

**What**: Per-net driver records (source, value, strength, delay, next state) the driver-access functions read.
**Where**: `crates/piperine-solver/src/digital/drivers.rs` (new), `state.rs`
**Depends on**: T33
**Reuses**: `LogicValue::resolve` (`digital/events.rs:20`) and `gates.rs`'s `Strength` table
**Requirement**: LRMC-C7-05 (data half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Multiple drivers on one net produce a correct, queryable driver list
- [ ] Unit tests over §7.9 driver/receiver segregation cases
- [ ] Gate check passes: `cargo test -p piperine-solver`

**Tests**: unit · **Gate**: quick
**Commit**: `feat(solver): per-net driver table for connect-module resolution (LRMC-C7-05)`

---

### T35: Driver-access system functions

**What**: `$driver_count`/`$driver_state`/`$driver_strength`/`$driver_delay`/`$driver_next_state`/`$driver_next_strength`/`$driver_type` and the `driver_update` task, wired to `DriverTable`.
**Where**: `crates/piperine-lang/src/eval/tasks.rs`, `crates/piperine-codegen/src/emit/runtime_io.rs`, `kernel/digital/`
**Depends on**: T34
**Reuses**: T5's trampoline mechanism; `SystemTasks::TABLE`
**Requirement**: LRMC-C7-05 (surface half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] All eight names parse, elaborate, and return live data inside a `connectmodule` body
- [ ] §7.6's own worked driver-resolution example simulates correctly
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): driver-access system functions (LRMC-C7-05)`

---

### T36: §7.3.6 cross-domain concurrency rules

**What**: Make each of the five named rules an explicit, LRM-cited check with its own fixture.
**Where**: `crates/piperine-lang/src/elab/typecheck.rs`
**Depends on**: T35
**Reuses**: Annex C enforcement structure (T27)
**Requirement**: LRMC-C7-06

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] One fixture per rule asserting the LRM-specified legality/semantics
- [ ] Each check's error names the §7.3.6 sub-clause
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): enforce §7.3.6 cross-domain concurrency rules (LRMC-C7-06)`

---

### T37: Connect modules reachable end-to-end

**What**: The proof that a real `.vams` source now bridges disciplines through `connectrules` instead of dying at `E4003`.
**Where**: `tests/vams_connect_modules_e2e.rs` (extend), fixtures
**Depends on**: T36
**Reuses**: T7/T8's resolution + eligibility; `InsertConnectModules`'s selection algorithm
**Requirement**: LRMC-C7-01

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] A mixed-discipline design elaborates, inserts the connect module, and simulates end-to-end from source (no hand-built `Design`)
- [ ] `connect.rs`'s carried-forward SPEC_DEVIATION note removed (it is no longer true)
- [ ] An ineligible mismatch still errors, now naming the missing rule
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): connect-module insertion reachable from real source (LRMC-C7-01)`

---

### T38: Stratified region scheduling wired into the live scheduler

**What**: `DigitalEvent` carries a region; the production scheduler drains through `RegionQueue`.
**Where**: `crates/piperine-solver/src/digital/{events.rs,scheduler.rs,state.rs}`
**Depends on**: T37
**Reuses**: `regions.rs`'s `RegionQueue` (already correct and unit-tested)
**Requirement**: LRMC-C7-12

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] §8.3's canonical example produces the LRM's specified ordering through the *production* path
- [ ] `Inactive` (`#0`) and `Monitor` regions exist and are exercised
- [ ] `core/element.rs`'s trait surface untouched (MD-46) — asserted by diff review noted in the commit body; if impossible, STOP and record MD-48 instead
- [ ] `regions.rs`'s "never wired" SPEC_DEVIATION note removed
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(solver): drain digital events through the four stratified regions (LRMC-C7-12)`

---

### T39: Gate primitives and UDPs into the POM

**What**: Lower `GateInstantiation` and `PrimitiveDecl` from the AST into the POM.
**Where**: `crates/piperine-lang/src/elab/lower/vams.rs`, `pom/module.rs`
**Depends on**: T38
**Reuses**: existing instance lowering; the AST nodes already parsed at T11-era work
**Requirement**: LRMC-C7-13 (lowering half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] A module instantiating `and`/`nand`/`buf` and a user `primitive` reaches the POM with its ports and table intact
- [ ] Unsupported switch-level primitives (`nmos`/`tran`/...) fail loud naming the gap
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): lower gate primitives and UDPs into the POM (LRMC-C7-13)`

---

### T40: Gate/UDP dispatch in the digital compiler

**What**: Give `GatePrimitive`/`UdpTable` their caller so instances produce scheduled events.
**Where**: `crates/piperine-codegen/src/kernel/digital/compile.rs`, `device/digital.rs`
**Depends on**: T39
**Reuses**: `kernel/digital/{gates.rs,udp.rs}` (LRM truth tables, already tested in isolation)
**Requirement**: LRMC-C7-13 (dispatch half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] A gate-level testbench with UDPs simulates and produces the LRM-specified values
- [ ] `gates.rs`/`udp.rs`'s SPEC_DEVIATION headers removed
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): dispatch gate primitives and UDPs in the digital compiler (LRMC-C7-13)`

---

### T41: `absdelta`-interpolated A2D events (§8.4.6)

**What**: Interpolate the crossing time at an analog-to-digital boundary rather than snapping to a sample.
**Where**: `crates/piperine-solver/src/digital/interface.rs`
**Depends on**: T40
**Reuses**: T23's detector; `piperine-signal::events::predict_crossing`
**Requirement**: LRMC-C7-14

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] A2D event time matches the interpolated crossing within the published error bound
- [ ] Non-interpolated behavior unchanged when `absdelta` is not in play
- [ ] Gate check passes: `cargo test -p piperine-solver`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(solver): absdelta-interpolated A2D event timing (LRMC-C7-14)`

---

### T42: `$discontinuity` with real numerical effect

**What**: Thread the call (and its `degree`) into step/order control instead of discarding it.
**Where**: `crates/piperine-codegen/src/flatten/analog.rs:549`, `kernel/analog/compile.rs`, `device/analog/instance.rs`
**Depends on**: T41
**Reuses**: `$bound_step`'s existing end-to-end path (`analog.rs:538-545` → `compile.rs:769-776` → `instance.rs:1501`)
**Requirement**: LRMC-C7-11

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] A discontinuous model announcing `$discontinuity` converges where it otherwise mis-steps — asserted numerically
- [ ] The empty match arm is gone
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): $discontinuity affects step control (LRMC-C7-11)`

---

### T43: `$display`-family end-to-end output

**What**: Digital and analog `$display`/`$write`/`$strobe`/`$monitor` produce real output through the sink.
**Where**: `crates/piperine-codegen/src/emit/stmt.rs:100-109`, `flatten/analog.rs:536-567`, `piperine-api/src/session/`
**Depends on**: T42
**Reuses**: T4/T5's sink + trampolines; T38's `Monitor` region for `$strobe`/`$monitor`; `eval/tasks.rs::format_display`
**Requirement**: LRMC-C7-07

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `$display` in a digital `always` block prints during a transient run (captured via `BufferSink`)
- [ ] `$display` in an `analog` block prints once per accepted step, not per Newton iteration
- [ ] `$strobe`/`$monitor` read settled values (Monitor region)
- [ ] `SessionBuilder::output(...)` exposed and covered by a root host test
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): $display family reaches the host output path (LRMC-C7-07)`

---

### T44: Format specifiers and escape sequences

**What**: Complete Table 9-1 specifier coverage plus §9.4.2 escapes and §9.4.4 `%m`, now that a consumer exists.
**Where**: `crates/piperine-lang/src/eval/tasks.rs:181-247`
**Depends on**: T43
**Reuses**: the existing specifier implementation; `LogicVec` for `%b`/`%h`/`%v` four-state rendering
**Requirement**: LRMC-C7-08

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `\n`, `\t`, `\\`, `\%`, octal `\ddd` handled
- [ ] `%b`/`%h` render `x`/`z` bits correctly (not as 0)
- [ ] `%m` prints the hierarchical scope name
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): complete format specifiers and escape sequences (LRMC-C7-08)`

---

### T45: Rewrite the system-task coverage test in VAMS

**What**: Replace the dead PHDL-syntax fixture with real `.vams` sources exercised through `parse_and_elaborate`.
**Where**: `crates/piperine-lang/tests/system_task_migration_completeness.rs`
**Depends on**: T44
**Reuses**: the target's existing intent; `tests/fixtures*` conventions
**Requirement**: LRMC-C7-10

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] All 12 tests pass (baseline was 12/12 failing — T1's record is the before)
- [ ] Each asserts the task's *effect*, not merely that the name resolves
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `test(lang): rewrite system-task coverage against real VAMS sources (LRMC-C7-10)`

---

### T46: §9.5 file I/O tasks

**What**: `$fopen`/`$fclose`/`$fdisplay`/`$fwrite`/`$fstrobe`/`$fmonitor`/`$fscanf`/`$sscanf`/`$rewind`/`$fseek`/`$ftell`/`$ferror`/`$feof`/`$fflush`/`$fgetc`.
**Where**: `crates/piperine-lang/src/eval/tasks.rs`, `crates/piperine-solver/src/core/output.rs` (`FileTable`), trampolines
**Depends on**: T45
**Reuses**: T4's `FileTable`; T44's format machinery
**Requirement**: LRMC-C7-09 (§9.5)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Round-trip test: write a file with `$fdisplay`, read it back with `$fscanf`
- [ ] Descriptor errors (bad handle, unwritable path, EOF) all typed and loud
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): §9.5 file I/O system tasks (LRMC-C7-09)`

---

### T47: §9.9/§9.13 stochastic tasks

**What**: `$random`/`$arandom` and the `$dist_*` family, matching the LRM's own published algorithm.
**Where**: `crates/piperine-lang/src/eval/tasks.rs` (+ a `rng` helper module)
**Depends on**: T46
**Reuses**: the LRM's printed reference implementation as the oracle
**Requirement**: LRMC-C7-09 (§9.9/§9.13)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Seeded sequences match the LRM's published values exactly
- [ ] Every `$dist_*` variant covered; seed in/out semantics correct
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: unit · **Gate**: quick
**Commit**: `feat(lang): §9.9/§9.13 stochastic system functions (LRMC-C7-09)`

---

### T48: §9.10 time functions and §9.12 command-line input

**What**: `$time`/`$realtime`/`$stime`, `$timeformat`, `$test$plusargs`/`$value$plusargs`.
**Where**: `crates/piperine-lang/src/eval/tasks.rs`, `SimCtx`/`Session` (host argv)
**Depends on**: T47
**Reuses**: existing `$abstime`; `SimCtx.abstime`
**Requirement**: LRMC-C7-09 (§9.6/§9.10/§9.12)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `$time`/`$realtime`/`$stime` differ correctly per the LRM (scaled/rounded vs. real)
- [ ] `$timeformat` affects `%t` rendering
- [ ] Plusargs supplied through the host surface reach the model (Rust and Python, MD-22)
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): §9.10 time functions and §9.12 plusargs (LRMC-C7-09)`

---

### T49: §9.16 `$simprobe` and §9.18 hierarchical parameter functions

**What**: The dynamic simulation probe and `$mfactor`/hierarchical system parameters.
**Where**: `crates/piperine-lang/src/eval/tasks.rs`, `crates/piperine-codegen/src/emit/analog_expr.rs` (read-only additions), `piperine-solver` introspection
**Depends on**: T48
**Reuses**: `SimCtx.mfactor`; `core/introspect.rs`; `Net` naming layer
**Requirement**: LRMC-C7-09 (§9.16/§9.18)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `$simprobe` returns the probed quantity during a run
- [ ] `$mfactor` and hierarchical parameter access match §9.18
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): §9.16 $simprobe and §9.18 hierarchical parameters (LRMC-C7-09)`

---

### T50: §9.19 `$port_connected` and §9.20 node-alias functions

**What**: Explicit binding detection plus `$analog_node_alias`/`$analog_port_alias`.
**Where**: `crates/piperine-lang/src/eval/tasks.rs`, `elab/lower/vams.rs`
**Depends on**: T49
**Reuses**: POM instance/port connection data
**Requirement**: LRMC-C7-09 (§9.19/§9.20)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `$port_connected` distinguishes connected/unconnected/implicitly-connected per §9.19
- [ ] Alias functions produce the LRM-specified aliasing behavior
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): §9.19 $port_connected and §9.20 node aliases (LRMC-C7-09)`

---

### T51: §9.21 `$table_model`

**What**: The table-interpolation system function (distinct from the `table()` operator).
**Where**: `crates/piperine-lang/src/eval/tasks.rs`, `crates/piperine-signal/src/behavior/table.rs` (reuse), codegen wiring
**Depends on**: T50
**Reuses**: `piperine-signal`'s existing `table` interpolation
**Requirement**: LRMC-C7-09 (§9.21)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] §9.21's own example (multidimensional interpolation + control string) matches the LRM's specified values
- [ ] File-backed table data loads through `FileTable`
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): §9.21 $table_model system function (LRMC-C7-09)`

---

### T52: 1364-inherited directives

**What**: `` `celldefine ``/`` `endcelldefine ``, `` `line ``, `` `pragma ``, `` `default_nettype ``, `` `nounconnected_drive ``/`` `unconnected_drive ``.
**Where**: `crates/piperine-lang/src/preprocessor.rs:191-271`, `directives.rs`
**Depends on**: T51
**Reuses**: T6's `DirectiveState`
**Requirement**: LRMC-C10-05, C10-06, C10-07, C10-08, C10-09

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Each directive parses and records its state; none raise `UndefinedMacro`
- [ ] `` `line `` remaps reported spans (error message asserts the remapped line)
- [ ] `` `default_nettype none `` makes an undeclared net an error
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): 1364-inherited compiler directives (LRMC-C10-05..09)`

---

### T53: `` `timescale ``/`` `resetall `` real behavior

**What**: Make the pass-through claim true — recorded state, no orphan `Tok::Directive`, and the false doc comment corrected.
**Where**: `crates/piperine-lang/src/preprocessor.rs:11-14,258-266`, `lexer.rs:67,162`
**Depends on**: T52
**Reuses**: T6's `DirectiveState`
**Requirement**: LRMC-C10-10

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] A file containing `` `timescale 1ns/1ps `` parses (today: unexpected-token error)
- [ ] Timescale feeds `$timeformat`/`%t` rendering (T48)
- [ ] `Tok::Directive` either consumed or removed — no token nothing consumes
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `fix(lang): make timescale/resetall handling real (LRMC-C10-10)`

---

### T54: §10.5 predefined macros

**What**: Pre-register `__VAMS_ENABLE__` and `__VAMS_COMPACT_MODELING__`.
**Where**: `crates/piperine-lang/src/preprocessor.rs:76-78`
**Depends on**: T53
**Reuses**: existing macro table
**Requirement**: LRMC-C10-03

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] The LRM's §10.5 portability idiom takes the `ifdef` branch, not the `else`
- [ ] `` `undef ``-ing them behaves per the LRM
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `fix(lang): predefine the §10.5 VAMS macros (LRMC-C10-03)`

---

### T55: `` `begin_keywords ``/`` `end_keywords ``

**What**: A scoped keyword set so a non-VAMS scope frees identifiers like `sin`.
**Where**: `crates/piperine-lang/src/{preprocessor.rs,keywords.rs,lexer.rs}`
**Depends on**: T54
**Reuses**: `keywords.rs`'s existing reserved set + `annex_b_keywords.rs` gate
**Requirement**: LRMC-C10-04

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] §10.6's own example (`sin` as a port name under a narrower scope) parses
- [ ] Unbalanced `` `begin_keywords `` fails loud
- [ ] `annex_b_keywords.rs` stays green
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): begin_keywords/end_keywords scoping (LRMC-C10-04)`

---

### T56: `` `default_transition `` semantics

**What**: Apply the directive's default rise/fall times to `transition()` calls that omit them.
**Where**: `crates/piperine-codegen/src/{resolve/pom/analog_ops.rs,flatten/analog.rs}`
**Depends on**: T55
**Reuses**: T6's parse + `DirectiveState`; existing `transition` operator
**Requirement**: LRMC-C10-02 (semantic half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] A `transition()` without explicit times uses the in-force directive value — asserted numerically
- [ ] Explicit arguments still win
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): apply default_transition to transition() (LRMC-C10-02)`

---

### T57: Port Annex D.3 `driver_access.vams`

**What**: The standard driver-access constants header, seeded like D.1/D.2.
**Where**: `crates/piperine-lang/headers/driver_access.vams` (new), builtin `SourceMap` seeding
**Depends on**: T56
**Reuses**: `headers/{disciplines,constants}.vams` porting pattern; T35's consumers
**Requirement**: LRMC-C10-16

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Content matches `VAMS-LRM-2-4.md` Annex D.3 (test compares against the LRM text)
- [ ] `` `include "driver_access.vams" `` resolves in a project with no dependency
- [ ] Constants usable with T35's `$driver_*` return values
- [ ] Gate check passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): port Annex D.3 driver_access.vams (LRMC-C10-16)`

---

### T58: Annex E degraded rows

**What**: Close the in-scope Annex E gaps (`vdslim`↔`limvds` naming, `ipwl`/`vpwl` PWL sources, `mesfet`), leaving only the netlist-reader-dependent E.2.1/E.3.2 out of scope.
**Where**: `crates/piperine-lang/src/annex_e.rs`, `crates/piperine-lang/headers/spice/`, `emit/analog_expr.rs` (naming only)
**Depends on**: T57
**Reuses**: `annex_e.rs`'s typed table as the gate; `tests/ngspice_*.rs` oracles
**Requirement**: spec P5 AC5

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every Table E.1/E.2 row in scope is `ExactMatch`/`AliasedName`, none `NotSupported`
- [ ] ngspice oracle targets stay green (this task touches `emit/analog_expr.rs` — the "not casually" file)
- [ ] E.2.1/E.3.2 remain disclosed as Out of Scope (LRMC-C10-17)
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): close in-scope Annex E mapping gaps`

---

### T59: Event-grained VPI callback registry

**What**: Registration on concrete simulation events with real deregistration, replacing `HookPhase` granularity as the VPI mechanism.
**Where**: `crates/piperine-plugin/src/{host.rs,registry.rs,vpi.rs}`
**Depends on**: T58
**Reuses**: `VpiTaskRegistry` (`eval/tasks.rs:349-430`); T38's region drain points as the event sites
**Requirement**: LRMC-C10-11

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `register_cb` returns a `CallbackId`; the callback fires at exactly the requested event
- [ ] `remove_cb` deregisters (asserted: no further fires)
- [ ] `HookPhase` lifecycle hooks still work unchanged
- [ ] Gate check passes: `cargo test -p piperine-plugin`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(plugin): event-grained VPI callback registry (LRMC-C10-11)`

---

### T60: Digital delay / timing-check model

**What**: The per-net/per-path delay records the scheduler and gate primitives both need.
**Where**: `crates/piperine-solver/src/digital/delays.rs` (new), `scheduler.rs`
**Depends on**: T59
**Reuses**: T34's `DriverTable`; T38's region scheduling
**Requirement**: LRMC-C10-12 (model half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Gate `#` delays produce correctly-timed events
- [ ] Timing-check records readable and writable
- [ ] Gate check passes: `cargo test -p piperine-solver`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(solver): digital delay and timing-check model (LRMC-C10-12)`

---

### T61: `vpi_get_delays`/`vpi_put_delays` equivalents

**What**: Native read/write routines over T60's model; `vpi.rs`'s `DelayProcessing` row flips to `Native`.
**Where**: `crates/piperine-plugin/src/{host.rs,vpi.rs}`
**Depends on**: T60
**Reuses**: T60's model; T57's `driver_access.vams` constants for the driver-info interpretation
**Requirement**: LRMC-C10-12 (surface half)

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] A plugin reads a delay, writes it, and observes the changed event timing in a live run
- [ ] `vpi_routines.rs` asserts the row is `Native`
- [ ] Gate check passes: `cargo test -p piperine-plugin`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(plugin): native delay-processing routines (LRMC-C10-12)`

---

### T62: `vpi_put_value` write path

**What**: Drive an object's value into a running solve, symmetric with the existing read path.
**Where**: `crates/piperine-plugin/src/{host.rs,vpi.rs}`, solver `SetQueue`
**Depends on**: T61
**Reuses**: the live-params `SetQueue` mechanism; `SolveResultView` read path
**Requirement**: LRMC-C10-13

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] A plugin writing a node/param value changes the subsequent solve — asserted numerically
- [ ] Illegal write targets fail loud
- [ ] MD-18 respected: no re-JIT, the write restamps on the compiled circuit
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `feat(plugin): vpi_put_value write path (LRMC-C10-13)`

---

### T63: `vpi_mcd_*` family over `FileTable`

**What**: Multi-channel descriptors as a channel mask over the same file table.
**Where**: `crates/piperine-plugin/src/{host.rs,vpi.rs}`, `piperine-solver/src/core/output.rs`
**Depends on**: T62
**Reuses**: T4's `FileTable`; T46's file-I/O tasks
**Requirement**: LRMC-C10-14

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] `mcd_open`/`mcd_close`/`mcd_printf`/`mcd_flush` equivalents write to every selected channel
- [ ] `vpi_printf` routes to the same sink
- [ ] Gate check passes: `cargo test -p piperine-plugin`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(plugin): vpi_mcd_* family over the file table (LRMC-C10-14)`

---

### T64: Flip the VPI category table and lock it

**What**: Every in-scope row becomes `Native`; the table stays the regression gate.
**Where**: `crates/piperine-plugin/src/vpi.rs:77-186`, `crates/piperine-plugin/tests/vpi_routines.rs`
**Depends on**: T63
**Reuses**: the existing typed-support-table pattern
**Requirement**: spec Success Criteria 4

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] No `Partial`/`NotSupported` remains for anything P5 covers
- [ ] The test fails if a row regresses (proven able to fail)
- [ ] Gate check passes: `cargo test -p piperine-plugin`

**Tests**: integration · **Gate**: quick
**Commit**: `test(plugin): lock every in-scope VPI category as Native`

---

### T65: Follow-up conformance audit pass

**What**: Re-run the audit's own four-chapter-range methodology against the closed tree.
**Where**: `docs/audit/followup-2026.md` (new)
**Depends on**: T64
**Reuses**: `docs/audit/README.md`'s stated method (four independent passes, clause + `file:line` evidence, negative greps)
**Requirement**: spec Success Criteria 2

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every closed LRMC-ID re-checked against the LRM text and the current source
- [ ] Zero repeat findings for anything this feature closed; any new finding recorded with evidence
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: none (report artifact; the matrix gate at T67 is the mechanical half) · **Gate**: build
**Commit**: `docs(audit): follow-up LRM conformance pass after closure`

---

### T66: Update audit rows and correct the spec docs

**What**: Each `docs/audit/*.md` row references its closing commit; the `laplace_*` claims in `docs/spec/` are corrected.
**Where**: `docs/audit/{ch1-4,ch5-6,ch7-9,ch10-12-annexes,README}.md`, `docs/spec/{part_v_builtins.md,part_i_language.md}`, `ROADMAP.md`, `CLAUDE.md` (known-gaps section)
**Depends on**: T65
**Reuses**: the audit tables' existing shape
**Requirement**: spec Success Criteria 1

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] Every non-Out-of-Scope row carries its closing commit hash
- [ ] `part_v_builtins.md`'s "all five LRM variants registered" claim corrected to the LRM's four
- [ ] `CLAUDE.md`'s known-gaps list reflects what actually remains
- [ ] Gate check passes: `cargo build --workspace`

**Tests**: none (docs) · **Gate**: build
**Commit**: `docs(audit): mark LRM findings closed with their commits`

---

### T67: Populate and lock the conformance matrix

**What**: Flip every LRMC-ID in the gate from `Pending` to its closing test, and run the final full-workspace verification against T1's baseline.
**Where**: `tests/lrm_conformance_matrix.rs`
**Depends on**: T66
**Reuses**: T9's skeleton; T1's baseline
**Requirement**: spec Success Criteria 1 and 3

**Tools**: MCP: NONE · Skill: NONE

**Done when**:
- [ ] All 51 IDs bound to a named, existing, non-ignored test; zero `Pending`
- [ ] A/B against T1's baseline: no target that passed before fails now (target-by-target, not aggregate)
- [ ] `cargo build --workspace` zero warnings
- [ ] `tests/suite_hygiene.rs` green (no disabled code, no `#[ignore]`, every new target has a `//!` header)
- [ ] Gate check passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `test(lrmc): lock the conformance matrix over all 51 findings`

---

## Phase Execution Map

```
Phase 0 → 1a → 1b → 2 → 3a → 3b → 3c → 4 → 5 → 6

Phase 0 : T1 → T2 → T3 → T4 → T5 → T6 → T7 → T8 → T9
Phase 1a: T10 → T11 → T12 → T13 → T14 → T15
Phase 1b: T16 → T17 → T18 → T19 → T20 → T21
Phase 2 : T22 → T23 → T24 → T25 → T26 → T27 → T28 → T29 → T30
Phase 3a: T31 → T32 → T33 → T34 → T35 → T36 → T37
Phase 3b: T38 → T39 → T40 → T41 → T42
Phase 3c: T43 → T44 → T45 → T46 → T47 → T48 → T49 → T50 → T51
Phase 4 : T52 → T53 → T54 → T55 → T56 → T57 → T58
Phase 5 : T59 → T60 → T61 → T62 → T63 → T64
Phase 6 : T65 → T66 → T67
```

67 tasks / 10 phases → Execute packs into ~10 batches of ~7 (phase boundaries only).

---

## Task Granularity Check

| Task group | Scope | Status |
|---|---|---|
| T1, T9, T67 | one artifact / one guard file | ✅ Granular |
| T2, T4, T34, T60 | one new type + its tests | ✅ Granular |
| T3, T10-T12, T19-T22, T42, T54 | one function / one match arm | ✅ Granular |
| T5, T13-T18, T23-T33, T35-T41, T43-T51, T52-T59, T61-T64 | one mechanism in one-to-two cohesive files | ✅ Granular |
| Split enforced | `absdelta` (T23 semantics / T24 wiring), named events (T25 form / T26 runtime), gates+UDP (T39 lowering / T40 dispatch), driver access (T34 data / T35 surface), delays (T60 model / T61 surface), assignment pattern (T13 parse / T14 elab) | ✅ Split at the crate/layer seam |
| T52 | five directives in one file | ⚠️ OK — one mechanism (`DirectiveState` arm), one file, cohesive |
| T46 | fifteen §9.5 names | ⚠️ OK — one subsection, one table + one sink consumer; splitting per-name would fragment one shared descriptor model |

## Diagram-Definition Cross-Check

| Task | Depends on (body) | Diagram shows | Status |
|---|---|---|---|
| T1 | None | phase head | ✅ |
| T2, T4, T6, T9 | T1 | T1 → … (Phase 0 chain) | ✅ |
| T3 | T2 | T2 → T3 | ✅ |
| T5 | T4 | T4 → T5 | ✅ |
| T7 | T6 | T6 → T7 | ✅ |
| T8 | T7 | T7 → T8 | ✅ |
| T10 | T3 | Phase 0 → Phase 1a | ✅ |
| T11-T21 | previous task | linear chain | ✅ |
| T22-T30 | previous task | linear chain | ✅ |
| T31-T37 | previous task | linear chain | ✅ |
| T38-T42 | previous task | linear chain | ✅ |
| T43-T51 | previous task | linear chain | ✅ |
| T52-T58 | previous task | linear chain | ✅ |
| T59-T64 | previous task | linear chain | ✅ |
| T65-T67 | previous task | linear chain | ✅ |

Backward-only rule: every dependency points to an earlier task in the same or an earlier phase. Phase 0's internal chain is drawn linear (T1→…→T9) although T2/T4/T6/T9 depend only on T1 — sequential execution makes the linear diagram accurate; no task depends on a later one. ✅

## Test Co-location Validation

| Task | Layer created/modified | Matrix requires | Task says | Status |
|---|---|---|---|---|
| T1 | baseline artifact | none | none | ✅ |
| T2 | pure semantics helper | unit | unit | ✅ |
| T3, T10-T20, T25, T28-T33, T36, T39, T44-T45, T50, T52-T55, T57 | lexer/grammar/elaborator | integration | integration | ✅ |
| T4, T23, T34, T47 | pure semantics helper / new solver type | unit | unit | ✅ |
| T5, T18, T21, T24, T26, T27, T30, T35, T37, T38, T40, T42, T43, T46, T48, T49, T51, T56, T58, T62, T67 | cross-crate / JIT ABI / host | integration | integration | ✅ |
| T22, T41, T59, T60, T61, T63, T64 | codegen / solver / plugin | integration | integration | ✅ |
| T6, T7, T8, T9, T13, T14, T15, T29 | elaborator / guard | integration | integration | ✅ |
| T65, T66 | report + docs | none | none | ✅ |

No `Tests: none` is used as deferral: T1/T65/T66 produce no code, and T67's matrix gate is the mechanical check for T65/T66's claims.

---

## Tools

No MCP servers or skills are needed for any task — this is an in-repo Rust workspace change set, and the LRM (`VAMS-LRM-2-4.md`) is in the tree. `Tools: MCP: NONE · Skill: NONE` on every task reflects that, not an omission.
