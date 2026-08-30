# Implementation ↔ Specification Gap Closure (wave 3) — Specification

*Directory name kept as `lrm-conformance-wave3` for continuity with waves 1–2;
the scope is wider than the name, and §0 says why.*

**Feature dir**: `.specs/features/lrm-conformance-wave3/`
**Status**: Drafted 2026-08-27 (not yet approved)
**Predecessors**: `lrm-conformance-closure` (wave 1, 54 LRMC ids),
`lrm-conformance-wave2` (T1–T72, `SPEC_DEVIATION.md` regenerated at T70)
**Evidence base**: a full `cargo test --workspace --no-fail-fast` run plus a
throwaway probe target that elaborated/compiled one minimal fixture per
suspect LRM clause (probe output archived in this feature's `design.md`
when it is written; the fixtures are reproduced inline in the ACs below).

---


## §0 Two axes, and the superset rule

**Piperine is a superset of Verilog-AMS, not an equal.** A gap audit that only
walks the LRM is therefore half an audit, and it is also *wrong* wherever it
treats a deliberate divergence as a defect. This feature runs two axes:

| Axis | Question | Authority | Gate |
|---|---|---|---|
| **A — LRM ↔ impl** | Does Piperine implement what the LRM requires? | `VAMS-LRM-2-4.md` | `tests/lrm_section_coverage.rs` (612 headings, each Covered / Informative / OutOfScope) |
| **B — Piperine spec ↔ impl** | Does Piperine implement what *its own* specification claims, including everything the LRM never mentions? | `docs/spec/` (Parts I–X + appendices), rewritten 2026-08-27 against the source | none today — **building one is part of this feature** (PSW3-10) |

**The superset rule, stated normatively so neither axis can be misread:**

