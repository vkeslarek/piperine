# docs/spec authoring guide — Piperine mixed-signal language (MSL)

Source of truth: `.specs/features/msl-spec/duel/spec_v5.md` (the converged design; 11 concepts,
65 constructs). Your job is to turn it into a **technical specification**: precise, complete,
internally consistent, normative. You do NOT redesign the language. You MAY NOT add concepts,
keywords, operators, attributes or macros. Where v5 is vague or silent, define the behavior in the
way that is most consistent with the rest of v5 and mark it with a footnote-style note
`> **Resolution.** …` so the reviewer can find every place you decided something. Where v5 lists
something as open (Appendix G, changelog_v5 "Left open", critique_v4 "Residual open items"),
keep it open in Appendix G — don't invent a resolution silently.

## Files and ownership (docs/spec/)

| File | Content (v5 §) | Error-code prefix |
|---|---|---|
| `index.md` | §0: scope, notation, conformance terms, the 11 concepts table, "who writes what", reading guide, file map | — |
| `part_i_values.md` | §1–3 | `E01xx` |
| `part_ii_carriers.md` | §4–7 | `E02xx` |
| `part_iii_structure.md` | §8–9 | `E03xx` |
| `part_iv_behavior.md` | §10–14 | `E04xx` |
| `part_v_execution.md` | §15–18 | `E05xx` |
| `part_vi_io.md` | §19–21 | `E06xx` |
| `part_vii_stdlib.md` | §22 | `E07xx` |
| `part_viii_host.md` | §23 | `E08xx` |
| `appendix_a_lexical.md` | App. A | `E09xx` |
| `appendix_b_grammar.md` | App. B (B.1 added-to-Rust table, B.2 placement matrix, B.3 complete EBNF of additions) | — |
| `appendix_c_implementation.md` | App. C (lowering to AIR, reference implementation) | — |
| `appendix_d_gallery_analog.md` / `appendix_e_gallery_digital.md` / `appendix_f_gallery_mixed.md` | App. D/E/F | — |
| `appendix_g_scope.md` | App. G + all residual open items, each with an id `G.n` | — |

Drop v5 Appendix H (duel change log); provenance lives in `.specs/features/msl-spec/duel/`.

## Numbering and cross-references (stable across files)
- Keep v5's section numbers exactly: §1 … §23, and subsections `§8.2`, rules inside a section
  numbered `§8.2.3` (section 8.2, rule 3). Headings: `## 8. Modules`, `### 8.2 The body`.
- Cross-reference as `[§15.2](part_v_execution.md#152-…)` — or simply `§15.2` in text when a
  link anchor is uncertain; the reviewer will normalize. Never renumber another part's sections.
- Definitions: **bold** the defined term at its single defining occurrence, e.g.
  "A **drive** is …". Every term used normatively must have exactly one defining occurrence in
  the whole spec; other parts reference it. Glossary terms owned by a part are listed at the top
  of that part under "Terms defined here".

## Normative language
- RFC 2119 words in bold caps: **MUST**, **MUST NOT**, **SHALL**, **SHOULD**, **MAY**. An
  implementation that violates a MUST is non-conforming.
- Each section, where applicable, in this order (omit empty ones, don't pad):
  1. Lead sentence: what the thing is (the rule in one sentence, as v5 has it).
  2. **Syntax** — EBNF excerpt (must match appendix_b_grammar.md verbatim).
  3. **Static semantics** — what elaboration/type-checking checks; numbered rules.
  4. **Dynamic semantics** — behavior per analysis (Dc/Ac/Noise/Tran/…) and per instant; numbered.
  5. **Errors** — table: code · condition · phase (parse/elaboration/run).
  6. **Examples** — short, compiling-in-principle code from v5 (adapt, don't invent new syntax).
- Rust inheritance: say once (index.md) that everything not stated is Rust 2024 value semantics;
  only state deltas.
- Tone: technical, terse, no marketing, no history ("v4 did X"), no mention of the duel/critic.
  English. Sentences < 30 words. Tables for multi-attribute facts.

## Completeness bar per definition
For every construct, the reader must be able to answer: where is it allowed (B.2), what type
does it have, when is it evaluated, what does it do in each analysis (§16.2 table), what errors
it can raise, how it interacts with unknown (known flag), with events/instants, and with
small-signal. If v5 answers it elsewhere, cross-reference; if not, resolve per above.

## Process
- Read STYLE.md, then spec_v5.md fully (you need global context for consistency), then
  critique_v4.md "Residual open items" and changelog_v5.md "Left open".
- Write only your assigned files in /home/keslarek/Git/piperine-msl-spec/docs/spec/.
- Do not touch git, code, or other files. Final message ≤ 8 lines: files, sizes, number of
  `Resolution` notes, any cross-part dependency you assumed (term X defined in part Y).
