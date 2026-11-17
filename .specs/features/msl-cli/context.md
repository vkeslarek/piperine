# MSL CLI Context

**Gathered:** 2026-11-15
**Spec:** `.specs/features/msl-cli/spec.md`
**Status:** Ready for execution once `msl-digital` merges. The user answered the eight questions on 2026-11-15; the answers reached the planner through the orchestrator.

---

## Feature Boundary

The command line and the project layer run a complete MSL project end to end: `Piperine.toml` discovery, the package graph, a top module compiled onto a backend, an analysis, its result and every `emit!` record on the terminal. The feature ends with a Book chapter about the command line whose examples are real projects.

---

## Implementation Decisions

### What the user decided (verbatim, 2026-11-15)

- "Depois desses desenvolvimentos do ir -> transpilation -> abi eu preciso que tu coloque na fila acertar o CLI, project para poder rodar algo completo." The CLI and the project layer are fixed so that something complete runs.
- "vamos planejar o cli para colocar no book também?" The feature includes a Book chapter.
- "Colocar print (com o emit!) para debug, exemplos completos" `emit!` records are the debug print; the examples are complete.

### The eight answers (user, 2026-11-15, relayed by the orchestrator)

| # | Question | The user's words | Decided |
| - | -------- | ---------------- | ------- |
| 1 | commands | "Esses + fmt agora" | `check`, `build`, `run`, `test`, `new`, `clean` and `fmt`. `fmt` gets a formatter that keeps plain comments first, as its own phase in `piperine-lang`, with round-trip tests over the standard library, the galleries, the examples and the Book's blocks; `fmt --check` for CI |
| 2 | analysis and stimulus | "Adoro a ideia de benches na propria linguagem ao invés de outra linguagem." | benches are MSL (a module without ports under `benches/`); `[run.<name>]` in `Piperine.toml`; flags override; a lone file runs with flags. The spec and the Book are amended where they say stimulus is host-only; sweeps, optimization and Monte Carlo stay in the host |
| 3 | output | "Tabela + plots" | aligned text table, `--csv`, and plots. The form was left to the planner: `--plot FILE.svg`, hand-written SVG, tested structurally; no terminal plot |
| 4 | records | "Ao vivo, todos os streams" | live, every stream, `--stream`, `--records FILE` as JSON Lines |
| 5 | backend | "Rust por padrão" | Rust by default; `--backend`, `PIPERINE_BACKEND`, the run table |
| 6 | `piperine-api` | "Agora, pequeno" | now and small: the old content deleted, `Project`, `RunSpec`, `Session`, `Table`, `Record` on the ABI; the CLI a shell; the root package builds |
| 7 | language server | "LSP só de diagnósticos agora" | **superseded on 2026-11-16**, see below |
| 8 | template | "Projeto RC passa-baixa" | the RC low-pass project for `new` and the Book's first example; the buck second |

### Amendment (user, 2026-11-16, relayed by the orchestrator)

- "O lang-server é o menos importante, vai ficar para bem depois." The language server leaves this feature: CLI-25 and tasks T50 to T54 are removed, `piperine-lang-server` and `editors/` stay untouched, and the crate stays outside `default-members`. This supersedes the answer to Q7. `Project::check_with` (the overlay of open buffers) was needed only by the server and is dropped with it.
- "Finalizando o cli + api na feature, pode fazer o wiring do root e criar os testes integrados. Eu recomendo que tudo que está no livro tenha ao menos 1 teste integrado". This is the next feature (root e2e), recorded in the spec under "After this feature". This feature already holds its own Book chapter's blocks by tests.

### Passing checks on the terminal (user, 2026-11-16, relayed by the coordinator)

- Question: how should the CLI handle the buck's `checks` stream, which writes one passing record per clock (about 120 000 lines for `startup`) against T21's "at most 16 lines"? The user chose "Só falhas por padrão (Recommended)".
- Decided: the terminal does not print `checks` records with `pass=true` (records of `require`, the DPWM's `Never` monitor); records with `pass=false` (or unknown) always print; the other streams stay live. A run key `all_checks` and the flag `--all-checks` print them all. `--records FILE` keeps every record: the filter is for the terminal only. Spec: CLI-06 AC 9, CLI-08 AC 3 and 6.

### What earlier decisions already fix

- Packages follow Cargo, no search path, `std` embedded, `piperine-project` is manifest, dependencies and lockfile (MD-58, MD-59; user: "-I é ruim demais, faremos modelo RUST 100%").
- The interpreter is the reference for the M1 analog subset and is frozen; the Rust backend runs everything; the generated Rust is the debug surface (`codegen/context.md` R1 to R4).
- Testbenches, sequences and sweeps live in the host, outside the language; the top module is the host's choice (`docs/spec/` Part VIII, G.17).
- `piperine-api` is the library face and the root package re-exports it (MD-20).
- Sonnet implements, Opus verifies; Opus implements what is delicate (S10).

### Agent's Discretion

Everything technical, and inside the answers: the form of a plot (Q3), how comments are kept (Q1), Also: crate layout, the data model of a run, the record decoder, the text formats, the error codes of the tool, the argument parser, the test layout. Each choice is a row of the spec's Assumptions table, open to correction.

### Declined / Undiscussed Gray Areas → Assumptions

None declined; all eight questions are answered above.

---

## Specific References

- Cargo, for the shape of commands, discovery and exit codes ("modelo RUST 100%").
- ngspice's print grid (`.tran tstep`) for the rows of a transient table.
- The Book is `docs/book/content.html`, built by `docs/book/build.py`. It stands alone: no § numbers, no repository paths, no links to other documents. Its `msl` blocks are parsed by `crates/piperine-lang/tests/book.rs`. Chapters 14 "The solver" and 15 "From source to solver" exist; the new chapter goes before the cheat sheet.

---

## Deferred Ideas

- Formatting options and range formatting.
- A terminal plot; PNG; interactive plots.
- The language server and the editor extension, whole (user: "vai ficar para bem depois").
- An integrated test for everything the Book shows (the next feature).
- `add`, `remove`, `tree`; `--locked`, `--offline`, an explicit `update`.
- Host stimulus from the command line (drives, waits, sequences) and a Python host.
- Noise, transfer function, pole-zero, periodic steady state, S-parameters, sensitivities and Monte Carlo as run tables.
- ngspice raw output.
- Trace records from `emit!` in `analog`.
- `piperine-parity`'s `Bench` and `Project` standing on `piperine-api` (queue item 4, root e2e).
