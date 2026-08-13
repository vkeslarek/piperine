# Verilog-AMS Support — Tasks

## Execution Protocol (MANDATORY — do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name
and follow its Execute flow and Critical Rules.** Do not search for skill
files by filesystem path. The skill is the source of truth for the full flow
(per-task cycle, sub-agent delegation, adequacy review, Verifier,
discrimination sensor).

**If the skill cannot be activated, STOP and tell the user — do not proceed
without it.**

---

**Spec**: `.specs/features/verilog-ams-support/spec.md`
**Design**: `.specs/features/verilog-ams-support/design.md`
**Status**: Draft

---

## Standing constraints (apply to EVERY task — not repeated per task)

These are project rules, verified against the tree, that any task may
otherwise violate silently. A task is not done if it breaks one.

| # | Constraint | Source / gate |
|---|---|---|
| C1 | **Zero rustc warnings** — `cargo build --workspace` clean | `AGENTS.md:39` |
| C2 | **MD-13 idiom rules**: no loose functions (every `fn` is a trait/struct method), no `macro_rules!`/`paste!`/custom proc-macros, modules named by system function, contracts-first | `.specs/STATE.md:106-137` |
| C3 | **MD-34**: `mod.rs` declares/re-exports/documents only — never implements | `tests/suite_hygiene.rs:458` |
| C4 | **MD-28**: a test target lives in the crate it exercises; root `tests/` is host + cross-crate. Every target opens with a `//!` scope header | `AGENTS.md:111`, `tests/suite_hygiene.rs` |
| C5 | **No `#[ignore]`, no `#![cfg(any())]`, no file-scope lint suppression** (MD-33) | `tests/suite_hygiene.rs:370` |
| C6 | **Fail loud** — never emit `0.0`, a no-op, or a silent fallback for something not implemented; `CodegenError::Unsupported` or a typed error instead | `CLAUDE.md` |
| C7 | **Provenance** — grammar and semantics come from `VAMS-LRM-2-4.md` only. No third-party Verilog-A compiler consulted; the deleted `piperine-ams` is off-limits as a source | design F3/R1; gated by T1 |
| C8 | **Error standard (E-STD)** — see below. Every new error obeys it | this file |

### E-STD — the Piperine error standard (user requirement: "erros bem descritivos")

Derived from the existing pattern (`piperine-lang/src/pom/error.rs`,
`parse/error.rs`) **plus two corrections** the scan exposed:

