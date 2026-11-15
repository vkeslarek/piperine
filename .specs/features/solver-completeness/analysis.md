# Solver completeness: analysis

**Date:** 2026-11-14. **Base:** `main` e129b869. **Read:** `docs/spec/` (Parts I to VIII, Appendices C and G) and `crates/piperine-solver/` (src 14.9k lines, tests 12.8k lines).

**Verdict.** The analog numeric core is mostly there: Newton, continuation, TR-BDF2, step control, rollback, limiting, the small-signal family. The discrete side and the coupling between the two are Verilog-AMS-shaped and do not carry MSL: time is a float, the digital engine knows only four-state nets with fixed strengths, digital runs *before* the analog solve instead of after the left value, there is no jump solve, no crossing location and no settle commit. About 3.4k lines of the crate serve constructs MSL does not have.

Paths below are relative to `crates/piperine-solver/src/`.

## 1. Baseline

| Command | Result |
| --- | --- |
| `cargo build -p piperine-solver` | green: the library compiles |
| `cargo test -p piperine-solver` | **does not build**: the dev-dependency `piperine-codegen` fails with 296 errors (it targets the deleted VAMS frontend). Cargo compiles dev-dependencies for every test target, so none of the 155 unit tests and 198 integration tests run, although only 5 of the 39 integration targets import `piperine_codegen` or `piperine_lang` (`adjoint_dc`, `adjoint_vs_sens`, `pss`, `sens`, `transient_reentry`) |

The solver has no runnable test today. Every other gap is unverifiable until this one closes.

## 2. Traceability matrix

Status: **present** (meets the rule), **partial** (a mechanism exists and misses part of the rule), **missing** (no mechanism). Surplus surface is section 3.

### 2.1 Time and the instant (§15)

| # | MSL rule | Solver component | Status | Note |
| --- | --- | --- | --- | --- |
| 1 | §15 T1 to T6: integer ticks, time resolution a session option, rounding rules | `digital/events.rs::DigitalEvent.time: f64`, `analyses/convergence.rs::PlanLimits.digital_time_epsilon` | missing | time is `f64` compared with an epsilon of 1e-12; no tick, no resolution option |
| 2 | §15 `E0502`: a scheduled time outside the tick range | none | missing | |
| 3 | §15 instants `(t, n)`, microstep loop steps 3.1 to 3.5 | `digital/scheduler.rs::evaluate_until_stable`, `evaluate_dag_ordered` | partial | a delta-cycle loop exists, with a two-phase seq/comb split; it has no apply/settle-structure/react stages, no microstep index, and the DAG variant applies an output in the same pass that produced it |
| 4 | §15 I2 snapshot: every reaction of a microstep reads one state; commits apply together | `digital/scheduler.rs` two-phase cycle | partial | holds for `seq_phase`; `comb_phase` outputs apply while later devices of the same pass still evaluate |
| 5 | §12.1.2 commit order: instant made, then source order, then execution order | `DigitalEvent::cmp` = `(time, region, seq)` | partial | `seq` restarts at 0 on every evaluation call and counts emission order, not source order |
| 6 | §12.1.4 transport per statement and level: a new commit removes later pending commits of the same statement | none | missing | events are keyed by net and source device; nothing is ever removed from the heap |
| 7 | §12.1.13, §12.1.17 `after d`, `d <= 0` is the next microstep, `+inf` never due, NaN is `E0435` | `digital/interface.rs::EventSink::emit(net, value, delay)` | partial | delay 0 is a same-time event; no validation, no error channel |
| 8 | §12.1.15 a reaction runs at most once per microstep | scheduler wake by `has_input_on` | present | one `seq_phase` and one `comb_phase` per device per cycle |
| 9 | §15 I7 `E0501`: bound on microsteps, a session option, the report names the carriers | `PlanLimits.max_delta_cycles` (1000), `Error::simple(Digital, ..)` | partial | bound exists and fails loud; `core/circuit.rs::run_digital_at_with_analog` hardcodes `PlanLimits::default()`, so the host cannot set it; the error names no carrier |
| 10 | §15 time-point procedure: solve the left value first, then collect, loop, settle | `analyses/transient.rs::attempt_step` | missing | digital events due at `t_next` run **before** the analog solve (`run_digital_at` then `execute_timestep`); there is no left value |
| 11 | §15 step 3.4, §15.2: the jump solve (backward Euler `h -> 0`, two passes, states continuous, impulsive flow) | none | missing | `TrBdf2Phase` has `Trapezoidal` and `Bdf2` only; a discrete change is absorbed by resetting `prev_h` (`accept_step`) and, for a host set, by re-solving the closed step (`apply_scheduled_sets`) |
| 12 | §15.2.6 integrator restart per state the jump moved | `transient.rs::accept_step` (`prev_h = 0` on a breakpoint) | partial | restart is global and unconditional, not per moved state; `propose_dt` restarts at 1e-3 of the last step, as C.2 states |
| 13 | §15 I6 lag: a reaction at `(t, n)` reads the solution after microstep `n - 1` | `EvalCtx.analog` | partial | reactions read the latest accepted solution; there is no per-microstep re-solve to lag behind |
| 14 | §15 event times that are not time points run without a solve (§17.2.2) | `transient.rs::predict_step` | missing | every digital event time is a mandatory landing point (`EventKind::Digital`, `Exact`); a clock nothing analog reads still cuts the analog step |
| 15 | §17.2.2 the dynamic test "does a reaction that counts fire at `(t, 0)`" | none | missing | needs per-reaction "touches analog" flags from the elaborator and a query on the queue |
| 16 | §15 step 4, §13 rule 17: the settle phase, once per time point | `core/element.rs::Element::on_digital_settled` | partial | a post-settle hook exists on accepted steps; it cannot commit |
| 17 | §15 S1 to S7: the settle commit, read by the solve that arrives at `t_next`, moves with a rejected step | none | missing | |
| 18 | §15 I8, §17.2.3: a rejected step leaves no trace | `digital/state.rs::checkpoint/rollback`, `Element::checkpoint_state/restore_state`, `EventQueue::checkpoint/rollback`, `NewtonRaphsonSolver::state_snapshot` | present | digital nets, queue, device state and analog history are restored |
| 19 | §15 I1 determinism | scheduler, event ordering | partial | `HashMap` and `HashSet` are used for membership only, which is safe; the ordering holes of rows 4 and 5 are observable |

