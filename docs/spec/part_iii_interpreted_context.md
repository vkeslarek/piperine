# Part III — Interpreted Context (Bench) — REMOVED

**This part was deleted (bench-removal, 2026-07-17).** The in-language
`bench` block no longer exists: the keyword is a plain syntax error, the
interpreter is gone, and the `piperine-bench` crate is deleted.

Everything the interpreted context did — running analyses, measuring through
result objects, staging parameter overrides, sweeping, asserting — is now
done by the **host APIs**: Python (`import piperine`, the scripting host)
or Rust (`piperine-api`; the root `piperine` crate re-exports it). See
**Part VIII — Host APIs**, and the
runnable gallery in `examples/*.py` (one twin per `.phdl` circuit).

The normative text survives in git history (last present at the
`solver-live-params` cycle close).
