# Root E2E Contract: the tree against the design

Checked at `6f172a58` (branch `feature/root-e2e`, rebased on `origin/main` `1a3d69dc`: `host-ii` and `solver-hygiene` are in). Every location `design.md` cites was read at `HEAD`.

## Cited locations

| Citation | Status |
| -------- | ------ |
| `piperine-parity/src/{ngspice,golden}.rs`: `Ngspice`, `Golden`, `PrintedNumber` | confirmed (`PrintedNumber` at `golden.rs:177`) |
| `piperine-parity/src/contract.rs`: `ToleranceContract`, `Point`, `Coordinate` | confirmed; `TF_RATIO` (`:96`), `PZ_ROOT` (`:104`), `SENSITIVITY` (`:115`) are there |
| `piperine-parity/src/{backend,scratch}.rs`: `Backend::selected`, `ScratchDir` | confirmed (`backend.rs:40`, `scratch.rs:10`, `ScratchDir::create(fixture)`) |
| `Ngspice::fixture_path`, `Bench::twin_path` | confirmed: `ngspice.rs:108`, `bench.rs:33`; both join `../../tests/ngspice` |
| `Cli::parse`, `Invocation::Run` | confirmed: `cli.rs:297` returns `Invocation`; `Invocation::Run { project, target, overrides }` at `:183` |
| `Project::file`, `Project::at`, `Project::request`, `Session::open`, `Session::run`, `RecordSink`, `Failure` | confirmed (`project.rs:84,66,162`; `session.rs:149`; `lib.rs` re-exports) |
| `Settings { rustc: None, cache }` in `piperine-transpile/src/settings.rs` | confirmed; **`piperine::Settings` does not exist** (`piperine-api` does not re-export it): the root takes `piperine-transpile` as a dev-dependency, as `[dev-dependencies]` already does |
| `piperine-cli/tests/book.rs`: `blocks`, `files_before`, `attributes`, `decode` | confirmed (`:106`, `:124`, `:94`, `:45`) |
| `parity_corpus.rs`: registry + exhaustiveness guard | confirmed; it already counts **16** twins (`== 16`); `Guard::unmatched_twins` over `stems("msl")`, `stems("cir")` |
| `suite_hygiene.rs` root naming rule, `ROOT_TARGET_EXEMPTIONS`; `support/sources.rs` walker | confirmed: `ROOT_TARGET_EXEMPTIONS: [(&str, &str); 0]` (`:651`); `sources.rs::root_targets` (`:73`), `integration_targets` (`:61`) |
| `workspace_shape.rs:131` `root_tests_hold_no_end_to_end_target` | confirmed at `:131`; counts `.cir` under `tests/ngspice` (`:140`) and forbids every `.rs` under root `tests/` |
| Root `Cargo.toml` dev-dependencies | moved: `piperine-lang`, `piperine-solver`, `piperine-transpile`, `tempfile`, `num-complex` are there; `piperine-parity`, `piperine-cli` are not (T7 adds them) |
| `tests/ngspice/` five twins, `parity_corpus` count 12 | moved: sixteen twins, count 16 (the original seven, `solver-hygiene`'s `tran_dq`, `tran_jump`, `tran_corner`, `tran_floor`, `host-ii`'s `tf_divider`, `pz_lowpass`, `sens_divider`, `pss_rc`, `temp_res`). Updated in `design.md` and `tasks.md` |
| the twins' first comment line names `tests/ngspice/<name>.cir` | confirmed on all 16 `.msl` except `devices.msl`; one more hit: `noise_rc.cir:8` names `tests/ngspice_n...` (a root target that no longer exists), which a bare `grep tests/ngspice` also matches (T6 note) |

## What `host-ii` and `solver-hygiene` did

- Root `Cargo.toml`: one `[[test]] name = "host_ii"` at `crates/piperine-api/tests/root_surface/host_ii.rs` (not under root `tests/`; `workspace_shape` keeps `.rs` out of root `tests/`). No dev-dependency was added for it.
- `tests/host_ii.rs` does not exist; the target is the `[[test]]` path above.
- `workspace_shape.rs` and `suite_hygiene.rs`: not touched by `host-ii` beyond what `main` had (the `.rs`-free root `tests/` rule and the empty exemption list stand).
- `HostRun` is in `piperine-parity/src/host.rs`; `tf` rows read `input_impedance` / `output_impedance` (`gain` first).

## The nine twins with no golden: flags and backends (from the binary at `HEAD`)

