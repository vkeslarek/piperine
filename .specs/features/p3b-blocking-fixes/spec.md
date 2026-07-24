# P3b — Blocking-Bug Fixes Specification

**Scope: Medium.** Three ROADMAP.md P3b gap-catalog rows, each `file:line`
evidenced. Re-verified against current source 2026-07-24 (code has moved
since the gap catalog was written; findings below reflect what's actually
in the tree today, not the stale catalog text).

## Problem Statement

`piperine build` is a stub that never calls the compiler. Digital codegen
rejects three constructs analog codegen already supports (user-`fn` inlining,
enum-pattern `match`, real↔4-state conversion), so valid-looking digital
PHDL fails to compile with no path forward. Both block a simulation or full
user use of the CLI/language — bugs, not missing features. **Re-verification
correction:** the ROADMAP's third row (`.tf` input impedance for a
current-source input returns a placeholder `1e20`) is **not reachable as
described** — see Finding below. The scope for that row changes accordingly.

## Verified Findings (2026-07-24)

| # | ROADMAP claim | Re-verification | Verdict |
|---|----------------|------------------|---------|
| 1 | `piperine build` stub, `build.rs:33` never calls compiler/elaborator | Confirmed: `crates/piperine-cli/src/commands/build.rs:33` is `// TODO: call compiler/elaborator` after printing "Building design for: …" — no elaboration, no codegen | **Real bug, in scope** |
| 2a | user-`fn` inlining missing in digital, `emit/builder.rs:334` | Confirmed: `call_expr` in `crates/piperine-codegen/src/emit/builder.rs` hits a resolved fn id and returns `CodegenError::unsupported("user function ... inlining in digital codegen — not yet implemented via POM path")` | **Real bug, in scope** |
| 2b | enum-pattern/`match` resolution missing in digital, `emit/stmt.rs:234` | Confirmed: `pattern_flag` in `crates/piperine-codegen/src/emit/stmt.rs`, `Pattern::Path` arm returns `CodegenError::unsupported("enum pattern ... enum resolution not yet wired")` | **Real bug, in scope** |
| 2c | real↔4-state conversion missing in digital, `emit/builder.rs:679` | Confirmed: the coercion match in `crates/piperine-codegen/src/emit/builder.rs`, `(DigTy::Quad, DigTy::Real) \| (DigTy::Real, DigTy::Quad)` arm returns `CodegenError::unsupported("real ↔ 4-state conversion in digital codegen")` | **Real bug, in scope** |
| 3 | `.tf` input impedance for current-source input returns placeholder `1e20`, `analyses/tf.rs:394` | **Traced the call graph**: `TfDriver::run` calls `calculate_gain()` *before* `calculate_input_resistance()` (`tf.rs:194,197`). `calculate_gain` already fails loud (`Error::simple(SolverDomain::Tf, "TF: current-source input is not supported (D5)")`) whenever `input_is_voltage_source()` is `false` — so `calculate_input_resistance`'s `else { Ok(1e20) }` branch at line ~394 is **provably unreachable**: it can only run when `calculate_gain` already succeeded, which requires `input_is_voltage_source() == Some(true)`. No live code path returns a silent wrong `1e20` to a user today. | **Not the bug as described — see PB-06/07 below for the corrected, narrower scope** |

## Goals

- [ ] `piperine build` actually elaborates and compiles the target design,
      surfacing real errors instead of silently succeeding on broken PHDL.
- [ ] Digital codegen accepts user-`fn` calls, enum-pattern `match`, and
      real↔4-state (`Quad`) coercions — the same constructs analog codegen
      already lowers.
- [ ] The `.tf` dead-code path is corrected: remove the unreachable
      placeholder and its misleading comment, and make the actual boundary
      (current-source `.tf` input is unsupported, D5) fail loud with a clear
      message at the point of first contact — already true for gain, extend
      the same fail-loud guarantee to input/output resistance so a future
      dead-code change can't silently reintroduce a wrong number.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Full current-source-input support for `.tf` (gain/impedance) | New solver math (D5), not a bug fix — ROADMAP explicitly scopes P3b to bugs, and P1 (solver complete) is CLOSED; this is a new capability, tracked separately if wanted |