### 2.2 Discrete carriers (§4, §5, §8.2.3, §12, §13)

| # | MSL rule | Solver component | Status | Note |
| --- | --- | --- | --- | --- |
| 20 | §5.1 a carrier holds a value of any `Repr` type, with a known flag per level | `digital/events.rs::LogicValue` (`Zero`, `One`, `X`, `Z`) | missing | one four-state scalar per net; MD-52 scalarizes vectors per bit; no reals, integers, structs or arrays |
| 21 | §5.2 one drive per writing module; a commit touches some levels and leaves the rest | `digital/drivers.rs::DriverTable` | partial | per-net, per-source driver records exist; the value is a `LogicValue` and there are no levels |
| 22 | §4, §5.2.10 `resolve` is a function of the nature, called over the drives in instance-path order | `LogicValue::resolve`, `drivers.rs::Strength::resolve` (LRM Table 7-3) | missing | resolution is hard-wired; a user `resolve` cannot be called |
| 23 | §5.2.11 `s.others()`: resolve of every drive but one's own | `DriverTable::resolved_by_kind` | missing | segregation is by ordinary/connect-module kind, not by reader |
| 24 | §8.2.3 definitions settle in dependency order; a loop among resolved drives relaxes, bounded by drive count plus one | `digital/topology.rs::DigitalTopology` (topo order, back edges) | partial | DAG order and back-edge restart exist; the bound is the global delta-cycle cap, not per loop |
| 25 | §5.3, §13 rule 15: `changed()` and `prev()`, change defined level by level with flags | scheduler compares `nets[i] != resolved` | partial | change detection exists for `LogicValue`; `prev()` is device-private edge memory |
| 26 | §13 rule 6: the first commit at `(0, 0)` is a change | `DigitalDevice::init` + `CircuitInstance::init_digital` | present | initial events are scheduled at 0 and propagate |
| 27 | §13 rule 4 `at(x)`: armed time, fires once per value, re-arms | `AnalogDevice::next_breakpoints` | partial | absolute breakpoints make the solver land; there is no scheduled-event object with re-arm rules and it is on the analog trait |
| 28 | §13 rule 16 `p.changed()` on a host `set` during a run | `transient.rs::ScheduledSet`, `SetQueue` | partial | scheduled param writes exist; they fire no event |
| 29 | §8.5 `extern "cosim"`: one reaction, returns commits and the next time it wants to run; never rolled back | `DigitalDevice::evaluate` | partial | the fused call exists; there is no "wake me at `t`" return, and a cosim device is advanced inside steps that can be rejected |

