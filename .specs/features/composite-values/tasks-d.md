# Composite Values Tasks (phase D: number widths)

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/composite-values/design-d.md`
**Status**: Draft (awaiting the user's approval; QD1 to QD8 are defaults the user may overrule)
**Build**: `cargo build -p piperine-numbers -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-lang --test suite_hygiene`
**Run purity**: `cargo test -p piperine-numbers --test purity`
**Run integers**: `cargo test -p piperine-numbers --test integers`
**Run formats**: `cargo test -p piperine-numbers --test formats`
**Run half**: `cargo test -p piperine-numbers --test half`
**Run quad**: `cargo test -p piperine-numbers --test quad`
**Run conversions**: `cargo test -p piperine-numbers --test conversions`
**Run precision**: `cargo test -p piperine-numbers --test precision`
**Run transcendental**: `cargo test -p piperine-numbers --release --test transcendental`
**Run irw**: `cargo test -p piperine-ir --test number_widths`
**Run interp**: `cargo test -p piperine-interp --test frozen`
**Run planw**: `cargo test -p piperine-plan --test number_widths`
**Run rtw**: `cargo test -p piperine-transpile --lib rt::widths`
**Run tw**: `cargo test -p piperine-transpile --test number_widths`
**Run typesw**: `cargo test -p piperine-lang --test types_number_widths`
**Run elabw**: `cargo test -p piperine-lang --test elab_number_widths`
**Run srcw**: `cargo test -p piperine-parity --release --test source_number_widths`
**Run apiw**: `cargo test -p piperine-api --test number_widths`
**Run clirun**: `cargo test -p piperine-cli --test run`
**Run book**: `cargo test -p piperine --test book_language --test book_inventory`
**Run docs**: `cargo test -p piperine-cli --test docs`
Buck targets: `cargo test -p piperine-parity --test buck_whole --test buck_dpwm --test buck_control --test buck_supervisor --test buck_boundary --test buck_closed_loop --test buck_power`

Executors: **every task is Sonnet**. Opus verifies the feature.

**Answers.** Q4 (user, 2026-11-23, answered directly): integers (a) any width as `u64` limbs in `rt`, carried as a `U64` array field plus metadata so the host reads one integer; `f16` (a) exact, stored as bits, operations in `f32` rounded to `f16`; `f128` (a) **complete**: a correctly rounded software float in `rt` for every operation and every H.2 method, transcendentals included, no residue in `GAPS.md`. design-d.md QD1 to QD8 are the planner's defaults.

Rules every task follows:

- **Base.** Batches D1, D2 and D3 (T91 to T109) run on `main` as it is: they create `crates/piperine-numbers/` and touch only it, the root `Cargo.toml` and `Cargo.lock` — no file phase C edits. Batch D4 starts with T110, which checks that phase C (T64 to T90) is merged on `main`; if not, the batch stops and reports.
- **Environment.** `export CARGO_TARGET_DIR=/var/tmp/piperine-target-d CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 TMPDIR=/var/tmp/piperine-tmp-d` in the same command as any cargo command (never `/var/tmp/piperine-target`, a read-only trap; never a target directory another worktree builds in). Commands run in the foreground; no polling loop; never `pkill` by pattern; never `git stash`; never `cargo clean`. If `df /` passes 90 %, stop and report.
- **Gates (user policy, 2026-11-21).** Per task: the `**Build**` line, then `cargo test -p <crate>` for every crate the task touched (its `Done when` names them). Once at the end of each batch: `cargo test --workspace --exclude piperine-lang-server --no-fail-fast`, the parity suite (`env -u PIPERINE_NGSPICE cargo test -p piperine-parity`, `cargo test -p piperine-cli --test twins`) and the buck targets above.
- **Parity is untouchable.** `crates/piperine-parity/src/contract.rs` and every `ToleranceContract` are not edited; the twin goldens, the transpile goldens of `crates/piperine-transpile/tests/golden/` and every parity target stay byte-identical; no tolerance moves.
- **No tolerance looser than correctly rounded.** Every numeric assertion of this phase is bit for bit. The oracles are fixed by design-d.md "Tech Decisions": `f32` against Rust's own `f32` evaluated in the test; `f16` and every exact `f128` operation against exact rational arithmetic written in the test (a fixture type over `num-bigint`, a dev-dependency); `f128` transcendentals against the reference vectors of T99; integers up to 128 bits against `u128`/`i128` arithmetic in the test, wider ones against `num-bigint` reduced modulo `2^N`. A test never uses the library under test as its own oracle, except T109, which uses the `f128` library (already proven by T104 to T108) and states it.
- **Only design-d.md "Goldens, pins and tests that may move" may move,** under the task it names; a replaced test is replaced by the acceptance test of the criterion that supersedes it, and the commit body names both.
- **The code follows the spec.** `docs/spec/` is the grammar and semantics authority. A divergence a task cannot fix in its scope stops the task and is reported.
- **Tests state the spec, not the code.** Expected values are the spec's own value or the oracle above; each assertion cites its rule. Test functions have exactly the names `## Checks` cites. No test is removed, weakened or skipped beyond the table above.
- **Code clarity (the user's rules, `.specs/features/code-clarity/spec.md`).** Readable at a glance; the module tree says where each thing is; each crate one purpose; **no free function, tests included** (every function has an owner type; a test's helpers are methods of a fixture type; only the `#[test]` functions themselves stand alone); **one purpose per type**; **no abbreviated name** (`exponent`, not `exp`; `precision`, not `prec`; `limbs`, not `l`; OO names); **no god method** and no needless abstraction; the data path evident. **Every new type, method, field, variant and constant carries a `///` doc comment**; a non-evident block cites its spec rule or the numerical source of its algorithm. AGENTS.md idiom rules bind (MD-13): **no macro** (no `macro_rules!`), no `unwrap()`/`expect()` on a path user input reaches, **no `unsafe`** (MD-63 item 1 binds every file of `piperine-numbers`, which generated code compiles). New concerns go to the files design-d.md "Components" names, never into `print/value.rs`, `types/check.rs` or `elab/consts.rs` beyond a dispatch arm.
- **`piperine-numbers` is embedded source.** A file refers to a sibling only as `super::<file>::`; it uses no crate but `core` and `std`; it panics nowhere (an operation the spec makes a panic returns `Option` and `rt/widths.rs` raises it). T91's purity test enforces this for every later task.
- **Conventions.** New IR `ValueType`/`Constant` variants are appended last; new IR fields carry `#[serde(default)]`. The interpreter refuses every new type by name (R1) in the same task that adds it to the IR (T111). Every new test target opens with a `//!` scope header naming its oracle, has no `#[ignore]` and no file-scope `#![allow]`. Source-level models live in `crates/piperine-parity/tests/number_widths/*.msl`.
- **Cross-lane files**: `spec_text.rs` and `docs.rs` are appended to, never edited above; `GAPS.md` keeps its section numbering; `tests/book/inventory.tsv` changes only in the rows T125 names.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. No push, no merge.
- **`lessons.py` is not run** during this feature (its `list` rewrites `.specs/lessons.json` and `.specs/LESSONS.md`).

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec - confirm before Execute. Guidelines found: `AGENTS.md` (MD-13 idiom rules, "Build and verify"), `CLAUDE.md` ("Where the tests are", MD-28 amended by MD-72), `crates/piperine-lang/tests/suite_hygiene.rs`, `.specs/STATE.md` MD-63, MD-78, MD-79; the phase A–B matrix of `tasks.md` and phase C's of `tasks-c.md`.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| Numeric library (`piperine-numbers`) | integration | every operation of every type bit for bit against its oracle: special values (±0, ±inf, NaN, subnormals, the extremes), ties, overflow, underflow, and the deterministic random sets each test names; every unary `f16` operation over all 65 536 inputs | `crates/piperine-numbers/tests/*.rs` | the `purity`, `integers`, `formats`, `half`, `quad`, `conversions`, `precision`, `transcendental` Run lines |
| IR (`piperine-ir`) | integration | each new type and constant built, typed, checked (valid and refused), printed, serialized | `crates/piperine-ir/tests/number_widths.rs` | `cargo test -p piperine-ir --test number_widths` |
| Interpreter refusals | integration | each new type refused by name | `crates/piperine-interp/tests/frozen.rs` | `cargo test -p piperine-interp --test frozen` |
| Plan (`piperine-plan`) | integration | the leaf, `count`, size and bytes of each new scalar, alone and inside a composite; equal layouts for one type | `crates/piperine-plan/tests/number_widths.rs` | `cargo test -p piperine-plan --test number_widths` |
| Runtime (`rt/widths.rs`) | unit | the panics, `Carried` bytes and levels round trip, conversions to the existing `rt` types | inline `#[cfg(test)] mod tests` of `rt/widths.rs` | `cargo test -p piperine-transpile --lib rt::widths` |
| Printer (`print/widths.rs`) | integration | each type, literal, operator, `as` and method compiled by `rustc` from hand-built IR and run; runtime files unchanged without the new types | `crates/piperine-transpile/tests/number_widths.rs` | `cargo test -p piperine-transpile --test number_widths` |
| Checker (`types/widths.rs`) | integration | every checker-decided AC with its code, span and name: widths, `E0103`, `E0909`, mismatches, method rows | `crates/piperine-lang/tests/types_number_widths.rs` | `cargo test -p piperine-lang --test types_number_widths` |
| Elaborator and const evaluator | integration | lowering, literals in their format, const contexts of every width, a const panic named | `crates/piperine-lang/tests/elab_number_widths.rs` | `cargo test -p piperine-lang --test elab_number_widths` |
| Source-level runs | integration | one test per run-time AC on the Rust backend, the tables of the Independent Test | `crates/piperine-parity/tests/source_number_widths.rs`, models in `tests/number_widths/` | `cargo test -p piperine-parity --release --test source_number_widths` |
| Host library | integration | each new cell decoded from a carrier, a param and a record, its text and JSON, `--param` read | `crates/piperine-api/tests/number_widths.rs` | `cargo test -p piperine-api --test number_widths` |
| Command line | integration | each output format with each new type | `crates/piperine-cli/tests/run.rs` | `cargo test -p piperine-cli --test run` |
| Book, spec text, repository records | integration | the language chapter's status box and claims; the removed `GAPS.md` row | `tests/book/language.rs`, `crates/piperine-cli/tests/docs.rs` | the `book`, `docs` Run lines |
| Contract re-reads (T91, T110) and the mechanical `Ty` widening (T114) | none | - (build gate and every existing target unchanged; findings on the Handoff) | - | build gate only |

