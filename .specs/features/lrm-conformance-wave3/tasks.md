# Implementation ↔ Specification Gap Closure (wave 3) — Tasks

## Execution Protocol (MANDATORY — do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name
and follow its Execute flow and Critical Rules.** Do not search for skill files
by filesystem path. The skill is the source of truth for the full flow
(per-task cycle, sub-agent delegation, adequacy review, Verifier,
discrimination sensor).

**If the skill cannot be activated, STOP and tell the user.**

---

**Spec**: `.specs/features/lrm-conformance-wave3/spec.md`
**Design**: `.specs/features/lrm-conformance-wave3/design.md`
**Status**: Draft (awaiting approval)

---

## Test Coverage Matrix

> Generated from the codebase, the project's own quality guidelines, and the
> spec. Guidelines found: `CLAUDE.md` ("Where the tests are", fail-loud rule),
> `AGENTS.md` (hard rules, test placement table), `.specs/STATE.md` MD-13
> (Rust idiom rules), MD-28 (test placement), MD-31 (a policy invariant lives
> in the gate), MD-51 (conformance declared at the test), and
> `tests/suite_hygiene.rs` (the mechanical enforcement: no switched-off tests,
> no `#[ignore]`, every target carries a `//!` scope header, no file-scope lint
> suppression).

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
|---|---|---|---|---|
| Grammar / lexer / preprocessor (`piperine-lang/src/{grammar,lexer.rs,preprocessor.rs}`) | integration | Every production the task adds: the LRM's own worked example where one exists, plus every listed edge case, plus one loud-failure case per rejected form. Declares `//! LRM-COVERS:` | `crates/piperine-lang/tests/*.rs` | `cargo test -p piperine-lang --test <target>` |
| Elaboration (`piperine-lang/src/elab/`, `pom/`) | integration | 1:1 with the spec ACs it implements; every error path asserted by message content (clause cited), never by `is_err()` alone | `crates/piperine-lang/tests/*.rs` | `cargo test -p piperine-lang --test <target>` |
| Codegen resolve/flatten/emit/kernel (`piperine-codegen/src/`) | integration | Compiled-behavior proof, not elaboration-only: the kernel/device produces the asserted numeric or structural outcome. Pure helpers get an inline `#[cfg(test)]` module | `crates/piperine-codegen/tests/*.rs` | `cargo test -p piperine-codegen --test <target>` |
| Solver (`piperine-solver/src/`) | unit + integration | Unit for pure helpers (inline `#[cfg(test)]`); integration for anything that stamps, schedules, or reports. Failure rules land in `failure_rules.rs` and its §16 row | `crates/piperine-solver/src/**` (inline), `crates/piperine-solver/tests/*.rs` | `cargo test -p piperine-solver` |
| Host surface / cross-crate (`piperine-api`, `piperine-python`, root shell) | integration | Both hosts where the surface is shared (MD-22); a Rust-only surface declares the asymmetry and the parity test asserts the declaration | root `tests/*.rs`, `crates/piperine-api/tests/*.rs`, `crates/piperine-python/tests/*.rs` | `cargo test -p piperine <target>` |
| Gates / policy invariants (`tests/lrm_section_coverage.rs`, the new `tests/spec_section_coverage.rs`, `tests/suite_hygiene.rs`) | integration | The gate must be **proven able to fail** (a temporary mutation turns it red, reverted; recorded in the task's commit body) — MD-31's standard | root `tests/*.rs` | `cargo test -p piperine --test <target>` |
| Docs (`docs/spec/*.md`, `README.md`) | none | Build gate only — but a claim added to `docs/spec` without a `SPEC-COVERS` verdict fails the axis-B gate from Phase 7 onward | — | gate only |

## Gate Check Commands

| Gate Level | When to Use | Command |
|---|---|---|
| Quick | After a task touching one crate with one target | `cargo test -p <crate> --test <target>` |
| Full | After a task whose behavior crosses crates (codegen→solver, host) | `cargo test -p <crate>` for each crate touched, plus `cargo test -p piperine <target>` for root targets |
| Build | End of every phase, and after any task touching a gate | `cargo build --workspace && cargo test --workspace` (zero warnings is the bar) |

**Baseline discipline (G5):** capture a per-target pass/fail baseline before
Phase 1 (`cargo test --workspace --no-fail-fast 2>&1 | tee baseline.log`) and
A/B **target by target** at every phase boundary — never an aggregate count.
The known-red target at baseline is `tests/lrm_section_coverage.rs`
(`covered_and_excluded_partition_the_lrm`, 105 uncovered).

---

## Execution Plan

Phases run sequentially; tasks run in order inside a phase.

### Phase 1 — Vector foundation (6 tasks)
```
T1 → T2 → T3 → T4 → T5 → T6
```

### Phase 2 — Vector access (8 tasks)
```
T7 → T8 → T9 → T10 → T11 → T12 → T13 → T14
```

### Phase 3 — specify + config (8 tasks)
```
T15 → T16 → T17 → T18 → T19 → T20 → T21 → T22
```

### Phase 4 — Discipline, nature, attributes (6 tasks)
```
T23 → T24 → T25 → T26 → T27 → T28
```

### Phase 5 — Branches, contributions, restrictions (7 tasks)
```
T29 → T30 → T31 → T32 → T33 → T34 → T35
```

### Phase 6 — Axis-A coverage backfill and gate closure (8 tasks)
```
T36 → T37 → T38 → T39 → T40 → T41 → T42 → T43
```

### Phase 7 — Axis-B gate (5 tasks)
```
T44 → T45 → T46 → T47 → T48
```

### Phase 8 — Axis-B gap closure (7 tasks)
```
T49 → T50 → T51 → T52 → T53 → T54 → T55
```

---

## Task Breakdown

### T1: Leading range on net declarations

**What**: `parse_net_decl` accepts `discipline [msb:lsb] name, …;` and applies
the range to every declarator that carries none of its own.
**Where**: `crates/piperine-lang/src/grammar/item.rs`
**Depends on**: None
**Reuses**: `parse_range` (already used by `parse_ground_decl`), `Declarator`,
`declarator_net_type`
**Requirement**: LRMW3-01

**Tools**: MCP NONE · Skill `tlc-spec-driven`

**Done when**:
- [x] `electrical [0:3] bus;` and `electrical [3:0] a, b;` elaborate to
      `NetType::Array` of width 4 for every declared name
- [x] A declarator carrying **both** a leading and its own range is a loud
      error naming both spans
- [x] Reversed (`[0:3]`) and descending (`[3:0]`) ranges both give width 4 and
      preserve the declared bit order in the POM (order itself is not yet a
      distinct field on `NetType::Array` — width symmetry is what's provable
      at this layer; full bit-order observability lands with T4/T7's
      element-node naming)
- [x] Non-constant bounds fail loud citing §3.6.3
- [x] New target declares `//! LRM-COVERS: 3.6.3, A.2.1.2`
- [x] Gate passes: `cargo test -p piperine-lang --test vams_vector_declarations`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): accept a leading range on net declarations (§3.6.3)`

---

### T2: Leading range on port declarations

**What**: same range slot on `input`/`output`/`inout` declarations
(`input [0:width-1] in;`), including the discipline-bearing form.
**Where**: `crates/piperine-lang/src/grammar/item.rs::parse_port_decl`
**Depends on**: T1
**Reuses**: T1's helper
**Requirement**: LRMW3-01, LRMW3-05

**Done when**:
- [x] `input [0:7] p;` and `inout electrical [3:0] bus;` elaborate to a
      width-8/4 port (the bare `input` fixture adds `wire` since an
      undisciplined, unconnected port fails Annex F resolution regardless
      of range — confirmed pre-existing/unrelated to this task)
- [x] `Port::net_type().width()` reports the declared width
- [x] The port-list form and the body-redeclaration form agree
- [x] Declares `//! LRM-COVERS: 3.7, A.2.1.2`
- [x] Gate passes: `cargo test -p piperine-lang --test vams_vector_declarations`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): accept a leading range on port declarations`

---

### T3: Leading range on 1364 net and variable declarations

**What**: `reg [3:0] q;`, `wire [7:0] w;`, `tri [1:0] t;`, `integer [3:0]`-shaped
declarations route through the same helper.
**Where**: `crates/piperine-lang/src/grammar/item.rs::{parse_net_decl,parse_var_decl}`
**Depends on**: T1
**Reuses**: `DigitalNetKind::from_keyword`, `declarator_net_type`
**Requirement**: LRMW3-01

**Done when**:
- [x] `reg [3:0] q;` / `wire [7:0] w;` elaborate to
      `NetType::Array(Digital1364(_), N)` (already carried through parse_net_decl
      by T1, unchanged here — same discipline-slot dispatch reg/wire already use)
- [x] A `reg` vector and an `electrical` vector of the same width produce the
      same node count with the correct domains
- [x] Declares `//! LRM-COVERS: A.2.2.1`
- [x] Gate passes: `cargo test -p piperine-lang --test vams_vector_declarations`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): accept a leading range on 1364 net/variable declarations`

---

### T4: One owner for element-node naming

**What**: extract the `"{name}[{i}]"` element-node format into a single helper
both `structure.rs` and `device/circuit.rs::resolve_node` call, and make the
element loop recursive over nested `NetType::Array`.
**Where**: `crates/piperine-codegen/src/resolve/pom/structure.rs`,
`crates/piperine-codegen/src/device/circuit.rs`
**Depends on**: T1
**Reuses**: the existing element loop (`structure.rs:117`) and
`resolve_node`'s indexed lookup
**Requirement**: LRMW3-01, LRMW3-04

**Done when**:
- [x] One function owns the format; both call sites use it (`element_node_name`
      in `structure.rs`, called by the registration loop and by
      `device::circuit::resolve_element_node`)
- [x] A 1-D vector yields `bus[0]`..`bus[N-1]`; a nested array yields
      `mem[d][b]` (proven directly against a synthetic nested `NetType::Array`
      — T5's grammar doesn't exist yet to produce one from source)
- [x] Round-trip test: every generated name resolves back through
      `resolve_node` to the node it was generated for
- [x] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: full
**Commit**: `refactor(codegen): one owner for vector element-node naming`

---

### T5: Memories — a second (depth) range

**What**: `reg [3:0] mem [0:7];` parses (`Declarator.depth_range`) and lowers to
`NetType::Array(Array(base, width), depth)`.
**Where**: `crates/piperine-lang/src/{grammar/item.rs,ast/item.rs,elab/lower/vams.rs}`
**Depends on**: T4
**Reuses**: `NetType::Array` nesting, `NetType::width()`'s product rule, T4's
naming helper
**Requirement**: LRMW3-04

**Done when**:
- [x] `reg [3:0] mem [0:7];` elaborates to depth 8 × width 4 and registers 32
      element nodes named `mem[d][b]` (element-node registration itself was
      already proven generically in T4's nested-array test; this task proves
      the grammar/elaboration side reaches that same `NetType::Array` shape)
- [x] `width()` reports 32
- [x] A memory declared with only a depth (`reg mem [0:7];`) works
- [x] A width×depth product above the configured cap fails loud naming the
      declaration and the cap (design risk row) — cap is `MAX_MEMORY_ELEMENTS
      = 65_536` in `elab/lower/vams.rs`
- [x] Declares `//! LRM-COVERS: A.2.2.1`
- [x] Gate passes: `cargo test -p piperine-lang --test vams_vector_declarations`
      and `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): memories — packed element range plus unpacked depth`