### 2.3 Operating point and analyses without time (§16)

| # | MSL rule | Solver component | Status | Note |
| --- | --- | --- | --- | --- |
| 30 | §16 step 3: Newton, then continuation that never changes the answer | `analyses/convergence.rs::ConvergencePlan` (gmin stepping, source stepping) | present | |
| 31 | §16 step 3: nodesets by path as Newton's starting point | `analyses/dc.rs::DcSolver::apply_initial_conditions` | present | |
| 32 | §16 step 4: rounds of an operating point, bounded, `E0506` when exceeded | `dc.rs::DcSolver::solve` mixed-signal loop | partial | the loop runs at most 20 rounds and then **returns the unconverged result silently**: a fail-loud hole |
| 33 | §16 O1, O2: `cross` compares successive rounds | `Element::accept_timestep` at `t = 0` | partial | devices see each round's solution; ordering of the firing microstep is not defined |
| 34 | §16 H1 to H6: held nodes, a hold through every round of a transient's operating point, released as the first step begins | `transient.rs::UicClamp`, `AnalogDevice::initial_conditions` | partial | a 1e12 S clamp exists; it is declared by a device per branch, not per node by the netlist, and H2's restart is implicit |
| 35 | §16.1 sweep points: discrete state carries from one point to the next, `settle` once per point | `DcSolver` (one point) | missing | sweeps live in `piperine-api` and restamp; whether digital state persists between two `DcSolver` runs is accidental, and nothing fires per point |
| 36 | §16.1.3 what waits for time never comes due; dropped at the end of the analysis | scheduler | partial | DC runs the queue at `t = 0` only; later events stay in the heap and leak into the next analysis |
| 37 | §16.1.4 a small-signal analysis linearizes at the fixed point, fails if it does not go quiet | `analyses/ac.rs`, `noise.rs` (DC point first) | partial | inherits row 32's silent exit |
| 38 | §16.4 continued transient: time, queue, discrete state, instance state and history kept | `TransientSolver::with_initial_state`, `TransientAnalysisOptions::with_start` | partial | analog values, nets and hidden digital state re-enter; the event queue and the analog history rows do not |
| 39 | §16.4.2 a `Dc`, `Ac` or `Noise` run at a transient's last instant | none | missing | `DcAnalysisState` carries no time; `now()` cannot stay |
| 40 | §11.1.1 `analysis(kind)` and `analysis(Static)` visible to the model | `analyses/context.rs::AnalysisKind`, `Element::start_analysis`, `load_dc` vs `load_transient` | partial | `Static` is the `load_dc` call itself; `AnalysisKind` has no `Noise`; the kind reaches a device only through `start_analysis` |

### 2.4 Convergence, steps, crossings, limiting (§17, §18)

