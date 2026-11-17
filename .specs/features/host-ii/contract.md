# Host II Contract: the tree against the design

Checked at `5d13c860` (branch `feature/host-ii`). Every location `design.md` cites was read at `HEAD`.

## Cited locations

| Citation | Status |
| -------- | ------ |
| `piperine-plan/src/netlist.rs:31` `Netlist::with_resolution` | confirmed |
| `piperine-solver/src/protocol/transient.rs:46` `ProtocolOptions::new` (20 rounds) | confirmed; it is built at two sites, `analyses/transient.rs:1073` and `analyses/dc.rs:240`, and T2 changes both |
| `piperine-solver/src/core/bind.rs:641` `DigitalLimits::default()` | confirmed; `analyses/adapters.rs:45` (`empty_digital`) is a placeholder engine over an empty plan, not a binding site |
| `piperine-digital-solver/src/limits.rs:16` `max_delta_cycles: 1000` | confirmed; `Digital::limits` is `pub(crate)` and the `E0501` text (`NotQuiet`, `error.rs`) names the tick and the nets, not the bound (see T3) |
| `piperine-solver/src/core/builder.rs:127` `CircuitBuilder::resolution` | confirmed |
| `piperine-solver/src/analyses/dc.rs:197` `apply_initial_conditions` | confirmed (doc comment at `:195`, `nodeset`) |
| `piperine-solver/src/analyses/transient.rs:598` `schedule_set` | confirmed |
| `piperine-solver/src/protocol/host.rs:89` `Protocol::set` | confirmed |
| `piperine-solver/src/protocol/transient.rs:252` `None => stop_tick`, `:118` `observe_step`, `:341` its call | confirmed |
| `piperine-analog-solver/src/device/assembly.rs:296` `Assembly::term_value(path, name) -> Option<f64>` | confirmed |
| `piperine-ir/src/model.rs:251` `ChildValue` | moved: the doc comment is at `:251`, `pub enum ChildValue { Const(Constant), Expr(ExprId) }` at `:256`; `Body::child_params: Vec<Vec<(u32, ChildValue)>>` at `:84` |
| `piperine-lang/src/elab/carriers.rs:152` writer of `ChildValue` | confirmed: `self.ir.child_param(body, child, param, value)` over `structure.child_params` (`IrBuilder::child_param`) |
| `piperine-api/src/table.rs:36` `Cell::decode`, `:154` `Table::transient`, `:183` `Table::on_grid` | confirmed |
| `piperine-api/src/session.rs` the `result.iter().last()` check of L56 | confirmed at `:251` |
| `piperine-cli/src/cli.rs:329` `every_run_key_has_its_flag` | confirmed |
| `piperine-lang/std/io.msl:9` `fatal` writes `sim` with `failed = true` | confirmed (`std/io.msl` lives under `crates/piperine-lang/`) |

## Names this feature takes from the code

| Name | As the code has it |
| ---- | ------------------ |
| `TransferFunctionAnalysisResult` (`core/result.rs:404`) | `gain: f64`, `input_resistance: f64`, `output_resistance: f64`, `tf_type: TransferType`. `design.md` calls the last two rows `input_impedance` and `output_impedance`; T12 names its `quantity` rows after the fields (`input_resistance`, `output_resistance`) |
| `PoleZeroResult` (`core/result.rs:452`) | `poles: Vec<Complex<f64>>`, `zeros: Vec<Complex<f64>>` (fields, not methods) |
| `SensResult::get` (`analyses/sens.rs:52`) | `get(&self, output: &str, label: &str, param: &str) -> Option<f64>` |
| `PssResult` (`analyses/pss.rs:75`) | `trace: TransientAnalysisResult`, `stats: PssStats`; the period starts at `options.tstab` (`pss.rs:171`, a local `t0`), the trace holds `[t0, t0 + T]`; there is no `t0` field on the result |
| `TransientSolver::schedule_set` | `(&mut self, t: f64, label: impl Into<String>, param: impl Into<String>, value: Value)` |
| `Context` (`analyses/context.rs`) | one field, `tolerances: Tolerances` (`gmin`, `reltol`, `vntol`, `abstol`, `trtol`, `temperature`, `tnom`, ...); `Default` and `Debug` are written by hand; `Solver::with_context` carries it to every driver |
| `DigitalLimits` | built at `core/bind.rs:641` (the circuit) and `analyses/adapters.rs:45` (a placeholder) |
| `ProtocolOptions` | built at `analyses/transient.rs:1073` and `analyses/dc.rs:240`; `max_rounds` is a public field |
| `Assembly::term_value` | `(&self, path: &str, name: &str) -> Option<f64>`, the last evaluation's value |
| `ChildValue::Expr` | written by `IrBuilder::child_param`, called from `elab/carriers.rs:152` |

## Probes

S2 (`GAPS.md`), a `Mid` whose child is `Resistor { r: rr }` under `R(a, b)`, 1 V at `a`; the `analog for Mid` reads `rr` so that the `set` has a pin (without it the run stops at the known H5 error `no PARAM pin rr`):

```
$ piperine run s2.msl --analysis op --param Top.m.rr=3000
run s2: op, top Top, backend rust

signal  value
Top.a   1.000000e0
Top.b   5.000000e-1
```

The child keeps 1k: 0.75 is the expected value. Confirmed.

L56, the Blinky of `crates/piperine-cli/tests/run.rs`:

```
$ piperine run blinky.msl --analysis transient --stop 3e-6
run blinky: transient, top Blinky, backend rust
@5.000000e-7 Blinky.ctr edge t=5.000000e-7 count=0
@1.500000e-6 Blinky.ctr edge t=1.500000e-6 count=1
@2.500000e-6 Blinky.ctr edge t=2.500000e-6 count=2
error: the transient ended with the last step at 0e0, before its stop at 3e-6: the solver does not step a design with no analog node, so a digital-only transient is refused
exit=1
```

Confirmed (exit 1).

## Baseline at `HEAD`, before any change

| Target | Result |
| ------ | ------ |
| `cargo test -p piperine-cli --test twins` | 5 passed |
| `parity_op` | 3 passed |
| `parity_dc` | 2 passed |
| `parity_ac` | 1 passed |
| `parity_tran` | 2 passed |
| `parity_grid` | 3 passed (it prints `SKIP tran_rc: ... PIPERINE_NGSPICE=skip` from its own test of the skip rule; the comparisons ran against ngspice 47) |
| `parity_corpus` | 4 passed |

`validate_tasks.py host-ii --execute` passed (the Build line is green at `HEAD`).

No citation changes a task. The two differences that touch a task: `ProtocolOptions` is built at two sites (T2), and the `E0501` text does not name the microstep bound (T3: the solver's own wrapper names it).
