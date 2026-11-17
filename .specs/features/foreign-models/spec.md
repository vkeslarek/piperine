# Foreign Models Specification

Opened 2026-11-17 from `GAPS.md` (audit of 2026-11-17) at the user's request: "gostaria que
abrisse feature e escrevesse a spec para todos esses gaps [...] Preciso que TODOS os gaps estejam
endereçados ok? Principalmente esses que tu listou que limitam a expressividade da linguagem ok?"
Spec only: no design, no tasks yet.

## Questions for the user (open)

Each blocks approval. The recommendation is the planner's; nothing below is decided until the
user answers.

| # | Question | Options | Recommendation |
| - | -------- | ------- | -------------- |
| Q1 | Where does the `"osdi"` adapter live? Part III §8.5.8 makes `"osdi"` a language ABI string reached "through an adapter over the device ABI". The user ruled on 2026-07-09 that the device ABI is Piperine's own and never tracks OSDI, and OSDI was extracted to the external repo `~/Git/piperine-osdi`; that repo targets the `Element` ABI that MD-62 deleted, and MD-60 removed the plugin system. | (a) **An in-tree crate `piperine-osdi`** above the solver: it depends on `piperine-abi` only, reads an OSDI 0.4 library and serves it as a device-ABI `Library` (abi.md §11.8); the three solver crates keep no OSDI and no `libloading` dependency. (b) **The external repo**, rebuilt on the device ABI: `#[link("x.osdi")]` is served only when that adapter is installed; without it an `"osdi"` module is `E0372` naming the missing adapter. (c) **Drop `"osdi"` from the language**: §8.5.8's set becomes `"cosim"` alone, `extern "osdi"` is `E0371`, and Verilog-A models reach Piperine only as device libraries compiled elsewhere. | **(a).** It keeps both rulings: the device ABI stays Piperine's (OSDI is one client behind an adapter, MD-11) and the solver core stays OSDI-free. (b) leaves a spec construct that a default install cannot run; (c) removes a stated purpose of the language (`docs/spec/index.md` item 2) |
| Q2 | How is the native symbol of an `extern "C" fn` found? §3 leaves it to "the toolchain"; MD-63 item 1 says generated code links `piperine-abi` "and nothing else". | (a) **A manifest table** `[native]` in `Piperine.toml` (`libs = ["m", "path/libfoo.so"]`, paths relative to the package), whose libraries `rustc` links into the device library; the C math library is always linked. Amends §3.6 (the closed key list of `E0171`) and MD-63 item 1. (b) **`#[link("file")]` on the `extern "C" fn`**, as on an extern module; amends Appendix A.1's attribute table. (c) **The C math library only**: no user library, no manifest key; any other symbol is refused | **(a).** The source stays free of paths, as Cargo keeps them in the manifest; one key serves every function of a package; (c) cannot reach the vendor code §3 rule 10 is for |
| Q3 | Is an `extern "C" fn` callable in a const context (a param default, a `where`, a `const`)? §3 makes it pure, and a pure function is callable "anywhere". | (a) **No**: refused with `E0122`; §3's class table gains "an `extern` function is not callable in a const context". The front end never runs native code. (b) **Yes**: the elaborator loads the native library and calls the symbol at elaboration; `piperine-lang` gains a native-call dependency | **(a).** Elaboration stays a pure function of the source (§0.3, C7); a foreign value at elaboration is rare and a host `set` serves it |

---

## Problem Statement

A model cannot reuse code it did not write. `extern "C" fn` and `#[derivative]` (Part I §3 rules
2, 10, 11) are `P0001: \`extern\` function` (`types/check.rs:399`); `extern "osdi"` and
`extern "cosim"` modules with `#[link]` (Part III §8.5.7 to §8.5.16) are `P0001: \`extern\`
module` (`types/check.rs:491`). Six of the spec's error codes for them (`E0123`, `E0124`, `E0371`
to `E0374`) are raised nowhere (`GAPS.md` §10). Part III §8.5 is 0 % covered (`GAPS.md` §9).

