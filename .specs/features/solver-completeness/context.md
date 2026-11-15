# Solver Completeness Context

**Gathered:** 2026-11-14
**Spec:** `.specs/features/solver-completeness/spec.md`
**Status:** Answered. The questions below are kept as asked; the decisions are in the two "User answers" sections at the end and in the Assumptions table of `spec.md`. Next: `.specs/features/solver-restructure/` (feature A).

---

## Feature Boundary

Give the solver an engine for every dynamic rule of the MSL specification (Parts II to VI, VIII) and remove what MSL does not use. The solver stays the numeric and event core: it evaluates no model expression, knows no MSL type, formats nothing. The elaborator, codegen and the host are other features. Evidence: `analysis.md`.

---

## Open Questions

### Q1. Where does the discrete engine live?

MSL's discrete side is: carriers, drives, `resolve`, commits with transport delay, the microstep loop, settle. Something must own it.

- **A (recommended). The event kernel lives in the solver; evaluation lives in elements.** The solver owns ticks, the queue, the carrier store, commit ordering, transport removal, the microstep loop, checkpoint and rollback. Elements (generated code) are called to evaluate a definition, a `resolve` or a reaction. Why: the invariants I1 to I8 of §15 (determinism, snapshot, order, no trace of rejection) are engine properties, and the jump and settle rules need the kernel and the analog driver in one loop. This is the "core do negócio".
- B. The solver owns only time and the analog system and calls one pluggable `DiscreteEngine` trait per instant; the engine itself is a new crate or part of codegen. Smaller solver, but the ordering and rollback rules leave the crate that owns time, and two crates must agree on every instant.
- C. Keep the present `DigitalDevice` seq/comb contract and bolt MSL on top in codegen. Least work now; rows 4, 5, 6, 17 of the analysis cannot be met.

### Q2. How does the solver store a carrier value?

- **A (recommended). Opaque bytes.** Each carrier and each drive is a fixed-size byte slot whose layout codegen chooses (levels, known flags). A change is a bytewise difference; codegen keeps unknown levels in a canonical form so that §5.3.4 to §5.3.6 hold bitwise (`0.0` to `-0.0` is a change, the same NaN is not). The solver never learns a type.
- B. A closed value enum in the solver (`Bool`, `Int`, `Real`, `Unknown`, aggregates). Easier to debug and to show to a host; the solver then mirrors the MSL type system and grows with it.

Consequence of A: the host reads carriers as bytes plus metadata from the elaborator. Is that acceptable for `piperine-api`?

### Q3. Remove the Verilog-AMS digital and IO surface now?

`analysis.md` S1 to S5, S8 to S12: stratified regions, the strength table, `specify` delays and timing checks, four-state `LogicValue`, `$display`/file IO, `$discontinuity`, `@initial` seeds, `initial_step`/`final_step`, `$simprobe`, the seq/comb contract.

- **A (recommended). Remove all of it in the first sub-feature**, with the tests that cover only it. No consumer compiles, MSL has no construct for any of it, and the new kernel replaces `digital/`.
- B. Keep it beside the new kernel until codegen is rebuilt. Two digital engines in a crate that must be minimal.

This deletes passing-in-principle tests (`vams_stratified_regions`, `digital_driver_segregation`, `specify_timing_check_enforcement`, `scheduler_region_queue_parity`, parts of `digital_topology`, `mixed_signal`). The skill's rule is that a test is never deleted without the user's word, so this needs an explicit yes.

### Q4. Remove the constraint, margin and coverage channels?

S6: `ValidationReport`, `MarginReport`, `CoverageReport`, `CheckPosture`, `RequireScope`, `evaluate_constraints` and the rest (the dv-core and dv-tester work).

- **A (recommended). Remove.** In MSL `require`, `cover` and `target` are stdlib functions that emit records on `checks`, and the host counts them (§19, §22.6, §23.2). The solver needs one record sink (SCH-05), not three report channels.
- B. Keep them as the transport under the `checks` stream. Faster for the host; the solver then knows what a margin is.

### Q5. What happens to the analyses the spec calls host procedures?

§16.3: transfer function, poles and zeros, S-parameters, DC sensitivity, distortion and periodic steady state are built by the host from the four solves.