## Gate Check Commands

> Generated from codebase and the user's gate policy (2026-11-21) - confirm before Execute.

| Gate Level | When to Use | Command |
| ---------- | ----------- | ------- |
| Task | After every task | the `**Build**` line, then `cargo test -p <crate>` for each crate the task touched, which includes the `Run` line(s) its `Done when` names |
| Batch | Once, after the last task of each batch (D1 to D6) | `cargo test --workspace --exclude piperine-lang-server --no-fail-fast`; `env -u PIPERINE_NGSPICE cargo test -p piperine-parity`; `cargo test -p piperine-cli --test twins`; the buck targets |

---

## Execution Plan

Phases are ordered and run sequentially - each phase completes before the next begins, and tasks within a phase execute in order. Phase numbers continue `tasks-c.md` (phases 16 to 23).

### Phase 24: The numeric crate

```
T91
```

### Phase 25: Exact arithmetic

```
T92 → T93 → T94 → T95 → T96 → T97 → T98
```

### Phase 26: The transcendental engine

```
T99 → T100 → T101 → T102 → T103 → T104
```

### Phase 27: The transcendental methods

```
T105 → T106 → T107 → T108 → T109
```

### Phase 28: IR, plan, runtime and checker types

```
T110 → T111 → T112 → T113 → T114 → T115
```

### Phase 29: Printer, elaborator and the float runs

```
T116 → T117 → T118 → T119 → T120 → T121
```

### Phase 30: Wide integers, host, Book and closure

```
T122 → T123 → T124 → T125 → T126
```

---

## Task Breakdown

### T91: The `piperine-numbers` crate and its purity test

**What**: Create the leaf crate `crates/piperine-numbers` (no dependency; `num-bigint` as a dev-dependency only, already in `Cargo.lock`), add it to the root `Cargo.toml` `members` and `default-members`; `lib.rs` declares the modules of design-d.md "Components" as they are added (empty at first) with a `//!` header stating MD-63 item 1 and the embedding rule; create `tests/purity.rs`, which reads every `src/*.rs` and fails on a `use` of a crate other than `core`/`std`/`super`, on `unsafe`, on `macro_rules!`, on `panic!`/`unwrap()`/`expect(` and on a module-level `fn` (MD-13 rule 2). Re-read the anchors design-d.md names (`types/check.rs:460-511`, `lexer.rs` suffixes, `catalogue.rs` keys, `consts.rs` wrap, `model.rs` `ValueType`/`Constant`, `check.rs:773-891`, `rt.rs` `Runtime::files`, `print/value.rs:605`, `Cell`) and write on the Handoff what differs from the design.
**Where**: `crates/piperine-numbers/tests/purity.rs`
**Depends on**: None
**Reuses**: `crates/piperine-ir/tests/purity.rs`'s pattern if present; `abi_source.rs`
**Requirement**: CV-35
**Executor**: Sonnet

**Done when**:

