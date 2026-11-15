# Codegen — decisions from the user discussion (2026-11-14)

Umbrella record for the features that connect MSL to the solver: `msl-elab`, `msl-ir`,
`ir-interp`, `ir-rust`, and the device ABI (decided by an adversarial duel, see below).
Nothing here is approved for execution; each feature gets its own spec, approval and tasks.

## Why (user's words)

"O que conecta o solver ao código era o codegen via cranelift que usava o mesmo ABI do solver.
No passado tive muitos problemas com o cranelift pois ele é MUITO low level. Era difícil
debugar, era difícil achar bugs, verificar, etc... Estou pensando em adotar uma linguagem mais
high level para uma geração intermediária."

## Decided

| # | Decision | User's words |
| - | -------- | ------------ |
| C1 | Cranelift is retired (its crate `piperine-codegen` is deleted by `solver-restructure`). | (above) + "Vamos remover o IR, cranelift e Python por agora." |
| C2 | The IR is an **object model**, not a text format: an arena model with typed ids and navigable handles, like the POM, with serde. A one-way printer (no parser) exists for debugging and golden tests only. | "Eu recomendo que a IR seja uma IR não textual e sim um object model." |
| C3 | Chain: **POM → (elaboration) → IR → Interpreter**, and **IR → Transpiler (Rust)**. The IR is a side artifact; the POM keeps mirroring the source (navigability rule). | "Seria de POM -> IR -> Interpreter ou IR -> Transpiler." |
| C4 | The production target language is **Rust**. MSL value-level code (fn, struct, enum, match, impl) is near-1:1 Rust, so the transpiler reprints most expressions; only MSL-specific forms are lowered (unknown values §3.3, SI literals, carriers/drives/commits, analog forms `+>`/ddt/idt/delay/noise, events and reactions). | "Eu aceito rust como linguagem final, sendo desnecessário traduzir muitas das expressões, o que simplifica o chain." |
| C5 | `rustc` is required in the environment for now; shipping a `rustc` inside the binary is a later decision. | "Recomendo shipar um rustc dentro do binário para não exigir no ambiente mas isso a gnt vê depois, por enquanto exige por simplicidade." |
| C6 | The interpreter is the reference backend: always available, the debug mode, and the differential oracle every fast backend is tested against. | proposed by the orchestrator, accepted ("O que acha?" → C2–C5 built on it) |
| C7 | Generated code is compiled once per design at elaboration, cached by hash; nothing is recompiled during a simulation (MD-18). | MD-18 |
| C8 | The **device ABI** is decided by an adversarial Fable duel (critic vs builder, elegance score ≥ 8). It is the interface for every device coupled to the solver: generated devices AND future external ones (e.g. AVR emulators, co-simulation). It must be as elegant as the language. | "Gostei da ideia de ter Duelo para definir um ABI beeeeeem interessante pois ele vai ser a interface dos devices acoplados no futuro (como, por exemplo, emuladores de AVR, etc...). Eu gostaria que o ABI fosse tão elegante quanto a linguagem" |
| C9 | Solver context the ABI must fit (from `solver-completeness`): one event owner dispatching to an analog and a digital sub-solver; the boundary is events only; carriers are opaque bytes; the solver holds physical, not language, complexity; the ABI may change freely. | solver-completeness context.md, rounds 1–2 |
| C10 | Model roles: logic changes in the core are implemented by Opus and verified by Fable. | "mexer no core core (mudança de logica) seja implementada por opus e verificada com fable" |

## Open (documented, not decided)

| # | Question | Current proposal |
| - | -------- | ---------------- |
| O1 | Jacobian: forward-mode automatic differentiation (generated analog code generic over a `Real` number type, run with dual numbers, same in the interpreter) vs symbolic differentiation on the IR. | AD with duals: removes a whole stage, exact to machine precision (keeps ngspice parity), cheap for 2–4-terminal devices; loses symbolic CSE (LLVM recovers part). |
| O2 | The IR node set: how analog contributions, the digital DAG definitions and event reactions are represented; how close value-level nodes stay to the POM's expression model. | IR mirrors POM expressions for value code; dedicated nodes for the lowered MSL forms. |
| O3 | Elaboration POM→IR: instantiation, monomorphization of generics/templates, param folding, generate/loops; what the IR keeps of the hierarchy (names for introspection and debug). | flat per-design IR with a name map back to POM handles. |
| O4 | Transpiler output shape: one crate per module vs per design; cache layout; how generated code reaches the ABI (C ABI function table over byte buffers vs Rust traits across a dylib). | one crate per module, `cdylib`, C ABI table (pending the duel). |
| O5 | Compile latency for big designs; whether the interpreter is the default for small runs. | cache per module; interpreter for fast iteration. |
| O6 | Order of delivery. | `msl-elab` + `msl-ir` + `ir-interp` first (first end-to-end MSL→solver, enables integration tests and ngspice parity), `ir-rust` after. |
| O7 | Shipping `rustc` with the binary. | later (C5). |
| O8 | Everything the ABI duel decides (device shape, state, events, small-signal/noise, introspection, external devices, versioning). | duel output. |

## Device ABI decisions (user, 2026-11-15)

The duel converged (`.specs/features/device-abi/abi.md`, elegance 8.1, 13 concepts, 6 entry points). Spec amendments of its §15:

| # | Amendment | User's choice |
| - | --------- | ------------- |
| 1 | §17.2 rule 2: a commit always advancing time by at least one tick does not make its instant a time point (`LATER`) | "Accept (Recommended)" |
| 8 | continuation variables visible to models as core environment functions `gmin()`, `source_scale()` | "Core fns gmin()/source_scale() (Recommended)" |
| 3b | `extern "cosim"` modules with continuous ports | "Not now (Recommended)" - stays discrete-only |
| 6, 2, 3a, 4, 9 | two canonical-byte sentences in Part I; time-point definition; extern C interface = device ABI; structure-changing `set` refused like a structural param; Appendix C note on records | "Yes, all of them (Recommended)" |
| 5, 7 | withdrawn by the critic | - |

These spec edits land in their own small feature (`spec-abi-amendments`), not yet planned.

## Lesson from solver-abi-integration T5 (2026-11-15)

The analog assembly builds a term's RHS as `-(v - Σ g·x)` from the partials. A device whose value is not the same floating-point expression as `Σ g·x` (e.g. a resistor writing `(vp-vn)/r` instead of `g·vp - g·vn`) leaves a last-bit RHS residue every solve; the step controller amplified it to 1e-12 relative drift after a zero crossing. Open item for msl-ir / ir-interp / ir-rust: for terms linear in their reads, emit the value as the exact `Σ partial·read` products the assembly uses (or have the assembly treat declared-linear terms as RHS-free), so generated devices match the hand-written ones bit for bit and ngspice parity holds.