---

### T6: Vector bounds and index-form diagnostics

**What**: the loud-error family for vectors: out-of-range index, non-constant
index, zero/negative width, index applied to a scalar.
**Where**: `crates/piperine-lang/src/elab/lower/vams.rs`,
`crates/piperine-codegen/src/resolve/pom/expr.rs`
**Depends on**: T5
**Reuses**: the const evaluator, `ElabErrorKind`
**Requirement**: LRMW3-01, spec Edge Cases

**Done when**:
- [x] `bus[9]` on a `[0:3]` net fails loud naming net, index and bounds
      (via the port-connection `name[index]` path, `expr_to_net_ref`/
      `validate_indexed_net_refs` — the only indexing surface that already
      resolves; analog/digital terminal indexing is T7/T9's own resolution
      work, not duplicated here)
- [x] A run-time-varying index fails loud citing §5.5.2's constant-expression rule
- [x] `scalar[0]` fails loud naming the scalar
- [x] A zero-width or malformed range fails loud (the width formula
      `|msb-lsb|+1` can never itself yield 0 — proven by a pinning test
      instead of a fabricated case; a non-integer bound already fails loud
      via the pre-existing const-eval type check)
- [x] Each error asserted by **message content**, not `is_err()`
- [x] Gate passes: `cargo test -p piperine-lang --test vams_vector_declarations`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): loud diagnostics for vector bounds and index forms`

---

### T7: Analog access through a constant index

**What**: replace `ident_from_expr` with `terminal_name_of`, resolving
`Expr::Index{base: Ident, index: const}` to the element node name.
**Where**: `crates/piperine-codegen/src/resolve/pom/expr.rs`
**Depends on**: T4
**Reuses**: T4's naming helper, the const environment already threaded for
genvars
**Requirement**: LRMW3-02

**Done when**:
- [x] `I(bus[0], g) <+ V(bus[0], g)/1e3;` compiles and stamps **that bit's**
      node only (asserted through a DC solve, not through elaboration)
- [x] `V(bus[3], bus[1])` resolves both terminals
- [x] A non-constant index still fails loud (T6's rule, re-asserted here so
      widening cannot silently swallow it)
- [x] Declares `//! LRM-COVERS: 5.5.2`
- [x] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): resolve analog terminals through a constant index`

---

### T8: Genvar-indexed vector access in an analog loop

**What**: the LRM §5.5.2 N-bit DAC example compiles and solves.
**Where**: `crates/piperine-codegen/src/flatten/analog.rs` (unroll environment
threading), test in `crates/piperine-codegen/tests/`
**Depends on**: T7
**Reuses**: `unroll_loop` (`flatten/analog.rs:703`)
**Requirement**: LRMW3-02

**Done when**:
- [x] §5.5.2's own `dac` example (verbatim from the LRM) elaborates, compiles
      and produces the analytic weighted sum in a transient run (adapted per
      `analog_loop_unroll.rs`'s own precedent: literal bound instead of a
      `parameter` bound — see commit body)
- [x] Each unrolled iteration resolves to its own bit (asserted by reading the
      per-bit node values, not by the summed output alone)
- [x] Declares `//! LRM-COVERS: 5.5.2`
- [x] Gate passes: `cargo test -p piperine-codegen` and `cargo test -p piperine-lang`
      (this task's actual fix landed in `piperine-lang`'s elaborator, not
      `flatten/analog.rs` — see commit body)

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): genvar-indexed vector access in analog loops`

---

### T9: Digital bit-select emission

**What**: `q[2]` in a digital expression resolves to the `q[2]` net
(scalarized, MD-52).
**Where**: `crates/piperine-codegen/src/emit/{digital_expr.rs,resolver.rs}`
**Depends on**: T3, T4
**Reuses**: the existing `Quad` net path; the `unsupported` arm at
`digital_expr.rs:79` is narrowed, not deleted
**Requirement**: LRMW3-01, LRMW3-03

**Done when**:
- [x] `always @(posedge clk) o = q[2];` compiles and propagates that bit
- [x] Reading a bit of an undriven vector yields `X` per Table 7-3, not a
      silent `0`
- [x] Anything the arm still cannot lower keeps failing loud
- [x] Declares `//! LRM-COVERS: 4.2.1`
- [x] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): digital bit-select emission`

---

### T10: Digital part-select emission

**What**: `b[7:4]` resolves to an ordered chunk list.
**Where**: `crates/piperine-codegen/src/emit/digital_expr.rs`
**Depends on**: T9
**Reuses**: T9's bit resolution
**Requirement**: LRMW3-03

**Done when**:
- [x] `n = b[7:4];` assigns the four selected bits in declared order
- [x] A descending and an ascending part-select of the same bits agree bit for
      bit
- [x] An out-of-range part-select fails loud
- [x] Declares `//! LRM-COVERS: 4.2.1`
- [x] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): digital part-select emission`

---

### T11: Whole-vector read and write

**What**: `q <= 4'b0101;` and `w = q;` expand to per-bit operations in the
region the assignment already selected.
**Where**: `crates/piperine-codegen/src/emit/{stmt.rs,digital_expr.rs}`
**Depends on**: T10
**Reuses**: the Active/Nba region split (T23 of wave 2), `DriverTable`
**Requirement**: LRMW3-03

