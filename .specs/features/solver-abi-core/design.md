# Solver ABI Core Design

**Spec**: `.specs/features/solver-abi-core/spec.md`
**Status**: Draft
**Authority**: `.specs/features/device-abi/abi.md`. Where this document and abi.md disagree on the C layout, abi.md wins and the design is wrong.

---

## Architecture Overview

One new leaf crate. A device links against it and nothing else; the two sub-solvers read shapes and call devices through it.

```mermaid
flowchart TD
    subgraph ABI[piperine-abi: no dependency]
        raw[raw: repr-C structs, u32 constants, List reading]
        layout[layout: BIT REAL BYTE, layout_eq, is_canonical]
        shape[shape: ShapeBuf, ShapeRef]
        device[device: Device, Values, Fault, Flow, Pod, lenses Eval and React]
        thunk[thunk: module of D, library]
        caller[caller: Handle, the solver-side call path]
        validate[validate: shape-local rules]
        verify[verify: finite-difference partials]
    end
    Dev[a device: impl Device] --> device
    device --> thunk
    thunk --> raw
    Dig[piperine-digital-solver] --> caller
    Ana[piperine-analog-solver] --> caller
    caller --> raw
    verify --> caller
```

A call, end to end:

```mermaid
sequenceDiagram
    participant S as solver (caller::Handle)
    participant T as thunk (extern "C")
    participant D as impl Device
    S->>T: eval(instance, k, raw::Eval)
    T->>D: catch_unwind( d.eval(k, lens) )
    D-->>T: Ok or Err(Fault) or panic
    T-->>S: OK, or FAULT with the note filled
```

---

## Code Reuse Analysis

### Existing Components to Leverage

| Component | Location | How to Use |
| --- | --- | --- |
| The struct text of the ABI | `.specs/features/device-abi/abi.md` §3 | copied field for field into `raw.rs`; the test compares sizes and offsets with a hand-written C-layout table |
| Worked examples | abi.md §11.1 to §11.6 | become the fixture devices of `tests/examples.rs`, constant for constant |
| Structural guards | `crates/piperine-solver/tests/crate_boundaries.rs`, `workspace_shape.rs` | extended, not replaced: same scan helpers, one new allowed dependency |
| Hygiene guard | `crates/piperine-lang/tests/suite_hygiene.rs` | walks the new crate with no registration; every new target opens with `//!` |

### Integration Points

| System | Integration Method |
| --- | --- |
| Workspace | `members` and `default-members` gain `crates/piperine-abi` |
| `solver-abi-digital`, `solver-abi-analog` | depend on `piperine-abi`; start bind with `validate`, call devices through `caller::Handle` |
| Codegen features (`ir-interp`, `ir-rust`) | implement `Device`; test with `verify_partials` |

---

## Components

### `raw`

- **Purpose**: the C contract, and nothing else.
- **Location**: `crates/piperine-abi/src/raw.rs`
- **Interfaces**: `Str`, `Bytes`, `List<T>`, `Library`, `Module`, `Value`, `Created`, `Instance`, `Note`, `Shape`, `Pin`, `Layout`, `Field`, `Term`, `Input`, `Watch`, `Cell`, `Partial`, `Function`, `Reaction`, `Statement`, `Eval`, `React`, `Host`, all `#[repr(C)]`, public fields. Constants as `pub const NAME: u32` grouped in plain modules: `kind` (`CONTINUOUS`, `DISCRETE`, `EVENT`), `role`, `global`, `pin_flag`, `leaf`, `field_flag`, `op` (`FLOW`, `DDT`, `DELAY`), `input` (`LIMIT`, `NOISE`, `STIM`), `watch` (`VAR`, `CROSS`, `CAP`, `TRACE`), `cell` (`PIN`, `TERM`, `INPUT`, `WATCH`, `AMBIENT`), `part` (`VALUE`, `PREVIOUS`, `CHANGED`, `TIME`, `LAST`, `AGE`, `PROPOSED`, `LIMITED`, `PSD`, `RE`, `IM`), `ambient` (`TIME`, `FREQ`, `STATIC`, `GMIN`, `SOURCE_SCALE`, `TICK`, `HORIZON`), `fn_flag` (`OWNS_CORRECTION`), `reaction_flag` (`IRREVERSIBLE`), `statement_flag` (`LATER`, `REPLACE`), `status` (`OK`, `FAULT`, `RESHAPE`, `FINISH`), `GROUND`, `DEFAULT`, `MAJOR = 1`, `MINOR`, `SYMBOL = "piperine_abi_1"`. `List<T>::get(i)` and `iter()` read by `stride`; `Str::as_str`, `Bytes::as_slice`.
- **Dependencies**: `core`, `std::ffi::c_void`.
- **Notes**: values of the constants are assigned once here, starting at 1 so that 0 is never a valid kind (an all-zero struct is caught as "unknown constant"). `unsafe impl Send + Sync` for the raw structs: they are immutable plain data once frozen (abi.md §3 "immutable description").

