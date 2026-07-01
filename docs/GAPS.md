# Piperine — Gap Analysis (open items only)

> **Purpose.** The authoritative list of divergences between the language
> design (`docs/piperine-hdl-spec.md`) and the current implementation.
> This file merges what were three overlapping documents —
> `docs/GAPS.md`, `docs/NGSPICE_GAPS.md`, and `docs/AMS-BUILTIN-TASKS.md`
> — into one. Items already resolved are dropped entirely rather than
> marked done; if you're looking for "what's fixed," `git log` and the
> code are the source of truth, not this file. What remains below is, as
> of 2026-07-01, still genuinely open.
>
> **How an entry reads:** ID, one-line title, severity, what's affected,
> then 3-6 sentences: what's broken (with a `file:line` citation), why it
> matters, and the fix direction. No essay, no acceptance-criteria
> checklists — pick up `git blame`/tests near the cited line if you need
> the full historical rationale.
>
> **Severity:** Critical (silent wrong results / blocks a core promise) ·
> High (blocks a whole class of examples) · Medium (partial/parse-only) ·
> Low (polish, ergonomics, docs).
>
> **NGSPICE cross-reference.** The model files in
> `crates/piperine-lang/headers/ngspice/` (`res`, `cap`, `ind`, `mut`,
> `dio`, `bjt`, `jfet`, `mos1`, `sw`, `csw`, `vsrc`, `isrc`, `vcvs`,
> `vccs`, `ccvs`, `cccs`) are the fidelity gold standard — they now parse,
> elaborate, and lower to IR end-to-end (2026-07-01). Items below marked
> "Affects: NGSPICE semiconductors" are what's still needed for
> *device-compile* and *numeric* fidelity, not frontend support.

---

## Table of contents