**Done when**:
- [x] `q <= 4'b0101;` sets the four bits per IEEE 1364 bit order
- [x] A non-blocking whole-vector swap (`a <= b; b <= a;`) swaps correctly —
      the region semantics survive scalarization
- [x] A width mismatch on assignment truncates/zero-extends per 1364 and the
      behavior is asserted (not left implicit)
- [x] Declares `//! LRM-COVERS: 4.2.1.3`
- [x] Gate passes: `cargo test -p piperine-codegen` and
      `cargo test -p piperine-solver`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): whole-vector digital read/write`

---

### T12: Memory read and write

**What**: `mem[a] <= d;` / `d = mem[a];` for a declared memory.
**Where**: `crates/piperine-codegen/src/emit/{digital_expr.rs,stmt.rs}`
**Depends on**: T5, T11
**Reuses**: T4's nested naming, T11's per-bit expansion
**Requirement**: LRMW3-04

**Done when**:
- [x] A word written at index `d` reads back at index `d` and leaves the other
      words untouched (asserted per word, not per memory)
- [x] A constant index is required; a non-constant index fails loud
- [ ] `$readmemh` into a declared memory populates it (reuses the existing
      task implementation) — **BLOCKED, see commit body**: verified there is
      no existing codegen wiring to reuse. `SystemTaskKind::Readmemh` is a
      recognized enum variant (`piperine-lang/src/eval/tasks.rs`) and
      `FileTable::readmem` (`piperine-solver/src/core/output.rs`) can parse a
      hex/binary text file into a flat `&mut [u64]`, but
      `emit/stmt.rs::S::TaskCall`'s dispatch has no arm for
      `"readmemh"`/`"readmemb"` at all — it falls to
      `other => Err(CodegenError::unknown_task(other))`. Wiring it needs a
      new JIT trampoline (file-argument resolution, a `FileTable::readmem`
      call, then unpacking each returned `u64` word into that word's
      per-bit scalarized nodes) — a materially larger feature than "reuse an
      existing implementation," out of this task's remaining scope.
- [x] Declares `//! LRM-COVERS: A.2.2.1`
- [x] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): memory read/write through a constant index`

---

### T13: Vector port binding and width checking

**What**: connecting a vector net to a vector port binds bit for bit; a width
mismatch fails loud citing §6.5.7.1.
**Where (as executed — wider than originally scoped)**: §6.5.7.1's own
width-mismatch fail-loud path (`CheckPortSizes`,
`crates/piperine-lang/src/elab/lower/passes.rs`) was already implemented and
needed no change. The "bind bit for bit" half turned out to be a
`piperine-codegen`-only gap, not a `piperine-lang` one:
`resolve/pom/structure.rs` (port registration), `resolve/mod.rs` (`Port`
shape), `resolve/pom/body.rs` (the `label.port` hierarchical-access map),
`device/builder.rs` (per-instance connection resolution + digital/A2D
bridging), `emit/analog_expr.rs` (`$port_connected`), and
`kernel/analog/compile.rs` (terminal ordering).
**Depends on**: T2, T4
**Reuses**: `CheckPortSizes` (already implemented), `NetRef{net,index}`,
`inline_sub_instances`' broadcast rule, `register_element_nodes`/
`element_node_name` (the wire-array registration/naming `structure.rs`
already had)
**Requirement**: LRMW3-05

**Done when**:
- [x] An 8-bit net on an 8-bit port connects bit for bit through the hierarchy
      (asserted by solving and reading a leaf-side bit)
- [x] A 4-bit net on an 8-bit port fails loud citing §6.5.7.1
- [x] The existing array-of-instances broadcast rule is unchanged (regression
      test kept green, named in the commit body)
- [x] Declares `//! LRM-COVERS: 6.5.7.1, 7.2.3`
- [x] Gate passes: `cargo test -p piperine-lang` and `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): bit-for-bit vector port binding with width checking`

---

### T14: PLA modeling tasks (§9.8)

**What**: `$async$and$array` and family load a plane from a declared memory and
drive the outputs — or, if the family cannot be lowered faithfully, each name
fails loud citing §9.8. (The spec's P3 AC2 allows either outcome; the choice is
made **in this task**, with the reason recorded in the commit body.)
**Where**: `crates/piperine-lang/src/eval/tasks.rs`,
`crates/piperine-codegen/src/emit/stmt.rs`
**Depends on**: T12
**Reuses**: the memory representation from T5/T12, the shared unknown-task loud
helper
**Requirement**: LRMW3-20

**Done when**:
- [ ] Either: the sync/async × and/or/nand/nor array tasks drive their outputs
      from a loaded plane, asserted against IEEE 1364's own example
- [x] Or: every name in the family fails loud citing §9.8, with the reason
      recorded in the commit body and in `docs/spec/part_iii_conformance.md`
      (chosen — see commit body)
- [x] Declares `//! LRM-COVERS: 9.8`
- [x] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): PLA modeling tasks (§9.8)`

---

### T15: `specify` grammar — specparams and simple paths

**What**: replace the token-skipping stub with real productions for
`specparam` declarations and simple path declarations (`=>`/`*>` with 1/2/3/6/12
delay values).
**Where**: `crates/piperine-lang/src/{grammar/item.rs,ast/item.rs}`
**Depends on**: None (independent of Phase 1–2)
**Reuses**: `parse_param_decl_body` for `specparam`, `parse_expr`
**Requirement**: LRMW3-06

**Done when**:
- [x] `specify (a => y) = (1.0, 1.0); endspecify` parses into `PathDecl` with
      both delays
- [x] The full-connection form `(a, b *> y, z) = 1.5;` parses with the right
      input/output lists
- [x] `specparam tr = 1.0;` inside the block parses and is readable from the AST
- [x] The block is no longer stored as an unread span (the old
      `items: vec![]` shape is gone)
- [x] Declares `//! LRM-COVERS: A.7.2, A.7.3, A.7.4`
- [x] Gate passes: `cargo test -p piperine-lang --test vams_specify`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): real specify-block grammar for paths and specparams`

---

### T16: `specify` grammar — system timing checks

**What**: `$setup`/`$hold`/`$setuphold`/`$width`/`$period`/`$recovery`/
`$removal`/`$skew` with their event/limit/notifier argument forms.
**Where**: `crates/piperine-lang/src/{grammar/item.rs,ast/item.rs}`
**Depends on**: T15
**Reuses**: T15's block parser; `TimingCheckKind`'s spelling from the solver
**Requirement**: LRMW3-07

**Done when**:
- [x] `$setup(d, posedge clk, 1.0);` parses into `TimingCheckDecl` with data
      event, reference event (edge-qualified), and limit
- [x] The optional notifier argument parses
- [x] An unknown `$…` inside a specify block fails loud naming it
- [x] Declares `//! LRM-COVERS: A.7.5, A.7.5.1, A.7.5.2, A.7.5.3`
- [x] Gate passes: `cargo test -p piperine-lang --test vams_specify`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): system timing-check grammar inside specify blocks`

---

### T17: Unlowerable `specify` forms fail loud

**What**: edge-sensitive paths, state-dependent paths, PATHPULSE,
`showcancelled`, and the pulse-control family produce a named error citing
their Annex A production.
**Where**: `crates/piperine-lang/src/grammar/item.rs`,
`crates/piperine-lang/src/pom/error.rs`
**Depends on**: T16
**Reuses**: the `UnsupportedConstruct` variant introduced in T22 — **or**, if
T22 has not landed yet, this task introduces it and T22 reuses it (whichever
runs first owns it; the commit body records which)
**Requirement**: LRMW3-06, spec P2 AC3

**Done when**:
- [x] Each unsupported form has its own test asserting the clause is cited
- [x] No specify construct reaches the POM as an unread span (asserted by a
      test that walks the parsed block and requires every item to be typed)
- [x] Declares `//! LRM-COVERS: A.7.2`
- [x] Gate passes: `cargo test -p piperine-lang --test vams_specify`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): unlowerable specify forms fail loud citing their production`

---

### T18: `specify` reaches the POM

**What**: `SpecifyBlock` (specparams, paths, checks) lands on `pom::Module`;
`elab_module`'s no-op arm is gone.
**Where**: `crates/piperine-lang/src/{pom/module.rs,elab/lower/vams.rs}`
**Depends on**: T17
**Reuses**: the module-item lowering shape used by `constraints`
**Requirement**: LRMW3-06, LRMW3-07

**Done when**:
- [ ] `Module::specify()` returns the authored block, in authored order
- [ ] Terminals naming something that is not a module port fail loud citing
      A.7.3
- [ ] POM serde round-trips the block (the wire contract rule)
- [ ] Declares `//! LRM-COVERS: A.7.3`
- [ ] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): lower specify blocks into the POM`

---

### T19: Path delays reach `DelayTable`

**What**: at circuit build, each path delay writes a `PathDelay` record for its
destination net.
**Where**: `crates/piperine-codegen/src/device/circuit.rs`
**Depends on**: T18
**Reuses**: `DelayTable::set`, `PathDelay::uniform`, the module `timescale`
already resolved at elaboration
**Requirement**: LRMW3-06

**Done when**:
- [ ] A `(a => y) = (2.0, 3.0)` path makes `y`'s rise and fall delays observable
      in a transient run (edge timing asserted, not the table read back)
- [ ] The delay is quantized by `` `timescale`` when one is in force
- [ ] **Disclosed deviation pinned**: two paths into one net collapse onto one
      record, asserted by a dedicated test naming the deviation
