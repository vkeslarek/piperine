# LRM Conformance Closure Specification

## Problem Statement

`docs/audit/` (2026-08-14, four independent chapter-range passes against
`VAMS-LRM-2-4.md`) found **55 real divergences** between what the LRM
specifies and what `piperine-lang`/`piperine-codegen`/`piperine-plugin`
actually do — 33 of them **Blocking** (a real `.vams`/`.va` source using the
affected construct cannot be authored or simulated at all today). Piperine's
own stated goal (`CLAUDE.md`) is that Verilog-AMS is its sole frontend and
"100% coverage" means every construct lowers to executable code — the audit
proved that claim is currently false across large surfaces: several compiler
directives don't parse, `$display` produces no output ever, `defparam`/
`paramset` are silently no-ops, gate primitives/UDPs/the ch.8 scheduler are
built but never called, and `InsertConnectModules` is permanently dead code.
User's own framing for this feature: **"TUDOOOOO precisa ser possível"** —
every LRM 2.4 construct the audit checked must become authorable and
correctly simulated, not just parseable-without-crashing.

## Goals

- [ ] Every one of the audit's 55 divergence findings (`docs/audit/{ch1-4,
      ch5-6,ch7-9,ch10-12-annexes}.md`) is closed: FORM findings parse per
      the LRM, FUNCTIONALITY findings elaborate/simulate per the LRM.
- [ ] VPI (ch.11-12) and Annex E items currently "Degraded (documented)" —
      typed `NotSupported` results the audit rated as the project's
      best-practiced fail-loud pattern — are upgraded to real native
      implementations, per the user's explicit widening of the audit's own
      bar (see Assumptions).
- [ ] No regression: every currently-passing test stays passing (verified
      the same way `verilog-ams-support` verified MD-47 — git-stash A/B,
      target-by-target, not aggregate counts).
- [ ] Every fix carries its own regression test derived from the LRM clause
      it closes, not from reading the fix's own implementation.

## Out of Scope

| Item | Reason |
|---|---|
| Annex G (change history), Annex H (glossary) | Informative, not normative (LRM's own front matter) — already ported to `docs/reference/`, audit spot-checks found them faithful. Nothing to close. |
| Annex A/B mechanical conformance gates themselves | Audit spot-checked both (`annex_a_grammar.rs`, `annex_b_keywords.rs`) and found them accurate and green — not a gap, no work item. Individual Annex-A `OutOfScope` grammar entries that back a real finding above ARE in scope via their own LRMC-ID. |
| Hosting third-party VPI `.so`/`.dll` C-ABI PLI applications | Pre-existing project decision (`verilog-ams-support` spec's own Out of Scope table, unchanged): VPI's *language surface* is in scope, routed through Piperine's native plugin ABI; literal C-ABI hosting is not, regardless of this feature's "100% native" bar for routine *categories*. |
| A distinct "Verilog-A subset" compilation mode (Annex C.4, `wreal` forbidden) | LRMC-C10-15 (Cosmetic, no real divergence) — Piperine always compiles the full Verilog-AMS grammar; adding a narrower opt-in profile is a new feature, not a conformance fix, and no `.va` file is blocked by its absence. |
| A raw SPICE-netlist (`.subckt`/`.model`) reader for Annex E.2.1/E.3.2 | Building a SPICE-netlist parser from scratch is a different, much larger feature than fixing LRM divergences in the VAMS frontend; Piperine's existing SPICE-facing path is header-based (`headers/spice/*.va`), not netlist-parsing, and that path is unaffected. Tracked as a documented, disclosed gap (LRMC-C10-17), not closed here. |
| Four-state (`x`/`z`) value propagation through the *analog* solver / MNA | LRMC-C1-02 requires the *value system* (POM `ValueType`, digital net values) to represent four-state correctly instead of silently coercing to `0` — that's in scope. Making the *analog* MNA solver reason about four-state conservative quantities is not an LRM requirement (four-state is a digital/`Integer`-literal concept) and is explicitly out of scope. |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| Scope = all 55 findings (Blocking + Degraded + Cosmetic), not just the 33 Blocking | All 55 | User's explicit choice: "Tudo (48) — Blocking + Degraded + Cosmetic" (the number was refined to 55 once the audit's tables were parsed row-by-row for this spec; same scope intent) | y |
| VPI/Annex E bar: typed `NotSupported` vs. real native equivalent | Real native equivalent required for every VPI ch.12 routine category and Annex E mapping the audit found `Degraded`/`NotSupported` | User's explicit choice: "Quero 100% mesmo" — a typed not-supported result, while fail-loud-compliant, is not "possible" in the sense the user means | y |
| **Design-phase rulings (2026-08-14, `design.md`):** the two `n` items below are now resolved — priority order confirmed with one amendment (`` `default_discipline ``/Annex F, LRMC-C10-01/18/19, hoisted into a new Phase 0 spine because LRMC-C7-01 cannot be tested without them), and four-state gets `Value::Logic(LogicVec{width,val,unk})` + `ValueType::Logic(w)`. Two further user rulings recorded there: the `SimOutput` sink in `piperine-solver` is the single runtime I/O path, and the ch.9 catalogue is implemented in full (see P3 AC8's amendment). | see `design.md` | — | y |
| Priority order across the four chapter-range findings | P1 = ch.1-4 (lexer/grammar/expression FORM fixes) first, because several later findings (assignment-pattern arrays used by `paramset`, custom access-function names used throughout ch.5 examples) are blocked by ch.1-4 FORM gaps; P2 = ch.5-6 (analog behavior + hierarchy); P3 = ch.7-9 (mixed-signal/scheduling/system-tasks — the largest, most cross-cutting bucket); P4 = ch.10-12 + annexes (directives, VPI, Annex C/D/E/F); P5 = the VPI/Annex-E native-equivalent upgrade (a distinct kind of work — new native implementations, not divergence fixes — layered after P4 closes the directive/annex-F findings those native equivalents may depend on, e.g. Annex F fixes touch the same discipline-resolution code VPI's `vpi_get_value`-family reads) | Dependency-ordered, not audit-file-ordered — matches how the original `verilog-ams-support` feature phased itself (parser before elaborator before codegen) | n (tech-lead call — flag if user wants a different order) |
| `defparam`/named-events/`paramset`/`InsertConnectModules`/the ch.8 scheduler/gate-primitives-UDPs wiring are each large enough to be their own task or small task cluster, not folded into a single "fix everything in this file" task | One task per LRMC-ID or tightly-coupled small cluster (e.g. the two `laplace_*` findings, C1-12 and C5-01, are one task since they're the same bug) | Matches the granularity discipline `verilog-ams-support` used (T1-T58, one file/pass/module per task) | y (methodology precedent) |
| Four-state (`x`/`z`/`?`) value representation (LRMC-C1-02) needs a new `ValueType` variant | Add `ValueType::FourState` (or equivalent) carrying a per-bit unknown/high-Z mask alongside the existing `Integer`/`Natural` variants, threaded through digital net values and system-task/comparison semantics | The audit found zero four-state representation anywhere in the value system — this is new surface, not a bug fix; flagged as the single largest data-model change in this feature, worth a Design-phase decision before Tasks | n (Design-phase call) |
| The `InsertConnectModules`/`ResolveDisciplines` ordering conflict (LRMC-C7-01) and Annex F's missing continuous-over-discrete rule (LRMC-C10-18/19) are fixed together, not separately | One coordinated fix: `ResolveDisciplines` gains discipline-domain awareness (continuous wins over discrete, `` `default_discipline `` consulted) *before* hard-failing, so genuinely incompatible disciplines still error but bridgeable ones reach `InsertConnectModules` | The two findings are causally linked — fixing one without the other still leaves connect-modules unreachable for real designs | y (root-cause coupling, not a preference) |
| Digital events in analog behavior (LRMC-C5-05) requires an Annex C typecheck-rule change, not a parser change | Remove/narrow the current blanket rejection in `elab/typecheck.rs`'s Annex C enforcement so `@(posedge clk)`-style sensitivity is permitted inside `analog` per §5.10.5, while everything else Annex C restricts stays restricted | LRM explicitly requires this exact case; the current rule is over-broad, not entirely wrong | y |

**Open questions:** none outstanding — every gray area is resolved above or
logged as an assumption with rationale; two items (`n`) are explicitly
flagged for Design-phase confirmation rather than silently decided.

---

## User Stories

### P1: Lexer, preprocessor, and expression-level FORM/FUNCTIONALITY conformance (ch.1-4) ⭐ MVP

**User Story**: As a Verilog-AMS author, I want every ch.1-4 lexical token,
literal form, operator, and expression construct the LRM defines to parse
and evaluate correctly, so that source using any standard literal/operator
form is never rejected by the lexer/parser before it even reaches
elaboration.

**Why P1**: Foundation layer — ch.5-9 findings about `paramset`s, custom
access functions, and assignment-pattern-initialized arrays are unreachable
until their own lexical/grammar prerequisites from ch.1-4 exist. This is
also the smallest slice that proves the audit-closure methodology: fix a
literal-lexing bug, write the LRM-example-derived regression test, gate,
commit — repeatable for every subsequent story.

**Acceptance Criteria** (each maps 1:1 to a Requirement ID below):

1. WHEN a based literal declares digits illegal for its base (e.g. `'d1F`, `'o89`) THEN the lexer SHALL reject it with a typed error naming the base and the illegal digit (LRMC-C1-01).
2. WHEN a based/unsized literal contains `x`/`z`/`?` four-state digits (e.g. `16'hx`, `12'hz`) THEN elaboration SHALL represent the four-state value, never silently coerce to `0` (LRMC-C1-02).
3. WHEN a based literal has whitespace between the base letter and its digits (e.g. `` 'D 3 ``, the LRM's own Example 2) THEN the lexer SHALL accept it (LRMC-C1-03).
4. WHEN an underscore appears in an illegal position in a number literal (e.g. immediately after a decimal point, `1._5`) THEN the lexer SHALL reject it, not silently strip it (LRMC-C1-04).
5. WHEN an attribute instance `(* ... *)` appears at an expression site the LRM's Syntax 2-5..2-10 permits (not just item/declaration-prefix sites) THEN the parser SHALL accept it (LRMC-C1-05).
6. WHEN source uses assignment-pattern syntax `'{expr, expr, ...}` for a parameter-array/string-list/multidimensional initializer THEN the lexer/parser SHALL accept it and the elaborator SHALL evaluate it per §3.4.4/3.4.6/3.4.8 (LRMC-C1-06).
7. WHEN an `aliasparam` aliases a non-`real` parameter THEN its `ValueType` SHALL match the aliased parameter's actual declared type, not be hardcoded to `Real` (LRMC-C1-07).
8. WHEN a discipline declares a custom `access` nature attribute (not `V`/`I`) THEN the grammar SHALL accept that name as an access-function call, and the elaborator SHALL resolve it to the correct branch per §3.6.1.2/§5.4.2 (LRMC-C1-08, LRMC-C1-09).
9. WHEN source uses the port access-function form `I(<port_name>)` THEN the parser SHALL accept the `<...>` delimiter form per §5.4.3's own worked example (LRMC-C1-10).
10. WHEN a real-valued constant with a nonzero fractional part is used where the LRM specifies implicit round-to-integer conversion (e.g. an array bound) THEN elaboration SHALL round, not error (LRMC-C1-11).
11. WHEN source calls `laplace_pm`/`laplace_nm`/`laplace_npm` (non-existent LRM names, a registry typo artifact) THEN it SHALL fail loud as an unknown operator; WHEN source calls the four real LRM spellings `laplace_np`/`laplace_zp`/`laplace_nd`/`laplace_zd` THEN all four SHALL resolve and simulate correctly (LRMC-C1-12, tracked jointly with LRMC-C5-01 as one fix).

**Independent Test**: Every AC above has a corresponding `.vams` snippet drawn from (or minimally adapted from) the LRM's own worked examples for that clause, compiled through `parse_and_elaborate`, asserting the LRM-specified outcome — runnable and demoable standalone without any P2-P5 work.

---

### P2: Analog behavior and hierarchical structures (ch.5-6)

**User Story**: As a device/testbench author, I want every ch.5 analog
behavior construct (events, operators, contribution semantics) and every
ch.6 hierarchical/instantiation construct to work exactly as declared, so
that real analog and structural designs — not just the vertical-slice
examples the prior `verilog-ams-support` feature proved — elaborate and
simulate correctly.

**Why P2**: The next dependency layer — depends on P1's literal/operator/
access-function fixes (several ch.5 examples use custom accessors and
`laplace_*`), and unblocks ch.7's connect-module work (P3) which needs
`defparam`/paramset-adjacent hierarchy semantics to be trustworthy first.

**Acceptance Criteria**:

1. WHEN source calls `laplace_nd(x, {num}, {den})` (the LRM's own canonical spelling) THEN it SHALL resolve and simulate correctly (LRMC-C5-01, same root cause as LRMC-C1-12).
2. WHEN `@(cross(expr, dir))` specifies a direction (`+1`/`-1`/`0`) THEN the event SHALL fire only for that crossing direction, not every crossing (LRMC-C5-02).
3. WHEN source uses `@(absdelta(expr, tol[, min_var[, max_rate]]))` THEN the event SHALL fire per §5.10.3.4's semantics (LRMC-C5-03).
4. WHEN source declares a named `event` and uses `-> my_event;`/`@(my_event)` THEN both SHALL parse and the trigger/sensitivity relationship SHALL work per §5.10.4 (LRMC-C5-04).
5. WHEN an `analog` block is sensitive to a digital `posedge`/`negedge`/state-change/named event per §5.10.5 THEN Annex C enforcement SHALL permit it, not reject it (LRMC-C5-05).
6. WHEN a `defparam hier.path.param = value;` statement is elaborated THEN the named parameter's value SHALL actually change, not be silently discarded (LRMC-C5-06).
7. WHEN a `paramset` is instantiated in place of a module THEN its pre-computed parameter bindings SHALL actually apply (LRMC-C5-07).
8. WHEN source declares `name_of_instance [msb:lsb] (port_connections)` THEN the parser SHALL accept it and elaboration SHALL create the specified range of instances (LRMC-C5-08).

**Independent Test**: A real analog module using named events + `absdelta` + directional `cross` simulates and produces the LRM-specified event timing; a hierarchy using `defparam`, a `paramset`, and an instance array elaborates into the exact instance/parameter graph the LRM specifies for each — checkable against `Design::modules`/`Design::flat_modules` directly, independent of P3-P5.

---

### P3: Mixed signal, scheduling, and system tasks (ch.7-9)

**User Story**: As a mixed-signal designer, I want connect-module bridging
(ch.7), the LRM's stratified event-scheduling cycle (ch.8), and the general
system task/function catalogue (ch.9) to actually run, not just exist as
untested-in-production Rust functions, so that `$display` prints, gate
primitives/UDPs simulate, and cross-discipline connections resolve through
`connectrules` instead of hard-failing.

**Why P3**: The largest, most cross-cutting bucket (15 findings) — and the
one with the most "fully built but zero callers" gaps, meaning most of the
work here is *wiring* existing tested code into the production pipeline
rather than writing new logic from scratch. Depends on P2's hierarchy work
(connect-module insertion sits in the same elaboration pass sequence as
`defparam`/generate) and on Annex F's discipline-domain fix (P4) — the two
are causally coupled per the Assumptions table.

**Acceptance Criteria**:

1. WHEN two nets with different but LRM-compatible disciplines connect across a module boundary and a matching `connectrules`/`connectmodule` pair exists THEN `InsertConnectModules` SHALL bridge them instead of `ResolveDisciplines` hard-failing first (LRMC-C7-01, coupled with P4's Annex F fix).
2. WHEN a `connectrules` block's body uses forms beyond the bare declaration shell (§7.7.1 Syntax 7-6) THEN the parser SHALL accept them (LRMC-C7-02).
3. WHEN a connect-module instantiation uses `connect_mode` or the parameter-passing attribute (§7.7.3/7.7.4) THEN elaboration SHALL apply their semantics, not just lex the keywords (LRMC-C7-03).
4. WHEN source uses `connect ... resolveto` (§7.4.4.3) THEN the parser and elaborator SHALL support it (LRMC-C7-04).
5. WHEN a `connectmodule` body calls `$driver_count`/`$driver_state`/`$driver_strength`/`driver_update`/`$driver_delay`/`$driver_next_state`/`$driver_next_strength`/`$driver_type` THEN these SHALL parse and return/apply live driver-resolution data (LRMC-C7-05).
6. WHEN an analog construct appears in a digital event-control expression or vice versa (§7.3.6's five listed cases) THEN the restriction SHALL be enforced exactly as the LRM lists it — verified, not assumed (LRMC-C7-06).
7. WHEN `$display`/`$write`/`$strobe`/`$monitor` execute (digital or analog context) THEN they SHALL produce actual output through the host's I/O path (LRMC-C7-07, LRMC-C7-08 closes with it — format-specifier/escape handling has a live consumer once this lands).
8. WHEN any ch.9 system task/function the LRM's Table 9-1-and-onward catalogue lists is called THEN it SHALL execute per its LRM semantics — every one of the ~30 currently-missing names (§9.5 file I/O, §9.9/§9.13 stochastic, §9.10 time, §9.12 plusargs, §9.16, §9.18-§9.21, §9.22/§9.23) is implemented natively; fail-loud `E4007` remains only for names the LRM does not define (LRMC-C7-09; amended at Design after the user's explicit "implementar todas nativamente" ruling — the earlier "or fail loud naming the missing task" alternative is withdrawn, it contradicted this spec's own "TUDOOOOO precisa ser possível" framing).
9. WHEN `crates/piperine-lang/tests/system_task_migration_completeness.rs` runs THEN it SHALL compile and pass against real `.vams` fixtures, not dead PHDL-era syntax (LRMC-C7-10).
10. WHEN `$discontinuity` is called inside an analog event action THEN it SHALL actually affect the solver's step-control/convergence behavior, not be elaborated and discarded (LRMC-C7-11).
11. WHEN a digital timestep processes events THEN they SHALL run in strict Active → Inactive → NBA → Monitor region order (re-entering Active on a later region's new Active-region event) per §8.3, using the already-built `RegionQueue` wired into the production scheduler (LRMC-C7-12).
12. WHEN a module instantiates a gate primitive (`and`/`nand`/`buf`/etc.) or a `primitive`/`endprimitive` UDP THEN it SHALL participate in the digital simulation with correct truth-table/state-table behavior, using the already-built `GatePrimitive`/`UdpTable` types wired into `kernel/digital/compile.rs`'s production compiler (LRMC-C7-13).
13. WHEN an analog-to-digital boundary uses `absdelta`-interpolated event timing (§8.4.6) THEN it SHALL be modeled, not silently absent (LRMC-C7-14).

**Independent Test**: A gate-level digital testbench using UDPs and multiple primitives, combined with an analog block calling `$display` and `$discontinuity`, and a mixed-discipline connect-module bridge, all in one design, simulates end-to-end and produces the LRM-specified console output, event ordering, and analog/digital coupling — independently checkable via `tests/vams_vertical_slice.rs`-style harness extended for this story, without needing P4/P5.

---

### P4: Compiler directives, VPI language surface, and Annex C/D/E/F

**User Story**: As any Verilog-AMS author, I want every ch.10 compiler
directive, the ch.11-12 VPI language surface, and Annex C/D/E/F's rules to
match the LRM exactly, so that real-world source files using
`` `celldefine ``/`` `default_discipline ``/`` `begin_keywords ``-style
directives, VPI-associated system calls, and Annex D's standard nature/
discipline/constant set all work without silent or undocumented gaps.

**Why P4**: Cross-cutting infrastructure findings, not a single subsystem —
grouped last because several (Annex F's continuous-over-discrete rule,
`` `default_discipline ``) are prerequisites the P3 connect-module work
(LRMC-C7-01) needs to be meaningful in practice, so this story's
Annex-F-and-directive items land *alongside* P3 in execution order even
though numbered P4 for narrative clarity — see the dependency note in
Assumptions and the Design phase for the actual task sequencing.

**Acceptance Criteria**:

1. WHEN source uses `` `default_discipline ``, `` `default_transition ``, `` `begin_keywords ``/`` `end_keywords ``, `` `celldefine ``/`` `endcelldefine ``, `` `nounconnected_drive ``/`` `unconnected_drive ``, `` `line ``, `` `pragma ``, or `` `default_nettype `` THEN the preprocessor SHALL parse them per their LRM/1364-2005-inherited semantics, not fail with `UndefinedMacro` (LRMC-C10-01 through LRMC-C10-09).
2. WHEN `` `timescale ``/`` `resetall `` are used THEN their documented "pass through" behavior SHALL be real, or the doc comment SHALL be corrected to match actual (documented, not silent) behavior (LRMC-C10-10).
3. WHEN a `.vams` file is parsed THEN `__VAMS_ENABLE__`/`__VAMS_COMPACT_MODELING__` SHALL be defined per §10.5 (LRMC-C10-03).
4. WHEN a plugin registers a VPI callback (`vpi_register_cb`), reads/writes an object value (`vpi_get_value`/`vpi_put_value`), or queries delay/timing info (`vpi_get_delays`/`vpi_put_delays`) THEN each SHALL have a real native equivalent — not the currently-typed `Partial`/`NotSupported` results — per the P5 VPI-native-upgrade scope (LRMC-C10-11, LRMC-C10-12, LRMC-C10-13, LRMC-C10-14 — tracked here for their FORM/FUNCTIONALITY divergence, closed under P5's native-equivalent work).
5. WHEN `ResolveDisciplines` encounters two different disciplines meeting at an undeclared net's connection THEN it SHALL resolve to the continuous discipline (Basic resolution's headline rule) instead of hard-failing, and SHALL consult an in-force `` `default_discipline `` directive before declaring the discipline unknown (LRMC-C10-18, LRMC-C10-19 — the root-cause fix that also unblocks LRMC-C7-01).
6. WHEN `connect ... resolveto` (Annex F §7.4.1 variant) is used THEN it SHALL be supported (LRMC-C10-20, same underlying feature as LRMC-C7-04).
7. WHEN Annex D.3's `driver_access.vams` standard definitions are referenced THEN they SHALL be ported into `headers/` like D.1/D.2 already are (LRMC-C10-16).

**Independent Test**: A `.vams` file exercising every listed directive plus `` `default_discipline ``-driven resolution across a mixed-discipline hierarchy parses and elaborates correctly end-to-end, independently of P5's VPI-routine-level work (P4's AC4 only requires the directive-level plumbing exists; the deep VPI native-equivalent build-out is P5's own test surface).

---

### P5: VPI routine categories and Annex E mappings — full native equivalents

**User Story**: As a plugin author relying on VPI-style introspection/
callback/value-access routines, or as someone cross-checking against SPICE
tooling via Annex E's defined mappings, I want every routine category and
mapping to have a complete, correctly-behaving native implementation — not
a typed "not supported" result — so that "possible" means "actually works,"
per the user's explicit bar for this feature.

**Why P5**: This is the user's explicit widening beyond the audit's own
findings (the audit itself rated VPI/Annex E as the *best*-documented area
of the codebase, precisely because its gaps were disclosed via typed
results rather than silent). Sequenced last because it depends on P4's
directive/Annex-F fixes (VPI value-access routines read through the same
discipline-resolution and net-naming machinery Annex F's fix touches) and
because building genuinely new native functionality (a delay/timing-check
model for `vpi_get_delays`/`vpi_put_delays`, a real event-granular callback
registry instead of `HookPhase`-level, write-path `vpi_put_value`) is a
different kind of work than fixing a divergence — closer to new capability
than to "make the parser accept what the LRM says."

**Acceptance Criteria**:

1. WHEN a plugin calls `vpi_register_cb` for a specific event (not just a coarse `HookPhase`) THEN the callback SHALL fire exactly at that event, and `vpi_remove_cb` SHALL actually deregister it (LRMC-C10-11, upgraded from `Partial`/`NotSupported`).
2. WHEN a plugin calls `vpi_get_delays`/`vpi_put_delays` THEN it SHALL read/write a real delay/timing-check model the digital kernel now maintains (LRMC-C10-12, upgraded from `NotSupported` — requires the digital kernel to gain a delay model, a real new capability).
3. WHEN a plugin calls `vpi_put_value` THEN it SHALL actually drive the target object's value through the solver, symmetric with the already-working `vpi_get_value` read path (LRMC-C10-13, upgraded from `Partial`).
4. WHEN a plugin calls any `vpi_mcd_*` routine THEN it SHALL have a real native equivalent (LRMC-C10-14, upgraded from `Partial`/`NotSupported`).
5. WHEN Annex E's SPICE-compatibility mappings that are currently `Degraded (documented)` are exercised (excluding the raw-netlist-reader-dependent E.2.1/E.3.2, which stay Out of Scope per this spec's own table) THEN they SHALL match Annex E's defined mapping exactly.

**Independent Test**: A plugin exercising `vpi_register_cb`/`vpi_put_value`/`vpi_get_delays` against a live simulation observes real, correct, event-accurate behavior — checkable via a `piperine-plugin` integration test independent of P1-P4 (though it requires them to already be merged, since it runs on top of the same elaboration/scheduling pipeline P3/P4 fixed).

---

## Edge Cases

- WHEN a `.vams` file uses a construct this spec closes AND a construct still genuinely out of scope (e.g. a raw SPICE netlist include) THEN the in-scope parts SHALL work and the out-of-scope part SHALL fail loud naming exactly what's unsupported and why (never a partial silent success).
- WHEN the four-state value work (LRMC-C1-02) encounters a four-state value flowing into a context that has no defined four-state semantics (e.g. an array index) THEN elaboration SHALL fail loud citing the LRM clause that makes such usage illegal, not silently truncate to a two-state value.
- WHEN `ResolveDisciplines`'s new continuous-over-discrete rule (LRMC-C10-18) encounters two *disciplines of the same domain* that are still incompatible (not a continuous/discrete mismatch) THEN it SHALL still hard-fail exactly as today — the fix narrows the false-positive case, it does not loosen genuine incompatibility detection.
- WHEN a regression test for a Blocking finding is written THEN it SHALL be derived from the LRM's own worked example for that clause wherever the LRM provides one (most do, per the audit's own evidence citations) — not from a synthetic minimal reproduction that might miss a real-world detail the LRM's example carries.
- WHEN a fix to one LRMC-ID risks affecting a different, already-closed LRMC-ID (e.g. the Annex F fix and the digital-events-in-analog fix both touch `elab/typecheck.rs`-adjacent code) THEN the task closing the later ID SHALL re-run the regression test for the earlier one as part of its own gate, not just its own new test.

---

## Requirement Traceability

Full evidence (LRM clause citation, exact `file:line`, and the negative
searches proving absence) lives in `docs/audit/{ch1-4,ch5-6,ch7-9,
ch10-12-annexes}.md` — this table is the tracking index, not a duplicate of
the evidence.

| Requirement ID | Story | LRM Clause | Category | Severity | Area |
|---|---|---|---|---|---|
| LRMC-C1-01 | P1 | §2.6.1 | FORM | Degraded | Over-permissive per-base digit validation |
| LRMC-C1-02 | P1 | §2.6.1 | FUNCTIONALITY | Blocking | Four-state x/z/? silently coerced to 0 |
| LRMC-C1-03 | P1 | §2.6.1 | FORM | Blocking | Based-literal whitespace after base letter unlexable |
| LRMC-C1-04 | P1 | §2.6.1/§2.6.2 | FORM | Cosmetic | Underscore placement over-permissive |
| LRMC-C1-05 | P1 | §2.9/§2.9.1 | FORM | Degraded | Expression-level attribute instances unsupported |
| LRMC-C1-06 | P1 | §3.4.2/3.4.4/3.4.6/3.4.8/4.2.14 | FORM | Blocking | Assignment-pattern `'{...}` unlexable |
| LRMC-C1-07 | P1 | §3.4.7 | FUNCTIONALITY | Cosmetic | `aliasparam` always typed `Real` |
| LRMC-C1-08 | P1 | §4.4/§3.6.1.2 | FORM | Blocking | Access-function name hardcoded to V/I |
| LRMC-C1-09 | P1 | §4.4/§5.4.2/§5.4.3 | FUNCTIONALITY | Blocking | Custom discipline accessor names not resolved |
| LRMC-C1-10 | P1 | §4.4/§5.4.3 | FORM | Blocking | Port access operator `I(<port>)` unparseable |
| LRMC-C1-11 | P1 | §4.2.1.1 | FUNCTIONALITY | Degraded | Real→integer conversion errors instead of rounding |
| LRMC-C1-12 | P1 | §4.5.11.1-4 | FUNCTIONALITY | Blocking | `laplace_*` registry typo (joint fix with LRMC-C5-01) |
| LRMC-C5-01 | P2 | §5.6 | FUNCTIONALITY | Blocking | `laplace_nd` never resolves (same root cause as LRMC-C1-12) |
| LRMC-C5-02 | P2 | §5.10.3.1 | FUNCTIONALITY | Degraded | `cross()` direction argument ignored |
| LRMC-C5-03 | P2 | §5.10.3.4 | FUNCTIONALITY | Blocking | `absdelta` event function unimplemented |
| LRMC-C5-04 | P2 | §5.10.4 | FORM+FUNCTIONALITY | Blocking | Named events (`->` trigger, `event` decl) unsupported |
| LRMC-C5-05 | P2 | §5.10.5 | FUNCTIONALITY | Blocking | Digital edge events in analog behavior wrongly rejected |
| LRMC-C5-06 | P2 | §6.3.1 | FUNCTIONALITY | Blocking | `defparam` silently discarded |
| LRMC-C5-07 | P2 | §6.4 | FUNCTIONALITY | Blocking | `paramset` parsed but never consumed |
| LRMC-C5-08 | P2 | §6.2.2 | FORM | Blocking | Array-of-instances syntax unparseable |
| LRMC-C7-01 | P3 | §7.4.4.1/§7.4.4.3/§7.5-7.8 | FUNCTIONALITY | Blocking | `InsertConnectModules` unreachable (coupled with LRMC-C10-18/19) |
| LRMC-C7-02 | P3 | §7.7.1 | FORM | Degraded | `connectrules` body syntax gap |
| LRMC-C7-03 | P3 | §7.7.3/§7.7.4 | FORM | Degraded | `connect_mode`/parameter-passing attribute unwired |
| LRMC-C7-04 | P3 | §7.4.4.3 | FORM | Cosmetic | `connect ... resolveto` unimplemented (disclosed) |
| LRMC-C7-05 | P3 | §7.6/§9.22-9.23 | FORM+FUNCTIONALITY | Blocking | Driver-access system functions absent |
| LRMC-C7-06 | P3 | §7.3.6 | FUNCTIONALITY | Degraded | Analog/digital concurrency-rule cases unverified |
| LRMC-C7-07 | P3 | §9.2/Table 9-1 | FUNCTIONALITY | Blocking | `$display`-family produces no output |
| LRMC-C7-08 | P3 | §9.2/§9.4.2/§9.4.4 | FUNCTIONALITY | Cosmetic | Format specifiers/escapes (moot until LRMC-C7-07 lands) |
| LRMC-C7-09 | P3 | §9.2-9.23 | FORM | Blocking | ~15 of 23 ch.9 system-task subsections unimplemented |
| LRMC-C7-10 | P3 | §9.2 | FUNCTIONALITY | Blocking | Coverage test fixture is dead PHDL syntax |
| LRMC-C7-11 | P3 | §9.17.1 | FUNCTIONALITY | Degraded | `$discontinuity` elaborates then discarded |
| LRMC-C7-12 | P3 | §8.3 | FUNCTIONALITY | Blocking | 4-region stratified scheduler unwired |
| LRMC-C7-13 | P3 | §8.5.3.5 | FUNCTIONALITY | Blocking | Gate primitives/UDPs unwired |
| LRMC-C7-14 | P3 | §8.4.6 | FUNCTIONALITY | Degraded | `absdelta`-interpolated A2D events unimplemented |
| LRMC-C10-01 | P4 | §10.2 | FORM+FUNCTIONALITY | Blocking | `` `default_discipline `` unparseable |
| LRMC-C10-02 | P4 | §10.3 | FORM+FUNCTIONALITY | Blocking | `` `default_transition `` unparseable + unmodeled |
| LRMC-C10-03 | P4 | §10.5 | FUNCTIONALITY | Blocking | Predefined macros never registered |
| LRMC-C10-04 | P4 | §10.6 | FORM | Blocking | `` `begin_keywords ``/`` `end_keywords `` unparseable |
| LRMC-C10-05 | P4 | ch.10 table | FORM | Blocking | `` `celldefine ``/`` `endcelldefine `` unparseable |
| LRMC-C10-06 | P4 | ch.10 table | FORM | Blocking | `` `nounconnected_drive ``/`` `unconnected_drive `` unparseable |
| LRMC-C10-07 | P4 | ch.10 table | FORM | Blocking | `` `line `` unparseable |
| LRMC-C10-08 | P4 | ch.10 table | FORM | Blocking | `` `pragma `` unparseable |
| LRMC-C10-09 | P4 | ch.10 table | FORM | Blocking | `` `default_nettype `` unparseable |
| LRMC-C10-10 | P4 | ch.10 table | FORM | Blocking | `` `timescale ``/`` `resetall `` doc comment is false |
| LRMC-C10-11 | P4→P5 | §11.2.1/Table 11-1 | FUNCTIONALITY | Degraded (P4) → closed (P5) | VPI callback registration event-grained upgrade |
| LRMC-C10-12 | P4→P5 | §11.4 Table 11-7 | FUNCTIONALITY | Degraded (P4) → closed (P5) | VPI delay/timing-check native model |
| LRMC-C10-13 | P4→P5 | §11.4 Table 11-8 | FUNCTIONALITY | Degraded (P4) → closed (P5) | `vpi_put_value` write path |
| LRMC-C10-14 | P4→P5 | §11.4 Table 11-10 | FUNCTIONALITY | Cosmetic (P4) → closed (P5) | `vpi_mcd_*` family |
| LRMC-C10-15 | Out of Scope | Annex C.4 | FORM | Cosmetic | No distinct Verilog-A subset mode (moot) |
| LRMC-C10-16 | P4 | Annex D.3 | FUNCTIONALITY | Degraded | `driver_access.vams` not ported |
| LRMC-C10-17 | Out of Scope | Annex E.2.1/E.3.2 | FUNCTIONALITY | Degraded | Raw SPICE-netlist reader doesn't exist |
| LRMC-C10-18 | P4 | Annex F §7.4.4.1 | FUNCTIONALITY | Blocking (undisclosed) | Continuous-over-discrete rule missing |
| LRMC-C10-19 | P4 | Annex F §7.4.4.1 step 4.b | FUNCTIONALITY | Blocking | `` `default_discipline `` fallback not consulted |
| LRMC-C10-20 | P4 | Annex F §7.4.1 | FUNCTIONALITY | Degraded | `connect ... resolveto` (same feature as LRMC-C7-04) |

**ID format:** `LRMC-[chapter-range]-[sequence]`, matching the audit
report each finding was first recorded in.

**Status values:** Pending → In Design → In Tasks → Implementing → Verified

**Coverage:** 51 requirement IDs (55 findings, 3 pairs deduplicated to a
single joint ID: LRMC-C1-12/LRMC-C5-01 share one root cause, LRMC-C7-04/
LRMC-C10-20 are the same `resolveto` feature seen from ch.7 and Annex F,
LRMC-C10-11..14 carry both a P4 divergence-closure AC and a P5
native-upgrade AC), **51 mapped to tasks** in `tasks.md` (T1–T67), 0
unmapped — `tests/lrm_conformance_matrix.rs` (T9, locked at T67) is the
mechanical guard that keeps this true. 2 findings (LRMC-C10-15, LRMC-C10-17) are Out of Scope, not
tracked toward closure.

---

## Success Criteria

- [ ] Every Blocking/Degraded/Cosmetic finding in `docs/audit/` that isn't
      listed in this spec's Out of Scope table has a closing commit, a
      regression test derived from its LRM clause, and its
      `docs/audit/*.md` row updated to reference the closing commit.
- [ ] A follow-up audit pass (same four-chapter-range methodology as the
      original) finds zero repeat findings for anything this spec closed.
- [ ] `cargo build --workspace` stays zero-warning; every test passing
      before this feature started stays passing (git-stash A/B verified,
      target-by-target, matching the `verilog-ams-support`/MD-47 precedent).
- [ ] Every VPI ch.12 routine category and Annex E mapping in scope (i.e.
      not LRMC-C10-15/17) returns real, correct results — no
      `VpiResult::NotSupported`/`Partial` remains for anything this spec's
      P5 covers.
