# Analog Operators Contract (T1)

Checked against `HEAD` 80409508 (branch `feature/analog-operators`). No code was edited; every probe below was a
throwaway (a test target or a one-line instrumentation, removed before this commit).

## Cited locations

| Cited | At HEAD | Verdict |
| ----- | ------- | ------- |
| `types/catalogue.rs:237` `CORE_ROWS` | `catalogue.rs:240` | moved (+3) |
| `types/analog.rs:170` `core_call` | `analog.rs:169` | moved (-1) |
| `std/analog.msl:12-14` | same | confirmed |
| `types/record.rs:119` `analog_trace`, `:111` | same | confirmed |
| `piperine-api/src/request.rs:38` `Analysis::NAMES` | same, 8 names, no `noise` | confirmed |
| `noise.rs:91` `NoiseSolver`, `:51` `input_source_name` | same; the field is read nowhere | confirmed |
| `noise.rs:373` `integrate_noise` | `noise.rs:376` | moved (+3) |
| `noise.rs:213` `record` | `noise.rs:214` | moved (+1) |
| `core/bind.rs:563` (NAMED branch), `:510` (node abstol) | same | confirmed |
| `core/builder.rs:124` `session_param` | same; called by no crate outside tests | confirmed |
| `piperine-plan/src/netlist.rs:60` `Netlist::circuit` | `netlist.rs:64` | moved (+4) |
| `delay.rs:76` `td > max` | `delay.rs:73` | moved (-3) |
| `watch.rs:136`, `transient.rs:302`, `operating_point.rs:42` | same | confirmed |
| `plan.rs:459` nature abstol | same | confirmed |
| `elab/known.rs:128`, `request.rs:615`, `session.rs:291` | same | confirmed |
| `abi_noise.rs:28` literal | same, and a second literal at `:191` | confirmed, one more |
| `docs.rs:268` `L47` assertion | same | confirmed |

Absorbed by T6: `NoiseAnalysisOptions` is also built literally in `tests/environment.rs:317`,
`tests/failure_rules.rs:173` and `:190`, besides the two in `abi_noise.rs`. Adding `output_flow` gains each
of those five literals the value `None` (a mechanical edit, no assertion changes).

## "What the code does today" rows, re-run

Front end, `piperine check` on a `D` module whose `analog for D` writes the form:

- `ddx(v(p, n), v(p, n))` : `P0001: core primitive `ddx` is not supported yet`
- `delay(v(p, n), 1e-9)` : `P0001: core primitive `delay` ...`
- `noise(1e-18, None)` : `P0001: core primitive `noise` ...`
- `white_noise(1e-18, None)` : `P0001: core primitive `noise`` located at `analog.msl:12:63` (through the stdlib)
- `simparam("gmin", 1e-12)` : `P0001: core primitive `simparam` ...`
- `emit!("newton", x = v(p, n))` in `analog` : `P0001: an `emit!` in an `analog` block is not supported yet`
  (a field written without `name = value` is `E0602` first)

Solver and host, by reading and by the probes of the nine facts: `Analysis::NAMES` has no `noise`; `input_source_name`
is read nowhere; `integrate_noise` is a trapezoid; `record` pushes a second value of a key at the next index
(misaligned series); no `tnom` key in `request.rs`; no crate but tests calls `session_param`; the transient step
reading a delivered jump accepts, fires, then delivers (no retake).

## The nine facts

1. **A `DELAY` history holds the operating point's value at time 0: holds.** The transient's `equilibrium` calls
   `adapters::accepted` -> `Assembly::accept`, which pushes `(amb.time, v)` for each `DELAY` term. Probe: a `DelayLine`
   (td 1 us, max 10 us) accepted at t=0 with v(a)=0.7, then a stage at 0.5 us (`t - td` < 0): right side of `o` is
   -0.7 and no matrix entry against the current v(a). `Delayed::at` returns `v0` for `s <= t0`. No gap, T33 adds nothing.