- [ ] Declares `//! LRM-COVERS: A.7.4`
- [ ] Gate passes: `cargo test -p piperine-codegen` and `cargo test -p piperine-solver`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): specify path delays populate the solver delay table`

---

### T20: Timing-check limits reach `TimingCheckTable`

**What**: each parsed timing check writes a `TimingCheckLimit`.
**Where**: `crates/piperine-codegen/src/device/circuit.rs`
**Depends on**: T19
**Reuses**: `TimingCheckTable::set`, `TimingCheckKind`
**Requirement**: LRMW3-07

**Done when**:
- [ ] Every parsed check kind maps to its `TimingCheckKind` (exhaustive match,
      no catch-all)
- [ ] Limits are readable per net after a build
- [ ] Declares `//! LRM-COVERS: A.7.5.1`
- [ ] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): timing-check limits populate the solver table`

---

### T21: Timing-check enforcement through the findings channel

**What**: a breached limit emits a `ValidationFinding` at accepted digital
events, under the run posture.
**Where**: `crates/piperine-solver/src/digital/{scheduler.rs,delays.rs}`,
`crates/piperine-solver/src/core/validation.rs`
**Depends on**: T20
**Reuses**: the dv-core findings channel, `CheckPosture`
**Requirement**: LRMW3-07, spec P2 AC2

**Done when**:
- [ ] A setup violation in a real transient run is reported naming the check,
      the nets, the observed separation and the limit
- [ ] A compliant run reports nothing
- [ ] `checks = "off"` evaluates nothing (asserted by a cost/behavior test, not
      by inspection)
- [ ] Enforcement never fires inside a rejected step (asserted with a forced
      rejection)
- [ ] Gate passes: `cargo test -p piperine-solver` and `cargo test -p piperine <root findings target>`

**Tests**: integration · **Gate**: full
**Commit**: `feat(solver): enforce timing checks through the findings channel`

---

### T22: `config`, `specparam` (module scope) and `task` stop vanishing

**What**: introduce `ElabErrorKind::UnsupportedConstruct { construct, clause,
replacement }` and use it for the three silent no-op arms.
**Where**: `crates/piperine-lang/src/{pom/error.rs,elab/lower/{register.rs,vams.rs}}`
**Depends on**: T17 (shared variant; whichever lands first owns it)
**Reuses**: the existing `miette` diagnostic shape
**Requirement**: LRMW3-08, spec P2 AC4

**Done when**:
- [ ] A `config` declaration fails loud citing A.1.5 and naming
      `Piperine.toml`/`SourceMap` as the replacement (spec §0 rule 1)
- [ ] A module-scope `specparam` outside a specify block, and a module-scope
      `task`, each fail loud citing their clause
- [ ] `elab_module`'s `ModuleItem` match has **no** silent `=> {}` arm left for
      these three (asserted by a test per construct, not by reading the code)
- [ ] Declares `//! LRM-COVERS: A.1.5, 3.4.4`
- [ ] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): config/specparam/task fail loud instead of vanishing`

---

### T23: Discipline attribute override (§3.6.2.5)

**What**: `flow.abstol = 10u;` inside a `discipline` overrides the bound
nature's attribute for nets of that discipline.
**Where**: `crates/piperine-lang/src/{grammar/item.rs,pom/nature.rs,elab/lower/disciplines.rs}`
**Depends on**: None
**Reuses**: T48-of-wave-2's `Tolerances::node_vntol_overrides` path
**Requirement**: LRMW3-11

**Done when**:
- [ ] The override reaches the solver and is **observable**: a tighter
      `abstol` changes the accepted-iteration count on a fixture
- [ ] Overriding an attribute §3.6.1.2 forbids fails loud
- [ ] Declares `//! LRM-COVERS: 3.6.2.5`
- [ ] Gate passes: `cargo test -p piperine-lang` and `cargo test -p piperine-solver`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): discipline-level nature attribute override (§3.6.2.5)`

---

### T24: Nature derived from a discipline binding (§3.6.2.6)

**What**: `nature ttl_net_curr : ttl.flow;` inherits that nature's attributes
and may override or add.
**Where**: `crates/piperine-lang/src/{grammar/item.rs,pom/nature.rs,elab/lower/disciplines.rs}`
**Depends on**: T23
**Reuses**: the existing nature-derivation chain and compatibility rule
**Requirement**: LRMW3-11

**Done when**:
- [ ] Attributes are inherited from the bound nature, including a value the
      discipline itself overrode in T23
- [ ] A local override and a new attribute both work
- [ ] Compatibility follows the derivation family (asserted by connecting two
      nets of derived disciplines)
- [ ] Declares `//! LRM-COVERS: 3.6.2.6`
- [ ] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): natures derived from a discipline binding (§3.6.2.6)`

---

### T25: Discipline user-defined attributes (§3.6.2.7)

**What**: a `discipline` may declare user attributes, readable from the POM.
**Where**: `crates/piperine-lang/src/{grammar/item.rs,pom/nature.rs}`
**Depends on**: T24
**Reuses**: the nature user-attribute storage
**Requirement**: LRMW3-11

**Done when**:
- [ ] A user attribute survives to `DisciplineDecl::attributes`
- [ ] An attribute colliding with a reserved binding name fails loud
- [ ] Declares `//! LRM-COVERS: 3.6.2.7`
- [ ] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): user-defined discipline attributes (§3.6.2.7)`

---

### T26: Out-of-module discipline declaration and precedence (§3.9/§3.10)

**What**: `electrical top.u1.n;` declares a net's discipline from another
module, and §3.10's precedence order decides conflicts.
**Where**: `crates/piperine-lang/src/{grammar/item.rs,elab/lower/disciplines.rs}`
**Depends on**: T25
**Reuses**: `ResolveDisciplines`, the flattened form for path lookup
**Requirement**: LRMW3-12

**Done when**:
- [ ] An out-of-module declaration resolves the target net's discipline
- [ ] Precedence follows §3.10 (out-of-module beats local declaration beats
      resolution), asserted per rank
- [ ] Two conflicting out-of-module declarations fail loud naming both spans
- [ ] Declares `//! LRM-COVERS: 3.9, 3.10`
- [ ] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): out-of-module discipline declarations and §3.10 precedence`

---

### T27: LRM standard attributes (§2.9.2, §3.6.3.1)

**What**: `desc`, `units`, `op`, `multiplicity` register as core-owned schemas
and surface on host descriptors.
**Where**: `crates/piperine-lang/src/lib.rs` (seeding),
`crates/piperine-lang/src/elab/lower/vams.rs`, `piperine-solver` descriptors
**Depends on**: None
**Reuses**: `register_schema`, the `rfport` precedent, MD-26's atomic-attribute
rule
**Requirement**: LRMW3-10

**Done when**:
- [ ] `(* desc="drain terminal" *) electrical d;` elaborates and the text is
      readable through the POM and the host descriptor
- [ ] `units`/`op`/`multiplicity` validate their value domains loudly
      (`op` ∈ {"yes","no"}, `multiplicity` ∈ {"multiply","divide","none"})
- [ ] A duplicate `desc` on both the net and the port declaration keeps the
      last and warns (§3.6.3.1's own rule)
- [ ] Declares `//! LRM-COVERS: 2.9.2, 3.6.3.1`
- [ ] Gate passes: `cargo test -p piperine-lang` and `cargo test -p piperine <descriptor target>`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): LRM standard attributes desc/units/op/multiplicity`

---

### T28: Paramset output variables (§6.4.3)

**What**: a described variable in a `paramset` is an output variable of
instances using it, overriding a same-named module variable, with the
`.module_var` reference form.
**Where**: `crates/piperine-lang/src/{grammar/item.rs,elab/lower/vams.rs}`,
`crates/piperine-codegen/src/device/`
**Depends on**: T27
**Reuses**: the paramset selection path, the opvar reporting path
**Requirement**: LRMW3-19

**Done when**:
- [ ] `(* desc="cut-off frequency" *) real ft;` in a paramset is readable as an
      opvar on an instance using that paramset
- [ ] A paramset output variable **replaces** a same-named module output variable
- [ ] An undescribed paramset variable **hides** the module's same-named output
      variable (§6.4.3's second rule)
- [ ] `.gm` (module-output reference) resolves inside the paramset expression
- [ ] Declares `//! LRM-COVERS: 6.4.3`
- [ ] Gate passes: `cargo test -p piperine-lang` and `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): paramset output variables (§6.4.3)`

