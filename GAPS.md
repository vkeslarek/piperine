# GAPS.md — what does not work yet

**Full audit 2026-11-17** of the implementation at `54139be1` against `docs/spec/` (Parts I–VIII,
Appendices A–H), section by section. It replaces the incremental file written after `msl-digital`,
`msl-cli` and `long-run`; every entry of that file that still holds is kept here, re-verified.

This file lists **known gaps**: what is missing, wrong or unproven today. Planned work and its
order live in `ROADMAP.md` ("Queue after ir-rust"); this file says what a user hits *now*. Remove
an entry in the commit that closes it.

**Ranking**, most serious first: **silent** (accepted, then wrong or ignored) › **broken** (does
not build or run) › **loud** (refused with a diagnostic) › **spec hole** (the spec does not say, or
says something the code does not do) › **unproven** (works, nothing falsifies it). The section
numbers are the previous file's (`piperine-cli/tests/docs.rs` reads them), so *Broken* is §2 and
*Silent* §3.

**Evidence.** `probe` means a lone `.msl` file run on 2026-11-17 with
`piperine run <file> --analysis op|transient` on the Rust backend; the repro is given inline where
it is short. A diagnostic is quoted as the binary prints it. `P0001` is the code of "not supported
yet"; `P0002` of a plain type mismatch.

**Size hints.** S: days, one crate. M: one feature of 10 to 25 tasks. L: several features, or a
change of the IR or the ABI.

**Counts.** 1 silent, 2 broken, 28 loud (L1 to L66 less the nine `msl-fixes`, the one `host-ii`, the two `unknown-semantics`, the nine `analog-structure`, the two `unknown-semantics-events` (L51, L52), the two `params-and-io` (L20, L39) and the nine `composite-values` rows (L1, L2, L3, L5, L6, L10, L16, L63, L66), the two `resolved-nets` rows (L22, L23) and the two `explicit-unknowns` rows (L64, L65) closed, S8 closed by `explicit-unknowns` phase A, L9 narrowed to its `generics` part and L4 to generic enums, in six tables) plus
7 backend and diagnostic notes, 9 spec holes or disagreements, 3 solver and performance points,
17 host gaps, 9 unproven.

