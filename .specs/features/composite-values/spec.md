# Composite Values Specification

**Approved**: 2026-11-18 - user: "Sobre as open questions, vou de aceito todas ok?" (every open question takes its recommended option; exceptions noted where the user decided otherwise)

## Questions answered (user, 2026-11-18: every recommendation accepted - "Sobre as open questions, vou de aceito todas ok?")

These block design. Each has options and a recommendation; none is decided.

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | How does a composite value live in the IR, the generated Rust and the device ABI? The IR has one real (`F64`), `bool`, unit and integers of 1 to 64 bits today (`piperine-ir/src/model.rs` `ValueType`) | (a) **IR aggregates.** `ValueType` gains struct, tuple, array and enum-with-payload types. The transpiler prints them as Rust structs, tuples, arrays and enums, near 1:1 (codegen C4). Where a value meets the ABI (a carried value, a param pin, a record field) it is flattened into the `Layout` the ABI already has: nested fields with dotted names, arrays as `count`, leaves `BOOL` to `F64`. No ABI change for leaves of 64 bits or less. (b) **Scalarize at elaboration.** Every composite becomes one IR scalar per leaf (`p.x`, `p.y`); a dynamic index becomes a select over the elements. The IR does not change. A loop over a large array, or a dynamic index into one, grows the IR with the array; the generated Rust no longer reads like the source (R3). (c) **Bytes in the IR.** A composite is a byte buffer typed by its layout, and every operation is a layout read or write. The IR loses its types and the generated Rust becomes byte shuffles | (a). It keeps the generated Rust readable (R3), keeps the IR typed for `Ir::check()`, and costs no ABI change. (b) fails on `[Bit<u8>; 1 << 16]` style data and on iterators; (c) gives up the type checks the IR has |
| Q2 | How does a host show a composite value: a carried composite in a table column, a composite field of a record, a composite param given with `--param`? | (a) **Leaves.** One column or field per leaf, dotted: `st.n`, `bits[3]`, `t.0`; an enum gives a variant column and its payload's leaves. (b) **One cell.** The value in MSL literal syntax (`Tx::Data { n: 5 }`, `[1, 2, 3]`, `(3, true)`), nested objects and arrays in JSON Lines. (c) **Both by format.** (b) in the text table, record lines and JSON Lines; (a) in CSV and the plot, which need scalars. A composite param is set by leaf path, `--param Top.f.num[2]=0.5` | (c). Text and JSON keep the value readable as written; CSV and the plot cannot hold a nested value. Variant names need the enum metadata of CV-22 |
| Q3 | An array of carriers costs what is written (§5.1.8: "An implementation **MUST NOT** require storage proportional to the declared length"; `[Bit<u8>; 1 << 32]` is legal). One net per element is proportional | (a) **Sparse, as the spec says.** An element's net exists from its first commit; the device ABI gains a way to declare a sparse array of discrete nets (an ABI change, minor version). (b) **Dense up to a bound.** Arrays of carriers up to a bound (e.g. 65 536 elements) are one net per element; a longer one is refused by name. A spec departure: §5.1.8 is amended | (a). Memories are the main use of long carrier arrays, and §5.1.8 was written for them. The ABI may change (codegen C9) |
| Q4 | `f16`, `f128` and integers wider than 64 bits. `ir-rust` RS-Q6 refused them by name under the R4 delegation: the ABI's leaves end at 64 bits and stable Rust has no `f16` or `f128`. §1 rule 5 allows `UInt<N>` for any const `N >= 1` | Integers: (a) any width, as `u64` limbs in the runtime `rt`, carried as a `U64` array field (no new leaf) plus metadata that the host reads it as one integer; (b) up to 128 bits through Rust's `u128`/`i128`, refused by name above. `f16`: (a) exact, stored as its bits, every operation computed in `f32` and rounded to `f16` (exact for `+ - * /` and `sqrt`); (b) refused by name. `f128`: (a) a software float in `rt` for every operation and every H.2 method, correctly rounded; (b) a software float for arithmetic, comparisons, `as` conversions, `sqrt`, `abs`, `mul_add`, `min`, `max`, `clamp`, the classification methods; the transcendental H.2 methods on `f128` (`sin`, `exp`, `ln`, `powf` and the rest) stay refused by name; (c) refused by name, and §1 drops `f128` (a spec departure) | Integers (a); `f16` (a); `f128` (b). The transcendental `f128` methods are a large numeric library that generated code may not link (MD-63 item 1: `piperine-abi` and `rt` only). Choose `f128` (a) to leave no residue; it is then its own phase of about 15 tasks |
| Q5 | A loop that never ends, or a recursion that never ends, inside a reaction or a function. Rust has no bound: the loop hangs the run, the recursion overflows the stack and kills the process | (a) **As Rust.** No bound. (b) **A bound, as a panic.** More than 16 777 216 (2^24) iterations of one loop in one evaluation, or a call depth over 1 024, is a panic (§1 rule 11): the analysis fails naming the construct, the instance and the instant. A spec sentence is added to §1 rule 11 | (b). A simulator must not hang or abort on a model's bug. Both bounds are far above any model in the corpus; the numbers are the planner's and the user may change them |

## Problem Statement