---

### T29: Port branches (§3.12.1)

**What**: `branch (<p>) probe_p;` declares a port branch; `I(probe_p)` reads
the flow into the port.
**Where**: `crates/piperine-lang/src/{grammar/item.rs,pom/branch.rs,elab/lower/flatten.rs}`,
`crates/piperine-codegen/src/flatten/analog.rs`
**Depends on**: None
**Reuses**: the force-branch / `HAS_INTERNAL_UNKNOWNS` machinery; `BranchKind`
gains `Port`
**Requirement**: LRMW3-13

**Done when**:
- [ ] §3.12.1's own `current_sink` example reads a nonzero port current that
      matches the current the connected network delivers
- [ ] A module **without** a port branch produces the identical node count as
      before (regression assert, named in the commit body)
- [ ] `V(<p>)` (a potential access on a port branch) fails loud per §5.4.3
- [ ] Declares `//! LRM-COVERS: 3.12.1, 5.4.3`
- [ ] Gate passes: `cargo test -p piperine-codegen` and `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): port branches read the flow into a port (§3.12.1)`

---

### T30: Hierarchical probe reads resolve at circuit build

**What**: `V(top.drv.x)` inside an expression resolves instead of failing with
`unresolved node`.
**Where**: `crates/piperine-codegen/src/{resolve/pom/body.rs,device/circuit.rs}`
**Depends on**: None
**Reuses**: the T50-of-wave-2 hierarchical-reference resolution and
`Design::flat_modules`
**Requirement**: LRMW3-15

**Done when**:
- [ ] A `$strobe`/expression read of a leaf net returns the leaf's solved value
- [ ] An unnamed branch is created in the **reading** module (§5.5.4), asserted
      through the branch catalog, not inferred
- [ ] An unresolvable path fails loud naming the path and the nearest resolved
      prefix
- [ ] Declares `//! LRM-COVERS: 5.5.4`
- [ ] Gate passes: `cargo test -p piperine-codegen` and `cargo test -p piperine <hier target>`

**Tests**: integration · **Gate**: full
**Commit**: `fix(codegen): resolve hierarchical probe reads at circuit build`

---

### T31: Hierarchical contributions (§5.6.8)

**What**: `V(top.drv.x) <+ 1.8;` and `V(top.drv.br_v) <+ 1.8;` drive the target.
**Where**: `crates/piperine-codegen/src/resolve/pom/expr.rs`,
`crates/piperine-codegen/src/flatten/analog.rs`
**Depends on**: T30
**Reuses**: T30's resolution; the unnamed-branch creation path
**Requirement**: LRMW3-15

**Done when**:
- [ ] A hierarchical contribution to a net drives it (asserted by solving and
      reading the leaf node)
- [ ] A hierarchical contribution to a **named** branch applies
- [ ] A hierarchical contribution that would turn the target into a switch
      branch fails loud citing §5.6.8.2
- [ ] Declares `//! LRM-COVERS: 5.6.8, 5.6.8.1, 5.6.8.2`
- [ ] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: full
**Commit**: `feat(codegen): hierarchical contributions to nets and named branches`

---

### T32: Indirect and direct contribution conflict (§5.6.7.2)

**What**: a branch that received an indirect assignment rejects a subsequent
`<+` on the same branch (or any parallel branch).
**Where**: `crates/piperine-codegen/src/flatten/analog.rs`
**Depends on**: None
**Reuses**: the branch-kind conflict machinery (`E4001`'s shape)
**Requirement**: LRMW3-16

**Done when**:
- [ ] The conflict fails loud citing §5.6.7.2 and labelling both statements
- [ ] A parallel branch between the same node pair is caught too
- [ ] A legal indirect-only and a legal direct-only body both still compile
- [ ] Declares `//! LRM-COVERS: 5.6.7.2`
- [ ] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(codegen): reject mixed indirect/direct contribution (§5.6.7.2)`

---

### T33: Analog-operator placement restrictions (§4.5.15)

**What**: an analog operator under a non-constant conditional, in an
event-triggered statement, in a `repeat`/`while`/non-genvar `for`, or inside a
user function is a compile-time error.
**Where**: `crates/piperine-lang/src/elab/typecheck.rs`
**Depends on**: None
**Reuses**: `AnalogOpRegistry` for operator identity, the const evaluator,
`Typecheck`'s accumulate-then-first shape
**Requirement**: LRMW3-17

**Done when**:
- [ ] Each of the four placements has its own failing fixture citing §4.5.15
- [ ] A constant-conditional placement still compiles (the legal case is
      asserted, so the check cannot be over-broad)
- [ ] `ddx` is exempt (§4.5.15's own exception)
- [ ] Declares `//! LRM-COVERS: 4.5.15`
- [ ] Gate passes: `cargo test -p piperine-lang` and `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): enforce §4.5.15 analog-operator placement restrictions`

---

### T34: Conditional-statement restrictions (§5.8.4)

**What**: event-control statements (`cross`, `timer`, …) under a non-constant
conditional are an error; analog filter functions likewise.
**Where**: `crates/piperine-lang/src/elab/typecheck.rs`
**Depends on**: T33
**Reuses**: T33's walker
**Requirement**: LRMW3-17

**Done when**:
- [ ] A `@(cross(...))` under a data-dependent `if` fails loud citing §5.8.4
- [ ] A nested conditional around a filter function fails loud
- [ ] The constant-condition case still compiles
- [ ] Declares `//! LRM-COVERS: 5.8.4`
- [ ] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): enforce §5.8.4 conditional-statement restrictions`

---

### T35: NaN/±infinity contribution is a named error (§7.3.2.1)

**What**: retarget the existing non-finite detection so the message cites
§7.3.2.1 and names the branch.
**Where**: `crates/piperine-solver/src/` (the non-finite stamp path),
`docs/spec/part_vii_solver.md` §16 row
**Depends on**: None
**Reuses**: the existing detection and its `failure_rules.rs` test
**Requirement**: LRMW3-18

**Done when**:
- [ ] Contributing `1.0/0.0` reports a typed error naming the branch and citing
      §7.3.2.1, not a generic convergence failure
- [ ] The pre-existing non-finite convergence-failure test still passes (it is
      updated in place, never deleted)
- [ ] The §16 table row and its enforcement cell are updated in the same commit
- [ ] Declares `//! LRM-COVERS: 7.3.2.1`
- [ ] Gate passes: `cargo test -p piperine-solver`

