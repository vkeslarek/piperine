# Verilog-AMS Support (PHDL → Verilog-AMS pivot) Specification

## Problem Statement

PHDL is Piperine's bespoke HDL, purpose-built to feed a native in-house solver
without any SPICE/Verilog-AMS dependency. It works, but it is a private
language: no existing Verilog-AMS model library, testbench, or tool
interoperates with it, and every device model (including the hand-ported
`headers/spice/*.phdl` ngspice models) had to be rewritten from scratch. This
feature pivots Piperine's frontend from PHDL to **Verilog-AMS LRM 2.4**
(`VAMS-LRM-2-4.md`, the Accellera 2.4.0 standard, 2014-05-30) as a
**superset**: full LRM 2.4 conformance as the floor, with Piperine's own
proven extensions (the `(* attribute *)`-driven declared-language-surface
mechanism, MD-24, `@device`/`@port` plugin schemas, capability system,
introspection) carried forward as attribute-instance-based extensions native
to the VAMS grammar (§2.9) rather than dropped. PHDL is retired outright —
this is a replacement of the frontend, not an addition alongside it.

The rest of the pipeline (POM → `piperine_codegen::resolve` → flatten/emit →
kernel → device → native solver, per `CLAUDE.md`'s pipeline diagram) is
**out of scope for behavior change**: this feature replaces
`parse_and_elaborate`'s input language and the POM-construction path feeding
it; `piperine-solver`, `piperine-api`, and the codegen stages downstream of
the POM are not being redesigned by this feature, only extended where the POM
itself must grow new node kinds VAMS requires and PHDL didn't have (nature,
branch, genvar/generate — see Assumptions).

**Why now:** the user has decided this is a "pivot on a super important part
of the business" — moving to an industry-standard HDL is the strategic
driver; the previous PHDL-only decision's rationale was not found recorded in
`.specs/STATE.md`'s decision log (searched; no MD-NN entry exists for it), so
this pivot is not reversing a documented tradeoff, it is a fresh direction
call.

## Precedent — this was tried before

