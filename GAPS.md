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

**Counts.** 3 silent, 2 broken, 49 loud (L1 to L59 less the nine `msl-fixes` and the one `host-ii` closed, in six tables) plus
7 backend and diagnostic notes, 9 spec holes or disagreements, 3 solver and performance points,
15 host gaps, 11 unproven.

| Section | Ids |
| ------- | --- |
| §2 Broken | B3, B4 |
| §3 Silent (read this first) | S3, S4, S6 |
| §4 Loud | L1–L59 (L11, L14, L15, L17, L41, L46, L48, L54, L55, L56 closed) |
| §5 Spec holes and disagreements | G1–G6 (the buck's), X1, X2, X5, X6 (X3, X4, X7, X8, X9 decided by `host-ii`) |
| §6 Solver, numerics, performance | V12, V13, V16 |
| §7 Host surface | H1–H15 |
| §8 Unproven | U1–U11 |
| §9 Coverage by spec part | — |
| §10 Reference lists, changed entries, clusters | — |
| §11 The Book against the binary | one row per Book block or claim the binary refuses, does not meet, never raises or the spec disputes |

---

## 1. What runs today

| Path | State |
| ---- | ----- |
| lex, parse, print, format | the whole grammar of Appendix B; every stdlib module, gallery model, spec fence and `examples/buck` (`msl_corpus`, `msl_format`: 277 sources) |
| POM, name and member resolution | complete over the same corpus (`pom_*`); crates, packages, path and git dependencies, the lockfile (`piperine-project`) |
| type check and elaboration | a **subset**: `bool`, `UInt<N>`/`Int<N>` up to 64 bits, `f64`, unit enums, `Option` and its methods (H.3), plain functions, continuous natures with a flow, `Bit<T>` of a scalar or unit enum, modules with `f64`/`bool` params and `where`, definitions with `after`, a straight-line `analog` block, reactions with `if`/`match`/`if let`, `at`, `cross`, `changed`, `prev`. Everything else is `P0001` by name (§4) |
| backends | IR → Rust → `rustc` → device library: everything the checker accepts. The interpreter is frozen at the M1 analog subset (R1) |
| analyses from a host | `op`, `dc` (one sweep), `ac`, `transient`; on both backends for the 7 ngspice twins |
| digital and mixed-signal | on the Rust backend only: `examples/buck` whole, closed loop at 0.8 V (`buck_closed_loop`, `buck_long`) |
| stdlib that runs | `known`, `known_or`, `is_known`, `rise`, `fall`, `high`, `init`, `every`, `vt`, `pnjlim`, `fetlim`, `deltalim`, `above`, `last_crossing`, `transition`, `D2A`, `A2D`, `Never`, `State`, `log`, `require`, `cover`, `target`, the six constants. About 28 of the 75 names of the §22 index |
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

### S4. A child's own signals read as `gnd` in the authored `Child` of the IR — size S

- `elab/children.rs:119` pads each `signal` of a placed module with `Terminal::Ground`;
  `Terminal` (`piperine-ir/src/model.rs:123`) has no `Own`. The flat instance table carries the
  right node, so simulation is correct; anyone walking the IR's child records sees ground.

### S6. Unknown integers are one flag per value, so `&` and `|` lose known bits (G6) — size L (ABI)

- §3.3.2 is per level. Probe: with `x` unknown, `((x & 1) & 2).known_or(9)` gives 9; the spec
  gives 0. `x & 0` and `b && false` are right. Ruled provisional on 2026-11-15 (`ir-rust` F-B4);
  listed here because the result differs from the spec text with no diagnostic. Per-level flags
  change the ABI's one known byte per value.

---

## 4. Loud — refused, though the spec allows it

Every entry is a diagnostic at type check or elaboration unless it says otherwise. The refusals
are tables with no wildcard arm: `types/unsupported.rs`, `elab/unlowered.rs`,
`types/catalogue.rs` (`F64_METHODS`, `INTEGER_METHODS`, `CORE_ROWS`). The diagnostic column
quotes the text after `P0001:` unless another code is shown.

### 4.1 Values (Part I §1–§3, Appendix H)

| # | Refused | Spec | Diagnostic (probe) | Size |
| - | ------- | ---- | ------------------ | ---- |
| L1 | `struct` values, struct expressions, field access | §1, §2 | `struct item used as a type`, `struct expression`, `field access` | L, with L2–L4 |
| L2 | tuples outside `let (a, b) = (x, y);` | §1 | `tuple expression`, `tuple type` | |
| L3 | arrays, indexing, slices, `&` | §1, H.5 | `array type`, `array expression`, `index expression`, `reference type` | |
| L4 | enums with a payload, `#[repr(uN)]`, an explicit discriminant, generic enums, an enum of one variant as a carrier | §2 rules 2, 5 | `enum Tx with a variant with a payload`, `with a #[repr] attribute` | |
| L5 | `let mut`, assignment, `for`, `while`, `loop`, `break`, `return` | §0.4, §11 rule 12, §12.1.11 | `let mut binding`, `assignment`, `for loop`, `while loop` | M |
| L6 | `match` arm guards, range patterns, bindings in or-patterns; `if let` other than `Some(p)` | §0.4 | `a match arm guard`, `range pattern in a match arm` | S |
| L7 | generic functions, `self` parameters, `impl` on a value type, user traits | §3 rule 9, §3.1 | `generic function`, `generic parameter used as a type` | L, with L30 |
| L8 | `impl` on a nature (other than the stdlib's `impl Bit`); a function that takes a carrier or a node and probes it (a handle) | §3.1, §3 rule 7 | `method mv`; `E0450: a probe is allowed in an analog block and a module body only` | M |
| L9 | closures other than the argument of `limit`; a function name passed as a value (`limit(x, limvds)`) | §3 rule 3, §14.7.1 | `closure outside an argument whose parameter has a function type`; `this name as a value` | S |
| L10 | recursion | §3 rule 12 | `a function that calls itself` | S |
| L12 | `extern "C" fn`, `#[derivative]` | §3 rules 2, 10, 11 | `extern function` | M |
| L13 | `f16`, `f32`, `f128`; integers wider than 64 bits | §1 rule 5 | `type f32`; `integer type u128` | M |
| L16 | every row of H.5, H.6, H.7 (arrays, ranges, iterators); the `Option` and `bool` rows of H.3 and H.4 run | H.5–H.7 | `E0161: no method .. on ..` | with L3 |
| L18 | `Unknown` as an initial value; `x.level(k)`; `merge`, `same`, `cast`, `logic`, `matches_bits` | §3.3.1, §3.3.4, §22.1 | `this literal in a carrier's initial value`; `method level`; `built-in Repr::WIDTH` | M |
| L19 | **Unknown control is a fault, not a merge.** An `if`, a `match` or a guard whose condition is unknown stops the run: `fault 3 in Top at tick 0: the condition of an if is unknown`. So `maybe = true` is never written. Refused at elaboration: a math function, a contribution or a `known_or` default of a value that may be unknown | §3.3.3, §1 rule 13, §19 rule 1 | run-time fault | L |
| L20 | a param named like a function it calls (`vt` and `vt()`). The previous file said the generated Rust failed to compile; the type checker now stops it first. As in Rust the local shadows the function, so the refusal is right and only its code is generic | §3.5.4 | `call of this declaration` | S |

### 4.2 Carriers (Part II §4–§7)

| # | Refused | Spec | Diagnostic (probe) | Size |
| - | ------- | ---- | ------------------ | ---- |
| L21 | natures with `resolve`: `Wire`, `Tri`, `SumNet`; `others()`; relaxation of loops | §4 rule 2, §5.2.2, §8.2.3 | `resolved nature Wire` | L |
| L22 | a discrete nature the project declares (`nature Byte { value: u8 }`) | §4 rule 1 | `discrete nature Byte` | S |
| L23 | signal-flow natures (`VSignal`) | §6 rule 2 | `signal-flow nature VSignal` | M |
| L24 | arrays of carriers (`[Bit; 8]`, `[Electrical; 4]`); element and field commits (`bits[3] <- true`) and definitions | §5.1.4, §12.1 | `array type`; `an element or field commit` | L, with L3 |
| L25 | bundles: a `struct` of carriers, `flip`, bundle literals, `impl` on a bundle | §7 | `struct item used as a type`, `flip type` | L |
| L26 | `p.inflow()`; `changed()` on a param | §6 rule 5, §5.3.1 | `method inflow`; `method changed on f64` | M |
| L27 | an initial value that reads a param (`signal s: Bit<f64> = k0;`, `signal a: Electrical = v0;`) | §5.1.5, §6 rule 6 | `a carrier's initial value that reads a param` | S |

Works, by probe: continuous natures with their own access names (`State` runs; `Thermal` and a
project's nature pass the checker), `potential`/`flow`, a flow probe of a potential branch, a
held node (`= 0.7` holds through a transient's operating point and is free in `op`), `prev()`
before and after the first change, `E0150`, `E0210`, `E0220`, `E0221`, `E0222`, `E0241`, `E0250`,
`E0251`, `E0310`.

### 4.3 Structure (Part III §8–§9)

| # | Refused | Spec | Diagnostic (probe) | Size |
| - | ------- | ---- | ------------------ | ---- |
| L28 | params of an integer, enum, `Option` or module type (`n: u8`, `md: Mode`, `rs: Option<f64>`, `cell: C`) | §8.1.15, §3.2.6 | `param of type u8` | M |
| L29 | `m` and `dtemp`: binding them at a placement and reading them | §8.5.1–§8.5.6 | `E0320: no param m in R`; `built-in m` | M |
| L30 | generic modules and const generics: the prelude's own `Short<N>` and every stdlib module built on them | §8 rule 4 | `generic module` | L |
| L31 | generation `for` and `if`, in a body and in a `digital` block | §8.2.7, §12 rule 1 | `generation for/if in a module body` | M |
| L32 | templates (`Name { .. }` as a value), placement through a template or a function, `Fn(P)` bounds | §8.4 | `module template Name { .. } outside a placement` | M, after L28 |
| L33 | `extern` modules: `"osdi"`, `"cosim"`, `#[link]` | §8.5.7–§8.5.16 | `extern module` | L |
| L34 | a value input of a module without a result (`module G(o: Electrical, x: f64)`) | §8.1.13, §8.3.1 | `a value input` | M |
| L35 | a module with a result that holds a node: every stateful analog operator a user or the stdlib writes | §8.3.2, §8.3.16 | `a module with a result that holds a node` | M |
| L36 | `inst.name` on a `pub` item of a child: probing a child's node, reading a child's carrier | §8.2.8 | `field access` | M |
| L37 | a body `let` used as a param value (`let c = 1.0 / (2.0 * PI * r * fc); Capacitor(b, gnd) { c }`) | §8.1.19 | `E0321: reads a name with no value yet` | S |
| L38 | feedback through an instance (`let y = idt(f(y));`) | §8.3.5 | `the feedback of .. through an instance` (`types/result.rs:236`) | with L35 |
| L39 | a top module with a continuous port: the host connects nothing | §8.1.14, §23.1 rule 1 | `top module Top has the continuous port a` | with H1 |

Works, by probe: `where` at elaboration and on a set (`E0317`, `E0802`), `E0312`, `E0314`,
`E0316`, `E0320`, `E0330`, `E0335` to `E0339`, `E0343`, `E0457`, definitions with and without
`after`, an expression at an `in` port, a result module without state, a body `let` shared by
blocks.

### 4.4 Analog (Part IV §11, §14)

| # | Refused | Spec | Diagnostic (probe) | Size |
| - | ------- | ---- | ------------------ | ---- |
| L40 | **A contribution under an `if` or a `match`**, and a `bound_step` there (Q4 of `msl-fixes`: `P0001` naming it): switch branches, `Switch`, `CapIc`, `Bounded`, `Water`, the `Diode` of §18 with `match rs`, the reset of `idt` | §11 rules 2, 3, 18 | `a contribution under an if or in a closure` | M to L |
| L42 | a `ddt` that is not a whole addend (`2.0 * ddt(q)`), a `ddt` with `abstol` | §14.5 | `a ddt that is not a whole addend of a flow contribution or of a potential contribution` | M |
| L43 | `ddx` | §14.2 | `core primitive ddx` | M |
| L44 | `delay` | §14.6 | `core primitive delay` | M |
| L45 | `noise`, and so `white_noise`, `flicker_noise`, `noise_table` | §14.8, §11.1.8 | `core primitive noise` | M |
| L47 | `simparam` (`freq()` runs) | §14.4 | `core primitive simparam` | S |
| L49 | No trace records: `emit!` in `analog` is refused | §19 rule 4, §11 rule 20 | `an emit! in an analog block` | M |
| L50 | a `limit` or an `ac_stim` under an `if`; either reading a carrier | §14.3, §14.7 | `a limit under an if or in a closure` | S |

Works, by probe: `ddt` of a flow and of a potential (capacitor, inductor, coupled inductors),
`limit` with a closure and a seed, `ac_stim` with a phase, `temperature()`, `gmin()`,
`source_scale()`, `now()`, `analysis(Static)`, an `if` *expression* on a probe, `E0415`.

### 4.5 Digital and events (Part IV §12–§13)

| # | Refused | Spec | Diagnostic (probe) | Size |
| - | ------- | ---- | ------------------ | ---- |
| L51 | an event read as a `bool`: `if fref.rise() { .. }` in a reaction, `on d.changed() && en && !en.changed()` | §13 rules 2, 9 | `an event read as a bool` | M |
| L52 | `settle` | §13 rule 17, §15 S1–S7 | `built-in settle` | M |
| L53 | a function that returns `Event` (`fn handshake(req: Bit, ack: Bit) -> Event`) | §13 rule 7 | `method rise on this receiver` | with L8 |

Works, by probe: commits with and without `after`, transport per statement (a pulse from two
commits), `at(0.0)` at `(0, 1)` and a negative `at` never, `every`, `cross` located within 0.1 ps
of `RC·ln 2`, `changed`, `rise`, `fall`, an event named by a body `let`, `a.rise() || b.rise()` as
a head, a reaction that probes, `E0430`, `E0432`, `E0433`, `E0442`, `E0443`, `E0501` naming the
carrier, wrapping arithmetic, an integer division by zero as a fault naming the operation.

### 4.6 Output, input, stdlib (Parts VI–VII)

| # | Refused | Spec | Size |
| - | ------- | ---- | ---- |
| L57 | `include_str!`, `parse_table`, `parse_hex`, `table_model` (`P0001` when reached; see S3) | §20, §22.6.4, §22.2.15 | M, after L3 |
| L58 | Stdlib refused with the form it is built on. **Generics (L30):** `Short`, `since`, `idt`, `idtmod`, `zi_*`, `laplace_*`, `noise_table`, `Stable`, `Setup`, `Hold`, `inertial`, `merge`, `cast`, `same`. **Resolved nets (L21):** `Logic`, `Strength`, `Wire`, `Tri`, `SumNet`, `Tribuf`, `Pullup`, `Pulldown`, `Keeper`, `Tran`, `Tgate`, `Bidir`, `TribufDelayed`. **A node in a result (L35):** `slew`. **Other:** `limexp` (`binding inside a module`), `AckWithin` (a `u32` param), `MinPulse` (`since`), `normal` (tuples, `cos`), `limvds` as a function value (L9) | §22 | by its group |
| L59 | `unwrap` or `expect` of an `Option` whose presence may be unknown; the closure of `map`, `and_then` or `then` when it is not a literal; an `expect` message that is not a string literal | H.3 | `P0001` naming the form | S |

### 4.7 Backends

- **The interpreter is frozen at the M1 analog subset** (R1, 2026-11-15); it refuses every
  digital or amended form by name.
- **`ir-rust` refuses** `UInt<N>`/`Int<N>` with N outside 1..=64 by name; the ABI's leaves end at
  64 bits.
- **Enum variant names are not carried.** A `Bit<Mode>` is its tag in the IR, on the ABI and in
  every observation: a host reads `2`, not `Regulate`. Size M (metadata on the shape).
- **A param whose value is a constant NaN** (`lo: f64 = 0.0 / 0.0`) fails the compiled row with "pin ... unlike its
  plan (a transpiler defect)": the plan folds NaN as `0xfff8…`, the library as `0x7ff8…`. Found by the
  `msl-fixes` fix round. Size S (canonicalize NaN bits at the plan/library boundary; owner `params-and-io`).
- **A drive, an event guard or an `at` argument that calls a function reading `freq()`** is refused loud by the
  IR's ambient-reads check (it read 0 silently before `msl-fixes` F1). §14.4 gives such a context 0; size S
  (owner `analog-operators`).
- **A DC sweep's points deliver no records**, so a `dc` run of a design with a record stream is `P0010`
  before anything compiles; `piperine test` counts it `FAIL <name> (did not run)`.

### 4.8 Diagnostics that are loud and say too little

- **73 of the spec's 180 error codes appear nowhere in the source** (list in §10). Most belong to
  a refused construct; `E0102`, `E0105`, `E0503` to `E0508` and `E0803` to `E0806` do not.
- `P0001` says "outside the M1 subset" for forms the spec defines.

| ID | Gap | Where | Size |
| -- | --- | ----- | ---- |
| D1 | A diagnostic differs from what the Book shows: a part it leaves out (the help line of `E0150`, the secondary label of `E0163`) or another code (`E0900` for an `emit!` in a module body, where the Book says `E0601`) | `docs/book/content.html` chapters 2 and 11, §11 below | S |
| D2 | A sentence of the Book that the code outran: the code does more than the Book says, so the Book is stale and the user decides (a `disputed` row of §11) | `docs/book/content.html`, §11 below | S |

---

## 5. Spec holes and disagreements

Each has a provisional rule or a note so work is not blocked; the spec sentence is still owed.
G1 to G6 are the buck's (`examples/buck/GAPS.md`). The spec's own open list is Appendix G
(`G.1` to `G.68`, about 30 open) and is not repeated here.

- **G4 — `f64 as u10` outside the range.** §1 rule 1 says "truncates or extends, as in Rust".
  Provisional: saturate, NaN → 0 (probe: `5000.7 as u10` is 1023).
- **G5 — shift by N or more bits.** No rule. Provisional: the mathematical result truncated to N
  bits (probe: `1u8 << 9` is 0). A negative amount panics in compiled code (a fault, F-B9).
- **G6 — unknown integers are per value** (S6).
- **G1 — no fixed-point type; G2 — no ranged integer.** Language proposals of the buck.
- **X1 — §15 describes a solver that is gone.** Its notes say the microsteps of `t` run before
  the solve of `t`, and that the device ABI order (solve, then react) "is adopted: `solver-abi`
  implements it and rewrites §15 when it changes the code". MD-62 changed the code; §15, §15.2
  and the rows of C.2 (`run_digital_at`, `predict_step`, `settle_digital`) were not rewritten.
  Nobody has compared what a reaction at `(t, 0)` reads with what the text says.
- **X2 — Appendix C.2.3 and G.38 say "nothing is executed".** Of the conformance set,
  `transition` runs; `zi_nd`, `inertial` and `Tran` are refused; the §15.1 trace has no test from
  source.
- **X5 — a type mismatch has no spec code.** The checker uses `P0002`; `E0417`, `E0432`, `E0337`
  and `E0210` cover four positions and the rest is "as in Rust".
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
| H1 | **No host sequences from the command line**, and from the library only what a Rust host does: `Session::drive` of a top `in` port and `Session::set_at` (a param set at a time) are delivered, and the command line takes neither (decision X8: a bench in MSL is its stimulus), and Python is B4. Left: `force` and `release`, a wait for an event, a read at an instant. The buck's scenarios S1 to S9 have no host yet (`buck-scenarios`) | §23.1 rules 1–4, 8, 11 | L |
| H2 | Analyses beyond `op`, `dc`, `ac`, `transient`, `tf`, `pz`, `sens`, `pss` (delivered by `host-ii`): `Noise` (also blocked by L45), S-parameters, Fourier and distortion, more than one sweep axis | §11.1, §16.3 | M each, on the device ABI |
| H3 | Session options: `reltol`, `abstol`, `vntol`, `trtol`, `temp`, `resolution`, `maxrounds`, `maxmicro`, `max_steps` and `nodeset` are run keys (delivered). Left: `ttol` (each `cross` carries its own default), `tnom` (waits for `simparam`, L47) | §23 table | S |
| H4 | Metadata: `piperine info` and `Session::metadata` list params, ports and streams (delivered), and `--print Top.r1.r` reads a param back. Left: the public items and sites; an operating-point variable (`--print Top.g.id` is `E0801`); the flow of a branch that is not a potential source in `transient`, `ac`, `pss` and `sens` (H14) | §21 table, rule 6 | S |
| H5 | A `set` of a param that has no device pin and that nothing derives from (`--param Top.r=..` where `r` is read by nobody): `E0802: .. no PARAM pin r; the instance declares no writable parameters`. A param a child derives from is no longer in this case: a `set` recomputes it through the generated function (`host-ii`, MD-69) | §23.1 rule 6 | S |
| H6 | Continued transients, and an analysis at a transient's last instant: both need a solver mechanism that does not exist (a transient the host can resume) | §16.4 | M |
| H7 | Monte Carlo, optimization, objectives and constraints from `target`s, bounds from `where` | §23.1 rule 5, §23.2 | L |
| H8 | Gradients: gradient params, `∂(record)/∂p`, saltation, `E0807` to `E0809`. The solver's `adjoint` and `sens` are not reached from MSL | §21 rule 7, §23.3 | L |
| H9 | Host error codes: `E0804` (drive of a carrier that is no top `in` port), `E0805` (late subscription) and `E0806` (past time) are delivered. `E0803` (timed `set` of a structural param) is checked but no MSL design reaches it: the elaborator marks no param structural, and generation in a module body is `P0001` | §23.1 | with L-generation |
| H10 | Command line: `add`, `remove`, `tree`, `update`, `--locked`, `--offline`, plugin scripts, a REPL, ngspice raw output, a terminal or PNG plot | — | S each |
| H11 | The root package's integrated tests: one per Book example and claim, through the api or the CLI (queue item 4), and the single root `tests/e2e/` with ngspice parity (4b) | ROADMAP | M |
| H12 | The stdlib, the galleries and `examples/buck/src` are not reformatted by `piperine fmt` (the formatter is proven on them) | — | S |
| H13 | Stale repository documents: the first 270 lines of `ROADMAP.md` (PHDL, a Python bench, Cranelift), the `STATE.md` handoff below `solver-restructure`, `SPEC_DEVIATIONS.md`, `CLEANUP_PLAN.md`, `docs/audit/`, `docs/reference/`, `VAMS-LRM-2-4.md` | — | S |
| H14 | A flow term `i(<instance>)` of an instance with no potential branch prints in `op` and `dc` only; `transient`, `ac`, `pss` and `sens` refuse it (`E0801`/`P0010`) because `StepView` carries no device term, so a transient table cannot show it. Needs a `StepView` read of `Assembly::term_value`. Owner: `host-analyses` | §21 | S |
| H15 | `pss` of a design whose `transition` holds an absolute time (`ramp.t0`, a `D2A` driven by a discrete clock) never closes its digital periodicity (`digital state is not periodic at T`), so a D2A-driven design cannot shoot; the `pss_rc` twin drives the source from `now()` instead. Any digital state that grows with time breaks it too (the stdlib `every` counter rises +2 per period). Needs time-like state marked through lang, IR, plan, transpile and solver; a `now()`-computed edge also lacks a breakpoint, so it needs a step cap. Owner: `periodic-small-signal` (user, 2026-11-18) | §16.3 | M |

---

## 8. Unproven

- **U1. Parity covers 7 small analog circuits** (`tests/ngspice/*.msl`: a divider, a current
  source, three diode circuits, `ac_rc`, `tran_rc`). 43 of the 50 `.cir` fixtures have no MSL
  twin: BJT, JFET, MOS 1 to 3, noise, distortion, homotopy, temperature. Two twins (`diode_iv`,
  `tran_rc`) sit near the tolerance floor.
- **U2. No oracle for digital or mixed-signal behavior.** The interpreter cannot run them; they
  are tested against the spec's own expected values and `examples/buck/SPEC.md` only.
- **U3. The buck's closed loop is proven at one operating point**: 0.8 V, Vin 12 V, 6.6 Ω,
  600 µs. Scenarios S1 to S9 of its SPEC §10 need H1.
- **U4. The galleries (Appendices D, E, F: 18 fences) are parsed and resolved only**
  (`std_galleries`); none is type-checked, elaborated or run. Most use a refused form.
- **U5. The Book.** Its 97 `msl` blocks are held to the parser and its 29 command-line listings
  to the binary (`piperine-cli/tests/book.rs`). No block is run as a model unless a listing runs
  it. Shown with their refusal: the ladder (generic module), the debouncer and the PWM (a `u8`
  param), the alarm line (`Wire`), the clipper (`settle`, `limexp`), the chapter-8 buck
  (`settle`), `Blinky` and `Count` (digital-only transient).
- **U6. Rules of Part V with no test that names them**: record order within a microstep (§19
  rule 8), the tick rounding of T2 to T4, `E0502`, the round bound `E0506`, crossings between
  sweep points (§16.1.2), a rejected step leaving no record (§19 rule 7).
- **U7. The `abstol` of a user nature in the convergence test, and compatible natures at one
  node** (§4 rules 5, 13; §17.1) are not exercised from source.
- **U8. AC from MSL is proven on one RC.** Nothing checks an AC run of a design with a digital
  part (§16.1.4) or a DC sweep that crosses a threshold (§16.1.2).
- **U9. The two backends agree on the M1 subset only** (`backends`, `twins`); everything else has
  one implementation and no second opinion.
- **U10. Parity tests are scattered** (`tests/ngspice/` + `piperine-parity`); the single root
  `tests/e2e/` does not exist yet (H11).
- **U11. `analysis(Noise)` is proven in the solver only.** The noise analysis's operating point
  carries the `NOISE` flag (`piperine-solver/tests/environment.rs`); no MSL design that reads it
  is run through a noise analysis from source.

---

## 9. Coverage by spec part

A rough share of the normative constructs of each part that run end to end from `.msl`. "Parses"
does not count.

| Part | Runs | Biggest holes |
| ---- | ---- | ------------- |
| I §1–§2 values, `Repr` | 35 % | composites, payload enums, floats other than `f64`, widths over 64 |
| I §3–§3.3 functions, calls, unknown values | 35 % | generics, `impl`, traits, handles, loops, the merge (L19), `level` |
| I §3.4–§3.6 files, crates, packages | 95 % | the check of unreached items (S3) |
| II §4–§5 natures, discrete carriers | 40 % | `resolve`, arrays, user discrete natures, `Unknown` initial values |
| II §6 nodes, probes | 70 % | signal-flow, `inflow`, a child's nodes |
| II §7 bundles | 0 % | all |
| III §8–§8.2 modules, ports, params, the body | 50 % | generics, generation, params that are not `f64` or `bool`, `pub` access, a `let` as a param value |
| III §8.3 instances in expressions | 30 % | value inputs, results with a node, feedback |
| III §8.4–§8.5 templates, `m`/`dtemp`, extern | 0 % | all |
| IV §10–§11 analog | 40 % | conditional contributions, where a `ddt` may sit |
| IV §12–§13 digital, events | 60 % | an event as a `bool`, `settle`, `Event` functions, loops, element commits |
| IV §14 core primitives | 50 % | `ddx`, `delay`, `noise`, `simparam`, `others`, `level`, `inflow`, `include_str!` |
| V §15–§18 execution | 60 % | the merge, `settle`, digital-only runs, error naming, the text of §15 itself (X1) |
| VI §19–§21 output, input, metadata | 35 % | traces, `maybe`, files, host metadata |
| VII §22 stdlib | 35 % (about 28 of 75 names) | everything generic, every resolved net, `idt`, `slew`, filters, noise, tables |
| VIII §23 host | 15 % | host actions, options, Monte Carlo, optimization, gradients |
| A lexical, B grammar | 100 % parsed | the placement matrix B.2 is enforced for the subset only |
| C reference implementation | defaults in place | the C.2 text is stale (X1, X2); 1 of the 5 conformance items runs |
| D, E, F galleries | 0 % proven | U4 |
| H inherited library | 75 % (56 of 75 method rows) | L16 (H.5 to H.7) |

---

## 10. Reference lists

**Error codes of the spec that no source file mentions (73 of 180).**
`E0102 E0105 E0110 E0111 E0122 E0123 E0124 E0126 E0127 E0131 E0132 E0133 E0134 E0151 E0203 E0204
E0205 E0224 E0242 E0243 E0244 E0260 E0261 E0262 E0264 E0313 E0315 E0322 E0333 E0340 E0342 E0351
E0354 E0360 E0361 E0362 E0371 E0372 E0373 E0374 E0413 E0414 E0416 E0418 E0451 E0452 E0453 E0454
E0456 E0503 E0504 E0505 E0507 E0508 E0610 E0612 E0613 E0614 E0701 E0702 E0703 E0704 E0705 E0706
E0803 E0804 E0805 E0806 E0807 E0808 E0809 E0904 E0906`

**Entries of the previous file that changed.**

- "A derived child param is not re-derived … no host can `set` yet": the second half stopped
  being true with `msl-cli` (`--param`, `dc --sweep`). It was S2, closed by `host-ii` (a `set` recomputes the derived params, MD-69).
- "An MSL local named like an MSL function makes the generated Rust fail to compile": the type
  checker stops it first (`types/call.rs:54`, there since `295b5ab8`). Now L20.
- `examples/buck/GAPS.md` G3 (a method on a carrier left unresolved) was already closed by
  `msl-digital`: it is a `P0001` naming the method (L15, since closed).
- Every other entry is kept: S4, B3, B4, L19, L37, §4.7, G1 to G6, the solver points (now V1 to V11), U1 to
  U3, U10.

**Clusters one feature would close.** Sizes are rough; the order is the user's.

| Cluster | Closes | Size |
| ------- | ------ | ---- |
| `set` and params | H5, L27, L28, L29, L37, H4 | M |
| Analog structure | L40, L42, L34, L35, L36, L38, L50: `Switch`, `CapIc`, `idt`, `slew`, the `Diode` of §18 | M to L |
| Analog operators | L43, L44, L45, L49, H2 (`Noise`) | M to L |
| Composite values | L1 to L6, L16 (H.5 to H.7), L24, L57 | L |
| Generics and generation | L7, L30, L31, L32, S3 for generic items: `Short`, `since`, the filters, the monitors, the Book's ladder | L |
| Resolved nets and bundles | L21, L22, L23, L25: `Wire`, `Tribuf`, `Tran`, `Bidir` | L |
| Unknown semantics | L18, L19, S6 (ABI), L51, L52, L53, L8 | L |
| Host II (delivered in part by `host-ii`) | H1, H3, H6, H9, H11, L39, then B4 (Python) | L |
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
chapter 11) is refused by design and has no row. `Owner` is the feature that closes the row: `root-e2e`
for a `diverges` row until its Phase 9 fix, `user` for a `disputed` one.

| Book | Gap | Owner | Today |
| ---- | --- | ----- | ----- |
| errors#1 | D1 | `root-e2e` | `E0150` gives no help line |
| errors#2 | D1 | `root-e2e` | `E0163` gives no secondary label on `mod util;` |
| errors~54e5381d | L40 | `analog-structure` | `E0414` is not raised: a contribution under an `if` is `P0001` on both branches |
| welcome~5b11a01a | D2 | `user` | the math methods such as `sin` run; the other items of the sentence are `P0001` as written |
| from-rust#1 | L6 | `composite-values` | reached from a top, `range_of` is `P0001` (a range pattern in a `match` arm; the enum has no `#[derive(Repr)]`) and `mean` is `P0001` (a reference type) |
| from-rust~64ee8d1d | D1 | `root-e2e` | `emit!("hello");` in a module body is `E0900` (a syntax error), not `E0601` |
| values#2 | L53 | `unknown-semantics-events` | `handshake`, a `fn` that returns `Event`, is `P0001`: method `rise` on this receiver is not supported yet |
| values~c389077a | L53 | `unknown-semantics-events` | the class of a function that returns `Event` does not run (`handshake`); the pure, `now()` and `emit!` classes do |
| values#15 | L12 | `foreign-models` | an `extern "C" fn` with its `#[derivative]` is `P0001`: `extern` function is not supported yet |
| values~3998361d | L12 | `foreign-models` | `E0123` is not raised: an `extern` function called in `analog` is `P0001` with or without the derivative |
| natures#2 | L24 | `composite-values` | `[Bit; 8]` is `P0001` (array type), `signal sda: Wire` is `P0001` (resolved nature `Wire`) and `logic("10xx")` reaches a generic function of the standard library (`P0001`); the other lines are accepted |
| natures#3 | L21 | `resolved-nets` | `alarm.msl` is twelve `P0001`: the `Wire` ports, `changed()` and field access on a `Wire`, the `struct` of `Logic` in `nets.msl` |
| natures#8 | L25 | `resolved-nets-bundles` | placed in a top, the `Stream` bundle is `P0001` (`struct` item used as a type) and `Slice` is `P0001` (generic module) |
| natures~e99c1920 | L21 | `resolved-nets` | the command refuses `alarm.msl` and runs nothing, as the sentence says: held with natures#3 |
| natures~52790298 | L25 | `resolved-nets-bundles` | `flip` and the bundle's directions are `P0001` with natures#8 |
| natures~6a449552 | L21 | `resolved-nets` | the `Pulldown` variant of the alarm needs the same `Wire`: refused with natures#3 |