- **A (recommended). Delete `disto`; keep `tf`, `pz`, `sp`, `sens`, `adjoint`, `pss` in the solver for now**, marked as host procedures to move when `piperine-api` is rebuilt. `disto` needs second and third derivative kernels in the `Element` ABI, overruns Cranelift on MOS2/MOS3, and the spec defines distortion as a transient plus Fourier. The others need the assembled linearization and re-entry internals, and no host crate builds today to receive them.
- B. Move all of them out now behind a small "linearization" API. Truly minimal solver; cannot be verified until a host crate compiles.
- C. Keep everything, `disto` included.

### Q6. The five test targets that depend on `piperine-codegen`

`adjoint_dc`, `adjoint_vs_sens`, `pss`, `sens`, `transient_reentry` build their circuits by compiling Verilog-A. They are why no solver test runs.

- **A (recommended). Rewrite them against hand-written Rust `Element`s with the same assertions and drop both dev-dependencies.** The solver is then testable alone, for good.
- B. Gate them behind a feature until codegen exists again. The repo's hygiene guard forbids switched-off tests, and the coverage is lost meanwhile.

### Q7. The jump solve: reference algorithm or something else?

- **A (recommended). Appendix C.2 as written**: backward Euler with `h` equal to one tick, time not advanced, started from the left value, repeated once, `1/h` rows scaled by `h`. It reuses the companion-model path with a third integration phase.
- B. A dedicated consistent-initialization solve (charge-conservation constraints assembled explicitly). Exact in the limit; needs a new device contract for states.

### Q8. Who decides which scheduled times are time points (§17.2.2)?

- **A (recommended). The elaborator computes the static reachability and gives the solver one flag per reaction and per carrier ("touches analog"); the kernel applies the dynamic test** at `(t, 0)`. The solver has no dependency graph and should not grow one.
- B. The solver derives reachability from the read and write sets elements declare. Self-contained; duplicates analysis the elaborator has already done.

### Q9. The residual test's tolerances (analysis row 42)

Today `Tolerances::residual_test` checks a node row against `vntol` (1 µV) and a branch row against `abstol` (1 pA). §17.1.2 says the reverse: a node row is a flow imbalance and uses the flow `abstol`.

- **A (recommended). Follow the spec**, with tolerances per row given at build time. This tightens node rows by six orders of magnitude and will move iteration counts; the ngspice-parity numbers were tuned on the present behavior.
- B. Keep the present mapping and record a deviation from §17.1.2.

### Q10. Should the solver depend on `piperine-ir`?

It uses one function, `predict_crossing` (a secant step).

- **A (recommended). Drop the dependency** and keep the 20 lines in the solver's crossing locator. The solver becomes a leaf crate, which is what "core" means in a workspace where everything else is being rebuilt.
- B. Keep it; `piperine-ir` may become the AIR v2 of Appendix C.1.

### Scope confirmations

- **Split.** Six ordered sub-features: `solver-baseline`, `solver-instants`, `solver-time-point`, `solver-crossings`, `solver-host-seam`, `solver-numerics` (`analysis.md` section 6). Recommended: approve the umbrella spec once, then design and task each sub-feature in order. Is that the pace you want?
- **Deferred.** Trajectory sensitivities across events (§23.3) and periodic small-signal (§16.3, G.37) are left out. Recommended: yes, both after a design runs end to end.

---

## Implementation Decisions

None yet. Each answer is written here, in the spec's Assumptions table, and when it sets a project rule, as a decision in `.specs/STATE.md`.

### Agent's Discretion

None granted yet.

### Declined / Undiscussed Gray Areas → Assumptions

None yet. The spec's Assumptions table already carries the recommended default of every question above, marked unconfirmed.

---

## Specific References

- User, 2026-11-14: "o solver tem que ser mínimo, tem que ser o core do negócio"; "ver o que falta desenvolver no solver para depois só conectar um ao outro".
- Appendix C.2 of `docs/spec/` is the reference algorithm for every numeric choice; C.2.3 is the conformance set.

---

## Deferred Ideas

- Trajectory sensitivities and the saltation correction (§23.3).
- Periodic small-signal and periodic noise (§16.3).
- The C interface of `extern "cosim"` (G.65), once the spec fixes it.
- Moving `tf`, `pz`, `sp`, `sens`, `adjoint`, `pss` into the rebuilt host crate.
- A fused or compiled discrete kernel for speed (the old digital-JIT notes); the kernel of this feature is the reference, not the fast path.

