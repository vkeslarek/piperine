# msl-digital: the contract with the merged ir-rust

**Task**: T1. **Read at**: `main` after `ir-rust` 9aa03fe8 and `solver-crossings` f46dd4e9, rebased into `feature/msl-digital`.
**Verdict**: no difference needs a redesign. Every lowering of design.md stands. Nine names or shapes differ from what the design took from the `ir-rust` design text; each is resolved below by a rename or by where a field lives, and the tasks it touches say so. Four items `ir-rust` left to this feature are folded in as tasks T64 and T65 and as extensions of T3 and T28.

Gate evidence: `cargo test -p piperine-ir --test digital` (8 passed) and `cargo test -p piperine-transpile --test dpwm` (3 passed) on this tree.

## Rows of design "What `ir-rust` gives and what this feature adds"

| Design row | Merged name or shape | Status |
| --- | --- | --- |
| `ValueType::{UInt, Int}` | `ValueType::UInt(u32)`, `ValueType::Int(u32)` (`model.rs`); width 1..=64 is a `check()` entry | confirmed; the width is `u32`, not `u8` (D1) |
| `Expr::Int` | `Expr::Int(i128)`: the value only, the type is the `ExprData` type | corrected (D2) |
| `Expr::Cast` | `Expr::Cast { expr, to: ValueType }` | confirmed |
| `Expr::Method` | `Expr::Method { method: Method, receiver, args }`, `Method::{Min, Max, Clamp, Floor}` | confirmed |
| `BinaryOp::*` | `BinaryOp::{Add, Sub, Mul, Div, Lt, Le, Gt, Ge, Eq, Ne, And, Or, Rem, BitAnd, BitOr, BitXor, Shl, Shr}`; `UnaryOp::{Neg, Not}` | confirmed; `And`/`Or` stay short-circuit |
| `ValueType::Flagged` | `ValueType::Flagged(Box<ValueType>)` | confirmed |
| `Expr::{Carrier, Unknown, KnownOr, IsKnown, High}` | `Expr::Carrier(u32)`, `Expr::Unknown`, `Expr::KnownOr { value, default }`, `Expr::IsKnown(ExprId)`, `Expr::High(ExprId)` | confirmed |
| `Carrier` | `Carrier { name, ty, role: CarrierRole::{In, Out, Signal}, init: Option<Constant>, origin }`, added by `IrBuilder::carrier(body, Carrier) -> u32` | confirmed; carriers are not in `BodySpec` (D3) |
| `Drive` | `Drive { carrier: u32, value, after: Option<ExprId>, origin }`, `IrBuilder::drive(body, Drive) -> u32` | confirmed (D3) |
| `NetData`, `InstanceData::nets` | `NetSpec { path, ty, declared_by, init: Option<Constant>, origin }`, `IrBuilder::net(NetSpec) -> NetId`, `IrBuilder::bind_nets(instance, Vec<NetId>)`; `InstanceData::nets` binds **every** carrier of the body, signals included, in the body's order | confirmed; signals are bound too (ir-rust T26), as design "Nets in the placement walk" assumed |
| `Event`, `Reaction`, `Stmt::Commit` | `Event { source: EventSource, guard: Option<ExprId>, origin }`, `EventSource::{Changed(u32), Cross(ExprId)}`, `Reaction { on: Vec<u32>, stmts, locals, origin }`, `Stmt::Commit { carrier, value, after }`; `IrBuilder::{event, reaction}` | confirmed; `Cross` holds no `ttol`/`etol` (spec "Out of Scope" already refuses them) |
| A1 `now()` | `Env` is `{Temperature, Gmin, SourceScale}`, ordered, the order of `Deps::env` | **new** `Env::Time` appended; printer word `time` |
| A2 `analysis(Static)` | as A1 | **new** `Env::Static` appended; printer word `static` |
| A3 `at(t)` | `EventSource` has `Changed`, `Cross` | **new** `EventSource::At(ExprId)`; its argument's reads need a dependency target (D6) |
| A4 `emit!` | `Stmt` has `Let`, `Expr`, `Contribute`, `Commit` | **new** `Stmt::Emit`, `BodyData::streams`, added by a builder method on a body id (D3) |
| A5 `i(a, b)`, `ddt` under a potential | `check()` has **no** rule refusing `Operator::Ddt` under `QuantityRole::Potential`; `Deps` has no branch field; the plan already has `Term::Current(b)` reading `Source::BranchFlow(b)`, the private flow pin, and `Term::Charge(b)` "negated on a potential branch" | **new** `Expr::Flow { branch }` only (D5, D7) |
| A6 `Child { r: r_top }` | `Child { name, body, nodes: Vec<Terminal>, params: Vec<(u32, Constant)>, origin }`, a literal struct built in four places, one in `piperine-interp/tests/support/circuits.rs` | **corrected**: the expression values live beside `Child`, not in its `params` (D4) |
| A7 `c.prev()` | no previous-value read; the plan's `rise` event function reads `[VALUE, CHANGED]` (ir-rust T32) | **new** `Expr::Previous(u32)`; its reads need an accessor beside `carrier_reads` (D6) |