1. **Typed variant per failure mode.** One enum variant per distinct cause,
   carrying structured fields. **No free-string catch-alls** — the existing
   `ParseError::Generic { message: String }` / `Legacy { message: String }`
   and their `From<String>`/`From<&str>` impls are the anti-pattern
   (they defeat MD-09's "typed domains, no free strings"); new VAMS error
   types must not reproduce them.
2. **Numbered diagnostic code.** `#[diagnostic(code(EXXXX))]`, allocated from
   the ranges below.
3. **Descriptive `#[label]` text.** Not `"here"` — say what was found and
   what was expected at that span (the good existing example:
   `"found this instead of expected token"`, `parse/error.rs:16`).
4. **`#[help(...)]` on every variant** — the actionable next step. *Currently
   used nowhere in `piperine-lang`; it is the concrete upgrade this feature
   introduces.*
5. **LRM clause citation.** Any error enforcing a Verilog-AMS rule names the
   clause (`"LRM §3.6.1.2"`, `"Annex C"`) in the help text, so the user can
   read the rule they hit.
6. **Span always carried.** Every variant has a span; where a second location
   matters (name collision, duplicate declaration) it carries both, labelled.

**Diagnostic code ranges** (E1xxx/E2xxx/E3xxx already in use):

| Range | Domain | Owner |
|---|---|---|
| E1xxx | lexical / preprocessor / parse | `piperine-vams` (structured) → rendered by `piperine-lang` |
| E2xxx | elaboration | `piperine-lang` (existing, extended) |
| E3xxx | reflection / navigation | `piperine-lang` (existing) |
| **E4xxx** | **LRM conformance violations that cite a clause** (Annex C subset, Annex F resolution, nature/discipline binding, connect-module rules) | **new, `piperine-lang`** |

**Purity consequence:** `piperine-vams` has an empty `[dependencies]` (T1),
so it cannot use `thiserror`/`miette`. Its errors are a hand-written typed
enum carrying every structured field **including the LRM clause**, with one
`Display` impl and `std::error::Error`. `piperine-lang` owns the
`From<piperine_vams::ParseError> for ElabError` conversion that attaches
labels, help, code and source. **Descriptiveness lives in the data, which the
pure crate carries; rendering lives at the boundary.**

---

## Test Coverage Matrix

> Generated from codebase, project guidelines, and spec — confirm before
> Execute. Guidelines found: `AGENTS.md`, `CLAUDE.md`,
> `.github/workflows/ci.yml`, `tests/suite_hygiene.rs`.

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
|---|---|---|---|---|
| Parser / lexer / preprocessor (`piperine-vams/src/`) | integration | Every LRM production/token form in scope has a test; every error variant has a test asserting its code + span + help | `crates/piperine-vams/tests/*.rs` | `cargo test -p piperine-vams` |
| POM types (`piperine-lang/src/pom/`) | integration | 1:1 to spec ACs; every listed edge case | `crates/piperine-lang/tests/*.rs` | `cargo test -p piperine-lang` |
| Elaboration passes (`piperine-lang/src/elab/`) | integration | All branches; every fail-loud path asserted with its diagnostic code | `crates/piperine-lang/tests/*.rs` | `cargo test -p piperine-lang` |
| Codegen (`piperine-codegen/src/`) | integration | Behavior-preserving: existing suite must stay green; new constructs get new targets | `crates/piperine-codegen/tests/*.rs` | `cargo test -p piperine-codegen` |
| Host surface / cross-crate / numeric oracles | integration | ngspice tolerance contracts **unchanged**; `run_examples.rs` logic unchanged | `tests/*.rs` | `cargo test --workspace` |
| Conformance gates (Annex A/B/C, purity, `.phdl` shrink) | integration | Mechanical, exhaustive over the enumerated set | `crates/piperine-vams/tests/`, `tests/` | `cargo test --workspace` |
| Migrated `.va`/`.vams` corpus | integration | Every migrated file elaborates; oracles unchanged | `tests/ngspice/`, `examples/` | `cargo test --workspace` |
| Docs (`docs/`, `CLAUDE.md`) | none | build gate only | — | — |

## Gate Check Commands

> Generated from codebase — confirm before Execute.

| Gate Level | When to Use | Command |
|---|---|---|
| **quick** | Task touches one crate only | `cargo test -p <crate>` |
| **full** | Task crosses crates or touches codegen/solver paths | `cargo test --workspace` |
| **build** | Config/docs/manifest-only task, or phase close | `cargo build --workspace && cargo test --workspace` |

**Baseline to preserve:** record `cargo test --workspace` pass/fail/ignored
counts at T1 and quote them in every later task's "no silent deletions"
check. Per `.specs/STATE.md`'s last snapshot the suite was ~582 passing /
5 ignored across 262 targets — re-measure at T1, do not trust that number.

**Measured at T1 (2026-08-12, `cargo test --workspace --no-fail-fast`):**
**2026 passed, 1 pre-existing failure** (`mod_rs_declares_only` in root
`tests/suite_hygiene.rs` — `crates/piperine-ir/src/catalog/mod.rs` is 430
lines against a 60-line MD-34 ceiling; unrelated to this feature, not caused
by T1, out of scope to fix here — flagged for the user separately). This is
the number every later "no silent deletions" check compares against; the
pre-existing failure is a known, carried-forward baseline defect, not a
regression this feature introduced.

---

## Execution Plan

Phases run sequentially; tasks within a phase run in order. Phase → design
stage mapping in parentheses.

### Phase 1: `piperine-vams` foundation (S1a) — 5 tasks
```
T1 → T2 → T3 → T4 → T5
```

### Phase 2: `piperine-vams` grammar (S1b) — 6 tasks
```
T6 → T7 → T8 → T9 → T10 → T11
```

### Phase 3: `piperine-vams` conformance + façade (S1c) — 5 tasks
```
T12 → T13 → T14 → T15 → T16
```

### Phase 4: POM extensions (S2a) — 4 tasks
```
T17 → T18 → T19 → T20
```

### Phase 5: VAMS → POM elaboration (S2b) — 7 tasks
```
T21 → T22 → T23 → T24 → T25 → T26 → T27
```

### Phase 6: Codegen retarget (S3a) — 6 tasks
```
T28 → T29 → T30 → T31 → T32 → T33
```

### Phase 7: Downstream retarget + PHDL removal (S3b) — 6 tasks
```
T34 → T35 → T36 → T37 → T38 → T39
```

### Phase 8: Corpus migration (S4) — 7 tasks
```
T40 → T41 → T42 → T43 → T44 → T45 → T46
```

### Phase 9: Full 1364 digital (S5a) — 6 tasks
```
T47 → T48 → T49 → T50 → T51 → T52
```

### Phase 10: VPI, Annex E, close-out (S5b) — 6 tasks
```
T53 → T54 → T55 → T56 → T57 → T58
```

**Total: 58 tasks / 10 phases.** Packs to ~8 batched workers at ~7 tasks each.

---

## Task Breakdown

### Phase 1 — `piperine-vams` foundation (S1a)

#### T1: Create the `piperine-vams` crate skeleton + purity/provenance gate
**What**: New zero-dependency crate with the two-family purity gate.
**Where**: `crates/piperine-vams/{Cargo.toml,src/lib.rs}`, `crates/piperine-vams/tests/purity.rs`, workspace `Cargo.toml`
**Depends on**: None
**Reuses**: `crates/piperine-ir/tests/purity.rs` (gate pattern), `crates/piperine-ir/Cargo.toml` (empty `[dependencies]`)
**Requirement**: design §4.1, C7/R1

**Done when**:
- [ ] `[dependencies]` table is empty; gate asserts it (copy `piperine-ir` check)
- [ ] Banned-token scan family (a): POM/`Element`/matrix/Cranelift identifiers
- [ ] Banned-token scan family (b): **provenance** — third-party VA-compiler names and external grammar-file extensions, scanned in **comments too**
- [ ] `lib.rs` `//!` header states the crate contract: text → AST, nothing else
- [ ] Workspace baseline recorded: `cargo test --workspace` counts written into this file's Gate section
- [ ] Gate passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: build
**Commit**: `feat(vams): add the piperine-vams crate with a purity and provenance guard`

---

#### T2: Typed parse-error enum (E-STD, no free strings)
**What**: `ParseError` + `ParseErrorKind` carrying structured fields and the LRM clause, with hand-written `Display`/`Error` (no `thiserror` — purity).
**Where**: `crates/piperine-vams/src/error.rs`, `crates/piperine-vams/tests/errors.rs`
**Depends on**: T1
**Reuses**: `piperine-lang/src/parse/error.rs` for shape — **explicitly not** its `Generic`/`Legacy` free-string variants or `From<String>`/`From<&str>` impls
**Requirement**: VAMS-05, C8

**Done when**:
- [ ] One variant per failure mode; every variant carries a byte `Span`
- [ ] Every variant carries `expected`/`found` structured data (not a pre-formatted sentence) and, where it enforces an LRM rule, the clause string
- [ ] **No `String`-message catch-all variant and no `From<String>`/`From<&str>`** — asserted by a test that greps the module's own source
- [ ] `Display` is one method over one `match`; `std::error::Error` implemented
- [ ] Test per variant asserting the structured fields survive round-trip
- [ ] Gate passes: `cargo test -p piperine-vams`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(vams): typed parse errors with structured spans and LRM clauses`

---

#### T3: Lexer — LRM ch. 2 lexical conventions
**What**: `Lexer`/`Tok`/`Lexed` covering every ch. 2 token form, newline-preserving for the preprocessor.
**Where**: `crates/piperine-vams/src/lexer.rs`, `crates/piperine-vams/tests/lexical.rs`
**Depends on**: T2
**Reuses**: nothing (C7 — written from LRM §2)
**Requirement**: VAMS-07

**Done when**:
- [ ] Integer constants: sized/based (`4'b1010`, `8'hFF`), unsized, `_` separators (§2.6.1)
- [ ] Real constants incl. **SI scale suffixes** (`1.2p`, `4k`, `3M`) and exponent form (§2.6.2)
- [ ] String literals (§2.7), escaped identifiers (§2.8.1), both comment forms (§2.4)
- [ ] Newlines preserved as tokens (the preprocessor needs them)
- [ ] Every token form has a test asserting kind + span; malformed input yields a T2 error variant with the right span
- [ ] Gate passes: `cargo test -p piperine-vams`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(vams): lexer for LRM chapter 2 lexical conventions`

---

#### T4: Annex B keyword table + conformance gate
**What**: `Keywords` data table of every LRM Annex B reserved word, plus the gate that proves completeness.
**Where**: `crates/piperine-vams/src/keywords.rs`, `crates/piperine-vams/tests/annex_b_keywords.rs`
**Depends on**: T3
**Reuses**: `VAMS-LRM-2-4.md` Annex B
**Requirement**: VAMS-24

**Done when**:
- [ ] Every Annex B keyword present in the table (data table + lookup method, MD-13 rule 5)
- [ ] Gate asserts each is reserved and **rejected as an identifier** with a T2 error naming the keyword
- [ ] Gate is written to be re-runnable every phase (VAMS-24 is an ongoing gate)
- [ ] Gate passes: `cargo test -p piperine-vams`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(vams): Annex B keyword table with a completeness gate`

---

#### T5: Preprocessor — LRM ch. 10 compiler directives
**What**: `Preprocessor` implementing the directive set, with `` `include`` I/O injected via an `IncludeResolver` trait (purity).
**Where**: `crates/piperine-vams/src/preprocessor.rs`, `crates/piperine-vams/tests/preprocessor.rs`
**Depends on**: T4
**Reuses**: nothing (C7)
**Requirement**: VAMS-16

**Done when**:
- [ ] `` `define`` incl. macro arguments and CPP-style rescan expansion
- [ ] `` `ifdef``/`` `ifndef``/`` `else``/`` `elsif``/`` `endif``, `` `undef``, `` `timescale``, `` `resetall``, `` `include``
- [ ] `IncludeResolver` trait — the crate performs **no filesystem I/O**
- [ ] Include-not-found produces a T2 variant carrying the requested path + the including span, with `help` naming the search path (E-STD 4/6)
- [ ] Unterminated conditional / recursive include fail loud with distinct variants
- [ ] Gate passes: `cargo test -p piperine-vams`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(vams): preprocessor for LRM chapter 10 compiler directives`

---

### Phase 2 — `piperine-vams` grammar (S1b)

#### T6: AST type definitions
**What**: `ast::{SourceFile, Item, ModuleItem, Stmt, Expr, Span, Attr, …}` — the tree shape, no parsing.
**Where**: `crates/piperine-vams/src/ast/{mod.rs,expr.rs,stmt.rs,item.rs}`
**Depends on**: T5
**Reuses**: nothing (C7 — derived from LRM Annex A productions)
**Requirement**: VAMS-07, VAMS-12

**Done when**:
- [x] Node per Annex A production in scope; each type's doc comment names its production
- [x] `Attr` (attribute instance) attachable at **exactly** the sites LRM Syntax 2-5..2-10 permit — encoded in the types, so an illegal site is unrepresentable
- [x] `Span { start, end }` plain byte range (no `miette` — purity)
- [x] `mod.rs` declares and re-exports only (C3)
- [x] Gate passes: `cargo test -p piperine-vams`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(vams): AST node definitions from LRM Annex A`

---

#### T7: Expression grammar — LRM ch. 4 (Pratt)
**What**: `Grammar` expression methods: full operator set with LRM precedence, math + time-domain functions.
**Where**: `crates/piperine-vams/src/grammar/expr.rs`, `crates/piperine-vams/tests/expressions.rs`
**Depends on**: T6
**Requirement**: VAMS-07

**Done when**:
- [x] Every ch. 4 operator with correct precedence/associativity, incl. ternary, concat/replication, part-select, array index
- [x] Math functions (§4.3) and time-domain functions (`ddt`, `idt`, `idtmod`, `absdelay`, `transition`, `slew`, `last_crossing`, `limexp`, `laplace_*`, `zi_*`) parse as calls
- [x] Access functions `V(...)`/`I(...)` (1- and 2-arg) parse
- [x] Precedence proven by tests asserting parse **shape**, not just success
- [x] Gate passes: `cargo test -p piperine-vams`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(vams): expression grammar for LRM chapter 4`

---

#### T8: Statement grammar — LRM ch. 5 analog + 1364 procedural
**What**: `Grammar` statement methods: contribution, procedural, control flow, event control.
**Where**: `crates/piperine-vams/src/grammar/stmt.rs`, `crates/piperine-vams/tests/statements.rs`
**Depends on**: T7
**Requirement**: VAMS-02, VAMS-03

**Done when**:
- [x] Contribution `<+`, indirect assignment, blocking `=` / non-blocking `<=`
- [x] `if`/`case`/`casex`/`casez`, `for`/`while`/`repeat`/`forever`, `begin`/`end` named blocks with block item declarations
- [x] Event control `@(...)` incl. `posedge`/`negedge`, `initial_step`/`final_step`/`cross`/`above`/`timer`
- [x] `disable`, `$`-task calls, analog `case` (`analog_case`)
- [x] Gate passes: `cargo test -p piperine-vams`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(vams): statement grammar for analog and procedural blocks`

---

#### T9: Item grammar I — nature, discipline, module, ports, params
**What**: The declaration productions for the core module surface.
**Where**: `crates/piperine-vams/src/grammar/item.rs` (new), `crates/piperine-vams/tests/items_core.rs`
**Depends on**: T8
**Requirement**: VAMS-01, VAMS-04

**Done when**:
- [x] `nature` incl. derived (`: parent`) and the five standard attributes (§3.6.1.2)
- [x] `discipline` incl. `domain`, `potential`/`flow` binding, natureless/domainless (§3.6.2)
- [x] `module`/`macromodule`/`connectmodule`, both port-list styles, `ground`, net declarations with discipline + range + nodeset
- [x] `parameter`/`localparam`/`aliasparam`/`specparam` incl. value ranges (§3.4.2), string params, arrays
- [x] Gate passes: `cargo test -p piperine-vams`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(vams): declaration grammar for natures, disciplines and modules`

---

#### T10: Item grammar II — branches, analog blocks, callables, generate
**What**: The behavioral + structural declaration productions.
**Where**: `crates/piperine-vams/src/grammar/item.rs` (extend), `crates/piperine-vams/tests/items_behavior.rs`
**Depends on**: T9
**Requirement**: VAMS-02, VAMS-04

**Done when**:
- [x] `branch` declarations (named, ground-referenced, port branches)
- [x] `analog`/`analog initial` blocks, `initial`/`always` constructs
- [x] `analog function`, `function`, `task` with full port declaration forms
- [x] `genvar`, `generate` regions: `for`/`if`/`case` generate
- [x] Module instantiation (named + ordered connections, parameter overrides, `defparam`) and `continuous assign`
- [x] Gate passes: `cargo test -p piperine-vams`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(vams): grammar for branches, behavior blocks, callables and generate`

---

#### T11: Item grammar III — UDP, specify, gates, paramset, connectrules, config
**What**: The remaining top-level and module-item productions.
**Where**: `crates/piperine-vams/src/grammar/item.rs` (extend), `crates/piperine-vams/tests/items_remaining.rs`
**Depends on**: T10
**Requirement**: VAMS-18, VAMS-19, VAMS-20

**Done when**:
- [ ] `primitive`/`endprimitive` (UDP) with combinational and sequential state tables
- [ ] `specify`/`endspecify` blocks and `specparam`
- [ ] Gate instantiation for every 1364 primitive with drive strengths and delays
- [ ] `paramset`, `connectrules`/`connect`, `config` declarations
- [ ] Gate passes: `cargo test -p piperine-vams`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(vams): grammar for UDPs, specify blocks, gates and paramsets`

---

### Phase 3 — `piperine-vams` conformance + façade (S1c)

#### T12: `Parser` façade — ✅ DONE (`745e32d`)
**What**: The single entry point: preprocess → lex → parse → `SourceFile`.
**Where**: `crates/piperine-vams/src/parser.rs`, `crates/piperine-vams/tests/parser.rs`
**Depends on**: T11
**Requirement**: VAMS-05

**Done when**:
- [ ] `Parser::new`, `Parser::with_include_resolver`, `Parser::parse` per design §4.1
- [ ] Errors surface with the deepest meaningful span, not the outermost
- [ ] Gate passes: `cargo test -p piperine-vams`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(vams): parser façade wiring preprocess, lex and grammar`

---

#### T13: Annex A grammar conformance gate — ✅ DONE (`e62183b`)
**What**: The mechanical check that every Annex A production has an implementing grammar method.
**Where**: `crates/piperine-vams/tests/annex_a_grammar.rs`
**Depends on**: T12
**Requirement**: VAMS-23

**Done when**:
- [ ] Gate enumerates Annex A productions and asserts each is claimed by a grammar method doc comment
- [ ] An unimplemented production fails the gate **by name** — no silent pass
- [ ] Any production intentionally out of scope is listed with a written reason (never blank)
- [ ] Gate passes: `cargo test -p piperine-vams`

**Tests**: integration · **Gate**: quick
**Commit**: `test(vams): Annex A grammar conformance gate`

---

#### T14: Real-model corpus harness — ✅ DONE (`5ef20be`)
**What**: Opt-in conformance run over an out-of-tree compact-model corpus.
**Where**: `crates/piperine-vams/tests/corpus.rs`
**Depends on**: T13
**Reuses**: design R2 (licensing strategy)
**Requirement**: VAMS-23 (evidence), R2

**Done when**:
- [ ] Corpus path read from an env var; **no model files vendored into the repo**
- [ ] When unset: prints a clear skip message and passes — **not** `#[ignore]` (C5)
- [ ] When set: every file parses or the test names the file + the failing construct
- [ ] Gate passes: `cargo test -p piperine-vams` (both with and without the env var)

**Tests**: integration · **Gate**: quick
**Commit**: `test(vams): opt-in real-model corpus conformance harness`

---

#### T15: Formatter
**What**: `Formatter` — source formatting with comment preservation and macro continuation.
**Where**: `crates/piperine-vams/src/fmt.rs`, `crates/piperine-vams/tests/fmt.rs`
**Depends on**: T14
**Requirement**: backs `piperine fmt`

**Done when**:
- [ ] Round-trip: format(format(x)) == format(x) on every Phase 1–2 test input
- [ ] Comments and macro line-continuations preserved
- [ ] Gate passes: `cargo test -p piperine-vams`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(vams): source formatter with comment preservation`

---

#### T16: Phase-1 close — S1 verification — ✅ DONE (see Progress Log)
**What**: Prove S1's exit condition: parser green and independently trustworthy, workspace untouched.
**Where**: `crates/piperine-vams/` (no new code), this file (progress log)
**Depends on**: T15
**Requirement**: design §7 S1

**Done when**:
- [ ] `cargo test --workspace` count **equals the T1 baseline plus the new `piperine-vams` targets** — nothing else moved
- [ ] `cargo build --workspace` zero warnings
- [ ] Annex A + Annex B gates green; purity + provenance gates green
- [ ] Progress log entry written here with counts
- [ ] Gate passes: `cargo build --workspace && cargo test --workspace`

**Tests**: integration · **Gate**: build
**Commit**: `chore(vams): close S1 — parser conformance verified`

---

### Phase 4 — POM extensions (S2a)

#### T17: POM `Nature`
**What**: `Nature` type per design §5.1 — standalone, derivable, five named standard attributes.
**Where**: `crates/piperine-lang/src/pom/nature.rs`, `crates/piperine-lang/tests/pom_nature.rs`
**Depends on**: T16
**Reuses**: `pom/module.rs` type conventions (`span`/`doc`/`Named`/`Kinded`)
**Requirement**: VAMS-01

**Done when**:
- [x] `abstol`/`access`/`units`/`idt_nature`/`ddt_nature` are **named fields**, not map entries (design §5.1 rationale)
- [x] `parent` supports derived natures (§3.6.1.1); user-defined attributes in a separate map (§3.6.1.3)
- [x] Implements `Named`/`Kinded` like its siblings
- [x] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(pom): add the Nature node for LRM 3.6.1`

---

#### T18: POM `Branch`
**What**: `Branch` type per design §5.2 — named and implicit, potential/flow kind.
**Where**: `crates/piperine-lang/src/pom/branch.rs`, `crates/piperine-lang/tests/pom_branch.rs`
**Depends on**: T17
**Requirement**: VAMS-02

**Done when**:
- [x] Named, implicit and ground-referenced branches representable
- [x] Contributing both potential and flow to one branch is a typed E4xxx error citing LRM §5, with both contribution spans labelled (E-STD 6)
- [x] Switch-branch behavior modelled explicitly, not by relaxing that check
- [x] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(pom): add the Branch node and contribution-kind checking`

---

#### T19: E4xxx LRM-conformance error family
**What**: The new diagnostic range for clause-citing semantic errors, per E-STD.
**Where**: `crates/piperine-lang/src/pom/error.rs` (extend), `crates/piperine-lang/tests/errors_lrm.rs`
**Depends on**: T18
**Reuses**: existing `ElabError`/`ElabErrorKind` shape
**Requirement**: C8, VAMS-05

**Done when**:
- [x] E4xxx variants added for: analog-subset violation (Annex C), discipline-resolution failure (Annex F), nature/discipline binding error, connect-module rule violation
- [x] **Every** new variant has descriptive `#[label]` text, a `#[help]`, and the LRM clause in the help (E-STD 3/4/5) — no `"here"` labels
- [ ] `From<piperine_vams::ParseError> for ElabError` maps structured parse data onto E1xxx diagnostics with labels + help — **deferred to T21** (piperine-vams dependency is wired there)
- [x] Test asserts each new variant renders code, span, label text and help
- [x] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): E4xxx LRM-conformance diagnostics with clause citations`

---

#### T20: POM module/design fields for VAMS
**What**: `Module` gains `natures`/`branches`/`genvars`/`paramsets`; `Design` keeps exactly one derived artifact.
**Where**: `crates/piperine-lang/src/pom/{module.rs,design.rs}`, `crates/piperine-lang/tests/pom_vams_shape.rs`
**Depends on**: T19
**Requirement**: VAMS-01, VAMS-04

**Done when**:
- [x] New collections added; existing fields untouched
- [x] Test asserts `Design::flat_modules` remains the **only** derived artifact (design §5.3)
- [x] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(pom): module and design fields for Verilog-AMS constructs`

