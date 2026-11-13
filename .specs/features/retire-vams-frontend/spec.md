# Retire the VAMS Frontend Specification

**Approved**: 2026-11-13 - user: "Pode apagar bixo"

## Problem Statement

`piperine-lang` now holds two frontends: the new MSL parser (`src/msl/`) and the Verilog-AMS lexer, preprocessor, parser, AST, elaborator and POM. MSL is the language going forward; the VAMS code is dead weight that every later MSL feature would have to work around.

## Goals

- [ ] `crates/piperine-lang` contains only the MSL frontend and what it needs.
- [ ] `piperine-lang` builds with zero warnings and all its remaining tests pass.

## Out of Scope

| Feature | Reason |
| ----------- | -------------- |
| Fixing crates that depend on the deleted API (codegen, api, cli, python, project, plugin, lang-server, root) | user: other crates may break; they come back with MSL elaboration |
| Deleting files outside `crates/piperine-lang` (root `examples/*.vams`, `tests/ngspice/`, `VAMS-LRM-2-4.md`, other crates' tests) | they belong to crates that are broken anyway; retired with their own features |
| Porting the spice device models to MSL | later feature; the `.va` sources stay in git history |
| A new object model / elaboration for MSL | next feature |

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | --------------- | --------- | ---------- |
| Delete the VAMS frontend now, before MSL elaboration exists | yes | user: "Pode apagar bixo" (answer to "apago o VAMS já?") | y |
| The POM (`src/pom/`) and `value/`, `math.rs`, `eval/` go too | delete | the POM embeds the VAMS AST (`crate::ast` is used ~45 times in `pom/`); it cannot outlive it. MSL elaboration will define its model | y - follows from "pode apagar"; reversible from git |
| Spice models `headers/spice/*.va` and `headers/*.vams` | delete | they are VAMS sources with no reader left; git history keeps them for porting | y - user: "Pode apagar bixo" |
| `piperine-lang` dev-dependencies on codegen/solver/ndarray | remove | only VAMS tests used them | y |
| Implementation by a Sonnet worker, Opus verifier | as before | user rule | y |

**Open questions:** none.

## User Stories

### P1: Only MSL remains in piperine-lang ⭐ MVP

**User Story**: As the maintainer, I want the VAMS frontend gone from `piperine-lang` so that the crate is the MSL frontend alone.

**Acceptance Criteria**:

1. The `piperine-lang` crate SHALL contain no VAMS frontend source: none of `src/{ast,grammar,elab,eval,pom,value}/`, `src/{lexer,parser,preprocessor,keywords,directives,error,fmt,annex_e,source_map,source_provenance,math,value,vams_include}.rs`, nor `headers/`.  <!-- ubiquitous -->
2. The `piperine-lang` crate SHALL contain no test target other than the `msl_*` targets and their `tests/support/` helpers.  <!-- ubiquitous -->
3. WHEN `cargo build -p piperine-lang --all-targets` runs THEN it SHALL succeed with zero warnings.  <!-- event-driven -->
4. WHEN the five `msl_*` test targets run THEN all their tests SHALL pass with the same test count as before (49).  <!-- event-driven -->
5. The `piperine-lang` `Cargo.toml` SHALL list no dependency that the remaining code does not use.  <!-- ubiquitous -->

**Independent Test**: build the crate and run its tests; list `src/`, `tests/`, `headers/`.

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----------- | ------ | ------- |
| RVF-01 | P1 (AC 1–2) | Execute | Pending |
| RVF-02 | P1 (AC 3–5) | Execute | Pending |

**Coverage:** 2 total, 2 mapped.

## Checks

**Build**: `cargo build -p piperine-lang --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^warning')`
**Run msl**: `cargo test -p piperine-lang --test msl_lexer --test msl_parser --test msl_errors --test msl_corpus --test msl_print`

**C1** - no VAMS frontend source or header remains in `crates/piperine-lang` (RVF-01, AC 1)
Proof: `cd crates/piperine-lang && for p in src/ast src/grammar src/elab src/eval src/pom src/value src/lexer.rs src/parser.rs src/preprocessor.rs src/keywords.rs src/directives.rs src/error.rs src/fmt.rs src/annex_e.rs src/source_map.rs src/source_provenance.rs src/math.rs src/value.rs src/vams_include.rs headers; do test ! -e "$p" || { echo "still present: $p"; exit 1; }; done`

**C2** - only `msl_*` test targets remain (RVF-01, AC 2)
Proof: `cd crates/piperine-lang/tests && test -z "$(ls *.rs | grep -v '^msl_')"`

**C3** - the crate builds with zero warnings (RVF-02, AC 3)
Proof: `cargo build -p piperine-lang --all-targets 2>&1 | (! grep -qE '^(warning|error)')`

**C4** - the 49 MSL tests still pass (RVF-02, AC 4)
Proof: `out=$(cargo test -p piperine-lang --test msl_lexer --test msl_parser --test msl_errors --test msl_corpus --test msl_print 2>&1) && ! echo "$out" | grep -qE 'FAILED|[1-9][0-9]* failed|test result: FAILED' && test "$(echo "$out" | grep -oE '[0-9]+ passed' | awk '{s+=$1} END {print s}')" -eq 49`

**C5** - no unused dependency in `Cargo.toml` (RVF-02, AC 5)
Proof: `cd crates/piperine-lang && code=$(find src tests -name '*.rs' | xargs sed 's@//.*@@') && for d in $(sed -n '/^\[dependencies\]/,/^\[/p' Cargo.toml | grep -oE '^[a-z0-9_-]+'); do n=$(echo $d | tr - _); printf "%s\n" "$code" | grep -qE "\b$n::|use $n\b|extern crate $n" || { echo "unused: $d"; exit 1; }; done && printf "%s\n" "$code" | grep -q 'chumsky::pratt' && printf "%s\n" "$code" | grep -q 'fn depth_over_256' && cargo tree -e features -i chumsky | grep -q 'feature "stacker"' && cargo tree -e features -i chumsky | grep -q 'feature "std"'`
