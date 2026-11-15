# Batch worker brief — ir-interp

You are a BATCH WORKER for feature `ir-interp` of M1 (the first MSL to solver path). The worktree and the
branch you work in are given in your dispatch message; work only there. Wave 2: `msl-ir` is merged into your worktree before you start (T1 needs nothing but `piperine-abi`); if `crates/piperine-ir` is missing when T2 starts, stop and report.

1. Activate the skill `tlc-spec-driven` by name (Skill tool) and follow its Execute flow
   (references/implement.md) as a batch worker: no sub-agents, no Verifier, no push, no merge.
2. Read: `.specs/features/ir-interp/{spec.md,design.md,tasks.md}` (your tasks, the rules at the top of
   tasks.md, the Test Coverage Matrix, Gate Check Commands, `## Checks`: test functions MUST use exactly
   the names the checks cite), `.specs/features/e2e-m1/overview.md` (the slice, the order, what M1
   covers), `.specs/features/codegen/context.md` (user decisions C1 to C10, S1 to S10 and the answers
   Q1 to Q8: binding), the skill's `references/coding-principles.md`, `CLAUDE.md`, `AGENTS.md`, and the
   code your tasks touch.
   Also read `.specs/features/device-abi/abi.md` in full (the contract), `.specs/features/msl-ir/design.md`
   (the input model), `crates/piperine-abi/src/{device,thunk,shape,verify,validate}.rs`,
   `crates/piperine-solver/tests/helpers/abi_devices.rs` and `tests/limiting_report.rs` (how devices are
   written and driven today), `crates/piperine-analog-solver/src/device/flow.rs` (the fold the affine
   rule must match).
3. Per task: assumptions/files/success → tests from the spec ACs (never from the implementation),
   run red first where the task adds behaviour or a guard → implement → the task's gate → mark it
   complete in tasks.md → `python3 /home/keslarek/.claude/skills/tlc-spec-driven/scripts/check_commit.py
   --message "<msg>"` → one atomic commit (code + tests + tasks.md), message = the task's **Commit**
   line + a body (note the red run, or why none applies), trailer
   the `Co-Authored-By:` line your session gives.

Rules:
- Do only the tasks of your batch, in order. Each task names its **Executor**; if a task of your batch
  names another model than you, stop before it and report.
- Follow the feature's spec and design exactly. The interfaces of `design.md` are contracts other M1
  features are written against in parallel: a change you believe necessary → stop and report.
- Fail loud: an unhandled form is a diagnostic, a `Fault` or a `check()` entry, never a silent default.
- ngspice parity is the proof of M1. A numeric assertion or tolerance is never loosened or retuned; a
  number you cannot explain → stop and report.
- Cargo: never set CARGO_TARGET_DIR (a shared one is configured); never `cargo clean`; never
  `--workspace`. Only `-p` of the crates your tasks name (piperine-abi, piperine-interp; piperine-solver, piperine-analog-solver and piperine-ir are read and tested, not edited).
- Zero warnings on every crate the `**Build**` line names, at every commit. The crates that do not build
  yet (root shell, project, api, cli, lang-server) are ignored.
- AGENTS.md idiom rules bind: no loose function, no macro of your own (derives are fine), modules named
  after what they do, no `unwrap()`/`expect()` on a user-input path.
- Tests: every target opens with a `//!` scope header; no `#[ignore]`; no file-scope `#![allow]`.
  Never weaken or delete an existing test. One you believe wrong → stop and report.
- Frozen: `crates/piperine-lang/src/msl/`, `src/pom/`, `std/*.msl`, `tests/ngspice/*.cir` and `*.msl`,
  `tests/fixtures*`. Never touch /home/keslarek/Git/piperine or another worktree.
- After any `lessons.py` call, run `git status`; restore `.specs/lessons.json` and `.specs/LESSONS.md`
  if they changed.
- Blocked, or a red gate you cannot fix → stop the batch and report. Never skip a task.

Final message (≤12 lines): tasks with commit hashes, test counts per target, deviations, places where
the spec or the design was not precise enough, blockers.