| # | MSL rule | Solver component | Status | Note |
| --- | --- | --- | --- | --- |
| 41 | §17.1.1 update test with the nature's `abstol` per unknown | `context.rs::Tolerances::has_converged`, `node_vntol_overrides` | partial | per-node potential `abstol` is honored; flow `abstol` is one global scalar |
| 42 | §17.1.2 residual test: a node row against the flow `abstol`, a potential-branch row against the potential `abstol` | `Tolerances::residual_test` | partial | the code applies `vntol` to node rows and `abstol` to branch rows, the reverse of the rule; needs a decision before any change (it mirrors how the ngspice-parity suite was tuned) |
| 43 | §17.1.3, §18 rule 5: an active `limit` vetoes convergence; bypass never while limiting | `AnalogDevice::limiting_report`, `NonLinearSystem::any_limiting_report`, `ElementCapabilities::BYPASS_OK` | present | |
| 44 | §18 rules 1 to 4, 6, 7: limiter state, seed, RHS correction, advance on accepted steps only | `LimitingReport`, `ElementCheckpoint`, `CircuitInstance::apply_limiting_reports` | present | the limiting function itself is device code |
| 45 | §17.1.4 `E0507`: a non-finite value fails the solve and names the instance | `analyses/stamp_validation.rs`, `math/newton_raphson.rs` (guess check) | present | the error is a string, see row 62 |
| 46 | §15.2 `E0503`: a singular solve names the branches or node | `math/faer.rs` (symbolic LU error; non-finite result check) | partial | fails loud with a generic message; no row-to-name mapping |
| 47 | §17.2.1 truncation error within `trtol`, measured with the natures' tolerances and a `ddt` instance's own `abstol` | `math/integration.rs::TrBdf2::milne_lte_indexed`, `AnalogDevice::suggest_transient_step` | partial | Milne over node potentials with one global `vntol`; per-state LTE is a device hint; "skip a node that straddles a jump" is the breakpoint exemption of the whole step |
| 48 | §17.2 advance A1 to A5, PI controller, rejection, `dt_min` behavior | `transient.rs::{predict_step, attempt_step, assess_step, accept_step, reject_step}`, `convergence.rs::PiController` | present | |
| 49 | §17.2.5, §14.10 `bound_step(dt)` caps the next step; `E0509` | `AnalogDevice::bound_step_hint`, `EventQueue::push_step_hint` | missing | the hook is declared and **never read**: `predict_step` does not call it; no validation of `dt` |
| 50 | §17.3 located crossings: retake until `ttol` or `etol`, round up to a tick, earliest first, fire at `(t, 0)` | `analyses/events.rs::EventKind::Crossing` (never constructed outside tests), `digital/interface.rs::interpolate_a2d_crossing` | missing | a crossing fires at the first accepted point after the sign change (Appendix G.6 states the same) |
| 51 | §13 rule 5: `cross` across a jump and across sweep points | none | missing | depends on rows 11 and 35 |
| 52 | §11 rule 2 switch branch: a branch's kind changes between solves | `math/newton_raphson.rs` (`symbolic` built once from a dry-run load) | partial | a device can stamp either row form on a branch unknown it allocated; the sparsity pattern is fixed at construction, so the union of both forms must be in the dry run, and nothing states or checks that |
| 53 | §11 rule 19, §14 errors raised by model code during a solve (`E0418`, `E0453`, `E0454`, `E0509`, `E0510`) | `AnalogDevice::load_dc/load_transient/load_ac` return `Vec<Stamp>` | missing | a device cannot fail a solve with its own error; only non-finite stamps are caught |

### 2.5 Small signal and noise (§11.1, §14.3, §14.8, §16.2)

| # | MSL rule | Solver component | Status | Note |
| --- | --- | --- | --- | --- |
| 54 | §16.2 small-signal sweep: one complex linear solve per frequency, `ac_stim` phasors, `ddt` as `jω`, `delay` as `exp(-jω td)`, `freq()` | `analyses/ac.rs`, `AnalogDevice::load_ac`, `AcAnalysisContext.frequency` | present | complex stamps carry any linear response |
| 55 | §11.1.8, §11.1.9 a noise source is an instance: one instance read on several paths is fully correlated, and every operator applies its response to it | `analyses/noise.rs::Noise { terminals, value }` | partial | each source is one real current PSD across one terminal pair; a correlated source with a complex weight per injection row cannot be expressed |
| 56 | §8.5.4 noise on a potential contribution | `Noise.terminals` (current injection only) | missing | |
| 57 | §14.8.3 `psd` evaluated only in `Noise`, at the point's frequency | `AnalogDevice::noise_current_psd(dc_point, ac_context)` | present | |
| 58 | §16.3 host-built analyses over the linearization: transfer function, poles and zeros, S-parameters, DC sensitivity, periodic steady state | `analyses/{tf, pz, sp, sens, adjoint, sensitivity, pss}.rs` | present | they live in the solver today; see section 3 and Q5 |

### 2.6 Host seam, output, extern (§8.5, §19, §21, §23)