## Rows of design "Dependencies on `ir-rust`"

| Design name | Merged name | Status |
| --- | --- | --- |
| `piperine-plan` | `piperine_plan::{BodyPlan, FunctionPlan, Mode, Pins, Source, Target, Term, ReactionPlan, ReactionMode, StatementPlan, CarrierLayout, DelayedPins, Netlist, Modules}` | confirmed |
| `piperine-transpile`, `Compiled::circuit` | `Transpiler::new(Arc<Ir>)`, `Transpiler::compile(&Settings) -> Result<Compiled, Error>`, `Compiled::{circuit, report, ir, source_path}`; `Report::warnings` | confirmed |
| `Backend` in `piperine-parity` | `piperine_parity::backend::Backend`; `Bench::{load_on, load_file_on, compiled, circuit}` | confirmed |
| Integer and unknown-value nodes (T24, T25) | rows above | confirmed |
| Carriers, drives, nets (T26) | rows above | confirmed |
| Events, reactions, commits (T27) | rows above; an `if` with no `else` is `Expr::If` with `otherwise` the unit literal, typed `()` | confirmed |
| `check()` for the digital forms (T28) | `Ir::check() -> Vec<Broken>`; typing rules in `piperine-ir/src/typing.rs` | confirmed |
| Buck-slice fixtures (T29) | `crates/piperine-ir/tests/support/digital.rs`: `Buck::{osc, ref_ramp, adc, comparator, dff, setpoint_reg, dpwm, pid, gate_driver, switch}`, functions `Buck::{rev3, rev3_flagged, dithered_width, dithered_width_flagged}`; goldens `tests/golden/buck/{osc, ref_ramp, adc, comparator, dff, setpoint_reg, dpwm, pid, gate_driver, switch}.txt` (structural, the contract) and `tests/golden/buck/full/*.txt`; pinned by `tests/digital.rs::the_buck_slice_bodies_pass_check_and_print_as_their_golden_texts`, each body alone under an empty `Top` | confirmed; the comparator fixture is `comparator.txt` (std `A2D`), `PowerSwitch` is `switch.txt` with the gate level in place of `transition` |
| Interpreter refusal (T30) | `Interpreter::new` rejects a body with a carrier, drive, event or reaction | confirmed |
| Digital plan, `rt` (T31 to T36) | `Mode::{Drive, Event, Cross, DelayedEvent}`, `Source::{Carrier, CarrierChanged, Delayed, DelayedChanged, Crossed}`, `Target::{Carrier, EventPin, Delayed, DelayedEventPin, Watch}`, pins `event:<k>`, `drive:<carrier>.now`, `drive:<carrier>.changed`, reaction `reaction:<k>`; `rt::{UInt, Int, Flagged, Commits}` (`rt/int.rs`, `rt/flag.rs`) | confirmed; ambient reads are named variants, not `Source::Ambient(u32)` (D8) |
| Digital printers, discrete nets (T37 to T41) | `print/{digital,reaction,cross}.rs`; `Netlist::circuit` binds `in`/`out` carriers as `Port::Net(<net path>)`, a `signal` as a private pin | confirmed; two wirings refused, owned here now (F-B1, F-B2) |
| Phase 8 tests (T42 to T46) | `crates/piperine-transpile/tests/{commits,dpwm,pid}.rs` with `tests/support/{mixed,alone}.rs` (`Wire`, `Built`, `Seen`); `crates/piperine-parity/tests/boundary.rs` | confirmed |
| The solver's record sink | `piperine_solver::protocol::Sink` (`protocol/sink.rs`) | confirmed |
| ABI parts and flags A3, A4, A7 need | `piperine_abi::raw`: `ambient::{TIME, STATIC}`, `part::PREVIOUS`, statement flags `LATER`, `REPLACE`, pin flag `RECORD` | confirmed |

