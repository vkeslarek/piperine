# Spice Models Tasks

## Execution Protocol (MANDATORY -- do not skip)

Implement these tasks with the `tlc-spec-driven` skill: **activate it by name and follow its Execute flow and Critical Rules.** Do not search for skill files by filesystem path. The skill is the source of truth for the full flow (per-task cycle, sub-agent delegation, the walk, check_proofs, the Verifier and the screens sub-agent).

**If the skill cannot be activated, STOP and tell the user - do not proceed without it.**

---

**Design**: `.specs/features/spice-models/design.md`
**Status**: Approved
**Approved**: 2026-11-27 - user (translated, relayed by the coordinator): "Please write the design and break it into tasks!"
**Build**: `cargo build --workspace --all-targets 2>&1 | tee /dev/stderr | (! grep -qE '^(warning|error)')`; `cargo test -p piperine-repository-guards --test suite`
**Run language**: `cargo test -p piperine-language`
**Run alias**: `cargo test -p piperine-language --test alias`
**Run stdlib**: `cargo test -p piperine-language --test stdlib`
**Run host**: `cargo test -p piperine-api`
**Run lsp**: `cargo test -p piperine-language-server`
**Run parity-<family>**: `cargo test -p piperine-parity --test spice_<family>` (families: `passive`, `source`, `switch`, `line`, `diode`, `jfet`, `mesfet`, `bipolar`, `mosfet`, `bsim`, `soi`, `hisim`)
**Run gates**: `cargo test -p piperine-repository-guards --test spice`
**Run book**: `cargo test -p piperine --test book`
**Run spec**: `cargo test -p piperine-repository-guards --test documentation`

**This feature executes after `code-clarity` and after `device-abi-simplification` have both merged to `main`.** The models compile onto the new device ABI, phase 9 adds a term operator to it, and the `alias` amendment changes the front end those features restructure. Every path, crate name and test target below names the tree as those two features leave it; T1 re-takes them on the merged `main` and corrects this file before anything else is written.

**The reference is ngspice-47**, for the sources a model is written from and for the oracle the parity targets run. A checkout of the tag is needed by the tasks that write a model (`git clone --depth 1 --branch ngspice-47 https://git.code.sf.net/p/ngspice/ngspice` into the scratch directory); `~/Git/ngspice` is ngspice-26 and is not the reference.

**The ABI version does not move** (user, 2026-11-27, on `device-abi-simplification`). Phase 9 changes the contract in place.

Executors: Sonnet, except the tasks marked **Opus**: the language core (`alias` resolution and binding, on-demand loading, `convolve`), the solver and ABI core, and the numerically delicate models (the junction functions, the diode, the transistors' currents and charges, the lines, the compact models). Opus verifies the feature; an author never verifies their own work. Fix rounds one to three are Sonnet's; from the fourth, Opus, by an agent that is not the Verifier.

**Lanes.** Phases 0 to 4 run in order in one lane. From phase 5 a phase may run as a parallel lane once the phases it depends on are merged (the `Depends on` of its first task says which), each in its own worktree with its own `CARGO_TARGET_DIR` and `TMPDIR`. A phase merges to `main` when its last task is committed: every phase is a delivery.

Rules every task follows (repeated so a worker needs no other file):

- **Environment.** Before any cargo command, in the same command: `CARGO_TARGET_DIR=/var/tmp/piperine-target-<lane> TMPDIR=/var/tmp/piperine-tmp-<lane> CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`. Commands run in the foreground; never `git stash`; never `cargo clean`; never `pkill` by pattern. If `df /` passes 90 %, stop and report.
- **Gate policy (user, 2026-11-23).** Per task: the `**Build**` line, `cargo test -p <crate>` for each crate the task touched, and what its `Gate` names. The full suite runs once, in the final verification.
- **No tolerance moves.** `crates/piperine-parity/src/contract.rs` is not edited. A twin that fails is a defect of the model or the solver, to fix; or a deviation written to the ledger with the ngspice source lines that cause it, approved by the user. A twin is never made to pass by a bound, a fitted constant or a narrower sweep.
- **The model is ngspice-47's.** A model task reads the C files its `What` names and restates their currents, charges, temperature update and noise, in ngspice's order, with `std::spice::constants`. It writes no derivative: the language differentiates. Where the C and ngspice's manual differ, the C is the reference.
- **Fail loud.** A parameter, a selector value or a terminal list the model does not yet cover is refused with a diagnostic that names it and has a `GAPS.md` row; nothing is approximated or ignored silently.
- **The interface rules are design §3.** Parameter names are ngspice's; everything else is full words; the rename table is closed. A family's page (`interface/<family>.md`) is written by the phase's planner, who decides what the rules leave open and records it there; **the user is informed in the phase's report and does not review the page** (user, 2026-11-27).
- **The notice.** Every file of `std/spice/` that restates an ngspice source starts with the header of T10 and every such module's `///` ends by naming its ngspice files and `std/spice/NOTICE`; a model task adds its files to the list in `NOTICE`.
- **The code follows the specification.** `alias` and `convolve` are written into `docs/spec/` (and `docs/abi/`) before their code; where code and specification differ, the code is wrong.
- **The object model mirrors the source.** No elaboration step replaces an alias by its expansion in what the POM shows.
- **Code rules** (Rust touched by phases 0 and 9, and test support): no free function; one purpose per type; full words; a `///` on every new item; no `#[allow]`; qualified enum variants; no macro. MSL in `std/spice/`: full words for everything but params, a `///` on every module and param, one device per file.
- **The Book.** Every listing runs; a value the text states is asserted; the Book is self-contained, says nothing by reference to another language or to ngspice's manual, and has no condescending phrase. Values may move with the solver while they stay close to ngspice.
- One atomic commit per task, message as given, ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. A finished phase merges to `main` and is pushed.

---

## Test Coverage Matrix

| Code Layer | Required Test Type | Coverage Expectation | Location Pattern | Run Command |
| ---------- | ------------------ | -------------------- | ---------------- | ----------- |
| The `alias` item (front end, elaboration) | unit and integration | every form of §8.6; each rule of design §12.3; each code with its span; identity with the written-out design | `crates/piperine-language/tests/alias/**` | `cargo test -p piperine-language --test alias` |
| `alias` for the host and the tools | integration | param origins, the alias list, `set`/sweep/gradient through an alias; the language server | `crates/piperine-api/tests/alias.rs`, `crates/piperine-language-server/tests/**` | `cargo test -p piperine-api`; `cargo test -p piperine-language-server` |
| The standard library's loading | integration | on-demand reading proven by a counter; every namespace reached by the library's own test; the timing bound | `crates/piperine-language/tests/stdlib/**` | `cargo test -p piperine-language --test stdlib` |
| `convolve` (ABI, solver, language) | unit and integration | layout against `docs/abi/`; history, restore, compaction; identity with an RC filter in DC, AC and transient on both backends | `crates/piperine-abi/tests/contract/**`, `crates/piperine-analog-solver/tests/boundary/**`, `crates/piperine-parity/tests/operators/**` | `cargo test -p piperine-abi`; `cargo test -p piperine-analog-solver`; `cargo test -p piperine-parity --test operators` |
| A model of `std::spice` | parity | its matrix of design §6.2 complete; every param set by a twin; three temperatures; both backends; ngspice's own decks where they exist | `crates/piperine-parity/tests/spice/<family>/<model>/**` | `cargo test -p piperine-parity --test spice_<family>` |
| Shared functions (`junction`, `meyer`) | unit | equality with the C original on a committed grid | `crates/piperine-parity/tests/spice/<family>/functions/**` | the family's parity target |
| The gates | integration | each gate with a violating and a clean input | `crates/piperine-repository-guards/tests/spice.rs` | `cargo test -p piperine-repository-guards --test spice` |
| The specifications (`docs/spec/`, `docs/abi/`) | integration | the documentation guards; Appendix I fresh | the guards crate | `cargo test -p piperine-repository-guards --test documentation` |
| The Book | integration | every listing of the new sections run, every stated value asserted, generated tables equal to the library's texts | `tests/book/**` | `cargo test -p piperine --test book` |
| Interface pages, notes, records | none | - | - | build gate only |

## Gate Check Commands

| Gate | Command |
| --- | --- |
| Build | the `**Build**` line |
| Per task | the task's `Gate` line, and `cargo test -p <crate>` for each crate touched |
| Per phase (its last task) | every `Run parity-<family>` the phase touched, `Run gates`, `Run book`, `Run spec` |
| Feature (once, in the final verification) | `cargo test --workspace` with ngspice-47 installed |


## Phases

| Phase | Content | Tasks | Count |
| --- | --- | --- | --- |
| 0 | Ground | T1–T19 | 19 |
| 1 | Resistors, capacitors, DC and AC sources, controlled sources | T20–T29 | 10 |
| 2 | Inductors, waveforms, behavioural sources | T30–T39 | 10 |
| 3 | Switches and the lossless line | T40–T45 | 6 |
| 4 | Diodes and the uniform RC line | T46–T54 | 9 |
| 5 | JFETs and the Statz MESFET | T55–T61 | 7 |
| 6 | The bipolar transistor | T62–T66 | 5 |
| 7 | MOS levels 1, 2, 3, 6 and 9 | T67–T76 | 10 |
| 8 | The power MOSFET, MESA and HFET | T77–T83 | 7 |
| 9 | The history convolution primitive | T84–T88 | 5 |
| 10 | Lossy and coupled lines | T89–T94 | 6 |
| 11 | BSIM1 and BSIM2 | T95–T99 | 5 |
| 12 | BSIM3 | T100–T108 | 9 |
| 13 | VBIC and HICUM | T109–T117 | 9 |
| 14 | BSIM4 | T118–T127 | 10 |
| 15 | Silicon-on-insulator | T128–T136 | 9 |
| 16 | HiSIM | T137–T144 | 8 |
| 17 | Close | T145–T148 | 4 |

**148 tasks.**

---

## Phase 0: Ground

The language amendment, the library's place in `std`, the proof machinery, the spike and the licence gate. Tasks T2 to T9 (the `alias` amendment) and T11 to T18 are independent of each other after T1 except where a dependency is named; the model phases start when phase 0 is merged.

### T1: Re-take the ground

**What**: Read the merged `main` (after `code-clarity` and `device-abi-simplification`). Correct every path, crate name, test target and `Run` line of this file and of design.md; record the baseline (build clean, the parity and book targets green, the time an empty design takes to elaborate).
**Where**: `.specs/features/spice-models/{tasks,design}.md`, `.specs/features/spice-models/baseline.md`
**Depends on**: none
**Requirement**: SPM-44
**Executor**: Sonnet

**Done when**:
- [ ] every `Run` line of this file runs
- [ ] every path of design §2 and §12.7 exists or is marked "to be created"
- [ ] baseline.md has the commit, the timings and the green targets

**Tests**: none
**Gate**: Build
**Commit**: `docs(specs): spice-models re-takes paths on the merged main`

### T2: `alias` in the language specification

**What**: Write design §12 into `docs/spec/`: the new §8.6 "Aliases" (rules 1 to 15, the error table, three examples), the syntax blocks and forms table of §8, §8.1.18, §8.4.4, Appendix A (contextual keyword), Appendix B (B.1, B.2, B.3, B.3.6), §21 (param origin, alias list), Appendix G (`G.53` annotated, the closed row with the user's words), the index (concept 6, glossary). Confirm `E0380` to `E0383` are free; take the next free numbers if not and correct design §12.5.
**Where**: `docs/spec/{part_iii_structure,part_vi_io,appendix_a_lexical,appendix_b_grammar,appendix_g_scope,index}.md`
**Depends on**: T1
**Requirement**: SPM-45 to SPM-54
**Executor**: Opus

**Done when**:
- [ ] every rule of design §12.3 and §12.4 is a numbered rule of §8.6
- [ ] the grammar of B.3 derives every example of §8.6 and of interface-review.md
- [ ] each new code is in §8.6's table and in the index of codes
- [ ] the specification's own guards pass (links, rule numbers, one definition per term)

**Tests**: the documentation guards
**Gate**: Build; Run spec
**Commit**: `docs(spec): the alias item (§8.6)`

### T3: `alias`: lexer, parser, syntax tree, printer

**What**: Parse `alias_decl` at item position; `alias` is a contextual keyword (an identifier elsewhere). Add the item to the syntax tree with its span, docs, visibility, generics, target path and bindings as written. The printer writes it back. `E0383` for an alias in a module body, a block or a function.
**Where**: `crates/piperine-language/src/{lexer,grammar,ast,print}/**`, `crates/piperine-language/tests/alias/parse.rs`
**Depends on**: T2
**Requirement**: SPM-45, SPM-53
**Executor**: Sonnet

**Done when**:
- [ ] every form of §8.6 parses and prints back to the same text
- [ ] `alias` used as a param, a signal and a local name still parses
- [ ] `E0383` is raised with the item's span in each of the three places
- [ ] an alias with a missing `;`, `=` or target has a parse error that names what is expected

**Tests**: unit and `tests/alias/parse.rs`
**Gate**: Build; Run language; Run alias
**Commit**: `feat(lang): parse the alias item`

### T4: `alias` in the object model

**What**: Add the item kind `Alias` to the POM (name, visibility, docs, generics, target as written, bindings as written, span) and record on each instance the head as written (alias or module path). The POM shows what the source says: nothing replaces an alias by its expansion. Serialization and the POM documentation follow.
**Where**: `crates/piperine-language/src/pom/**`, `docs/pom/`, `crates/piperine-language/tests/alias/pom.rs`
**Depends on**: T3
**Requirement**: SPM-52
**Executor**: Sonnet

**Done when**:
- [ ] an alias of an alias of a module is three items, each walkable to the next
- [ ] an instance placed through an alias names that alias; one placed by the module name names the module
- [ ] the POM's own round-trip and navigability tests pass with aliases present

**Tests**: `tests/alias/pom.rs`
**Gate**: Build; Run language; Run alias
**Commit**: `feat(lang): the alias item in the object model`

### T5: `alias`: names and visibility

**What**: Resolve an alias in the type namespace of its file: `use`, `pub use`, `pub`, paths across crates, uniqueness. `E0380` (target not a module or an alias), `E0381` (an alias that depends on itself, through any chain), `E0382` (an alias in a type position). An alias with generics passes them to its target and its bounds are checked.
**Where**: `crates/piperine-language/src/elaboration/**` (name resolution), `crates/piperine-language/tests/alias/names.rs`
**Depends on**: T4
**Requirement**: SPM-45, SPM-50, SPM-53
**Executor**: Opus

**Done when**:
- [ ] an alias is imported, re-exported and reached by path as a module is
- [ ] a private alias is invisible from another crate
- [ ] each of the three codes is raised with the span of the offending path
- [ ] a generic alias `Fifo8<T>` places `AsyncFifo<T, 3>` and a wrong argument is the existing bound error

**Tests**: `tests/alias/names.rs`
**Gate**: Build; Run language; Run alias
**Commit**: `feat(lang): resolve alias names`

### T6: `alias`: binding and placement

**What**: Elaborate the binding chain (design §12.3 rules 5 to 9): defaults, then each alias from the innermost target outwards, then the placement's block; a later binding replaces an earlier one. A placement whose head is an alias takes a param block. `Alias { … }` and the bare name are template expressions of the module's type, complete under §8.4.2. Alias values are const expressions over constants and the alias's const generics (`E0321`); a name that is no param or repeats in one block is `E0320`; a required param still unbound is `E0319` naming param, module and alias; every `where` is checked per instance with the final values (`E0317`). A template *value* still takes no block (`E0361`).
**Where**: `crates/piperine-language/src/elaboration/**`, `crates/piperine-language/tests/alias/bind.rs`
**Depends on**: T5
**Requirement**: SPM-46 to SPM-49
**Executor**: Opus

**Done when**:
- [ ] `CMim(a, b) { l, w }` and `CMim(a, b) { l, w, cj: 3m }` elaborate to `SemiconductorCapacitor` instances with the alias's and the placement's `cj`
- [ ] an alias of an alias replaces and adds
- [ ] `Option` params: left out is `None`; `name: None` un-binds what an alias bound
- [ ] a function returning `NCH { w, l }` places through `C: Fn(P)`
- [ ] each refusal has its code, its span and the names the rule lists
- [ ] the instance is identical, in the elaborated design, to the one written out in full

**Tests**: `tests/alias/bind.rs`
**Gate**: Build; Run language; Run alias
**Commit**: `feat(lang): alias bindings and placements`

### T7: `alias` for the host

**What**: Metadata (§21): the origin of each instance param (`default`, `alias <path>`, `placement`, `host`), and the list of aliases with target, bindings and the paths of their instances. Prove that `set`, a `Dc` sweep and a gradient of a param bound by an alias behave as for any param, `where` included.
**Where**: `crates/piperine-api/src/**` (metadata), `crates/piperine-api/tests/alias.rs`
**Depends on**: T6
**Requirement**: SPM-51
**Executor**: Sonnet

**Done when**:
- [ ] `list` shows the origin of every param of an instance placed through a two-level alias
- [ ] the alias list names every instance, in generation loops too
- [ ] a `set` that breaks a `where` is refused (`E0802`); an accepted one moves the origin to `host`
- [ ] a sweep and a gradient of an alias-bound param equal those of the same design written out in full

**Tests**: `crates/piperine-api/tests/alias.rs`
**Gate**: Build; Run host
**Commit**: `feat(api): alias origins in the metadata`

### T8: `alias` in the tools and on both backends

**What**: The language server parses the item, completes param names inside its block and goes to the definition of an alias head; the syntax files under `editors/` highlight the keyword. Parity fixtures `aliases/*.msl` run the same design written with aliases and written out, on `interp` and `rust`, with equal results.
**Where**: `crates/piperine-language-server/**`, `editors/**`, `crates/piperine-parity/tests/aliases/`
**Depends on**: T6
**Requirement**: SPM-48, SPM-53
**Executor**: Sonnet

**Done when**:
- [ ] go-to-definition and completion tests pass
- [ ] the fixtures give identical records on both backends and against the written-out form
- [ ] `piperine fmt` leaves a formatted alias unchanged

**Tests**: `crates/piperine-language-server/tests/`, `crates/piperine-parity/tests/aliases/`
**Gate**: Build; Run lsp; `cargo test -p piperine-parity --test aliases`
**Commit**: `feat(tools): alias in the language server and the editors`

### T9: `alias` in the Book

**What**: "Modules and hierarchy" gains the section "Naming a configured module: `alias`": why (the same params at every placement), the item, binding what is left, replacing, an alias of an alias, an alias as a template value and in a binning function, what the host sees. Three run listings at least. The cheat sheet gains its row.
**Where**: `docs/book/content.html`, `tests/book/`
**Depends on**: T7
**Requirement**: SPM-54
**Executor**: Sonnet

**Done when**:
- [ ] every listing runs and every stated value is asserted
- [ ] the section is self-contained and says nothing by reference to another language
- [ ] the Book's outline and phrase guards pass

**Tests**: `tests/book/alias.rs`
**Gate**: Build; Run book
**Commit**: `docs(book): the alias item`

### T10: The notice of `std::spice`

**What**: The user approved the licence handling (2026-11-27): ngspice's Modified BSD notice goes at the root of the spice module of the standard library, and each model's description refers to it and to its ngspice source. Write `std/spice/NOTICE` (the notice of ngspice-47's `COPYING`, and the list of derived files, which each model task extends). Fix the header every model file starts with and the sentence every model's `///` ends with ("From ngspice-47 `<files>`; see `std/spice/NOTICE`."). A guard requires both on every file and module of `std/spice/` that restates an ngspice source.
**Where**: `crates/piperine-language/std/spice/NOTICE`, the guards crate
**Depends on**: T1
**Requirement**: SPM-25
**Executor**: Sonnet

**Done when**:
- [ ] `NOTICE` holds ngspice's notice verbatim
- [ ] the guard fails on a model file without the header and on a module whose `///` does not name its source and the notice
- [ ] design §2.1 shows the header text

**Tests**: the header guard
**Gate**: Build; Run gates
**Commit**: `docs(std): the notice of std::spice`

### T11: The standard library loads on demand

**What**: A namespace file of `std` is read, parsed and checked when a path first resolves into it (design §13). The library's own test reaches every namespace, so an error in an unreached file is still found. A guard measures the elaboration of an empty design and of the `rc` example before and after, and holds the difference under the bound baseline.md states.
**Where**: `crates/piperine-language/src/pom/stdlib.rs` and the resolver, `crates/piperine-language/tests/stdlib/`
**Depends on**: T1
**Requirement**: SPM-07
**Executor**: Opus

**Done when**:
- [ ] a design that names no item of a `std` namespace does not parse that namespace's file (a counter in the test proves it)
- [ ] every existing target passes unchanged
- [ ] the timing guard passes
- [ ] §3.6.2 and §22 say when a file of `std` is read

**Tests**: `tests/stdlib/on_demand.rs`
**Gate**: Build; Run language; Run stdlib
**Commit**: `feat(lang): std namespaces load on demand`

### T12: The `std::spice` skeleton

**What**: `pub mod spice;` in `std/lib.msl`, outside the prelude; `spice/mod.msl` with the namespaces of interface-review §1 (empty), `NGSPICE_VERSION`, `constants.msl` (ngspice's values from `const.h`), `temperature.msl` (`device_kelvin`). §22.7 in the specification; the generator of Appendix I and its freshness test. A guard: no item of `std::spice` is reachable without its path.
**Where**: `crates/piperine-language/std/{lib.msl,spice/**}`, `docs/spec/{part_vii_stdlib,appendix_i_spice}.md`, the embedded table
**Depends on**: T11, T10
**Requirement**: SPM-05, SPM-07, SPM-08, SPM-18, SPM-20
**Executor**: Sonnet

**Done when**:
- [ ] `use std::spice::constants::CHARGE;` resolves; `CHARGE` alone does not
- [ ] the constants equal ngspice-47's to the last digit (a test reads the values from this file's table)
- [ ] `device_kelvin` gives ngspice's instance temperature for the four cases of interface-review §2.7
- [ ] Appendix I is generated and fresh

**Tests**: `tests/stdlib/spice.rs`
**Gate**: Build; Run stdlib; Run spec
**Commit**: `feat(std): the std::spice skeleton`

### T13: The map is reproducible

**What**: Move `tools/extract.py` and `tools/render.py` under test: a target renders `models/*.md` from an ngspice-47 checkout named by `NGSPICE_SOURCE` and compares with the committed files; without the variable it prints that it did not run and does not report a pass. `render.py` also writes `models/parameters.tsv` (device, table, ngspice name, MSL name, type, exclusion row) for the gates.
**Where**: `.specs/features/spice-models/{tools,models}/`, `crates/piperine-repository-guards/tests/spice.rs`
**Depends on**: T1
**Requirement**: SPM-01, SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] a fresh rendering is byte-identical
- [ ] `parameters.tsv` has one row per settable parameter of the 52 in-scope devices
- [ ] the closed list of design §3.9 is a column of it, with no parameter in neither

**Tests**: the `spice` guard target
**Gate**: Build; Run gates
**Commit**: `test(guards): the spice model map is reproducible`

### T14: The parity scaffold

**What**: The layout of design §6.3 in `piperine-parity`: the `matrix.toml` reader (case, analysis, quantities, contract, or a reason), one target per family that walks its matrices on both backends, and the oracle-version gate against `std::spice::NGSPICE_VERSION`. No model yet: the scaffold is proven with a matrix over a module of the test's own.
**Where**: `crates/piperine-parity/src/spice/**`, `crates/piperine-parity/tests/spice_*.rs`
**Depends on**: T12
**Requirement**: SPM-27, SPM-28, SPM-35, SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] a matrix cell with no twin and no reason fails naming the cell
- [ ] a cell names a contract by its constant; a number in `matrix.toml` is refused
- [ ] with another ngspice version installed the targets say so and do not pass
- [ ] the existing parity targets are unchanged

**Tests**: `crates/piperine-parity/tests/spice_scaffold.rs`
**Gate**: Build; `cargo test -p piperine-parity`
**Commit**: `test(parity): the spice matrix scaffold`

### T15: The gates

**What**: In the guards crate: parameter completeness (each row of `parameters.tsv` is a param of its module or an exclusion), name fidelity (a param name is the ngspice name or in the rename table), parameter coverage (a param no twin sets to a non-default value is listed; a ceiling per model that only goes down), matrix completeness, the notice header. Each gate has a violating and a clean input of its own.
**Where**: `crates/piperine-repository-guards/tests/spice.rs`
**Depends on**: T13, T14
**Requirement**: SPM-02, SPM-09, SPM-27, SPM-33
**Executor**: Sonnet

**Done when**:
- [ ] each gate fails on its violating input with the model and the parameter named
- [ ] the gates pass on the empty library
- [ ] a model absent from `std::spice` is reported as "not yet delivered", by phase

**Tests**: the `spice` guard target
**Gate**: Build; Run gates
**Commit**: `test(guards): the spice gates`

### T16: The twin generator

**What**: Test support in `piperine-parity`: from one table (module, ports, a card of `name = value` pairs, the analysis and its probes) write the `.cir` and the `.msl` of a twin, the card as an `alias` in MSL and a `.model` in ngspice, the names mapped by the rename table.
**Where**: `crates/piperine-parity/src/spice/generate.rs`
**Depends on**: T14, T6
**Requirement**: SPM-27, SPM-33
**Executor**: Sonnet

**Done when**:
- [ ] a generated pair for a module of the test's own runs in both simulators
- [ ] a renamed param (`as_`, `mj`) is spelled each way on each side
- [ ] generated files are committed, and regenerating them changes nothing

**Tests**: unit
**Gate**: Build; `cargo test -p piperine-parity`
**Commit**: `test(parity): generate spice twins from one card`

### T17: The scale spike

**What**: Measure N18 before any compact model is planned. A synthetic module with 900 params, 60 body `let`s per selector, an `analog` block of 3,000 `let`s in BSIM's shape (chains, `if` on probes, `exp`/`ln`/`sqrt`), four internal nodes, twelve `ddt` states and fourteen noise sources: elaborate, plan and run an operating point, a DC sweep, an AC sweep and a short transient on `interp` and on `rust`. Report times and memory per stage; name what does not scale. **If a stage fails or exceeds the bound baseline.md sets, stop and report: the finding becomes a solver item with its own owner before phase 10.**
**Where**: `crates/piperine-parity/tests/scale/`, `.specs/features/spice-models/spike.md`
**Depends on**: T6
**Requirement**: SPM-26, SPM-44
**Executor**: Opus

**Done when**:
- [ ] spike.md has the numbers for each stage and backend
- [ ] the fixture is a kept test with a time bound
- [ ] each failure or excess has a `GAPS.md` row and is reported to the user

**Tests**: `crates/piperine-parity/tests/scale.rs`
**Gate**: Build; `cargo test -p piperine-parity --test scale`
**Commit**: `test(parity): the compact-model scale spike`

### T18: The capabilities the first families assume

**What**: A fixture for each assumption of design §8 that the specification allows and no test proves: N16 (a `pub` node of an instance connected to a sibling), N11 (a `laplace_*` filter whose coefficients are body `let`s over params), N14 (a function returning a template through an alias, under `Fn(P)`), a result module with a param block placed inside a value input (`VoltageSource(p, n, pulse() { … })`), an `at` whose argument is a param of a result module. A fixture that is refused gets a `GAPS.md` row with an owner and stops the task.
**Where**: `crates/piperine-parity/tests/spice/capabilities/`
**Depends on**: T6
**Requirement**: SPM-26, SPM-44
**Executor**: Sonnet

**Done when**:
- [ ] each of the five runs on both backends, or is a named `GAPS.md` row reported to the user

**Tests**: `crates/piperine-parity/tests/spice_capabilities.rs`
**Gate**: Build; `cargo test -p piperine-parity --test spice_capabilities`
**Commit**: `test(parity): capabilities std::spice relies on`

### T19: Close phase 0

**What**: `GAPS.md`: the excluded devices and code models by name ("not provided"), the refused parameters of design §3.9, the capability N3 with its owner (phase 9 of this feature). `ROADMAP.md`: the feature and its phases. `CLAUDE.md`: `alias`, `std::spice`. Merge the phase.
**Where**: `GAPS.md`, `ROADMAP.md`, `CLAUDE.md`, `models/excluded.md`
**Depends on**: T9, T8, T12, T15, T16, T17, T18, T10
**Requirement**: SPM-04, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] the six devices and 81 code models are listed by name as not provided
- [ ] every gate runs in the suite
- [ ] phase 0 is merged to `main`