| Section | Ids |
| ------- | --- |
| §2 Broken | B3, B4 |
| §3 Silent (read this first) | S3 (S6 and S8 closed by `explicit-unknowns`) |
| §4 Loud | L1–L66 (L64 and L65 closed by `explicit-unknowns`; L11, L14, L15, L17, L18, L19, L20, L39, L41, L46, L48, L54, L55, L56, L26, L27, L34–L38, L42, L50, L5, L6, L10, L1, L2, L3, L16, L66, L51, L22, L23 closed; L9 and L4 narrowed; L64 to L66 found by `composite-values`) |
| §5 Spec holes and disagreements | G1–G5 (the buck's; G6 closed by `explicit-unknowns`), X1, X2, X6 (X3, X4, X7, X8, X9 decided by `host-ii`; X5 closed by `params-and-io`) |
| §6 Solver, numerics, performance | V12, V13, V16 |
| §7 Host surface | H1–H18 (H11 closed by `root-e2e`, H12 by `params-and-io`) |
| §8 Unproven | U1–U11 (U5, U10 closed by `root-e2e`) |
| §9 Coverage by spec part | — |
| §10 Reference lists, changed entries, clusters | — |
| §11 The Book against the binary | one row per Book block or claim the binary refuses, does not meet, never raises or the spec disputes |

---

## 1. What runs today

| Path | State |
| ---- | ----- |
| lex, parse, print, format | the whole grammar of Appendix B; every stdlib module, gallery model, spec fence and `examples/buck` (`msl_corpus`, `msl_format`: 277 sources) |
| POM, name and member resolution | complete over the same corpus (`pom_*`); crates, packages, path and git dependencies, the lockfile (`piperine-project`) |
| type check and elaboration | a **subset**: `bool`, `UInt<N>`/`Int<N>` up to 64 bits, `f64`, structs, tuples, arrays and enums with or without a payload (value types; `#[derive(Repr)]` on the ones a carrier holds), `Option` and its methods (H.3), plain functions, continuous natures with a flow, `Bit<T>` of a scalar, an enum, a struct, a tuple, an array or an `Option`, modules with `f64`/`bool` params and `where`, definitions with `after`, an `analog` block whose contributions sit under `if`/`match` (switch branches), `ddt` anywhere, modules with a result, a value input or a node (hoisted), feedback through them, `inst.name`, `p.inflow()`, a body `let` as a param value, an initial value over params, `changed` and `prev` of a param, reactions with `if`/`match`/`if let`, `at`, `cross`, `changed`, `prev`. Everything else is `P0001` by name (§4) |
| backends | IR → Rust → `rustc` → device library: everything the checker accepts. The interpreter is frozen at the M1 analog subset (R1) |
| analyses from a host | `op`, `dc` (one sweep), `ac`, `transient`; on both backends for the 7 ngspice twins |
| digital and mixed-signal | on the Rust backend only: `examples/buck` whole, closed loop at 0.8 V (`buck_closed_loop`, `buck_long`), scenarios S2 to S10 on the Rust host (`buck_scenarios`) |
| stdlib that runs | `rise`, `fall`, `init`, `every`, `settle`, `vt`, `pnjlim`, `fetlim`, `deltalim`, `limexp`, `above`, `last_crossing`, `transition`, `normal`, `D2A`, `A2D`, `Never`, `State`, `log`, `require`, `cover`, `target`, the six constants. About 30 of the 75 names of the §22 index |
| command line and host library | `check`, `build`, `run`, `test`, `new`, `clean`, `fmt`; tables, record lines, CSV, JSON Lines, SVG; `--param` before a run; a transient keeps rows, not steps: `piperine run startup` in `examples/buck` prints the whole 600 µs start |
| language server | **does not build**, deferred by the user |

---

## 2. Broken — does not build or run

### B3. `piperine-lang-server` does not build — size M, deferred

- It targets the deleted front end and sits outside `default-members`. User, 2026-11-16: "O
  lang-server é o menos importante, vai ficar para bem depois." `piperine check` gives the same
  diagnostics meanwhile.

### B4. No Python host — size L

- There is no Python crate; `examples/buck/README.md`'s Python and the sketches of §23 have no
  implementation.

---

## 3. Silent — accepted, then wrong or ignored

### S3. The type checker checks only what a run's top reaches — size M

- **What.** `Types::check` walks the reachable set from the top (`types/check.rs:147`). A
  function or module nothing reaches is resolved and never typed:
  `fn broken(x: f64) -> bool { x + 1.0 }` and a module that calls `ddx` pass `piperine check`
  (`ok: u (2 crates, 0 runs)`). A library package with no run is never type-checked. An unreached
  `const X: &str = include_str!("missing.txt")` is accepted: `E0610`, `E0612` and `E0613` are
  raised nowhere.
- **Impact.** `check` is green for a library that cannot compile. §0.3 puts every static check
  in elaboration, of the whole model.
- Generic items need an instantiation to check (X6); the others do not.

---

## 4. Loud — refused, though the spec allows it

Every entry is a diagnostic at type check or elaboration unless it says otherwise. The refusals
are tables with no wildcard arm: `types/unsupported.rs`, `elab/unlowered.rs`,
`types/catalogue.rs` (`F64_METHODS`, `INTEGER_METHODS`, `CORE_ROWS`). The diagnostic column
quotes the text after `P0001:` unless another code is shown.

### 4.1 Values (Part I §1–§3, Appendix H)

| # | Refused | Spec | Diagnostic (probe) | Size |
| - | ------- | ---- | ------------------ | ---- |
| L4 | generic enums (`enum Tagged<T> { .. }`; §2: every other enum, a payload enum, `#[repr(uN)]`, explicit discriminants and an enum of one variant, is a value type, and the one a carrier holds derives `Repr`) | §2 rules 2, 5 | `generic enum` | `generics` GN-06 |
| L7 | generic functions, `self` parameters, `impl` on a value type, user traits | §3 rule 9, §3.1 | `generic function`, `generic parameter used as a type` | L, with L30 |
| L8 | `impl` on a nature (other than the stdlib's `impl Bit`); a function that takes a carrier or a node and probes it (a handle) | §3.1, §3 rule 7 | `method mv`; `E0450: a probe is allowed in an analog block and a module body only` | M |
| L9 | a capturing closure passed to a user `fn` parameter (it needs an `Fn` bound: `generics` GN-01) | §3 rule 3 | `P0001` naming the form | with L7 |
| L12 | `extern "C" fn`, `#[derivative]` | §3 rules 2, 10, 11 | `extern function` | M |
| L13 | `f16`, `f32`, `f128`; integers wider than 64 bits | §1 rule 5 | `type f32`; `integer type u128` | M |

### 4.2 Carriers (Part II §4–§7)

| # | Refused | Spec | Diagnostic (probe) | Size |
| - | ------- | ---- | ------------------ | ---- |
| L21 | natures that implement `Resolve<X, V>` (declared by the model; the library has no net nature); `others()`; relaxation of loops through the nets of such a nature (a loop that does not settle is `E0511`, raised by the digital engine) | §4 rule 2, §5.2.2, §8.2.3 | `resolved nature Wire` | L |
| L24 | arrays of carriers (`[Bit; 8]`, `[Electrical; 4]`); element and field commits (`bits[3] <- true`) and definitions | §5.1.4, §12.1 | `array type`; `an element or field commit` | L, with L3 |
| L25 | bundles: a `struct` of carriers, `flip`, bundle literals, `impl` on a bundle | §7 | `struct item used as a type`, `flip type` | L |

Works, by probe: continuous natures with their own access names (`State` runs; `Thermal` and a
project's nature pass the checker), `potential`/`flow`, a flow probe of a potential branch, a
held node (`= 0.7` holds through a transient's operating point and is free in `op`), `prev()`
before and after the first change, `E0150`, `E0210`, `E0220`, `E0221`, `E0222`, `E0241`, `E0250`,
`E0251`, `E0310`.

### 4.3 Structure (Part III §8–§9)

| # | Refused | Spec | Diagnostic (probe) | Size |
| - | ------- | ---- | ------------------ | ---- |
| L28 | params of an integer, enum, `Option` or module type (`n: u8`, `md: Mode`, `rs: Option<f64>`, `cell: C`); also owes `explicit-unknowns` EU-06 AC 1 (an `Option<T>` param no instance binds is `None`, check C20, test `an_unbound_option_param_is_none`, interim `an_unbound_option_param_waits_for_params_and_io`) | §8.1.15, §3.2.6 | `param of type u8` | M, `params-and-io` B5-B6 |
| L29 | `m` and `dtemp`: binding them at a placement and reading them | §8.5.1–§8.5.6 | `E0320: no param m in R`; `built-in m` | M |
| L30 | generic modules and const generics: the prelude's own `Short<N>` and every stdlib module built on them | §8 rule 4 | `generic module` | L |
| L31 | generation `for` and `if`, in a body and in a `digital` block | §8.2.7, §12 rule 1 | `generation for/if in a module body` | M |
| L32 | templates (`Name { .. }` as a value), placement through a template or a function, `Fn(P)` bounds | §8.4 | `module template Name { .. } outside a placement` | M, after L28 |
| L33 | `extern` modules: `"osdi"`, `"cosim"`, `#[link]` | §8.5.7–§8.5.16 | `extern module` | L |

Works, by probe: `where` at elaboration and on a set (`E0317`, `E0802`), `E0312`, `E0314`,
`E0316`, `E0320`, `E0330`, `E0335` to `E0339`, `E0343`, `E0457`, definitions with and without
`after`, an expression at an `in` port, a result module without state, a body `let` shared by
blocks.

### 4.4 Analog (Part IV §11, §14)

| # | Refused | Spec | Diagnostic (probe) | Size |
| - | ------- | ---- | ------------------ | ---- |
| L40 | the three spec models that wait on another feature's dependency: the stdlib `idt` and `Water` (they need `generics`) and the §18 `Diode` with `match rs` (it needs `params-and-io`); a contribution under an `if` or a `match`, `Switch`, `CapIc` and `Bounded` run | §11 rules 2, 3, 18, §18 | `generic module` (`idt`, `Water`); `the diode needs params-and-io` | M |
| L43 | `ddx` | §14.2 | `core primitive ddx` | M |
| L44 | `delay` | §14.6 | `core primitive delay` | M |
| L45 | `noise`, and so `white_noise`, `flicker_noise`, `noise_table` | §14.8, §11.1.8 | `core primitive noise` | M |
| L47 | `simparam` (`freq()` runs) | §14.4 | `core primitive simparam` | S |
| L49 | No trace records: `emit!` in `analog` is refused | §19 rule 4, §11 rule 20 | `an emit! in an analog block` | M |

Works, by probe: `ddt` of a flow and of a potential (capacitor, inductor, coupled inductors),
`limit` with a closure and a seed, `ac_stim` with a phase, `temperature()`, `gmin()`,
`source_scale()`, `now()`, `analysis(Static)`, an `if` *expression* on a probe, `E0415`.

### 4.5 Digital and events (Part IV §12–§13)

| # | Refused | Spec | Diagnostic (probe) | Size |
| - | ------- | ---- | ------------------ | ---- |
| L53 | a function that returns `Event` (`fn handshake(req: Bit, ack: Bit) -> Event`) | §13 rule 7 | `method rise on this receiver` | with L8 |

Works, by probe: commits with and without `after`, transport per statement (a pulse from two
commits), `at(0.0)` at `(0, 1)` and a negative `at` never, `every`, `cross` located within 0.1 ps
of `RC·ln 2`, `changed`, `rise`, `fall`, an event named by a body `let`, `a.rise() || b.rise()` as
a head, a reaction that probes, `E0430`, `E0432`, `E0433`, `E0442`, `E0443`, `E0501` naming the
carrier, wrapping arithmetic, an integer division by zero as a fault naming the operation.

### 4.6 Output, input, stdlib (Parts VI–VII)

| # | Refused | Spec | Size |
| - | ------- | ---- | ---- |
| L57 | `include_str!`, `parse_table`, `parse_hex`, `table_model` (`P0001` when reached; see S3) | §20, §22.6.4, §22.2.15 | M |
| L58 | Stdlib refused with the form it is built on. **Generics (L30):** `Short`, `since`, `idt`, `idtmod`, `zi_*`, `laplace_*`, `noise_table`, `Stable`, `Setup`, `Hold`, `inertial`; `cast` and `same` run today as the checker's own intrinsics (`types::intrinsic`, `cast` is an `Option<B>`, `None` for a tag that names no variant), refused only for a `cast` to a tuple or an array that holds an enum, until `generics` compiles them from `std/values.msl`. **Resolved nets (L21):** `Logic`, `Strength`, `Driven`, `Tribuf`, `Pullup`, `Pulldown`, `Keeper`, `Tran`, `Tgate`, `Bidir`, `TribufDelayed`. **Other:** `AckWithin` (a `u32` param), `MinPulse` (`since`), `limvds` as a function value (L9) | §22 | by its group |
| L59 | the closure of `map`, `and_then` or `then` when it is not a literal; an `expect` message that is not a string literal | H.3 | `P0001` naming the form | S |
| L60 | the stdlib `slew` does not limit an ideal (jump) step: §15.2 rule 4 fires a jump's crossing after the solve, so the output passes the jump in one step. Fix = retake the step when a jump-crossing flips a carrier read by `analog` (owner `analog-operators`, AO-13; found by `analog-structure` AS-09 AC 6, which covers a step with a finite edge) | §15.2 rule 4, §22.2.10 | none: the output follows the jump | M |
| L61 | sweeping or setting a key param after `Session::open`: a param that fixes the structure (a selector, an initial value, a reached param) is folded into the body at elaboration, so a `dc` sweep of it, or a `set` of it between open and the run, needs one elaboration per value. Today `E0803` refuses both naming the param; the before-run request param (`--param`) works (owner `sweeps-montecarlo-optimization`; found by `analog-structure` verification F1) | §23.1 rule 7, §8.1.2 | `E0803` | M |
| L62 | a `switch` branch between two floating nodes with an undriven control panics in faer (`attempt to subtract with overflow`, `faer.rs:47`) instead of a named solver error (owner `spec-closure`; found by the `analog-structure` fix round) | §11 (switch branches), §15 | a switch between two floating nodes whose control no source drives | S |

### 4.7 Backends

- **The interpreter is frozen at the M1 analog subset** (R1, 2026-11-15); it refuses every
  digital or amended form by name.
- **`ir-rust` refuses** `UInt<N>`/`Int<N>` with N outside 1..=64 by name; the ABI's leaves end at
  64 bits.
- **A param whose value is a constant NaN** (`lo: f64 = 0.0 / 0.0`) fails the compiled row with "pin ... unlike its
  plan (a transpiler defect)": the plan folds NaN as `0xfff8…`, the library as `0x7ff8…`. Found by the
  `msl-fixes` fix round. Size S (canonicalize NaN bits at the plan/library boundary; owner `params-and-io`).
- **A drive, an event guard or an `at` argument that calls a function reading `freq()`** is refused loud by the
  IR's ambient-reads check (it read 0 silently before `msl-fixes` F1). §14.4 gives such a context 0; size S
  (owner `analog-operators`).
- **A DC sweep's points deliver no records**, so a `dc` run of a design with a record stream is `P0010`
  before anything compiles; `piperine test` counts it `FAIL <name> (did not run)`.

### 4.8 Diagnostics that are loud and say too little

- **67 of the spec's 187 error codes are raised nowhere in the source** (list in §10). Most belong to
  a refused construct; `E0105` does not.
- `P0001` says "outside the M1 subset" for forms the spec defines.

| ID | Gap | Where | Size |
| -- | --- | ----- | ---- |
| D3 | An error code the spec names that no source raises, for a construct the compiler does implement: the checker stops the model with another code (`E0333` is `P0002`, a type mismatch), `params-and-io`'s PI-13 keeps the code listed | `docs/spec/part_iii_structure.md`, §10 | S |

---

## 5. Spec holes and disagreements

Each has a provisional rule or a note so work is not blocked; the spec sentence is still owed.
G1 to G5 are the buck's (`examples/buck/GAPS.md`; G6, unknown integers per value, closed with S6 by `explicit-unknowns`). The spec's own open list is Appendix G
(`G.1` to `G.68`, about 30 open) and is not repeated here.

- **G4 — `f64 as u10` outside the range.** §1 rule 1 says "truncates or extends, as in Rust".
  Provisional: saturate, NaN → 0 (probe: `5000.7 as u10` is 1023).
- **G5 — shift by N or more bits.** No rule. Provisional: the mathematical result truncated to N
  bits (probe: `1u8 << 9` is 0). A negative amount panics in compiled code (a fault, F-B9).
- **G1 — no fixed-point type; G2 — no ranged integer.** Language proposals of the buck.
- **X1 — §15 describes a solver that is gone.** Its notes say the microsteps of `t` run before
  the solve of `t`, and that the device ABI order (solve, then react) "is adopted: `solver-abi`
  implements it and rewrites §15 when it changes the code". MD-62 changed the code; §15, §15.2
  and the rows of C.2 (`run_digital_at`, `predict_step`, `settle_digital`) were not rewritten.
  Nobody has compared what a reaction at `(t, 0)` reads with what the text says.
- **X2 — Appendix C.2.3 and G.38 say "nothing is executed".** Of the conformance set,
  `transition` runs; `zi_nd`, `inertial` and `Tran` are refused; the §15.1 trace has no test from
  source.
- **X6 — what `check` owes a library.** Is a generic body checked once against its bounds, or per
  instantiation only (§3 rule 9 reads as the latter)? S3 depends on the answer.

---

## 6. Solver, numerics and performance

From the solver internals review (artifact `TEV6Hqo6d3QRi1ssZtJTW5`), the `solver-abi-integration`
verification, and this audit. The hygiene points V1 to V11, V14 and V15 closed in `solver-hygiene`;
what remains is speed (V12, V13) and the periodic linearization (V16).

The solver's default transient result keeps every accepted step, with its re-entry state (N2):
about 58 KB a step, which is 25 GB over the buck's whole start. That is the default by design;
`Reentry::LastStep` is the opt-out. A host run through `piperine-api` does not pay it:
it records only the signals it prints, on the print grid as the steps arrive, and keeps its last
step only (60 001 rows peak at about 11 MiB).

| # | Point | Evidence | Size |
| - | ----- | -------- | ---- |
| V12 | **A mixed design steps at every digital edge.** The buck takes about 45 000 accepted steps per 40 µs (about 675 000 for the 600 µs start), 130 s in a dev build: the analog side lands near every 200 MHz instant. §17.2.2 says a clock nothing analog depends on costs nothing, which suggests the reachability set is coarser than the rule; not measured here. Multi-rate stepping is the user's next item after `long-run` | `long-run`, ROADMAP 3c | L |
| V13 | No surrogate or averaged model for runs longer than a start | ROADMAP 3c | L |
| V16 | The periodic small-signal and periodic noise linearization is not implemented | C.2, G.37 | L |

---

## 7. Host surface (Part VIII, §21)

`piperine-solver` still has noise, transfer function, pole-zero, S-parameters, sensitivity,
adjoint and PSS (`core/circuit.rs:111` to `:172`); the host exposes four analyses.

| # | Missing | Spec | Size |
| - | ------- | ---- | ---- |
| H1 | **No host sequences from the command line**, and from the library only what a Rust host does: `Session::drive` of a top `in` port and `Session::set_at` (a param set at a time) are delivered, and the command line takes neither (decision X8: a bench in MSL is its stimulus), and Python is B4. Left: `force` and `release`, a wait for an event, a read at an instant. The buck's scenarios S2 to S10 run on the Rust host (`buck-scenarios`) | §23.1 rules 1–4, 8, 11 | L |
| H2 | Analyses beyond `op`, `dc`, `ac`, `transient`, `tf`, `pz`, `sens`, `pss` (delivered by `host-ii`): `Noise` (also blocked by L45), S-parameters, Fourier and distortion, more than one sweep axis | §11.1, §16.3 | M each, on the device ABI |
| H3 | Session options: `reltol`, `abstol`, `vntol`, `trtol`, `temp`, `resolution`, `maxrounds`, `maxmicro`, `max_steps` and `nodeset` are run keys (delivered). Left: `ttol` (each `cross` carries its own default), `tnom` (waits for `simparam`, L47) | §23 table | S |
| H4 | Metadata: `piperine info` and `Session::metadata` list params, ports and streams (delivered), and `--print Top.r1.r` reads a param back. Left: the public items and sites; an operating-point variable (`--print Top.g.id` is `E0801`); the flow of a branch that is not a potential source in `transient`, `ac`, `pss` and `sens` (H14) | §21 table, rule 6 | S |
| H5 | A `set` of a param that has no device pin and that nothing derives from (`--param Top.r=..` where `r` is read by nobody): `E0802: .. no PARAM pin r; the instance declares no writable parameters`. A param a child derives from is no longer in this case: a `set` recomputes it through the generated function (`host-ii`, MD-69) | §23.1 rule 6 | S |
| H6 | Continued transients, and an analysis at a transient's last instant: both need a solver mechanism that does not exist (a transient the host can resume) | §16.4 | M |
| H7 | Monte Carlo, optimization, objectives and constraints from `target`s, bounds from `where` | §23.1 rule 5, §23.2 | L |
| H8 | Gradients: gradient params, `∂(record)/∂p`, saltation, `E0807` to `E0809`. The solver's `adjoint` and `sens` are not reached from MSL | §21 rule 7, §23.3 | L |
| H9 | Host error codes: `E0804` (drive of a carrier that is no top `in` port), `E0805` (late subscription) and `E0806` (past time) are delivered. `E0803` (timed `set` of a structural param) is checked but no MSL design reaches it: the elaborator marks no param structural, and generation in a module body is `P0001` | §23.1 | with L-generation |
| H10 | Command line: `add`, `remove`, `tree`, `update`, `--locked`, `--offline`, plugin scripts, a REPL, ngspice raw output, a terminal or PNG plot | — | S each |
| H13 | Stale repository documents: the first 270 lines of `ROADMAP.md` (PHDL, a Python bench, Cranelift), the `STATE.md` handoff below `solver-restructure`, `editors/vscode` (a Verilog-AMS extension) and `piperine-lang-server` (postponed); `SPEC_DEVIATIONS.md`, `CLEANUP_PLAN.md`, `docs/audit/`, `docs/reference/` and `VAMS-LRM-2-4.md` are deleted (2026-11-20) | — | S |
| H14 | A flow term `i(<instance>)` of an instance with no potential branch prints in `op` and `dc` only; `transient`, `ac`, `pss` and `sens` refuse it (`E0801`/`P0010`) because `StepView` carries no device term, so a transient table cannot show it. Needs a `StepView` read of `Assembly::term_value`. Owner: `host-analyses` | §21 | S |
| H15 | `pss` of a design whose `transition` holds an absolute time (`ramp.t0`, a `D2A` driven by a discrete clock) never closes its digital periodicity (`digital state is not periodic at T`), so a D2A-driven design cannot shoot; the `pss_rc` twin drives the source from `now()` instead. Any digital state that grows with time breaks it too (the stdlib `every` counter rises +2 per period). Needs time-like state marked through lang, IR, plan, transpile and solver; a `now()`-computed edge also lacks a breakpoint, so it needs a step cap. Owner: `periodic-small-signal` (user, 2026-11-18) | §16.3 | M |
| H16 | No run key or flag for the transient's minimum step (`dt_min`, ngspice's `delmin`): `piperine run` always runs the default floor, so a design that needs ngspice's `delmin=1n` (the `tran_corner` twin) is run through the library (`Session::circuit` into `transient`). Owner: `host-analyses` | §23 table | S |
| H17 | A `--stream <name>` that is also the name of a signal fails after the run prints: `error: cannot decode a value of type bool: 24 bytes where its layout holds 2` (the record layout of the stream is read with the signal's). Owner: `host-analyses` | §19 | S |
| H18 | A root file whose stem is a keyword (`module.msl`, `analog.msl`, `extern.msl`) is `E0171`: the crate takes the stem as its name, so the file must be renamed; the message says the crate would be named `module_`. Owner: `cli-packages` | §3.5 | S |

---

## 8. Unproven

- **U1. Parity covers 16 small circuits** (`tests/e2e/twins/*.msl`: a divider, a current
  source, three diode circuits, `ac_rc`, `tran_rc`, `tran_dq`, `tran_jump`, `tran_corner`,
  `tran_floor`, `tf_divider`, `pz_lowpass`, `sens_divider`, `pss_rc`, `temp_res`). 43 of the 59
  `.cir` fixtures have no MSL twin: BJT, JFET, MOS 1 to 3, noise, distortion, homotopy. Two twins
  (`diode_iv`, `tran_rc`) sit near the tolerance floor.
- **U2. No oracle for digital or mixed-signal behavior.** The interpreter cannot run them; they
  are tested against the spec's own expected values and `examples/buck/SPEC.md` only.
- **U3. The buck's scenarios S2 to S10 run on the Rust host** (`buck-scenarios`:
  `tests/buck_scenarios.rs` over `tests/e2e/buck/`). Only S1 at 3.3 V and S1b stay unrun: the start is
  proven at one operating point (0.8 V, Vin 12 V, 6.6 Ω, 600 µs by `buck_long`), and each 3.3 V
  scenario run checks its own start only as a precondition.
- **U4. The galleries (Appendices D, E, F: 18 fences) are parsed and resolved only**
  (`std_galleries`); none is type-checked, elaborated or run. Most use a refused form.
- **U6. Rules of Part V with no test that names them**: record order within a microstep (§19
  rule 8), the tick rounding of T2 to T4, `E0502`, the round bound `E0506`, crossings between
  sweep points (§16.1.2), a rejected step leaving no record (§19 rule 7).
- **U7. The `abstol` of a user nature in the convergence test, and compatible natures at one
  node** (§4 rules 5, 13; §17.1) are not exercised from source.
- **U8. AC from MSL is proven on one RC.** Nothing checks an AC run of a design with a digital
  part (§16.1.4) or a DC sweep that crosses a threshold (§16.1.2).
- **U9. The two backends agree on the M1 subset only** (`backends`, `twins`); everything else has
  one implementation and no second opinion.
- **U11. `analysis(Noise)` is proven in the solver only.** The noise analysis's operating point
  carries the `NOISE` flag (`piperine-solver/tests/environment.rs`); no MSL design that reads it
  is run through a noise analysis from source.

---

## 9. Coverage by spec part

A rough share of the normative constructs of each part that run end to end from `.msl`. "Parses"
does not count.

| Part | Runs | Biggest holes |
| ---- | ---- | ------------- |
| I §1–§2 values, `Repr` | 60 % | floats other than `f64`, widths over 64, generic enums; `let mut`, assignment, patterns, structs, tuples, arrays and payload enums run |
| I §3–§3.3 functions, calls, uncertainty and absence | 45 % | generics, `impl`, traits, handles; loops, recursion, closures, function values, arrays, composites and the level reads run |
| I §3.4–§3.6 files, crates, packages | 95 % | the check of unreached items (S3) |
| II §4–§5 natures, discrete carriers | 40 % | `Resolve`, arrays, user discrete natures, initial values by `Default` run (`explicit-unknowns` phase A) |
| II §6 nodes, probes | 85 % | signal-flow |
| II §7 bundles | 0 % | all |
| III §8–§8.2 modules, ports, params, the body | 60 % | generics, generation, params that are not `f64` or `bool` |
| III §8.3 instances in expressions | 70 % | generic results, the stdlib `idt` and `Water` (L40) |
| III §8.4–§8.5 templates, `m`/`dtemp`, extern | 0 % | all |
| IV §10–§11 analog | 70 % | the §18 `Diode` (L40), an ideal-step `slew` (L60) |
| IV §12–§13 digital, events | 60 % | `Event` functions, loops, element commits |
| IV §14 core primitives | 50 % | `ddx`, `delay`, `noise`, `simparam`, `others`, `include_str!` |
| V §15–§18 execution | 60 % | digital-only runs, error naming, the text of §15 itself (X1) |
| VI §19–§21 output, input, metadata | 35 % | traces, files, host metadata |
| VII §22 stdlib | 36 % (about 30 of 75 names; `normal` runs) | everything generic, every resolved net, `idt`, filters, noise, tables |
| VIII §23 host | 15 % | host actions, options, Monte Carlo, optimization, gradients |
| A lexical, B grammar | 100 % parsed | the placement matrix B.2 is enforced for the subset only |
| C reference implementation | defaults in place | the C.2 text is stale (X1, X2); 1 of the 5 conformance items runs |
| D, E, F galleries | 0 % proven | U4 |
| H inherited library | 100 % (75 of 75 method rows) | none: H.5 to H.7 run |

---

## 10. Reference lists

**Error codes of the spec that no source file mentions (34 of 189).** A code counts as mentioned when it is spelled
outside a comment in `crates/*/src` (a literal, or an enum variant's `as_str`); `E0375` is named in a doc
comment only (`pom/builtin.rs`), so it is on the list, and `E0702` is on it since `logic` is refused `P0001` until T45 raises it again. `E0503`, `E0504`, `E0505`, `E0507` and `E0508` are
raised: they are the variants of `RunCode` (`piperine-analog-solver/src/error.rs`). `E0803` to `E0806` are raised
by `session.rs` and `host.rs`; `E0701` by the elaborator.
`E0123 E0124 E0127 E0131 E0132 E0133 E0134 E0203 E0225 E0313 E0322 E0340 E0342
E0360 E0362 E0371 E0372 E0373 E0374 E0375 E0451 E0452 E0453 E0454 E0456 E0614
E0702 E0703 E0704 E0705 E0706 E0807 E0808 E0809`

**Entries of the previous file that changed.**

- "A derived child param is not re-derived … no host can `set` yet": the second half stopped
  being true with `msl-cli` (`--param`, `dc --sweep`). It was S2, closed by `host-ii` (a `set` recomputes the derived params, MD-69).
- "An MSL local named like an MSL function makes the generated Rust fail to compile": the type
  checker stops it first (`types/call.rs:54`, there since `295b5ab8`); closed by `params-and-io` as `E0180` (L20).
- `examples/buck/GAPS.md` G3 (a method on a carrier left unresolved) was already closed by
  `msl-digital`: it is a `P0001` naming the method (L15, since closed).
- Every other entry is kept: B3, B4, §4.7, G1 to G5, the solver points (now V1 to V11), U1 to
  U3.

**Clusters one feature would close.** Sizes are rough; the order is the user's.

| Cluster | Closes | Size |
| ------- | ------ | ---- |
| `set` and params | H5, L28, L29, H4 | M |
| Analog structure (delivered, MD-71) | what is left of L40: the stdlib `idt` and `Water` after `generics`, the §18 `Diode` after `params-and-io` | S |
| Analog operators | L43, L44, L45, L49, H2 (`Noise`) | M to L |
| Composite values | L24, L57, and L4 for generic enums (phases A and B closed L1 to L3, L5, L6, L10, L16 and the part of L9 that is not `generics`; MD-79) | L |
| Generics and generation | L7, L30, L31, L32, S3 for generic items: `Short`, `since`, the filters, the monitors, the Book's ladder | L |
| Resolved nets and bundles | L21, L25: natures that implement `Resolve`, `Tribuf`, `Tran`, `Bidir` | L |
| Unknown semantics | L53, L8 (L18 and L19 closed by `unknown-semantics`; L51 and L52 by `unknown-semantics-events`; S6, S8 and G6 closed by `explicit-unknowns`; its phase B is `Logic<N>`, L58) | L |
| Host II (delivered in part by `host-ii`) | H1, H3, H6, H9, then B4 (Python) | L |
| Speed | V12, V13, V16, PSS on the device ABI | L |
| Foreign models | L12, L33 | L |
| Language server | B3 | M, deferred |
| Documents | X1, X2, H13 | S |

---

## 11. The Book against the binary

One row per block or sentence of the Book that the binary refuses (`refused`), does not meet
(`diverges`), never raises (`unraised`) or that the spec contradicts (`disputed`), under the id of the
gap above that explains it. `Book` is the row's id in `tests/book/inventory.tsv`; `tests/book/inventory_guard.rs`
holds this section equal to the inventory. A faulty model the Book teaches (the worked `SampleHold` of
chapter 14) is refused by design and has no row. `Owner` is the feature that closes the row: `root-e2e`
for a `diverges` row until its Phase 9 fix, `user` for a `disputed` one.

| Book | Gap | Owner | Today |
| ---- | --- | ----- | ----- |
| types#7 | L7 | `generics` | reached from a top, `mean` is `P0001` (a generic function over the length of an array); `smoothed`, over the `const` table `TAPS`, runs |
| types#16 | L7 | `generics` | reached from a top, `reads_high` is `P0001`: the method `switches_at` of an `impl` on the enum `Family` |
| types#17 | L7 | `generics` | reached from a top, `range_of` runs (an enum without `Repr` is a value; its range pattern is accepted) and `mean` is `P0001` (a generic function over an array) |
| generics#1 | L7 | `generics` | the trait `Receiver` and its `impl`s are accepted and `Family::LEVELS` is read; a method (`switches_at`, the default `reads_high`) is `P0001` |
| generics~872b633d | L7 | `generics` | a default body of a trait does not run: the method `reads_high` on `Family` is `P0001` |
| generics#2 | L7 | `generics` | reached from a top, `pick`, `larger` and `overdrive` are `P0001`: a generic function |
| generics#4 | L7 | `generics` | reached from a top, `in_tolerance` is `P0001`: a struct expression of the generic struct `Window` |
| generics#5 | L7 | `generics` | reached from a top, `dot` and `mean_of` are `P0001`: a generic function with a const parameter, over an array type |
| generics#9 | L9 | `generics` | reached from a top, `mid_ramp` is `P0001` (a generic function with an `Fn` bound); `clean`, a call of a function value, runs |
| values#8 | L53 | `unknown-semantics-events` | `handshake`, a `fn` that returns `Event`, is `P0001`: method `rise` on this receiver is not supported yet |
| values~c389077a | L53 | `unknown-semantics-events` | the class of a function that returns `Event` does not run (`handshake`); the pure, `now()` and `emit!` classes do |
| values#22 | L12 | `foreign-models` | an `extern "C" fn` with its `#[derivative]` is `P0001`: `extern` function is not supported yet |
| values~3998361d | L12 | `foreign-models` | `E0123` is not raised: an `extern` function called in `analog` is `P0001` with or without the derivative |
| natures#8 | L25 | `resolved-nets-bundles` | placed in a top, the `Stream` bundle is `P0001` (`struct` item used as a type) and `Slice` is `P0001` (generic module) |
| natures~52790298 | L25 | `resolved-nets-bundles` | `flip` and the bundle's directions are `P0001` with natures#8 |
| modules#1 | L30 | `generics` | `ladder.msl` is `P0001`, a generic module, on `Ladder::<3>`, for an operating point and for the DC sweep |
| modules#2 | L31 | `generics` | next to the generic ladder, generation `for` in a `digital` block is `P0001` |
| modules#6 | L30 | `generics` | `lowpass` is `P0001`: its `idt` is a generic module; the module that returns a value with feedback through an instance is behind it (L38) |
| modules#8 | L30 | `generics` | `PiAw` is `P0001`: the `idt` of the integral is a generic module (its `VSignal` ports are typed signal-flow nodes) |
| modules#10 | L32 | `generics` | `Cell` as a type, the arrays of carriers of `MemoryBank` and the generic module are `P0001` |
| modules#12 | L32 | `generics` | `nch(0.13u, 2u)(d, g, s, b)`: placement through a template value or a function is `P0001` |
| modules#14 | L33 | `foreign-models` | `extern "osdi" module Bsim4` is `P0001`: `extern` module is not supported yet |
| modules~b53307eb | L30 | `generics` | the const generic and its `where N >= 1` are refused with modules#1 |
| modules~87ce6f6e | L31 | `generics` | the `half` block over the ladder is refused with modules#2 |
| modules~3f8e84ba | L31 | `generics` | the `half` stream is refused with modules#2 |
| modules~8b927a42 | L30 | `generics` | the value inputs of `lowpass` are refused with modules#6 |
| modules~1faf8c12 | L29 | `params-and-io` | `m` and `dtemp` in a param block are `E0320`, no param `m`: the Book's four resistors in parallel do not run until `params-and-io` implements them |
| modules~4bd9131d | L31 | `generics` | `E0340` is not raised: a `for` in a body is `P0001`, generation is refused |
| modules~4bcb9f1d | L30 | `generics` | `E0304` is not raised: `Ladder::<0>` is `P0001`, the generic module is refused |
| analog#7 | L49 | `analog-operators` | `emit!("newton", ..)` in an `analog` block is `P0001` |
| analog#11 | L45 | `analog-operators` | `noise(..)` is `P0001`: core primitive `noise` |
| analog~139c67b5 | L49 | `analog-operators` | a trace from `analog` is refused with analog#7: no record per Newton iteration exists |
| analog~b103eeb0 | L49 | `analog-operators` | no trace line reaches the command: the `emit!` in `analog` is refused with analog#7 |
| digital#7 | L28 | `params-and-io` | `param of type u8 is not supported yet` on `n: u8` |
| digital#11 | L28 | `params-and-io` | `param of type u8 is not supported yet` on `period: u8` |
| digital#14 | L28 | `params-and-io` | `PwmCheck` is `P0001` twice: `limit: u8` and `cycles: u32` |
| digital#21 | L30 | `generics` | `Dff<T: Repr>` placed as `Dff::<u8>` is `P0001`: generic module |
| digital~bc5825b1 | L28 | `params-and-io` | the debouncer is refused at `n: u8`, as the sentence says |
| digital~bdd2508f | L28 | `params-and-io` | the PWM is refused at `period: u8`, as the sentence says: held with digital#11 |
| digital~a0ca3f9d | L28 | `params-and-io` | the check does not run as the Book places it: `limit: u8` and `cycles: u32` are refused with digital#14 |
| digital~d7032a06 | L28 | `params-and-io` | the form is refused as the sentence says (digital#7) |
| digital~d1d69398 | L28 | `params-and-io` | the debouncer and the PWM are refused (digital#7, digital#11); the counter runs |
| packages#12 | L57 | `composite-values` | `parse_table(include_str!(..))` is `P0001`, a generic function, and `&IV` is `P0001`, a reference type; `include_str!` itself is accepted in a `const` |
| core-std#8 | L30 | `generics` | `Reg` placing `Setup(clk, d) { ts: 100p }` is `P0001`: `Setup<T: Repr>` is a generic module |
| core-std#10 | L28 | `params-and-io` | `Watchdog` is `P0001` on `limit: u16`; with the param written as a literal the watchdog runs and ends the run at the fourth edge |
| cheatsheet#2 | L30 | `generics` | `module M<const N: usize>(..)` placed as `M::<2>` is `P0001`: generic module |
| cheatsheet#3 | L31 | `generics` | the `for k in 0..N { .. }` of the body items is `P0001`: generation in a module body; the instance, the named expression and the definitions around it are accepted |
| cheatsheet#7 | L49 | `analog-operators` | the `emit!("newton", ..)` in `analog` is `P0001` (L49); the `on settle` reactions run |
| cheatsheet#8 | L32 | `generics` | `cell: C;` and the placements through a template or a function need a generic module: `P0001` |
| cheatsheet#9 | L33 | `foreign-models` | `extern "osdi" module Bsim4` with its `#[link]` is `P0001`: `extern` module |
| cheatsheet#10 | L57 | `composite-values` | with its `emit!` the card is `E0601` (a statement among items); without it `parse_table(include_str!(..))` is `P0001`, a generic function, and `table_model(.., IV, ..)` an array whose length is a generic parameter |
| values#18 | L58 | `explicit-unknowns` | `Logic<4>` is `P0001`: the library struct `Logic` has no generic parameter yet (made to run by explicit-unknowns, phase B: T48 (`Logic<N>` from `std`)) |
| values~c30bad4d | L58 | `explicit-unknowns` | held with values#18: `Logic<4>` is `P0001` (made to run by explicit-unknowns, phase B: T48 (`Logic<N>` from `std`)) |
| core-std~6e04c51d | L58 | `explicit-unknowns` | `E0702` is not raised: `logic("10z1")` is `P0001`, because `logic` is a generic `std` function (made to run by explicit-unknowns, phase B: T45 (`logic` and `matches_bits` as generic `std` items)) |
| natures#2 | L21 | `resolved-nets` | `[Bit; 8]` is `P0001` (an array of carriers, L24) and `Bit<Logic<4>>` is `P0001` (`Logic` has no generic parameter yet); `Bit<Option<u8>>` is a carried type and runs, and `Sda`, declared further on in the chapter, is `E0161` when the block stands alone (made to run by resolved-nets, which lands after explicit-unknowns phase B (`Logic<4>`): `Sda` is the chapter's own `impl Resolve<Driven>` nature, which needs `Resolve` and `Driven` (T27 to T32)) |
| natures#3 | L21 | `resolved-nets` | `alarm.msl` is `E0161`: `Driven`, `Resolve`, `Pullup` and the `Logic` behind them are not in the library yet, so a nature that implements `Resolve<Driven>` is not read (made to run by resolved-nets, B3 and later, T27 to T32 (`Resolve`, `Driven`, `Pullup`, `Tribuf`, `Keeper`)) |
| natures~e99c1920 | L21 | `resolved-nets` | the command refuses `alarm.msl` and runs nothing, as the sentence says: held with natures#3 (made to run by resolved-nets, B3 and later, T27 to T32 (`Resolve`, `Driven`, `Pullup`, `Tribuf`, `Keeper`)) |
| digital#16 | L21 | `resolved-nets` | `IrqLine`, a nature the model declares with `impl Resolve<Driven>`: `Driven` is `E0161` (not in the library yet) (made to run by resolved-nets, B3 and later, T27 to T32 (`Resolve`, `Driven`, `Pullup`, `Tribuf`, `Keeper`)) |
| digital#17 | L21 | `resolved-nets` | `AndWire` and `impl Resolve<bool>`: `Resolve` is `E0161` (not in the library yet) (made to run by resolved-nets, B3 and later, T27 to T32 (`Resolve`, `Driven`, `Pullup`, `Tribuf`, `Keeper`)) |
| digital#18 | L21 | `resolved-nets` | `Sda` and its pull-up: `Driven` is `E0161` (not in the library yet) (made to run by resolved-nets, B3 and later, T27 to T32 (`Resolve`, `Driven`, `Pullup`, `Tribuf`, `Keeper`)) |
| digital#19 | L21 | `resolved-nets` | `TriBus` with `Tribuf` and `Keeper`: `Driven` is `E0161` (not in the library yet) (made to run by resolved-nets, B3 and later, T27 to T32 (`Resolve`, `Driven`, `Pullup`, `Tribuf`, `Keeper`)) |
| digital#20 | L21 | `resolved-nets` | `DataBus` and `impl Resolve<Option<u8>, Data>`: `Resolve` is `E0161` (not in the library yet) (made to run by resolved-nets, B3 and later, T27 to T32 (`Resolve`, `Driven`, `Pullup`, `Tribuf`, `Keeper`)) |
| natures~91c3bffe | L21 | `resolved-nets` | the `Pulldown` variant of the alarm needs the same nature: refused with natures#3 (made to run by resolved-nets, B3 and later, T27 to T32 (`Resolve`, `Driven`, `Pullup`, `Tribuf`, `Keeper`)) |
| core-std#6 | L21 | `resolved-nets` | `IrqLine`, a nature the model declares with `impl Resolve<Driven>`: `Driven` is `E0161` (not in the library yet) (made to run by resolved-nets, B3 and later, T27 to T32 (`Resolve`, `Driven`, `Pullup`, `Tribuf`, `Keeper`)) |

Three observations the root tests made are no Book row, so they are not above: they are entries of §7,
each with its owner. A run has no key or flag for `dt_min` (H16, `host-analyses`); a `--stream` named like
a signal fails decoding (H17, `host-analyses`); a root file stem that is a keyword is `E0171` (H18,
`cli-packages`).
