# MSL CLI Specification

**Approved**: 2026-11-15 - user: "Esses + fmt agora", "Adoro a ideia de benches na propria linguagem", "Tabela + plots", "Ao vivo, todos os streams", "Rust por padrão", "Agora, pequeno", "LSP só de diagnósticos agora", "Projeto RC passa-baixa" (the answers to Q1–Q8)

**Amended**: 2026-11-16 - user: "O lang-server é o menos importante, vai ficar para bem depois." The language server (CLI-25, the answer to Q7) leaves this feature; `piperine-lang-server` stays untouched and outside the default build.

The approval and the eight answers reached the planner through the orchestrator on 2026-11-15; the answers are recorded one per row under "Assumptions & Open Questions" and in `context.md`.

## Problem Statement

An MSL project can be run only from a Rust test. `piperine-cli` (43 errors), `piperine-api` (165) and the root package still target the deleted Verilog-AMS front end and the deleted `Element` solver interface; they sit outside the default build and rot. `piperine-project` builds but nothing on the run path uses it. After `ir-rust` and `msl-digital` the whole chain exists (source, type check, elaboration, IR, two backends, solver, records through the sink), and the user asked for the command line next: "Depois desses desenvolvimentos do ir -> transpilation -> abi eu preciso que tu coloque na fila acertar o CLI, project para poder rodar algo completo."

This feature makes `piperine run` execute a complete MSL project, shows `emit!` records as its debug output ("Colocar print (com o emit!) para debug, exemplos completos"), and ends with a Book chapter about the command line ("vamos planejar o cli para colocar no book também?"). With the user's answers it also brings `fmt` on a printer that keeps comments, and plots; and it says in the spec and in the Book that a bench is written in MSL ("Adoro a ideia de benches na propria linguagem ao invés de outra linguagem.").

## Goals

- [ ] `piperine run` on a project prints the result of an operating point or a transient and every `emit!` record, on the Rust backend and on the interpreter.
- [ ] `examples/buck` runs its closed-loop start from the command line: `piperine run startup` in that directory.
- [ ] The seven ngspice twins of `tests/ngspice/` run from the command line on both backends, in all four analyses.
- [ ] `piperine check`, `build`, `test`, `new`, `clean` and `fmt` work on an MSL project; `fmt` keeps every comment.
- [ ] `--plot FILE.svg` writes the result of a sweep or a transient as an SVG.
- [ ] `docs/spec/` and the Book say that a bench (stimulus and monitors) is MSL, and that sweeps, Monte Carlo and optimization stay in the host.
- [ ] `piperine-api`, `piperine-cli`, `piperine-project` and the root package are in the default build with zero warnings; `cargo build` and `cargo test` at the root cover them.
- [ ] The Book has a chapter "The command line" before the cheat sheet, whose examples are real projects and whose output blocks are what the binary prints.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| `add`, `remove`, `tree`, `expand`, `plugin`, plugin scripts, the `-i` REPL | the old code is deleted with the VAMS front end; dependency editing and the plugin system are separate features |
| A Python host, `*_tb.py` testbenches | there is no Python crate (Q7) |
| The language server, in any form, and the VS Code extension | deferred by the user, 2026-11-16: "O lang-server é o menos importante, vai ficar para bem depois." `piperine-lang-server` and `editors/` are not touched; the crate stays a workspace member outside `default-members` and still does not build |
| Formatting options, a configuration file for `fmt`, formatting a range | one canonical layout, as the printer has today |
| A terminal plot, PNG output, interactive plots, more than one chart layout | Q3: one SVG per run, written by hand with no plotting dependency. A terminal plot would put variable-width drawing on a standard output the Book compares byte for byte |
| Reformatting `crates/piperine-lang/std`, the galleries or `examples/buck/src` | the formatter is proven on them; rewriting them is a separate, reviewable change |
| Host stimulus: `drive`, `force`, `release`, waits, sequences, a `set` during a run | the stimulus of a CLI run is an MSL bench module; the host API for sequences comes with the host library |
| Monte Carlo, optimization, sensitivities, noise, transfer function, pole-zero, periodic steady state, S-parameters from the CLI | the solver has them; the CLI asks for `op`, `dc`, `ac` and `transient` only. Each further analysis is a table key away, added when a design needs it |
| ngspice raw files | Q3 |
| `emit!` inside `analog` (trace records) | `msl-digital` lowers `emit!` in reactions only; in `analog` it is still `P0001`. A trace is shown the day the front end lowers it, with no CLI change |
| ngspice comparison from the CLI, moving the twins, a root `tests/e2e/` | queue item 4. This feature runs the twins; it does not compare them again |
| Replacing `piperine-parity`'s `Bench` and `Project` by `piperine-api` | queue item 4 decides where the harness stands |
| The scenarios S1 to S9 of the buck's own spec | queue item 4. This feature runs the one closed-loop start `msl-digital` proves |
| Workspaces, `--locked`, `--offline`, `piperine update` | not needed to run a project; the resolver already works offline when the lockfile and the checkouts are present |
| Shipping `rustc` with the binary | decision C5 of the codegen record: later |
| Publishing the Book as an artifact | the orchestrator's step after the merge |

---

## Assumptions & Open Questions

