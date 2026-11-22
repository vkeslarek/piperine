# Code Clarity Specification

**Approved**: 2026-11-18 - user: "Depois do 12 insere um refinamento no meio somente para refactor de visibilidade de código e estrutura interna. [...] Cria a spec para mim mas deixa anotado que o restante dos achados vai ter que passar um FAble para analisar o código completo e montar a spec completa"

**Amended**: 2026-11-20 - user: "Tem uma coisa que quero que inclua no code-clarity: Se precisar mudar a spec para deixar conceitos mais simples, pode mudar! É bem ruim quando por exemplo eu odeio casos de borda em linguagem! Por exemplo, poder chamar o emit! com varargs e não poder chamar uma função do usuário com varargs. Por isso acho legal ter as ESPECIFICAÇÕES das coisas escritas na linguagem mesmo que a implementação fique no core e isso tudo no prelude. Saca? Quanto mais excessões dentro da linguagem e flows a gnt cortar, melhor!" Adds CC-11 (the language has no special cases) and the second half of the inventory (every special case of the language). CC-11 may change the language and `docs/spec`; CC-09 still holds for everything else.

> **This spec is the frame; the findings inventory is not complete.** Before design, a Fable reads the whole codebase against the seven rules below and writes the complete inventory (`findings.md` in this folder): every crate, module, free function, abbreviated name, god method, misplaced abstraction and hidden data path, each with its rule and the requirement it falls under. **The inventory also lists every special case of the language (CC-11)**: every builtin, macro, operator, statement or flow that can do something a user-written item cannot (for example `emit!` takes a variable number of arguments while a user function cannot), every builtin whose signature is not expressible in MSL, and every rule of `docs/spec` with an exception clause. The requirements here are the rules made testable; the inventory says where they are broken today. Design and tasks come after the inventory.

## Position in the queue

User, 2026-11-18, verbatim: "Eu gostaria que engatilhasse até o 12. Depois do 12 insere um refinamento no meio somente para refactor de visibilidade de código e estrutura interna. O refinamento tem que ser feito depois do fechamento da 12 pois vamos ter código novo entrando sempre e aí depois do refactor entra o 13 com a verificação de cosure da spec usando o Fable e desenvolvimento dos gaps finais."

It runs after item 12 of the queue (`resolved-nets`, `resolved-nets-bundles`) closes and before item 13 (`spec-closure`, with a Fable spec-closure verification and the final gaps). While it runs, no other feature lands code.

## The rules (user, 2026-11-18, verbatim)

"Qual é a ideia desse novo refinamento: deixar o código limpo, organizado, fácil de ler e seguindo nossas regrinhas básicas de limpeza. E quais são as regrinhas básicas de limpeza? Aqui estão:
1. Você tem que bater o olho no código e entender o que ele faz.
2. Do mesmo jeito que bater o olho no código faz você entender ele, bater o olho em toda estrutura de mods e arquivos de um crate te diz exatamente onde encontrar cada coisa, ou seja, a estrutura de mods tem que estar sempre bem colocada, com os objetos organizados de maneira lógica, hierarquica e fácil de ler.
3. Assim como a estrutura de mods tem que estar organizada, os crates também tem que estar. Esse eu acho que não vai ter tanta mudança mas cada crate tem que ter um objetivo específico dentro do fluxo macro ou fazer suporte em vários outros crates.
4. Não quero métodos soltos nos mods, cada método tem que ter um dono!
5. Vamos seguir sempre boas práticas de OO aqui. Cada objeto tem um objetivo único, seu nome tem que dizer muito bem o que ele faz, sem nomes abreviados.
6. Abstrações demais e god methods não são legais, deixam o código sujo.
7. Abstrações bem colocadas podem ser super úteis e simplificam e apoiam a lógica da aplicação. Isso inclusive ajuda a manter métodos bem organizados e limpos. O caminho do dado sempre tem que estar evidenciado."

## Problem Statement