### `layout`

- **Purpose**: the shipped layouts and the two byte rules the solver needs.
- **Location**: `crates/piperine-abi/src/layout.rs`
- **Interfaces**: `pub static BIT: Layout` (`{ v: BOOL @0, known @1 }`, size 2), `REAL` (`F64 @0`, known @8, size 16, align 8), `BYTE` (`U8 @0`, known @1); `LOW`, `HIGH` byte constants; `layout_eq(&Layout, &Layout) -> bool` (P4); `is_canonical(&Layout, &[u8]) -> bool` (P3); `leaf_size(leaf) -> Option<usize>`.

### `shape`

- **Purpose**: let Rust code build a shape as data and keep its lists alive.
- **Location**: `crates/piperine-abi/src/shape.rs`
- **Interfaces**: `ShapeBuf::new()`; `pin(PinSpec) -> u32`, `term(TermSpec) -> u32`, `input(..)`, `watch(..)`, `function(FunctionSpec) -> u32`, `reaction(ReactionSpec) -> u32` (each spec is a plain struct with `&str` names and `Vec`s; the index returned is the entry's index); `freeze(self) -> ShapeRef`; `ShapeRef = Arc<ShapeBuf>`; `ShapeBuf::as_raw(&self) -> *const Shape`; `ShapeBuf::shape(&self) -> &Shape`.
- **Notes**: `freeze` moves every string and list into boxed storage owned by the `ShapeBuf` and only then writes the pointers, so nothing reallocates afterwards. The prototype of a device is a `ShapeRef` held in a `OnceLock`.

### `device`

- **Purpose**: the Rust side of abi.md §7.
- **Location**: `crates/piperine-abi/src/device.rs`
- **Interfaces**:
  - `pub trait Device: Sized + Send + 'static { fn prototype() -> &'static Shape; fn create(values: &Values<'_>) -> Result<(Self, ShapeRef), Fault>; fn eval(&self, k: u32, cx: &mut Eval<'_>) -> Result<(), Fault>; fn react(&self, k: u32, cx: &mut React<'_>) -> Result<Flow, Fault>; }`
  - `Values`: `get(pin) -> Option<&[u8]>`, `f64(pin) -> Option<f64>`.
  - `Fault { code: u32, message: String }`; `Flow::{Continue, Finish}`.
  - `unsafe trait Pod`, implemented by hand for `bool`-free leaves (`u8`..`u64`, `i8`..`i64`, `f32`, `f64`) and `[T; N]`.
  - lens `Eval<'a>`: `f64(i)`, `u8(i)`, `bool(i)`, `i64(i)`, `bytes(i)`, `get::<T: Pod>(i) -> Option<&T>`, `out::<T: Pod>(j) -> Option<&mut T>`, `out_bytes(j)`, `set(j, f64)`, `wanted(p) -> bool`, `jac() -> &mut [f64]`. Lens `React<'a>`: the read accessors plus `commit(statement, bytes, after) -> Result<(), Fault>`.
- **Notes**: a lens borrows the frame and the `Function` or `Reaction` entry of the instance's shape, which is how it knows the byte length of each cell (pin layout size; 8 for a real part; 1 for `CHANGED`, `AGE`). `default` methods `eval` and `react` are not provided: a device with no reaction writes `Ok(Flow::Continue)`.

### `thunk`

- **Purpose**: the generic `extern "C"` entry points.
- **Location**: `crates/piperine-abi/src/thunk.rs`
- **Interfaces**: `pub fn module<D: Device>(name: &'static str) -> Module`; `pub fn library(name: &'static str, build: &'static str, modules: &'static [Module]) -> Library`.
- **Notes**: the instance is `Box<Inst<D>>` with `Inst { device: D, shape: ShapeRef }`, cast to `*mut Instance`. Each thunk wraps the method in `catch_unwind(AssertUnwindSafe(..))`; a panic payload that is a `&str` or `String` goes to the note. `create` decodes `List<Value>` into `Values` after checking each `pin` is a `PARAM` pin of the prototype.

### `caller`

- **Purpose**: the one `unsafe` call path of the solver side.
- **Location**: `crates/piperine-abi/src/caller.rs`
- **Interfaces**: `Handle::create(module: &'static Module, values: &[(u32, &[u8])]) -> Result<Handle, CallError>`; `handle.shape() -> &Shape`; `handle.eval(k, input: &[*const u8], output: &[*mut u8], seed: &[u8], jac: &mut [f64]) -> Result<(), CallError>`; `handle.react(k, input: &[*const u8], commit: &mut dyn FnMut(Commit<'_>) -> u32) -> Result<Flow, CallError>`; `Drop` calls `destroy`. `CallError { status: u32, code: u32, message: String }` (`status` is `FAULT` or `RESHAPE`). `Commit { statement, offset, bytes, after }`.
- **Notes**: `react` passes a `Host` whose `commit` trampolines into the closure through `host_data`. `Handle` is `Send`, not `Sync` (abi.md §12: no two calls on one instance overlap).

### `validate`

- **Purpose**: the shape-local rules of abi.md §9.
- **Location**: `crates/piperine-abi/src/validate.rs`
- **Interfaces**: `validate(&Shape) -> Result<(), ShapeError>`; `ShapeError { rule: Rule, entry: String, origin: String }`; `Rule` is a plain enum with one variant per rule of the spec's list, `Display` giving the rule's sentence.
- **Notes**: it reads lists only through `List::get`, checks indices before following them, and interprets no constant before checking it is known.

### `verify`

- **Purpose**: the verifier of abi.md §8.
- **Location**: `crates/piperine-abi/src/verify.rs`
- **Interfaces**: `verify_partials(handle: &Handle, k: u32, input: &[Vec<u8>], rel_tol: f64) -> Verdict`; `Verdict::{Exact, Skipped { limit: u32 }, Mismatch(Vec<Mismatch>)}`; `Mismatch { partial: u32, declared: f64, finite: f64 }`.
- **Notes**: central difference with step `1e-6 * max(1, |x|)` on the `F64` at byte `at` of `reads[read]`; all partials seeded for the analytic call. A partial against a `LIMIT` input's `VALUE` is perturbed like any read.

---

## Error Handling Strategy

| Error Scenario | Handling | User Impact |
| --- | --- | --- |
| a device panics | `catch_unwind`; `FAULT`, message in the note | the solver reports instance and message; the process lives |
| a device returns `Fault` | `FAULT`, `note.code = fault.code` | the device's own code reaches the host |
| a shape breaks a shape-local rule | `ShapeError` naming rule, entry, `origin` | the author reads `file:line:col` of the offending item |
| `Host.commit` refuses | `cx.commit` returns `Err(Fault)` with the status | the reaction propagates it with `?` |
| a typed access does not fit the cell | `None` | the device decides; no out-of-bounds read exists |

---

## Risks & Concerns

| Concern | Location (file:line) | Impact | Mitigation |
| --- | --- | --- | --- |
| abi.md was never compiled; two Rust-side signatures cannot be written as printed | abi.md §7 (`Arc<Shape>`, `const fn module`) | the trait differs from the document | the C layout is untouched; T9 edits abi.md §7 to the compiled signatures in the same commit |
| A guard forbids any workspace dependency under a sub-solver | `crates/piperine-solver/tests/crate_boundaries.rs` (`sub_solvers_depend_on_no_workspace_crate`) | the next feature's first manifest edit would fail it | T10 changes the guard to "`piperine-abi` only" and records MD-61 |
| abi.md does not say what `get::<T>` returns `None` for | abi.md §7, §12 | lens behavior is under-specified | SAC-06 AC 6 fixes one rule: `None` when `T` does not fit the cell |
| The numeric values of the constants, and the field offsets of `REAL` and `BYTE`, are not in abi.md | abi.md §3, §3.1 P3 | two producers could disagree | `raw.rs` and `layout.rs` are the single assignment; tests pin every value; T2 writes the two layouts into abi.md §3.1 |
| `unsafe` is concentrated in three files | `raw.rs` (`List::get`), `thunk.rs`, `caller.rs` | a layout mistake is undefined behavior | the layout test, the examples through real thunks, and no `unsafe` outside those files (a hygiene-style scan test in `tests/contract.rs`) |

---

## Tech Decisions (only non-obvious ones)

| Decision | Choice | Rationale |
| --- | --- | --- |
| Where `validate`, `verify` and `caller` live | in `piperine-abi` | a device author runs the checks with no solver; both sub-solvers share one call path |
| Shape ownership | `ShapeRef = Arc<ShapeBuf>` | "a library returns the same shape to every instance of equal structure"; the shape lives until its last instance |
| Constants start at 1 | 0 is always "unknown constant" | zeroed memory never passes validation |
| Tests | integration targets under `crates/piperine-abi/tests/`, one per concern | the checks cite plain test names; the public surface is what is tested |
| Project-level decision | MD-61 appended to `.specs/STATE.md` by T10 | `piperine-abi` under the three solver crates amends MD-60's "no in-tree crate sits under any of them" |
