# Analog Structure Contract

Written by T1 against `HEAD` after `git rebase origin/main` (main carries `host-ii` at `1a3d69dc` and `solver-hygiene` at `ec4d13b6`). The design assumed `99d05af2` plus `520e546e` and `6c3df7cd`. Probes ran on the Rust backend with the binary built from this tree (`piperine run <file> --analysis op`).

## Cited locations

| Cited by the design | Now | Status |
| ------------------- | --- | ------ |
| `elab/contribute.rs:31` (`scope.nesting > 0`) | `:32` | moved 1 |
| `elab/analog.rs:223` (`ConditionalBoundStep`), `:157` (`"limit" if scope.nesting == 0`) | same | confirmed |
| `elab/analog.rs:191` (`ddt` refusal, `DdtOffTop`) | `:192` | moved 1 |
| `check.rs:1123` (`Stmt::Contribute`) | `:1121` | moved 2 |
| `check.rs:1596` (`site`) | same | confirmed |
| `model.rs:123` `Terminal`, `:136` `BranchKind` | same | confirmed |
| `elab/unlowered.rs:64` `refuse_digital_forms`, `elaborator.rs` call | `:64`; call at `elaborator.rs:97` (the design's `:99` is the id allocation, not moved in kind) | confirmed |
| `elab/hoist.rs:206` `hoistable` | same | confirmed |
| `types/result.rs:236` | `self_read` is `:227`, the `P0001` line `:236` | confirmed |
| `types/unsupported.rs:22` | same | confirmed |
| `elab/consts.rs:192` | `:191`-`:193` | moved 1 |
| `types/call.rs:14` | same | confirmed |
| `elab/children.rs:112-119` | padding at `:117`-`:119` | confirmed |
| `Frame` in `elab/hoist.rs` | `struct Frame` is in `elab/carriers.rs:36`; `hoist.rs` uses it | moved file: T27/T30 read `carriers.rs` |
| `Scope::no_locals` in `elab/lower.rs` | the field is read in `elab/compound.rs:90,102` and set in `elab/drives.rs:56` | moved file: T20 follows those |
| `ac.rs:124` `is_static: false` | `piperine-solver/src/analyses/ac.rs:124` | confirmed |
| `device/watch.rs:100` (`CAP` check) | `piperine-analog-solver/src/device/watch.rs:93-103` | confirmed (T14 edits the `v.is_finite() && v > 0.0` test) |
| `Mode::Ttol` pattern | `piperine-plan/src/plan.rs:69,164` | confirmed |
| `TermPlan::abstol` (V1) | `piperine-analog-solver/src/device/plan.rs:93,107,325` and `assembly.rs:455`, `math/integration.rs:124` | merged |
| `ParamPath::refuse_timed` (HII-13) | `piperine-api/src/session.rs:118` (`refuse_timed(&self, ir)`); `E0803` text in `host.rs:83` | confirmed |
| `piperine-plan/src/derive.rs` (X9) | `DerivePlan`, `Binding`, `ParamReads` exported from `piperine-plan/src/lib.rs:12`; `piperine-api/src/params.rs` holds the host table `Params` (`Params::of`, `set`, `changes`) over a `Derive` trait | confirmed; host-ii added the table the design called the session's |
| `Context.nodeset` | `piperine-solver/src/analyses/context.rs:58`; `Session::open` sets it at `session.rs:206` | added by host-ii, no effect on this feature |
| `session.rs` `Session::open` (T22 touches) | before-run params are applied at `session.rs:209-212` through `Session::set_param` after the circuit is built | T22 passes the key-param bindings to the elaborator before this; the loop stays for every other param |

## Probes ("What the code does today")

| Gap | Probe | Output (first line) |
| --- | ----- | ------------------- |
| L40 | `if c > 0.0 { i(a, gnd) +> 1.0; }` | `P0001: a contribution under an `if` or in a closure is not supported yet` |
| L40 | `if c > 0.0 { bound_step(1n); }` | `P0001: a `bound_step` under an `if`, a `match` or in a nested block is not supported yet` |
| L42 | `i(a, gnd) +> 2.0 * ddt(q * v(a))` | `P0001: a `ddt` that is not a whole addend of a flow contribution or of a potential contribution is not supported yet` |
| L42 | `ddt(q * v(a), 1e-3)` | `P0001: a `ddt` with a tolerance is not supported yet` |
| L50 | `let t = if c > 0.0 { ac_stim(1.0) } else { 0.0 }; v(a, gnd) +> t;` | `P0001: an `ac_stim` under an `if` or in a closure is not supported yet` |
| L34 | `module G(o: Electrical, x: f64) {}` placed | `P0001: a value input is not supported yet` |
| L35 | a module `-> Electrical` with a `signal` and an `analog` block | `P0001: a module with a result that holds a node port, is not supported yet` |
| L36 | `let z = c.r;` with `pub r` on the child | `P0001: field access is not supported yet` (`v(c.mid)` gives `probe through an access that names several natures`, an earlier refusal) |
| L37 | `Cap { c: c }` with `let c = 1.0;` | `E0321: a param value is not an expression over params and constants: it reads a name with no value yet` |
| L26 | `r.changed()` on a param | `P0001: method `changed` on `f64` is not supported yet` |
| L27 | `signal s: Bit<f64> = k0;` | `P0001: a carrier's initial value that reads a param is not supported yet` |
| L38 | `let y = Integ(y)` | the `P0001` "the feedback of .. through an instance" is `types/result.rs:236`; the probe module needs a result with no direct dependence to reach it, `types/result.rs` is its only source (read, not run: a result module needs the stdlib `idt`, which waits for `generics`) |
| S4 | a `signal` of a placed module | the `Child` record pads it with `Terminal::Ground` (`children.rs:119`); the run is right, the authored tree shows `gnd` |

Two rows the design lacked: a contribution under an `if` is refused before a `limit` or `ac_stim` under the same `if` is seen (the contribution's `P0001` comes first, so the probe for L50 puts the stimulus in a `let`).

## The eight facts

1. **`CHANGED` of a param pin.** A timed host `set` holds: `Session::set_param` -> `CircuitInstance::set` (`piperine-solver/src/core/circuit.rs:216`) shadows the write, then `host_commit`s it due at the set's tick (`piperine-digital-solver/src/engine.rs:223`), and `Store::write` stamps `changed_at` when the bytes differ (`store.rs:229`): an event function reading `CHANGED` of the pin fires at that instant. **A before-run binding does NOT hold as the spec assumes.** `Digital::start_at` (`engine.rs:171`) zeroes the store and commits every pin's `init` at `(0, 0)`; `DigitalSide::start` (`piperine-solver/src/analyses/adapters.rs:152`) then commits the host's writes (a before-run binding is a host write) at `(0, 0)`. Both writes differ from the zeroed store when non-zero, so `CHANGED` reads true at `(0, 0)` for a param with a non-zero default or binding. Read from the source, not run: no event reads a param pin until T10/T33. **Absorbed by T33** (it must keep an event on a param from firing at `(0, 0)`: the function reads the analysis-start condition, or ignores the start microstep). Reported to the orchestrator before phase 5 as design asks.
2. **`PREVIOUS` of a param pin before any `set`.** It does not hold its own value: the store starts with zeroed `previous` (`store.rs:135`), the init commit at `(0, 0)` leaves `previous` = 0 bytes (0.0), and a before-run write leaves it = the default. AS-14 AC 2 needs T33 to read `PREVIOUS` as the value when no `set` has happened (the same start condition as fact 1). **Absorbed by T33.**
3. **`DDT` `abstol`.** Holds: `TermPlan::abstol` (`device/plan.rs:325`) binds a `DDT` term's discrete `F64` pin, `assembly.rs:455` reads it, `math/integration.rs:124` uses `state.abstol.unwrap_or(tol.abstol)` for the LTE.
4. **AC sweep re-runs a `STATIC` reader with `false`.** Holds: `piperine-solver/src/analyses/ac.rs:124` sets `is_static: false`; `op`, `tf`, `pz` (`:277`) and sensitivity set `true`; transient sets `false` (`protocol/transient.rs:348`).
5. **A device `Fault` reaches the run error.** Holds: `assembly.rs:525` and `:665` return `device_error("instance `<path>`: function `<name>` (<origin>): <fault text>")` (`device/mod.rs:25`, domain `Device`). A fault text that starts `E0418: branch ...` arrives whole after the instance path.
6. **Residual test of a private flow pin's row.** `Tolerances::residual_within` (`piperine-analog-solver/src/tolerances.rs:104`) uses `abstol + reltol * scale` for every branch variable (`reference.variable().is_branch()`), whatever the row's unit. A switch branch's `b<k>.eq` row is such a row, as a potential branch's is today (volts tested against an ampere floor). No change; a switch test runs with default tolerances.
7. **Library cache key.** `piperine-transpile/src/compile.rs:36-54`: rustc version text, flags, the generated source, the runtime files and the ABI files. Instance params are not in the key (they reach the library at `create(values)`); a key-param variant changes the generated source, hence the key, as design expects.
8. **What refuses `limit` / `ac_stim` reading a carrier.** The transpiler, not the elaborator or `Ir::check`: `print/analog.rs:583` ("a `{limit|ac_stim}` function reading the carrier `c`", asserted by `piperine-transpile/tests/reject.rs:128,137`). Probes: `error: `a `ac_stim` function reading the carrier `c`` has no Rust lowering yet` and the same for `limit`. **Absorbed by T21**, which rewrites those two `reject.rs` rows to assert the new behaviour and says so in its commit.

## Differences and the task that absorbs each

- Facts 1 and 2 (before-run `CHANGED`/`PREVIOUS`): T33, reported to the orchestrator before phase 5.
- Fact 8: T21, plus its `reject.rs` rewrite.
- `Frame` and `no_locals` live in `carriers.rs`, `compound.rs` and `drives.rs`: T20, T27, T30 read those files.
- `Terminal::Own` needs one arm in `piperine-lang/src/elab/place.rs:124` as well as `piperine-plan`, `piperine-transpile` and `piperine-interp` (T2).
