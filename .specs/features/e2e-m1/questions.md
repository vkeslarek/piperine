# M1: what is the user's to decide

Eight questions, ordered by how much work each answer moves. Each spec is written with the recommended answer, so a "yes" to a recommendation changes nothing, and any other answer names the files to edit. Everything not listed here was closed from code and spec evidence; `.specs/features/codegen/context.md` lists what and where.

## Q1. May `piperine-abi` gain a constructor for a module that carries data?

**Why it is asked.** The generic interpreter device is one Rust type serving every IR body. `Device::prototype()` and `Device::create()` are static functions, and `thunk::module::<D>` sets `Module.data` to null (`crates/piperine-abi/src/thunk.rs:28`). So the `Device` trait cannot express "one type, many modules", although abi.md §3 reserves `data` for exactly this ("an interpreter's IR"). It touches a finished crate and the Rust side of abi.md §7; the `#[repr(C)]` contract does not change.

| Option | Consequence |
| --- | --- |
| **(a) Add `device::Factory` and `thunk::module_with::<F>(name, &'static F)`** (recommended) | about 80 lines in `piperine-abi`, reusing its thunk helpers; `Device` and every existing device stay untouched; `unsafe` stays inside `piperine-abi`; one sentence added to abi.md §7 |
| (b) The interpreter writes its own `extern "C"` thunks | no ABI-crate change, but `unsafe` in `piperine-interp` duplicating `thunk.rs`, against the crate's rule that `unsafe` lives in `raw`, `thunk` and `caller` only |
| (c) One generated Rust type per body | impossible without a compiler at run time; that is M3 |

Blocks: `ir-interp` INT-08, and through it the whole slice.

## Q2. What does a parity test do when ngspice is not installed?

The old harness passed silently. `ci.yml` does not install ngspice today.

| Option | Consequence |
| --- | --- |
| **(a) Fail, with `PIPERINE_NGSPICE=skip` as an explicit opt-out that prints `SKIP`** (recommended) | a green run always means the comparison ran, or someone typed the opt-out; `cargo test` at the root needs ngspice or the variable; CI installs ngspice (one `apt-get` line) |
| (b) Skip silently, as before | nothing to install; a machine without ngspice reports parity it never checked |
| (c) Fail always, no opt-out | strictest; contributors without ngspice cannot run the default suite |

Under every option the MSL side (build, elaborate, solve) still runs and must succeed. Affects: `msl-parity` PAR-04, the CI workflow.

## Q3. Which numbers go in the tolerance contract?

| Option | Consequence |
| --- | --- |
| **(a) The old contract with the current floor corrected: 1e-3 relative everywhere; 1e-6 V; 1e-12 A; AC magnitude floor 1e-12; AC phase 1e-4 rad** (recommended) | the old rationale said "ngspice's abstol for currents" and wrote 1e-9; ngspice's `abstol` is 1e-12, and with `gmin` in the diode model the reverse region agrees far below it |
| (b) The old contract unchanged (current floor 1e-9 A) | every reverse-bias point of `diode_iv` (about 1e-12 A) passes whatever the solver returns |
| (c) Tighter than the solvers' own `reltol` (for example 1e-6 relative) | would hold for the linear fixtures today, and fail for reasons that are not bugs once two Newton loops stop at different iterates |

Affects: `msl-parity` PAR-05.

## Q4. Is the M1 device library a fixture or the start of a library?

`std` has no `Resistor`, `Capacitor`, source or `Diode`: it holds natures, limiters and operators. M1 needs the five models somewhere.

| Option | Consequence |
| --- | --- |
| **(a) A fixture: `tests/ngspice/devices.msl`, beside the twins** (recommended) | nothing is promised to users; the models are as small as the parity needs (no `rs`, no noise, no temperature coefficients) |
| (b) Add them to `std` now | every MSL file sees `Resistor` with no import; it becomes public surface before M2 has exercised params, `Option` and noise on it, and §22 of the specification would need the text |
| (c) A separate `spice` crate now | the right home for ngspice-faithful models later (as the Verilog-A era had), but a package and a dependency path to design before a single model runs |

Affects: `msl-parity` PAR-01. A model library is its own feature after M1 under (a).

## Q5. Which codes carry "not supported yet" and "mismatched types"?

The specification gives every error a code `Eppnn` owned by a part, and has none for a construct the compiler does not handle yet, nor for a plain type mismatch (`let x: bool = 1.0;`): §0.4 leaves that to Rust.

| Option | Consequence |
| --- | --- |
| **(a) Compiler-owned codes outside the spec's space: `P0001` unsupported construct, `P0002` mismatched types** (recommended) | cannot collide with a later spec code; `P0001` disappears as M2 fills the language; `P0002` can become a Part I code by a later spec edit |
| (b) Add `E0106` (type mismatch) to Part I now, keep `P0001` | a spec edit with its `spec_text` guard (MD-57) inside M1 |
| (c) No code on these diagnostics | allowed by §0.3 item 4 ("SHOULD carry the error code"); tests then match on message text |

Affects: `msl-typeck` TYP-02, TYP-05; `msl-elab` ELAB-06.

## Q6. What is the IR crate called?

`piperine-ir` is the name every spec and decision uses, and it is also the name of a retired crate: `crates/piperine-solver/tests/workspace_shape.rs` lists it in `RETIRED_CRATES` and fails if a directory or a manifest names it.

| Option | Consequence |
| --- | --- |
| **(a) `piperine-ir`, and the guard's list drops the name in the same commit** (recommended) | the obvious name; the git history has two unrelated crates under it |
| (b) A new name (for example `piperine-eom`, "elaborated object model") | no guard edit; every document that says "the IR crate" needs the new name explained |

Affects: `msl-ir`, and the dependency lines of three other crates.

## Q7. Does the current source get a sixth twin?

S1 lists voltage and current sources; none of the five named fixtures places a current source.

| Option | Consequence |
| --- | --- |
| **(a) Add `isrc_r.cir` and `isrc_r.msl`: 1 mA into 1 kΩ, operating point** (recommended) | one new six-line fixture; every M1 device is compared with ngspice |
| (b) No twin; `Isource` is checked against Ohm's law in `ir-interp`'s tests | the fixture list stays at five; its sign convention is not compared with ngspice's |
| (c) Drop `Isource` from M1 | one model fewer; S1 said current sources |

Affects: `msl-parity` PAR-08.

## Q8. When is Appendix C amended?

Appendix C.1 (informative) describes lowering to "AIR v2" and says "every function is inlined". S7 keeps functions, and the IR is the object model of `msl-ir`. Nothing normative is contradicted.

| Option | Consequence |
| --- | --- |
| **(a) After M1 runs, in one spec edit together with the pending `spec-abi-amendments`** (recommended) | the appendix is rewritten once, against a lowering that exists and passes parity |
| (b) Now, as a task of `msl-elab` | the appendix would describe M1's subset and be rewritten again in M2 |
| (c) Leave it | an informative appendix that describes a design nobody is building |

Affects: `docs/spec/appendix_c_implementation.md`, the Book.
