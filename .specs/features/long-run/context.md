# Long Run Context

**Gathered:** 2026-11-17
**Spec:** `.specs/features/long-run/spec.md`
**Status:** Decisions 1 to 7 bind. Four questions are open (spec.md, "Open questions for the user").

The user's decisions reached the planner through the orchestrator. One sentence arrived verbatim and is quoted as such; the rest arrived as the orchestrator's statement of what the user decided and is recorded here in that wording, not as a quote.

---

## Feature Boundary

A transient run of any length keeps only the signals it asks for, on the output grid it asks for, in memory that does not grow with the number of solver steps. The digitally controlled buck (`examples/buck`) then runs its whole 600 µs start with the full `vout` waveform, and a host test in Rust measures that waveform against `examples/buck/SPEC.md`.

---

## Implementation Decisions

### The goal (user, 2026-11-17, intent as relayed)

Run the digitally controlled buck long: the whole 600 µs startup with the full `vout` waveform, and test it completely.

### The seven decisions (user, 2026-11-17, relayed by the orchestrator)

| # | Topic | Decided | The user's words |
| - | ----- | ------- | ---------------- |
| 1 | Layering | `emit!` carries data out to a stream and is the language's only output channel. Sampling and observation policy (which signals, the output grid, the window, interpolate or hold or drop) is not language: it belongs to the run, which is the `[run.<name>]` table of `Piperine.toml` and the matching command-line flags. Every new observation option is a run-table key with its flag, under the key and flag pairing of `msl-cli` CLI-03. | "Coloca no manifest também" |
| 2 | What a run records | Only what it asks for (its `print` columns, the requested signals), never the full snapshot. Streaming: memory is constant in the run's length, except for the output rows. | relayed |
| 3 | The print grid | The grid is the user's output step only. The internal step stays the solver's (truncation error, breakpoints, `max_step`). The grid is applied while the run goes: an analog signal is interpolated linearly, a digital signal holds its last committed value. It behaves like ngspice's TSTEP. Rows never extrapolate. | relayed |
| 4 | Waveform measurements | Overshoot, settling into ±10 mV and ripple are computed by a host test in Rust through `piperine-api`, reading the run's table. They are not MSL and not a `.meas` of the manifest. `piperine test` keeps judging MSL check records only. | relayed |
| 5 | The fuller buck test | The whole startup to 600 µs on a fine grid, measuring overshoot, settling and ripple against the limits of `examples/buck/SPEC.md`, plus the long-run checks that make sense: a memory bound, and the wall time reported, not asserted. | relayed |
| 6 | Out of scope (future) | Multi-rate stepping (digital edges that do not cut analog steps), surrogate or averaged models, periodic steady state (the next feature), and `on above { emit! }` (`P0001`), which goes to the gaps summary. | relayed |
| 7 | Parity | ngspice parity is the top priority. The 7 twins stay byte-identical where the feature does not touch them. The grid interpolation is cross-checked against ngspice's TSTEP output where that is meaningful. No tolerance is retuned. | relayed |

### What earlier decisions already fix

- The command line is a shell over `piperine-api`; a run is a `[run.<name>]` table whose keys are also flags, and a flag wins (MD-65). Every rule of what a run prints is a `piperine-api` method with a Rust test.
- `piperine-project` accepts and locates the `[run]` table and does not read it; `piperine-api` reads it (MD-65 item 2).
- The solver is three crates and stays simple; the owner (`piperine-solver`) runs the protocol between the two sub-solvers (MD-60). The device ABI is the only device contract (MD-62).
- ngspice parity is measured, never asserted; a numeric tolerance is written only in the `ToleranceContract` (MD-38).
- A test target lives in the crate it exercises (MD-28).
- No macro magic: data and plain helpers (project memory, AGENTS.md rule 5).

### Agent's Discretion

Every technical choice: where the solver hands an accepted step to the host, how the recorder is built, the names of the new types, the memory figures, the test layout, the fine grid's value. Each is a row of the spec's Assumptions table or of the design's Tech Decisions table, open to correction.

### Declined / Undiscussed Gray Areas

None declined. Four gray areas are the user's and are asked, not assumed: spec.md, "Open questions for the user" (Q1 to Q4).

---

## Specific References

- ngspice's `.tran tstep tstop tstart` with `.options interp`: rows on multiples of `tstep`, interpolated from the internal steps; `tests/ngspice/tran_rc.cir` already exports that grid.
- `examples/buck/SPEC.md`: §2 (regulation ±1 % or ±10 mV, ripple ≤ 10 mV peak to peak), §3.3 (ripple estimate), §7.1 and §7.2 (setpoint code 0 is 0.8 V, 128 ADC codes, a ramp of 0.256 ms), §8.6 (power-good 128 µs after `Regulate`), §10 S1 (overshoot ≤ 15 mV, timing tolerance ±0.05 ms).
- `GAPS.md` §6: "A transient keeps every accepted step" and "Re-entry state is stored on every accepted step (N2)", about 58 KB per step.
- `msl-cli` contract D10: `from` (`record_from`) as the one bound available without a solver edit.

---

## Deferred Ideas

- Multi-rate stepping: digital edges that do not cut analog steps.
- Surrogate or averaged models of the switching stage.
- Periodic steady state on the device ABI (the next feature).
- `on above(..) { emit!(..) }`, refused today with `P0001` (`GAPS.md` §4).
- Rows written straight to a file as they are made, so that even the output rows are not held.
- Waveform measurements as a library of `piperine-api` (overshoot, settling, ripple as methods): this feature computes them in the test.
- The buck's scenarios S1b to S9, which need host stimulus during a run (`ROADMAP.md` queue item 4).
