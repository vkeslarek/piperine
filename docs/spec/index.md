# Piperine Hardware Description Language — Formal Specification

**Version {{ phdl_version }}**

The authoritative specification of the Piperine Hardware Description Language (PHDL)
and its mixed-signal simulation environment.

## Scope

PHDL is a single, strongly-typed, mixed-signal hardware description language with **one
grammar and three execution contexts**:

1. **Elaboration** — `mod` bodies, type annotations, structural control. Resolved once
   into a fixed, monomorphic netlist. Pure, total (bounded recursion).
2. **Compiled solve** — `analog` and `digital` behavior blocks. Lowered to JIT-compiled
   analog kernels and an event-driven digital interpreter. Executed by the solver
   engines during DC/AC/tran/noise/TF analyses.
3. **Interpreted context** — `bench` blocks. Effectful tree-walk over an already
   elaborated design. Drives analyses, measures, stages overrides, emits artifacts.

The grammar is *the same* across all three contexts (`fn`-body grammar — `var`, `if`/
`else`, `match`, `for`, `return`, expressions). What differs is **purity**, **effect
availability**, and **system tasks**. The formerly separate "bench language" is not a
second language; it is PHDL interpreted rather than compiled, with a bench-specific
syscall set. Part III covers it as such.

## Document set

| Part | File | Covers |
|------|------|--------|
| **I** | [Language](part_i_language.md) | Normative core: goals, lexical, types, modules, behavior, functions, attributes, system tasks, extension model, phase model, No-Magic, rejected features |
| **II** | [Elaboration](part_ii_elaboration.md) | Source → elaborated design: const eval, type resolution, structural unrolling, monomorphization, bundle expansion, validation catalog |
| **III** | [Interpreted Context (Bench)](part_iii_interpreted_context.md) | The interpreted face of PHDL: execution model, name resolution, analyses, results, sweeps, host-neutral API, bench system tasks |
| **IV** | [Reflection & Selector](part_iv_reflection_selector.md) | The Piperine Object Model (POM) and the selector query language |
| **V** | [Builtins Reference](part_v_builtins.md) | Math, analog operators, `$`-syscalls, `@`-events, diagnostics, prelude/stdlib, system-task availability matrix |
| **VI** | [Plugins](part_vi_plugins.md) | Plugin extensibility model: devices, lifecycle hooks, custom scripts, attribute schemas, security model |
| **App. A** | [Worked Examples](appendix_a_worked_examples.md) | End-to-end architectures spanning analog, digital, interpreted, and mixed-signal |
| **App. B** | [Complete Grammar (EBNF)](appendix_b_grammar.md) | Consolidated LL(1) grammar for the whole language |

## Conventions

- **Normative** prose defines the contract; **non-normative** text is marked *note* or
  *rationale* and is explanatory.
- Each section carries its own **BNF** productions (cross-referenced into Appendix B)
  and its own **validation rules** (cross-referenced into Part II §11 where error codes
  are cataloged).
- Code blocks tagged `phdl` illustrate the surface syntax; blocks tagged `ebnf` are
  grammar productions.
- Error codes follow the pattern `ENNNT` where `N` is the catalog (E2xxx elaboration,
  E3xxx reflection); see Part II §11.
