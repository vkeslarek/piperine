# Part III — LRM Conformance

**Authority:** `VAMS-LRM-2-4.md` (Accellera Verilog-AMS 2.4.0, May 2014) is the
only grammar and semantics source for Piperine's front end (CLAUDE.md's
provenance rule C7). This Part is the conformance record: what is implemented,
what deviates, what is open — each with evidence a reader can re-run.

*This Part replaces the root `SPEC_DEVIATION.md` audit document and the former
"Interpreted context" stub.*

---

## §1 The mechanical gate

Conformance is not a prose claim in this repository. Three test targets bind it:

| Target | What it enforces |
|---|---|
| `tests/lrm_section_coverage.rs` | Parses **every** `##`..`######` heading id out of `VAMS-LRM-2-4.md` itself (612 today: 486 numbered + 126 annex) and requires each to be either **Covered** — declared by a `//! LRM-COVERS: <ids>` header on a test target or inline `#[cfg(test)]` module that contains a live, non-`#[ignore]`d `#[test]` — or **excluded** in `.specs/features/lrm-conformance-wave2/coverage_exclusions.toml` as `Informative` (the LRM's own front matter says so) or `OutOfScope` (mapped to a row in a feature spec's Out of Scope table). No cell may be empty. Because the id set is read from the LRM, a heading the gate does not know about fails it. |
| `tests/lrm_conformance_matrix.rs` | Cross-checks the wave-1 and wave-2 finding registries against those features' own `spec.md` traceability tables (`include_str!`, so a spec edit that renames or drops an ID is caught), and verifies every "Closed" entry names a test that really exists and is not `#[ignore]`d. |
| `crates/piperine-lang/tests/annex_a_grammar.rs` | Walks Annex A's productions and records, per production, whether the parser implements it — see Appendix B. |

### 1.1 Current gate reading

```text
612 LRM-2-4 headings
├── 475 Covered   (a live test declares the id)
├──  20 Excluded  (Annex G informative ×6, C.19/C.20 pointers, Annex E netlist-profile ×12)
└── 105 No verdict → tests/lrm_section_coverage.rs::covered_and_excluded_partition_the_lrm FAILS
```

The gate is **red on this branch**. Wave 2's plan predicted zero gaps at its
T69; T69 closed 364 of the 469 open at that point and the remaining 105 were
never re-planned. §5 below enumerates all 105 with triage; the work is
specified in `.specs/features/lrm-conformance-wave3/spec.md`.

*Reproduce:* `cargo test --test lrm_section_coverage`.

---

## §2 What conformance means here

Three rules govern every entry in this Part.

1. **Fail loud (CLAUDE.md).** A construct Piperine cannot lower faithfully must
   produce a typed error naming the construct and, where one exists, its LRM
   clause. Emitting `0.0`, a no-op, or an unread span is a defect of the same
   class as a wrong answer — waves 1 and 2 classified those as *silent drops*
   and treated them as the most severe finding type.
2. **Disclosure is not conformance.** A documented deviation is still a
   deviation; it belongs in §4 or §5, never in the "implemented" column.
3. **A claim needs a falsifiable test.** `LRM-COVERS` on a file with no live
   test is itself a gate failure (`declaring_files_have_a_live_test`).
4. **Piperine is a superset, not an equal.** An LRM clause Piperine
   deliberately does not implement is an exclusion with a reason naming what
   replaces it (§4.2), never a defect; a clause implemented differently on
   purpose is a deviation with a pinning test (§4.1). Conversely, the
   extension surface (§6) is **not** exempt from proof — it needs the same
   live test an LRM clause needs. Today it has no coverage gate of its own:
   that second axis is specified in
   `.specs/features/lrm-conformance-wave3/spec.md` §0 and tracked there as
   requirement family `PSW3-*`. Where this Part says "covered", it means
   axis A (the LRM) unless it says otherwise.

---

## §3 Implemented surface (summary)

The implemented surface is described mechanism-by-mechanism in Parts I, II, V,
and X. Summarized against the LRM's own chapter structure:

| LRM chapter | State |
|---|---|
| **2 Lexical conventions** | Implemented: identifiers (incl. escaped), all number forms with SI suffixes and based literals, strings with escapes, comments, attribute instances `(* … *)`, compiler directives (`` `define``/`` `include``/`` `ifdef``/`` `timescale``/`` `default_discipline``…). Gap: the four *standard* attributes of §2.9.2 (`desc`/`units`/`op`/`multiplicity`) are rejected as unknown schemas — §5. |
| **3 Data types** | Implemented: `parameter`/`localparam` (incl. ranges, `from`/`exclude`, string and array-valued), `integer`/`real`/`realtime`/`time` variables with initializers, `genvar`, `nature`/`discipline` declarations and derivation, `ground`, `nodeset`, real nets (`wreal`), 1364 net types (`wire`/`reg`/`tri`), branches, `paramset` (incl. overloading). Gaps: vectors/ranges on any declaration, nature-attribute override from a discipline, nature derived from a discipline binding, discipline user attributes, out-of-module discipline declarations — §5. |
| **4 Expressions** | Implemented: the full operator set incl. `===`/`!==`, short-circuiting, real/integer conversion, all math builtins (`log` = log10, `limexp` with Newton-state limiting), every analog operator (`ddt`, `idt`, `idtmod`, `absdelay`, `transition`, `slew`, `last_crossing`, `limexp`, `laplace_*`, `zi_*`, `ddx`, `table`, `analysis`, noise functions, `ac_stim`). Gaps: nature-attribute access (`a.potential.abstol`), `noise_table`/`noise_table_log`, `zi_*`'s `t0`, §4.5.15's restrictions are unenforced — §4/§5. |
| **5 Analog behavior** | Implemented: contributions (direct, indirect, switch branches, implicit/fixed-point), analog operators with their tolerance arguments, event control (`initial_step`, `final_step` with analysis lists, `cross`, `above`, `timer`, named events), analog functions with defaults and output arguments, `for`/`while`/`repeat` with elaboration-constant bounds, `case`. Gaps: vector branch access, hierarchical contributions, §5.8.4's restrictions — §5. |
| **6 Hierarchy** | Implemented: module/paramset instantiation with named and positional overrides, `defparam` (rejecting `localparam` targets), `generate for`/`if`/`case` unrolling of instances, nets, vars and behavior blocks, hierarchical references and `$root`, `$mfactor` scaling and hierarchical composition, port width checks. Gaps: parameters declared inside a `generate` region, paramset output variables — §4/§5. |
| **7 Mixed signal** | Implemented: discipline resolution (Basic and Detail, Annex F), automatic connect-module insertion with §7.8.5 naming and direction-aware selection, `connectrules`, driver access and driver/receiver segregation, X/Z reads in a continuous context failing loud, `wreal` semantics. Gaps: connect-module naming for built-in gate primitives, supply-sensitive connect-module examples — §5. |
| **8 Scheduling semantics** | Implemented: the six stratified regions (Active, D2a, Inactive, Nba, AnalogMacroProcess, Monitor) with LRM ordering, blocking vs non-blocking write regions, timescale quantization, analog/digital synchronization with rollback. |
| **9 System tasks and functions** | Implemented: display/strobe/monitor/write families with deferral rules, file I/O (`$fopen`…`$fscanf`, `$readmem*`), `$finish`/`$stop`, simulation control and `$simparam`/`$simprobe`, `$random`/`$dist_*` (IEEE 1364-2005 §17.9.3 bit-exact), `$temperature`/`$vt`/`$abstime`, `$discontinuity`, `$bound_step`, `$limit`, `$param_given`/`$port_connected`, `$driver_*`, `$analog_node_alias` (constant form), `$mfactor`, placement functions, math system functions incl. `$clog2`. Gaps: PLA modeling tasks (§9.8 — **deliberate loud rejection, T14/LRMW3-20**: all 16 names, `$async$…`/`$sync$…` × `and`/`nand`/`or`/`nor` × `array`/`plane`, are recognized by `piperine-lang/src/eval/tasks.rs::SystemTaskKind::PlaArray` so a call gets a §9.8-specific codegen error rather than a fuzzy-matched "unknown task" one, but none is lowered — a real implementation needs a new JIT trampoline for runtime plane-loading file I/O, the same gap left open for `$readmemh`/`$readmemb` below, plus a combinational-array evaluation model with no existing analog anywhere in this codebase; `crates/piperine-codegen/tests/vams_pla_tasks_rejected.rs` pins the rejection), `$readmemh`/`$readmemb` populating a *scalarized* memory (T12/LRMW3-04: the solver-side `FileTable::readmem` parser exists, but `emit/stmt.rs`'s `S::TaskCall` dispatch has no arm wiring it to a memory's per-bit nodes at all), `%r` in some paths, file-I/O rollback semantics during rejected iterations — §5. |
| **10 Compiler directives** | Implemented. |
| **11/12 VPI** | Not implemented as a C ABI (out of scope, §4.2). Each of §11.4's ten routine categories maps to a native Piperine mechanism or a typed `NotSupported` in `crates/piperine-plugin/src/vpi.rs`, locked by `crates/piperine-plugin/tests/vpi_routines.rs`. |
| **Annex A** | The parser's production coverage is tracked per production — Appendix B. Gaps: A.1.5 configurations, A.2.2.1's vector/net-type range forms, A.2.2.2 strengths, A.7 specify blocks. |
| **Annex B** | Full keyword table implemented (`crates/piperine-lang/src/keywords.rs`). |
| **Annex C** | Piperine compiles the Verilog-AMS superset; it does not enforce the Verilog-A subset restrictions as a separate mode — §4.3. |
| **Annex D** | `disciplines.vams`, `constants.vams`, `driver_access.vams` shipped in `crates/piperine-lang/headers/` and resolvable with no project dependency. |
| **Annex E** | `$limit` and `$mfactor` implemented; the SPICE *netlist* profile is out of scope (no `.subckt`/`.model` reader) — §4.2. |
| **Annex F** | Both resolution algorithms implemented (`elab/lower/disciplines.rs`). |

---

## §4 Deviations (implemented, but not as the LRM states)

Each row is a deliberate, source-documented divergence. "Evidence" names the
file that carries the reasoning, so a reader can challenge it at the source.

### 4.1 Behavioral deviations

| # | LRM | Deviation | Evidence |
|---|---|---|---|
| D1 | §9.20 | `$analog_node_alias`/`$analog_port_alias` resolve a **constant** target by synthesizing an ideal short (`V(local, target) <+ 0`) rather than merging the two MNA unknowns; a **dotted/hierarchical** constant target folds to the LRM's "did not resolve" value `0`; a **non-constant** target fails loud citing MD-18 (no re-topology during simulation). | `crates/piperine-codegen/src/resolve/pom/expr.rs`, `tests/node_alias_e2e.rs`, `crates/piperine-codegen/tests/vams_port_connected.rs` |
| D2 | §9.17.1 | `$discontinuity(degree)` is accepted and is **provably inert** for the TR-BDF2 integrator — pinned by a dedicated test rather than a comment. | `crates/piperine-codegen/tests/discontinuity_degree_inert.rs` |
| D3 | §4.6 Table 4-17 | `analysis("ic")` and `analysis("nodeset")` parse but never compare true: the solver does not track those two sub-phases. Documented sentinel, not a silent alias; any other unknown name fails loud. | `crates/piperine-codegen/src/emit/analog_expr.rs` |
| D4 | §3.3, §4.2.13 | String concatenation/replication works only when the whole expression is elaboration-constant; a dynamic string expression is rejected loud (the JIT digital ABI has no runtime string type). | `crates/piperine-codegen/src/emit/digital_expr.rs` |
| D5 | §7.9 | Every scheduler-level driver is recorded as `Ordinary`; the ordinary-vs-connect-module classification is not threaded to that layer yet, though `DriverTable::resolved_by_kind` implements the segregation and is tested directly. | `crates/piperine-solver/src/digital/scheduler.rs`, `…/digital/drivers.rs` |
| D6 | §6.6 | `parameter`/`localparam` declared **inside** a `generate` region are not unrolled (nets, vars, behavior and instances are). | `crates/piperine-lang/src/elab/lower/generate.rs` |
| D7 | §6.5.8 | `abstol` inheritance is implemented for **potential** natures per node; the flow half is not. | `crates/piperine-solver/src/analyses/context.rs` |
| D8 | §4.5.12 | `zi_*`'s `t0` (first-transition start time) is not applied — it fails loud with the architectural reason (no sample-and-hold state to shift). | `crates/piperine-codegen/src/resolve/pom/analog_ops.rs` |
| D9 | §4.5.4/§4.5.5 | `idt`/`idtmod`'s trailing `abstol`\|`nature` argument (distinct from `ddt`'s two-argument form, which is implemented) is not applied. | `crates/piperine-codegen/src/resolve/pom/analog_ops.rs` |
| D10 | §4.6.4.3/§4.6.4.4 | `noise_table`/`noise_table_log` are not implemented. They fail loud naming the call — they used to return a silent `0.0`. | `crates/piperine-codegen/tests/analysis_dependent_functions.rs` |

### 4.2 Scope exclusions

| Excluded | Reason |
|---|---|
| Hosting third-party C-ABI VPI/PLI `.so`/`.dll` applications | Piperine's extension ABI is its own native plugin contract (Part VI). VPI's *language surface* stays in scope through that ABI; each §11.4 category is mapped, never silently absent. |
| A raw SPICE netlist (`.subckt`/`.model`) reader | Piperine's SPICE path is header-based: `` `include "spice/…" `` model libraries (Part V). Annex E's netlist-profile sections are excluded against this row. |
| Library/configuration management (`config`/`design`/`liblist`, Annex A.1.5) | Source-set selection belongs to `Piperine.toml` and the `SourceMap` (`piperine-project`), not the HDL. **Today this parses and is discarded — a fail-loud violation listed in §5.** |
| Annex G (change history), Annex H (glossary) | Tagged "(informative)" by the LRM's own front matter. |

### 4.3 Verilog-A subset (Annex C)

Annex C is normative and defines Verilog-A as a *restriction* of Verilog-AMS
(no `wreal`, no `discrete` domain binding, a reduced keyword set, …). Piperine
compiles `.va` and `.vams` with the same front end and **does not enforce the
restriction**: a `.va` file using a Verilog-AMS-only construct compiles rather
than being rejected. This is a superset acceptance, not a semantic difference —
every Verilog-A-legal program behaves as Annex C requires.

---

## §5 Open gaps (not implemented)

Everything below was verified by elaborating or compiling a minimal fixture
against this branch. `ELAB-ERR` = parse/elaborate failure, `LOWER-ERR` =
elaborates but does not lower, `ACCEPTED` = compiles although the LRM makes it
illegal, `SILENT` = parses and is discarded.

### 5.1 Missing grammar families

| LRM | Construct | Observed |
|---|---|---|
| §3.6.3, A.2.1.2, A.2.2.1 | Ranges on declarations: `electrical [0:3] bus;`, `input [0:3] p;`, `reg [3:0] q;`, `wire [3:0] w;` | ELAB-ERR `expected an identifier, found '['` |
| §4.2.1 | Part-select `b[7:4]` | ELAB-ERR |
| §9.8, A.2.2.1 | Memories `reg [3:0] mem [0:7];` | ELAB-ERR |
| §5.5.2 | Vector signal access — `electrical bus[0:3];` then `V(bus[0], g)` | LOWER-ERR `unresolved name` |
| §5.5.2 | Vector access under a genvar `for` loop | ELAB-ERR `undefined name: i` in the port-connection index |
| §3.12.1 | Port branch `branch (<p>) probe_p;` | ELAB-ERR `expected an identifier, found '<'` |
| §3.6.2.5 | `flow.abstol = 10u;` inside a `discipline` | ELAB-ERR `expected an identifier, found '.'` |
| §3.6.2.6 | `nature n : ttl.flow;` (nature derived from a discipline binding) | ELAB-ERR |
| §3.6.2.7 | User-defined attribute in a `discipline` | ELAB-ERR `expected 'domain', 'potential', or 'flow'` |
| §3.10 | Out-of-module discipline declaration `electrical u1.n1;` | ELAB-ERR `expected ';', found '.'` |
| §6.4.3 | Paramset output variables (`(* desc=… *) real ft;` in a `paramset`) | ELAB-ERR `expected 'parameter', found 'real'` |
| A.2.2.2 | Drive strengths `assign (strong1, weak0) w = r;` | ELAB-ERR |
| §2.9.2, §3.6.3.1 | Standard attributes `desc`/`units`/`op`/`multiplicity` | ELAB-ERR `unknown attribute schema 'desc'` |
| §5.5.3 | Nature-attribute access `ddt(V(a,b), a.potential.abstol)` | LOWER-ERR |
| §5.6.8/§5.6.8.1/§5.6.8.2, §5.5.4 | Hierarchical contributions and unnamed hierarchical branches (`V(top.drv.x) <+ 1.8;`, `V(top.drv.a)` in an expression) | ELAB-ERR / LOWER-ERR |

**Vectors are the largest gap.** No range form is accepted anywhere in the
grammar today, in either the analog or digital half. `README.md`'s
`generate`-built resistor ladder over `electrical tap [n-1:0]` does not
compile as written. This blocks §5.5.2, most of §9.8, and parts of §4.2.1.

### 5.2 Parse-and-vanish (fail-loud violations)

| LRM | Construct | Observed |
|---|---|---|
| A.7.2–A.7.5.3 | `specify … endspecify` — path declarations, path delays, system timing checks | SILENT: the block parses and its body is stored as an unread `String`. The solver's `DelayTable`/`TimingCheckTable` (`crates/piperine-solver/src/digital/delays.rs`) exist and are unit-tested, but no front-end path populates them from source. |
| A.1.5 | `config … endconfig` | SILENT: `ConfigDecl { body: String::new() }` — the declaration is consumed and dropped. |
| §3.4.4 | `specparam` | SILENT: a no-op arm in `elab_module`'s `ModuleItem` match (`elab/lower/vams.rs`). |
| §9 | `task` declared inside a module | SILENT: the same no-op arm; a task call has no lowering path. |

### 5.3 Illegal constructs that are accepted

| LRM | Rule | Observed |
|---|---|---|
| §4.5.15 | An analog operator may not appear under a non-constant conditional, in an event-triggered statement, in `repeat`/`while`/non-genvar `for`, or in a user-defined function | ACCEPTED — `ddt()` under a data-dependent `if` compiles |
| §5.8.4 | Analog filter functions and event-control statements may not be conditionally executed unless the condition is constant | ACCEPTED |
| §7.3.2.1 | It is illegal to contribute ±infinity or NaN to a branch | ACCEPTED — `V(a,g) <+ 1.0/0.0;` compiles |

### 5.4 Implemented but unclaimed

These probe clean and need only a coverage claim (a live test declaring
`//! LRM-COVERS:`), not new behavior: §4.2.3 (short-circuit evaluation),
§4.6.4.5/§4.6.4.6 (diode and correlated noise), §5.4.4 (unassigned sources),
§5.6.6 (implicit contributions), §9.4.7 (`%r`), §9.7.1/§9.7.2
(`$finish`/`$stop`), §9.9 (stochastic tasks), §9.14 (math system functions),
§9.22.1–§9.22.6 and §9.23–§9.23.4 (driver access), §7.4.2 (`wreal` port
connection).

### 5.5 The full list of 105

```text
ch. 1   1.3.4 1.3.4.1 1.3.4.2 1.3.5
ch. 2   2.9.2
ch. 3   3.6.2.5 3.6.2.6 3.6.2.7 3.6.3.1 3.9 3.10 3.12.1
        3.13 3.13.1 3.13.2 3.13.3 3.13.4
ch. 4   4.2.1 4.2.1.2 4.2.1.3 4.2.3 4.5.15 4.6.4.5 4.6.4.6
ch. 5   5.4.4 5.5.2 5.5.3 5.5.4 5.6.6 5.6.7.2 5.6.8 5.6.8.1 5.6.8.2 5.8.4
ch. 6   6.4.3 6.6.2.1 6.9.3
ch. 7   7.2.3 7.3.2.1 7.4.2 7.8.5.1 7.8.6
ch. 8   8.2 8.4.1 8.4.3.1 8.4.7
ch. 9   9.4.7 9.5.1.1 9.5.1.2 9.5.9 9.7.1 9.7.2 9.8 9.9 9.14
        9.22.1 9.22.2 9.22.3 9.22.4 9.22.5 9.22.6
        9.23 9.23.1 9.23.2 9.23.3 9.23.4
ch. 12  12.7 12.8 12.9 12.22 12.22.1 12.22.2
        12.31.1 12.31.2 12.31.3 12.31.4 12.32.2 12.33.2 12.36
Annex A A.1.5 A.2.2 A.2.2.1 A.2.2.2
        A.7.2 A.7.3 A.7.4 A.7.5 A.7.5.1 A.7.5.2 A.7.5.3
Annex C C.1 C.2 C.3 C.4 C.5 C.6 C.8 C.9 C.10 C.11 C.12 C.13 C.14 C.16 C.17
```

Triage: chapters 1, 3.13, 7.2.3, 8.x and Annex C are prose or delta clauses
that need cross-declared coverage claims; chapter 12 needs the VPI category
mapping declared per section; the rest are the behavioral gaps of §5.1–§5.3.

---

## §6 Sanctioned extensions beyond the LRM

Piperine accepts a small, deliberate superset. Each item is an extension, not
a divergence: no LRM-legal program changes meaning because of it.

| Extension | Where |
|---|---|
| `constraint begin … end` with `require`/`var`/`target`/`cover`/`cover cross`, analysis scoping (`@ dc`/`@ tran`/`@ ac`) and `after`/`dur` windows | MD-49; Part VII §3.5, Part VIII |
| `tol gauss/uniform(...) [global]` on parameters (the random-variable catalogue) | Part VIII sampling/Monte Carlo |
| `const <name> = <expr>;` at file scope | MD-50, Part I |
| `(* rfport, num, z0 *)` attribute schema for S-parameter ports | Part VII `.sp` |
| `(* device = … *)` / `(* port = … *)` attribute schemas | Part VI |
| `///` doc comments attached to declarations | Part I §1.1; surfaced by the LSP |
| `@(initial_step) V(a,b) <+ value;` as a UIC seed (SPICE `.ic`) | Part VII §10.1 |
| `\%` escape in strings; `'{`, `<<<`, `>>>` lexemes; `$unconnected` sentinel nets | `crates/piperine-lang/src/lexer.rs`, `pom/net_type.rs` |
| `paramset name (module);` (parenthesized form) alongside the LRM's `paramset name module;` | `crates/piperine-lang/src/grammar/item.rs` |

---

## §7 History

| Wave | Feature dir | Outcome |
|---|---|---|
| Wave 1 | `.specs/features/lrm-conformance-closure/` | 54 LRMC ids from the 2026-08-14 `docs/audit/` findings, closed and locked by `tests/lrm_conformance_matrix.rs`. |
| Wave 2 | `.specs/features/lrm-conformance-wave2/` | T1–T72 against the 2026-08-19 audit: all 10 silent drops, all 15 semantic divergences, 13 of 14 missing grammar families, most ignored operator arguments and all disclosed deviations. Introduced the §1 coverage gate. Left 105 sections without a verdict. |
| Wave 3 | `.specs/features/lrm-conformance-wave3/` | Planned, not started: §5's gaps and the gate's closure. |

Each closed wave-2 item cites the commit that closed it; the per-item table
lives in that feature's own directory and is not duplicated here.
