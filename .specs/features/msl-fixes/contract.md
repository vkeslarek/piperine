# MSL Fixes Contract

Checked against `feature/msl-fixes` at `443371fa` (T1, 2026-11-17). No code edited. Probes ran with the
binary of this tree (`piperine run <file> --analysis transient --stop 5e-9 --top Top`, Rust backend), the
probe files kept outside the repository.

## Cited locations

Every `file:line` of `design.md` was read. All are confirmed at the line named, with these exceptions:

| Cited | Found | Absorbed by |
| ----- | ----- | ----------- |
| `crates/piperine-analog-solver/src/assembly.rs:262`, `:366`, `:577` | `crates/piperine-analog-solver/src/device/assembly.rs:262`, `:366`, `:577` (same lines, under `device/`) | none needed; T23, T25, T34 edit `device/assembly.rs` and `device/watch.rs` |
| `types/catalogue.rs:117` `CORE_ROWS` has no row | `CORE_ROWS` opens at `:112`; the rows `ddt`, `limit`, `ac_stim`, `temperature`, `gmin`, `source_scale`, `now`, `cross`, `at` are at `:113-:122`; no row for `bound_step`, `freq`, `analysis` | T7, T33 |
| `elab/events.rs:111` | the refusal of a split head over a body that emits is at `:111-:114` (`self.emits_in(&r.body)`) | T27 |
| `types/call.rs:91` `apply`, `elab/functions.rs:24`, `types/expr.rs:226`, `types/module.rs:71`, `types/analog.rs:188`/`:192`, `elab/analog.rs:13`, `elab/unlowered.rs:30` | as cited | T2 to T7 |

## Probes

| Item | Output today |
| ---- | ------------ |
| S1 | `on at(1n) { finish(); } on at(3n) { .. }` prints `@1.000000e-9 Top.m sim finish=true`; with a digital-only top the run fails "last step at 0e0, before its stop"; with an analog node `fatal("boom")` prints `sim finish=true failed=true msg="boom"` and the run goes on to its rows and exits 0 |
| S5 | `c: f64 = 1.0 where c > d; d: f64 = 0.0;` is accepted (no `E0318`) |
| B1 | `i(gnd, p) +> 1m;` gives `error[E0282]: type annotations needed for [_; 1]` at `let at_zero_ = Self::terms_block();` |
| B2 | `on absdelta(v(p), 0.2) { emit!(..) }` gives `error[E0308]` in `fn cross1_block<R: Real>(..) -> R` (the block mixes `R` and `rt::Flagged<f64>`) |
| L11 | `f(1.0)` gives `P0002: mismatched types: expected 3 argument(s), found 1`; `f(1.0, None, Some(3.0))` gives `P0001: an expression the type checker gave no value type is not supported yet` |
| L17 | `Md::Lo < Md::Hi` gives `P0002: expected f64, found enum Md` |
| L41 | `P0001: a second analog block for one module is not supported yet` |
| L46 | `P0001: core primitive bound_step is not supported yet` (`freq` the same) |
| L47 | `P0001: analysis of a kind other than Static is not supported yet` |
| L54 | `P0001: a ttol or etol given to cross is not supported yet` (a digital `on cross(e, 1p, 1u)`) |
| L55 | `P0001: a head whose disjuncts each need a reaction, over a body that emits a record, is not supported yet` |
| L14 | `P0001: method sin on f64 is not supported yet` |

## Solver facts

1. **A reaction may read an event pin's `CHANGED` byte (C12): holds.** By reading: `abi.md` (cell table) gives an event pin
   `CHANGED` as "one byte: 1 if it fired in this microstep" with no restriction on the reader; the shape validator's
   `ChangedReadByWriter` (`piperine-abi/src/validate.rs:517`) concerns a function that writes a non-event pin, and
   `QuietChangeRead` concerns a `QUIET` pin; neither names a reaction reading an event pin; the digital solver reads
   `store.changed(id)` for `part::CHANGED` and `fire_net` dirties the readers (`piperine-digital-solver/src/run.rs:120`, `:163`).
   No run: the form cannot be written in MSL until T27, so T28 proves it end to end.
2. **The analysis globals (C10): holds.** `core/bind.rs:486-491` binds `global::DC`, `TRAN`, `AC`, `NOISE` (`START`, `SETTLE`)
   to the body's pin when it declares one. `DcSolver::new` builds its side with `global::DC` (`analyses/dc.rs:236`); `ac.rs:167`,
   `noise.rs:125` (and `tf`, `pz`, `sp`, `sens`, `adjoint`) reach their operating point through it, so an AC or noise
   operating point carries `DC` today. The transient does not: `analyses/transient.rs:1034` builds its side with
   `global::TRAN` and its operating point runs through the protocol on that side (`:1077`), so T35 touches `ac.rs` and `noise.rs` only.
3. **A `CROSS` watch of NaN fails at `device/watch.rs:100` (C8): holds, by a throwaway run.** In `piperine-analog-solver/tests/assembly_watch.rs`
   (the file was restored, nothing committed), `asm.accept(&[f64::NAN, 0.0], ..)` on the comparator module returned
   `Err(Simple { domain: Device, detail: "instance `cmp`: watch `up` (abi.md:11.5) is not finite: NaN (abi.md A4)" })`.

## Differences and who absorbs them

- The `assembly.rs` path (above): no task changes; the design's component text is read as `device/assembly.rs`.
- Nothing else differs. No fact fails, so nothing goes to the orchestrator before phase 2.