---

## User answers (2026-11-14, discussion round 1)

**Vision (user's words):** "Eu estava pensando em um refactor cuidadoso. Eu queria que o core digital funcionasse de algumas maneiras: 1. A fronteira é via eventos, sempre... Isso conecta de maneira bonita com o analog e mixed signal. 2. Todo o path depois do evento de borda é feito estilo Verilator, onde a propagação é executada em DAG, até convergir ou falhar na conversão. 3. Separar em uma fronteira BEM delimitada o que ocorre no analog e o que ocorre no digital, praticamente como dois solvers distintos que rodam no mesmo solver."

| Q | User's words | Reading |
| - | ------------ | ------- |
| Q1 | "De acordo, pode ser modelado como "solver" o dono dos eventos e dispatch para sub solvers analogicos e digitais. Se quiser dividir em 3 crates, o dono dos eventos, o digital-solver e o analog-solver cada um com seu código exclusivo." | A, with the event owner dispatching to an analog and a digital sub-solver; 3 crates allowed |
| Q2 | "Prefiro bytes também. Solver é o core do negócio e tem que ser mais simples, não ter a complexidade da linguagem e sim a complexidade física." | A, opaque bytes |
| Q8 | "Na verdade o solver analógico roda em todo timestep... Como natureza contínua, se existir qualquer código analog, tem que rodar ele." | analog solves at every analog timestep whenever analog code exists (to confirm: which digital events cut an analog step) |
| Q3 | "Ok, o que precisar apagar, pode apagar, aproveitando que estamos fazendo um makeover dele..." | A, explicit yes to delete the VAMS surface and its tests |
| Q4 | "Concordo e na verdade o sink de eventos é bem importante para debug, etc..." | A; the event/record sink is a first-class debug surface |
| Q5 | "Sim, por enquanto beleza." | A |
| Q10 | "Vamos remover o IR, cranelift e Python por agora. Vamos deixar apenas a API testável em RUST, solver, linguagem, apenas o core. Pode ser?" | drop `piperine-ir`; remove IR, Cranelift (codegen) and Python crates; keep only the core: language, solver, testable Rust API (to confirm the exact crate list) |
| Q6 | "Fio, verilog vai embora." | A: rewrite the 5 targets on hand-written `Element`s; Verilog-A goes |
| Q7 | "Companion model é uma boa mas eu manteria o algoritmo atual do solver que funcionou bem nos testes, inclusive cross validation e convergence com descontinuidades (caracteristico de sistemas mixed signal)." | keep the solver's present discontinuity algorithm; Appendix C.2 is amended to describe it, not the reverse |
| Q9 | "Não, a paridade com ngspice é um strenght bem alto e um ponto positivissimo." | B: keep the present tolerance mapping (ngspice parity); §17.1.2 is amended to match |
| ABI | "Ah, fica a vontade para mudar o ABI também ok?" | the `Element` ABI may change freely in this refactor |

## User answers (2026-11-14, discussion round 2)

User's words: "lang-server é bom ficar... cli também mas ambos vão ter sua própria feature. plugin pro enquanto vai embora. api aceito. Q8. Ok Q10. respondido antes. Q9. mantém Q7 e Q9. Ok . Core cleanup + solver-split poderia ser uma feature só pois é refactor de estrtura. Digital dag + analog split uma feature separada pois quero que mexer no core core (mudança de logica) seja implementada por opus e verificada com fable. Sobre testes, por enquanto vamos ter que nos contentar com unitários pois integrado só vai existir depois que conectarmos o solver ao lang (e vamos planejar isso com cuidado pois precisamos conversar sobre codegen e afins)"

| Topic | Decision |
| ----- | -------- |
| Crates kept | `piperine-lang`, `piperine-project`, solver crates, `piperine-api` (reduced to a solver-backed surface: `Session` over Rust-built circuits, results, waveform, units), `piperine-cli` and `piperine-lang-server` (stay in the tree; each gets its own later feature) |
| Crates removed | `piperine-ir`, `piperine-codegen`, `piperine-python`, `piperine-plugin`, `piperine-plugin-macros`; the root package's VAMS e2e tests and `examples/*.vams` |
| Q8 | a digital event cuts an analog step only when it crosses the boundary (changes something analog reads); purely digital activity never forces an analog solve |
| Q7 / Q9 | the solver's present discontinuity algorithm and tolerance mapping stay; spec Appendix C.2 and §17.1.2 are amended to describe them |
| Feature split | (A) structure refactor = core cleanup + solver split into three crates (event owner / analog / digital), no logic change - Sonnet implements, Opus verifies. (B) logic change = digital DAG (Verilator-style) + analog/digital boundary split - **Opus implements, Fable verifies**. Then time-point, crossings, host seam, numerics. |
| Tests | unit tests only for now; integration (and ngspice parity runs) come after solver↔lang connection, which will be planned separately (codegen discussion pending) |

## User answers (2026-11-14, round 3)

| Topic | User's choice | Effect |
| ----- | ------------- | ------ |
| solver-restructure approval | "Approve and run (Recommended)" | approved; Sonnet implements, Opus verifies; merge + push at the end |
| `tests/suite_hygiene.rs`, `examples/*.py` + plots | "Move guard, delete py (Recommended)" | the hygiene guard moves into a crate that builds (piperine-lang tests) and keeps enforcing; `examples/*.py` and plots are deleted |
| §15.2 instantaneous jump vs the kept algorithm | "Amend §15.2 to the solver (Recommended)" | the logic feature amends §15.2 (with C.2 and §17.1.2) to describe the kept algorithm |

## User answers (2026-11-15, through the coordinator)

| Topic | Decision | Effect |
| ----- | -------- | ------ |
| Goal for the day | "a ideia é sair hoje ainda com ABI feito e reestruturação do solver (+ logica do dag eval + analog) prontos" | the plans are lean; the port of the tests is the long pole |
| The element contract | the device ABI (`.specs/features/device-abi/abi.md`); amendments 1, 2, 3a, 4, 6, 8, 9 accepted, 3b not now, 5 and 7 withdrawn | `Element` is replaced; feature B (`solver-event-boundary`) is replaced by the `solver-abi-*` features |
| Split | `spec-abi-amendments` (docs); `solver-abi-core` (the crate `piperine-abi` and the owner protocol); `solver-abi-digital` and `solver-abi-analog` in parallel in separate worktrees, neither touching the other's crate; `solver-abi-integration` (the owner wires both, test devices and mixed tests ported) | five feature directories; see the umbrella's "After the device ABI" |
| Model roles | Sonnet implements and Opus verifies by default; a feature that is delicate and not large is implemented by Opus and verified by a different Opus; the orchestrator decides | supersedes the round-2 row "Opus implements, Fable verifies" above |

## Orchestrator decisions under the user's go-ahead (2026-11-15)

User: "Ok, deixo tu ir tocando, pode ir disparando implementações em seguida ok?" and "vamos fazer a impl via Sonnet e verifier em Opus mas quando a feature é muito delicada e não grande, pode lançar direto um Opus. Eu deixo a decisão do que é delicado para ti."

| Item | Decision |
| ---- | -------- |
| MD-61: `piperine-abi` sits under all three solver crates | yes - it is the shared contract the ABI defines; amends MD-60's "no in-tree crate under a sub-solver" |
| ~60 tests whose whole subject is deleted (strength table, LogicValue, seq/comb, EventQueue, capability flags, bypass) | removed by name into removed.txt under Q3 ("o que precisar apagar, pode apagar") |
| Roles | amendments: Sonnet/Opus; core: Opus implements (protocol, the contract everything uses); digital: Sonnet/Opus; analog: Opus implements (numeric, small); integration: Sonnet/Opus (large) |

## Orchestrator note (2026-11-15)

User decided the order at a scheduled instant: "ABI order: solve, then react (Recommended)" - reactions at t read the analog solution at t; the kept discontinuity handling (landing exempt, restart, 1e-3) stays.
## User answer (2026-11-15): order at a scheduled instant

User: "ABI order: solve, then react (Recommended)" - reactions at t read the analog solution at t (left value); the kept discontinuity handling stays.
Question: at a scheduled digital instant t, do reactions read the analog value at t (ABI: solve, then react) or the previous time point (today's code: microsteps, then solve)?
User's choice: "ABI order: solve, then react (Recommended)". solver-abi implements the ABI order (left value at t first, then the instant), keeping the kept discontinuity handling (landing exempt, restart, 1e-3); §15 is rewritten back to that order by the solver-abi feature that changes the code. spec-abi-amendments keeps describing today's code with the §15.2 note.
