# Follow-up LRM Conformance Audit — after `lrm-conformance-closure`

**Date**: 2026-08-15
**Scope**: re-checks `docs/audit/README.md`'s original findings against the
current source, focused on the findings this feature (`.specs/features/
lrm-conformance-closure/`) actually closed. Same method as the original four
reports: cite an exact LRM clause plus current `file:line` evidence (or a
negative grep proving the old gap is gone), never carry forward a claim
without re-checking it against the tree as it stands today. Report-only.

**Coverage note**: this feature's 51 requirement IDs span P1-P5 (`tasks.md`
T1-T67). P1/P2 (ch.1-4, ch.5-6 — `LRMC-C1-*`/`LRMC-C5-*`) and most of P3
(`LRMC-C7-01..04, 06..13`) were closed earlier in this same working session,
before this transcript's own compaction boundary; those closures are not
re-derived clause-by-clause in this follow-up (each still carries a real,
named, passing test per `tests/lrm_conformance_matrix.rs`'s own mechanical
`every_closed_entry_names_a_real_test` gate, and the full workspace suite
ran clean target-for-target against the T1 baseline after every task in this
document's own scope). This follow-up re-verifies, clause by clause, the
findings closed in the *compacted* portion of the session: `LRMC-C7-05`,
`LRMC-C7-14`, and the entire ch.10-12/Annex-D/E block (`LRMC-C10-01..14,
16`) plus Annex E's in-scope gaps (spec P5 AC5) — i.e. everything
`docs/audit/ch7-9.md` rows 14/23 and `docs/audit/ch10-12-annexes.md`
describe as Blocking/Degraded that this document's own author (this
session) personally implemented and tested.

## ch7-9.md findings re-checked

| Original finding | Was | Now | Evidence |
|---|---|---|---|
| §7.6/§9.22-9.23, `$driver_*` family + `driver_update` | Blocking — zero grammar/table registration, `connectmodule` driver introspection unwritable | Closed (LRMC-C7-05, T35) | `crates/piperine-lang/src/eval/tasks.rs`'s `SystemTasks::TABLE` carries all 8 names; `crates/piperine-lang/tests/vams_system_tasks.rs::driver_access_calls_parse_and_elaborate_inside_a_connectmodule_body` compiles a real `connectmodule` body calling all 7 functions + `driver_update`. **Disclosed, not silently narrowed**: name-level resolution + parse/elaborate acceptance only — `piperine_solver::digital::DriverTable` (T34) exists but nothing populates it yet (§7.9's actual driver-receiver segregation routing is separately scoped future work, per `tasks.rs`'s own SPEC_DEVIATION doc comment on this block) — a real runtime-value read from inside a compiled connectmodule is not yet reachable. Re-verified today: `grep -n "DriverTable::set_driver" crates/ -r` still finds no caller, confirming the disclosure is still accurate, not stale. |
| §8.4.6, `absdelta`-interpolated A2D events | Degraded — zero occurrences of `absdelta` anywhere in `piperine-solver`/`piperine-codegen` | Closed (LRMC-C7-14, T41, marked partial at the time) | `crates/piperine-solver/src/digital/interface.rs::interpolate_a2d_crossing` — unit-tested (`a2d_crossing_interpolates_within_the_published_error_bound`, `non_crossing_samples_yield_no_interpolation`) against a published error bound. Re-verified today: the function exists, is exported (`piperine_solver::abi::interpolate_a2d_crossing`), and both its unit tests still pass (`cargo test -p piperine-solver --lib a2d_crossing` → 2/2). |

## ch10-12-annexes.md findings re-checked

Every row in the original report's Chapter 10 section is closed. Re-derived
directly against the current `preprocessor.rs`/`directives.rs` (not copied
from commit messages):

