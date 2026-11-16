# MSL CLI Context

**Gathered:** 2026-11-15
**Spec:** `.specs/features/msl-cli/spec.md`
**Status:** Waiting for the user's answers to the eight questions at the top of the spec. Design and tasks are written against the recommended options and change with the answers.

---

## Feature Boundary

The command line and the project layer run a complete MSL project end to end: `Piperine.toml` discovery, the package graph, a top module compiled onto a backend, an analysis, its result and every `emit!` record on the terminal. The feature ends with a Book chapter about the command line whose examples are real projects.

---

## Implementation Decisions

### What the user decided (verbatim, 2026-11-15)

- "Depois desses desenvolvimentos do ir -> transpilation -> abi eu preciso que tu coloque na fila acertar o CLI, project para poder rodar algo completo." The CLI and the project layer are fixed so that something complete runs.
- "vamos planejar o cli para colocar no book também?" The feature includes a Book chapter.
- "Colocar print (com o emit!) para debug, exemplos completos" `emit!` records are the debug print; the examples are complete.

### What earlier decisions already fix

- Packages follow Cargo, no search path, `std` embedded, `piperine-project` is manifest, dependencies and lockfile (MD-58, MD-59; user: "-I é ruim demais, faremos modelo RUST 100%").
- The interpreter is the reference for the M1 analog subset and is frozen; the Rust backend runs everything; the generated Rust is the debug surface (`codegen/context.md` R1 to R4).
- Testbenches, sequences and sweeps live in the host, outside the language; the top module is the host's choice (`docs/spec/` Part VIII, G.17).
- `piperine-api` is the library face and the root package re-exports it (MD-20).
- Sonnet implements, Opus verifies; Opus implements what is delicate (S10).

### Agent's Discretion

Everything technical: crate layout, the data model of a run, the record decoder, the text formats, the error codes of the tool, the argument parser, the test layout. Each choice is a row of the spec's Assumptions table, open to correction.

### Declined / Undiscussed Gray Areas → Assumptions

None declined. Eight are not answered yet and are asked in the spec ("Open questions for the user"): the command set, how an analysis is requested and where stimulus lives, what `run` prints, how records are shown, the backend default, whether `piperine-api` is rebuilt now, Python and the language server, the template of `new`. Each has a recommended option; the spec's Assumptions table marks the rows that follow one with `n - Q<k>`.

---

## Specific References

- Cargo, for the shape of commands, discovery and exit codes ("modelo RUST 100%").
- ngspice's print grid (`.tran tstep`) for the rows of a transient table.
- The Book is `docs/book/content.html`, built by `docs/book/build.py`. It stands alone: no § numbers, no repository paths, no links to other documents. Its `msl` blocks are parsed by `crates/piperine-lang/tests/book.rs`. Chapters 14 "The solver" and 15 "From source to solver" exist; the new chapter goes before the cheat sheet.

---

## Deferred Ideas

- `fmt`, once the printer keeps plain comments.
- `add`, `remove`, `tree`; `--locked`, `--offline`, an explicit `update`.
- Host stimulus from the command line (drives, waits, sequences) and a Python host.
- Noise, transfer function, pole-zero, periodic steady state, S-parameters, sensitivities and Monte Carlo as run tables.
- Plots and ngspice raw output.
- Trace records from `emit!` in `analog`.
- The language server on the MSL front end.
- `piperine-parity`'s `Bench` and `Project` standing on `piperine-api` (queue item 4, root e2e).