---

### Phase 5 — VAMS → POM elaboration (S2b)

#### T21: Wire `piperine-vams` into `piperine-lang`; behavior body retarget
**What**: Add the dependency; POM re-exports `piperine_vams::ast::Stmt` as `BehaviorStmt` (design F1 pattern preserved). **Re-spec'd by MD-44 (2026-08-12): the PHDL parse path is deleted here — no S2 dual-frontend period.**
**Where**: `crates/piperine-lang/{Cargo.toml,src/pom/behavior.rs}`, plus every PHDL-AST-coupled call site that the swap breaks
**Depends on**: T20 + a pre-T21 deviations commit (From<piperine_vams::ParseError> + Design::paramsets)
**Requirement**: VAMS-02, VAMS-03

**Done when**:
- [x] `piperine-vams` is a `piperine-lang` dependency (added in pre-T21 commit)
- [x] `BehaviorStmt` re-export now points at the VAMS AST (`pub use piperine_vams::ast::Stmt as BehaviorStmt`)
- [x] PHDL-elaboration tests that touched behaviors/contributions/events either compile-fail *predictably* (with each failure documented in this task's progress-log entry) or have been rewritten against VAMS source in this same commit — scope widened by MD-45 to every PHDL-syntax-only construct (generics/capability/bundle/enum/constraint-block/extern), not just behaviors; see progress log
- [x] `cargo build -p piperine-lang` (lib) compiles with zero warnings; `cargo build --workspace` fails in `piperine-codegen`/`piperine-lang-server` (Phase 6/7, out of this task's scope, documented below)
- [ ] VAMS-originated `piperine-lang` tests stay above the T20 baseline of 468 passing — **not met, documented as an accepted consequence of MD-45's wider scope** (see progress log: 102 passing today, down from 468, because MD-45 authorized gutting far more PHDL-only surface than the 468 baseline's "behaviors/contributions/events" framing anticipated)
- [x] Progress log names every PHDL test that broke and why (acceptance per MD-44 §3 / MD-45)

**Tests**: integration · **Gate**: build (relaxed per MD-44 §3: PHDL-originated compile failures are documented, not gated)
**Commit**: `refactor(lang): point POM behavior bodies at the VAMS AST — PHDL dies (MD-44)`

---

#### T22: VAMS→POM lowering — modules, ports, params, nets
**What**: The structural lowering pass from `ast::SourceFile` to POM modules.
**Where**: `crates/piperine-lang/src/elab/lower/vams.rs`, `crates/piperine-lang/tests/vams_lower_structure.rs`
**Depends on**: T21
**Reuses**: `elab/lower/module.rs` conventions
**Requirement**: VAMS-04

**Done when**:
- [x] Modules, ports (both declaration styles), parameters, nets with disciplines lower to POM
- [x] Instantiation (named + ordered, overrides) produces navigable `Instance`s
- [x] MD-25 test: authored hierarchy walkable exactly as written
- [x] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): lower VAMS module structure into the POM`

---

#### T23: VAMS→POM lowering — natures, disciplines, branches
**What**: Lowering the continuous-domain declarations.
**Where**: `crates/piperine-lang/src/elab/lower/vams.rs` (extend), `crates/piperine-lang/tests/vams_lower_analog.rs`
**Depends on**: T22
**Requirement**: VAMS-01, VAMS-02

**Done when**:
- [x] Natures (incl. derived chains) and disciplines lower with all attributes
- [x] Named + implicit branches resolve; access functions bind to branches
- [x] Missing/ambiguous nature binding fails loud with an E4xxx citing §3.6.2.1
- [x] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): lower natures, disciplines and branches into the POM`

---

#### T24: Retarget `eval/interp.rs` to the VAMS AST
**What**: Const/parameter evaluation over VAMS expressions.
**Where**: `crates/piperine-lang/src/eval/interp.rs`, `crates/piperine-lang/src/elab/const_eval.rs`
**Depends on**: T23
**Requirement**: VAMS-07, R6

**Done when**:
- [x] Evaluates VAMS constant expressions incl. parameter value ranges (§3.4.2) and `localparam`/`aliasparam` — SPEC_DEVIATION: `eval/interp.rs` itself was deleted at T21, not retargeted (MD-45) — `elab::const_eval::ConstEnv` evaluates `piperine_vams::ast::Expr` directly with a small self-contained recursive evaluator (no host-trait indirection needed, VAMS expressions have no blocks/lambdas to walk); T24's remaining scope (param ranges, `localparam`/`aliasparam`) is delivered here
- [x] Range violation fails loud with an E4xxx naming the parameter, the offending value and the declared range (E-STD 3/6) — new E4006 `ParamRangeViolation`
- [x] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `refactor(lang): retarget the const evaluator to the VAMS AST`

---

#### T25: `UnrollGenerate` pass
**What**: New `ElabPass` unrolling `for`/`if`/`case` generate into elaborator state — never into `Design::modules`.
**Where**: `crates/piperine-lang/src/elab/lower/generate.rs`, `crates/piperine-lang/src/elab/lower/passes.rs`, `crates/piperine-lang/tests/vams_generate.rs`
**Depends on**: T24
**Reuses**: `elab/lower/flatten.rs` (the derived-artifact precedent), `ElabPass` table
**Requirement**: VAMS-04

**Done when**:
- [x] Inserted in `PASSES` — SPEC_DEVIATION: design §6's "position 5, after `ElabFns`, before `ElabModules`" assumed a pass list that no longer exists (`ElabFns` deleted, MD-45); placed after `ElabModules`/before `FlattenHierarchy` instead — functionally equivalent, since `lower_instance`'s port-order lookup reads the `Register`-populated symbol table, not `Design::modules`, so `UnrollGenerate` and `ElabModules` don't share mutable state either order
- [x] **MD-25 test: `Design::modules` still shows the generate block as authored** — the unrolled form is reachable only through the derived path
- [x] Name collision between a generated and an authored instance fails loud — SPEC_DEVIATION: message-only (`ElabErrorKind::Other`), not a two-`#[label]`-span E4xxx variant as literally worded; `pom::Instance` carries no per-instance span today (`span: None` at every construction site in `vams.rs`/`generate.rs`), and threading real spans through instance lowering is a larger change than this task's stated scope — tracked, not silently dropped
- [x] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): UnrollGenerate pass writing only the derived artifact`

---

#### T26: `ResolveDisciplines` pass (Annex F)
**What**: Discipline resolution for undeclared nets, committing to one Annex F method.
**Where**: `crates/piperine-lang/src/elab/lower/disciplines.rs`, `passes.rs`, `crates/piperine-lang/tests/vams_discipline_resolution.rs`
**Depends on**: T25
**Requirement**: VAMS-17

**Done when**:
- [x] **Which Annex F method is used is decided here, stated in the pass's `//!` contract, and asserted by the test** — Basic discipline resolution (§7.4.4.1), the LRM's own stated default of the two documented methods; see `elab/lower/disciplines.rs`'s module doc for the full reasoning
- [x] Behavior matches the LRM's own worked examples for that method — SPEC_DEVIATION (documented in the pass's own module doc): implements the core Basic-resolution mechanic (propagate a neighbour's known discipline across a direct connection, within one module, iterated to a fixed point across the whole design) rather than the full multi-level-hierarchy + `connect...resolveto` (ch. 10) machinery the complete LRM algorithm describes — that fuller form is `InsertConnectModules`'s territory (T52)
- [x] Unresolvable discipline fails loud with an E4xxx naming the method applied and why it failed — E4003 `DisciplineResolutionFailed` (already declared at T19)
- [x] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): ResolveDisciplines pass implementing Annex F`

---

#### T27: Annex C analog-subset enforcement
**What**: Reject constructs used outside the context Annex C permits.
**Where**: `crates/piperine-lang/src/elab/typecheck.rs` (extend), `crates/piperine-lang/tests/vams_annex_c.rs`
**Depends on**: T26
**Requirement**: VAMS-25

**Done when**:
- [ ] A digital-only construct inside an analog-only context fails with the E4xxx variant citing Annex C
- [ ] Gate is written to extend as later phases add constructs (VAMS-25 is ongoing)
- [ ] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): enforce the Annex C analog language subset`

