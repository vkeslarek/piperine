# Params and I/O Specification

**Approved**: 2026-11-18 - user: "Sobre as open questions, vou de aceito todas ok?" (every open question takes its recommended option; exceptions noted where the user decided otherwise)

Opened 2026-11-17 from `GAPS.md` (audit of 2026-11-17) at the user's request: "gostaria que
abrisse feature e escrevesse a spec para todos esses gaps [...] Preciso que TODOS os gaps estejam
endereçados ok? Principalmente esses que tu listou que limitam a expressividade da linguagem ok?"
Spec only: no design, no tasks yet.

This feature gathers what a model is parameterized by and what it reads from outside: params of
every value type (L28), the top's continuous ports (L39), input files and tables (L57), the codes
of type errors (X5, L20), the formatting of the shipped sources (H12), and every error code of the
spec that no source raises and no other feature claims (`GAPS.md` §10).

## Questions answered (user, 2026-11-18: every recommendation accepted - "Sobre as open questions, vou de aceito todas ok?")

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | L39: what does the host API do with a continuous port of the top? The user decided X8 (2026-11-17, `host-ii`): "Só pela API host" - a top's ports are driven only through `piperine-api`, never a manifest key; `host-ii` drives discrete `in` ports and leaves continuous ones. | (a) **A host source per node port**: `piperine-api` attaches to each node port of the top a potential source to `gnd` with a DC value, an optional AC magnitude and phase, and an optional piecewise-linear waveform; a node port without one is refused at session start (`E0810`); a value input of the top is refused (`E0811`). (b) **Refusal only**: §8.1.14 and §23.1 rule 1 are amended to say a top's ports are discrete; a continuous port is `E0810` and the user wraps the design in an MSL bench module. (c) **(a), and a value input bound by the host** as a constant per analysis, changed by a timed `set` | **(a).** It is what §23.1 rule 1 implies for a node ("the host connects the top's ports", §8.1.14) and what the Book's analog examples need to be driven without a wrapper; the source is the same mechanism a host `force` on a node uses (§23.1 Resolution). A value input is an expression of a parent, and a top has none, so refusing it is the smaller rule |
| Q2 | X5: which code does a plain type mismatch carry? Today it is `P0002`, an implementation code the spec does not list; the spec gives codes to four positions only (`E0417`, `E0432`, `E0337`, `E0210`) and says "as in Rust" for the rest. | (a) **A new spec code `E0106`** in Part I §1's error table: "an expression whose type is not the type its position requires, where no other row names the position"; `P0002` disappears from the source. (b) **Keep `P0002`** and amend Appendix C to say diagnostics outside the spec's tables carry `P` codes. (c) **One code per position** (operand, argument, return, binding, condition, connection, param value): seven new rows | **(a).** One row closes the hole, a user reads a spec code for the most common error, and the four specific codes keep winning where they apply |

---

## Problem Statement

A module's params can only be `f64` or `bool`: `n: u8`, `md: Mode`, `rs: Option<f64>` and
`cell: Dram1T1C` are `P0001: param of type ..` (`types/module.rs:64`), so the Book's debouncer and
PWM do not build (`GAPS.md` U5). A top module with a node or a value input is refused because the
host connects nothing (`elab/elaborator.rs:144`, L39). A design cannot read a data file:
`include_str!`, `parse_table`, `parse_hex` and `table_model` are refused (L57). A type mismatch
carries `P0002`, which the spec does not define (X5), and a param named like the function it
calls is refused with a generic `P0001` (L20). The stdlib, the galleries and `examples/buck` are
not in the formatter's output form (H12). Finally, 73 of the spec's 180 error codes appear in no
source file (`GAPS.md` §10); 17 of them belong to no other feature and belong here.

## Goals