**Tests**: none
**Gate**: Build; Run gates; Run book; Run spec
**Commit**: `docs: spice-models phase 0 closes`

---

## Phase 1: Resistors, capacitors, DC and AC sources, controlled sources

The first delivery: eight modules, the twins every later phase builds on, and the first sections of the Book's part.

### T20: Interface page: `Resistor`, `SemiconductorResistor`, `Capacitor`, `SemiconductorCapacitor`, `VoltageSource`, `CurrentSource`, the four controlled sources

**What**: Write `interface/linear.md` for `Resistor`, `SemiconductorResistor`, `Capacitor`, `SemiconductorCapacitor`, `VoltageSource`, `CurrentSource`, the four controlled sources: per module the ports and one row per param (MSL name, ngspice name, type, default, unit, meaning, given-ness rule, `where`), from `models/*.md`, the ngspice-47 setup, check and parameter files and the model's reference manual; the usage rules; the list of ngspice parameters with no param and the row of design §3.9 each falls under. Apply design §3 as written. The planner of the phase decides what the rules leave open and records it in the page; the user does not review the page. The tables of interface-review §3.1, §3.2, §4.1 and §4.3 are the start; complete the units of the capacitor and of the sources.
**Where**: `.specs/features/spice-models/interface/linear.md`
**Depends on**: T19
**Requirement**: SPM-06, SPM-11, SPM-12, SPM-42
**Executor**: Sonnet

**Done when**:
- [ ] every settable ngspice parameter of these devices is a row or a named exclusion
- [ ] every `unresolved` and `given-ness read` row of the map has a decided declaration
- [ ] every row has a unit, or `—` for a pure number
- [ ] a point design §3 does not decide is decided here by the phase's planner, written under "Decisions taken" with its reason, and listed in the phase's report to the user (the user is informed, not asked)

**Tests**: none (document)
**Gate**: Build
**Commit**: `docs(specs): spice-models interface page, linear`

### T21: `Resistor` and `SemiconductorResistor`

**What**: Both modules per `interface/linear.md`: conductance with `m` and `scale`; `tc1`/`tc2` or `tce`; `ac`; the 1 pΩ floor; thermal noise and flicker noise over the effective noise area; `noisy`; `bv_max` as a warning record placed only when given. Written from `res/{ressetup,restemp,resload,resnoise,resparam,ressoachk}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/passive.msl`
**Depends on**: T20
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] `r` required in one, `rsh` and `l` in the other; `r` given in the geometric one wins
- [ ] the temperature factor is ngspice's for each of the three coefficient cases
- [ ] noise sources are named `thermal` and `1overf`
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/passive/`
**Gate**: Build; Run parity-passive; Run stdlib
**Commit**: `feat(std): spice Resistor and SemiconductorResistor`

### T22: Parity matrix: the resistors

**What**: Fill the parity matrix of the resistors (design §6.2): value; geometry with `narrow` and `short`; `tc1`/`tc2` and `tce` at three temperatures; `tnom`; `m` and `scale`; `ac` in an AC sweep; thermal and flicker noise with and without geometry; `noisy` off. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. Reuse the netlists `divider`, `temp_res`, `tnom_res`, `noise_rc` of `tests/e2e/twins/`. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/passive/`, `crates/piperine-parity/tests/spice_passive.rs`
**Depends on**: T21
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-passive; Run gates
**Commit**: `test(parity): spice the resistors matrix`

### T23: `Capacitor` and `SemiconductorCapacitor`

**What**: Both modules: `ddt(c_effective * v)`; the geometric value from `cj`, `cjsw`, `l`, `w`, `narrow`, `short`, `del`, or from `di` and `thick`; temperature coefficients; `m`, `scale`; `ic` by §16 H7; `bv_max`. Written from `cap/{capsetup,captemp,capload,capparam,capsoachk}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/passive.msl`
**Depends on**: T20
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] the geometric capacitance equals `captemp.c`'s for each given-ness case
- [ ] `ic` holds the branch only in a transient's operating point
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/passive/`
**Gate**: Build; Run parity-passive; Run stdlib
**Commit**: `feat(std): spice Capacitor and SemiconductorCapacitor`

### T24: Parity matrix: the capacitors

**What**: Fill the parity matrix of the capacitors (design §6.2): value; geometry; `di`/`thick`; temperature; `m`, `scale`; `ic` (the `uic_rc` netlist); AC; a transient charge and discharge. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/passive/`, `crates/piperine-parity/tests/spice_passive.rs`
**Depends on**: T23
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-passive; Run gates
**Commit**: `test(parity): spice the capacitors matrix`

