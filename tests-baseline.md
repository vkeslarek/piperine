# Phase 0+ baseline test counts

Captured before/after moving codegen from `piperine-lang` to `piperine-codegen`.

## Phase 0 baseline (pre-refactor)

| Crate | Tests passed |
|---|---|
| `piperine-ams` (lib) | 0 |
| `piperine-ams` (lex) | 1 |
| `piperine-ams` (to_phdl) | 17 |
| `piperine-ams` (parse_single) | 1 |
| `piperine-ams` (suite) | 1 |
| `piperine-ams` (lexer_test) | 1 |
| **piperine-ams total** | **21** |
| `piperine-codegen` (ams_ir) | 31 |
| `piperine-codegen` (ppr_ir) | 23 |
| **piperine-codegen total** | **54** |
| `piperine-lang` (lib) | 0 |
| `piperine-lang` (codegen) | 13 |
| `piperine-lang` (elab) | 26 |
| `piperine-lang` (integ) | 20 |
| `piperine-lang` (integration) | 2 |
| **piperine-lang total** | **61** |
| `piperine-solver` (cosim) | 7 |
| `piperine-solver` (digital_topology) | 27 |
| `piperine-solver` (mixed_signal) | 25 |
| `piperine-solver` (osdi) | 27 (1 ignored) |
| `piperine-solver` (lib) | 7 |
| **piperine-solver total** | **93** (1 ignored) |

Phase 0 grand total: **229 passing**.

## After phase 1 (codegen relocated)

229 passing (no regressions from Phase 1).

## After phase 1.4 (IR → Device, analog)

229 passing, 0 ignored (Phase 1.4 wrapped the analog lowering).

## After phase 1.5 (IR → Device, digital)

229 passing + 3 new (DFF, Buf, missing-module).

## After phase 1.6 (`from_ir`)

243 passing.

## After phase 2 (E2E IR → solver)

250 passing.

## After phase 2.7 (numeric DC validation + transient)

**258 passing** (current).

## Breakdown at current state

| File / bin | Tests | Notes |
|---|---|---|
| `tests/ams_ir_test.rs` | 31 | AMS → IR structural |
| `tests/ppr_ir_test.rs` | 23 | PPR → IR structural |
| `tests/codegen_api_tests.rs` | 4 | API surface pinning (2 ignored — Phase 1.5/1.6 done) |
| `tests/ir_analog_to_device_tests.rs` | 3 | IR analog lowering |
| `tests/ir_digital_to_interp_tests.rs` | 3 | IR digital lowering |
| `tests/from_ir_tests.rs` | 3 | full IR → CircuitInstance glue |
| `tests/codegen_e2e_tests.rs` | 8 | PPR end-to-end solver runs (DC numeric, transient) |
| `tests/ams_ir_e2e_tests.rs` | 7 | AMS boilerplate (resistor/capacitor/vsource/isource/vramp/vstep/noisy) |

Any drop below 258 means a regression.