- [ ] A param may have any value type the language carries today: an integer, a unit enum, an `Option`, or a module (a template bound where it is declared or placed); the Book's debouncer and PWM build and run.
- [ ] A host drives every port of a top module, or the run is refused with a spec code naming the port.
- [ ] A design reads a table or a memory image from a file at elaboration, and `table_model` interpolates it inside `analog` with its derivative.
- [ ] Every diagnostic the type checker emits for a mismatch carries a spec code.
- [ ] `piperine fmt --check` passes on the stdlib, the galleries and `examples/buck`, and a guard keeps it so.
- [ ] Every error code of `docs/spec/` either appears in the source or is listed, by a guard test, under the feature that owns it; this feature's 17 codes are raised.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Templates as values outside a param: in a `const`, as a function result (binning), `Fn(P)` bounds, generic modules (L32, L30) | `generics`. This feature delivers the module-typed param and a template written at its binding (PI-04) |
| Arrays, tuples, `&` as a type (L2, L3, L24) | `composite-values`. L57 depends on its arrays of tuples and of integers |
| Array carriers fed from a file (`signal mem: [Bit<u32>; 1024] = ROM;`, §20's `BootRom`) | `composite-values` (L24); this feature delivers the `const` |
| A derived child param re-derived on a `set` (S2, X9), metadata operations (H4), host drives of discrete ports (H1 part, H9) | `host-ii` |
| `force`, `release` and waiting on an event (H1 rest) | `host-iii` |
| A digital-only transient (L56), which the Book's debouncer and PWM need to run | `host-ii`; PI-01 AC 6 depends on it |
| Checking unreached items, so that an unreached `include_str!` is checked (S3, X6) | `generics` |
| The 57 absent codes that belong to a construct another feature owns | Named per code in "Gaps closed"; PI-13's guard keeps each one listed until its feature raises it |
| Enum variant names on the ABI and in observations (`GAPS.md` §4.7) | No feature claims it in this round; PI-02 needs the names for params only, from the elaborated design |
| `f16`, `f32`, `f128` and integers over 64 bits as param types (L13) | `composite-values` |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Param types delivered | `bool`, `f64`, `UInt<N>`/`Int<N>` for N in 1..=64, unit enums, `Option<T>` of any of these, and a concrete module type | What the checker carries today (`GAPS.md` §1); wider types arrive with their own feature | n - planner |
| How a host names an enum param's value | By variant name: `--param Top.md=Slow`, `Value::Enum("Slow")`; a name the enum lacks is `E0802` listing the variants | Readable, stable under reordering | n - planner |
| How a host sets an `Option` param | A value gives `Some(value)`; the text `None` (CLI) or `Value::None` (library) gives `None` | §3.2.3 wraps; a host must be able to undo a binding | n - planner |
| The boundary with `generics` on module-typed params | Here: a param of a concrete module type, its default or binding written as a template `Name { params }`, and placement through it `cell(connections)` with `E0361` on a param block there. `generics`: templates in `const`s and function results, generic `C: Fn(P)`, `E0360`, `E0362` | L28 names `cell: C`; §8.4.4 is the rule of such a placement. `GAPS.md` sizes L32 "after L28" | n - planner |
| A module-typed param is structural | Yes (§8.4.6): a host `set` during a run is `E0803` (raised by `host-ii`); a `Dc` sweep over it re-elaborates | The spec | y - spec |
| L39's source kind | Potential only (to `gnd`); no host current source | §23.1's Resolution: a host action on a node is a potential source to `gnd`; a current source is an MSL module the user can place | n - planner |
| L39 on the command line | `piperine run` and `piperine test` of a top with a node port fail with `E0810` naming the port, as no manifest key drives a port (X8) | X8 | y - user (X8) |
| `table_model`'s spline end conditions (G.64) | Cubic `3`: not-a-knot, so a cubic sampled at four or more points is reproduced; quadratic `2`: the spline whose first segment is the parabola through the first three points; both written in Appendix C.2 | §22.2.15 rule 8 wants them documented; not-a-knot makes AC 3 of PI-10 exact | n - planner |
| `table_model`'s derivative at a sample where the linear interpolant has a kink | The slope of the segment to the right; at the last sample, of the segment to the left | One-sided and deterministic; the Jacobian needs one value | n - planner |
| The guard of codes | A test target `spec_codes` in `crates/piperine-lang/tests/` reads every error table of `docs/spec/` and every non-test `.rs` file under `crates/` | Mechanical closure: a code is either in the source or listed with its owner | n - planner |
| Formatting the galleries | The fences of Appendices D, E and F are rewritten in the formatter's output in `docs/spec/`; `spec_text.rs` and `std_galleries` must stay green (MD-57) | H12 names the galleries | n - planner |

**Open questions:** none - answered 2026-11-18, every recommendation accepted (see the top of this file).

**Implicit-requirement dimensions.** Input validation: PI-01 AC 4, PI-02 AC 4, PI-08, PI-09,
PI-10 (every malformed file, value or control string is refused with its code). Failure states:
PI-08 AC 2 and 3, PI-10 AC 5 (a panic under known control). External-dependency failure: PI-08
(a file absent or unreadable). State-transition integrity: PI-07 AC 4 (a host change of a node
source at a time is a jump). Observability: every refusal names its file, row, port or param.
Idempotency, auth, rate limits, concurrency, data lifecycle: N/A because elaboration reads files
once and a run is a function of its inputs (§20 rule 3, §15 I1).

---

## User Stories

### P1: A param of any carried type ⭐ MVP

**User Story**: As a model author, I want params of integer, enum, `Option` and module type, so
that a counter's length, a mode, an optional series resistance and a swappable cell are params a
host can list and set.

**Why P1**: L28. It blocks the Book's debouncer and PWM and every digital block with a width or a
count; the user named "the gaps that limit the expressiveness of the language" first.

**PI-01. Integer params.**

**Acceptance Criteria**:
1. WHEN a module declares `n: u8 = 4 where n >= 1;` and a reaction reads `n` THEN the system SHALL elaborate and run it with `n` equal to 4 of type `u8`.
2. WHEN a placement binds `{ n: 7 }` THEN the instance SHALL read 7.
3. WHEN `analog` reads `n as f64` THEN the contribution SHALL use 7.0 for that instance.
4. IF a host sets an integer param to text that is not an integer literal of the param's type, or to a value outside its range THEN the system SHALL refuse the set with `E0802` naming the param and the value.
5. WHEN a host sets `--param Top.db.n=6` THEN the run SHALL read 6 from its start.
6. WHEN the Book's `Debounce`, `Pwm` and `PwmCheck` examples run (after `host-ii` lifts L56) THEN they SHALL build and print the records the Book's listings show.

**PI-02. Enum params.**

**Acceptance Criteria**:
1. WHEN a module declares `md: Mode = Mode::Fast;` of a unit enum and a reaction or `analog` matches on `md` THEN the system SHALL run the arm of the bound variant.
2. WHEN a placement binds `{ md: Mode::Slow }` THEN the instance SHALL read `Mode::Slow`.
3. WHEN a host sets `--param Top.md=Slow` THEN the run SHALL read `Mode::Slow`.
4. IF a host sets an enum param to a name the enum does not declare THEN the system SHALL refuse with `E0802` naming the param and listing the variants.

**PI-03. `Option` params.**

**Acceptance Criteria**:
1. WHEN a placement leaves out `rs: Option<f64>;` THEN the instance SHALL read `None`.
2. WHEN a placement binds `{ rs: 10.0 }` THEN the instance SHALL read `Some(10.0)` (§3.2.3), and `{ rs: None }` SHALL read `None` (§3.2.10).
3. WHEN a model reads an `Option` param through `unwrap_or`, `is_some`, `match` or `if let Some(x)` THEN the system SHALL give the value of the bound case.
4. WHEN a host sets an `Option<T>` param to a value of `T` THEN the run SHALL read `Some` of it, and to `None` it SHALL read `None`.

**PI-04. Module-typed params.**

**Acceptance Criteria**:
1. WHEN a module declares `cell: Dram1T1C = Dram1T1C { cs: 30f };` and its body places `cell(c)` THEN the system SHALL place a `Dram1T1C` with `cs` 30 fF, identical to `Dram1T1C(c) { cs: 30f }` (§8.4.9).
2. WHEN a parent binds `{ cell: Dram1T1C { cs: 10f } }` THEN the child SHALL place a `Dram1T1C` with `cs` 10 fF.
3. IF a placement whose head is a module-typed param carries a param block THEN the system SHALL report `E0361`.
4. IF the template bound to a module-typed param leaves a required param unbound THEN the system SHALL report `E0319` naming it (§8.4.2).
5. IF a template of another module is bound to a param of type `Dram1T1C` THEN the system SHALL report `E0106` (Q2 (a)) naming both modules.
6. The POM SHALL keep the placement through the param as written, and the IR SHALL hold the concrete module (the navigability rule).

**PI-05. The interpreter.**

**Acceptance Criteria**:
1. IF a design run on the interpreter backend has a param of a type other than `f64` or `bool` THEN the system SHALL refuse it naming the param and the transpiled backend.

**Independent Test**: `piperine-lang` elaboration tests per type; a `piperine-cli` test that runs
a small counter with `--param` over each type; the Book's listings (`crates/piperine-cli/tests/book.rs`).

---

### P1: A design reads data from files ⭐ MVP

**User Story**: As an analog designer, I want to read a measured I-V table and a ROM image from
files into constants and interpolate the table inside `analog`, so that measured data drives a
model without being pasted into source.

**Why P1**: L57. `table_model` is the only data-driven device form of the spec.

**PI-08. `include_str!`.**

**Acceptance Criteria**:
1. WHEN `const T: &str = include_str!("data.txt");` names a UTF-8 file THEN the system SHALL give `T` the file's text, read once at elaboration, with the path resolved against the directory of the source file (§20 rules 2, 3).
2. IF `include_str!` appears outside the initializer of a `const` item THEN the system SHALL report `E0610`.
3. IF the file cannot be read THEN the system SHALL report `E0612` naming the resolved path.
4. IF the file is not valid UTF-8 THEN the system SHALL report `E0613` naming the resolved path and the byte offset of the first invalid sequence.
5. WHEN the file changes after elaboration THEN a run of the elaborated design SHALL be unchanged until the host re-elaborates (§20 rule 3).

**PI-09. `parse_table` and `parse_hex`.**

**Acceptance Criteria**:
1. WHEN `const IV: [(f64, f64); 64] = parse_table(include_str!("diode_iv.tbl"));` reads a file of 64 rows of two whitespace-separated real literals (SI suffixes allowed, blank lines and `#` comments skipped) THEN the system SHALL give `IV` those 64 pairs in order.
2. WHEN `const ROM: [u32; 1024] = parse_hex(include_str!("boot.hex"));` reads a `$readmemh` text (`_` inside a word, `//` and `/* */` comments, `@address` records) THEN the system SHALL give `ROM` the words at their addresses, a digit `x` or `z` making its four levels unknown.
3. IF the text has more or fewer rows than the `const`'s length, a row with another number of columns, or an address the text does not write THEN the system SHALL report `E0614` naming the file and the row.
4. IF the text is not in the format at all (a token that is not a real literal, a word that is not hexadecimal) THEN the system SHALL report `E0706` naming the file, the row and the token.

**PI-10. `table_model`.**

**Acceptance Criteria**:
1. WHEN `i(a, k) +> table_model(v(a, k), &IV, "1C");` runs `op` and a DC sweep THEN the flow SHALL equal the linear interpolation of `IV` at each point to 1e-15 relative.
2. WHEN the control is `"D"` THEN the result SHALL be the value of the closest sample.
3. WHEN the control is `"3"` and the table samples a cubic polynomial at 4 or more points THEN the result SHALL reproduce the polynomial at any `x` inside the table to 1e-12 relative.
4. WHEN extrapolation is `C` THEN outside the table the result SHALL be the end value; WHEN it is `L` THEN the end slope's line; two characters SHALL apply to the low end then the high end, and none SHALL be `L` at both.
5. IF the control is `E` at an end and `x` is outside the table at that end under known control THEN the system SHALL fail the analysis with `E0705` naming the instance and the call.
6. IF the control string is not of the form of §22.2.15 THEN the system SHALL report `E0703` naming it.
7. IF the abscissae are not strictly increasing, or the table has fewer points than the degree needs (2 for `1` and `D`, 3 for `2`, 4 for `3`) THEN the system SHALL report `E0704` naming the first offending row.
8. WHEN a device with `table_model` is checked by the ABI's partials verifier THEN the verifier SHALL pass, the partial being the derivative of the interpolant (§22.2.15 rule 5).
9. WHEN this feature merges THEN Appendix C.2 SHALL document the end conditions of the splines `2` and `3` (G.64), with an assertion in `spec_text.rs`.

**Independent Test**: A fixture package with `diode_iv.tbl` and `boot.hex`; elaboration tests per
code; a transpiled DC sweep of the table diode against the interpolation computed in the test.

---

### P1: The host drives every port of the top ⭐ MVP

**User Story**: As a host author, I want to drive a top module's nodes from `piperine-api`, so
that I can simulate a circuit as it is, without writing a bench module around it.

**Why P1**: L39. Today such a top cannot run at all.

**PI-07. Continuous ports of the top.**

**Acceptance Criteria**:
1. WHERE Q1 is answered (a) or (c), WHEN a host attaches a source of value 1.5 V to a node port `a` of the top before `op` THEN `v(a)` SHALL be 1.5 V.
2. WHERE Q1 is answered (a) or (c), WHEN the source has a piecewise-linear waveform THEN a transient SHALL land on each of its corners and `v(a)` SHALL equal the waveform there.
3. WHERE Q1 is answered (a) or (c), WHEN the source has an AC magnitude and phase THEN an `ac` analysis SHALL use them as the port's stimulus.
4. WHERE Q1 is answered (a) or (c), WHEN a host changes the source's value at a time during a transient THEN the change SHALL be a jump at that time (§15.2).
5. IF a session starts a run of a top with a node port the host has not driven THEN the system SHALL refuse it with `E0810` naming the port.
6. IF a top module has a value input and Q1 is answered (a) or (b) THEN the system SHALL refuse it with `E0811` naming the input.
7. WHERE Q1 is answered (c), WHEN a host binds a value input of the top THEN the run SHALL read that value as a constant of each analysis, and a timed `set` of it SHALL be a jump.
8. WHEN this feature merges THEN §8.1.14 and §23.1 SHALL state what the host does with a continuous port and carry the rows `E0810` (and `E0811` unless Q1 is (c)), each asserted in `spec_text.rs`.

**Independent Test**: A `piperine-api` test drives an RC's input node with DC, PWL and AC
sources and compares with the closed forms; a CLI test asserts `E0810`.

---

### P1: Type errors carry spec codes

**User Story**: As a model author, I want every type error to carry a code the spec defines, so
that I can look it up.

**Why P1**: X5 and L20; every user meets a mismatch first.

**PI-06. A plain mismatch, and a param that hides a function.**

**Acceptance Criteria**:
1. WHERE Q2 is answered (a), WHEN an expression's type differs from the type its position requires and no other row of the spec names that position THEN the system SHALL report `E0106` naming the expected and found types.
2. WHERE Q2 is answered (a), the system SHALL contain no source file that emits the code `P0002`.
3. IF a call's callee resolves to a param, a `let` or a `const` that is not a function, and a function of that name is in scope behind it THEN the system SHALL report `E0106` ("expected a function, found ..") with a note naming the hidden function and its declaration (L20, §3.5.4).
4. WHEN this feature merges THEN Part I §1 SHALL carry the `E0106` row (or Appendix C the rule of Q2 (b)), and every Book listing that printed `P0002` SHALL print the new code.

**Independent Test**: `crates/piperine-lang/tests/types_*.rs` per position; the Book's listings.

---

### P2: The shipped sources are formatted

**User Story**: As a contributor, I want the stdlib, the galleries and the buck in the
formatter's own form, so that `piperine fmt` on a copy changes nothing.

**Why P2**: H12; no behavior depends on it.

**PI-11. Formatted sources.**

**Acceptance Criteria**:
1. WHEN `piperine fmt --check` runs over each file of `crates/piperine-lang/std/` THEN it SHALL exit 0.
2. WHEN `piperine fmt --check` runs in `examples/buck` THEN it SHALL exit 0.
3. The system SHALL hold each `rust` fence of Appendices D, E and F equal to the formatter's output of that fence, checked by a test.
4. WHEN the sources are rewritten THEN the printed tree of each (`msl::print`) SHALL be unchanged, and `std_galleries`, `std_excerpts`, `spec_text` and every buck test SHALL pass unedited.

---

### P1: Every error code of the spec is raised or owned

**User Story**: As the owner of the spec, I want every error code it defines raised by the source,
or listed with the feature that will raise it, so that no code is forgotten.

**Why P1**: `GAPS.md` §10: 73 codes; the user asked that every gap be addressed.

**PI-12. The 16 codes owned here.**

**Acceptance Criteria**:
1. IF `&` appears other than as a shared borrow in a call argument or as an array, slice or string parameter type (`let r = &x;`, `fn f(x: &f64)`) THEN the system SHALL report `E0102` naming the form.
2. IF a run panics under known control (an integer division or remainder by zero, a shift by a negative amount under `spec-closure` Q3 (a), an `unwrap` of `None`) THEN the system SHALL fail the analysis with `E0105` naming the operation, the instance and the time.
3. IF a solve-reading function is called in a const context, or an emitting function where `emit!` is not allowed THEN the system SHALL report `E0122` naming the function and its class.
4. IF a nature's quantity carries an attribute other than `access`, `units` and `abstol`, an attribute of the wrong type, or an `abstol` that is not positive THEN the system SHALL report `E0204` naming the attribute.
5. IF the two `access` names of one nature are equal THEN the system SHALL report `E0205` naming them.
6. IF an expression is connected to a carrier port that is not an `in` port (`R(2.0 * v(a), gnd)`) THEN the system SHALL report `E0315` naming the port.
7. IF the name of an instance without a result is used as a value THEN the system SHALL report `E0333` naming the instance.
8. IF an attribute is not in Appendix A.1's table (`#[cfg(test)]`, `#[allow(..)]`) or a listed attribute sits on another kind of item (`#[link]` on a `fn`, `#[repr(u2)]` on a `struct`) THEN the system SHALL report `E0904` naming the attribute and the item.
9. The codes `E0610`, `E0612`, `E0613`, `E0614`, `E0703`, `E0704`, `E0705` and `E0706` SHALL be raised as PI-08 to PI-10 state.

**PI-13. The guard of codes.**

**Acceptance Criteria**:
1. The system SHALL have a test that lists every code of every error table under `docs/spec/` and fails for a code that appears in no non-test source file under `crates/` and in no entry of its allow-list.
2. The system SHALL give each allow-list entry the owning feature and the `GAPS.md` id of the construct.
3. IF a code of the allow-list appears in a non-test source file THEN the test SHALL fail naming the stale entry.
4. WHEN this feature merges THEN the allow-list SHALL hold none of the 16 codes of PI-12, nor `E0361`, `E0106`, `E0810` or `E0811`.

**PI-14. Records.**

**Acceptance Criteria**:
1. WHEN this feature merges THEN `GAPS.md` SHALL no longer list L20, L28, L39, L57, X5 and H12, and §10 SHALL list only the codes still in PI-13's allow-list.

---

## Edge Cases

- WHEN an integer param's default is a const expression of another integer type (`n: u8 = K as u8`) THEN the system SHALL evaluate the `as` under Part I §1 rule 1.
- IF a `where` on an `Option` param reads it as a number (`rs > 0.0`) THEN the system SHALL report `E0106` (Q2 (a)), not accept it.
- WHEN a top module's node port is `gnd`-connected inside the design only THEN the host SHALL still drive it under PI-07 or the run SHALL be refused.
- WHEN `include_str!` names the same file from two `const`s THEN the system SHALL read it once.
- IF a `parse_table` row has a value whose nearest `f64` is infinite THEN the system SHALL report `E0706` naming the row.
- WHEN `table_model` is called with a table of one row under `D` THEN the system SHALL report `E0704`.

---

## Gaps closed

| GAPS id | Closed by |
| ------- | --------- |
| L20 a param named like a function it calls | PI-06 AC 3 |
| L28 params of integer, enum, `Option` or module type | PI-01 to PI-05 |
| L39 a top module with a continuous port | PI-07 (Q1); X8 stands as decided |
| L57 `include_str!`, `parse_table`, `parse_hex`, `table_model` | PI-08 to PI-10 |
| X5 a type mismatch has no spec code | PI-06 AC 1, 2 and 4 (Q2) |
| H12 `piperine fmt` over the stdlib, galleries and `examples/buck` | PI-11 |

**The 73 codes of `GAPS.md` §10.** "Raised" means a requirement raises the code where the spec
says. Each code owned elsewhere stays in PI-13's allow-list until its feature raises it.

| Code | Owner | Requirement or construct |
| ---- | ----- | ------------------------ |
| `E0102` | params-and-io | PI-12 AC 1, raised |
| `E0105` | params-and-io | PI-12 AC 2, raised |
| `E0110` | composite-values | L1, L4 (`derive(Repr)`) |
| `E0111` | composite-values | L4 (`#[repr(uN)]`) |
| `E0122` | params-and-io | PI-12 AC 3, raised (foreign-models FM-04 AC 6 for `extern` functions) |
| `E0123` | foreign-models | FM-04 AC 1 |
| `E0124` | foreign-models | FM-04 AC 2, 3 |
| `E0126` | composite-values | L10 (recursion) |
| `E0127` | generics | L7 (generic functions) |
| `E0131` | generics | L8 (`impl` on a nature) |
| `E0132` | generics | L8 |
| `E0133` | generics | L8 (handles) |
| `E0134` | generics | L7 (traits) |
| `E0151` | unknown-semantics | L18, L19 |
| `E0203` | resolved-nets | L21 |
| `E0204` | params-and-io | PI-12 AC 4, raised |
| `E0205` | params-and-io | PI-12 AC 5, raised |
| `E0224` | resolved-nets | L21 (`others()`) |
| `E0242` | resolved-nets | L23 (signal-flow) |
| `E0243` | analog-structure | L26 (`inflow`) |
| `E0244` | analog-structure | L36 (a child's nodes) |
| `E0260` | resolved-nets | L25 (bundles) |
| `E0261` | resolved-nets | L25 |
| `E0262` | resolved-nets | L25 |
| `E0264` | resolved-nets | L25 |
| `E0313` | composite-values | L24 (array carriers) |
| `E0315` | params-and-io | PI-12 AC 6, raised |
| `E0322` | generics | L31 (generation) |
| `E0333` | params-and-io | PI-12 AC 7, raised |
| `E0340` | generics | L31 |
| `E0342` | generics | L31 |
| `E0351` | composite-values | L5 (loops) |
| `E0354` | analog-structure | L35, L38 |
| `E0360` | generics | L32 (`Fn(P)`) |
| `E0361` | params-and-io | PI-04 AC 3, raised (`generics` extends it to other template values) |
| `E0362` | generics | L32 |
| `E0371` | foreign-models | FM-07 AC 1 |
| `E0372` | foreign-models | FM-07 AC 2, 3 |
| `E0373` | foreign-models | FM-07 AC 4, 5 |
| `E0374` | foreign-models | FM-07 AC 6 |
| `E0413` | resolved-nets | L23 (a nature without `flow`) |
| `E0414` | analog-structure | L40 |
| `E0416` | composite-values | L5 |
| `E0418` | analog-structure | L40 |
| `E0451` | analog-operators | L43 (`ddx`) |
| `E0452` | analog-operators | L44 (`delay`) |
| `E0453` | analog-operators | L44 |
| `E0454` | analog-operators | L45 (`noise`) |
| `E0456` | analog-operators | L47 (`simparam`) |
| `E0503` | solver-hygiene (in flight) | §4.8, V15 |
| `E0504` | solver-hygiene (in flight) | §4.8 |
| `E0505` | solver-hygiene (in flight) | §4.8 |
| `E0507` | solver-hygiene (in flight) | §4.8 |
| `E0508` | solver-hygiene (in flight) | V3 |
| `E0610` | params-and-io | PI-08 AC 2, raised |
| `E0612` | params-and-io | PI-08 AC 3, raised |
| `E0613` | params-and-io | PI-08 AC 4, raised |
| `E0614` | params-and-io | PI-09 AC 3, raised |
| `E0701` | unknown-semantics | L18 (`cast`) |
| `E0702` | unknown-semantics | L18 (`logic`, `matches_bits`) |
| `E0703` | params-and-io | PI-10 AC 6, raised |
| `E0704` | params-and-io | PI-10 AC 7, raised |
| `E0705` | params-and-io | PI-10 AC 5, raised |
| `E0706` | params-and-io | PI-09 AC 4, raised |
| `E0803` | host-ii (in flight) | H9 |
| `E0804` | host-ii (in flight) | H9 |
| `E0805` | host-ii (in flight) | H9 |
| `E0806` | host-ii (in flight) | H9 |
| `E0807` | host-iii | H8 (gradients) |
| `E0808` | host-iii | H8 |
| `E0809` | host-iii | H8 |
| `E0904` | params-and-io | PI-12 AC 8, raised |
| `E0906` | unknown-semantics | L18 (`Unknown`) |

Counts: 73 codes; 17 raised here (`E0361` taken from `generics`' L32 because PI-04 meets it first),
6 by `foreign-models`, 50 by the other features. No code is closed by amending the spec: each
condition is reachable once its construct exists. Three codes are added to the spec: `E0106`
(Q2), `E0810` and `E0811` (Q1).

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| PI-01 | P1: a param of any carried type | - | Pending |
| PI-02 | P1: a param of any carried type | - | Pending |
| PI-03 | P1: a param of any carried type | - | Pending |
| PI-04 | P1: a param of any carried type | - | Pending |
| PI-05 | P1: a param of any carried type | - | Pending |
| PI-06 | P1: type errors carry spec codes | - | Pending |
| PI-07 | P1: the host drives every port of the top | - | Pending |
| PI-08 | P1: a design reads data from files | - | Pending |
| PI-09 | P1: a design reads data from files | - | Pending |
| PI-10 | P1: a design reads data from files | - | Pending |
| PI-11 | P2: the shipped sources are formatted | - | Pending |
| PI-12 | P1: every error code is raised or owned | - | Pending |
| PI-13 | P1: every error code is raised or owned | - | Pending |
| PI-14 | P1: every error code is raised or owned | - | Pending |

**Coverage:** 14 total, 0 mapped to tasks (spec only).

---

## Dependencies and size

- **Depends on:** `composite-values` for arrays of tuples and of integers and `&` as a call
  argument (PI-09, PI-10); `host-ii` (in flight) for `E0802` on a `set`, the drive API PI-07
  builds on, and L56 for PI-01 AC 6; `msl-fixes` (in flight) for `E0140`/`E0141` at calls.
  PI-01 to PI-07, PI-11, PI-12 (except AC 9) and PI-13 need none of the not-yet-started features.
- **Touches:** `piperine-lang` (`types/module.rs`, `types/expr.rs`, `types/call.rs`, elaboration
  of params and consts, the const evaluator), `piperine-ir` and `piperine-plan` (param leaves of
  integer, enum and `Option` type), `piperine-transpile` (`table_model` and its derivative),
  `piperine-api` (node sources, param text), `docs/spec/` Part I §1, Part III §8.1.14,
  Part VIII §23.1, Appendix C.2, the galleries.
- **Size:** L. About 30 tasks: params about 10, files and `table_model` about 9, the top's ports
  about 4, codes and the guard about 5, formatting 2. It splits cleanly into two features at
  the P1 boundaries if the user prefers.

## Sources

- `GAPS.md` L20, L28, L39, L57, X5, X8, H12, U5, §4.8, §10.
- `docs/spec/part_i_values.md` §1, §3, §3.2, §3.5.4; `part_iii_structure.md` §8.1, §8.4;
  `part_vi_io.md` §20; `part_vii_stdlib.md` §22.2.15, §22.6.4, its errors;
  `part_viii_host.md` §23.1; `appendix_a_lexical.md` A.1.
- `.specs/features/host-ii/spec.md` (branch `feature/host-ii`): X8 decided, L39 left.
- `.specs/STATE.md` MD-25 (navigability), MD-57 (spec edits and `spec_text`), MD-65.

## Success Criteria

- [ ] The Book's debouncer and PWM build; a param of each type is set from the command line.
- [ ] A measured-table diode runs inside `analog` and passes the partials verifier.
- [ ] A top with a node port runs from `piperine-api`, or is refused with `E0810`.
- [ ] `spec_codes` is green with an allow-list of at most 56 entries (50, plus `foreign-models`' 6 until it merges), each naming its owner.
