# Kickstart prompt — `language-overhaul`

> Hand this whole file to the model that will execute the feature. It is written
> as an instruction, not as documentation. It assumes zero prior context.

---

You are picking up a large, fully-specified refactor in the **Piperine**
repository (`~/Git/piperine`, Rust workspace, edition 2024). The planning is
done. Your job is execution, and the plan is deliberately not yours to redesign.

## 1. Read these first, completely, in this order

| File | What it gives you |
|---|---|
| `CLAUDE.md` (repo root) | Project map, crate responsibilities, build/test commands, unbreakable rules |
| `AGENTS.md` | Hard rules, including the Rust idiom rules |
| `.specs/STATE.md` — `## Decisions` | Every `MD-NN`. These are **binding project constraints**, not suggestions. Pay special attention to MD-13, MD-18, MD-24, MD-25, MD-28, MD-31, MD-35, MD-38, **MD-40**, **MD-41** |
| `.specs/features/language-overhaul/ideal.md` | The settled design and the reasoning of record. Five revisions of argument; the *why* behind every choice |
| `.specs/features/language-overhaul/spec.md` | 32 traceable requirements (`LOV-01`…`LOV-32`), acceptance criteria in WHEN/THEN/SHALL |
| `.specs/features/language-overhaul/design.md` | Architecture, component contracts, code-reuse map, risks |
| `.specs/features/language-overhaul/tasks.md` | **42 atomic tasks in 8 phases.** This is your work queue |
| `GAPS.md` (repo root) | The defect audit that motivated the whole thing. Read Tier A especially |

Do not start editing before you have read `ideal.md` and `tasks.md` end to end.

## 2. Execution protocol

Activate the **`tlc-spec-driven`** skill by name and follow its Execute flow.
It is the source of truth for the per-task cycle. **If you cannot activate it,
stop and say so — do not proceed without it.**

Its non-negotiable rules, restated because they matter most here:

1. Tests derive from the spec's acceptance criteria and assert **spec-defined
   outcomes** — never mirror the implementation.
2. The gate must pass before a task is done. The test runner decides, not your
   judgment.
3. **One atomic commit per task.** Never batch. Never weaken, skip, or delete a
   test to make it pass.
4. After the final task, a fresh Verifier runs automatically (author ≠ verifier).

Work tasks **in order**. `tasks.md` lists dependencies per task; the phase map
is the execution order.

## 3. What this feature is, in one paragraph

Piperine compiles a hardware description language (PHDL) into a native circuit
simulator. It has **no intermediate representation** — instead it has four
partial, overlapping forms, and the digital half goes from raw AST straight to
Cranelift. In two days of building one small mixed-signal circuit, five defects
surfaced, and **every one lived at a seam between stages**, not inside one. A
1841-test suite missed all five, because every test exercises one stage against
a hand-built fixture, so nothing in the tree can disagree with anything else.
You are building the missing IR — five concepts, printable, with a reference
interpreter that must agree with the JIT backends. The disagreement check is the
point; the cleanup is a side effect.

## 4. The frontier — what you must NOT touch

| Off limits | Why |
|---|---|
| `crates/piperine-solver/` | The `Element` ABI is the frozen frontier. A separate future feature splits analog/digital devices; **not this one** |
| `crates/piperine-api/`, hosts, plugins | Same frontier |
| `tests/ngspice_*.rs`, `tests/support/ngspice.rs`, `tests/ngspice_parity_matrix.rs` | This is the **oracle**. It is validated against real ngspice and it is expensive. Modifying a tolerance to make a test pass forfeits the only thing that can falsify a parity claim (MD-38) |
| `crates/piperine-lang/headers/spice/` | Bounded edits only, each justified in the commit body. These models are the validated asset |
| `Design::modules` / POM navigability | UNBREAKABLE rule (MD-25). The IR is a *separate artifact* built from the POM |

If a task seems to require crossing one of these, **stop and ask**. It means
either the task is wrong or you have misread it.

## 5. Traps that will bite you

These are learned, not hypothetical. Each one already cost real time.