## Names the generated code prints

`Text::ident` (`piperine-transpile/src/print/text.rs`) maps `~` to `_v` (`rev3~1` is `rev3_v1`), adds one `_` to a name that ends in `_` or is reserved, and raws a keyword. It does **not** map `.` or `$`. A hoisted carrier `gate.x` or `above$0.init$0.up` would print as no Rust identifier (D9).

## Differences and how each is resolved

- **D1 - widths are `u32`.** `ValueType::UInt(u32)`. The checker's `Ty::{UInt(u8), Int(u8)}` converts at lowering. Rename only.
- **D2 - `Expr::Int(i128)` has no `ty` field.** The literal's type is the `ty` given to `IrBuilder::expr`. The elaborator passes the type from `Types::literal`. Rename only.
- **D3 - carriers, drives, events, reactions are added by builder methods on a body id**, not through `BodySpec` (every M1 test builds `BodySpec` literally). The elaborator calls `IrBuilder::body` first, then `carrier`/`drive`/`event`/`reaction`. Every amendment that adds a per-body list (A4 `streams`) follows the same pattern: a `BodyData` field with `#[serde(default)]`, a builder method on a body id, a `Body` handle accessor. No literal-built struct gains a field.
- **D4 - A6 does not change `Child::params`.** `Child` is built literally in `piperine-ir` tests, `piperine-lang/src/elab/children.rs` and `piperine-interp/tests/support/circuits.rs`, and the interpreter's tests are frozen. A6 is stored beside the child, as ir-rust T26 stored carriers beside `BodySpec`: `Child::params` keeps the constant entries; a param value that reads the parent's params is `ChildValue::Expr(ExprId)` in a per-body list set by `IrBuilder::child_param(body, child, param, ChildValue)` and read by `Body::child_params(child) -> &[(u32, ChildValue)]`, printed only when present. T21 and T32 read "`Child::params`" in this sense. Where a lowering is concerned nothing changes: the authored child shows the expression, the flat instance table holds the constant.
- **D5 - there is no `check()` rule against `ddt` under a potential contribution to replace.** T20 adds the `Expr::Flow` rule (the branch has a potential contribution in the body) and asserts that a potential contribution with a `Ddt` addend passes `check()`; the "M1 rule replaced" line of T20 is void. The refusal lives in `piperine-lang` (`Unlowered::DdtOffTop`), which T35 relaxes as planned.
- **D6 - `Deps` keeps its M1 fields** (a test builds it literally). New reads that are not nodes, params, environment or sites are given as accessors beside `Body::carrier_reads`: the flow reads of A5 (`Body::flow_reads(target)`), the previous-value reads of A7 (`Body::previous_reads(target)`). The argument of an `At` event (A3) is a new dependency target, `Target::At(k)`, beside `Target::Cross(k)`. `Env::{Time, Static}` go into `Deps::env` as the other environment reads do.
- **D7 - A5 in the plan needs no new `Source`.** `Source::BranchFlow(b)` is the private flow pin of potential branch `b` and is already a differentiable read; `Term::Charge(b)` already carries the `ddt` addends of a potential branch. T45 maps `Body::flow_reads` to `Source::BranchFlow` and checks the charge term of a potential branch against abi.md A1.
- **D8 - ambient reads are named `Source` variants** (`Temperature`, `Gmin`, `SourceScale`, `Static`), not `Source::Ambient(u32)`. `Source::Static` already exists (the `limit` rules read it). A1 adds `Source::Time`; A2 reuses `Source::Static`. Design "Plan and printer amendments" reads `Source::Ambient(TIME)` as `Source::Time` and `Source::Ambient(STATIC)` as `Source::Static`.
- **D9 - `Text::ident` does not cover `.` and `$`.** The IR keeps the names DIG-10 AC 1 fixes (`gate.x`, `transition$0`). The transpiler maps them, as it maps `~`: `.` prints as `_d` and `$` as `_s` (`gate_dx`, `above_s0_dinit_s0_dup`), snake-case names no MSL name of the same body meets except by collision, which `LibraryPrinter` refuses as it refuses a variant name collision. The ABI pin keeps the IR name. This is printer work and lands with the first generated code that holds a hoisted name: T40 (its Done-when gains the case).