| # | MSL rule | Solver component | Status | Note |
| --- | --- | --- | --- | --- |
| 59 | §23.1 `set` of a param by path, before and during a run, with what it invalidates | `core/introspect.rs::Invalidation`, `CircuitInstance::set_element_param`, `TransientSolver::schedule_set` | present | a structural set is refused loud (`Invalidation::Rebuild`) |
| 60 | §23.1.1, .4 host drive of a discrete carrier, before the run and at a scheduled time; `force` and `release` on a carrier and on a node | none | missing | the host can only write params |
| 61 | §23.1.2, §17.2.6 host read or probe at a scheduled time makes it a time point; host waits for an event; `E0806` | `Element::on_digital_settled`, result containers | partial | values are read from recorded steps; no scheduled host action, no refusal of a past time |
| 62 | §15 to §18 run errors `E0501` to `E0510` carry the code, the instant and the names | `error.rs::Error { domain, detail: String }` | partial | typed domain, free-text detail; no code, instant or name fields |
| 63 | §19 `emit!`: typed records, instant and iteration attached, a reaction's record in a rejected step does not exist, an `analog` record stays, `finish` ends the analysis at its instant | `core/output.rs::SimOutput` (text sink), `core/validation.rs`, `core/margin.rs`, `core/coverage.rs` | missing | the existing channels are text output and VAMS-era verification reports; there is no record sink with accept/reject semantics and no way for a record to end an analysis |
| 64 | §21 metadata: params, operating-point variables at the last accepted solution, nodes and carriers by path | `core/introspect.rs` (`Introspect`, descriptors), `core/net.rs::Net` | present | naming and paths are the elaborator's; the solver carries labels |
| 65 | §8.5 `extern "osdi"`: loaded every iteration, limiting vetoes, state advances on accepted steps, opvars and noise in metadata | `Element` ABI (the `piperine-osdi` plugin implements it) | present | |
| 66 | §14.4 `temperature()`, `dtemp`, `simparam` session parameters | `AnalogDevice::set_temperature`, `Tolerances::{temperature, tnom}` | partial | temperature is there; there is no session-parameter map for `simparam` (`E0456`) |
| 67 | §23.3 sensitivities along a trajectory, across events (saltation), `E0807` to `E0809` | `analyses/adjoint.rs`, `sens.rs` (DC only) | missing | large; proposed as a later feature |
| 68 | §16.3 periodic small-signal and periodic noise | none | missing | the spec itself lists it as unimplemented and untraced (G.37); out of scope here |

### 2.7 Counts

| Status | Rows | Count |
| --- | --- | --- |
| present | 8, 18, 26, 30, 31, 43, 44, 45, 48, 54, 57, 58, 59, 64, 65 | 15 |
| partial | 3, 4, 5, 7, 9, 12, 13, 16, 19, 21, 24, 25, 27, 28, 29, 32, 33, 34, 36, 37, 38, 40, 41, 42, 46, 47, 52, 55, 61, 62, 66 | 31 |
| missing | 1, 2, 6, 10, 11, 14, 15, 17, 20, 22, 23, 35, 39, 49, 50, 51, 53, 56, 60, 63, 67, 68 | 22 |

Surplus items (section 3): 14.

## 3. Surplus: solver surface MSL does not need

"Minimal" cuts both ways. Each item serves a Verilog-AMS or PHDL construct with no MSL counterpart, or duplicates what MSL writes as ordinary model text.

