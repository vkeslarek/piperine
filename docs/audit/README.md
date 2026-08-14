# LRM Conformance Audit — `VAMS-LRM-2-4.md` vs. `piperine-lang`/`piperine-codegen`

**Date**: 2026-08-14
**Scope**: every normative chapter (1–12) and annex (A–H) of `VAMS-LRM-2-4.md`,
audited chunk by chunk against the actual parser/elaborator/codegen source — not
against project docs, which this same audit found to be wrong in places (see
`ch1-4.md`/`ch5-6.md`'s `laplace_*` finding, which directly contradicts
`docs/spec/part_v_builtins.md` and `part_i_language.md`, both rewritten and
committed *earlier the same day* as this audit).

**Method**: four independent passes, each covering a contiguous chapter range,
run by fresh sub-agents with no shared context — every finding cites an exact
LRM clause plus `file:line` evidence in piperine's source (or the negative grep
proving absence). No finding is carried forward from a prior document without
independent re-verification. This is a **report-only** audit — nothing in
`crates/` was modified to produce it.

**Two divergence categories** (every finding is exactly one):
- **FORM** — a textual construct the LRM permits that piperine's parser
  (`crates/piperine-lang/src/{lexer.rs,grammar/,ast/,preprocessor.rs,keywords.rs}`)
  does not accept, accepts into the wrong shape, or over-accepts beyond the LRM.
- **FUNCTIONALITY** — the construct parses, but the elaborator or codegen doesn't
  implement its meaning, implements it incorrectly, or silently drops it.

**Three severities**: **Blocking** (a real `.vams`/`.va` file using the construct
cannot be authored/simulated at all), **Degraded** (parses/elaborates but wrong or
incomplete behavior), **Cosmetic** (works, LRM's exact spelling/message/edge case
differs).

## Reports

| File | Scope | Findings | Blocking |
|---|---|---|---|
| [`ch1-4.md`](ch1-4.md) | Ch.1 Introduction, Ch.2 Lexical, Ch.3 Data types, Ch.4 Expressions | 11 (FORM 6 / FUNCTIONALITY 5) | 7 |
| [`ch5-6.md`](ch5-6.md) | Ch.5 Analog behavior, Ch.6 Hierarchical structures | 8 (FUNCTIONALITY 6 / FORM 1 / mixed 1) | 7 |
| [`ch7-9.md`](ch7-9.md) | Ch.7 Mixed signal, Ch.8 Scheduling, Ch.9 System tasks | 12 negative + 3 positive-contrast rows | 8 |
| [`ch10-12-annexes.md`](ch10-12-annexes.md) | Ch.10 Directives, Ch.11-12 VPI, Annexes A-H | ~17 negative + 8 conformant call-outs | 11 |

