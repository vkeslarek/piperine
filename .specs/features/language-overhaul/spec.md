# language-overhaul Specification

**Companion:** `ideal.md` rev 5 — the settled design and the reasoning of
record. This spec is the testable form of §1–§8 of that document, scoped to
migration steps 1–4 (§6). Every design question D1–D12 is answered there; none
is re-opened here.

## Problem Statement

Five defects surfaced in two days of building one small mixed-signal circuit,
and every one of them lives **between** compiler stages rather than inside one
(`GAPS.md` T0, A0, A0b, A0c, A0d). A 1841-test suite missed all five because
each test exercises a single stage against a hand-built fixture, so nothing in
the tree can disagree with anything else in the tree. The structural cause is
that there is no intermediate representation: four partial, overlapping forms
exist (POM, `resolve`, `flatten`, and the raw AST the digital path consumes),
and `LoweredBody` carries two of them in one struct with no ownership rule.

The same absence produces the three complaints the project owner named: codegen
is a black box, the device DAG is assembled invisibly, and the author has lost
traceability of his own code. It also caps the language: the digital half cannot
express a bus, a memory, or an indexed write, so a processor is not
representable and the spec's own `SarAdc` example does not compile.

## Goals

- [ ] **One IR**, five concepts (`ideal.md` §1), replacing four partial forms —
      measured by `flatten/` and the digital AST→Cranelift path being deleted,
      not refactored.
- [ ] **Seam defects become unrepresentable**: each of A0, A0b, A0c, A0d has a
      test proving the construct that caused it now fails at IR-verify or
      lowering time, loudly.
- [ ] **A differential oracle**: a reference interpreter that must agree with the
      JIT backends on every fixture, in CI.
- [ ] **Traceability**: the IR is printable and diffable after every pass.
- [ ] **Completeness**: opamp, buck, and an RV32I core each lower and simulate
      correctly (`ideal.md` §4).
- [ ] **Zero parity regression**: `tests/ngspice_*` and the parity matrix pass
      unchanged throughout.

## Out of Scope

