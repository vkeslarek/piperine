# Piperine Documentation

Welcome to the Piperine documentation.

Piperine's frontend is **Verilog-AMS** (LRM 2.4) — the authoritative
grammar/semantics source is `VAMS-LRM-2-4.md` at the repo root. PHDL, the
project's own earlier language, has been retired in full (MD-44/MD-45).

Choose a section from the navigation menu:
* **Specification**: `docs/spec/` — Piperine's own architecture spec
  (elaboration, solver, host API, plugins, codegen pipeline), written
  against the now-retired PHDL originally and retained as
  historical/architectural reference; see its own supersession notice
  (`docs/spec/index.md`).
* **Reference**: `docs/reference/` — Verilog-AMS LRM Annex G (change
  history) and Annex H (glossary), ported as informative reference
  material.

A user manual (CLI, library, and toolchain guides) is not written yet — it is
tracked as a backlog item in `ROADMAP.md` (P6). Until then, `README.md`,
`CLAUDE.md`, and the worked examples in the specification's Appendix A are the
usage documentation.