The pieces exist below the language. The device ABI already has an `IRREVERSIBLE` reaction flag
(abi.md R6, `piperine-abi/src/raw.rs:771`) that the digital sub-solver honours
(`piperine-digital-solver/src/run.rs:628`), a `RESHAPE` status (abi.md §9), and a sketch of an
OSDI adapter (abi.md §11.8). `piperine-transpile` already opens a library that exports
`piperine_abi_1` (`load.rs:85`). What is missing is the language path: checking the declaration
against the foreign artifact, placing the instance, and the per-ABI contracts of §8.5.

## Goals

- [ ] A model calls a C function, in any block, and inside `analog` with the derivatives it names; the Jacobian it gives passes the ABI's partials verifier.
- [ ] A model places an `extern "cosim"` module whose library exports `piperine_abi_1`, and the foreign step runs once per instant that needs it, never in a step that is later rejected.
- [ ] A model places an `extern "osdi"` module, an OSDI 0.4 compiled Verilog-A model, through an adapter over the device ABI, and its operating point agrees with the same device written in MSL under the parity contract.
- [ ] Every error code of §3 and §8.5 for these forms (`E0123`, `E0124`, `E0371` to `E0374`) is raised where the spec says.

## Out of Scope

| Feature | Reason |
| ------- | ------ |
| `m` and `dtemp` bound at a placement and read (L29) | `generics` owns L29. This feature passes the effective values to a foreign model once they exist (FM-13) |
| Templating an extern module, `Fn(P)` bounds over it (§8.5.12, L32) | `generics` |
| Metadata operations that list a foreign model's params, opvars, noise sources and internal nodes (§21 rule 9, H4) | `host-ii` owns the metadata surface; FM-12 gives it the names |
| A host `set` of a structural param (`E0803`) in general | `host-ii`; FM-14 covers the extern-module case only |
| The interpreter backend | Frozen at the M1 analog subset (R1, 2026-11-15): it refuses `extern` items by name (FM-15) |
| Python, the language server | `python-host`, `lang-server` |
| Compiling Verilog-A to OSDI | An external compiler's job (OpenVAF); the toolchain loads its output |
| `extern "intrinsic" fn` | The stdlib's own mechanism (§3 rule 10); each intrinsic belongs to the feature of its stdlib item |
| A2D/D2A converters in front of a foreign model with analog pins (§8.5.15) | They are stdlib modules that already run; nothing foreign is involved |

---

## Assumptions & Open Questions

| Assumption / decision | Chosen default | Rationale | Confirmed? |
| --------------------- | -------------- | --------- | ---------- |
| The artifact of `"cosim"` | A shared library exporting `piperine_abi_1` (§8.5.8), loaded by the same loader as a transpiled design | §8.5.8 and G.65 say so; one loader | y - spec |
| `#[link]` without `model` on a library that holds several modules | `E0372` naming the modules the library holds | §8.5.9: `model` names the model "when the artifact holds several" | n - planner |
| How a `"cosim"` instance meets "never rolled back" | Its reaction is flagged `IRREVERSIBLE` (abi.md R6): the owner accepts the analog solution up to the instant before running it | The ABI's own mechanism, already honoured by the digital sub-solver | n - planner |
| What a `"cosim"` model is given at a microstep | Its `in` ports' values; it returns commits to its `out` ports, applied at the next microstep, and the next time it wants to run | §8.5 contract table | y - spec |
| OSDI fixtures | (1) a hand-written OSDI 0.4 descriptor in a test crate (a linear resistor and a diode with one internal node), built by the test, so no external tool is needed; (2) an OpenVAF-compiled diode, run only when `openvaf` is on `PATH`, else the harness prints its skip line as the ngspice harness does | Deterministic tests on every machine; one real compiled model where the tool exists | n - planner |
| What is checked against the OSDI descriptor | Terminal count and order; each listed param exists with a compatible type (`f64` for a real, an integer type for an integer, `&str` is refused, `E0373`) | §8.5.10 | y - spec |
| The parity reference for `"osdi"` | The MSL diode of `tests/ngspice/devices.msl` and its ngspice twin, under the unchanged `ToleranceContract::DC_VOLTAGE` and `DC_CURRENT` | MD-38: parity is measured | n - planner |
| Where an `extern "C" fn` may be called | Every position its class allows except a const context, pending Q3 | §3's class table | n - planner (Q3) |
| A native symbol that no linked library defines | A build failure with a new spec code `E0128` naming the function and the libraries searched; §3's error table gains the row | §3 leaves location to the toolchain; a failure still needs a code (MD-24's "no silent fallback") | n - planner |
| The derivative functions | Any function of the right signature, MSL or `extern "C"` | §3 rule 11 says "names one function", not an `extern` one | y - spec |
| How a derivative is used | The chain rule: the partial of `f(g1, …, gk)` against an unknown is `Σ f_i(g) · ∂g_i`; `f_i` is the `i`-th named function | §3 rule 1: calls inside `analog` are differentiated through | y - spec |

