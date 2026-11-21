# Params and I/O Design

**Spec**: `.specs/features/params-and-io/spec.md` (approved 2026-11-18, amended 2026-11-20)
**Status**: Approved (2026-11-20, under the user's standing AFK rule; see "Decisions taken (user AFK)")
**Base**: `main` at `494a7f75`. Batches B1 to B4 (phases 1 to 7) start from it now. B5 and B6
(phases 8 to 12) start only once `composite-values` phases A–B (its B1 to B10, through T63) are on
`main` (D6, D7).

---

## What the code does today (`main` at `494a7f75`)

| Area | Today | Where |
| ---- | ----- | ----- |
| Param types | `f64` and `bool` only; any other type is `P0001` "param of type `..`" | `crates/piperine-lang/src/types/module.rs` (`Checker::param`) |
| Params below the checker | `ir::Param { ty: ValueType, default: Option<Constant> }`; `Constant::Int` already reaches the netlist as `ParamValue::Int` (`Netlist::circuit`); the host's `Params` and `Derive` hold every value as an `f64` | `piperine-ir/src/model.rs:125`, `piperine-plan/src/netlist.rs`, `piperine-api/src/params.rs` |
| Enum and `Option` in the IR | no IR enum type: a unit enum is a `UInt` tag (`EnumLayout`); an `Option` is the pair `(is, value)` (`Expr::Unwrap`). `composite-values` T48 and T50 make both IR types | `piperine-ir/src/model.rs:694` |
| A placement through a param | `P0001` "placement through a template value or a function" | `types/call.rs` (`CallClass::Instance { module: None, .. }`) |
| A top with a continuous port | `P0001` "top module `..` has the continuous port `..`" for a node and for a value input | `crates/piperine-lang/src/elab/elaborator.rs:176` |
| The host's drives | `Session::drive` (a discrete `in` port), `Session::set_at` (a timed `set`, a time point and a jump), `E0802`/`E0803` | `piperine-api/src/host.rs` |
| A potential source | none in the stdlib or the solver; the ngspice twins write `Vsource` in `tests/e2e/twins/devices.msl`; `CircuitBuilder::device` takes only ABI modules | `piperine-solver/src/core/builder.rs` |
| Type mismatches | `P0002` "mismatched types" at 11 sites (`check_as`'s default code, `apply`'s arity, `Option` of what, analog operands); a bad `as` goes through `mismatch_text` | `types/expr.rs:18,347,547,575`, `types/call.rs:174,286,354`, `types/analog.rs:91,225` |
| A param hiding a function (L20) | `P0001` "call of this declaration" | `types/call.rs` (the `_ =>` arm of `callee_resolution`) |
| Const faults | `elab/diagnose.rs` `const_fault`: `Dynamic` is `E0321`, `Unsupported` and `NoValue` are `P0001`, `Panic` is `E0104`, `Unknown` is `E0151`; nothing raises `E0188` | `crates/piperine-lang/src/elab/diagnose.rs` |
| `include_str!` | parsed (`ExprKind::IncludeStr`); the checker accepts it in a `const`; anywhere else `P0001` "`include_str!`" | `types/unsupported.rs:43` |
| `parse_table`, `parse_hex`, `table_model` | declared in the stdlib as generic `extern "intrinsic"` functions; refused as generic functions (`P0001`) | `std/io.msl:18`, `std/analog.msl:287` |
| Attributes | `#[derive]` and `#[repr]` read by name; any other attribute is ignored | `pom/resolve/names.rs:243`, `types/compound.rs:25` |
| Nature attributes | folded in `elab/nature.rs`; a wrong attribute name or a non-positive `abstol` has no spec code | `crates/piperine-lang/src/elab/nature.rs` |
| A run panic | the ABI thunk's `catch_unwind` makes a `Fault` "panic: <message>"; the host prints it without a spec code | `piperine-abi/src/thunk.rs:95`, `piperine-transpile/src/rt/control.rs` |
| The formatter | `msl::format::Formatter::format`; `msl_format` proves it is a fixed point over the corpus, but the stdlib, the galleries and `examples/buck` are not in its output form | `crates/piperine-lang/src/msl/format.rs`, `tests/msl_format.rs` |
| Error codes | 189 codes in the spec's tables, 68 spelled in no non-test source (`GAPS.md` §10, recounted for this design) | `docs/spec/*.md`, `crates/*/src` |

---

## Architecture Overview

The feature is six independent strands that share only the guard of codes. Nothing new crosses
the device ABI; the IR gains one expression pair (`Table`, `TableSlope`) in B6.

```
docs/spec error tables ──spec_codes.rs──► every code: in crates/*/src, or in code_owners.tsv with its owner
checker (types/) ───────── E0180/E0182/E0141/E0188, L20, E0204 E0205 E0244 E0315 E0333 E0354 E0904 E0906, E0361
top with node ports ──elab/host_sources.rs──► IR body `HostSource` + one instance per node port (POM untouched)
                    ──piperine-api source.rs──► NodeSource {dc, ac, pwl} → params of that instance (set / set_at)
module-typed param ──types/module.rs──► Ty::Module param, template value; ──elab──► concrete placement (IR)
include_str! ──elab/files.rs (IncludedFiles)──► Constant::Text, read once per path
[B5] scalar params ──on composite-values' param path (CompositeLayout, leaf set)──► integer / enum / Option params
[B6] parse_table/parse_hex ──ConstEval──► Constant::Aggregate;  table_model ──IR Table/TableSlope──► rt::TableModel
```

### Type errors carry spec codes (X5, L20)

`Checker::check` passes `E0180` to `check_as` where it passed `P0002`; every other site that
builds a "mismatched types" diagnostic takes the code its row names: `E0182` for `as` (§1.4.5),
`E0141` for an arity mismatch of a function value, a probe or a builtin (`apply`), `E0180` for
the rest. The four specific codes (`E0417`, `E0432`, `E0337`, `E0210`) keep winning where they
apply. L20: when the callee path resolves to a param, a `let` or a `const` that is not a
function, the checker looks the name up again in the enclosing namespaces skipping locals; if a
function is found, `E0180` "expected a function, found `<type>`" carries a note at the hidden
function's declaration (§3.5.4). `E0188` comes from `const_fault` when the position is a `const`
item (a `Dynamic` read, a `NoValue` outside a trait, a cycle the const evaluator detects); a param
default keeps `E0321`.

### Codes of constructs that run (PI-12)

Each is one diagnostic at the site that today refuses the form generically or accepts it:
`E0204`/`E0205` in `elab/nature.rs`; `E0315` in `Checker::connect`; `E0333` where a path to a
placement of a module without a result is typed as a value; `E0904` from a data table of A.1
(`AttributeCatalogue`: attribute name → the item kinds it may sit on) walked once over every
attribute; `E0244` where a probe or contribution names a child's node that is not `pub`;
`E0354` where a result is read outside the positions §8.3.10 allows; `E0906` for `Unknown` with
no inferable type; `E0105` where `piperine-api` turns a device fault into a `Failure`: a fault
whose message comes from a model panic under known control carries `E0105` with the operation,
the instance and the time (`E0705` when the panic is `table_model`'s, D19).

### The top's continuous ports (L39, Q1 (a))

The elaborator gives the IR (never the POM, MD-25) one body `HostSource` when the top has a node
port, built by `HostSources` in `elab/host_sources.rs` from IR expressions, with no MSL source:

```
node p (port), params mag, phase, t0, v0, t1, v1 (f64, defaults 0, 0, 0, 0, 1, 0)
analog: v(p, gnd) +> v0 + (v1 - v0) * ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);  ac_stim(mag, phase)
```

and one instance of it per node port of the top, at the path `<top>.<port>.source`, its node
bound to the port's design node. A value input of the top is `E0811` at elaboration. In
`piperine-api`, `NodeSource { dc, ac: Option<(mag, phase)>, corners: Vec<(t, v)> }` is given per
port by `Session::source(port, NodeSource)` before a run; a run that starts with a node port
without one is `E0810` naming it. DC sets `v0 = v1 = dc`; AC sets `mag`, `phase`; a PWL is
turned into timed sets of the segment params (`t0, v0, t1, v1` of corner `k` and `k + 1`) at
each corner's time through `set_at`, so each corner is a time point (§15.2) and the value is
exact on the segment. `Session::source_at(port, value, at)` is a timed set with `v0 = v1`: a
jump (PI-07 AC 4). The CLI has no manifest key for a port (X8): `piperine run`/`test` of such a
top is `E0810`.

### Module-typed params (PI-04)

`Checker::param` accepts `Ty::Module(decl)` for a param whose type names a module; its default
or a binding is a template value `Name { params }` (`ExprKind::Struct` resolving to a module),
typed `Ty::Module(Name)`, with a template of another module `E0180` naming both. A placement
whose head is such a param (`cell(c)`) is checked against the module's ports; a param block on
it is `E0361`. The param is structural (§8.4.6): the elaborator folds the template (module and
its param block, `E0319` for a required param left unbound) and places the concrete module
where `cell(c)` is written. The POM keeps `cell(c)` as written; the IR holds the concrete
instance (MD-25).

### `include_str!` (PI-08)

The checker types `include_str!(..)` as `&str` in the initializer of a `const` and refuses it
anywhere else with `E0610`. `IncludedFiles` (owned by the `Elaboration`) resolves the path
against the directory of the source file that holds it (an absolute path as it is, §20
Resolution), reads each resolved path once and caches the text; an unreadable file is `E0612`
naming the resolved path, a file that is not UTF-8 is `E0613` with the byte offset of the first
invalid sequence (`Utf8Error::valid_up_to`). The value is a `Constant::Text` of the `const`.

### Scalar params (B5, PI-01 to PI-03, PI-05)

On `composite-values`' param path (its T47, T58, T59): `Checker::param` accepts `UInt<N>`/`Int<N>`
(1..=64), unit enums and `Option<T>` of these; the elaborator folds defaults and bindings to
`Constant::Int`, the IR enum constant and the IR `Option` constant; the plan lays them out in
the param pin (`CompositeLayout` for `Option`); the host parses `--param`/`set` text by type: an
integer literal of the type and range, a variant name of the enum (`E0802` listing the
variants), `None` or a value of `T` for `Option<T>` (D24). The interpreter refuses any param
that is not `f64` or `bool`, naming the param and the transpiled backend (R1).

### Files and tables (B6, PI-09, PI-10)

`parse_table` and `parse_hex` are typed by the intrinsic catalogue: their generic arguments come
from the `const`'s declared type (`[(f64, f64); 64]`, `[u32; 1024]`), with no general generics
(D16). `ConstEval` folds them into `Constant::Aggregate`: `TableText` reads rows of real literals
(SI suffixes, blank lines and `#` comments skipped), `HexText` reads a `$readmemh` text
(`_`, `//`, `/* */`, `@address`, `x`/`z` digits unknown). Shape errors are `E0614` (file and
row), format errors `E0706` (file, row, token). `table_model(x, &T, "ctrl")` is checked against
the catalogue; the elaborator parses the control into `TableControl` (degree `D`/`1`/`2`/`3`,
extrapolation low and high `C`/`L`/`E`; `E0703`), validates the table (`E0704`: abscissae not
strictly increasing, too few points), and lowers to `Expr::Table { x, table, control }` with
`Expr::TableSlope` as its derivative (the symbolic derivative rule of `elab/derivatives.rs`).
The printer calls `rt::TableModel` (spline coefficients computed once per table, D18); an `x`
outside the table under `E` raises `Panic` coded `E0705`.

### Formatting (PI-11)

The stdlib files, `examples/buck/src` and the `rust` fences of Appendices D, E and F are
rewritten in the formatter's output; `msl_format` and `std_galleries` gain the guard tests;
`msl::print` of each is unchanged.

### The guard of codes (PI-13)

`crates/piperine-lang/tests/spec_codes.rs` reads every row `| \`Ennnn\` |` of every table under
`docs/spec/` (through `support/spec_tables.rs`), and every non-test `.rs` file under
`crates/*/src`, counting a code as raised when it is spelled on a line that is not a comment
(GAPS §10's rule). `support/code_owners.tsv` (code, owner feature, construct) is the allow-list.
Three tests: every code is raised or listed; every listed code is unraised (a stale entry fails
naming it); every entry names an owner and a construct. A fourth test after T5: no source
spells `P0002`.

---

## Code Reuse Analysis

### Existing Components to Leverage

| Component | Location | How to Use |
| --------- | -------- | ---------- |
| `Checker::coded`, `check_as(expr, ty, code)` | `types/module.rs`, `types/expr.rs` | every new code is one `coded` call at its site |
| `Elaboration::const_fault` | `elab/diagnose.rs` | `E0188` is a position-aware arm |
| `ConstEval`, `ConstFault` | `elab/consts.rs` | `include_str!`, `parse_table`, `parse_hex` fold here |
| `IrBuilder` | `piperine-ir/src/build.rs` | the `HostSource` body and its instances |
| `Session::set_at`, `ParamPath`, `Params` | `piperine-api/src/host.rs`, `params.rs` | node sources are params of the source instances |
| `spec_tables::section` and row helpers | `crates/piperine-lang/tests/support/spec_tables.rs` | the guard reads the spec's tables |
| `Formatter::format` | `msl/format.rs` | rewrite and guard the shipped sources |
| `Panic::raise` | `piperine-transpile/src/rt/control.rs` | `E0705` and `E0105` |
| `piperine_abi::verify` | `crates/piperine-abi/src/verify.rs` | the partials of `table_model` (PI-10 AC 8) |
| `ToleranceContract` | `crates/piperine-parity/src/contract.rs` | host sources against `ac_rc.cir` and a new `tran_pwl_rc.cir` |
| `composite-values`' `CompositeLayout`, `TypeArena`, `ConstValue`, `types/class.rs` | after its merge | B5 and B6 build on them |

### Integration Points

| System | Integration Method |
| ------ | ------------------ |
| `piperine-cli` | `--param` text by type (B5); `E0810` for a top with a node port |
| `piperine-api` | `NodeSource`, `Session::source`, `Session::source_at`; `E0105`/`E0705` on a run fault |
| `piperine-transpile` | `rt::TableModel`, the printer arm of `Expr::Table`/`TableSlope` (B6) |
| `piperine-interp` | refuses `Expr::Table`/`TableSlope` and non-`f64`/`bool` params by name |
| The Book (`tests/book/`, `inventory.tsv`) | rows printing `P0002`, the L28 and L57 rows, the top-port rows |

---

## Components

### C1. `SpecCodes` guard (`crates/piperine-lang/tests/spec_codes.rs`)

- **Purpose**: every code of the spec raised or owned.
- **Interfaces**: `CodeTable::from_spec(dir)`, `SourceSpellings::from_crates(dir)`, `CodeOwners::load(path)`; three tests, a fourth after T5.
- **Reuses**: `support/spec_tables.rs`.

### C2. Checker codes (`types/`)

- **Purpose**: the codes of PI-06 and PI-12 at their sites; `AttributeCatalogue` in a new `types/attributes.rs`.

### C3. `HostSources` (`crates/piperine-lang/src/elab/host_sources.rs`)

- **Purpose**: the IR body `HostSource` and one instance per node port of the top; `E0811`.
- **Reuses**: `IrBuilder`, the analog lowering's contribution and `ac_stim` IR forms.

### C4. `NodeSource` and `NodeSources` (`crates/piperine-api/src/source.rs`)

- **Purpose**: a host's source per node port; the segment schedule of a PWL; `E0810`.
- **Interfaces**: `Session::source(&mut self, port: &str, source: NodeSource) -> Result<(), Failure>`, `Session::source_at(&mut self, port: &str, value: f64, at: f64) -> Result<(), Failure>`.

### C5. Module-typed params (`types/module.rs`, `elab/children.rs`)

- **Purpose**: PI-04; placement through the param.

### C6. `IncludedFiles` (`crates/piperine-lang/src/elab/files.rs`)

- **Purpose**: read each included file once, resolve paths, `E0612`/`E0613`.

### C7. `TableText`, `HexText` (`crates/piperine-lang/src/elab/tables.rs`, B6)

- **Purpose**: the §22.6.4 formats folded to `Constant::Aggregate`; `E0614`, `E0706`.

### C8. `TableControl`, `Expr::Table`, `rt::TableModel` (B6)

- **Purpose**: `table_model` from the checker to the generated Rust, with its derivative.

---

## Data Models

```rust
/// One host source on a node port of the top (§23.1, Q1 (a)).
pub struct NodeSource {
    /// The value at the operating point and between corners when no corner is given.
    pub dc: f64,
    /// The AC stimulus: magnitude and phase in degrees.
    pub ac: Option<(f64, f64)>,
    /// The corners `(time, value)` of a piecewise-linear waveform, strictly increasing in time.
    pub corners: Vec<(f64, f64)>,
}

/// The parsed control string of `table_model` (§22.2.15).
pub struct TableControl {
    pub degree: TableDegree,          // Discrete, Linear, Quadratic, Cubic
    pub below: TableExtrapolation,    // Clamp, Linear, Error
    pub above: TableExtrapolation,
}
```

IR (B6), appended last: `Expr::Table { x: ExprId, table: u32, control: TableControl }`,
`Expr::TableSlope { .. }` with the same fields. The checker's `Ty` gains no variant for module
params (`Ty::Module` exists).

---

## Error Handling Strategy

| Error Scenario | Handling | User Impact |
| -------------- | -------- | ----------- |
| A mismatch at a position no row names | `E0180` expected/found | was `P0002` |
| A bad `as` | `E0182` naming both types | was `P0002` |
| Arity of a function value, probe or builtin | `E0141` expected/found counts | was `P0002` |
| A `const` initializer that needs a run, a `const` without a value, a cyclic `const` | `E0188` naming the `const` | was `E0321`/`P0001` |
| A run of a top with an undriven node port | `E0810` naming the port, host phase, before any analysis | was `P0001` at elaboration |
| A top with a value input | `E0811` naming the input, elaboration | was `P0001` |
| A model panic under known control during a run | `E0105` naming the operation, the instance and the time | was a bare fault text |
| `table_model` outside the table under `E` | `E0705` naming the instance and the call | new |
| `--param` of a module-typed param | `E0802` "a module-typed param is bound in the source" (D13) | new |

---

## Goldens, pins and tests that may move

Only these move, each under the task named; a replaced test is replaced by the acceptance test
of the criterion that supersedes it, and the commit body names both and the spec line.

| Pin | Moves in | Why |
| --- | -------- | --- |
| every test asserting `P0002` (`crates/piperine-lang/tests/types_*.rs`, `elab_*.rs`, `tests/book/*.rs`, `crates/piperine-cli/tests/book.rs`) | T3, T4, T5 | the spec code replaces it (A1, A2); the expected code changes, the expected span and message keep |
| tests asserting `P0001` for "call of this declaration" | T6 | L20 |
| tests asserting `E0321`/`P0001` on a `const` initializer | T7 | `E0188` (§3.9.2) |
| tests asserting the `P0001` "continuous port" of a top (`elab_entry.rs`, the CLI) | T17, T20 | `E0811`, `E0810` |
| tests asserting `P0001` "placement through a template value" | T21 | PI-04 |
| tests asserting `P0001` "`include_str!`" | T23 | PI-08 |
| tests asserting `P0001` "param of type `..`" (`types_modules.rs`, `elab_params.rs`, `tests/book/digital.rs`, `core_std.rs`) | T30, T35 | L28 |
| the Book rows of L28, L39, L57, X5 and the `P0002` listings in `tests/book/inventory.tsv` | the task that changes the block | `inventory_guard` prints the corrected row |
| `crates/piperine-cli/tests/docs.rs` rows of L20, L28, L39, L57, X5, H12 | T28, T44 | the rows leave `GAPS.md` |
| `crates/piperine-lang/std/*.msl`, `examples/buck/src/*.msl`, the D/E/F fences | T25, T26 | formatter output; printed trees unchanged |

The 7 twin goldens, the transpile goldens of `crates/piperine-transpile/tests/golden/`, every
`ToleranceContract` and every parity target stay byte-identical (the formatting of the buck is
checked against the unchanged printed tree and every buck target). `tran_pwl_rc.cir` is a new
twin with no `.command` file, so the CLI twin count of `crates/piperine-cli/tests/twins.rs` stays
as it is (D11).

---

## Cross-lane touches

| Lane | Shared files | Rule |
| ---- | ------------ | ---- |
| `composite-values` (in flight, B1–B10) | `types/module.rs` (`param`), `types/call.rs`, `types/expr.rs`, `elab/body.rs`, `elab/consts.rs`, `piperine-api/src/host.rs`, `params.rs` | B1–B4 touch `param` only for PI-04 and `call.rs`/`expr.rs` only for codes; T29 rebases, and every `P0002` its branch added becomes the code `docs/spec` names (the guard fails otherwise) |
| `unknown-semantics-events`, `analog-operators` (siblings, item 10) | `tests/book/*.rs`, `inventory.tsv`, `spec_text.rs`, `docs.rs`, `GAPS.md` | rows edited per row, never reordered; appended to, never edited above |
| `generics` (after CV) | `types/call.rs` (templates), `code_owners.tsv` | `generics` removes its own entries as it raises them |
| every lane | `crates/piperine-lang/tests/support/code_owners.tsv` | a lane that raises a listed code removes its row in the same commit (the guard's stale-entry test enforces it) |

---

## Risks & Concerns

| # | Risk | Mitigation |
| - | ---- | ---------- |
| R1 | `composite-values` lands late; B5 and B6 wait | B1–B4 deliver L20, L39, X5, H12 and 19 of the 22 codes without it; T28 records the pause |
| R2 | The `HostSource` body's `clamp` or `ac_stim` is not in the interpreter's subset | the interpreter refuses a design with host sources by name, naming the transpiled backend (R1 of codegen) |
| R3 | Formatting the buck changes a generated name or a span a test reads | the printed tree is compared before and after (PI-11 AC 4); any test that reads text positions is listed in the commit |
| R4 | `E0244`, `E0354` or `E0906` may be unreachable today (another code wins first) | T1 probes each and records the form that reaches it; a code that no form reaches stays in the allow-list under the feature whose construct reaches it, and the finding is reported (D4) |
| R5 | A new `P0002` arrives from `composite-values` at the rebase | the guard's `no_source_spells_p0002` fails at T29, which maps each to its `docs/spec` code |
| R6 | ngspice is absent on the machine for the new PWL twin | the parity harness's recorded golden is used as for the other twins; generating it is part of T19 |

---

## Divergences found (current code vs docs/spec in this area), each a fix task

| # | Divergence | Spec | Fix |
| - | ---------- | ---- | --- |
| V1 | A plain mismatch is `P0002` | Part I `E0180` | T5 |
| V2 | A bad `as` is `P0002` | §1.4.5, `E0182` | T3 |
| V3 | A call of a named emitting function where `emit!` is not allowed is `E0601` | §6 `E0601` row: such a call is `E0122` | T36 |
| V4 | An unknown attribute (`#[cfg(test)]`) is accepted and ignored | A.1 Resolution: an error, `E0904` | T11 |
| V5 | A top's node port is refused at elaboration | §8.1.14, §23.1: the host connects it | T16 to T20 |
| V6 | A non-constant `const` initializer is `E0321` | §3.9.2, `E0188` | T7 |

---

## Decisions taken (user AFK)

The user said "always take the most reasonable default and tell later" ("Eu gostaria que
engatilhasse até o 12"). Each default follows the approved spec, `docs/spec/` (the authority)
and the active MD decisions; each is reported back.

| # | Open point | Default taken | Why |
| - | ---------- | ------------- | --- |
| D1 | Q2 named a new code `E0106`; the spec's revision 38 since added `E0180` with that very row | `E0180`; no spec row is added; the spec keeps no `E0106` (A1) | `docs/spec` is the authority; one code per condition |
| D2 | X5's scope | every checker diagnostic of a type or const error that runs gets its spec code: `E0180`, `E0182` (`as`), `E0141` (arity), `E0188` (const expressions) (A2) | the spec's success criterion "every diagnostic the type checker emits for a mismatch carries a spec code"; `P0002` disappears |
| D3 | `E0102` and `E0122` were this spec's PI-12 AC 1 and 3; `composite-values` raises `E0102` (T43) and `E0122` for closures (T16) | they stay `composite-values`'; this feature raises `E0122` for named functions after its rebase (T36) and checks `E0102` through the guard (A3) | one owner per construct; no duplicate implementation across lanes |
| D4 | `E0244`, `E0354`, `E0906` appeared unowned after `analog-structure` and `unknown-semantics` closed their gaps | raised here (A4); a code no reachable form can reach is reported and listed under the feature whose construct reaches it | the sibling specs' rule: codes of constructs that already run are `params-and-io`'s |
| D5 | The code table predates four deliveries and revision 38 | recounted at `494a7f75`: 68 of 189; `E0807` to `E0809` are `gradients`' (A5, A6) | the guard reads the spec as it is |
| D6 | Scalar params (PI-01 to PI-03, PI-05) do not need arrays | they wait for `composite-values` phase B all the same: its T47, T58 and T59 rebuild the param check, the param pin and the host's `set` by leaf path, and its T48 and T50 make enums and `Option` IR types; doing them before would write that path twice | ROADMAP lists `composite-values` before this feature; its design says this feature adds integer, enum and `Option` params on its path |
| D7 | Order against the in-flight lanes | B1 to B4 start from `main` at `494a7f75` now; B5 and B6 start once `composite-values` A–B is on `main` (T29 rebases). If it is not there when B4 closes, the feature pauses and reports | the caller's split |
| D8 | How the host source is built (Q1 (a) gives no mechanism) | an IR body `HostSource` the elaborator builds from IR expressions, one instance per node port at `<top>.<port>.source`; no MSL source and no stdlib module; the POM is untouched | the spec defines no source module, so none is added to the language (code follows spec); the IR is the side artifact MD-25 allows |
| D9 | A PWL with no solver primitive | corners become timed sets of the instance's segment params (`set_at`), each a time point; a jump is a timed set with `v0 = v1` | `set_at` already has §15.2's semantics; linear between corners is exact |
| D10 | Phases of the two new codes | `E0810` host (session start); `E0811` elaboration (the top is chosen when elaboration starts, G.17); both rows in §23.1's errors table with their phase, and one sentence each in §8.1.14 and §23.1 rule 1 | each is raised where the information first exists |
| D11 | ngspice parity for PI-07 | the RC of `ac_rc.cir` and `tran_rc.cir` written as a top with the node port `vin` and driven by the host: AC against `ac_rc.cir` (`AC_MAGNITUDE`, `AC_PHASE`), the DC step against `tran_rc.cir` (`TRAN_VOLTAGE`), and a PWL against a new twin `tran_pwl_rc.cir` (no `.command`) under `TRAN_VOLTAGE`; no tolerance moves | the user's rule: parity first, never retune |
| D12 | The guard's shape | `crates/piperine-lang/tests/spec_codes.rs` with the allow-list as data in `tests/support/code_owners.tsv` (code, owner, construct); "raised" = spelled on a non-comment line of `crates/*/src` | GAPS §10's counting rule; data, not macros (user preference) |
| D13 | `--param` or `set` of a module-typed param | `E0802` "a module-typed param is bound in the source"; a `set` during a run stays `E0803` (structural) | the spec gives a host no text for a template |
| D14 | `include_str!` in a source with no file on disk (in-memory, stdlib) and a relative path | `E0612` naming the path and saying the source has no directory | deterministic; never the process's working directory |
| D15 | `E0613`'s offset | `Utf8Error::valid_up_to` of the file's bytes | the first invalid sequence, as PI-08 AC 4 says |
| D16 | `parse_table`, `parse_hex`, `table_model` are generic intrinsics | typed by the intrinsic catalogue from the `const`'s declared type and the argument's type; no general generic functions | `generics` (L7) is not on `main`; the three are closed forms |
| D17 | `table_model` in the IR | `Expr::Table`/`Expr::TableSlope`, appended last, the table a folded constant and the control parsed at elaboration (`E0703`, `E0704` there) | elaboration errors at elaboration; the run only interpolates |
| D18 | Spline end conditions and the derivative at a kink | cubic not-a-knot; quadratic: first segment the parabola through the first three points; at a sample the right segment's slope, at the last sample the left's; written in Appendix C.2 (T27) | the spec's own assumptions table |
| D19 | How the host knows a panic's code | `rt` raises `Panic` with a payload that starts with the code (`E0705: ..`); the API reads a leading code and otherwise gives `E0105` | one decoding place; the ABI's fault text is unchanged |
| D20 | What is formatted | `crates/piperine-lang/std/*.msl`, `examples/buck/src/*.msl`, the D/E/F fences; `tests/e2e/buck` follows `examples/buck` if it is a copy | PI-11's list |
| D21 | The decision record | one `MD-` entry with the next free number at merge, written at T28 and completed at T44 | STATE practice |
| D22 | Approval lines | tasks.md: `**Approved**: 2026-11-20 - under the user's standing AFK rule - user: "Eu gostaria que engatilhasse até o 12"`; the spec keeps its 2026-11-18 approval and gains an `**Amended**` line | the siblings' practice |
| D23 | Test placement | checker codes in the existing `types_*` targets by area (`types_values` X5 and L20, `types_modules` `E0315`/`E0333`/`E0361`/module params, `types_carriers` `E0244`, `elab_analog` `E0204`/`E0205`, `types_results` `E0354`, `types_unknowns` `E0906`, `types_structure` `E0904`); `elab_values` `E0188`; new `elab_files.rs` for files and tables; `run_errors` for `E0105`; new `crates/piperine-api/tests/host_sources.rs`; new `crates/piperine-parity/tests/source_host.rs` and `source_tables.rs` for parity and runs | MD-28: a target lives in the crate it exercises; one concern per target |
| D24 | Host text of the new param types | an integer literal (decimal, `0x`, `0b`, `_`) in range of `N`; a bare variant name; `None` or a value of `T` for `Option<T>`; anything else `E0802` naming the param and the value (listing variants for an enum) | the spec's assumptions table |
| D25 | The §18 `Diode` (what remains of L40) waits on "generics and params-and-io" (ROADMAP 3f) | not in this spec; listed for the user | its spec does not name it |

---

## Tech Decisions

| Decision | Choice | Rationale |
| -------- | ------ | --------- |
| The ABI | unchanged | node sources are ordinary device instances; params keep their pins |
| IR | `Expr::Table`, `Expr::TableSlope` appended last (B6); the `HostSource` body is ordinary IR | no new body kind |
| The `MD-` entry | "params of every carried type; the host drives a top's node through an IR source; files read at elaboration; every spec code raised or owned (`spec_codes`)" | D21 |