MSL's value world is Rust's (§0.4), but the type checker accepts only `bool`, `UInt<N>`/`Int<N>` up to 64 bits, `f64`, unit enums and `Option` (`GAPS.md` §1). Structs, tuples beyond `let (a, b) = (x, y);`, arrays, slices, enums with a payload, `let mut`, loops, `return`, most patterns, closures, function values, recursion, `f32` and the other float widths are refused with `P0001` (`GAPS.md` L1 to L6, L9, L10, L13, L16, L24). A model that keeps a table, a shift register, a state with data, or a loop over coefficients cannot be written, and the stdlib's own filters (`zi_*`, `laplace_*`), `normal` and the monitors are refused because of it (L58). The user asked for every gap closed, the ones that limit the language's expressiveness first.

## Goals

- [ ] Every value form of Part I §1 to §3.2 and Appendix H.5 to H.7 that is refused today type-checks, elaborates and runs on the Rust backend, with the meaning Rust gives it (§0.4).
- [ ] Composite values are carried (`Bit<Point>`, `Bit<Tx>`, `Bit<[u8; 4]>`), passed as params and written in records, with the layouts of §2.
- [ ] Arrays of carriers (`[Bit; 8]`, `[Electrical; 4]`) with element and field commits work, at a cost proportional to what is written (Q3).
- [ ] The value types of §1 rule 5 that are refused today (`f16`, `f32`, `f128`, integers over 64 bits) are accepted, or refused by a name the user chose (Q4).
- [ ] Each of the ids L1 to L6, L9, L10, L13, L24, and L16's rows of H.5 to H.7 and `normal`, leaves `GAPS.md` in the commit that closes it.
- [ ] The 7 ngspice twins print the same bytes as before; no tolerance changes.

## Phases

The feature is four sub-features, each designed and tasked on its own (`design-<phase>.md`, `tasks-<phase>.md` in this folder), in this order. A and D are independent of each other.

| Phase | Name | Ids | Requirements | Size |
| ----- | ---- | --- | ------------ | ---- |
| A | Control and functions | L5, L6, L9, L10 | CV-01 to CV-08 | M, about 22 tasks |
| B | Aggregates | L1, L2, L3, L4 (all but generic enums), L16 (H.5 to H.7, `normal`) | CV-09 to CV-22 | L, about 38 tasks |
| C | Arrays of carriers | L24 | CV-23 to CV-28 | M, about 20 tasks |
| D | Number widths | L13 | CV-29 to CV-33 | M, about 15 tasks (30 with `f128` option (a)) |
| all | Cross-cutting | — | CV-34 to CV-37 | carried by each phase |

Total: L, about 95 tasks (110 with `f128` (a)).

## Dependencies

| On | What it gives this feature | Needed by |
| -- | -------------------------- | --------- |
| `msl-fixes` (in flight) | `cos`, `ln`, `sqrt` of H.2 (L14) and the `Option` methods of H.4 (L16 H.3/H.4 rows) | CV-16 (`normal`), stdlib bodies |
| `generics` phase A | type parameters, `Fn` bounds on functions, `impl` blocks, hand-written `impl Repr` | generic enums and structs (L4, closed there as GN-06), capturing closures passed to a user function (L9, GN-01) |
| `unknown-semantics` | the merge under unknown control (L19) and per-level known flags (S6) | CV-08: assignments, loops and `break` under unknown control; the flags of a composite's levels |
| `params-and-io` | params of integer, enum and `Option` type (L28); the spec codes of §10 for constructs that already run | composite params reuse its param path (CV-18) |
| `analog-structure` | `inst.name` on a child's `pub` item (L36) | element addressing across instances in CV-23 |