Code has entered fast, in many features and lanes. Each feature kept its own tests green, but no pass has looked at the codebase as a whole for readability: where things live, who owns each function, whether names say what things do, whether the path of the data from `.msl` to the solver and back to the host can be followed by reading. This feature makes the code follow the seven rules without changing what it does.

## Goals

- [ ] Every crate states its one purpose in the macro flow (or the crates it supports), and its contents match it (rule 3).
- [ ] The module tree of every crate tells where each thing is; reading `lib.rs` and the module tree is enough to find any object (rule 2).
- [ ] No free function in a module: every function has an owner type (rule 4), with the exceptions listed in the inventory and justified (e.g. `main`, test helpers, macro-free const fns the spec requires).
- [ ] Names say what things do, with no abbreviations except an explicit glossary of domain terms (rule 5).
- [ ] No god methods and no abstraction without a second use or a clear simplification (rules 6, 7).
- [ ] The data path is evident: each crate documents the path its data takes through it, and the code reads in that order (rule 7).
- [ ] Every struct, enum, trait, method and function carries a comment saying what it is for, and code whose why is not obvious carries a comment saying why (user addition, 2026-11-18).
- [ ] The language has as few special cases as possible: what a builtin can do, a user-written item can do too; every builtin is declared in MSL in the prelude, with its implementation in the core (user addition, 2026-11-20, CC-11).
- [ ] Behaviour is unchanged outside CC-11: every test, golden, twin and Book listing is byte-identical; no tolerance moves.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| New behaviour, fixes or language forms | A refactor only; a defect found is logged in `GAPS.md` with its owner, not fixed here unless the fix is the refactor itself |
| Performance work | `multi-rate` and others own it |
| Generated Rust (`piperine-transpile` output) style beyond names it carries from the IR | The generated code is a debug surface (R3) with its own goldens; it follows the renames only |
| Public spec or Book text outside CC-11 | Unchanged, except a crate or module name the Book's "How Piperine is organized" chapter cites; CC-11 changes the spec and the Book where it removes a special case |

## User Stories

### P1: The structure tells where things are ⭐ MVP

**User Story**: As a reader of the code, I want each crate and module tree to tell me where each thing is, so I can find code without searching.

**Acceptance Criteria** (CC-01, CC-02, CC-03):

1. WHEN the feature closes THEN every crate's `lib.rs` SHALL open with a doc comment stating its purpose in the macro flow (or the crates it supports) and its module map, and a test SHALL fail if a crate lacks it (CC-01)
2. WHEN a crate's module tree is read THEN each module SHALL hold one concern named by the module, with no `utils`, `helpers`, `misc` or `common` module outside test support (CC-02)
3. WHEN the crate graph is read THEN each crate SHALL have one purpose and no crate SHALL hold code whose concern belongs to another crate per the inventory; `crate_boundaries` SHALL encode the final dependency rules (CC-03)

**Independent Test**: a structure guard test over every crate's `lib.rs` docs and module names; `crate_boundaries` green.

---

### P1: Every function has an owner ⭐ MVP

**User Story**: As a reader, I want every function to belong to a type, so behaviour is found next to the data it acts on.

**Acceptance Criteria** (CC-04):

1. WHEN the feature closes THEN no non-test module of any crate SHALL define a free `fn` other than the exceptions listed in `findings.md` with their reason, and a guard test SHALL fail on a new free `fn` not in that list (CC-04)
2. WHEN a free function is moved to a type THEN the type SHALL be the owner of the data it reads or writes, as the inventory names it (CC-04)

**Independent Test**: the free-function guard over `crates/*/src`.

---

### P1: Names say what things do ⭐ MVP

**User Story**: As a reader, I want names that say what a type, method or field does, so I don't need to read the body to know.

**Acceptance Criteria** (CC-05):

1. WHEN the feature closes THEN no type, method, field or module name SHALL be an abbreviation, except the domain glossary in `findings.md` (for example `msl`, `ir`, `abi`, `ac`, `dc`, `op`, `pss`, `lte`), and a guard test SHALL check names against that glossary (CC-05)
2. WHEN a type is read THEN its name SHALL state its one purpose; a type with two purposes per the inventory SHALL be split (CC-05)

