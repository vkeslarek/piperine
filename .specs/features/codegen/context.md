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

## The first end-to-end slice (user, 2026-11-15)

The orchestrator proposed a vertical slice from MSL to the solver instead of finishing each stage in full, with the recommendations below. The user's answer, verbatim: "Concordo e pode disparar". The rows are decided by that answer.

| # | Decision | Closes |
| - | -------- | ------ |
| S1 | **M1, analog parity.** Devices: resistor, capacitor, voltage and current sources, diode with `limit` and `pnjlim`. Analyses: OP, DC, AC, transient. Fixtures: `divider`, `diode_iv`, `diode_series`, `ac_rc`, and an RC transient. The path is `.msl` → parse and POM → type check (subset) → IR → elaboration → interpreter (one generic `Device` over the IR, through the ABI) → solver, and the result is compared with the `.cir` twin run by ngspice. | O6, ELAB-Q5 |
| S2 | **M2** is digital and mixed-signal: the galleries of Appendices E and F and the conformance set of Appendix C.2.3. | ELAB-Q5 |
| S3 | **M3** is `ir-rust`: the transpiler, with the interpreter as its oracle. | O6 |
| S4 | Jacobians come from forward-mode dual numbers, in the interpreter and later in generated Rust. No symbolic differentiation pass exists on the IR. | O1, IR-Q2, INT-Q1, RS-Q5 |
| S5 | Type checking is its own feature, `msl-typeck`, scoped to the M1 subset. | ELAB-Q1 |
| S6 | The IR holds one body per monomorphized module and a flat instance table. | O3, IR-Q3, ELAB-Q2 |
| S7 | Elaboration produces one body per distinct variant and keeps functions as functions. Nothing is inlined in full. | ELAB-Q3 |
| S8 | Two backends are compared bit for bit on the discrete side and within the session tolerances on the analog side. | INT-Q3 |
| S9 | Parity is its own small feature, `msl-parity`: a `.msl` file and its `.cir` twin, one `ToleranceContract` per domain as the only place a numeric tolerance is written, one test per fixture. | - |
| S10 | Model roles for the implementation: Sonnet implements and Opus verifies; Opus implements directly when the work is delicate and not large; Fable plans. | amends C10 for this slice |

The five features of M1 (`msl-typeck`, `msl-ir`, `msl-elab`, `ir-interp`, `msl-parity`) are specified and designed in one pass. Their map is `.specs/features/e2e-m1/overview.md`; what is still the user's to decide is `.specs/features/e2e-m1/questions.md`. None of the five specs is approved yet.

## Closed by the M1 design pass (2026-11-15)

Closed from code and spec evidence, not by a user answer. Each is argued in the design named.

| Open item | Closed as | Where |
| --------- | --------- | ----- |
| O2, IR-Q1 (node set) | value nodes mirror the POM's expression forms; the analog forms are dedicated nodes: probe, environment read, `limit` site, `ac_stim` site, and a contribution split into addends, each direct or under `ddt` | `msl-ir/design.md` |
| IR-Q5 (POM properties) | all four: immutable after build, `Send + Sync`, deterministic ids, `check()` | `msl-ir/design.md` |
| IR-Q7 (`docs/ir/`) | not in M1; rustdoc only until the node set covers the digital side | `msl-ir/spec.md` |
| ELAB-Q4 (lowered forms) | follows the node set above | `msl-elab/design.md` |
| ELAB-Q6 (`extern` modules) | rejected with a diagnostic | `msl-elab/spec.md` |
| ELAB-Q7 (crate) | `piperine_lang::elab`; `piperine-lang` depends on the IR crate | `msl-elab/design.md` |
| INT-Q2 (debug mode) | not in M1: the printed IR and the solver's trace flags are the debug surface | `ir-interp/spec.md` |
| INT-Q4 (default backend) | not applicable while one backend exists | `ir-interp/spec.md` |
| INT-Q5 (ABI fit) | one generic device; the shape is derived from the body, see the mapping table | `ir-interp/design.md` |
| INT-Q6 (crate) | `piperine-interp`, depending on the IR crate, `piperine-abi` and `piperine-solver` | `ir-interp/design.md` |
| INT-Q7 (wide numerics) | not in M1: the subset has `f64` and `bool` only | `msl-typeck/spec.md` |
| The linear-term lesson of `solver-abi-integration` T5 | a term affine in its continuous reads is written as `c + Σ partial·read`, the products the assembly subtracts | `ir-interp/design.md` |

## M1 questions answered (user, 2026-11-15)

The eight questions of `.specs/features/e2e-m1/questions.md`, with the user's choice labels verbatim.

| # | Question | User's choice | Decided |
| - | -------- | ------------- | ------- |
| Q1 | may `piperine-abi` gain a constructor for a module that carries data | "Add Factory to abi (Recommended)" | `device::Factory` and `thunk::module_with`; `Device` and `module::<D>` unchanged; one sentence added to abi.md §7 |
| Q2 | a parity test when ngspice is absent | "Fail, opt-out env (Recommended)" | fail, with `PIPERINE_NGSPICE=skip` as the explicit opt-out; CI installs ngspice |
| Q3 | the tolerance contract's numbers | "Old contract, 1e-12 A floor (Recommended)" | 1e-3 relative; 1e-6 V; 1e-12 A; AC magnitude floor 1e-12; AC phase 1e-4 rad |
| Q4 | the M1 device library | "Accept all (Recommended)" | a fixture, `tests/ngspice/devices.msl`; a library is a feature after M1 |
| Q5 | codes for "not supported yet" and "mismatched types" | "Accept all (Recommended)" | compiler-owned `P0001` and `P0002` |
| Q6 | the IR crate's name | "Accept all (Recommended)" | `piperine-ir`; `RETIRED_CRATES` drops the name in the commit that creates the crate |
| Q7 | a sixth twin for the current source | "Accept all (Recommended)" | `isrc_r.cir` and `isrc_r.msl` |
| Q8 | when Appendix C is amended | "Accept all (Recommended)" | after M1 runs, in one spec edit with `spec-abi-amendments` |

With these the five M1 specs are approved (user: "Concordo e pode disparar", and the answers above). Tasks are approved under the user's standing instruction: "Ok, deixo tu ir tocando, pode ir disparando implementações em seguida ok?".

## Order after M1 (user, 2026-11-15)

User, verbatim: "Mergeia o buck e as features prontas. Pode ir rodando os grandes passos. Eu deixaria o interpreter de lado por enquanto e faria o transpiler primeiro, dá menos trabalho apesar do debug não ser perfeito, ainda podemos olhar no resultado intermediário."

| # | Decision | Amends |
| - | -------- | ------ |
| R1 | `ir-rust` (M3) comes **before** M2. The interpreter is frozen at its M1 subset; it is not extended to digital or mixed-signal for now. | S2, S3, O6 |
| R2 | M2 (digital, mixed, galleries E/F, conformance C.2.3, the buck) is built on the transpiler path. Where the interpreter covers a form (the M1 analog subset), it stays the differential oracle; elsewhere the oracle is the gallery's own expected behavior and ngspice where a twin exists. | C6 for the forms the interpreter lacks |
| R3 | The generated Rust is the debug surface: kept on disk, formatted, readable, each item naming its IR/MSL path ("ainda podemos olhar no resultado intermediário"). | RS-Q8 |
| R4 | The orchestrator settles the open RS-Q questions under the user's standing delegation ("pode ir disparando implementações", "Pode ir rodando os grandes passos") and records each answer in `ir-rust/spec.md`. | - |
