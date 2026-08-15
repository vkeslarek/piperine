# LRM Audit — Chapters 10-12, Annexes A-H

Audited against `VAMS-LRM-2-4.md` (Accellera Verilog-AMS LRM 2.4). Report-only; no source
files modified. Branch `feature/device-verification`.

| LRM Clause | Requirement (paraphrase) | Category | Severity | Evidence (file:line) | Finding |
|---|---|---|---|---|---|
| 10.2 `` `default_discipline `` | Directive sets/clears the default discipline applied by discipline resolution (Annex F) to nets with no declared discipline | FORM + FUNCTIONALITY | Blocking | `crates/piperine-lang/src/preprocessor.rs:191-271` (no `"default_discipline"` match arm) → falls to `_ if active_now => handle_macro_invocation` (line 267-269) → `self.macros.get(name)` returns `None` → `Err(ParseErrorKind::UndefinedMacro)` (line 347-352). Negative search: `grep -rn default_discipline crates/piperine-lang/src` hits only the doc-comment listing it as future grammar-layer work (`preprocessor.rs:12`) | Any source using `` `default_discipline `` fails to parse at all (hard error), not "passed through" as the doc comment (lines 11-14) claims. Doc-comment claim is itself inaccurate. |
| 10.3 `` `default_transition `` | Sets default rise/fall time for `transition()` filter calls that omit them | FORM + FUNCTIONALITY | Blocking (parse) / Degraded (semantic weight) | Same code path as above — no `"default_transition"` arm in `handle_directive_or_macro` | Same UndefinedMacro parse failure; the directive's own semantic effect (default transition timing) is also nowhere modeled in codegen (`grep -rn default_transition crates/piperine-codegen/src` — no hits) |
| 10.4 `` `define ``/`` `undef `` | CPP-style object/function-like macros, rescan expansion | FORM + FUNCTIONALITY | Conformant | `preprocessor.rs:246-253,338-390` | Implemented faithfully: params, rescan self-reference guard, arity checking |
| 10.5 Predefined macros `__VAMS_ENABLE__`/`__VAMS_COMPACT_MODELING__` | These macros "shall always be defined" during Verilog-AMS parsing | FUNCTIONALITY | Blocking (silent) | `grep -rn "__VAMS_ENABLE__\|__VAMS_COMPACT_MODELING__" crates/piperine-lang/src` → no hits anywhere in the crate | Neither macro is ever pre-registered in `Preprocessor::new()`/`macros: HashMap::new()` (`preprocessor.rs:76-78`). A source using the LRM's own portability idiom (`` `ifdef __VAMS_ENABLE__ ``) silently takes the `else` branch always — no parse error, just silently wrong behavior. Worse than the fail-loud directives above because nothing signals the divergence. |
| 10.6 `` `begin_keywords ``/`` `end_keywords `` (`"VAMS-2.3"` version specifier) | Scopes the reserved-keyword set; the LRM's own example requires an identifier like `sin` to be usable as a port name under a non-VAMS keyword scope | FORM | Blocking (parse) | No match arm in `handle_directive_or_macro`; `grep -rn begin_keywords crates/piperine-lang/src` → only the same doc-comment listing | Falls to macro-invocation path → `UndefinedMacro` parse failure |
| `` `celldefine ``/`` `endcelldefine `` (1364-2005 inherited, ch.10 Table) | Cell-boundary marking directive | FORM | Blocking (parse) | No handling anywhere in `preprocessor.rs`, `grammar/`, `lexer.rs`, `keywords.rs` | `UndefinedMacro` parse failure on any file using them |
| `` `nounconnected_drive ``/`` `unconnected_drive `` | Unconnected-port drive-strength directive | FORM | Blocking (parse) | Same negative search as above | `UndefinedMacro` parse failure |
| `` `line `` | Source-line renumbering directive | FORM | Blocking (parse) | Same negative search | `UndefinedMacro` parse failure |
| `` `pragma `` | Tool-pragma directive (arguments ignorable by non-supporting tools per 1364-2005, but the directive syntax itself must still parse) | FORM | Blocking (parse) | Same negative search | `UndefinedMacro` parse failure |
| `` `default_nettype `` | Sets default net type for undeclared identifiers used as nets | FORM | Blocking (parse) | Same negative search | `UndefinedMacro` parse failure |
| `` `timescale ``/`` `resetall `` | 1364-2005-inherited directives, textually inert per this crate's own scope note | FORM | Blocking | `preprocessor.rs:258-266` passes the raw directive text through (`out.push_str(&source[directive_start..name_end])`) into the token stream; lexer emits `Tok::Directive` (`lexer.rs:67,162`); `grep -rn "Tok::Directive" crates/piperine-lang/src/grammar/*.rs` → the only match is `cursor.rs:173`'s error-formatting `Display` arm, no grammar production ever consumes the token | The doc comment's claim ("pass through as directive text for the grammar layer... to handle", `preprocessor.rs:259-262`) is false: nothing downstream handles `Tok::Directive`. Any file containing a bare `` `timescale 1ns/1ps `` — one of the single most common real-world Verilog-A directives — hits an unexpected-token parse error wherever the directive falls |
| 11.2.1 `vpi_register_cb`/`vpi_remove_cb`/`vpi_get_cb_info` (Table 11-1) | Simulation-event/time/action callback registration | FUNCTIONALITY | Degraded (documented) | `crates/piperine-plugin/src/vpi.rs:79-90` | `Partial`: registration modeled via `HookPhase`, phase-grained not event-grained; `vpi_remove_cb` explicitly `NotSupported` (hooks live for the run) — self-disclosed, not silent |
| 11.2.1 `vpi_register_systf`/`vpi_register_analog_systf` (Tables 11-2/11-3) | User-defined system task/function + analog-systf registration | FUNCTIONALITY | Conformant | `crates/piperine-plugin/src/host.rs:431-620`, `crates/piperine-lang/src/eval/tasks.rs:349-430` (`VpiTaskKind`, `VpiTaskRegistry`) | Native routing implemented and tested (`vpi_routines.rs::systf_and_analog_systf_route_through_t53s_native_abi`); collision-with-builtin and duplicate-registration both fail loud |
| 11.4 Table 11-7, `vpi_get_delays`/`vpi_put_delays` | Delay/timing-limit read/write | FUNCTIONALITY | Degraded (documented) | `crates/piperine-plugin/src/vpi.rs:138-146` | `NotSupported`, explicitly reasoned: no delay/timing-check model exists in the digital kernel — real, named gap, not silent |
| 11.4 Table 11-8, `vpi_put_value` | Live value injection back into a running solve | FUNCTIONALITY | Degraded (documented) | `crates/piperine-plugin/src/vpi.rs:148-156` | `Partial` — read half (`vpi_get_value`/`vpi_get_analog_value`) native via `SolveResultView`; write half `NotSupported`, self-disclosed |
| 11.4 Table 11-10, `vpi_mcd_*` family | Multi-channel-descriptor file I/O | FUNCTIONALITY | Cosmetic (documented) | `crates/piperine-plugin/src/vpi.rs:167-184` | `Partial` — `vpi_printf`/`vpi_chk_error`/`vpi_free_object`/`vpi_compare_objects` all have named native equivalents; `vpi_mcd_*` itself `NotSupported`, self-disclosed as a minor gap |
| 12 (per-category rollup, Tables 11-1..11-10) | Every VPI routine category defined | FUNCTIONALITY | Conformant (self-audited) | `crates/piperine-plugin/src/vpi.rs:77-186`, `crates/piperine-plugin/tests/vpi_routines.rs` | All 10 categories carry an explicit typed support level (`Native`/`Partial`/`NotSupported`) with a named mechanism/gap — no silent absence (C6 honored). Tests pass (`cargo test -p piperine-plugin --test vpi_routines`, not independently re-run in this audit but source inspection confirms the assertions match the table) |
| Annex C.4, `wreal` forbidden in Verilog-A | A Verilog-A-only tool must reject `wreal` | FORM | Cosmetic (no divergence for a full-VAMS product) | `grep -n wreal crates/piperine-lang/src/elab/typecheck.rs` → no hits | Piperine has no distinct "Verilog-A subset" compilation mode (no flag anywhere gates a narrower profile) — it always compiles full Verilog-AMS, so this and the other Annex-C-only restrictions (C.4 `discrete` domain binding forbidden, C.4 `` `default_discipline `` forbidden, C.5 `===`/`!==` forbidden, C.16 keyword subtraction) are moot rather than violated. Noted as a scope observation, not a bug, since nothing in the codebase claims Verilog-A-only conformance as a separate product mode |
| Annex C.7, "No digital behavior or events... `casex`/`casez` not supported" | Digital-only constructs forbidden inside analog behavior | FUNCTIONALITY | Conformant (partial, self-disclosed ongoing) | `crates/piperine-lang/src/elab/typecheck.rs:50-91` | `<=` (non-blocking assign), `casex`/`casez`, `posedge`/`negedge` inside `@(...)` all correctly rejected with `AnnexCSubsetViolation` (E4002) inside `analog`/`analog initial` bodies. Doc comment (lines 9-20) honestly states this is an "ongoing gate," not a closed one |
| Annex D.1 `disciplines.vams` | 11 disciplines, 15 natures | FUNCTIONALITY | Conformant | `crates/piperine-lang/headers/disciplines.vams` (full file) vs. `VAMS-LRM-2-4.md:18251-18624` | Byte-for-byte content match (values, `abstol` defaults, `` `ifdef ``-override macros) verified for all 11 disciplines/15 natures |
| Annex D.2 `constants.vams` | Math/physical constants incl. the `PHYSICAL_CONSTANTS_*` selector chain | FUNCTIONALITY | Conformant | `crates/piperine-lang/headers/constants.vams` vs. `VAMS-LRM-2-4.md:18564-18654` | Full match, including the NIST1998-default fallback chain |
| Annex D.3 `driver_access.vams` | `DRIVER_UNKNOWN`/`DRIVER_DELAYED`/... bit-flag constants for VPI driver introspection | FUNCTIONALITY | Degraded | `find crates/piperine-lang/headers -iname "*driver*"` → no file; `grep -rn "DRIVER_UNKNOWN\|driver_access" crates/piperine-lang/` → no hits anywhere | Entirely un-ported, and — unlike Annex E's gaps — not documented anywhere as a deliberate omission. Practical impact is low (these constants exist to interpret `vpi_get_delays`'s driver-info struct, and delay processing is already `NotSupported` per the ch.11 table above), but it is a silent gap, not a named one (violates the project's own C6 fail-loud-never-silent norm at the documentation level) |
| Annex E, Table E.1/E.2 | SPICE primitive/parameter/port preferred names; limiting functions | FUNCTIONALITY | Conformant (self-audited, minor documented gaps) | `crates/piperine-lang/src/annex_e.rs:70-261`, `tests/vams_annex_e.rs` | 16/19 Table E.1 rows `AliasedName`/`ExactMatch`, 3 (`ipwl`, `vpwl`, `mesfet`) honestly `NotSupported`; Table E.2's `vdslim`↔`limvds` naming mismatch is a documented `SPEC_DEVIATION` (fixing it means touching `emit/analog_expr.rs`, one of `CLAUDE.md`'s "files not to edit casually") |
| Annex E.2.1 / E.3.2 | Case-insensitive SPICE-netlist name matching; `port_discipline` attribute for analog-primitive discipline resolution | FUNCTIONALITY | Degraded (documented) | `crates/piperine-lang/src/annex_e.rs:16-19`; `grep -rn port_discipline crates/piperine-lang/` → only the annex_e.rs doc comment | Both presuppose a raw-SPICE-netlist reader Piperine never built (confirmed: no `.subckt`/`.model` parser exists in the tree) — self-disclosed as out of scope rather than silently missing |
| Annex F / LRM §7.4.4.1, "Basic discipline resolution algorithm" (the default) | At each hierarchy level where continuous and discrete disciplines meet on an undeclared net, the net resolves to **continuous** (the LRM's own defining behavior of this algorithm, illustrated in Figure 7-3/`NetC`/`NetD`) | FUNCTIONALITY | Blocking (undisclosed) | `crates/piperine-lang/src/elab/lower/disciplines.rs:99-150`, specifically the conflict branch at lines 118-129: `Some(d) if d != callee_disc => return Err(DisciplineResolutionFailed{ reason: "incompatible disciplines...", ...})` | The implementation never classifies disciplines by domain (continuous vs. discrete) — any two *different* discipline names meeting at a connection are treated as a hard elaboration error, not resolved via continuous-wins-over-discrete precedence. This is the algorithm's own headline rule (§7.4.4.1: "At each level of the hierarchy where continuous and discrete meet for an undeclared net, that net segment is declared continuous") and it is simply absent — not narrowed, not mentioned in the module's own doc comment (lines 1-29), which candidly discloses the `connect`/`resolveto` gap (line 28-29) but not this one |
| Annex F / LRM §7.4.4.1 step 4.b, "apply `` `default_discipline `` directives" | When no neighbouring discipline is found, fall back to the in-force `` `default_discipline `` directive before declaring the net's discipline unknown | FUNCTIONALITY | Blocking | `disciplines.rs` has no reference to any default-discipline state; ties directly to the ch.10 finding above (the directive itself is unimplemented, so there is nothing to apply here even in principle) | Compounding gap: even if `` `default_discipline `` parsed, `ResolveDisciplines` has no mechanism to consult it |
| Annex F §7.4.1, `connect ... resolveto` compatible-discipline resolution | Named connect-statement resolution of multi-discipline nets | FUNCTIONALITY | Degraded (documented) | `disciplines.rs:17-29` doc comment: "`connect`/`connectrules`-driven resolution... is future work (T52, `InsertConnectModules`)" | Self-disclosed, tracked (T52), not a silent gap — lower severity than the two findings above precisely because it is disclosed |
| Annex A (spot-check only, not a full re-audit) | Grammar conformance gate accuracy | FORM | Conformant (spot-check passed) | `crates/piperine-lang/tests/annex_a_grammar.rs` (`cargo test -p piperine-lang --test annex_a_grammar` → 3/3 pass) | 15 `Implemented` sections spot-checked (A.1.6, A.1.9, A.2.1.1-3, A.2.3, A.2.6-8, A.4.1-2, A.5.1-3, A.6.1/3/4/5/6/7/8/10, A.8.1/3, A.9.1/3) all resolve to real, non-stub grammar methods citing the section in their doc comments (`grammar/item.rs:30,51,549,909,955`, `grammar/cursor.rs:105`, `grammar/stmt.rs:287`); ~10 `OutOfScope` entries spot-checked (A.1.5, A.1.8, A.2.2.1-3, A.3.1-4, A.5.4, A.7.2-5.3) all cite a real, still-open task (T48/T49/T52) rather than a stale/closed one per `tasks.md` |
| Annex B (spot-check only) | Keyword list re-derivation gate | FORM | Conformant (spot-check passed) | `cargo test -p piperine-lang --test annex_b_keywords` → 5/5 pass | No mismatch found |
| Annex G (spot-check only, informative) | Change-history table ported without garbling | — | Conformant (spot-check passed) | `docs/reference/annex_g_change_history.md` vs. `VAMS-LRM-2-4.md:19192-19230` | Table G.1 content matches (spot-checked the `$realtime`/`initial_step`/`final_step`/`ground` rows) |
| Annex H (spot-check only, informative) | Glossary ported without garbling | — | Conformant (spot-check passed) | `docs/reference/annex_h_glossary.md` vs. `VAMS-LRM-2-4.md:19664-19700` | Entries (AMS, behavioral description/model, block, branch, compact model) match verbatim in substance |

## Chapter 10 — Compiler directives

The scope Piperine's own `preprocessor.rs` doc comment claims (lines 8-14) is narrower than
LRM ch.10's Table 10.1 directive list, and — critically — the doc comment's own framing of
that narrowing is inaccurate. It states that out-of-scope directives (`` `default_discipline ``,
`` `celldefine ``, `` `pragma ``, and by extension every other Table 10.1 entry not on the
implemented list) "pass through untouched for the grammar layer (T9+) to handle." Tracing the
actual control flow in `handle_directive_or_macro` (lines 191-271) shows this is false for
every one of them except `` `timescale ``/`` `resetall ``, which get a dedicated (but
dead-ended) pass-through arm; everything else — `` `default_discipline ``, `` `default_transition ``,
`` `celldefine ``/`` `endcelldefine ``, `` `begin_keywords ``/`` `end_keywords ``, `` `line ``,
`` `pragma ``, `` `nounconnected_drive ``/`` `unconnected_drive ``, `` `default_nettype `` —
falls into the generic macro-invocation branch and raises `ParseErrorKind::UndefinedMacro`,
a hard parse failure. And the two that *do* pass through (`` `timescale ``/`` `resetall ``)
land on a lexer token (`Tok::Directive`) that no grammar production ever consumes, so they
fail to parse too, just one stage later. Practically: **any real-world `.va`/`.vams` file using
any Table 10.1 directive outside the `define`/`undef`/`ifdef`-family/`include` set fails to
parse**, including `` `timescale ``, one of the single most common directives in the wild.

Separately, §10.5's two predefined macros (`__VAMS_ENABLE__`, `__VAMS_COMPACT_MODELING__`),
which the LRM mandates be "always... defined during parsing," are not pre-registered anywhere
— a silent-wrong-behavior gap (the LRM's own portability-idiom example takes the wrong branch)
rather than a loud one, which is a sharper divergence from the project's stated fail-loud
philosophy than the parse failures above.

Conformant subset: `define`/`undef` (10.4) and the `ifdef`/`ifndef`/`else`/`elsif`/`endif`/
`include` family — 8 of 21 Table 10.1 directives — plus §10.6's `` `begin_keywords `` version
specifier is unimplemented, meaning even a project that never uses `sin` as a port name is
unaffected but one that does (the LRM's own §10.6 example) cannot express the workaround.

## Chapter 11 — Using VPI routines

Audited against the repo's own explicit scope narrowing (spec.md's Out of Scope table):
literal `.so` VPI hosting is out of scope by design; the language-level surface (parsing +
native-ABI routing) is in scope. Under that framing, ch.11's callback/registration surface is
reasonably well covered: `vpi_register_systf`/`vpi_register_analog_systf` (Tables 11-2/11-3,
the two categories §11.2.1 calls out for "a primary use... for the registration of
user-defined system tasks and functions") are fully native and tested end to end
(`PluginHost::register_system_task`/`resolve_system_task`/`call_system_task`,
`VpiTaskRegistry`). The remaining categories (simulation callbacks, delay processing, value
processing, time processing, misc utilities) are each explicitly typed `Partial` or
`NotSupported` with a named reason in `vpi.rs`'s data table — never silently absent, matching
the project's own C6 rule. No unaudited category was found.

## Chapter 12 — VPI routine definitions

Ch.12 has no taxonomy of its own beyond ch.11 §11.4's ten categories (it just lists routines
alphabetically); `vpi.rs`'s audit table is organized on exactly that basis and every category
carries a support level. This is a self-consistent, honestly-labeled implementation of the
project's own redefined scope — no divergence found beyond what's already itemized under
ch.11 above (the two chapters share one audit surface in this codebase).

## Annex C — Analog language subset

`typecheck.rs`'s Annex C enforcement covers exactly the three constructs its own doc comment
claims (non-blocking assignment, `casex`/`casez`, `posedge`/`negedge` inside analog event
control) and is explicitly framed as an "ongoing" gate, not a finished one. The rest of Annex
C (C.4's `wreal`/`discrete`-domain/`` `default_discipline ``-forbidden-in-Verilog-A rules,
C.5's `===`/`!==` restriction, C.16's keyword-subtraction list) has no enforcement anywhere,
but this is not a bug in the ordinary sense: Piperine has no separate "Verilog-A-only" product
mode to enforce these restrictions *for* — it always compiles the full Verilog-AMS grammar.
Annex C's restrictions exist to define a narrower, analog-only-tool compliance profile; a tool
that implements the superset (Verilog-AMS) trivially satisfies "does not need Annex C
restrictions" by construction, unless it also wants to *offer* a restricted mode as a product
feature, which Piperine does not claim to.

## Annex D — Standard definitions

`disciplines.vams` and `constants.vams` are faithful, essentially verbatim ports (spot-checked
in full against the LRM's own listing — every discipline, nature, `abstol` default, and the
constants' `` `ifdef ``-chain selector logic all match). `driver_access.vams` (D.3) was not
ported at all, and unlike every other gap found in this audit, this one carries no
acknowledgment anywhere in the codebase — no doc comment, no `SPEC_DEVIATION`, no ROADMAP
entry found by grep. Low practical severity (its only consumer, VPI driver introspection, is
itself a documented `NotSupported` gap in ch.11's table) but a process gap: it is the one
finding in this audit that is silent rather than named.

## Annex E — SPICE compatibility

The most thoroughly self-audited annex in the codebase. `annex_e.rs` walks every Table E.1 row
and Table E.2 row, typing each `ExactMatch`/`AliasedName`/`NotSupported` with a concrete note,
and the three real gaps (`ipwl`, `vpwl`, `mesfet` — no PWL waveform, no MESFET model) plus the
one naming mismatch (`vdslim` vs. `limvds`) are all named, tested, and cross-referenced to
exactly which file would need to change to fix them and why that file is out of this task's
remit. E.2.1/E.3.2 (case-insensitive netlist matching, `port_discipline` attribute) are
correctly identified as presupposing a raw-SPICE-netlist reader that doesn't exist and was
never in scope. No new divergence found beyond what T55 already disclosed.

## Annex F — Discipline resolution methods

**Method actually implemented: Basic discipline resolution, LRM §7.4.4.1** (the doc comment's
citation is textually accurate — §7.4.4.1 is a real, distinctly-named section, not a
paraphrase). However, the implementation is **not faithful to that method** in one significant,
undisclosed way: §7.4.4.1's entire point (illustrated by its own Figure 7-3 worked example) is
that when continuous and discrete disciplines meet at an undeclared net, the net resolves to
continuous rather than erroring. `ResolveDisciplines` (`disciplines.rs`) has no concept of
discipline domain (continuous vs. discrete) at all — it does pure name-identity propagation
and treats any two non-identical disciplines meeting at a connection as a hard
`DisciplineResolutionFailed` error. This means Basic resolution's headline behavior is simply
absent, not narrowed-with-disclosure like the (separately, correctly disclosed) `` `default_discipline ``-application step and `connect ... resolveto` step. A real mixed-signal
design relying on continuous-wins-over-discrete resolution — the LRM's own primary worked
example for this algorithm — fails to elaborate in Piperine today.

## Conformant-subsection tally

| Area | Conformant | Partial (disclosed) | Divergent/Blocking | Total sections audited |
|---|---|---|---|---|
| Ch.10 directives (Table 10.1, 21 entries + §10.5) | 8 | 0 | 13 | 22 |
| Ch.11 VPI categories (Tables 11-1..11-10) | 2 fully Native | 6 Partial/self-disclosed | 1 (Delay) NotSupported, disclosed | 10 |
| Ch.12 (shares ch.11's table) | — | — | — | (see ch.11) |
| Annex C (analog subset) | 1 clause (C.7) | 0 | 0 (rest moot — no Verilog-A mode) | 1 real target |
| Annex D (standard definitions) | 2 (D.1, D.2) | 0 | 1 (D.3, undisclosed) | 3 |
| Annex E (SPICE compatibility) | 16/19 primitives, 2/3 limit fns exact | 3 primitives + 1 fn disclosed gap | 0 undisclosed | 22 mapped items |
| Annex F (discipline resolution) | 0 | 1 (connect/resolveto, disclosed) | 2 (continuous-precedence rule, `` `default_discipline `` application — both undisclosed) | 3 sub-rules checked |

## Annex A/B/G/H spot-check note

All four passed. Annex A: the mechanical gate (`annex_a_grammar.rs`) runs clean (3/3 tests),
and its own third test cross-checks `SECTIONS` against the in-repo LRM text directly, so it
cannot silently drift; 15 `Implemented` and ~10 `OutOfScope` entries were manually spot-checked
and all cited grammar methods/reasons are real and current (no stale `OutOfScope` reasons
pointing at closed tasks). Annex B: `annex_b_keywords.rs` runs clean (5/5 tests). Annex G/H:
one-sentence content spot-checks against `docs/reference/annex_g_change_history.md` and
`annex_h_glossary.md` found both faithfully ported, not garbled. No mismatch found in any of
the four.

## Closure status (updated 2026-08-15, `lrm-conformance-closure` feature)

Every non-Conformant, non-OutOfScope row above is now closed. Closing commit
per finding (see `docs/audit/followup-2026.md` for the re-verification
evidence):

| Finding (LRM clause) | Closing commit(s) |
|---|---|
| §10.2 `` `default_discipline `` | `6ff31cd2` |
| §10.3 `` `default_transition `` (parse) | `6ff31cd2` |
| §10.3 `` `default_transition `` (semantic weight) | `53ec1ff8` |
| §10.5 predefined macros | `4505d34b` |
| §10.6 `` `begin_keywords``/`` `end_keywords`` | `28fcbec8` |
| `` `celldefine``/`` `endcelldefine``/`` `line``/`` `pragma``/`` `default_nettype``/`` `unconnected_drive`` | `d81cf3d8` |
| `` `timescale``/`` `resetall`` | `318b9bef` |
| §11.2.1 Table 11-1, `vpi_register_cb`/`vpi_remove_cb` | `d82ff417` |
| §11.4 Table 11-7, `vpi_get_delays`/`vpi_put_delays` | `bf1b5026`, `9fb004b8` |
| §11.4 Table 11-8, `vpi_put_value` | `71a83e05` |
| §11.4 Table 11-10, `vpi_mcd_*` family | `b14a4814` |
| Annex D.3 `driver_access.vams` | `0daa46ed` |
| Annex E, `vdslim`/`limvds` naming + `ipwl`/`vpwl` | `26224919` |
| Annex E, `mesfet` | Deliberately not closed — see `annex_e.rs`'s own note; no faithful ngspice source exists |
| Annex F, continuous-over-discrete precedence + `` `default_discipline `` fallback | `2f287f9f` |
| Annex F, `connect ... resolveto` | `3a039597` |
