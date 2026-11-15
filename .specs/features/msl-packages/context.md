# msl-packages — user decisions

Recorded 2026-11-14, before planning.

| # | Question | Decision | User's words |
| - | -------- | -------- | ------------ |
| U1 | Include/search paths (`-I`) besides named dependencies? | No search paths. 100% Rust/Cargo model: every dependency is named in the manifest; a crate reaches another only through that name. | "-I é ruim demais, faremos modelo RUST 100%." |
| U2 | Where does `std` live? | Embedded in the binary (built into the `piperine-lang` crate). | "std fica embutida o binário." |
| U3 | Scope of `piperine-project` | Keep only manifest, dependencies (path/git resolution) and lockfile, ported to MSL. Delete everything else (binary/release device fetch, plugin sections, VAMS `SourceMap` recipe) to be redeveloped later. | "Vamos fazer rollback disso e trazer ele apenas para dependencias, manifesto e lockfile mesmo ... O restante do piperine-project pode apagar para podermos desenvolver depois" |
| U4 | Order | `msl-packages` first; `msl-stdlib` after, as the first real crate loaded through it. | "Legal, gostei!" (to the two-feature proposal) |
| U5 | Roles | Fable plans, Sonnet implements, Opus verifies. | standing rule: "Lança sempre Sonnet para implementação, Opus para verifier" |
| U6 | A local item/import and a dependency share a name | Rust 2018 exactly: in an ordinary path the local name wins; in a `use` path the clash is an ambiguity error (help: rename the dependency with `package = …`, or write `crate::name`). Never hidden silently. | answer "Rust: error in `use` (Recommended)" to the name-clash question |
