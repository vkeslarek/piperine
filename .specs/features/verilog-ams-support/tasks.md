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
- [ ] Contribution `<+`, indirect assignment, blocking `=` / non-blocking `<=`
- [ ] `if`/`case`/`casex`/`casez`, `for`/`while`/`repeat`/`forever`, `begin`/`end` named blocks with block item declarations
- [ ] Event control `@(...)` incl. `posedge`/`negedge`, `initial_step`/`final_step`/`cross`/`above`/`timer`
- [ ] `disable`, `$`-task calls, analog `case` (`analog_case`)
- [ ] Gate passes: `cargo test -p piperine-vams`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(vams): statement grammar for analog and procedural blocks`

---

#### T9: Item grammar I — nature, discipline, module, ports, params
**What**: The declaration productions for the core module surface.
**Where**: `crates/piperine-vams/src/grammar/item.rs` (new), `crates/piperine-vams/tests/items_core.rs`
**Depends on**: T8
**Requirement**: VAMS-01, VAMS-04

**Done when**:
- [ ] `nature` incl. derived (`: parent`) and the five standard attributes (§3.6.1.2)
- [ ] `discipline` incl. `domain`, `potential`/`flow` binding, natureless/domainless (§3.6.2)
- [ ] `module`/`macromodule`/`connectmodule`, both port-list styles, `ground`, net declarations with discipline + range + nodeset
- [ ] `parameter`/`localparam`/`aliasparam`/`specparam` incl. value ranges (§3.4.2), string params, arrays
- [ ] Gate passes: `cargo test -p piperine-vams`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(vams): declaration grammar for natures, disciplines and modules`

---

#### T10: Item grammar II — branches, analog blocks, callables, generate
**What**: The behavioral + structural declaration productions.
**Where**: `crates/piperine-vams/src/grammar/item.rs` (extend), `crates/piperine-vams/tests/items_behavior.rs`
**Depends on**: T9
**Requirement**: VAMS-02, VAMS-04

**Done when**:
- [ ] `branch` declarations (named, ground-referenced, port branches)
- [ ] `analog`/`analog initial` blocks, `initial`/`always` constructs
- [ ] `analog function`, `function`, `task` with full port declaration forms
- [ ] `genvar`, `generate` regions: `for`/`if`/`case` generate
- [ ] Module instantiation (named + ordered connections, parameter overrides, `defparam`) and `continuous assign`
- [ ] Gate passes: `cargo test -p piperine-vams`

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

#### T12: `Parser` façade
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

#### T13: Annex A grammar conformance gate
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

#### T14: Real-model corpus harness
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

#### T16: Phase-1 close — S1 verification
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
- [ ] `abstol`/`access`/`units`/`idt_nature`/`ddt_nature` are **named fields**, not map entries (design §5.1 rationale)
- [ ] `parent` supports derived natures (§3.6.1.1); user-defined attributes in a separate map (§3.6.1.3)
- [ ] Implements `Named`/`Kinded` like its siblings
- [ ] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(pom): add the Nature node for LRM 3.6.1`

---

#### T18: POM `Branch`
**What**: `Branch` type per design §5.2 — named and implicit, potential/flow kind.
**Where**: `crates/piperine-lang/src/pom/branch.rs`, `crates/piperine-lang/tests/pom_branch.rs`
**Depends on**: T17
**Requirement**: VAMS-02

**Done when**:
- [ ] Named, implicit and ground-referenced branches representable
- [ ] Contributing both potential and flow to one branch is a typed E4xxx error citing LRM §5, with both contribution spans labelled (E-STD 6)
- [ ] Switch-branch behavior modelled explicitly, not by relaxing that check
- [ ] Gate passes: `cargo test -p piperine-lang`

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
- [ ] E4xxx variants added for: analog-subset violation (Annex C), discipline-resolution failure (Annex F), nature/discipline binding error, connect-module rule violation
- [ ] **Every** new variant has descriptive `#[label]` text, a `#[help]`, and the LRM clause in the help (E-STD 3/4/5) — no `"here"` labels
- [ ] `From<piperine_vams::ParseError> for ElabError` maps structured parse data onto E1xxx diagnostics with labels + help
- [ ] Test asserts each new variant renders code, span, label text and help
- [ ] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): E4xxx LRM-conformance diagnostics with clause citations`

---

#### T20: POM module/design fields for VAMS
**What**: `Module` gains `natures`/`branches`/`genvars`/`paramsets`; `Design` keeps exactly one derived artifact.
**Where**: `crates/piperine-lang/src/pom/{module.rs,design.rs}`, `crates/piperine-lang/tests/pom_vams_shape.rs`
**Depends on**: T19
**Requirement**: VAMS-01, VAMS-04

**Done when**:
- [ ] New collections added; existing fields untouched
- [ ] Test asserts `Design::flat_modules` remains the **only** derived artifact (design §5.3)
- [ ] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(pom): module and design fields for Verilog-AMS constructs`

