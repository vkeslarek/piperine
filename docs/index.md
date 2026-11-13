# Piperine Documentation

Welcome to the Piperine documentation.

Piperine's frontend is **Verilog-AMS** (LRM 2.4) — the authoritative
grammar/semantics source is `VAMS-LRM-2-4.md` at the repo root. PHDL, the
project's own earlier language, has been retired in full (MD-44/MD-45).

Choose a section from the navigation menu:
* **Specification**: `docs/spec/` — the Piperine mixed-signal language
  (MSL) specification: a Rust-like language for analog, digital and
  mixed-signal models (11 concepts). It is the target design for the next
  frontend; the current Verilog-AMS frontend is not described by it.
  Design record: `.specs/features/msl-spec/`.
* **Reference**: `docs/reference/` — Verilog-AMS LRM Annex G (change
  history) and Annex H (glossary), ported as informative reference
  material.

A user manual (CLI, library, and toolchain guides) is not written yet — it is
tracked as a backlog item in `ROADMAP.md` (P6). Until then, `README.md`,
`CLAUDE.md`, and the worked examples in the specification's Appendix A are the
usage documentation.