| # | Surface | Location | Lines | Why MSL does not need it |
| --- | --- | --- | --- | --- |
| S1 | LRM chapter 8 stratified regions (`Region`, `RegionQueue`, the `region` key of `DigitalEvent`) | `digital/regions.rs` | 382 | MSL has no blocking writes, no `#0`, no regions (G.0); order is §12.1.2 |
| S2 | LRM Table 7-2/7-3 strengths and driver kinds (`Strength`, `DriverKind`, `emit_with_strength`) | `digital/drivers.rs` | 377 | `Strength` and `Logic::wired` are stdlib source (§22.3) over a generic `resolve` |
| S3 | `specify` path delays and timing checks (`DelayTable`, `TimingCheckTable`, `check_timing_violations`) | `digital/delays.rs`, `digital/state.rs` | ~330 | `Setup`, `Hold`, `MinPulse` are stdlib monitors (§22.5); delays are `after` |
| S4 | Four-state `LogicValue` as the only carrier value | `digital/events.rs` | - | replaced by typed carriers (row 20) |
| S5 | `$display`/`$fopen` text and file IO (`SimOutput`, `FileTable`, `McdTable`, `parse_memory_text`, `Context.io`) | `core/output.rs` | 840 | no format strings, no files read or written during a run (G.0); output is `emit!` records |
| S6 | Constraint and verification channels (`ValidationReport`, `MarginReport`, `CoverageReport`, `CheckPosture`, `CoverPosture`, `RequireScope`, `Element::{validation_reports, evaluate_constraints, margin_report, coverage_report, reset_margin_tracking}`, `EMITS_VALIDATION`) | `core/{validation, margin, coverage}.rs`, `core/element.rs`, `core/circuit.rs`, `analyses/context.rs` | ~450 | `require`, `cover`, `target` are stdlib functions that emit records on `checks`; the host counts them (§23.2) |
| S7 | Distortion by nonlinear currents (`DistoSolver`, `Disto2`, `Disto3`, `load_disto2/3`, `HAS_DISTO2/3`, `NUMERIC_JACOBIAN`) | `analyses/disto.rs`, `core/element.rs` | 1310 | §16.3: distortion is a transient plus the host's Fourier analysis; the Hessian kernels overrun Cranelift on MOS2/MOS3 already |
| S8 | `$discontinuity` hook | `AnalogDevice::discontinuity` | - | §14.13: there is none; a discontinuity is a carrier change, which is a jump |
| S9 | `@initial` UIC seeds declared by a device | `AnalogDevice::initial_conditions` | - | G.0: no `uic`; held nodes are netlist data (row 34) |
| S10 | `initial_step`/`final_step` hooks | `Element::{start_analysis, finish_analysis}` | - | G.0: no end-of-analysis event; the start is the first commit (§13 rule 6). The analysis kind still has to reach devices (row 40), so the hook's data survives in another shape |
| S11 | `$simprobe` | `core/introspect.rs::simprobe` | ~90 | no MSL construct; a model cannot read another instance's opvars |
| S12 | Seq/comb two-phase device contract and the A2D sampling flags (`seq_phase`, `comb_phase`, `SAMPLES_ANALOG`, `DEPENDS_ON_DIGITAL`, `interpolate_a2d_crossing`) | `core/element.rs`, `digital/interface.rs` | - | MSL reactions and definitions are not clocked/combinational processes; `A2D` and `D2A` are stdlib modules over `cross` and a carrier read |
| S13 | Runtime-operator observability for stateful VAMS operators (`runtime_banks`, `record_device_state`, `list_state_slot_names`, `list_force_terminal_pairs`) | `core/element.rs`, `analyses/transient.rs` | - | `idt`, `slew`, `transition`, `laplace_*`, `zi_*` are stdlib source over `ddt` and carriers; their state is nodes and carriers, already recorded. Keep only if `delay`/`limit` state must be host-visible |
| S14 | Unused dependencies | `Cargo.toml`: `rayon`, `rand_distr` (0 uses); `tracing-subscriber` and the Cranelift log filter in `Context::init_global`; `piperine-ir` (one function, `predict_crossing`); dev-dependencies `piperine-lang`, `piperine-codegen` | - | a core crate should be a leaf; the dev-dependencies are why no test runs |

Roughly 3.4k source lines (S1, S2, S3, S5, S6, S7, S11) plus their tests.

Candidates that stay, flagged: `tf`, `pz`, `sp`, `sens`, `adjoint`, `pss` are host procedures by §16.3. They need the assembled linearization and re-entry internals, and no host crate builds today, so moving them cannot be verified now (Q5).

## 4. Belongs elsewhere: not solver gaps

