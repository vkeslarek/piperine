# Piperine Hardware Definition Language — Complete Specification

The single authoritative reference. Part I is the normative language specification; Parts II–VI
are the elaboration model, formal grammar, reflection API, selector, and builtins reference. All
parts are consistent with the current design: `UInt`/`SInt`/`Complex` are library bundles;
disciplines are conservative or storage; metadata rides on `@` attributes; the core grammar is
closed and grows through extension layers.

## Contents

- **I — Language Specification.** Goals and governing rules (No-Magic, tier independence,
  No-Bloat), the value/net model, lexical form, modules, types, attributes, functions and
  generation, `analog`/`digital` behavior, phases, the extension model, rejected features, and
  worked architectures.
- **II — Elaboration.** Source → `ElabProgram`: const evaluation, type/discipline resolution,
  structural elaboration, monomorphization, bundle expansion, events, injected stdlib, validation.
- **III — Grammar.** The LL(1) EBNF, including SI literals, attributes, inferred `var` types,
  named ports, and match patterns.
- **IV — Reflection (POM).** The typed object graph for `bench`/Python/Rust and the plugin ABI.
- **V — Selector.** The query language whose axes are POM relations and predicates POM attributes.
- **VI — Builtins.** Normative catalog of math, analog operators, `$`-syscalls, tasks, events,
  and the prelude/stdlib, with fidelity gaps and the alias policy.

The `bench` block and the uniform simulation API are specified separately in
`crates/piperine-bench/docs/SPEC.md`.

---