**Open questions:** Q1, Q2 and Q3 at the top of this file wait for the user.

**Implicit-requirement dimensions.** Input validation: FM-04, FM-07, FM-08 (every refusal of
§3 and §8.5 with its code). Failure states: FM-03 AC 3 (a missing symbol), FM-07 AC 3 (an
unloadable library), FM-11 AC 4 (a model that faults). Concurrency and ordering: FM-09 (a
foreign step at most once per instant, never undone). State-transition integrity: FM-14 (a
reshape is refused during a run). External-dependency failure: FM-07 AC 3 and FM-16 AC 3 (the
foreign artifact or the OpenVAF tool absent). Idempotency, auth, rate limits, data lifecycle:
N/A because the toolchain is local and a run is a function of its inputs (§15 I1).
Observability: FM-12 (a foreign model's names reach the metadata).

---

## User Stories

### P1: A model calls a C function ⭐ MVP

**User Story**: As a model author, I want to call a C function from MSL, with its derivatives
inside `analog`, so that I can reuse a vendor's or a library's numerics without porting them.

**Why P1**: L12. It is the smallest foreign form and the one `#[derivative]` exists for.

**FM-01. A call of an `extern "C" fn` runs the native code.**

**Acceptance Criteria**:
1. WHEN a reaction, a body `let`, a definition or an `analog` block calls an `extern "C" fn` whose symbol a linked library defines THEN the compiled device SHALL call that symbol with the arguments and use its result.
2. WHEN the twin `extern_exp` (the diode of `tests/ngspice/devices.msl` with `exp` declared `#[derivative(exp)] extern "C" fn exp(x: f64) -> f64;` from the C math library) runs `op` THEN its node potentials SHALL agree with the MSL diode twin's under `ToleranceContract::DC_VOLTAGE`.
3. WHEN an `extern "C" fn` takes or returns `bool` or an integer type THEN the call SHALL pass the value as the C type of that width (`bool` as `bool`, `UInt<N>`/`Int<N>` as the smallest of 8, 16, 32, 64 bits that holds `N`).

**FM-02. Unknown arguments.**

**Acceptance Criteria**:
1. WHEN any level of any argument of an `extern "C" fn` call is unknown THEN the system SHALL give the call an unknown result and SHALL NOT call the symbol (§3 rule 15, §3.3.2).

**FM-03. Linking.**

**Acceptance Criteria**:
1. WHERE Q2 is answered (a) the system SHALL link every library named by the package's `[native]` table, and the C math library, into the device library of a design that calls an `extern "C" fn`.
2. WHEN a design calls no `extern "C" fn` THEN the system SHALL link its device library against `piperine-abi` alone, as MD-63 item 1 says today.
3. IF no linked library defines the symbol of a called `extern "C" fn` THEN the system SHALL fail the build with `E0128` naming the function and each library searched.

**FM-04. `#[derivative]` and the checks of §3.**

**Acceptance Criteria**:
1. IF an `extern` function without `#[derivative]` is called inside `analog` THEN the system SHALL report `E0123` naming the function and the call.
2. IF `#[derivative]` names a number of functions other than the number of `f64` parameters of its `extern` function THEN the system SHALL report `E0124` naming both counts.
3. IF a function named by `#[derivative]` does not have the signature of its `extern` function THEN the system SHALL report `E0124` naming that function and the parameter or result that differs.
4. WHEN an `extern "C" fn` with `#[derivative(f1, …, fk)]` is called inside `analog` THEN the Jacobian entry of the call against each unknown SHALL be the sum over `i` of `fi(args)` times the partial of the `i`-th argument against that unknown.
5. WHEN the device of FM-01 AC 2 is checked by the ABI's partials verifier THEN the verifier SHALL pass.
6. WHERE Q3 is answered (a), IF an `extern "C" fn` is called in a const context THEN the system SHALL report `E0122` naming the function and the const context.

**Independent Test**: A `piperine-lang` test per code; a `piperine-transpile` test that builds a
design calling `exp` and `hypot` from the C math library and compares each record with the MSL
`f64` method; the `extern_exp` twin through `piperine-parity`.

---

### P1: A model places a co-simulated digital model ⭐ MVP

**User Story**: As a mixed-signal designer, I want to place an RTL model compiled elsewhere as
an `extern "cosim"` module, so that a block I already have in another simulator runs inside my
design.

**Why P1**: L33, the `"cosim"` half. The ABI already carries everything it needs
(`IRREVERSIBLE`, `HORIZON`), so it is the cheaper half of L33.

**FM-05. Elaborating an `extern "cosim"` module.**

**Acceptance Criteria**:
1. WHEN a design places `#[link("lib.so", model = "M")] extern "cosim" module N(ports) { params }` and the library exports `piperine_abi_1` with a module `M` whose shape matches THEN the system SHALL elaborate the placement into an instance of that foreign module.
2. WHEN `#[link]` gives no `model` and the library holds exactly one module THEN the system SHALL use that module.
3. The system SHALL resolve a relative `#[link]` path against the directory of the source file that holds the `#[link]`.

**FM-06. Running an `extern "cosim"` instance.**

**Acceptance Criteria**:
1. WHEN an `in` port of a `"cosim"` instance changes at microstep `(t, n)` THEN the system SHALL run the foreign step once at `(t, n)` with the values of its `in` ports.
2. WHEN the foreign step returns commits THEN the system SHALL apply them to its `out` ports at `(t, n + 1)`.
3. WHEN the foreign step returns a time it wants to run at THEN the system SHALL schedule that time (§17.2) and run the step there with the inputs of that instant.
4. IF any level of an `in` port is unknown at a microstep where the step would run THEN the system SHALL NOT run the foreign step and SHALL commit `Unknown` to every `out` port (§8.5.16).
5. WHILE an analysis does not advance time the system SHALL never run a step at a time the model asked for (§8.5.14).

**FM-07. Checking a `"cosim"` declaration against its library.**

**Acceptance Criteria**:
1. IF an extern module's ABI string is other than `"osdi"` or `"cosim"` THEN the system SHALL report `E0371` naming the string.
2. IF an extern module has no `#[link]` THEN the system SHALL report `E0372` naming the module.
3. IF the `#[link]` file cannot be opened, exports no `piperine_abi_1`, holds no module named by `model`, or holds several modules and no `model` is given THEN the system SHALL report `E0372` naming the resolved path and the cause.
4. IF the declared ports differ from the foreign module's port pins in number, order, direction or carried type THEN the system SHALL report `E0373` naming the first port that differs.
5. IF a declared param does not exist in the foreign module, or its type differs THEN the system SHALL report `E0373` naming the param.
6. IF a `"cosim"` module declares a port that is not a carrier of a one-driver discrete nature with a direction THEN the system SHALL report `E0374` naming the port.

**FM-08. A foreign step is never undone.**

**Acceptance Criteria**:
1. The system SHALL flag every reaction of a `"cosim"` instance `IRREVERSIBLE`.
2. WHEN a transient with a `"cosim"` instance rejects analog steps THEN the foreign step SHALL be called exactly once per instant at which FM-06 runs it, counted by a test library that records its calls, and never at an instant later undone.

**FM-09. A param a `"cosim"` body does not list.**

**Acceptance Criteria**:
1. WHEN the foreign module has a param that the body does not list THEN the system SHALL pass the foreign default for it.

**Independent Test**: A test crate builds a small `"cosim"` library in Rust on `piperine-abi`
(a UART receiver: `in clk`, `in rx`, `out byte`, `out dv`, as in §8.5's example) that counts its
calls; a design places it next to an analog RC whose crossings force rejected steps; the
received bytes, the `dv` pulses and the call count are asserted.

---

### P2: A model places a compiled Verilog-A model

**User Story**: As an analog designer, I want to place a Verilog-A model compiled to OSDI 0.4,
such as BSIM4, as an `extern "osdi"` module, so that I can use foundry models MSL does not
ship.

**Why P2**: L33, the `"osdi"` half. It depends on Q1 and is the larger half: collapse of
internal nodes, limiting through simulator callbacks, noise and operating-point variables.

**FM-10. Elaborating an `extern "osdi"` module.**

**Acceptance Criteria**:
1. WHERE Q1 is answered (a) or (b), WHEN a design places an `extern "osdi"` module whose `#[link]` file is an OSDI 0.4 library holding the `model` named THEN the system SHALL elaborate it into an instance of the adapter's device-ABI module for that model (abi.md §11.8).
2. IF the OSDI descriptor's terminals differ from the declared ports in number THEN the system SHALL report `E0373` naming both counts.
3. IF a declared param is not in the descriptor, or has an incompatible type THEN the system SHALL report `E0373` naming the param.
4. IF an `"osdi"` module declares a port that is not a node of a continuous nature THEN the system SHALL report `E0374` naming the port.
5. IF the file is not an OSDI 0.4 library THEN the system SHALL report `E0372` naming the resolved path and the version found.
6. WHERE Q1 is answered (c), IF a module is declared `extern "osdi"` THEN the system SHALL report `E0371`.

**FM-11. Running an `extern "osdi"` instance.**

**Acceptance Criteria**:
1. WHEN the hand-written OSDI resistor (`r` = 1 kΩ) is placed in the divider of `tests/ngspice/divider.cir` THEN `op` SHALL give the potentials of the MSL divider twin to the bit.
2. WHERE `openvaf` is on `PATH`, WHEN the OpenVAF-compiled diode replaces the MSL diode in the twins `diode_series` and `diode_iv` THEN `op` and the DC sweep SHALL agree with the ngspice goldens under `ToleranceContract::DC_VOLTAGE` and `DC_CURRENT`.
3. WHEN an OSDI model limits a quantity through the simulator callbacks THEN the system SHALL hold convergence while the limiting is active (§18 rule 9).
4. IF the OSDI model's `eval` or `setup` reports an error THEN the system SHALL fail the analysis with a `FAULT` naming the instance and the model's message.
5. WHEN a transient or `ac` runs a design with an `"osdi"` instance THEN the instance SHALL be evaluated in every solve as an `analog` block is (§8.5.13), its reactive parts as `DDT` terms.
6. WHEN the OSDI model declares noise sources THEN the system SHALL give them to the noise analysis as `NOISE` inputs.

**FM-12. Foreign names in the metadata.**

**Acceptance Criteria**:
1. WHEN a host asks a session for the metadata of an `"osdi"` instance THEN the system SHALL list its params, operating-point variables, noise sources and internal nodes under the descriptor's names (§21 rule 9).

**FM-13. `m` and `dtemp` for a foreign model.**

**Acceptance Criteria**:
1. WHEN an `"osdi"` instance is created THEN the system SHALL pass its effective multiplicity as the descriptor's multiplicity and the analysis temperature plus its effective `dtemp` as its temperature.
2. WHILE `m` and `dtemp` cannot be bound at a placement (L29, `generics`) the system SHALL pass a multiplicity of 1 and an offset of 0.

**FM-14. A change of structure.**

**Acceptance Criteria**:
1. IF a host `set` during a run gives an extern instance param values for which its `setup` returns `RESHAPE` THEN the system SHALL refuse the set with `E0803` naming the instance and the param, and change nothing.
2. WHEN a point of a `Dc` sweep gives such values THEN the system SHALL re-create that instance with its new shape for that point (§8.5.10).

**Independent Test**: The hand-written OSDI test crate (resistor, diode with an internal node and
`$limit`), built by the test; the divider and diode twins through `piperine-parity`.

---

### P2: The backends and the documents say what holds

**User Story**: As a user, I want every backend to say what it does with a foreign form, and the
spec to say how a symbol is found.

**Why P2**: Without it the interpreter's refusal and the toolchain rules are unwritten.

**FM-15. The interpreter.**

**Acceptance Criteria**:
1. IF a design run on the interpreter backend calls an `extern "C" fn` or places an extern module THEN the system SHALL refuse it naming the form and the transpiled backend.

**FM-16. Spec text and records.**

**Acceptance Criteria**:
1. WHEN this feature merges THEN Part I §3 SHALL state how an `extern "C" fn` symbol is found (Q2) and give the `E0128` row, and Part III §8.5 SHALL state where the `"osdi"` adapter lives (Q1), each with an assertion in `crates/piperine-lang/tests/spec_text.rs` (MD-57).
2. WHEN this feature merges THEN `GAPS.md` SHALL no longer list L12 and L33, and `.specs/STATE.md` SHALL record the answers to Q1 and Q2 as one `MD-xx` decision.
3. IF the OSDI parity test cannot run because `openvaf` is absent THEN the test SHALL print one skip line naming the tool and pass, as the ngspice harness does.

**Independent Test**: `spec_text.rs`, `crates/piperine-cli/tests/docs.rs`, and an interpreter
test per refused form.

---

## Edge Cases

- WHEN two `extern "C" fn` items in two crates name the same symbol THEN the system SHALL call the one symbol for both.
- IF a `"cosim"` step returns a requested time earlier than the current instant THEN the system SHALL fail the run with a `FAULT` naming the instance and both times.
- IF a `"cosim"` step returns a requested time outside the tick range THEN the system SHALL fail the run with `E0502` naming the instance.
- WHEN a `"cosim"` instance has no `in` port THEN the system SHALL run its step at time 0 and at each time it asks for.
- WHEN an `extern "C" fn` returns NaN inside `analog` THEN the system SHALL fail the solve with `E0507` (`solver-hygiene`) naming the instance and the contribution.
- IF the same OSDI library is linked by two extern modules THEN the system SHALL load it once.

---

## Gaps closed

| GAPS id or code | Closed by | How |
| --------------- | --------- | --- |
| L12 `extern "C" fn` | FM-01, FM-02, FM-03, FM-15 | runs on the transpiled backend; refused by name on the frozen interpreter |
| L12 `#[derivative]` | FM-04 | the chain rule through the named functions |
| L33 `extern` modules, `"cosim"` | FM-05 to FM-09 | elaborated, checked against the library, irreversible reactions |
| L33 `extern` modules, `"osdi"` | FM-10 to FM-14 | an adapter over the device ABI, placed where Q1 says; under Q1 (c), `E0371` with the spec amended |
| L33 `#[link]` | FM-05 AC 3, FM-07 AC 2 and 3 | required, path relative to the source file |
| `E0123` | FM-04 AC 1 | raised |
| `E0124` | FM-04 AC 2 and 3 | raised |
| `E0371` | FM-07 AC 1, FM-10 AC 6 | raised |
| `E0372` | FM-07 AC 2 and 3, FM-10 AC 5 | raised |
| `E0373` | FM-07 AC 4 and 5, FM-10 AC 2 and 3 | raised |
| `E0374` | FM-07 AC 6, FM-10 AC 4 | raised |

`E0122` for a const-context call (FM-04 AC 6) is raised here for `extern` functions only; the
code's general case is `params-and-io` PI-12.

---

## Requirement Traceability

| Requirement ID | Story | Phase | Status |
| -------------- | ----- | ----- | ------ |
| FM-01 | P1: a model calls a C function | - | Pending |
| FM-02 | P1: a model calls a C function | - | Pending |
| FM-03 | P1: a model calls a C function | - | Pending |
| FM-04 | P1: a model calls a C function | - | Pending |
| FM-05 | P1: a co-simulated digital model | - | Pending |
| FM-06 | P1: a co-simulated digital model | - | Pending |
| FM-07 | P1: a co-simulated digital model | - | Pending |
| FM-08 | P1: a co-simulated digital model | - | Pending |
| FM-09 | P1: a co-simulated digital model | - | Pending |
| FM-10 | P2: a compiled Verilog-A model | - | Pending |
| FM-11 | P2: a compiled Verilog-A model | - | Pending |
| FM-12 | P2: a compiled Verilog-A model | - | Pending |
| FM-13 | P2: a compiled Verilog-A model | - | Pending |
| FM-14 | P2: a compiled Verilog-A model | - | Pending |
| FM-15 | P2: backends and documents | - | Pending |
| FM-16 | P2: backends and documents | - | Pending |

**Coverage:** 16 total, 0 mapped to tasks (spec only).

---

## Dependencies and size

- **Depends on:** `msl-fixes` (in flight) for `E0141`/`E0140` at calls; `solver-hygiene` (in
  flight) for `E0507` on a non-finite foreign result; `host-ii` (in flight) for the metadata
  surface (FM-12) and `E0803` (FM-14); `generics` for `m`, `dtemp` and templating (FM-13 AC 1).
  P1 needs none of the not-yet-started features.
- **Touches:** `piperine-lang` (`types/check.rs`, elaboration of extern items), `piperine-ir`
  (an extern call node, a foreign instance), `piperine-plan`, `piperine-transpile` (linking,
  loading), `piperine-project` (`[native]` under Q2 (a)), a new `piperine-osdi` crate under Q1
  (a), `docs/spec/` Part I §3, §3.6, Part III §8.5.
- **Size:** L. About 35 tasks: P1 `extern "C" fn` about 10, P1 `"cosim"` about 10, P2 `"osdi"`
  about 15.

## Sources

- `GAPS.md` L12, L33, §9 (III §8.4 to §8.5: 0 %), §10 (codes absent from the source).
- `docs/spec/part_i_values.md` §3 rules 2, 10, 11, 15 and its errors; `part_iii_structure.md`
  §8.5.7 to §8.5.16 and its errors; `appendix_a_lexical.md` A.1 (attributes);
  `appendix_g_scope.md` G.65.
- `.specs/features/device-abi/abi.md` R6, §9, §11.7, §11.8.
- `.specs/STATE.md` MD-11, MD-60 item 5, MD-62, MD-63; the user's decision of 2026-07-09 that the
  device ABI never tracks OSDI and that OSDI lives outside the solver core.

## Success Criteria

- [ ] `extern_exp` agrees with the MSL diode twin, and the partials verifier passes on it.
- [ ] A `"cosim"` UART receiver decodes its bytes in a mixed run with rejected steps, called once per instant.
- [ ] An OSDI resistor reproduces the divider to the bit; an OpenVAF diode agrees with ngspice where the tool exists.
- [ ] `E0123`, `E0124`, `E0371` to `E0374` each have a test that raises them.