A full `piperine-ams` crate (hand-written Verilog-AMS parser + grammar +
`.vams` headers + BSIM6/HICUM/ASMHEMT Verilog-A model fixtures) existed and
worked well enough that `piperine-cli check` dispatched on file extension
(`.va`/`.vams` → AMS parser, `.phdl`/`.ppr` → PHDL parser) with a
`main.phdl` → `main.vams` fallback (commits `f2a9b67`, `e58f021`, `1a0b780`,
2026-06-24..30). It was deleted in `1aaf6cf` (2026-07-03); the actual
"PHDL becomes the only frontend" decision is recorded after the fact in
`ee39953`'s message: *"Verilog-AMS (the IR's other former producer) is gone;
with PHDL/PPR as the only frontend, the IR had become a redundant parallel
representation of the POM."* No commit states why VAMS was dropped in favor
of PHDL rather than the reverse. **This feature is not starting from zero on
prior art** — `piperine-ams`'s old grammar/lexer/parser (`git show
1aaf6cf~1:crates/piperine-ams/...`) is recoverable from git history as a
reference implementation, even though its target IR (the old standalone
`piperine-ir`, unrelated to the current `piperine-ir`/former
`piperine-signal` crate) no longer exists and its output must now target the
POM directly.

## Current-architecture baseline (what this feature must reconcile with)

Established by codebase scan before writing this spec:

| VAMS concept | PHDL today | Gap for VAMS |
|---|---|---|
| `discipline`/`nature` | `DisciplineDecl`/`DisciplineItem` (`parse/parser/discipline_decl.rs`) — nature is inline inside a discipline, not a standalone reusable declaration | VAMS's `nature` is a **top-level, independently declared and referenced** construct (LRM §3.6.1) — POM needs a new first-class `Nature` node PHDL never had |
| `branch` / access functions `V()`/`I()` | No first-class `Branch` type; `<+` (`BindOp::Contribution`) resolves through net/terminal references directly | VAMS has explicit optional `branch` declarations (named branches, `V(br)`) plus implicit branches — POM needs a `Branch` concept |
| `analog`/digital `always` block | `BehaviorKind::Analog`/`Digital` already first-class in `pom/behavior.rs` | Direct match — lowest-risk area |
| Contribution statement `V(p,n) <+ expr` | Already exists, documented as PHDL's behavioral-source primitive (MD-39) | Direct match |
| `genvar` / generate blocks | **Does not exist anywhere** in `piperine-lang` | New POM construct + elaboration-time unrolling, entirely new |
| `(* attribute *)` instances | PHDL's own `extern attribute` / `@device`/`@port` schema mechanism (MD-24) is hand-rolled, not attribute-syntax-driven | VAMS's native attribute-instance syntax (§2.9, attached to modules/ports/items/UDPs/port-connections) becomes the carrier for the same MD-24 schemas — this is the "superset" hook the user confirmed |
| Const evaluator (`eval/interp.rs`, 863 lines) | Imports `crate::parse::ast` directly — **coupled to PHDL's concrete AST**, not the POM | Must be retargeted to VAMS AST or re-pointed at the POM; cannot be reused as-is |
| `headers/spice/*.phdl` (mos/bjt/jfet/diode/...) | Hand-ported from **ngspice C source**, not Verilog-A | Manual rewrite into VAMS is a translation of PHDL's own syntax, not a port from a third source — mechanical but nontrivial (P2 story below) |
| OSDI / `piperine-osdi` | Does not exist in this repo; referenced only as an external/future plugin path (`ROADMAP.md`); zero live Verilog-A→OSDI compilation dependency today | **Decision (user, this round): OSDI is a strict subset of Piperine's native `Element` ABI (MD-11: "OSDI as checklist, not ABI") — this feature targets the native ABI only and does not pursue OSDI at all**, not even as a later phase. Compact models parsed from `.va` bind to the native ABI directly, same as every other device |

## Goals

- [ ] Verilog-AMS LRM 2.4 becomes Piperine's sole frontend: `.va`/`.vams`
      source parses, elaborates to the POM, and reaches the existing
      solver pipeline unchanged downstream of the POM.
- [ ] Full Verilog-1364 digital semantics (event-driven scheduler, `always`/
      `initial`, all gate primitives, UDPs) are supported, not just the
      subset the current digital kernel already covers.
- [ ] Piperine's own extensions (declared-language-surface/MD-24 schemas,
      capability system, introspection, constraint blocks) survive the
      pivot, re-expressed through VAMS's native `(* attribute *)` syntax —
      Piperine's grammar is a **superset** of LRM 2.4, never a subset.
- [ ] PHDL is fully retired: `piperine-lang`'s PHDL parser/elaborator/headers
      are removed once VAMS parity is reached (see phasing — removal is the
      last P1 task, gated on the existing PHDL test suite's equivalent
      passing under VAMS).
- [ ] All example/fixture/stdlib `.phdl` sources are manually rewritten to
      `.vams`/`.va`, one at a time, as explicit tasks (no automated
      translator).

## Out of Scope

**Correction (user, after first draft): "100% dos pontos do LRM" — every
normative chapter and annex must map to a requirement, none dropped for
scope. Only the items below survive as genuinely out of scope, and none of
them are LRM content — they're tooling/architecture decisions the LRM itself
doesn't mandate.** The full chapter/annex-by-chapter mapping is the **LRM
Coverage Map** at the end of this document; nothing in ch. 1–12 or Annex A–F
is excluded. Annex E (SPICE compatibility) moved into P3 (VAMS-22). VPI
(ch. 11–12) stays fully in scope as *language surface* (parses, elaborates,
executes); only its C-ABI hosting mechanism is re-anchored (see next table).

| Item | Reason |
|---|---|
| Hosting third-party VPI `.so`/`.dll` PLI applications written against the literal IEEE VPI C headers | Not an LRM requirement — the LRM defines VPI *behavior*, not that a specific C ABI must be the implementation vehicle. Piperine's plugin ABI is native Rust traits (`plugin-abi-custom` decision, `piperine-plugin`); VPI's language-level surface (every system task/function ch. 11 describes, every routine ch. 12 defines) is fully in scope and routes through that native ABI instead |
| OSDI / `piperine-osdi` in any form (native Verilog-A→OSDI compilation, OSDI-wrapper loading, OSDI as a target ABI) | User decision this round: OSDI is a strict subset of Piperine's native `Element` ABI (MD-11: "OSDI as checklist, not ABI"). Compact models parsed from `.va` bind to the native ABI directly — OSDI is not pursued at all, not even deferred to a later phase |
| Automated PHDL→VAMS source translator | User chose manual rewrite; an automated translator is extra tooling never used past the one-time migration, not LRM content |
| Annex G (informative change history), Annex H (informative glossary) | Explicitly marked **informative**, not **normative**, by the LRM itself (front matter, §1.4) — no behavior to implement; ported into project docs as reference material only (VAMS-26, P3), not a compiler requirement |
| Changing `piperine-solver`/`piperine-api`/anything downstream of the POM | This feature is a frontend replacement; the native solver, `Element` ABI, and host API are unaffected by design — not LRM content, an existing architecture boundary. **Scoped by T50/MD-46 (2026-08-13, resolving design.md §8's R7):** this row means no redesign of the **analog** solver or the `Element` ABI — extending `piperine-solver`'s **digital event scheduler** to the LRM ch. 8 stratified event queue (active/inactive/NBA/monitor regions) is in-scope-by-necessity for VAMS-20 and is not blocked by this row |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| POM must gain `Nature` (standalone) and `Branch` (named + implicit) node kinds | Extend the POM rather than replace it; keep the POM-navigability UNBREAKABLE RULE (`CLAUDE.md`) — the POM must mirror **VAMS** source structure now, exactly as it mirrored PHDL before | The rule is language-agnostic by design ("mirrors the structure of the original code"); swapping the source language doesn't suspend it | n (flagged for Design-phase confirmation — this is an architecture decision, not a WHAT-level one, but the direction is unambiguous enough to record as the working assumption) |
| `genvar`/generate blocks are new POM + elaboration surface, not retrofitted onto anything existing | Build fresh; no PHDL precedent to reuse | Grep-confirmed zero existing `genvar` concept anywhere in `piperine-lang` | y (fact, not a choice) |
| `eval/interp.rs`'s const evaluator is rebuilt against the POM (or a new VAMS-AST-shaped evaluator), not reused as-is | Retarget to operate post-elaboration on the POM where possible, matching how `resolve/pom::LoweredBody` already works downstream | `eval/interp.rs` imports `crate::parse::ast` directly (PHDL-AST-coupled); reusing it verbatim is not possible once that AST is gone | n (Design-phase call) |
| MD-24 "declared language surface" rule (every referenceable name resolves to a textual declaration) is **kept**, re-anchored on VAMS's own normative Annex B (keywords) / Annex D (standard definitions) plus Piperine's `(* attribute *)`-carried schemas for the Piperine-specific surface (`@device`/`@port` etc.) | Keep MD-24's spirit; source of truth for the *language's own* built-ins becomes the LRM itself (Annex D ships as the built-in `disciplines.vams`-equivalent), Piperine's own additions stay attribute-declared | Directly reuses the user's confirmed "superset with attributes" framing | y |
| VPI-driven system tasks/functions are parsed and elaborated (so `.va`/`.vams` sources using them don't fail to parse) but their *runtime implementation* routes through Piperine's existing native plugin ABI, not a literal VPI C shim | Parse fully (LRM conformance), implement via native ABI | Consistent with `plugin-abi-custom` (device ABI is native traits, OSDI-equivalents are wrapped, not the ABI itself) | y |
| `headers/spice/*.phdl` device models are rewritten into `.va` **by hand**, preserving the ngspice-faithful behavior they already encode (not re-derived from a different source) | Manual, one file at a time, P2 | User confirmed manual rewrite for migration; these files' docstrings already cite exact ngspice source files as their oracle — that provenance must survive the rewrite | y |
| Full Verilog-1364 digital (UDPs, all gate primitives, full event scheduler edge cases) ships in P3, not P1 | P1 ships a working analog+digital vertical slice (proven end-to-end against the existing solver, matching what PHDL demonstrably does today); full digital completeness is explicitly a later phase within the same feature, not a separate feature | User asked for full LRM/full digital as the feature's *end state*; per the skill's own P1=vertical-slice principle, a first commit that boils the ocean has no working demo — phased P1→P2→P3 delivers the same final scope without blocking on it | n (phasing choice within an already-confirmed full-scope goal — flag for pushback if the user wants P1 itself to be full digital) |
| Attribute-carried Piperine extensions (capability system, introspection, constraint blocks) map 1:1 onto VAMS module/port/item attribute-instance syntax (LRM §2.9, Syntax 2-5..2-10) | Yes — confirmed available on module, port, item, function/task/block, port-connection, and UDP declarations, which covers every site PHDL's own `extern`/capability declarations currently attach to | LRM text read directly (§2.9.1) before writing this spec | y |

**Open questions:** none outstanding — every gray area above is either
resolved or logged as an assumption with rationale, per the closure gate.
Architecture-level questions explicitly deferred to the Design phase are
marked `n` above and are not blocking Specify.

---

## User Stories

### P1: VAMS vertical slice replaces PHDL for a working analog+digital circuit ⭐ MVP

**User Story**: As a Piperine device/circuit author, I want to write a
module in Verilog-AMS syntax (disciplines, nature, branches, analog block
with contribution statements, a digital `always` block, hierarchical
instantiation) and have it parse, elaborate to the POM, and simulate on the
existing native solver — so that the pivot's core value (industry-standard
syntax, unmodified solver) is proven end-to-end before expanding coverage.

**Why P1**: Nothing else in this feature matters if VAMS source can't reach
a working simulation. This is the smallest slice that proves the whole
pipeline: lexer → parser → elaborator → POM (with new `Nature`/`Branch`
nodes) → existing `piperine_codegen`/solver, unchanged downstream.

**Acceptance Criteria**:

1. WHEN a `.vams` file declares a `nature` and a `discipline` binding it
   (LRM §3.6.1–3.6.2) THEN the elaborator SHALL produce POM `Nature` and
   discipline entries equivalent in expressive power to today's
   `headers/disciplines.phdl` (`Electrical`, potential/flow, units, access
   functions).
2. WHEN a `.vams` module declares an `analog` block containing a
   contribution statement `V(p,n) <+ <expr>;` referencing an implicit or
   named branch THEN the elaborator SHALL resolve it to the same POM
   contribution representation PHDL's `<+` already produces, reaching
   `piperine_codegen::resolve::lower_bodies` unchanged.
3. WHEN a `.vams` module declares a digital `always @(...)` block with
   blocking/non-blocking assignment THEN it SHALL elaborate to a POM
   `BehaviorKind::Digital` body, reusing the existing digital kernel path.
4. WHEN one module instantiates another with named or ordered port
   connections and parameter overrides THEN the POM SHALL expose the same
   navigable instance→submodule→sub-instance tree the POM-navigability
   UNBREAKABLE RULE already requires for PHDL, now mirroring VAMS source
   structure instead.
5. WHEN a `.vams` source has a lexical, syntax, or elaboration error THEN
   the compiler SHALL fail loud with a source-location-anchored diagnostic
   (line/column), never silently drop or approximate the construct — same
   fail-loud philosophy as `CodegenError::Unsupported`.
6. WHEN the P1 vertical slice is complete THEN a single representative
   example (RC circuit analog + a simple digital counter, mirroring an
   existing PHDL test fixture pair) SHALL simulate on
   `piperine-solver`/`piperine-api` and produce results matching the
   PHDL-authored equivalent within existing numeric tolerances
   (`tests/support/ngspice.rs`'s `ToleranceContract`, reused not
   redefined).
7. WHEN a `.vams` source uses LRM ch. 2 lexical forms (integer/real
   constants incl. SI-scale suffixes, string literals, escaped
   identifiers, `` `define``-expanded tokens) or ch. 4 expressions (all
   operators, math functions, time-domain functions `ddt`/`idt`/
   `absdelay`/`transition`/`slew`/`limexp`/`last_crossing`/etc.) THEN they
   SHALL parse and evaluate with the same semantics the LRM defines —
   these already have PHDL precedent (`headers/math.phdl`,
   `headers/operators.phdl`) to cross-check against, so this ships in P1
   alongside the vertical slice rather than waiting for P2/P3.
8. WHEN the parser/elaborator's own test suite is checked against LRM
   Annex A (formal BNF) and Annex B (keyword list) THEN every
   Annex-A-defined production SHALL have a corresponding parser rule and
   every Annex-B keyword SHALL be reserved — tracked as an ongoing
   conformance gate through P1–P3, not a one-time task (VAMS-23/VAMS-24).
9. WHEN a construct is used outside the context LRM Annex C's analog
   language subset permits (e.g. a digital-only construct inside an
   analog-only evaluation context) THEN elaboration SHALL fail loud citing
   the Annex C restriction — first enforced in P1 for the constructs P1
   already supports, extended as later phases add more.

**Independent Test**: Compile and simulate the RC+counter VAMS example
end-to-end; diff its output against the existing PHDL fixture's recorded
result. Separately, run the parser conformance gate (VAMS-23/24) against
Annex A/B and confirm no production/keyword is unhandled for the
constructs P1 claims to support.

---

### P2: Standard library and example corpus migrated to Verilog-AMS

**User Story**: As a Piperine maintainer, I want the existing SPICE-derived
device models and example corpus rewritten in Verilog-AMS so that the
project has a working, VAMS-native standard library instead of a
PHDL-shaped one nobody can build on anymore.

**Why P2**: The P1 slice proves the pipeline; P2 makes the pivot usable —
without it, VAMS support exists but there's nothing to write with.

**Acceptance Criteria**:

1. WHEN `headers/spice/mos.phdl`, `bjt.phdl`, `jfet.phdl`, `diode.phdl`,
   `sources.phdl`, `passives.phdl`, `switches.phdl`, `controlled.phdl`,
   `tline.phdl`, `urc.phdl` are rewritten to `.va` THEN each SHALL retain
   its existing ngspice-source docstring provenance and pass the same
   validation this repo already runs against it (`tests/ngspice_*.rs`
   family), unmodified in tolerance/oracle, only in source language.
2. WHEN `headers/prelude.phdl`, `constants.phdl`, `math.phdl`,
   `operators.phdl`, `tasks.phdl` are ported THEN their VAMS equivalents
   SHALL be sourced from LRM Annex D ("standard definitions") where the LRM
   defines the equivalent construct, and from Piperine's own
   attribute-carried extension surface where it doesn't (device/port
   schemas, capabilities, introspection, constraints).
3. WHEN `examples/*.phdl` and `examples/*.py` host scripts are migrated
   THEN `tests/run_examples.rs` (the sole "every example elaborates/runs"
   gate) SHALL continue to pass unmodified in its own logic, only its
   input corpus changes extension.
4. WHEN migration of a given file is done THEN the `.phdl` original SHALL
   be deleted in the same commit — no `.phdl` file survives past its own
   migration task (avoids a straggling dual-frontend period).

**Independent Test**: `cargo test --workspace` plus `tests/run_examples.rs`
green with zero `.phdl` files remaining in the tree.

---

### P2: `(* attribute *)`-carried Piperine extensions (superset surface)

**User Story**: As a plugin/device author, I want `@device`/`@port`-style
schemas, capability declarations, and introspection metadata expressed as
VAMS attribute instances on modules/ports/items, so that Piperine's
declared-language-surface guarantee (MD-24) and plugin system survive the
pivot without becoming second-class or bolted-on.

**Why P2**: Required for `piperine-plugin`/`piperine-plugin-macros` to keep
working at all post-pivot; sequenced after P1 (core language must work
first) but before P3 (full digital/VPI completeness isn't needed for
plugins to function).

**Acceptance Criteria**:

1. WHEN a module declaration carries `(* device = "plugin_name", type =
   "..." *)` (or the equivalent attribute-instance form for the existing
   `extern attribute device {...}` schema) THEN the elaborator SHALL bind
   it exactly as PHDL's `@device` attribute does today, with no schema
   authority moved into a per-plugin stub (MD-21/D2 stays enforced).
2. WHEN an attribute instance targets a construct Piperine's own schema
   registry hasn't declared THEN name lookup SHALL fail loud
   (`ElabErrorKind::Other`-equivalent), never silently accept an unknown
   attribute — MD-24's "no silent fallback" rule ports verbatim.
3. WHEN a capability/constraint block equivalent is expressed via
   attribute instances on a module or item THEN it SHALL reach
   `piperine_codegen` with the same semantics `ConstraintBlock` (PHDL
   `pom/constraint.rs`) provides today.

**Independent Test**: A plugin device module written in VAMS with
attribute-carried `@device`/`@port` schema compiles and loads through
`piperine-plugin` exactly as its PHDL predecessor did.

---

### P2: General system tasks/functions, compiler directives, discipline resolution

**User Story**: As a device/testbench author, I want the LRM's general
(non-VPI) system tasks and functions (ch. 9 — `$display`-family,
`$bound_step`, `$abstime`, `$temperature`, analog/digital conversion
functions, etc.), compiler directives (ch. 10 — `` `define``, `` `include``,
`` `ifdef``/`` `ifndef``/`` `else``/`` `endif``, `` `timescale``, etc.), and
discipline-resolution rules for undeclared nets (Annex F) all working, so
that real-world `.vams` source — not just the P1 vertical slice's
hand-picked subset — compiles.

**Why P2**: These are broadly used across ordinary `.vams` source (not an
edge feature like UDPs or VPI); needed before the migration story (P2:
"Standard library and example corpus") can succeed on real files, so it's
sequenced alongside/just before that story.

**Acceptance Criteria**:

1. WHEN `.vams` source calls a ch. 9 general system task/function (not
   VPI-associated) THEN it SHALL execute with LRM-defined semantics —
   PHDL's own `extern task` mechanism (`headers/tasks.phdl`) is direct
   precedent for how these bind to native implementations.
2. WHEN `.vams` source uses a ch. 10 compiler directive THEN preprocessing
   SHALL apply before parsing proper, matching LRM-defined text
   substitution/conditional-inclusion semantics, including
   file-not-found on `` `include`` failing loud (per the fail-loud
   philosophy, VAMS-05).
3. WHEN a net's discipline is undeclared and ambiguous THEN the compiler
   SHALL apply one of Annex F's two resolution methods, chosen and
   documented explicitly (not left to implementation-defined behavior per
   compiler instance) — this closes the "Edge Cases" item flagged in the
   first draft rather than leaving it open.

**Independent Test**: A `.vams` file using `` `define``/`` `ifdef``,
a general system task, and at least one undeclared-discipline net compiles
and elaborates correctly.

---

### P3: Full Verilog-1364 digital completeness

**User Story**: As a mixed-signal designer, I want the full digital
subset LRM 2.4 inherits from IEEE 1364 (all gate primitives, UDPs, complete
event-scheduler semantics including all edge/level-sensitivity
combinations) so that arbitrary existing Verilog digital testbenches and
gate-level netlists work unmodified, not just the subset today's digital
kernel already covers.

**Why P3**: The P1 slice already proves mixed-signal works end-to-end with
today's digital-kernel subset; full 1364 completeness (UDP tables, every
gate-primitive strength/delay combination) is real but large scope that
doesn't block the pivot's core value and is explicitly sequenced last per
the phasing assumption above.

**Acceptance Criteria**:

1. WHEN a module instantiates any IEEE-1364 gate primitive (`and`, `nand`,
   `buf`, `bufif0/1`, `not`, `notif0/1`, `or`, `nor`, `xor`, `xnor`, pass
   switches, etc.) THEN it SHALL simulate with correct truth-table/strength
   semantics.
2. WHEN a module defines a `primitive ... endprimitive` (UDP, combinational
   or sequential) THEN the digital kernel SHALL evaluate it against its
   state table exactly as LRM/1364 defines.
3. WHEN digital events interact with analog contributions in the same
   timestep (mixed-signal scheduling, LRM ch. 7–8) THEN ordering SHALL
   match the LRM's defined simulation cycle, not an approximation of it.

**Independent Test**: A gate-level digital testbench using UDPs and
multiple primitive strengths, sourced from an existing public
Verilog-1364 test corpus, simulates correctly.

---

### P3: VPI-driven system tasks routed through the native plugin ABI

**User Story**: As a model author relying on VPI-callable system
tasks/functions in existing Verilog-AMS source, I want those tasks to
parse and execute (via Piperine's own plugin ABI) so that source using them
isn't rejected outright, even though Piperine never hosts literal VPI `.so`
PLI applications.

**Why P3**: Real LRM conformance requirement (ch. 11–12's language surface)
but lowest-priority — no current Piperine consumer needs this and it's
explicitly bounded (see Out of Scope) to language-level parsing +
native-ABI routing, not full VPI hosting.

**Acceptance Criteria**:

1. WHEN `.vams` source calls any VPI-associated system task/function THEN
   it SHALL parse successfully (LRM Annex B keyword/task coverage) rather
   than failing as an unknown identifier — full ch. 11 language-surface
   coverage, not a hand-picked subset.
2. WHEN such a task executes THEN it SHALL route through Piperine's native
   plugin ABI (`piperine-plugin`), never require a `dlopen`'d IEEE-VPI-ABI
   shared object; every ch. 12 routine category (object/handle traversal,
   callback registration, value get/put, etc.) SHALL have a native-ABI
   equivalent, even where the equivalent is "not applicable, returns a
   defined not-supported result" for a routine class with no native
   analogue (documented per-routine, not silently absent).

**Independent Test**: A `.vams` source file using a documented VPI system
task compiles and its task call reaches a native Rust handler.

---

### P3: SPICE compatibility (Annex E)

**User Story**: As an author importing or cross-checking against SPICE
decks, I want Annex E's SPICE-compatibility rules (its specific
`.subckt`/behavioral-source correspondence rules, not just Piperine's
existing ad hoc SPICE→PHDL-equivalent import path) implemented for VAMS, so
that LRM conformance is complete and existing SPICE-adjacent tooling
(`tests/ngspice_*.rs` oracles) has a documented, LRM-anchored mapping
instead of an informal one.

**Why P3**: Real LRM content (normative annex), but Piperine already has a
working SPICE-import path (MD-39) that doesn't depend on this annex —
sequenced last because nothing else in this feature blocks on it.

**Acceptance Criteria**:

1. WHEN Annex E defines a SPICE-construct-to-Verilog-AMS-construct mapping
   THEN Piperine's SPICE-facing tooling SHALL follow that mapping
   explicitly where it exists, documented against the annex clause it
   implements.

**Independent Test**: A SPICE-derived construct covered by Annex E maps to
the annex-defined VAMS construct, cited by clause number in code comments
or docs.

---

## Edge Cases

- WHEN a `.vams` file references LRM Annex C's "analog language subset"
  markers (constructs restricted in analog-only contexts) THEN the
  elaborator SHALL enforce the same restriction, not silently allow
  digital constructs inside an analog-only context.
- WHEN discipline resolution for an undeclared net is ambiguous between
  the LRM's two resolution methods (Annex F) THEN the compiler SHALL pick
  one method explicitly and document the choice (P2/P3, not blocking P1
  since P1's vertical-slice example declares disciplines explicitly).
- WHEN a `genvar`/generate-loop produces a name collision with an existing
  monomorphized instance name (PHDL precedent: `urc__5`-style naming) THEN
  elaboration SHALL fail loud with a clear collision diagnostic, not
  silently rename.
- WHEN a migrated `.va` device model's simulated output no longer matches
  its recorded ngspice oracle after translation (P2) THEN the migration
  task SHALL NOT be considered done until the discrepancy is root-caused
  and fixed — tolerance contracts are never loosened to paper over a
  translation bug.
- WHEN an attribute instance (P2, superset surface) is attached to a
  construct VAMS itself doesn't permit attributes on (per LRM Syntax
  2-5..2-10) THEN it SHALL be a parse error, not a silently-ignored
  attribute.

---

## Requirement Traceability

Every requirement carries its own validation test — no `VAMS-NN` closes on
"implemented", only on "implemented and its listed test passes". Tests are
spec-anchored (assert LRM-defined outcomes) per the skill's execution
contract, written in the Tasks/Execute phase, named here so none is
forgotten.

| Requirement ID | Story | Validation Test | Phase | Status |
|---|---|---|---|---|
| VAMS-01 | P1: nature/discipline elaboration | `piperine-lang` test: parse+elaborate a `nature`+`discipline` pair, assert POM `Nature`/discipline fields match LRM §3.6 semantics (units, access fn, potential/flow) | Design | Pending |
| VAMS-02 | P1: analog block + contribution statement | `piperine-lang`/`piperine-codegen` test: `V(p,n) <+ expr` reaches `resolve::pom::LoweredBody` with the same shape as the PHDL `<+` equivalent | Design | Pending |
| VAMS-03 | P1: digital always block | `piperine-codegen` test: `always @(...)` elaborates to `BehaviorKind::Digital`, exercised through the digital kernel | Design | Pending |
| VAMS-04 | P1: hierarchical instantiation, POM navigability | `piperine-lang` test: multi-level instantiation with named+ordered port connections; assert instance→submodule→sub-instance tree is walkable (POM-navigability regression test, same shape as existing PHDL ones) | Design | Pending |
| VAMS-05 | P1: fail-loud diagnostics | `piperine-lang` test: inject a lexical/syntax/elaboration error, assert a source-location-anchored `Err`, never a silent fallback | Design | Pending |
| VAMS-06 | P1: end-to-end RC+counter parity vs PHDL baseline | root `tests/`: RC+counter `.vams` example simulated via `piperine-api`, diffed against the existing PHDL fixture's recorded result within `ToleranceContract` | Design | Pending |
| VAMS-07 | P1: lexical (ch. 2) + expressions/math/time-domain fns (ch. 4) | `piperine-lang` test: every literal form (integer w/ SI suffix, real, string, escaped identifier) and every ch. 4 operator/math/time-domain function parses+evaluates against a known expected value | Design | Pending |
| VAMS-08 | P2: SPICE-derived device headers migrated to `.va` | root `tests/ngspice_*.rs`: unchanged tolerance/oracle, run against the migrated `.va` sources | - | Pending |
| VAMS-09 | P2: stdlib headers (prelude/constants/math/operators/tasks, Annex D) migrated | `piperine-lang` test: each migrated header parses+elaborates standalone; values cross-checked against their PHDL predecessor's constants/behavior | - | Pending |
| VAMS-10 | P2: examples corpus migrated, `run_examples.rs` green | root `tests/run_examples.rs`, unmodified in logic, run against the migrated corpus | - | Pending |
| VAMS-11 | P2: `.phdl` deleted per-file on migration | `suite_hygiene.rs`-style repo scan: assert zero `.phdl`/`.ppr` files remain once VAMS-11 is marked done | - | Pending |
| VAMS-12 | P2: `@device`/`@port` schema via attribute instances | `piperine-plugin` test: a VAMS module with `(* device = ... *)` loads through the plugin host identically to its PHDL predecessor | - | Pending |
| VAMS-13 | P2: unknown-attribute fail-loud (MD-24 parity) | `piperine-lang` test: an attribute instance targeting an undeclared schema fails elaboration, never silently accepted | - | Pending |
| VAMS-14 | P2: constraint-block semantics via attribute instances | `piperine-codegen` test: attribute-carried constraint reaches the same `ConstraintBlock`-equivalent semantics as PHDL's | - | Pending |
| VAMS-15 | P2: general (non-VPI) system tasks/functions (ch. 9) | `piperine-lang` test: each ported ch. 9 task/function executes with LRM-defined behavior | - | Pending |
| VAMS-16 | P2: compiler directives (ch. 10) | `piperine-lang` test: `` `define``/`` `include``/`` `ifdef`` family, incl. `` `include``-not-found failing loud | - | Pending |
| VAMS-17 | P2: discipline resolution for undeclared nets (Annex F) | `piperine-lang` test: undeclared-net discipline resolves per the documented chosen Annex F method, asserted against LRM's worked examples | - | Pending |
| VAMS-18 | P3: full IEEE-1364 gate primitive set | `piperine-codegen` test: each primitive's truth table/strength behavior asserted | - | Pending |
| VAMS-19 | P3: UDP support | `piperine-codegen` test: a combinational and a sequential UDP evaluated against their state tables | - | Pending |
| VAMS-20 | P3: full mixed-signal scheduling (ch. 7–8) | root `tests/`: mixed analog/digital event ordering test matching the LRM's defined simulation cycle | - | Pending |
| VAMS-21 | P3: VPI system tasks + routines, full ch. 11–12 surface via native ABI | `piperine-plugin` test: representative VPI-associated task/routine per category reaches a native handler; unsupported-class routines return the documented not-supported result | - | Pending |
| VAMS-22 | P3: SPICE compatibility (Annex E) | root `tests/ngspice_*.rs`-adjacent test: an Annex-E-mapped SPICE construct produces the annex-defined VAMS construct | - | Pending |
| VAMS-23 | P1–P3 (ongoing gate): Annex A formal grammar conformance | `piperine-lang` conformance test: every Annex-A production exercised by at least one parser test (tracked as a coverage gate, re-run every phase) | Design | Pending |
| VAMS-24 | P1–P3 (ongoing gate): Annex B keyword-list conformance | `piperine-lang` test: every Annex-B keyword is reserved and rejected as an identifier | Design | Pending |
| VAMS-25 | P1+ (extended per phase): Annex C analog-subset restriction enforcement | `piperine-lang` test: a digital-only construct inside an analog-only context fails elaboration citing Annex C | Design | Pending |
| VAMS-26 | P3: Annex G/H ported to project docs as reference (non-normative) | Docs-only — validated by review, not a test; tracked so it isn't silently skipped | - | Pending |

**ID format:** `VAMS-NN`

**Status values:** Pending → In Design → In Tasks → Implementing → Verified

**Coverage:** 26 total, 0 mapped to tasks yet (Tasks phase not started), 26
unmapped ⚠️. Every LRM chapter (1–12) and annex (A–H) has at least one
requirement ID — see LRM Coverage Map below for the audit. Every requirement
has a named validation test except VAMS-26 (docs-only, reviewed not tested,
explicitly marked so).

---

## LRM Coverage Map

Every clause of `VAMS-LRM-2-4.md`'s own table of contents, mapped to the
requirement(s) that cover it — this is the literal answer to "bata 100% dos
pontos do LRM": nothing in the LRM's own chapter/annex list is unmapped.

| LRM Chapter/Annex | Title | Requirement(s) | Phase |
|---|---|---|---|
| 1 | Verilog-AMS introduction (concepts, Kirchhoff's laws, systems) | VAMS-01, VAMS-02 | P1 |
| 2 | Lexical conventions | VAMS-07, VAMS-24 | P1 |
| 3 | Data types (incl. nature/discipline/net/parameter/genvar) | VAMS-01 | P1 |
| 4 | Expressions | VAMS-07 | P1 |
| 5 | Analog behavior | VAMS-02 | P1 |
| 6 | Hierarchical structures | VAMS-04 | P1 |
| 7 | Mixed signal | VAMS-03, VAMS-20 | P1 (subset) / P3 (full) |
| 8 | Scheduling semantics | VAMS-03, VAMS-20 | P1 (subset) / P3 (full) |
| 9 | System tasks and functions | VAMS-15 (general), VAMS-21 (VPI-associated) | P2 / P3 |
| 10 | Compiler directives | VAMS-16 | P2 |
| 11 | Using VPI routines | VAMS-21 | P3 |
| 12 | VPI routine definitions | VAMS-21 | P3 |
| A | Formal syntax definition (BNF) | VAMS-23 | Ongoing, P1–P3 |
| B | List of keywords | VAMS-24 | Ongoing, P1–P3 |
| C | Analog language subset | VAMS-25 | Ongoing, P1–P3 |
| D | Standard definitions | VAMS-01, VAMS-09 | P1 / P2 |
| E | SPICE compatibility | VAMS-22 | P3 |
| F | Discipline resolution methods | VAMS-17 | P2 |
| G | Change history (informative) | VAMS-26 | P3, docs-only |
| H | Glossary (informative) | VAMS-26 | P3, docs-only |

---

## Success Criteria

- [ ] A `.vams` source module (analog+digital) compiles and simulates on
      the unmodified native solver, matching an existing PHDL fixture's
      recorded numeric result within existing tolerance contracts (P1).
- [ ] Zero `.phdl` files remain in the repository; `cargo test --workspace`
      and `tests/run_examples.rs` are green against the fully migrated VAMS
      corpus (P2).
- [ ] `piperine-plugin` device/port loading works unmodified in spirit
      (attribute-carried, fail-loud on unknown schema) against VAMS source
      (P2).
- [ ] A gate-level digital testbench using UDPs and multiple primitive
      strengths simulates correctly (P3).
- [ ] Every LRM chapter (1–12) and normative annex (A–F) has at least one
      passing acceptance criterion exercising it; Annex G/H are ported to
      docs (VAMS-26). The LRM Coverage Map above is the audit trail — no
      chapter/annex is unmapped (P3, closing criterion for the whole
      feature).
- [ ] `CLAUDE.md`'s pipeline diagram and crate-responsibility table for
      `piperine-lang` are updated to describe VAMS, not PHDL, once P1 lands
      (documentation debt tracked as a task, not left implicit).