**Tests**: integration · **Gate**: full
**Commit**: `feat(solver): name the branch and cite §7.3.2.1 on a non-finite contribution`

---

### T36: Coverage backfill — clean-probe clauses

**What**: add `//! LRM-COVERS:` to the existing tests that already prove
§4.2.3, §4.6.4.5, §4.6.4.6, §5.4.4, §5.6.6, §7.4.2 — writing the missing test
where the clause is proven only incidentally.
**Where**: `crates/piperine-codegen/tests/`, `crates/piperine-lang/tests/`
**Depends on**: None
**Reuses**: existing targets
**Requirement**: LRMW3-23

**Done when**:
- [ ] Each id is declared by a target whose assertions actually prove the clause
      (an incidental pass is not a claim — where none exists, the test is
      written in this task)
- [ ] `declared_ids_exist_in_the_lrm` and `declaring_files_have_a_live_test`
      stay green
- [ ] Gate passes: `cargo test -p piperine --test lrm_section_coverage`

**Tests**: integration · **Gate**: full
**Commit**: `test(lrm-coverage): declare clauses proven by existing behavior tests`

---

### T37: Coverage backfill — display, file I/O, control, math system functions

**What**: §9.4.7, §9.5.1.1, §9.5.1.2, §9.5.9, §9.7.1, §9.7.2, §9.9, §9.14.
**Where**: `crates/piperine-codegen/tests/`, `crates/piperine-lang/tests/`
**Depends on**: T36
**Reuses**: the existing file-I/O and task targets
**Requirement**: LRMW3-24, LRMW3-25

**Done when**:
- [ ] `%r` on a real in the digital context is asserted
- [ ] Multi-analysis file append (§9.5.1.1) and cross-context descriptor sharing
      (§9.5.1.2) each have a test
- [ ] §9.5.9's rejected-iteration rule is either implemented and tested, or
      declared a limitation with a pinning test (the choice recorded in the
      commit body)
- [ ] `$finish`/`$stop` diagnostic levels asserted
- [ ] Gate passes: `cargo test -p piperine --test lrm_section_coverage`

**Tests**: integration · **Gate**: full
**Commit**: `test(lrm-coverage): display/file-I/O/control/math clause coverage`

---

### T38: Coverage backfill — driver access (§9.22, §9.23)

**What**: declare the eleven driver-access ids on the tests that prove them.
**Where**: `crates/piperine-lang/tests/vams_system_tasks.rs`,
`crates/piperine-solver/src/digital/drivers.rs` (inline module),
`crates/piperine-codegen/tests/vams_driver_access_gap.rs`
**Depends on**: T37
**Reuses**: the wave-2 T54 driver work
**Requirement**: LRMW3-26

**Done when**:
- [ ] Each of §9.22.1–§9.22.6 and §9.23–§9.23.4 is declared by a test asserting
      that function's own return contract
- [ ] `$driver_type`'s bit meanings are asserted against `driver_access.vams`
- [ ] Gate passes: `cargo test -p piperine --test lrm_section_coverage`

**Tests**: integration · **Gate**: full
**Commit**: `test(lrm-coverage): driver-access clause coverage`

---

### T39: Coverage backfill — namespaces, signal-flow, scheduling

**What**: §3.13–§3.13.4, §1.3.4–§1.3.5, §7.2.3, §8.2, §8.4.1, §8.4.3.1, §8.4.7.
**Where**: `crates/piperine-lang/tests/`, `crates/piperine-solver/tests/`
**Depends on**: T38
**Reuses**: existing initialization and scheduling targets
**Requirement**: LRMW3-21, LRMW3-22, LRMW3-27

