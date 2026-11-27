# Code Clarity: the suite report

The numbers of the test suite before and after the feature (CC-12 AC 1, AC 6). The census columns are
taken by `SuiteCensus::take()` of `piperine-repository-guards` (a static count of the sources: targets
`cargo test` runs, `#[test]` functions in them and in `src/`, targets of three tests or fewer, lines under
`tests/`). There is no build-time column. Test run time is the key metric (user 2026-11-26: "O build não precisa de métrica... Tá de boa"; "Tempo e build é muito relativo... O importante é menor tempo de testes (conjunto mais bem dimensionado deles)"): the goal is a smaller, better-sized suite that runs faster.

## Before

Taken at the base of `feature/code-clarity` plus T1 to T8, after the language server left (T2) and the
guards crate arrived (T3 to T7). Targets declared `test = false` (the buck targets) are not counted.

| Crate | Integration targets | Tests in them | Unit tests (in `src/`) | Targets with 3 tests or fewer | Test lines (`tests/`) | Run s |
|---|---|---|---|---|---|---|
| `piperine` (root) | 26 | 422 | 0 | 3 | 12654 | 271 |
| `piperine-abi` | 11 | 52 | 4 | 5 | 2833 | 0 |
| `piperine-analog-solver` | 13 | 66 | 44 | 4 | 4226 | 0 |
| `piperine-api` | 26 | 228 | 79 | 6 | 6596 | 383 |
| `piperine-cli` | 10 | 195 | 14 | 2 | 4732 | 159 |
| `piperine-digital-solver` | 16 | 164 | 2 | 1 | 5927 | 0 |
| `piperine-interp` | 7 | 57 | 15 | 3 | 2912 | 0 |
| `piperine-ir` | 15 | 162 | 0 | 0 | 10253 | 0 |
| `piperine-lang` | 81 | 1520 | 2 | 4 | 36348 | 288 |
| `piperine-numbers` | 8 | 22 | 0 | 6 | 2739 | 329 |
| `piperine-parity` | 62 | 596 | 49 | 15 | 16591 | 624 |
| `piperine-plan` | 9 | 61 | 6 | 4 | 3125 | 0 |
| `piperine-project` | 5 | 53 | 0 | 0 | 1335 | 0 |
| `piperine-repository-guards` | 8 | 69 | 33 | 3 | 2450 | n/a |
| `piperine-solver` | 56 | 344 | 38 | 22 | 16196 | 3 |
| `piperine-transpile` | 29 | 117 | 53 | 14 | 8999 | 145 |
| **total** | **382** | **4128** | **339** | **92** | **137916** | **2202** |

Against the inventory of `findings.md` 3.1 (taken before T2 and T5): `piperine-solver` is 56 targets, not
58, because `workspace_shape` and `crate_boundaries` moved to the guards crate (T5); `piperine-lang` is
81 targets, not 80, and 1 520 tests, because `suite_hygiene` left it and the nested-reach and
generics-phase-E targets arrived with the base; `piperine-parity` is 62 targets and `piperine-cli`
10 with 195 tests for the same reason. The guards crate holds 8 targets (`boundaries`, `documentation`,
`naming`, `ownership`, `proof_ledger`, `structure`, `suite`, `worklists`) and 33 unit tests.

*Run s*: the before values are those of `findings.md` 3.1, one serial pass over the executables with ngspice, measured at the inventory base `93c831ffc` (2 202 s in all); they were not re-measured at T8, because the full suite runs once, at T155, and that run measures the after value to compare with these.

## After

Written by T152.