2. **Noise columns pass through `DELAY` and `DDT` term responses: holds.** Probe device with one `NOISE` input
   on a `DDT` term and one on a `DELAY` term (td 1 us) at 100 kHz: rows `(0, j628318.53)` and
   `(0, 0.809017 - j0.587785)` = `jw` and `exp(-jw td)`.
3. **A `Fault` in a noise sweep reaches the error with code and instance path: holds** (probe: `Device: instance
   `top.x1`: function `psd` (probe): fault (status 2, code 454): bad density`). That function reads only a param, so it
   is also the parameter-cone path of a function; `noise_sources` returns it unchanged.
4. **Operating point and transient drain `TRACE` records; AC and noise write none: holds.** `drain_traces` is called
   from `protocol/operating_point.rs:42` and `protocol/transient.rs:302` only; `ac.rs` and `noise.rs` never call it.
5. **`encode` of `ParamValue::Real` into a flagged `F64` (`REAL`) sets the known byte; an absent `OPTIONAL` `NAMED`
   pin reads zero: holds.** Probe (`REAL` layout, `OPTIONAL`, `GLOBAL NAMED` `k`): with the session value 7.0 the net
   init is `7.0` bytes then `1` at offset 8; with no session value the site is `Site::Absent`, and `plan.rs:270` gives
   `Source::Zero` (all zero, known byte 0).
6. **The binder does not confuse a `GLOBAL NAMED` pin and a `PARAM` pin of one name: holds, no refusal needed.**
   Probe: a body with a `PARAM` `k` (given 3.0) and a `GLOBAL NAMED` `k` (session 7.0): two nets, the param net
   `3.0`, the named net `7.0`. Host values match only `role::PARAM` pins; the session table matches the `GLOBAL`
   pin; the net keys differ (`param:` / `named:`). T26's `P0001` for the collision is not needed.
7. **A potential branch's private flow pin carries its nature's flow `abstol`: does not hold (U7 is a defect).**
   `plan.rs` builds the private pin as `PinSpec::continuous(..)` with no `abstol`; the node pins get
   `[potential abstol, flow abstol]` (`plan.rs:459`). The binder keeps a per-node override only for `AnalogVariable::Node`
   (`bind.rs:510`), so a branch unknown has no per-unknown `abstol`. T10 (D18) fixes the plan.
8. **Targets whose run takes the AO-13 retake: none.** Instrumentation in `transient_loop` logged every step that
   follows an instant which delivered a boundary, fires a crossing, and whose reactions deliver another boundary.
   The whole plain `cargo test --no-fail-fast` (295 targets, `PIPERINE_NGSPICE` unset) logged zero such steps. T8's
   retake moves no existing `buck_*`, `protocol_*` or twin target on this evidence; T8 still runs them.
9. **`NoiseColumn` name is `<instance path>.<input name>`; two `NOISE` inputs may share a name: holds.** Probe: both
   inputs of `top.x1` named `x` give two columns both named `top.x1.x` (the shape accepts duplicates). The solver
   splits on the last `.` into (element, source); the `record` misalignment of H2 follows from this, and T3 fixes it.

## Differences absorbed

- Line moves above: none changes a task.
- Fact 7: T10 fixes `plan.rs` (already named by D18).
- Fact 6: T26 need not refuse a collision.
- Five `NoiseAnalysisOptions` literals, not one (T6).
- ngspice sums the per-source integrals for its total (`DIOnoise`, `N_INTEG`): see T3.

## Re-anchor at B4 (T17)

`main` at `2fda3089` already carries `composite-values` and `generics` (phases A and B) and `params-and-io`; the branch
`feature/analog-operators` was reset onto it, so no rebase conflict arose. The IR anchors design.md cites moved only by
line number (`Expr` gained the aggregate and loop forms, `BodyData` its generic fields); `Expr::Derivative`,
`Derivative`, `IrBuilder::derivative`, `Body::derivatives`, `deps::Target::Derivative` and `Walk::finish` are where the
design reads them. Delay sites follow the derivative site's pattern at each. The workspace carries one warning that
predates this feature (`crates/piperine/tests/book/types.rs` `Reached::run` unused, test `book_types`); it is not touched.