| `for` in a digital body (`emit/stmt.rs:98`) | Explicitly parked in P6 by the existing ROADMAP note — niche, not blocking |
| Multi-bit (bus) patterns in a digital `match` | Not part of the three verified findings; a separate, larger gap (`emit/stmt.rs` `BitPattern` `_ =>` arm) — not blocking today, no evidence it's hit in practice |
| `piperine build`'s output artifact format (what "build" produces on disk) | The stub prints and stops; the fix makes it actually run the pipeline and report errors — persisting a build artifact is a separate, undesigned feature |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| What does `piperine build` build, given there's no "top module" concept enforced at elaboration (`Design::top()` is only ever set by test code and the selector evaluator, never by the CLI elaboration path)? | `build` elaborates the target file(s) (same file-discovery as `check`), then for every **zero-port module** in each elaborated design (a module with no `inout`/`input`/`output` ports is a self-contained circuit, matching every example's `mod Board()`/`mod Top()` convention — see `examples/*.phdl`), runs the full codegen pipeline (`lower_bodies` + `CircuitCompiler::new`) to catch compile-time (not just elaboration-time) errors. Reports per-module pass/fail; a project with zero zero-port modules is not an error (library-only project) but prints a note. | No existing "top" marker exists in `Piperine.toml` or PHDL syntax; zero-port is the only structural signal every example already follows, and it's exactly the signal `CircuitCompiler`/`Session::compile` need (a circuit root has nothing left to connect) | n (assumption — no user available synchronously this session; logged per Medium-scope closure gate) |
| The `.tf` finding is corrected scope (dead-code cleanup + defensive fail-loud), not the ROADMAP's literal "fix the wrong number" | See Verified Findings row 3 | Re-verification is mandatory per this task's own instructions ("verify each file:line claim... code may have moved"); implementing new current-source `.tf` math would be scope creep into engine work explicitly marked P1-CLOSED/out-of-scope elsewhere | y (self-resolved via code tracing, not a judgment call) |
| Digital `fn` inlining reuses the analog inliner's approach vs. a digital-specific one | Reuse the same POM-level fn-inlining approach analog codegen already uses (`flatten/analog.rs`'s fn inliner, per CLAUDE.md "PHDL pre-folds param defaults... honored by both the interpreter and codegen's fn inliner") — adapted for the digital `Codegen`/`Builder` trait rather than duplicated logic where the two overlap (both need to substitute call args into a copied body and lower it in the caller's context) | CLAUDE.md explicitly names this shared mechanism; avoids two divergent inliners for the same PHDL construct | n (Design-level judgment, resolved during Execute by following the existing analog inliner's shape) |

**Open questions:** none — all resolved or logged above.

---

## User Stories

### P1: `piperine build` actually builds ⭐ MVP

**User Story**: As a Piperine user, I want `piperine build` to actually
elaborate and compile my design so that I find codegen errors before trying
to `run`/`test`, not after.

**Why P1**: Blocks the core CLI workflow — a command with the express purpose
of validating a design currently validates nothing beyond "the file exists."

**Acceptance Criteria**:

1. WHEN `piperine build` runs on a project with a valid zero-port top module
   THEN the command SHALL elaborate the design, run the full codegen pipeline
   (`lower_bodies` + `CircuitCompiler::new`) on every zero-port module, and
   print a per-module success line, exiting `0`.
2. WHEN the target PHDL fails to elaborate (parse/const-eval/instantiation
   error) THEN `piperine build` SHALL print the elaboration error and exit
   non-zero — never silently succeed.
3. WHEN elaboration succeeds but codegen fails for a zero-port module (e.g. an
   `Unsupported` construct) THEN `piperine build` SHALL print the codegen
   error attributed to that module and exit non-zero.
4. WHEN a project has no zero-port modules (library-only) THEN `piperine
   build` SHALL print a note that there is nothing to build and exit `0` (not
   an error — a library project is valid).

**Independent Test**: Run `piperine build` against a known-good example
(`examples/01_voltage_divider.phdl`) → exit 0, module reported. Run against a
PHDL file with a deliberate elaboration error → exit non-zero with the error
printed. Run against a PHDL file whose zero-port module contains a construct
that fails codegen (e.g. an unsupported operator) → exit non-zero, error
attributes the failing module.

---

### P2: Digital codegen accepts `fn`, enum `match`, and real↔`Quad` coercion

**User Story**: As a PHDL author, I want a digital body to call a user `fn`,
`match` on an enum, and mix `Real`/4-state values the same way analog bodies
do, so that a construct that elaborates without error also compiles.

