# LRM Conformance Closure Design

**Spec**: `.specs/features/lrm-conformance-closure/spec.md`
**Evidence base**: `docs/audit/{README,ch1-4,ch5-6,ch7-9,ch10-12-annexes}.md`
**Status**: Approved (user, 2026-08-14) → Tasks

---

## Architecture Overview

The audit's 55 findings are not 55 independent bugs. Read by *mechanism*
rather than by chapter, they collapse onto **three missing backbones** plus a
long tail of leaf fixes that each become small once a backbone exists:

| Backbone (absent today) | Findings it unblocks |
|---|---|
| A runtime **I/O path** from JIT'd/interpreted bodies back into the host | C7-07, C7-08, C7-09, C7-10, C10-14, and every §9.5/§9.9/§9.10/§9.12 task |
| A **four-state value layer** in the frontend value system | C1-01, C1-02, C1-04 (and correct `%b`/`%h` rendering for the above) |
| A **discipline-domain + directive-state** model | C7-01, C7-02..05, C10-01..10, C10-18, C10-19, C10-20/C7-04 |

So the design adds a **Phase 0 "spine"** that builds exactly those three, and
then runs the spec's own P1–P5 stories as leaf work on top. Nothing in P0 is
speculative surface: each of its three components has ≥3 spec ACs that cannot
be satisfied without it, and none of them are reachable by a single-chapter
task without being under-built (the failure mode the audit itself documented
three times over — `gates.rs`, `udp.rs`, `regions.rs` are all "correct
semantics, no caller", built inside a task whose `Where` excluded the wiring).

```mermaid
graph TD
    subgraph P0["P0 — spine (new mechanism, no LRMC closed by itself)"]
        IO["RuntimeIo<br/>SimOutput sink + JIT trampolines + FileTable"]
        FS["LogicVec / ValueType::Logic<br/>four-state value layer"]
        DD["DisciplineDomain + DirectiveState<br/>Annex F + ch.10 state"]
    end

    subgraph P1["P1 — ch.1-4 lexer/grammar/expr"]
        LEX[lexer.rs: based digits, ws, underscores, '{ ]
        EXPR["grammar/expr.rs: access fns, I(&lt;port&gt;), attr instances"]
        OPS[analog_ops.rs: laplace_* registry]
    end

    subgraph P2["P2 — ch.5-6 analog + hierarchy"]
        EV[named events, absdelta, cross dir, Annex C narrowing]
        HIER[defparam, paramset, array-of-instances]
    end

    subgraph P3["P3 — ch.7-9 mixed signal / scheduling / tasks"]
        CONN[connectrules, connect_mode, resolveto, driver access]
        SCHED[RegionQueue wiring, gates/UDP wiring, absdelta A2D]
        TASKS[ch.9 catalogue — full native]
    end

    subgraph P4["P4 — ch.10 directives + Annex C/D/E/F"]
        DIR[preprocessor directive arms + predefined macros]
        ANNEX[driver_access.vams, Annex E deltas]
    end

    subgraph P5["P5 — VPI native equivalents"]
        VPI[event-grained callbacks, delay model, put_value, mcd_*]
    end

    FS --> LEX
    FS --> TASKS
    IO --> TASKS
    IO --> SCHED
    IO --> VPI
    DD --> CONN
    DD --> DIR
    P1 --> P2 --> P3
    DIR --> DD
    P3 --> P5
    P4 --> P5
    ALL["lrm_conformance_matrix.rs<br/>(registry+exhaustiveness gate, MD-31)"]
    P1 --> ALL
    P2 --> ALL
    P3 --> ALL
    P4 --> ALL
    P5 --> ALL
```

**Execution order ≠ story numbering.** P4's directive/Annex-F items
(C10-01, C10-18, C10-19) are *prerequisites* of P3's connect-module work
(C7-01), exactly as `spec.md`'s own Assumptions table and P4's "Why P4" note
flag. The design resolves that by hoisting those three into **P0's `DD`
component** and leaving the rest of P4 (the other directives, Annex D.3,
Annex E deltas) in narrative position. This confirms the spec's open
assumption on priority order with one amendment, recorded below.

---

## Confirmation of the spec's two open assumptions

| Spec assumption (`n`) | Design ruling |
|---|---|
| Priority order P1→P5 (dependency-ordered) | **Confirmed with one amendment**: `` `default_discipline `` (C10-01), the Annex F continuous-over-discrete rule (C10-18) and its default-discipline fallback (C10-19) move into P0's `DisciplineDomain` component, because C7-01 (P3) cannot be tested without them. Everything else keeps the spec's order. |
| Four-state needs a new `ValueType` variant | **Confirmed**, shape decided (user, this session): a vector `LogicVec { width, val, unk }` behind `Value::Logic` + `ValueType::Logic(width)`. `Value::Quad(u8)` (already present, `value.rs:44`) and `piperine_solver::abi::LogicValue` (`digital/events.rs:11`) stay as the 1-bit scalar forms; conversion between them is explicit, never implicit. |

---

## Code Reuse Analysis

### Existing components to leverage

| Component | Location | How to use |
|---|---|---|
| `LogicValue` (0/1/X/Z) | `crates/piperine-solver/src/digital/events.rs:11` | The 1-bit projection of `LogicVec`; `LogicVec::bit(i) -> LogicValue` is the bridge. Do **not** widen this enum. |
| Digital JIT 4-state encoding (`i64` 0/1/2/3) | `kernel/digital/compile.rs` (`//!` "Value encoding") | Already four-state at runtime — the frontend value layer is the only place four-state is missing. `LogicVec` lowers to this encoding per bit. |
| `jit_builder.symbol(name, ptr)` | `kernel/{analog,digital}/compile.rs:185-191,64-67,453-456` | The exact mechanism math intrinsics already use — `RuntimeIo`'s trampolines register the same way. No new JIT machinery. |
| `SimCtx` | `emit/abi.rs:10` | Gains one pointer field (`io: *mut SimOutputVt`) appended last, per the file's own "appended last to preserve JIT-ABI field offsets" convention. |
| `$bound_step` end-to-end path | `flatten/analog.rs:538-545` → `kernel/analog/compile.rs:769-776` → `device/analog/instance.rs:1501` → `Element::bound_step_hint` (`core/element.rs:190`) | The reference implementation for wiring an analog system task to a real numerical effect — `$discontinuity` (C7-11) copies this shape exactly. |
| `SystemTasks::TABLE` + `format_display` + `SimEnvironment` | `crates/piperine-lang/src/eval/tasks.rs:95-116,181-247` | The ch.9 semantics already exist as pure functions with unit tests. P3 extends the table and *calls* them from the trampoline — never a second copy (MD-36's spirit). |
| `VpiTaskRegistry` / `PluginHost::register_system_task` | `eval/tasks.rs:349-430`, `piperine-plugin/src/host.rs:431-620` | Native `vpi_register_systf` already works and is tested; P5's callback/value/delay work extends this registry rather than inventing a parallel one. |
| `RegionQueue` / `Region` | `piperine-solver/src/digital/regions.rs` | Correct and unit-tested in isolation. P3 wires it into `DigitalState::evaluate_until_stable`/`evaluate_dag_ordered` — no re-derivation of the ordering rule. |
| `GatePrimitive` / `UdpTable` | `kernel/digital/{gates.rs,udp.rs}` | Same: LRM-faithful truth tables already tested. P3 adds the POM lowering + `compile.rs` dispatch that gives them a caller. |
| `InsertConnectModules` | `elab/lower/connect.rs` (+ its bypass tests, `:319-368`) | The selection algorithm is proven correct; P0's `DD` removes the `ResolveDisciplines` veto so it becomes reachable, and those bypass tests convert into real end-to-end tests. |
| `headers/` Annex D ports | `crates/piperine-lang/headers/{disciplines,constants}.vams` | `driver_access.vams` (C10-16) is a third file in the same builtin `SourceMap` seeding pattern. |
| `annex_e.rs` typed support table | `crates/piperine-lang/src/annex_e.rs:70-261` | The pattern for P5's Annex E closure — flip rows from `NotSupported`/deviation to real, keeping the table as the gate. |
| `spec_failure_rules_guard.rs` / `capabilities_contract.rs` | `tests/` | The registry+exhaustiveness shape (MD-31) the new `lrm_conformance_matrix.rs` gate copies. |

### Integration points

| System | Integration method |
|---|---|
| `piperine-api` `Session`/`SessionBuilder` | New `SessionBuilder::output(impl SimOutput)`; default `StdoutSink`. `SimHooks` is untouched (hooks stay per-analysis; I/O is per-timestep and belongs below them). |
| `piperine-solver` `Context` | Carries `Rc<dyn SimOutput>`; analog and digital devices both read it from there, so one sink serves both engines. |
| `piperine-plugin` | `PluginHost` gains the event-grained callback registry (P5) and can supply its own `SimOutput` (`vpi_printf`/`vpi_mcd_*` route to the same sink). |
| `piperine-python` | Inherits the sink through `piperine-api`; a Python host can capture output as a string buffer (MD-22 uniform-host rule). |

---

## Components

### P0.1 `RuntimeIo` — the simulation output path

- **Purpose**: give every executing body (analog JIT, digital JIT, elaboration-time tasks) one way to emit text, open/close files, and report diagnostics to the host.
- **Location**: `crates/piperine-solver/src/core/output.rs` (trait + sinks + `FileTable`), trampolines in `crates/piperine-codegen/src/emit/runtime_io.rs`, host wiring in `crates/piperine-api/src/session/`.
- **Interfaces**:
  - `trait SimOutput { fn write(&self, ch: Channel, text: &str); fn flush(&self); }` — `Channel::{Stdout, Stderr, File(FileId)}`.
  - `struct StdoutSink` (default), `struct BufferSink` (tests, deterministic capture), `struct FileTable` (`$fopen`/`$fclose`/`$fdisplay` descriptors, §9.5).
  - `extern "C" fn pip_task_display(ctx: *mut SimCtx, fmt_id: u32, argc: u32, argv: *const TaskArg)` and siblings (`pip_task_finish`, `pip_task_fatal`, `pip_task_strobe`, `pip_task_monitor_arm`).
  - `struct FormatPool` — interned format strings + arg descriptors resolved at compile time, so the trampoline never allocates a format string per call (MD-18: no re-JIT, and no per-timestep string parsing either).
- **Dependencies**: `SimCtx` gains `io` pointer; `AnalogKernel::diagnostics()` (`kernel/analog/kernel.rs:693`, today caller-less) becomes the compile-time half feeding `FormatPool`.
- **Reuses**: `eval/tasks.rs::format_display` for rendering; `jit_builder.symbol` for registration; `$bound_step`'s plumbing shape.
- **Closes (with P3 leaf tasks)**: C7-07, C7-08, C7-09 (display/control half), C7-10, C10-14.

### P0.2 `LogicVec` — the four-state value layer

- **Purpose**: represent `x`/`z`/`?` bits as first-class values through lexing, const-eval, comparison, and formatting.
- **Location**: `crates/piperine-lang/src/value.rs` (`Value::Logic`), `pom/net_type.rs` (`ValueType::Logic(u32)`), `elab/const_eval.rs` (literal → value), a new `value/logic.rs` for the bit algebra.
- **Interfaces**:
  - `struct LogicVec { width: u32, val: Vec<u64>, unk: Vec<u64> }` — bit *i*: `unk=0` → `val` is 0/1; `unk=1` → `val` 0 = `X`, 1 = `Z`.
  - `LogicVec::{from_based_literal(base, digits, width), bit(i) -> LogicValue, has_unknown(), to_i64_exact() -> Option<i64>, case_eq(&self,&other) -> bool}` (`===`/`!==`, Annex C.5-aware).
  - `ValueType::Logic(width)` joins the existing `ValueType` enum; `TypeRef` unchanged.
- **Dependencies**: none new. Serde derives follow the existing `Value` contract.
- **Reuses**: `LogicValue` for per-bit projection; the digital JIT's existing 0/1/2/3 encoding as the lowering target.
- **Closes**: C1-02 directly; is the precondition for C1-01/C1-04 rejecting *correctly* (per-base digit legality is only meaningful once `x`/`z` are legal digits with a representation) and for `%b`/`%h`/`%v` rendering four-state under C7-08.
- **Fail-loud edge (spec Edge Cases)**: `to_i64_exact()` returning `None` at an array index / bit width / generate bound is an `ElabErrorKind` citing the LRM clause — never a truncation to two-state.

### P0.3 `DisciplineDomain` + `DirectiveState` — Annex F and ch.10 state

- **Purpose**: teach discipline resolution what a *domain* is, and give the preprocessor a place to keep directive state that elaboration can read.
- **Location**: `crates/piperine-lang/src/pom/nature.rs` (domain on the discipline record), `elab/lower/disciplines.rs` (resolution rules), `preprocessor.rs` (+ a new `directives.rs` for the state struct).
- **Interfaces**:
  - `enum DisciplineDomain { Continuous, Discrete }` — read from the discipline's `domain` attribute (Annex D.1's own disciplines already declare it).
  - `ResolveDisciplines::resolve_one_round` conflict arm (`disciplines.rs:118-129`) becomes a three-way decision: same discipline → ok; different domains → **continuous wins** (§7.4.4.1); same domain, different names → check `connectrules` eligibility, then `` `default_discipline ``, and only then the existing `E4003` hard failure.
  - `struct DirectiveState { default_discipline: Option<String>, default_transition: Option<(f64,f64)>, default_nettype: NetTypeDefault, keyword_scopes: Vec<KeywordScope>, celldefine_spans: Vec<Span>, unconnected_drive: Option<Drive>, line_map: LineMap }` — produced by the preprocessor, carried on `SourceFile`, consumed by elaboration/codegen.
- **Dependencies**: `InsertConnectModules`'s eligibility check must be callable *from* `ResolveDisciplines` — extract `connect.rs`'s rule-matching into a `fn eligible_bridge(&Elaborator, a, b) -> Option<RuleId>` used by both passes (no pass reordering, so `passes.rs`'s documented ordering rationale stays valid).
- **Reuses**: `connect.rs`'s selection algorithm and its hand-built bypass tests.
- **Closes**: C10-01 (parse half + state), C10-18, C10-19; unblocks C7-01, C7-04/C10-20.
- **Preserves**: the spec's edge case — two *same-domain* incompatible disciplines still hard-fail exactly as today.

### P1 components (ch.1-4)

| Component | Location | Notes |
|---|---|---|
| Per-base digit validation + literal whitespace + underscore rules | `lexer.rs:399-486` | `is_based_digit` becomes `is_based_digit(base, ch)`; `lex_based_digits` calls the existing-but-unused `skip_inline_whitespace` (`lexer.rs:171`). C1-01, C1-03, C1-04. |
| Assignment pattern `'{...}` | `lexer.rs` `'` dispatch + `ast::Expr::AssignmentPattern` + `grammar/expr.rs` + `const_eval` | A new AST node, not a desugaring — §3.4.6's string valid-value list is not an array literal. C1-06. |
| Expression-level attribute instances | `grammar/expr.rs` (`parse_unary`/`parse_expr_bp`/`parse_primary`) | Reuses `cursor.rs:109`'s `parse_attr_instances`; attributes attach to the expression node and are otherwise inert. C1-05. |
| Access-function name resolution | `grammar/expr.rs:192-195` + `elab/lower/vams.rs:761` | Grammar stops hardcoding `V`/`I`: any identifier that the *elaborator* knows is an access name becomes an `AccessFunction`. Because the grammar has no symbol table, the parse produces `Expr::Call` and `resolve_contribution_branches` promotes it using `Design::disciplines`' `access` attributes + the always-legal `potential`/`flow` generics. C1-08, C1-09. |
| Port access `I(<p>)` | `grammar/expr.rs` `parse_access_function` + `ast::PortAccess` | C1-10. |
| Real→integer rounding | `elab/const_eval.rs:242,252` | Round-half-away-from-zero per §4.2.1.1, replacing the `fract() == 0.0` guard. C1-11. |
| `laplace_*` registry | `resolve/pom/analog_ops.rs:394-396` | `["zd","zp","nd","np"]`, matching `zi_*` two lines below. Plus a guard test asserting registry keys ⊆ `keywords.rs`'s reserved set (this bug class, mechanically). C1-12 + C5-01. |
| `aliasparam` typing | `elab/lower/vams.rs:95-104` | Type from the aliased param. C1-07. |

### P2 components (ch.5-6)

| Component | Location | Notes |
|---|---|---|
| `cross` direction | `flatten/analog.rs:76-82` | Read `args[1]` → existing `CrossDir`. C5-02. |
| `absdelta` | `flatten/analog.rs` + `kernel/analog` + `piperine-signal/src/events.rs` | New `EventSource::AbsDelta`; the *semantics* (threshold on |Δexpr| with `min_var`/`max_rate`) go in `piperine-signal` beside `cross`/`above`/`timer` (MD-36), codegen calls it. C5-03 (and reused by C7-14). |
| Named events | `grammar/stmt.rs` (`->`), `elab/lower/vams.rs:142`, POM `Module::events` | Declaration lowers to a POM record (MD-25: authored structure preserved); trigger/sensitivity is an event-source kind on both engines. C5-04. |
| Annex C narrowing | `elab/typecheck.rs:67-68` | Permit `posedge`/`negedge`/named-event sensitivity inside `analog` per §5.10.5, keep `<=`/`casex`/`casez` rejected. Removes `flatten/analog.rs:56-60`'s "defensive empty list" — it becomes a live path. C5-05. |
| `defparam` | new `elab/lower/defparam.rs` pass, after `ElabModules`, before `ResolveDisciplines` | Applies hierarchical overrides onto `Instance` params. **MD-25 constraint**: writes parameter *values*, never restructures `Design::modules`. C5-06. |
| `paramset` | `elab/lower/register.rs:40` + instantiation lookup in `vams.rs` | An instance naming a paramset resolves to `(module, bound params)`; monomorphized name follows the `urc__5` precedent, tree stays walkable. C5-07. |
| Array of instances | `grammar/item.rs:842-855` + `ast::Instance.range` + `UnrollGenerate`-adjacent expansion | Range parsed into the AST; expansion produces N instances with per-element port slicing. Authored form stays in the POM as a single ranged `Instance` with the expansion in the derived flat form (MD-25). C5-08. |

### P3 components (ch.7-9)

| Component | Location | Notes |
|---|---|---|
| `connectrules` body / `connect_mode` / parameter-passing attr / `resolveto` | `grammar/item.rs:1086-1121`, `elab/lower/connect.rs` | Grammar productions per Syntax 7-6 + `merged`/`split` segmentation in insertion. C7-02, C7-03, C7-04/C10-20. |
| Driver access (`$driver_count`/`$driver_state`/…/`driver_update`) | new `piperine-solver/src/digital/drivers.rs` (`DriverTable`) + `eval/tasks.rs` table entries + trampolines | Needs a real per-net driver list in the digital state — new capability, sized as its own task cluster. C7-05, and the consumer that makes `driver_access.vams` (C10-16) meaningful. |
| §7.3.6 concurrency rules | `elab/typecheck.rs` | Five named rules, each an explicit check with an LRM-cited error, each with its own fixture — replaces "falls out incidentally". C7-06. |
| ch.9 catalogue — full native | `eval/tasks.rs` (table + semantics), `RuntimeIo` (effects), `emit/stmt.rs:100-109` + `flatten/analog.rs:536-567` (call sites) | Split by subsection: §9.5 file I/O → `FileTable`; §9.9/§9.13 stochastic → the LRM's own specified RNG + `$dist_*`; §9.10 time; §9.12 plusargs (host argv on `Session`); §9.16 `$simprobe`; §9.18/§9.19/§9.20 hierarchical/binding/alias → POM+`Net` introspection; §9.21 `$table_model` → `piperine-signal`'s existing `table` interpolation. C7-09 (user ruling: implement, don't fail-loud). |
| `$discontinuity` | `flatten/analog.rs:549` + `AnalogKernel` + `Element` step control | Mirrors `$bound_step`'s existing path exactly; `degree` argument forces step/order control at the announced point. C7-11. |
| Stratified scheduler wiring | `piperine-solver/src/digital/{scheduler.rs,events.rs,state.rs}` | `DigitalEvent` gains a `region: Region` field; `evaluate_until_stable`/`evaluate_dag_ordered` drain through `RegionQueue`. `Monitor` region is what makes `$strobe`/`$monitor` correct, so this task lands after `RuntimeIo`. C7-12. |
| Gates/UDP wiring | `elab/lower/vams.rs` (lower `GateInstantiation`/`PrimitiveDecl` into the POM) + `kernel/digital/compile.rs` (dispatch) + `device/digital.rs` | The three-hop wiring `gates.rs`/`udp.rs` were written to await. C7-13. |
| `absdelta`-interpolated A2D | `piperine-solver/src/digital/interface.rs` + `piperine-signal::events::predict_crossing` | Reuses tier-2 crossing prediction already in `piperine-signal`. C7-14. |

### P4 components (ch.10 + annexes)

| Component | Location | Notes |
|---|---|---|
| Directive arms | `preprocessor.rs:191-271` + new `directives.rs` | One match arm per directive, each writing `DirectiveState` (P0.3) rather than a `Tok::Directive` nobody consumes. Deletes the false doc comment (`preprocessor.rs:11-14`, `:259-262`) — C10-10 closes by making the claim true for `` `timescale ``/`` `resetall `` (recorded state, semantically inert). C10-01..C10-10. |
| Predefined macros | `Preprocessor::new()` (`preprocessor.rs:76`) | `__VAMS_ENABLE__`, `__VAMS_COMPACT_MODELING__` pre-registered. C10-03. |
| `` `begin_keywords `` scoping | `keywords.rs` + lexer | A keyword-set stack; the LRM's §10.6 example (`sin` as a port name) is the fixture. C10-04. |
| `driver_access.vams` | `crates/piperine-lang/headers/` + builtin `SourceMap` seed | Verbatim Annex D.3 port, same treatment as D.1/D.2. C10-16. |
| Annex E deltas | `annex_e.rs` | `vdslim`↔`limvds` naming + the rows the audit lists as degraded, excluding the netlist-reader-dependent E.2.1/E.3.2 (Out of Scope). Part of P5's AC5. |

### P5 components (VPI native equivalents)

| Component | Location | Notes |
|---|---|---|
| Event-grained callback registry | `piperine-plugin/src/host.rs` + `registry.rs` | `CallbackId`-returning registration on concrete simulation events (value change, timestep begin/end, region drain), with real deregistration. Replaces `HookPhase`-granularity as the *VPI* mechanism; `HookPhase` stays for lifecycle hooks. C10-11. |
| Delay/timing model | `piperine-solver/src/digital/delays.rs` | Per-net/per-path delay records the scheduler already needs for gate primitives (`#` delays), read/written by the `get_delays`/`put_delays` equivalents — genuinely new capability, sized accordingly. C10-12. |
| `put_value` write path | `piperine-plugin/src/vpi.rs` + solver `SetQueue` | Symmetric with the existing `SolveResultView` read path; reuses the live-params `SetQueue` mechanism rather than a second injection route. C10-13. |
| `vpi_mcd_*` | `RuntimeIo::FileTable` | The multi-channel descriptor family *is* `FileTable` with a channel mask — one mechanism, two spellings. C10-14. |
| `vpi.rs` table | `piperine-plugin/src/vpi.rs:77-186` | Every in-scope row flips to `Native`; the table remains the gate (a row still typed `Partial`/`NotSupported` for anything P5 covers fails the suite). |

### Cross-cutting: `lrm_conformance_matrix.rs` (the gate)

- **Purpose**: make "51 findings closed" mechanically checkable rather than a claim in a doc (MD-31).
- **Location**: `tests/lrm_conformance_matrix.rs` (root — cross-crate proof, MD-28).
- **Shape**: a table of all 51 LRMC-IDs × {LRM clause, story, closing test name}; the test asserts every ID has a named, existing, non-`#[ignore]`d test, and that no ID is unaccounted for. Same registry+exhaustiveness pattern as `capabilities_contract.rs`/`spec_failure_rules_guard.rs`. Must be proven able to fail (inject, watch, revert) before it counts.

---

## Data Models

```rust
// P0.2 — four-state (crates/piperine-lang/src/value/logic.rs)
pub struct LogicVec { width: u32, val: Vec<u64>, unk: Vec<u64> }
//   bit i: unk=0 → val bit is 0/1 ; unk=1 → val bit 0 = X, 1 = Z
pub enum Value  { /* … */ Logic(LogicVec) }
pub enum ValueType { /* … */ Logic(u32) }

// P0.1 — runtime I/O (crates/piperine-solver/src/core/output.rs)
pub enum Channel { Stdout, Stderr, File(FileId) }
pub trait SimOutput { fn write(&self, ch: Channel, text: &str); fn flush(&self); }
#[repr(C)] pub struct TaskArg { kind: u8, i: i64, f: f64, ptr: *const u8, len: u32 }

// P0.3 — Annex F + directives
pub enum DisciplineDomain { Continuous, Discrete }
pub struct DirectiveState { /* default_discipline, default_transition, default_nettype,
                               keyword_scopes, celldefine spans, unconnected_drive, line_map */ }

// P3 — scheduling + drivers (piperine-solver/src/digital/)
pub struct DigitalEvent { time: f64, net: DigitalNet, value: LogicValue,
                          source: usize, seq: u64, region: Region /* new */ }
pub struct DriverTable { /* per-net driver list: source, value, strength, delay, next_state */ }
```

**Relationships**: `LogicVec::bit(i)` → `LogicValue` (the solver's existing
1-bit type); `DriverTable` rows are what `$driver_*` and `vpi_get_delays`
both read; `DirectiveState.default_discipline` is consumed only by
`ResolveDisciplines` step 4.b.

---

## Error Handling Strategy

| Scenario | Handling | Author impact |
|---|---|---|
| Illegal digit for a base (`'o89`) | New `ParseErrorKind::IllegalBasedDigit { base, digit }` | Lexical error naming base and digit (C1-01) |
| Four-state value at a context with no four-state semantics (array index, bit width) | `ElabErrorKind` citing the LRM clause that makes it illegal | Loud, never truncated (spec Edge Case) |
| Two same-domain incompatible disciplines | Existing `E4003`, unchanged | Genuine incompatibility still fails (spec Edge Case) |
| Cross-domain mismatch with no eligible `connectrules` | `E4005` from `InsertConnectModules` (now reachable), not `E4003` | Error names the missing rule, not the mismatch |
| Unknown `$task` after the ch.9 build-out | Existing `E4007` + near-match suggestions (`tasks.rs:118-127`) | Unchanged mechanism, far smaller miss set |
| `$fatal` during a run | `RuntimeIo` flushes, then aborts the analysis with a typed solver error | Matches `$finish`/`$stop` semantics (§9.7) |
| `$fopen` failure / unwritable path | Typed error surfaced through the sink, analysis aborts | No silent zero descriptor |
| Any in-scope VPI routine still unimplemented at P5 close | `vpi.rs` table row typed `NotSupported` → `vpi_routines.rs` fails | The gate, not a doc, catches regression |

---

## Risks & Concerns

| Concern | Location | Impact | Mitigation |
|---|---|---|---|
| **MD-46 freeze vs. scheduler wiring** — MD-46 forbids touching `core/element.rs`'s trait surface; `regions.rs`'s own header says wiring regions was skipped partly for that reason | `.specs/STATE.md` MD-46; `digital/regions.rs:1-25` | C7-12 could stall mid-task | Design keeps the change inside `digital/` (region tag on `DigitalEvent`, not on `Element`). If a task proves an `Element` method is unavoidable, it **stops and records a superseding `MD-48`** — never a silent edit. |
| **`SimCtx` is a JIT ABI struct** — field offsets are baked into emitted code | `emit/abi.rs:10-37` | A mis-ordered field silently corrupts every device | New `io` pointer appended **last**, following the file's own two prior precedents; an ABI-offset assertion test lands with it. |
| **`emit/analog_expr.rs` is "do not edit casually"** and Annex E's `vdslim` fix needs it | `CLAUDE.md`; `annex_e.rs` | Regression risk in the shared residual/Jacobian skeleton | Annex E naming fix is its own isolated task with the ngspice oracles (`tests/ngspice_*.rs`) as its gate, not bundled with other work. |
| **`$display` in the analog JIT executes per Newton iteration**, not per accepted timestep | `flatten/analog.rs:536-567` | Naive wiring prints thousands of duplicate lines; a user reads it as a bug | `RuntimeIo` gates analog diagnostics on *accepted* steps (converged + committed), matching what SPICE-family tools do; the rule is stated in the sink's doc and has its own test. |
| **`defparam` + `paramset` + array-of-instances all touch elaboration structure** — the MD-25 tripwire | `elab/lower/`, `pom/design.rs` | An accidental `Design::modules` rewrite breaks the UNBREAKABLE rule | Each of the three tasks carries a POM-navigability assertion (authored hierarchy walkable, expansion only in `flat_modules`) in its own gate. |
| **`system_task_migration_completeness.rs` currently fails 12/12** | `crates/piperine-lang/tests/` | The "no regression" baseline is not "all green" | Baseline is captured target-by-target **before** work starts (the MD-47 A/B precedent); this target's 12 failures are a known-red entry that C7-10 flips to green. |
| **Four-state touches `Value`, which serializes** | `value.rs:26-33` | A new variant changes POM serialization | `LogicVec` is data (not a runtime handle) → round-trips like the other data variants; a serde round-trip test lands in the same task. |
| **Volume** — ~55-65 atomic tasks | this feature | Context exhaustion, drift between early and late tasks | Sub-agent batching at Execute (~8-9 batches of ~7); the `lrm_conformance_matrix.rs` gate + per-task LRM-derived fixtures keep late batches honest without re-reading early ones. |
| **`$random`/`$dist_*` must match the LRM's specified algorithm**, not any RNG | §9.9/§9.13 | Wrong numbers that look plausible | The LRM prints the reference C source for these; fixtures assert the LRM's own published sequences. |

---

## Tech Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Overall structure | Infra spine (P0) then chapter leaves | Three backbones carry ≥3 ACs each; the audit found three subsystems already built-without-callers because wiring fell outside a task's `Where` (user-confirmed) |
| Four-state representation | `LogicVec { width, val, unk }` + `ValueType::Logic(w)` | Width-carrying, per-bit X/Z; `Value::Quad`/`LogicValue` stay as the 1-bit forms (user-confirmed) |
| Output path | `SimOutput` trait in `piperine-solver` + JIT trampolines + stdout default | One mechanism for analog JIT, digital JIT, file I/O, and `vpi_printf`/`vpi_mcd_*`; testable via `BufferSink` (user-confirmed) |
| ch.9 scope | Implement all ~30 missing tasks natively | "TUDOOOOO precisa ser possível"; fail-loud E4007 stays only for names the LRM does not define (user-confirmed) |
| Annex F fix location | Inside `ResolveDisciplines`, no pass reordering | `passes.rs:37-54`'s ordering rationale stays valid; the eligibility check is extracted from `connect.rs` and shared |
| ch.9 semantics home | `piperine-lang::eval::tasks` (existing), *not* `piperine-signal` | MD-36's subject is device runtime operators (`transition`/`limit`/`table`); ch.9 task semantics already live in `tasks.rs` with unit tests — a third home would be the duplication MD-36 exists to prevent. `absdelta` **is** a device-runtime operator and does go to `piperine-signal`. |
| Access-function promotion | Elaborator promotes `Expr::Call` → `AccessFunction` using discipline `access` names | The grammar has no symbol table; hardcoding a name list in the parser is exactly the bug being fixed |
| Conformance tracking | `tests/lrm_conformance_matrix.rs` registry gate | MD-31: a policy invariant lives in the gate, not the document |

> **Project-level decisions to record at Tasks/Execute time**: (1) the `SimOutput` sink as *the* host I/O contract for every executing body (candidate `MD-48`); (2) if and only if scheduler wiring forces an `Element` trait change, a `MD-49` explicitly superseding MD-46's freeze. Neither is recorded now — they land with the task that proves them.

---

## Phase Plan (input to Tasks)

| Phase | Content | Est. tasks |
|---|---|---|
| P0 | `RuntimeIo`, `LogicVec`, `DisciplineDomain`+`DirectiveState` (incl. C10-01/18/19), baseline A/B capture, `lrm_conformance_matrix` skeleton | ~9 |
| P1 | ch.1-4: 11 LRMC-IDs | ~10 |
| P2 | ch.5-6: 8 LRMC-IDs | ~9 |
| P3 | ch.7-9: 14 LRMC-IDs, ch.9 catalogue split by subsection | ~18 |
| P4 | remaining directives, Annex D.3, Annex E deltas | ~8 |
| P5 | VPI native equivalents | ~7 |
| Close | follow-up audit pass (same four-range methodology), `docs/audit/*.md` rows updated with closing commits, `docs/spec/` corrections (`part_v_builtins.md`, `part_i_language.md` `laplace_*` claims) | ~3 |

Total ≈ 64 tasks → Execute offers ~9 sub-agent batches (batches are whole
consecutive phases packed to ~7 tasks; P3 splits into 3 batches by its own
sub-clusters, which are independent).