**Done when**:
- [ ] Signal-flow gets real fixtures: a potential-only and a flow-only
      discipline each driving a working module, plus the illegal-contribution
      rule (§1.3.4's own "current contributions are not legal") asserted loud
- [ ] Namespace rules asserted (an access function shadowed by a local
      identifier; a net declared only at module scope)
- [ ] Initialization order (nodeset → `analog initial` → `initial`) asserted
- [ ] Gate passes: `cargo test -p piperine --test lrm_section_coverage`

**Tests**: integration · **Gate**: full
**Commit**: `test(lrm-coverage): namespace, signal-flow and scheduling clauses`

---

### T40: Coverage backfill — connect modules and elaboration order

**What**: §6.6.2.1, §6.9.3, §7.8.5.1, §7.8.6.
**Where**: `crates/piperine-lang/tests/`, root `tests/vams_connect_modules_e2e.rs`
**Depends on**: T39
**Reuses**: the wave-2 connect-module work, the host sweep surface for dynamic
parameters
**Requirement**: LRMW3-27

**Done when**:
- [ ] Dynamic parameters (§6.6.2.1) asserted through a DC parameter sweep on a
      compiled session (MD-18 path)
- [ ] Connect-module naming for built-in gate primitives (§7.8.5.1) implemented
      or declared a limitation with a pinning test
- [ ] A supply-sensitive connect-module example (§7.8.6) elaborates
- [ ] Gate passes: `cargo test -p piperine --test lrm_section_coverage`

**Tests**: integration · **Gate**: full
**Commit**: `test(lrm-coverage): connect-module and elaboration-order clauses`

---

### T41: Coverage backfill — Annex C and chapter 12

**What**: cross-declare the fifteen Annex C ids on the chapter tests that prove
the same clause, and map the thirteen ch. 12 ids onto the existing
`VpiCategory` entries and their locks.
**Where**: `crates/piperine-lang/tests/`, `crates/piperine-plugin/tests/vpi_routines.rs`
**Depends on**: T40
**Reuses**: `crates/piperine-plugin/src/vpi.rs`'s category table
**Requirement**: LRMW3-28, LRMW3-29

**Done when**:
- [ ] Each Annex C id is declared **with a written reason** naming the chapter
      test that proves the same delta (never a bare claim)
- [ ] Each ch. 12 id maps to a `VpiCategory` whose support level
      (`Native`/`Partial`/`NotSupported`) is asserted by `vpi_routines.rs`
- [ ] An id whose category is `NotSupported` becomes an `OutOfScope` row in the
      wave-3 exclusions file instead of a coverage claim
- [ ] Gate passes: `cargo test -p piperine --test lrm_section_coverage`

**Tests**: integration · **Gate**: full
**Commit**: `test(lrm-coverage): Annex C cross-declarations and VPI category mapping`

---

### T42: Wave-3 exclusions file, read by the gate

**What**: the gate reads **this** feature's Out of Scope table too, and a
wave-3 `coverage_exclusions.toml` carries the rows §4.2 of the conformance Part
names.
**Where**: `tests/lrm_section_coverage.rs`,
`.specs/features/lrm-conformance-wave3/coverage_exclusions.toml`
**Depends on**: T41
**Reuses**: the wave-2 exclusions parser and its `spec_row` cross-check
**Requirement**: LRMW3-30, PSW3-09

**Done when**:
- [ ] Every `out_of_scope` row's `spec_row` matches a real row in a real
      spec.md Out of Scope table (wave 2's **or** wave 3's)
- [ ] Every row names what replaces the LRM mechanism (spec §0 rule 1) —
      asserted by requiring a non-empty `replacement` field
- [ ] The stale "expected until wave-2's closure work lands" message and the
      `piperine-signal` header are gone (PSW3-06)
- [ ] Gate passes: `cargo test -p piperine --test lrm_section_coverage`

**Tests**: integration · **Gate**: full
**Commit**: `test(lrm-coverage): wave-3 exclusions and honest gate messaging`

---

### T43: Axis-A gate green, and proven able to fail

**What**: `covered_and_excluded_partition_the_lrm` reaches zero gaps and is
mutation-checked.
**Where**: `tests/lrm_section_coverage.rs`
**Depends on**: T42 (and, transitively, every Phase 1–5 task)
**Reuses**: —
**Requirement**: LRMW3-30, spec G1

**Done when**:
- [ ] Zero uncovered sections
- [ ] A temporary removal of one `LRM-COVERS` line turns the gate red, and a
      temporary bogus id turns `declared_ids_exist_in_the_lrm` red — both
      reverted, both recorded in the commit body
- [ ] `docs/spec/part_iii_conformance.md` §1.1's numbers are regenerated from
      the real run in the same commit
- [ ] Gate passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: build
**Commit**: `test(lrm-coverage): close the LRM coverage matrix to zero gaps`

---

### T44: `SPEC-COVERS` syntax rule

**What**: `suite_hygiene.rs` validates `//! SPEC-COVERS: <ids>` the way it
already validates `LRM-COVERS`.
**Where**: `tests/suite_hygiene.rs`
**Depends on**: None
**Reuses**: `lrm_covers_declarations_are_well_formed`
**Requirement**: PSW3-10

**Done when**:
- [ ] Well-formed ids (`part_viii#15.2`, `appendix_c#3`) accept; malformed ones
      fail with a named message
- [ ] The check is proven able to fail (temporary malformed declaration)
- [ ] Gate passes: `cargo test -p piperine --test suite_hygiene`

**Tests**: integration · **Gate**: quick
**Commit**: `test(hygiene): SPEC-COVERS declaration syntax rule`

---

### T45: Spec heading parser and declaration scraper

**What**: `tests/spec_section_coverage.rs` parses every `docs/spec/*.md`
heading into ids and scrapes `SPEC-COVERS` declarations.
**Where**: `tests/spec_section_coverage.rs`
**Depends on**: T44
**Reuses**: `lrm_section_coverage.rs`'s heading parser and scraper shapes,
`tests/support/sources.rs`
**Requirement**: PSW3-10, spec G7

**Done when**:
- [ ] `spec_sections()` returns a stable id set; a count assertion pins it so a
      spec edit that adds a heading is visible
- [ ] `declared_ids_exist_in_the_spec` catches a typo'd id
- [ ] `declaring_files_have_a_live_test` catches a claim with no live test
- [ ] Gate passes: `cargo test -p piperine --test spec_section_coverage`

**Tests**: integration · **Gate**: quick
**Commit**: `test(spec-coverage): heading parser and SPEC-COVERS scraper`

---

### T46: Axis-B partition test and exclusions

**What**: every spec heading resolves to `Covered`, `DeclaredLimitation` or
`Informative`.
**Where**: `tests/spec_section_coverage.rs`,
`.specs/features/lrm-conformance-wave3/spec_coverage_exclusions.toml`
**Depends on**: T45
**Reuses**: the wave-2 exclusions-parser shape
**Requirement**: PSW3-10, spec G7

**Done when**:
- [ ] The partition test exists and reports the real gap list grouped by spec file
- [ ] `DeclaredLimitation` rows require a non-empty reason
- [ ] The test is proven able to fail (a heading with no verdict)
- [ ] Gate passes: `cargo test -p piperine --test spec_section_coverage`

**Tests**: integration · **Gate**: quick
**Commit**: `test(spec-coverage): partition every docs/spec heading`

---

### T47: Backfill `SPEC-COVERS` across existing targets

**What**: declare what the existing suite already proves about Piperine's own
surface (host API, plugins, POM, IR, constraints, sampling, drivers).
**Where**: `crates/*/tests/*.rs`, root `tests/*.rs`
**Depends on**: T46
**Reuses**: existing targets and their `//!` headers
**Requirement**: PSW3-10, spec G7

**Done when**:
- [ ] Every spec heading with an existing proving test is `Covered`
- [ ] No claim is added to a target whose assertions do not prove it
- [ ] The remaining red list is the honest axis-B gap list, and it matches the
      spec's B1–B10 plus whatever the backfill uncovers
- [ ] Gate passes: `cargo test -p piperine --test spec_section_coverage`

**Tests**: integration · **Gate**: full
**Commit**: `test(spec-coverage): backfill SPEC-COVERS on the existing suite`

---

### T48: Extension gate (B9)

**What**: one test enumerating every sanctioned extension and requiring a live
test per extension.
**Where**: `tests/spec_section_coverage.rs` (or a sibling target), driven by
`docs/spec/part_iii_conformance.md` §6's table
**Depends on**: T47
**Reuses**: the §6 table read via `include_str!` (the anti-drift pattern wave 2
used for spec tables)
**Requirement**: PSW3-07

**Done when**:
- [ ] Every row of the extensions table names a live test
- [ ] Adding a row without a test turns it red (proven)
- [ ] The wave-2 targets whose coverage claims were dropped because they prove
      an extension (`vams_constraint_block.rs`, `vams_param_distributions.rs`,
      `vams_doc_comments.rs`, `vams_device_port_attrs.rs`) are now claimed here
- [ ] Gate passes: `cargo test -p piperine --test spec_section_coverage`

**Tests**: integration · **Gate**: full
**Commit**: `test(spec-coverage): every sanctioned extension names its test`

---

### T49: Selector — implement or remove the inert axes (B1)

**What**: `constraint::` dispatches to `Module::constraints()`; `driver::`,
`load::`, `parent::`, `ancestor::` either traverse or are removed from the
grammar — never an empty selection.
**Where**: `crates/piperine-lang/src/pom/selector/{eval.rs,parse.rs,ast.rs}`
**Depends on**: None
**Reuses**: `Module::constraints()`, the flattened form for connectivity
**Requirement**: PSW3-01, spec P2 (axis B) AC3

**Done when**:
- [ ] `constraint::` returns the authored constraint statements
- [ ] Each remaining axis either returns real matches (with a test) or is a
      parse error naming it as unimplemented
- [ ] No axis returns an empty selection for an unimplemented reason
- [ ] Declares `//! SPEC-COVERS: part_iv#9`
- [ ] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: full
**Commit**: `feat(lang): selector axes return matches or fail loud`

---

### T50: Prove the retired POM/value/error variants unreachable (B2/B3/B4)

**What**: a test asserting no Verilog-AMS source can produce
`Kind::{Enum,Bundle,Capability}`, `Value::{Record,List,Tuple,EnumVariant}`, or
the retired `ElabErrorKind` variants — and marking each `#[doc(hidden)]`
reserved.
**Where**: `crates/piperine-lang/tests/`, `crates/piperine-lang/src/pom/`
**Depends on**: None
**Reuses**: the POM traversal used by the selector
**Requirement**: PSW3-02

**Done when**:
- [ ] A traversal over every fixture in the corpus produces none of them
- [ ] Each variant carries a `reserved — unreachable from Verilog-AMS` doc line
      pointing at this test
- [ ] Removal is scheduled separately (breaking change), recorded in the commit
      body
- [ ] Declares `//! SPEC-COVERS: part_iv#10`
- [ ] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `test(pom): prove the retired variants unreachable from Verilog-AMS`

---

### T51: Declare and assert the host asymmetries (B5)

**What**: the parity test asserts the declared Rust-only list instead of
silently tolerating it.
**Where**: `tests/host_parity.rs`, `docs/spec/appendix_c_host_surface.md`
**Depends on**: T47
**Reuses**: `host_parity.rs`'s probe-and-discriminate shape
**Requirement**: PSW3-03, spec P2 (axis B) AC4

**Done when**:
- [ ] A declared Rust-only surface that **gains** a Python binding fails the
      test until the declaration is updated (the asymmetry list is exact, not a
      floor)
- [ ] An undeclared asymmetry fails the test
- [ ] Appendix C's list and the test's list are cross-checked via `include_str!`
- [ ] Declares `//! SPEC-COVERS: appendix_c#3`
- [ ] Gate passes: `cargo test -p piperine --test host_parity`

**Tests**: integration · **Gate**: full
**Commit**: `test(host): assert the declared Rust/Python asymmetries exactly`

---

### T52: Pin the `piperine-ir` delegation (B6)

**What**: a test that fails if codegen re-grows a private copy of a runtime
family.
**Where**: `crates/piperine-codegen/tests/`
**Depends on**: None
**Reuses**: the purity-guard idiom (`piperine-ir/tests/purity.rs`)
**Requirement**: PSW3-04

**Done when**:
- [ ] The test asserts codegen's operator/detector/limiter paths route through
      `piperine_ir` (by behavior — identical outputs for identical inputs —
      not by grepping imports)
- [ ] The already-corrected `ops.rs` ownership comment is cross-referenced
- [ ] Declares `//! SPEC-COVERS: part_ix#1`
- [ ] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: quick
**Commit**: `test(codegen): pin the piperine-ir delegation for runtime families`

---

### T53: Test the legacy `` `include`` aliases (B7)

**What**: the `discipline.h`/`disciplines.h`/`constants.h` fallback is proven,
including its precedence.
**Where**: `crates/piperine-lang/tests/`
**Depends on**: None
**Reuses**: `SourceMapIncludeResolver`
**Requirement**: PSW3-05

**Done when**:
- [ ] `` `include "discipline.h" `` resolves to Annex D's `disciplines.vams`
- [ ] A project shipping its **own** `discipline.h` wins over the alias
- [ ] An unknown legacy name still fails loud
- [ ] Declares `//! SPEC-COVERS: part_i#9`
- [ ] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `test(lang): legacy include aliases and their precedence`

---

### T54: Example corpus (B10)

**What**: rename the retired-name example and add one exercising the extension
surface (`constraint` + `tol` + the optimizer).
**Where**: `examples/`
**Depends on**: None
**Reuses**: `tests/run_examples.rs`'s dual contract
**Requirement**: PSW3-08

**Done when**:
- [ ] `14_bundle_model_card.*` is renamed to a Verilog-AMS-accurate name, with
      its `.py` twin updated
- [ ] A new example declares a `require` with a `tol`-carrying parameter and
      runs a centering or Monte Carlo pass, asserting a numeric outcome
- [ ] `cargo test -p piperine --test run_examples` is green
- [ ] Declares `//! SPEC-COVERS: appendix_a#1` on the run-examples target
- [ ] Gate passes: `cargo test -p piperine --test run_examples`

**Tests**: integration · **Gate**: full
**Commit**: `docs(examples): rename the retired-name example, add an extension example`

---

### T55: Axis-B gate green, and the docs regenerated

**What**: `covered_and_excluded_partition_the_spec` reaches zero unverdicted
headings; `docs/spec` Parts III/IV/IX and `appendix_c` are updated with what
the closure work changed.
**Where**: `tests/spec_section_coverage.rs`, `docs/spec/*.md`
**Depends on**: T54 (and every Phase 7–8 task)
**Reuses**: —
**Requirement**: PSW3-10, spec G8

**Done when**:
- [ ] Zero unverdicted headings
- [ ] Part IV §10's divergence list is regenerated against what T49/T50 changed
- [ ] Part III §4/§5 regenerated from the real gate output
- [ ] Gate passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: build
**Commit**: `test(spec-coverage): close the axis-B matrix and regenerate the spec`

---

## Phase Execution Map

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8

Phase 1:  T1 → T2 → T3 → T4 → T5 → T6
Phase 2:  T7 → T8 → T9 → T10 → T11 → T12 → T13 → T14
Phase 3:  T15 → T16 → T17 → T18 → T19 → T20 → T21 → T22
Phase 4:  T23 → T24 → T25 → T26 → T27 → T28
Phase 5:  T29 → T30 → T31 → T32 → T33 → T34 → T35
Phase 6:  T36 → T37 → T38 → T39 → T40 → T41 → T42 → T43
Phase 7:  T44 → T45 → T46 → T47 → T48
Phase 8:  T49 → T50 → T51 → T52 → T53 → T54 → T55
```

Execution is strictly sequential. 55 tasks → **8 batches** at the ~7-task
budget (Phase 2/3/6 are already 8, which the packer treats as one batch each).

---

## Task Granularity Check

| Task | Scope | Status |
|---|---|---|
| T1–T3 | one grammar production each | ✅ granular |
| T4 | one helper + its two call sites | ✅ granular |
| T5, T12 | one representation change + its access path | ✅ cohesive |
| T7 | one function replaced | ✅ granular |
| T15–T17 | one grammar family each (paths / checks / rejections) | ✅ granular |
| T18–T21 | one lowering hop each (POM → delay table → check table → enforcement) | ✅ granular |
| T23–T28 | one clause each | ✅ granular |
| T29–T35 | one clause each | ✅ granular |
| T36–T41 | one clause group each, ≤8 ids | ✅ cohesive |
| T44–T46 | one gate component each | ✅ granular |
| T49–T54 | one axis-B gap each | ✅ granular |
| T43, T55 | closure + proof-of-failure per axis | ✅ cohesive |

---

## Diagram-Definition Cross-Check

| Task | Depends on (body) | Diagram shows | Status |
|---|---|---|---|
| T1 | none | phase head | ✅ |
| T2 | T1 | T1 → T2 | ✅ |
| T3 | T1 | T2 → T3 (chain, T1 upstream) | ✅ |
| T4 | T1 | chain within phase 1 | ✅ |
| T5 | T4 | T4 → T5 | ✅ |
| T6 | T5 | T5 → T6 | ✅ |
| T7 | T4 (phase 1) | backward, cross-phase | ✅ |
| T8 | T7 | T7 → T8 | ✅ |
| T9 | T3, T4 | backward, cross-phase | ✅ |
| T10 | T9 | T9 → T10 | ✅ |
| T11 | T10 | T10 → T11 | ✅ |
| T12 | T5, T11 | backward + in-phase | ✅ |
| T13 | T2, T4 | backward | ✅ |
| T14 | T12 | in-phase backward | ✅ |
| T15 | none | phase head | ✅ |
| T16–T21 | previous task | chain | ✅ |
| T22 | T17 | in-phase backward | ✅ |
| T23 | none | phase head | ✅ |
| T24–T28 | previous task | chain | ✅ |
| T29, T30, T32, T33, T35 | none | phase-internal order only | ✅ |
| T31 | T30 | T30 → T31 | ✅ |
| T34 | T33 | T33 → T34 | ✅ |
| T36–T43 | previous task | chain | ✅ |
| T43 | T42 + all of phases 1–5 | phase order guarantees it | ✅ |
| T44–T48 | previous task | chain | ✅ |
| T49, T50, T52, T53, T54 | none | phase-internal order only | ✅ |
| T51 | T47 | backward, cross-phase | ✅ |
| T55 | T54 + phases 7–8 | phase order guarantees it | ✅ |

No task depends on a later phase.

---

## Test Co-location Validation

| Task | Layer created/modified | Matrix requires | Task says | Status |
|---|---|---|---|---|
| T1–T3, T5, T6 | grammar + elaboration | integration | integration | ✅ |
| T4 | codegen resolve | integration | integration | ✅ |
| T7–T12, T14 | codegen resolve/emit | integration | integration | ✅ |
| T13 | elaboration + codegen | integration | integration | ✅ |
| T15–T18, T22 | grammar + elaboration | integration | integration | ✅ |
| T19, T20 | codegen device | integration | integration | ✅ |
| T21 | solver | unit + integration | integration (with an inline unit for the limit comparison) | ✅ |
| T23–T28 | grammar + elaboration (+ solver reach in T23) | integration | integration | ✅ |
| T29–T32 | codegen | integration | integration | ✅ |
| T33, T34 | elaboration | integration | integration | ✅ |
| T35 | solver | unit + integration | integration | ✅ |
| T36–T43 | gates + tests | integration | integration | ✅ |
| T44–T48 | gates | integration | integration | ✅ |
| T49 | POM selector | integration | integration | ✅ |
| T50 | POM | integration | integration | ✅ |
| T51 | host surface | integration | integration | ✅ |
| T52 | codegen | integration | integration | ✅ |
| T53 | elaboration | integration | integration | ✅ |
| T54 | examples (host + lang) | integration | integration | ✅ |
| T55 | gates + docs | integration | integration | ✅ |

No task defers its tests to another task.

---

## Tools per task

Unless a task says otherwise: **MCP** NONE (this is a local Rust workspace;
no MCP server is configured for it) · **Skill** `tlc-spec-driven` for the
execute cycle. No task uses a web/browser tool: the LRM is in-repo
(`VAMS-LRM-2-4.md`) and is the only external authority any task needs.
