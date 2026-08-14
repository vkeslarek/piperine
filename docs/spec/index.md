# Piperine — Formal Specification

**Version {{ vams_version }}**

> **Frontend notice (2026-08, `verilog-ams-support` feature, MD-42/44/45/47).**
> PHDL is retired — zero `.phdl`/`.ppr` sources remain in the tree. **Verilog-AMS
> (LRM 2.4, `.vams`/`.va`) is the sole frontend**, implemented by `piperine-lang`
> (`crates/piperine-lang/src/{ast,grammar,lexer.rs,preprocessor.rs,parser.rs,keywords.rs}`
> for parsing, `elab/`/`pom/`/`eval/` for elaboration into the POM). Parts I, II, IV, and
> V below are full rewrites, grounded directly in this source and in `VAMS-LRM-2-4.md`
> (the LRM copy at the repo root) — they are **normative** for what a `.vams` source
> file means today, not historical reference. See `.specs/features/verilog-ams-support/`
> (spec/design/tasks/validation) for the migration's own record, and
> `.specs/STATE.md` for the decision log (MD-42 through MD-47).
>
> **MD-47 (2026-08-13):** the parser used to live in a separate crate, `piperine-vams`,
> behind a zero-dependency purity gate. That crate no longer exists — its modules were
> merged physically into `piperine-lang`, which kept its own name. Every source
> reference in this document set to `crates/piperine-lang/src/...` covers both the
> parser and the elaborator; there is no `crates/piperine-vams/` on this branch. See
> Part I §0 for the full mechanics.
>
> Annex G (LRM change history) and Annex H (LRM glossary) are ported as reference
> material under `docs/reference/` and are unaffected by this rewrite.

The authoritative specification of Piperine's Verilog-AMS frontend, elaboration
pipeline, native circuit solver, and host APIs, as they exist on this branch.

## Scope

A `.vams` source file has **one grammar and two execution contexts**, mirroring the LRM
itself:

1. **Elaboration** — `module`/`nature`/`discipline`/`paramset` declarations,
   `generate` regions, hierarchical instantiation. Resolved once into a `Design` (the
   POM, Part IV): every `generate` region unrolled into derived instances, every net's
   discipline resolved, the analog language subset (Annex C) enforced. Pure, total.
2. **Compiled solve** — `analog`/`analog initial`/`always`/`initial` behavior blocks.
   Lowered to JIT-compiled analog kernels and an event-driven digital interpreter,
   executed by the solver engines during DC/AC/tran/noise/TF analyses (Part VII).

