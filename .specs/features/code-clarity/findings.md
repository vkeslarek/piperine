# Code Clarity — Findings Inventory

**Written**: 2026-11-25, by the Fable that spec Assumption 1 names ("o restante dos achados vai ter que passar um Fable para analisar o código completo").
**Base**: `902f83ae1` (`origin/main`, "chore: merge resolved-nets-bundles"). Every count, file and line below is taken at that commit. `generics` phase E still lands on `main` after this file; design re-runs the counts on its own base before fixing numbers into guards.
**Scope**: read-only. No code changed. This file is the inventory half of the spec; design and tasks come after the user answers §9.

## How the numbers were taken

- **Rust items**: a `syn` 2 visitor over every `.rs` file of the tree (971 files), one row per file, module, struct, enum, trait, alias, const, static, field, variant, `impl`, method, free `fn`, nested `fn`, lint attribute, `macro_rules!`, `.unwrap()`/`.expect()` call and `panic!`-family call, each with file, line, length, doc-comment flag and whether it sits in test code (a file under `tests/`, or a `#[cfg(test)]` module). A function's length is signature line to closing brace. "Undocumented" means no `///` (or `//!` for a file). Trait-`impl` methods are not counted as undocumented.
- **Strings and tables** (CC-11, `P0001`): `grep` and small scripts over `crates/piperine-lang/src`, each hit read.
- **Tests**: `cargo test --workspace --exclude piperine-lang-server --no-run --timings` on a cold target directory (`CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, 12 cores), then **every** test executable run once, one after another, from its package directory, timed, with ngspice installed. Nothing was sampled. The buck targets (`test = false`) were not built or run. Other lanes were building on the same machine during part of the run, so run times are upper bounds; build times come from Cargo's own per-unit clock and the same caveat applies.
- **Citations**: every `#[test]` name searched in `.specs/features/*/*.md`, `tests/book/inventory.tsv`, `GAPS.md`, `ROADMAP.md` and the Book.
- **Unused items**: by name. An item is "never referenced" when its name occurs once in all Rust code, "test-only" when it occurs once in non-test code and at least once in test code. This under-counts (a common name hides a dead item); it never over-counts.
- `piperine-lang-server` does not build (§4.3). Its rows are shown but left out of totals unless a line says otherwise.

What the scanner does **not** see: names of locals and parameters, comment quality, and whether a `pub` item is used only inside its crate. Design should not treat those as clean.

---

## 0. Headline numbers

| | Today |
|---|---|
| Crates | 15 workspace + root shell; 14 build, 1 (`piperine-lang-server`) does not |
| Non-test Rust | 134 907 lines in 495 files (131 398 without the language server) |
| Test Rust | 138 450 lines in 476 files — **more test code than product code** |
| Free functions outside tests | **417** in 80 files (+71 in the language server), plus 11 the language requires |
| Free helper functions in tests | **1 678** (1 572 in `tests/`, 106 in unit-test modules), beside 4 496 `#[test]` functions |
| Functions over 40 / 60 / 100 lines (non-test) | 316 / 134 / 50; the longest is 518 lines |
| Types that are god objects | `Elaboration`: 716 methods, 12 219 lines, in 67 files. `Checker` (types): 552 methods, 8 913 lines, 54 files, 68 fields |
| Undocumented items (non-test, language server out) | 180 types, 939 functions and methods, 160 consts and statics, 2 506 fields and variants |
| `#[allow]` | 12 outside tests (3 `too_many_arguments`, 4 `dead_code` on test-support includes, 5 in the language server); **486 in tests, every one `dead_code`** |
| Macros | 0 `macro_rules!`, 0 proc-macro crates of ours; third-party: 19 `select!`, 1 `#[derive(Logos)]`, 1 `thiserror::Error`, 370 serde derives |
| `unwrap`/`expect` outside tests | 121 (+7 language server); 28 of them guard one `Option` that every analysis assumes is `Some` |
| Language special cases (CC-11) | 65 names in a hidden table, 75 method rows, 16 core signatures, 5 intrinsics typed by hand, 2 `!` forms, 220 string-keyed sites in 60 files of the front end |
| `P0001` refusal sites | 501 in 82 files (`types` 99, `elab` 402), through 3 parallel channels |
| Test targets | 373 integration targets + 16 unit-test executables = **389 link steps** (buck targets, `test = false`, not counted) |
| Tests | 4 496 `#[test]` functions in the source; 4 330 run by default (4 025 in targets, 305 in `src/`) |
| Build of all test targets, cold | 3 min 40 s wall on 12 cores; **1 753 CPU-seconds** in workspace units, 1 561 of them in test executables; 12 GB of target directory |
| Run of all test targets | **36 min 42 s** serial, with ngspice; 26 % of it in 5 executables, 72 % in the packages that call `rustc`; the run leaves 60 cache directories (2.5 GB) in `TMPDIR` |
| Tests cited by a proof line, task or inventory row | 4 022 of 4 351 distinct names (92 %); 3 349 proof rows in 44 features |

---

## 1. The rules, crate by crate (CC-01 to CC-08, CC-10)

### 1.1 One table

| Crate | Lines (`src/`) | Free fn (src) | Free helper fn (tests) | Fns > 40 lines | > 60 | > 100 | Undocumented types | Undocumented fns and methods | Undocumented fields and variants | `unwrap`/`expect` | `panic!` family | `#[allow]` src | `#[allow]` tests |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `(root)` | 8 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 26 |
| `piperine-abi` | 3583 | 32 | 66 | 9 | 3 | 0 | 0 | 35 | 177 | 3 | 5 | 0 | 0 |
| `piperine-analog-solver` | 5987 | 31 | 98 | 11 | 4 | 2 | 21 | 56 | 140 | 23 | 0 | 0 | 35 |
| `piperine-api` | 8370 | 6 | 142 | 9 | 6 | 2 | 3 | 20 | 165 | 0 | 0 | 0 | 0 |
| `piperine-cli` | 2338 | 0 | 100 | 3 | 0 | 0 | 1 | 15 | 36 | 0 | 0 | 0 | 1 |
| `piperine-digital-solver` | 4205 | 27 | 119 | 14 | 7 | 4 | 3 | 17 | 128 | 12 | 4 | 1 | 9 |
| `piperine-interp` | 2014 | 0 | 12 | 8 | 3 | 0 | 0 | 12 | 33 | 1 | 1 | 1 | 18 |
| `piperine-ir` | 10662 | 0 | 1 | 30 | 18 | 11 | 32 | 139 | 393 | 10 | 12 | 0 | 22 |
| `piperine-lang` | 57589 | 254 | 415 | 156 | 59 | 19 | 89 | 441 | 972 | 11 | 41 | 2 | 51 |
| `piperine-lang-server` (does not build) | 3509 | 71 | 19 | 21 | 11 | 4 | 4 | 39 | 25 | 7 | 0 | 5 | 1 |
| `piperine-numbers` | 4038 | 0 | 0 | 3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `piperine-parity` | 2700 | 0 | 136 | 0 | 0 | 0 | 0 | 21 | 61 | 14 | 60 | 0 | 153 |
| `piperine-plan` | 3414 | 0 | 18 | 8 | 3 | 1 | 0 | 3 | 36 | 0 | 9 | 0 | 8 |
| `piperine-project` | 1068 | 20 | 27 | 5 | 2 | 0 | 4 | 8 | 33 | 0 | 0 | 0 | 4 |
| `piperine-solver` | 11792 | 47 | 380 | 29 | 13 | 5 | 26 | 118 | 274 | 47 | 6 | 0 | 102 |
| `piperine-transpile` | 13630 | 0 | 145 | 31 | 16 | 6 | 1 | 54 | 58 | 0 | 5 | 3 | 56 |
| **total** | **134907** | **488** | **1678** | **337** | **145** | **54** | **184** | **978** | **2531** | **128** | **143** | **12** | **486** |

Columns: the totals include the language server, the text below leaves it out. *Free fn (src)* leaves out `main` and `extern "C"` functions; *free helper fn (tests)* leaves out `#[test]` functions; *fns* counts free functions and methods. `(root)` is the re-export shell (`src/lib.rs`, 8 lines) and its `tests/`.

Three crates are already clean on most columns and are the model for the rest: **`piperine-numbers`** (0 free functions, 0 undocumented items, `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` at `src/lib.rs:9-10`, no function over 44 lines), **`piperine-plan`** (0 free functions, 3 undocumented methods; its one 292-line constructor aside) and **`piperine-cli`** (0 free functions, nothing over 60 lines).

### 1.2 Free functions (CC-04)

**417 free functions in 80 non-test files.** Per crate: lang 254, solver 47 (20 of them in a `#[cfg(test)]` file, see below), abi 32, analog-solver 31, digital-solver 27, project 20, api 6; ir, plan, transpile, interp, numbers, parity and cli have none. The full list by file is Appendix A.

Where they concentrate, and the owner each group lacks:

| # | Where | Count | What they are | Proposed owner |
|---|---|---|---|---|
| F1 | `lang/src/msl/parser/{entry,base,check,expr,msl,ty,item,pat}.rs` | 124 | `chumsky` combinator builders: each returns a parser value (`expr::build` is one 430-line function, `expr.rs:148`). The grammar has no type at all | A `Grammar` owner per production family (`ExpressionGrammar`, `ItemGrammar`, `PatternGrammar`, `TypeGrammar`, `ModuleGrammar`) whose methods are the productions; `ParseCheck` for `check.rs`. See question Q4 |
| F2 | `lang/src/msl/lexer.rs` | 27 | lexer callbacks and number classification (`classify_number`, 96 lines, `:704`) | `Lexer`, `NumberLiteral` |
| F3 | `lang/src/pom/resolve/classify.rs` | 32 | the classification pass (`run`, `port_kind`, `path_res`, …) | `Classifier` (the pass already threads one `Ctx`) |
| F4 | the rest of `lang/src/pom/**` (56) and `types/{catalogue,record,continuity}.rs`, `msl/diag.rs` (6) | 62 | pass entry points all called `run` (`resolve/{mod,classify,members,shape,leaves}.rs`), walkers (`pom/body.rs:269-345`, `reflect/body.rs:621-660`), lookups (`pom/builtin.rs:148-190`) | one type per pass with `run` as its method; `BodyWalk`; `Builtins` |
| F5 | `lang/src/msl/print.rs` | 9 | `print`, `enc`, `dec`, `doc`, `indent`, `braces`, `binop`, `vis` | `Printer` (exists, 50 methods) |
| F6 | `digital-solver/src/run.rs` | 15 | the whole execution of functions and reactions: `call_reaction` (148 lines, `:598`), `run_reactions` (123 lines, **10 parameters**, `:765`, under `#[allow(clippy::too_many_arguments)]` at `:764`), `relax` (9 parameters), `write_element` (9), `write_net` (8). The file declares 13 types and none of them owns the work | `ReactionRun` holding the engine parts those parameter lists pass around |
| F7 | `digital-solver/src/plan.rs` (11), `dag.rs` (1) | 12 | `analyze` (102 lines), `reachability`, `crossing_reach`, `check_bindings` | `DigitalPlan` / `Reachability` |
| F8 | `analog-solver/src/device/*.rs` (28) and 3 single functions (`stamp_validation.rs`, `crossing.rs`, `math/integration.rs`) | 31 | the stamping of each capability as loose functions over `&mut Assembly` state: `bind_instance` (260 lines, `plan.rs:283`), `stamp_ddt`, `stamp_flow`, `stamp_delay`, `advance`, `accept_watches` | methods of `Assembly`'s capability parts (`DdtStamp`, `FlowStamp`, `LimitState`, `Watches`), `AnalogPlan::bind_instance` |
| F9 | `solver/src/core/bind.rs` | 5 | `bind` is **371 lines** (`:557`) in a file that already has a unit struct `Bind` (`:541`) | `Bind::run` split by pin role (PORT / PARAM / GLOBAL / PRIVATE, the four the module doc names) |
| F10 | `solver/src/analyses/*.rs` (17), `solver/src/{core,protocol}/*.rs` (5 beside `bind.rs`) | 22 | numeric helpers (`solve_dense`, `dominant_monodromy_magnitude` in `pss.rs:392-424`, `restamp_jacobian` in `adjoint.rs:224`, `crossing_estimate` in `transient.rs:1085`), error builders, `is_gnd_name` | the solver type of the file |
| F11 | `abi/src/{thunk,validate,caller,raw,verify,layout}.rs` | 32 | the thunks' bodies, the validator (`validate`, `check_pin`, `check_function`, `cell_bytes` 97 lines) | `Validator` (holds the `Rule` list it reports), `Thunk<D>` |
| F12 | `project/src/*.rs` | 20 | `resolve`, `write_lockfile`, `parse_version`, `sync_and_checkout`, `find_project_root`, `load_project` | `Resolver`, `Lockfile`, `Version`, `GitCheckout`, `Project` |
| F13 | `api/src/request.rs` | 6 | TOML field readers (`span`, `number`, `i64_of`, `is_identifier`) | `Keys` (exists) |
| F14 | `solver/src/analyses/test_devices.rs` | 20 | test doubles, `#[cfg(test)]` at `analyses/mod.rs:24`; counted here because the file is under `src/` | move to test support with the other devices (§3.5) |

**Exceptions the language requires** (the list CC-04 asks for; 13 functions today, and the test functions):

| Function | Why it cannot move |
|---|---|
| `cli/src/main.rs:10` `main` | the binary's entry point |
| `abi/src/thunk.rs:156,162,229,235,258,264,316,322` `create`, `create_with`, `destroy`, `destroy_with`, `eval`, `eval_with`, `react`, `react_with` | `extern "C"` functions whose addresses fill the ABI's function table |
| `abi/src/caller.rs:209,215` `trampoline`, `element_trampoline` | `extern "C"` callbacks |
| the exported `piperine_abi_1` symbol of generated code | the ABI's one exported symbol (printed by `piperine-transpile` into the generated source, not a Rust item of the tree) |
| `abi/src/thunk.rs` `module`, `library` | generated code spells `piperine_abi::thunk::{library, module}` and `module::<T>(..)`; the five transpile goldens pin them, and design.md "What may move" lets a golden move only in T132/T137. Recorded as exceptions by Phase 11 fix round 1 (P11-4). Owner: user review at T155 / device-abi-simplification |
| every `#[test] fn` (4 496) | the test harness needs a free function |

Nothing else needs to stay free. `const fn row(..)` table builders (`pom/builtin.rs:36`, `types/catalogue.rs:58-62`) become associated `const fn`s of the row type.

**Nested functions** (`fn` inside `fn`): 30, of which 5 outside tests (3 in lang, 1 in api, 1 in the language server); same rule, same fix.

**Tests**: CLAUDE.md rule 3 says "tests included". The root `tests/` already follows it (483 tests, **0** free helpers: helpers are methods of fixture types). The crates do not: 1 572 free helpers in `tests/` directories, led by `piperine-lang` 415, `piperine-solver` 371, `piperine-parity` 131, `piperine-digital-solver` 115, `piperine-transpile` 112, `piperine-api` 108. See §3.5 for the ones that are also duplicated.

### 1.3 Names (CC-05)

The scanner split every declared name (types, functions, methods, fields, variants, modules, consts) into words and checked them against a dictionary: **260 distinct non-word parts, 1 877 occurrences** in declared names. They fall into three groups.

**(a) Not abbreviations — they mirror a spelling the reader already knows.** Proposed as the glossary of CC-05:

| Group | Terms |
|---|---|
| Project and domain | `msl`, `pom`, `ir`, `abi`, `cli`, `std`, `id`/`ids`, `param`/`params` (the MSL word), `repr`, `ngspice`, `rustc`, `netlist`, `nodeset`, `simparam`, `gnd`, `csv`, `json`, `svg`, `git`, `dag` |
| Analyses and numerics | `ac`, `dc`, `op`, `tran`, `pss`, `pz`, `sp`, `tf`, `lte`, `mna`, `bdf` (`TrBdf2`), `gmin`, `abstol`, `reltol`, `vntol`, `trtol`, `ttol`, `etol`, `uic`, `psd`, `tstab`, `adjoint`, `phasor`, `ziv` (Ziv's rounding) |
| MSL spellings carried into Rust | `ddt`, `ddx`, `idt`, `idtmod`, `ac_stim`, `prev`, `inflow`, and the Appendix H method names (`sqrt`, `atan2`, `powf`, `powi`, `signum`, `fract`, `trunc`, `hypot`, `rem_euclid`, …) |
| Words the Rust language fixes | `fmt`, `eq`, `ne`, `cmp`, `partial_cmp`, `deref`, `iter`, `len`, `shl`, `shr`, `bitand`, `bitor`, `bitxor`, `fn`, `impl`, `struct`, `enum`, `const`, `mut`, `bool`, `str` |

**(b) Abbreviations to spell out.** Declared names only (uses are several times more):

| Abbreviation | Declared names | Examples | Spell as |
|---|---|---|---|
| `decl` / `decls` | 115 | `DeclId`, `DeclKind`, `decl_data`, `ArrayDecl` (`solver/src/core/bind.rs:45`) | `declaration` |
| `expr` / `exprs` | 111 | `Expr` ×3 (`ir/src/model.rs:608`, `lang/src/msl/ast.rs:239`, `lang/src/pom/reflect/body.rs:83`), `ExprKind`, `lower_expr_unchecked` | `expression` |
| `stmt` / `stmts` | 66 | `Stmt` ×3, `walk_stmt` | `statement` |
| `ty` | 61 | `Ty` (`lang/src/types/ty.rs:5`), 34 fields named `ty` | `type_`/`ValueType`/`CheckedType` by meaning |
| `pat` / `pats` | 39 | `Pat`, `PatKind`, `PatNode` | `pattern` |
| `init` / `inits` | 42 | `init_bytes`, `local_inits` | `initial`, `initializer` |
| `arg` / `args` | 30 | `Arg` ×3, `ArgRow`, `block_args` | `argument` |
| `dt`, `h` | 38 | `dt_min`, `prev_h`, `dt_min_floor_hits` (`solver/src/core/result.rs:37-42,250`) | `time_step` |
| `stim` / `stims` | 26 | `Stim`, `StimValue`, `StimIm` (`plan/src/plan.rs:75,122,215`) | `stimulus` |
| `eval` | 25 | `Eval`, `EvalBuffers`, `eval_on` | Q2: it is the ABI contract's own word |
| `spec` / `specs` | 22 | `PinSpec`, `TermSpec`, `FunctionSpec` (`abi/src/shape.rs`), `DeviceSpec` | `…Description`, or keep (Q2) |
| `dir` | 18 | **two meanings**: `Dir` is a port *direction* (`lang/src/msl/ast.rs:519`), `dir` is a *directory* (`project/src/resolver.rs:36`, `ScratchDir`) | `direction`, `directory` |
| `ident` | 16 | `ident`, `carrier_ident` | `identifier` |
| `attr` / `attrs` | 16 | `Attr` ×2, `attr_value` | `attribute` |
| `lit` | 14 | `Lit` ×2, `LitKind` | `literal` |
| `res` | 12 | **two meanings**: field `res` is a `Resolution` (`lang/src/pom/body.rs:36,120`), enum `Res` is a result type (`lang/src/types/catalogue.rs:29`) | `resolution`, `ResultType` |
| `gen` | 10 | `Gen`, `GenFor`, `GenElseNode` | `generation` |
| `ptr`, `buf`, `idx`, `vec`, `tol`, `rel`, `src`, `jac`, `coeffs`, `prec`, `loc`, `inst`, `im`, `wrt`, `td`, `dp` | 53 | `prototype_ptr`, `ShapeBuf`, `idx()`, `step_to_vec`, `TOL_REAL`, `DP_REL`, `src_scale`, `jac`, `bdf2_coeffs`, `Prec`, `Loc`, `Inst`, `StimIm`, `wrt`, `td`, `dp_rel` | the full word |
| `deps`, `env`, `ret`, `ctx`/`Ctx` ×3, `cx` (`HostCx`), `krate`, `recv`/`Recv`, `kw`/`Kw`, `punct`/`Punct`, `lex`, `diag`, `sig` (`CoreSig`), `obs` (`Obs`), `mk`, `tok`, `enc`, `dec`, `vis`, `kid` (`Kid`) | 62 | `Deps`, `Env`, `ret`, `krate` (7 fields), `Kw` (59 variants) | `Dependencies`, `Environment`, `result`, `Context`, `crate_name`, `Receiver`, `Keyword`, `Punctuation`, … |
| one-letter fields | 30 | `k` ×4, `x` ×4, `h` ×4, `d` ×3, `t`, `q`, `g`, `c`, `y`, `s`, `w`; test doubles `R`, `C`, `L` (`solver/src/analyses/test_devices.rs:55,101,283`) | the quantity's name |

About **760 declared names** change; with their uses this is the largest mechanical edit of the feature. The naming guard of CC-05 can be this same scan with the glossary of (a) as its allow-list.

**(c) Module and crate names.** `elab` (→ `elaboration`), `rt` (→ `runtime`), `deps`, `diag`, `ty`, `pat`, `iv`, `num`, `sens` beside `sensitivity` (two modules, `solver/src/analyses/sens.rs` is `.sens` by finite difference and `sensitivity.rs` is the `∂F/∂p` collector: the names do not tell them apart), `core` (in `piperine-solver`: bind, builder, circuit, net, result, introspect — "core" names no concern), `base` and `entry` (parser), `alone`, `both`, `mixed` (test support). Crate `piperine-interp` (→ `piperine-interpreter`, Q3).

**Type names that say too little.** 125 type names are defined more than once across non-test code (302 definitions). The worst: `Step` ×8, `Source` ×6, `Walk` ×5, `Held` ×5, `Site` ×4, `Shape` ×4, `Flow` ×4, `Value` ×4, `Frame` ×4, `Bound` ×4, `Outcome` ×4, `Key` ×4, `Ctx` ×3, `Scope` ×3, `Row` ×2, and `AnalysisKind` ×3 of which **two are in the same crate** (`solver/src/protocol/sides.rs:188`, `solver/src/analyses/context.rs:25`; the third is `ir/src/model.rs:1112`). A name like `Step` or `Walk` only means something next to its file; each needs its subject (`UnrollStep`, `ManifestWalk`, `DependencyWalk`).

**Stale vocabulary in comments and messages** (CC-10 AC 4, found by the same scan): 29 mentions of "the M1 subset" in non-test code, two of them in the text every user sees for `P0001` (`lang/src/types/check.rs:420`, `lang/src/elab/elaborator.rs:294`: "… is outside the M1 subset"); at least 605 feature-requirement tags (`DIG-09`, `CV-24`, `GN-17`, `SAC-13`, …) and 61 task ids (`T47`) in doc comments, where CC-10 AC 3 asks for the spec rule (`§n`) or decision (`MD-n`); `Cargo.toml` of the language server still says "PHDL".

### 1.4 Types with more than one purpose (CC-05 AC 2, CC-06)

| # | Type | Size | Why it is more than one thing | Split along |
|---|---|---|---|---|
| G1 | `Elaboration` (`lang/src/elab/elaborator.rs:52`) | 716 methods, 12 219 lines, `impl` blocks in **67 files**, 37 fields; its companion `Scope` has 42 fields (`elab/lower.rs:26`) | every lowering of every form is a method of the one object: expressions, patterns, carriers, drives, events, analog operators, generation, generics, reach, records, structural params. The 72 files of `elab/` are one type cut into pages | one owner per IR product, each holding only what it reads: `ValueLowering` (expressions, patterns, aggregates, iterators), `AnalogLowering` (contributions, `ddt`/`ddx`/`delay`/`limit`/`noise` sites), `DigitalLowering` (carriers, drives, events, reactions, records), `Structure` (placements, generation, templates, reach, structural keys), `FunctionInstances`; `Elaboration` keeps the pass order only (`elaborator.rs` already documents it) |
| G2 | `Checker` (`lang/src/types/check.rs:54`) | 552 methods, 8 913 lines, 54 files, **68 fields** | same shape: one object types every form | `ExpressionTyping`, `PatternTyping`, `ModuleTyping`, `CarrierTyping`, `GenericsSolver` (inference, substitution, obligations, bounds), `TraitTable` (exists as data: `types/traits.rs`), over one shared `Types` result table |
| G3 | `ConstEval` (`lang/src/elab/consts.rs:184`) | 169 methods, 2 287 lines, 8 files | a second evaluator of the expression language beside the lowering (130 of the 501 `P0001` sites are its own, §6) | keep one type; split by value family as its files already do; share the refusal owner of §6 |
| G4 | `ir::Checker` (`ir/src/check.rs:190`) | 96 methods, 2 817 lines; `expr` is **518 lines** (`:1728`), `ids` 200, `code` 185, `stmt` 129 | reference check, type check and class check in one walk | `IdCheck` (dangling ids), `TypeCheck` (per `Expr` variant, one method each), `ClassCheck`; `check/` already holds five part files |
| G5 | `ValuePrinter` (`transpile/src/print/value.rs:72`) | 116 methods, 1 536 lines, 6 files, 22 fields | prints expressions, statements, environment reads and record writes | `ExpressionPrinter`, `StatementPrinter`, `RecordPrinter` |
| G6 | `ir::Printer` (`ir/src/print.rs:136`) | 51 methods, 1 382 lines; `body` 241 lines, `expr` 229, `arenas` 130 | one printer for bodies, expressions and arenas | per-section printers; these are the "printers' tables" CC-06 lets design list as length exceptions, but only the `match` tables, not `body` |
| G7 | `TransientSolver` (`solver/src/analyses/transient.rs:571`) | 46 methods, 1 027 lines, in a 2 012-line file with 17 types | options, UIC clamps, the nonlinear system, the step controller, scheduled sets and the device attempt in one file; `solve` is 210 lines (`:1117`) | `transient/{options,system,step_control,scheduled_sets,attempt}.rs` |
| G8 | `Session` (`api/src/session.rs:57`) | 58 methods, 852 lines, 20 fields; `analysis` 179 lines (`:704`) | open, run each analysis, params, probes, metadata | one runner per analysis family behind `Session::analysis` |
| G9 | `Keys` (`api/src/request.rs:219`) | 38 fields; `spec` 185 lines (`:566`) | reads every key of a run table in one method | one reader per key group (grid, options, sweep, noise) |
| G10 | `BodyPlan::new` (`plan/src/plan.rs:553`) | one 292-line constructor | builds terms, pins, sources and function plans inline | a `BodyPlanBuilder` with one method per table |
| G11 | `Design` (`lang/src/pom/design.rs:67`) | 121 methods in 7 files | storage, queries and reflection entry points | acceptable if reflection stays in `reflect/`; check after G1/G2 |

### 1.5 God methods (CC-06) and the proposed limit

Distribution of the 6 897 non-test functions and methods (language server left out), length from signature to closing brace:

| p50 | p75 | p90 | p95 | p99 | max |
|---|---|---|---|---|---|
| 6 | 13 | 26 | 40 | 86 | 518 |

| Limit | Functions over it | Share of functions | Lines they hold | Share of all function lines |
|---|---|---|---|---|
| 30 | 506 | 7.3 % | 30 452 | 37.7 % |
| **40** | **316** | **4.6 %** | 23 777 | 29.4 % |
| 50 | 208 | 3.0 % | 18 879 | 23.4 % |
| 60 | 134 | 1.9 % | 14 774 | 18.3 % |
| 80 | 74 | 1.1 % | 10 562 | 13.1 % |
| 100 | 50 | 0.7 % | 8 404 | 10.4 % |

**Proposal: 40 lines.** It is the 95th percentile, so 95 % of the code already meets it, and three crates meet it almost whole (`piperine-numbers` max 44, `piperine-parity` max 32, `piperine-cli` 3 over). The work is 316 functions; the 134 over 60 lines (Appendix B) hold most of the lines and are where the readability is lost. Exceptions to list by name in design: a function whose body is one exhaustive `match` with one-line arms over an IR or syntax enum (the "printers' tables" of the spec), for which the guard counts the longest arm instead. A second number worth guarding: **parameters**. 262 functions take 6 or more parameters and 24 take 8 or more (`digital-solver/src/run.rs:765` takes 10); propose a limit of 5 beyond `self`, the rest a parameter object (CLAUDE.md rule 5). 66 closures of 15 lines or more hide inside otherwise short functions; the guard should count them as functions.

The ten longest: `ir::Checker::expr` 518 (`ir/src/check.rs:1728`), `msl::parser::expr::build` 430 (`lang/src/msl/parser/expr.rs:148`), `solver::core::bind::bind` 371 (`solver/src/core/bind.rs:557`), `BodyPlan::new` 292 (`plan/src/plan.rs:553`), `analog-solver::device::plan::bind_instance` 260 (`analog-solver/src/device/plan.rs:283`), `ir::Printer::body` 241 (`ir/src/print.rs:388`), `ir::Printer::expr` 229 (`:928`), `Elaboration::lower_expr_unchecked` 213 (`lang/src/elab/lower.rs:302`), `ir::deps::Walk::expr` 212 (`ir/src/deps.rs:942`), `TransientSolver::solve` 210 (`solver/src/analyses/transient.rs:1117`).

### 1.6 Abstractions that do not earn their place, and logic without an owner (CC-07)

| # | Finding | Evidence | Proposal |
|---|---|---|---|
| A1 | `Device::react` is required of every device, analog-only ones included | `abi/src/device.rs:24`; the 3-line no-op `Resistor::react` body exists **102 times** in tests (173 `impl Device` blocks in test code) | a default method (the ABI trait can carry `fn react(..) { Ok(Flow::Continue) }` without changing the `#[repr(C)]` contract), or an `AnalogDevice` trait. Q8: this touches `piperine-abi`'s public trait |
| A2 | `CircuitInstance.abi: Option<Bound>` is `None` only in the empty circuit `CircuitInstance::new` makes (`solver/src/core/circuit.rs:377`); every analysis assumes `Some` | `solver/src/core/circuit.rs:39`; **28** `.expect("a circuit of devices…")` (`analyses/{transient,dc,noise,ac,tf,pz,sp,sens,adjoint}.rs`, `core/circuit.rs:253-330`) | two types (an unbound and a bound circuit), or the field as `Bound`; the 28 `expect`s disappear |
| A3 | 13 traits with one implementor | `ElementSource` (`abi/src/caller.rs:64`), `Factory` (`abi/src/device.rs:27`), `NetRead` (`solver/src/protocol/step.rs:18`), `Limbs` (`numbers/src/limbs.rs:9`), `In` (`lang/src/msl/parser/base.rs:166`, a trait alias), `StepperStrategy`, `HomotopyDriver` (`analog-solver/src/convergence.rs:178,309`), `LinearSystem`, `SymbolicMatrix`, `SymbolicLinearSystem` (`analog-solver/src/math/linear.rs:23-49`), `FaerToNdarray` (`math/faer.rs:211`), `InitialValueApplyExt` (`math/iv.rs:14`), `DiscreteSide` (1 real + 1 scripted stub in tests) | per trait: remove, or keep with the reason written (`DiscreteSide`/`AnalogSide` are the protocol's seam and a test proves it, keep; the linear-algebra layer over `faer` has one backend and three traits, remove) |
| A4 | The same rule written in two crates | a `sim` record with `failed` is the fatal one: `api/src/verdict.rs:79` **and** `cli/src/commands/run.rs:79` (CLAUDE.md: "every rule of what a run prints or judges lives" in the api). An out-of-range float spelled `inf`/`infinity`: `lang/src/elab/override_text.rs:60` **and** `api/src/param_text.rs:119`. `LogicPattern`: `lang/src/types/logic_pattern.rs:20` **and** `api/src/logic_pattern.rs:15`. `Backend`: `api/src/backend.rs:9` **and** `parity/src/backend.rs:7`. Ground names `"gnd" \| "GND" \| "vss" \| "VSS"`: `solver/src/core/builder.rs:54` **and** `api/src/noise.rs:69` | one owner each; the cli reads `Verdict` |
| A5 | Three implementations of the reserved streams' meaning, joined by string literals | `"sim"`/`"checks"` in `lang/src/types/record.rs:44-55`, `transpile/src/print/value.rs:1299`, `api/src/{record.rs:193,241,verdict.rs:79-82}`, `cli/src/commands/run.rs:79`, `parity/src/scenario/run.rs:27` | goes away with CC-11 S10 (the record types declared in `std/io.msl`); until then one `ReservedStream` type in `piperine-ir` |
| A6 | Arbitrary-width integers twice | `lang/src/elab/consts/integers.rs` (`DynamicInteger`, 50 methods, 392 lines) beside `numbers/src/wide.rs` (`WideUInt`/`WideInt`, const-generic width) | `DynamicInteger` is numeric code with no elaboration in it; it belongs in `piperine-numbers` (CC-03) |
| A7 | Two refusal helpers with the same body | `Checker::unsupported` (`lang/src/types/check.rs:419`) and `Elaboration::unsupported` (`lang/src/elab/elaborator.rs:293`) are byte-identical | §6 |
| A8 | Debug prints in the solver's hot path | `eprintln!` behind trace flags: `analog-solver/src/convergence.rs:543,569,625`, `solver/src/analyses/transient.rs:1756`; and a unit test that greps its own source for `eprintln!` (`solver/src/analyses/tf.rs:455-500`) | a `Trace` sink owned by `Policy`; the grep test goes (§3.4) |
| A9 | Code in `lib.rs` | `interp/src/lib.rs` is 238 lines (`Interpreter`, `Rejected`), `transpile/src/lib.rs` 152 (`Transpiler`) | `lib.rs` holds the crate doc, `mod` and `pub use` only (the rule `suite_hygiene::mod_rs_declares_only` already applies to `mod.rs`) |

### 1.7 Doc comments (CC-10)

| Kind | Undocumented / total (non-test, language server out) |
|---|---|
| structs | 99 / 957 |
| enums | 62 / 310 |
| traits | 8 / 39 |
| type aliases | 11 / 42 |
| free functions | 110 / 428 |
| inherent methods | 789 / 5 549 |
| trait methods (declarations) | 40 / 204 |
| consts and statics | 160 / 270 |
| fields | 1 866 / 3 472 |
| enum variants | 640 / 1 694 |
| files without `//!` | 1 in `src/` (`solver/src/lib.rs`, which has no doc at all), 5 in tests (`transpile/tests/…`) |

By crate the gaps are in the 1.1 table. The largest single holes: `piperine-ir` (139 methods, 20 structs, 12 enums, 393 fields and variants: the IR's own model, `ir/src/model.rs`, is the least documented file of the tree), `piperine-solver` (99 methods, 24 structs), `piperine-lang` (366 methods, 43 structs, 41 enums), `piperine-abi` (89 of 104 consts and statics: the role and flag constants of `raw.rs`), `piperine-analog-solver` (8 of 12 traits). `#![deny(missing_docs)]` holds today in one crate (`piperine-numbers`). The 30 files with most undocumented items are Appendix C.

Crate-level docs (CC-01, CC-08): every `lib.rs` but `piperine-solver`'s (0 lines of `//!`) states a purpose, and abi, analog-solver, digital-solver, api, cli and lang also give a module map. **None states the data path** as CC-08 asks (input type → stages → output type); `piperine-transpile`'s and `piperine-plan`'s come closest. `piperine-project`'s is one line; ir, numbers, plan, interp, transpile, parity and project give no module map. Two are stale: `piperine-parity`'s says "the library serves the integration targets of this crate only" while the root, the api and the cli depend on it, and `piperine-ir`'s says its node set "covers the analog subset" of the twins.

### 1.8 `#[allow]`, lint suppression, macros

- **Outside tests: 12.** `#[allow(clippy::too_many_arguments)]` ×3 (`digital-solver/src/run.rs:764`, `lang/src/elab/generation.rs:173,310`): each is the "design problem behind an allow" rule 5 names; 24 functions take 8 or more parameters, so 21 more would need it the day clippy gates the build. `#[allow(dead_code)]` ×4 on `#[cfg(test)] #[path]` includes of another crate's test support (`transpile/src/lib.rs:34,39,44`, `interp/src/lib.rs:20`). 5 in the language server.
- **In tests: 486**, all `#[allow(dead_code)]`, nearly all on a `#[path = "…"] mod` line: parity 153, solver 102, transpile 56, lang 51, analog-solver 35, root 26, ir 22, interp 18. They are the price of sharing test support by textual include (§3.5): each target compiles the whole support file and uses a part.
- **File-scope suppression: 2**, both in tests (`transpile/tests/support/compiled.rs:4`, `lang-server/tests/common/mod.rs:7`). `suite_hygiene::no_file_scope_lint_suppression` is green with the first in the tree, so its walk does not reach `tests/support/`.
- **Macros**: no `macro_rules!` and no proc-macro of ours anywhere (17 `thread_local!` in tests). Third-party macro use in product code: `select!` of `chumsky` ×19 (parser), `#[derive(Logos)]` ×1 (`lang/src/msl/lexer.rs:140`), `#[derive(thiserror::Error)]` ×1 (`analog-solver/src/error.rs`), `Serialize`/`Deserialize` on 185 types (lang 98, ir 87). The serde derives have **one** non-test consumer path (`Design::from_json`, `lang/src/pom/design.rs:177`, itself referenced only from tests) plus one unit test that round-trips the IR to flip a flag (`api/src/session.rs:1055`). Q5 asks whether rule 7 covers third-party macros and whether the serialization surface is wanted.

### 1.9 `unwrap` / `expect` / `panic!` on paths user input reaches (rule 9)

121 `unwrap`/`expect` method calls outside tests (Appendix D lists every line): solver 47, analog-solver 23, parity 14, digital-solver 12, lang 11, ir 10, abi 3, interp 1; api, cli, plan, transpile, numbers and project have none. The 10 of `piperine-ir` are calls of a method of its own named `expect`, so the real count is 111.

| Class | Sites | Verdict |
|---|---|---|
| `Option<Bound>` never `None` (A2) | 28 in `solver/src/analyses/*`, `core/circuit.rs` | a design is user input and reaches every one; remove with the type change |
| NaN or an empty orbit from a user's circuit | `solver/src/analyses/pz.rs:250` (`partial_cmp(..).unwrap()` twice: a NaN root panics), `pss.rs:234,236,278,369` (`.last().unwrap()` on a shoot's steps) | **real user-input panics**; return the solver's `Error` |
| Newton state and netlist indices | `analog-solver/src/math/newton_raphson.rs:348-549` (6), `netlist.rs:254,256`, `tolerances.rs:151`, `device/{plan,assembly,flow,watch}.rs` (14) | invariants of a validated shape; each needs either the type that makes it impossible or the `Error` |
| Parser and lexer bookkeeping | `lang/src/msl/parser/entry.rs:388,557-560`, `lexer.rs:310,619` | source text reaches them; the invariants look sound, but rule 9 has no "looks sound" clause: make them total |
| IR check, POM printer | `ir/src/check.rs` (10, all `self.expect(..)`: a method named `expect`, **not** `Option::expect`; false positives), `lang/src/pom/{to_ast,check,reflect/crates}.rs` (5) | rename `Checker::expect` (it reads as a panic); the POM ones are invariants |
| `panic!("…; check() refuses it")` | 8 in `plan/src/digital.rs:396-745`, 1 in `plan/src/plan.rs:530`, 2 in `transpile/src/print/{contribution.rs:80,value.rs:401}` | the plan trusts `Ir::check`; an `Ir` built by a host through `IrBuilder` reaches them. `ir/src/build.rs:226-601` panics on an out-of-range id (7) |
| `piperine-parity` | 14 `expect`, 60 `panic!` | a test harness; out of rule 9's scope, in rule 3's |

### 1.10 Module and file structure (CC-01, CC-02, CC-03, CC-08)

| # | Finding | Evidence | Proposal |
|---|---|---|---|
| M1 | Two flat directories hold half the front end | `lang/src/elab/`: 72 files and `consts/` (10 more); `lang/src/types/`: 67 files. Related files sit apart alphabetically (`reach.rs`, `reach_through_templates.rs`, `reach_values.rs`; `drives.rs`, `drive_loops.rs`, `drive_overlaps.rs`; `carriers.rs`, `carrier_arrays.rs`, `elements.rs`, `element_commits.rs`, `element_coverage.rs`; `class.rs` beside `classes.rs`; `cover.rs` beside `coverage.rs`) | sub-modules that follow the split of G1/G2: `elab/{value,analog,digital,structure,generics,consts}/`, `types/{value,pattern,module,carrier,generics,traits}/` |
| M2 | Module names that name no concern | no `utils`/`helpers`/`misc`/`common` exists in any `src/` (clean). But: `solver/src/core/`, parser `base.rs` and `entry.rs`, `lang/src/types/table.rs` (the result table **and** the `Method`, `Builtin`, `Callee`, `Continuity` enums), `lang/src/types/model.rs` (the whole-model check), `ir/src/handle.rs`, `ir/src/written.rs`; in tests: `solver/tests/helpers/`, `lang-server/tests/common/`, support files `alone.rs`, `both.rs`, `mixed.rs` | rename by content (`solver/src/circuit/`, `types/result_table.rs`, …) |
| M3 | Two file layouts for a module with children | `x/mod.rs` in 14 directories, `x.rs` + `x/` in 7 (`cli/src/{commands,render}`, `ir/src/{check,deps}`, `lang/src/elab/consts`, `transpile/src/{print,rt}`) | pick one (Q6) |
| M4 | Three oversized `mod.rs` registered as pending for this feature | `lang/tests/suite_hygiene.rs:808-816`: `pom/reflect/mod.rs` (512 lines), `pom/resolve/mod.rs` (167), `pom/lower/mod.rs` (160) | split; delete the three exemptions |
| M5 | A file is where the design said it should not be, recorded as a deviation | 9 `SPEC_DEVIATION` comments: `solver/src/analyses/adapters.rs:8` (belongs in `protocol/` but a guard forbids the import there), `analog-solver/src/device/assembly.rs:222,393,415,477`, `lang/src/pom/{body.rs:9,source.rs:81}`, `solver/src/analyses/transient.rs:1433,1564` | CLAUDE.md rule 1: a recorded deviation is a defect. Either the code moves or the design text is amended; the comments go |
| M6 | Test doubles under `src/` | `solver/src/analyses/test_devices.rs` (359 lines, 20 free functions, types `R`, `C`, `L`) | test support |
| M7 | Workspace guards live in the wrong crate | `solver/tests/workspace_shape.rs` (15 tests over the root manifest and `examples/`), `solver/tests/crate_boundaries.rs` (8), `lang/tests/suite_hygiene.rs` (23 tests over every crate), `cli/tests/docs.rs` (60 tests over `GAPS.md`, `STATE.md`, `ROADMAP.md`) | MD-28 puts a target in the crate it exercises; these exercise the repository. One home (the root package, or a `piperine-repository-guards` test-only crate); the CC-01/04/05/06/10 guards land there too |
| M8 | Crate purposes | all 14 building crates have one purpose and the dependency arrows of CLAUDE.md hold (`crate_boundaries`, `protocol_seam`, `numbers/tests/purity.rs` are green). Misplaced code: A4 (rules duplicated across crates), A6 (`DynamicInteger`), M6, and `piperine-parity` (test-only) is a dependency of the root's tests, the api's and the cli's, which makes it the de-facto shared test-support crate without the name | fix the four; consider naming what `piperine-parity` has become (Q7) |
| M9 | Data path not readable from the entry points | `lang`: the path is `msl::parse → pom::Design::builder → types::Types::check → elab::Elaboration` and `elaborator.rs` documents the pass order well; `api/src/session.rs` (`Session::open`, 1 087 lines) is where the path from `Ir` to a running circuit is hardest to follow (plan, transpile-or-interpret, bind, analysis in one type); `solver`: two entry layers, `analyses/` drivers and `protocol/`, joined by `analyses/adapters.rs` | each `lib.rs` states the path (CC-08 AC 1); `ARCHITECTURE.md` does not exist today and CC-08 AC 2 needs it or the Book chapter |

---

## 2. Language special cases (CC-11)

The front end resolves what the language provides in **three hidden tables and about 220 string comparisons in 60 files**, while `std/` (11 files, 1 193 lines of MSL) already declares most of the library in the language. The mechanism CC-11 asks for exists: `extern "intrinsic" fn` (§3 rule 10), used by 5 functions, and `extern STRING module` in the grammar (Appendix B, `module_decl`). It is simply not used for the core.

### 2.1 The hidden tables

| Table | Where | Rows |
|---|---|---|
| `BUILTINS` | `lang/src/pom/builtin.rs:46-112` | 65: 13 types (3 of them the families `uN`, `iN`, `fN`), 7 variants (`Some`, `None`, `Dc`, `Transient`, `Ac`, `Noise`, `Static`), 18 traits, 3 trait members, 11 functions, 6 modules, 1 event, 1 statement, 4 methods, 1 node |
| Method catalogue (Appendix H) | `lang/src/types/catalogue.rs:70-217` | 75: `f64` 32, integers 13, `bool` 2, arrays 7, ranges 2, iterators 10, `Option` 9; recorded as `types::Method` (70 variants, `types/table.rs:22`) and `ir::Method` (36, `ir/src/model.rs:996`) |
| Core signatures (§14) | `lang/src/types/catalogue.rs:299-320` (`CORE_ROWS`) | 16, with parameter kinds no MSL signature has (`Probe`, `Text`, `Limiter`) and a position class (`Allowed::{Analog, Instances, Everywhere}`) |
| Intrinsics by namespace and name | `lang/src/types/intrinsic.rs:30-40` | 5 |
| Reserved stream fields | `lang/src/types/record.rs:44-55` | 2 streams, 7 fields |

String-keyed dispatch after resolution, the "peepholes": `id.info().name` matched as text at 43 sites (e.g. `lang/src/elab/analog.rs:174-200` lowers each core primitive by `match name { "temperature" => …, "ddt" => … }`); `Resolution::Builtin` handled at 72 sites; `Ty::Option` at 86.

### 2.2 The cases

Default for each is **removal** (spec Assumption 4). "Keep" needs the user's review. *Numeric risk* is CC-11 AC 4's question: none of these changes what a run computes.

| # | Special case | What a builtin can do that a user item cannot | Evidence | Proposal | Effort |
|---|---|---|---|---|---|
| S1 | **`emit!`** | takes any number of `name = value` fields; a user `fn` has fixed positional parameters. It is also one of two `!` forms in a language with "no macros" (A.1 rule 4) | parser `lang/src/msl/parser/expr.rs:240-300`, `item.rs:489`, `msl.rs:299`; typed in `types/record.rs` | **(a)** `emit` becomes an ordinary `extern "intrinsic" fn emit<R>(stream: str, record: R)` over a plain struct: `emit("checks", Check { kind: "require", name, pass })`. The field list and its types then come from a struct the model (or `std/io.msl`) declares, so the reserved-field table (S2) disappears too. **(b)** user functions gain named variadic arguments. **(c)** keep. Recommended: (a). Cost: `emit!` is written 152 times in `.msl` corpora, 98 in the Book, 57 in the spec, 405 in Rust test strings | L |
| S2 | Reserved streams `sim` and `checks` | their field names and types are fixed by the compiler, and `finish` ends the run because the backend matches the text | `types/record.rs:44-55`; `transpile/src/print/value.rs:1299`; `api/src/verdict.rs:79-82`; `cli/src/commands/run.rs:79` | with S1(a): `pub struct Sim { finish: bool, failed: bool, msg: str }` and `pub struct Check { … }` in `std/io.msl`; the backend recognises the stream by the std declaration | with S1 |
| S3 | **`include_str!`** | the second `!` form | `parser/expr.rs:305` | `pub const extern "intrinsic" fn include_str(path: str) -> str;` beside `parse_table` (already `const extern "intrinsic"`). With S1, A.1 rules 4 and 5 become one sentence: there is no `!` form | S |
| S4 | **Method catalogue** | 75 methods on `f64`, integers, `bool`, arrays, ranges, iterators and `Option` exist with no declaration; a user cannot read their signatures in `std/`, and a receiver type "without a catalogue" is `P0001` | `types/catalogue.rs`; `Lookup::Uncatalogued` at `:153` | `impl f64 { pub extern "intrinsic" fn sqrt(self) -> f64; … }` in a new `std/numbers.msl`, `impl<const N: usize> Int<N> { … }` for the signed-only rows (`abs`, `signum`), `impl<T> Option<T>`, `impl<T, const N: usize> [T; N]`. Method lookup becomes the ordinary one; each intrinsic binds to its `ir::Method` through one table keyed by the declaration. The 14 rows typed from the receiver's payload or a closure (`map`, `filter`, `fold`, `any`, `all`, `zip`, `sum`, `then`, `and_then`, `unwrap_or_else`, …; `Lookup::Payload`) need generic methods, most with an `Fn` bound, which `generics` phase D delivered | L |
| S5 | **Types that cannot be written** | a range, an iterator and a closure have a type no model can name (H.9), so `iter()` has a signature MSL cannot express | `docs/spec/appendix_h_library.md:250`; `Ty::Iterator`, `Ty::Range` | name them (`Range<T>`, `Iterator<T>`) as std types, or keep with the reason that they exist only as sub-expressions of one statement. Needed by S4's iterator rows | M, spec |
| S6 | **`Option<T>`, `Some`, `None`** | a generic enum the model could write, held as a builtin type and two builtin variants; `?`, trailing optional arguments (§8.1.8) and optional params hang on it | `BUILTINS` rows; `Ty::Option` 86 sites in lang, `ValueType::Option` in ir (19), transpile (13) | `pub enum Option<T> { None, Some(T) }` in std, found by the compiler as a language item for `?` and optional arguments; the IR may keep its own representation. Methods through S4 | L |
| S7 | **`Analysis` and its variants; `Static`** | `Dc`, `Transient`, `Ac`, `Noise` are bare names in every scope with no enum behind them; `analysis(Static)` takes a name that is no analysis kind | `BUILTINS` rows; `elab/analog.rs:183-190`, `types/analog.rs:332`, `types/continuity.rs:140` | `pub enum Analysis { Dc, Transient, Ac, Noise }` in std and re-exported variants in the prelude; `Static` becomes a function (`is_static()`) or a fifth variant | S |
| S8 | **Traits** | `Repr`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Default`, `Debug`, `Hash`, `From`, `BitAnd`, `BitOr`, `BitXor`, `Not`, `Shl`, `Shr`, `Resolve` (16) are traits by table row; their function signatures are code (`types/operators.rs:195-210`: `takes_self = !matches!(trait_name, "From" \| "Default")`; `types/bounds.rs:266-376`; `types/generics.rs:163-172`), although the spec itself writes `pub trait Repr { … }` (`part_i_values.md:826`) and `trait From<T> { fn from(x: T) -> Self; }` (H.8 rule 8) | as left | declare them in a new `std/traits.msl` exactly as the spec prints them; the checker reads the signature from the declaration. `Repr::WIDTH`, `encode`, `decode` stop being three `Member` rows | M |
| S9 | **Arithmetic and ordering operators have no trait** | `&`, `\|`, `^`, `!`, `<<`, `>>`, `==` can be given to a struct by an `impl`; `+`, `-`, `*`, `/`, `%` and unary `-` cannot (H.10 rule 5) | `docs/spec/appendix_h_library.md` H.10 | add `Add`, `Sub`, `Mul`, `Div`, `Rem`, `Neg` to H.10 with the same `Rhs`/`Out` shape. A spec amendment and a new capability; the user decides whether it belongs here or in `spec-closure` | M, spec |
| S10 | **`Fn(P1, …, Pn)` and `Nature`** | `Fn` takes a parenthesised list of any length; no user trait can. `Nature` is implemented by a `nature` declaration, never by an `impl` | `BUILTINS`; `types/module_bounds.rs` | keep both, reason written: they are bounds on *kinds* of item (callables, natures), not traits a model implements. Review | — |
| S11 | **Core primitives of §14** | 16 signatures only the checker knows (and `potential`, `flow`, `settle` typed apart); `ddx`'s second argument is "a probe, judged by name" (`CoreParam::Probe`), `simparam`'s a literal text, `limit`'s a function; `potential`/`flow` take one or two nodes of any nature; each has a position class | `types/catalogue.rs:232-320`; `elab/analog.rs:174-200`; `types/analog.rs:71-332` | a new `std/core.msl`: `pub extern "intrinsic" fn now() -> f64;`, `pub extern "intrinsic" module ddt(…) -> f64;`, … with the parameters §14 gives each The optional trailing parameters are already expressible (§8.1.8). What is **not** expressible today and needs either a general form or a written reason: a parameter that must be a probe (`ddx`), an overload by argument count (`potential(a)` / `potential(a, b)`), and the position class (the spec has function classes in §3; an intrinsic can declare its class) | L |
| S12 | **`bound_step(dt)`** | a statement spelled as a call, peeked by the parser | `BuiltinKind::Statement`; `parser/msl.rs:298`, `parser/check.rs:378` | an `extern "intrinsic" fn bound_step(dt: f64)` of the analog class; the two parser peeks exist only to word `E0404` and go | S |
| S13 | **Carrier methods** `changed`, `prev`, `others`, `inflow` | methods by table row (`BuiltinKind::Method`), dispatched by name | `types/call.rs:326-331` | `impl<T: Repr> Bit<T> { pub extern "intrinsic" fn changed(self) -> Event; pub extern "intrinsic" fn prev(self) -> T; }` in `std/carriers.msl` (an inherent `impl` on a nature is legal). `others` (on any `W: Nature + Resolve<V>`) and `inflow` (on any node port) need a method given by a bound; if that is not expressible they stay, with the reason | M |
| S14 | **`Bit::rise` / `Bit::fall`** | declared in MSL **with bodies** (`std/values.msl:18-38`) that the compiler ignores: the checker maps the two names to `Builtin::Rise`/`Builtin::Fall` (`types/carrier.rs:259-262`), the elaborator lowers them with its own `rise_guard`/`fall_guard` (`elab/events.rs:228-242`), and a refusal is worded for the case "the checker knows only the stdlib's own `rise`" (`types/call.rs:320`) | as left | compile the body that is written (it is `self.changed() && !self.prev() && self`, all expressible), or mark it `extern "intrinsic"`. Today the source says one thing and the compiler does another | S |
| S15 | **The 5 intrinsics are recognised by `(namespace, name)` strings and typed by hand** | `parse_table<Row, const N>` has "Row: f64 or a tuple of f64" as a *comment* (`std/io.msl:37`), `parse_hex<T, …>` has "T: an integer type" as a comment: the bound is in `types/intrinsic.rs:75-90`, not in the signature | `types/intrinsic.rs:30-40,56-95`; `Logic` found by scanning every declaration for the names `logic`/`Logic` (`:123-130`) | the bounds become traits declared in std (`TableRow`, `Integer`) so the signature carries the contract; the compiler finds an intrinsic by its declaration, once, at resolution | M |
| S16 | **Two shadowing rules for what the language provides** | a core name cannot be redefined (`E0457`, `pom/builtin.rs:190`); a std name can ("a project's own function of the same name is an ordinary function", §22 rule 3) | as left | once the core is declared in std (S11), one rule: the prelude's names shadow like any import. `E0457` leaves the spec, or is kept for `gnd` only | S, spec |
| S17 | **Primitive types** `bool`, `uN`/`iN`/`fN`, `usize`, `isize`, `UInt`, `Int`, `Float`, `str`, `Event`, and `gnd` | they have no declaration | `BUILTINS` rows 0-11, 30-31 | keep: the type grammar needs leaves. Propose a documentation-only declaration in `std` so they have a place a reader finds them, and the guard of CC-11 AC 2 lists them as the closed set of primitives. Review | S |
| S18 | **Nature quantity keys** `access`, `units`, `abstol` | `access: v` *declares* a function named `v` | `elab/nature.rs:93-101`, `pom/resolve/classify.rs:51-56` | keep: it is the `nature` declaration's own grammar (counted with `nature` in Appendix B). Review | — |
| S19 | **`#[derive(…)]` over a closed set** | only the 8 traits of H.8 derive; a user trait cannot | `types/derives.rs:50-60` | keep: there is no macro system by decision (A.1 rule 4). Review | — |
| S20 | **`extern "intrinsic"` only in `std`** | the one flow only the library may use (`E0125`, `pom/resolve/shape.rs:27`) | as left | keep: it is the mechanism of this requirement. Review | — |
| S21 | **Hidden spellings** | `GND`, `vss`, `VSS` name ground in the solver's builder and in a noise request; the spec knows `gnd` only (no `vss` in `docs/spec`) | `solver/src/core/builder.rs:54`, `api/src/noise.rs:69` | remove the three aliases (a leftover of the Verilog-AMS front end) | S |

Not special cases, checked: SI suffixes and numeric literals (lexical grammar, Appendix A), `self`/`Self`/`crate`/`super` (path grammar), the std modules written in MSL with bodies (`idt`, `transition`, `slew`, `laplace_*`, `zi_*`, `pnjlim`, `fetlim`, the converters, the monitors: all compile from their source).

**What S4, S6, S8, S11 and S13 need from the language**, in one list, because each is a question of "does a general form exist": inherent `impl` blocks on primitive types written in `std`; `extern "intrinsic"` on a method and on a module; a lang-item marker so the compiler finds `Option`, `Analysis` and the operator traits without a name comparison; default type parameters on a trait (`BitAnd<Rhs = Self, Out = Self>`, which H.10 already prints). Design must check each against `generics` phase E's final state.

**The guard of CC-11 AC 2**: a test that walks every name the resolver can return as `Resolution::Builtin` and fails unless it is in the closed primitive list of S17; and one that fails on a string comparison against a std item name in `crates/piperine-lang/src` (the 220-site scan of this section, as a test).

---

## 3. The test suite (CC-12)

### 3.1 Size

| Crate | Integration targets | Tests in them | Unit tests (in `src/`) | Targets with ≤ 3 tests | Test lines (`tests/`) | Build CPU s | Run s |
|---|---|---|---|---|---|---|---|
| `piperine-solver` | 58 | 367 | 38 | 22 | 16805 | 514 | 3 |
| `piperine-analog-solver` | 13 | 66 | 44 | 4 | 4226 | 269 | 0 |
| `piperine-lang` | 80 | 1497 | 1 | 4 | 36782 | 186 | 288 |
| `piperine-parity` | 60 | 565 | 49 | 15 | 15671 | 135 | 624 |
| `piperine-digital-solver` | 16 | 164 | 2 | 1 | 5927 | 74 | 0 |
| `piperine-api` | 26 | 228 | 79 | 6 | 6588 | 72 | 383 |
| `(root)` | 26 | 422 | 0 | 3 | 12650 | 70 | 271 |
| `piperine-transpile` | 29 | 117 | 53 | 14 | 9000 | 66 | 145 |
| `piperine-ir` | 15 | 162 | 0 | 0 | 10253 | 58 | 0 |
| `piperine-numbers` | 8 | 22 | 0 | 6 | 2739 | 39 | 329 |
| `piperine-abi` | 11 | 52 | 4 | 5 | 2833 | 23 | 0 |
| `piperine-plan` | 9 | 61 | 6 | 4 | 3125 | 17 | 0 |
| `piperine-cli` | 10 | 192 | 14 | 2 | 4666 | 16 | 159 |
| `piperine-interp` | 7 | 57 | 15 | 3 | 2912 | 15 | 0 |
| `piperine-project` | 5 | 53 | 0 | 0 | 1335 | 5 | 0 |
| **total** | **373** | **4025** | **305** | **89** | **135512** | **1561** | **2202** |

*Build CPU* is the sum of the compile time of the crate's test executables from `cargo --timings` (cold, parallel build; it is CPU-seconds, not wall time). *Run* is one serial pass over the executables. The buck targets (`test = false`: 7 in parity, 2 in cli, 1 in api, 2 at the root) are not in these numbers.

- **373 integration targets and 16 unit-test executables** (389 link steps); **4 330 tests ran** (4 025 in targets, 305 in `src/`), 0 ignored; the 104 `#[test]` functions of the on-demand buck targets make up the difference to the source count. Every executable passed.
- **Run: 36 min 42 s** serial. It is concentrated: the 5 slowest executables take 26 % and the 25 slowest 53 % (Appendix F). `piperine-numbers`/`transcendental` alone is 273 s for 7 tests on 12 threads (exhaustive checks against big-integer arithmetic), which is 12 % of the suite's run time.
- **89 targets hold 3 tests or fewer** (19 hold exactly one). Each is a link step of the whole dependency graph for a handful of assertions.
- The numbers of §0 count `#[test]` functions in the source (4 496, the language server's 62 included); the run count is what Cargo executes with the default features.

### 3.2 Where the build time goes

Test executables cost 1 561 CPU-seconds to build against 193 for all the library code they link. The cost is per target, not per test:

| Crate | Targets | Build CPU s (targets only) | Median per target | Slowest target | Why |
|---|---|---|---|---|---|
| `piperine-solver` | 58 | 473 | 7.6 | `transient_reentry` 16 s | most targets re-compile hand-written ABI devices (`tests/helpers/abi_devices.rs`, 894 lines, included 41 times; `abi_mixed.rs`, 1 089 lines, 22 times; `partials.rs` 31 times) at `opt-level = 2` |
| `piperine-analog-solver` | 13 | 248 | 14.1 | `assembly_ddt` 36 s | `tests/support/devices.rs` (**1 738 lines**) is compiled into each target, optimised |
| `piperine-lang` | 80 | 148 | 1.6 | `spec_text` 6 s | many small targets; each links the whole front end |
| `piperine-parity` | 60 | 131 | 2.1 | `source_aggregates` 4 s | 39 `source_*` targets |
| `piperine-digital-solver` | 16 | 73 | 5.1 | `advance` 10 s | `tests/support/devices.rs` (1 253 lines) per target |
| `(root)` | 26 | 70 | 2.5 | `integrated` 4 s | 23 `book_*` targets, one per chapter and two guards |
| `piperine-api` | 26 | 59 | 2.1 | `host_actions` 6 s | 26 targets over the same session fixtures |
| `piperine-transpile` | 29 | 55 | 1.7 | `number_widths` 4 s | 29 targets; `ir/tests/support/digital.rs` (1 028 lines) and `devices.rs` included by path |
| `piperine-ir` | 15 | 51 | 3.9 | `structure` 5 s | 15 targets sharing `tests/support/` by `#[path]` |
| `piperine-numbers` | 8 | 38 | 3.3 | `integers` 17 s | optimised (`opt-level = 3`) exhaustive arithmetic tests |

Merging targets is the lever with the best ratio in this section: a support module compiled once per crate instead of once per target. An estimate from the per-target medians: `piperine-solver` 58 → 8 targets and `piperine-analog-solver` 13 → 3 remove about 520 of the 1 561 CPU-seconds on their own.

**Where the run time goes.** 1 581 of the 2 202 run-seconds (72 %) are in the five packages whose tests run designs on the compiled backend, which calls `rustc` for each design's device library: parity 624 s, api 383 s, root 271 s, cli 159 s, transpile 145 s. The eight crates that never call it (solver, analog-solver, digital-solver, ir, abi, plan, interp, project: 1 091 tests) run in 4 s together; the rest is `piperine-numbers` (329 s, exhaustive arithmetic) and `piperine-lang` (288 s, an unoptimised front end over the corpora).

The compile cache is not shared: **33 `static CACHE: OnceLock<tempfile::TempDir>` in 31 test files** (api 19, cli 8, parity 5, root 1; e.g. `api/tests/analyses.rs:28`, `tests/book/support/model.rs:168`) give each target, sometimes each fixture, a cache directory of its own. The same design is therefore compiled again by every target that uses it, and a `TempDir` held by a `static` is never dropped: the one run of this inventory left **60 directories and 2.5 GB** behind in `TMPDIR`. One cache per test run, owned by the fixture library of §3.5 and placed under the target directory, removes the repeated `rustc` calls and the leak. How many of the 1 581 s it saves was not measured.

### 3.3 Duplicated and near-duplicated tests

No two tests have an identical body. The duplication is by **layer**: the same design proven through each stage that existed when a feature landed.

| # | Duplicate | Evidence | Which is the strongest |
|---|---|---|---|
| T1 | The ngspice twins are compared twice: through the harness (`parity/tests/parity_{op,dc,ac,tran,noise,tf,pz,sens,…}.rs`, 12 targets) and through the user's entry point (`tests/scenarios.rs`, one test per twin with the **same names**: `divider`, `diode_iv`, `diode_rev`, `diode_series`, `isrc_r`, `diode_rs`, `tnom_res`, `pz_lowpass`, `tf_divider`, `sens_divider`, …). Both run ngspice | `tests/scenarios.rs:1-6` says so itself: "the `parity_*` targets there keep their own comparisons" | `scenarios.rs` (the path a user takes). CC-12 AC 5 forbids reducing the parity oracles, so this is Q9, not a default |
| T2 | The digital examples are proven from a hand-built IR (feature `ir-rust`, before the front end could produce them) and again from MSL source (feature `msl-digital`): `transpile/tests/commits.rs` ↔ `parity/tests/source_commits.rs` (4 tests with the same names), `transpile/tests/dpwm.rs` ↔ `parity/tests/buck_dpwm.rs` (3 same names), `transpile/tests/pid.rs` ↔ `parity/tests/buck_control.rs` (identical 41-line `model` and 12-line `script` helpers), `parity/tests/boundary.rs` ↔ `parity/tests/buck_boundary.rs` (2 same names) | the hand-built fixtures: `ir/tests/support/digital.rs` (1 028 lines, `Buck::dpwm` is a 173-line function building the IR by hand) included by path **60 times**, `ir/tests/support/devices.rs` (686 lines) 23 times, `transpile/tests/support/mixed.rs` 40 times | the source-level tests: they prove the same rule through the real pipeline. Caveat: the three `buck_*` ones are on demand (`test = false`), so today the default run holds the hand-built side only; the swap needs the source side in the default run. The hand-built ones stay only where they reach an IR form the front end cannot produce yet |
| T3 | One guard copied into 27 targets | `its_devices_pass_the_partials_verifier` exists in 27 files of `solver/tests/` (`abi_tran.rs:404`, `sensitivity_fallback.rs`, `checkpoint_restore.rs`, `probe_selection.rs`, …), each verifying that target's own hand-written devices | disappears when the devices are shared (§3.5): one test over the shared set |
| T4 | The same refusal asserted at up to four levels | a bundle rule: `pom_bundles`, `types_bundles`, `elab_bundles`, `source_bundles`, plus a Book test; 30 `types_*` and 26 `elab_*` targets in lang mirror 39 `source_*` in parity | by design each level proves a different criterion; candidates are only the pairs whose assertion is the same diagnostic code on the same source. Design lists them per feature; not counted here |
| T5 | 57 test names exist in more than one target (140 definitions) | list in Appendix E | review each pair |

### 3.4 Tests that pin obsolete or superseded behaviour, and tests of prose

| # | Target | Tests | What it pins | Proposal |
|---|---|---|---|---|
| P1 | `cli/tests/docs.rs` (1 074 lines) | **60** | sentences of `GAPS.md`, `ROADMAP.md`, `.specs/STATE.md`, `CLAUDE.md` and a README: 51 of them are of the form "`GAPS.md` no longer lists X" or "`STATE.md` records decision Y", one appended per closed feature (`gaps_drops_what_generics_phase_d_fix_round_2_closed`, `state_records_the_cli_decision`, …). Each was true the day it was written and can only fail when someone edits a tracking file. `GAPS.md` itself says its section numbers are frozen because this target reads them ("the section numbers are the previous file's (`piperine-cli/tests/docs.rs` reads them)") | remove the 51 absence and record pins; keep the structural ones (`gaps_section_10_lists_exactly_the_owned_codes`, `the_roadmap_is_forward_looking`, the buck README's command lines) in the repository-guard target (M7). 60 proof rows cite these tests |
| P2 | `lang/tests/spec_text.rs` (2 693 lines) | **148** | 471 `contains(…)` assertions on the wording of `docs/spec/*.md`: one test per decision (`the_library_declares_no_net_nature`, `part_v_the_step_that_arrives_at_a_landing_is_an_ordinary_step`). The spec is the authority; a test that the spec says what it says proves nothing about the implementation | keep the structural guards (every error code defined once, every method called in a fence is catalogued, the construct count, the file map) and move them next to `spec_codes.rs`; drop the wording pins. The Book's claims (`tests/book/claims_guard.rs`) are different: they tie a sentence to a run |
| P3 | `solver/tests/removed_surface.rs`, `workspace_shape.rs`, parts of `lang/tests/suite_hygiene.rs` | 12 + 7 + 6 | names removed by `solver-restructure`, crates retired, `phdl_corpus_does_not_grow`, `lrm_covers_declarations_are_well_formed`, `no_dead_architecture_identifiers`: guards against a past that no longer has a way back (no PHDL parser, no Verilog-AMS front end exists to regress to) | remove; `suite_hygiene` keeps what guards the present suite |
| P4 | `solver/src/analyses/tf.rs:455-500` | 1 | a unit test that reads its own source file and fails on `eprintln!` | replaced by removing the debug prints (A8) |
| P5 | `interp/tests/frozen.rs` and the `*Forms` refusal lists | — | the interpreter is "frozen at the M1 analog subset" (`interp/src/widths.rs:1-4`) and refuses every later form by name (`CompositeForms`, `GenericForms`, `OperatorForms`, `NumberWidths`, `ParamTypes`); tests pin each refusal | not obsolete, but each new IR form adds a refusal and a test. Q10 asks what the interpreter is for now |

In all, **132 tests are named for an absence or a record** (`no_longer`, `removed`, `retired`, `drops`, `gaps_…`, `state_records…`).

### 3.5 Helpers duplicated across targets

- **219 groups of identical helper bodies in more than one file, 760 copies, 2 036 redundant lines** (exact match after whitespace normalisation; near-copies are not counted). The largest: the device set of `abi/tests/support/examples.rs` copied into `analog-solver/tests/support/devices.rs` (`diode_shape` 57 lines, `comparator_shape` 36, `Diode::eval` 26, `flip_flop_shape` 21, `dac_shape` 17, `resistor_shape`, `capacitor_shape`, `pnjlim`); `Bench::settings` and `settings` ×7 each; `Kept::record` ×10; `Ignore::record` ×26 across root, api and parity; `design_of` ×10 and `diagnostics` ×11 and `snippet` ×8 in `lang/tests/types_*.rs`; `binary` ×7 and `shared_cache` ×5 in `cli/tests`; `net` ×11 and `pin` ×13 in `solver/tests`.
- **Sharing by `#[path]`**: 599 `#[path = "…"] mod` lines in test code, **159 of them reaching into another crate's `tests/support/`** (`../../piperine-ir/tests/support/digital.rs` 60 times, `../../piperine-transpile/tests/support/mixed.rs` 40, `../../piperine-ir/tests/support/devices.rs` 23). This is what the 485 `#[allow(dead_code)]` pay for, and it is invisible to Cargo: a change to one support file rebuilds targets in three crates.
- **Hand-written ABI devices**: 173 `impl Device` blocks in test code, 190 `static` shapes in `solver/tests` alone. Most `solver/tests` targets declare their own two or three devices in the file.

Proposal (CC-12 AC 4): one test-support **library** per layer, compiled once and used as a dev-dependency, with fixture *types* whose methods are the helpers (the root's `tests/` already works this way): the ABI device set (resistor, capacitor, diode, sources, comparator, flip-flop, DAC, timer) in one place for abi, analog-solver, digital-solver and solver; the IR fixtures for ir, plan, interp and transpile; a `Checked`/`Elaborated` source fixture for lang. `piperine-parity` already is such a library for the source-level tests. Q7.

### 3.6 Tests no check or criterion needs

A test is "cited" when its name appears in a `proofs*.md` row, a `tasks*.md` `## Checks` line, `tests/book/inventory.tsv`, `GAPS.md` or the Book.

| | Count |
|---|---|
| distinct test names (language server out) | 4 351 |
| cited somewhere | 4 022 (92 %) — in a proofs file 3 599, in a tasks file 3 456, in `inventory.tsv` 327 |
| `#[test]` functions cited by nothing | **333** of 4 434 |
| targets and unit-test modules with no cited test at all | 19 |

By crate: lang 105, solver 81, digital-solver 31, parity 25, api 21, analog-solver 14, transpile 14, root 13, interp 8, ir 8, cli 6, abi 5, plan 2. The clusters: `digital-solver/tests/advance.rs` (13 of 36), `solver/tests/probe_selection.rs` (10 of 11), `solver/tests/live_params.rs` (9 of 11), `lang/tests/spec_text.rs` (14), `lang/tests/elab_structure.rs` (9), `lang/tests/buck_example.rs` (7 of 7), `solver/tests/{stepper_fold,nature_abstol_inheritance}.rs` (3 of 3 each), unit tests of `solver/src/core/{builder,net}.rs` and `analyses/{sp,pz}.rs`. An uncited test is not a useless one (older solver tests predate the proof format); it is one that can be merged or removed **without re-pointing anything**, so these go first.

### 3.7 The constraint on every removal (CC-12 AC 5)

- **3 349 proof rows in 44 features** cite 3 029 distinct `run <target> / test <name>` pairs. A proof line names the **target** as well as the test, so merging two targets re-points every row of both, even when no test changes. 106 pairs are cited by more than one row.
- **`tests/book/inventory.tsv`**: 817 rows, about 330 distinct `covered_by` references `<crate>/<target>::<fn>`, 69 of them to `piperine-cli/book`, the rest to the root's `book_*` targets and 5 other cli targets.
- **`GAPS.md`** section numbering is read by `cli/tests/docs.rs`; `lang/tests/support/code_owners.tsv` is read by guards.
- So: (1) the re-pointing must be a script over `.specs/features/*/proofs*.md`, `tasks*.md` and `inventory.tsv`, run in the same commit as each merge, with `check_proofs.py` (the skill's script, not in the repository) run for every feature afterwards; (2) **renaming a test is as expensive as merging a target**, so tests keep their names; (3) the order is: remove uncited tests and prose pins first (333 + P1 to P3, where P1's 60 rows are cited by proofs of features whose criterion was "the tracking file says so"), merge targets second, deduplicate layers (T1, T2) last and only on the user's answer.
- The parity oracles (`parity_*`, the twins, `ToleranceContract`, the buck targets) are not reduced by anything proposed here except T1, which is a question.

### 3.8 Candidate merges (CC-12 AC 3)

| Crate | Today | Proposed targets | Notes |
|---|---|---|---|
| `piperine-solver` | 58 | `protocol` (8 → 1), `abi_devices` (7 → 1), `analyses` (ac, noise, pz, pss, sens, adjoint, tf…), `transient` (reentry, truncation, checkpoint, landing, jump, crossing, stepper), `host` (host_*, live_params, probe_selection, session), `guards` | about 8; largest saving (§3.2) |
| `piperine-analog-solver` | 13 | `assembly` (the 7 `assembly_*`), `numerics`, `boundary` | 3 |
| `piperine-digital-solver` | 16 | `engine` (advance, instant, queue, store, settle, checkpoint, relax), `plan` (bind, dag), `boundary` | 3 |
| `piperine-lang` | 80 | `msl` (6 → 1), `pom` (10 → 1), `types` (30 → by the modules of G2, about 6), `elab` (26 → about 6), `std` (3 → 1), `guards` | about 16 |
| `piperine-parity` | 60 (+7 buck) | `source` by Part of the spec (39 → about 8), `parity` (12 → 1 if ngspice's per-twin caching is kept per process), `harness` | about 11 |
| `piperine-transpile` | 29 | `print`, `compile` (cache, rustc, load, compile, compiled), `digital`, `analog`, `golden` | about 6 |
| `piperine-api` | 26 | `session`, `host`, `records`, `params`, `analyses` | about 6 |
| root | 26 | `book` (the 23 `book_*` files as modules of one target), `integrated`, `scenarios`, `host_ii` | 4; CLAUDE.md says "one target per Book chapter": the files stay, the link steps go from 23 to 1. Q11 |
| abi, ir, plan, interp, cli, numbers, project | 65 | one or two each | about 12 |

From 373 link steps to roughly 70. Each merged target keeps a `//!` header and one `mod` per former target, so a test's path changes only in its target name.

---

## 4. Dead code, unused items, the language server

### 4.1 Never referenced (12)

`DirtySet::is_dirty` (`digital-solver/src/run.rs:75`), `AdjointOptions::new_diff` (`solver/src/analyses/adjoint.rs:63`), `BinaryFormat::round_integer` (`numbers/src/format.rs:161`), `IrBuilder::wide_at` (`ir/src/build.rs:161`), `Params::keep_exact` (`api/src/params.rs:576`), `GenericParam::default_type` (`lang/src/pom/reflect/items.rs:191`), `Ty::has_repr` (`lang/src/types/ty.rs:406`), `Assembly::ddt_unknowns` (`analog-solver/src/device/assembly.rs:498`), `ConvergencePlan::{with_limits, with_schedules, with_trace}` (`analog-solver/src/convergence.rs:425-438`), variant `DeferReason::InferredGenerics` (`lang/src/pom/resolution.rs:28`). All are `pub`, which is why the compiler is silent.

### 4.2 Referenced only from tests (78)

`pub` items whose only callers are tests: either API kept for a host that does not exist yet, or leftovers. By file: `lang/src/pom/reflect/hardware.rs` 9 (`Module::analog_blocks`, `Instance::via_template`, …; the POM is a public reflection surface, `docs/pom/` documents it, so these are probably wanted), `parity/src/scenario/trace.rs` 5 and `parity/src/golden.rs` 4 (a test library: expected), `solver/src/protocol/statistics.rs` 5, `solver/src/analyses/transient.rs` 5 (`with_reentry`, `with_dt_min`, `with_record_from`, `with_probe_selection`, `set_plan`), `solver/src/core/result.rs` 4, `digital-solver/src/engine.rs` 3, `api/src/source.rs` 3, `numbers` 5, `ir` 3, and single items elsewhere (`project::load_project`, `abi::verify::verify_partials`, `Design::from_json`, `Session::noise_probe`, `Session::drive_pattern`, `ConstEval::files_read`, `Error::with_code`). Design decides per item: public surface with a doc comment saying who it is for, or removed with its test.

An exact list of unused `pub` items needs the compiler (`-W unreachable_pub` and making crate-internal items `pub(crate)`); design should run it once the language server is out of the way.

### 4.3 `piperine-lang-server`

3 509 lines in 26 files, 2 938 lines of tests. `cargo check -p piperine-lang-server` fails with **90 errors**: it imports `piperine_lang::{ast, lexer, error, parse, elab::registry, source_provenance, SourceMap, SourceMapIncludeResolver, seed_device_port_schemas}`, none of which exists; its manifest describes it as the "LSP language server for PHDL". It has 71 free functions, 14 files without `//!`, 5 `#[allow]`s and functions up to 138 lines. `editors/vscode` beside it is a Verilog-AMS extension (`GAPS.md` H13). `GAPS.md` B3 and `ROADMAP.md` line 91 record the user's decision to defer the server to last.

**Recommendation: delete the crate and `editors/vscode` from the tree in this feature; the `lang-server` feature rebuilds it on the POM's reflection (`pom::Reflect`, which is the surface a server needs and did not exist when this code was written).** Reasons: (1) it cannot be made to follow any rule here without first being made to build, which is the deferred feature itself; (2) every guard this feature adds (free functions, names, docs, length) must either exempt it or fail on it, and an exemption list that starts with a whole crate is how exemptions rot; (3) nothing in it survives the front-end change: the 90 errors are in the types every handler takes; the LSP plumbing (`server.rs`, `dispatch.rs`, `text_pos.rs`) is the only reusable part and stays in git history; (4) `--workspace` works again and CLAUDE.md loses its "never `cargo build --workspace` bare" rule. It is the user's call (Q1): CLAUDE.md says "do not edit".

---

## 5. Largest files and where to split them

| Lines | File | Holds | Split by responsibility |
|---|---|---|---|
| 2 411 | `ir/src/check.rs` | `Checker` (2 052 lines of methods) with `expr` 518, `ids` 200, `code` 185, `stmt` 129 | G4: `check/{ids,types,classes,statements}.rs`, one method per `Expr` variant |
| 2 012 | `solver/src/analyses/transient.rs` | 17 types; `TransientSolver` 1 027 lines | G7 |
| 1 641 | `transpile/src/print/value.rs` | `ValuePrinter` 990 lines + 284 lines of unit tests | G5 |
| 1 626 | `ir/src/print.rs` | `Printer` 1 382 lines | G6: `print/{body,expression,arenas}.rs` |
| 1 388 | `ir/src/model.rs` | **76 types**: the whole IR object model in one file, and the least documented one | `model/{body,analog,digital,value,function,types}.rs` following the comment banners already in the file |
| 1 320 | `ir/src/deps.rs` | `Walk` 696 lines (`expr` 212, `body` 144, `finish` 110), `Deps`, `BodyDeps`, `Sources` | the result types apart from the walk that computes them |
| 1 271 | `api/src/request.rs` | `Keys` (38 fields, `spec` 185 lines), `Overrides` (37 fields), `RunSpec`, 425 lines of unit tests | G9: `request/{run_table,overrides,options,grid}.rs` |
| 1 243 | `plan/src/plan.rs` | 11 types; `BodyPlan::new` 292 lines | G10 |
| 1 138 | `abi/src/raw.rs` | 27 `#[repr(C)]` types and 97 constants | by section of abi.md: `raw/{views,shape,calls,constants}.rs` (`unsafe` stays confined, as the crate doc promises) |
| 1 110 | `lang/src/pom/reflect/hardware.rs` | 14 reflection handles (`Module`, `Port`, `Instance`, `Behavior`, …) | one file per handle family: `module.rs`, `instance.rs`, `behavior.rs`, `nature.rs` |
| 1 087 | `api/src/session.rs` | `Session` 667 lines | G8 |
| 1 067 + 977 | `lang/src/elab/consts.rs`, `consts/composite.rs` | `ConstEval` | G3 |
| 1 028 | `lang/src/msl/print.rs` | `Printer` 747 lines + 9 free functions | `print/{items,expressions,hardware}.rs` |
| 1 013 | `lang/src/elab/structural.rs` | 8 types; 524 lines of `Elaboration` methods | with G1's `Structure` |
| 950 | `plan/src/digital.rs` | `Digital` 489 lines, `CarrierLayout`, `RecordLayout`; 8 `panic!`s | `digital/{carriers,records,reactions}.rs` |
| 946 | `lang/src/elab/lower.rs` | `Scope` (42 fields), `lower_expr_unchecked` 213 lines, `lower_path` 104 | with G1's `ValueLowering` |
| 927 | `solver/src/core/bind.rs` | 12 types and the 371-line `bind` | F9 |
| 896 | `interp/src/walk.rs` | `Walk` 437 lines + 223 of unit tests | by statement family |
| 887 | `digital-solver/src/run.rs` | 13 types, 15 free functions | F6 |
| 875 | `lang/src/types/check.rs` | `Checker`'s 68 fields and the declaration worklist | with G2 |

Test files over 1 000 lines: `lang/tests/spec_text.rs` 2 693 (P2), `analog-solver/tests/support/devices.rs` 1 738, `lang/tests/pom_resolve.rs` 1 541, `lang/tests/suite_hygiene.rs` 1 418, `lang/tests/msl_errors.rs` 1 393, `digital-solver/tests/support/devices.rs` 1 253, `solver/tests/helpers/abi_mixed.rs` 1 089, `cli/tests/docs.rs` 1 074 (P1), `ir/tests/support/digital.rs` 1 028.

---

## 6. `P0001`: which refusal sites are code-structure problems

The sweep itself (probe each site, give it a `GAPS.md` row or prove it unreachable) belongs to `spec-closure`, as `.specs/features/generics/tasks-d.md` "Fix round 15" hands it off. That hand-off counted 492 sites in 80 files; at this base the same `grep` gives **501 in 82 files**: `types` 99, `elab` 402 (of which the const evaluator 130). What is structure, not coverage:

| # | Problem | Evidence | What code-clarity does |
|---|---|---|---|
| R1 | **Three refusal channels with no shared owner.** `Checker::unsupported` (`types/check.rs:419`), `Elaboration::unsupported` (`elab/elaborator.rs:293`) — identical bodies — and `ConstFault::Unsupported { expr, what }` (113 constructions), turned into a diagnostic later by `elab/diagnose.rs` | 352 `self.unsupported(…)`, 113 `ConstFault::Unsupported`, 17 `Self::unsupported`, 16 bare, 3 through a closure | one `Refusal` type: the form refused, its span, its owner. The three channels construct it; one place words the diagnostic |
| R2 | **The refused form is free text.** 311 distinct literal messages, 71 built with `format!`; nothing ties a message to its `GAPS.md` row, so "every refusal has an owner" (rule 2) cannot be checked by a test. Error codes have such a table (`lang/tests/support/code_owners.tsv`); `P0001` has none | the message is the only identity a site has | an enum of refused forms (`Unlowered` exists already for the checker-accepts/elaborator-refuses list: `elab/unlowered.rs`, 34 lines) with the `GAPS.md` id as data. Then the sweep of `spec-closure` is a table to fill, and a guard fails on a refusal with no owner. **This is the piece that makes the sweep cheap**; doing it here first is the natural order |
| R3 | **The same refusal written at several sites.** 25 messages are raised from more than one place (68 sites): "a `str` that is not a text after inlining" ×4 (`elab/records.rs:92,169,180,251`), "an `Option` whose payload has no zero in the IR" ×3 (`elab/compound.rs:662,688,702`), "an item of a child whose module is not known" ×3 in three files (`elab/reach.rs:159`, `reach_values.rs:49`, `reach_through_templates.rs:261`), "method `{}`" ×3 (`types/call.rs:303,332,338`), and 20 pairs | repeated logic without an owner (CC-07) | each becomes one method that refuses |
| R4 | **The same form refused in two phases.** "`expect` with a message that is not a string literal" (`types/call.rs:440` and `elab/compound.rs:560`), "this name as a pattern" (`types/patterns.rs:311` and `elab/patterns.rs:327`): the elaborator's copy cannot be reached when the checker's fires first | 2 exact pairs; more exist with different wording, which only R2's enum would reveal | the later site becomes an internal guard (`P0004`) or goes |
| R5 | **Two evaluators refuse the same expression language separately.** `ConstEval` (130 sites in `elab/consts*`) and the lowering (272 sites) each list the forms they do not handle; a form added to one is not added to the other | `elab/consts.rs` 42 sites, `consts/composite.rs` 32, `consts/numbers.rs` 19, `consts/control.rs` 15 | out of scope as a redesign; R2's enum at least makes the two lists comparable |
| R6 | **A milestone name in user-facing text.** Every `P0001` ends "… is outside the M1 subset" | `types/check.rs:420`, `elab/elaborator.rs:294` | the message changes (CC-09 note: this moves goldens and Book status boxes that pin the text; Q12) |
| R7 | **`types/unsupported.rs`** is "the one place that names a construct outside the M1 subset" (its header) and holds 147 lines, while 99 sites in `types/` refuse elsewhere | `types/unsupported.rs:1` | the header becomes true (R2) or the file goes |

---

## 7. Everything ranked by value over effort

Value: how much it moves the seven rules or the suite's cost. Effort: S (days, one crate), M (one to two weeks, several crates), L (a batch of its own).

| Rank | Finding | Value | Effort | Risk to CC-09 |
|---|---|---|---|---|
| 1 | Language server out of the tree (§4.3) | unblocks every workspace-wide guard; −6 400 lines | S | none |
| 2 | Prose pins and dead guards out (P1, P2, P3; 230 tests, about 4 500 lines) | removes the tests that fail on documentation edits | S | proof rows of the pins re-pointed or retired (Q13) |
| 3 | Shared test-support libraries, one shared compile cache, and target merges for solver, analog-solver, digital-solver (§3.2, §3.5, §3.8) | about 520 of 1 561 build CPU-seconds, 486 `#[allow(dead_code)]`, 2 000 duplicated lines | M | proof rows re-pointed by script |
| 4 | `Refusal` owner and the refused-form table (R1, R2, R3) | makes rule 2 checkable and the `spec-closure` sweep a table | M | none if messages are kept byte-identical |
| 5 | `Option<Bound>` (A2), the real user-input panics (§1.9 row 2), dead items (§4.1), hidden ground aliases (S21), duplicated rules (A4) | 5 small fixes, each removes a class | S | S21 changes what is accepted: spec-conformant, but say so |
| 6 | Doc comments everywhere + `#![deny(missing_docs)]` (CC-10) | 1 322 items, 2 531 fields and variants | M, mechanical but must be written with understanding | none |
| 7 | Free functions get owners outside the parser (F2 to F14: 293 functions) | rule 4 | M | none |
| 8 | Split the god methods over 60 lines (134) and the largest files (§5) | rule 6 | L | none; transpile goldens guard the printers |
| 9 | CC-11 small cases: S3, S7, S12, S14, S16, S21 | removes six exceptions cheaply | S to M | spec amendments (S7, S16) |
| 10 | Names (§1.3 b, c: about 760 declarations) | rule 5 | L, mechanical | public api names: Book and examples follow (CC-09 AC 2) |
| 11 | Split `Elaboration` and `Checker` (G1, G2) and regroup `elab/`, `types/` (M1) | the largest readability gain of the feature; rule 5's "one purpose per type" | L | none, but the widest diff; do after 7 and 8 so the pieces are already small |
| 12 | CC-11 declarations in std: traits (S8), core (S11), methods (S4), carrier methods (S13), intrinsic bounds (S15) | the requirement's centre: three hidden tables become MSL | L | diagnostics may change wording; no numeric change |
| 13 | CC-11 language changes: `emit` (S1/S2), `Option` as an enum (S6), named iterator types (S5), arithmetic operator traits (S9) | removes the special cases the user named | L each, spec + Book + corpora | S1 rewrites about 700 `emit!` sites; all need the user's choice first |
| 14 | Parser grammar owners (F1: 124 functions) | rule 4 for the last 30 % of free functions | M | none; depends on Q4 |
| 15 | Remaining target merges (lang, parity, transpile, api, root) and layer deduplication (T1, T2) | link steps 373 → about 70 | M | T1 only with the user's yes |
| 16 | Single-implementor traits (A3), `lib.rs` code (A9), module names (M2, M3), trace sink (A8) | rule 7 | S each | none |

---

## 8. Proposed batches for design

Order follows two constraints: guards that scan the workspace need the language server gone and the test suite cheap first; renames come after moves so each line is touched once.

| Batch | Contents | Findings | Closes |
|---|---|---|---|
| **B0 — Ground** | remove the language server and `editors/vscode` (on Q1); repository-guard target in one home (M7); baseline report of CC-12 AC 6 from §3.1 | §4.3, M7 | precondition of every guard |
| **B1 — Lean tests I** | remove uncited tests where they duplicate (§3.6), the prose pins and the dead-architecture guards (P1 to P4); the re-pointing script for proofs and `inventory.tsv` | §3.4, §3.6, §3.7 | CC-12 AC 2, 5 |
| **B2 — Lean tests II** | test-support libraries (ABI devices, IR fixtures, source fixtures) with owner types; one compile cache per test run in place of the 33 per-target ones (§3.2); merge targets crate by crate; drop the 485 `#[allow(dead_code)]` and the cross-crate `#[path]`s; `Device::react` default (A1, on Q8) | §3.2, §3.5, §3.8, T3 | CC-12 AC 3, 4, 6 |
| **B3 — Small defects of structure** | A2, A4, A6, A8, A9, §1.9 user-input panics, §4.1 dead items, §4.2 decisions, M5 deviations, M6 | §1.6, §1.9, §4 | parts of CC-03, CC-07 |
| **B4 — Refusals have an owner** | R1 to R4, R6, R7 | §6 | rule 2; hands `spec-closure` its table |
| **B5 — Owners and small methods, back end** | free functions and god methods of abi, analog-solver, digital-solver, solver, plan, project, api, transpile, ir, interp (F6 to F13, G4 to G10); file splits of §5 for those crates; the length and parameter guards | §1.2, §1.5, §5 | CC-04, CC-06 for 11 crates |
| **B6 — Owners and small methods, front end** | lexer, POM passes, printer (F2 to F5); parser grammar (F1, on Q4); split `Checker` (G2) then `Elaboration` (G1), `ConstEval` (G3); regroup `types/` and `elab/` (M1, M4) | §1.2, §1.4, §1.10 | CC-02, CC-04, CC-06 for lang |
| **B7 — No special cases I: declare what exists** | `std/traits.msl`, `std/core.msl`, `std/numbers.msl`; S3, S4, S7, S8, S11 to S17, S21; the three tables go; the prelude-declaration guard | §2 | CC-11 AC 2 |
| **B8 — No special cases II: change the language** | S1/S2, S5, S6, S9, S16 as the user chooses; spec, Book, corpora and tests in the same commits | §2 | CC-11 AC 1, 3 |
| **B9 — Names** | the glossary; the rename list of §1.3; module and crate names (Q3); the naming guard | §1.3 | CC-05 |
| **B10 — Words** | doc comments on every item, `#![deny(missing_docs)]` per crate, the private-item doc guard; crate `lib.rs` purpose, module map and data path; `ARCHITECTURE.md` or the Book chapter; feature tags in comments replaced by spec rules | §1.7, M9 | CC-01, CC-08, CC-10 |
| **B11 — Close** | the remaining merges and layer deduplication (T1, T2 on Q9); before/after report; full suite once | §3.3, §3.8 | CC-12 AC 6, CC-09 |

B7 before B9: declaring builtins in MSL deletes code that B9 would otherwise rename. B10 last among the code batches: a doc comment written before a type is split or renamed is written twice.

---

## 9. Open questions for the user (asked before design)

| # | Question | Default if not answered |
|---|---|---|
| Q1 | **Language server**: delete `crates/piperine-lang-server` and `editors/vscode` now and rebuild in the `lang-server` feature on the POM, or keep them broken and exempt from every guard? | delete (§4.3) |
| Q2 | **Glossary**: is the list of §1.3 (a) the glossary? Two calls in particular: `eval`/`Eval` (the ABI contract's word, 25 names) and `…Spec` (`PinSpec`, `TermSpec`, 22 names): glossary, or spelled out? And `param`: glossary because it is the MSL word? | all three in the glossary |
| Q3 | **Crate and module renames**: `piperine-interp` → `piperine-interpreter`, `elab` → `elaboration`, `rt` → `runtime`, `solver/src/core` → a name that says what it holds. Crate names appear in the Book's organisation chapter and in every proof line (`run <target>` is safe, `-p <crate>` is not) | rename modules; keep crate names |
| Q4 | **The parser**: 124 of the 417 free functions are `chumsky` combinator builders. Give them grammar owner types (methods that return parsers), or list the parser as an exception to rule 4 with the reason "a combinator grammar is a set of productions"? | owner types |
| Q5 | **Rule 7 and third-party macros**: do `select!` (19), `#[derive(Logos)]` and the serde derives (370, with one non-test consumer) fall under "no macros"? If the POM/IR JSON surface has no user, it can go with its 185 derives | third-party derives allowed; ask whether JSON export of the POM is a wanted feature |
| Q6 | **One module layout**: `x/mod.rs` (14 today) or `x.rs` + `x/` (7 today)? | `x.rs` + `x/` (no file named `mod.rs`; the tab says what the file is) |
| Q7 | **Test support as libraries**: may each layer get a dev-only support crate (or `piperine-parity` be renamed to what it has become), replacing the 159 cross-crate `#[path]` includes? New crates are a workspace change | yes, one support crate for device fixtures and one for IR fixtures |
| Q8 | **`Device::react` default method** in `piperine-abi` (A1): acceptable change to the ABI's public trait? It removes 102 no-op bodies and changes no layout | yes |
| Q9 | **The twins compared twice** (T1): keep both `parity_*` and `scenarios.rs`, or keep the user-entry-point one and let `parity_*` hold only what `scenarios` does not reach? CC-12 AC 5 says the parity oracles are not reduced | keep both |
| Q10 | **The interpreter**: it is frozen at the M1 analog subset and refuses every later form. Is it still the differential oracle (then it must grow), or a reference for the analog core only (then say so in its crate doc and stop adding refusal tests)? Not a clarity question, but it decides how much of `piperine-interp` this feature polishes | reference for the analog core; polish as is |
| Q11 | **Book targets**: one test target with a module per chapter instead of 23 targets? CLAUDE.md says "one target per Book chapter" | merge; update CLAUDE.md's sentence |
| Q12 | **`P0001` wording**: drop "outside the M1 subset" from the message (R6)? It moves every golden and Book status box that pins the text, so it is a behaviour change CC-09 forbids unless CC-11-style approval covers it | change it, in B4, with the pins |
| Q13 | **Proof rows of removed prose pins** (P1): a closed feature's criterion "GAPS.md drops X" loses its test. Re-point the row to a structural guard, or mark the criterion as retired with this feature's id? `check_proofs.py` must stay green either way | retire the row with a note naming code-clarity |
| Q14 | **CC-11, the big four** (rank 13), one by one: (a) `emit!` → `emit("stream", Record { … })` (about 700 sites rewritten), or user functions gain named variadic arguments, or keep; (b) `Option` as a std enum; (c) named `Range`/`Iterator` types; (d) arithmetic operator traits | (a) the struct form; (b) yes; (c) yes; (d) defer to `spec-closure` |
| Q15 | **CC-11, the keeps**: S10 (`Fn`, `Nature`), S17 (primitive types, `gnd`), S18 (nature quantity keys), S19 (closed `derive`), S20 (`extern "intrinsic"` only in `std`) are proposed as kept with a written reason. Agreed? | agreed |
| Q16 | **The method-length limit**: 40 lines (316 functions to split), with exhaustive one-line-arm `match` tables measured by their longest arm; and 5 parameters beyond `self`. Or 60 (134 functions)? | 40 and 5 |
| Q17 | **Tests follow rule 3 too** (CLAUDE.md says so): 1 678 free helper functions in tests become methods of fixture types. Confirm the scope, since it is the single largest count in this file | yes, done inside B2 as support moves into libraries |
| Q18 | **Scope of comment tags**: replace feature and task ids in doc comments (`DIG-09`, `T47`; at least 666) by spec rules and `MD-n`, as CC-10 AC 3 reads? The ids are how a reader finds the feature folder today | replace where a spec rule exists; keep `MD-n` |

---

## Appendix A — Free functions outside tests, by file (CC-04)

417 functions in 80 files; the language server's 71 are not listed. `main` and the `extern "C"` functions of §1.2 are left out.

| File | n | Functions (line) |
|---|---|---|
| `abi/src/thunk.rs` | 12 | `module`:26 `module_with`:42 `library`:56 `write_note`:68 `panic_message`:84 `guarded`:95 `cut`:110 `cut_mut`:136 `create_from`:178 `destroy_inst`:247 `eval_on`:277 `react_on`:335 |
| `abi/src/validate.rs` | 9 | `known`:214 `known_bits`:218 `validate`:272 `check_pin`:352 `check_layout`:409 `cell_bytes`:426 `check_function`:524 `check_reaction`:605 `check_lists`:650 |
| `abi/src/raw.rs` | 3 | `pod_ref`:930 `pod_mut`:940 `pod_bytes`:949 |
| `abi/src/verify.rs` | 2 | `f64_at`:30 `verify_partials`:42 |
| `abi/src/caller.rs` | 2 | `empty_note`:60 `check_cells`:412 |
| `abi/src/layout.rs` | 2 | `leaf_size`:27 `layout_eq`:39 |
| `abi/src/device.rs` | 1 | `cell_lens`:108 |
| `abi/src/shape.rs` | 1 | `index`:158 |
| `analog-solver/src/device/small_signal.rs` | 6 | `response`:24 `stamp_matrix`:37 `stamp_source`:50 `strength`:60 `phasor`:65 `stimulus`:194 |
| `analog-solver/src/device/limit.rs` | 5 | `active`:40 `take_iterates`:47 `linear`:64 `vetoes`:80 `advance`:94 |
| `analog-solver/src/device/plan.rs` | 5 | `entry_error`:175 `is_analog`:236 `column`:241 `source`:253 `bind_instance`:283 |
| `analog-solver/src/device/assembly.rs` | 3 | `max_of`:637 `run`:698 `check_finite`:728 |
| `analog-solver/src/device/watch.rs` | 3 | `tolerance`:53 `accept_watches`:77 `record_traces`:136 |
| `analog-solver/src/device/ddt.rs` | 2 | `companion_current`:89 `stamp_ddt`:109 |
| `analog-solver/src/device/flow.rs` | 2 | `stamp_rows`:80 `stamp_flow`:105 |
| `analog-solver/src/device/mod.rs` | 1 | `device_error`:25 |
| `analog-solver/src/device/delay.rs` | 1 | `stamp_delay`:68 |
| `analog-solver/src/stamp_validation.rs` | 1 | `non_finite_stamp_error`:20 |
| `analog-solver/src/crossing.rs` | 1 | `predict_crossing`:31 |
| `analog-solver/src/math/integration.rs` | 1 | `bdf2_coeffs`:17 |
| `api/src/request.rs` | 6 | `entries`:754 `span`:760 `span_of`:764 `number`:769 `i64_of`:777 `is_identifier`:784 |
| `digital-solver/src/run.rs` | 15 | `fault`:97 `write_net`:102 `write_element`:125 `fire_net`:146 `cell_bytes`:230 `call_function`:283 `apply`:348 `run_functions`:365 `run_cone`:382 `walk`:397 `relax`:437 `fired_reactions`:533 `call_reaction`:598 `earliest_other`:748 `run_reactions`:765 |
| `digital-solver/src/plan.rs` | 11 | `bind_error`:225 `pin_name`:230 `nets_of`:236 `analyze`:251 `reachability`:359 `crossing_reach`:411 `array_net`:452 `check_bindings`:461 `check_layouts`:517 `find_writers`:597 `parameter_cone`:717 |
| `digital-solver/src/dag.rs` | 1 | `components`:161 |
| `lang/src/pom/resolve/classify.rs` | 32 | `access_table`:51 `run`:169 `bundle_structs`:189 `port_kind`:211 `path_res`:231 `child_named`:241 `missing_binding`:258 `repeated_binding`:265 `is_written_option`:271 `positional_slots`:281 `is_param`:286 `is_port`:290 `is_field`:294 `is_fn_param`:298 `struct_expressions`:303 `struct_patterns`:345 `enclosing_module`:367 `is_fn_bounded`:379 `through_value`:386 `let_template`:408 `template_module`:417 `block_template`:465 `callee_class`:474 `nature_of_expr`:528 `probe_callee`:554 `named_arguments`:579 `param_block`:607 `args_first_nature`:620 `contribution_targets`:626 `declares_flow`:636 `is_flow_access`:641 `call_expressions`:645 |
| `lang/src/msl/parser/entry.rs` | 30 | `token_input`:8 `to_diagnostic`:17 `is_operator`:78 `summarize_expected`:95 `join_alternatives`:136 `finish`:156 `parse_type`:209 `parse_pat`:217 `parse_generics`:225 `parse_expr`:233 `parse_module_item`:241 `parse_block`:249 `is_open`:264 `is_close`:268 `ends_operand`:274 `classify`:297 `skip_overflow`:317 `overflow`:340 `chain_overflow`:352 `closer_of`:465 `unclosed_delimiters`:476 `drop_dangling_docs`:514 `angle_pairs`:546 `lex_limited`:577 `parse_bytes`:683 `parse`:694 `unused_keyword`:737 `added_keyword`:747 `names_expected`:753 `name_error`:758 |
| `lang/src/msl/parser/base.rs` | 27 | `removed`:56 `reference_head`:63 `skip_to_end`:98 `skip_head_rest`:107 `visibility`:119 `is_place`:136 `not_a_place`:147 `handles`:185 `punct`:213 `kw`:217 `ident`:221 `path_ident`:228 `lit`:240 `lit_expr`:252 `delim_open`:257 `delim_close`:261 `skip_group`:267 `macro_error`:288 `string_lit`:300 `first_of`:311 `is_lbrace`:322 `is_closer`:326 `starts_item`:331 `skip_rest_of_item`:345 `skip_rest_of_stmt`:384 `recover`:399 `recover_unless`:409 |
| `lang/src/msl/lexer.rs` | 27 | `line_comment`:202 `block_comment`:209 `string`:234 `float_dot`:254 `raw_string`:266 `cook`:283 `lex`:289 `lex_with_comments`:296 `lex_bytes`:418 `classify_ident`:436 `raw_ident`:450 `loop_label`:468 `dropped_class`:482 `lex_failure`:498 `int_suffix`:543 `int_parts`:560 `int_value`:574 `float_suffix`:590 `unescape`:614 `int_suffix_text`:673 `range_diag`:681 `classify_number`:704 `is_type_suffix`:801 `si_letter`:805 `unknown_suffix`:809 `float_value`:816 `parse_f64`:831 |
| `lang/src/msl/parser/check.rs` | 16 | `file`:74 `items`:81 `overload`:92 `argument_order`:107 `item_kind`:126 `module`:146 `module_items`:154 `generation`:180 `digital`:205 `impl_items`:222 `fn_decl`:232 `misplaced`:240 `block`:255 `stmt`:261 `expr`:342 `expr_inner`:346 |
| `lang/src/msl/parser/expr.rs` | 14 | `mk`:17 `boxed`:21 `gt`:30 `eq_tok`:34 `next_not_angle`:40 `next_is_not`:44 `touching`:49 `gt_op`:72 `apply_post`:119 `binary`:139 `build`:148 `block_like`:581 `block`:653 `stmt`:665 |
| `lang/src/msl/parser/msl.rs` | 13 | `word`:12 `nature_decl`:19 `signal_decl`:67 `place`:80 `gen_of`:122 `statement_with_arrow`:175 `module_item`:211 `recover_item`:215 `misplaced_emit`:340 `module_decl`:354 `reaction`:438 `digital_item`:448 `behavior_block`:472 |
| `lang/src/msl/parser/item.rs` | 11 | `meta`:7 `rejected_meta`:67 `removed_where`:79 `dir`:89 `fn_decl`:95 `where_clause`:197 `const_item`:231 `impl_items`:242 `fields`:271 `use_tree`:320 `build`:351 |
| `lang/src/msl/parser/ty.rs` | 10 | `path`:8 `bound`:13 `path_with`:17 `generic_args`:39 `neg_lit_expr`:68 `fn_sugar`:76 `generics`:95 `qualified_path`:128 `sigtype`:142 `build`:169 |
| `lang/src/msl/print.rs` | 9 | `print`:30 `enc`:115 `dec`:128 `doc`:146 `indent`:159 `braces`:167 `binop`:1009 `vis`:1018 `dir`:1022 |
| `lang/src/pom/builtin.rs` | 6 | `row`:36 `builtin`:148 `builtin_method`:161 `builtin_member`:167 `sized`:174 `is_core_primitive`:190 |
| `lang/src/pom/reflect/body.rs` | 6 | `text_in`:79 `nested_stmts`:621 `walk_stmt`:629 `walk_block`:650 `walk_expr`:656 `walk_expr_inner`:660 |
| `lang/src/pom/body.rs` | 5 | `expr_children`:269 `stmt_children`:319 `stmt_own_exprs`:329 `block_exprs`:340 `preorder`:345 |
| `lang/src/pom/resolve/members.rs` | 4 | `run`:70 `local_types`:120 `child_field`:411 `child_fn`:419 |
| `lang/src/pom/resolve/scopes.rs` | 4 | `first_of`:131 `duplicates`:144 `namespace_table`:171 `collect_members`:200 |
| `lang/src/pom/resolve/mod.rs` | 3 | `run`:75 `std_prelude`:148 `dependency_names`:157 |
| `lang/src/pom/resolve/imports.rs` | 3 | `import_prefer`:75 `path_diagnostic`:563 `occupies`:612 |
| `lang/src/pom/index.rs` | 3 | `place_root`:96 `place_targets`:115 `build`:134 |
| `lang/src/pom/to_ast.rs` | 3 | `loc`:39 `ident`:43 `use_tree`:649 |
| `lang/src/pom/resolve/shape.rs` | 2 | `run`:16 `extern_module`:55 |
| `lang/src/pom/source.rs` | 2 | `normalize`:60 `mod_candidates`:84 |
| `lang/src/pom/crates.rs` | 2 | `is_crate_name`:59 `check_name`:131 |
| `lang/src/pom/reflect/mod.rs` | 2 | `decl_roots`:223 `qualified_segment`:269 |
| `lang/src/pom/reflect/items.rs` | 2 | `impls_named`:119 `method_of`:125 |
| `lang/src/pom/lower/mod.rs` | 2 | `lower_file`:42 `item_name`:148 |
| `lang/src/types/catalogue.rs` | 2 | `row`:58 `signed`:62 |
| `lang/src/types/record.rs` | 2 | `reserved_field`:44 `is_reserved`:53 |
| `lang/src/msl/parser/pat.rs` | 2 | `build`:7 `binding`:202 |
| `lang/src/pom/resolve/leaves.rs` | 1 | `run`:330 |
| `lang/src/pom/design.rs` | 1 | `crate_name`:556 |
| `lang/src/pom/reflect/hardware.rs` | 1 | `deep_items`:22 |
| `lang/src/pom/reflect/index.rs` | 1 | `writers_of`:29 |
| `lang/src/pom/lower/body.rs` | 1 | `name`:14 |
| `lang/src/pom/lower/hardware.rs` | 1 | `nature_kind`:18 |
| `lang/src/pom/lower/items.rs` | 1 | `use_tree`:188 |
| `lang/src/pom/lower/meta.rs` | 1 | `doc`:11 |
| `lang/src/types/continuity.rs` | 1 | `bind_pattern`:60 |
| `lang/src/msl/diag.rs` | 1 | `render`:87 |
| `project/src/resolver.rs` | 6 | `crate_root`:54 `resolve`:59 `fault`:79 `write_lockfile`:89 `graph_fault`:238 `sanitize`:249 |
| `project/src/manifest.rs` | 5 | `widen`:175 `entries`:193 `is_commit_id`:202 `parse_version`:455 `parse_requirement`:469 |
| `project/src/git.rs` | 4 | `normalize_url`:30 `is_remote`:41 `source_text`:54 `sync_and_checkout`:68 |
| `project/src/lockfile.rs` | 3 | `quote`:119 `widen`:137 `error`:149 |
| `project/src/lib.rs` | 2 | `find_project_root`:20 `load_project`:34 |
| `solver/src/analyses/test_devices.rs` | 20 | `pin`:19 `term`:23 `partial`:27 `flow`:31 `param`:35 `shared`:41 `module_of`:45 `nodes`:49 `r_shape`:59 `resistor`:95 `c_shape`:105 `capacitor`:141 `bias_shape`:151 `dc_bias`:187 `branch_shape`:197 `wire`:251 `vsrc`:279 `inductor`:308 `quad_shape`:318 `freq_nonlinear`:357 |
| `solver/src/analyses/adapters.rs` | 6 | `instant_error`:56 `digital_error`:64 `empty_digital`:72 `event_net`:401 `accepted`:412 `traces`:419 |
| `solver/src/core/bind.rs` | 5 | `bind_error`:395 `encode`:400 `analog_pins`:511 `create`:551 `bind`:557 |
| `solver/src/analyses/sensitivity.rs` | 4 | `step`:63 `index_refs`:210 `diff_residual`:224 `net_derivatives`:297 |
| `solver/src/analyses/adjoint.rs` | 3 | `restamp_jacobian`:224 `op_vector`:244 `output_index`:263 |
| `solver/src/analyses/pss.rs` | 2 | `dominant_monodromy_magnitude`:392 `solve_dense`:424 |
| `solver/src/core/builder.rs` | 1 | `is_gnd_name`:53 |
| `solver/src/core/net.rs` | 1 | `analog_label`:126 |
| `solver/src/protocol/operating_point.rs` | 1 | `too_many_rounds`:13 |
| `solver/src/protocol/step.rs` | 1 | `observe`:73 |
| `solver/src/protocol/host.rs` | 1 | `host_write`:56 |
| `solver/src/analyses/transient.rs` | 1 | `crossing_estimate`:1085 |
| `solver/src/analyses/ac.rs` | 1 | `operating_vector`:105 |

## Appendix B — Functions and methods over 60 lines (CC-06)

134, language server left out. Length is signature line to closing brace.

| Lines | Location | Function |
|---|---|---|
| 518 | `ir/src/check.rs:1728` | `Checker::expr` |
| 430 | `lang/src/msl/parser/expr.rs:148` | `build` |
| 371 | `solver/src/core/bind.rs:557` | `bind` |
| 292 | `plan/src/plan.rs:553` | `BodyPlan::new` |
| 260 | `analog-solver/src/device/plan.rs:283` | `bind_instance` |
| 241 | `ir/src/print.rs:388` | `Printer::body` |
| 229 | `ir/src/print.rs:928` | `Printer::expr` |
| 213 | `lang/src/elab/lower.rs:302` | `Elaboration::lower_expr_unchecked` |
| 212 | `ir/src/deps.rs:942` | `Walk::expr` |
| 210 | `solver/src/analyses/transient.rs:1117` | `TransientSolver::solve` |
| 207 | `lang/src/elab/compound.rs:301` | `Elaboration::lower_option` |
| 200 | `ir/src/check.rs:294` | `Checker::ids` |
| 192 | `lang/src/msl/parser/pat.rs:7` | `build` |
| 188 | `lang/src/msl/parser/item.rs:351` | `build` |
| 185 | `ir/src/check.rs:1306` | `Checker::code` |
| 185 | `api/src/request.rs:566` | `Keys::spec` |
| 179 | `api/src/session.rs:704` | `Session::analysis` |
| 177 | `solver/src/protocol/transient.rs:317` | `Protocol::transient_loop` |
| 163 | `lang/src/pom/resolve/names.rs:71` | `Names::decl` |
| 161 | `transpile/src/print/library.rs:143` | `LibraryPrinter::body` |
| 159 | `solver/src/analyses/pss.rs:186` | `PssSolver::solve` |
| 152 | `transpile/src/print/limit.rs:70` | `AnalogPrinter::limit` |
| 148 | `digital-solver/src/run.rs:598` | `call_reaction` |
| 144 | `ir/src/deps.rs:597` | `Walk::body` |
| 134 | `transpile/src/print/value.rs:743` | `ValuePrinter::piece` |
| 131 | `lang/src/types/expr.rs:40` | `Checker::check_as` |
| 130 | `ir/src/print.rs:1259` | `Printer::arenas` |
| 130 | `lang/src/msl/parser/expr.rs:665` | `stmt` |
| 129 | `digital-solver/src/instant.rs:115` | `Digital::microsteps` |
| 129 | `ir/src/check.rs:1557` | `Checker::stmt` |
| 123 | `digital-solver/src/run.rs:765` | `run_reactions` |
| 122 | `lang/src/msl/parser/msl.rs:215` | `recover_item` |
| 121 | `lang/src/pom/resolve/names.rs:696` | `Names::expr_inner` |
| 120 | `lang/src/types/call.rs:297` | `Checker::method_call` |
| 120 | `lang/src/msl/lexer.rs:296` | `lex_with_comments` |
| 116 | `lang/src/elab/events.rs:206` | `Elaboration::disjuncts` |
| 113 | `lang/src/msl/parser/ty.rs:169` | `build` |
| 110 | `ir/src/deps.rs:1156` | `Walk::finish` |
| 110 | `analog-solver/src/math/newton_raphson.rs:442` | `NewtonRaphsonSolver::solve_with_strategy` |
| 109 | `transpile/src/print/library.rs:32` | `LibraryPrinter::design` |
| 109 | `lang/src/elab/children.rs:245` | `Elaboration::lower_child` |
| 107 | `transpile/src/print/analog.rs:677` | `AnalogPrinter::terms_function` |
| 106 | `solver/src/analyses/transient.rs:1568` | `TransientSolver::device_attempt` |
| 104 | `lang/src/elab/lower.rs:842` | `Elaboration::lower_path` |
| 104 | `lang/src/types/analog.rs:57` | `Checker::probe` |
| 103 | `lang/src/pom/check.rs:246` | `Refs::decl` |
| 103 | `lang/src/msl/print.rs:529` | `Printer::expr_inner` |
| 102 | `transpile/src/print/analog.rs:270` | `AnalogPrinter::reads` |
| 102 | `digital-solver/src/plan.rs:251` | `analyze` |
| 101 | `ir/src/differentiate.rs:216` | `Differentiation::method` |
| 100 | `solver/src/analyses/adjoint.rs:118` | `AdjointSolver::solve` |
| 99 | `lang/src/types/expr.rs:627` | `Checker::path` |
| 99 | `lang/src/msl/parser/item.rs:95` | `fn_decl` |
| 97 | `abi/src/validate.rs:426` | `cell_bytes` |
| 96 | `lang/src/msl/lexer.rs:704` | `classify_number` |
| 95 | `solver/src/analyses/sensitivity.rs:110` | `ParamStampCollector::collect_device` |
| 95 | `ir/src/check.rs:1151` | `Checker::discrete` |
| 95 | `lang/src/msl/parser/entry.rs:577` | `lex_limited` |
| 94 | `transpile/src/print/types.rs:417` | `TypeItems::enum_carried` |
| 94 | `lang/src/pom/lower/body.rs:161` | `Lowerer::expr_inner` |
| 92 | `ir/src/print.rs:1391` | `Printer::form` |
| 91 | `ir/src/deps.rs:771` | `Walk::run` |
| 90 | `transpile/src/print/shape.rs:106` | `ShapePrinter::prototype` |
| 88 | `digital-solver/src/dag.rs:32` | `Dag::build` |
| 87 | `lang/src/msl/parser/check.rs:346` | `expr_inner` |
| 85 | `project/src/manifest.rs:291` | `Walk::dependency` |
| 84 | `plan/src/digital.rs:756` | `Digital::functions` |
| 84 | `lang/src/types/expr.rs:202` | `Checker::infer_kind` |
| 83 | `api/src/request.rs:372` | `Source::key` |
| 83 | `lang/src/pom/check.rs:99` | `Refs::expr` |
| 83 | `lang/src/elab/lower.rs:518` | `Elaboration::lower_call` |
| 82 | `transpile/src/print/site.rs:232` | `AnalogPrinter::cap` |
| 81 | `ir/src/check.rs:845` | `Checker::types` |
| 81 | `ir/src/print.rs:824` | `Printer::stmt` |
| 80 | `abi/src/validate.rs:524` | `check_function` |
| 80 | `api/src/session.rs:360` | `Session::open_announced` |
| 80 | `lang/src/elab/consts/composite.rs:776` | `ConstEval::run_row` |
| 80 | `lang/src/msl/parser/check.rs:261` | `stmt` |
| 79 | `transpile/src/print/digital.rs:233` | `DigitalPrinter::event` |
| 79 | `abi/src/validate.rs:272` | `validate` |
| 79 | `lang/src/msl/parser/msl.rs:354` | `module_decl` |
| 77 | `lang/src/elab/compound.rs:636` | `Elaboration::lower_option_row` |
| 77 | `lang/src/types/module.rs:242` | `Checker::placement` |
| 76 | `transpile/src/print/site.rs:87` | `AnalogPrinter::stim` |
| 76 | `transpile/src/print/cross.rs:19` | `DigitalPrinter::cross` |
| 76 | `lang/src/pom/to_ast.rs:222` | `Rebuild::expr_inner` |
| 76 | `lang/src/elab/hoist.rs:241` | `Elaboration::hoist` |
| 76 | `interp/src/device.rs:50` | `Interpreted::inputs` |
| 76 | `interp/src/composite.rs:82` | `CompositeForms::walk` |
| 75 | `project/src/manifest.rs:377` | `Walk::package` |
| 75 | `lang/src/elab/elements.rs:342` | `Elaboration::node_run` |
| 74 | `digital-solver/src/run.rs:437` | `relax` |
| 74 | `solver/src/analyses/tf.rs:104` | `TransferFunctionSolver::new` |
| 74 | `solver/src/analyses/tf.rs:379` | `TransferFunctionSolver::calculate_output_resistance` |
| 74 | `lang/src/types/expr.rs:301` | `Checker::binary` |
| 74 | `lang/src/types/expr.rs:773` | `Checker::let_statement` |
| 74 | `analog-solver/src/device/assembly.rs:538` | `Assembly::param_stamps` |
| 73 | `plan/src/netlist.rs:86` | `Netlist::circuit` |
| 73 | `lang/src/pom/index.rs:134` | `build` |
| 72 | `lang/src/elab/body.rs:178` | `Elaboration::lower_body_under_substitution` |
| 72 | `lang/src/types/analog.rs:209` | `Checker::core_call` |
| 72 | `lang/src/types/check.rs:201` | `Checker::new` |
| 71 | `transpile/src/shape_check.rs:31` | `ShapeCheck::entries` |
| 71 | `lang/src/pom/resolve/mod.rs:75` | `run` |
| 71 | `lang/src/elab/place.rs:293` | `Elaboration::bind_params` |
| 70 | `transpile/src/print/value.rs:1119` | `ValuePrinter::stmt` |
| 70 | `lang/src/msl/parser/expr.rs:581` | `block_like` |
| 69 | `lang/src/pom/check.rs:555` | `Design::check_nodes` |
| 68 | `transpile/src/print/digital.rs:97` | `DigitalPrinter::bind` |
| 68 | `solver/src/analyses/sensitivity.rs:224` | `diff_residual` |
| 67 | `solver/src/analyses/noise.rs:219` | `NoiseSolver::solve` |
| 67 | `ir/src/statements.rs:24` | `Statements::of` |
| 66 | `solver/src/analyses/sp.rs:136` | `SpSolver::new` |
| 66 | `lang/src/elab/structural.rs:392` | `Elaboration::classify_key_params` |
| 66 | `analog-solver/src/math/newton_raphson.rs:302` | `NewtonRaphsonSolver::solve` |
| 65 | `solver/src/analyses/tf.rs:274` | `TransferFunctionSolver::calculate_gain` |
| 65 | `lang/src/elab/patterns.rs:63` | `Elaboration::test_pattern` |
| 65 | `interp/src/walk.rs:294` | `Walk::form_name` |
| 64 | `lang/src/elab/functions.rs:16` | `Elaboration::lower_function` |
| 64 | `lang/src/elab/returns.rs:218` | `Elaboration::lower_inlined_option_call` |
| 63 | `transpile/src/print/types.rs:120` | `TypeItems::item` |
| 63 | `digital-solver/src/run.rs:283` | `call_function` |
| 63 | `ir/src/check.rs:578` | `Checker::expr_children` |
| 63 | `api/src/probe.rs:64` | `Probe::resolve` |
| 63 | `lang/src/elab/place.rs:83` | `Elaboration::place_instance` |
| 63 | `lang/src/elab/carriers.rs:217` | `Elaboration::lower_carrier` |
| 63 | `lang/src/elab/compound.rs:527` | `Elaboration::lower_option_method` |
| 63 | `lang/src/elab/records.rs:73` | `Elaboration::lower_emit` |
| 63 | `lang/src/elab/element_commits.rs:45` | `Elaboration::place_target` |
| 63 | `lang/src/types/check.rs:472` | `Checker::lower_type_node` |
| 62 | `lang/src/elab/iterators.rs:181` | `Elaboration::prepare` |
| 62 | `lang/src/types/module.rs:379` | `Checker::connections` |
| 61 | `api/src/request.rs:459` | `Keys::with` |
| 61 | `lang/src/elab/consts.rs:956` | `ConstEval::known_method` |

## Appendix C — The 30 files with most undocumented items (CC-10)

Types, functions, inherent methods, trait method declarations, consts and statics without `///`; fields and variants are not in this count.

| File | undocumented items (types, fns, methods, consts) |
|---|---|
| `abi/src/raw.rs` | 80 |
| `lang/src/pom/reflect/body.rs` | 60 |
| `lang/src/pom/reflect/hardware.rs` | 55 |
| `lang/src/msl/ast.rs` | 53 |
| `ir/src/handle.rs` | 52 |
| `solver/src/core/result.rs` | 46 |
| `lang/src/pom/reflect/items.rs` | 42 |
| `lang/src/msl/print.rs` | 39 |
| `solver/src/analyses/test_devices.rs` | 33 |
| `lang/src/pom/to_ast.rs` | 33 |
| `lang/src/pom/design.rs` | 26 |
| `analog-solver/src/netlist.rs` | 25 |
| `ir/src/model.rs` | 23 |
| `ir/src/print.rs` | 20 |
| `abi/src/validate.rs` | 19 |
| `transpile/src/print/shape.rs` | 18 |
| `ir/src/check.rs` | 18 |
| `transpile/src/rt/real.rs` | 17 |
| `ir/src/differentiate.rs` | 14 |
| `lang/src/msl/parser/entry.rs` | 14 |
| `ir/src/deps.rs` | 13 |
| `lang/src/pom/lower/body.rs` | 13 |
| `lang/src/msl/parser/base.rs` | 13 |
| `analog-solver/src/math/newton_raphson.rs` | 13 |
| `project/src/manifest.rs` | 12 |
| `ir/src/build.rs` | 12 |
| `api/src/plot.rs` | 12 |
| `lang/src/pom/check.rs` | 12 |
| `lang/src/msl/parser/expr.rs` | 12 |
| `lang/src/msl/parser/check.rs` | 12 |

## Appendix D — Every `unwrap` / `expect` call outside tests (rule 9)

| File | n | Lines |
|---|---|---|
| `solver/src/analyses/transient.rs` | 11 | 399, 685, 769, 973, 1178, 1309, 1492, 1770, 1827, 1927, 1939 |
| `ir/src/check.rs` | 10 | 1346, 1357, 1417, 1422, 1425, 1432, 1438, 1485, 1566, 1619 |
| `digital-solver/src/run.rs` | 8 | 294, 295, 296, 308, 615, 616, 625, 843 |
| `solver/src/analyses/noise.rs` | 7 | 327, 369, 385, 387, 469, 497, 517 |
| `solver/src/analyses/pss.rs` | 7 | 200, 210, 222, 234, 236, 278, 369 |
| `analog-solver/src/device/plan.rs` | 7 | 256, 259, 306, 344, 462, 499, 517 |
| `analog-solver/src/math/newton_raphson.rs` | 6 | 348, 359, 365, 520, 543, 549 |
| `parity/src/scenario/trace.rs` | 5 | 72, 77, 87, 106, 138 |
| `parity/src/scenario/planted.rs` | 4 | 25, 26, 27, 28 |
| `solver/src/analyses/adapters.rs` | 4 | 439, 476, 482, 558 |
| `solver/src/analyses/pz.rs` | 4 | 250, 250, 273, 309 |
| `lang/src/msl/parser/entry.rs` | 4 | 388, 557, 558, 560 |
| `analog-solver/src/device/assembly.rs` | 4 | 71, 518, 552, 638 |
| `digital-solver/src/dag.rs` | 3 | 110, 182, 201 |
| `solver/src/core/circuit.rs` | 3 | 253, 254, 330 |
| `lang/src/pom/to_ast.rs` | 3 | 357, 593, 645 |
| `parity/src/scenario/discrete.rs` | 2 | 110, 110 |
| `solver/src/analyses/sensitivity.rs` | 2 | 114, 303 |
| `solver/src/analyses/dc.rs` | 2 | 107, 301 |
| `lang/src/msl/lexer.rs` | 2 | 310, 619 |
| `analog-solver/src/device/flow.rs` | 2 | 57, 74 |
| `analog-solver/src/netlist.rs` | 2 | 254, 256 |
| `parity/src/operating_point.rs` | 1 | 37 |
| `parity/src/deviation.rs` | 1 | 34 |
| `parity/src/host.rs` | 1 | 56 |
| `abi/src/verify.rs` | 1 | 32 |
| `abi/src/device.rs` | 1 | 148 |
| `abi/src/shape.rs` | 1 | 159 |
| `digital-solver/src/plan.rs` | 1 | 525 |
| `solver/src/core/bind.rs` | 1 | 843 |
| `solver/src/analyses/adjoint.rs` | 1 | 229 |
| `solver/src/analyses/sp.rs` | 1 | 101 |
| `solver/src/analyses/sens.rs` | 1 | 85 |
| `solver/src/analyses/ac.rs` | 1 | 128 |
| `solver/src/analyses/tf.rs` | 1 | 229 |
| `solver/src/analyses/context.rs` | 1 | 99 |
| `lang/src/pom/check.rs` | 1 | 482 |
| `lang/src/pom/reflect/crates.rs` | 1 | 32 |
| `analog-solver/src/device/watch.rs` | 1 | 65 |
| `analog-solver/src/tolerances.rs` | 1 | 151 |
| `interp/src/walk.rs` | 1 | 361 |

## Appendix E — Test names defined in more than one place (CC-12)

| Test name | Defined in |
|---|---|
| `its_devices_pass_the_partials_verifier` | `solver/tests/abi_tran.rs`, `solver/tests/sensitivity_fallback.rs`, `solver/tests/checkpoint_restore.rs`, `solver/tests/probe_selection.rs`, … (27 files, all under `solver/tests/`) |
| `a_form_not_run_yet_is_stopped_before_the_run_with_p0001_naming_it` | `tests/book/generics.rs`, `tests/book/types.rs`, `tests/book/language.rs` |
| `a_carried_type_without_repr_is_e0112` | `lang/tests/types_aggregates.rs`, `lang/tests/types_carriers.rs` |
| `a_let_bound_to_a_node_is_a_contribution_target` | `parity/tests/source_templates.rs`, `lang/tests/types_handles.rs` |
| `a_let_bound_to_a_node_probes_the_node` | `parity/tests/source_templates.rs`, `lang/tests/elab_handles.rs` |
| `a_limited_diode_reaches_the_unlimited_operating_point` | `solver/tests/limiting_report.rs`, `analog-solver/tests/assembly_limit.rs` |
| `a_non_finite_value_is_written_as_is` | `transpile/tests/differential.rs`, `interp/tests/terms.rs` |
| `a_panic_in_compiled_code_is_a_fault_and_the_instance_survives` | `transpile/tests/reaction.rs`, `transpile/tests/compiled.rs` |
| `a_simparam_entry_naming_an_option_is_p0010` | `cli/tests/options.rs`, `api/tests/options.rs` |
| `abs_has_the_derivative_of_sign` | `transpile/src/rt/real.rs`, `interp/src/dual.rs` |
| `ac_rc` | `parity/tests/parity_ac.rs`, `tests/scenarios.rs` |
| `an_array_of_carriers_as_a_field_is_driven_and_read_by_index` | `parity/tests/source_bundles.rs`, `lang/tests/types_bundles.rs` |
| `an_unbound_option_param_is_none` | `parity/tests/source_explicit.rs`, `lang/tests/elab_params.rs` |
| `an_unchanged_commit_fires_nothing` | `parity/tests/source_commits.rs`, `transpile/tests/commits.rs` |
| `an_unknown_top_is_one_diagnostic` | `lang/tests/types_entry.rs`, `lang/tests/elab_entry.rs` |
| `an_unsettled_loop_is_e0511_naming_its_drives` | `digital-solver/tests/relax.rs`, `solver/tests/protocol_transient.rs` |
| `delay_tline` | `parity/tests/parity_tran.rs`, `tests/scenarios.rs` |
| `diode_iv` | `parity/tests/parity_dc.rs`, `tests/scenarios.rs` |
| `diode_rev` | `parity/tests/parity_dc.rs`, `tests/scenarios.rs` |
| `diode_rs` | `parity/tests/parity_op.rs`, `tests/scenarios.rs` |
| `diode_series` | `parity/tests/parity_op.rs`, `tests/scenarios.rs` |
| `divider` | `parity/tests/parity_op.rs`, `tests/scenarios.rs` |
| `e0180_a_logic_read_in_analog_is_a_mismatch` | `tests/book/mixed.rs`, `tests/book/analog.rs` |
| `e0404_a_commit_belongs_in_a_reaction` | `tests/book/welcome.rs`, `tests/book/analog.rs` |
| `e0414_a_branch_that_switches_kind_on_a_probe_is_refused` | `tests/book/errors.rs`, `tests/book/analog.rs` |
| `e0901_there_is_no_println` | `tests/book/welcome.rs`, `tests/book/values.rs` |
| `every_fixture_shape_is_valid` | `digital-solver/tests/devices.rs`, `analog-solver/tests/plan.rs` |
| `isrc_r` | `parity/tests/parity_op.rs`, `tests/scenarios.rs` |
| `its_device_passes_the_partials_verifier` | `solver/tests/noise_report.rs`, `solver/tests/truncation.rs` |
| `m_and_dtemp_are_ordinary_names` | `lang/tests/pom_resolve.rs`, `lang/tests/types_implicit.rs` |
| `noise_diode` | `parity/tests/parity_noise.rs`, `tests/scenarios.rs` |
| `noise_rc` | `parity/tests/parity_noise.rs`, `tests/scenarios.rs` |
| `noise_switch` | `parity/tests/parity_noise.rs`, `tests/scenarios.rs` |
| `pss_rc` | `parity/tests/parity_pss.rs`, `tests/scenarios.rs` |
| `pz_lowpass` | `parity/tests/parity_pz.rs`, `tests/scenarios.rs` |
| `sens_divider` | `parity/tests/parity_sens.rs`, `tests/scenarios.rs` |
| `simparam_flag_is_repeatable_and_a_non_number_is_p0010` | `cli/tests/options.rs`, `api/tests/options.rs` |
| `temp_res` | `parity/tests/parity_temp.rs`, `tests/scenarios.rs` |
| `tf_divider` | `parity/tests/parity_tf.rs`, `tests/scenarios.rs` |
| `the_adc_commits_the_rounded_code_after_tconv` | `parity/tests/boundary.rs`, `parity/tests/buck_boundary.rs` |
| `the_comparator_trips_at_its_thresholds` | `parity/tests/boundary.rs`, `parity/tests/buck_boundary.rs` |
| `the_crate_depends_on_the_abi_crate_only` | `digital-solver/tests/boundary.rs`, `analog-solver/tests/boundary.rs` |
| `the_dpwm_dithers_keeps_dead_time_and_never_overlaps` | `parity/tests/buck_dpwm.rs`, `transpile/tests/dpwm.rs` |
| `the_later_statement_wins` | `parity/tests/source_commits.rs`, `transpile/tests/commits.rs` |
| `the_oscillator_toggles_every_half_period` | `parity/tests/buck_dpwm.rs`, `transpile/tests/dpwm.rs` |
| `the_ramp_and_the_pid_follow_their_difference_equations` | `parity/tests/buck_control.rs`, `transpile/tests/pid.rs` |
| `tnom_res` | `parity/tests/parity_op.rs`, `tests/scenarios.rs` |
| `traces_are_recorded_only_for_a_named_stream` | `parity/tests/source_traces.rs`, `api/tests/traces.rs` |
| `tran_corner` | `parity/tests/parity_tran.rs`, `tests/scenarios.rs` |
| `tran_dq` | `parity/tests/parity_tran.rs`, `tests/scenarios.rs` |
| `tran_floor` | `parity/tests/parity_tran.rs`, `tests/scenarios.rs` |
| `tran_jump` | `parity/tests/parity_tran.rs`, `tests/scenarios.rs` |
| `tran_rc` | `parity/tests/parity_tran.rs`, `tests/scenarios.rs` |
| `two_commits_exchange_two_carriers` | `parity/tests/source_commits.rs`, `transpile/tests/commits.rs` |
| `two_runs_give_the_same_changes` | `parity/tests/buck_dpwm.rs`, `transpile/tests/dpwm.rs` |
| `two_statements_make_a_pulse` | `parity/tests/source_commits.rs`, `transpile/tests/commits.rs` |
| `uic_rc` | `parity/tests/parity_tran.rs`, `tests/scenarios.rs` |

## Appendix F — Slowest test executables (CC-12)

**Slowest to run**

| Crate | Target | Tests | Run s |
|---|---|---|---|
| `piperine-numbers` | `transcendental` | 7 | 273 |
| `piperine-api` | `buck_accuracy` | 4 | 95 |
| `piperine-api` | `session` | 31 | 77 |
| `piperine-parity` | `source_nets` | 25 | 69 |
| `piperine-api` | `power_stage_bench` | 1 | 68 |
| `piperine-cli` | `book` | 18 | 61 |
| `piperine-parity` | `deviation` | 1 | 58 |
| `piperine-numbers` | `half` | 2 | 40 |
| `piperine-lang` | `pom_corpus` | 6 | 36 |
| `piperine-parity` | `source_carrier_arrays` | 18 | 35 |
| `(root)` | `integrated` | 42 | 33 |
| `piperine-lang` | `elab_control` | 9 | 29 |
| `piperine-parity` | `source_explicit` | 25 | 29 |
| `piperine-lang` | `msl_format` | 18 | 28 |
| `piperine-cli` | `twins` | 6 | 28 |
| `piperine-parity` | `source_control` | 20 | 27 |
| `piperine-parity` | `source_number_widths` | 9 | 24 |
| `piperine-cli` | `integrated` | 3 | 23 |
| `piperine-parity` | `source_logic` | 25 | 23 |
| `piperine-parity` | `source_generation` | 8 | 21 |
| `piperine-parity` | `source_events` | 28 | 21 |
| `piperine-parity` | `source_aggregates` | 28 | 20 |
| `(root)` | `book_core_std` | 21 | 20 |
| `(root)` | `book_welcome` | 13 | 19 |
| `(root)` | `book_cli` | 19 | 18 |

**Slowest to build** (CPU seconds of the one executable)

| Crate | Target | Build s |
|---|---|---|
| `piperine-solver` | `piperine_solver` (unit tests) | 41 |
| `piperine-lang` | `piperine_lang` (unit tests) | 38 |
| `piperine-analog-solver` | `assembly_ddt` | 36 |
| `piperine-analog-solver` | `boundary` | 33 |
| `piperine-analog-solver` | `assembly_limit` | 33 |
| `piperine-analog-solver` | `assembly_flow` | 25 |
| `piperine-analog-solver` | `assembly_small_signal` | 23 |
| `piperine-analog-solver` | `assembly_noise` | 21 |
| `piperine-analog-solver` | `piperine_analog_solver` (unit tests) | 21 |
| `piperine-numbers` | `integers` | 17 |
| `piperine-solver` | `transient_reentry` | 16 |
| `piperine-solver` | `host_surface` | 16 |
| `piperine-solver` | `protocol_transient` | 15 |
| `piperine-solver` | `step_observer` | 15 |
| `piperine-solver` | `abi_devices` | 14 |
| `piperine-analog-solver` | `plan` | 14 |
| `piperine-api` | `piperine_api` (unit tests) | 14 |
| `piperine-analog-solver` | `assembly_watch` | 13 |
| `piperine-solver` | `protocol_crossings` | 13 |
| `piperine-analog-solver` | `assembly_param` | 12 |
| `piperine-analog-solver` | `noise_numerics` | 12 |
| `piperine-solver` | `parity_baseline` | 12 |
| `piperine-analog-solver` | `assembly_eval` | 12 |
| `piperine-solver` | `landing_rule` | 12 |
| `piperine-analog-solver` | `assembly_delay` | 12 |

## Appendix G — The scan, for the guards

The guards of CC-01, CC-04, CC-05, CC-06 and CC-10 can be the scan of this inventory run as a test, so the numbers here and the guards' numbers are the same by construction:

- parse each `.rs` file with `syn` (already in the dependency graph through `serde_derive`; as a dev-dependency of the guard crate only), with `proc-macro2`'s `span-locations` for lines;
- test code is a file under a `tests/` directory, or an inline `mod` carrying `#[cfg(test)]`; a `#[cfg(test)] mod x;` declaration marks the file `x.rs` as test code too (this inventory did **not** follow such declarations, which is why `solver/src/analyses/test_devices.rs` and the `#[path]` includes in `transpile/src/lib.rs` and `interp/src/lib.rs` appear in the non-test columns);
- a free function is an `ItemFn` not inside an `impl` or a `trait`; allowed when it is `main`, carries `#[test]`, or has an ABI (`extern "C"`);
- a name is checked word by word (split on `_` and on case changes) against a word list plus the glossary;
- a function's length is `block.end.line − sig.start.line + 1`; a closure of 15 lines or more counts as a function;
- an item is documented when it has a `doc` attribute; a method inside `impl Trait for T` is exempt.