Rows `Q1` to `Q8` are the user's answers of 2026-11-15, in the user's words. The other rows are the planner's technical choices, open to correction.

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| Q1 Command set | `check`, `build`, `run`, `test`, `new`, `clean` and `fmt`; `fmt` stands on a formatter that keeps plain comments, built in this feature, and has `--check` | user decision | y - user: "Esses + fmt agora" |
| Q2 Where a run is described | `[run.<name>]` in `Piperine.toml`; each key is also a flag; flags override keys; a lone file runs with flags | user decision (option a) | y - user: "Adoro a ideia de benches na propria linguagem ao invés de outra linguagem." |
| Where the stimulus lives | an MSL bench: a file under `benches/`, a crate of its own depending on the package and on the package's dependencies | the mechanism `msl-digital` already runs the buck with; it keeps stimulus out of the library other packages consume | y - user: "Adoro a ideia de benches na propria linguagem ao invés de outra linguagem." |
| Q3 Output of `run` | header line, records live, aligned text table; `--csv FILE`; and plots | user decision | y - user: "Tabela + plots" |
| The form of a plot | `--plot FILE.svg` (key `plot`): one SVG, x the time, the swept param or the frequency, one path per printed real signal on a shared axis, one lane per discrete signal under it; an `ac` run gets a magnitude panel in dB over a phase panel in degrees, x logarithmic with `per_decade`. Written by hand, no plotting crate. It draws every accepted step, whatever `print_step` is. An `op` run has nothing to plot: `P0010`. No terminal plot | the orchestrator asked the planner to decide the form; an SVG opens in any browser and editor and is checked structurally by a test | y - planner, inside Q3 |
| Q4 Record display | every stream, live, one line each; `--stream`; `--records FILE` as JSON Lines | user decision (option a) | y - user: "Ao vivo, todos os streams" |
| Q5 Default backend | `rust`; `--backend`, then `PIPERINE_BACKEND`, then the run's `backend` key | user decision (option a) | y - user: "Rust por padrão" |
| Q6 `piperine-api` | rewritten small, now; the old content deleted | user decision (option a) | y - user: "Agora, pequeno" |
| Q7 Language server | not in this feature. `piperine-lang-server` stays untouched and outside the default build; Python stays later too | user decision of 2026-11-16, which supersedes the answer of 2026-11-15 ("LSP só de diagnósticos agora") | y - user: "O lang-server é o menos importante, vai ficar para bem depois." |
| Q8 Template of `new`, first Book example | the RC low-pass project `examples/rc`; the buck is the Book's second example | user decision (option a) | y - user: "Projeto RC passa-baixa" |
| How `fmt` keeps comments | the lexer gives plain comments as a list beside the tokens; the tree, the parser and the POM do not change. The formatter anchors each comment to the item, member, statement or list element it stands before, after (same line) or inside (an empty or ending block), and prints it there. A statement holding a comment no anchor reaches is written as it is in the source, re-indented. One blank line between two statements or members is kept when the source has one or more. `fmt` refuses to write a file whose comments would not all come out, in order | the smallest change that cannot lose a comment; rustfmt's own rule for what it cannot place | y - planner, inside Q1 |
| What `fmt` formats | every `.msl` under `src/` and `benches/` of the package, or the files named; in place | Cargo's rule | y - planner, inside Q1 |
| Where the spec says stimulus is host-only | amended: a **bench** is an MSL module without ports that places the design with its sources, stimulus and monitors; the host picks it as the top and asks the analysis. Sweeps, Monte Carlo, optimization, regression, `force` and interactive sequences stay in the host | user decision (Q2), relayed by the orchestrator with the instruction to amend the text | y - user: "Adoro a ideia de benches na propria linguagem ao invés de outra linguagem." |
| Manifest rule | the manifest section of `docs/spec/` (rule 3) gains one table, `[run]`, whose content the language does not define; a dependency's `[run]` is not read | the manifest is the package's one file; the language still owns only `[package]` and `[dependencies]` | y - planner, inside Q2 |
| Who reads `[run]` | `piperine-project` accepts the table and hands it over uninterpreted with its spans; `piperine-api` reads it | MD-58: the project crate owns the file, not the host's meaning | y - planner |
| Codes of the tool's own errors | `P0010`: a run request that is malformed (a key of the wrong type, an unknown key, a missing or contradictory number, a bench file that does not exist). `P0011`: no top can be chosen. Host errors keep the spec's codes: `E0801` for a path that names nothing, `E0802` for a refused `set` | `P00xx` is the tool-owned range (`P0001` to `P0004`); a path and a `set` are host actions the spec already codes | y - planner |
| Choice of the top | the `top` key, else the one module without ports of the root crate of the run (the bench file's crate, or the package's crate when the run has no bench); none or several is `P0011` listing the candidates | G.17: the choice is the host's; this is the rule `piperine-parity` uses today | y - planner |
| A bench's crate | named `bench`; it depends on the package under the package's name and on each of the package's direct dependencies under their names | Cargo's rule for an integration test | y - planner |
| A lone file | `piperine run <file>.msl` with flags. When a manifest is found upward from the file's directory and the file is not under that package's `src/`, the file is a bench of that package; with no manifest it is a crate alone. A file under `src/` is refused: it is part of the crate | the twins have no manifest; a bench is run the same way with or without a table | y - planner |
| Numbers in a run table | TOML integers and floats only (`stop = 2e-3`); no SI-suffix strings | no second number syntax in a second file format | y - planner |
| Analyses | `op`, `dc`, `ac`, `transient` | what the parity suite proves on both backends | y - planner |
| DC sweep | `sweep` is a param path; points are `from + k*step` while not past `to`; each point is a `set` followed by an operating point | how `parity_dc` runs it today | y - planner |
| AC grid | `per_decade` (logarithmic, ngspice `dec`) or `points` (linear, total), exactly one | ngspice's two common grids | y - planner |
| Transient rows | one row per accepted step; with `print_step`, one row per multiple of it up to `stop`, continuous values interpolated linearly and discrete values held; with `from` (optional, ngspice's `tstart`), only the steps at or after it are kept and printed, so a long mixed run holds a bounded result (T1, contract D10) | ngspice's print grid; a 400 000-step run must be printable | y - planner |
| What can be printed | a node (its potential), a discrete net (its value), `i(<instance>)` (the flow of the instance's first potential branch); paths are relative to the top or written in full | the three things `piperine-parity` reads today | y - planner |
| Default `print` | the top module's own signals and ports, in declaration order | small, predictable, never empty for a bench | y - planner |
| Number format | reals as `{:.6e}` (`7.500000e0`); `bool` as `true`/`false`; integers in decimal; an unknown value as `x`; a text field in double quotes | fixed width, exact, stable across platforms | y - planner |
| AC columns | `mag(<path>)` and `phase(<path>)`, phase in degrees | what an engineer reads off a Bode plot | y - planner |
| Exit codes | 0 done; 1 the design, the run or a test failed; 2 the command line is wrong | Cargo's and clap's convention | y - planner |
| Where output goes | results and records on standard output; diagnostics and progress on standard error | a pipe gets data only | y - planner |
| Standard output is deterministic | no wall-clock time, no absolute path, no cache key on standard output | the Book's output blocks are compared with the binary byte for byte | y - planner |
| `test` verdict | a run passes when it completes and no `checks` record has `pass` false or unknown; a run with no check passes and says so | the spec: an unknown `pass` is not a pass | y - planner |
| `check` without a run | parse and resolve only, and it says that type checking was skipped | type checking starts from a top, and a library has none | y - planner |
| `clean` | removes `<project>/target`; `--cache` also removes the compiled-design cache directory | the checkouts are per project, the cache is per user | y - planner |
| Argument parser | `clap` with its builder interface, no derive | AGENTS.md rule 5: no macros | y - planner |
| The old tests of `piperine-api` and `piperine-cli` | deleted with the code they test | they test an interface that no longer exists and do not compile; this is the deletion Q6 approved, not a weakened test. | y - user: "Agora, pequeno" |
| `examples/Piperine.toml`, `examples/Piperine.lock` | deleted | a manifest in the old `[project]` format that names no package | y - planner |
| The buck's `startup` run | the scenario of `msl-digital` DIG-16: setpoint code 0, `en` raised, Vin 12 V, a 6.6 Ω load, 600 µs; the reference ends at 128. `idle` is DIG-15's: the converter compiled, `en` low, mode `Off` (0) | the one closed-loop start that feature proves; this feature adds no new scenario. DIG-16 is P2 there: if `msl-digital` merges without it, T1 reports it and CLI-08 AC 2, 3 and 5 wait, carried meanwhile by `idle` | y - planner |
| Order | starts after `msl-digital` merges; task T1 reads the merged code and records every name this design takes from its design text | the record decoder and the bench forms depend on it | y - planner |

**Open questions:** none - all resolved or logged above.

**Implicit-requirement dimensions.** Input validation and bounds: CLI-03, CLI-22 AC 8, CLI-23 AC 9, CLI-09 AC 3 and 5, CLI-10 AC 2 and 3, CLI-17 AC 3 and 4. Failure and partial-failure states: CLI-04 AC 4 to 6, CLI-05 AC 6, CLI-07 AC 4 to 7, CLI-11 AC 3, CLI-16 AC 3 and 5. Idempotency and retry: CLI-23 AC 3 (formatting twice), CLI-22 AC 10, CLI-15 AC 2 (a second build compiles nothing), CLI-17 AC 3 (`new` never overwrites), CLI-18 AC 3. Auth boundaries and rate limits: N/A because everything is local and single-user. Concurrency and ordering: CLI-06 AC 3 (record order is the sink's); two `piperine` processes on one cache are safe by the transpiler's cache (its rename-into-place rule), not by this feature. Data lifecycle: CLI-18 (what `clean` removes); CLI-11 AC 2 and CLI-12 AC 4 (an output file is replaced, never appended). Observability: CLI-06, CLI-15 AC 3, and progress on standard error (CLI-07 AC 8). External-dependency failure: CLI-04 AC 5 (`rustc` missing), CLI-02 AC 6 (a git dependency that cannot be fetched keeps its package code). State-transition integrity: N/A because a run is one pass from source to result; a `set` is made only before the analysis (CLI-10).

---

## User Stories

### P1: The host crates build again ⭐ MVP

**User Story**: As a contributor, I want the host crates in the default build so that `cargo build` and `cargo test` at the root prove the command line.

**Why P1**: nothing else can be tested while the crates do not compile.

**Acceptance Criteria**:

1. The system SHALL build `piperine-project`, `piperine-api`, `piperine-cli` and the root package `piperine` as default members with zero warnings.
2. The system SHALL keep the binary named `piperine` in `piperine-cli`.
3. The system SHALL keep the root package a re-export of `piperine-api` with no code of its own.
4. The system SHALL hold no source file of `piperine-api` or `piperine-cli` that names the deleted front end or the deleted `Element` interface.
5. WHEN `piperine --help` runs THEN the system SHALL list exactly the commands `check`, `build`, `run`, `test`, `new`, `clean` and `fmt`, and exit 0.
6. WHEN `piperine --version` runs THEN the system SHALL print `piperine` and the workspace version, and exit 0.
7. IF a command or a flag is not one the system defines THEN the system SHALL print the usage error on standard error and exit 2.

**Independent Test**: `cargo build` at the root, then `target/debug/piperine --help`.

---

### P1: A project is found and loaded ⭐ MVP

**User Story**: As a designer, I want to run commands anywhere inside my project so that I never name its root.

**Why P1**: every command starts here.

**Acceptance Criteria**:

1. WHEN a command runs in a directory under a package THEN the system SHALL use the nearest `Piperine.toml` found upward from the working directory.
2. WHEN `--project <dir>` is given THEN the system SHALL use the package at `<dir>` and SHALL NOT search upward.
3. IF no manifest is found and the command needs a project THEN the system SHALL report `E0170` on standard error and exit 1.
4. WHEN a manifest holds a `[run]` table THEN the system SHALL accept the manifest, and `piperine-project` SHALL give the table's text spans to its caller without reading its keys.
5. WHEN a dependency's manifest holds a `[run]` table THEN the system SHALL ignore that table.
6. IF the package or a dependency fails a manifest rule THEN the system SHALL render each package error with its code, file and source line on standard error and exit 1.
7. WHEN a run names a bench file THEN the system SHALL load a crate graph whose root crate is that file, named `bench`, depending on the package under the package's name and on each direct dependency of the package under its name.
8. WHEN a run names no bench THEN the system SHALL use the package's own crate as the root crate.
9. WHEN `piperine run <file>.msl` names a file with no manifest upward from the file's directory THEN the system SHALL load that file as a crate alone.
10. WHEN `piperine run <file>.msl` names a file under a package and outside its `src/` THEN the system SHALL load that file as a bench of that package.
11. IF `piperine run <file>.msl` names a file under a package's `src/` THEN the system SHALL refuse with `P0010`, saying the file is part of the crate, and exit 1.

**Independent Test**: `piperine check` from `examples/rc/benches/` finds `examples/rc/Piperine.toml`.

---

### P1: A run request ⭐ MVP

**User Story**: As a designer, I want to describe a run once in the manifest and adjust it with flags so that a run is repeatable.

**Why P1**: the run command reads nothing else.

**Acceptance Criteria**:

1. The system SHALL read from `[run.<name>]` the keys `bench`, `top`, `analysis`, `stop`, `max_step`, `print_step`, `sweep`, `from`, `to`, `step`, `per_decade`, `points`, `print`, `streams`, `params`, `backend`, `csv`, `records` and `plot`, and no other.
2. The system SHALL accept each of those keys as a flag of `run`: `--bench`, `--top`, `--analysis`, `--stop`, `--max-step`, `--print-step`, `--sweep`, `--from`, `--to`, `--step`, `--per-decade`, `--points`, `--print` (repeatable), `--stream` (repeatable), `--param <path>=<value>` (repeatable), `--backend`, `--csv`, `--records` and `--plot`.
3. WHEN a flag and a key of the named run give the same setting THEN the system SHALL use the flag.
4. IF a run table holds a key outside the list, or a value of the wrong type, THEN the system SHALL report `P0010` at that key's span in `Piperine.toml` and exit 1.
5. IF `analysis` is absent or is not one of `op`, `dc`, `ac`, `transient` THEN the system SHALL report `P0010` naming the four values.
6. IF a `transient` run has no `stop`, or `stop`, `max_step` or `print_step` is not greater than 0, THEN the system SHALL report `P0010` naming the key.
7. IF a run gives a key that its analysis does not read THEN the system SHALL report `P0010` naming the key and the analysis.
8. IF `bench` names a file that does not exist THEN the system SHALL report `P0010` at the value's span.
9. WHEN `piperine run` is given no name and the project has exactly one run THEN the system SHALL run it.
10. IF `piperine run` is given no name and the project has no run or several THEN the system SHALL list the run names on standard error and exit 2.
11. IF `piperine run <name>` names no run of the project THEN the system SHALL list the run names on standard error and exit 2.
12. IF a run is asked from flags alone with no `--analysis` THEN the system SHALL exit 2 naming the flag.

**Independent Test**: a manifest with `stopp = 1e-3` under a run fails `piperine check` with `P0010` pointing at `stopp`.

---

### P1: A session ⭐ MVP

**User Story**: As a designer, I want my top module compiled onto a backend so that an analysis can run on it.

**Why P1**: the chain from source to a circuit, in one call.

**Acceptance Criteria**:

1. WHEN a run gives `top` THEN the system SHALL elaborate the module of that name in the root crate.
2. WHEN a run gives no `top` and the root crate holds exactly one module without ports THEN the system SHALL elaborate it.
3. IF a run gives no `top` and the root crate holds no module without ports or several THEN the system SHALL report `P0011` listing the candidates and exit 1.
4. IF parsing, resolution, type checking or elaboration reports a diagnostic THEN the system SHALL render every diagnostic with its code, file, line and source excerpt on standard error, name the stage, and exit 1.
5. IF the Rust backend is chosen and no `rustc` is found THEN the system SHALL print the transpiler's error, which names `rustc` and the interpreter, and exit 1.
6. IF the interpreter is chosen and the design holds a form it does not run THEN the system SHALL print the rejection naming the body and `--backend rust`, and exit 1.
7. WHEN `--backend` is given THEN the system SHALL use it; otherwise WHEN `PIPERINE_BACKEND` is `interp` or `rust` THEN the system SHALL use that; otherwise WHEN the run has a `backend` key THEN the system SHALL use it; otherwise the system SHALL use `rust`.
8. IF `--backend`, `PIPERINE_BACKEND` or `backend` holds another value THEN the system SHALL name the two valid values and exit 2 for the flag and 1 otherwise.
9. The system SHALL compile a design at most once per `run` and SHALL NOT invoke `rustc` during an analysis.

**Independent Test**: `Session::open` on `tests/ngspice/divider.msl` returns a circuit on both backends.

---

### P1: Operating point and transient results ⭐ MVP

**User Story**: As a designer, I want the numbers of my signals so that I can see what the circuit does.

**Why P1**: the first results a user asks for.

**Acceptance Criteria**:

1. WHEN an `op` run completes THEN the system SHALL give a table with the columns `signal` and `value` and one row per printed signal, in `print` order.
2. WHEN a `transient` run completes with no `print_step` THEN the system SHALL give a table whose first column is `time` and which has one row per accepted step and one column per printed signal.
3. WHEN a run has no `print` THEN the system SHALL print the top module's own signals and ports, in declaration order.
4. The system SHALL accept in `print` a node path, a discrete net path and `i(<instance path>)`, each written in full or relative to the top.
5. The system SHALL name a column by the full path of its signal, `i(<full instance path>)` for a flow.
6. IF a `print` entry names nothing in the elaborated design THEN the system SHALL report `E0801` naming the entry before any analysis runs, and exit 1.
7. WHEN the divider of `tests/ngspice/divider.msl` runs `op` THEN the system SHALL give 7.5 for `Divider.mid`.
8. WHEN the RC of `tests/ngspice/tran_rc.msl` runs `transient` to 10e-6 THEN the system SHALL give for `TranRc.out` at the first row at or after 1.5915494309189535e-6 a value within 1e-3 of `1 - exp(-t/1.5915494309189535e-6)` at that row's time.
9. IF the solver fails an analysis THEN the system SHALL print the solver's error and the analysis on standard error, print no table, and exit 1.
10. The system SHALL render a real as `{:.6e}`, a `bool` as `true` or `false`, an integer in decimal and an unknown value as `x`.
11. (Amended 2026-11-17, verification F2.) IF a `transient` run's last step is before its `stop` (the solver does not step a design with no analog node) THEN the system SHALL fail the run naming that cause, give no table, CSV or plot, keep the records already written, and exit 1.

**Independent Test**: `piperine run tests/ngspice/divider.msl --analysis op` prints `Divider.mid  7.500000e0`.

---

### P1: Records from `emit!` ⭐ MVP

**User Story**: As a designer, I want `emit!` to print so that I can debug a reaction the way I debug a program.

**Why P1**: the user's words: "Colocar print (com o emit!) para debug".

**Acceptance Criteria**:

1. WHEN a reaction executes `emit!("s", a = x, b = y)` during a run THEN the system SHALL deliver one record with the time, the instance path of the site, the stream name `s` and the fields `a` and `b` in source order with their values.
2. The system SHALL print a record as one line: `@<time> <instance path> <stream> <field>=<value> ...`, fields separated by one space.
3. The system SHALL print records in the order the solver's sink delivers them, each before any record of a later time.
4. WHEN a field is a text literal THEN the system SHALL print it in double quotes with the text the source gives.
5. WHEN a field's value is unknown THEN the system SHALL print `x` for it.
6. WHEN a `require` of the standard library fails in a reaction THEN the system SHALL print one record of the stream `checks` with `pass=false`.
7. The system SHALL print every record line of an analysis before the result table of that analysis.
8. WHEN a design holds no `emit!` site THEN the system SHALL print no record line.
9. (Amended 2026-11-16, user: "Só falhas por padrão (Recommended)".) The system SHALL NOT print, on the terminal, a record of the stream `checks` whose `pass` is true, unless `all_checks = true` or `--all-checks` is given; a `checks` record whose `pass` is false or unknown always prints, and every other stream prints as before. The filter is for the terminal only: `records = FILE` keeps every record.

**Independent Test**: the bench of `examples/rc` prints one `trip` record when the output crosses 0.5 V.

---

### P1: `piperine run` ⭐ MVP

**User Story**: As a designer, I want one command that runs my project and shows what happened.

**Why P1**: the vertical slice: "rodar algo completo".

**Acceptance Criteria**:

1. WHEN `piperine run <name>` completes THEN the system SHALL print on standard output, in this order: the header line `run <name>: <analysis>, top <Top>, backend <backend>`, the record lines, one empty line, and the result table.
2. The system SHALL print a text table as one header row and one row per data row, cells left-aligned, each column padded to its widest cell, columns separated by two spaces, no trailing spaces.
3. WHEN `piperine run` completes THEN the system SHALL exit 0.
4. IF any stage before the analysis fails THEN the system SHALL print nothing on standard output and exit 1.
5. IF the analysis fails after records were printed THEN the system SHALL keep the records printed, print the error on standard error and exit 1.
6. The system SHALL print no wall-clock time, no absolute path and no cache key on standard output.
7. WHEN the run is a lone file THEN the system SHALL print the file's stem as the run name in the header.
8. WHILE the Rust backend compiles a design that is not cached, the system SHALL print one line on standard error saying that it compiles and which run.

**Independent Test**: in `examples/rc`, `piperine run step` prints the header, one `trip` record and the table.

---

### P1: The buck runs from the command line ⭐ MVP

**User Story**: As the author of the buck, I want `piperine run startup` so that the flagship example runs with one command.

**Why P1**: the queue's definition of done for this item.

**Acceptance Criteria**:

1. The system SHALL hold in `examples/buck` a bench `benches/startup.msl` and the runs `idle` and `startup` in its `Piperine.toml`.
2. WHEN `piperine run startup` runs in `examples/buck` THEN the system SHALL complete on the Rust backend and print, in the last row of its table, the value 128 for the controller's reference.
3. WHEN `piperine run startup` runs in `examples/buck` THEN the system SHALL print no `checks` record with `pass=false` (a passing check is hidden by default, CLI-06 AC 9, so no `checks` line is printed at all), and `--records FILE` SHALL keep more than 1000 passing checks of the DPWM's monitor, so the claim is not vacuous.
4. WHEN `piperine run idle` runs in `examples/buck` THEN the system SHALL complete and print the supervisor's mode as 0 in every row.
5. WHEN `piperine test` runs in `examples/buck` THEN the system SHALL exit 0.
6. (Amended 2026-11-16.) WHEN `piperine run idle` or `piperine run startup` runs in `examples/buck` with no flag THEN the system SHALL print at most 16 lines on standard output.

**Independent Test**: `cd examples/buck && piperine run startup`.

---

### P2: DC sweep and AC

**User Story**: As a designer, I want sweeps so that I can draw an I-V curve and a frequency response.

**Why P2**: the other two analyses the parity suite proves; the MVP stands without them.

**Acceptance Criteria**:

1. WHEN a `dc` run completes THEN the system SHALL give a table whose first column is the swept param's full path and which has one row per point `from + k*step`, for every `k` from 0 while the point is not past `to`.
2. WHEN an `ac` run completes THEN the system SHALL give a table whose first column is `freq` and which has, per printed signal, the columns `mag(<path>)` and `phase(<path>)` with the phase in degrees.
3. IF a `dc` run lacks `sweep`, `from`, `to` or `step`, or `step` is 0 or points away from `to`, THEN the system SHALL report `P0010` naming the key.
4. WHEN an `ac` run gives `per_decade = n` THEN the system SHALL sweep logarithmically with `round(n * log10(to / from)) + 1` points from `from` to `to`.
5. IF an `ac` run gives both `per_decade` and `points`, or neither, or `from` is not greater than 0, or `to` is less than `from`, THEN the system SHALL report `P0010` naming the keys.
6. WHEN an `ac` run gives `points = n` THEN the system SHALL sweep linearly with `n` points from `from` to `to`.
7. WHEN the RC of `tests/ngspice/ac_rc.msl` runs `ac` from 1e3 to 1e7 with 10 points per decade THEN the system SHALL give at 1e5 a magnitude within 1e-3 of 0.7071067811865476 and a phase within 1e-2 degrees of -45 for `AcRc.out`.
8. WHEN the diode of `tests/ngspice/diode_iv.msl` sweeps `v1.dc` from -1 to 0.8 by 0.05 THEN the system SHALL give 37 rows.
9. IF `sweep` names no param of the elaborated design THEN the system SHALL report `E0801` naming it and exit 1.
10. (Amended 2026-11-17, verification F1.) IF a `dc` run's design holds a record stream THEN the system SHALL refuse the run with `P0010` naming the streams before anything compiles, and exit 1, so `piperine test` counts it `FAIL <name> (did not run)`: a point of a sweep has no time to stamp its records with `@<time>`.

**Independent Test**: `piperine run tests/ngspice/ac_rc.msl --analysis ac --from 1e3 --to 1e7 --per-decade 10 --print out`.

---

### P2: Params

**User Story**: As a designer, I want to set a param for one run so that I can try a value without editing the source.

**Why P2**: cheap, and the DC sweep already needs the same write.

**Acceptance Criteria**:

1. WHEN a run gives `params = { "<path>" = <value> }` or `--param <path>=<value>` THEN the system SHALL set each param before the analysis, in the order given, the table's entries before the flags'.
2. IF a param path names nothing THEN the system SHALL report `E0801` naming it and exit 1.
3. IF the design refuses a value THEN the system SHALL report `E0802` naming the path and the value, run no analysis, and exit 1.
4. WHEN the divider runs `op` with `--param r2.r=1000` THEN the system SHALL give 5 for `Divider.mid`.

**Independent Test**: the fourth criterion, from the command line.

---

### P2: Output formats

**User Story**: As a designer, I want the table in a file a spreadsheet or a script reads.

**Why P2**: the table on the terminal is enough for the MVP.

**Acceptance Criteria**:

1. WHEN `csv` or `--csv <file>` is given THEN the system SHALL write the table to that file as CSV, one header row with the column names, cells separated by `,`, the numbers in the same format as the text table, and print `wrote <file> (<n> rows)` in place of the table.
2. WHEN the CSV file exists THEN the system SHALL replace it.
3. IF the CSV file cannot be written THEN the system SHALL print the error naming the path and exit 1.
4. WHEN a `transient` run gives `print_step = h` THEN the system SHALL give one row per `k*h` for every `k` from 0 while `k*h` is not past `stop`; WHEN the run also gives `from` THEN the rows SHALL start at the first `k*h` not before `from` (T1, contract D10).
5. WHILE a print grid is in force the system SHALL give a continuous signal its value interpolated linearly between the two accepted steps around the row's time, and a discrete signal the value of the last accepted step at or before it.
6. The system SHALL resolve a relative `csv` or `records` path against the project directory, and against the working directory for a lone file.

**Independent Test**: `piperine run step --csv out.csv` in `examples/rc`, then read `out.csv`.

---

### P2: Record filter and record file

**User Story**: As a designer debugging one block, I want only its stream, or all of them in a file I can filter.

**Why P2**: live printing of everything is the MVP.

**Acceptance Criteria**:

1. WHEN `streams` or `--stream` is given THEN the system SHALL print only the records whose stream name, or whose `<instance path>.<stream>`, equals one of the entries.
2. IF a `streams` entry matches no stream of the elaborated design THEN the system SHALL report `E0801` naming it before any analysis runs, and exit 1.
3. WHEN `records` or `--records <file>` is given THEN the system SHALL write every kept record to that file as one JSON object per line with the keys `time`, `instance`, `stream` and `fields`, and print no record line.
4. WHEN the records file exists THEN the system SHALL replace it.
5. The system SHALL write in a record's `fields` a real as a JSON number, a `bool` as a JSON boolean, an integer as a JSON number, a text as a JSON string and an unknown value as `null`.

**Independent Test**: `piperine run step --stream trip` prints the `trip` record and no other.

---

### P2: The twins run from the command line

**User Story**: As a maintainer, I want every parity twin to run through the binary so that the user's entry point is proven on the circuits ngspice already agrees with.

**Why P2**: it widens the proof; the MVP already runs two twins.

**Acceptance Criteria**:

1. WHEN each of `divider`, `isrc_r`, `diode_series` runs `op` from the command line THEN the system SHALL exit 0 on the interpreter and on the Rust backend.
2. WHEN each of `diode_iv`, `diode_rev` runs its `dc` sweep, `ac_rc` its `ac` sweep and `tran_rc` its `transient` from the command line THEN the system SHALL exit 0 on the interpreter and on the Rust backend.
3. The system SHALL print byte-equal standard output on the two backends for each of the seven twins, the header line aside.

**Independent Test**: the e2e target that runs the fourteen commands.

---

### P2: `piperine check`

**User Story**: As a designer, I want my sources checked without running anything so that I get errors fast.

**Why P2**: `run` already reports the same errors.

**Acceptance Criteria**:

1. WHEN `piperine check` runs on a project THEN the system SHALL resolve the package graph, parse and resolve every crate, load every bench of every run, and type-check and elaborate the top of every run, invoking no backend.
2. WHEN every stage passes THEN the system SHALL print `ok: <package> (<c> crates, <r> runs)` and exit 0.
3. IF any stage reports a diagnostic THEN the system SHALL render every diagnostic of every run, print no `ok` line, and exit 1.
4. WHEN the project has no run THEN the system SHALL parse and resolve only and print `ok: <package> (<c> crates, 0 runs; type checking needs a run)`.
5. WHEN `piperine check <file>.msl` names a lone file THEN the system SHALL check that file's crate with the top rule of a session.

**Independent Test**: break a type in `examples/rc/src/lib.msl` and read `P0002` with the file and line.

---

### P2: `piperine build`

**User Story**: As a designer, I want to compile ahead and read the generated Rust so that the first run is fast and I can see what runs.

**Why P2**: `run` compiles on demand.

**Acceptance Criteria**:

1. WHEN `piperine build` runs THEN the system SHALL compile the top of every run on its backend and print one line per run: `built <name>: compiled`, `built <name>: cached`, or `built <name>: interp, nothing to compile`.
2. WHEN `piperine build` runs twice on an unchanged project THEN the system SHALL print `cached` for every Rust run the second time and invoke the compiler for no design.
3. WHEN `--verbose` is given THEN the system SHALL print, after each compiled or cached line, a line with the path of the generated Rust source, and that file SHALL exist.
4. IF a run fails any stage THEN the system SHALL render its error, go on with the other runs, and exit 1.

**Independent Test**: `piperine build --verbose` in `examples/rc`, then open the printed path.

---

### P2: `piperine test`

**User Story**: As a designer, I want one command that says whether every `require` of my project holds.

**Why P2**: `run` shows the failing records already.

**Acceptance Criteria**:

1. WHEN `piperine test` runs THEN the system SHALL execute every run of the project in manifest order, or only the runs named as arguments.
2. WHEN a run completes and no `checks` record has `pass` false or unknown THEN the system SHALL print `PASS <name> (<n> checks)`.
3. IF a run completes with a `checks` record whose `pass` is false or unknown THEN the system SHALL print `FAIL <name> (<k> of <n> checks failed)` followed by the line of each such record.
4. The system SHALL print no result table and no other record under `test`.
5. IF a run does not complete THEN the system SHALL print `FAIL <name> (did not run)` on standard output and the error on standard error.
6. The system SHALL print the last line `<r> runs, <p> passed, <f> failed` and exit 0 when `f` is 0 and 1 otherwise.
7. IF the project has no run THEN the system SHALL print `0 runs, 0 passed, 0 failed` and exit 0.

**Independent Test**: a fixture project whose bench holds a failing `require` exits 1 with one `FAIL` line and the record.

---

### P2: `piperine new`

**User Story**: As a newcomer, I want a project that runs so that my first command shows something.

**Why P2**: a project can be written by hand.

**Acceptance Criteria**:

1. WHEN `piperine new <name>` runs THEN the system SHALL create the directory `<name>` holding `Piperine.toml`, `src/lib.msl`, `benches/step.msl` and `.gitignore`, and print `created <name>`.
2. The system SHALL write files equal to those of `examples/rc`, the package name replaced by `<name>`.
3. IF the directory `<name>` exists THEN the system SHALL write nothing and exit 1 saying so.
4. IF `<name>` is not a package name the manifest rule allows THEN the system SHALL write nothing and exit 1 stating the rule.
5. WHEN `piperine check`, `piperine run step` and `piperine test` run in a project `new` created THEN each SHALL exit 0.

**Independent Test**: `piperine new demo && cd demo && piperine run step`.

---

### P2: `piperine clean`

**User Story**: As a designer, I want to remove what the tool wrote.

**Why P2**: housekeeping.

**Acceptance Criteria**:

1. WHEN `piperine clean` runs in a project THEN the system SHALL remove `<project>/target` and print `removed <project-relative path>`.
2. WHEN `--cache` is given THEN the system SHALL also remove the compiled-design cache directory the transpiler's settings name, and say so on standard error.
3. WHEN there is nothing to remove THEN the system SHALL print `nothing to remove` and exit 0.
4. The system SHALL remove no file outside those two directories.
5. (Amended 2026-11-17, verification F3.) IF the cache directory, canonical, is `/`, or is or holds the project directory, the working directory or the home directory, or holds no `rust/` directory, THEN the system SHALL remove nothing, `target` included, say why on standard error and exit 1; from a cache it SHALL remove only the entries the transpiler's cache layout writes under `rust/`.

**Independent Test**: `piperine build && piperine clean --cache && piperine build` prints `compiled` again.

---

### P1: A bench is MSL, in the spec

**User Story**: As a reader of the specification, I want it to say where a bench lives so that it agrees with the tool.

**Why P1**: the user chose benches in the language; the spec says the opposite in four places.

**Acceptance Criteria**:

1. The system SHALL define in Part VIII of `docs/spec/` a **bench**: a module without ports that places the design with its sources, its stimulus and its monitors, written in MSL and chosen as the top by the host.
2. The system SHALL state in the spec's index (principle 3) that benches are MSL and that sweeps, Monte Carlo, optimization and regression live in the host.
3. The system SHALL keep in the table of the testbench section the host's rows for `force` and `release`, constrained random stimulus, sweeps and Monte Carlo, and SHALL move stimulus written as reactions and timers to the model's column.
4. The system SHALL NOT state anywhere in `docs/spec/` that stimulus or a testbench lives only in the host.
5. The system SHALL keep every error code of `docs/spec/` defined exactly once.

**Independent Test**: `cargo test -p piperine-lang --test spec_text`.

---

### P2: Plots

**User Story**: As a designer, I want to see a waveform, not only read its numbers.

**Why P2**: the user's answer: "Tabela + plots". The table already carries the result.

**Acceptance Criteria**:

1. WHEN `plot` or `--plot <file>` is given on a `transient`, `dc` or `ac` run THEN the system SHALL write one SVG document to that file and print `wrote <file>` on standard output after the table.
2. The system SHALL draw one path per printed real signal, each carrying its full path in a `data-signal` attribute, and a legend naming each.
3. The system SHALL map a value `v` at abscissa `x` to the point whose coordinates are linear in `x` between the plot area's left and right edges and linear in `v` between its bottom and top edges, the edges standing for the smallest and largest abscissa and the axis range.
4. The system SHALL draw an x axis and a y axis, each with its tick marks, its tick labels and its title: `time`, the swept param's full path, or `freq`.
5. WHEN a printed signal is discrete THEN the system SHALL draw it as a step path in a lane of its own under the plot area, never on the real axis.
6. WHEN the run is `ac` THEN the system SHALL draw two panels, the magnitude in dB over the phase in degrees, and a logarithmic x axis when the grid is `per_decade`.
7. WHEN a `transient` run has a `print_step` THEN the system SHALL still draw every accepted step.
8. IF `plot` is given on an `op` run THEN the system SHALL report `P0010` saying an operating point has nothing to plot.
9. WHEN a value is unknown THEN the system SHALL break the path at it.
10. The system SHALL write the same bytes for the same table.
11. IF the file cannot be written THEN the system SHALL print the error naming the path and exit 1.

**Independent Test**: `piperine run step --plot step.svg` in `examples/rc`, then open `step.svg` in a browser.

---

### P2: A formatter that keeps comments

**User Story**: As a designer, I want my file formatted without losing a word I wrote.

**Why P2**: `fmt` cannot ship on a printer that drops comments.

**Acceptance Criteria**:

1. The system SHALL give, for a source text that parses, a formatted text that holds every plain comment of the source, each once, in source order, with its text unchanged.
2. The system SHALL give a formatted text whose tree equals the source's tree, spans aside.
3. The system SHALL give the same text when it formats its own output.
4. WHEN a comment stands on its own line before an item, a member, a statement or a list element THEN the system SHALL print it on its own line before that same node, at that node's indentation.
5. WHEN a comment follows a node on the node's last line THEN the system SHALL print it after that node on the same line, one space before it.
6. WHEN a comment is the last thing in a block, or the only thing in it, THEN the system SHALL print it inside that block, before its closing brace.
7. WHEN a statement holds a comment that is neither before, after nor at the end of a node the formatter anchors to THEN the system SHALL print that statement as the source writes it, re-indented.
8. WHEN two statements or two members are separated by one or more blank lines THEN the system SHALL print exactly one blank line between them.
9. IF the source does not parse THEN the system SHALL return the diagnostics and no text.
10. The system SHALL satisfy criteria 1 to 3 on every file of the standard library, every gallery fixture, every file of `examples/buck` and `examples/rc`, every MSL twin of `tests/ngspice/` and every `msl` block of the Book.
11. The system SHALL leave `piperine_lang::msl::print` giving the text it gives today for every tree.

**Independent Test**: `cargo test -p piperine-lang --test msl_format`.

---

### P2: `piperine fmt`

**User Story**: As a designer, I want one command that formats my project, and one my CI can fail on.

**Why P2**: the user's answer: "Esses + fmt agora".

**Acceptance Criteria**:

1. WHEN `piperine fmt` runs in a project THEN the system SHALL rewrite in place every `.msl` file under `src/` and `benches/` whose formatted text differs, and print `formatted <project-relative path>` for each.
2. WHEN `piperine fmt <file>...` names files THEN the system SHALL format those files only.
3. WHEN `--check` is given THEN the system SHALL write no file, print `would format <path>` for each file that differs, and exit 1 when there is one and 0 otherwise.
4. IF a file does not parse THEN the system SHALL render its diagnostics, leave that file untouched, go on with the others, and exit 1.
5. WHEN every file is already formatted THEN the system SHALL print nothing and exit 0.
6. WHEN `piperine fmt --check` runs in `examples/rc` or in a project `piperine new` created THEN the system SHALL exit 0.

**Independent Test**: add spaces and a comment to `examples/rc/benches/step.msl`, run `piperine fmt`, and read the comment back.

---

### P1: The Book chapter

**User Story**: As a reader of the Book, I want a chapter on the command line with complete examples so that I can run a project by copying it.

**Why P1**: the user asked for it with the feature: "vamos planejar o cli para colocar no book também?", "exemplos completos". It is last in order because it shows real output.

**Acceptance Criteria**:

1. The system SHALL hold in `docs/book/content.html` a chapter with the id `cli` and the title `The command line`, placed after `compiler` and before `cheatsheet`, the chapter numbers consecutive from 1.
2. The system SHALL show in that chapter the tree of the RC project and the full text of its `Piperine.toml`, `src/lib.msl` and `benches/step.msl`, each equal to the file of `examples/rc`.
3. The system SHALL show in that chapter, each under the command that produces it, the standard output of `piperine check`, `piperine run op`, `piperine run step` and `piperine test` in `examples/rc`, each equal to what the binary prints.
4. The system SHALL show in that chapter the buck: its bench, its `startup` run table, the command, and the standard output of `piperine run startup` in `examples/buck`, equal to what the binary prints.
5. The system SHALL explain in that chapter print debugging with `emit!`: the statement, the record line it gives and how to filter a stream.
6. The system SHALL list in that chapter every command, every key of a run table with its flag, the exit codes, the backend choice and the codes `P0010` and `P0011`.
7. The system SHALL keep the chapter stand-alone: no `§`, no path into the repository's `crates/`, `docs/`, `.specs/` or `tests/`, and no link out of the Book.
8. The system SHALL keep every `msl` block of the Book accepted by the parser.
9. The system SHALL state in the chapter `compiler` that the command line exists, and SHALL NOT list it there as work to come.
10. The system SHALL list the seven commands in the cheat sheet.
11. The system SHALL show in the chapter `cli` the plot of the RC step response, the SVG equal to the file `piperine run step --plot` writes in `examples/rc`.
12. The system SHALL show in the chapter `cli` `piperine fmt` and `piperine fmt --check` and say that comments are kept.
13. The system SHALL say in the Book's first chapter that a bench is written in MSL and that sweeps, Monte Carlo and optimization live in the host, and SHALL NOT say there that testbenches or stimulus do not live in MSL.
14. (Amended 2026-11-17, verification F6.) The system SHALL hold every listing of the whole Book that claims to be the binary's output (`data-cli-output`, any chapter, any project) equal to the binary's whole output and exit code; a `data-cli-lines` range SHALL only select the lines the page shows, inside the block, and the block SHALL hold the whole output.

**Independent Test**: `python3 docs/book/build.py`, open `docs/book/index.html`, copy the RC project out of chapter 16 and run it.

---

### P2: Gap lists and instruction files

**User Story**: As the next planner, I want the repository to say what builds and what is still missing.

**Why P2**: nothing runs differently, but stale instructions cost every later session.

**Acceptance Criteria**:

1. The system SHALL state in `CLAUDE.md` and `AGENTS.md` that `piperine-api`, `piperine-cli`, `piperine-project` and the root package build, that `piperine-lang-server` stays outside the default build, with the role of each as this feature leaves it.
2. The system SHALL remove from `GAPS.md` the entries this feature closes and add each gap it leaves: no host sequences from the command line, no Python, the language server (deferred), no trace records.
3. The system SHALL mark queue item 3 of `ROADMAP.md` as done.
4. The system SHALL record in `.specs/STATE.md` the decision that the command line is a shell over `piperine-api` and that a run is a `[run.<name>]` table.
5. The system SHALL say in `examples/buck/README.md` how the buck is run from the command line.

**Independent Test**: `cargo test -p piperine-solver --test workspace_shape` (the crate table guard) and a read of the four files.

---

## Edge Cases

- IF a run table is named with a word that is not an identifier (`[run."a b"]`) THEN the system SHALL report `P0010` at the name.
- IF two flags give one single-valued setting twice THEN the system SHALL exit 2 naming the flag.
- IF `print` holds the same signal twice THEN the system SHALL print the column twice, in the order given.
- WHEN a transient's `print_step` is greater than `stop` THEN the system SHALL give the single row at time 0.
- WHEN a top module has discrete `in` ports THEN the system SHALL run it with those ports undriven, their value unknown.
- IF a top module has a continuous port THEN the system SHALL render the front end's diagnostic and exit 1.
- WHEN a record's stream is one of the two reserved streams THEN the system SHALL print its name without an instance prefix in the stream column and SHALL still print the site's instance path.
- IF standard output is closed while records are printed (a broken pipe) THEN the system SHALL stop and exit 1 without a panic message.
- WHEN `piperine run` is given `--csv` and `--records` THEN the system SHALL print the header line and the `wrote` line only.
- IF `piperine new` is given no name THEN the system SHALL exit 2.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| CLI-01 | P1: The host crates build again | Tasks | In Tasks |
| CLI-02 | P1: A project is found and loaded | Tasks | In Tasks |
| CLI-03 | P1: A run request | Tasks | In Tasks |
| CLI-04 | P1: A session | Tasks | In Tasks |
| CLI-05 | P1: Operating point and transient results | Tasks | In Tasks |
| CLI-06 | P1: Records from `emit!` | Tasks | In Tasks |
| CLI-07 | P1: `piperine run` | Tasks | In Tasks |
| CLI-08 | P1: The buck runs from the command line | Tasks | In Tasks |
| CLI-09 | P2: DC sweep and AC | Tasks | In Tasks |
| CLI-10 | P2: Params | Tasks | In Tasks |
| CLI-11 | P2: Output formats | Tasks | In Tasks |
| CLI-12 | P2: Record filter and record file | Tasks | In Tasks |
| CLI-13 | P2: The twins run from the command line | Tasks | In Tasks |
| CLI-14 | P2: `piperine check` | Tasks | In Tasks |
| CLI-15 | P2: `piperine build` | Tasks | In Tasks |
| CLI-16 | P2: `piperine test` | Tasks | In Tasks |
| CLI-17 | P2: `piperine new` | Tasks | In Tasks |
| CLI-18 | P2: `piperine clean` | Tasks | In Tasks |
| CLI-19 | P1: The Book chapter | Tasks | In Tasks |
| CLI-20 | P2: Gap lists and instruction files | Tasks | In Tasks |
| CLI-21 | P1: A bench is MSL, in the spec | Tasks | In Tasks |
| CLI-22 | P2: Plots | Tasks | In Tasks |
| CLI-23 | P2: A formatter that keeps comments | Tasks | In Tasks |
| CLI-24 | P2: `piperine fmt` | Tasks | In Tasks |

**Coverage:** 24 total, 24 mapped to tasks, 0 unmapped. The IDs follow the order the stories were written in; in this file the stories of CLI-21 to CLI-24 stand before CLI-19. CLI-25 (diagnostics in the editor) was removed on 2026-11-16 and its ID is not reused.

---

## Sources

- The user, 2026-11-15: "Depois desses desenvolvimentos do ir -> transpilation -> abi eu preciso que tu coloque na fila acertar o CLI, project para poder rodar algo completo." - the feature; `ROADMAP.md` "Queue after ir-rust" item 3.
- The user, 2026-11-15: "vamos planejar o cli para colocar no book também?" - CLI-19.
- The user, 2026-11-15, the eight answers relayed by the orchestrator: "Esses + fmt agora" (CLI-23, CLI-24), "Adoro a ideia de benches na propria linguagem ao invés de outra linguagem." (CLI-02, CLI-21), "Tabela + plots" (CLI-22), "Ao vivo, todos os streams" (CLI-06, CLI-12), "Rust por padrão" (CLI-04), "Agora, pequeno" (CLI-01), "LSP só de diagnósticos agora" (superseded on 2026-11-16), "Projeto RC passa-baixa" (CLI-17, CLI-19).
- The user, 2026-11-16: "O lang-server é o menos importante, vai ficar para bem depois." - the language server leaves the feature.
- The user, 2026-11-15: "Colocar print (com o emit!) para debug, exemplos completos" - CLI-06, and the complete examples of CLI-19.
- `GAPS.md` §2 - what does not build today.
- `.specs/features/codegen/context.md` R1 to R4 - the interpreter is the reference for the M1 analog subset, the Rust backend runs everything, the generated Rust is the debug surface.
- `.specs/features/msl-packages/spec.md`, MD-58, MD-59 - the Cargo model, no search path, `piperine-project` owns the manifest.
- `.specs/features/msl-digital/` (spec, design, contract; read on its branch) - records through `RECORD` pins and the sink, a record's text kept in the IR site, a top with discrete ports, the buck's closed-loop start.
- `docs/spec/part_vi_io.md` (streams, records, checks, metadata paths), `part_viii_host.md` (the host, `E0801`, `E0802`), `appendix_g_scope.md` G.17 (the top is the host's choice), `part_i_values.md` (the manifest).
- `crates/piperine-parity/src/{bench,backend}.rs` - how a design runs today.

---

## After this feature

The user's next step, verbatim (2026-11-16): "Finalizando o cli + api na feature, pode fazer o wiring do root e criar os testes integrados. Eu recomendo que tudo que está no livro tenha ao menos 1 teste integrado".

That is the next feature (the root e2e fixtures, queue item 4), not this one. What this feature already does toward it: every file block and every output block of the Book's chapter on the command line is held to the example projects and to the binary's output by a test (CLI-19 AC 2 to 4 and 11), and every `msl` block of the Book parses and survives the formatter (CLI-19 AC 8, CLI-23 AC 10). An integrated test for everything else the Book shows belongs to that next feature.

---

## Success Criteria

- [ ] In `examples/rc`: `piperine run step` prints a header, one `trip` record and a table, and exits 0, on both backends.
- [ ] In `examples/buck`: `piperine run startup` ends with the reference at 128 and no failed check.
- [ ] `cargo build` and `cargo test` at the root build and test the four host crates with zero warnings.
- [ ] `piperine fmt` on a file with comments keeps every one; `piperine fmt --check` passes on `examples/rc`.
- [ ] Every output block of the Book's chapter "The command line" is byte-equal to the binary's output, proven by a test.
