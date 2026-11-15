# Batch worker brief — msl-stdlib

You are a BATCH WORKER for feature `msl-stdlib` in the worktree /home/keslarek/Git/piperine-msl-stdlib
(branch `feature/msl-stdlib`). Work only there.

1. Activate the skill `tlc-spec-driven` by name (Skill tool) and follow its Execute flow
   (references/implement.md) as a batch worker: no sub-agents, no Verifier, no push, no merge.
2. Read: `.specs/features/msl-stdlib/{spec.md,design.md,tasks.md}` (your tasks, the Test Coverage
   Matrix, Gate Check Commands, `## Checks` — test functions MUST use exactly the names the checks
   cite so `test <name> ... ok` matches), the skill's `references/coding-principles.md`, the code
   already in `crates/piperine-lang/src/{msl,pom}/`, `crates/piperine-project/src/`, `context.md` (user decisions and answers, binding), and the docs/spec sections your tasks cite.
3. Per task: assumptions/files/success → tests from the spec ACs (never from the implementation) →
   implement → task gate → mark complete in tasks.md → `python3
   /home/keslarek/.claude/skills/tlc-spec-driven/scripts/check_commit.py --message "<msg>"` → one
   atomic commit (code + tests + tasks.md), message = the task's **Commit** line + body, trailer
   `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

Rules:
- Cargo: never set CARGO_TARGET_DIR (the environment provides a shared one); never `cargo clean`,
  never `--workspace`. Only `-p piperine-lang` / `-p piperine-project`.
- `piperine-lang` builds with zero warnings at every commit (`**Build**` line). Other workspace
  crates are already broken — ignore them.
- No macros of your own (`macro_rules!`/proc-macros); derives are fine. Plain data + helpers.
- Tests: every new target opens with a `//!` scope header; no `#[ignore]`; no file-scope `#![allow]`.
- Never weaken, skip or delete a test. A test you believe wrong per the spec → stop and report.
- After any `lessons.py` call, run `git status`; restore `.specs/lessons.json`/`LESSONS.md` if touched.
- Blocked or red gate you cannot fix → stop the batch and report.

Final message (≤12 lines): tasks with commit hashes, test counts per target, deviations (with
`SPEC_DEVIATION` markers placed), spec-precision gaps, blockers.