**Independent Test**: the naming guard; the inventory's split list closed.

---

### P2: Methods are small and abstractions earn their place

**User Story**: As a maintainer, I want no god methods and no needless abstraction, so the code stays clean as it grows.

**Acceptance Criteria** (CC-06, CC-07):

1. WHEN the feature closes THEN no method SHALL exceed the length limit fixed in design from the inventory's distribution, except the generated-code printers' tables listed in `findings.md`, and a guard SHALL check it (CC-06)
2. WHEN an abstraction (trait, generic, wrapper type) has one implementor and simplifies nothing per the inventory THEN it SHALL be removed; WHEN repeated logic is found THEN it SHALL get an owner type (CC-07)

**Independent Test**: the method-length guard; the inventory's abstraction list closed.

---

### P2: The data path is evident

**User Story**: As a reader, I want to follow the data from `.msl` to the solver and back to the host by reading, so I understand the system end to end.

**Acceptance Criteria** (CC-08):

1. WHEN the feature closes THEN each crate's `lib.rs` doc SHALL state the path its data takes (input type → stages → output type) and the code of the crate's entry point SHALL call those stages in that order (CC-08)
2. WHEN `ARCHITECTURE.md` (or the Book's organization chapter) is read THEN it SHALL show the whole path `.msl → POM → types → elab → IR → plan → transpile/interp → ABI → solver → api → cli` with the owner type of each step (CC-08)

**Independent Test**: a docs test that the path in each `lib.rs` names existing types; reviewer reading.

---

### P1: The code is commented ⭐ MVP

User, 2026-11-18, verbatim: "Coloca na feature do code-clarity também para comentar os códigos, métodos e structs".

**User Story**: As a reader, I want every type and method to say what it is for, so I understand the code without reading every body.

**Acceptance Criteria** (CC-10):

1. WHEN the feature closes THEN every struct, enum, trait, type alias, method and function of every crate, public or private, SHALL carry a doc comment (`///`) stating what it is for and, for a method, what it returns or changes; every struct and enum field and variant whose meaning is not evident from its name and type SHALL carry one too (CC-10)
2. WHEN a crate is built THEN `#![deny(missing_docs)]` SHALL hold for its public items, and a guard test SHALL fail on a private struct, enum, trait or fn without a doc comment, outside test code (CC-10)
3. WHEN a block's reason is not evident (an ordering constraint, a spec rule it implements, a numeric guard, a workaround) THEN it SHALL carry a comment saying why and citing the spec rule or decision (§n, MD-n) where one applies; comments SHALL say what and why, never restate the code line by line (CC-10)
4. WHEN a comment and the code disagree after a refactor commit THEN the commit SHALL fix the comment; stale names in comments are found by the naming guard of CC-05 (CC-10)

**Independent Test**: `missing_docs` per crate plus the private-item doc guard; reviewer reading for what/why quality.

---

### P1: Nothing changes behaviour ⭐ MVP

**User Story**: As the user, I want the refactor to change nothing a run produces.

**Acceptance Criteria** (CC-09):

1. WHEN each commit of this feature lands THEN the full `cargo test` SHALL pass, the 7 twin goldens, the transpile goldens, every parity target and every Book listing SHALL be byte-identical, and no tolerance SHALL change (CC-09)
2. WHEN a public item of `piperine-api` is renamed THEN the Book, examples and tests SHALL follow in the same commit (CC-09)

**Independent Test**: full suite per commit; goldens unchanged.
### P1: The language has no special cases ⭐ MVP

**User Story**: As a user of MSL, I want builtins to follow the same rules as the items I write, and every builtin to be declared in the language itself, so there are no exceptions to learn and no flow only the compiler may use.

**Acceptance Criteria** (CC-11):

1. WHEN the inventory lists a special case THEN design SHALL either remove it by amending `docs/spec` so that the general rule covers it (for example, user functions gain the variadic form `emit!` uses, or `emit!` loses it), or keep it with a written reason in `findings.md` that the user reviews; the default is to remove it (CC-11)
2. WHEN the feature closes THEN every builtin function, method, macro, operator trait and type the user can name SHALL have its signature and its documentation written in MSL in the prelude (`std`), with its body marked as provided by the core; the compiler SHALL resolve builtins through those declarations, not through a hidden table, and a guard test SHALL fail on a builtin with no prelude declaration (CC-11)
3. WHEN a spec rule is simplified THEN the Book, examples, tests and diagnostics SHALL follow in the same commit, and the spec's revision note SHALL name the special case removed (CC-11)
4. WHEN a simplification would move a numeric result (parity, twins, tolerance) THEN it SHALL stop and be reported; CC-11 changes what can be written, never what a run computes (CC-11, CC-09)

**Independent Test**: the prelude-declaration guard over every builtin; the inventory's special-case list closed (each removed or kept with the user's review).