`generics` depends on this feature: its const generics need arrays (B), its generated arrays need arrays of carriers (C), and its stdlib items (`zi_*`, `laplace_*`) need loops (A) and arrays (B, C). Recommended order across the two features: CV-A, CV-B, GN-A, GN-B, CV-C, GN-C, GN-D, GN-E; CV-D anywhere after CV-B.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| Generic structs and enums, `impl` blocks, methods with `self`, user traits, hand-written `impl Repr` | `generics` (L7, and L4's generic enums as GN-06) |
| Bundles: a `struct` of carriers, `flip`, bundle literals | `resolved-nets` (L25). A struct here is a struct of values |
| Unknown control as a merge (§3.3.3), `Unknown`, `level`, per-level flags of composites | `unknown-semantics` (L18, L19, S6). Until it lands, a new form under unknown control faults as `if` does today |
| `include_str!`, `parse_table`, `parse_hex`, `table_model` | `params-and-io` (L57); they need arrays from this feature |
| Params of integer, enum, `Option` or module type | `params-and-io` (L28) and `generics` (templates). Params of struct, tuple and array type are here (CV-18) |
| The H.1 to H.4 method rows, `<` on enums (L14 to L17) | `msl-fixes` |
| Extending the interpreter | R1: frozen at the M1 analog subset; it refuses each new form by name (CV-34) |
| Spec codes of `GAPS.md` §10 for constructs that already run, the panic codes `E0104`/`E0105` | `params-and-io`. Codes that only a construct of this feature can raise (`E0102`, `E0103`, `E0110`, `E0111`, `E0126`, `E0211`, `E0416`) are raised here |
| `extern "C" fn` with composite arguments | §3 rule 10 restricts them to scalars; `foreign-models` (L12) |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Representation in the IR and on the ABI | open: Q1 | The central question of the feature | n - user |
| Host view of composites | open: Q2 | User-facing | n - user |
| Sparse carrier arrays | open: Q3 | §5.1.8 or a departure | n - user |
| `f16`, `f128`, wide integers | open: Q4 | Supersedes or keeps RS-Q6 | n - user |
| Runaway loops and recursion | open: Q5 | A departure from Rust if bounded | n - user |
| What "as Rust" means for a test | a test's expected value is the same expression evaluated by Rust in the test itself, or the spec's own value where the spec gives one | The interpreter is frozen (R1), so it is no oracle for these forms (R2) | n - planner |
| Backend | the Rust backend only; the interpreter refuses by name | R1, R2 | y - R1 |
| A form under unknown control | faults naming the form, as `if` does today, until `unknown-semantics` lands | L19 is that feature's | n - planner |
| `let mut` in a module body | stays refused: a body `let` is a name for an expression (§8.2), not a variable | Appendix B.2 placement matrix; the Book says so | y - spec |
| Const contexts | every form of this feature also works where the spec evaluates at elaboration: `const` items, `const fn`, param defaults, `where`, generation ranges | The stdlib's `poly_q`, `poly_s` and `conjugate_closed` are `const fn` with loops, tuples, arrays and iterators | n - planner |
| Exhaustiveness | a non-exhaustive `match` or a refutable `let` without `else` is an elaboration error naming a missing pattern, as Rust; its code is the one `params-and-io` gives type errors (X5), `P0002` meanwhile | Rust's rule; X5 is not this feature's | n - planner |
| Enum variant names on the host | carried as metadata of the shape (CV-22), which also closes the `GAPS.md` §4.7 note "Enum variant names are not carried" | Q2 cannot print `Tx::Data` without them; no other feature owns the note | n - planner |

**Open questions:** none - answered 2026-11-18, every recommendation accepted (see the top of this file).

**Implicit-requirement dimensions.** Input validation and bounds: CV-13 AC 3 (index out of range), CV-32 AC 3 (literal range), Q5 (loop and recursion bounds). Failure states: every phase's refusal by name (CV-34) and panics (CV-13, CV-07). Idempotency and retry: N/A because elaboration and a run are deterministic functions of the source. Auth and rate limits: N/A, a local tool. Concurrency and ordering: CV-25 AC 4 (last write wins per level among element and field commits, §12.1.2). Data lifecycle: CV-27 (storage of carrier arrays from the first commit). Observability: CV-21, CV-22 (the host sees composites and variant names). External-dependency failure: N/A, no external service; `rustc` absence is handled by the existing build path. State-transition integrity: CV-25 (an element commit changes that element only; `changed()` per element).

---

## User Stories

### P1: Control flow and functions as in Rust (phase A) ⭐ MVP

**User Story**: As a model author, I want `let mut`, loops, early `return`, the full pattern language, closures, function values and recursion, so that a function or a reaction is written as I would write it in Rust.

**Why P1**: Loops and mutation are what every numeric routine needs (the stdlib's `zi_nd` reaction is a loop over `let mut acc`); they cost no IR type change.

**Acceptance Criteria** (CV-01, `let mut` and assignment):

1. WHEN a function body, a reaction body or an `analog` block declares `let mut x = e;` and later assigns `x = e2;` or `x += e2;` (every compound operator of Rust) THEN the system SHALL type-check, elaborate and run it with the value Rust gives.
2. IF an assignment targets a binding declared without `mut`, a param, a carrier or a body `let` THEN the system SHALL refuse it at elaboration with a diagnostic naming the target.
3. IF a module body declares `let mut` THEN the system SHALL refuse it with the placement code of Appendix B.2.

**Acceptance Criteria** (CV-02, loops):

1. WHEN a function, a reaction or an `analog` block runs `for p in range_or_iterator { .. }`, `while c { .. }`, `while let p = e { .. }` or `loop { .. }` with `break`, `break value` and `continue` THEN the system SHALL run it with the values Rust gives.
2. WHEN an `analog` loop contains a contribution or a placement and its trip count is a const expression THEN the system SHALL unroll it at elaboration, each iteration's placement a distinct instance (§8.3.4).
3. IF an `analog` loop that contains a contribution or a placement has a trip count that is not a const expression THEN the system SHALL refuse it with `E0416`.
4. WHEN an `analog` loop contains no contribution and no placement THEN the system SHALL accept any trip count and differentiate through the loop (§11 rule 5).
5. WHEN a reaction body commits inside a loop THEN the system SHALL treat the commit as one statement run several times (§12.1.14), the last write winning per level (§12.1.2).

**Acceptance Criteria** (CV-03, `return` and `?`):

1. WHEN a function returns early with `return e;` from inside a loop, an `if` or a `match` THEN the system SHALL give the caller `e`.
2. WHEN a function whose result is `Option<T>` applies `?` to an `Option` that is `None` THEN the system SHALL return `None` from that function.
3. IF `return` appears outside a function body THEN the system SHALL refuse it at elaboration naming the position.

**Acceptance Criteria** (CV-04, patterns):

1. WHEN a `match` arm has a guard `p if c` THEN the system SHALL take the arm only when the pattern matches and `c` is true, as Rust.
2. WHEN a pattern is a range `a..=b`, `a..b` or `a..` over an integer, a float or an enum without payload THEN the system SHALL match it as Rust.
3. WHEN an or-pattern binds names (`A(x) | B(x)`) or a pattern uses `name @ p` THEN the system SHALL bind them as Rust, and SHALL refuse an or-pattern whose alternatives bind different names or types.
4. WHEN `if let`, `while let` or `let .. else` uses any refutable pattern (literal, range, tuple, enum variant with payload, struct, nested) THEN the system SHALL run it as Rust.
5. IF a `match` is not exhaustive, or a refutable pattern is used in `let` without `else` THEN the system SHALL refuse it at elaboration naming one missing pattern.

**Acceptance Criteria** (CV-05, closures):

1. WHEN a closure `|a, b| e` is bound by `let`, called, or passed to a catalogue method (H.7) or to `limit` THEN the system SHALL run it, capturing by value (§3 rule 3).
2. The system SHALL give a closure the function class of its body (§3), and SHALL refuse a call of it where that class is not allowed with `E0122`.
3. WHEN a closure is called in `analog` THEN the system SHALL differentiate through it as through a direct call (§3 rule 1).
4. IF a closure contains a form excluded by §3 rule 6 inside a function THEN the system SHALL report `E0121`.

**Acceptance Criteria** (CV-06, function values):

1. WHEN a function's name is used as a value (`limit(x, limvds)`, `let f = vt;`) or a parameter has a function-pointer type `fn(f64) -> f64` THEN the system SHALL call the named function through it.
2. WHEN a closure that captures nothing is passed where a function-pointer type is expected THEN the system SHALL accept it, as Rust.
3. IF a closure that captures is passed where a function-pointer type is expected THEN the system SHALL refuse it, as Rust.
4. The system SHALL NOT carry a function value or a closure (§1 rule 9), and SHALL refuse one as a carried type with `E0112`.

**Acceptance Criteria** (CV-07, recursion):

1. WHEN a function called from a reaction or from another function calls itself, directly or through others THEN the system SHALL run it.
2. WHEN a recursion is reached from `analog` and its depth is decided by params and constants THEN the system SHALL unroll it at elaboration (§3 rule 12).
3. IF a recursion reached from `analog` cannot be unrolled at elaboration THEN the system SHALL refuse it with `E0126`, naming the function.
4. WHERE Q5 is answered (b), IF a call depth exceeds 1 024 or one loop runs more than 2^24 iterations in one evaluation THEN the system SHALL fail the analysis as a panic naming the function or loop, the instance and the instant, and SHALL NOT abort the process.

**Acceptance Criteria** (CV-08, unknown control):

1. WHILE `unknown-semantics` has not landed, IF the condition of a loop, a guard, a `break` or a `return` is unknown at run time THEN the system SHALL fault naming the construct, as an `if` faults today.
2. WHEN `unknown-semantics` lands THEN the system SHALL apply its merge rule to every form of this phase, with no change to this feature's code paths other than that rule.

**Independent Test**: a `piperine-lang` type-and-elaborate test per form; a transpile test per form whose expected values are the same Rust expression evaluated in the test; the stdlib's `zi_nd` reaction body compiled as a free function.

---

### P1: Structs, tuples, enums with data and arrays (phase B) ⭐ MVP

**User Story**: As a model author, I want structs, tuples, enums with payloads, arrays and slices as values, carried and passed as params, so that a model holds structured data as Rust does.

**Why P1**: L1 to L4 are the biggest hole of Part I (35 % runs, `GAPS.md` §9); the stdlib's filters and `normal` need them.

**Acceptance Criteria** (CV-09, structs):

1. WHEN a project declares a struct with named fields, a tuple struct or a unit struct THEN the system SHALL accept it as a value type.
2. WHEN an expression builds a struct (`P { x: 1.0, y }`, shorthand fields, `..base`, `T(1, 2)`, a unit struct) THEN the system SHALL give the value Rust gives.
3. WHEN an expression reads `s.field`, `t.0` or a nested field THEN the system SHALL give that field's value.
4. IF a struct expression leaves a field out without `..base`, names a field twice or names an unknown field THEN the system SHALL refuse it naming the field.
5. WHEN a `let mut` struct's field is assigned (`s.x = 2.0;`) THEN the system SHALL change that field only.

**Acceptance Criteria** (CV-10, tuples):

1. WHEN tuples are built, returned from a function, passed as an argument, nested, or read with `.0` THEN the system SHALL give the values Rust gives.
2. WHEN a tuple pattern destructures in `let`, `match`, `if let`, a closure parameter or a `for` THEN the system SHALL bind each component.

**Acceptance Criteria** (CV-11, enums):

1. WHEN an enum has tuple or struct variants THEN the system SHALL accept constructing each variant and matching it with bindings of its fields.
2. WHEN an enum has `#[repr(uN)]` or explicit discriminants THEN the system SHALL use them as the tag (§2 rules 2, 5), and a fieldless enum cast with `as` to an integer SHALL give its discriminant.
3. IF a discriminant does not fit `N` levels, or two discriminants are equal THEN the system SHALL refuse it with `E0111`.
4. WHEN an enum has one variant THEN the system SHALL give its tag zero levels (§2 rule 5), and a carrier of it SHALL hold the variant's payload levels only.
5. WHEN `Option<T>` holds a struct, a tuple, an array or an enum THEN the system SHALL treat it as any other enum with a payload.

**Acceptance Criteria** (CV-12, comparison and derives):

1. WHEN a struct, tuple, array or enum derives `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Clone`, `Copy` or `Default` (H.8) THEN the system SHALL give `==`, `!=`, `<`, `<=`, `>`, `>=` and `T::default()` the results Rust's derives give.
2. IF `==` or `<` is applied to a type that does not derive or implement the trait THEN the system SHALL refuse it naming the trait.
3. WHEN a type derives `Debug` or `Hash` THEN the system SHALL accept the derive and give it no run-time effect.

**Acceptance Criteria** (CV-13, arrays and slices):

1. WHEN an array is built (`[a, b, c]`, `[x; N]` with `N` a const expression), indexed with a `usize`, assigned by element in a `let mut`, nested, or passed to a `&[T]` parameter THEN the system SHALL give the values Rust gives.
2. WHEN a slice is taken (`&a[i..j]`, `&a[..]`) THEN the system SHALL give the elements `i` to `j - 1`.
3. IF an index or a slice range is out of bounds THEN the system SHALL panic (§1 rule 11): at elaboration an elaboration error, during a run a failed analysis naming the instance, the instant and the index.
4. IF an array length is not a const expression THEN the system SHALL refuse it naming the length.

**Acceptance Criteria** (CV-14, references):

1. WHEN `&` borrows an array, a slice or a string for a function parameter, or any value as a call argument (§1 rule 2, D7) THEN the system SHALL accept it.
2. IF `&` appears anywhere else THEN the system SHALL refuse it with `E0102`.
3. WHEN an element reference from `iter`, `first`, `last` or `get` is read with `*`, compared, or used in arithmetic THEN the system SHALL read it as the element, as Rust's auto-deref does.

**Acceptance Criteria** (CV-15, Appendix H.5 to H.7):

1. The system SHALL implement every row of H.5 (`len`, `is_empty`, `iter`, `contains`, `first`, `last`, `get`), H.6 (`contains`, `is_empty` on `a..b` and `a..=b`) and H.7 (`map`, `filter`, `fold`, `sum`, `count`, `zip`, `enumerate`, `rev`, `any`, `all`), each with the result Rust gives.
2. WHEN a `for` loop iterates an array, a slice, a range or any H.7 adaptor chain THEN the system SHALL visit the elements in Rust's order.
3. IF a method outside Appendix H is called on an array, a slice, a range or an iterator THEN the system SHALL refuse it with `E0161`.

**Acceptance Criteria** (CV-16, `normal`):

1. WHEN `normal(rng, mean, sigma)` is called THEN the system SHALL return the sample and the next state of §22.2.17's SplitMix64 and Box-Muller text, bit for bit equal to that text compiled by Rust in the test.
2. WHEN `sigma` is `0.0` THEN the system SHALL return `mean` as the sample.
3. WHEN the §22.2.17 jitter example runs as a clock THEN the system SHALL give the same sequence of edge times on every run with the same seed.

**Acceptance Criteria** (CV-17, carried composites):

1. WHEN a struct or enum derives `Repr` THEN the system SHALL lay it out as §2's table says (fields first-lowest; tag lowest, then the widest payload, padded with false levels), and `T::WIDTH` SHALL be that width.
2. IF a type that derives `Repr` has a field without `Repr` THEN the system SHALL refuse it with `E0110` naming the field.
3. WHEN a signal or port is `Bit<T>` for `T` a derived struct, a derived enum, a tuple, an array or an `Option` of one THEN the system SHALL accept its initial value, definitions, commits, reads, `changed()` and `prev()` over the whole value.
4. WHEN a composite carrier is committed with a value equal to its current one THEN the system SHALL NOT fire `changed()`.
5. WHEN a derived enum's tag names no variant on decode THEN the system SHALL read the value as unknown (§2 rule 9).

**Acceptance Criteria** (CV-18, composite params):

1. WHEN a module declares a param of struct, tuple or array type, with or without a default and a `where` THEN the system SHALL accept it, bind it at a placement, and read it in every block.
2. WHEN a host sets a composite param before a run THEN the system SHALL apply it in the form Q2 chooses, and check its `where` (`E0317`).

**Acceptance Criteria** (CV-19, records):

1. WHEN `emit!` writes a field of a composite value type THEN the system SHALL deliver the record with that field, and the type of the field SHALL be in the stream's metadata (§19 rule 13).
2. IF two sites of one stream give a field of the same name different composite types THEN the system SHALL refuse it with `E0605`.

**Acceptance Criteria** (CV-20, the IR and the ABI):

1. The system SHALL represent composite values in the IR and on the device ABI in the way Q1 chooses, with `Ir::check()` checking their types.
2. WHEN a composite value crosses the device ABI (a carried value, a param, a record field) THEN the system SHALL give the reading side the value the writing side wrote, for every value of the type.
3. The system SHALL give two pins of one net equal layouts (abi.md P4) for every composite carried type.
4. The system SHALL keep the generated Rust readable, each item naming its IR and MSL path (R3), a struct or enum of the source appearing as one Rust type.

**Acceptance Criteria** (CV-21, the host view):

1. WHEN `piperine run` prints a composite carrier, or a record with a composite field, in each output format (table, record lines, CSV, JSON Lines, plot) THEN the system SHALL show it as Q2 chooses.
2. WHEN a printed signal names an element or a field (`Top.st.n`, `Top.v[2]`) THEN the system SHALL print that leaf alone.

**Acceptance Criteria** (CV-22, enum names on the host):

1. WHEN a host reads a carried enum THEN the system SHALL give it the variant's name (`Regulate`), not the tag (`2`), in every output format that prints text.
2. The system SHALL carry each enum type's variant names and field names in the module's metadata, without changing the bytes of a net.

**Independent Test**: per-construct `piperine-lang` and transpile tests against Rust-evaluated expected values; a mixed design carrying `Bit<Tx>` of §2's example through a commit, a definition and a record, run with `piperine run` in each output format.

---

### P1: Arrays of carriers (phase C) ⭐ MVP

**User Story**: As a designer of a register file, a bus or a ladder of nodes, I want `[Bit; 8]` and `[Electrical; 4]` with element commits, so that a model with many carriers is written once.

**Why P1**: L24 blocks every bus, memory and generated array, and the stdlib's `zi_nd` and `laplace_nd`.

**Acceptance Criteria** (CV-23, declaring and connecting):

1. WHEN a module declares `signal x: [N; K];` or a port `[N; K]` for a discrete or a continuous nature `N` and a const `K` THEN the system SHALL make `K` carriers, each with its own events (§5.1.4).
2. WHEN a parent passes an array of carriers to an array port, or an element `x[k]` with a const `k` to a scalar port THEN the system SHALL connect element to element.
3. IF the lengths of an array argument and its port differ THEN the system SHALL refuse it naming both lengths.

**Acceptance Criteria** (CV-24, reading):

1. WHEN an array of discrete carriers is read whole THEN the system SHALL give an array value of the elements' values with their flags (§5.1.3).
2. WHEN `lines[k].changed()`, `lines[k].prev()`, `lines[k].rise()` or a probe `v(bl[k])` is written for an element THEN the system SHALL apply it to that element only.
3. IF a carrier method is applied to a whole array of carriers THEN the system SHALL refuse it with `E0211`.
4. WHEN an element was never committed THEN the system SHALL read it as unknown, and its `prev()` as unknown (§5.1.8).

**Acceptance Criteria** (CV-25, element and field commits):

1. WHEN a reaction commits `bits[3] <- true;` or `bits[i] <- x;` with `i` computed at run time THEN the system SHALL change that element only, and fire `changed()` on that element only.
2. WHEN a reaction commits a field of a struct-valued carrier (`st.n <- 3;`) or an element of an array-valued carrier (`word[2] <- true;` on `Bit<[bool; 8]>`) THEN the system SHALL change those levels only.
3. WHEN a module body defines one element (`bits[0] = a;`) THEN the system SHALL treat that element as one carrier for the drive rules (§5.2.4), refusing a definition and commits of one element with `E0221`.
4. WHEN two commits of one microstep write overlapping levels of one carrier THEN the system SHALL keep the last write per level (§12.1.2).
5. IF a run-time element index is out of bounds THEN the system SHALL panic naming the carrier, the index, the instance and the instant.

**Acceptance Criteria** (CV-26, arrays of nodes in `analog`):

1. WHEN an `analog` block contributes to elements of an array of nodes in a loop of const trip count (`for k in 0..N { i(bl[k], gnd) +> v(bl[k]) / r; }`) THEN the system SHALL give the circuit the same solution as the same contributions written out one by one.
2. IF an `analog` block indexes an array of nodes with an index that is not a const expression after unrolling THEN the system SHALL refuse it naming the index.

**Acceptance Criteria** (CV-27, storage):

1. WHERE Q3 is answered (a), WHEN a design declares `signal mem: [Bit<u8>; 1 << 32];` and commits 1 000 distinct elements THEN the system SHALL run it with a peak heap during the run that does not depend on the declared length, measured against the same design with a length of `1 << 16`.
2. WHERE Q3 is answered (b), IF an array of carriers is longer than the bound THEN the system SHALL refuse it by name at elaboration.

**Acceptance Criteria** (CV-28, the host):

1. WHEN a host prints `Top.lines[3]` or `Top.lines` THEN the system SHALL print that element, or every element in the form Q2 chooses.

**Independent Test**: an 8-bit shift register over `[Bit; 8]` checked against its expected sequence; an R-ladder over `[Electrical; 4]` against the written-out circuit and its ngspice twin's values; the memory test of CV-27.

---

### P2: Number widths (phase D)

**User Story**: As a model author, I want `f32`, `f16`, `f128` and integers wider than 64 bits, so that a model of a DSP datapath or a wide counter keeps its width.

**Why P2**: Less common than A to C; most models compute in `f64` and 64 bits. The spec allows them (§1 rule 5) and the user asked for every gap.

**Acceptance Criteria** (CV-29, `f32`):

1. WHEN a value has type `f32` (literals with an `f32` suffix, `as` conversions, arithmetic, the H.2 methods) THEN the system SHALL give the IEEE 754 single-precision results Rust gives.
2. WHEN `Bit<f32>` is carried THEN the system SHALL carry it as the ABI's `F32` leaf.
3. IF an `f32` value is used where `f64` is expected (a contribution, a probe comparison) without `as` THEN the system SHALL refuse it as a type mismatch (§1 rule 7).

**Acceptance Criteria** (CV-30, `f16`):

1. WHERE Q4 gives `f16` option (a), WHEN a value has type `f16` THEN the system SHALL give results bit for bit equal to IEEE 754 binary16 arithmetic, rounded to nearest-even, for `+ - * /`, `sqrt`, comparisons and `as` conversions.
2. WHERE Q4 gives `f16` option (b), IF a model uses `f16` THEN the system SHALL refuse it by name.

**Acceptance Criteria** (CV-31, `f128`):

1. WHERE Q4 gives `f128` option (a) or (b), WHEN a value has type `f128` THEN the system SHALL give results bit for bit equal to IEEE 754 binary128 for arithmetic, comparisons, `as` conversions and the H.2 methods the chosen option covers.
2. WHERE Q4 gives `f128` option (b), IF an H.2 method outside that option's list is called on `f128` THEN the system SHALL refuse it as `method <name> on f128`, and `GAPS.md` SHALL list the refused methods.
3. WHERE Q4 gives `f128` option (c), IF a model uses `f128` THEN the system SHALL refuse it by name, and §1 SHALL be amended in the same change.

**Acceptance Criteria** (CV-32, wide integers):

1. WHERE Q4 gives integers option (a), WHEN a value has type `UInt<N>` or `Int<N>` for any const `N` over 64 THEN the system SHALL wrap every operation modulo `2^N` (§1 rule 6) and implement the H.1 methods for it.
2. WHERE Q4 gives integers option (b), WHEN `N` is 65 to 128 THEN the system SHALL do the same, and IF `N` is over 128 THEN the system SHALL refuse it by name.
3. IF a literal does not fit its type THEN the system SHALL report `E0909`.
4. WHEN a wide integer is carried or printed THEN the system SHALL give a host its exact value, not its limbs.

**Acceptance Criteria** (CV-33, widths):

1. IF an integer width is not a const expression `>= 1`, or a float width is outside {16, 32, 64, 128} THEN the system SHALL refuse it with `E0103`.

**Independent Test**: per type, arithmetic and conversion tables checked against Rust's `f32`, against a binary16 reference written in the test, and against `u128` arithmetic for widths up to 128.

---

### P1: Every phase keeps the rest intact

**User Story**: As the maintainer, I want each phase to land without moving a number and to say plainly what it still refuses.

**Why P1**: Parity and the refusal-by-name rule are the project's guarantees.

**Acceptance Criteria** (CV-34, refusal by name):

1. WHEN the interpreter backend meets a form of this feature THEN the system SHALL refuse it by the form's name, never run it with a guessed meaning (R1).
2. WHILE a phase has not landed, IF a model uses one of its forms THEN the system SHALL keep refusing it with `P0001` naming the form.

**Acceptance Criteria** (CV-35, no regression):

1. WHEN each phase merges THEN the 7 ngspice twins SHALL print the bytes they printed before it, on each backend, with `ToleranceContract` unedited.
2. WHEN each phase merges THEN `cargo build` SHALL give no warning and `cargo test` SHALL pass.

**Acceptance Criteria** (CV-36, const contexts):

1. WHEN a form of this feature appears in a const context (a `const` item, a `const fn`, a param default, a `where`, a generation range) THEN the system SHALL evaluate it at elaboration with the value it has at run time.
2. IF a const evaluation panics THEN the system SHALL report an elaboration error naming the expression.

**Acceptance Criteria** (CV-37, bookkeeping):

1. WHEN a phase closes a `GAPS.md` id or row THEN the same commit SHALL remove it from `GAPS.md` and update `§9 Coverage` for the parts it touches.
2. WHEN a phase lets a gallery fence (Appendices D to F) or a Book example check that was refused before THEN the system SHALL hold it to the checker in the existing gallery or Book test.

**Independent Test**: `cargo test` with the twins' byte goldens; the interpreter's refusal test per form; `GAPS.md` reviewed in each closing commit.

---

## Edge Cases

- WHEN an array has length 0 THEN the system SHALL accept it, `len()` SHALL be 0 and `first()` SHALL be `None`.
- WHEN a struct has no field or a tuple is `()` THEN the system SHALL give it `WIDTH` 0 and accept it as a carried type of zero levels.
- WHEN a `for` loop's range is empty THEN the system SHALL run its body zero times, in `analog` too.
- WHEN a closure captures a value that a later `let mut` assignment changes THEN the system SHALL keep the captured value (capture by value, §3 rule 3).
- WHEN a payload enum's variant is narrower than the widest THEN the system SHALL pad with false levels, and SHALL ignore the padding on decode (§2 rule 6).
- IF a slice range has its start after its end THEN the system SHALL panic as Rust does.
- WHEN `get(i)` is called past the end THEN the system SHALL return `None` and SHALL NOT panic.
- WHEN an element of an array of carriers is passed to an `out` port of a child THEN the system SHALL give that child the drive of that element only (§5.2.6).

---

## Gaps closed

| Id | What | Closed by |
| -- | ---- | --------- |
| L1 | struct values, struct expressions, field access | CV-09, CV-12, CV-17, CV-20 |
| L2 | tuples outside `let (a, b) = (x, y);` | CV-10 |
| L3 | arrays, indexing, slices, `&` | CV-13, CV-14 |
| L4 | payload enums, `#[repr(uN)]`, explicit discriminants, one-variant enums as carriers | CV-11, CV-17 |
| L4 | generic enums | `generics` GN-06 (a generic item; checked and instantiated by that feature) |
| L5 | `let mut`, assignment, `for`, `while`, `loop`, `break`, `return` | CV-01, CV-02, CV-03 |
| L6 | match guards, range patterns, bindings in or-patterns, `if let` beyond `Some(p)` | CV-04 |
| L9 | closures beyond `limit`'s argument; a function name as a value | CV-05, CV-06; a capturing closure passed to a user function needs a `Fn` bound: `generics` GN-01 |
| L10 | recursion | CV-07 |
| L13 | `f16`, `f32`, `f128`, integers over 64 bits | CV-29 to CV-33. Residue only if the user picks a refusing option of Q4 (`f128` (b): the transcendental methods on `f128` stay refused by name; (c): `f128` refused and §1 amended) |
| L16 | the rows of H.5, H.6, H.7 | CV-15 |
| L16 | `normal` | CV-16 |
| L24 | arrays of carriers; element and field commits and definitions | CV-23 to CV-28 |
| §4.7 note | enum variant names not carried | CV-22 |

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| CV-01 | P1: control and functions (A) | - | Pending |
| CV-02 | P1: control and functions (A) | - | Pending |
| CV-03 | P1: control and functions (A) | - | Pending |
| CV-04 | P1: control and functions (A) | - | Pending |
| CV-05 | P1: control and functions (A) | - | Pending |
| CV-06 | P1: control and functions (A) | - | Pending |
| CV-07 | P1: control and functions (A) | - | Pending |
| CV-08 | P1: control and functions (A) | - | Pending |
| CV-09 | P1: aggregates (B) | - | Pending |
| CV-10 | P1: aggregates (B) | - | Pending |
| CV-11 | P1: aggregates (B) | - | Pending |
| CV-12 | P1: aggregates (B) | - | Pending |
| CV-13 | P1: aggregates (B) | - | Pending |
| CV-14 | P1: aggregates (B) | - | Pending |
| CV-15 | P1: aggregates (B) | - | Pending |
| CV-16 | P1: aggregates (B) | - | Pending |
| CV-17 | P1: aggregates (B) | - | Pending |
| CV-18 | P1: aggregates (B) | - | Pending |
| CV-19 | P1: aggregates (B) | - | Pending |
| CV-20 | P1: aggregates (B) | - | Pending |
| CV-21 | P1: aggregates (B) | - | Pending |
| CV-22 | P1: aggregates (B) | - | Pending |
| CV-23 | P1: arrays of carriers (C) | - | Pending |
| CV-24 | P1: arrays of carriers (C) | - | Pending |
| CV-25 | P1: arrays of carriers (C) | - | Pending |
| CV-26 | P1: arrays of carriers (C) | - | Pending |
| CV-27 | P1: arrays of carriers (C) | - | Pending |
| CV-28 | P1: arrays of carriers (C) | - | Pending |
| CV-29 | P2: number widths (D) | - | Pending |
| CV-30 | P2: number widths (D) | - | Pending |
| CV-31 | P2: number widths (D) | - | Pending |
| CV-32 | P2: number widths (D) | - | Pending |
| CV-33 | P2: number widths (D) | - | Pending |
| CV-34 | P1: every phase keeps the rest intact | - | Pending |
| CV-35 | P1: every phase keeps the rest intact | - | Pending |
| CV-36 | P1: every phase keeps the rest intact | - | Pending |
| CV-37 | P1: every phase keeps the rest intact | - | Pending |

**Coverage:** 37 total, 0 mapped to tasks (spec only).

---

## Sources

- The user, 2026-11-17: "gostaria que abrisse feature e escrevesse a spec para todos esses gaps ... Preciso que TODOS os gaps estejam endereçados ok? Principalmente esses que tu listou que limitam a expressividade da linguagem ok?"
- `GAPS.md` (audit of 2026-11-17): L1 to L6, L9, L10, L13, L16, L24, L58, §4.7, §9.
- `docs/spec/part_i_values.md` §1, §2, §3, §3.2; `part_ii_carriers.md` §5.1, §5.2; `part_iii_structure.md` §8.3.4; `part_iv_behavior.md` §11 rule 12, §12.1; `part_vi_io.md` §19; `part_vii_stdlib.md` §22.2.12 to §22.2.17; `appendix_h_library.md` H.5 to H.8; `appendix_b_grammar.md` B.3.6.
- `.specs/features/device-abi/abi.md` §3 and §3.1 (layouts, leaves, P3, P4).
- `.specs/features/ir-rust/spec.md` RS-Q6 (the earlier refusal of wide types by name); `.specs/features/codegen/context.md` C4, C9, R1 to R4; `.specs/STATE.md` MD-56, MD-61 to MD-64.
- `crates/piperine-lang/std/analog.msl` (`zi_nd`, `poly_q`, `laplace_nd`): the stdlib forms this feature must accept.

---

## Success Criteria

- [ ] Every program of the Independent Tests type-checks, elaborates and runs on the Rust backend with Rust's values.
- [ ] `GAPS.md` no longer lists L1 to L3, L5, L6, L9 (but for the `generics` row), L10, L24, the H.5 to H.7 rows and `normal`; L4 and L13 only as the user's Q4 answer leaves them.
- [ ] The 7 twins' bytes and every parity test are unchanged.
