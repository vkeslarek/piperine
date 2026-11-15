# Batch worker brief — solver-restructure

You are a BATCH WORKER for feature `solver-restructure` in the worktree
/home/keslarek/Git/piperine-solver-completeness (branch `feature/solver-completeness`). Work only there.

1. Activate the skill `tlc-spec-driven` by name (Skill tool) and follow its Execute flow
   (references/implement.md) as a batch worker: no sub-agents, no Verifier, no push, no merge.
2. Read: `.specs/features/solver-restructure/{spec.md,design.md,tasks.md}` (your tasks, the Test
   Coverage Matrix, Gate Check Commands, `## Checks` — test functions MUST use exactly the names the
   checks cite), `.specs/features/solver-completeness/context.md` (user decisions, binding), the
   skill's `references/coding-principles.md`, and the code your tasks touch.
3. Per task: assumptions/files/success → tests from the spec ACs (never from the implementation),
   run red first where the task adds behaviour or guards → implement → task gate → mark complete in
   tasks.md → `python3 /home/keslarek/.claude/skills/tlc-spec-driven/scripts/check_commit.py
   --message "<msg>"` → one atomic commit (code + tests + tasks.md), message = the task's **Commit**
   line + body (note the red run, or why none applies to a pure move/delete), trailer
   `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

Rules:
- STRUCTURE ONLY: no solver logic change. Moved code keeps behaviour; ported tests keep assertions.
- Cargo: never set CARGO_TARGET_DIR (shared one provided); never `cargo clean`, never `--workspace`.
  Only `-p piperine-lang`, `-p piperine-solver`, `-p piperine-analog-solver`, `-p piperine-digital-solver`.
- Kept green crates build with zero warnings at every commit (`**Build**` line). Crates the spec lists
  as expected-broken (project, cli, lang-server, api, root shell) are ignored.
- No macros of your own (`macro_rules!`/proc-macros); derives are fine.
- Tests: every target opens with a `//!` scope header; no `#[ignore]`; no file-scope `#![allow]`.
- Never weaken a test. Delete a test ONLY where a task names it (user approved those deletions);
  log removed tests where the task says (`removed.txt`). A surviving test you believe wrong → stop.
- Never touch /home/keslarek/Git/piperine or other worktrees.
- After any `lessons.py` call, run `git status`; restore `.specs/lessons.json`/`LESSONS.md` if touched.
- Blocked or red gate you cannot fix → stop the batch and report.

Final message (≤12 lines): tasks with commit hashes, test counts per target, deviations, spec-precision
gaps, blockers.