| Concern | Owner |
| --- | --- |
| Parsing, name resolution, type checking, const evaluation, generation, monomorphization, bundles, templates (§1 to §9, C.1.1) | `piperine-lang` and the elaborator |
| Known-flag propagation through operators, unknown control in reactions (§3.3, §12.1.19) | codegen. The solver stores the bytes of a carrier, flags included, and compares them (row 20) |
| Value inputs, results, named expressions: inlining and hoisting (§8.3) | elaborator |
| `m` and `dtemp` scaling of contributions, probes and noise (§8.5.4) | codegen |
| Symbolic differentiation, `ddx`, the Jacobian (§11 rule 5, §14.2) | codegen |
| The body of a `limit` function, `pnjlim`, `fetlim`, `limvds`, `limexp` (§18, §22.2.4) | stdlib source, compiled by codegen; the solver keeps the veto and the state lifecycle |
| Which kind a switch branch has in a solve, and `E0418` when both are enabled (§11 rule 2) | codegen: the device decides at the start of a solve and reports through the run-error channel (row 53) |
| `idt`, `idtmod`, `slew`, `transition`, `above`, `absdelta`, `last_crossing`, `laplace_*`, `zi_*`, `table_model`, `inertial`, `every`, `since`, `init` (§14.13, §22) | stdlib source. They need rows 11, 17, 27, 50 and nothing of their own |
| `delay` history buffer and its interpolation (§14.6) | device runtime in codegen; the solver gives accepted-step and rollback hooks (row 18) |
| `white_noise`, `flicker_noise`, `noise_table`, `vt`, `db10`, `normal`, the constants (§22.2) | stdlib source |
| `D2A`, `A2D`, `Bidir`, `Wire`, `Tri`, `SumNet`, `Tgate`, `Keeper`, strengths (§22.3, §22.4) | stdlib source over `resolve` and `cross` |
| Monitors, `require`, `cover`, `target`, `log`, `fatal` (§22.5, §22.6) | stdlib source over `emit!`; counting and judging is the host's (§23.2) |
| `parse_table`, `parse_hex`, `include_str!` (§20) | elaboration |
| Hierarchical paths, the `<module>#<k>` naming, gradient-param dependency analysis (§21) | elaborator |
| Reachability of a reaction to the analog side (§17.2.2, static half) | elaborator; the solver consumes one flag per reaction |
| Checking a `set` against `where`, structural params (`E0802`, `E0803`) | elaborator metadata plus the host; the solver refuses `Rebuild` |
| Sweeps, Monte Carlo, optimization, regression, scoreboards, Fourier, waveform math, result formatting, files, Python (§23) | `piperine-api` and `piperine-python` |
| Loading an OSDI library, descriptor checks (`E0372` to `E0374`) | the `piperine-osdi` plugin and the elaborator |
| The C interface of `"cosim"` (G.65) | open in the spec; the solver offers the generic wake request only (row 29) |
| JIT, Cranelift, kernels, the `SimCtx` ABI | codegen |

## 5. Ranked gap summary

What blocks connecting MSL, most blocking first.

1. **No runnable solver test** (section 1). Nothing below can be proven until the suite builds without the dead frontend.
2. **The instant model** (rows 1 to 7, 9, 19): integer ticks, `(t, n)`, the three-stage microstep, commit ordering and per-statement transport. Every `digital` block depends on it.
3. **Typed carriers, drives and `resolve`** (rows 20 to 25). Without them only four-state scalar nets exist; `Bit<f64>`, `Bit<u8>`, structs and every resolved net of §22.3 have nowhere to live.
4. **The mixed time point** (rows 10 to 17): left value, the jump solve, time points versus event times, the settle phase and its commit. This is what makes a `Switch`, a `SampleHold`, `idt` with reset, `transition` and `zi_nd` correct. `transition`, `zi_nd` and `inertial` are the spec's own conformance set (C.2.3).
5. **Operating point with rounds, held nodes, sweep points, continuation** (rows 32 to 39). Row 32 is a silent wrong answer today.
6. **Crossings and `bound_step`** (rows 49, 50, 51). `cross` is the only analog-to-digital path in MSL; `bound_step` is declared and unwired.
7. **Model-raised run errors and structured errors** (rows 53, 62, 46). Small, and every other piece reports through them.
8. **Host seam and records** (rows 60, 61, 63, 66): host drives, force/release, scheduled probes, the record sink with `finish`.
9. **Numeric conformance** (rows 41, 42, 47, 52, 55, 56): per-nature tolerances, LTE per state, switch-branch pattern, correlated noise.
10. **Later features**: trajectory sensitivities across events (row 67), periodic small-signal (row 68).

## 6. Proposed split

The work is too large for one feature (53 open rows, a new event kernel, a new solve kind). Six ordered sub-features, each testable with hand-written Rust `Element`s, no frontend needed:

| Order | Sub-feature | Closes | Depends on |
| --- | --- | --- | --- |
| 1 | `solver-baseline`: the suite builds and runs with no in-tree dev-dependency; the surplus of section 3 is removed | section 1, section 3 | - |
| 2 | `solver-instants`: ticks, the event kernel, typed carriers, drives, `resolve`, commits, the microstep loop, `E0501`/`E0502`, structured run errors | rows 1 to 9, 19 to 28, 53, 62 | 1 |
| 3 | `solver-time-point`: left value, the jump solve, per-state restart, time points versus event times, settle and settle commits, the operating-point rounds, held nodes, sweep points, continuation | rows 10 to 17, 32 to 40 | 2 |
| 4 | `solver-crossings`: located crossings, `cross` across jumps and sweep points, `bound_step` | rows 49 to 51 | 3 |
| 5 | `solver-host-seam`: host drives, force/release, scheduled probes and waits, the record sink and `finish`, session parameters, the cosim wake request | rows 29, 60, 61, 63, 66 | 3 |
| 6 | `solver-numerics`: per-nature tolerances, named singular solves, LTE per state, switch-branch pattern, correlated and potential noise | rows 41, 42, 46, 47, 52, 55, 56 | 1 |

Sub-features 4, 5 and 6 are independent of each other. After 1 to 5 the elaborator and codegen have an engine for every dynamic rule of Parts III to VI. Rows 67 and 68 stay out (see `spec.md`, Out of Scope).

## 7. After the user's answers (2026-11-14)

The matrix above is the state of the code and is unchanged. The decisions in `context.md` (rounds 1 and 2) change what closing a row means and how the work is cut.

| Row | Was | Now |
| --- | --- | --- |
| 10, 11, 12, 13 (left value, jump solve, per-state restart, lag) | missing or partial against §15 and §15.2 | the solver's present discontinuity handling is kept (Q7). Appendix C.2 is amended to describe it, and what §15.2 says normatively is an open question of feature (B) |
| 14, 15 (event times that are not time points) | missing | Q8: a digital event cuts an analog step only when it changes something analog reads. Closed by feature (B) |
| 42 (residual tolerances reversed) | partial | the present mapping is kept for ngspice parity (Q9); §17.1.2 is amended. Per-row tolerances given at build time remain a gap (numerics) |
| 3, 4, 24 (microstep loop, snapshot, relaxation) | partial | the digital side becomes a levelized DAG propagated to a fixed point, Verilator-style (the user's vision), in feature (B) |

Section 3, per feature:

| Surplus | Removed by | Why there |
| --- | --- | --- |
| S1 regions, S3 `specify` delays and timing checks, S5 text and file IO, S6 verification channels, S7 `disto`, S11 `$simprobe`, S14 dependencies | (A) `solver-restructure` | none of them changes the result of a surviving test |
| S2 strengths and `DriverTable`, S4 four-state `LogicValue`, S12 seq/comb contract and A2D flags | (B) `solver-event-boundary` | they are the live digital logic; replacing them is the logic change |
| S8 `discontinuity`, S9 `initial_conditions`, S10 `start_analysis`/`finish_analysis`, S13 runtime banks | `solver-time-point` | they are part of the kept discontinuity and start-up paths; revisited with held nodes and continuation |

Section 6, replaced by:

| Order | Feature | Kind | Builds / verifies |
| --- | --- | --- | --- |
| A | `solver-restructure`: suite runs alone, dead crates and surplus removed, three-crate split | structure, no logic change | Sonnet / Opus |
| B | `solver-event-boundary`: typed carriers, levelized DAG propagation, event-only analog/digital boundary, ticks, typed run errors; the spec amendments of Q7 and Q9 | logic | Opus / Fable |
| 3 | `solver-time-point`: settle, operating-point rounds, held nodes, sweep points, continuation | logic | to be set |
| 4 | `solver-crossings` | logic | to be set |
| 5 | `solver-host-seam` | logic | to be set |
| 6 | `solver-numerics` | logic | to be set |

Crates of the workspace after (A): kept `piperine-lang`, `piperine-project`, `piperine-solver`, `piperine-analog-solver`, `piperine-digital-solver`, `piperine-api`, `piperine-cli`, `piperine-lang-server`; removed `piperine-ir`, `piperine-codegen`, `piperine-python`, `piperine-plugin`, `piperine-plugin-macros`.