---

## Edge Cases

- WHEN removing a special case needs a new general form (variadic parameters, a trait for an operator) THEN that form is specified in `docs/spec` and implemented for user items too, not added only for the builtin (CC-11).
- WHEN a rule conflicts with the spec's own naming (an MSL keyword, a spec term) THEN the spec's term wins and is added to the glossary.
- WHEN a free function is required by the language (`main`, a `#[test]`, an exported `extern "C"` symbol of the ABI) THEN it is listed as an exception, not moved.
- WHEN moving code would change a public API used by the Book's listings THEN the Book follows in the same commit (CC-09).

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| CC-01 | P1: structure | Design | Pending |
| CC-02 | P1: structure | Design | Pending |
| CC-03 | P1: structure | Design | Pending |
| CC-04 | P1: owners | Design | Pending |
| CC-05 | P1: names | Design | Pending |
| CC-06 | P2: small methods | Design | Pending |
| CC-07 | P2: abstractions | Design | Pending |
| CC-08 | P2: data path | Design | Pending |
| CC-09 | P1: no behaviour change | Design | Pending |
| CC-10 | P1: commented code | Design | Pending |
| CC-11 | P1: no special cases | Design | Pending |

**Coverage:** 11 total, 0 mapped to tasks, 11 unmapped (tasks come after the Fable inventory).

## Assumptions & Open Questions

| # | Assumption / Question | Default chosen | Confirmed by user? |
| - | --------------------- | -------------- | ------------------ |
| 1 | The full findings inventory is written by a Fable reading the whole codebase before design | Yes, `findings.md` | y - user: "deixa anotado que o restante dos achados vai ter que passar um FAble para analisar o código completo e montar a spec completa" |
| 2 | No other feature lands code while this one runs | Yes | y - user: "O refinamento tem que ser feito depois do fechamento da 12 pois vamos ter código novo entrando sempre" |
| 3 | The method-length limit and the glossary are fixed in design from the inventory's numbers | Yes | n - design decides, the user sees it with the inventory |
| 4 | CC-11 may amend `docs/spec` to remove special cases; the default for each listed case is removal | Yes | y - user: "Se precisar mudar a spec para deixar conceitos mais simples, pode mudar! [...] Quanto mais excessões dentro da linguagem e flows a gnt cortar, melhor!" |
| 5 | Builtins are declared in MSL in the prelude, implementation in the core | Yes | y - user: "ter as ESPECIFICAÇÕES das coisas escritas na linguagem mesmo que a implementação fique no core e isso tudo no prelude" |

**Open questions:** none for now; the Fable inventory may raise some, asked before design.

## Success Criteria

- [ ] The seven rules hold over every crate, checked by the guards CC-01, CC-04, CC-05, CC-06 and by the inventory closed.
- [ ] Every type and method is commented (CC-10).
- [ ] Every test, golden and Book listing is byte-identical before and after.