| Original finding | Was | Now | Evidence |
|---|---|---|---|
| §10.2 `` `default_discipline `` | Blocking (`UndefinedMacro`) | Already closed pre-compaction (LRMC-C10-01) | Unaffected by this session's own work; re-confirmed still green: `cargo test -p piperine-lang --test vams_directives default_discipline` → all pass |
| §10.3 `` `default_transition `` | Blocking (parse) + the directive's semantic weight nowhere modeled | Closed (LRMC-C10-02 — parse half pre-compaction, **semantic half T56 this session**) | `crates/piperine-codegen/src/resolve/pom/analog_ops.rs::Transition::lower` now reads `ctx.default_transition` (threaded from `Design::directives()`) as the `rise`/`fall` fallback instead of a bare `0.0`. `crates/piperine-codegen/tests/default_transition_semantics.rs` (3/3) proves the directive value is used when arguments are omitted, an explicit argument still wins, and the no-directive case is unchanged. Re-verified today: `grep -n "ctx.default_transition" crates/piperine-codegen/src/resolve/pom/analog_ops.rs` confirms the read site still exists at the stated location. |
| §10.5 predefined macros | Blocking (silent) — `__VAMS_ENABLE__`/`__VAMS_COMPACT_MODELING__` never registered, portability idiom silently took the wrong branch | Closed (LRMC-C10-03, T54) | `crates/piperine-lang/src/preprocessor.rs::predefined_macro_table` pre-registers both in every `Preprocessor::new`/`with_include_resolver`; `` `undef `` is a guarded no-op against them. `vams_directives.rs::vams_enable_ifdef_takes_the_ams_branch`/`vams_compact_modeling_ifdef_takes_the_compact_branch`/`undef_has_no_effect_on_predefined_macros` (3/3) prove the LRM's own §10.5 idiom now takes the `` `ifdef`` branch, via a from-range violation only that branch's clause could trigger — not merely that the macro resolves. |
| §10.6 `` `begin_keywords``/`` `end_keywords`` | Blocking (parse) | Closed (LRMC-C10-04, T55) | `preprocessor.rs` parses and records into `DirectiveState.keyword_scopes` (a balanced stack); unbalanced directives fail loud with new typed errors. **Scope note carried forward accurately**: `Keywords::expect_identifier` (the reserved-word check the LRM example is nominally about) has zero call sites in `grammar/*.rs` — verified again today via `grep -rln "Keywords::expect_identifier" crates/piperine-lang/src/grammar/` (no hits) — so the LRM's own `sin`-as-port-name example already parsed *unconditionally* before this task; T55 closes the directive-parsing gap specifically, not a keyword-restriction regression that never existed. |
| `` `celldefine``/`` `endcelldefine``, `` `line``, `` `pragma``, `` `default_nettype``, `` `nounconnected_drive``/`` `unconnected_drive`` | Blocking (parse), five separate `UndefinedMacro` failures | Closed (LRMC-C10-05..09, T52) | All five parse and record state (`open_celldefine`/`celldefine_spans`, `line_map` + `DirectiveState::line_for_offset`, pragma consumed to end-of-line, `default_nettype none` now a typed `E4007` on an implicit net via `elab_module`'s threaded `default_nettype` parameter, `unconnected_drive`/`nounconnected_drive`). 21 tests in `vams_directives.rs`, all still passing. |
| `` `timescale``/`` `resetall`` | Blocking — doc comment claimed pass-through, actually dead-ended on an unconsumed `Tok::Directive` | Closed (LRMC-C10-10, T53) | Both are now fully parsed/consumed (no residual token); `` `timescale`` records `DirectiveState.timescale`, feeding a new `TimeFormat::from_timescale`; `` `resetall`` clears `default_discipline`/`default_nettype`/`unconnected_drive`/open `celldefine` per §10.2's own "in addition to resetall" clause. Re-verified today: `grep -n "Tok::Directive" crates/piperine-lang/src/grammar/*.rs` still finds only `cursor.rs`'s `Display` formatting arm — but this is no longer reachable for `timescale`/`resetall` specifically, since the preprocessor no longer emits that token for them at all (confirmed by re-reading `handle_directive_or_macro`'s current `"timescale"`/`"resetall"` arms — neither pushes to `out` anymore). |
| §11.2.1 Table 11-1, `vpi_register_cb`/`vpi_remove_cb` | Degraded (documented) — phase-grained only, no deregistration | Closed (LRMC-C10-11, T59) | `crates/piperine-plugin/src/registry.rs::{SimEvent, CallbackId, CallbackRegistry}` + `PluginHost::register_cb`/`remove_cb`. `tests/callback_registry.rs` (4/4) proves event-kind discrimination and real deregistration (no further fire). **Scope note, verified again today**: per-net/per-region event granularity is still not reachable (`piperine-solver` has no dependency on `piperine-plugin` — `grep -n piperine-plugin crates/piperine-solver/Cargo.toml` → no hits, confirming the architectural boundary the T59 commit cited is still real, not just asserted). |
| §11.4 Table 11-7, `vpi_get_delays`/`vpi_put_delays` | Degraded (documented) — `NotSupported`, no delay/timing-check model existed | Closed (LRMC-C10-12, T60/T61) | `crates/piperine-solver/src/digital/delays.rs::{PathDelay, DelayTable, TimingCheckKind, TimingCheckLimit, TimingCheckTable}` (the model) + `PluginHost::get_delays`/`put_delays` (the surface). `tests/delay_processing.rs` (2/2) proves a live-run event-timing change, not just a data round-trip. |
| §11.4 Table 11-8, `vpi_put_value` | Partial — write half `NotSupported` | Closed (LRMC-C10-13, T62) | `PluginHost::put_value` wraps the pre-existing `piperine::Session::set` (already MD-18-compliant). `tests/put_value.rs` (4/4): numeric solve change, zero `Session::rebuilds()` across the write (MD-18 proved, not assumed), unknown-param and unknown-device both fail loud. |
| §11.4 Table 11-10, `vpi_mcd_*` family | Partial/Cosmetic — `vpi_mcd_*` itself `NotSupported` | Closed (LRMC-C10-14, T63) | `crates/piperine-solver/src/core/output.rs::McdTable` (a bitmask over the same `FileTable`) + `PluginHost::mcd_open/close/printf/flush/name` + `vpi_printf` (routes through the identical `SimOutput` sink as `mcd_printf`'s stdout bit — proved by both landing in the same `BufferSink` stream in one test). `tests/mcd_family.rs` (5/5). |
| Annex D.3 `driver_access.vams` | Degraded — entirely un-ported, undisclosed (violated the project's own C6 norm at the doc level) | Closed (LRMC-C10-16, T57) | `crates/piperine-lang/headers/driver_access.vams` (new) — the 12 `DRIVER_*` bit-flag constants, ported by hand from the LRM's own Annex D.3 listing. `tests/vams_headers.rs::driver_access_vams_bit_values_match_the_lrm_exactly` re-derives all 12 values straight from `VAMS-LRM-2-4.md`'s own text at test time (independent of the header file), mirroring `annex_b_keywords.rs`'s own re-derivation methodology — not just a byte-copy comparison. |
| Annex E Table E.1/E.2 — `vdslim`/`limvds` naming, `ipwl`/`vpwl`, `mesfet` | Self-audited Conformant with 3 named `NotSupported` gaps + 1 naming `SPEC_DEVIATION` | 2 of 3 gaps closed, naming mismatch closed, `mesfet` deliberately stays open (T58) | `$limit()` now accepts `"vdslim"` (the LRM's own spelling) as well as `"limvds"` (`emit/analog_expr.rs`, `kernel/analog/limits.rs`); `headers/spice/sources.va`'s `isrc`/`vsrc` gained `wave = 6` (PWL, an 8-point bounded form — codegen has no array-parameter support, disclosed). `mesfet` **stays `NotSupported` on purpose**: `headers/spice/` is built by ngspice-source-faithful translation and ngspice itself ships no MESFET model to translate from — fabricating one from general theory would break the crate's own sourcing discipline, the same way inventing VAMS grammar from a third-party compiler would break C7's provenance rule for the parser. `crates/piperine-lang/src/annex_e.rs`'s own test (`exactly_one_table_e1_row_is_a_real_gap`) locks this at exactly one remaining gap. |
| ch.11/12 VPI category rollup | Conformant (self-audited) — every category typed, several `Partial`/`NotSupported` | All four P5-tracked categories (`spec.md` Success Criteria 4, `LRMC-C10-11..14`) now `Native`; `TimeProcessing` correctly still `Partial` (not one of the four tracked IDs) | `crates/piperine-plugin/tests/vpi_routines.rs::every_p5_covered_category_is_locked_at_native`, with `p5_lock_lets_a_real_regression_fail` proving the lock is a real trap (T64). |

## Zero repeat findings

No finding closed by this feature reappears under re-verification. The one
finding this follow-up deliberately does **not** close —
`mesfet` (Annex E Table E.1) — was already correctly typed `NotSupported`
in the original report and remains so, now with an explicit reason (no
faithful source exists) rather than the original's weaker "a real gap"
phrasing; this is a clarification, not a regression.

## What this follow-up does not cover

Per the coverage note above, this document does not re-derive the P1/P2
findings or the P3 findings closed before this session's compaction
boundary (`LRMC-C1-*`, `LRMC-C5-*`, `LRMC-C7-01..04,06..13`). Their
continued correctness rests on `tests/lrm_conformance_matrix.rs`'s
mechanical `every_closed_entry_names_a_real_test` gate (which resolves each
citation against the *current* tree, not a frozen snapshot) plus every full
`cargo build --workspace && cargo test --workspace` gate check run after
each of this session's own tasks passing with the pre-existing baseline
unchanged, target-by-target, per `.specs/features/lrm-conformance-closure/
baseline.md`. A reader wanting a from-scratch re-derivation of those
findings should run a dedicated follow-up pass scoped to P1-P3 specifically.
