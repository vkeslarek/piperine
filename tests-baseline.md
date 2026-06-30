# Phase 0 — Baseline test counts

Captured before moving codegen from `piperine-lang` to `piperine-codegen`.
These counts are the regression guard for Phase 1.

| Crate | Tests passed | Total |
|---|---|---|
| `piperine-ams` (lib)   | 0 | 0 |
| `piperine-ams` (lex)   | 1 | 1 |
| `piperine-ams` (to_phdl) | 17 | 17 |
| `piperine-ams` (parse_single) | 1 | 1 |
| `piperine-ams` (suite) | 1 | 1 |
| `piperine-ams` (lexer_test) | 1 | 1 |
| **piperine-ams total** | **21** | **21** |
| `piperine-codegen` (ams_ir) | 31 | 31 |
| `piperine-codegen` (ppr_ir) | 23 | 23 |
| **piperine-codegen total** | **54** | **54** |
| `piperine-lang` (lib)       | 0  | 0 |
| `piperine-lang` (codegen)   | 13 | 13 |
| `piperine-lang` (elab)      | 26 | 26 |
| `piperine-lang` (integ)     | 20 | 20 |
| `piperine-lang` (integration) | 2 | 2 |
| **piperine-lang total**     | **61** | **61** |
| `piperine-solver` (cosim)   | 7 | 7 |
| `piperine-solver` (digital_topology) | 27 | 27 |
| `piperine-solver` (mixed_signal) | 25 | 25 |
| `piperine-solver` (osdi)    | 27 | 27 (1 ignored) |
| `piperine-solver` (lib)     | 7 | 7 |
| **piperine-solver total**   | **93** | **93** (1 ignored) |

**Grand total before refactor**: 229 passing.

Any drop below these counts in a phase means a regression.