- [Part A — Residual silent-bug / correctness items](#part-a)
- [Part B — Type system](#part-b)
- [Part C — Standard library](#part-c)
- [Part D — Codegen: forces, analog operators, noise, functions](#part-d)
- [Part E — Mixed-signal bridges (A2D / D2A)](#part-e)
- [Part F — `from_ir`: hierarchy, recursion](#part-f)
- [Part G — AMS frontend](#part-g)
- [Part H — Solver: integration, timestep, convergence](#part-h)
- [Part I — PHDL language features](#part-i)
- [Part J — Diagnostics, events, `$assert`](#part-j)
- [Part K — Architecture cleanup](#part-k)
- [Part L — Documentation](#part-l)
- [Part M — Model-specific numeric fidelity](#part-m)
- [Summary table](#summary-table)

---

## Part A — Residual silent-bug / correctness items {#part-a}

> Everything originally in Part A (A.1–A.16) is **done** (2026-07-01).

---

## Part B — Type system {#part-b}

> B.1 (width-matching typecheck) and B.2 (discipline-crossing rejection)
> are **done** — `crates/piperine-lang/src/elab/typecheck.rs`, with
> passing tests (`width_mismatch_on_named_connection_is_caught`, etc.).


### B.5 No `Boolean`→`Quad` implicit widening or cast enforcement

**Severity:** Medium · **Affects:** typecheck; ties to J.4

No widening/cast logic exists. `Boolean` should implicitly widen to
`Quad` (`0→0q0`, `1→0q1`); everything else needs an explicit cast
(`real(x)`/`int(x)`/`bit(x)`), which first needs those recognized as
casts rather than ordinary calls (J.4).

---

## Part C — Standard library {#part-c}

> C.1 (`Ground` predefined) and C.3 (`Type`/`Net` root capabilities) are
> **done** — see `headers/prelude.phdl`, `headers/capabilities.phdl`.

### C.2 `UInt[N]`/`SInt[N]` not in the prelude

**Severity:** High · **Affects:** any example needing generic bit-vector
arithmetic (`Accumulator`, `Driver[N]`)

Only `capabilities.phdl`/`collections.phdl` exist as always-in-scope
prelude; no `UInt[N]`/`SInt[N]` bundle-with-arithmetic-impls file. The
file itself can be added now (it would parse and elaborate), but the
`impl Add for UInt[N]` method bodies won't actually *run* until I.2
(capability dispatch) and I.3 (`Self` substitution) land — so this is
gated on those two, not just a missing file.

---

## Part D — Codegen: forces, analog operators, noise, functions {#part-d}

> D.4 (noise sources consumed by `Device::noise_current_psd`) is **done**
> end-to-end — confirmed this session: `piperine-solver/src/solver/noise.rs:106`
> calls it on real devices in the actual noise-analysis solve path, not
> just unit tests.

### D.1 Potential forces `V(p,n) <- expr` rejected (ideal voltage source)

**Severity:** Critical · **Affects:** `vsrc`, `ccvs`, `mut`, `vcvs`, any
ideal-source pattern

`ir_analog_to_device.rs` rejects every potential-nature `Force` fail-loud.
This is the single biggest "spec examples don't run" gap — blocks the
canonical `VSource`/op-amp/bit-to-voltage patterns. Needs MNA
branch-current rows (H.4 is the prerequisite) plus a JIT `force_residual`
function stamping `V+ − V− − expr = 0`.

### D.2 `idt`/`idtmod` rejected/approximate at device-compile

**Severity:** High · **Affects:** any model needing time-integral state
(phase accumulators, integrators)

Recognized in the frontend IR (`lowering/analog_ops.rs`), but
`ir_analog_to_device.rs` doesn't give them a faithful companion-model
stamp (only `Ddt` has one). Needs a reactive-split stamp mirroring
`ddt`'s mechanism but for `state_next = state_old + x·dt`.

### D.3 `ddx`, `delay`, `transition`, `slew`, `laplace_*`, `zi_*` fail-loud at device-compile

**Severity:** Medium · **Affects:** waveform-shaping/filter models
(D2A drivers, RC/filter behavioral models)

All recognized in the IR, all rejected at `ir_analog_to_device.rs`.
Suggested implementation order: `delay` (ring buffer) → `transition`
(waveform queue) → `slew` (rate limiter) → `ddx` (compile-time symbolic
derivative) → `laplace_*`/`zi_*` (state-space filters, AC-only
initially).

### D.5 User `fn` calls: analog rejects, digital silently returns 0

**Severity:** High · **Affects:** any model using a helper `fn` in an
analog contribution or digital expression

`IrFunction` tables are populated by both frontends but read by no
analog-codegen path (only the pretty-printer reads them); analog rejects
non-builtin calls fail-loud, digital silently returns 0 instead of
erroring. Needs an `inline_user_call` pass (alpha-substitute params,
inline the `Return` expr, depth-capped ~32) run before validation, plus a
purity check (no `Contrib`/`Force`/external assigns in the fn body).

### D.6 Digital interpreter: `if`/`match`/const-bounded loops unreachable from the IR path

**Severity:** High · **Affects:** any non-trivial digital block lowered
through IR (state machines, SAR ADC)

The interpreter itself handles `If`/`Match` fine, but
`ir_digital_to_interp::lower_stmt` drops them (`_ => None`) — a digital
block with `if` inside `@posedge` compiles through IR but silently loses
the branch. Needs the missing `IrStmt` variants wired into the IR→
interpreter lowering; unbounded `while`/`repeat`/`forever` should reject,
not attempt.



### D.8 `$limit("pnjlim"|"fetlim", ...)` rejected; `limexp` is stateless

**Severity:** High · **Affects:** `dio`, `bjt`, `jfet`, `mos1` — every
NGSPICE semiconductor's Newton convergence

`SimQuery::Limit` is explicitly rejected at the JIT validator
(`ir_emit.rs:570`). The `limexp` substitute (`exp(min(x,80))`) prevents
overflow but has no state — it doesn't track the previous Newton
iteration's value, which is the actual point of `pnjlim`/`fetlim`
(bounding the *voltage step* between iterations, not just clamping the
exponent). Needs a new per-call-site state slot (same mechanism as
`ddt`/`idt`) storing the prior iteration's value, plus real
`pnjlim`/`fetlim` math in `emit_sim`. This is the largest remaining hard
numerical deviation from NGSPICE for every semiconductor model.

### D.9 `ac_stim(mag, phase)` — AC analysis has no stimulus

**Severity:** Medium · **Affects:** `vsrc`, `isrc`, any AC small-signal
source

`IrExpr::AcStim` reaches the IR (confirmed this session:
`ngspice_ir_tests.rs`'s `vsrc` test asserts it) but is validated out at
device-compile time — every AC analysis of a PHDL-compiled source
currently returns the trivial zero response. Needs either the AC RHS
stamping `IrExpr::AcStim` directly, or solver-level injection keyed off
instance `ac_mag`/`ac_phase` params. Depends on D.10 to select the
AC-vs-not branch correctly.

---

## Part E — Mixed-signal bridges (A2D / D2A) {#part-e}

### E.1 Solver never passes real analog voltages to `eval_discrete`

**Severity:** Critical · **Affects:** every mixed-signal example
(comparators, SAR ADC, delta-sigma, synchronizers)

`Device::eval_discrete`'s `analog_voltages` parameter is always `&[]` at
every call site — a comparator reading `V(vp)` in a digital block reads 0
through the real solver loop. Needs `Device::analog_input_terminals()`
declared, and the transient solver building + passing a real per-device
slice after each analog solve.

### E.2 D2A bridge: analog devices never read digital state

**Severity:** Critical · **Affects:** bit-to-voltage DACs, delta-sigma
feedback, any 1-bit DAC pattern

`load_dc`/`load_transient` never see digital state — an analog block
reading a `Bit` port (`if (d == 1) { vhigh } else { vlow }`) can't. Needs
a `digital_state` parameter threaded into `Device::load_dc`/
`load_transient`, plus `PhdlDevice` declaring which digital nets its
analog body reads.

### E.3 `cross`/`above` don't drive digital state

**Severity:** High · **Affects:** comparator/level-detector patterns
crossing into digital

Parsed and validated (`elab/event.rs`), but no solver mechanism actually
detects an analog crossing and fires the corresponding digital event.
Needs `Device::analog_event_probes()` + the transient solver evaluating
each probe after every accepted step, interpolating the crossing time,
and pushing a digital event.

### E.4 `@ above`/`@ cross` never fire to update persistent `var` state

**Severity:** Medium · **Affects:** `sw`, `csw` — directly blocks switch
hysteresis

Same underlying gap as E.3, but the sink is a persistent `var` slot (real
since this session's Part-I work) instead of a digital net. `sw_state`
elaborates and lowers correctly now (`IrModule.vars` +
`IrStmt::Assign`), but nothing in the solver ever *evaluates* the
`@ above(...)` condition at runtime, so it never transitions from its
initial value. Reuses E.3's crossing-detection infrastructure once built
— only the write-target differs.

### E.5 `@ initial` never fires for analog IC/off initialization

**Severity:** Medium · **Affects:** `cap`, `ind`, `dio`, `sw`, `csw` —
every model using `@ initial { V(...) <- ic; }` or
`@ initial { sw_state = ...; }`

Digital `init_digital()` runs `initial` events at t=0 (once H.5's bug is
also fixed); there's no analog equivalent — a capacitor's
`@ initial { if (ic != 0.0) { V(p,n) <- ic; } }` never executes, so a
UIC transient starts from the solver's DC guess, not the user's
specified IC. Needs an `init_analog()` mirroring the digital machinery,
run once before the first Newton iteration. Depends on D.1 (forces) for
the `V <-` form to even compile.

---

## Part F — `from_ir`: hierarchy, recursion {#part-f}

### F.1 `from_ir` doesn't recurse into child-module instances

**Severity:** High · **Affects:** any multi-level module hierarchy

Only the top module's direct instances are walked; a child's own
`instances` are never expanded. Needs recursive `instantiate_module` with
hierarchical net-name prefixing (`parent.child.port`).

### F.2 `from_ir` doesn't compile the top module's own analog/digital body

**Severity:** High · **Affects:** any "container with parasitics"
pattern

The parent module's own `analog`/`digital` blocks are silently dropped —
only child instances become devices. Needs the top module's own body
compiled once F.3's hierarchical-name resolution exists.

### F.3 Hierarchical `name.port`/`name[i].port` references don't reduce

**Severity:** High · **Affects:** any parent-block reference into a
child's or array-instance's port

`eval_net_ref` reduces `Ident.field` but not
`Field(Index(base,i), field)` — `rseg[i].n` style refs aren't reducible.
Needs the indexed-field arm added, producing flat names matching what
`for`-unrolling already generates (`rseg_0_n`, etc.).

### F.4 Structural `for`/`if` (`generate`) not unrolled in AMS `from_ir`

**Severity:** Medium · **Affects:** AMS `generate`/`loop_generate`/
`if_generate`/`case_generate`

PHDL's structural `for` is fine (unrolled at elaboration); AMS's
`generate` constructs are dropped entirely at parse time. Needs unrolling
implemented in AMS's `convert_module` before IR lowering.

---

## Part G — AMS frontend {#part-g}

### G.1 AMS digital `initial`/`always` never reach `IrDigitalBody`

**Severity:** High · **Affects:** every pure-digital AMS/Verilog fixture

`InitialConstruct`/`AlwaysConstruct` are dropped at parse;
`from_ams.rs` hardcodes `digital: None`. The statement-lowering machinery
already exists for digital flavors inside analog blocks — the gap is
purely that top-level `initial`/`always` items are discarded before
reaching it.

### G.2 AMS `param_ports` (`#(parameter ...)`) discarded

**Severity:** Medium · **Affects:** any AMS module using header-style
parameters instead of body `parameter` decls

Parsed into `ModuleDecl.param_ports` but never merged into the module's
actual parameter list.

### G.3 AMS `Parameter.constraints` (`from`/`exclude`) dropped

**Severity:** Low · **Affects:** AMS parameter validation

Parsed but never carried into `IrParam` or checked at instantiation —
invalid parameter values pass silently.

### G.4 AMS formatter has zero test coverage

**Severity:** Low · **Affects:** `piperine-ams/src/fmt.rs`

No idempotency check, no golden snapshots; `tests/fixtures_fmt/` is
actually a parse corpus, not a formatter corpus.

---

## Part H — Solver: integration, timestep, convergence {#part-h}

### H.1 Trapezoidal integration not wired up (backward Euler only)

**Severity:** Medium · **Affects:** transient accuracy for all reactive
devices

`IntegrationMethod` enum exists with correct Trapezoidal/Gear
coefficients but is never consulted — only backward Euler runs. Needs
the method threaded into `TransientAnalysisOptions`, and devices'
`load_transient` extended to apply the trapezoidal history term (`beta`),
not just `alpha`.

### H.2 LTE-based timestep control — infrastructure dead, `$bound_step` has zero effect

**Severity:** Medium · **Affects:** transient step-size selection and
every `$bound_step(dt)` call site

`TruncationError`/`BreakpointProvider` are fully implemented and
unit-tested but have zero call sites; `accept_timestep` is never called
so `charge_history` stays empty. **Confirmed this session:**
`bound_step_hint()`'s default trait impl returns `f64::INFINITY`
unconditionally and `PhdlDevice` never overrides it — so `$bound_step(dt)`
lowers correctly to `IrStmt::BoundStep` (frontend/IR side works) but has
**no runtime effect whatsoever**; the solver simply never reads the
hint. Needs `accept_timestep` called post-step, LTE computed from
`charge_history`, and `bound_step_hint` actually consulted for step-size
capping.

### H.3 No gmin/source stepping for DC convergence

**Severity:** Medium · **Affects:** any circuit with a floating node or
hard-nonlinear DC

`Context.gmin` is only forwarded to OSDI plugins — the in-tree solver
never stamps gmin conductances to ground itself, and there's no
gmin-stepping or source-stepping homotopy. Floating nodes fail to
converge with no diagnostic.

### H.4 Voltage-source branch-current rows missing from MNA

**Severity:** Critical · **Affects:** prerequisite for D.1 — every ideal
voltage source

The MNA matrix is sized per-node only — no branch-current unknown exists
for voltage sources. This is the solver-side blocker underneath D.1;
OSDI tests deliberately avoid voltage sources because of it. Needs
`Netlist` to allocate branch-current indices and the matrix dimension to
grow accordingly.



### H.6 BJT excess phase (PTF) has no state-recurrence operator

**Severity:** Low · **Affects:** `bjt` at RF frequencies (`PTF > 0`,
`f·TF > 0.1`)

Model uses `cex = cbe` (no phase correction); NGSPICE's Weil-approximation
phase filter isn't implemented. Default `PTF=0` gives exact parity, so
this only matters for RF designers. Proposed approach: two chained `idt`
integrators forming the 2nd-order Weil section, once D.2/D.3 land.

### H.7 Diode breakdown voltage uses first-order approximation

**Severity:** Low · **Affects:** `dio` near breakdown (avalanche
photodiodes, Zener regulators, ESD clamps)

NGSPICE iterates up to 25 times to match forward/reverse regions at
breakdown; the faithful header uses only the first-guess formula (~1%
error — acceptable for most circuits, not bit-identical). No language
mechanism exists for bounded numerical iteration inside an analog block
(would need const-bounded recursion in `const_eval`, tied to I.8).
Documented as an accepted accuracy gap, not planned near-term.

---

## Part I — PHDL language features {#part-i}

> I.1 (`BundleDecl`), I.9 (mod-body `var` for value types) and I.11–I.15 (`&&`/`||`,
> `else if` expressions, `sinh`/`cosh`/`tanh`, bundle-typed `param`
> flattening, mod-body persistent `var`) are all **done** (2026-07-01).

### I.2 Capability dispatch: operators aren't sugar for capability methods

**Severity:** High · **Affects:** any user type implementing
`Add`/`Sub`/etc. (e.g. the planned `UInt[N]`, see C.2)

`a + b` stays a raw `Binary` expr even when the operand type has
`impl Add`; user-defined operator overloading is inert. Needs a
capability-resolution pass rewriting operator exprs into method calls
when the operand type isn't a primitive. Depends on B.1 (done), I.3
(`Self`), I.6 (`BundleLit` for method bodies).

### I.3 `Self` dropped from `impl` method bodies

**Severity:** High · **Affects:** every `impl` method — the receiver is
silently discarded

`FnParam::SelfParam => None` — methods lose their receiver entirely; no
substitution of `Self` for the implementing type anywhere. Foundational
for I.2/C.2.

### I.4 Generic modules/bundles with `<T: Cap>` don't monomorphize

**Severity:** High · **Affects:** `Adder<T>`, `Pair<T>`, any
type-parameterized (not just const-parameterized) construct

Const-param generics (`[N]`) work and monomorphize on demand; type-param
generics (`<T: Cap>`) are parsed then discarded, and generic modules are
skipped entirely at elaboration. Needs the same monomorphization pattern
extended to type substitution, plus bound validation against the impl
registry (I.5).

### I.5 No capability-conformance check for `impl ... for`

**Severity:** Medium · **Affects:** any `impl` block

No check that an `impl` provides every method its capability requires
(including transitively through super-capabilities), or that
default-body methods are correctly inherited when omitted — an
incomplete impl silently passes today.

### I.6 `BundleLit` isn't const-evaluable or constructible at codegen

**Severity:** High · **Affects:** every bundle-literal construction,
especially inside `impl` method bodies

`const_eval` has no `BundleLit` arm, codegen has no construction path,
and bundle field defaults are never applied for omitted fields. *Note:*
this session's bundle-typed-**param** flattening (I.14) is a narrower,
already-solved slice of this — general `BundleLit` *value* construction
(independent of the `param` context) is still open.

### I.7 Enum discriminants not evaluated; `match` not checked for exhaustiveness

**Severity:** Medium · **Affects:** any enum-typed `match`

Variant discriminant expressions are captured but never const-evaluated
(no auto-increment, no explicit-value comparison), and nothing checks
that a `match` over an enum covers every variant or has a wildcard.

### I.8 Higher-order functions: lambdas, `map`/`reduce` execution, bounded recursion

**Severity:** Medium (advanced feature) · **Affects:** any spec example
using `reduce(parts, |a,b| a+b)` or recursive `fn`

Lambdas parse but aren't reduced at elaboration and are rejected at
codegen; `map`/`reduce` exist in the prelude with stubbed bodies;
`const_eval` has no `Expr::Call` arm at all, so no recursion (bounded or
otherwise) works. Needs lambda capture-analysis (const-only captures),
`map`/`reduce` actually evaluated at elaboration time, and a depth-capped
(~256) recursive-call arm in `const_eval`.

### I.10 `pub` visibility never enforced

**Severity:** Low · **Affects:** module resolution (`use`)

`is_pub` is captured on every declaration but never checked — private
items are freely importable cross-package. The `pub` keyword is
currently decorative.

---

## Part J — Diagnostics, events, `$assert` {#part-j}

### J.1 `$bound_step`/`$finish`/`$discontinuity` — AST shape only, cosmetic

**Severity:** Low · **Affects:** AST cleanliness, not runtime behavior

The AST still lumps every `$name(...)` statement into one
`BehaviorStmt::Diagnostic{sys,args}` variant, but
`lowering/stmt.rs:122-212` already special-cases `bound_step`/
`finish`/`stop`/`discontinuity` into their correct distinct `IrStmt`
variants before falling through to `Diagnostic` — the semantic-loss this
item originally worried about no longer happens. What's left is purely
giving these their own AST variants for cleanliness (matching the
`AnalogOp`/`SystemFunction` registry pattern), not a correctness issue.
Low priority; arguably not worth doing.

### J.2 `$finish`/`$stop` and `$fatal` have no runtime effect

**Severity:** Medium · **Affects:** any model using simulation-control
tasks

**New finding, this session.** `IrStmt::Finish` lowers correctly from
`$finish`/`$stop`, but nothing in `piperine-solver` checks for it — no
abort flag, no early-return anywhere. Likewise `$fatal` tags
`Severity::Fatal` on a `Diagnostic` node but gets exactly the same
runtime handling as `$warning`/`$error` (a log message) — it does not
stop the simulation. Both need a solver-side abort mechanism (e.g. an
`AtomicBool` or a `Result`-returning step function) checked after each
Newton iteration / event.

### J.3 `$assert(cond, msg)` isn't a real assertion

**Severity:** Medium · **Affects:** any `@ initial { $assert(...); }`
setup-validation pattern

Just another generic `Diagnostic` — no distinct AST handling, no
elaboration-time const-cond evaluation for the `@ initial`
"validates setup" semantic the spec describes.

### J.4 `$error`/`$warn`/`$info` names unvalidated, untested; casts not recognized

**Severity:** Low/Medium · **Affects:** diagnostic severity mapping;
typecheck (prerequisite for B.5)

Any `$name(...)` is accepted without checking it's a recognized
diagnostic name; no test exercises the severity mapping. Separately,
`real(x)`/`int(x)`/`bit(x)` parse as ordinary function calls
(`Expr::Call(Ident("real"), [x])`) rather than a distinct `Expr::Cast` —
no coercion validation, no cast-specific codegen lowering.

---

## Part K — Architecture cleanup {#part-k}

### K.1 Deprecate the `from_elab` analog path

**Severity:** Medium (downgraded — mostly done) · **Affects:**
`piperine-codegen` maintenance surface

The dedicated `from_elab` module is already removed
(`piperine-codegen/src/lib.rs:9,23` — commented out, "moved to
piperine-lang"), and `compile_analog_module` now routes through
`ppr_to_ir` → `ir_analog_to_device`, the same IR path used everywhere
else. What remains: the shared Cranelift skeleton
(`codegen/analog.rs`) is still generic over an `AnalogExpr` trait meant
to serve both PHDL `Expr` and `IrExpr` — worth confirming no production
code path still instantiates it over `Expr` before closing this out
entirely.

### K.2 `IrFunction` table is dead data

**Severity:** High · **Affects:** IR/codegen contract honesty

Populated by both frontends, read by no codegen path (only the
pretty-printer). Resolved by D.5 (inlining); if D.5 doesn't land, the
field should be removed rather than left as dead data contradicting the
documented contract.

### K.4 IR net references are flat strings, not structured

**Severity:** Medium · **Affects:** `IrConnection`, `BranchAccess`,
`Contrib`/`Force` — prerequisite for F.3

Hierarchical refs (`name.port`, `name[i].port`) are strings re-parsed
downstream by every consumer. A structured `IrNetRef` enum
(`Simple`/`Indexed`/`Field`/`IndexedField`) would make hierarchy
first-class.

### K.5 `Port` enum doc references a nonexistent file

**Severity:** Low · **Affects:** `piperine-solver/src/port.rs` docblock

Cites "Section 3 of SOLVER_COSIMULATION.md" (doesn't exist); also claims
a cross-layer role the solver doesn't actually use (`AnalogReference`/
`DigitalNet` are the real solver abstractions). Just needs the docblock
corrected.

### K.6 `docs/` still has overlapping markdown files

**Severity:** Medium (onboarding cost) · **Affects:** every new
contributor

This merge (folding `GAPS.md` + `NGSPICE_GAPS.md` + `AMS-BUILTIN-TASKS.md`
into one file) addresses the gap-tracking sprawl specifically. The
broader spec/architecture doc set (`piperine-hdl-spec.md`,
`CODEGEN-IR.md`, `IR-JIT-SPEC.md`, `SHARED-IR-DESIGN.md`,
`piperine-hdl-elaboration-phase.md`, etc.) still has no single marked
source of truth and likely contains stale/superseded designs — still
open.

### K.7 Stale test-count references between `AGENTS.md` and `tests-baseline.md`

**Severity:** Low · **Affects:** onboarding accuracy

Numbers disagree; pick one source of truth and point to it.

---

## Part L — Documentation {#part-l}

### L.1 README describes a nonexistent Python+ngspice+PyO3 architecture

**Severity:** Critical (onboarding) · **Affects:** every new
contributor's first impression

Lists crates that don't exist, references a Python CLI that isn't real,
points at a nonexistent `ARCHITECTURE.md`. Highest cost/benefit
documentation fix in the project — rewrite to describe the actual
Rust/IR/JIT architecture with real crate names and real CLI commands.

### L.2 Most crates' `lib.rs` have no module-level docblock

**Severity:** Low · **Affects:** `piperine-codegen`, `piperine-solver`,
`piperine-ams` (only `piperine-lang` has one)

No pipeline diagram, no quick-start, no module table for `cargo doc`/IDE
hover to surface.

### L.3 `piperine-solver` has no re-export surface

**Severity:** Low · **Affects:** ergonomics for any external consumer

Only `pub mod` declarations — everything needs fully-qualified paths
(`piperine_solver::solver::dc::DcSolver`).

### L.4 No negative-assertion tests for the historical silent-bug fixes

**Severity:** Medium · **Affects:** regression protection

The fixed Part-A items (flow reads, temperature/vt/abstime, digital
shift/pow/reduce ops, `from_ir` error propagation, etc.) mostly lack a
test asserting the *old* wrong behavior stays gone (mirroring the
existing `power_law_contribution_uses_pow_not_add` pattern in
`piperine-codegen`'s test suite). Without them, a silent-fallback
regression could slip back in unnoticed.

### L.5 Test-device fixtures duplicated across test files

**Severity:** Low · **Affects:** `piperine-solver` test suite
maintenance

`Inverter`/`DFF` reimplemented in three separate places instead of a
shared `tests/helpers/devices.rs`.

---

## Part M — Model-specific numeric fidelity {#part-m}

### M.1 MOS Meyer capacitances: capacitance vs. charge formulation

**Severity:** Low · **Affects:** `mos1`

NGSPICE's `DEVqmeyer` (`devsup.c:624-689`) incrementally updates charge
(`Q_new = Q_old + C·ΔV`); the `mos1` header instead uses the capacitance
formulation `I(g,sp) <+ cgs_total * ddt(V(g,sp))` — correct for
small-signal (AC) response but not bit-identical to NGSPICE's transient
charge update. The Meyer model's charges have no closed form in the
linear region (the integral involves `ln` terms with singularities at
region boundaries), so a faithful `ddt(Q)` port needs those derived
per-region first. Accepted accuracy gap for switched-capacitor-sensitive
circuits; negligible for most others.

---

## Summary table {#summary-table}

| ID | Gap | Severity | Affects |
|----|-----|----------|---------|

| B.5 | No `Boolean`→`Quad` widening / cast enforcement | Medium | typecheck |
| C.2 | `UInt[N]`/`SInt[N]` not in prelude | High | generic bit-vector examples |
| D.1 | `V(p,n) <- expr` (ideal voltage source) rejected | Critical | vsrc, ccvs, mut, vcvs |
| D.2 | `idt`/`idtmod` not device-compiled faithfully | High | integrators, phase accumulators |
| D.3 | `ddx`/`delay`/`transition`/`slew`/`laplace_*`/`zi_*` rejected | Medium | filter/waveform models |
| D.5 | User `fn` inlining missing (analog reject / digital 0) | High | any model with helper fns |
| D.6 | Digital `if`/`match`/loops unreachable from IR path | High | digital state machines |
| D.8 | `$limit`/`pnjlim`/`fetlim` rejected; `limexp` stateless | High | dio, bjt, jfet, mos1 |
| D.9 | `ac_stim` rejected — AC analysis has no stimulus | Medium | vsrc, isrc |
| D.10 | `$analysis` rejected — no DC/tran branching | Medium | vsrc, isrc |
| E.1 | Solver never passes real analog voltages to `eval_discrete` | Critical | all mixed-signal examples |
| E.2 | D2A bridge: analog never reads digital state | Critical | bit-to-voltage DACs |
| E.3 | `cross`/`above` don't drive digital state | High | comparator/level-detector |
| E.4 | `@above`/`@cross` don't fire to update persistent `var` | Medium | sw, csw |
| E.5 | `@initial` never fires for analog IC/off | Medium | cap, ind, dio, sw, csw |
| F.1 | `from_ir` doesn't recurse into child instances | High | multi-level hierarchy |
| F.2 | `from_ir` drops the top module's own body | High | container-with-parasitics pattern |
| F.3 | Hierarchical `name.port`/`name[i].port` don't reduce | High | parent→child/array port refs |
| F.4 | AMS `generate` not unrolled | Medium | AMS generate blocks |
| G.1 | AMS `initial`/`always` never reach `IrDigitalBody` | High | pure-digital AMS fixtures |
| G.2 | AMS `param_ports` discarded | Medium | header-style AMS params |
| G.3 | AMS parameter `from`/`exclude` constraints dropped | Low | AMS param validation |
| G.4 | AMS formatter untested | Low | piperine-ams/src/fmt.rs |
| H.1 | Trapezoidal integration not wired (Euler only) | Medium | transient accuracy |
| H.2 | LTE timestep control dead; `$bound_step` has zero effect | Medium | transient step sizing |
| H.3 | No gmin/source stepping | Medium | hard-nonlinear DC |
| H.4 | No MNA branch-current rows | Critical | prerequisite for D.1 |
| H.6 | BJT excess phase (PTF) unimplemented | Low | bjt at RF |
| H.7 | Diode breakdown: first-order approx, not 25-iter fixed point | Low | dio near breakdown |
| I.1 | `BundleDecl` not exposed on `Design` | High | prereq for B.3, I.3, I.6 |
| I.2 | Capability dispatch not sugar for operators | High | user operator overloading |
| I.3 | `Self` dropped from `impl` bodies | High | every `impl` method |
| I.4 | Type-param generics `<T: Cap>` don't monomorphize | High | Adder\<T\>, Pair\<T\> |
| I.5 | No capability-conformance check | Medium | `impl` completeness |
| I.6 | `BundleLit` not const-evaluable/constructible | High | bundle-literal construction |
| I.7 | Enum discriminants/match exhaustiveness unchecked | Medium | enum `match` |
| I.8 | Lambdas/map/reduce/recursion not executable | Medium | higher-order fn examples |
| I.10 | `pub` visibility unenforced | Low | module resolution |
| J.1 | `$bound_step` etc. share one AST variant (cosmetic) | Low | AST cleanliness only |
| J.2 | `$finish`/`$stop`/`$fatal` have no runtime effect | Medium | simulation-control tasks |
| J.3 | `$assert` not a real assertion | Medium | `@initial` setup validation |
| K.1 | `from_elab` path — mostly removed, confirm fully closed | Medium | codegen maintenance |
| K.2 | `IrFunction` table is dead data | High | IR/codegen contract honesty |
| K.4 | IR net refs are flat strings | Medium | prereq for F.3 |
| K.5 | `Port` doc references nonexistent file | Low | solver docblock |
| K.6 | Doc sprawl (spec/architecture docs) | Medium | onboarding |
| K.7 | Stale test-count refs (AGENTS.md vs tests-baseline.md) | Low | onboarding accuracy |
| L.1 | README describes nonexistent architecture | Critical | onboarding |
| L.2 | Most crates lack module-level docblocks | Low | cargo doc / IDE hover |
| L.3 | `piperine-solver` has no re-export surface | Low | external ergonomics |
| L.4 | No negative-assertion tests for old silent bugs | Medium | regression protection |
| L.5 | Test-device fixtures duplicated | Low | solver test maintenance |
| M.1 | MOS Meyer caps: C-formulation vs charge-formulation | Low | mos1 |