Driving analyses, measuring, and verifying are **host** concerns — the Python/Rust host
APIs (Part VIII), not language constructs. There is no in-language `bench` context
(removed before PHDL's own retirement — Part III).

## Document set

| Part | File | Covers |
|------|------|--------|
| **I** | [Language](part_i_language.md) | Verilog-AMS as Piperine implements it: lexical/type/expression/analog/hierarchy semantics per LRM chapter, the digital subset's live-vs-parse-only split, Piperine's `(* device *)`/`(* port *)` attribute extension, preprocessor, and what is explicitly out of scope |
| **II** | [Elaboration](part_ii_elaboration.md) | The real `PASSES` pipeline (`Register` → `LowerDeclarations` → `ElabModules` → `ResolveDisciplines` → `InsertConnectModules` → `UnrollGenerate` → `Typecheck` → `FlattenHierarchy`), const evaluation, the E-STD error catalog, POM navigability |
| **III** | [Interpreted Context — REMOVED](part_iii_interpreted_context.md) | The in-language bench was removed; hosts drive simulations (Part VIII) |
| **IV** | [Reflection & Selector](part_iv_reflection_selector.md) | The current POM shape (natures/branches as first-class nodes, the PHDL-era value-layer nodes removed), the selector query language, and where the two now disagree (the `Node` graph is narrower than the `Kind` enum suggests) |
| **V** | [Builtins Reference](part_v_builtins.md) | Math functions and analog operators as native Rust tables (no more `extern`-declared surface), LRM ch. 9 system tasks, the Annex D stdlib headers and SPICE model library, and the gate/UDP/system-task "parsed but not wired" surface |
| **VI** | [Plugins](part_vi_plugins.md) | Plugin extensibility model: devices, lifecycle hooks, custom scripts, attribute schemas, security model |
| **VII** | [Solver](part_vii_solver.md) | Device ABI, analog/digital namespaces, MNA stamping, analyses, mixed-signal execution, convergence aids |
| **VIII** | [Host APIs](part_viii_host_api.md) | Python (`import piperine`) + Rust (`piperine-api`): load/Design/Module, analyses + results, LiveSession, `piperine run`/`-i`/`test` |
| **IX** | [Runtime Semantics (piperine-signal)](part_ix_signal_semantics.md) | The pure computation crate shared by compiled devices and host devices: runtime operators, event detectors, the junction limiter, table interpolation, `Behavior`/`Measure`/`Wave`, derivative verification, digital edge logic (IEEE 1364) |
| **X** | [Codegen Pipeline (piperine-codegen)](part_x_codegen_pipeline.md) | POM → resolve → flatten → emit → kernel → device: symbolic differentiation, Cranelift emission/CSE, compiled kernels, `CircuitCompiler`, the piperine-signal delegation boundary, the fail-loud coverage contract |
| **App. A** | [Worked Examples](appendix_a_worked_examples.md) | End-to-end `.vams` architectures spanning analog, digital, and mixed-signal |
| **App. B** | [Grammar](appendix_b_grammar.md) | Piperine's sole EBNF authority is the LRM's own Annex A; this appendix covers the conformance gate and Piperine's one syntactic extension (`(* device *)`/`(* port *)`) |
| **App. C** | [Host API Surface Reference](appendix_c_host_surface.md) | Flat inventory of the Python + Rust host surfaces, asymmetries, and the surface-review sheet |

## Reading this document set honestly

Parts I/II/IV/V and this index were rewritten by reading the current source and the
independent Verifier's report (`.specs/features/verilog-ams-support/validation.md`) —
not by search-replacing "PHDL" with "Verilog-AMS" in the prior text. Two disclosure
patterns recur throughout, worth knowing before reading further:

- **"Implemented, not wired."** Several LRM ch. 7-9 surfaces (gate primitives, UDPs,
  `$`-system tasks, the ch. 8 stratified event-region cycle) have a full, independently
  tested Rust implementation of their *semantics*, with zero elaboration-pipeline or
  codegen path connecting a `.vams` source construct to that implementation. Each such
  gap is named at the specific file and task that left it open (Part I §6/§7, Part V
  §3/§5) — it is a real, load-bearing limitation, not a documentation nicety.
- **This branch's own project documentation is not always current.** Two concrete
  corrections this rewrite makes, both verified directly against source rather than
  against `ROADMAP.md`/`CLAUDE.md`'s prose: `laplace_*`/`zi_*` operators are fully
  registered in `piperine-codegen`'s `AnalogOpRegistry` today (Part V §2, Part I §10),
  contradicting both documents' current "not declared, fail loud" claim; and the POM's
  `constraint::` selector axis is live syntax that dispatches to nothing
  (`Axis::Constraint => {}`, Part IV §10), because `pom::ConstraintBlock` itself was
  deleted at MD-45 and never rebuilt (validation.md Gap 3).

## Conventions

- **Normative** prose defines the contract; **non-normative** text is marked *note* or
  *rationale* and is explanatory.
- Code blocks tagged `verilogams` illustrate surface syntax (drawn from `examples/
  *.vams` where possible); blocks tagged `rust` show the implementing type/function
  signature directly.
- Error codes follow the pattern `ENNNN` (elaboration, Part II §6) / `P0NNN` (plugins,
  Part VI). Reflection-layer errors (`ReflectError`, E3xxx) are cataloged in Part IV §3.
