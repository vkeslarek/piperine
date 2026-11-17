# Long Run Contract: the merged tree against the design

Checked at `ef7184d7` (branch `feature/long-run`, on `a5f23ea4`). `design.md` cites lines of `dbcd8307`.
The rebase moved no cited line of the solver, the analog solver or the digital solver.

## Cited locations

| Citation | Status |
| -------- | ------ |
| `core/result.rs:83` `TransientAnalysisResult`, `:129` `TransientStep`, `:202` `ReentryState` | confirmed |
| `analyses/transient.rs` `:89` `probe_selection`, `:712` `record_step`, `:778` `snapshot`, `:1085` `ran?`, `:1091`-`:1098` final re-entry, `:1177` `device_record_start`, `:1238` `device_capture`, `:1246` `device_reentry_state`, `:1460` `record_step` call | confirmed |
| `analyses/adapters.rs` `:51` `Settled`, `:130` tick to seconds, `:177` `mark`, `:403` `HostHooks`, `:432` `install` | confirmed |
| `analyses/adapters.rs:209` `DigitalSide::drain` | moved: `drain` is at `:210` (the doc comment line is `:209`) |
| `analyses/solver.rs:50` `Solver::with_sink` | confirmed |
| `protocol/transient.rs:323` `self.delivery.flush(t_c)` | confirmed; the call sits before the `final_mark \|\| outcome.finish` return, so the final step is observed too |
| `protocol/sides.rs:136` `Observation`, `:147` `Change { bytes }` | confirmed |
| `piperine-analog-solver/src/netlist.rs:219` `reference_for`, `:132` `idx` | confirmed |
| `piperine-digital-solver/src/run.rs:114` the one `Change` push | confirmed |
| `piperine-api/src/session.rs:230` the `transient` arm | confirmed |
| `piperine-api/src/table.rs` `:36` `Cell::decode`, `:154` `Table::transient`, `:183` `Table::on_grid` | confirmed |
| `piperine-api/src/session.rs` `Session::discrete` | confirmed at `:296` (`discrete(circuit, path) -> Option<Vec<u8>>`, a read by path) |
| `piperine-cli/src/commands/test.rs:30` | moved: `Collected` is at `:29`, `judge` at `:96`, the `Rc<RefCell<Collected>>` at `:109` (absorbed by T13) |
| `piperine-cli/src/cli.rs` "the test around line 331" | confirmed in substance: `every_run_key_has_its_flag` at `:329` |
| `piperine-api/src/record.rs` (`Delivery`), `verdict.rs`, `probe.rs` | present; no `ObservationKind::Change` handling in `piperine-api/src` today (T9 adds it) |

No difference needs a new task. The two moved citations are line drift only.

## Digital net writers and their `Change` observation

Every write to the store's discrete bytes, in `piperine-digital-solver/src`:

| Writer | Site | Observation |
| ------ | ---- | ----------- |
| Applied commits (reaction commits, host commits, `init` commits, which `start` queues as `Writer::Host` commits due at (0, 0)) | `instant.rs:66` `apply_due` -> `write_net` | `Change` with the net's name and the full bytes, when the settled bytes differ from the value before the microstep |
| Discrete function outputs outside a relaxed group | `run.rs:255` `apply` -> `write_net` | `Change` |
| Relaxed (resolved) group, intermediate writes | `run.rs:354`, `:372`, `:413` direct `store.write` | none, by design: `:419`-`:422` put each written net back to its saved state and then call `write_net` once with the settled bytes, so the run sees one `Change` per net (none when the settled bytes equal the saved ones) |
| Relaxed group, non-settling field zeroed | `run.rs:413` | `Unsettled`, then the one `Change` of `:421` carries the zeroed bytes |
| Shadow check of a host `PARAM`/`GLOBAL` write | `engine.rs:290` `shadow` | none, correctly: it runs on a clone of the store with a throwaway `Obs`; nothing of the engine changes |
| `Store::new` (`start`, `engine.rs:152`) | `store.rs` | none: every net starts at zero bytes. The nonzero `init` arrives as the commit above, at instant 0 |
| `Digital::restore` (`engine.rs:267`) | `store = cp.store.clone()` | none: a rollback; `obs.truncate` forgets the observations of the undone span, so observations and store stay consistent |
| Event fire | `run.rs:122` `fire_net` | `Fire`, which the solver adapter turns into `ObservationKind::Crossing` for a crossing net and into `Change { bytes: [] }` otherwise (`adapters.rs:214`-`:216`); an event net holds no bytes |

Result: no writer of a discrete net's bytes is without a `Change` observation. Three points T9 must hold:

1. A net's value before instant 0 is all zero bytes; its first nonzero value comes as a `Change` at tick 0. The recorder's start cell for a `Net` column must read the net at the start observation or assume zero bytes, and the per-step check of the design catches a mismatch.
2. A resumed transient restores the store from a re-entry state (no `Change` for the restored values). A recorder that starts from such a run reads each net's value at the start observation (T4), which is the net reader, not a change.
3. Within one microstep `Obs::flush` sorts by (kind, net name, statement) with a stable sort, so the per-net order of changes is the order they were made. Across microsteps the order is the instant's. Both are what a hold-and-apply recorder needs.