**~48 total divergence findings**, of which **33 are Blocking** — a real
`.vams`/`.va` source using the affected construct cannot be authored or
simulated at all today, not merely "works differently." Exact totals aren't
summed to one number because the four workers used slightly different table
conventions (some include conformant rows for contrast, some don't) — read each
report's own tally section for its precise count; this index is the map, not a
replacement for the underlying evidence.

## Cross-report highlights (the findings that matter most)

**A real, previously-undocumented regression, found independently by two of the
four workers and refined by the second:** `laplace_nd`/`laplace_zd` — two of the
four canonical Laplace-filter operator spellings, including the exact one used in
the LRM's own canonical example — are never registered as callable operators
(`crates/piperine-codegen/src/resolve/pom/analog_ops.rs:384-403` registers a typo'd
variant array `["np","zp","pm","nm","npm"]` instead of the real `["zd","zp","nd","np"]`
set `zi_*` correctly uses two lines below it). This directly contradicts
`docs/spec/part_v_builtins.md` and `part_i_language.md`, both committed earlier
this session — those need a follow-up correction once this is fixed or
re-disclosed.

**The single largest gap by volume**: LRM chapter 9 (system tasks/functions) —
of ~23 substantive subsections, only ~6 have a runtime-reachable implementation;
~15 have no implementation at all by name, and 2 more (`$display`-family,
`$finish`/`$stop`) resolve as names but never reach a consumer that acts on them.
`$display` traced precisely: parses → elaborates → in digital bodies dies at
`emit/stmt.rs:105-109` (literal no-op); in analog bodies survives into a
`FlatDiagnostic` stored on `AnalogKernel` behind a `diagnostics()` getter with
**zero callers anywhere in the codebase** (`kernel/analog/kernel.rs:693`).

**Three "fully built, completely unwired" subsystems**, all previously
self-disclosed by their own task's Progress Log but now independently confirmed
with exact call-graph evidence: gate primitives (`kernel/digital/gates.rs`) and
UDPs (`kernel/digital/udp.rs`) have zero callers in the actual digital-body
compiler; the ch.8 four-region stratified scheduler (`kernel/digital/regions.rs`)
has exactly one reference outside its own file (a bare `pub use`), never called
by the production two-phase scheduler.

**`InsertConnectModules` (ch.7) is permanently dead code for real source**,
confirmed precisely: `ResolveDisciplines` (`elab/lower/disciplines.rs:118-129`)
unconditionally hard-fails any discipline mismatch before `InsertConnectModules`
(a later pass) ever runs, per `elab/lower/passes.rs`'s pass order — the
connect-module selection algorithm is correct (its own tests bypass
`ResolveDisciplines` by hand to prove it), but no real design can ever reach it.

**A silently-wrong Annex F implementation, undisclosed unlike its sibling gaps**:
`ResolveDisciplines`'s own doc comment correctly names "Basic discipline
resolution, §7.4.4.1" as the method implemented — but the actual code has no
concept of discipline *domain* (continuous vs. discrete) at all; it does pure
name-identity propagation and hard-errors on any two non-identical disciplines
meeting at a connection. Basic resolution's headline behavior — continuous wins
over discrete — is simply absent, not narrowed-with-disclosure like the
(correctly disclosed) `` `default_discipline ``/`` connect ... resolveto ``
gaps next to it in the same file.

**Chapter 10 (compiler directives) has a doc comment that actively misdescribes
its own scope**: `` `timescale ``/`` `resetall ``/`` `celldefine ``/`` `line ``/
`` `pragma ``/`` `default_nettype ``/`` `nounconnected_drive ``/`` `begin_keywords ``
either throw `UndefinedMacro` or produce a `Tok::Directive` token no grammar rule
consumes — the preprocessor's doc comment claims these "pass through", they
don't. `__VAMS_ENABLE__`/`__VAMS_COMPACT_MODELING__` (LRM §10.5, "shall always
be defined") are never registered — silently wrong, not even a loud error.

**Assignment-pattern syntax `'{...}`** (`crates/piperine-lang/src/lexer.rs`) is
unlexable — blocks every parameter-array initializer, string-parameter
valid-value list, and multidimensional array initializer the LRM's own §3.4.4/
§3.4.6/§3.4.8 examples use as the canonical form.

**Digital events inside analog behavior are actively rejected**, though LRM
§5.10.5 explicitly requires "analog behavior can be made sensitive to digital
events, including posedge events, negedge events" — Annex C enforcement
(`elab/typecheck.rs`) treats this as a violation instead.

## What's genuinely solid

Not everything is a gap. All four Annex A/B/G/H spot-checks passed clean (the
mechanical conformance gates for grammar-section coverage and keyword-set
completeness both run green and re-derive from the LRM text at test time, so
they can't silently drift). Annex E (SPICE compatibility) and the ch.11/12 VPI
routing are the best-audited areas in the whole set — both are honestly
self-documented with typed support levels and named gaps, consistent with the
project's fail-loud (C6) norm, rather than silently absent. Chapters 2-4's core
lexical/expression/parameter machinery is 35/43 directly-checked subsections
fully conformant. Analog contribution statements, procedural blocks, and
`initial_step`/`final_step`/`above`/`timer` events (ch.5) are solid.

## Next step

This is a report, not a fix plan — per the user's request, the goal here was
"tell me every point of divergence," not "fix every point of divergence." A
follow-up pass (new tasks, likely a new `.specs/features/` cycle given the
volume) is the natural next step if the goal is closing these gaps toward "tudo
precisa ser possível."