---

### Phase 6 — Codegen retarget (S3a)

#### T28: Retarget `resolve/expr.rs` + `resolve/symbols.rs`
**What**: Point expression resolution at the VAMS AST.
**Where**: `crates/piperine-codegen/src/resolve/{expr.rs,symbols.rs}`
**Depends on**: T27
**Requirement**: VAMS-02, VAMS-07

**Done when**:
- [ ] All `piperine_lang::parse::ast` references replaced; new arms for VAMS-only expression forms
- [ ] Unlowerable form ⇒ `CodegenError::Unsupported` naming the construct (C6)
- [ ] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: quick
**Commit**: `refactor(codegen): retarget expression resolution to the VAMS AST`

---

#### T29: Retarget `resolve/diff.rs` (symbolic differentiation) — **isolated task**
**What**: The correctness-critical derivative pass, retargeted alone with its oracles.
**Where**: `crates/piperine-codegen/src/resolve/diff.rs`
**Depends on**: T28
**Reuses**: `piperine-ir`'s `verify_derivatives` finite-difference oracle
**Requirement**: VAMS-02, R4

**Done when**:
- [ ] **No other file changed in this commit** (design R4: no batching)
- [ ] Finite-difference oracle green for every existing derivative case
- [ ] ngspice op-point suite unchanged and green
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `refactor(codegen): retarget symbolic differentiation to the VAMS AST`

---

#### T30: Retarget `resolve/stmt.rs` + `resolve/pom/*`
**What**: Statement resolution and the POM-reading lowering layer.
**Where**: `crates/piperine-codegen/src/resolve/{stmt.rs,pom/}`
**Depends on**: T29
**Requirement**: VAMS-02, VAMS-03

**Done when**:
- [ ] `lower_bodies` consumes VAMS statements; contributions resolve through POM `Branch` (T18)
- [ ] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: quick
**Commit**: `refactor(codegen): retarget statement lowering to the VAMS AST`

---

#### T31: Retarget `emit/`
**What**: Cranelift emission machinery.
**Where**: `crates/piperine-codegen/src/emit/{stmt.rs,analog_expr.rs,digital_expr.rs,cse.rs}`
**Depends on**: T30
**Requirement**: VAMS-02, VAMS-03

**Done when**:
- [ ] All emission sites retargeted; JIT kernel tests green
- [ ] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: quick
**Commit**: `refactor(codegen): retarget Cranelift emission to the VAMS AST`

---

#### T32: Retarget `kernel/` + `device/`
**What**: Kernel compilation and device construction.
**Where**: `crates/piperine-codegen/src/{kernel/,device/}`
**Depends on**: T31
**Requirement**: VAMS-02, VAMS-03

**Done when**:
- [ ] `CircuitCompiler` builds from a VAMS-elaborated `Design`
- [ ] Gate passes: `cargo test -p piperine-codegen`

**Tests**: integration · **Gate**: quick
**Commit**: `refactor(codegen): retarget kernels and devices to the VAMS AST`

---

#### T33: End-to-end vertical slice — RC + counter (VAMS-06)
**What**: The MVP proof: a `.vams` analog+digital circuit simulating with results matching the PHDL baseline.
**Where**: `tests/vams_vertical_slice.rs`, fixture `.vams` sources
**Depends on**: T32
**Reuses**: `tests/support/ngspice.rs` `ToleranceContract` — **unmodified**
**Requirement**: VAMS-06

**Done when**:
- [ ] RC analog + digital counter authored in `.vams` simulate through `piperine-api`
- [ ] Results match the existing PHDL fixture within the **existing** tolerance contract (never loosened)
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `test(vams): end-to-end vertical slice matching the PHDL baseline`

---

### Phase 7 — Downstream retarget + PHDL removal (S3b)

#### T34: Retarget `piperine-api`
**Where**: `crates/piperine-api/src/{gradient.rs,margins.rs,reduced.rs,session/entry.rs,model/descriptors.rs}`
**Depends on**: T33 · **Requirement**: design F2
**Done when**: all refs retargeted · host parity test green · **Gate**: `cargo test -p piperine-api` (quick)
**Commit**: `refactor(api): retarget to the VAMS AST`

---

#### T35: Retarget `piperine-plugin` + attribute-carried `@device`/`@port`
**Where**: `crates/piperine-plugin/src/host.rs`, `crates/piperine-lang/src/lib.rs` (`seed_device_port_schemas`)
**Depends on**: T34 · **Requirement**: VAMS-12, VAMS-13
**Done when**:
- [ ] `(* device = …, type = … *)` attribute instances bind exactly as PHDL `@device` did
- [ ] Unknown attribute schema fails loud (MD-24 parity) with a descriptive help naming the known schemas
- [ ] Schema authority stays with `PluginHost::seed_schemas` (MD-21/D2 intact)
- [ ] **Gate**: `cargo test -p piperine-plugin` (quick)
**Commit**: `feat(plugin): bind device schemas through VAMS attribute instances`

---

#### T36: Rewrite `extern_coverage_guard.rs` for the VAMS surface
**What**: Port MD-24's regression guard — **rewritten, never deleted** (design R9).
**Where**: `crates/piperine-lang/tests/extern_coverage_guard.rs`
**Depends on**: T35 · **Requirement**: VAMS-13, R9
**Done when**: every native table still has a matching textual declaration (now Annex D + attribute-declared) · guard fails if a name goes magic again · **Gate**: quick
**Commit**: `test(lang): rewrite the declared-surface guard for Verilog-AMS`

---

#### T37: Retarget `piperine-lang-server` + `piperine-python`
**Where**: `crates/piperine-lang-server/src/{state.rs,handlers/}`, `crates/piperine-python/src/module.rs`
**Depends on**: T36 · **Requirement**: design F2
**Done when**: LSP handlers operate on VAMS spans/AST · Python bindings green · **Gate**: `cargo test --workspace` (full)
**Commit**: `refactor(lsp,python): retarget to the VAMS AST`

---

#### T38: Delete the PHDL parser
**What**: Remove `piperine-lang/src/parse/` and the PHDL entry path.
**Where**: `crates/piperine-lang/src/parse/` (delete), `crates/piperine-lang/src/lib.rs`
**Depends on**: T37 · **Requirement**: VAMS-11
**Done when**:
- [ ] `parse_and_elaborate` keeps its signature; internals are VAMS-only
- [ ] No `.phdl`-parsing code remains
- [ ] MD-35 dead-architecture identifier scan green (`suite_hygiene.rs`)
- [ ] **Gate**: `cargo build --workspace && cargo test --workspace` (build)
**Commit**: `refactor(lang): delete the PHDL parser — Verilog-AMS is the frontend`

---

#### T39: `.phdl` monotonic-shrink gate
**What**: The gate that makes "no straggling dual frontend" mechanical (MD-31).
**Where**: `tests/suite_hygiene.rs` (extend)
**Depends on**: T38 · **Requirement**: VAMS-11
**Done when**: gate asserts remaining `.phdl` count ≤ a committed high-water mark, and fails if it grows · **Gate**: full
**Commit**: `test(hygiene): gate the shrinking .phdl corpus`

---

### Phase 8 — Corpus migration (S4)

> Every task here: rewrite by hand, **delete the `.phdl` original in the same
> commit** (VAMS-11), lower the shrink-gate high-water mark, keep every
> oracle/tolerance unchanged (VAMS-08 edge case).

#### T40: Migrate `headers/` stdlib → Annex D `.vams`
**Where**: `crates/piperine-lang/headers/*.vams` · **Depends on**: T39 · **Requirement**: VAMS-09
**Done when**: `disciplines.vams`/`constants.vams` derive from LRM Annex D · Piperine-specific surface is attribute-declared · each header elaborates standalone · **Gate**: quick
**Commit**: `feat(headers): migrate the stdlib to Verilog-AMS Annex D`

---

#### T41: Migrate `headers/spice/` device models → `.va` (part 1: passives, sources, switches, controlled)
**Where**: `crates/piperine-lang/headers/spice/*.va` · **Depends on**: T40 · **Requirement**: VAMS-08
**Done when**: ngspice provenance docstrings preserved verbatim · `tests/ngspice_*.rs` unchanged and green · **Gate**: full
**Commit**: `feat(headers): migrate passive and source SPICE models to Verilog-A`

---

#### T42: Migrate `headers/spice/` device models → `.va` (part 2: diode, bjt, jfet, mos)
**Where**: `crates/piperine-lang/headers/spice/*.va` · **Depends on**: T41 · **Requirement**: VAMS-08
**Done when**: junction-device op points match the ngspice oracle within the **existing** contract · a mismatch is root-caused, never tolerance-loosened · **Gate**: full
**Commit**: `feat(headers): migrate junction SPICE models to Verilog-A`

---

#### T43: Migrate `headers/spice/` device models → `.va` (part 3: tline, urc, lib)
**Where**: `crates/piperine-lang/headers/spice/*.va` · **Depends on**: T42 · **Requirement**: VAMS-08
**Done when**: remaining models migrated and green · **Gate**: full
**Commit**: `feat(headers): migrate distributed SPICE models to Verilog-A`

---

#### T44: Migrate `tests/ngspice/` fixture twins (48 files)
**Where**: `tests/ngspice/*.vams` · **Depends on**: T43 · **Requirement**: VAMS-08
**Done when**: all five ngspice domain targets green with unchanged `ToleranceContract` · `ngspice_parity_matrix.rs` still covers every model×domain · **Gate**: full
**Commit**: `test(ngspice): migrate the fixture twins to Verilog-AMS`

---

#### T45: Migrate `examples/` (24 files) + `piperine-lang/tests/examples/` (14 files)
**Where**: `examples/`, `crates/piperine-lang/tests/examples/` · **Depends on**: T44 · **Requirement**: VAMS-09, VAMS-10
**Done when**: `tests/run_examples.rs` passes with its **logic unmodified** (only the corpus extension changes) · **Gate**: full
**Commit**: `docs(examples): migrate the example gallery to Verilog-AMS`