- **A green build is not a correct build.** Every defect this feature exists to
  fix compiled cleanly, ran to completion, and produced plausible statistics
  while being wrong. When you finish a task, ask what would still pass if the
  feature did nothing — if the answer is "everything", the test is wrong.
- **Port `diff.rs`, do not rewrite it.** `piperine-codegen/src/resolve/diff.rs`
  is symbolic differentiation. Its rules were each learned from a failure.
  Rewriting from the mathematics silently drops them. Only the representation
  it walks changes. (Same applies to every family in T11: **move, with the tests
  travelling intact.**)
- **Both bodies carry raw AST today.** `AnalogBody.stmts` and `DigitalBody.stmts`
  are both `Vec<piperine_lang::parse::ast::Stmt>`. The analog side is *not*
  already resolved — only its expressions are. Lowering the analog body is new
  work, not a port. See `ideal.md` §1.7 M1.
- **The IR must never know what a volt is.** Disciplines name their own natures
  (`Thermal` is `temp`/`pwr`, `Magnetic` is `mmf`/`phi`). Ops are
  `read_across`/`read_through`, nature-generic. Hardcoding `V`/`I` breaks every
  non-electrical model, and `headers/spice/` already uses thermal nodes.
- **No catch-all arm.** The worst defect found (`A0c`) was a `parse_dest` that
  ended in `Ok((Flow, GROUND, GROUND))`, so an unrecognised destination became a
  ground-to-ground contribution and the authored value was silently discarded.
  A `_ =>` arm in a match over `Op` or a net kind is a defect, and T17 gates it.
- **`cargo test` bare at the repo root only runs the root package.** Always
  `--workspace`.
- **Zero warnings is the bar**, not a goal (`CLAUDE.md`).
- **No macros. No loose functions.** MD-13 rules 2 and 5. If a helper does not
  belong to a trait or a struct, the abstraction is missing. Data tables + plain
  helpers.
- **The concept budget is five.** `ideal.md` §2. A sixth concept in the IR is a
  signal to stop and re-examine, not to add. T17 gates it mechanically.
- **Invariants live in gates, not documents** (MD-31). If you find yourself
  writing "developers should remember to…", write a test instead. And prove the
  gate can fail: inject the violation, watch it go red, revert.

## 6. Practical notes

- Build: `cargo build --workspace`. Test: `cargo test --workspace`. One crate:
  `cargo test -p <crate>`.
- The CLI binary is at `target/debug/piperine` after `cargo build -p piperine-cli`.
- A full `cargo test --workspace` takes a few minutes. Use `-p` while iterating.
- Python testbenches run via `piperine run script.py`; **stdout needs an
  explicit `sys.stdout.flush()`** or output is lost on error.
- `tests/suite_hygiene.rs` will fail you for: switched-off test code, `#[ignore]`,
  a test target without a `//!` scope header, file-scope lint suppression, and
  dead-architecture identifiers. Read it before writing a new test target.

## 7. Where to start

**T1** in `tasks.md`: rename `piperine-signal` → `piperine-ir`. Mechanical, and
it sets up everything else. Its gate is the inherited `tests/purity.rs` (the
crate must depend on nothing in-tree) plus a zero-warning workspace build.

Then T2 onward, in order.

## 8. Natural checkpoints

- **End of Phase 3 (T17)** — the IR exists, is verified, prints, has a reference
  interpreter and a differential harness, and the four historical defects are
  proven unrepresentable. **The existing compiler is still untouched.** This is
  the safest point to pause and report.
- **End of Phase 5 (T25)** — opamp, buck, and RV32I all lower and simulate,
  proving completeness before any PHDL is migrated.
- **End of Phase 7 (T34)** — the language is on the IR, parity is green, and the
  replaced stages are deleted.

## 9. How to report

After each phase, report: tasks completed with commit hashes, test counts before
and after, gate results, and **any deviation from the plan with its reason**. If
you had to deviate, say so plainly — a silent deviation is worse than a loud
one, and this project's whole thesis is that silence is where defects live.

If you hit something the plan did not anticipate, do not improvise around it.
Say what you found, what the plan assumed, and what you propose. The planning
documents were written to be corrected by evidence — three of `ideal.md`'s five
revisions came from exactly that.