**Why P2**: Each is an existing `CodegenError::Unsupported` fail point (not a
silent wrong answer — CLAUDE.md's "fail loud" rule is already honored) but
blocks otherwise-valid PHDL from compiling at all; a device author cannot
work around it from PHDL.

**Acceptance Criteria**:

1. WHEN a digital body calls a user-defined `fn` with no side effects other
   than its return expression THEN codegen SHALL inline the call (substitute
   argument expressions for the fn's parameters, lower the body in the
   caller's context) and produce a working digital kernel — no
   `CodegenError::Unsupported`.
2. WHEN a digital `match` scrutinizes a value against an enum-variant pattern
   (`Pattern::Path`) THEN codegen SHALL resolve the variant to its
   discriminant and emit the equality comparison — matching the same variant
   as the const evaluator would at elaboration time.
3. WHEN a digital body coerces a `Real` expression into a 4-state (`Quad`)
   context, or a `Quad` expression into a `Real` context THEN codegen SHALL
   emit a defined, deterministic conversion (not `CodegenError::Unsupported`).
   Conversion semantics: `Real → Quad` — any finite value maps to `1` (nonzero)
   or `0` (zero), matching analog's boolean truthiness convention already used
   elsewhere in the codebase (e.g. `V(intg) > 0.0` pattern in the README's
   delta-sigma example — a real compared to produce a boolean already exists;
   this AC is about an *unconditional* value coercion, so it reuses the
   existing `(DigTy::Int, DigTy::Real)`/`(DigTy::Real, DigTy::Int)` numeric
   round-trip already implemented two arms above, composed with the existing
   `Int↔Quad` arms already implemented immediately below it — i.e. `Real → Quad`
   = `Real → Int → Quad`, `Quad → Real` = `Quad → Int → Real`, reusing code
   already proven correct in this same function, not new conversion logic).

**Independent Test**: Hand-built `LoweredBody` fixtures (matching the existing
style in `crates/piperine-codegen/tests/digital_jit.rs`) for each of the three
constructs, compiled via `DigitalKernel::compile` and driven through the
existing event-driven test harness; each asserts the produced output value
for a known input, not just "it compiles."

---

### P3: `.tf` dead-code correction

**User Story**: As a maintainer, I want the `.tf` input-resistance
placeholder to either be provably unreachable-and-removed, or to fail loud
if it ever becomes reachable, so a future refactor can't silently
reintroduce a wrong `1e20` reading.

**Why P3**: Lowest severity of the three — re-verification showed no user can
hit this today (see Finding #3). Still worth closing: `#![allow(dead_code)]`
at the top of `tf.rs` is exactly the kind of blanket suppression that lets a
dead branch survive refactors until it isn't dead anymore.

**Acceptance Criteria**:

1. WHEN `calculate_input_resistance` is reached (which the call graph
   guarantees only happens after `calculate_gain` has already confirmed a
   voltage-source input) THEN the function SHALL NOT contain a reachable
   `else` branch implying a current-source case — the `if input_is_voltage`
   branching on a value that is structurally always `true` at this call site
   SHALL be replaced with either (a) removing the dead branch and asserting
   the invariant, or (b) a call that fails loud with a clear "unreachable —
   calculate_gain should have already rejected this" message rather than
   returning `1e20`.
2. WHEN `cargo build -p piperine-solver` runs after the fix THEN it SHALL
   compile without needing `#![allow(dead_code)]` for this reason (the
   attribute may stay if something else in the file still needs it — verify
   before removing the attribute itself).

**Independent Test**: `cargo test -p piperine-solver` (existing `.tf` tests
stay green); a new unit test constructs the driver state directly (bypassing
the public constructor's fail-loud path, since a black-box test cannot reach
this branch through the public API by definition) to prove the guard fires
if the invariant is ever violated — OR, if that's not constructible without
reaching into private internals in a way the codebase doesn't already do
elsewhere, the AC is satisfied by (a) alone plus the existing `.tf` test
suite staying green (documented as a spec-precision note, not silently
skipped).

---

## Edge Cases

- WHEN `piperine build` is run with an explicit `file` argument that isn't
  the project's `src/main.phdl` THEN it SHALL build that file specifically
  (matching `check`'s existing single-file override behavior).
- WHEN a digital `fn` call has a parameter that shadows an outer digital
  variable name THEN inlining SHALL NOT capture the outer variable —
  substitution must be hygienic (reuse whatever hygiene mechanism the analog
  inliner already has, per the fn-inlining assumption above).
- WHEN a digital `match` pattern references an enum variant that doesn't
  exist on the scrutinee's type THEN this SHALL fail loud at codegen (already
  the existing `CodegenError::Unsupported` shape — AC2 only closes the
  *valid*-variant path, not name validation, which is a separate concern
  already handled or not by earlier passes).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
|---|---|---|---|
| PB-01 | P1: build actually builds | T1 | Verified |
| PB-02 | P1: elaboration error → exit non-zero | T1 | Verified |
| PB-03 | P1: codegen error → exit non-zero, attributed | T1 | Verified |
| PB-04 | P1: no zero-port modules → note, exit 0 | T1 | Verified |
| PB-05 | P2: digital `fn` inlining | T2 | Verified |
| PB-06 | P2: digital enum-pattern `match` | T3 | Verified |
| PB-07 | P2: digital real↔`Quad` coercion | T4 | Verified |
| PB-08 | P3: `.tf` dead-code removed/guarded | T5 | Verified |

**ID format:** `PB-NN`. **Status values:** Pending → In Design → In Tasks →
Implementing → Verified. **Coverage:** 8 total, 8 mapped to tasks.
**Status**: Complete — all 8 requirements delivered, commits `2fa0e69` (T1),
`dae7e71` (T2/T3/T4), `b600700` (T5). See `validation.md` for the Verifier
report.

---

## Success Criteria

- [ ] `piperine build` on every `examples/*.phdl` with a zero-port module
      exits 0 and reports it; `cargo test -p piperine-cli` + the existing
      `tests/run_examples.rs` stay green.
- [ ] The three digital-codegen `CodegenError::Unsupported` fail points
      (fn inlining, enum pattern, real↔Quad) are gone for the AC-scoped
      cases; `cargo test -p piperine-codegen` green with new tests added.
- [ ] `tf.rs`'s dead placeholder branch is gone or provably guarded;
      `cargo test -p piperine-solver` green.
- [ ] `cargo test --workspace` green, zero new warnings.