- [x] `the_numeric_crate_is_embeddable_source` passes under `**Run purity**`
- [x] The Handoff names every anchor found or its difference
- [x] Gate check passes: Task (`piperine-numbers`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(numbers): the embeddable numeric crate`

---

### T92: `Limbs`: magnitude arithmetic on `u64` limbs

**What**: `limbs.rs`: the trait `Limbs` implemented for `[u64]` (little-endian limbs): add and subtract with carry and borrow, multiply into a buffer, divide with remainder (Knuth, TAOCP vol. 2 §4.3.1 algorithm D), shift left and right by any count, compare, bit length, trailing zeros, decimal digits of a magnitude and a magnitude from decimal digits. Create `tests/integers.rs` with a fixture type `BigOracle` over `num-bigint`.
**Where**: `crates/piperine-numbers/src/limbs.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: none
**Requirement**: CV-32
**Executor**: Sonnet

**Done when**:

- [x] `limbs_agree_with_big_integers_on_every_operation` passes under `**Run integers**` (edge limbs `0`, `1`, `u64::MAX`, the divisor normalization cases of algorithm D, and 10 000 deterministic random operand pairs of 1 to 40 limbs)
- [x] Gate check passes: Task (`piperine-numbers`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(numbers): limb arithmetic`

---

### T93: `WideUInt` and `WideInt`: integers wider than 64 bits

**What**: `wide.rs`: `WideUInt<const N: u32, const L: usize>` and `WideInt<N, L>` (`L = ceil(N/64)`, asserted at compile time as `rt::UInt` asserts its width), Copy, every value in canonical form (unsigned below `2^N`, signed sign-extended); `+ - *` unary `-`, `!`, `& | ^`, shifts by any integer amount with ROADMAP G5's rule (a shift by `N` or more gives 0, or the sign for a signed `>>`), `Eq`/`Ord` as integers; `checked_divide`/`checked_remainder -> Option` (None for a zero divisor); `as` among every integer width (truncate, extend by the source's sign: §1.4.5) including from and to `u64`/`i64`; H.1 in total form: `abs` (least value wraps to itself, H.1 rule 1), `signum`, `min`, `max`, `clamp_checked -> Option` (None when `lo > hi`), `pow` (modulo `2^N`, `x^0 = 1`), `rem_euclid_checked`, `checked_add/sub/mul -> Option`, `saturating_add/sub/mul`, `add/sub/mul_overflows`. Tests in `tests/integers.rs`.
**Where**: `crates/piperine-numbers/src/wide.rs`
**Depends on**: T92
**Reuses**: `Limbs`; `rt/int.rs`'s method names (so generated code spells them alike)
**Requirement**: CV-32
**Executor**: Sonnet

**Done when**:

- [x] `wide_integers_up_to_128_bits_agree_with_u128_and_i128` passes under `**Run integers**` (`N` = 65, 100, 127, 128, every operator and H.1 method, the operands 0, 1, `MAX`, `MIN`, `-1` and 10 000 random pairs, expected values computed with `u128`/`i128` wrapping arithmetic masked to `N` bits in the test)
- [x] `wide_integers_above_128_bits_agree_with_big_integers_modulo_two_to_the_n` passes under `**Run integers**` (`N` = 129, 200, 256, 1000)
- [x] Gate check passes: Task (`piperine-numbers`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(numbers): integers of any width`

---

### T94: `BinaryFormat`: rounding an exact value to an IEEE 754 format

**What**: `format.rs`: `BinaryFormat` with the constants `HALF`, `SINGLE`, `DOUBLE`, `QUAD` (bits, precision 11/24/53/113, exponent range) and `round(sign, magnitude: &[u64], binary_exponent: i64, sticky: bool) -> u128` giving the bits of the nearest value of the format, ties to even, with gradual underflow to subnormals and zero and overflow to infinity (IEEE 754-2019 §4.3.1, §7.4); `classify(bits)` (zero, subnormal, normal, infinite, NaN). Create `tests/formats.rs` with a fixture type `ExactRational` over `num-bigint` (a fraction, its comparison with the two neighbouring midpoints of a format).
**Where**: `crates/piperine-numbers/src/format.rs`
**Depends on**: T93
**Reuses**: `Limbs`
**Requirement**: CV-30, CV-31
**Executor**: Sonnet

**Done when**:

- [x] `rounding_to_every_format_is_nearest_ties_to_even` passes under `**Run formats**` (for each format: every exact tie, one above and below, the largest finite value and the first overflow, the smallest subnormal and half of it, and 20 000 random exact rationals; for `SINGLE` and `DOUBLE` the expected bits are also Rust's own `f32`/`f64` from the same rational, so the fixture is checked against Rust)
- [x] Gate check passes: Task (`piperine-numbers`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(numbers): correct rounding to every binary format`

---

### T95: `Float16`: exact `f16`

**What**: `half.rs`: `Float16(u16)`; `+ - * /` and `sqrt` computed in `f32` and rounded once to `f16` (exact, because 24 ≥ 2·11 + 2: the user's Q4 (a)); unary `-`; `PartialEq`/`PartialOrd` by hand as IEEE 754 (NaN unordered and unequal, `-0.0 == 0.0`); `is_nan`, `is_finite`, `is_infinite`; the exact H.2 methods (`abs`, `floor`, `ceil`, `round` ties away, `trunc`, `fract`, `signum`, `min`/`max` with H.2 rule 3, `clamp_checked`, `rem_euclid` exact then rounded once (rule 9, `d = 0` NaN), `hypot` in `f64` then rounded once); its bits in and out. Create `tests/half.rs`.
**Where**: `crates/piperine-numbers/src/half.rs`
**Depends on**: T94
**Reuses**: `BinaryFormat::HALF`
**Requirement**: CV-30
**Executor**: Sonnet

**Done when**:

- [x] `every_unary_f16_operation_is_correctly_rounded_over_all_inputs` passes under `**Run half**` (all 65 536 bit patterns: `sqrt`, `-`, the exact H.2 methods and the predicates, against `ExactRational`)
- [x] `binary_f16_operations_are_correctly_rounded` passes under `**Run half**` (`+ - * /`, comparisons, `min`, `max`, `rem_euclid`, `hypot` over every pair of the special values and 2^22 deterministic random pairs)
- [x] Gate check passes: Task (`piperine-numbers`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(numbers): exact f16`

---

### T96: `Float128`: exact `f128` arithmetic

**What**: `quad.rs`: `Float128(u128)`; `+ - * /` and `sqrt` exact by limb arithmetic then rounded once by `BinaryFormat::QUAD`; unary `-`; IEEE 754 `PartialEq`/`PartialOrd` by hand; the predicates; `abs`, `floor`, `ceil`, `round` (ties away), `trunc`, `fract`, `signum`, `min`/`max` (H.2 rule 3), `clamp_checked`, `rem_euclid` (exact remainder, one rounding, H.2 rule 9) and `hypot` (`x² + y²` exact, then the correctly rounded square root: no overflow in the squares, H.2 rule 8). Create `tests/quad.rs`.
**Where**: `crates/piperine-numbers/src/quad.rs`
**Depends on**: T95
**Reuses**: `Limbs`, `BinaryFormat::QUAD`, `ExactRational` (test)
**Requirement**: CV-31
**Executor**: Sonnet

**Done when**:

- [x] `f128_arithmetic_and_square_root_are_correctly_rounded` passes under `**Run quad**` (special values, subnormal operands and results, exact ties, overflow, cancellation, and 50 000 random pairs per operation against `ExactRational`; `sqrt` checked by squaring the bracketing midpoints exactly)
- [x] `f128_exact_methods_follow_h2` passes under `**Run quad**` (every exact H.2 method and predicate, with `-0.0`, `±inf`, NaN, and `hypot` at the largest finite value)
- [x] Gate check passes: Task (`piperine-numbers`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(numbers): correctly rounded f128 arithmetic`

---

### T97: Conversions among every width

**What**: `convert.rs`: `NumberConversion` (one owner for `as`, §1.4.5): float to float of every pair of `Float16`, `f32`, `f64`, `Float128` (nearest, ties to even, overflow to infinity, NaN to NaN), rounded once from the source (never through an intermediate format); integer of any width (`u64`/`i64`-backed and `WideUInt`/`WideInt`) to every float (nearest, ties to even); every float to every integer (truncate toward zero, clamp to the target's range, NaN to 0). Create `tests/conversions.rs`.
**Where**: `crates/piperine-numbers/src/convert.rs`
**Depends on**: T96
**Reuses**: `BinaryFormat::round`, `ExactRational` (test)
**Requirement**: CV-30, CV-31, CV-32
**Executor**: Sonnet

**Done when**:

- [x] `every_as_conversion_follows_the_table_of_1_4_5` passes under `**Run conversions**` (each pair over the special values, the edges of each range and 5 000 random values; the `f32`↔`f64` and `u64`/`i64`↔`f32`/`f64` pairs are compared with Rust's own `as`, the rest with `ExactRational`; a double-rounding case for `f64 → f16` that `f64 → f32 → f16` gets wrong is in the set)
- [x] Gate check passes: Task (`piperine-numbers`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(numbers): as conversions among every width`

---

### T98: `DecimalText`: literals and the host's text

**What**: `decimal.rs`: `DecimalText::nearest(text, format) -> Option<u128>` reads a decimal literal (digits, underscores, fraction, exponent, the SI multiplier of A.2 as a power of ten) into the nearest value of a `BinaryFormat`, ties to even (A.8 rule 8, exact: no intermediate `f64`); `DecimalText::integer(text, width, signed)` reads decimal and `0x`/`0o`/`0b` integers of any width, `None` past the type (A.2 rule 8); `DecimalText::shortest(bits, format)` gives the shortest decimal that reads back to the same bits (QD6), `inf`, `-inf`, `NaN` as the existing `f64` text spells them; `DecimalText::integer_text` gives a wide integer's exact decimal. Tests in `tests/formats.rs`.
**Where**: `crates/piperine-numbers/src/decimal.rs`
**Depends on**: T97
**Reuses**: `BinaryFormat`, `Limbs`
**Requirement**: CV-30, CV-31, CV-32
**Executor**: Sonnet

**Done when**:

- [x] `decimal_text_reads_to_the_nearest_value_and_prints_the_shortest` passes under `**Run formats**` (for `SINGLE` and `DOUBLE`, 20 000 random texts against Rust's `str::parse` and `Display`; for `HALF` and `QUAD`, the same texts against `ExactRational`, with midpoint texts of 40 digits and every power of ten in range; shortest text read back gives the same bits for every `f16` and 20 000 random `f128`)
- [x] Gate check passes: Task (`piperine-numbers`), then the Batch gate

**Tests**: integration
**Gate**: batch

**Commit**: `feat(numbers): decimal text to and from every format`

---

### T99: The reference vectors of the `f128` transcendentals

**What**: `tests/vectors/generate.py` (Python 3, `mpmath` and `fractions` only; run once by the task, never by `cargo test`): for each H.2 transcendental (`exp`, `ln`, `log2`, `log10`, `powf`, `powi`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`, `tanh`) it computes the function at 1 024 bits, rounds to binary128 by exact comparison with the two neighbouring midpoints, raising the precision until the decision is made, and writes `tests/vectors/<function>.tsv` (input bits and expected bits in hex, a header naming the generator, `mpmath`'s version and the date) with the sets of design-d.md "The oracle of the `f128` transcendentals" (~2 000 rows each). The generator also writes `exact.tsv`: the exact cases of design-d.md with their exact results. Create `tests/transcendental.rs` with a fixture type `ReferenceVectors` (the reader of the `.tsv` files) and a test that every file parses and every expected value is a valid binary128.
**Where**: `crates/piperine-numbers/tests/vectors/generate.py`
**Depends on**: None (its phase follows the previous one)
**Reuses**: none
**Requirement**: CV-31
**Executor**: Sonnet

**Done when**:

- [x] Every `.tsv` of the 16 functions and `exact.tsv` exists, generated by the committed script
- [x] `the_reference_vectors_are_complete_and_well_formed` passes under `**Run transcendental**`
- [x] Gate check passes: Task (`piperine-numbers`)

**Tests**: integration
**Gate**: task

**Commit**: `test(numbers): binary128 reference vectors for the H.2 transcendentals`

---

### T100: `MultiPrecision`: floats of any precision with directed rounding

**What**: `precision.rs`: `MultiPrecision` (sign, limb magnitude, binary exponent, precision in bits) with add, subtract, multiply, divide and square root, each rounded toward −∞ or +∞ as asked, scaling by powers of two, comparison, conversion from a `Float128` (exact) and to the pair of `f128` bounds; `Interval` (a lower and an upper `MultiPrecision`) with the same operations, each widening outward. Create `tests/precision.rs`.
**Where**: `crates/piperine-numbers/src/precision.rs`
**Depends on**: T99
**Reuses**: `Limbs`, `BinaryFormat`
**Requirement**: CV-31
**Executor**: Sonnet

**Done when**:

- [x] `directed_rounding_brackets_the_exact_result` passes under `**Run precision**` (each operation at 120, 240 and 1 000 bits on 5 000 random operands: the lower result is at most, and the upper at least, the exact `ExactRational` result, and each is the adjacent value of its precision)
- [x] Gate check passes: Task (`piperine-numbers`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(numbers): multi-precision floats with directed rounding`

---

### T101: `ReferenceConstants`: π, ln 2 and ln 10 to any precision

**What**: `constants.rs`: `ReferenceConstants::pi(precision)`, `ln_two(precision)`, `ln_ten(precision)` as `Interval`s whose width is at most two units of the last place, computed by series with proven error bounds (π by Machin's formula, ln 2 and ln 10 by the `atanh` series of `ln((1 + t)/(1 − t))`), cached per precision behind `std::sync::OnceLock` (safe std, MD-63 item 1).
**Where**: `crates/piperine-numbers/src/constants.rs`
**Depends on**: T100
**Reuses**: `MultiPrecision`, `Interval`
**Requirement**: CV-31
**Executor**: Sonnet

**Done when**:

- [x] `reference_constants_contain_their_published_digits` passes under `**Run precision**` (π, ln 2 and ln 10 to 1 000 decimal places, written in the test from OEIS A000796, A002162, A002392, lie inside the interval at 3 300 bits; π's interval at 16 600 bits contains the 1 000-digit value)
- [x] Gate check passes: Task (`piperine-numbers`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(numbers): pi, ln 2 and ln 10 to any precision`

---

### T102: `ArgumentReduction`: modulo π/2 and ln 2

**What**: `reduction.rs`: `ArgumentReduction::quarter_turns(x: Float128, precision) -> (quadrant, Interval)` reduces any finite `f128` modulo π/2 with π carried to the bits `x`'s exponent needs (Payne–Hanek: the reduced value keeps `precision` bits after the cancellation), and `ln_two_steps(x, precision) -> (k, Interval)` writes `x = k·ln 2 + r` with `|r| <= ln 2 / 2`.
**Where**: `crates/piperine-numbers/src/reduction.rs`
**Depends on**: T101
**Reuses**: `ReferenceConstants`, `Interval`
**Requirement**: CV-31
**Executor**: Sonnet

**Done when**:

- [x] `reduction_keeps_full_precision_for_every_finite_argument` passes under `**Run precision**` (the largest finite `f128`, the `f128` nearest to multiples of π/2 found by the T99 generator and written in `exact.tsv`'s neighbours, and 2 000 random arguments: the reduced interval contains the `ExactRational` reduction against π at 20 000 bits and is no wider than `2^-precision` relative)
- [x] Gate check passes: Task (`piperine-numbers`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(numbers): argument reduction for any finite f128`

---

### T103: `ZivRounding`: from an interval to one correctly rounded `f128`

**What**: `ziv.rs`: `ZivRounding::round(evaluation, x)` evaluates an `Interval`-valued function at 160 bits, then 320, 640, … until both bounds round (`BinaryFormat::QUAD`) to the same bits, and returns them; a function first offers its exact cases (design-d.md "Exact cases") and the loop runs only when there is none; a precision past 2^20 bits is a defect the method reports as `None` (never reached by a function with its exact cases, which T104 to T108 prove).
**Where**: `crates/piperine-numbers/src/ziv.rs`
**Depends on**: T102
**Reuses**: `Interval`, `BinaryFormat`
**Requirement**: CV-31
**Executor**: Sonnet

**Done when**:

- [ ] `ziv_rounding_raises_precision_until_the_bounds_agree` passes under `**Run precision**` (a synthetic function whose interval straddles a midpoint at 160 and 320 bits and not at 640 returns the right bits after three evaluations; an exact case returns without evaluating; a function whose value is a midpoint and that offers no exact case returns `None` at the cap)
- [ ] Gate check passes: Task (`piperine-numbers`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(numbers): Ziv rounding of interval evaluations`

---

### T104: `exp`, `ln`, `log2`, `log10` on `f128`

**What**: `transcendental.rs` (`impl Float128`, a sibling file of `quad.rs`): `exp` (reduction by ln 2, Taylor series of the remainder with an error bound), `ln` (`ln(m·2^e) = e·ln 2 + ln m`, `ln m` by the `atanh` series), `log2`, `log10` (`ln x / ln 2`, `/ ln 10`), each through `ZivRounding` with its exact cases (`exp(±0) = 1`, `ln(1) = +0`, `log2(2^k) = k`, `log10(10^k) = k`), and H.2 rule 7 (`ln` of a negative is NaN, `ln(±0) = -inf`, overflow to infinity, underflow to subnormals and zero).
**Where**: `crates/piperine-numbers/src/transcendental.rs`
**Depends on**: T103
**Reuses**: `ZivRounding`, `ArgumentReduction`, `ReferenceConstants`
**Requirement**: CV-31
**Executor**: Sonnet

**Done when**:

- [ ] `exp_and_logarithms_match_the_reference_vectors_bit_for_bit` passes under `**Run transcendental**` (every row of `exp.tsv`, `ln.tsv`, `log2.tsv`, `log10.tsv` and their rows of `exact.tsv`)
- [ ] Gate check passes: Task (`piperine-numbers`), then the Batch gate

**Tests**: integration
**Gate**: batch

**Commit**: `feat(numbers): correctly rounded exp and logarithms on f128`

---

### T105: `powf` and `powi` on `f128`

**What**: in `transcendental.rs`: `powf` (`exp(e·ln x)` as an `Interval`, with IEEE 754 §9.2.1 `pow`'s special cases as Rust's `powf` gives them: `x^±0 = 1`, `1^y = 1`, negative base with a non-integer exponent NaN (H.2 rule 7), signed zeros and infinities) and `powi(e: i32)`; exact cases: a result computed exactly by `Limbs` when its exact value has at most 2·113 + 2 significant bits (an integer exponent; a base that is an exact square for exponent `k/2`), otherwise the Ziv loop (neither representable nor a midpoint).
**Where**: `crates/piperine-numbers/src/transcendental.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `ZivRounding`, `exp`, `ln`
**Requirement**: CV-31
**Executor**: Sonnet

**Done when**:

- [ ] `powers_match_the_reference_vectors_bit_for_bit` passes under `**Run transcendental**` (every row of `powf.tsv`, `powi.tsv` and their exact rows, the midpoint cases among them)
- [ ] Gate check passes: Task (`piperine-numbers`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(numbers): correctly rounded powf and powi on f128`

---

### T106: `sin`, `cos`, `tan` on `f128`

**What**: in `transcendental.rs`: `sin`, `cos`, `tan` through `ArgumentReduction::quarter_turns` and Taylor series of the reduced argument with error bounds, through `ZivRounding`; exact cases `sin(±0) = ±0`, `tan(±0) = ±0`, `cos(±0) = 1`; infinities and NaN give NaN.
**Where**: `crates/piperine-numbers/src/transcendental.rs`
**Depends on**: T105
**Reuses**: `ArgumentReduction`, `ZivRounding`
**Requirement**: CV-31
**Executor**: Sonnet

**Done when**:

- [ ] `circular_functions_match_the_reference_vectors_bit_for_bit` passes under `**Run transcendental**` (every row of `sin.tsv`, `cos.tsv`, `tan.tsv`, the largest finite argument and the arguments nearest multiples of π/2 among them)
- [ ] Gate check passes: Task (`piperine-numbers`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(numbers): correctly rounded sin, cos and tan on f128`

---

### T107: `asin`, `acos`, `atan`, `atan2` on `f128`

**What**: in `transcendental.rs`: `atan` (argument halving and the series), `asin`/`acos` through `atan` of `x/sqrt(1 − x²)` as intervals, `atan2` (H.2 rule 8: the angle of `(x, y)` in `[-π, π]`, IEEE 754 §9.2.1's signed-zero and infinity cases as Rust's `atan2` gives them); exact cases at signed zeros, `acos(1) = +0`; a magnitude above 1 is NaN for `asin`/`acos` (H.2 rule 7).
**Where**: `crates/piperine-numbers/src/transcendental.rs`
**Depends on**: T106
**Reuses**: `ReferenceConstants::pi`, `ZivRounding`
**Requirement**: CV-31
**Executor**: Sonnet

**Done when**:

- [ ] `inverse_circular_functions_match_the_reference_vectors_bit_for_bit` passes under `**Run transcendental**` (every row of `asin.tsv`, `acos.tsv`, `atan.tsv`, `atan2.tsv`)
- [ ] Gate check passes: Task (`piperine-numbers`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(numbers): correctly rounded inverse circular functions on f128`

---

### T108: `sinh`, `cosh`, `tanh` on `f128`

**What**: in `transcendental.rs`: `sinh`, `cosh`, `tanh` from `exp` intervals (with the series near zero so `sinh` and `tanh` keep full relative precision there), overflow to infinity, `tanh` saturating to ±1; exact cases at ±0.
**Where**: `crates/piperine-numbers/src/transcendental.rs`
**Depends on**: T107
**Reuses**: `exp`, `ZivRounding`
**Requirement**: CV-31
**Executor**: Sonnet

**Done when**:

- [ ] `hyperbolic_functions_match_the_reference_vectors_bit_for_bit` passes under `**Run transcendental**` (every row of `sinh.tsv`, `cosh.tsv`, `tanh.tsv`)
- [ ] Gate check passes: Task (`piperine-numbers`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(numbers): correctly rounded hyperbolic functions on f128`

---

### T109: The `f16` transcendentals

**What**: in `half.rs`: every H.2 transcendental on `Float16` computed in `f64` by Rust's method and rounded once to `f16` (QD3), H.2 rule 7's domain cases as `f64` gives them. The test's oracle is the `f128` library (proven bit for bit by T104 to T108) rounded to `f16` by `BinaryFormat`, with an ambiguity guard: an input whose `f128` result lies within one `f128` unit of an `f16` midpoint fails the test naming it (the oracle could not decide), so a pass means each result is the correctly rounded one.
**Where**: `crates/piperine-numbers/src/half.rs`
**Depends on**: T108
**Reuses**: `Float128`'s transcendentals (test), `NumberConversion`
**Requirement**: CV-30
**Executor**: Sonnet

**Done when**:

- [ ] `every_f16_transcendental_is_correctly_rounded_over_all_inputs` passes under `**Run transcendental**` (each one-argument method over all 65 536 inputs; `powf`, `atan2` over every pair of special values and 2^20 deterministic pairs; `powi` over all inputs with exponents −4 to 4)
- [ ] Gate check passes: Task (`piperine-numbers`), then the Batch gate

**Tests**: integration
**Gate**: batch

**Commit**: `feat(numbers): correctly rounded f16 transcendentals`

---

### T110: Rebase on phase C and re-read the anchors

**What**: Rebase the phase D branch onto `main` once phase C (T64 to T90) is merged there (stop and report if not); confirm the `**Build**` line is green with zero warnings; re-read the anchors D3 to D6 edit, as phase C left them (`model.rs` `ValueType`/`Constant`/`Ir`, `check.rs` width rule, `plan/composite.rs` and `plan/array.rs`, `digital.rs` `CarrierLayout`, `rt.rs` `Runtime::files` with phase C's `rt/array.rs`, `print/value.rs` `rust_type`, `types/check.rs` widths, `catalogue.rs`, `consts.rs`, `table.rs` `Cell`, `interp/src/composite.rs`), and write on the Handoff what moved or differs from design-d.md.
**Where**: `.specs/features/composite-values/tasks-d.md` (Handoff only)
**Depends on**: None (its phase follows the previous one)
**Reuses**: phase C's T64 procedure
**Requirement**: CV-35
**Executor**: Sonnet

**Done when**:

- [ ] Phase C's T90 commit is an ancestor of the branch head; the head is recorded on the Handoff
- [ ] Every anchor is found or its difference is on the Handoff
- [ ] Gate check passes: Task (Build line only)

**Tests**: none
**Gate**: task

**Commit**: `docs(composite-values): phase D contract re-read`

---

### T111: IR: the new float types, wide widths and their constants

**What**: `ValueType::{F32, F16, F128}` and `Constant::{FloatBits(u128), Wide(WideId)}` appended; `WideData { limbs }` and `Ir.wides` (`#[serde(default)]`) with an `IrBuilder` method; `check/widths.rs`: an integer width in `1..=2^32 − 1` (RS-Q6's 1..=64 superseded, VD3), a `FloatBits` constant only at a non-`f64` float type and holding no bit past its width, a `Wide` constant only at an integer type over 64 bits with `ceil(N/64)` limbs in canonical form; typing of operators, `as` and `Method`/`MathFn` for every float and integer width; `print.rs` spells `1.5f32`, `f16(0x3c00)`, `f128(0x3fff…)`, `wide(0x…)`. The interpreter refuses each new type by name in `crates/piperine-interp/src/widths.rs` (`NumberWidths`: `a value of type f32`, `… f16`, `… f128`, `… u200`) before `CompositeForms` (R1, CV-34 AC 1). Create `crates/piperine-ir/tests/number_widths.rs`.
**Where**: `crates/piperine-ir/src/check/widths.rs`
**Depends on**: T110
**Reuses**: `check/aggregates.rs` rule style, `CompositeForms`
**Requirement**: CV-29, CV-30, CV-31, CV-32, CV-34
**Executor**: Sonnet

**Done when**:

- [ ] `number_width_types_and_constants_are_checked_printed_and_serialized` passes under `**Run irw**`
- [ ] `each_number_width_is_refused_by_name` passes under `**Run interp**`
- [ ] Every existing `piperine-ir` and `piperine-interp` target passes (an IR without the new forms prints and serializes byte-identical)
- [ ] Gate check passes: Task (`piperine-ir`, `piperine-interp`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(ir): f16, f32, f128 and integers of any width`

---

### T112: Plan: the layouts and bytes of the new scalars

**What**: `crates/piperine-plan/src/widths.rs` (`NumberLayout`): `f32` the `F32` leaf (size 4, align 4), `f16` one `U16` field, `f128` one `U64` field of `count` 2 and `UInt<N>`/`Int<N>` with `N > 64` one `U64` field of `count` `ceil(N/64)`, low limb first (design-d.md "ABI"); the constant bytes of `FloatBits` and `Wide`; `CarrierLayout`/`CompositeLayout` dispatch to it, so a new scalar inside a composite is one leaf with its `count`; `abi.md` §3.1 P3 gains the sentence "the IR type is the metadata" and `validate.rs` is shown (by test) to accept a `U64` field of `count > 1`. Create `crates/piperine-plan/tests/number_widths.rs`.
**Where**: `crates/piperine-plan/src/widths.rs`
**Depends on**: T111
**Reuses**: `CarrierLayout`, `CompositeLayout`, `PinLayout`
**Requirement**: CV-29, CV-32
**Executor**: Sonnet

**Done when**:

- [ ] `each_new_scalar_has_its_leaf_count_and_bytes` passes under `**Run planw**` (alone, in a struct, in an array and in a record field; two pins of one type have equal layouts, abi.md P4)
- [ ] Gate check passes: Task (`piperine-plan`, `piperine-abi`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(plan): layouts of f16, f32, f128 and wide integers`

---

### T113: Runtime: the library embedded, and MSL's rules over it

**What**: `rt.rs` embeds every `piperine-numbers` source file as `rt/numbers/<file>.rs` with a generated module root, written only for a design that uses a new width (`RuntimeForms { digital, numbers }`), so every other design's runtime files and cache key are what they are today; `rt/widths.rs`: `impl Div`/`Rem` for `WideUInt`/`WideInt` raising `rt::Panic` on a zero divisor (§1 rule 11), `Clamp` raising on `lo > hi` or a NaN bound (H.1 rule 3, H.2 rule 4), `rem_euclid` by zero for wide integers, `impl rt::Carried` for `f32`, `Float16`, `Float128`, `WideUInt`, `WideInt` (bytes of T112's layout, levels of §2's table), and `Convert` between them and `rt::UInt`, `rt::Int`, `f64`/`R` (dropping the tangent of an `R`, QD4).
**Where**: `crates/piperine-transpile/src/rt/widths.rs`
**Depends on**: T112
**Reuses**: `abi_source.rs` embedding, `rt::Panic`, `rt::Carried`, `LevelWords`, `NumberConversion`
**Requirement**: CV-29, CV-30, CV-31, CV-32
**Executor**: Sonnet

**Done when**:

- [ ] `the_runtime_raises_msl_panics_and_carries_every_width` passes under `**Run rtw**` (each panic's message, `Carried` bytes and levels round trip for each type, conversions against `NumberConversion`)
- [ ] `a_design_without_number_widths_gets_todays_runtime_files` passes under `**Run tw**`
- [ ] Gate check passes: Task (`piperine-transpile`)

**Tests**: unit
**Gate**: task

**Commit**: `feat(transpile): the numeric runtime, embedded when a design uses it`

---

### T114: Checker: integer widths as `u32`

**What**: `Ty::UInt(u8)`/`Ty::Int(u8)` become `Ty::UInt(u32)`/`Ty::Int(u32)` across `crates/piperine-lang/src/types/` and `elab/` with no behaviour change (widths over 64 stay `P0001` until T115), so T115 is a change of rules only.
**Where**: `crates/piperine-lang/src/types/ty.rs`
**Depends on**: T113
**Reuses**: none
**Requirement**: CV-32
**Executor**: Sonnet

**Done when**:

- [ ] Every existing `piperine-lang` target passes unchanged
- [ ] Gate check passes: Task (`piperine-lang`)

**Tests**: none
**Gate**: task

**Commit**: `refactor(lang): integer widths held as u32`

---

### T115: Checker: every width of §1 rule 5

**What**: `types/widths.rs` (`impl Checker`): `Ty::{F32, F16, F128}`; `Float<N>`/`fN` for `N` in {16, 32, 64, 128}; `UInt<N>`/`Int<N>`/`uN`/`iN` for any width `1..=2^32 − 1`, the width any const expression folded as an array length folds (VD1); `E0103` for a width below 1 or not const, a float width outside {16, 32, 64, 128} and an integer width past Appendix C.2's limit (QD1; C.2 and `E0103`'s row amended in the same task); a float literal typed by its position (§1.4.2) and `E0909` when its nearest value in its format is infinite (QD5); an integer literal `E0909` when it does not fit its type (CV-32 AC 3); the `as` pairs of §1.4.5 for every width; `catalogue.rs` keys H.2 rows on every float type and H.1 rows on every integer type; an `f32` where an `f64` is required is the existing mismatch (CV-29 AC 3). `types_aggregates.rs::phase_c_and_d_forms_stay_p0001_by_name` leaves (design-d.md "may move"). Create `crates/piperine-lang/tests/types_number_widths.rs`.
**Where**: `crates/piperine-lang/src/types/widths.rs`
**Depends on**: T114
**Reuses**: `float_type`/`integer_type`, `fold_length`, `catalogue.rs`
**Requirement**: CV-29, CV-30, CV-31, CV-32, CV-33
**Executor**: Sonnet

**Done when**:

- [ ] `every_float_and_integer_width_type_checks` passes under `**Run typesw**`
- [ ] `a_width_outside_the_rules_is_e0103` passes under `**Run typesw**`
- [ ] `a_literal_out_of_its_type_is_e0909` passes under `**Run typesw**`
- [ ] `an_f32_where_an_f64_is_required_is_a_mismatch` passes under `**Run typesw**`
- [ ] Gate check passes: Task (`piperine-lang`), then the Batch gate

**Tests**: integration
**Gate**: batch

**Commit**: `feat(lang): f16, f32, f128 and integers of any width type-check`

---

### T116: Printer: the new widths as Rust

**What**: `print/widths.rs` (`impl ValuePrinter`), reached by one dispatch arm each from `rust_type`, `integer_path`, the literal, operator, `as` and method arms of `print/value.rs`: `f32` as `f32` with Rust's operators and methods; `f16` as `rt::Float16`, `f128` as `rt::Float128`, `UInt<N>`/`Int<N>` over 64 as `rt::WideUInt<N, L>`/`rt::WideInt<N, L>`; literals from `FloatBits`/`Wide` as `rt::Float128::from_bits(0x…)`, `rt::WideUInt::<N, L>::from_limbs([…])`; `as` through `rt::Convert`; `analog` reads of these types with no tangent (QD4). Create `crates/piperine-transpile/tests/number_widths.rs` (hand-built IR, built by `rustc`, run).
**Where**: `crates/piperine-transpile/src/print/widths.rs`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `ValuePrinter`, `rt/widths.rs`
**Requirement**: CV-29, CV-30, CV-31, CV-32
**Executor**: Sonnet

**Done when**:

- [ ] `number_widths_compile_and_compute_as_their_library` passes under `**Run tw**` (one function per type with every operator, `as` pair and method, its results compared with Rust's `f32` and with `piperine-numbers` called in the test; a zero divisor panics naming it)
- [ ] Every transpile golden is byte-identical
- [ ] Gate check passes: Task (`piperine-transpile`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(transpile): print f16, f32, f128 and wide integers`

---

### T117: Elaborator: lowering the new widths

**What**: `elab/widths.rs` (`impl Elaboration`): the checker's types to `ValueType`; a float literal read from its raw text into its position's format by `DecimalText::nearest` (A.8 rule 8, never through `f64`); an integer literal of a wide type to a `Wide` constant; `as` and methods lowered with their operand types; carriers, params and record fields of the new types; their default values (`0`, `0.0`). Create `crates/piperine-lang/tests/elab_number_widths.rs`.
**Where**: `crates/piperine-lang/src/elab/widths.rs`
**Depends on**: T116
**Reuses**: `DecimalText`, the scalar lowering of `elab/`
**Requirement**: CV-29, CV-30, CV-31, CV-32
**Executor**: Sonnet

**Done when**:

- [ ] `number_widths_lower_to_their_ir_types_and_constants` passes under `**Run elabw**` (`0.1f32`, `0.1` as `f16` and as `f128` carry the bits Rust's `0.1f32` and the exact rational nearest give; a double-rounding literal is read right)
- [ ] Gate check passes: Task (`piperine-lang`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(lang): lower f16, f32, f128 and wide integers`

---

### T118: Const evaluation over every width

**What**: `elab/number_consts.rs` (`NumberValue`): the const evaluator's values of `f32` (Rust's `f32`), `f16`, `f128` and integers over 64 bits (`piperine-numbers`), dispatched from `consts.rs` by one arm; every operator, `as` and H.1/H.2 method in a const context gives the value it has at run time (CV-36 AC 1); integers over 64 bits use `WideUInt`/`WideInt`, fixing the `1i128 << bits` wrap (VD2); a panic (a zero divisor, `clamp` bounds) is an elaboration error naming the expression (CV-36 AC 2).
**Where**: `crates/piperine-lang/src/elab/number_consts.rs`
**Depends on**: T117
**Reuses**: `ConstEval`, `piperine-numbers`
**Requirement**: CV-36, CV-31, CV-32
**Executor**: Sonnet

**Done when**:

- [ ] `const_contexts_compute_every_width_as_the_run_does` passes under `**Run elabw**` (a `const`, a `const fn`, a param default and an array length over each width, compared with the same expression run by the generated code of T116 and with the oracles)
- [ ] `a_const_panic_of_a_new_width_names_the_expression` passes under `**Run elabw**`
- [ ] Gate check passes: Task (`piperine-lang`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(lang): const evaluation over every number width`

---

### T119: Source runs: `f32`

**What**: Models in `crates/piperine-parity/tests/number_widths/f32.msl` and `crates/piperine-parity/tests/source_number_widths.rs` (its helpers methods of a fixture type `WidthBench`): an `f32` table of `+ - * /`, comparisons, `as` from and to `f64` and integers, and every H.2 method, emitted and compared bit for bit with the same expressions evaluated by Rust's `f32` in the test (CV-29 AC 1); a `Bit<f32>` carrier whose pin is the `F32` leaf and whose committed value reads back (CV-29 AC 2).
**Where**: `crates/piperine-parity/tests/source_number_widths.rs`
**Depends on**: T118
**Reuses**: the source-level harness of `source_carrier_arrays.rs`
**Requirement**: CV-29
**Executor**: Sonnet

**Done when**:

- [ ] `f32_computes_what_rust_f32_computes` passes under `**Run srcw**`
- [ ] `a_carried_f32_is_the_f32_leaf` passes under `**Run srcw**`
- [ ] Gate check passes: Task (`piperine-parity`)

**Tests**: integration
**Gate**: task

**Commit**: `test(parity): f32 from source`

---

### T120: Source runs: `f16`

**What**: `number_widths/f16.msl`: an `f16` table of `+ - * /`, `sqrt`, comparisons, `as` conversions from and to `f32`, `f64`, `f128` and integers, and every H.2 method, emitted as bits and compared with a binary16 reference written in the test (`ExactRational` for the exact operations; for the transcendentals the `f128` reference vectors rounded to `f16` with the ambiguity guard of T109) (CV-30 AC 1); a `Bit<f16>` carrier reads back its bits.
**Where**: `crates/piperine-parity/tests/number_widths/f16.msl`
**Depends on**: T119
**Reuses**: `WidthBench`
**Requirement**: CV-30
**Executor**: Sonnet

**Done when**:

- [ ] `f16_is_ieee_binary16_bit_for_bit` passes under `**Run srcw**`
- [ ] Gate check passes: Task (`piperine-parity`)

**Tests**: integration
**Gate**: task

**Commit**: `test(parity): f16 from source`

---

### T121: Source runs: `f128`

**What**: `number_widths/f128.msl`: an `f128` table of arithmetic, comparisons, `as` conversions and **every** H.2 method on inputs drawn from the T99 vectors (at least 50 per method, the hard cases included), emitted as bits and compared bit for bit with the vectors and with `ExactRational` for the exact operations (CV-31 AC 1); a `Bit<f128>` carrier whose pin is a `U64` field of `count` 2 reads back its value.
**Where**: `crates/piperine-parity/tests/number_widths/f128.msl`
**Depends on**: T120
**Reuses**: `WidthBench`, `ReferenceVectors` (read from `crates/piperine-numbers/tests/vectors/`)
**Requirement**: CV-31
**Executor**: Sonnet

**Done when**:

- [ ] `f128_is_ieee_binary128_bit_for_bit_in_every_operation_and_method` passes under `**Run srcw**`
- [ ] Gate check passes: Task (`piperine-parity`), then the Batch gate

**Tests**: integration
**Gate**: batch

**Commit**: `test(parity): f128 from source, every H.2 method`

---

### T122: Source runs: wide integers

**What**: `number_widths/wide.msl`: `UInt<N>` and `Int<N>` for `N` = 65, 100, 128, 200, 1000 with every operator, shift, `as` and H.1 method, emitted and compared with `u128`/`i128` arithmetic masked to `N` bits in the test for `N <= 128` and with `num-bigint` modulo `2^N` above (CV-32 AC 1); a division by zero fails the analysis naming it (§1 rule 11); `let x: u200 = 0x1_0000…` past the type is `E0909` (CV-32 AC 3); a `Bit<UInt<200>>` carrier is one `U64` field of `count` 4.
**Where**: `crates/piperine-parity/tests/number_widths/wide.msl`
**Depends on**: None (its phase follows the previous one)
**Reuses**: `WidthBench`
**Requirement**: CV-32
**Executor**: Sonnet

**Done when**:

- [ ] `wide_integers_wrap_modulo_two_to_the_n_in_every_operation` passes under `**Run srcw**`
- [ ] `a_wide_literal_past_its_type_is_e0909` passes under `**Run srcw**`
- [ ] Gate check passes: Task (`piperine-parity`)

**Tests**: integration
**Gate**: task

**Commit**: `test(parity): wide integers from source`

---

### T123: Host: exact cells of every width

**What**: `crates/piperine-api/src/number_cell.rs`: `Cell::Float(FloatCell)` and `Cell::Wide(WideCell)` (appended), decoded from a carrier's, a param's and a record field's bytes by the IR type (the metadata, design-d.md "ABI"), also inside a `CompositeCell`; their text is `DecimalText::shortest` (floats, QD6) and the exact decimal (integers), JSON a number, the plot value the nearest `f64`; `param_text.rs` reads `--param` text of each new type (A.8 rule 8, `0x`/`0o`/`0b` for integers). Create `crates/piperine-api/tests/number_widths.rs`.
**Where**: `crates/piperine-api/src/number_cell.rs`
**Depends on**: T122
**Reuses**: `Cell`, `CompositeCell`, `param_text.rs`, `DecimalText`
**Requirement**: CV-32, CV-29, CV-30, CV-31
**Executor**: Sonnet

**Done when**:

- [ ] `the_host_reads_each_width_as_one_exact_value` passes under `**Run apiw**` (a `UInt<200>` of value `2^199 + 1` reads as that integer, never as limbs, CV-32 AC 4; `0.1f32`, `f16`, `f128` read with their shortest text; a param set by text reads back)
- [ ] Gate check passes: Task (`piperine-api`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(api): exact cells of f16, f32, f128 and wide integers`

---

### T124: CLI: the new widths in every output format

**What**: `piperine run` prints a carried and an emitted `f32`, `f16`, `f128` and `UInt<200>` in the text table, record lines, JSON Lines (a number), CSV and the plot (Q2 (c)); `--param Top.k=…` sets an `f128` and a `u200` param.
**Where**: `crates/piperine-cli/tests/run.rs`
**Depends on**: T123
**Reuses**: phase B's T62 rendering
**Requirement**: CV-32, CV-21
**Executor**: Sonnet

**Done when**:

- [ ] `number_widths_print_exactly_in_every_format` passes under `**Run clirun**`
- [ ] Gate check passes: Task (`piperine-cli`)

**Tests**: integration
**Gate**: task

**Commit**: `feat(cli): number widths in every output format`

---

### T125: The Book's status box

**What**: `docs/book/content.html:697`'s status box no longer lists "floats other than `f64`, and integers wider than 64 bits" (it keeps the item in a block and `%` on floats); `tests/book/language.rs::a_form_not_run_yet_is_stopped_before_the_run_with_p0001_naming_it` loses its `f32` and `u128` rows (design-d.md "may move"), and a new test runs the two models it refused (`let x: f32 = 1.5;`, `let x: u128 = 1;`) and reads `1.5` and `1`; `tests/book/inventory.tsv` follows the sentence's new id (CV-37 AC 2).
**Where**: `tests/book/language.rs`
**Depends on**: T124
**Reuses**: `ValueFile`, `Chapter`
**Requirement**: CV-37
**Executor**: Sonnet

**Done when**:

- [ ] `a_form_not_run_yet_is_stopped_before_the_run_with_p0001_naming_it` and `floats_of_every_width_and_wide_integers_run` pass under `**Run book**`, and `book_inventory` passes
- [ ] Gate check passes: Task (`piperine`)

**Tests**: integration
**Gate**: task

**Commit**: `test(book): floats of every width and wide integers run`

---

### T126: Phase D bookkeeping

**What**: `GAPS.md` drops L13 and updates §9 Coverage ("I §1–§2 values": floats other than `f64` and widths over 64 leave the "missing" text) and the counts line, with no residue (Q4 (a) everywhere); `spec.md`'s traceability marks CV-29 to CV-33 done (CV-30 AC 2, CV-31 AC 2–3 and CV-32 AC 2 are inapplicable: Q4 chose (a)); `.specs/STATE.md` MD-79 gains the phase D item of design-d.md "Tech Decisions"; `ROADMAP.md`'s phase D row says delivered; `.specs/features/ir-rust/spec.md` RS-Q6 gets a line "superseded by composite-values Q4 (2026-11-23)"; `CLAUDE.md`/`AGENTS.md` crate tables gain `piperine-numbers`.
**Where**: `GAPS.md`
**Depends on**: T125
**Reuses**: `docs.rs`
**Requirement**: CV-37
**Executor**: Sonnet

**Done when**:

- [ ] `gaps_drops_what_composite_values_phase_d_closed` passes under `**Run docs**`
- [ ] Gate check passes: Task (`piperine-cli`), then the Batch gate

**Tests**: integration
**Gate**: batch

**Commit**: `docs: composite-values phase D closes L13`

---

## Checks

CV-30 AC 2, CV-31 AC 2, CV-31 AC 3 and CV-32 AC 2 are inapplicable: their `WHERE` names a Q4 option the user did not choose (2026-11-23: (a) for every type). They get no check.

**C146** - `f32` literals, `as` conversions, arithmetic and every H.2 method give, bit for bit, the results Rust's `f32` gives for the same expressions. (CV-29, AC 1; T119)
Proof: run srcw | test f32_computes_what_rust_f32_computes ... ok

**C147** - `Bit<f32>` is carried as the ABI's `F32` leaf and reads back its committed value. (CV-29, AC 2; T119)
Proof: run srcw | test a_carried_f32_is_the_f32_leaf ... ok

**C148** - An `f32` where an `f64` is required (a contribution, a probe comparison, an `f64` parameter) is a type mismatch. (CV-29, AC 3; T115)
Proof: run typesw | test an_f32_where_an_f64_is_required_is_a_mismatch ... ok

**C149** - `f16` `+ - * /`, `sqrt`, comparisons, `as` conversions and the H.2 methods give IEEE 754 binary16 results bit for bit, from source. (CV-30, AC 1; T120)
Proof: run srcw | test f16_is_ieee_binary16_bit_for_bit ... ok

**C150** - Every unary `f16` operation is correctly rounded over all 65 536 inputs. (CV-30, AC 1, the library half; T95)
Proof: run half | test every_unary_f16_operation_is_correctly_rounded_over_all_inputs ... ok

**C151** - `f16` `+ - * /`, comparisons, `min`, `max`, `rem_euclid` and `hypot` are correctly rounded on every pair of special values and 2^22 random pairs. (CV-30, AC 1, the library half; T95)
Proof: run half | test binary_f16_operations_are_correctly_rounded ... ok

**C152** - Every `f16` transcendental is correctly rounded over all inputs (pairs for `powf`, `atan2`). (CV-30, AC 1 with H.2 rule 6, QD3; T109)
Proof: run transcendental | test every_f16_transcendental_is_correctly_rounded_over_all_inputs ... ok

**C153** - `f128` arithmetic, comparisons, `as` conversions and every H.2 method give IEEE 754 binary128 results bit for bit, from source. (CV-31, AC 1; T121)
Proof: run srcw | test f128_is_ieee_binary128_bit_for_bit_in_every_operation_and_method ... ok

**C154** - `f128` `+ - * /` and `sqrt` are correctly rounded. (CV-31, AC 1, the library half; T96)
Proof: run quad | test f128_arithmetic_and_square_root_are_correctly_rounded ... ok

**C155** - The exact H.2 methods and predicates on `f128` follow H.2 rules 1 to 5, 8 to 10. (CV-31, AC 1; T96)
Proof: run quad | test f128_exact_methods_follow_h2 ... ok

**C156** - Every `as` conversion among every float and integer width follows §1.4.5, rounded once. (CV-30 AC 1, CV-31 AC 1, CV-32 AC 1; T97)
Proof: run conversions | test every_as_conversion_follows_the_table_of_1_4_5 ... ok

**C157** - `exp`, `ln`, `log2`, `log10` on `f128` match the correctly rounded reference vectors bit for bit. (CV-31, AC 1; T104)
Proof: run transcendental | test exp_and_logarithms_match_the_reference_vectors_bit_for_bit ... ok

**C158** - `powf` and `powi` on `f128` match the reference vectors bit for bit, exact and midpoint cases included. (CV-31, AC 1; T105)
Proof: run transcendental | test powers_match_the_reference_vectors_bit_for_bit ... ok

**C159** - `sin`, `cos`, `tan` on `f128` match the reference vectors bit for bit, up to the largest finite argument. (CV-31, AC 1; T106)
Proof: run transcendental | test circular_functions_match_the_reference_vectors_bit_for_bit ... ok

**C160** - `asin`, `acos`, `atan`, `atan2` on `f128` match the reference vectors bit for bit. (CV-31, AC 1; T107)
Proof: run transcendental | test inverse_circular_functions_match_the_reference_vectors_bit_for_bit ... ok

**C161** - `sinh`, `cosh`, `tanh` on `f128` match the reference vectors bit for bit. (CV-31, AC 1; T108)
Proof: run transcendental | test hyperbolic_functions_match_the_reference_vectors_bit_for_bit ... ok

**C162** - The reference vectors exist for every H.2 transcendental and every expected value is a valid binary128. (CV-31, AC 1, the oracle; T99)
Proof: run transcendental | test the_reference_vectors_are_complete_and_well_formed ... ok

**C163** - `UInt<N>`/`Int<N>` with `N` over 64 wrap modulo `2^N` in every operation and implement the H.1 methods, from source. (CV-32, AC 1; T122)
Proof: run srcw | test wide_integers_wrap_modulo_two_to_the_n_in_every_operation ... ok

**C164** - Wide integers of 65 to 128 bits agree with `u128`/`i128` arithmetic masked to `N` bits. (CV-32, AC 1, the library half; T93)
Proof: run integers | test wide_integers_up_to_128_bits_agree_with_u128_and_i128 ... ok

**C165** - Wide integers above 128 bits agree with big-integer arithmetic modulo `2^N`. (CV-32, AC 1, the library half; T93)
Proof: run integers | test wide_integers_above_128_bits_agree_with_big_integers_modulo_two_to_the_n ... ok

**C166** - A literal that does not fit its type is `E0909`, for every width. (CV-32, AC 3; T115)
Proof: run typesw | test a_literal_out_of_its_type_is_e0909 ... ok

**C167** - A wide literal past its type is `E0909` from source. (CV-32, AC 3; T122)
Proof: run srcw | test a_wide_literal_past_its_type_is_e0909 ... ok

**C168** - A carried or printed wide integer gives the host its exact value (`2^199 + 1`), not its limbs. (CV-32, AC 4; T123)
Proof: run apiw | test the_host_reads_each_width_as_one_exact_value ... ok

**C169** - Every output format prints each new width exactly. (CV-32, AC 4, the CLI half; T124)
Proof: run clirun | test number_widths_print_exactly_in_every_format ... ok

**C170** - An integer width not a const expression `>= 1` (or past C.2's limit), or a float width outside {16, 32, 64, 128}, is `E0103`. (CV-33, AC 1; T115)
Proof: run typesw | test a_width_outside_the_rules_is_e0103 ... ok

**C171** - `f16`, `f32`, `f128` and integers of any width (a const-expression width included) type-check. (CV-33 AC 1 accepted side, VD1; T115)
Proof: run typesw | test every_float_and_integer_width_type_checks ... ok

**C172** - The interpreter refuses each new width by name. (CV-34, AC 1; T111)
Proof: run interp | test each_number_width_is_refused_by_name ... ok

**C173** - Const contexts compute every width as the run does. (CV-36, AC 1; T118)
Proof: run elabw | test const_contexts_compute_every_width_as_the_run_does ... ok

**C174** - A const panic of a new width is an elaboration error naming the expression. (CV-36, AC 2; T118)
Proof: run elabw | test a_const_panic_of_a_new_width_names_the_expression ... ok

**C175** - Float literals of every width are read into their format directly (A.8 rule 8) and lower to their IR constants. (CV-29 to CV-32, AC 1, the elaboration half; T117)
Proof: run elabw | test number_widths_lower_to_their_ir_types_and_constants ... ok

**C176** - `piperine-numbers` is embeddable runtime source: no dependency, no `unsafe`, no macro, no panic, siblings only through `super`. (MD-63 item 1, CV-35; T91)
Proof: run purity | test the_numeric_crate_is_embeddable_source ... ok

**C177** - Limb arithmetic agrees with big integers. (CV-32, AC 1, the engine; T92)
Proof: run integers | test limbs_agree_with_big_integers_on_every_operation ... ok

**C178** - Rounding to every IEEE 754 format is nearest, ties to even, with subnormals and overflow. (CV-30, CV-31, AC 1, the engine; T94)
Proof: run formats | test rounding_to_every_format_is_nearest_ties_to_even ... ok

**C179** - Decimal text reads into the nearest value of each format and the shortest text reads back. (CV-29 to CV-32, A.8 rule 8, QD6; T98)
Proof: run formats | test decimal_text_reads_to_the_nearest_value_and_prints_the_shortest ... ok

**C180** - Multi-precision directed rounding brackets the exact result. (CV-31, AC 1, the engine; T100)
Proof: run precision | test directed_rounding_brackets_the_exact_result ... ok

**C181** - π, ln 2 and ln 10 contain their published 1 000-digit values. (CV-31, AC 1, the engine; T101)
Proof: run precision | test reference_constants_contain_their_published_digits ... ok

**C182** - Argument reduction keeps full precision for every finite `f128`. (CV-31, AC 1, the engine; T102)
Proof: run precision | test reduction_keeps_full_precision_for_every_finite_argument ... ok

**C183** - The Ziv loop raises precision until the bounds agree and ends on an exact case. (CV-31, AC 1, the engine; T103)
Proof: run precision | test ziv_rounding_raises_precision_until_the_bounds_agree ... ok

**C184** - The IR checks, prints and serializes the new types and constants; an IR without them is byte-identical. (CV-29 to CV-32; T111)
Proof: run irw | test number_width_types_and_constants_are_checked_printed_and_serialized ... ok

**C185** - Each new scalar has its ABI leaf, `count` and bytes, alone and in composites, with equal layouts per type. (CV-29 AC 2, CV-32 AC 4, the plan half; T112)
Proof: run planw | test each_new_scalar_has_its_leaf_count_and_bytes ... ok

**C186** - A design that uses no new width gets today's runtime files and cache key. (CV-35, AC 1; T113)
Proof: run tw | test a_design_without_number_widths_gets_todays_runtime_files ... ok

**C187** - The printed Rust of each width compiles with `rustc` and computes what its library computes; a zero divisor panics naming it. (CV-29 to CV-32, the printer half; T116)
Proof: run tw | test number_widths_compile_and_compute_as_their_library ... ok

**C188** - The Book's status box no longer lists floats and wide integers; the models it refused run. (CV-37, AC 2; T125)
Proof: run book | test a_form_not_run_yet_is_stopped_before_the_run_with_p0001_naming_it ... ok
Proof: run book | test floats_of_every_width_and_wide_integers_run ... ok

**C189** - `GAPS.md` no longer lists L13 and its §9 coverage and counts follow, with no residue. (CV-37, AC 1; T126)
Proof: run docs | test gaps_drops_what_composite_values_phase_d_closed ... ok

**C190** - The twins print the bytes they printed before phase D, with `ToleranceContract` unedited. (CV-35, AC 1; T126)
Proof: `cargo test -p piperine-cli --test twins`

**C192** - The runtime raises §1 rule 11's panics for wide integers and `clamp`, and carries every new width's bytes and levels (§2). (CV-32 AC 1, CV-29 AC 2, the runtime half; T113)
Proof: run rtw | test widths::tests::the_runtime_raises_msl_panics_and_carries_every_width ... ok

**C191** - The build has no warning. (CV-35, AC 2; T126)
Proof: `cargo build -p piperine-numbers -p piperine-abi -p piperine-digital-solver -p piperine-analog-solver -p piperine-solver -p piperine-ir -p piperine-interp -p piperine-plan -p piperine-transpile -p piperine-lang -p piperine-api -p piperine-cli -p piperine-parity -p piperine --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`

---

## Handoff

The feature's live state, written as it happens - a compaction gives no warning.

**Settled mid-build:** Planning (2026-11-23): Q4 answered by the user directly (integers (a), `f16` (a), `f128` (a) complete, recorded in spec.md). design-d.md QD1 to QD8 are the planner's defaults, pending the user's approval of these tasks. D1 to D3 (T91 to T109) may run beside phase C; T110 waits for phase C's merge.
**Defaults and findings (D1 to D3, numeric crate):**
- T102: ArgumentReduction::quarter_turns raises its working precision (precision + exponent + 160, doubling) until the remainder interval is relatively narrow; ln_two_steps takes an Interval (so exp can reduce y*ln x) and refuses values above 2^60. ReferenceConstants quantizes the precision up to a multiple of 256 so nearby requests share one cached entry. The reduction test checks the interval against a 20 000-bit pi (floor from pi_bits.hex, so the oracle remainder is exact within |n| * 2^-20001), skipping only arguments within 2^-200 of a quarter turn where the quadrant is ambiguous; ln 2 is checked against the 1000-decimal value.
- T101: the constants' tests compare with the committed 1000-decimal values (truncated, from mpmath, cross-checked inside generate.py against the well-known leading digits of OEIS A000796, A002162, A002392; the digits were not typed from memory beyond those prefixes) as consistency intervals [T, T + 10^-1000] since the series interval at 3 300 bits is narrower than the 1000th decimal; the returned interval is carried at precision + 32 guard bits, so it is far narrower than two units in the last place. A static OnceLock<Mutex<HashMap>> caches per (constant, precision).
- T100: Interval ops take their working precision from the interval; a sticky bit is valid only on a magnitude longer than the precision (documented on MultiPrecision::rounded; add pads the larger operand to precision+3 bits before using the far-operand shortcut, which found a bug the directed-rounding test caught). The tests' shared fixtures were split into small files (tests/support/exact*.rs, one concern each, included per target with #[path]) so no target carries unused code.
- T99: the vectors have 1 300 to 2 700 rows per function (the plan said about 2 000; the test requires at least 1 000), computed by tests/vectors/generate.py with mpmath 1.3.0 at 1 024 bits (Ziv loop on the oracle side, margin 2^-984 relative) and exact Fraction rounding; exact.tsv holds 1 180 exact cases. Extra data files: constants.txt (pi, ln 2, ln 10 to 1 000 decimals, asserted against their well-known leading digits inside the generator), pi_bits.hex (floor(pi * 2^20000)), half_pi.tsv (the f128 nearest to k*pi/2, k odd up to 16 370 bits, hex); the sin/cos/tan sets also contain 300 such arguments each. powi rows carry the i32 exponent in decimal. Special-value rules are those of C99 Annex F / Rust: powf(x, +-0) = 1, powf(1, y) = 1, powi(x, 0) = 1 for any x, atan2 with signed zeros and infinities, min(-0,+0) not involved.
- T98: DecimalText also reads inf, -inf, NaN (any case) so the host's text reads back; shortest() is plain notation for decimal exponents -5..=16 and d.ddde<x> outside it, shortest_scientific() is always Rust's {:e} form (the tests compare it with Rust's own f32/f64 output); a tie between two equally close shortest candidates goes up, as Rust does. The shortest search is exact (one big division, then 36-digit integers, binary search over the digit count). D1 (T91 to T98) closed; per the gate policy no workspace suite was run, only the Build line and cargo test -p piperine-numbers.
- T97: float to integer maps a float whose magnitude passes the target to the range edge and truncates the rest exactly; the double-rounding case is 1 + 2^-11 + 2^-40 (f64 to f16 once gives 0x3C01, through f32 gives 0x3C00).
- T96: the f128 operand generator mixes random bits with exponents near the anchor, sparse significands (ties), subnormals and values near overflow; hypot and rem_euclid are checked on one random pair in four (1 000 pairs) plus every special pair and the largest finite value. All exact operations are IeeeArithmetic, tested against ExactOracle (independent code).
- Root Cargo.toml gains [profile.dev.package.{piperine-numbers,num-bigint,num-integer,num-traits}] opt-level = 3 so the numeric tests (all 65 536 f16 inputs, 2^22 operand pairs against big-integer arithmetic) run in a dev build in about a minute, not hours; no result depends on it. T95: rem_euclid and hypot are checked on one random pair in eight (2^19 pairs), the other operations on all 2^22. Hypot, like rem_euclid, is the exact IeeeArithmetic (not f64), so it is correctly rounded by construction. IeeeArithmetic (src/arithmetic.rs, exact operations of any BinaryFormat) is a file the design table did not list: Float16 and Float128 delegate their exact methods to it. Decisions the spec left open, followed by code and oracle alike: min(-0,+0) = -0 and max = +0; rem_euclid with an infinite divisor leaves a non-negative x and gives +inf for a negative x (Rust's), and an exact zero remainder keeps the sign of x; fract(x) = x - trunc(x) in IEEE terms (+0 for an integer, NaN for an infinity).
- Build gate: the Build line already prints one warning on main before this work (dead code 'run' in root test tests/book/types.rs); it is not ours and is untouched; the gate used here is zero warnings and zero errors from piperine-numbers plus no error anywhere.
- T91 anchors re-read on main (52b2223c): all found. Differences: check.rs sized_type is at 460-475 and integer_type 505-515 (a GenericInteger{signed,width} Ty variant and ConstTerm already exist, so D4's checker widening must carry it); catalogue.rs keys F64_METHODS/INTEGER_METHODS at 69/105 and dispatch at 140-141 (INTEGER_METHODS also covers Ty::GenericInteger); consts.rs wrap is 1i128 << bits at 918/933/942; model.rs ValueType at 906 and Constant at 1034; rt.rs Runtime::files(digital: bool) at 58; Cell at table.rs:19; rust_type refusal at print/value.rs ~605; types check at ir check.rs 773. No tasks-d.md approval line exists; the caller authorized execution.

**Dispatched:** none yet.
**Walked:** none (no screen; every criterion is proven by a test).

---

## Phase Execution Map

```
Phase 24 → Phase 25 → … → Phase 30

Phase 24: T91
Phase 25: T92 -----→ T93 -----→ T94 -----→ T95 -----→ T96 -----→ T97 -----→ T98
Phase 26: T99 -----→ T100 ----→ T101 ----→ T102 ----→ T103 ----→ T104
Phase 27: T105 ----→ T106 ----→ T107 ----→ T108 ----→ T109
Phase 28: T110 ----→ T111 ----→ T112 ----→ T113 ----→ T114 ----→ T115
Phase 29: T116 ----→ T117 ----→ T118 ----→ T119 ----→ T120 ----→ T121
Phase 30: T122 ----→ T123 ----→ T124 ----→ T125 ----→ T126
```

Execution is strictly sequential - there is no intra-phase parallelism. A single agent (or batch worker) works one task at a time, in order.

**Batches** (~7 tasks, whole phases, every worker Sonnet; the Batch gate runs once at each batch's end):

| Batch | Phases | Tasks | Notes |
| ----- | ------ | ----- | ----- |
| D1 | 24, 25 | T91 to T98 | the crate, exact integers and floats, conversions, decimal text; may run beside phase C |
| D2 | 26 | T99 to T104 | reference vectors, the multi-precision engine, `exp` and logarithms; may run beside phase C |
| D3 | 27 | T105 to T109 | the remaining `f128` transcendentals and the `f16` ones; may run beside phase C |
| D4 | 28 | T110 to T115 | starts only when phase C is on `main`; IR, plan, runtime, checker |
| D5 | 29 | T116 to T121 | printer, elaborator, const evaluation, `f32`/`f16`/`f128` from source |
| D6 | 30 | T122 to T126 | wide integers from source, host, CLI, Book, closure; closes L13 |