| Item | Reason |
|---|---|
| Solver analog/digital device split | Endorsed in principle (`ideal.md` §6.1, D12 retired MD-01's objection) but deferred — its own feature. Splitting before the IR would recreate A0d with a bigger blast radius. |
| `piperine-solver`, `piperine-api`, hosts, plugins | The `Element` ABI is the frozen frontier for this feature. |
| Synthesis / netlist output | Processor bar is (a) *expressible and simulates correctly*, not (b) synthesizable. A synthesis backend becomes a plugin pass later. |
| IR optimization passes | Explicitly out for v1 (`ideal.md` §1.3). The IR must **permit** passes and ship only what the backends need. |
| Rewriting `headers/spice/` | Those models are the ngspice-validated asset. Bounded edits only. |
| Fixing T0 | Already fixed (2026-08-09) and lives beyond the frontier anyway. |
| `laplace_*` / `zi_*` | Undeclared today and failing at elaboration. The catalog gives them a home; implementing them is separate. |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| IR crate identity | `piperine-signal` **becomes** `piperine-ir` | D8. Crate identity transfers; its purity guard becomes the IR's structural property | y |
| 4-state encoding | (value, mask) word pair | D3. Ordinary integer ops on the value word; packed 2-bit lanes need shift+mask per lane on every op | y |
| Control flow in the IR | Structured; **no flattening pass at all** | D9. Symbolic diff is happy over structured control (`d/dx (if c then f else g)` = `if c then f' else g'`); Cranelift emits branches | y |
| Inferred state | Front-end pass emits an explicit `cell` | D10. Keeps the single-writer rule checkable; gives `InferredLatch` (spec'd, never implemented) a place to fire | y |
| Aggregates | `[T; D]` arrays, orthogonal to `bits<N>` width | D11. One concept covers memories and analog buses; collapsing a digital bus into an array of bits would make a 32-bit add 32 ops | y |
| Signal-flow (`Voltage`/`Current`) | Stays declared, stays non-functional, but must fail **loud** | D2. §1.2's net typing gives the loud failure for free; today it dies as `SymbolicSingular` | y |
| Net arrays (`wire x : Electrical[8]`) | Expanded by elaboration, never reach the IR | `gap-3` array-net expansion already landed; nets stay scalar below the POM | y |
| `real` is always scalar | Yes — width attaches only to `bits`; multiple reals are `[real; D]` | Keeps two value classes plus one aggregate rather than a width on both | y |
| PHDL surface stability | Existing syntax stays; §P3 only **adds** | "PHDL is modified, not replaced" (user); `headers/spice/` must survive | y |
| Migration order | IR standalone → backends+interpreter → prove on 3 designs → move the language | D6 (user). The IR is proven before anything depends on it | y |

**Open questions:** none — all resolved in `ideal.md` rev 5 (D1–D12) or logged
above.

### Implicit-requirement dimensions sweep (Large scope — every dimension resolved)

| Dimension | Resolution |
|---|---|
| Input validation & bounds | **LOV-04** — the IR verifier rejects malformed IR (unknown cell writer, width mismatch, out-of-range chunk, nature/net-kind mismatch) |
| Failure / partial-failure | **LOV-05** — every lowering failure is a named error; no catch-all arm anywhere in the pipeline (the A0c mechanism) |
| Idempotency / retry / duplicate | **LOV-06** — lowering is deterministic: the same POM produces byte-identical printed IR across runs |
| Auth boundaries & rate limits | **N/A** — a compiler IR has no authorization surface |
| Concurrency / ordering | **LOV-07** — order-dependent semantics (internal-unknown allocation order, digital event ordering, block evaluation order) are explicit in the IR, not implied by traversal |
| Data lifecycle / expiry | **N/A** — the IR is in-memory; dumps are developer artifacts with no retention contract |
| Observability | **LOV-08** — `--emit=ir` after each pass, stable enough to `diff`; `\user` vs `$generated` name provenance |
| External-dependency failure | **N/A by construction** — **LOV-09** requires `piperine-ir` to have no in-tree dependency (inherited purity guard), so there is no dependency to fail |
| State-transition integrity | **LOV-03** — one declared writer per cell, enforced by the verifier (the A0d mechanism) |

---

## User Stories

### P1: The IR exists, is checkable, and two engines agree on it ⭐ MVP

**User Story**: As the toolchain author, I want a single printable IR with a
verifier and two independent evaluators, so that a semantic disagreement becomes
a red build instead of a wrong waveform found months later in a real design.

**Why P1**: This is the whole thesis. It is demo-able with **no PHDL at all** —
hand-written IR fixtures, run two ways, compared. It is also the only ordering
that de-risks the rest: the IR is proven before anything depends on it.

**Acceptance Criteria**:

1. WHEN a value, net, cell, block, or catalog entry is constructed THEN the IR
   SHALL represent it using exactly the five concepts of `ideal.md` §1, and a
   sixth concept SHALL NOT exist.
2. WHEN an IR module is printed THEN the output SHALL be textual, stable across
   runs, and re-parseable into an equivalent module (round-trip).
3. WHEN a cell has zero writers or more than one writer THEN the verifier SHALL
   reject the module naming the cell and its writers.
4. WHEN a block reads or writes a net THEN it SHALL do so through a named
   nature-generic op (`read_across`, `read_through`, `contribute_through`,
   `force_across`, `read_signal`, `drive`), and no default/catch-all arm SHALL
   exist in any dispatch over those ops.
5. WHEN a `signal` net is handed to the analog backend for stamping THEN the
   verifier SHALL reject it before any matrix is built.
6. WHEN the same IR fixture is evaluated by the reference interpreter and by a
   JIT backend THEN the two SHALL agree within the fixture's declared tolerance
   on every output and every cell value.
7. WHEN an intrinsic is declared in the catalog THEN its signature, private
   state shape, derivative rule (if `real`-valued), declared breakpoints, and
   legal domains SHALL be declared in one place, and both evaluators SHALL read
   that one declaration.

**Independent Test**: Hand-write an IR module for an RC divider and a 4-bit
counter; run each through the interpreter and the JIT; assert agreement; then
mutate the module to violate each of AC3/AC4/AC5 and assert the verifier rejects.

---

### P2: The three-design completeness bar, expressed directly in IR

**User Story**: As a circuit designer, I want the IR to be provably able to
express an opamp, a buck converter, and an RV32I core, so that "complete"
becomes a passing test rather than a claim.

**Why P2**: Each design stresses a different axis and none is a superset
(`ideal.md` §4). Proving them at IR level, before the front-end exists, is what
keeps the surface work in P3 honest — the language must reach a bar the engine
already clears.

**Acceptance Criteria**:

1. WHEN the opamp fixture is evaluated THEN the IR SHALL produce a residual and
   a Jacobian derived by the differentiation pass, and the DC solution SHALL
   match a hand-computed operating point.
2. WHEN the buck fixture is evaluated THEN one design SHALL contain analog and
   digital blocks that sample each other explicitly, with `timer` and `cross`
   triggers firing at the declared instants.
3. WHEN the RV32I fixture is evaluated THEN a `[bits<32>; 32]` register file
   SHALL be read and written by address, a `bits<32>` add SHALL be one op, and
   an indexed bit write SHALL be expressible.
4. WHEN a memory cell is accessed THEN the access SHALL be a distinct op from
   the cell declaration, so that port count is inspectable before any backend
   lowering.
5. WHEN any of the three fixtures runs under both evaluators THEN they SHALL
   agree (P1 AC6 applies to all three).

**Independent Test**: Three IR fixture files, each with a golden result;
`cargo test` runs them under both evaluators.

---

### P3: PHDL lowers to the IR, with parity preserved

**User Story**: As a model author, I want my existing PHDL — including all of
`headers/spice/` — to compile through the IR with no numerical change, so that
the migration costs me nothing.

**Why P3**: This is the step that retires `resolve`/`flatten`/the digital AST
path. It must be numerically invisible, and the parity matrix is the judge.

**Acceptance Criteria**:

1. WHEN any module in `headers/spice/` is compiled THEN it SHALL lower to IR and
   produce a device whose behavior is numerically unchanged.
2. WHEN the full `tests/ngspice_*` suite runs against the IR path THEN every
   assertion SHALL pass with its existing tolerance contract unmodified.
3. WHEN a PHDL construct cannot be lowered THEN the compiler SHALL emit a named
   error identifying the construct and its source location, and SHALL NOT emit
   a no-op, a zero, or a ground-to-ground contribution.
4. WHEN a bundle-typed value reaches type conversion THEN it SHALL be expanded
   before the IR or rejected by name — the silent `ValueType::Bundle(_) =>
   Type::Real` coercion SHALL NOT survive.
5. WHEN latch inference derives state THEN the front-end SHALL emit an explicit
   `cell` and SHALL raise the `InferredLatch` diagnostic that Part I §10.3 and
   §16 specify.
6. WHEN the migration is complete THEN `flatten/` and the digital AST→Cranelift
   path SHALL be deleted, not retained behind a flag.

**Independent Test**: The existing parity suite, unmodified, run against the IR
path.

---

### P4: The PHDL surface reaches the bar the IR already clears

**User Story**: As a designer, I want to write a bus, a slice, a memory, and an
indexed write in PHDL, so that the digital half of the language can express a
state machine and a datapath.

**Why P4**: Last because it depends on all of the above. It is also where the
"complete language" goal is actually delivered to users — P2 proves the engine
can, P4 lets someone write it.

**Acceptance Criteria**:

1. WHEN `Bit[32]` is declared THEN it SHALL lower to one value of width 32, not
   32 independent scalar nets.
2. WHEN `x[31:25]` or `concat(a, b)` is written THEN it SHALL lower to a chunk
   reference with no computation emitted.
3. WHEN `code[idx] = 0` is written in a digital body THEN it SHALL compile —
   specifically, the `SarAdc` example printed in Part I §10.3 SHALL compile and
   simulate.
4. WHEN a multi-bit pattern (`0b1??0`) appears in a `match` THEN it SHALL
   compile.
5. WHEN `for` appears in a digital body THEN it SHALL compile (unrolled at
   elaboration or as an IR loop).
6. WHEN `UInt`/`SInt` are referenced THEN they SHALL resolve to declarations
   shipped in `headers/`, not to a test fixture.
7. WHEN a memory is declared in PHDL THEN it SHALL lower to an array-typed cell.

**Independent Test**: An `examples/` FSM + datapath design that elaborates,
compiles, and simulates — gated by `tests/run_examples.rs`.

---

## Edge Cases

- WHEN an IR module declares a cell written by both an analog and a digital
  block THEN the verifier SHALL reject it (the A0d shape).
- WHEN a block drives a net it does not declare as an output THEN the verifier
  SHALL reject it.
- WHEN a chunk reference exceeds the width of its source value THEN the verifier
  SHALL reject it naming both widths.
- WHEN a `read_across` and a `read_through` are applied to the same node pair
  THEN they SHALL produce distinct values (the A0 shape — a test asserts a
  hand-computed current differs from the hand-computed voltage).
- WHEN a memory is read and written at the same address in one evaluation THEN
  the IR SHALL define the ordering explicitly rather than leaving it to
  traversal order.
- WHEN a `force_across` appears THEN exactly one internal branch unknown SHALL
  be counted for it, and the count SHALL match what the device allocates.
- WHEN an intrinsic is invoked in a domain its catalog entry does not permit
  THEN lowering SHALL fail by name.
- WHEN the IR is printed twice for the same input THEN the two outputs SHALL be
  byte-identical.

---

## Requirement Traceability

| ID | Story | Requirement | Phase | Status |
|---|---|---|---|---|
| LOV-01 | P1 | Five concepts, no sixth | Design | Pending |
| LOV-02 | P1 | Printable, stable, round-trippable IR | Design | Pending |
| LOV-03 | P1 | One declared writer per cell, verifier-enforced | Design | Pending |
| LOV-04 | P1 | Verifier rejects malformed IR (widths, chunks, nature/net-kind, writers) | Design | Pending |
| LOV-05 | P1/P3 | Named errors everywhere; no catch-all dispatch arm | Design | Pending |
| LOV-06 | P1 | Deterministic lowering — byte-identical printed IR | Design | Pending |
| LOV-07 | P1 | Explicit ordering for order-dependent semantics | Design | Pending |
| LOV-08 | P1 | `--emit=ir` per pass; `\user` vs `$generated` provenance | Design | Pending |
| LOV-09 | P1 | `piperine-ir` has no in-tree dependency (purity) | Design | Pending |
| LOV-10 | P1 | Reference interpreter | Design | Pending |
| LOV-11 | P1 | Differential agreement interpreter vs JIT, in CI | Design | Pending |
| LOV-12 | P1 | Intrinsic catalog: one declaration, both evaluators | Design | Pending |
| LOV-13 | P1 | Nature-generic net ops; no `V`/`I` in the IR | Design | Pending |
| LOV-14 | P1 | `signal` unstampable by construction (A0b) | Design | Pending |
| LOV-15 | P1 | Symbolic differentiation as an IR pass over structured control | Design | Pending |
| LOV-16 | P2 | Opamp fixture: residual + Jacobian + golden DC point | Design | Pending |
| LOV-17 | P2 | Buck fixture: cross-domain sampling, `timer`/`cross` triggers | Design | Pending |
| LOV-18 | P2 | RV32I fixture: array cell, width-native add, indexed write | Design | Pending |
| LOV-19 | P2 | Memory access ops distinct from declaration; port count inspectable | Design | Pending |
| LOV-20 | P3 | `headers/spice/` numerically unchanged through the IR | Design | Pending |
| LOV-21 | P3 | ngspice parity suite passes with tolerances unmodified | Design | Pending |
| LOV-22 | P3 | Bundle expansion before the IR; silent `=> Type::Real` removed | Design | Pending |
| LOV-23 | P3 | Latch inference emits an explicit cell + raises `InferredLatch` | Design | Pending |
| LOV-24 | P3 | `flatten/` and the digital AST path deleted | Design | Pending |
| LOV-25 | P3 | Internal-unknown budget counted from the IR, matching allocation | Design | Pending |
| LOV-26 | P4 | `Bit[N]` is one width-N value | Design | Pending |
| LOV-27 | P4 | Slice and concat as chunk references, no computation | Design | Pending |
| LOV-28 | P4 | Indexed write; Part I §10.3's `SarAdc` compiles | Design | Pending |
| LOV-29 | P4 | Multi-bit `match` patterns | Design | Pending |
| LOV-30 | P4 | `for` in digital bodies | Design | Pending |
| LOV-31 | P4 | `UInt`/`SInt` shipped in `headers/` | Design | Pending |
| LOV-32 | P4 | PHDL memory declaration → array-typed cell | Design | Pending |

**ID format:** `LOV-NN`
**Status values:** Pending → In Design → In Tasks → Implementing → Verified
**Coverage:** 32 total, 0 mapped to tasks, 32 unmapped ⚠️ (Tasks phase pending)

---

## Success Criteria

- [ ] `piperine-ir` exists as a crate with no in-tree dependency, carrying
      exactly five concepts plus a catalog.
- [ ] Every IR fixture agrees between the reference interpreter and the JIT
      backends; disagreement is a red build.
- [ ] Each of A0, A0b, A0c, A0d has a test proving its construct is now rejected
      loudly rather than silently mis-executed.
- [ ] The three completeness fixtures (opamp, buck, RV32I) pass under both
      evaluators.
- [ ] `tests/ngspice_*` and `tests/ngspice_parity_matrix.rs` pass unchanged,
      with tolerance contracts unmodified.
- [ ] `flatten/` and the digital AST→Cranelift path are deleted from the tree.
- [ ] `--emit=ir` produces output a human can read and `diff` between passes.
- [ ] The `SarAdc` example from Part I §10.3 compiles and simulates.
- [ ] `cargo build --workspace` at zero warnings; `cargo test --workspace` green.