### T25: `VoltageSource` and `CurrentSource`

**What**: `dc`, `acmag`, `acphase` (degrees), source stepping through `source_scale()`, the optional waveform value input (used in a transient; its value at time 0 is the DC value when `dc` is left out), `m` on the current source. Written from `vsrc/{vsrcload,vsrcacld,vsrcpar,vsrctemp}.c`, `isrc/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/source.msl`
**Depends on**: T20
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] the DC value follows ngspice's rule for the four cases (dc, waveform, both, neither)
- [ ] `acphase` is converted to radians once
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/source/`
**Gate**: Build; Run parity-source; Run stdlib
**Commit**: `feat(std): spice VoltageSource and CurrentSource`

### T26: The four linear controlled sources

**What**: `VoltageControlledVoltageSource`, `VoltageControlledCurrentSource`, `CurrentControlledCurrentSource`, `CurrentControlledVoltageSource`: `gain`, `m` on the current outputs; the current-controlled pair owns its sensing branch `(sp, sn)`. Written from `vcvs/*`, `vccs/*`, `cccs/*`, `ccvs/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/controlled.msl`
**Depends on**: T25
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] the sensing branch is a short whose flow is read
- [ ] each equals its ngspice element in a twin where ngspice has the named 0 V source
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/source/`
**Gate**: Build; Run parity-source; Run stdlib
**Commit**: `feat(std): spice The four linear controlled sources`

### T27: Parity matrix: the sources and controlled sources

**What**: Fill the parity matrix of the sources and controlled sources (design §6.2): DC, AC magnitude and phase, both together; source stepping in a circuit that needs it (`homotopy_bias`, `homotopy_sink`); each controlled source in DC, AC and transfer function; gain signs; `m`. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/source/`, `crates/piperine-parity/tests/spice_source.rs`
**Depends on**: T26
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-source; Run gates
**Commit**: `test(parity): spice the sources and controlled sources matrix`

### T28: The existing twins use `std::spice`

**What**: Replace `Resistor`, `Capacitor`, `Vsource` and `Isource` of `tests/e2e/twins/devices.msl` by the library's modules in every twin that uses them. No golden changes.
**Where**: `tests/e2e/twins/*.msl`
**Depends on**: T22, T24, T27
**Requirement**: SPM-34
**Executor**: Sonnet

**Done when**:
- [ ] every twin that placed those four now names `std::spice`
- [ ] every golden and every expected output is byte-identical
- [ ] `devices.msl` keeps only what the library does not yet have

**Tests**: the existing twin targets
**Gate**: Build; `cargo test -p piperine-parity`
**Commit**: `test(twins): the linear devices come from std::spice`

### T29: Book and close: phase 1

**What**: Write the Book sections of this phase: the part's opening chapter "Using the SPICE library" (first version: `use`, names, units, an `alias` as a card, temperature, multiplicity, what differs from ngspice, what is not provided, the start of the translation table); "Passive elements": resistor, capacitor; "Sources and switches": DC and AC sources, the four controlled sources, current sensing. Build the generator of the reference tables. Each device section has the seven parts of design §7 (what it is, choosing it, placing it, a model card as an `alias`, the parameters explained by group, use cases, reading it back and limits); every listing is a run listing; the reference tables are generated from the `///` texts. Regenerate Appendix I. Update `GAPS.md` and `ROADMAP.md` for the phase. Merge the phase.
**Where**: `docs/book/content.html`, `tests/book/`, `docs/spec/appendix_i_spice.md`, `GAPS.md`, `ROADMAP.md`
**Depends on**: T28
**Requirement**: SPM-37 to SPM-41, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] every listing of the new sections is run by the `book` target and every stated value asserted
- [ ] the generated tables equal the library's `///` texts (the test fails on a difference)
- [ ] no phrase the Book's guards refuse; nothing says "as in ngspice" without saying what that is
- [ ] Appendix I is fresh
- [ ] the gates of design §6.4 are at zero for this phase's models
- [ ] the phase is merged to `main`

**Tests**: `tests/book/spice_*.rs`
**Gate**: Build; Run book; Run parity-passive; Run gates; Run spec
**Commit**: `docs(book): spice phase 1`

---

## Phase 2: Inductors, waveforms, behavioural sources

### T30: Interface page: `Inductor`, `GeometricInductor`, `MutualInductor`, the eight waveforms, `BehavioralVoltageSource`, `BehavioralCurrentSource`

**What**: Write `interface/reactive.md` for `Inductor`, `GeometricInductor`, `MutualInductor`, the eight waveforms, `BehavioralVoltageSource`, `BehavioralCurrentSource`: per module the ports and one row per param (MSL name, ngspice name, type, default, unit, meaning, given-ness rule, `where`), from `models/*.md`, the ngspice-47 setup, check and parameter files and the model's reference manual; the usage rules; the list of ngspice parameters with no param and the row of design §3.9 each falls under. Apply design §3 as written. The planner of the phase decides what the rules leave open and records it in the page; the user does not review the page. For each waveform, the parameters ngspice defaults from the `.tran` card and what C3 makes of each.
**Where**: `.specs/features/spice-models/interface/reactive.md`
**Depends on**: T29
**Requirement**: SPM-06, SPM-11, SPM-12, SPM-42
**Executor**: Sonnet

**Done when**:
- [ ] every settable ngspice parameter of these devices is a row or a named exclusion
- [ ] every `unresolved` and `given-ness read` row of the map has a decided declaration
- [ ] every row has a unit, or `—` for a pure number
- [ ] a point design §3 does not decide is decided here by the phase's planner, written under "Decisions taken" with its reason, and listed in the phase's report to the user (the user is informed, not asked)

**Tests**: none (document)
**Gate**: Build
**Commit**: `docs(specs): spice-models interface page, reactive`

### T31: `Inductor`, `GeometricInductor`, `MutualInductor`

**What**: The inductor as a potential branch `ddt(l * i + sqrt(l) * coupled)`, with the public node `link` of a nature `FluxLink` the library declares (design §5.2); the geometric inductance of ngspice's model; `MutualInductor(a, b) { k }` contributing `k · potential(other)` into each link; temperature coefficients, `m`, `scale`, `ic`. Written from `ind/{indsetup,indtemp,indload,indparam,mutsetup,muttemp}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/passive.msl`
**Depends on**: T30
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] two coupled inductors give `M = k·sqrt(l1·l2)` exactly
- [ ] an uncoupled inductor's link carries no flow and changes no result
- [ ] three inductors coupled pairwise solve
- [ ] `k` outside [−1, 1] is refused by `where`
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/passive/`
**Gate**: Build; Run parity-passive; Run stdlib
**Commit**: `feat(std): spice Inductor, GeometricInductor, MutualInductor`

### T32: Parity matrix: the inductors

**What**: Fill the parity matrix of the inductors (design §6.2): value; geometry; temperature; `ic`; AC of an RL and an RLC; a transformer at `k` 0.1, 0.9 and 1 in AC and transient; three windings. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/passive/`, `crates/piperine-parity/tests/spice_passive.rs`
**Depends on**: T31
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-passive; Run gates
**Commit**: `test(parity): spice the inductors matrix`

### T33: Waveforms `pulse`, `sine`, `exponential`

**What**: Result modules with params per `interface/reactive.md`. `pulse` keeps its segment in a signal and schedules each corner with `at`; `np` ends the train. Written from `vsrc/vsrcload.c`, `vsrc/vsrcacct.c` (breakpoints); the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/source.msl`
**Depends on**: T30
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] each corner is a time point
- [ ] the value at every instant equals `vsrcload.c`'s formula
- [ ] `tr`, `tf` not positive are refused by `where`
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/source/`
**Gate**: Build; Run parity-source; Run stdlib
**Commit**: `feat(std): spice Waveforms pulse, sine, exponential`

### T34: Waveforms `piecewise_linear`, `frequency_modulated`, `amplitude_modulated`

**What**: `piecewise_linear<const N>` with `r` (repeat) and `td`; the two modulated waveforms with ngspice's limits (`mdi` limited to `fc / fm`). Written from `vsrc/vsrcload.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/source.msl`
**Depends on**: T33
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] a PWL corner is a time point; times not increasing are refused
- [ ] `r` repeats from the given time as ngspice does
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/source/`
**Gate**: Build; Run parity-source; Run stdlib
**Commit**: `feat(std): spice Waveforms piecewise_linear, frequency_modulated, amplitude_modulated`

### T35: Waveforms `transient_noise`, `transient_random`

**What**: White, 1/f and random-telegraph noise in time, and the four random distributions, each with a `seed` param and its state in a signal. Written from `vsrc/vsrcload.c`, `maths/misc/randnumb.c`, `frontend/trannoise/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/source.msl`
**Depends on**: T34
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] the same seed gives the same run on both backends
- [ ] mean, variance and spectrum slope match ngspice's within the `NOISE` contract over a long run
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/source/`
**Gate**: Build; Run parity-source; Run stdlib
**Commit**: `feat(std): spice Waveforms transient_noise, transient_random`

### T36: Parity matrix: the waveforms

**What**: Fill the parity matrix of the waveforms (design §6.2): each waveform with defaults and with every parameter set; corners between print points; a waveform on a current source; two waveforms added; the statistical comparison of the two random ones. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. Reuse `tran_pwl_rc`, `tran_square_rc`, `tran_corner`. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/source/`, `crates/piperine-parity/tests/spice_source.rs`
**Depends on**: T35
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-source; Run gates
**Commit**: `test(parity): spice the waveforms matrix`

### T37: `BehavioralVoltageSource` and `BehavioralCurrentSource`

**What**: The expression is the value input; `tc1`, `tc2`, `reciproctc`, `temp`, `dtemp`, `m`, `reciprocm` as ngspice applies them. Written from `asrc/{asrcload,asrcacld,asrctemp,asrcpar}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/source.msl`
**Depends on**: T30
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] the temperature factor multiplies or divides as `reciproctc` says
- [ ] an expression over a voltage and over a sensed current both linearize in AC
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/source/`
**Gate**: Build; Run parity-source; Run stdlib
**Commit**: `feat(std): spice BehavioralVoltageSource and BehavioralCurrentSource`

### T38: Parity matrix: the behavioural sources

**What**: Fill the parity matrix of the behavioural sources (design §6.2): a product of two voltages; a nonlinear function; a current-dependent expression; temperature coefficients; AC around a bias; a polynomial source (the `POLY` translation). Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/source/`, `crates/piperine-parity/tests/spice_source.rs`
**Depends on**: T37
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-source; Run gates
**Commit**: `test(parity): spice the behavioural sources matrix`

### T39: Book and close: phase 2

**What**: Write the Book sections of this phase: "Passive elements": inductor, coupled inductors and transformers; "Sources and switches": the eight waveforms (each drawn with its parameters), a waveform of your own, behavioural sources; the translation table's B-source function names. Each device section has the seven parts of design §7 (what it is, choosing it, placing it, a model card as an `alias`, the parameters explained by group, use cases, reading it back and limits); every listing is a run listing; the reference tables are generated from the `///` texts. Regenerate Appendix I. Update `GAPS.md` and `ROADMAP.md` for the phase. Merge the phase.
**Where**: `docs/book/content.html`, `tests/book/`, `docs/spec/appendix_i_spice.md`, `GAPS.md`, `ROADMAP.md`
**Depends on**: T32, T36, T38
**Requirement**: SPM-37 to SPM-41, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] every listing of the new sections is run by the `book` target and every stated value asserted
- [ ] the generated tables equal the library's `///` texts (the test fails on a difference)
- [ ] no phrase the Book's guards refuse; nothing says "as in ngspice" without saying what that is
- [ ] Appendix I is fresh
- [ ] the gates of design §6.4 are at zero for this phase's models
- [ ] the phase is merged to `main`

**Tests**: `tests/book/spice_*.rs`
**Gate**: Build; Run book; Run parity-source; Run gates; Run spec
**Commit**: `docs(book): spice phase 2`

---

## Phase 3: Switches and the lossless line

### T40: Interface page: `VoltageControlledSwitch`, `CurrentControlledSwitch`, `LosslessTransmissionLine`

**What**: Write `interface/switching.md` for `VoltageControlledSwitch`, `CurrentControlledSwitch`, `LosslessTransmissionLine`: per module the ports and one row per param (MSL name, ngspice name, type, default, unit, meaning, given-ness rule, `where`), from `models/*.md`, the ngspice-47 setup, check and parameter files and the model's reference manual; the usage rules; the list of ngspice parameters with no param and the row of design §3.9 each falls under. Apply design §3 as written. The planner of the phase decides what the rules leave open and records it in the page; the user does not review the page. 
**Where**: `.specs/features/spice-models/interface/switching.md`
**Depends on**: T39
**Requirement**: SPM-06, SPM-11, SPM-12, SPM-42
**Executor**: Sonnet

**Done when**:
- [ ] every settable ngspice parameter of these devices is a row or a named exclusion
- [ ] every `unresolved` and `given-ness read` row of the map has a decided declaration
- [ ] every row has a unit, or `—` for a pure number
- [ ] a point design §3 does not decide is decided here by the phase's planner, written under "Decisions taken" with its reason, and listed in the phase's report to the user (the user is informed, not asked)

**Tests**: none (document)
**Gate**: Build
**Commit**: `docs(specs): spice-models interface page, switching`

### T41: The two switches

**What**: A `Bit` state with hysteresis changed at located crossings of the control; `ron`, `roff`; `initial`; the thermal noise of the present resistance; the current switch owns its sensing branch. Written from `sw/{swsetup,swload,swnoise,swtrunc}.c`, `csw/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/switch.msl`
**Depends on**: T40
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] hysteresis closes above `vt + vh` and opens below `vt − vh`
- [ ] with `initial` left out the operating point settles the state from the control
- [ ] noise follows the state
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/switch/`
**Gate**: Build; Run parity-switch; Run stdlib
**Commit**: `feat(std): spice The two switches`

### T42: Parity matrix: the switches

**What**: Fill the parity matrix of the switches (design §6.2): on and off in DC; a sweep up and down through the hysteresis; `initial` each way; transient switching of a load; noise in each state (the `noise_switch`, `noise_switch_off`, `noise_csw` netlists). Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/switch/`, `crates/piperine-parity/tests/spice_switch.rs`
**Depends on**: T41
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-switch; Run gates
**Commit**: `test(parity): spice the switches matrix`

### T43: `LosslessTransmissionLine`

**What**: The method of characteristics with `delay`; `td` or `f` with `nl` under one `where`; the four initial conditions. Written from `tra/{trasetup,traload,traacld,tratemp,traaccept,tratrunc}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/line.msl`
**Depends on**: T40
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] `td` from `f` and `nl` equals ngspice's
- [ ] a matched line shows no reflection; an open one doubles the step
- [ ] AC is `exp(−jω·td)`
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/line/`
**Gate**: Build; Run parity-line; Run stdlib
**Commit**: `feat(std): spice LosslessTransmissionLine`

### T44: Parity matrix: the lossless line

**What**: Fill the parity matrix of the lossless line (design §6.2): matched, open and shorted ends with a step and with a sine; `td` and `f`/`nl`; AC magnitude and phase; initial conditions (the `delay_tline` netlist). Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/line/`, `crates/piperine-parity/tests/spice_line.rs`
**Depends on**: T43
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-line; Run gates
**Commit**: `test(parity): spice the lossless line matrix`

### T45: Book and close: phase 3

**What**: Write the Book sections of this phase: "Sources and switches": switches and hysteresis, a relaxation oscillator; "Transmission lines": the lossless line, reflections and termination. Each device section has the seven parts of design §7 (what it is, choosing it, placing it, a model card as an `alias`, the parameters explained by group, use cases, reading it back and limits); every listing is a run listing; the reference tables are generated from the `///` texts. Regenerate Appendix I. Update `GAPS.md` and `ROADMAP.md` for the phase. Merge the phase.
**Where**: `docs/book/content.html`, `tests/book/`, `docs/spec/appendix_i_spice.md`, `GAPS.md`, `ROADMAP.md`
**Depends on**: T42, T44
**Requirement**: SPM-37 to SPM-41, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] every listing of the new sections is run by the `book` target and every stated value asserted
- [ ] the generated tables equal the library's `///` texts (the test fails on a difference)
- [ ] no phrase the Book's guards refuse; nothing says "as in ngspice" without saying what that is
- [ ] Appendix I is fresh
- [ ] the gates of design §6.4 are at zero for this phase's models
- [ ] the phase is merged to `main`

**Tests**: `tests/book/spice_*.rs`
**Gate**: Build; Run book; Run parity-switch; Run gates; Run spec
**Commit**: `docs(book): spice phase 3`

---

## Phase 4: Diodes and the uniform RC line

The first nonlinear device, and the functions every junction of every later model uses.

### T46: Interface page: `Diode`, `GeometricDiode`, the two self-heating modules, `UniformRcLine`

**What**: Write `interface/diode.md` for `Diode`, `GeometricDiode`, the two self-heating modules, `UniformRcLine`: per module the ports and one row per param (MSL name, ngspice name, type, default, unit, meaning, given-ness rule, `where`), from `models/*.md`, the ngspice-47 setup, check and parameter files and the model's reference manual; the usage rules; the list of ngspice parameters with no param and the row of design §3.9 each falls under. Apply design §3 as written. The planner of the phase decides what the rules leave open and records it in the page; the user does not review the page. Every one of the 68 model params with its unit; which `tlev`/`tlevc` value selects which temperature law; which checks of `diosetup.c` and `diotemp.c` are fatal.
**Where**: `.specs/features/spice-models/interface/diode.md`
**Depends on**: T45
**Requirement**: SPM-06, SPM-11, SPM-12, SPM-42
**Executor**: Sonnet

**Done when**:
- [ ] every settable ngspice parameter of these devices is a row or a named exclusion
- [ ] every `unresolved` and `given-ness read` row of the map has a decided declaration
- [ ] every row has a unit, or `—` for a pure number
- [ ] a point design §3 does not decide is decided here by the phase's planner, written under "Decisions taken" with its reason, and listed in the phase's report to the user (the user is informed, not asked)

**Tests**: none (document)
**Gate**: Build
**Commit**: `docs(specs): spice-models interface page, diode`

### T47: `std::spice::junction`

**What**: Functions: depletion charge and capacitance with the `fc` linearization; the temperature laws of saturation current, built-in potential, grading and capacitance for each `tlev`/`tlevc`; the critical voltage; `limitlog`. Pure `fn`s, each documented with the ngspice lines it restates. Written from `dio/diotemp.c`, `devsup.c` (`DEVpnjlim`, `DEVlimitlog`), `bjt/bjttemp.c` for the shared forms; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/junction.msl`
**Depends on**: T46
**Requirement**: SPM-19, SPM-20, SPM-22, SPM-25
**Executor**: Opus

**Done when**:
- [ ] each function equals its C original on a grid of inputs to 1e-12 relative (a test holds the grid, computed once from ngspice's code and committed with its provenance)
- [ ] `limitlog` satisfies `f(x, x) == x`
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/diode/`
**Gate**: Build; Run parity-diode; Run stdlib
**Commit**: `feat(std): spice std::spice::junction`

### T48: `Diode`: currents and temperature

**What**: Forward current with knee (`ikf`), sidewall, recombination (`isr`, `nr`), reverse with knee (`ikr`), breakdown with ngspice's matching of `bv` to `ibv`, tunnelling, the junction shunt `gmin()`, series resistance with its internal node when `rs > 0`, `area`, `pj`, `m`; limiting by `pnjlim` with ngspice's critical voltages; `off`; the full temperature update. Written from `dio/{diosetup,diotemp,dioload}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/diode.msl`
**Depends on**: T47
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] each region's current equals ngspice's in a single-point twin
- [ ] the breakdown voltage after matching equals `diotemp.c`'s
- [ ] with `rs` zero there is no internal node
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/diode/`
**Gate**: Build; Run parity-diode; Run stdlib
**Commit**: `feat(std): spice Diode: currents and temperature`

### T49: `Diode`: charge, noise, reported quantities

**What**: Depletion charge (bottom and sidewall), diffusion charge `tt`, soft recovery `vp` with its node; shot, thermal and flicker noise for the bottom and the sidewall junction; the operating-point variables of `dioask.c`; `ic`; the safe-operating-area records. Written from `dio/{dioload,dionoise,dioask,diosoachk}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/diode.msl`
**Depends on**: T48
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] noise sources carry ngspice's names
- [ ] `cd`, `gd`, `charge` equal ngspice's reported values
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/diode/`
**Gate**: Build; Run parity-diode; Run stdlib
**Commit**: `feat(std): spice Diode: charge, noise, reported quantities`

### T50: `GeometricDiode` and the self-heating modules

**What**: Level 3: scaling by `l`, `w` with `xw`, the metal and polysilicon capacitances. `DiodeSelfHeating` and `GeometricDiodeSelfHeating`: the junction temperature from the `Thermal` port, `rth0`, `cth0`, the dissipated power as its flow. The four modules share their equations through functions. Written from `dio/{diosetup,diotemp,dioload}.c` (level 3, the thermal branches); the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/diode.msl`
**Depends on**: T49
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] the level-3 areas and capacitances equal `diosetup.c`'s
- [ ] with the thermal port held at 0 K rise the self-heating module equals the plain one
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/diode/`
**Gate**: Build; Run parity-diode; Run stdlib
**Commit**: `feat(std): spice GeometricDiode and the self-heating modules`

### T51: Parity matrix: the diodes

**What**: Fill the parity matrix of the diodes (design §6.2): forward low injection, knee, resistance-limited; reverse leakage, breakdown, tunnelling, recombination; `area` and `pj`; sidewall; each `tlev`/`tlevc`; three temperatures; AC at four biases; reverse recovery with `tt` and `vp`; each noise source at two frequencies; self-heating in DC and transient; level 3 geometry. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. Reuse the ten diode netlists of `tests/e2e/twins/` and move those twins to the library's `Diode`. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/diode/`, `crates/piperine-parity/tests/spice_diode.rs`
**Depends on**: T50
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-diode; Run gates
**Commit**: `test(parity): spice the diodes matrix`

### T52: `UniformRcLine`

**What**: Generation of the lump ladder with ngspice's geometric progression and lump count; capacitors, or reverse diodes when `isperl` is given. Written from `urc/urcsetup.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/line.msl`
**Depends on**: T49
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] the lump values equal `urcsetup.c`'s for `n` given and for `n` from `fmax`
- [ ] `l` and `n` are structural
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/line/`
**Gate**: Build; Run parity-line; Run stdlib
**Commit**: `feat(std): spice UniformRcLine`

### T53: Parity matrix: the uniform RC line

**What**: Fill the parity matrix of the uniform RC line (design §6.2): the six `urc_*` netlists of `tests/e2e/twins/`: lump counts 2, 5, 10, the count from `fmax`, a taper, the diode variant; AC and transient. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/line/`, `crates/piperine-parity/tests/spice_line.rs`
**Depends on**: T52
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-line; Run gates
**Commit**: `test(parity): spice the uniform RC line matrix`

### T54: Book and close: phase 4

**What**: Write the Book sections of this phase: "Diodes" (the whole chapter: junction, DC parameters, breakdown, charge and recovery, temperature, noise, the geometric diode, self-heating; a rectifier, a Zener regulator, reverse recovery, a varactor); "Passive elements": the uniform RC line. Each device section has the seven parts of design §7 (what it is, choosing it, placing it, a model card as an `alias`, the parameters explained by group, use cases, reading it back and limits); every listing is a run listing; the reference tables are generated from the `///` texts. Regenerate Appendix I. Update `GAPS.md` and `ROADMAP.md` for the phase. Merge the phase.
**Where**: `docs/book/content.html`, `tests/book/`, `docs/spec/appendix_i_spice.md`, `GAPS.md`, `ROADMAP.md`
**Depends on**: T51, T53
**Requirement**: SPM-37 to SPM-41, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] every listing of the new sections is run by the `book` target and every stated value asserted
- [ ] the generated tables equal the library's `///` texts (the test fails on a difference)
- [ ] no phrase the Book's guards refuse; nothing says "as in ngspice" without saying what that is
- [ ] Appendix I is fresh
- [ ] the gates of design §6.4 are at zero for this phase's models
- [ ] the phase is merged to `main`

**Tests**: `tests/book/spice_*.rs`
**Gate**: Build; Run book; Run parity-diode; Run gates; Run spec
**Commit**: `docs(book): spice phase 4`

---

## Phase 5: JFETs and the Statz MESFET

### T55: Interface page: `Jfet`, `Jfet2`, `Mesfet`

**What**: Write `interface/jfet.md` for `Jfet`, `Jfet2`, `Mesfet`: per module the ports and one row per param (MSL name, ngspice name, type, default, unit, meaning, given-ness rule, `where`), from `models/*.md`, the ngspice-47 setup, check and parameter files and the model's reference manual; the usage rules; the list of ngspice parameters with no param and the row of design §3.9 each falls under. Apply design §3 as written. The planner of the phase decides what the rules leave open and records it in the page; the user does not review the page. 
**Where**: `.specs/features/spice-models/interface/jfet.md`
**Depends on**: T54
**Requirement**: SPM-06, SPM-11, SPM-12, SPM-42
**Executor**: Sonnet

**Done when**:
- [ ] every settable ngspice parameter of these devices is a row or a named exclusion
- [ ] every `unresolved` and `given-ness read` row of the map has a decided declaration
- [ ] every row has a unit, or `—` for a pure number
- [ ] a point design §3 does not decide is decided here by the phase's planner, written under "Decisions taken" with its reason, and listed in the phase's report to the user (the user is informed, not asked)

**Tests**: none (document)
**Gate**: Build
**Commit**: `docs(specs): spice-models interface page, jfet`

### T56: `Jfet`

**What**: Shichman-Hodges with the Sydney University terms (`b`, `vtotc`, `bex`, `betatce`); gate junctions with the shared functions; `rd`, `rs` nodes; limiting by `fetlim` and `pnjlim`; noise per `nlev` with `gdsnoi`; reported quantities; both polarities. Written from `jfet/{jfetset,jfettemp,jfetload,jfetnoi,jfetask}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/jfet.msl`
**Depends on**: T55
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] the drain current equals ngspice's in each region and in reverse
- [ ] noise equals ngspice's for `nlev` 0 to 3
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/jfet/`
**Gate**: Build; Run parity-jfet; Run stdlib
**Commit**: `feat(std): spice Jfet`

### T57: `Jfet2`

**What**: The Parker-Skellern model: `psmodel.c`'s current and charge, its 39 params. Written from `jfet2/{jfet2set,jfet2temp,jfet2load,jfet2noi,psmodel}.c`, `jfet2parm.h`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/jfet.msl`
**Depends on**: T55
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] current and charges equal `psmodel.c`'s on the grid of the ngspice `tests/jfet` decks
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/jfet/`
**Gate**: Build; Run parity-jfet; Run stdlib
**Commit**: `feat(std): spice Jfet2`

### T58: `Mesfet`

**What**: The Statz model: current, the gate charge model, noise. Written from `mes/{messetup,mestemp,mesload,mesnoise,mesask}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/mesfet/mesfet.msl`
**Depends on**: T55
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] current and capacitances equal ngspice's across the knee
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/mesfet/`
**Gate**: Build; Run parity-mesfet; Run stdlib
**Commit**: `feat(std): spice Mesfet`

### T59: Parity matrix: the JFETs

**What**: Fill the parity matrix of the JFETs (design §6.2): transfer and output curves; pinch-off, ohmic, saturation, reverse; gate forward conduction; both polarities; AC capacitances; noise per `nlev`; three temperatures; ngspice's `tests/jfet` decks. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. Reuse `jfet_bias`, `jfet_id_vds`, `noise_jfet`, `noise_jfet_nlev3`. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/jfet/`, `crates/piperine-parity/tests/spice_jfet.rs`
**Depends on**: T56, T57
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-jfet; Run gates
**Commit**: `test(parity): spice the JFETs matrix`

### T60: Parity matrix: the MESFET

**What**: Fill the parity matrix of the MESFET (design §6.2): transfer and output curves; both polarities; AC; noise; ngspice's `tests/mes` decks. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/mesfet/`, `crates/piperine-parity/tests/spice_mesfet.rs`
**Depends on**: T58
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-mesfet; Run gates
**Commit**: `test(parity): spice the MESFET matrix`

### T61: Book and close: phase 5

**What**: Write the Book sections of this phase: "JFETs, MESFETs and HFETs": the Shichman-Hodges JFET, Parker-Skellern, the Statz MESFET; a JFET source follower, a MESFET amplifier. Each device section has the seven parts of design §7 (what it is, choosing it, placing it, a model card as an `alias`, the parameters explained by group, use cases, reading it back and limits); every listing is a run listing; the reference tables are generated from the `///` texts. Regenerate Appendix I. Update `GAPS.md` and `ROADMAP.md` for the phase. Merge the phase.
**Where**: `docs/book/content.html`, `tests/book/`, `docs/spec/appendix_i_spice.md`, `GAPS.md`, `ROADMAP.md`
**Depends on**: T59, T60
**Requirement**: SPM-37 to SPM-41, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] every listing of the new sections is run by the `book` target and every stated value asserted
- [ ] the generated tables equal the library's `///` texts (the test fails on a difference)
- [ ] no phrase the Book's guards refuse; nothing says "as in ngspice" without saying what that is
- [ ] Appendix I is fresh
- [ ] the gates of design §6.4 are at zero for this phase's models
- [ ] the phase is merged to `main`

**Tests**: `tests/book/spice_*.rs`
**Gate**: Build; Run book; Run parity-jfet; Run gates; Run spec
**Commit**: `docs(book): spice phase 5`

---

## Phase 6: The bipolar transistor

### T62: Interface page: `Bjt`

**What**: Write `interface/bjt.md` for `Bjt`: per module the ports and one row per param (MSL name, ngspice name, type, default, unit, meaning, given-ness rule, `where`), from `models/*.md`, the ngspice-47 setup, check and parameter files and the model's reference manual; the usage rules; the list of ngspice parameters with no param and the row of design §3.9 each falls under. Apply design §3 as written. The planner of the phase decides what the rules leave open and records it in the page; the user does not review the page. All 123 model params; the `subs` default by polarity; `tlev`/`tlevc`; the per-parameter temperature coefficients; the quasi-saturation set.
**Where**: `.specs/features/spice-models/interface/bjt.md`
**Depends on**: T54
**Requirement**: SPM-06, SPM-11, SPM-12, SPM-42
**Executor**: Sonnet

**Done when**:
- [ ] every settable ngspice parameter of these devices is a row or a named exclusion
- [ ] every `unresolved` and `given-ness read` row of the map has a decided declaration
- [ ] every row has a unit, or `—` for a pure number
- [ ] a point design §3 does not decide is decided here by the phase's planner, written under "Decisions taken" with its reason, and listed in the phase's report to the user (the user is informed, not asked)

**Tests**: none (document)
**Gate**: Build
**Commit**: `docs(specs): spice-models interface page, bjt`

### T63: `Bjt`: currents and temperature

**What**: Gummel-Poon transport current with Early effect and high injection (`nkf`), leakage, base resistance with `irb` modulation, `re`, `rc` nodes, the substrate junction for vertical and lateral devices, quasi-saturation (`rco`, `vo`, `gamma`), `area`, `areab`, `areac`, `m`; limiting; `off`; the temperature update. Written from `bjt/{bjtsetup,bjttemp,bjtload}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bipolar/bjt.msl`
**Depends on**: T62
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] the Gummel plot equals ngspice's over ten decades, both polarities
- [ ] the substrate current has ngspice's sign for lateral and vertical
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bipolar/`
**Gate**: Build; Run parity-bipolar; Run stdlib
**Commit**: `feat(std): spice Bjt: currents and temperature`

### T64: `Bjt`: charge, noise, reported quantities

**What**: Junction and diffusion charges, `xcjc` split, the bias dependence of `tf`, excess phase `ptf`, the quasi-saturation charge `qco`; the six noise sources; the reported quantities; `icvbe`, `icvce`; safe-operating-area records. Written from `bjt/{bjtload,bjtnoise,bjtask,bjtsoachk}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bipolar/bjt.msl`
**Depends on**: T63
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] `ft` against `ic` equals ngspice's
- [ ] each noise source equals ngspice's at two frequencies
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bipolar/`
**Gate**: Build; Run parity-bipolar; Run stdlib
**Commit**: `feat(std): spice Bjt: charge, noise, reported quantities`

### T65: Parity matrix: the bipolar transistor

**What**: Fill the parity matrix of the bipolar transistor (design §6.2): Gummel plot; output curves with Early effect; forward and reverse active, saturation, cut-off; quasi-saturation; both polarities; lateral and vertical; AC and `ft`; storage time; each noise source; three temperatures; each `tlev`. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. Reuse `bjt_ce`, `bjt_mirror`, `temp_bjt`. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/bipolar/`, `crates/piperine-parity/tests/spice_bipolar.rs`
**Depends on**: T64
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-bipolar; Run gates
**Commit**: `test(parity): spice the bipolar transistor matrix`

### T66: Book and close: phase 6

**What**: Write the Book sections of this phase: "Bipolar transistors": Gummel-Poon from the ideal transistor up, Early effect, high injection, base resistance, charge and `ft`, temperature, noise; a common-emitter stage, a current mirror over temperature, a switching transistor. Each device section has the seven parts of design §7 (what it is, choosing it, placing it, a model card as an `alias`, the parameters explained by group, use cases, reading it back and limits); every listing is a run listing; the reference tables are generated from the `///` texts. Regenerate Appendix I. Update `GAPS.md` and `ROADMAP.md` for the phase. Merge the phase.
**Where**: `docs/book/content.html`, `tests/book/`, `docs/spec/appendix_i_spice.md`, `GAPS.md`, `ROADMAP.md`
**Depends on**: T65
**Requirement**: SPM-37 to SPM-41, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] every listing of the new sections is run by the `book` target and every stated value asserted
- [ ] the generated tables equal the library's `///` texts (the test fails on a difference)
- [ ] no phrase the Book's guards refuse; nothing says "as in ngspice" without saying what that is
- [ ] Appendix I is fresh
- [ ] the gates of design §6.4 are at zero for this phase's models
- [ ] the phase is merged to `main`

**Tests**: `tests/book/spice_*.rs`
**Gate**: Build; Run book; Run parity-bipolar; Run gates; Run spec
**Commit**: `docs(book): spice phase 6`

---

## Phase 7: MOS levels 1, 2, 3, 6 and 9

### T67: Interface page: `Mos1`, `Mos2`, `Mos3`, `Mos6`, `Mos9`

**What**: Write `interface/mos.md` for `Mos1`, `Mos2`, `Mos3`, `Mos6`, `Mos9`: per module the ports and one row per param (MSL name, ngspice name, type, default, unit, meaning, given-ness rule, `where`), from `models/*.md`, the ngspice-47 setup, check and parameter files and the model's reference manual; the usage rules; the list of ngspice parameters with no param and the row of design §3.9 each falls under. Apply design §3 as written. The planner of the phase decides what the rules leave open and records it in the page; the user does not review the page. Which of `vto`, `kp`, `gamma`, `phi` is computed from which process params when left out, per level.
**Where**: `.specs/features/spice-models/interface/mos.md`
**Depends on**: T54
**Requirement**: SPM-06, SPM-11, SPM-12, SPM-42
**Executor**: Sonnet

**Done when**:
- [ ] every settable ngspice parameter of these devices is a row or a named exclusion
- [ ] every `unresolved` and `given-ness read` row of the map has a decided declaration
- [ ] every row has a unit, or `—` for a pure number
- [ ] a point design §3 does not decide is decided here by the phase's planner, written under "Decisions taken" with its reason, and listed in the phase's report to the user (the user is informed, not asked)

**Tests**: none (document)
**Gate**: Build
**Commit**: `docs(specs): spice-models interface page, mos`

### T68: Shared MOS parts: Meyer capacitances, bulk junctions

**What**: Functions for the Meyer gate capacitances, the bulk junction currents and charges with area and perimeter, the overlap capacitances, the drain and source resistances from `rd`/`rs` or `rsh` and squares. Decide risk R4 here: `c * ddt(v)`, or ngspice's averaging with the previous capacitance kept in a signal; the gate-charging twin decides. Written from `devsup.c` (`DEVqmeyer`, `DEVfetlim`, `DEVlimvds`), `mos1/mos1load.c` (junctions); the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/mosfet/meyer.msl`
**Depends on**: T67
**Requirement**: SPM-19, SPM-22, SPM-24
**Executor**: Opus

**Done when**:
- [ ] each function equals its C original on a grid
- [ ] the gate-charging twin passes `TRAN_VOLTAGE` with the form chosen, and spike.md records which
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/mosfet/`
**Gate**: Build; Run parity-mosfet; Run stdlib
**Commit**: `feat(std): spice Shared MOS parts: Meyer capacitances, bulk junctions`

### T69: `Mos1`

**What**: Shichman-Hodges with source/drain interchange, process-derived parameters, body effect, the shared parts, noise per `nlev`, the reported quantities, `icv…`, both polarities. Written from `mos1/{mos1set,mos1temp,mos1load,mos1noi,mos1ask}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/mosfet/mos1.msl`
**Depends on**: T68
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] `vto`, `kp`, `gamma`, `phi` derived from process params equal `mos1temp.c`'s
- [ ] current equals ngspice's in each region, in reverse mode and at three body biases
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/mosfet/`
**Gate**: Build; Run parity-mosfet; Run stdlib
**Commit**: `feat(std): spice Mos1`

### T70: `Mos2`

**What**: Grove-Frohman: the saturation voltage by ngspice's quartic solution, velocity saturation, narrow- and short-channel terms, subthreshold with `nfs`. Written from `mos2/{mos2set,mos2temp,mos2load,mos2noi}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/mosfet/mos2.msl`
**Depends on**: T69
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] `vdsat` equals ngspice's on the grid
- [ ] subthreshold slope equals ngspice's
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/mosfet/`
**Gate**: Build; Run parity-mosfet; Run stdlib
**Commit**: `feat(std): spice Mos2`

### T71: `Mos3`

**What**: The semi-empirical model: `theta`, `eta`, `kappa`, `delta`, `vmax`, `xj`. Written from `mos3/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/mosfet/mos3.msl`
**Depends on**: T69
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] current and `vdsat` equal ngspice's on the grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/mosfet/`
**Gate**: Build; Run parity-mosfet; Run stdlib
**Commit**: `feat(std): spice Mos3`

### T72: `Mos6`

**What**: Sakurai-Newton. ngspice has no noise entry for it: the module places none; AC follows from the block. The model's `ps` is `ps_sat`. Written from `mos6/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/mosfet/mos6.msl`
**Depends on**: T69
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] current equals ngspice's on the grid
- [ ] the module places no noise source
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/mosfet/`
**Gate**: Build; Run parity-mosfet; Run stdlib
**Commit**: `feat(std): spice Mos6`

### T73: `Mos9`

**What**: The modified level 3. Written from `mos9/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/mosfet/mos9.msl`
**Depends on**: T71
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] current equals ngspice's on the grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/mosfet/`
**Gate**: Build; Run parity-mosfet; Run stdlib
**Commit**: `feat(std): spice Mos9`

### T74: Parity matrix: `Mos1`, `Mos2`, `Mos3`

**What**: Fill the parity matrix of `Mos1`, `Mos2`, `Mos3` (design §6.2): per model: `id`–`vgs` at low and high `vds`, `id`–`vds` at four `vgs`, each at three body biases; subthreshold, linear, saturation, reverse mode; both polarities; derived against given parameters; junction diodes; AC at five biases; an inverter and a ring oscillator; noise per `nlev`; three temperatures. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. Reuse the fifteen `nmos*` and `noise_mos*` netlists and `uic_mos`. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/mosfet/`, `crates/piperine-parity/tests/spice_mosfet.rs`
**Depends on**: T70, T71
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-mosfet; Run gates
**Commit**: `test(parity): spice Mos1, Mos2, Mos3 matrix`

### T75: Parity matrix: `Mos6`, `Mos9`

**What**: Fill the parity matrix of `Mos6`, `Mos9` (design §6.2): the same rows, and ngspice's `tests/mos6` decks. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/mosfet/`, `crates/piperine-parity/tests/spice_mosfet.rs`
**Depends on**: T72, T73
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-mosfet; Run gates
**Commit**: `test(parity): spice Mos6, Mos9 matrix`

### T76: Book and close: phase 7

**What**: Write the Book sections of this phase: "MOSFETs: the classic levels": level 1 from the square law, process-derived parameters, body effect, junctions and overlap, the Meyer capacitances, levels 2, 3, 6, 9 and what each adds; an inverter's transfer curve and delay, a differential pair. Each device section has the seven parts of design §7 (what it is, choosing it, placing it, a model card as an `alias`, the parameters explained by group, use cases, reading it back and limits); every listing is a run listing; the reference tables are generated from the `///` texts. Regenerate Appendix I. Update `GAPS.md` and `ROADMAP.md` for the phase. Merge the phase.
**Where**: `docs/book/content.html`, `tests/book/`, `docs/spec/appendix_i_spice.md`, `GAPS.md`, `ROADMAP.md`
**Depends on**: T74, T75
**Requirement**: SPM-37 to SPM-41, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] every listing of the new sections is run by the `book` target and every stated value asserted
- [ ] the generated tables equal the library's `///` texts (the test fails on a difference)
- [ ] no phrase the Book's guards refuse; nothing says "as in ngspice" without saying what that is
- [ ] Appendix I is fresh
- [ ] the gates of design §6.4 are at zero for this phase's models
- [ ] the phase is merged to `main`

**Tests**: `tests/book/spice_*.rs`
**Gate**: Build; Run book; Run parity-mosfet; Run gates; Run spec
**Commit**: `docs(book): spice phase 7`

---

## Phase 8: The power MOSFET, MESA and HFET

### T77: Interface page: `Vdmos`, `VdmosSelfHeating`, `Mesa`, `Hfet1`, `Hfet2`

**What**: Write `interface/power.md` for `Vdmos`, `VdmosSelfHeating`, `Mesa`, `Hfet1`, `Hfet2`: per module the ports and one row per param (MSL name, ngspice name, type, default, unit, meaning, given-ness rule, `where`), from `models/*.md`, the ngspice-47 setup, check and parameter files and the model's reference manual; the usage rules; the list of ngspice parameters with no param and the row of design §3.9 each falls under. Apply design §3 as written. The planner of the phase decides what the rules leave open and records it in the page; the user does not review the page. The renamed `mj` and `m_knee`; the three MESA levels over one table; which HFET and MESA defaults the extractor did not find.
**Where**: `.specs/features/spice-models/interface/power.md`
**Depends on**: T54
**Requirement**: SPM-06, SPM-11, SPM-12, SPM-42
**Executor**: Sonnet

**Done when**:
- [ ] every settable ngspice parameter of these devices is a row or a named exclusion
- [ ] every `unresolved` and `given-ness read` row of the map has a decided declaration
- [ ] every row has a unit, or `—` for a pure number
- [ ] a point design §3 does not decide is decided here by the phase's planner, written under "Decisions taken" with its reason, and listed in the phase's report to the user (the user is informed, not asked)

**Tests**: none (document)
**Gate**: Build
**Commit**: `docs(specs): spice-models interface page, power`

### T78: `Vdmos` and `VdmosSelfHeating`

**What**: The power MOSFET: quasi-saturation, subthreshold, the nonlinear gate-drain capacitance, the body diode with recovery, the thermal network with junction and case nodes. Written from `vdmos/{vdmosset,vdmostemp,vdmosload,vdmosnoi,vdmosask,vdmossoachk}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/mosfet/vdmos.msl`
**Depends on**: T77
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] current, the capacitances and the body diode equal ngspice's
- [ ] with the thermal ports held, the self-heating module equals the plain one
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/mosfet/`
**Gate**: Build; Run parity-mosfet; Run stdlib
**Commit**: `feat(std): spice Vdmos and VdmosSelfHeating`

### T79: `Mesa`

**What**: The three levels selected by `level: MesaLevel`; ngspice has no noise entry: none placed. Written from `mesa/{mesasetup,mesatemp,mesaload}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/mesfet/mesa.msl`
**Depends on**: T77
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] each level equals ngspice's on the grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/mesfet/`
**Gate**: Build; Run parity-mesfet; Run stdlib
**Commit**: `feat(std): spice Mesa`

### T80: `Hfet1`

**What**: The HFET model with its gate and parasitic nodes. Written from `hfet1/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/mesfet/hfet1.msl`
**Depends on**: T77
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] current and charges equal ngspice's on the grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/mesfet/`
**Gate**: Build; Run parity-mesfet; Run stdlib
**Commit**: `feat(std): spice Hfet1`

### T81: `Hfet2`

**What**: The simplified HFET. Written from `hfet2/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/mesfet/hfet2.msl`
**Depends on**: T80
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] current and charges equal ngspice's on the grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/mesfet/`
**Gate**: Build; Run parity-mesfet; Run stdlib
**Commit**: `feat(std): spice Hfet2`

### T82: Parity matrix: `Vdmos`, `Mesa`, `Hfet1`, `Hfet2`

**What**: Fill the parity matrix of `Vdmos`, `Mesa`, `Hfet1`, `Hfet2` (design §6.2): per model: transfer and output curves, both polarities, AC, transient switching, temperature; VDMOS body diode and self-heating; each MESA level; ngspice's `tests/mesa` and `tests/hfet` decks. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/mesfet/`, `crates/piperine-parity/tests/spice_mesfet.rs`
**Depends on**: T78, T79, T81
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-mesfet; Run gates
**Commit**: `test(parity): spice Vdmos, Mesa, Hfet1, Hfet2 matrix`

### T83: Book and close: phase 8

**What**: Write the Book sections of this phase: "MOSFETs: the classic levels": the power MOSFET and a power switch with its body diode; "JFETs, MESFETs and HFETs": MESA and HFET. Each device section has the seven parts of design §7 (what it is, choosing it, placing it, a model card as an `alias`, the parameters explained by group, use cases, reading it back and limits); every listing is a run listing; the reference tables are generated from the `///` texts. Regenerate Appendix I. Update `GAPS.md` and `ROADMAP.md` for the phase. Merge the phase.
**Where**: `docs/book/content.html`, `tests/book/`, `docs/spec/appendix_i_spice.md`, `GAPS.md`, `ROADMAP.md`
**Depends on**: T82
**Requirement**: SPM-37 to SPM-41, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] every listing of the new sections is run by the `book` target and every stated value asserted
- [ ] the generated tables equal the library's `///` texts (the test fails on a difference)
- [ ] no phrase the Book's guards refuse; nothing says "as in ngspice" without saying what that is
- [ ] Appendix I is fresh
- [ ] the gates of design §6.4 are at zero for this phase's models
- [ ] the phase is merged to `main`

**Tests**: `tests/book/spice_*.rs`
**Gate**: Build; Run book; Run parity-mesfet; Run gates; Run spec
**Commit**: `docs(book): spice phase 8`

---

## Phase 9: The history convolution primitive

Capability N3, created inside this feature (user, 2026-11-27): `convolve`, a core primitive, which the lossy line needs and any model with a memory kernel can use. Design §15 is its specification. It touches the language, the plan, both backends, the device ABI and the analog solver.

### T84: `convolve` in the specifications

**What**: Write design §15 into the language specification (§14: the table row and a new subsection; §16.2: its row in every kind of solve; Appendix B.2: where it may be placed; §8.3.6; the error table) and into the device ABI's specification `docs/abi/` (the term operator `CONVOLUTION`, its aspects and fields, the validation rules, the access table). Re-check every name against `docs/abi/` as `device-abi-simplification` left it; where design §15.4 and that document differ in a name, the document's vocabulary wins and design §15.4 is corrected. The ABI version does not move.
**Where**: `docs/spec/{part_iv_behavior,part_v_execution,appendix_b_grammar,part_iii_structure}.md`, `docs/abi/**`
**Depends on**: T45
**Requirement**: SPM-55 to SPM-58
**Executor**: Opus

**Done when**:
- [ ] `convolve` has its signature, its static rules and one row per kind of solve
- [ ] `docs/abi/` states the operator's contract in one sentence, its fields with offsets, and its rules
- [ ] both documents' guards pass
- [ ] the exported symbol and `MAJOR`/`MINOR` are unchanged

**Tests**: the documentation guards; the ABI layout test reads the new listings
**Gate**: Build; Run spec
**Commit**: `docs(spec): the convolve primitive`

### T85: `CONVOLUTION` in the device ABI

**What**: Add the fourth term operator and what it needs to `piperine-abi`: the term's kernel function and response function references, the two compaction tolerances, the aspects the solver writes (`TERM_KERNEL_LAG`) and the device writes (`TERM_KERNEL_VALUE`, `TERM_STEADY_GAIN`, `TERM_RESPONSE`), the builder entries, the validation rules, the layout test. Contract in place, no version bump, no second symbol.
**Where**: `crates/piperine-abi/src/**`, `crates/piperine-abi/tests/contract/**`, the transpiler's embedded source table
**Depends on**: T84
**Requirement**: SPM-57
**Executor**: Opus

**Done when**:
- [ ] the layout and constant tests pass against `docs/abi/`
- [ ] each new validation rule refuses its violating shape with the entry named
- [ ] a kernel function that reads anything but the lag and parameters is refused
- [ ] every existing target of the crate passes unchanged

**Tests**: `crates/piperine-abi/tests/contract/`
**Gate**: Build; `cargo test -p piperine-abi`
**Commit**: `feat(abi): the CONVOLUTION term operator`

### T86: `convolve` in the analog solver

**What**: The history of a convolution term (accepted times and values, with marks, restore and release as the other histories have), the weights from the kernel's second integral over the piecewise-linear history, the present sample's weight as the term's partial, the history sum computed once per step, compaction of collinear samples under the term's tolerances, the equilibrium value from the steady gain, the small-signal response in AC and noise, the behaviour at a jump and on a rejected step.
**Where**: `crates/piperine-analog-solver/src/**`, `crates/piperine-analog-solver/tests/boundary/**`
**Depends on**: T85
**Requirement**: SPM-56, SPM-57
**Executor**: Opus

**Done when**:
- [ ] a hand-written device with an exponential kernel equals the RC filter it is, in transient (first and second order steps), AC and DC
- [ ] a rejected step and a restore leave the history as it was
- [ ] compaction changes a result by less than its tolerance and bounds the history of a constant signal
- [ ] a non-finite kernel value fails the solve with `E0507` naming the term

**Tests**: `crates/piperine-analog-solver/tests/boundary/convolution.rs`
**Gate**: Build; `cargo test -p piperine-analog-solver`
**Commit**: `feat(solver): convolution terms`

### T87: `convolve` in the language and both backends

**What**: The core module `convolve` from the checker to the plan to the interpreter and the generated Rust: its value inputs, the kernel and response closures lowered to device functions, its partial, its placement rules, its diagnostics.
**Where**: `crates/piperine-language/src/**`, `crates/piperine-plan/**`, `crates/piperine-interpreter/**`, `crates/piperine-transpile/**`
**Depends on**: T86
**Requirement**: SPM-55, SPM-56
**Executor**: Opus

**Done when**:
- [ ] an MSL module using `convolve` with an exponential kernel equals an RC low-pass under `TRAN_VOLTAGE`, `AC_MAGNITUDE` and `AC_PHASE`, on both backends
- [ ] a kernel that is zero before a lag gives a pure delay equal to `delay`
- [ ] each misuse has its code and span
- [ ] `ddx` through `convolve` holds its result constant, as for any instance (§14.2.3)

**Tests**: `crates/piperine-parity/tests/operators/convolve*.msl`, the language crate's tests
**Gate**: Build; Run language; `cargo test -p piperine-parity --test operators`
**Commit**: `feat(lang): the convolve primitive`

### T88: `convolve` in the Book, and close

**What**: The Book's chapter "Core and standard library" gains `convolve`: what a memory kernel is, the second integral and why the model supplies it, a worked low-pass with its response, when to prefer `laplace_*` or `delay`. `GAPS.md`: N3 closed. Merge the phase.
**Where**: `docs/book/content.html`, `tests/book/`, `GAPS.md`
**Depends on**: T87
**Requirement**: SPM-58
**Executor**: Sonnet

**Done when**:
- [ ] every listing runs
- [ ] the cheat sheet and the table of core primitives list it
- [ ] the phase is merged to `main`

**Tests**: `tests/book/`
**Gate**: Build; Run book
**Commit**: `docs(book): the convolve primitive`

---

## Phase 10: Lossy and coupled lines

### T89: Interface page: `SingleLossyTransmissionLine`, `CoupledTransmissionLines`, `LossyTransmissionLine`

**What**: Write `interface/lines.md` for `SingleLossyTransmissionLine`, `CoupledTransmissionLines`, `LossyTransmissionLine`: per module the ports and one row per param (MSL name, ngspice name, type, default, unit, meaning, given-ness rule, `where`), from `models/*.md`, the ngspice-47 setup, check and parameter files and the model's reference manual; the usage rules; the list of ngspice parameters with no param and the row of design §3.9 each falls under. Apply design §3 as written. The planner of the phase decides what the rules leave open and records it in the page; the user does not review the page. For the LTRA model: which of its kernels (`h1'`, `h2`, `h3'`) each line case (RLC, RC, RG, LC) uses, and the closed forms of their second integrals in `ltramisc.c`.
**Where**: `.specs/features/spice-models/interface/lines.md`
**Depends on**: T88
**Requirement**: SPM-06, SPM-11, SPM-12, SPM-42
**Executor**: Opus

**Done when**:
- [ ] every settable ngspice parameter of these devices is a row or a named exclusion
- [ ] every `unresolved` and `given-ness read` row of the map has a decided declaration
- [ ] every row has a unit, or `—` for a pure number
- [ ] a point design §3 does not decide is decided here by the phase's planner, written under "Decisions taken" with its reason, and listed in the phase's report to the user (the user is informed, not asked)

**Tests**: none (document)
**Gate**: Build
**Commit**: `docs(specs): spice-models interface page, lines`

### T90: `SingleLossyTransmissionLine`

**What**: ngspice's Padé model of a single lossy line: the delay and the rational part, the coefficients computed in body `let`s by `txlsetup.c`'s algorithm, as `laplace_*` filters and a `delay`. Written from `txl/{txlsetup,txlload}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/line.msl`
**Depends on**: T89
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] the poles and residues equal `txlsetup.c`'s for three lines
- [ ] a step response equals ngspice's under `TRAN_VOLTAGE`
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/line/`
**Gate**: Build; Run parity-line; Run stdlib
**Commit**: `feat(std): spice SingleLossyTransmissionLine`

### T91: `CoupledTransmissionLines`

**What**: The multiconductor model for `N` lines: modal decomposition and the per-mode approximation of `cplsetup.c`, with const-generic `N` and matrix params. Written from `cpl/{cplsetup,cplload}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/line.msl`
**Depends on**: T90
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] `N = 1` equals `SingleLossyTransmissionLine`
- [ ] a coupled pair's near- and far-end crosstalk equal ngspice's
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/line/`
**Gate**: Build; Run parity-line; Run stdlib
**Commit**: `feat(std): spice CoupledTransmissionLines`

### T92: `LossyTransmissionLine`

**What**: The four line cases of the LTRA model. Each port equation is a direct term, a `delay` term for the wavefront and `convolve` terms whose second integrals are the closed forms of `ltramisc.c`; the AC responses are `ltraacld.c`'s; the step-control options map to `bound_step` and the compaction tolerances. Written from `ltra/{ltraset,ltratemp,ltraload,ltraacld,ltramisc,ltratrun,ltraacct}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/line.msl`
**Depends on**: T89
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] each kernel's second integral equals `ltramisc.c`'s on a grid of lags
- [ ] a lossless LTRA line equals `LosslessTransmissionLine`
- [ ] the RC, RG, LC and RLC cases each run a step response
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/line/`
**Gate**: Build; Run parity-line; Run stdlib
**Commit**: `feat(std): spice LossyTransmissionLine`

### T93: Parity matrix: the lossy and coupled lines

**What**: Fill the parity matrix of the lossy and coupled lines (design §6.2): matched, open and shorted ends; three lengths; a step and a sine; each LTRA case; AC of each; a coupled pair's crosstalk; ngspice's `tests/transmission` decks. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/line/`, `crates/piperine-parity/tests/spice_line.rs`
**Depends on**: T91, T92
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-line; Run gates
**Commit**: `test(parity): spice the lossy and coupled lines matrix`

### T94: Book and close: phase 10

**What**: Write the Book sections of this phase: "Transmission lines": lossy and coupled lines, which model for which problem; a mismatched line's ringing, crosstalk on a coupled pair. Each device section has the seven parts of design §7 (what it is, choosing it, placing it, a model card as an `alias`, the parameters explained by group, use cases, reading it back and limits); every listing is a run listing; the reference tables are generated from the `///` texts. Regenerate Appendix I. Update `GAPS.md` and `ROADMAP.md` for the phase. Merge the phase.
**Where**: `docs/book/content.html`, `tests/book/`, `docs/spec/appendix_i_spice.md`, `GAPS.md`, `ROADMAP.md`
**Depends on**: T93
**Requirement**: SPM-37 to SPM-41, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] every listing of the new sections is run by the `book` target and every stated value asserted
- [ ] the generated tables equal the library's `///` texts (the test fails on a difference)
- [ ] no phrase the Book's guards refuse; nothing says "as in ngspice" without saying what that is
- [ ] Appendix I is fresh
- [ ] the gates of design §6.4 are at zero for this phase's models
- [ ] the phase is merged to `main`

**Tests**: `tests/book/spice_*.rs`
**Gate**: Build; Run book; Run parity-line; Run gates; Run spec
**Commit**: `docs(book): spice phase 10`

---

## Phase 11: BSIM1 and BSIM2

The rehearsal for BSIM3: the first models whose parameters carry length and width sensitivities.

### T95: Interface page: `Bsim1`, `Bsim2`

**What**: Write `interface/bsim12.md` for `Bsim1`, `Bsim2`: per module the ports and one row per param (MSL name, ngspice name, type, default, unit, meaning, given-ness rule, `where`), from `models/*.md`, the ngspice-47 setup, check and parameter files and the model's reference manual; the usage rules; the list of ngspice parameters with no param and the row of design §3.9 each falls under. Apply design §3 as written. The planner of the phase decides what the rules leave open and records it in the page; the user does not review the page. 
**Where**: `.specs/features/spice-models/interface/bsim12.md`
**Depends on**: T76, T17
**Requirement**: SPM-06, SPM-11, SPM-12, SPM-42
**Executor**: Sonnet

**Done when**:
- [ ] every settable ngspice parameter of these devices is a row or a named exclusion
- [ ] every `unresolved` and `given-ness read` row of the map has a decided declaration
- [ ] every row has a unit, or `—` for a pure number
- [ ] a point design §3 does not decide is decided here by the phase's planner, written under "Decisions taken" with its reason, and listed in the phase's report to the user (the user is informed, not asked)

**Tests**: none (document)
**Gate**: Build
**Commit**: `docs(specs): spice-models interface page, bsim12`

### T96: `Bsim1`

**What**: The size-dependent parameters as body `let`s; `b1eval.c`'s current; `b1moscap.c`'s charges for each `xpart`; junctions; noise. Written from `bsim1/{b1set,b1temp,b1eval,b1ld,b1moscap,b1noi}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bsim/bsim1.msl`
**Depends on**: T95
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] current and charges equal ngspice's on the geometry grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bsim/`
**Gate**: Build; Run parity-bsim; Run stdlib
**Commit**: `feat(std): spice Bsim1`

### T97: `Bsim2`

**What**: The same structure with BSIM2's equations and 135 params. Written from `bsim2/{b2set,b2temp,b2eval,b2ld,b2moscap,b2noi}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bsim/bsim2.msl`
**Depends on**: T96
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] current and charges equal ngspice's on the geometry grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bsim/`
**Gate**: Build; Run parity-bsim; Run stdlib
**Commit**: `feat(std): spice Bsim2`

### T98: Parity matrix: `Bsim1`, `Bsim2`

**What**: Fill the parity matrix of `Bsim1`, `Bsim2` (design §6.2): the MOS rows of design §6.2 and a geometry grid (short, long, narrow, wide, four corners); ngspice's `tests/bsim1` and `tests/bsim2` decks. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/bsim/`, `crates/piperine-parity/tests/spice_bsim.rs`
**Depends on**: T97
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-bsim; Run gates
**Commit**: `test(parity): spice Bsim1, Bsim2 matrix`

### T99: Book and close: phase 11

**What**: Write the Book sections of this phase: "MOSFETs: BSIM": what a BSIM model is, size-dependent parameters, BSIM1 and BSIM2. Each device section has the seven parts of design §7 (what it is, choosing it, placing it, a model card as an `alias`, the parameters explained by group, use cases, reading it back and limits); every listing is a run listing; the reference tables are generated from the `///` texts. Regenerate Appendix I. Update `GAPS.md` and `ROADMAP.md` for the phase. Merge the phase.
**Where**: `docs/book/content.html`, `tests/book/`, `docs/spec/appendix_i_spice.md`, `GAPS.md`, `ROADMAP.md`
**Depends on**: T98
**Requirement**: SPM-37 to SPM-41, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] every listing of the new sections is run by the `book` target and every stated value asserted
- [ ] the generated tables equal the library's `///` texts (the test fails on a difference)
- [ ] no phrase the Book's guards refuse; nothing says "as in ngspice" without saying what that is
- [ ] Appendix I is fresh
- [ ] the gates of design §6.4 are at zero for this phase's models
- [ ] the phase is merged to `main`

**Tests**: `tests/book/spice_*.rs`
**Gate**: Build; Run book; Run parity-bsim; Run gates; Run spec
**Commit**: `docs(book): spice phase 11`

---

## Phase 12: BSIM3

### T100: Interface page: `Bsim3` and what `Bsim3V32`, `Bsim3V1`, `Bsim3V0` change

**What**: Write `interface/bsim3.md` for `Bsim3` and what `Bsim3V32`, `Bsim3V1`, `Bsim3V0` change: per module the ports and one row per param (MSL name, ngspice name, type, default, unit, meaning, given-ness rule, `where`), from `models/*.md`, the ngspice-47 setup, check and parameter files and the model's reference manual; the usage rules; the list of ngspice parameters with no param and the row of design §3.9 each falls under. Apply design §3 as written. The planner of the phase decides what the rules leave open and records it in the page; the user does not review the page. All 423 model params with units from the BSIM3v3.3 manual, grouped; the selectors and their accepted values; which checks of `b3check.c` are fatal.
**Where**: `.specs/features/spice-models/interface/bsim3.md`
**Depends on**: T99
**Requirement**: SPM-06, SPM-11, SPM-12, SPM-42
**Executor**: Opus

**Done when**:
- [ ] every settable ngspice parameter of these devices is a row or a named exclusion
- [ ] every `unresolved` and `given-ness read` row of the map has a decided declaration
- [ ] every row has a unit, or `—` for a pure number
- [ ] a point design §3 does not decide is decided here by the phase's planner, written under "Decisions taken" with its reason, and listed in the phase's report to the user (the user is informed, not asked)

**Tests**: none (document)
**Gate**: Build
**Commit**: `docs(specs): spice-models interface page, bsim3`

### T101: `Bsim3`: setup and temperature

**What**: The params; the size- and temperature-dependent parameters (`pParam`) as body `let`s; the `where`s of the fatal checks. Written from `bsim3/{b3set,b3temp,b3check}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bsim/bsim3.msl`
**Depends on**: T100
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] every `pParam` field equals ngspice's for the four corners of the geometry grid at three temperatures
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bsim/`
**Gate**: Build; Run parity-bsim; Run stdlib
**Commit**: `feat(std): spice Bsim3: setup and temperature`

### T102: `Bsim3`: drain current

**What**: Threshold, mobility for each `mobmod`, saturation, output resistance, substrate current, the junction diodes, source/drain resistance, following `b3ld.c` statement by statement. Written from `bsim3/b3ld.c` (DC part); the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bsim/bsim3.msl`
**Depends on**: T101
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] `ids`, `isub`, `vth`, `vdsat` equal ngspice's on the grid for each `mobmod`
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bsim/`
**Gate**: Build; Run parity-bsim; Run stdlib
**Commit**: `feat(std): spice Bsim3: drain current`

### T103: `Bsim3`: charge, NQS, noise, reported quantities

**What**: Charges for `capmod` 0 to 3 and each `xpart`; the NQS charge node; thermal and flicker noise for each `noimod`; the reported quantities; safe-operating-area records. Written from `bsim3/{b3ld,b3noi,b3ask,b3soachk}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bsim/bsim3.msl`
**Depends on**: T102
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] the capacitances equal ngspice's at five biases per `capmod`
- [ ] noise equals ngspice's per `noimod`
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bsim/`
**Gate**: Build; Run parity-bsim; Run stdlib
**Commit**: `feat(std): spice Bsim3: charge, NQS, noise, reported quantities`

### T104: Parity matrix: `Bsim3`

**What**: Fill the parity matrix of `Bsim3` (design §6.2): the MOS rows; the geometry grid; each selector at each value; NQS in AC and transient; a binned card; three temperatures; ngspice's `tests/bsim3` decks. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. Cards are generated from one table (T16). A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/bsim/`, `crates/piperine-parity/tests/spice_bsim.rs`
**Depends on**: T103
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-bsim; Run gates
**Commit**: `test(parity): spice Bsim3 matrix`

### T105: `Bsim3V32`

**What**: Version 3.2 to 3.2.4, written as `Bsim3` with the differences `models/bsim.md` lists and a line-by-line comparison of the two load functions. Written from `bsim3v32/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bsim/bsim3_v32.msl`
**Depends on**: T104
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] current and charges equal ngspice's `bsim3v32` on the grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bsim/`
**Gate**: Build; Run parity-bsim; Run stdlib
**Commit**: `feat(std): spice Bsim3V32`

### T106: `Bsim3V1` and `Bsim3V0`

**What**: Versions 3.1 and 3.0. Written from `bsim3v1/*`, `bsim3v0/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bsim/{bsim3_v1,bsim3_v0}.msl`
**Depends on**: T105
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] each equals its ngspice model on the grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bsim/`
**Gate**: Build; Run parity-bsim; Run stdlib
**Commit**: `feat(std): spice Bsim3V1 and Bsim3V0`

### T107: Parity matrix: the earlier BSIM3 versions

**What**: Fill the parity matrix of the earlier BSIM3 versions (design §6.2): the DC, AC and noise rows on the grid, per version, with a card of that version. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/bsim/`, `crates/piperine-parity/tests/spice_bsim.rs`
**Depends on**: T106
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-bsim; Run gates
**Commit**: `test(parity): spice the earlier BSIM3 versions matrix`

### T108: Book and close: phase 12

**What**: Write the Book sections of this phase: "MOSFETs: BSIM": BSIM3, working with a foundry card (an alias per card, binning, corners, mismatch), the parameter groups, the selectors, the earlier versions; a ring oscillator from a card, `gm/id` curves. Each device section has the seven parts of design §7 (what it is, choosing it, placing it, a model card as an `alias`, the parameters explained by group, use cases, reading it back and limits); every listing is a run listing; the reference tables are generated from the `///` texts. Regenerate Appendix I. Update `GAPS.md` and `ROADMAP.md` for the phase. Merge the phase.
**Where**: `docs/book/content.html`, `tests/book/`, `docs/spec/appendix_i_spice.md`, `GAPS.md`, `ROADMAP.md`
**Depends on**: T107
**Requirement**: SPM-37 to SPM-41, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] every listing of the new sections is run by the `book` target and every stated value asserted
- [ ] the generated tables equal the library's `///` texts (the test fails on a difference)
- [ ] no phrase the Book's guards refuse; nothing says "as in ngspice" without saying what that is
- [ ] Appendix I is fresh
- [ ] the gates of design §6.4 are at zero for this phase's models
- [ ] the phase is merged to `main`

**Tests**: `tests/book/spice_*.rs`
**Gate**: Build; Run book; Run parity-bsim; Run gates; Run spec
**Commit**: `docs(book): spice phase 12`

---

## Phase 13: VBIC and HICUM

### T109: Interface page: `Vbic`, `VbicSelfHeating`

**What**: Write `interface/vbic.md` for `Vbic`, `VbicSelfHeating`: per module the ports and one row per param (MSL name, ngspice name, type, default, unit, meaning, given-ness rule, `where`), from `models/*.md`, the ngspice-47 setup, check and parameter files and the model's reference manual; the usage rules; the list of ngspice parameters with no param and the row of design §3.9 each falls under. Apply design §3 as written. The planner of the phase decides what the rules leave open and records it in the page; the user does not review the page. 
**Where**: `.specs/features/spice-models/interface/vbic.md`
**Depends on**: T66
**Requirement**: SPM-06, SPM-11, SPM-12, SPM-42
**Executor**: Opus

**Done when**:
- [ ] every settable ngspice parameter of these devices is a row or a named exclusion
- [ ] every `unresolved` and `given-ness read` row of the map has a decided declaration
- [ ] every row has a unit, or `—` for a pure number
- [ ] a point design §3 does not decide is decided here by the phase's planner, written under "Decisions taken" with its reason, and listed in the phase's report to the user (the user is informed, not asked)

**Tests**: none (document)
**Gate**: Build
**Commit**: `docs(specs): spice-models interface page, vbic`

### T110: `Vbic`: currents and temperature

**What**: VBIC's transport, base, parasitic transistor, avalanche and quasi-saturation currents; the temperature update. Written from `vbic/{vbicsetup,vbictemp,vbicload}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bipolar/vbic.msl`
**Depends on**: T109
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] each branch current equals ngspice's on the bias grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bipolar/`
**Gate**: Build; Run parity-bipolar; Run stdlib
**Commit**: `feat(std): spice Vbic: currents and temperature`

### T111: `Vbic`: charge, excess phase, noise, self-heating

**What**: Charges, the excess-phase nodes, the thirteen noise sources, the reported quantities, `VbicSelfHeating` with the `dt` port. Written from `vbic/{vbicload,vbicnoise,vbicask,vbicsoachk}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bipolar/vbic.msl`
**Depends on**: T110
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] `ft` and noise equal ngspice's
- [ ] with `dt` held the self-heating module equals the plain one
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bipolar/`
**Gate**: Build; Run parity-bipolar; Run stdlib
**Commit**: `feat(std): spice Vbic: charge, excess phase, noise, self-heating`

### T112: Parity matrix: `Vbic`

**What**: Fill the parity matrix of `Vbic` (design §6.2): the bipolar rows; avalanche; self-heating in DC and transient; ngspice's `tests/vbic` decks. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/bipolar/`, `crates/piperine-parity/tests/spice_bipolar.rs`
**Depends on**: T111
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-bipolar; Run gates
**Commit**: `test(parity): spice Vbic matrix`

### T113: Interface page: `Hicum2`, `Hicum2SelfHeating`

**What**: Write `interface/hicum.md` for `Hicum2`, `Hicum2SelfHeating`: per module the ports and one row per param (MSL name, ngspice name, type, default, unit, meaning, given-ness rule, `where`), from `models/*.md`, the ngspice-47 setup, check and parameter files and the model's reference manual; the usage rules; the list of ngspice parameters with no param and the row of design §3.9 each falls under. Apply design §3 as written. The planner of the phase decides what the rules leave open and records it in the page; the user does not review the page. 
**Where**: `.specs/features/spice-models/interface/hicum.md`
**Depends on**: T66
**Requirement**: SPM-06, SPM-11, SPM-12, SPM-42
**Executor**: Opus

**Done when**:
- [ ] every settable ngspice parameter of these devices is a row or a named exclusion
- [ ] every `unresolved` and `given-ness read` row of the map has a decided declaration
- [ ] every row has a unit, or `—` for a pure number
- [ ] a point design §3 does not decide is decided here by the phase's planner, written under "Decisions taken" with its reason, and listed in the phase's report to the user (the user is informed, not asked)

**Tests**: none (document)
**Gate**: Build
**Commit**: `docs(specs): spice-models interface page, hicum`

### T114: `Hicum2`: currents and temperature

**What**: HICUM level 2's transfer current with its implicit charge equation as an internal node, the base, avalanche and tunnelling currents, the temperature update. Written from `hicum2/{hicum2setup,hicumL2,hicumL2temp}.*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bipolar/hicum2.msl`
**Depends on**: T113
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] each branch current equals ngspice's on the bias grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bipolar/`
**Gate**: Build; Run parity-bipolar; Run stdlib
**Commit**: `feat(std): spice Hicum2: currents and temperature`

### T115: `Hicum2`: charge, NQS, noise, self-heating

**What**: Charges, the vertical NQS nodes, the fourteen noise sources with correlation where the model has it, `Hicum2SelfHeating`. Written from `hicum2/{hicumL2,hicum2noise,hicum2ask}.*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bipolar/hicum2.msl`
**Depends on**: T114
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] `ft` and noise equal ngspice's
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bipolar/`
**Gate**: Build; Run parity-bipolar; Run stdlib
**Commit**: `feat(std): spice Hicum2: charge, NQS, noise, self-heating`

### T116: Parity matrix: `Hicum2`

**What**: Fill the parity matrix of `Hicum2` (design §6.2): the bipolar rows; high-current fall-off; self-heating; ngspice's `tests/hicum2` decks. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/bipolar/`, `crates/piperine-parity/tests/spice_bipolar.rs`
**Depends on**: T115
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-bipolar; Run gates
**Commit**: `test(parity): spice Hicum2 matrix`

### T117: Book and close: phase 13

**What**: Write the Book sections of this phase: "Bipolar transistors": VBIC and what it adds, HICUM, self-heating. Each device section has the seven parts of design §7 (what it is, choosing it, placing it, a model card as an `alias`, the parameters explained by group, use cases, reading it back and limits); every listing is a run listing; the reference tables are generated from the `///` texts. Regenerate Appendix I. Update `GAPS.md` and `ROADMAP.md` for the phase. Merge the phase.
**Where**: `docs/book/content.html`, `tests/book/`, `docs/spec/appendix_i_spice.md`, `GAPS.md`, `ROADMAP.md`
**Depends on**: T112, T116
**Requirement**: SPM-37 to SPM-41, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] every listing of the new sections is run by the `book` target and every stated value asserted
- [ ] the generated tables equal the library's `///` texts (the test fails on a difference)
- [ ] no phrase the Book's guards refuse; nothing says "as in ngspice" without saying what that is
- [ ] Appendix I is fresh
- [ ] the gates of design §6.4 are at zero for this phase's models
- [ ] the phase is merged to `main`

**Tests**: `tests/book/spice_*.rs`
**Gate**: Build; Run book; Run parity-bipolar; Run gates; Run spec
**Commit**: `docs(book): spice phase 13`

---

## Phase 14: BSIM4

### T118: Interface page: `Bsim4` and what `Bsim4V7`, `Bsim4V6`, `Bsim4V5` change

**What**: Write `interface/bsim4.md` for `Bsim4` and what `Bsim4V7`, `Bsim4V6`, `Bsim4V5` change: per module the ports and one row per param (MSL name, ngspice name, type, default, unit, meaning, given-ness rule, `where`), from `models/*.md`, the ngspice-47 setup, check and parameter files and the model's reference manual; the usage rules; the list of ngspice parameters with no param and the row of design §3.9 each falls under. Apply design §3 as written. The planner of the phase decides what the rules leave open and records it in the page; the user does not review the page. All 891 model params with units from the BSIM4.8 manual, grouped; the selectors and their accepted values; the fatal checks of `b4check.c`; the instance params that override the card.
**Where**: `.specs/features/spice-models/interface/bsim4.md`
**Depends on**: T108
**Requirement**: SPM-06, SPM-11, SPM-12, SPM-42
**Executor**: Opus

**Done when**:
- [ ] every settable ngspice parameter of these devices is a row or a named exclusion
- [ ] every `unresolved` and `given-ness read` row of the map has a decided declaration
- [ ] every row has a unit, or `—` for a pure number
- [ ] a point design §3 does not decide is decided here by the phase's planner, written under "Decisions taken" with its reason, and listed in the phase's report to the user (the user is informed, not asked)

**Tests**: none (document)
**Gate**: Build
**Commit**: `docs(specs): spice-models interface page, bsim4`

### T119: `Bsim4`: setup and temperature

**What**: The params; `pParam` as body `let`s; the layout-dependent parasitics of `b4geo.c`; stress and well-proximity terms; the `where`s. Written from `bsim4/{b4set,b4temp,b4check,b4geo}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bsim/bsim4.msl`
**Depends on**: T118
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] every `pParam` field equals ngspice's for the grid corners at three temperatures
- [ ] the source/drain geometry of each `geomod` equals `b4geo.c`'s
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bsim/`
**Gate**: Build; Run parity-bsim; Run stdlib
**Commit**: `feat(std): spice Bsim4: setup and temperature`

### T120: `Bsim4`: drain current

**What**: Threshold, mobility for each `mobmod`, saturation, output resistance, `rdsmod`, impact ionization, the junctions with each `diomod`. Written from `bsim4/b4ld.c` (DC part); the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bsim/bsim4.msl`
**Depends on**: T119
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] `ids`, `isub`, `vth`, `vdsat` equal ngspice's on the grid for each selector value
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bsim/`
**Gate**: Build; Run parity-bsim; Run stdlib
**Commit**: `feat(std): spice Bsim4: drain current`

### T121: `Bsim4`: gate current, GIDL, the gate and body networks

**What**: Gate tunnelling for each `igcmod`/`igbmod`, GIDL and GISL, the gate resistance network for each `rgatemod` and the body network for each `rbodymod`: structural params that add nodes. Written from `bsim4/b4ld.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bsim/bsim4.msl`
**Depends on**: T120
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] each gate-current component equals ngspice's
- [ ] the node count follows `rgatemod` and `rbodymod` as in ngspice
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bsim/`
**Gate**: Build; Run parity-bsim; Run stdlib
**Commit**: `feat(std): spice Bsim4: gate current, GIDL, the gate and body networks`

### T122: `Bsim4`: charge, NQS, noise, reported quantities

**What**: Charges for each `capmod`; transient and AC NQS; thermal noise for each `tnoimod` with the correlated induced gate noise as one source on two paths; flicker for each `fnoimod`; gate-current shot noise; the reported quantities. Written from `bsim4/{b4ld,b4noi,b4ask,b4soachk}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bsim/bsim4.msl`
**Depends on**: T121
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] capacitances equal ngspice's at five biases per `capmod`
- [ ] the noise of each source and the correlated term equal ngspice's
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bsim/`
**Gate**: Build; Run parity-bsim; Run stdlib
**Commit**: `feat(std): spice Bsim4: charge, NQS, noise, reported quantities`

### T123: Parity matrix: `Bsim4`

**What**: Fill the parity matrix of `Bsim4` (design §6.2): the MOS rows; the geometry grid; each selector at each value; gate current; NQS; stress parameters; a binned card; three temperatures; ngspice's `tests/bsim4` decks. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. Cards are generated from one table (T16). A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/bsim/`, `crates/piperine-parity/tests/spice_bsim.rs`
**Depends on**: T122
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-bsim; Run gates
**Commit**: `test(parity): spice Bsim4 matrix`

### T124: `Bsim4V7`

**What**: Version 4.7, as `Bsim4` with its differences. Written from `bsim4v7/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bsim/bsim4_v7.msl`
**Depends on**: T123
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] equals ngspice's `bsim4v7` on the grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bsim/`
**Gate**: Build; Run parity-bsim; Run stdlib
**Commit**: `feat(std): spice Bsim4V7`

### T125: `Bsim4V6` and `Bsim4V5`

**What**: Versions 4.6 and 4.5. Written from `bsim4v6/*`, `bsim4v5/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/bsim/{bsim4_v6,bsim4_v5}.msl`
**Depends on**: T124
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Sonnet

**Done when**:
- [ ] each equals its ngspice model on the grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/bsim/`
**Gate**: Build; Run parity-bsim; Run stdlib
**Commit**: `feat(std): spice Bsim4V6 and Bsim4V5`

### T126: Parity matrix: the earlier BSIM4 versions

**What**: Fill the parity matrix of the earlier BSIM4 versions (design §6.2): the DC, AC and noise rows on the grid, per version, with a card of that version. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/bsim/`, `crates/piperine-parity/tests/spice_bsim.rs`
**Depends on**: T125
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-bsim; Run gates
**Commit**: `test(parity): spice the earlier BSIM4 versions matrix`

### T127: Book and close: phase 14

**What**: Write the Book sections of this phase: "MOSFETs: BSIM": BSIM4, its parameter groups and selectors, layout effects, the noise models, the earlier versions; `ft` against bias, a binned card. Each device section has the seven parts of design §7 (what it is, choosing it, placing it, a model card as an `alias`, the parameters explained by group, use cases, reading it back and limits); every listing is a run listing; the reference tables are generated from the `///` texts. Regenerate Appendix I. Update `GAPS.md` and `ROADMAP.md` for the phase. Merge the phase.
**Where**: `docs/book/content.html`, `tests/book/`, `docs/spec/appendix_i_spice.md`, `GAPS.md`, `ROADMAP.md`
**Depends on**: T126
**Requirement**: SPM-37 to SPM-41, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] every listing of the new sections is run by the `book` target and every stated value asserted
- [ ] the generated tables equal the library's `///` texts (the test fails on a difference)
- [ ] no phrase the Book's guards refuse; nothing says "as in ngspice" without saying what that is
- [ ] Appendix I is fresh
- [ ] the gates of design §6.4 are at zero for this phase's models
- [ ] the phase is merged to `main`

**Tests**: `tests/book/spice_*.rs`
**Gate**: Build; Run book; Run parity-bsim; Run gates; Run spec
**Commit**: `docs(book): spice phase 14`

---

## Phase 15: Silicon-on-insulator

### T128: Interface page: `Soi3`, `Bsim3SoiFd`, `Bsim3SoiDd`, `Bsim3SoiPd`, `Bsim4Soi`

**What**: Write `interface/soi.md` for `Soi3`, `Bsim3SoiFd`, `Bsim3SoiDd`, `Bsim3SoiPd`, `Bsim4Soi`: per module the ports and one row per param (MSL name, ngspice name, type, default, unit, meaning, given-ness rule, `where`), from `models/*.md`, the ngspice-47 setup, check and parameter files and the model's reference manual; the usage rules; the list of ngspice parameters with no param and the row of design §3.9 each falls under. Apply design §3 as written. The planner of the phase decides what the rules leave open and records it in the page; the user does not review the page. **The terminal variants:** list each terminal list ngspice accepts per model (body contact, body, thermal node) and the module name for each; the planner decides and the report to the user lists them.
**Where**: `.specs/features/spice-models/interface/soi.md`
**Depends on**: T127
**Requirement**: SPM-06, SPM-11, SPM-12, SPM-42
**Executor**: Opus

**Done when**:
- [ ] every settable ngspice parameter of these devices is a row or a named exclusion
- [ ] every `unresolved` and `given-ness read` row of the map has a decided declaration
- [ ] every row has a unit, or `—` for a pure number
- [ ] a point design §3 does not decide is decided here by the phase's planner, written under "Decisions taken" with its reason, and listed in the phase's report to the user (the user is informed, not asked)

**Tests**: none (document)
**Gate**: Build
**Commit**: `docs(specs): spice-models interface page, soi`

### T129: `Soi3`

**What**: STAG: front and back gate, the body, the thermal network of up to five stages. Written from `soi3/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/soi/soi3.msl`
**Depends on**: T128
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] current and charges equal ngspice's on the grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/soi/`
**Gate**: Build; Run parity-soi; Run stdlib
**Commit**: `feat(std): spice Soi3`

### T130: `Bsim3SoiPd`

**What**: Partially depleted BSIM3SOI with body, gate current and self-heating; its terminal variants. Written from `bsim3soi_pd/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/soi/bsim3_soi_pd.msl`
**Depends on**: T129
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] current, body current and charges equal ngspice's on the grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/soi/`
**Gate**: Build; Run parity-soi; Run stdlib
**Commit**: `feat(std): spice Bsim3SoiPd`

### T131: `Bsim3SoiFd` and `Bsim3SoiDd`

**What**: The fully and dynamically depleted models. Written from `bsim3soi_fd/*`, `bsim3soi_dd/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/soi/{bsim3_soi_fd,bsim3_soi_dd}.msl`
**Depends on**: T130
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] each equals its ngspice model on the grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/soi/`
**Gate**: Build; Run parity-soi; Run stdlib
**Commit**: `feat(std): spice Bsim3SoiFd and Bsim3SoiDd`

### T132: Parity matrix: `Soi3` and the BSIM3SOI models

**What**: Fill the parity matrix of `Soi3` and the BSIM3SOI models (design §6.2): the MOS rows; floating body and body contact; the kink; self-heating; ngspice's `tests/bsim3soipd`, `bsim3soifd`, `bsim3soidd` decks. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/soi/`, `crates/piperine-parity/tests/spice_soi.rs`
**Depends on**: T131
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-soi; Run gates
**Commit**: `test(parity): spice Soi3 and the BSIM3SOI models matrix`

### T133: `Bsim4Soi`: setup, temperature, currents

**What**: BSIMSOI 4 for each `soimod`: currents, body currents, gate current. Written from `bsimsoi/{b4soiset,b4soitemp,b4soild}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/soi/bsim4_soi.msl`
**Depends on**: T132
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] currents equal ngspice's on the grid per `soimod`
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/soi/`
**Gate**: Build; Run parity-soi; Run stdlib
**Commit**: `feat(std): spice Bsim4Soi: setup, temperature, currents`

### T134: `Bsim4Soi`: charge, networks, noise, self-heating

**What**: Charges, the gate and body networks, noise, the thermal node, the terminal variants. Written from `bsimsoi/{b4soild,b4soinoi,b4soiask}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/soi/bsim4_soi.msl`
**Depends on**: T133
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] capacitances and noise equal ngspice's
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/soi/`
**Gate**: Build; Run parity-soi; Run stdlib
**Commit**: `feat(std): spice Bsim4Soi: charge, networks, noise, self-heating`

### T135: Parity matrix: `Bsim4Soi`

**What**: Fill the parity matrix of `Bsim4Soi` (design §6.2): the MOS rows; each `soimod`; body contact; self-heating; ngspice's `tests/bsimsoi` decks. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/soi/`, `crates/piperine-parity/tests/spice_soi.rs`
**Depends on**: T134
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-soi; Run gates
**Commit**: `test(parity): spice Bsim4Soi matrix`

### T136: Book and close: phase 15

**What**: Write the Book sections of this phase: "SOI and HiSIM": body effects in SOI, the body contact, self-heating, the BSIMSOI family, STAG; a floating-body kink. Each device section has the seven parts of design §7 (what it is, choosing it, placing it, a model card as an `alias`, the parameters explained by group, use cases, reading it back and limits); every listing is a run listing; the reference tables are generated from the `///` texts. Regenerate Appendix I. Update `GAPS.md` and `ROADMAP.md` for the phase. Merge the phase.
**Where**: `docs/book/content.html`, `tests/book/`, `docs/spec/appendix_i_spice.md`, `GAPS.md`, `ROADMAP.md`
**Depends on**: T135
**Requirement**: SPM-37 to SPM-41, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] every listing of the new sections is run by the `book` target and every stated value asserted
- [ ] the generated tables equal the library's `///` texts (the test fails on a difference)
- [ ] no phrase the Book's guards refuse; nothing says "as in ngspice" without saying what that is
- [ ] Appendix I is fresh
- [ ] the gates of design §6.4 are at zero for this phase's models
- [ ] the phase is merged to `main`

**Tests**: `tests/book/spice_*.rs`
**Gate**: Build; Run book; Run parity-soi; Run gates; Run spec
**Commit**: `docs(book): spice phase 15`

---

## Phase 16: HiSIM

### T137: Interface page: `Hisim2`, `HisimHv1`, `HisimHv2`

**What**: Write `interface/hisim.md` for `Hisim2`, `HisimHv1`, `HisimHv2`: per module the ports and one row per param (MSL name, ngspice name, type, default, unit, meaning, given-ness rule, `where`), from `models/*.md`, the ngspice-47 setup, check and parameter files and the model's reference manual; the usage rules; the list of ngspice parameters with no param and the row of design §3.9 each falls under. Apply design §3 as written. The planner of the phase decides what the rules leave open and records it in the page; the user does not review the page. The `co…` selectors; the substrate and thermal terminals of the high-voltage models and their module names.
**Where**: `.specs/features/spice-models/interface/hisim.md`
**Depends on**: T127
**Requirement**: SPM-06, SPM-11, SPM-12, SPM-42
**Executor**: Opus

**Done when**:
- [ ] every settable ngspice parameter of these devices is a row or a named exclusion
- [ ] every `unresolved` and `given-ness read` row of the map has a decided declaration
- [ ] every row has a unit, or `—` for a pure number
- [ ] a point design §3 does not decide is decided here by the phase's planner, written under "Decisions taken" with its reason, and listed in the phase's report to the user (the user is informed, not asked)

**Tests**: none (document)
**Gate**: Build
**Commit**: `docs(specs): spice-models interface page, hisim`

### T138: `Hisim2`: setup, temperature, currents

**What**: The surface-potential solution as ngspice computes it, the drain, substrate and gate currents, the junctions. Written from `hisim2/{hsm2set,hsm2temp,hsm2eval,hsm2ld}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/hisim/hisim2.msl`
**Depends on**: T137
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] surface potentials and currents equal ngspice's on the grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/hisim/`
**Gate**: Build; Run parity-hisim; Run stdlib
**Commit**: `feat(std): spice Hisim2: setup, temperature, currents`

### T139: `Hisim2`: charge, NQS, noise

**What**: Charges, NQS, the noise sources including induced gate noise. Written from `hisim2/{hsm2eval,hsm2noi,hsm2ask}.c`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/hisim/hisim2.msl`
**Depends on**: T138
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] capacitances and noise equal ngspice's
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/hisim/`
**Gate**: Build; Run parity-hisim; Run stdlib
**Commit**: `feat(std): spice Hisim2: charge, NQS, noise`

### T140: Parity matrix: `Hisim2`

**What**: Fill the parity matrix of `Hisim2` (design §6.2): the MOS rows; the geometry grid; the selectors; ngspice's `tests/hisim` decks. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/hisim/`, `crates/piperine-parity/tests/spice_hisim.rs`
**Depends on**: T139
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-hisim; Run gates
**Commit**: `test(parity): spice Hisim2 matrix`

### T141: `HisimHv1`

**What**: The high-voltage model: the drift region, self-heating, the substrate terminal; its terminal variants. Written from `hisimhv1/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/hisim/hisim_hv1.msl`
**Depends on**: T140
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] currents, charges and temperature rise equal ngspice's on the grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/hisim/`
**Gate**: Build; Run parity-hisim; Run stdlib
**Commit**: `feat(std): spice HisimHv1`

### T142: `HisimHv2`

**What**: Version 2, as `HisimHv1` with its differences. Written from `hisimhv2/*`; the file's header names those files and carries the notice reference.
**Where**: `crates/piperine-language/std/spice/hisim/hisim_hv2.msl`
**Depends on**: T141
**Requirement**: SPM-08 to SPM-12, SPM-15 to SPM-26
**Executor**: Opus

**Done when**:
- [ ] equals ngspice's `hisimhv2` on the grid
- [ ] every param has its `///` (meaning, unit, ngspice name where it differs)
- [ ] the module elaborates and solves an operating point on `interp` and `rust`
- [ ] one smoke twin per polarity or variant passes

**Tests**: smoke twins in `crates/piperine-parity/tests/spice/hisim/`
**Gate**: Build; Run parity-hisim; Run stdlib
**Commit**: `feat(std): spice HisimHv2`

### T143: Parity matrix: `HisimHv1`, `HisimHv2`

**What**: Fill the parity matrix of `HisimHv1`, `HisimHv2` (design §6.2): the MOS rows at high voltage; the drift region; self-heating; ngspice's `tests/hisimhv1` and `tests/hisimhv2` decks. Each cell a `.cir`/`.msl` twin and a row of `matrix.toml`, compared under an existing `ToleranceContract`, on both backends. A cell that fails is a defect to fix in the model, or a deviation written to the ledger with the ngspice source lines and taken to the user for approval; no contract is edited.
**Where**: `crates/piperine-parity/tests/spice/hisim/`, `crates/piperine-parity/tests/spice_hisim.rs`
**Depends on**: T142
**Requirement**: SPM-27 to SPM-36
**Executor**: Sonnet

**Done when**:
- [ ] every cell of the matrix has a twin or a recorded reason
- [ ] the parameter-coverage gate lists no param of these modules
- [ ] three temperatures and one `tnom` other than 27 °C are covered
- [ ] both backends pass
- [ ] `contract.rs` is unchanged

**Tests**: the family's parity target
**Gate**: Build; Run parity-hisim; Run gates
**Commit**: `test(parity): spice HisimHv1, HisimHv2 matrix`

### T144: Book and close: phase 16

**What**: Write the Book sections of this phase: "SOI and HiSIM": HiSIM2 and HiSIM_HV; a high-voltage switch. Each device section has the seven parts of design §7 (what it is, choosing it, placing it, a model card as an `alias`, the parameters explained by group, use cases, reading it back and limits); every listing is a run listing; the reference tables are generated from the `///` texts. Regenerate Appendix I. Update `GAPS.md` and `ROADMAP.md` for the phase. Merge the phase.
**Where**: `docs/book/content.html`, `tests/book/`, `docs/spec/appendix_i_spice.md`, `GAPS.md`, `ROADMAP.md`
**Depends on**: T143
**Requirement**: SPM-37 to SPM-41, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] every listing of the new sections is run by the `book` target and every stated value asserted
- [ ] the generated tables equal the library's `///` texts (the test fails on a difference)
- [ ] no phrase the Book's guards refuse; nothing says "as in ngspice" without saying what that is
- [ ] Appendix I is fresh
- [ ] the gates of design §6.4 are at zero for this phase's models
- [ ] the phase is merged to `main`

**Tests**: `tests/book/spice_*.rs`
**Gate**: Build; Run book; Run parity-hisim; Run gates; Run spec
**Commit**: `docs(book): spice phase 16`

---

## Phase 17: Close

### T145: The Book's reference and translation table

**What**: Complete the part's "Reference" chapter (every param of every model, generated; the index of ngspice names and aliases) and the netlist-to-MSL translation table of the opening chapter (every element letter, every `.model` type and level, the refused forms). Read the part once as a newcomer and fix what does not stand alone.
**Where**: `docs/book/content.html`, `tests/book/`
**Depends on**: T61, T83, T94, T117, T136, T144
**Requirement**: SPM-37 to SPM-41
**Executor**: Sonnet

**Done when**:
- [ ] every in-scope device of spec.md's inventory has a row of the translation table
- [ ] every ngspice alias is in the index
- [ ] the Book's guards pass

**Tests**: `tests/book/`
**Gate**: Build; Run book
**Commit**: `docs(book): the SPICE reference and the translation table`

### T146: The records

**What**: `GAPS.md` (what is not provided, what is refused, each deviation of the ledger), `ROADMAP.md`, `CLAUDE.md`, `README.md` (a short showcase of a circuit from `std::spice`), `.specs/STATE.md`. The remaining `tests/e2e/twins/devices.msl` models are replaced by the library and the file is removed.
**Where**: the files named
**Depends on**: T145
**Requirement**: SPM-04, SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] no test declares a device the library has
- [ ] every gate of design §6.4 is at zero
- [ ] each deviation of the ledger has the user's approval recorded

**Tests**: the gates
**Gate**: Build; Run gates
**Commit**: `docs: spice-models records`

### T147: Final verification

**What**: The full suite once (`cargo test --workspace`) with ngspice-47 installed. The Verifier (Opus, not an author of the tasks it judges) judges each check's test against its criterion; a second round of fixes is by an agent that is not the Verifier.
**Where**: `.specs/features/spice-models/verification.md`
**Depends on**: T146
**Requirement**: SPM-01 to SPM-58
**Executor**: Opus

**Done when**:
- [ ] the suite is green with no tolerance and no expected value edited
- [ ] verification.md has one row per requirement with its test and the verdict

**Tests**: the whole suite
**Gate**: Build; `cargo test --workspace`
**Commit**: `docs(specs): spice-models verification`

### T148: Merge and publish

**What**: Merge to `main` and push. Refresh the published Book and specification. Remove the worktrees. Report to the user: what shipped, the deviations, what is not provided.
**Where**: —
**Depends on**: T147
**Requirement**: SPM-43
**Executor**: Sonnet

**Done when**:
- [ ] `main` holds the feature
- [ ] the published Book has the new part
- [ ] no worktree of the feature remains

**Tests**: none
**Gate**: Build
**Commit**: `chore: merge spice-models`

---