---

### Phase 5 — VAMS → POM elaboration (S2b)

#### T21: Wire `piperine-vams` into `piperine-lang`; behavior body retarget
**What**: Add the dependency; POM re-exports `piperine_vams::ast::Stmt` as `BehaviorStmt` (design F1 pattern preserved).
**Where**: `crates/piperine-lang/{Cargo.toml,src/pom/behavior.rs}`
**Depends on**: T20
**Requirement**: VAMS-02, VAMS-03

**Done when**:
- [ ] PHDL parse path still compiles and its tests still pass (both frontends alive — S2)
- [ ] `BehaviorStmt` re-export now points at the VAMS AST
- [ ] Gate passes: `cargo test --workspace`

**Tests**: integration · **Gate**: full
**Commit**: `refactor(lang): point POM behavior bodies at the VAMS AST`

---

#### T22: VAMS→POM lowering — modules, ports, params, nets
**What**: The structural lowering pass from `ast::SourceFile` to POM modules.
**Where**: `crates/piperine-lang/src/elab/lower/vams.rs`, `crates/piperine-lang/tests/vams_lower_structure.rs`
**Depends on**: T21
**Reuses**: `elab/lower/module.rs` conventions
**Requirement**: VAMS-04

**Done when**:
- [ ] Modules, ports (both declaration styles), parameters, nets with disciplines lower to POM
- [ ] Instantiation (named + ordered, overrides) produces navigable `Instance`s
- [ ] MD-25 test: authored hierarchy walkable exactly as written
- [ ] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): lower VAMS module structure into the POM`

---

#### T23: VAMS→POM lowering — natures, disciplines, branches
**What**: Lowering the continuous-domain declarations.
**Where**: `crates/piperine-lang/src/elab/lower/vams.rs` (extend), `crates/piperine-lang/tests/vams_lower_analog.rs`
**Depends on**: T22
**Requirement**: VAMS-01, VAMS-02

**Done when**:
- [ ] Natures (incl. derived chains) and disciplines lower with all attributes
- [ ] Named + implicit branches resolve; access functions bind to branches
- [ ] Missing/ambiguous nature binding fails loud with an E4xxx citing §3.6.2.1
- [ ] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): lower natures, disciplines and branches into the POM`

---

#### T24: Retarget `eval/interp.rs` to the VAMS AST
**What**: Const/parameter evaluation over VAMS expressions.
**Where**: `crates/piperine-lang/src/eval/interp.rs`, `crates/piperine-lang/src/elab/const_eval.rs`
**Depends on**: T23
**Requirement**: VAMS-07, R6

**Done when**:
- [ ] Evaluates VAMS constant expressions incl. parameter value ranges (§3.4.2) and `localparam`/`aliasparam`
- [ ] Range violation fails loud with an E4xxx naming the parameter, the offending value and the declared range (E-STD 3/6)
- [ ] Gate passes: `cargo test -p piperine-lang`

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
- [ ] Inserted at PASSES position 5 (after `ElabFns`, before `ElabModules`) per design §6
- [ ] **MD-25 test: `Design::modules` still shows the generate block as authored** — the unrolled form is reachable only through the derived path
- [ ] Name collision between a generated and an authored instance fails loud with both spans labelled
- [ ] Gate passes: `cargo test -p piperine-lang`

**Tests**: integration · **Gate**: quick
**Commit**: `feat(lang): UnrollGenerate pass writing only the derived artifact`

---

#### T26: `ResolveDisciplines` pass (Annex F)
**What**: Discipline resolution for undeclared nets, committing to one Annex F method.
**Where**: `crates/piperine-lang/src/elab/lower/disciplines.rs`, `passes.rs`, `crates/piperine-lang/tests/vams_discipline_resolution.rs`
**Depends on**: T25
**Requirement**: VAMS-17

**Done when**:
- [ ] **Which Annex F method is used is decided here, stated in the pass's `//!` contract, and asserted by the test** — the spec's "documented, not implementation-defined" criterion
- [ ] Behavior matches the LRM's own worked examples for that method
- [ ] Unresolvable discipline fails loud with an E4xxx naming the method applied and why it failed
- [ ] Gate passes: `cargo test -p piperine-lang`

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
   user accepting.
2. **T50 is a hard decision gate** (design R7). If the user declines the
   digital-scheduler scope amendment, T51 and T52 must be re-scoped before
   Phase 9 proceeds.
3. **T26 carries an undecided sub-choice**: which of Annex F's two
   discipline-resolution methods is adopted. Decided in-task against the LRM's
   worked examples, then documented in the pass contract.
4. **Corpus source (T14)**: the out-of-tree compact-model corpus must be
   obtained from upstream publishers (design R2). Not blocking — T14 passes
   with the corpus absent.

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
