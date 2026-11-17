# MSL Fixes Specification

The user chose this feature on 2026-11-17 as lane A's first: "Correções + Catálogo juntas" (`context.md`). It is not approved yet.

## Open questions for the user

Four decisions are the user's. Each has the planner's recommendation; the spec and the tasks are written to the recommended option, and the tasks that change with another answer are named.

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | Does S3 (the type checker checks only what a run's top reaches) belong to this feature? | (a) **Defer** to a feature of its own, with X6. (b) **A narrowed S3 here**: type-check every non-generic item of the project's own crates, `std` exempt, a `P0001` in an unreached item failing `piperine check`. | (a). S3 is M and not separable cleanly. Checking every item changes what `check` accepts across the corpus: `std` alone holds about 47 refused names (`GAPS.md` L58), so the rule needs a `std` exemption the spec does not state, and a library with an unreached refused form would stop checking. Its three elaboration codes (`E0610`, `E0612`, `E0613`) belong to `include_str!` (L57), which is out. Under (b) one phase of about 4 tasks is added and the question of `std` goes into `docs/spec/` first (MD-57). |
| Q2 | How does a `cross` whose argument may be unknown reach the solver (B2, `absdelta`)? §13 rule 5: "An `e` that is unknown in either solution does not fire." A `CROSS` watch is one `F64` with no known flag, and abi.md A4 fails a solve on any non-finite watch. | (a) **Amend abi.md A4**: a `CROSS` watch whose value is NaN is unknown; it does not fail the solve, does not fire and is not located. The generated watch writes NaN when its argument is unknown. One cross-lane touch in `piperine-analog-solver` (the finite check of a `CROSS` watch). (b) **Refuse** a `cross` over a value that may be unknown, by name at elaboration: B2 becomes loud and `absdelta` stays refused. | (a). NaN compares false both ways, so the detector's own `before <= 0.0 && now > 0.0` already gives "does not fire in either solution" with no new comparison; ±infinity still fails the solve. Under (b) T23 and T24 become one elaboration refusal and FIX-05 AC 2 to 4 leave the feature. |
| Q3 | `analysis(Ac)` and `analysis(Noise)` must be true in their analysis's operating point (§11.1.1). Today the AC and noise analyses solve their operating point with the `DC` analysis flag. | (a) **A minimal solver touch**: the operating point of an AC or noise analysis carries that analysis's flag. Cross-lane into `piperine-solver` (`analyses/dc.rs` and its calls in `ac.rs`, `noise.rs`, `transient.rs`). (b) **Leave L48 out**: all four kinds stay `P0001`, since `analysis(Dc)` alone would be wrong inside an AC run. | (a). The change is one argument on the operating-point constructor; no numeric path moves, and no twin reads an analysis flag. Under (b) T35 leaves, T33, T34 and T36 lose their analysis half, and FIX-11 is out of scope. |
| Q4 | `bound_step` under an `if` or a `match` (§14.10.2: "A call under an untaken branch caps nothing"). | (a) **Straight-line only now**; under an `if` or `match` it is `P0001` by name, with the conditional contributions of L40. (b) **Support it here**. | (a). A `CAP` watch must be finite and positive (abi.md, `E0509`), so an untaken branch needs a second "no cap" encoding in the ABI or the plan; that is the same work as L40's conditional analog statements. The Book's `if ctl.high() { bound_step(1n); }` (Appendix listing, not a run) stays refused. |

## Problem Statement

The audit of 2026-11-17 (`GAPS.md`) found a run that ignores `finish()` and `fatal()` and passes `piperine test`, a `where` that reads a later param, two forms that make `rustc` refuse the generated source (a literal current source, `absdelta`), and a dozen spec forms refused with `P0001` or a misleading `P0002`. Appendix H's inherited library is 19 % implemented (14 of 75 method rows): `sin`, `log10` and `saturating_add` are refused, which blocks `db10`, `db20` and two Book examples.

## Goals

- [ ] A `finish` or `fatal` record ends the analysis at its instant; `fatal` fails `piperine run` (exit 1) and `piperine test` (`FAIL` with the message).
- [ ] Every correctness item of the cluster this feature keeps either runs as the spec says or is refused by name for a reason this spec states.
- [ ] Every row of Appendix H.1, H.2, H.3 and H.4 type-checks, elaborates and runs on the Rust backend with Rust's semantics; a method outside those rows is `E0161`.
- [ ] `db10` and `db20` run; the Book's cycle counter runs and its listing is the binary's output.
- [ ] The 7 twins print the bytes they print today, and every parity target passes unedited.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| S3, the check of unreached items | Q1 recommendation (a): M, needs a `std` exemption the spec does not state, and its codes belong to L57 |
| `simparam` (half of L47) | §14.4.6: "This specification names no session parameter"; names are the host's (G.66, X7). Session parameters are lane B (H3). It stays `P0001` |
| Rows of H.5, H.6, H.7 (arrays, ranges, iterators) | They need arrays (L3), a value form of their own |
| `normal` (§22.2) | Needs tuple values (L2) |
| `limexp` | Refused as "binding inside a module" (L58, a module result with a `limit`): the analog-structure cluster |
| The Book's clipper running | It also needs `settle` (L52) and `limexp`; its refusal listing is brought up to date (CAT-05 AC 3) |
| `bound_step` under `if` or `match` | Q4 recommendation (a); with L40 |
| A math method of a value that may be unknown in `analog` (L19) | Analog has no unknowns (§3.3.5); the existing refusal stays. In reactions the new rows carry the flag |
| The interpreter running the new forms | R1: it is frozen at the M1 subset. Each new IR form is refused there by name (FIX-14) |
| A `ttol` or `etol` that reads a carrier, a probe or a local | The tolerance pins of abi.md are discrete `F64` pins; this feature lowers a constant or param expression only and refuses the rest by name |
| X9, a derived param recomputed on a host `set` | Lane B (`host-ii`), user decision of 2026-11-17 (`context.md`) |
| `piperine-api` and `piperine-cli` beyond S1 | Lane B. S1's three touches are named in `design.md` |
| Solver numerics, error naming (V1 to V11, §4.8) | Lane C. Q2 and Q3's touches are named in `design.md` |
| A `finish` with a message, `continue` after a `finish` (G.35) | §22.6.2 gives `finish()` no parameter; G.35 is open |
| The language server (B3) | Deferred by the user |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| X4: what `fatal` and `finish` do to a run | `finish` ends the run successfully; `fatal` ends it failed: `piperine run` exits 1, `piperine test` prints `FAIL` with the message | User, 2026-11-17: "Encerra e falha (Recommended)" (`context.md`) | y - user |
| What `piperine run` prints after `fatal` | the table up to the instant on standard output as for any run, then `fatal at <t>: <msg>` on standard error, exit 1 | X4 decides the exit code; the line is the planner's layout, held by a test | n - planner |
| A `sim` record whose `finish` field is unknown | recorded; the analysis goes on | §19 Resolution: "A `finish` record ends the analysis only when its `finish` field is known true" | y - spec |
| A finish before `stop` and the short-run failure of `msl-cli` CLI-05 AC 11 | a run whose last step is the instant of a `finish` record is not short; one that ends early with no such record still fails | §19 rule 5 says the instant is the last time point | n - planner |
| The grid of a run ended by `finish` | rows `k*h` not past the finish instant | `long-run` LR-05 AC 1 with `stop` read as the run's end | n - planner |
| A method outside a complete catalogue | `E0161` naming the method and the receiver type, for `f64`, the integer types, `Option` and `bool` | Appendix H rule 1; the Book's error table already says `x.cbrt()` is `E0161` | y - spec |
| `signum` and `abs` on an unsigned integer | `E0161`: H.1 lists them for signed types only | Appendix H rule 1 and the H.1 note | y - spec |
| Integer `abs` at the minimum, `pow` past the width | wrap: `(-128i8).abs()` is `-128`; `2u8.pow(9)` is 0 | H.1: "Arithmetic wraps (§1)"; `pow` "wraps" | y - spec |
| `unwrap` of `None`, `expect` of `None`, `rem_euclid` by zero, `clamp` with `lo > hi` | a panic, so a run fault naming it (§1 rule 11) | Appendix H rule 3; the integer division by zero already faults this way | y - spec |
| The message of `expect` | a string literal; any other `&str` is `P0001` by name | A text is kept in the IR as a literal only (MD-64 item 3) | n - planner |
| Closures of `map`, `and_then`, `unwrap_or_else`, `then` | a closure literal written at the call; a function name passed as a value stays refused (L9) | The checker types a closure only where its parameter type is known; here it is the receiver's | n - planner |
| The new forms on the interpreter | refused by name, each naming the form | R1; none is a one-line change there (each needs a pin, a watch or a method row the walk does not have) | n - planner (R1) |
| Q1 | defer S3 | see the top of this file | n - open |
| Q2 | NaN is "unknown" in a `CROSS` watch | see the top of this file | n - open |
| Q3 | the operating point of an AC or noise analysis carries its flag | see the top of this file | n - open |
| Q4 | `bound_step` straight-line only | see the top of this file | n - open |
| L11 is wider than `GAPS.md` says | a user `fn` with an `Option` parameter does not elaborate at all today, even with `None` passed positionally ("an expression the type checker gave no value type", probe 2026-11-17); FIX-06 covers the parameter, not only the call | probe | n - planner |
| L54 and named arguments of core primitives | `cross(e, etol: 1u)` (§3.2's own example) is in FIX-12: a named argument of a core primitive is refused today | §3.2.4, §3.2 examples | n - planner |

**Open questions:** Q1 to Q4 above, each with a recommendation; the rest is resolved or logged.

**Implicit-requirement dimensions.** Input validation and bounds: FIX-03, FIX-06 AC 5 and 6, FIX-09 AC 3, FIX-12 AC 4, CAT-01 AC 6, CAT-02 AC 6 and 7, CAT-03 AC 8. Failure and partial-failure states: FIX-02 AC 2 and 5, CAT-02 AC 5, CAT-03 AC 2 and 3 (faults name what failed). Idempotency and retry: N/A because a run is not retried; a session compiles once (`msl-cli`). Auth and rate limits: N/A because the tool is local. Concurrency and ordering: FIX-13 AC 3 (one reaction runs once per microstep). Data lifecycle: N/A because nothing is stored beyond a run. Observability: FIX-02 AC 2 and 3 (the fatal message is printed). External-dependency failure: N/A because `rustc` and ngspice are handled as today. State-transition integrity: FIX-01 (no step after a finish), FIX-05 AC 3 (an unknown solution never fires).

---

## User Stories

### P1: A run ends where the model says ⭐ MVP

**User Story**: As the author of a bench, I want `finish()` to end my run and `fatal()` to end it as a failure, so that a bench that stops on a condition stops and a regression that calls `fatal` is red.

**Why P1**: It is the only silent defect of the cluster: a failing bench passes today.

**Acceptance Criteria** (FIX-01, the analysis ends):

1. WHEN a reaction at instant `t` writes a `sim` record whose `finish` field is known true THEN the system SHALL end the transient after that instant, its last accepted step at `t` and no record or commit after it: the probe `on at(1n) { finish(); } on at(3n) { k <- 9; emit!("late", x = 1); }` run to 4 ns gives no `late` record and `k` 0 at the end.
2. WHEN a reaction calls `fatal("boom")` THEN the system SHALL end the transient at that instant as AC 1 says.
3. IF the `finish` field of a `sim` record is false or unknown THEN the system SHALL go on to the run's `stop`.
4. WHEN an operating point's instant writes a `sim` record whose `finish` is known true THEN the system SHALL end that analysis at the instant (§19 Resolution).

**Acceptance Criteria** (FIX-02, the host reads the end; cross-lane, `design.md`):

1. WHEN a `transient` run ends on a `finish` record before its `stop` THEN the system SHALL give its table up to the finish instant, with no grid row past it, and SHALL NOT report the short-run failure of CLI-05 AC 11.
2. WHEN a run ends on a `sim` record whose `failed` is known true THEN `piperine run` SHALL print the table, print `fatal at <t>: <msg>` on standard error with the record's time and `msg`, and exit 1.
3. WHEN `piperine test` judges a run that wrote a `sim` record whose `failed` is known true THEN the system SHALL print `FAIL <run>` with the record's `msg`.
4. WHEN `piperine test` judges a run that ended on `finish()` with no failed check THEN the system SHALL print `PASS <run>`.
5. IF a transient ends before its `stop` with no `finish` record at its last instant THEN the system SHALL fail the run as CLI-05 AC 11 says.

**Independent Test**: the probe of AC 1 through `piperine run`, and a two-run project whose `fatal` run is `FAIL` under `piperine test`.

---

### P1: Defects that `rustc` or the checker shows ⭐ MVP

**User Story**: As a model author, I want a literal current source, `absdelta` and a `where` over a later param to behave as the spec says, so that a legal model builds and an illegal one is refused.

**Why P1**: Two are broken builds of legal models; one is a silent acceptance.

**Acceptance Criteria** (FIX-03, S5):

1. IF a param's `where` reads a param declared after it THEN the system SHALL report `E0318` naming the later param, whatever the two values: `c: f64 = 1.0 where c > d; d: f64 = 0.0;` is `E0318`.
2. WHEN a `where` reads its own param and earlier ones only THEN the system SHALL accept it and evaluate it as today.

**Acceptance Criteria** (FIX-04, B1):

1. WHEN a flow contribution reads no param, no probe and no input (`i(gnd, a) +> 1m;`) THEN the system SHALL build and run it on the Rust backend: into 1 kΩ the operating point gives `v(a)` = 1.0 V, as the interpreter gives.
2. WHEN a potential contribution reads nothing (`v(a, gnd) +> 2.0;`) THEN the system SHALL build and run it: `v(a)` = 2.0 V.
3. WHEN a flow contribution reads only a carrier (`i(gnd, a) +> lv.known_or(0.0) * 1m;`, `lv` a `Bit<f64>` committed to 2.0 at 1 ns) THEN the system SHALL build and run it: `v(a)` = 2.0 V after 1 ns into 1 kΩ.
4. The system SHALL print the generated Rust of every body that reads a value exactly as before this feature.

**Acceptance Criteria** (FIX-05, B2):

1. WHEN a `cross` argument reads a carrier that may be unknown THEN the system SHALL build the design on the Rust backend.
2. WHEN `on absdelta(v(a), 0.2) { emit!("hit", t = now()); }` runs on `v(a)` = 1 V/µs·t to 0.9 µs THEN the system SHALL write `hit` records at 0 and at 0.2, 0.4, 0.6 and 0.8 µs, each within the solver's default `ttol` of the exact time.
3. IF a `cross` argument is unknown at an accepted solution THEN the system SHALL NOT fire it at that solution nor at the next one (§13 rule 5).
4. IF a `cross` argument is plus or minus infinity THEN the system SHALL fail the solve naming the watch, as abi.md A4 says today.

**Independent Test**: each probe of `GAPS.md` B1, B2 and S5 run through the Rust backend.

---

### P1: Calls, comparisons and blocks the spec allows ⭐ MVP

**User Story**: As a model author, I want optional and named arguments, ordered enums and several `analog` blocks to work, so that the code the spec shows compiles.

**Why P1**: Each is a legal form refused with a misleading diagnostic or `P0001`.

**Acceptance Criteria** (FIX-06, L11; `fn f(x: f64, y: Option<f64>, z: Option<f64>) -> f64 { x + y.unwrap_or(10.0) + z.unwrap_or(100.0) }`):

1. WHEN trailing optional parameters are left out (`f(1.0)`) THEN the system SHALL pass `None` for each: 111.0.
2. WHEN an optional parameter is passed by name (`f(1.0, z: 3.0)`) THEN the system SHALL bind it and leave the others `None`: 14.0.
3. WHEN a plain value is passed for an `Option<T>` parameter (`f(1.0, 2.0)`) THEN the system SHALL pass `Some` of it: 103.0.
4. WHEN `None` or `Some(v)` is passed positionally (`f(1.0, None, Some(3.0))`) THEN the system SHALL pass it as it is: 14.0.
5. IF a named argument names a parameter that is not optional or that an earlier argument bound THEN the system SHALL report `E0140` naming it.
6. IF a call has fewer positional arguments than required parameters, or more arguments than parameters, THEN the system SHALL report `E0141`, not `P0002`.
7. WHEN an `Option<T>` parameter is followed by a required one THEN the system SHALL take it positionally only (§3.2.8): leaving it out is `E0141`.

**Acceptance Criteria** (FIX-07, L17):

1. WHEN an enum derives `PartialOrd` THEN the system SHALL compare its values with `<`, `<=`, `>` and `>=` in declaration order: for `enum Md { Lo, Hi }`, `Md::Lo < Md::Hi` is true and `Md::Hi <= Md::Lo` is false, in a reaction on the Rust backend.
2. IF an enum compared with `<`, `<=`, `>` or `>=` does not derive `PartialOrd` THEN the system SHALL report `P0002` naming "a type that derives `PartialOrd`".

**Acceptance Criteria** (FIX-08, L41):

1. WHEN a module has two `analog` blocks THEN the system SHALL run them as one in source order: two blocks each with `i(gnd, a) +> 0.5m;` into 1 kΩ give `v(a)` = 1.0 V.
2. WHEN two `analog` blocks of one module each bind a local of the same name THEN the system SHALL keep the two apart (§10 rule 7).

**Independent Test**: the probes of `GAPS.md` L11, L17 and L41.

---

### P1: Core primitives of analog and events ⭐ MVP

**User Story**: As a model author, I want `bound_step`, `freq()`, `analysis(kind)`, the tolerances of `cross`, and an `above` that emits, so that the analog and event primitives of §13 and §14 the cluster names run.

**Why P1**: The cycle counter of the Book needs `bound_step`; the rest are small refusals of spec forms.

**Acceptance Criteria** (FIX-09, L46):

1. WHEN an `analog` block executes `bound_step(dt)` straight-line THEN the system SHALL make every accepted step of a transient after its first at most `dt`: a 1 kHz sine source with `bound_step(0.02 / f)` run to 2 ms has no step longer than 20 µs.
2. WHEN two instances cap the step THEN the system SHALL take the smaller cap.
3. IF `dt` is not finite or not positive at an accepted solution THEN the system SHALL fail the run with `E0509` naming the watch.
4. IF `bound_step` is under an `if` or a `match` THEN the system SHALL report `P0001` naming "a `bound_step` under an `if` or a `match`" (Q4).

**Acceptance Criteria** (FIX-10, L47):

1. WHEN an AC analysis evaluates `freq()` THEN the system SHALL give it the frequency of the sweep point: a 1 mA AC source into `i(a, gnd) +> v(a, gnd) * (1m + freq() * 1n);` gives `|v(a)|` = 1m / (1m + f · 1n) at each `f`, within `ToleranceContract::AC_MAGNITUDE`.
2. WHEN an operating point or a transient step evaluates `freq()` THEN the system SHALL give 0.
3. WHEN a reaction evaluates `freq()` THEN the system SHALL give 0 (§14.4.4).
4. IF `simparam` is called THEN the system SHALL report `P0001` naming it (out of scope).

**Acceptance Criteria** (FIX-11, L48; Q3):

1. WHEN an `op` run evaluates `analysis(Dc)` THEN the system SHALL give true, and false for `Transient`, `Ac` and `Noise`.
2. WHEN a transient evaluates `analysis(Transient)` in its operating point and in its steps THEN the system SHALL give true, and false for the three others.
3. WHEN an AC analysis evaluates `analysis(Ac)` in its operating point and in its sweep THEN the system SHALL give true, and `analysis(Dc)` false.
4. WHEN the solver's noise analysis evaluates `analysis(Noise)` in its operating point THEN the system SHALL give true.
5. WHEN a reaction evaluates `analysis(kind)` THEN the system SHALL give the kind of the analysis whose instant it runs in.

**Acceptance Criteria** (FIX-12, L54):

1. WHEN `cross` is given a `ttol` and an `etol`, positionally or by name (`cross(e, 1p)`, `cross(e, etol: 1u)`, `cross(e, 1p, 1u)`) THEN the system SHALL type-check, build and run it.
2. WHEN `cross` has a `ttol` or an `etol` THEN the system SHALL declare its `CROSS` watch with a tolerance pin holding that value, not `DEFAULT`.
3. WHEN a `cross` with `ttol` locates a crossing of an RC charge THEN the system SHALL fire it within `ttol` of `RC · ln 2`.
4. IF a `ttol` or `etol` reads a carrier, a probe or a local THEN the system SHALL report `P0001` naming "a `cross` tolerance that is not a constant or a param".

**Acceptance Criteria** (FIX-13, L55):

1. WHEN `on above(v(a) - 0.5) { emit!("hit", x = 1); }` runs on `v(a)` = 1 V/µs·t to 1 µs THEN the system SHALL write exactly one `hit` record, within `ttol` of 0.5 µs.
2. WHEN the same reaction runs on `v(a)` = 1 V from the start THEN the system SHALL write exactly one `hit` record, at time 0.
3. WHEN two disjuncts of one head fire in one microstep THEN the system SHALL run the reaction once: `on (init() && v(a) > 0.0) || (init() && v(a) > -1.0) { emit!("hit", x = 1); }` at `v(a)` = 1 V writes one record.

**Acceptance Criteria** (FIX-14, the interpreter):

1. IF a design that reaches the interpreter holds a form this feature adds to the IR THEN the interpreter SHALL refuse it naming the form (`bound_step`, `freq`, `analysis(Dc)`, the method's name), never run it with a default.

**Independent Test**: the probes of `GAPS.md` L46 to L48, L54 and L55 on the Rust backend; the cycle counter of the Book.

---

### P1: The inherited library of Appendix H ⭐ MVP

**User Story**: As a model author, I want the numeric, `Option` and `bool` methods of Appendix H with Rust's behavior, so that `x.sin()`, `n.saturating_add(1)` and `o.map(..)` work as in Rust.

**Why P1**: The user chose the catalogue with the fixes; it unblocks `db10`, `db20` and the Book's cycle counter.

**Acceptance Criteria** (CAT-01, H.2 `f64`):

1. The system SHALL type each of the 32 rows of H.2 with its signature on `f64`: `powi` takes an `i32`; `is_nan`, `is_finite` and `is_infinite` give `bool`; every other row gives `f64`.
2. WHEN a row is called in an `analog` block or a reaction on the Rust backend THEN the system SHALL give the value Rust's `f64` method gives, bit for bit, at the test's points.
3. WHEN a differentiable row (`sin cos tan asin acos atan atan2 sinh cosh tanh powf powi exp ln log10 log2 sqrt hypot`) is read by a contribution THEN its partials SHALL agree with finite differences under `piperine_abi::verify_partials`.
4. WHEN `floor`, `ceil`, `round`, `trunc` or `signum` is read by a contribution THEN its partial with respect to the receiver SHALL be 0; for `fract` and `rem_euclid` it SHALL be 1.
5. WHEN a row is called on a value that may be unknown in a reaction THEN the result SHALL be unknown exactly when an operand is (§3.3).
6. IF a method that H.2 does not list is called on `f64` (`x.cbrt()`) THEN the system SHALL report `E0161` naming the method and `f64`.

**Acceptance Criteria** (CAT-02, H.1 integers):

1. The system SHALL type each of the 13 rows of H.1 with its signature on `UInt<N>` and `Int<N>`: `pow` takes a `u32`; `checked_*` give `Option<Self>`.
2. WHEN `pow` overflows THEN the system SHALL wrap: `2u8.pow(9)` is 0.
3. WHEN `saturating_add`, `saturating_sub` or `saturating_mul` passes a bound THEN the system SHALL clamp at it: `250u8.saturating_add(10)` is 255, `(-100i8).saturating_sub(100)` is -128.
4. WHEN `checked_add`, `checked_sub` or `checked_mul` overflows THEN the system SHALL give `None`, and `Some` of the exact result otherwise: `250u8.checked_add(10).is_none()` is true, `250u8.checked_add(5).unwrap_or(0)` is 255.
5. IF `rem_euclid` is given zero THEN the system SHALL fault the run, naming `rem_euclid`.
6. WHEN `abs` and `signum` are called on a signed integer THEN the system SHALL give Rust's wrapping result: `(-128i8).abs()` is -128, `(-5i8).signum()` is -1, `(-7i8).rem_euclid(3)` is 2.
7. IF `abs` or `signum` is called on an unsigned integer, or a method H.1 does not list on an integer, THEN the system SHALL report `E0161` naming it.

**Acceptance Criteria** (CAT-03, H.3 `bool` and H.4 `Option`):

1. WHEN `unwrap` or `expect` is called on `Some(v)` THEN the system SHALL give `v`.
2. IF `unwrap` is called on `None` THEN the system SHALL fault the run, naming `unwrap`.
3. IF `expect("msg")` is called on `None` THEN the system SHALL fault the run with `msg` in the fault.
4. WHEN `unwrap_or_else(|| d)` is called THEN the system SHALL give the payload of `Some`, and `d` for `None`, evaluating `d` only for `None`.
5. WHEN `map(|x| e)` and `and_then(|x| o)` are called THEN the system SHALL give `Some(e)` and `o` with `x` bound to the payload for `Some`, and `None` for `None`.
6. WHEN `a.or(b)` is called THEN the system SHALL give `a` when it is `Some`, else `b`.
7. WHEN `c.then(|| v)` and `c.then_some(v)` are called on a `bool` THEN the system SHALL give `Some(v)` when `c` is true and `None` otherwise.
8. IF `expect` is given a message that is not a string literal THEN the system SHALL report `P0001` naming it.
9. IF a method that H.3 or H.4 does not list is called on a `bool` or an `Option` THEN the system SHALL report `E0161` naming it.

**Acceptance Criteria** (CAT-04, the stdlib that the catalogue unblocks):

1. WHEN `db10(100.0)` and `db20(-10.0)` are evaluated on the Rust backend THEN the system SHALL give 20.0 and 20.0.
2. The system SHALL no longer refuse any stdlib item whose only refused form was a row of H.1 to H.4 (`db10`, `db20`).

**Independent Test**: a design per receiver type run from source on the Rust backend, its values checked against Rust's own methods.

---

### P2: The Book and the records say what is true

**User Story**: As a reader of the Book and of `GAPS.md`, I want the examples this feature unblocks to run and the gap list to drop what closed.

**Why P2**: The behavior is P1; this keeps the documents held to it. The user's rule: every Book example a fix unblocks runs.

**Acceptance Criteria** (CAT-05, the Book):

1. WHEN the Book's cycle counter (`cycles.msl`, chapter 8) runs THEN its listing SHALL be the binary's output and SHALL show three `pulse` records with `n=1`, `n=2` and `n=3`, each within 1 µs of 15.9 µs plus a whole number of milliseconds, and `CycleCounter.count` 3 in the last row.
2. The system SHALL keep every command-line listing of the Book equal to the binary's output (`crates/piperine-cli/tests/book.rs`), each prose sentence that says a form "is not compiled yet" updated where the form now runs.
3. WHEN the Book's clipper runs THEN its listing SHALL show the binary's first refusal, which no longer names `sin` or `bound_step`.

**Acceptance Criteria** (DOC-01, the records):

1. WHEN this feature merges THEN `GAPS.md` SHALL no longer list S1, S5, B1, B2, L11, L14, L15, L17, L41, L46, L48, L54, L55, the `Option` and `bool` part of L16, nor `db10`/`db20` in L58, and SHALL still list S3, `simparam`, the rows of H.5 to H.7, `normal` and `limexp`.
2. WHEN this feature merges THEN `ROADMAP.md` SHALL record `msl-fixes` as delivered, and `.specs/STATE.md` SHALL hold the project-level decision of `design.md` ("Tech Decisions").

**Acceptance Criteria** (PAR-01, what does not move):

1. WHEN each of the 7 twins runs from the command line THEN the system SHALL print, on each backend, the bytes of `crates/piperine-cli/tests/goldens/`.
2. The system SHALL pass `parity_op`, `parity_dc`, `parity_ac`, `parity_tran`, `parity_corpus`, `parity_grid` and `piperine-solver`'s `parity_baseline` with their sources, `crates/piperine-parity/src/contract.rs` and `crates/piperine-transpile/tests/golden/` unedited.

**Independent Test**: `cargo test -p piperine-cli --test book --test docs --test twins` and the parity targets.

---

## Edge Cases

- IF two reactions of one microstep write `finish` and `fatal` THEN the system SHALL end the analysis once and judge the run failed.
- WHEN a `finish` record is written at a run's `stop` THEN the system SHALL give the same table as a run without it.
- WHEN `f(1.0, z: 3.0, y: 2.0)` names two optional parameters out of order THEN the system SHALL bind both (§3.2.9): 6.0.
- WHEN an enum of one variant is compared with `<` THEN the system SHALL give false.
- WHEN `powi` is given a negative exponent THEN the system SHALL give Rust's result: `2.0.powi(-1)` is 0.5.
- WHEN `x.sqrt()` or `x.ln()` of a negative constant is read by a contribution THEN the system SHALL fail the solve naming the non-finite cell, as abi.md A4 says.
- WHEN a `checked_*` result is read with `is_some` in a reaction and an operand is unknown THEN the result SHALL be unknown.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| FIX-01 | P1: a run ends where the model says | Tasks | In Tasks |
| FIX-02 | P1: a run ends where the model says | Tasks | In Tasks |
| FIX-03 | P1: defects that `rustc` or the checker shows | Tasks | In Tasks |
| FIX-04 | P1: defects that `rustc` or the checker shows | Tasks | In Tasks |
| FIX-05 | P1: defects that `rustc` or the checker shows | Tasks | In Tasks |
| FIX-06 | P1: calls, comparisons and blocks | Tasks | In Tasks |
| FIX-07 | P1: calls, comparisons and blocks | Tasks | In Tasks |
| FIX-08 | P1: calls, comparisons and blocks | Tasks | In Tasks |
| FIX-09 | P1: core primitives | Tasks | In Tasks |
| FIX-10 | P1: core primitives | Tasks | In Tasks |
| FIX-11 | P1: core primitives | Tasks | In Tasks |
| FIX-12 | P1: core primitives | Tasks | In Tasks |
| FIX-13 | P1: core primitives | Tasks | In Tasks |
| FIX-14 | P1: core primitives | Tasks | In Tasks |
| CAT-01 | P1: Appendix H | Tasks | In Tasks |
| CAT-02 | P1: Appendix H | Tasks | In Tasks |
| CAT-03 | P1: Appendix H | Tasks | In Tasks |
| CAT-04 | P1: Appendix H | Tasks | In Tasks |
| CAT-05 | P2: the Book and the records | Tasks | In Tasks |
| DOC-01 | P2: the Book and the records | Tasks | In Tasks |
| PAR-01 | P2: the Book and the records | Tasks | In Tasks |

**Coverage:** 21 total, 21 mapped to tasks, 0 unmapped.

---

## Differences from the source

None - the product matches the source (`docs/spec/`, Appendix H). Where it stops short, the form is refused by name and listed in Out of Scope.

## Sources

- The user, 2026-11-17, through the orchestrator: the scope, the binding rules and X4 (`context.md`).
- `GAPS.md` (audit of 2026-11-17): S1, S3, S5, B1, B2, L11, L14 to L17, L41, L46 to L48, L54, L55, L58, X4, X6; §10 clusters.
- `docs/spec/appendix_h_library.md` H.1 to H.4 and rules 1 to 3: CAT-01 to CAT-03.
- `docs/spec/part_i_values.md` §3.2 (FIX-06), §2 and H.8 (FIX-07); `part_iii_structure.md` §8.1.1 and `E0318` (FIX-03); `part_iv_behavior.md` §10 rule 4 (FIX-08), §11.1.1 (FIX-11), §13 rules 1 and 5 (FIX-05, FIX-13), §14.4 (FIX-10), §14.9 (FIX-12), §14.10 (FIX-09); `part_vi_io.md` §19 rules 5, 15 and Resolutions (FIX-01, FIX-02); `part_vii_stdlib.md` §22.6.2 (`finish`, `fatal`).
- `.specs/features/device-abi/abi.md`: A4, watches (`CROSS`, `CAP`, `ttol`, `etol`), ambient `FREQ`, globals `DC`/`TRAN`/`AC`/`NOISE`.
- Probes of 2026-11-17 on `4293de6b` (`design.md`, "What the code does today").

---

## Success Criteria

- [ ] Each probe of `GAPS.md` for the items kept here gives the spec's result on the Rust backend.
- [ ] `piperine test` is red for a run that calls `fatal`.
- [ ] The Book's cycle counter prints its three pulses.
- [ ] The 7 twins' bytes and every parity target are unchanged; the build has no warning.