Two shape notes for phase 8, no redesign: `StatementPlan` is `{ carrier: u32, min_after }`; the arming statement of A3 writes an event pin and the record statement of A4 writes a `RECORD` pin, neither a carrier, so T41 and T43 give `StatementPlan` a target in place of `carrier` (it is built only in `piperine-plan` and `piperine-transpile` sources, never literally in a test). The T29 fixtures lower `if let Some(d) = x.known()` as `if x.is_known() { let d = x.known_or(0); … }`: the `Option` pair of design "From a source form to IR nodes" is substituted as its two expressions, never bound to two locals, or DIG-14 AC 1 fails on a choice.

## What `ir-rust` left to this feature

From `ir-rust` tasks.md `## Handoff`, "Left for msl-digital". None was covered by a task of this feature before T1.

| Item | Where it goes | Why there |
| --- | --- | --- |
| **F-B1** a parent's `signal` on its children's net (refused today, C106 of ir-rust) | **T65** (phase 8, new): the plan makes a `signal` carrier that a child's carrier port is bound to (`Body::child_carriers`, T64) a `PORT` pin, and `Netlist::circuit` binds it to its net by `Port::Net`; the refusal and its test are replaced by a run where the parent's commits reach the child | the solver binds a `PRIVATE` pin to no named net; a `PORT` pin on a named net needs no solver change |
| **F-B2** a net's own `init` (refused today, C107 of ir-rust) | **T28** writes a net's `init` as the folded initial value of the carrier that declares it; **T65** lifts the refusal: the init reaches the solver as the `HAS_INIT` of that carrier's pin, now bound to the net (F-B1); a net whose init no bound pin carries is seeded through one additive `CircuitBuilder` method, the only solver change, made only if the run needs it and stated in the commit | the source's `signal x: Bit = false` is the net's init (part II §5); folding needs no solver change in the case the buck has |
| **`Child` records no carrier bindings** | **T64** (phase 4, new): `Body::child_carriers(child)`, one entry per carrier port of the child's body (the parent's carrier index, or open), set by `IrBuilder::bind_child_carriers`, printed only when present, checked by `check()`; **T28** fills it for every placement | the POM-navigability rule: the authored tree shows what a placement connects; beside `Child` for the reason of D4 |
| **F-B8** `Float<16>`/`Float<128>` refused by name | **T3**, extended: `Float<16>`, `Float<128>`, `f16` and `f128` are `P0001` naming the type, the span on the type as written; check C9 widened | DIG-01 AC 9 is the type-check rule F-B8 asks for; the IR has no such type, so the checker is the one place |