1. An LRM clause Piperine deliberately does **not** implement is an
   `OutOfScope` row with a written reason, **not** a defect. The reason must
   name what replaces it (`config` → `Piperine.toml`; VPI C ABI → the native
   plugin ABI; the SPICE netlist profile → header-based `` `include`` models;
   Annex C's subset *restrictions* → Piperine compiles the superset).
2. An LRM clause Piperine implements **differently on purpose** is a
   **deviation** with a source-level reason and a pinning test, not a gap
   (today's list: `docs/spec/part_iii_conformance.md` §4.1).
3. A construct Piperine implements that the LRM never mentions is an
   **extension**. An extension is not exempt from proof: it needs the same
   thing an LRM clause needs — a live test, and a line in the spec. Axis B
   exists because extensions currently have neither a coverage gate nor a
   completeness audit.
4. Neither axis may be closed by weakening the other. "Covered by an LRM test"
   never justifies dropping an extension, and "it's an extension" never
   justifies leaving an LRM clause unaccounted for.

**What lives on axis B** (the surface the LRM says nothing about): the
`constraint begin` block and its margins channel, the `tol` random-variable
catalogue, `const` at file scope, `///` doc comments, the `@device`/`@port`/
`rfport` attribute schemas, the UIC-seed idiom, the whole host API (analyses,
sweeps, sampling, Monte Carlo, the optimize/center/high-sigma driver, aging,
coverage, the host device library and sequencer), the plugin system, the POM
and its selector, `piperine-ir` (IR core, catalog, printer/parser,
interpreter), and the CLI.

## Problem Statement

Wave 2 closed the 2026-08-19 audit's §1–§5 findings, but its own closure
gate never went green: `tests/lrm_section_coverage.rs::covered_and_excluded_partition_the_lrm`
**fails on `main`-of-this-branch with 105 of 612 LRM-2-4 sections neither
`Covered` nor excluded**. Wave 2's tasks.md predicted "T69 closes the matrix
to zero gaps"; T69's own commit message says it closed 364 of 469 — the
remaining 105 were never re-planned.

Direct probing of those 105 shows they are not all bookkeeping. **Whole
grammar families the LRM treats as basic are missing from the frontend** —
most importantly vectors: `electrical [0:3] bus`, `input [0:3] p`,
`reg [3:0] q`, `wire [3:0] w`, a part-select `b[7:4]`, and a memory
`reg [3:0] mem [0:7]` are all *parse errors* today ("expected an
identifier, found '['"), and even the post-declarator form
(`electrical bus[0:3]`) fails to lower a literal index
(`LOWER-ERR unresolved name`). `README.md` advertises a `generate`-built
resistor ladder over `electrical tap [n-1:0]` that cannot compile.

Two constructs additionally **parse and vanish**, which violates
CLAUDE.md's fail-loud contract the same way wave 2's §1 findings did:
a `specify … endspecify` block (its body is stored as an opaque, unread
`String`) and a `config … endconfig` declaration (`ConfigDecl { body:
String::new() }`).

## Goals

- [ ] **G1** — `tests/lrm_section_coverage.rs::covered_and_excluded_partition_the_lrm`
      is **green**: every one of the 612 LRM-2-4 headings is `Covered` by a
      live test, `Informative` per the LRM's own front matter, or
      `OutOfScope` against a row in **this** spec's Out of Scope table.
      (The gate currently reads only wave 2's `spec.md`; extending it to
      read this one is part of the work — see LRMW3-30.)
- [ ] **G2** — Every gap in the "Probed gaps" table below is either
      implemented with an LRM-clause-derived test, or converted into a
      **loud** failure with the clause cited. No probed gap may remain a
      silent parse-and-drop.
- [ ] **G3** — Vector (bus) support is real end to end: declaration
      (net/port/reg/wire, pre- and post-range), indexed access in analog and
      digital contexts, part-selects, and memories.
- [ ] **G4** — The nine items still open in `SPEC_DEVIATION.md` §6 are
      re-decided: implemented, or re-stated as an OutOfScope row here with a
      clause-linked reason (disclosure alone is not conformance — wave 2's
      own G2 rule, carried forward).
- [ ] **G5** — Zero regression, measured target-by-target against a captured
      baseline (the MD-47 method wave 2 used), never an aggregate count.
- [ ] **G6** — Every fix carries a test derived from the LRM clause and its
      own worked example where one exists, never from reading the fix.
- [ ] **G7 (axis B)** — Every Piperine-only surface named in §0 carries a
      **verdict** in a mechanical gate the same way an LRM section does:
      implemented and tested, or declared unimplemented with a reason. The
      gate reads `docs/spec/` itself, so a spec edit that adds a claim without
      a test fails it (PSW3-10).
- [ ] **G8 (axis B)** — Every gap in the "Axis B — Piperine spec vs impl"
      table below is closed, or restated in `docs/spec/` as a declared
      limitation with its own pinning test. A surface that is documented as
      working and is not is the same defect class as a silent drop.

## Out of Scope

| Item | Reason |
|---|---|
| Hosting third-party C-ABI VPI `.so`/`.dll` PLI applications | Unchanged project decision (waves 1 and 2). VPI's *language surface* stays in scope through the native plugin ABI (`crates/piperine-plugin/src/vpi.rs`). |
| A raw SPICE netlist (`.subckt`/`.model`) reader | Unchanged from waves 1–2. Piperine's SPICE path is header-based. |
| Annex G (change history), Annex H (glossary) | The LRM's own front matter tags them "(informative)". |
| The literal `vpiHandle`/C-struct VPI object model (`vpi_handle_multi`, `t_vpi_analog_systf_data`, `vlog_startup_routines`, `vpi_sim_control`) | These sections define a C ABI, not language semantics. Piperine's equivalent is the native plugin ABI; each such section maps to a `VpiCategory` entry that is `Native`, `Partial`, or a typed `NotSupported`, never a silent absence. |
| Library/configuration management (`config`/`design`/`liblist`/`use`, Annex A.1.5) | Source-set selection is `Piperine.toml` + the `SourceMap`'s job (`piperine-project`), not the HDL's. In scope here only as a **loud** rejection replacing today's silent parse-and-drop. |
| Verilog-A *subset enforcement* (Annex C's restriction list) as a separate compilation mode | Piperine implements the Verilog-AMS superset; a `.va` file is compiled with the same front end. Annex C's per-clause deltas are covered by the corresponding chapter tests, not by a second, restricted parser. |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
|---|---|---|---|
| Vector support covers **packed 1-D** ranges only (`[msb:lsb]`) plus 1-D memories (`reg [w] mem [d]`) | No multi-dimensional packed arrays, no SystemVerilog `logic [3:0][7:0]` | LRM-2-4 / IEEE 1364-2005 itself defines only 1-D packed vectors and (up to N-D) memories; 1-D memories cover Annex A.2.2.1's `variable_type` and §9.8's PLA planes | n |
| Analog vector nets expand at elaboration into one scalar net per bit | Each bit is an independent MNA node, named `bus[i]` | Matches the POM navigability rule (authored structure preserved, expansion is derived) and the existing `Ladder`-style genvar idiom | n |
| `specify` blocks lower to the existing `DelayTable`/`TimingCheckTable` (`crates/piperine-solver/src/digital/delays.rs`) | Path delays become per-net delay records; timing checks become `TimingCheckLimit`s | Those tables already exist and are already exercised by unit tests; only the front-end path is missing | n |
| A `specify`/`config` construct that cannot be lowered fails loud, citing the clause | Never a silent opaque span | CLAUDE.md's fail-loud contract; identical to wave 2's §1 class | n |
| §4.5.15/§5.8.4 restrictions are enforced at **compile time** | An analog operator under a non-constant conditional is a `CodegenError`, not a runtime warning | The LRM states them as illegality, and the restriction exists to protect operator state | n |
| Annex C sections are `Covered` by the corresponding chapter tests, cross-declared | e.g. C.4 declares the same ids the chapter-3 data-type tests prove | Annex C is normative but definitionally a delta list over clauses 1–12 | n |
| §1.3.4/§1.3.4.1/§1.3.4.2/§1.3.5 (signal-flow systems) are `Covered` by real signal-flow fixtures | A potential-only and a flow-only discipline, each driving a working module | These clauses carry normative rules ("current contributions to that node are not legal"), so they need tests, not an Informative tag | n |

**Open questions:** none — all resolved or logged above.

---

## Probed gaps (evidence)

Every row below was produced by elaborating/compiling a minimal fixture
against this branch's HEAD. `ELAB-ERR` = parse/elaborate failure;
`LOWER-ERR` = elaborates but does not lower; `ACCEPTED` = compiles today
although the LRM makes it illegal; `SILENT` = parses and is discarded.

| LRM | Construct | Observed today |
|---|---|---|
| §3.6.3, A.2.1.2, A.2.2.1 | `electrical [0:3] bus;` / `input [0:3] p;` / `reg [3:0] q;` / `wire [3:0] w;` | ELAB-ERR `expected an identifier, found '['` |
| §4.2.1 | part-select `b[7:4]` | ELAB-ERR `expected an identifier, found '['` |
| §9.8, A.2.2.1 | memory `reg [3:0] mem [0:7];` | ELAB-ERR `expected an identifier, found '['` |
| §5.5.2 | `electrical bus[0:3];` + `V(bus[0], g)` | LOWER-ERR `2 unresolved name(s)` |
| §5.5.2 | `electrical bus[0:3];` + genvar index in an analog `for` | ELAB-ERR `const eval error in 'port connection index': undefined name: i` |
| §2.9.2, §3.6.3.1 | `(* desc="drain terminal" *) electrical d;` | ELAB-ERR `unknown attribute schema 'desc'` |
| §3.6.2.5 | `flow.abstol = 10u;` inside a `discipline` | ELAB-ERR `expected an identifier, found '.'` |
| §3.6.2.6 | `nature ttl_net_curr : ttl.flow;` | ELAB-ERR `expected an identifier, found '.'` |
| §3.6.2.7 | user-defined attribute in a `discipline` | ELAB-ERR `expected 'domain', 'potential', or 'flow'` |
| §3.10 | out-of-module net discipline declaration (`electrical u1.n1;`) | ELAB-ERR `expected ';', found '.'` |
| §3.12.1 | port branch `branch (<p>) probe_p;` | ELAB-ERR `expected an identifier, found '<'` |
| §5.5.3 | `ddt(V(a,b), a.potential.abstol)` | LOWER-ERR `1 unresolved name(s)` |
| §5.6.8/§5.6.8.1/§5.6.8.2 | `V(drv.x) <+ 1.8;` (hierarchical contribution) | ELAB-ERR at the instantiation; hierarchical *probe* in `$strobe` also LOWER-ERRs |
| §6.4.3 | paramset output variable (`(*desc=…*) real ft;` in a `paramset`) | ELAB-ERR `expected 'parameter', found 'real'` |
| A.2.2.2 | drive strengths `assign (strong1, weak0) w = r;` | ELAB-ERR `expected ')', found ','` |
| A.7.2–A.7.5.3 | `specify (a => y) = (1.0,1.0); endspecify`, `$setup(...)` | SILENT — parses; body kept as an unread `String` |
| A.1.5 | `config cfg; design lib.top; … endconfig` | SILENT — parses; `ConfigDecl.body` is `String::new()` |
| §3.4.4 | `specparam tr = 1.0;` | SILENT — no-op arm in `elab_module`'s `ModuleItem` match |
| §9 | `task` declared in a module | SILENT — same no-op arm; a task call has no lowering path |
| §4.5.15, §5.8.4 | `ddt()` under a non-constant `if` | ACCEPTED — the LRM makes it illegal |
| §7.3.2.1 | `V(a,g) <+ 1.0/0.0;` (infinity contributed to a branch) | ACCEPTED — the LRM makes it illegal |
| §9.8 | `$async$and$array(...)` | ELAB-ERR (blocked behind memories) |

Sections that probe **clean** today and therefore need only a coverage
claim (a real test declaring `//! LRM-COVERS:`), not new behavior:
§4.2.3 (short-circuit), §4.6.4.5/§4.6.4.6 (diode/correlated noise),
§5.4.4 (unassigned sources), §5.6.6 (implicit contributions),
§9.4.7 (`%r`), §9.7.1/§9.7.2 (`$finish`/`$stop`), §9.9 (stochastic),
§9.14 (`$clog2` + math aliases), §9.22.x/§9.23.x (driver access, landed
in wave-2 T54), §7.4.2 (`wreal` port connections).

---

## Axis B — Piperine spec vs impl (probed gaps)

Evidence read from the source this session. None of these is an LRM matter;
all of them are Piperine's own surface.

| # | Surface | Claim | Reality | Evidence |
|---|---|---|---|---|
| B1 | POM selector | `Design::select` documents axes `inst`/`net`/`port`/`param`/`behavior`/`attr`/`constraint`/`driver`/`load`/`parent`/`ancestor` | `constraint::` dispatches to an **empty arm**; `driver::`/`load::`/`parent::`/`ancestor::` share one arm that matches nothing. All five parse and return an empty selection — the only place in the POM surface where "nothing happened" is not loud | `crates/piperine-lang/src/pom/selector/eval.rs` (`Axis::Constraint => {}`) |
| B2 | POM node kinds | `pom::node::Kind` is the node taxonomy | `Enum`, `Bundle`, `Capability` are retired-front-end variants no Verilog-AMS source can produce and no traversal returns | `crates/piperine-lang/src/pom/node.rs` |
| B3 | POM values | `Value` is the POM value type | `Record`/`List`/`Tuple`/`EnumVariant` are unreachable from Verilog-AMS | `crates/piperine-lang/src/value.rs` |
| B4 | Elaboration errors | `ElabErrorKind` is the elaboration error catalog | Bundle-typed-parameter, match-exhaustiveness, `mod`-body and package-visibility variants are unreachable | `crates/piperine-lang/src/pom/error.rs` |
| B5 | Host parity (MD-22) | Python and Rust are one API | `Driver`/`Policy` (optimize/center/high-sigma), `SamplePlan`, `monte_carlo`, `CoverageDatabase` are **Rust-only**; `Waveform` measurements/transforms and `ComplexWaveform` margins are Rust-only; `Module.constraints` has no Python binding | `crates/piperine-python/python/piperine/__init__.py` (`__all__`), `docs/spec/appendix_c_host_surface.md` §3 |
| B6 | `piperine-ir` purity/ownership | One implementation per runtime family, called by both codegen and host devices | True in code (codegen calls `piperine_ir::ops::Operator`, `::events::EventDetector`, `::limit`), but `ops.rs`'s own module doc still says "Codegen keeps its own copy until `codegen-simplification` delegates" — a stale claim in the crate that owns the contract | `crates/piperine-ir/src/ops.rs` |
| B7 | `` `include`` resolution | Namespace → root → literal path | An **undocumented-until-now** legacy-alias fallback (`discipline.h`/`disciplines.h`/`constants.h` → the Annex D files) exists in the working tree with no test | `crates/piperine-lang/src/vams_include.rs` |
| B8 | Coverage gate infrastructure | The gate is the source of truth for conformance | `tests/lrm_section_coverage.rs`'s own header still names `piperine-signal`, and the failure message still says "expected until wave-2's closure work lands" — a red gate that describes itself as expected-red | `tests/lrm_section_coverage.rs` |
| B9 | Extensions have no gate | §0 rule 3 | No test enumerates the sanctioned extensions (`constraint`, `tol`, `const`, `///`, `@device`/`@port`/`rfport`, UIC seed, `$unconnected`, `'{`/`<<<`/`>>>`, the parenthesized `paramset`) and asserts each has a live test — the wave-2 backfill explicitly *dropped* those targets' coverage claims because they prove an extension, not an LRM clause | `.specs/features/lrm-conformance-wave2/coverage_exclusions.toml` (T7 note) |
| B10 | Examples as documentation | `examples/` is the worked corpus CI proves | `14_bundle_model_card.vams` still carries a retired-front-end name; the corpus has no example for the extension surface (`constraint`/`tol`/optimizer) beyond `live_optimize` | `examples/` |


## User Stories

### P1: Vectors are a real data type ⭐ MVP

**User Story**: As a device/testbench author, I want buses — declared,
sliced, indexed, and instantiated — so that an N-bit design is one module,
not N copies.

**Why P1**: It is the largest missing family, it is basic LRM (clause 3 and
IEEE 1364-2005 alike), and `README.md` already advertises it.

**Acceptance Criteria**:
1. WHEN a module declares `electrical [0:3] bus;` (pre-range) or
   `electrical bus[0:3];` (post-range) THEN elaboration SHALL produce four
   scalar analog nets addressable as `bus[0]`..`bus[3]`, and the authored
   declaration SHALL remain a single vector net in the POM (MD-25).
2. WHEN an analog block contributes through a literal index
   (`I(bus[0], g) <+ V(bus[0], g)/1e3;`) THEN the compiled kernel SHALL
   stamp that bit's node and no other.
3. WHEN an analog `for` loop over a `genvar` indexes a vector net THEN each
   unrolled iteration SHALL resolve to its own bit (LRM §5.5.2's own N-bit
   DAC example is the fixture).
4. WHEN a module declares `reg [3:0] q;` or `wire [3:0] w;` THEN the digital
   kernel SHALL carry four bits, and a whole-vector assignment
   (`q <= 4'b0101;`) SHALL set them per IEEE 1364-2005 bit order.
5. WHEN an expression uses a part-select (`b[7:4]`) or a bit-select on a
   vector variable THEN it SHALL evaluate to the selected bits.
6. WHEN a module declares a memory (`reg [3:0] mem [0:7];`) THEN indexed
   read/write of a word SHALL work in a digital procedural block.
7. WHEN a port is declared with a range (`input [0:width-1] in;`) THEN
   instantiating it with a vector net SHALL connect bit-for-bit, and a
   width mismatch SHALL fail loud citing §6.5.7.1.

**Independent Test**: LRM §5.5.2's N-bit `dac` example compiles and its
transient output matches the analytic weighted sum.

---

### P2 (axis B): Piperine's own surface is proven, not asserted

**User Story**: As a user reading `docs/spec/`, I want every Piperine-only
capability the specification claims to be backed by a live test, so the
superset is as trustworthy as the standard part.

**Why P2**: The superset is the reason to choose Piperine. It currently has no
coverage gate at all, so a regression in the extension surface is invisible in
a way an LRM regression is not.

**Acceptance Criteria**:
1. WHEN the axis-B gate runs THEN every Piperine-only surface enumerated in §0
   SHALL resolve to `Covered { test }` or `Declared limitation { reason }` —
   no surface without a verdict.
2. WHEN `docs/spec/` claims a Piperine-only capability works THEN a live,
   non-`#[ignore]`d test SHALL exist for it, and the gate SHALL fail if the
   claim is added without one.
3. WHEN a selector axis is named in the grammar THEN it SHALL either return
   real matches or fail loud — an empty selection from an unimplemented axis
   is forbidden (B1).
4. WHEN a host surface exists on one host and not the other THEN the
   asymmetry SHALL be declared in `docs/spec/appendix_c_host_surface.md` **and**
   asserted by the parity test, so an undeclared asymmetry fails (B5).
5. WHEN a public POM/enum variant is unreachable from Verilog-AMS source THEN
   it SHALL be removed or proven unreachable by a test (B2/B3/B4).
6. WHEN an extension is added to the language THEN it SHALL land with a test
   and a `docs/spec/` line in the same change (B9's rule, going forward).

**Independent Test**: the axis-B gate fails when a spec claim's test is
deleted, and passes when it is restored — proven the way wave 2 proved its own
gates could fail.

---

### P2 (axis A): Nothing parses and vanishes

**User Story**: As a model author, I want every construct Piperine accepts
to either do something or say why not, so I never ship a model whose timing
or configuration was silently ignored.

**Why P2**: Fail-loud is a project contract (CLAUDE.md); these are the last
two known violations of it.

**Acceptance Criteria**:
1. WHEN a `specify` block declares a path delay (`(a => y) = (1.0,1.0);`)
   THEN it SHALL populate the module's `PathDelay` records and the delay
   SHALL be observable on the digital output's transition time.
2. WHEN a `specify` block declares a system timing check
   (`$setup(d, posedge clk, 1.0);`) THEN it SHALL populate
   `TimingCheckTable` and a violating stimulus SHALL be reported.
3. WHEN a `specify` construct is accepted by the grammar but cannot be
   lowered THEN elaboration SHALL fail loud naming the construct and the
   Annex A production, never store it as an unread span.
4. WHEN a source file contains a `config` declaration THEN Piperine SHALL
   fail loud citing A.1.5 and pointing at `Piperine.toml`, never accept and
   discard it.
5. WHEN a continuous assignment carries drive strengths
   (`assign (strong1, weak0) w = r;`) THEN the strengths SHALL reach the
   `DriverTable` and participate in Table 7-3 resolution.

**Independent Test**: each construct has a fixture asserting either the
implemented effect or the exact loud error.

---

### P2: Discipline, nature, and attribute surface is complete

**User Story**: As a discipline author, I want the LRM's declaration forms
(attribute override, nature derivation, user attributes, standard
attributes, out-of-module discipline declarations) to work, so a
non-electrical or multi-physics discipline set is expressible.

**Acceptance Criteria**:
1. WHEN a `discipline` overrides a bound nature attribute
   (`flow.abstol = 10u;`) THEN nets of that discipline SHALL solve with the
   overridden tolerance (§3.6.2.5).
2. WHEN a `nature` is derived from a discipline's binding
   (`nature n : ttl.flow;`) THEN it SHALL inherit that nature's attributes
   and MAY override or add attributes (§3.6.2.6).
3. WHEN a `discipline` declares a user-defined attribute THEN it SHALL be
   readable through the POM and SHALL NOT be a parse error (§3.6.2.7).
4. WHEN a declaration carries `desc`, `units`, `op`, or `multiplicity`
   THEN the attribute SHALL be accepted without a plugin schema and SHALL
   surface on the corresponding host descriptor (§2.9.2, §3.2.1, §3.6.3.1).
5. WHEN a net's discipline is declared out-of-module (`electrical u1.n1;`)
   THEN §3.10's precedence order SHALL decide, and a conflict SHALL fail
   loud naming both declarations.
6. WHEN an expression reads a nature attribute through a net
   (`a.potential.abstol`) THEN it SHALL evaluate to that attribute's value
   at elaboration time (§5.5.3).

---

### P2: Branch and contribution completeness

**User Story**: As a device author, I want port branches, hierarchical
contributions, and unnamed hierarchical branches, so a model can probe and
drive exactly what the LRM says it can.

**Acceptance Criteria**:
1. WHEN a module declares `branch (<p>) probe_p;` THEN `I(probe_p)` SHALL
   read the flow into port `p` (§3.12.1, §5.4.3).
2. WHEN an analog block contributes to a hierarchical net
   (`V(top.drv.x) <+ 1.8;`) THEN an unnamed branch SHALL be created in the
   contributing module and the target net SHALL be driven (§5.6.8.1).
3. WHEN an analog block contributes to a hierarchical *named* branch
   (`V(top.drv.br_v) <+ 1.8;`) THEN the contribution SHALL apply, and a
   contribution that would turn it into a switch branch SHALL fail loud
   (§5.6.8.2).
4. WHEN an access function reads a hierarchical net (`V(top.drv.a)`) inside
   `$strobe`/an expression THEN an unnamed branch SHALL be created and read
   (§5.5.4).
5. WHEN a branch receives both an indirect and a direct contribution THEN
   the second SHALL fail loud citing §5.6.7.2.

---

### P2: Illegal analog constructs fail loud

**Acceptance Criteria**:
1. WHEN an analog operator (`ddt`, `idt`, `transition`, `laplace_*`, …)
   appears under a conditional whose controlling expression is not
   elaboration-constant THEN compilation SHALL fail citing §4.5.15.
2. WHEN an analog operator appears inside an event-triggered statement, a
   `repeat`/`while`/non-genvar `for`, or a user-defined function THEN
   compilation SHALL fail citing §4.5.15.
3. WHEN an event control statement (`cross`, `timer`, …) appears under a
   non-constant conditional THEN compilation SHALL fail citing §5.8.4.
4. WHEN a contribution's value is NaN or ±infinity THEN the solver SHALL
   report a typed error naming the branch, never stamp it (§7.3.2.1).

---

### P3: Paramset output variables, PLA tasks, and remaining coverage

**Acceptance Criteria**:
1. WHEN a `paramset` declares a described variable THEN it SHALL be an
   output variable of instances using that paramset, overriding a
   same-named module output variable, and `.module_output_variable` SHALL
   read the underlying module's value (§6.4.3).
2. WHEN a design uses the IEEE 1364-2005 PLA tasks
   (`$async$and$array` and family) THEN they SHALL load the plane from the
   named memory and drive the outputs (§9.8) — or fail loud naming the
   task, if this is re-scoped out.
3. WHEN the coverage gate runs THEN every LRM heading SHALL carry a verdict
   (G1), including Annex C cross-declarations and the ch. 12 VPI category
   mapping.

---

## Edge Cases

- WHEN a vector's range bounds are not elaboration-constant THEN elaboration
  SHALL fail loud citing §3.6.3 (no runtime-sized nets).
- WHEN a vector index is out of the declared range THEN elaboration SHALL
  fail loud naming the net, the index, and the declared bounds.
- WHEN a zero-width or reversed range is declared THEN the declared bit
  order SHALL be preserved and a zero-width net SHALL fail loud.
- WHEN a `specify` path names a terminal that is not a module port THEN
  elaboration SHALL fail loud citing A.7.3.
- WHEN a nature-attribute read names an attribute the nature does not
  declare THEN elaboration SHALL fail loud listing the declared attributes.
- WHEN two out-of-module discipline declarations disagree THEN elaboration
  SHALL fail loud naming both spans (§3.10).

---

## Requirement Traceability

| Requirement ID | LRM sections | Story | Status |
|---|---|---|---|
| LRMW3-01 | 3.6.3 (range), A.2.1.2, A.2.2, A.2.2.1 | P1 vectors — declarations | Pending |
| LRMW3-02 | 5.5.2 | P1 vectors — analog access + genvar loops | Pending |
| LRMW3-03 | 4.2.1, 4.2.1.2, 4.2.1.3 | P1 vectors — part-selects, real/int conversion in vector context | Pending |
| LRMW3-04 | 9.8 (memories half), A.2.2.1 | P1 vectors — memories | Pending |
| LRMW3-05 | 6.5.7.1 (re-check), 7.2.3 | P1 vectors — port width binding | Pending |
| LRMW3-06 | A.7.2, A.7.3, A.7.4 | P2 specify — path delays | Pending |
| LRMW3-07 | A.7.5, A.7.5.1, A.7.5.2, A.7.5.3 | P2 specify — timing checks | Pending |
| LRMW3-08 | A.1.5 | P2 config — loud rejection | Pending |
| LRMW3-09 | A.2.2.2 | P2 drive strengths | Pending |
| LRMW3-10 | 2.9.2, 3.6.3.1 | P2 standard attributes | Pending |
| LRMW3-11 | 3.6.2.5, 3.6.2.6, 3.6.2.7 | P2 discipline/nature extensions | Pending |
| LRMW3-12 | 3.9, 3.10 | P2 discipline precedence + primitives | Pending |
| LRMW3-13 | 3.12.1 | P2 port branches | Pending |
| LRMW3-14 | 5.5.3 | P2 nature-attribute access | Pending |
| LRMW3-15 | 5.5.4, 5.6.8, 5.6.8.1, 5.6.8.2 | P2 hierarchical branches/contributions | Pending |
| LRMW3-16 | 5.6.7.2 | P2 indirect vs direct contribution | Pending |
| LRMW3-17 | 4.5.15, 5.8.4 | P2 analog-operator restrictions | Pending |
| LRMW3-18 | 7.3.2.1 | P2 NaN/inf contribution | Pending |
| LRMW3-19 | 6.4.3 | P3 paramset output variables | Pending |
| LRMW3-20 | 9.8 (task half), 9.9 | P3 PLA + stochastic tasks | Pending |
| LRMW3-21 | 3.13, 3.13.1, 3.13.2, 3.13.3, 3.13.4 | P3 namespace rules | Pending |
| LRMW3-22 | 1.3.4, 1.3.4.1, 1.3.4.2, 1.3.5 | P3 signal-flow systems | Pending |
| LRMW3-23 | 4.2.3, 4.6.4.5, 4.6.4.6, 5.4.4, 5.6.6 | P3 coverage claims for clean probes | Pending |
| LRMW3-24 | 9.4.7, 9.5.1.1, 9.5.1.2, 9.5.9 | P3 display/file-I/O semantics | Pending |
| LRMW3-25 | 9.7.1, 9.7.2, 9.14 | P3 `$finish`/`$stop`/math system functions | Pending |
| LRMW3-26 | 9.22.1–9.22.6, 9.23–9.23.4 | P3 driver-access coverage claims | Pending |
| LRMW3-27 | 6.6.2.1, 6.9.3, 7.4.2, 7.8.5.1, 7.8.6, 8.2, 8.4.1, 8.4.3.1, 8.4.7 | P3 elaboration/connect/scheduling clauses | Pending |
| LRMW3-28 | 12.7, 12.8, 12.9, 12.22, 12.22.1, 12.22.2, 12.31.1–12.31.4, 12.32.2, 12.33.2, 12.36 | P3 VPI category mapping | Pending |
| LRMW3-29 | C.1–C.14, C.16, C.17 | P3 Annex C cross-declarations | Pending |
| LRMW3-30 | — (gate) | G1 — coverage gate reads this spec's Out of Scope table and goes green | Pending |
| LRMW3-31 | 4.6.4.3, 4.6.4.4 | `noise_table`/`noise_table_log` (wave-2 §6 leftover) | Pending |
| LRMW3-32 | 4.5.12, 4.5.4, 4.5.5, 6.5.8, 6.6, 3.3/4.2.13 | remaining wave-2 §6 leftovers (`zi_*` `t0`, tail `abstol`, flow `abstol` inheritance, params inside `generate`, dynamic string concat) | Pending |
| PSW3-01 | B1 | selector: implement the five inert axes, or remove them from the grammar — never leave a silent empty selection | Pending |
| PSW3-02 | B2, B3, B4 | retire the unreachable POM `Kind`/`Value`/`ElabErrorKind` variants (a breaking change, so it needs an explicit decision) or document each as reserved with a test proving it is unreachable | Pending |
| PSW3-03 | B5 | close or formally declare the Python/Rust asymmetries: driver/sampling/Monte Carlo/coverage, `Waveform` measurements+transforms, `Module.constraints` | Pending |
| PSW3-04 | B6 | correct `piperine-ir`'s own stale ownership claim, and pin the delegation with a test that fails if codegen re-grows a private copy | Pending |
| PSW3-05 | B7 | test the legacy `` `include`` aliases and their precedence (a project's own `discipline.h` must win) | Pending |
| PSW3-06 | B8 | the coverage gate stops describing its own red state as expected; its header names `piperine-ir` | Pending |
| PSW3-07 | B9 | one gate enumerating every sanctioned extension and requiring a live test per extension | Pending |
| PSW3-08 | B10 | rename the retired-name example; add an example exercising `constraint`/`tol`/the optimizer | Pending |
| PSW3-09 | §0 rules 1–2 | every `OutOfScope`/deviation row carries a written reason naming what replaces the LRM mechanism | Pending |
| PSW3-10 | G7 | the axis-B gate itself: read `docs/spec/` and require a live test behind every claim of an implemented Piperine-only surface | Pending |

**Coverage:** 42 requirement IDs (32 axis A + 10 axis B); 0 mapped to tasks
(tasks.md not written yet).

---

## Success Criteria

- [ ] `cargo test --workspace` is green, including
      `tests/lrm_section_coverage.rs` with **zero** uncovered sections.
- [ ] `README.md`'s advertised vector/`generate` ladder compiles and runs as
      an example under `tests/run_examples.rs`.
- [ ] `SPEC_DEVIATION.md` regenerates with an empty "still open" section, or
      with every remaining row pointing at an Out of Scope row in this spec.
- [ ] No construct in the grammar reaches the POM as an unread span.
- [ ] The axis-B gate exists, is green, and can be shown to fail (a deleted
      test behind a spec claim turns it red).
- [ ] `docs/spec/part_iii_conformance.md` §4.2's exclusions each name what
      replaces the LRM mechanism, and §6's extension list each name a test.