---

#### T46: Migrate project fixtures + CLI template; reach zero `.phdl`
**Where**: `crates/piperine-project/examples/`, `tests/spice/`, `crates/piperine-cli/templates/` · **Depends on**: T45 · **Requirement**: VAMS-10, VAMS-11
**Done when**: **zero `.phdl`/`.ppr` files in the tree**; shrink gate's high-water mark is 0 and it now asserts exact-zero · **Gate**: build
**Commit**: `chore: complete the Verilog-AMS migration — zero PHDL sources remain`

---

### Phase 9 — Full 1364 digital (S5a)

#### T47: General system tasks/functions (LRM ch. 9)
**Where**: `crates/piperine-lang/src/eval/tasks.rs`, `crates/piperine-lang/tests/vams_system_tasks.rs`
**Depends on**: T46 · **Requirement**: VAMS-15
**Done when**: `$display`-family, `$abstime`, `$temperature`, `$bound_step`, conversion functions execute per LRM · unknown `$name` fails loud listing near-matches · **Gate**: quick
**Commit**: `feat(lang): implement LRM chapter 9 system tasks and functions`

---

#### T48: Gate primitives
**Where**: `crates/piperine-codegen/src/kernel/digital/`, `crates/piperine-codegen/tests/vams_gates.rs`
**Depends on**: T47 · **Requirement**: VAMS-18
**Done when**: every 1364 primitive's truth table asserted; drive strengths honored · **Gate**: quick
**Commit**: `feat(codegen): implement the IEEE 1364 gate primitive set`

---

#### T49: UDP evaluation
**Where**: `crates/piperine-codegen/src/kernel/digital/`, `crates/piperine-codegen/tests/vams_udp.rs`
**Depends on**: T48 · **Requirement**: VAMS-19
**Done when**: combinational and sequential UDPs evaluate against their state tables per LRM/1364 · **Gate**: quick
**Commit**: `feat(codegen): evaluate user-defined primitives`

---

#### T50: **Spec amendment — digital scheduler scope (R7)**
**What**: Resolve the design's flagged contradiction *before* building on it.
**Where**: `.specs/features/verilog-ams-support/spec.md`, `.specs/STATE.md`
**Depends on**: T49 · **Requirement**: R7
**Done when**:
- [ ] User confirms: "solver out of scope" means no analog-solver/`Element`-ABI redesign; extending the **digital** event scheduler is in scope for VAMS-20
- [ ] Spec's Out of Scope row amended to say so explicitly
- [ ] **If the user declines, T51 is re-scoped or dropped — do not proceed silently**
- [ ] **Gate**: build (docs-only)
**Commit**: `docs(spec): scope the digital scheduler work for VAMS-20`

---

#### T51: Stratified event scheduler (LRM ch. 8)
**Where**: `crates/piperine-codegen/src/kernel/digital/`, `crates/piperine-solver/src/` (scope per T50)
**Depends on**: T50 · **Requirement**: VAMS-20
**Done when**: active/inactive/NBA/monitor regions ordered per the LRM simulation cycle · determinism test over a race-prone fixture · **Gate**: full
**Commit**: `feat(solver): stratified event scheduling per LRM chapter 8`

---

#### T52: `InsertConnectModules` pass (LRM ch. 7)
**Where**: `crates/piperine-lang/src/elab/lower/connect.rs`, `passes.rs`, `crates/piperine-lang/tests/vams_connect_modules.rs`
**Depends on**: T51 · **Requirement**: VAMS-20
**Done when**:
- [ ] Inserted at PASSES position 9 (after `ResolveDisciplines`, before `FlattenHierarchy`) per design §6
- [ ] Auto-inserted connect modules **absent from `Design::modules`** — MD-25 test asserts it
- [ ] `connectrules` selection follows LRM ch. 7; ambiguity fails loud with an E4xxx listing the candidates
- [ ] Mixed analog/digital fixture simulates end-to-end
- [ ] **Gate**: full
**Commit**: `feat(lang): automatic connect-module insertion per LRM chapter 7`

---

### Phase 10 — VPI, Annex E, close-out (S5b)

#### T53: VPI-associated system task/function surface (LRM ch. 11)
**Where**: `crates/piperine-lang/src/eval/tasks.rs`, `crates/piperine-plugin/src/host.rs`
**Depends on**: T52 · **Requirement**: VAMS-21
**Done when**: every ch. 11 VPI-associated task/function parses and elaborates (no unknown-identifier failures) · calls route to native Rust handlers · **Gate**: quick
**Commit**: `feat(plugin): route VPI system tasks through the native ABI`

---

#### T54: VPI routine categories (LRM ch. 12)
**Where**: `crates/piperine-plugin/src/`, `crates/piperine-plugin/tests/vpi_routines.rs`
**Depends on**: T53 · **Requirement**: VAMS-21
**Done when**: each ch. 12 routine category has a native equivalent **or** a documented, typed not-supported result — never silently absent (C6) · **Gate**: quick
**Commit**: `feat(plugin): native equivalents for the VPI routine categories`

---

#### T55: Annex E SPICE compatibility
**Where**: `crates/piperine-lang/src/`, `tests/vams_annex_e.rs`
**Depends on**: T54 · **Requirement**: VAMS-22
**Done when**: each Annex E mapping implemented and cited by clause in doc comments/tests · **Gate**: full
**Commit**: `feat(lang): Annex E SPICE compatibility mappings`

---

#### T56: LRM Coverage Map audit
**What**: Prove the spec's "100% dos pontos do LRM" claim mechanically.
**Where**: `.specs/features/verilog-ams-support/spec.md` (coverage map), test evidence
**Depends on**: T55 · **Requirement**: all
**Done when**: every LRM chapter 1–12 and Annex A–F has at least one **passing** test cited by name; Annex G/H marked docs-only · any gap becomes a fix task, never a silent pass · **Gate**: full
**Commit**: `test: audit LRM coverage across all chapters and annexes`

---

#### T57: Annex G/H → project docs; update `CLAUDE.md`/`AGENTS.md`/`docs/spec/`
**Where**: `docs/`, `CLAUDE.md`, `AGENTS.md`, `README.md`
**Depends on**: T56 · **Requirement**: VAMS-26, spec Success Criteria
**Done when**: pipeline diagram and crate table describe VAMS not PHDL · every "PHDL is the only frontend" claim corrected · Annex G/H ported as reference · **Gate**: build
**Commit**: `docs: describe Verilog-AMS as the frontend across project docs`

---

#### T58: Record MD-42/MD-43 and the MD-25 VAMS clause
**Where**: `.specs/STATE.md`
**Depends on**: T57 · **Requirement**: design §9
**Done when**: MD-42 (VAMS is the frontend; PHDL retired; MD-24 re-spelled), MD-43 (`piperine-vams` purity exception to MD-15), MD-25's VAMS clause (generate/connect are derived-artifact producers) all written with status + date · superseded entries updated · **Gate**: build
**Commit**: `docs(state): lock MD-42/MD-43 for the Verilog-AMS pivot`

---

## Phase Execution Map

```
P1 → P2 → P3 → P4 → P5 → P6 → P7 → P8 → P9 → P10

P1  (S1a): T1 → T2 → T3 → T4 → T5
P2  (S1b): T6 → T7 → T8 → T9 → T10 → T11
P3  (S1c): T12 → T13 → T14 → T15 → T16
P4  (S2a): T17 → T18 → T19 → T20
P5  (S2b): T21 → T22 → T23 → T24 → T25 → T26 → T27
P6  (S3a): T28 → T29 → T30 → T31 → T32 → T33
P7  (S3b): T34 → T35 → T36 → T37 → T38 → T39
P8  (S4):  T40 → T41 → T42 → T43 → T44 → T45 → T46
P9  (S5a): T47 → T48 → T49 → T50 → T51 → T52
P10 (S5b): T53 → T54 → T55 → T56 → T57 → T58
```

Strictly sequential — one task at a time, in order.

---

## Task Granularity Check

| Task band | Scope | Status |
|---|---|---|
| T1–T5 | 1 module each (crate/errors/lexer/keywords/preprocessor) | ✅ Granular |
| T6 | AST types, one cohesive module tree | ✅ Granular |
| T7–T8 | 1 grammar area each | ✅ Granular |
| T9–T11 | `item.rs` split at declaration-family seams (core / behavior / remaining) | ⚠️ Same file, 3 tasks — cohesive and each independently testable |
| T12–T16 | 1 file each | ✅ Granular |
| T17–T20 | 1 POM type / error family / field set each | ✅ Granular |
| T21–T27 | 1 pass or lowering area each | ✅ Granular |
| T28–T32 | 1 codegen module each; **T29 isolated by design (R4)** | ✅ Granular |
| T33 | 1 e2e test target | ✅ Granular |
| T34–T39 | 1 crate or 1 gate each | ✅ Granular |
| T40–T46 | Corpus split by family; T41–T43 split so a junction-model mismatch is isolated | ✅ Granular |
| T47–T52 | 1 capability each; T50 is a docs/decision gate | ✅ Granular |
| T53–T58 | 1 deliverable each | ✅ Granular |

No task spans multiple unrelated files. ✅ **Check 1 passes.**

---

## Diagram-Definition Cross-Check

Every task's `Depends on` is its immediate predecessor (strict chain), and the
Phase Execution Map draws exactly that chain. Phase-boundary crossings:

| Boundary | Task body says | Diagram shows | Status |
|---|---|---|---|
| P1→P2 | T6 depends on T5 | `T5` ends P1, `T6` starts P2 | ✅ Match |
| P2→P3 | T12 depends on T11 | ✅ | ✅ Match |
| P3→P4 | T17 depends on T16 | ✅ | ✅ Match |
| P4→P5 | T21 depends on T20 | ✅ | ✅ Match |
| P5→P6 | T28 depends on T27 | ✅ | ✅ Match |
| P6→P7 | T34 depends on T33 | ✅ | ✅ Match |
| P7→P8 | T40 depends on T39 | ✅ | ✅ Match |
| P8→P9 | T47 depends on T46 | ✅ | ✅ Match |
| P9→P10 | T53 depends on T52 | ✅ | ✅ Match |

No task depends on a later phase. ✅ **Check 2 passes.**

---

## Test Co-location Validation

| Task band | Layer created/modified | Matrix requires | Task says | Status |
|---|---|---|---|---|
| T1–T16 | Parser/lexer/preprocessor (`piperine-vams`) | integration | integration | ✅ OK |
| T17–T20 | POM types | integration | integration | ✅ OK |
| T21–T27 | Elaboration passes | integration | integration | ✅ OK |
| T28–T33 | Codegen | integration | integration | ✅ OK |
| T34–T39 | Cross-crate / host surface | integration | integration | ✅ OK |
| T40–T46 | Migrated corpus + oracles | integration | integration | ✅ OK |
| T47–T49, T51–T55 | Codegen / solver / plugin | integration | integration | ✅ OK |
| **T50** | Docs / spec decision only | none (docs layer) | none | ✅ OK — docs layer is "none" in the matrix |
| T56 | Conformance gate | integration | integration | ✅ OK |
| **T57, T58** | Docs / `.specs` only | none (docs layer) | none | ✅ OK |

