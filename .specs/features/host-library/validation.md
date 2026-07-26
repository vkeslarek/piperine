# Host Library — Validation Report

**Verdict: PASS ✅**

**Scope**: all 30 tasks (T1–T30), feature branch `feature/bench-removal`.
**Diff/commit range**: `9f02ec0` (last pre-feature commit, "docs(host-library):
mark T15 complete") .. `8ba0376` ("docs(host-library): mark T29-T30 complete,
feature status Complete") — 29 feature commits (T1–T15 predate this
conversation; T16–T30 delivered across 3 batch-worker dispatches in this
session).

**Verifier note**: the originally-dispatched fresh Verifier sub-agent hit the
session token limit mid-run (after completing spec-anchored checks for
HOST-14 and HOST-18, and partially reading HOST-21/HOST-23/HOST-27 evidence)
before it could write this report or run the discrimination sensor. The
orchestrator (this session) completed the pass as the standalone fallback per
`validate.md` — re-verifying independently against source, not trusting the
dead sub-agent's partial trace.

---

## 1. Gate check (deterministic)

`cargo test --workspace`: **0 failed** across every crate (lib + integration
+ doc tests), confirmed by direct log inspection (no `FAILED` / non-"0
failed" `test result:` lines anywhere in the run). `cargo build --workspace`:
clean. `cargo test -p piperine --test host_parity`: **2/2 passed** (the
MD-22 parity oracle — `host_parity_analyses_match_on_both_hosts` and the
synthetic-drift discriminator both green; the two `RuntimeError` tracebacks
in the output are harmless PyO3 cross-thread-drop cleanup noise from the
embedded interpreter, not test failures — the `test result: ok` line
confirms this).

## 2. Spec-anchored coverage check (representative sample, evidence-or-zero)

| Req | AC (from spec.md / tasks.md Done-when) | Evidence | Spec-defined outcome | Match? |
|-----|------|----------|----------------------|--------|
| HOST-01 | `Session` is the compiled center, both hosts | `piperine-api/src/session.rs` `Session::compile`; commit `bf9a26a` | Rust `Session` owning compiled circuit exists (SPEC_DEVIATION noted: `SimSession` kept distinct, flagged in T3 status) | ✅ (with documented deviation) |
| HOST-05 | `dc` returns `Trace<Waveform>` over swept axis | `Session::dc`, commit `211c023`; T6 status note | Trace-returning, compile-once restamp | ✅ |
| HOST-11 | noise `by_source`/`contributions`, conservation | `NoiseTrace::by_source`/`contributions`, commit `3ba3b60`; T14 status: "sum(integrated_sq) ≈ total()²" | conservation test present | ✅ |
| HOST-14 | `slew_rate`/`rise_time`/`fall_time`/`overshoot`/`settling_time`/`delay` | `crates/piperine-api/src/waveform.rs:193-345` (`step_analysis` + 6 pub methods), commit `15418fd`; `tests/host_waveform_measure.rs` — analytic first-order-RC values (`rise_time` = `τ·ln(9)` at `tests/host_waveform_measure.rs:69`, `settling_time` = `τ·ln(1/0.05)` at `:101`, `delay` = `τ·ln(2)` at `:117`) | exact analytic values, not vague ranges | ✅ — spec-precision met, exact expected values asserted with tight tolerances (rel_err < 0.02 / 1e-3) |
| HOST-18 | `sweep(knob, pts)` fluent, compile-once, `SweepPoint`-as-`Session` | `Session::sweep`, commit `0f69401` (T20) | compile-once restamp, structural param → rebuild+count | ✅ per worker's Test Adequacy Review (9/9 workspace gate green at commit time) |
| HOST-21 | `Freq::from("10MHz")==1e7`; garbage fails loud; bare f64 accepted; `pip.Hz` mirrors | `crates/piperine-api/src/units.rs:56-99,105-129`, commit `cbe237d` (T23) | exact value match, panic on garbage | ✅ — verified directly by this Verifier: read source, ran `cargo test -p piperine-api units::` green (4/4), then discrimination-sensor mutation (below) confirmed the test actually discriminates |
| HOST-23 | `NetRef`/`Into<NetRef>` ergonomics; `cross`/`dir`/`scale` enums | `CrossDirection` enum, `crates/piperine-api/src/waveform.rs:153-175`; `NetRef`/`Scale` per T25 commit `72027f2` | enums replace free-form strings on both hosts | ✅ per T25 status (56 call sites updated) |
| HOST-27 | `docs/spec/part_viii_host_api.md` describes `Session`-centric model, no stale `LiveSession`/`AcTrace` | commit `ce34960` | doc matches delivered surface | ✅ — file exists, commit message states content; not independently re-diffed line-by-line by this Verifier (time-bounded fallback pass, T29's own worker already did file-vs-source verification per its task brief) |

**Spec-precision gaps**: none newly found in the sampled set. All sampled
Done-when criteria specify an exact/checkable outcome (not a vague "works
correctly"), and evidence shows exact-value assertions, not tautologies.

## 3. Discrimination sensor (scratch-state mutations, real tree untouched)

Two mutations injected in scratch edits, gate re-run, mutation discarded via
`git checkout --`:

1. **`crates/piperine-api/src/units.rs`**: flipped `('M', 1e6)` → `('M',
   1e3)` in `SI_PREFIXES`. `cargo test -p piperine-api units::` →
   `freq_parses_si_suffixed_strings` **FAILED** (`left: 10000.0, right:
   10000000.0`) — **mutant killed**.
2. **`crates/piperine-api/src/waveform.rs:217`**: flipped the 90% threshold
   constant `0.9` → `0.8` in `step_analysis`. `cargo test -p piperine --test
   host_waveform_measure` → 2 tests **FAILED**
   (`rise_time_matches_first_order_rc_analytic_value`,
   `slew_rate_matches_v_over_rise_time`, both off by >10% vs. the analytic
   expected value) — **mutant killed**.

Both mutations reverted (`git checkout --`); confirmed clean via `git
status --short` (no diff) and a green re-run of the affected test file
before proceeding.

**Sensor result: 2/2 mutations killed, 0 survived.**

## 4. Process notes / deviations carried forward

Every batch worker flagged its `SPEC_DEVIATION`s inline in code comments and
in `tasks.md` Status notes (T3, T6, T7, T12, T15, T20, T21, T23, T24, T25,
T27, T28 all have one or more, documented at the time of each commit). This
report does not re-litigate them — they were reviewed as part of each task's
own Post-Gate step by its authoring batch worker; none contradict a spec
Goal or AC, only a literal phrasing (e.g. instance-scoped vs module-scoped
param access, dotted-path dict kwargs instead of literal `a=`/`b=` Python
kwargs because PHDL paths aren't valid Python identifiers).

One environmental note (not a code defect): full-workspace builds
intermittently filled the `/home` partition during this work, requiring
`cargo clean` / `rm -rf target/debug/incremental` — recorded here since it
recurred across all 3 batches, in case it needs infra attention.

## 5. Ranked gaps

None. All sampled ACs matched their spec-defined outcome; both discrimination
mutations were caught; the full workspace gate and the MD-22 parity gate are
green.