| Twin | Command flags | Backends |
| ---- | ------------- | -------- |
| `tf_divider` | `--analysis tf --input v1 --output vout` | both |
| `pz_lowpass` | `--analysis pz --input v1 --output out` | both |
| `sens_divider` | `--analysis sens --wrt r1.r --wrt r2.r --wrt v1.dc --print mid` | both |
| `temp_res` | `--analysis op --temp 75` | both |
| `tran_dq` | `--analysis transient --stop 10e-6 --max-step 1e-8 --print-step 1e-7 --print a` | both |
| `pss_rc` | `--analysis pss --period 4e-6 --tstab 8e-6 --max-step 1e-9 --print-step 1e-7 --print vout` | Rust only |
| `tran_jump` | `--analysis transient --stop 2e-6 --max-step 1e-8 --print-step 1e-7 --print top` | Rust only |
| `tran_corner` | same as `tran_jump` | Rust only |
| `tran_floor` | `--analysis transient --stop 2e-6 --max-step 1e-8 --print-step 1e-7 --print n` | Rust only; fails `E0508` at 1 us by design, so its golden is empty |

- The interpreter refuses `pss_rc`, `tran_jump`, `tran_corner` and `tran_floor` ("the interpreter cannot run this design (use --backend rust)": they read `now()`). `parity_tran.rs` and `deviation.rs` (`COMPILED_ONLY`) already run them on the compiled backend only.
- The command line has no flag for ngspice's `delmin=1n` (`dt_min`): `tran_corner` through the binary runs with the default floor, so its command is not the parity run. A scenario that needs it uses the library (T8).
- `pz_lowpass`'s output is `out` (the raw-identifier signal `r#out` is not accepted by `--output`).

## The readers of `tests/ngspice` (count of lines naming it, per file)

`piperine-api/tests/{analyses.rs (3), grid.rs (2), options.rs (2), probes.rs (2), records.rs (1)}`; `piperine-cli/tests/{analyses.rs (1, comment), options.rs (2), run.rs (3), twins.rs (2)}`; `piperine-ir/src/lib.rs (1, doc)`, `piperine-ir/tests/support/devices.rs (2, doc)`; `piperine-lang/tests/{elab_analog.rs (1), elab_digital.rs (2), elab_entry.rs (4), msl_format.rs (2), suite_hygiene.rs (1, doc), types_entry.rs (2)}`; `piperine-parity/src/{bench.rs (4), host.rs (1, doc), lib.rs (1, doc), ngspice.rs (`fixture_path`)}`; `piperine-parity/tests/{buck/divider/devices/lib.msl (1, comment), deviation.rs (2), harness.rs (path-free), parity_corpus.rs (6), parity_grid.rs (1), twins.rs (2)}`; `piperine-solver/tests/workspace_shape.rs (2)`; `piperine-transpile/{src/print/text.rs (4, test strings, a printed origin), tests/differential.rs (1, doc)}`; the twins themselves (`tests/ngspice/*.msl:1`, `noise_rc.cir:8`). Path readers (as opposed to comments): the three `piperine-api` helpers, `piperine-cli/tests/{options,run,twins}.rs`, `piperine-lang/tests/{elab_analog,elab_digital,elab_entry,msl_format,types_entry}.rs`, `piperine-parity/{src/bench.rs, src/ngspice.rs, tests/deviation.rs, tests/parity_corpus.rs, tests/parity_grid.rs, tests/twins.rs}`, `piperine-solver/tests/workspace_shape.rs`.

## The Book at `HEAD`

18 chapters; 221 `<pre>` blocks, 214 of them with `data-lang`: 99 `msl`, 54 `text`, 43 `sh`, 10 `toml`, 6 `rust`, 2 `diag`; 7 `<pre>` carry no `data-lang`. 35 blocks carry `data-cli-output` (listings; 29 before `host-ii`). The sentence count of design "Data Models" is not recounted here: it needs the claim detector T14 writes.

## Baseline before any change (`env -u PIPERINE_NGSPICE`, no `SKIP` line)

- `cargo test -p piperine-cli --test twins`: 5 passed.
- `cargo test -p piperine-parity` (all targets): `--lib` 21; `backend` 4; `backends` 1; `bench` 4; `boundary` 3; `buck_*` 3+4+2+3+5+4+2+3+3; `deviation` 1; `harness` 5; `parity_ac` 1; `parity_corpus` 4; `parity_dc` 2; `parity_grid` 3; `parity_op` 3; `parity_pss` 2; `parity_pz` 2; `parity_sens` 2; `parity_temp` 3; `parity_tf` 2; `parity_tran` 6; `project` 2; `run_errors` 3; `source_catalogue` 17; `source_environment` 10; `source_fixes` 24; `twins` 5. 0 failed.