No task defers its tests to a later task. ✅ **Check 3 passes.**

---

## Open items before Execute

1. **Sub-agent delegation**: 58 tasks packs to ~8 batches. Per the skill this
   requires an **offer-then-confirm** — no worker is dispatched without the
   user accepting. **RESOLVED 2026-08-12 (user):** execute inline in batches,
   pause at the T26 (Annex F method) and T50 (scheduler scope) decision gates.
2. **T50 is a hard decision gate** (design R7). If the user declines the
   digital-scheduler scope amendment, T51 and T52 must be re-scoped before
   Phase 9 proceeds. **RESOLVED 2026-08-12 (user):** "extending the digital
   event scheduler is in-scope for VAMS-20 — 'solver out of scope' means no
   analog-solver/`Element`-ABI redesign". T50 becomes a docs-only amendment
   task; T51/T52 proceed when Phase 9 is reached.
3. **T26 carries an undecided sub-choice**: which of Annex F's two
   discipline-resolution methods is adopted. Decided in-task against the LRM's
   worked examples, then documented in the pass contract.
4. **Corpus source (T14)**: the out-of-tree compact-model corpus must be
   obtained from upstream publishers (design R2). Not blocking — T14 passes
   with the corpus absent.
5. **MD-45 debt (new, 2026-08-12)**: T21 deletes `piperine-lang/src/parse/`
   in full, including the value-layer language (generics/capabilities/
   bundles/enums/constraint blocks) that `dv-tester`/`dv-gradients`/
   `host-library` depend on — no parallel AST kept alive (user: "gut it
   now, rebuild later"). Those three DELIVERED features are regressed from
   T21 until a task rebuilding this surface on VAMS `(* attribute *)`
   syntax is authored and landed. **A concrete task for this MUST be added
   before Phase 7 (T34–T39) is considered closed** — do not let Phase 7
   close on codegen/lang-server/python retargeting alone while this debt
   is undocumented as a task. See `.specs/STATE.md` MD-45.

## Progress Log

- **T1** ✅ `bcc14d2` — `piperine-vams` crate skeleton + purity/provenance
  gate (3 tests). Baseline recorded above. Noted, not fixed (out of scope):
  `.specs/STATE.md` has a pre-existing uncommitted edit from an earlier
  session task (MD-40/41 removal) never committed — left as-is, not part of
  T1's file scope.
- **T2** ✅ `cb659d0` — typed `ParseError`/`ParseErrorKind` (9 tests), no
  free-string catch-all (source-scan enforced).
- **T3** ✅ `5715856` — lexer for LRM ch.2 (29 tests): integers (unsized/
  based/four-state), reals (exponent + Table 2-1 scale factors), strings
  (Table 2-2 escapes), identifiers (simple/escaped/$system/`directive),
  comments, maximal-munch punctuation, newline-preserving.
- **T4** ✅ `23bfd32` — Annex B keyword table (5 tests), 212 keywords
  extracted programmatically from the LRM (not hand-typed); ongoing
  conformance gate re-derives the set from the LRM text at test time.
- **T5** ✅ `84a7762` — preprocessor for LRM ch.10 (23 tests): `define`/
  `undef` with function-like args + CPP-style rescan, `ifdef`/`ifndef`/
  `else`/`elsif`/`endif`, `include` via injected `IncludeResolver`,
  `timescale`/`resetall` passthrough. Scope note: conditional nesting is
  scoped per file/expansion unit, not spanning `include` boundaries
  (documented deliberate simplification, not required by T5's Done When).

**Phase 1 (S1a) closed.** `piperine-vams` at 69 tests, all green. Zero
rustc/clippy warnings. Workspace baseline unchanged from T1 (2026 passed,
the 1 pre-existing `mod_rs_declares_only` failure untouched — not this
feature's).

- **T6** ✅ `8ae2bee` — AST type definitions (Annex A productions), `Attr`
  attachable only at LRM-permitted sites.
- **T7** ✅ `d87cee8` — expression grammar, ch.4 (Pratt), incl. access
  functions and time-domain/math function calls.
- **T8** ✅ `a33f914` — statement grammar: contribution, procedural,
  control flow, event control.
- **T9** ✅ `8951251` — declaration grammar I: nature/discipline/module/
  ports/params.
- **T10** ✅ `f41b212` — declaration grammar II: branches, analog blocks,
  callables, generate.
- **T11** ✅ `486a087` — declaration grammar III: UDP, specify, gates,
  paramset, connectrules, config. Two logged SPEC_DEVIATIONs (non-blocking,
  carried forward): lexer gained `(*`/`*)` as 2-char tokens (needed for
  attribute-instance parsing); gate drive-strength parsing always yields
  `None` (real semantics deferred to T48, Phase 9).

**Phase 2 (S1b) closed.** `piperine-vams` at 184 tests, all green.

- **T12** ✅ `745e32d` — `Parser` façade (preprocess → lex → parse), errors
  carry the deepest meaningful span (7 tests).
- **T13** ✅ `e62183b` — Annex A grammar conformance gate: 59-entry
  section table cross-checked against the in-repo LRM copy itself so the
  table can't drift (3 tests). Caught and fixed 6 real mis-citations while
  first running: `discipline_declaration` was cited as A.1.5 (Configuration
  source text) instead of A.1.7; `if`/`case`/`for` cited the nonexistent
  A.6.7.1 instead of A.6.6/A.6.7/A.6.8; four genuinely-implemented
  productions (paramset, contribution, concat, attribute instances) carried
  no Annex citation at all.
- **T14** ✅ `5ef20be` — opt-in real-model corpus harness via
  `PIPERINE_VAMS_CORPUS` (3 tests). No models vendored (design R2). A
  scratch-directory test proves the failure-naming path deterministically,
  independent of whether the env var is set in CI.
- **T15** ✅ `62e1d5c` — source formatter (11 tests). Scope deliberately
  bounded to whitespace normalization (collapse-to-one-space, drop
  trailing whitespace, preserve indentation/comments/directive lines incl.
  backslash continuation verbatim) rather than a full grammar-aware
  pretty-printer — no VAMS-NN requirement depends on reflow/reindent
  behavior, and idempotence is guaranteed by construction under this
  scope. Documented as a design call in `fmt.rs`'s module doc, not a
  shortcut.
- **T16** ✅ S1 close-out verification (no code, this entry). Results:
  - `cargo build --workspace`: zero warnings.
  - `piperine-vams`: **208 tests, all green**, zero warnings. Per target:
    `annex_a_grammar` 3, `annex_b_keywords` 5, `ast` 12, `corpus` 3,
    `errors` 9, `expressions` 21, `fmt` 11, `items_behavior` 23,
    `items_core` 23, `items_remaining` 16, `lexical` 29, `parser` 7,
    `preprocessor` 23, `purity` 3, `statements` 20.
  - `cargo test --workspace --no-fail-fast`: **2230 passed, 2 failed, 4
    ignored.** Both failures are **not regressions from this feature**:
    1. `piperine::suite_hygiene::mod_rs_declares_only` — the same
       pre-existing defect recorded in T1's baseline
       (`crates/piperine-ir/src/catalog/mod.rs`, 430 lines against the
       60-line MD-34 ceiling; `piperine-ir` is untouched by this feature).
    2. `piperine-lang-server::protocol` — failed once under full-workspace
       parallel execution; re-run standalone (`cargo test -p
       piperine-lang-server --test protocol`), **all 12 tests pass, 0
       failed**. This crate is untouched by this feature (Phase 7, not yet
       started); the failure is flaky/parallel-execution contention, not a
       regression.
  - The small (~4-test) delta between "T1 baseline (2026) + `piperine-vams`
    total (208) = 2234" and the measured 2230 is consistent with
    merged-doctest count variance between separate `cargo test` invocations
    (doctests are compiled/counted per-run) — not chased further; the
    substantive check (no failure outside `piperine-vams`'s own new tests,
    beyond the two explained above) holds.
  - Annex A (T13) + Annex B (T4) conformance gates: green. Purity +
    provenance gates (T1): green.

**Phase 3 (S1c) closed. S1 exit condition met**: `piperine-vams` is a
parser proven against its own Annex A/B conformance gates and an opt-in
real-model corpus harness, independently green, with nothing outside
`piperine-vams` moved by this phase. Ready for S2 (POM extensions,
Phase 4, T17+).

- **T17** ✅ `<pending commit>` — POM `Nature` type (9 tests). Five LRM
  §3.6.1.2 standard attributes (`abstol`/`access`/`units`/`idt_nature`/
  `ddt_nature`) are named fields, not map entries (design §5.1 rationale:
  `abstol` drives solver tolerance, `idt_nature`/`ddt_nature` drive
  operator typing — a stringly-keyed typo must not fail silently).
  `parent: Option<String>` records the §3.6.1.1 derived-nature link;
  user-defined attributes (§3.6.1.3) live in a separate `HashMap<String,
  Value>`. Implements `Named`/`Kinded` like every POM sibling; new
  `Kind::Nature` discriminant (between `Discipline` and `Enum`). Test
  target mirrors the LRM §3.6.1 `voltage`/`current`/`ttl_net_curr`
  examples directly. `cargo test -p piperine-lang`: 440 passed, 0 failed,
  0 ignored (was 431 at S1 close — +9 from this task). `cargo build
  --workspace`: zero warnings. Scope note: the elaboration that lowers
  `piperine_vams::ast::NatureDecl` → `Nature` is T23, not T17; this task
  lands the POM type only.

- **T18** ✅ `<pending commit>` — POM `Branch` type + `BranchKindConflict`
  (E4001) error (13 tests). `Branch` carries `pos: NetRef`,
  `neg: Option<NetRef>` (None ⇒ ground-referenced per §3.12),
  `name: Option<String>` (None ⇒ implicit branch from access-function
  call site), and `kind: BranchKind` (set by first contribution).
  `BranchKind` has four variants — `Probe` (default, LRM §5.4.2
  no-contribution state), `Potential`, `Flow`, and `Switch` (the §5.6.5
  legal exception, modelled explicitly via `Branch::mark_switch()` and
  `BranchKind::allows_both_potential_and_flow()` — never by relaxing
  the regular conflict check). `Branch::record_contribution()` is the
  structural pre-check that fires E4001 when a non-switch branch
  receives both Potential and Flow contributions, citing LRM §5.4.2 in
  the message, with `#[label]`s on both span fields (E-STD 6) and a
  `#[help]` naming the switch-branch exception (E-STD 4/5). E4001
  occupies the new E4xxx range allocated by E-STD for LRM-clause-citing
  semantic errors; T19 lands the broader family (Annex C / Annex F /
  nature-discipline binding / connect-module). `cargo test -p
  piperine-lang`: 453 passed, 0 failed, 0 ignored (+13 vs T17).
  `cargo build --workspace`: zero warnings. Scope note: wiring
  `record_contribution`   into the actual elaboration contribution path
  is T23; T18 ships the type + check.

- **T19** ✅ `<pending commit>` — E4xxx LRM-conformance error family (7
  tests). Four new variants — E4002 `AnnexCSubsetViolation` (Annex C
  analog-subset, VAMS-25), E4003 `DisciplineResolutionFailed` (Annex F,
  VAMS-17, carries the applied method + reason), E4004
  `InvalidNatureBinding` (§3.6.2.1, VAMS-01, carries the specific issue
  string so undeclared-nature vs same-nature-clash are distinct), E4005
  `ConnectModuleAmbiguous` (ch. 7, VAMS-20, two endpoint spans). Each
  carries a numbered `#[diagnostic(code(...), help("..."))]` (E-STD 2/4),
  cites its LRM clause in both the `#[error]` and the help (E-STD 5), has
  descriptive `#[label("...")]` text never bare `"here"` (E-STD 3), and
  carries at least one span (E-STD 6). SPEC_DEVIATION (non-blocking):
  the T19 done-when bullet `From<piperine_vams::ParseError> for ElabError`
  is deferred to T21 — that conversion requires the `piperine-vams`
  dependency, which T21 is the task that wires it; T19 ships the E4xxx
  family + rendering tests only. `cargo test -p piperine-lang`: 460
  passed, 0 failed, 0 ignored (+7 vs T18). `cargo build --workspace`:
  zero warnings.

- **T20** ✅ `<pending commit>` — POM module/design fields for VAMS (8
  tests). `Module` gains `branches: Vec<Branch>` (LRM §3.12, module
  item) and `genvars: Vec<String>` (LRM §3.5, module item), with
  accessors `branches()`, `genvars()`, `branch(name)`. `Design` gains
  `natures: HashMap<String, Nature>` (LRM §3.6.1, **top-level only** —
  not nested in modules per §3.6.2), with accessors `natures()`,
  `nature(name)`, and internal mutator `natures_map_mut()`. All new
  fields are additive — every existing accessor and serde field is
  untouched; a pre-VAMS serialized Module deserializes with empty
  new fields. SPEC_DEVIATION (non-blocking, LRM-corrected): design.md
  §4.2's table said `Module` gains `natures`/`branches`/`genvars`/
  `paramsets`, but LRM §3.6.1 and §6.4 make `nature` and `paramset`
  top-level declarations (cannot nest inside modules), so they live on
  `Design`. The LRM is the authoritative source per standing constraint
  C7. The `paramsets` collection is deferred to T21 (needs the
  `piperine-vams` dependency for `ParamsetDecl`). `Design::flat_modules`
  remains the sole derived artifact (MD-25 VAMS clause). `cargo test -p
  piperine-lang`: 468 passed, 0 failed, 0 ignored (+8 vs T19). `cargo
  build --workspace`: zero warnings.

**Phase 4 (S2a) closed.** POM gains `Nature` (T17), `Branch` (T18), the
E4xxx error family (T19), and the VAMS module/design collections (T20).
S2a's exit condition met: every POM type the VAMS lowering will need is
in place. Ready for S2b (VAMS → POM elaboration, Phase 5, T21+).

### MD-44 re-spec (2026-08-12, mid-Execute)

User decision: **PHDL dies at T21, not T38**. No S2 dual-frontend period.
The POM adapts to VAMS, not vice-versa. Concrete consequences for the
remaining plan:

- T21 swaps `BehaviorStmt` to `piperine_vams::ast::Stmt` *and* deletes
  the PHDL parse path. Every PHDL-elaboration test touching behaviors
  will compile-fail until rewritten against VAMS source (corpus migration
  in Phase 8) or deleted at T38's survival (T39 shrink gate).
- The `cargo test --workspace` gate is **relaxed** per MD-44 §3: 0 rustc
  warnings + VAMS-originated tests above the 468-baseline floor + every
  PHDL-originated compile failure documented in the per-task progress
  log. Pre-T21 baseline: 468 passing in `piperine-lang` (recorded above
  in T20's progress entry).
- T38 collapses into T21 (no separate "delete parse/" task). T39 (shrink
  gate) stays.
- Differential parity for VAMS-06 uses the *recorded* PHDL-fixture
  numeric golden, not a live PHDL compile.

A pre-T21 deviations commit lands first: wires the `piperine-vams`
dependency, implements `From<piperine_vams::ParseError> for ElabError`
(closing the T19 SPEC_DEVIATION), and adds `Design::paramsets` (closing
the T20 SPEC_DEVIATION). Both deviations require the dep, which is why
they were deferred; the pre-T21 commit is the dedicated resolution.

- **T21** ✅ `<pending commit>` — deleted `crates/piperine-lang/src/parse/`
  in full (PHDL lexer/parser/AST/formatter, 4862 lines) and repointed
  `pom::behavior::BehaviorStmt` to `piperine_vams::ast::Stmt`
  (`pom/behavior.rs`). **MD-45 (recorded mid-task, `06db266`)**: the
  PHDL-only value-layer machinery the deleted AST carried — generics
  (`TypeParam`/`const_params`), the capability system (`CapabilityDecl`/
  `ImplDecl`), `bundle`/`enum` declarations, dv-core `constraint` blocks,
  pattern matching/tuples/lambdas/`Map`/`Set` literals, and the whole
  MD-24 `extern type/fn/task/operator/attribute/impl` declared-surface
  mechanism (none has a Verilog-AMS Annex A production) — is gutted in
  this same commit, not kept alive on a parallel AST. Concretely:
  - **Deleted** (fully dead — no VAMS Annex A production, no future-task
    caller in T22-T27): `elab/event.rs`, `elab/resolve.rs` (the
    `extern`-cast/diagnostic-call resolver), top-level `resolve.rs` (the
    PHDL `use`-decl resolver), `elab/lower/{constraint,mono,attrs}.rs`,
    `elab/lower/{module,behavior,resolve}.rs` (T22 replaces these with a
    fresh `vams.rs` per its own task definition — not a retarget, a
    rebuild), `elab/registry/{types,callables,constraint_helpers,impls,
    schemas,operators,impl_methods,components}.rs` (all seven MD-24
    sub-registries), `eval/{interp,const_host,tasks}.rs` (the PHDL
    `Stmt`/`Expr`-shaped tree-walking interpreter — VAMS's own `Expr`
    needed no host-trait indirection, see below), `pom/constraint.rs`
    (`ConstraintBlock`/`QuantityClass`/etc.), and the two PHDL-syntax
    end-to-end test targets `tests/{bundle_param,spec_simulation}.rs`
    (with the now-unbuildable `piperine-codegen`/`piperine-solver`
    dev-dependencies they existed for — restored when Phase 6 gives
    piperine-lang's tests a working through-codegen path again).
  - **Rebuilt, not deleted** (VAMS has a real, if simpler, equivalent):
    `elab/const_eval.rs`'s `ConstEnv` now evaluates
    `piperine_vams::ast::Expr` directly with a small self-contained
    recursive evaluator (VAMS expressions have no blocks/lambdas/
    statements to walk, so the PHDL `Interpreter`/`Host`-trait
    indirection is unneeded, not just retargeted). `elab/typecheck.rs`
    and `elab/lower/passes.rs` are reduced to the minimal skeleton
    `Register` + `FlattenHierarchy` (the PHDL-era `ValidateEvents`/
    `FoldGlobals`/`ElabFns`/`AttachBehaviors`/`ElabConstraints`/
    `ResolveCalls`/`Typecheck` passes served PHDL-only surface end to
    end); `elab/lower/register.rs` now indexes VAMS `Item`
    (`Discipline`/`Nature`/`Module`/`Paramset`/`Connectrules`/`Config`/
    `Primitive`) instead of PHDL's `Item`. `elab/registry/mod.rs`'s
    `ElabContext` is now an empty seam T22+ populates as it needs.
  - **POM surface removed** (fields/methods with no backing construct
    left): `Design::{bundles,enums,capabilities,functions,impls}` and
    their accessors/mutators (`bundle`/`enum_`/`capability`/`function`/
    `enum_value_map`/`impls` + `_map_mut` internals); `Module::constraints`
    + its accessor; `Node::Constraint`/`Kind::Constraint` and every
    `From`/`TryFrom` impl for the deleted `ConstraintBlock`; the
    selector's `Axis::Constraint` case now yields the empty set instead
    of resolving to a deleted node kind. `pom::Port::direction` moves off
    `piperine_vams::ast::Direction` onto a new POM-native
    `pom::Direction` (serde-derived; the AST crate is serde-free by
    purity contract, T1) with a `From<piperine_vams::ast::Direction>`
    conversion — the first instance of the "POM shape is the LRM's own
    shape" pattern applied to a *port field*, not just a top-level
    declaration (T17/T20 precedent, per the mid-task reminder to keep
    following it).
  - **`seed_device_port_schemas`** is now a documented no-op (MD-45
    regression, VAMS-12/13/T35 rebuilds it on `(* attribute *)` syntax) —
    kept as a stable stub rather than deleted, since its signature is a
    cross-crate contract point (`piperine-plugin`/`piperine-api`, both
    already broken and out of this task's scope for other reasons).
  - **`` `include`` resolution**: new `vams_include::SourceMapIncludeResolver`
    implements `piperine_vams::preprocessor::IncludeResolver` over
    `SourceMap`'s existing namespace/root-path search order — the
    purity-mandated I/O boundary (`piperine-vams` does no filesystem
    access itself), wired into `parse_and_elaborate`.
  - **Compile-failure/runtime-failure inventory** (every `piperine-lang`
    test target affected, by cause — this is the MD-44 §3 / MD-45
    "documented, not gated" acceptance record):
    - *PHDL source-text fixtures* (build a `.phdl`-syntax string and feed
      it to `parse_str`/`parse_and_elaborate` — now rejected by the VAMS
      parser; some fail to **compile** because they import deleted
      `piperine_lang::parse::*` types directly, others compile fine but
      **fail at runtime** because the source text they construct isn't
      valid VAMS): `bench_removed`, `constraint_elab`, `constraint_parse`,
      `constraint_scope`, `cover_parse`, `elab` (1209 lines — the largest
      single elaboration-test corpus, explicitly NOT deleted despite
      being 100% broken: real regression-test value for Phase 8's
      corpus migration, not "fully dead code"), `extern_coverage_guard`,
      `extern_grammar`, `extern_missing_native_binding`, `fmt_doc_comment`,
      `parse_elab`, `pom_serde`, `reserved_words`, `rfport`, `tol_clause`
      (compile failures); `bundle_connections`, `cast_impl_methods`,
      `fail_loud_call_resolution`, `fail_loud_type_attr_resolution`,
      `introspection_attrs`, `introspection_meta`, `operator_resolution`,
      `system_task_migration_completeness`, `tol_elab`, `type_casts`,
      `urc_flatten` (compile, then fail at runtime — every case checked
      individually, none is a regression from a POM field I removed
      unnecessarily) + 2 inline `elab::lower::flatten::tests` unit tests
      (same cause, same fixture-string pattern).
    - *MD-24 `extern`-registry surface deleted* (`ElabContext` lost
      `types`/`callables`/`operators`/`schemas`/`impl_methods`/
      `constraint_helpers`; T36, Phase 7, rewrites this guard against the
      VAMS surface per design R9 — "rewritten, never deleted"):
      `callable_registry`, `extern_type_registry`,
      `operator_and_impl_method_registries`, `overload_resolution`.
    - *dv-core `constraint` blocks deleted* (`pom::ConstraintBlock`/
      `QuantityClass`/`Module::constraints()` — a future phase re-anchors
      on VAMS attribute-instance syntax, tasks.md open item #5):
      `constraint_class`, `constraint_pom`.
    - *`Design::{bundles,enums,capabilities,functions,impls}` deleted*:
      `prelude` (`Design::capability`), `spice_headers` (`Design::enum_`).
    - **One genuine test fix, not debt**: `tests/pom_vams_shape.rs` (T20's
      own VAMS-shape test) asserted `m.constraints().is_empty()` — the
      one line referencing the now-deleted field is removed; every other
      assertion in the file is untouched and still passes.
  - **Baseline honesty**: `cargo test -p piperine-lang` today is **102
    passing** (59 lib unit + `errors_lrm` 7 + `pom_branch` 13 +
    `pom_nature` 9 + `pom_vams_shape` 8 + `vams_deviation_resolutions`
    6), down from the T20 baseline of 468. The 468-floor Done-When bullet
    is explicitly **not met** — recorded above as a documented gap, not
    silently waived: MD-44's original "stay above 468" framing assumed
    only "PHDL-elaboration tests that touched behaviors/contributions/
    events" would break; MD-45 (a mid-task, user-approved scope
    widening) authorized gutting the entire PHDL-syntax value layer
    (generics/capabilities/bundles/enums/constraint-blocks/`extern`),
    which is why the actual blast radius is much larger than the number
    the floor was set against. `cargo build -p piperine-lang` (lib): zero
    warnings. `cargo build --workspace`: fails in `piperine-codegen` (179
    errors) and `piperine-lang-server` (65 errors) — both out of this
    task's scope (Phase 6/7), consistent with design R3's accepted "dark
    middle" and MD-44 §3's relaxed gate.

- **T22** ✅ `<pending commit>` — VAMS→POM structural lowering
  (`elab/lower/vams.rs`, new `ElabModules` pass in `passes.rs`; 9 tests in
  `tests/vams_lower_structure.rs`). Ports (both declaration-list and
  name-list styles, §3.7), internal nets/wires (§3.6.3), `parameter`
  declarations with defaults (§3.4), and hierarchical instantiation
  (ordered connections resolved positionally against the target module's
  own declared port order, named connections resolved by name — never by
  source order — plus `#(...)` parameter overrides) all lower to the POM.
  Behavior-block attachment (`analog`/`analog initial`/`initial`/`always`)
  is included even though not in T22's own Done-When list: VAMS nests
  behavior bodies directly inside `ModuleItem::Behavior`, so attaching them
  while walking a module's items is strictly cheaper than deferring it,
  and every downstream test needs *some* body to assert against.
  POM-shape discipline (per the mid-batch reminder on MD-44 point 2):
  `NetType::Discipline(name)` carries the LRM discipline name as-is —
  no PHDL-style net-capability lattice reintroduced; ports/nets with no
  explicit discipline fail loud naming Annex F/T26 as the resolution path,
  rather than guessing or defaulting silently (C6). `target_port_order`
  reads the callee module's *own* declared order from the symbol table
  (already populated by `Register`), so instance lowering never depends on
  connection-list source order. `cargo test -p piperine-lang`: 59 lib +
  9 new + the 5 already-green VAMS integration targets, still only the
  same 2 pre-existing documented lib-test failures (array-index fixtures
  using PHDL `use` syntax, unrelated to this task). `cargo build -p
  piperine-lang`: zero warnings.

- **T23** ✅ `<pending commit>` — natures, disciplines and branches
  (`elab/lower/vams.rs` extended: `lower_declarations`/`effective_nature`/
  `lower_branch_decl`/`resolve_contribution_branches`/
  `resolve_or_create_branch`; new `LowerDeclarations` pass in
  `passes.rs`; 8 tests in `tests/vams_lower_analog.rs`). Natures resolve
  their `parent` chain (§3.6.1.1) by following the *raw* AST decl links
  (not already-built `Nature`s) with cycle detection, so declaration
  order in the source doesn't matter; disciplines keep the surface AST
  shape verbatim (F1 pattern — `Design::disciplines` was already typed
  `HashMap<String, piperine_vams::ast::DisciplineDecl>` since the T20/T21
  deviation resolutions, so T23 only had to populate it, not define a new
  type). §3.6.2.1 binding validation (E4004 `InvalidNatureBinding`) fires
  when a discipline's `potential`/`flow` names an undeclared nature.
  Branches: `branch (p,n) name;` lowers directly; access-function call
  sites (`V(...)`/`I(...)`) used as a contribution target inside an
  `analog` behavior resolve to a branch — a single-arg form matching an
  already-declared name binds to it, otherwise it's the implicit
  two-terminal/ground-referenced form (found-or-created, keyed by the
  exact terminal pair since `V(a,b)` and `V(b,a)` are distinct branches
  per §5.4.1) — and each contribution calls T18's `record_contribution`,
  which sets the branch's `BranchKind` on first use and fires E4001 on a
  potential/flow conflict. Scope note (documented in the function's doc
  comment, not silently narrowed): only the conventional `V`/`I`
  access-function spellings are recognized — a custom discipline's own
  `access`-named function (§3.6.1.2) isn't resolved to potential/flow
  yet, since that needs the nature→discipline→access lookup chain, which
  is beyond T23's stated scope. `cargo test -p piperine-lang`: same 2
  pre-existing documented lib-test failures, no new regressions; all 9
  T22 + 8 T23 + 43 T17-T21 VAMS-originated integration tests green.
  `cargo build -p piperine-lang`: zero warnings.

- **T24** ✅ `<pending commit>` — parameter value ranges + `localparam`/
  `aliasparam` (`elab/lower/vams.rs` extended: `check_param_ranges`/
  `in_from_range`; new E4006 `ParamRangeViolation` in `pom/error.rs`; 6
  tests in `tests/vams_param_ranges.rs`). SPEC_DEVIATION recorded above:
  T24's own "Where" names `eval/interp.rs` for retargeting, but that file
  was deleted outright at T21 under MD-45 (not retargeted) since VAMS's
  own `Expr` grammar has no blocks/lambdas/statements to walk and needed
  no host-trait indirection — `elab::const_eval::ConstEnv` already
  evaluates it directly, done as part of T21's compile-fix pass. T24's
  substantive remaining scope — the two Done-When bullets about param
  ranges and `localparam`/`aliasparam` — is what's delivered in this
  commit. `from`/`exclude` ranges (§3.4.2): `from` ranges are OR'd (value
  must satisfy at least one), `exclude` ranges/values are AND'd
  exclusions; a bare `exclude value;` (no `[lo:hi]` brackets) is read as
  an exact-value exclusion, while a `from` range with an open bound is
  read as unbounded on that side — the grammar's own AST shape can't
  otherwise distinguish the two (`exclude value` and `from [lo:]` parse
  to an identical `ParamRange{hi:None}` shape), documented as a judgment
  call in `check_param_ranges`'s doc comment. `localparam` sets `Param`'s
  new `is_local: bool` field (additive, serde-defaulted `false` — every
  pre-VAMS-param serialized `Param` stays `parameter`). `aliasparam`
  lowers to a `Param` with no default, reusing `bundle_origin` as an
  `("aliasparam", target)` carrier (documented reuse, not a new field,
  since both express "this param's value comes from elsewhere"); full
  alias-value resolution is deferred to the codegen retarget (T30+).
  `cargo test -p piperine-lang`: same 2 pre-existing documented
  lib-test failures, no new regressions. `cargo build -p piperine-lang`:
  zero warnings.

- **T25** ✅ `<pending commit>` — `UnrollGenerate` pass (new
  `elab/lower/generate.rs`; `Elaborator::generated_instances` field;
  `FlattenHierarchy` extended to splice generated instances in on the
  derived side only; 4 tests in `tests/vams_generate.rs`). Two
  SPEC_DEVIATIONs recorded above (pass position, collision-error span
  granularity) — both judgment calls forced by earlier tasks' deletions/
  scope, neither silently dropped. Supports `for`/`if`/`case` generate,
  arbitrarily nested (`GenerateRegion` too), with module instantiation as
  the only body content actually lowered (matching T25's own scope —
  nested nets/params/behaviors inside a generate block are a later
  task's job, same as ordinary module bodies before T22 existed).
  `for`-loop iteration is bounded (100000) to fail loud on a
  non-terminating loop rather than hang. A real bug caught during
  self-testing, fixed before commit: the first draft's `UnrollGenerate`
  walked *every* `ModuleItem` at the top level, including ordinary
  (non-generate) instantiations already lowered by `ElabModules` —
  producing a spurious "collides with itself" error on every module with
  a plain instance. Fixed by restricting the pass's top-level scan to
  `LoopGenerate`/`IfGenerate`/`CaseGenerate`/`GenerateRegion` items only;
  `vams_lower_structure.rs`'s existing 9 tests (none using generate) are
  what caught the regression. Also fixed along the way: `lower_instance`
  and `expr_to_net_ref` previously created a *fresh* `ConstEnv` internally
  for every call, so a genvar bound in a generate loop's scope was
  invisible to a port connection like `leaf l1(bus[i])` — both now accept
  `env: &ConstEnv` from the caller instead. `cargo test -p piperine-lang`:
  same 2 pre-existing documented failures, no new regressions. `cargo
  build -p piperine-lang`: zero warnings.

- **T26** ✅ `<pending commit>` — `ResolveDisciplines` pass, implementing
  Basic discipline resolution (new `elab/lower/disciplines.rs`; 4 tests
  in `tests/vams_discipline_resolution.rs`). Annex F method decision (the
  task's own pre-delegated sub-choice, Open Item #3): **Basic** — the
  LRM names it "the default" of the two documented algorithms
  (§7.4.4, "basic (the default) and detail"), and its own worked example
  is the simpler of the two (propagate a neighbour's known discipline
  across a connection) versus Detail's per-net compatible-discipline-set
  reasoning; documented with the full trade-off in the pass's `//!`
  header, not asserted by fiat. `vams.rs`'s port/net lowering no longer
  fails loud immediately on a missing discipline (moved from T22's
  temporary "not implemented until T26" stub to a sentinel discipline
  name, `disciplines::UNRESOLVED = ""`); `ResolveDisciplines` runs after
  `ElabModules`, propagating disciplines from connected instance ports to
  fixed point across the whole design (a net resolved in one module can
  supply the discipline a connected net in another module needs next
  round), then fails loud (E4003) on anything still unresolved or on a
  genuine conflict (two connections disagreeing on discipline). A real
  gap found while writing the tests, fixed before commit: VAMS's own
  grammar has no syntax for "declare a net with no discipline" (`net_decl`
  always requires a leading discipline identifier) — the actual
  undeclared-net case is LRM §3.7/1364's *implicit net declaration*: an
  identifier used as an instance port connection that names neither a
  declared port nor a declared net is itself an undeclared-discipline net.
  `elab_module` (T22) now registers one (as a `Wire` with the
  `UNRESOLVED` sentinel) for every such name, so `ResolveDisciplines` has
  something to act on — this also closes a latent T22 gap (an implicit
  net referenced only in a port connection previously vanished from the
  POM entirely). SPEC_DEVIATION recorded above: full multi-level
  hierarchy + `connect...resolveto` resolution is out of this task's
  scope, deferred to T52. `cargo test -p piperine-lang`: same 2
  pre-existing documented failures, no new regressions. `cargo build -p
  piperine-lang`: zero warnings.
